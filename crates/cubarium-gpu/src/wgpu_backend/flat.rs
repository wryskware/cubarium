//! The flat scene passes on safe wgpu resources, with the existing CPU scene ABI.
#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};

use super::upload::{self, UploadRing};
use anyhow::{Result, ensure};

use super::shader::Shader;
use super::{FrameSource, Gpu, PresentPass, TargetSlot};
use crate::atlas::Atlas;
use crate::palette::SceneUniforms;
use crate::render::{FillProfile, f16};
use crate::scene::{LAYERS, Layer, RingLayout, Scene, SpriteInstance};

pub const RASTER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
use crate::render::SCRATCH_SIDE;
const INSTANCE_CAPACITY: usize = 8192;

struct ScratchRegion {
    origin: wgpu::Origin3d,
    size: wgpu::Extent3d,
}

pub struct Renderer {
    closed: bool,
    atlas: wgpu::Texture,
    timing: super::timing::Timing,
    pub layout: RingLayout,
    pub bend_substep: bool,
    pub filter_bilinear: bool,
    pub art_scale: f32,
    pub fill_profile: bool,
    fill: FillProfile,
    opaque_by_origin: HashMap<[u16; 2], u32>,
    raster: wgpu::Texture,
    raster_view: wgpu::TextureView,
    scratch: wgpu::Texture,
    scratch_cursor: (u32, u32, u32),
    scratch_regions: Vec<ScratchRegion>,
    field_images: [wgpu::Texture; 2],
    field_revision: Option<u64>,
    recorded: VecDeque<Option<u64>>,
    background: wgpu::RenderPipeline,
    water: wgpu::RenderPipeline,
    sprites: wgpu::RenderPipeline,
    present: HashMap<wgpu::TextureFormat, PresentPass>,
    present_frames: std::collections::VecDeque<Option<wgpu::TextureFormat>>,
    uploads: UploadRing,
    uniforms: wgpu::Buffer,
    uniform_stride: u64,
    instances: Vec<wgpu::Buffer>,
    groups: Vec<wgpu::BindGroup>,
    record_slots: VecDeque<usize>,
    scratch_bytes: Vec<u16>,
}

