//! Bloom plans preserve the original cell and smooth-pyramid algorithms.
#![forbid(unsafe_code)]
use super::{Gpu, shader::Shader};
use crate::{
    bloom::{BloomStyle, SMOOTH_GAIN, cell_grid, level_sizes, smooth_levels, smooth_sigma_px},
    voxel::VoxelParams,
};
use anyhow::Result;
struct Target {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}
struct Pass {
    mode: i32,
    source: Option<usize>,
    target: Option<usize>,
    src: (u32, u32),
    bind: wgpu::BindGroup,
}
pub struct Bloom {
    pub emission: wgpu::TextureView,
    _emission: wgpu::Texture,
    targets: Vec<Target>,
    passes: Vec<Pass>,
    cell: wgpu::RenderPipeline,
    add: wgpu::RenderPipeline,
    style: BloomStyle,
    grid: (u32, u32, u32),
}
impl Bloom {
    pub fn new(gpu: &Gpu, p: &VoxelParams) -> Result<Self> {
        let target = |name, size: (u32, u32), format| {
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
            Target {
                _texture: texture,
                view,
                size,
            }
        };
        let raster = (p.raster_w, p.raster_h);
        let emission = target(
            "bloom emission",
            raster,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        let grid = cell_grid(p);
        let sizes = match p.bloom_style {
            BloomStyle::Blocky => vec![(grid.0, grid.1); 2],
            BloomStyle::Smooth => {
                let mut s = level_sizes(p, smooth_levels(p));
                s.push(*s.last().unwrap());
                s
            }
        };
        let targets: Vec<_> = sizes
            .into_iter()
            .map(|s| target("bloom level", s, wgpu::TextureFormat::Rgba16Float))
            .collect();
        let mut plan = vec![];
        match p.bloom_style {
            BloomStyle::Blocky => {
                plan.push((0, None, Some(0), raster));
                plan.push((1, Some(0), Some(1), targets[0].size));
                plan.push((2, Some(1), None, targets[1].size));
            }
            BloomStyle::Smooth => {
                let n = targets.len() - 1;
                for i in 0..n {
                    plan.push((
                        0,
                        i.checked_sub(1),
                        Some(i),
                        if i == 0 { raster } else { targets[i - 1].size },
                    ));
                }
                plan.push((1, Some(n - 1), Some(n), targets[n - 1].size));
                plan.push((2, Some(n), Some(n - 1), targets[n].size));
                for i in (1..n).rev() {
                    plan.push((3, Some(i), Some(i - 1), targets[i].size));
                }
                plan.push((4, Some(0), None, targets[0].size));
            }
        }
        let linear = p.bloom_style == BloomStyle::Smooth;
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bloom inputs"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: linear },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 32,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 33,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let nearest = gpu
            .device
            .create_sampler(&wgpu::SamplerDescriptor::default());
        // The shader explicitly rejects vertical out-of-range taps, reproducing the
        // original transparent border while horizontal sampling wraps around the ring.
        let smooth = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let passes = plan
            .into_iter()
            .map(|(mode, source, target, src)| {
                let view = source.map_or(&emission.view, |i| &targets[i].view);
                let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("bloom pass"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&emission.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 32,
                            resource: wgpu::BindingResource::Sampler(&nearest),
                        },
                        wgpu::BindGroupEntry {
                            binding: 33,
                            resource: wgpu::BindingResource::Sampler(&smooth),
                        },
                    ],
                });
                Pass {
                    mode,
                    source,
                    target,
                    src,
                    bind,
                }
            })
            .collect();
        let pl = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("bloom"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 48,
            });
        let vertex = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("fullscreen"),
                source: wgpu::ShaderSource::Wgsl(Shader::Fullscreen.runtime_source().into()),
            });
        let shader = if linear {
            Shader::BloomSmooth
        } else {
            Shader::Bloom
        };
        let fragment = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(shader.name()),
                source: wgpu::ShaderSource::Wgsl(shader.runtime_source().into()),
            });
        let pipeline = |format, blend| {
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("bloom"),
                    layout: Some(&pl),
                    vertex: wgpu::VertexState {
                        module: &vertex,
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fragment,
                        entry_point: Some("main"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                })
        };
        let cell = pipeline(wgpu::TextureFormat::Rgba16Float, None);
        let add = pipeline(
            wgpu::TextureFormat::Rgba8UnormSrgb,
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        Ok(Self {
            emission: emission.view,
            _emission: emission._texture,
            targets,
            passes,
            cell,
            add,
            style: p.bloom_style,
            grid,
        })
    }
    pub fn destroy(&mut self) {
        self._emission.destroy();
        for target in &self.targets {
            target._texture.destroy();
        }
        self.passes.clear();
        self.targets.clear();
    }
    pub fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        p: &VoxelParams,
        raster: &wgpu::TextureView,
    ) {
        let (cw, ch, off) = self.grid;
        let last = self.targets.len().saturating_sub(2);
        let sigma =
            smooth_sigma_px(p) / (p.raster_w as f32 / self.targets[last].size.0.max(1) as f32);
        let taps = (3.0 * sigma).ceil().clamp(1.0, 32.0) as u32;
        for step in &self.passes {
            let _ = step.source;
            let size = step
                .target
                .map_or((p.raster_w, p.raster_h), |i| self.targets[i].size);
            let view = step.target.map_or(raster, |i| &self.targets[i].view);
            let push = match self.style {
                BloomStyle::Blocky => [
                    step.mode as u32,
                    p.s,
                    p.bloom_radius.max(0.0).round() as u32,
                    0,
                    cw,
                    ch,
                    off,
                    p.raster_h,
                    p.bloom.max(0.0).to_bits(),
                    0,
                    0,
                    0,
                ],
                BloomStyle::Smooth => [
                    step.mode as u32,
                    step.src.0,
                    step.src.1,
                    taps,
                    size.0,
                    size.1,
                    0,
                    0,
                    (p.bloom.max(0.0) * SMOOTH_GAIN).to_bits(),
                    sigma.to_bits(),
                    0,
                    0,
                ],
            };
            let attachment = Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if step.target.is_none() {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    },
                    store: wgpu::StoreOp::Store,
                },
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom"),
                color_attachments: &[attachment],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(if step.target.is_none() {
                &self.add
            } else {
                &self.cell
            });
            pass.set_bind_group(0, &step.bind, &[]);
            pass.set_immediates(0, bytemuck::cast_slice(&push));
            pass.draw(0..3, 0..1);
        }
    }
}
