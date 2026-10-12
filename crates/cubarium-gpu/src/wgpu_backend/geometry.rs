//! Safe geometry passes. CPU packing is shared with the original presenter.
#![forbid(unsafe_code)]
use super::upload::{self, Upload};
use super::{
    Gpu,
    shader::{Shader, Specialization},
};
use crate::{
    figures::{self, FigureAtlasStats, FigureFrame, FigureVoxel, Packed},
    plants::{self, PlantList, PlantMotion, PlantStats},
};
use anyhow::Result;
use std::collections::VecDeque;

pub struct Plane {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}
impl Plane {
    fn new(gpu: &Gpu, name: &str, size: (u32, u32), format: wgpu::TextureFormat) -> Self {
        let texture = gpu.texture(
            name,
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            wgpu::TextureDimension::D2,
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let view = texture.create_view(&Default::default());
        Self { texture, view }
    }
}
pub(crate) fn upload_capacity() -> u64 {
    (figures::MAX_VOXELS * 16
        + figures::MAX_TILES * 144
        + figures::MAX_FIGS * 48
        + figures::MAX_PALETTE * 32
        + figures::MAX_BONES * 48
        + plants::MAX_PLANT_CELLS * 16
        + plants::MAX_PLANTS * 4) as u64
}
fn buffers<const N: usize>(gpu: &Gpu, sizes: [usize; N]) -> Vec<[wgpu::Buffer; N]> {
    (0..crate::voxel::STAGING_RING)
        .map(|_| {
            sizes.map(|size| {
                upload::buffer(
                    gpu,
                    "geometry ring data",
                    size as u64,
                    wgpu::BufferUsages::STORAGE,
                )
            })
        })
        .collect()
}
fn storage(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn sampled(binding: u32, sample_type: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}
fn image_layout(gpu: &Gpu, types: &[wgpu::TextureSampleType]) -> wgpu::BindGroupLayout {
    let mut entries: Vec<_> = types
        .iter()
        .enumerate()
        .map(|(i, t)| sampled(i as u32, *t))
        .collect();
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 32,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
        count: None,
    });
    gpu.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("geometry images"),
        entries: &entries,
    })
}
fn image_group(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    views: &[&wgpu::TextureView],
) -> wgpu::BindGroup {
    let sampler = gpu
        .device
        .create_sampler(&wgpu::SamplerDescriptor::default());
    let mut entries: Vec<_> = views
        .iter()
        .enumerate()
        .map(|(i, view)| wgpu::BindGroupEntry {
            binding: i as u32,
            resource: wgpu::BindingResource::TextureView(view),
        })
        .collect();
    entries.push(wgpu::BindGroupEntry {
        binding: 32,
        resource: wgpu::BindingResource::Sampler(&sampler),
    });
    gpu.bind_group(&wgpu::BindGroupDescriptor {
        label: Some("geometry images"),
        layout,
        entries: &entries,
    })
}
fn data_group(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    buffers: &[wgpu::Buffer],
) -> wgpu::BindGroup {
    let entries: Vec<_> = buffers
        .iter()
        .enumerate()
        .map(|(i, b)| wgpu::BindGroupEntry {
            binding: i as u32,
            resource: b.as_entire_binding(),
        })
        .collect();
    gpu.bind_group(&wgpu::BindGroupDescriptor {
        label: Some("geometry data"),
        layout,
        entries: &entries,
    })
}
fn target(format: wgpu::TextureFormat, blend: bool) -> Option<wgpu::ColorTargetState> {
    Some(wgpu::ColorTargetState {
        format,
        // Geometry colours are premultiplied. Keep the destination alpha:
        // emission alpha carries the walk's encoded depth for the weather pass.
        blend: blend.then_some(wgpu::BlendState {
            color: wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING.color,
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        }),
        write_mask: wgpu::ColorWrites::ALL,
    })
}
// Figure-only coverage accumulates separately from the raster/depth alpha.
fn coverage_target(format: wgpu::TextureFormat) -> Option<wgpu::ColorTargetState> {
    Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })
}
fn pipeline(
    gpu: &Gpu,
    layouts: &[&wgpu::BindGroupLayout],
    immediate_size: u32,
    vertex: Shader,
    fragment: Shader,
    spec: Specialization,
    targets: &[Option<wgpu::ColorTargetState>],
    depth: Option<wgpu::CompareFunction>,
    strip: bool,
) -> Result<wgpu::RenderPipeline> {
    let layout = gpu.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(fragment.name()),
        bind_group_layouts: &layouts.iter().map(|l| Some(*l)).collect::<Vec<_>>(),
        immediate_size,
    });
    let module = |s: Shader| {
        gpu.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(s.name()),
            source: wgpu::ShaderSource::Wgsl(s.runtime_source().into()),
        })
    };
    let (vs, fs) = (module(vertex), module(fragment));
    let constants = |s: Shader| -> Result<Vec<(String, f64)>> { s.runtime_constants(spec) };
    let (vc_owned, fc_owned) = (constants(vertex)?, constants(fragment)?);
    let vc: Vec<_> = vc_owned.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let fc: Vec<_> = fc_owned.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    Ok(gpu.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment.name()),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &vs,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &vc,
                ..Default::default()
            },
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: if strip {
                wgpu::PrimitiveTopology::TriangleStrip
            } else {
                wgpu::PrimitiveTopology::TriangleList
            },
            ..Default::default()
        },
        depth_stencil: depth.map(|depth_compare| wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(depth_compare),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &fs,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &fc,
                ..Default::default()
            },
            targets,
        }),
        multiview_mask: None,
        cache: None,
    }))
}
fn attachment(
    view: &wgpu::TextureView,
    clear: Option<wgpu::Color>,
) -> Option<wgpu::RenderPassColorAttachment<'_>> {
    Some(wgpu::RenderPassColorAttachment {
        view,
        resolve_target: None,
        depth_slice: None,
        ops: wgpu::Operations {
            load: clear.map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear),
            store: wgpu::StoreOp::Store,
        },
    })
}
fn depth(view: &wgpu::TextureView, clear: f32) -> wgpu::RenderPassDepthStencilAttachment<'_> {
    wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(clear),
            store: wgpu::StoreOp::Store,
        }),
        stencil_ops: None,
    }
}
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct FigurePush {
    k: [i32; 4],
    atlas: [f32; 4],
}