impl Renderer {
    pub fn new(gpu: &Gpu, atlas: &Atlas, layout: RingLayout) -> Result<Self> {
        gpu.validated("flat renderer construction", || {
            Self::build(gpu, atlas, layout)
        })
    }
    fn build(gpu: &Gpu, atlas: &Atlas, layout: RingLayout) -> Result<Self> {
        ensure!(layout.is_valid(), "invalid scene layout {layout:?}");
        let texture = |name, size, format, usage| {
            gpu.texture(name, size, wgpu::TextureDimension::D2, format, usage)
        };
        let raster = texture(
            "world raster",
            extent(layout.w, layout.h),
            RASTER_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let raster_view = raster.create_view(&Default::default());
        let atlas_image = texture(
            "sprite atlas",
            extent(atlas.width, atlas.height),
            RASTER_FORMAT,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let mut upload = gpu.encoder("sprite atlas");
        gpu.upload_texture(
            &mut upload,
            &atlas_image,
            wgpu::Origin3d::ZERO,
            extent(atlas.width, atlas.height),
            4,
            &atlas.rgba,
        )?;
        gpu.queue.submit([upload.finish()]);
        let atlas_view = atlas_image.create_view(&Default::default());
        let scratch = texture(
            "scratch atlas",
            extent(SCRATCH_SIDE, SCRATCH_SIDE),
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let scratch_view = scratch.create_view(&Default::default());
        let field_images = std::array::from_fn(|_| {
            texture(
                "scene fields",
                extent(layout.cells_x(), layout.cells_y()),
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            )
        });
        let field_views = field_images
            .each_ref()
            .map(|t| t.create_view(&Default::default()));
        let nearest = gpu
            .device
            .create_sampler(&wgpu::SamplerDescriptor::default());
        let mut entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<SceneUniforms>() as u64),
            },
            count: None,
        }];
        for binding in 1..=4 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 32,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
            count: None,
        });
        let bind_layout = gpu.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene ABI"),
            entries: &entries,
        });
        let pipeline_layout = gpu.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let background = pipeline(
            gpu,
            &pipeline_layout,
            Shader::Fullscreen,
            Shader::Background,
            false,
            &[],
        )?;
        let water = pipeline(
            gpu,
            &pipeline_layout,
            Shader::Fullscreen,
            Shader::Water,
            true,
            &[],
        )?;
        let attributes = [
            attribute(0, wgpu::VertexFormat::Float32x2, 0),
            attribute(1, wgpu::VertexFormat::Float32x2, 8),
            attribute(2, wgpu::VertexFormat::Uint16x4, 16),
            attribute(3, wgpu::VertexFormat::Uint16x4, 24),
            attribute(4, wgpu::VertexFormat::Uint16x4, 32),
            attribute(5, wgpu::VertexFormat::Float32x4, 40),
            attribute(6, wgpu::VertexFormat::Float32x4, 56),
            attribute(7, wgpu::VertexFormat::Float32x4, 72),
            attribute(8, wgpu::VertexFormat::Float32x4, 88),
            attribute(9, wgpu::VertexFormat::Float32x4, 104),
            attribute(10, wgpu::VertexFormat::Float32x4, 120),
        ];
        let buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SpriteInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &attributes,
        })];
        let sprites = pipeline(
            gpu,
            &pipeline_layout,
            Shader::SpriteVertex,
            Shader::SpriteFragment,
            true,
            &buffers,
        )?;
        let uniform_size = std::mem::size_of::<SceneUniforms>() as u64;
        let alignment = u64::from(gpu.device.limits().min_uniform_buffer_offset_alignment);
        let uniform_stride = uniform_size.div_ceil(alignment) * alignment;
        let uniforms = upload::buffer(
            gpu,
            "scene uniform ring",
            uniform_stride * crate::voxel::STAGING_RING as u64,
            wgpu::BufferUsages::UNIFORM,
        );
        let instances: Vec<_> = (0..crate::voxel::STAGING_RING)
            .map(|_| {
                upload::buffer(
                    gpu,
                    "scene instance ring",
                    (INSTANCE_CAPACITY * std::mem::size_of::<SpriteInstance>()) as u64,
                    wgpu::BufferUsages::VERTEX,
                )
            })
            .collect();
        let groups = (0..crate::voxel::STAGING_RING)
            .map(|slot| {
                let mut entries = vec![wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniforms,
                        offset: slot as u64 * uniform_stride,
                        size: wgpu::BufferSize::new(uniform_size),
                    }),
                }];
                for (i, view) in [&field_views[0], &field_views[1], &atlas_view, &scratch_view]
                    .into_iter()
                    .enumerate()
                {
                    entries.push(wgpu::BindGroupEntry {
                        binding: i as u32 + 1,
                        resource: wgpu::BindingResource::TextureView(view),
                    });
                }
                entries.push(wgpu::BindGroupEntry {
                    binding: 32,
                    resource: wgpu::BindingResource::Sampler(&nearest),
                });
                gpu.bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("scene resources"),
                    layout: &bind_layout,
                    entries: &entries,
                })
            })
            .collect();
        let capacity = uniform_stride
            + instances[0].size()
            + 2 * upload::texture_bytes(extent(layout.cells_x(), layout.cells_y()), 8)
            + upload::texture_bytes(extent(SCRATCH_SIDE, SCRATCH_SIDE), 8)
            + 1024;
        let uploads = UploadRing::new(gpu, capacity, crate::voxel::STAGING_RING);
        Ok(Self {
            closed: false,
            atlas: atlas_image,
            timing: Default::default(),
            layout,
            bend_substep: false,
            filter_bilinear: false,
            art_scale: 1.0,
            fill_profile: false,
            fill: FillProfile::default(),
            opaque_by_origin: atlas
                .frames
                .iter()
                .map(|f| ([f.x, f.y], f.opaque))
                .collect(),
            raster,
            raster_view,
            scratch,
            scratch_cursor: (0, 0, 0),
            scratch_regions: vec![],
            field_images,
            field_revision: None,
            recorded: VecDeque::new(),
            background,
            water,
            sprites,
            present: HashMap::new(),
            present_frames: Default::default(),
            uploads,
            uniforms,
            uniform_stride,
            instances,
            groups,
            record_slots: VecDeque::new(),
            scratch_bytes: vec![0; (SCRATCH_SIDE * SCRATCH_SIDE * 4) as usize],
        })
    }

    pub fn destroy(&mut self, _gpu: &Gpu) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.raster.destroy();
        self.atlas.destroy();
        self.scratch.destroy();
        for image in &self.field_images {
            image.destroy();
        }
        self.present.clear();
        self.scratch_regions.clear();
    }
    pub fn scratch_begin(&mut self) {
        self.scratch_cursor = (0, 0, 0);
        self.scratch_regions.clear();
    }

    pub fn scratch_push(&mut self, w: u32, h: u32, pixels: &[[f32; 4]]) -> Option<[u16; 2]> {
        if w == 0 || h == 0 || w > SCRATCH_SIDE || pixels.len() < (w * h) as usize {
            return None;
        }
        let (mut x, mut y, mut shelf) = self.scratch_cursor;
        if x + w > SCRATCH_SIDE {
            x = 0;
            y += shelf;
            shelf = 0;
        }
        if y + h > SCRATCH_SIDE {
            return None;
        }
        for row in 0..h {
            for col in 0..w {
                let src = pixels[(row * w + col) as usize].map(f16);
                let dst = ((y + row) * SCRATCH_SIDE + x + col) as usize * 4;
                self.scratch_bytes[dst..dst + 4].copy_from_slice(&src);
            }
        }
        self.scratch_regions.push(ScratchRegion {
            origin: wgpu::Origin3d { x, y, z: 0 },
            size: extent(w, h),
        });
        self.scratch_cursor = (x + w, y, shelf.max(h));
        Some([x as u16, y as u16])
    }

    pub fn fill(&self) -> FillProfile {
        self.fill
    }

    pub fn record(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        target: Option<TargetSlot<'_>>,
    ) -> Result<usize> {
        let before = self.recorded.len();
        let result = self.record_inner(gpu, encoder, scene, target);
        if result.is_err() && self.recorded.len() > before {
            <Self as FrameSource>::frame_discarded(self);
        }
        result
    }
    fn record_inner(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        target: Option<TargetSlot<'_>>,
    ) -> Result<usize> {
        ensure!(!self.closed, "flat renderer is destroyed");
        ensure!(
            scene.layout == self.layout,
            "scene layout differs from renderer"
        );
        gpu.check_alive()?;
        ensure!(
            self.recorded.len() < crate::voxel::STAGING_RING,
            "renderer is at frame capacity"
        );
        ensure!(
            scene.instance_count() <= INSTANCE_CAPACITY,
            "too many sprite instances"
        );
        let slot = (0..crate::voxel::STAGING_RING)
            .find(|slot| !self.record_slots.contains(slot))
            .unwrap();
        self.record_slots.push_back(slot);
        self.present_frames.push_back(None);
        self.recorded.push_back(self.field_revision);
        let timing = self.timing.begin(gpu, 4);
        let mut writer = self.uploads.begin(gpu, slot)?;
        if let Some(t) = &timing {
            t.stamp(encoder, 0);
        }
        let block = SceneUniforms::new(
            self.layout,
            scene.fields.producer_max,
            scene.seconds,
            scene.f,
            self.bend_substep,
            self.filter_bilinear,
            self.art_scale,
        );
        writer.bytes(
            encoder,
            &self.uniforms,
            slot as u64 * self.uniform_stride,
            bytemuck::bytes_of(&block),
        )?;
        let instance_buffer = &self.instances[slot];
        let mut offsets = [(0u32, 0u32); crate::scene::LAYER_COUNT];
        let mut count = 0;
        for layer in LAYERS {
            let list = &scene.layers[layer as usize];
            offsets[layer as usize] = (count as u32, list.len() as u32);
            writer.bytes(
                encoder,
                instance_buffer,
                (count * std::mem::size_of::<SpriteInstance>()) as u64,
                bytemuck::cast_slice(list),
            )?;
            count += list.len();
        }
        if self.field_revision != Some(scene.fields.revision) {
            let planes = field_halves(self.layout, &scene.fields);
            for (image, halves) in self.field_images.iter().zip(&planes) {
                writer.texture(
                    encoder,
                    image,
                    wgpu::Origin3d::ZERO,
                    extent(self.layout.cells_x(), self.layout.cells_y()),
                    8,
                    bytemuck::cast_slice(halves),
                )?;
            }
            self.field_revision = Some(scene.fields.revision);
        }
        let partial_bytes: u64 = self
            .scratch_regions
            .iter()
            .map(|r| upload::texture_bytes(r.size, 8))
            .sum();
        if partial_bytes > upload::texture_bytes(extent(SCRATCH_SIDE, SCRATCH_SIDE), 8) {
            writer.texture(
                encoder,
                &self.scratch,
                wgpu::Origin3d::ZERO,
                extent(SCRATCH_SIDE, SCRATCH_SIDE),
                8,
                bytemuck::cast_slice(&self.scratch_bytes),
            )?;
        } else {
            for region in &self.scratch_regions {
                let at = ((region.origin.y * SCRATCH_SIDE + region.origin.x) * 4) as usize;
                writer.texture_rows(
                    encoder,
                    &self.scratch,
                    region.origin,
                    region.size,
                    8,
                    bytemuck::cast_slice(&self.scratch_bytes[at..]),
                    SCRATCH_SIDE * 8,
                    SCRATCH_SIDE,
                )?;
            }
        }
        writer.finish(encoder);
        if let Some(t) = &timing {
            t.stamp(encoder, 1);
        }
        let bind = &self.groups[slot];
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene raster"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.raster_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_bind_group(0, bind, &[]);
            pass.set_pipeline(&self.background);
            pass.draw(0..3, 0..1);
            pass.set_vertex_buffer(0, instance_buffer.slice(..));
            let sprites = |pass: &mut wgpu::RenderPass<'_>, layer: Layer| {
                let (first, count) = offsets[layer as usize];
                if count > 0 {
                    pass.set_pipeline(&self.sprites);
                    pass.draw(0..4, first..first + count);
                }
            };
            sprites(&mut pass, Layer::GroundCover);
            pass.set_pipeline(&self.water);
            pass.draw(0..3, 0..1);
            for layer in [Layer::Plants, Layer::Tall, Layer::Rain, Layer::Bodies] {
                sprites(&mut pass, layer);
            }
        }
        if let Some(t) = &timing {
            t.stamp(encoder, 2);
        }
        if let Some(target) = target {
            let present = match self.present.entry(target.format) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(PresentPass::new(gpu, target.format)?)
                }
            };
            let shown = present.record(
                gpu,
                encoder,
                &self.raster_view,
                target.view,
                target.transform,
            );
            ensure!(shown, "present recording failed");
            *self.present_frames.back_mut().unwrap() = Some(target.format);
        }
        if let Some(t) = timing {
            t.stamp(encoder, 3);
            self.timing.finish(encoder, t);
        }
        if self.fill_profile {
            self.fill = self.profile(scene);
        }
        Ok(count)
    }

    fn profile(&self, scene: &Scene) -> FillProfile {
        let world = self.layout.scale as f32 * self.art_scale;
        let mut p = FillProfile {
            instances: scene.instance_count(),
            ..FillProfile::default()
        };
        for layer in LAYERS {
            for i in &scene.layers[layer as usize] {
                let k = f64::from((world * i.scale.max(1e-3)).powi(2));
                let (qw, qh) = i.quad_texels();
                let quad = f64::from(qw * qh) * k;
                p.quad_px += quad;
                p.per_layer[layer as usize] += quad;
                let pad = f64::from(i.bend[0].abs() + 1.0);
                p.tile_px += (f64::from(i.size[0]) + 2.0 * pad) * (f64::from(i.size[1]) + 2.0) * k;
                let opaque: u32 = i
                    .frames
                    .iter()
                    .zip(i.weights)
                    .filter(|(_, w)| *w > 0.0)
                    .filter_map(|(o, _)| self.opaque_by_origin.get(o).copied())
                    .max()
                    .unwrap_or(0);
                p.opaque_px += f64::from(opaque) * k;
            }
        }
        p
    }
}

