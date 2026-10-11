//! Safe resources for the backend migration. Recording owns uploads: dropping an
//! encoder drops its copies, without a queue write escaping a discarded frame.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use wgpu::util::DeviceExt;

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub name: String,
}

impl Gpu {
    pub fn open() -> Result<Self> {
        pollster::block_on(Self::open_async())
    }

    async fn open_async() -> Result<Self> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .context("request wgpu adapter")?;
        let name = adapter.get_info().name;
        let required_features = adapter.features()
            & (wgpu::Features::IMMEDIATES
                | wgpu::Features::TIMESTAMP_QUERY
                | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Cubarium"),
                required_features,
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .context("request wgpu device")?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            name,
        })
    }

    pub fn encoder(&self, label: &str) -> wgpu::CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
    }

    pub fn texture(
        &self,
        label: &str,
        size: wgpu::Extent3d,
        dimension: wgpu::TextureDimension,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage,
            view_formats: &[],
        })
    }

    /// A staging buffer retained by the command encoder, not a deferred queue write.
    pub fn upload_texture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        origin: wgpu::Origin3d,
        size: wgpu::Extent3d,
        bytes_per_texel: u32,
        bytes: &[u8],
    ) -> Result<()> {
        ensure!(
            size.width > 0
                && size.height > 0
                && size.depth_or_array_layers > 0
                && bytes_per_texel > 0,
            "texture upload requires nonempty dimensions"
        );
        let row = size
            .width
            .checked_mul(bytes_per_texel)
            .context("upload row overflow")?;
        let stride =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let rows = (size.height as usize)
            .checked_mul(size.depth_or_array_layers as usize)
            .context("upload rows overflow")?;
        ensure!(
            bytes.len() == row as usize * rows,
            "texture upload byte count does not match extent"
        );
        let mut padded = vec![0; stride as usize * rows];
        for (src, dst) in bytes
            .chunks_exact(row as usize)
            .zip(padded.chunks_exact_mut(stride as usize))
        {
            dst[..src.len()].copy_from_slice(src);
        }
        let staging = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("texture upload"),
                contents: &padded,
                usage: wgpu::BufferUsages::COPY_SRC,
            });
        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(size.height),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            size,
        );
        Ok(())
    }
}

/// Shared last pass: nearest panel mapping or the existing linear-light window
/// area average. Uniform data is captured in the recording's own buffer.
pub struct PresentPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl PresentPass {
    pub fn panel_transform(
        transform: crate::render::PresentTransform,
        size: (u32, u32),
    ) -> [f32; 8] {
        transform.push(size)
    }

    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Self {
        let shader = gpu
            .device
            .create_shader_module(wgpu::include_wgsl!("../shaders/present.wgsl"));
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("present resources"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(32),
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("present"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("present"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
        Self { pipeline, layout }
    }

    pub fn record(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        raster: &wgpu::TextureView,
        target: &wgpu::TextureView,
        transform: [f32; 8],
    ) {
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("present transform"),
                contents: bytemuck::cast_slice(&transform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(raster),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("present"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

type MapReply = Arc<Mutex<Option<Result<(), wgpu::BufferAsyncError>>>>;

struct ReadbackSlot {
    buffer: wgpu::Buffer,
    ready: MapReply,
    tag: u64,
    submission: Option<wgpu::SubmissionIndex>,
}

/// Ordered asynchronous readback. A mapped slot is reusable only after poll has
/// copied its bytes and unmapped it. Completion callbacks never touch renderer state.
pub struct ReadbackRing {
    slots: Vec<ReadbackSlot>,
    flight: VecDeque<usize>,
    size: (u32, u32),
    stride: u32,
}

impl ReadbackRing {
    pub fn new(gpu: &Gpu, size: (u32, u32), slots: usize) -> Result<Self> {
        ensure!(
            size.0 > 0 && size.1 > 0 && slots > 0,
            "readback requires a nonempty extent and ring"
        );
        let row = size.0.checked_mul(4).context("readback row overflow")?;
        let stride =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let slots = (0..slots)
            .map(|_| ReadbackSlot {
                buffer: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("readback"),
                    size: u64::from(stride) * u64::from(size.1),
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                ready: Arc::new(Mutex::new(None)),
                tag: 0,
                submission: None,
            })
            .collect();
        Ok(Self {
            slots,
            flight: VecDeque::new(),
            size,
            stride,
        })
    }

    pub fn in_flight(&self) -> usize {
        self.flight.len()
    }
    pub fn can_submit(&self) -> bool {
        self.flight.len() < self.slots.len()
    }

    /// Submit the world/present recording and its readback copy in one ordered batch.
    pub fn submit(
        &mut self,
        gpu: &Gpu,
        mut encoder: wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        tag: u64,
    ) -> Result<bool> {
        if !self.can_submit() {
            return Ok(false);
        }
        ensure!(
            texture.width() == self.size.0 && texture.height() == self.size.1,
            "readback texture extent changed"
        );
        ensure!(
            matches!(
                texture.format(),
                wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb
            ),
            "readback expects RGBA8 texture"
        );
        let i = (0..self.slots.len())
            .find(|i| !self.flight.contains(i))
            .expect("free readback slot");
        let slot = &mut self.slots[i];
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &slot.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.stride),
                    rows_per_image: Some(self.size.1),
                },
            },
            wgpu::Extent3d {
                width: self.size.0,
                height: self.size.1,
                depth_or_array_layers: 1,
            },
        );
        slot.tag = tag;
        slot.submission = Some(gpu.queue.submit([encoder.finish()]));
        let ready = slot.ready.clone();
        slot.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                *ready.lock().expect("readback callback mutex") = Some(result);
            });
        self.flight.push_back(i);
        Ok(true)
    }

    pub fn poll(&mut self, gpu: &Gpu, mut done: impl FnMut(u64, &[u8])) -> Result<usize> {
        gpu.device.poll(wgpu::PollType::Poll)?;
        let mut count = 0;
        while let Some(&i) = self.flight.front() {
            let slot = &mut self.slots[i];
            let Some(result) = slot.ready.lock().expect("readback callback mutex").take() else {
                break;
            };
            if let Err(error) = result {
                slot.submission = None;
                self.flight.pop_front();
                return Err(error).context("map readback buffer");
            }
            let view = slot.buffer.slice(..).get_mapped_range()?;
            let row = self.size.0 as usize * 4;
            let bytes: Vec<u8> = view
                .chunks_exact(self.stride as usize)
                .flat_map(|r| r[..row].iter().copied())
                .collect();
            drop(view);
            slot.buffer.unmap();
            slot.submission = None;
            self.flight.pop_front();
            done(slot.tag, &bytes);
            count += 1;
        }
        Ok(count)
    }

    pub fn drain(&mut self, gpu: &Gpu, mut done: impl FnMut(u64, &[u8])) -> Result<usize> {
        let mut count = 0;
        while let Some(&i) = self.flight.front() {
            gpu.device.poll(wgpu::PollType::Wait {
                submission_index: self.slots[i].submission.clone(),
                timeout: Some(Duration::from_secs(30)),
            })?;
            count += self.poll(gpu, &mut done)?;
        }
        Ok(count)
    }
}

pub struct Headless {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub readback: ReadbackRing,
}

impl Headless {
    pub fn new(gpu: &Gpu, size: (u32, u32), slots: usize) -> Result<Self> {
        let texture = gpu.texture(
            "headless output",
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            wgpu::TextureDimension::D2,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let view = texture.create_view(&Default::default());
        let readback = ReadbackRing::new(gpu, size, slots)?;
        Ok(Self {
            texture,
            view,
            readback,
        })
    }
}