pub struct Figures {
    pub walk_depth: Plane,
    pub sun: Plane,
    pub comp_depth: Plane,
    pub weather_surface: Option<Plane>,
    colour: Plane,
    emit: Plane,
    z: Plane,
    tile_depth: Plane,
    image_layout: wgpu::BindGroupLayout,
    tile_pipeline: wgpu::RenderPipeline,
    comp_pipeline: wgpu::RenderPipeline,
    sun_pipeline: wgpu::RenderPipeline,
    models: Vec<FigureVoxel>,
    model_dirty: Option<std::ops::Range<usize>>,
    model_recorded: Vec<Option<std::ops::Range<usize>>>,
    buffers: Vec<[wgpu::Buffer; 5]>,
    groups: Vec<wgpu::BindGroup>,
    packed: Packed,
    data: Option<wgpu::BindGroup>,
    images: Option<wgpu::BindGroup>,
    raster: (u32, u32),
    world: (u32, u32, u32),
    stats: FigureAtlasStats,
}
impl Figures {
    /// Release backing allocations after the renderer has drained submissions.
    pub fn destroy(&mut self) {
        self.data = None;
        self.images = None;
        self.models = Vec::new();
        self.packed = Packed::default();
        for plane in [
            &self.walk_depth,
            &self.sun,
            &self.comp_depth,
            &self.colour,
            &self.emit,
            &self.z,
            &self.tile_depth,
        ] {
            plane.texture.destroy();
        }
        if let Some(plane) = self.weather_surface.take() {
            plane.texture.destroy();
        }
    }
    pub fn new(
        gpu: &Gpu,
        world_layout: &wgpu::BindGroupLayout,
        raster: (u32, u32),
        world: (u32, u32, u32),
        raster_format: wgpu::TextureFormat,
        emission_format: Option<wgpu::TextureFormat>,
        weather: bool,
        spec: Specialization,
    ) -> Result<Self> {
        let data_layout = gpu.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("figure data"),
            entries: &(0..5).map(storage).collect::<Vec<_>>(),
        });
        let image_layout = image_layout(
            gpu,
            &[wgpu::TextureSampleType::Float { filterable: false }; 4],
        );
        let layouts = [world_layout, &data_layout, &image_layout];
        let tile_pipeline = pipeline(
            gpu,
            &layouts[..2],
            32,
            Shader::FigureVertex,
            Shader::FigureFragment,
            spec,
            &[
                target(wgpu::TextureFormat::Rgba8UnormSrgb, false),
                target(wgpu::TextureFormat::Rgba8UnormSrgb, false),
                target(wgpu::TextureFormat::R32Float, false),
            ],
            Some(wgpu::CompareFunction::LessEqual),
            false,
        )?;
        let comp_pipeline = pipeline(
            gpu,
            &layouts,
            32,
            Shader::CompositeVertex,
            Shader::CompositeFragment,
            spec,
            &[
                target(raster_format, true),
                emission_format.and_then(|f| target(f, true)),
                weather
                    .then(|| coverage_target(wgpu::TextureFormat::Rgba16Float))
                    .flatten(),
            ],
            Some(wgpu::CompareFunction::LessEqual),
            true,
        )?;
        let sun_pipeline = pipeline(
            gpu,
            &layouts[..2],
            32,
            Shader::FigureSunVertex,
            Shader::FigureSunFragment,
            spec,
            &[],
            Some(wgpu::CompareFunction::Greater),
            false,
        )?;
        let atlas = (figures::ATLAS_W, figures::ATLAS_H);
        let models = upload::buffer(
            gpu,
            "append-only figure models",
            (figures::MAX_VOXELS * 16) as u64,
            wgpu::BufferUsages::STORAGE,
        );
        let buffers: Vec<_> = buffers(
            gpu,
            [
                figures::MAX_TILES * 144,
                figures::MAX_FIGS * 48,
                figures::MAX_PALETTE * 32,
                figures::MAX_BONES * 48,
            ],
        )
        .into_iter()
        .map(|[tiles, figs, palette, bones]| [models.clone(), tiles, figs, palette, bones])
        .collect();
        let groups = buffers
            .iter()
            .map(|b| data_group(gpu, &data_layout, b))
            .collect();
        Ok(Self {
            walk_depth: Plane::new(gpu, "walk depth", raster, wgpu::TextureFormat::R32Float),
            sun: Plane::new(
                gpu,
                "figure sun",
                (2 * world.0, figures::SUN_MAP_H),
                wgpu::TextureFormat::Depth32Float,
            ),
            comp_depth: Plane::new(
                gpu,
                "figure composite depth",
                raster,
                wgpu::TextureFormat::Depth32Float,
            ),
            weather_surface: weather.then(|| {
                Plane::new(
                    gpu,
                    "figure weather",
                    raster,
                    wgpu::TextureFormat::Rgba16Float,
                )
            }),
            colour: Plane::new(
                gpu,
                "figure colour",
                atlas,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            ),
            emit: Plane::new(
                gpu,
                "figure emission",
                atlas,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            ),
            z: Plane::new(gpu, "figure atlas z", atlas, wgpu::TextureFormat::R32Float),
            tile_depth: Plane::new(
                gpu,
                "figure atlas depth",
                atlas,
                wgpu::TextureFormat::Depth32Float,
            ),
            image_layout,
            tile_pipeline,
            comp_pipeline,
            sun_pipeline,
            models: vec![],
            model_dirty: None,
            model_recorded: vec![None; crate::voxel::STAGING_RING],
            buffers,
            groups,
            packed: Packed::default(),
            data: None,
            images: None,
            raster,
            world,
            stats: FigureAtlasStats::default(),
        })
    }
    pub fn put_models(&mut self, first: usize, cells: &[FigureVoxel]) -> bool {
        let Some(end) = first
            .checked_add(cells.len())
            .filter(|n| *n <= figures::MAX_VOXELS)
        else {
            return false;
        };
        self.models
            .resize(self.models.len().max(end), FigureVoxel::default());
        if bytemuck::cast_slice::<_, u8>(&self.models[first..end])
            != bytemuck::cast_slice::<_, u8>(cells)
        {
            self.models[first..end].copy_from_slice(cells);
            self.owe_models(first..end);
        }
        true
    }
    pub fn voxels(&self) -> usize {
        self.models.len()
    }
    pub fn atlas_stats(&self) -> FigureAtlasStats {
        self.stats
    }
    pub fn has_tiles(&self) -> bool {
        self.packed.tiles() > 0
    }
    pub fn drawing(&self) -> bool {
        self.packed.figures() > 0
    }
    pub fn prepare(
        &mut self,
        _gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        writer: &mut Upload<'_>,
        slot: usize,
        frame: &FigureFrame,
        s: u32,
        rise: u32,
        base: i32,
        view_x: i32,
        ring_width: u32,
    ) -> Result<usize> {
        let p = figures::pack(frame, s, rise, base, ring_width, view_x);
        self.stats = FigureAtlasStats {
            width: figures::ATLAS_W,
            height: figures::ATLAS_H,
            logical_width: p.logical_atlas.0,
            logical_height: p.logical_atlas.1,
            tile_capacity: figures::MAX_TILES,
            composite_capacity: figures::MAX_FIGS,
            tiles: p.tiles(),
            figures: frame.figures.len().saturating_sub(p.dropped),
            composites: p.figures(),
            requested_supersample: p.requested_supersample,
            supersample: p.supersample,
            dropped: p.dropped,
            omitted_extras: p.omitted_extras,
        };
        let buffers = &self.buffers[slot];
        if let Some(range) = self.model_dirty.take() {
            self.model_recorded[slot] = Some(range.clone());
            writer.bytes(
                encoder,
                &buffers[0],
                (range.start * 16) as u64,
                bytemuck::cast_slice(&self.models[range]),
            )?;
        }
        for (buffer, bytes) in buffers[1..].iter().zip([
            bytemuck::cast_slice(&p.tiles),
            bytemuck::cast_slice(&p.figs),
            bytemuck::cast_slice(&p.palette),
            bytemuck::cast_slice(&p.bones),
        ]) {
            writer.bytes(encoder, buffer, 0, bytes)?;
        }
        self.data = Some(self.groups[slot].clone());
        let dropped = p.dropped;
        self.packed = p;
        Ok(dropped)
    }
    fn owe_models(&mut self, range: std::ops::Range<usize>) {
        self.model_dirty = Some(match self.model_dirty.take() {
            Some(old) => old.start.min(range.start)..old.end.max(range.end),
            None => range,
        });
    }
    pub fn retired(&mut self, slot: usize) {
        self.model_recorded[slot] = None;
    }
    pub fn discarded(&mut self, slot: usize) {
        if let Some(range) = self.model_recorded[slot].take() {
            self.owe_models(range);
        }
    }
    pub fn set_occluder(&mut self, gpu: &Gpu, view: &wgpu::TextureView) {
        self.images = Some(image_group(
            gpu,
            &self.image_layout,
            &[&self.colour.view, &self.emit.view, &self.z.view, view],
        ));
    }
    pub fn record_sun(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        world: &wgpu::BindGroup,
        offset: u32,
        sun: Option<([f32; 4], [f32; 4])>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("figure sun"),
            color_attachments: &[],
            depth_stencil_attachment: Some(depth(&self.sun.view, 0.0)),
            ..Default::default()
        });
        if let (Some((fs, fm)), Some(data)) = (sun, self.data.as_ref()) {
            pass.set_pipeline(&self.sun_pipeline);
            pass.set_bind_group(0, world, &[offset]);
            pass.set_bind_group(1, data, &[]);
            for (i, t) in self.packed.tiles.iter().enumerate() {
                if !self.packed.casts[i] {
                    continue;
                }
                let push = FigurePush {
                    k: [
                        i as i32,
                        self.world.0 as i32,
                        self.world.1 as i32,
                        figures::SUN_MAP_H as i32,
                    ],
                    atlas: [fs[0], fs[1], fs[2], fm[1]],
                };
                pass.set_immediates(0, bytemuck::bytes_of(&push));
                pass.draw(
                    0..36,
                    t.orig[3] as u32..t.orig[3] as u32 + self.packed.counts[i],
                );
            }
        }
    }
    pub fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        world: &wgpu::BindGroup,
        offset: u32,
        raster: &wgpu::TextureView,
        emission: Option<&wgpu::TextureView>,
        s: u32,
        rise: u32,
    ) {
        if !self.drawing() {
            return;
        }
        let data = self.data.as_ref().expect("prepared figures");
        let p = &self.packed;
        let (aw, ah) = p.logical_atlas;
        {
            let colours = [
                attachment(&self.colour.view, Some(wgpu::Color::TRANSPARENT)),
                attachment(&self.emit.view, Some(wgpu::Color::TRANSPARENT)),
                attachment(
                    &self.z.view,
                    Some(wgpu::Color {
                        r: figures::FAR as f64,
                        g: 0.,
                        b: 0.,
                        a: 0.,
                    }),
                ),
            ];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("figure tiles"),
                color_attachments: &colours,
                depth_stencil_attachment: Some(depth(&self.tile_depth.view, 1.)),
                ..Default::default()
            });
            pass.set_pipeline(&self.tile_pipeline);
            pass.set_bind_group(0, world, &[offset]);
            pass.set_bind_group(1, data, &[]);
            pass.set_viewport(0., 0., aw as f32, ah as f32, 0., 1.);
            for (i, t) in p.tiles.iter().enumerate() {
                pass.set_scissor_rect(
                    t.rect[0] as u32,
                    t.rect[1] as u32,
                    t.rect[2] as u32,
                    t.rect[3] as u32,
                );
                let push = FigurePush {
                    k: [i as i32, s as i32, rise as i32, 0],
                    atlas: [aw as f32, ah as f32, figures::DEPTH_BIAS, 0.],
                };
                pass.set_immediates(0, bytemuck::bytes_of(&push));
                pass.draw(0..36, t.orig[3] as u32..t.orig[3] as u32 + p.counts[i]);
            }
        }
        let colours = [
            attachment(raster, None),
            emission.and_then(|v| attachment(v, None)),
            self.weather_surface
                .as_ref()
                .and_then(|p| attachment(&p.view, Some(wgpu::Color::TRANSPARENT))),
        ];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("figure composite"),
            color_attachments: &colours,
            depth_stencil_attachment: Some(depth(&self.comp_depth.view, 1.)),
            ..Default::default()
        });
        pass.set_pipeline(&self.comp_pipeline);
        pass.set_bind_group(0, world, &[offset]);
        pass.set_bind_group(1, data, &[]);
        pass.set_bind_group(2, self.images.as_ref().expect("figure occluder"), &[]);
        let push = FigurePush {
            k: [
                self.raster.0 as i32,
                s as i32,
                rise as i32,
                self.raster.1 as i32,
            ],
            atlas: [
                aw as f32,
                ah as f32,
                figures::DEPTH_BIAS,
                self.world.2 as f32,
            ],
        };
        pass.set_immediates(0, bytemuck::bytes_of(&push));
        pass.draw(0..4, 0..p.figures() as u32);
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct PlantPush {
    geom: [i32; 4],
    k: [i32; 4],
    t: [f32; 4],
    layer: [f32; 4],
}
pub struct Plants {
    pub ids: Plane,
    pub depth: Plane,
    pub occluder: Plane,
    pub weather_surface: Option<Plane>,
    motion: PlantMotion,
    aa: u32,
    layer: (u32, u32),
    raster: (u32, u32),
    world: (u32, u32, u32),
    staged: Vec<PlantList>,
    drawn: PlantList,
    tips: Vec<f32>,
    drawn_tips: Vec<f32>,
    recorded_tips: VecDeque<Option<Vec<f32>>>,
    spare_tips: Vec<Vec<f32>>,
    cell_version: u64,
    cell_written: Vec<u64>,
    buffers: Vec<[wgpu::Buffer; 2]>,
    groups: Vec<wgpu::BindGroup>,
    seconds: f64,
    rain: [f32; 3],
    stats: PlantStats,
    images: wgpu::BindGroup,
    data: Option<wgpu::BindGroup>,
    vis_pipeline: wgpu::RenderPipeline,
    shade_pipeline: wgpu::RenderPipeline,
}
impl Plants {
    /// Release backing allocations after the renderer has drained submissions.
    pub fn destroy(&mut self) {
        self.data = None;
        self.staged = Vec::new();
        self.drawn = PlantList::default();
        self.tips = Vec::new();
        self.drawn_tips = Vec::new();
        self.recorded_tips = VecDeque::new();
        for plane in [&self.ids, &self.depth, &self.occluder] {
            plane.texture.destroy();
        }
        if let Some(plane) = self.weather_surface.take() {
            plane.texture.destroy();
        }
    }
    pub fn new(
        gpu: &Gpu,
        world_layout: &wgpu::BindGroupLayout,
        raster: (u32, u32),
        world: (u32, u32, u32),
        raster_format: wgpu::TextureFormat,
        emission_format: Option<wgpu::TextureFormat>,
        weather: bool,
        spec: Specialization,
        motion: PlantMotion,
        walk_depth: &wgpu::TextureView,
        staging: usize,
    ) -> Result<Self> {
        let aa = motion.supersample.clamp(1, plants::MAX_SUPERSAMPLE);
        let layer = (raster.0 * aa, raster.1 * aa);
        let ids = Plane::new(gpu, "plant ids", layer, wgpu::TextureFormat::R32Uint);
        let depth = Plane::new(gpu, "plant depth", layer, wgpu::TextureFormat::Depth32Float);
        let occluder = Plane::new(gpu, "plant occluder", raster, wgpu::TextureFormat::R32Float);
        let weather_surface = weather.then(|| {
            Plane::new(
                gpu,
                "plant weather",
                raster,
                wgpu::TextureFormat::Rgba16Float,
            )
        });
        let data_layout = gpu.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("plant data"),
            entries: &[storage(0), storage(1)],
        });
        let image_layout = image_layout(
            gpu,
            &[
                wgpu::TextureSampleType::Uint,
                wgpu::TextureSampleType::Depth,
                wgpu::TextureSampleType::Float { filterable: false },
            ],
        );
        let images = image_group(gpu, &image_layout, &[&ids.view, &depth.view, walk_depth]);
        let layouts = [world_layout, &data_layout, &image_layout];
        let vis_pipeline = pipeline(
            gpu,
            &layouts[..2],
            64,
            Shader::PlantVertex,
            Shader::PlantVis,
            spec,
            &[target(wgpu::TextureFormat::R32Uint, false)],
            Some(wgpu::CompareFunction::LessEqual),
            false,
        )?;
        let shade_pipeline = pipeline(
            gpu,
            &layouts,
            64,
            Shader::Fullscreen,
            Shader::PlantShade,
            spec,
            &[
                target(raster_format, true),
                emission_format.and_then(|f| target(f, true)),
                target(wgpu::TextureFormat::R32Float, false),
                weather
                    .then(|| target(wgpu::TextureFormat::Rgba16Float, false))
                    .flatten(),
            ],
            None,
            false,
        )?;
        let buffers = buffers(gpu, [plants::MAX_PLANT_CELLS * 16, plants::MAX_PLANTS * 4]);
        let groups = buffers
            .iter()
            .map(|b| data_group(gpu, &data_layout, b))
            .collect();
        Ok(Self {
            ids,
            depth,
            occluder,
            weather_surface,
            motion,
            aa,
            layer,
            raster,
            world,
            staged: vec![PlantList::default(); staging],
            drawn: PlantList::default(),
            tips: vec![],
            drawn_tips: vec![],
            recorded_tips: VecDeque::new(),
            spare_tips: vec![],
            cell_version: 0,
            cell_written: vec![u64::MAX; crate::voxel::STAGING_RING],
            buffers,
            groups,
            seconds: 0.,
            rain: [0.; 3],
            stats: PlantStats::default(),
            images,
            data: None,
            vis_pipeline,
            shade_pipeline,
        })
    }
    pub fn motion(&self) -> PlantMotion {
        self.motion
    }
    pub fn staging(&mut self, into: usize) -> &mut PlantList {
        &mut self.staged[into]
    }
    pub fn adopt(&mut self, from: usize) {
        // Staging stays intact until its upload retires, so a discarded recording
        // can adopt the same list again without toggling back to the old picture.
        if self.drawn.cells != self.staged[from].cells {
            self.cell_version += 1;
        }
        self.drawn.clone_from(&self.staged[from]);
        let n = self.drawn.cells.len();
        self.stats = PlantStats {
            cells: n.min(plants::MAX_PLANT_CELLS),
            plants: self.drawn.plants.len(),
            changing: self
                .drawn
                .cells
                .iter()
                .filter(|c| f32::from_bits(c[2]) > plants::SETTLED)
                .count(),
            dropped: n.saturating_sub(plants::MAX_PLANT_CELLS),
        };
        self.take_tips(self.seconds, self.rain);
    }
    pub fn stats(&self) -> PlantStats {
        self.stats
    }
    pub fn drawn(&self) -> (&PlantList, f64) {
        (&self.drawn, self.seconds)
    }
    pub fn set_clock(&mut self, seconds: f64, rain: [f32; 3]) -> bool {
        self.take_tips(seconds, rain);
        self.tips != self.drawn_tips || seconds < self.drawn.settled_at
    }
    fn take_tips(&mut self, seconds: f64, rain: [f32; 3]) {
        self.seconds = seconds;
        self.rain = rain;
        let n = self.drawn.plants.len().min(plants::MAX_PLANTS);
        plants::tips_into(
            &self.drawn.plants[..n],
            &self.motion.breeze,
            self.world.0,
            seconds,
            rain,
            &mut self.tips,
        );
    }
    pub fn prepare(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        writer: &mut Upload<'_>,
        slot: usize,
    ) -> Result<()> {
        let n = self.drawn.cells.len().min(plants::MAX_PLANT_CELLS);
        if self.cell_written[slot] != self.cell_version {
            writer.bytes(
                encoder,
                &self.buffers[slot][0],
                0,
                bytemuck::cast_slice(&self.drawn.cells[..n]),
            )?;
            self.cell_written[slot] = self.cell_version;
        }
        writer.bytes(
            encoder,
            &self.buffers[slot][1],
            0,
            bytemuck::cast_slice(&self.drawn_tips),
        )?;
        self.data = Some(self.groups[slot].clone());
        Ok(())
    }
    pub fn discarded(&mut self, slot: usize) {
        self.cell_written[slot] = u64::MAX;
    }
    /// Move snapshots between recordings instead of cloning plant tips. Skipped
    /// frames need only an ordering marker; retired snapshots become scratch.
    pub fn frame_recorded(&mut self, redraw: bool) {
        let previous = if redraw {
            let previous = std::mem::replace(&mut self.drawn_tips, std::mem::take(&mut self.tips));
            self.tips = self.spare_tips.pop().unwrap_or_default();
            Some(previous)
        } else {
            None
        };
        self.recorded_tips.push_back(previous);
    }
    pub fn frame_retired(&mut self) {
        if let Some(Some(previous)) = self.recorded_tips.pop_front() {
            self.spare_tips.push(previous);
        }
    }
    pub fn frame_discarded(&mut self) {
        if let Some(Some(previous)) = self.recorded_tips.pop_back() {
            self.spare_tips
                .push(std::mem::replace(&mut self.drawn_tips, previous));
        }
    }
    pub fn drawing(&self) -> bool {
        !self.drawn.cells.is_empty()
    }
    fn push(&self, (s, rise, base): (u32, u32, i32), view_x: i32, cord_reach: f32) -> PlantPush {
        PlantPush {
            geom: [s as i32, rise as i32, base, self.world.0 as i32],
            k: [
                self.drawn.cells.len().min(plants::MAX_PLANT_CELLS) as i32,
                self.aa as i32,
                view_x,
                self.world.2 as i32,
            ],
            t: [
                (self.seconds - self.drawn.epoch) as f32,
                self.motion.grow_s,
                figures::DEPTH_BIAS,
                cord_reach,
            ],
            layer: [
                self.layer.0 as f32,
                self.layer.1 as f32,
                self.raster.0 as f32,
                self.raster.1 as f32,
            ],
        }
    }
    pub fn record_vis(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        world: &wgpu::BindGroup,
        offset: u32,
        geom: (u32, u32, i32),
        view_x: i32,
        cord_reach: f32,
    ) {
        let colours = [attachment(&self.ids.view, Some(wgpu::Color::TRANSPARENT))];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("plant visibility"),
            color_attachments: &colours,
            depth_stencil_attachment: Some(depth(&self.depth.view, 1.)),
            ..Default::default()
        });
        let n = self.drawn.cells.len().min(plants::MAX_PLANT_CELLS) as u32;
        if n == 0 {
            return;
        }
        pass.set_pipeline(&self.vis_pipeline);
        pass.set_bind_group(0, world, &[offset]);
        pass.set_bind_group(1, self.data.as_ref().expect("prepared plants"), &[]);
        pass.set_immediates(0, bytemuck::bytes_of(&self.push(geom, view_x, cord_reach)));
        pass.draw(0..12, 0..2 * n);
    }
    pub fn record_shade(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        world: &wgpu::BindGroup,
        offset: u32,
        raster: &wgpu::TextureView,
        emission: Option<&wgpu::TextureView>,
        geom: (u32, u32, i32),
        view_x: i32,
    ) {
        let colours = [
            attachment(raster, None),
            emission.and_then(|v| attachment(v, None)),
            attachment(&self.occluder.view, None),
            self.weather_surface
                .as_ref()
                .and_then(|p| attachment(&p.view, None)),
        ];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("plant shade"),
            color_attachments: &colours,
            ..Default::default()
        });
        pass.set_pipeline(&self.shade_pipeline);
        pass.set_bind_group(0, world, &[offset]);
        pass.set_bind_group(1, self.data.as_ref().expect("prepared plants"), &[]);
        pass.set_bind_group(2, &self.images, &[]);
        pass.set_immediates(0, bytemuck::bytes_of(&self.push(geom, view_x, 0.)));
        pass.draw(0..3, 0..1);
    }
}