impl FrameSource for Renderer {
    type Frame<'a> = &'a Scene;
    fn raster_size(&self) -> (u32, u32) {
        (self.layout.w, self.layout.h)
    }
    fn raster_texture(&self) -> &wgpu::Texture {
        &self.raster
    }
    fn raster_view(&self) -> &wgpu::TextureView {
        &self.raster_view
    }
    fn record_frame(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        frame: &Scene,
        target: Option<TargetSlot<'_>>,
    ) -> Result<()> {
        self.record(gpu, encoder, frame, target).map(|_| ())
    }
    fn gpu_ms(&self, gpu: &Gpu) -> f64 {
        self.gpu_split(gpu).map_or(f64::NAN, |t| t.iter().sum())
    }
    fn frames_in_flight(&self) -> usize {
        self.recorded.len()
    }
    fn frame_retired(&mut self) {
        self.timing.retire();
        self.present_frames.pop_front();
        self.recorded.pop_front();
        self.record_slots.pop_front();
        self.uploads.retire();
    }
    fn frame_discarded(&mut self) {
        self.timing.discard();
        if let Some(Some(format)) = self.present_frames.pop_back() {
            if let Some(p) = self.present.get(&format) {
                p.discarded();
            }
        }
        if let Some(slot) = self.record_slots.pop_back() {
            self.uploads.discard_slot(slot);
        }
        if let Some(previous) = self.recorded.pop_back() {
            self.field_revision = previous;
        }
    }
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}
fn attribute(
    shader_location: u32,
    format: wgpu::VertexFormat,
    offset: u64,
) -> wgpu::VertexAttribute {
    wgpu::VertexAttribute {
        shader_location,
        format,
        offset,
    }
}

