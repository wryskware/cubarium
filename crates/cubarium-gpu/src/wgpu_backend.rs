//! Safe resources for the backend migration. Recording owns uploads: dropping an
//! encoder drops its copies, without a queue write escaping a discarded frame.
#![forbid(unsafe_code)]

pub mod bloom;
pub mod flat;
pub mod geometry;
pub mod headless;
pub mod shader;
pub mod sunvis;
pub mod timing;
pub mod voxel;
pub mod window;

/// Renderer-neutral target for the shared last pass. The target owns submission
/// and completion; a renderer records into its encoder without queue writes.
pub struct TargetSlot<'a> {
    pub view: &'a wgpu::TextureView,
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub transform: [f32; 8],
}

/// Shared safe frame boundary. Recorded frames retire in submission order.
pub trait FrameSource {
    type Frame<'a>;
    fn raster_size(&self) -> (u32, u32);
    fn raster_texture(&self) -> &wgpu::Texture;
    fn raster_view(&self) -> &wgpu::TextureView;
    /// On error, this source rolls back only the failed attempt. The caller drops
    /// the encoder; explicit frame_discarded applies only to successful records.
    fn record_frame(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        frame: Self::Frame<'_>,
        target: Option<TargetSlot<'_>>,
    ) -> Result<()>;
    fn frame_retired(&mut self) {}
    fn frame_discarded(&mut self) {}
    fn frames_in_flight(&self) -> usize {
        0
    }
    fn frame_capacity(&self) -> usize {
        usize::MAX
    }
    fn content_version(&self) -> Option<u64> {
        None
    }
    fn redrew_last(&self) -> bool {
        true
    }
    fn gpu_ms(&self, _gpu: &Gpu) -> f64 {
        0.0
    }
    fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>> {
        let mut readback = ReadbackRing::new(gpu, self.raster_size(), 1)?;
        let mut bytes = Vec::new();
        readback.submit(
            gpu,
            gpu.encoder("raster readback"),
            self.raster_texture(),
            0,
        )?;
        readback.drain(gpu, |_, pixels| bytes.extend_from_slice(pixels))?;
        Ok(bytes)
    }
}

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

static PREFERRED_DEVICE: Mutex<Option<String>> = Mutex::new(None);

/// Keep rendering on the host's adapter when possible. `CUBARIUM_GPU` takes priority.
pub fn prefer_device(name: &str) {
    *PREFERRED_DEVICE.lock().unwrap_or_else(|e| e.into_inner()) = Some(name.to_owned());
}

fn preferred_device() -> Option<String> {
    std::env::var("CUBARIUM_GPU")
        .ok()
        .filter(|name| !name.is_empty())
        .or_else(|| {
            PREFERRED_DEVICE
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        })
}

impl Gpu {
    pub fn open() -> Result<Self> {
        pollster::block_on(Self::open_async())
    }

    /// Create the window's instance and surface before selecting its adapter.
    /// Pass a descriptor created with `new_with_display_handle_from_env`.
    /// The owned display is essential for GLES presentation. The surface retains
    /// the window owner, so neither handle borrows the platform event loop.
    pub fn open_for_window<W>(
        descriptor: wgpu::InstanceDescriptor,
        window: Arc<W>,
    ) -> Result<(Self, wgpu::Surface<'static>)>
    where
        W: wgpu::DisplayAndWindowHandle + 'static,
    {
        ensure!(
            descriptor.display.is_some(),
            "window GPU requires an owned display"
        );
        let backends = descriptor.backends;
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance
            .create_surface(window)
            .context("create owned window surface")?;
        let gpu = pollster::block_on(Self::open_instance(instance, backends, Some(&surface)))?;
        Ok((gpu, surface))
    }

    async fn open_async() -> Result<Self> {
        let descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        let backends = descriptor.backends;
        Self::open_instance(wgpu::Instance::new(descriptor), backends, None).await
    }

    async fn open_instance(
        instance: wgpu::Instance,
        backends: wgpu::Backends,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self> {
        let preferred = if let Some(name) = preferred_device() {
            let name = name.to_lowercase();
            instance
                .enumerate_adapters(backends)
                .await
                .into_iter()
                .find(|adapter| {
                    adapter.get_info().name.to_lowercase().contains(&name)
                        && surface.is_none_or(|surface| adapter.is_surface_supported(surface))
                })
        } else {
            None
        };
        let adapter = match preferred {
            Some(adapter) => adapter,
            None => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: surface,
                    ..Default::default()
                })
                .await
                .context("request wgpu adapter")?,
        };
        let name = adapter.get_info().name;
        let required_features = adapter.features()
            & (wgpu::Features::IMMEDIATES
                | wgpu::Features::TIMESTAMP_QUERY
                | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
                | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
        // Portable defaults cover our 4096x2048 figure atlas, <=4 MiB storage
        // records, <=4 KiB uniforms, six storage buffers and four attachments.
        // Figure composites add four images to the world's seventeen; plants
        // use the largest immediate block (64 bytes). Preserve adapter texture
        // resolution for wide worlds/displays; those caps reserve no images.
        let required_limits = wgpu::Limits {
            max_sampled_textures_per_shader_stage: 21,
            max_immediate_size: if required_features.contains(wgpu::Features::IMMEDIATES) {
                64
            } else {
                0
            },
            ..wgpu::Limits::default().using_resolution(adapter.limits())
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Cubarium"),
                required_features,
                required_limits,
                memory_hints: wgpu::MemoryHints::MemoryUsage,
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
        target_size: (u32, u32),
    ) -> [f32; 8] {
        transform.push(target_size)
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

    pub fn can_submit_source<S: FrameSource>(&self, source: &S) -> bool {
        self.can_submit() && source.frames_in_flight() < source.frame_capacity()
    }

    pub fn submit_source<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        source: &mut S,
        frame: S::Frame<'_>,
        tag: u64,
    ) -> Result<bool> {
        if !self.can_submit_source(source) {
            return Ok(false);
        }
        let mut encoder = gpu.encoder("world and readback");
        source.record_frame(gpu, &mut encoder, frame, None)?;
        match self.submit(gpu, encoder, source.raster_texture(), tag) {
            Ok(true) => Ok(true),
            result => {
                source.frame_discarded();
                result
            }
        }
    }

    pub fn poll_source<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        source: &mut S,
        mut done: impl FnMut(u64, &[u8]),
    ) -> Result<usize> {
        self.poll(gpu, |tag, bytes| {
            source.frame_retired();
            done(tag, bytes);
        })
    }

    pub fn drain_source<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        source: &mut S,
        mut done: impl FnMut(u64, &[u8]),
    ) -> Result<usize> {
        self.drain(gpu, |tag, bytes| {
            source.frame_retired();
            done(tag, bytes);
        })
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