fn field_halves(layout: RingLayout, fields: &crate::scene::Fields) -> [Vec<u16>; 2] {
    let n = layout.cell_count();
    let mut planes = [vec![0; n * 4], vec![0; n * 4]];
    let at = |v: &Vec<f32>, i: usize| v.get(i).copied().unwrap_or(0.0);
    for i in 0..n {
        planes[0][i * 4..i * 4 + 4].copy_from_slice(&[
            f16(at(&fields.producer, i)),
            f16(at(&fields.water, i)),
            f16(at(&fields.detritus, i)),
            f16(at(&fields.rain, i)),
        ]);
        planes[1][i * 4] = f16(at(&fields.growth, i));
        planes[1][i * 4 + 1] = f16(at(&fields.tall, i));
    }
    planes
}

fn pipeline(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    vertex: Shader,
    fragment: Shader,
    blend: bool,
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
) -> Result<wgpu::RenderPipeline> {
    let module = |shader: Shader| {
        gpu.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(shader.name()),
            source: wgpu::ShaderSource::Wgsl(shader.runtime_source().into()),
        })
    };
    let (vs, fs) = (module(vertex), module(fragment));
    gpu.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment.name()),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &vs,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            buffers,
        },
        primitive: wgpu::PrimitiveState {
            topology: if buffers.is_empty() {
                wgpu::PrimitiveTopology::TriangleList
            } else {
                wgpu::PrimitiveTopology::TriangleStrip
            },
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &fs,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: RASTER_FORMAT,
                blend: blend.then_some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Renderer {
    pub fn gpu_split(&self, gpu: &Gpu) -> Option<[f64; 3]> {
        let t = self.timing.stamps()?;
        let ns = f64::from(gpu.queue.get_timestamp_period()) / 1e6;
        Some([
            t[1].wrapping_sub(t[0]) as f64 * ns,
            t[2].wrapping_sub(t[1]) as f64 * ns,
            t[3].wrapping_sub(t[2]) as f64 * ns,
        ])
    }
}
