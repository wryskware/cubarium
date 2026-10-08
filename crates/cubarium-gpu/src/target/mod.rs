//! Where a rendered frame goes.
//!
//! * [`Headless`] — no target at all: render the world raster and read it back. The
//!   golden-image test and every desktop screenshot use this, and so does the board
//!   when it is measuring the scene passes without the panel in the way.
//! * [`ShimScanout`] (feature `scanout`) — **the board, in production**: the same
//!   exported `VK_IMAGE_TILING_LINEAR` dma-bufs, handed to `cube-screen-shim` over
//!   GS-2's `SOCK_SEQPACKET` socket. The daemon keeps DRM master and flips, so the
//!   service stays up and two workers can be on the device at once.
//! * [`Scanout`] (feature `scanout`) — the same images flipped by *this* process, which
//!   needs DRM master and therefore the daemon stopped. Kept because it is the path the
//!   spike measured end to end, so it is the control when a socket frame looks wrong.
//! * [`window::WindowPresenter`] — the desktop development window: a swapchain on a
//!   window the host opened (`minifb`'s, through its raw handles), presented on a thread
//!   of its own so that a present that blocks — a hidden window on Wayland — stops the
//!   picture and not the world.
//! * A second desktop window — a `winit` window and an `ash` swapchain — lives in the
//!   standalone `crates/cubarium-gpu/window` crate, for the ring's synthetic scene. It is
//!   out of this crate only so that `winit`'s 110-package tail stays out of the
//!   workspace's shared lockfile and off the board.

use anyhow::{Context, Result, ensure};
use ash::vk;

use crate::present::{FrameSource, PresentPass, WindowFit};
use crate::render::{RASTER_FORMAT, TargetImage, framebuffer};
use crate::vk::Gpu;

pub mod presenter;
pub mod readback;
pub mod window;

#[cfg(feature = "scanout")]
pub mod dmabuf;
#[cfg(feature = "scanout")]
pub mod kms;
#[cfg(feature = "scanout")]
mod scanout;
#[cfg(feature = "scanout")]
mod shim;
#[cfg(feature = "scanout")]
pub use scanout::Scanout;
#[cfg(feature = "scanout")]
pub use shim::{PANEL as SHIM_PANEL, SOCKET as SHIM_SOCKET, ShimPresenter, ShimScanout};

/// Render into the world raster and nothing else.
pub struct Headless {
    command_buffer: vk::CommandBuffer,
    fence: vk::Fence,
    output: Option<HeadlessOutput>,
}

struct HeadlessOutput {
    image: TargetImage,
    memory: vk::DeviceMemory,
    sampler: vk::Sampler,
    pass: PresentPass,
    entry: (vk::RenderPass, vk::Pipeline),
    command_buffer: vk::CommandBuffer,
    size: (u32, u32),
    fit: WindowFit,
}

impl Headless {
    pub fn new<S: FrameSource>(gpu: &Gpu, src: &S) -> Result<Headless> {
        let d = &gpu.device;
        let command_buffer = unsafe {
            d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(src.command_pool())
                    .command_buffer_count(1),
            )
        }?[0];
        let fence = unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?;
        Ok(Headless {
            command_buffer,
            fence,
            output: None,
        })
    }

    /// Draw the native raster and the normal letterboxed presentation pass into an
    /// offscreen target of `size`, pixels. No window or display transport is opened.
    pub fn with_size<S: FrameSource>(gpu: &Gpu, src: &S, size: (u32, u32)) -> Result<Headless> {
        ensure!(
            size.0 > 0 && size.1 > 0,
            "headless target must have positive dimensions"
        );
        let raster = src
            .raster_view()
            .context("renderer does not lend its raster")?;
        let mut target = Self::new(gpu, src)?;
        let d = &gpu.device;
        let sampler = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST),
                None,
            )
        }?;
        let mut pass = PresentPass::new(gpu, raster, sampler)?;
        let entry = pass.entry(
            gpu,
            RASTER_FORMAT,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;
        let (image, memory) = gpu.image(
            size.0,
            size.1,
            RASTER_FORMAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::SAMPLED,
        )?;
        let view = gpu.view(image, RASTER_FORMAT)?;
        let framebuffer = framebuffer(d, entry.0, view, size.0, size.1)?;
        let command_buffer = unsafe {
            d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(src.command_pool())
                    .command_buffer_count(1),
            )
        }?[0];
        target.output = Some(HeadlessOutput {
            image: TargetImage {
                image,
                view,
                framebuffer,
            },
            memory,
            sampler,
            pass,
            entry,
            command_buffer,
            size,
            fit: WindowFit::fit(src.raster_size(), size, false),
        });
        Ok(target)
    }

    /// Draw one frame and wait for it, including any sized presentation attachment.
    /// Returns the renderer's timestamp milliseconds (its native passes only).
    pub fn draw<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        src: &mut S,
        frame: S::Frame<'_>,
    ) -> Result<f64> {
        let d = &gpu.device;
        unsafe {
            d.reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
        }?;
        src.record_frame(gpu, self.command_buffer, frame, None)?;
        let mut commands = [self.command_buffer; 2];
        if let Some(output) = &self.output {
            unsafe {
                d.reset_command_buffer(
                    output.command_buffer,
                    vk::CommandBufferResetFlags::empty(),
                )?;
                d.begin_command_buffer(
                    output.command_buffer,
                    &vk::CommandBufferBeginInfo::default(),
                )?;
                output.pass.record_push(
                    d,
                    output.command_buffer,
                    &output.image,
                    output.size,
                    output.fit.push(),
                    output.entry.0,
                    output.entry.1,
                );
                d.end_command_buffer(output.command_buffer)?;
            }
            commands[1] = output.command_buffer;
        }
        let commands = &commands[..if self.output.is_some() { 2 } else { 1 }];
        unsafe {
            d.reset_fences(&[self.fence])?;
        }
        gpu.submit(
            &[vk::SubmitInfo::default().command_buffers(commands)],
            self.fence,
        )?;
        unsafe {
            d.wait_for_fences(&[self.fence], true, u64::MAX)?;
        }
        // This target waits for its own frame, so the frame is done here.
        src.frame_retired();
        Ok(src.gpu_ms(gpu))
    }

    /// The world raster as `w × h` RGBA8, already sRGB-encoded by the attachment — the
    /// bytes a PNG wants.
    pub fn read<S: FrameSource>(&self, gpu: &Gpu, src: &S) -> Result<Vec<u8>> {
        src.read_raster(gpu)
    }

    /// Read the optional sized presentation attachment; otherwise read the native raster.
    pub fn read_output<S: FrameSource>(&self, gpu: &Gpu, src: &S) -> Result<Vec<u8>> {
        let Some(output) = &self.output else {
            return self.read(gpu, src);
        };
        let (w, h) = output.size;
        let size = u64::from(w) * u64::from(h) * 4;
        let host = gpu.readback_buffer(size)?;
        gpu.one_shot(src.command_pool(), |cb| unsafe {
            crate::vk::barrier(
                &gpu.device,
                cb,
                output.image.image,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            );
            gpu.device.cmd_copy_image_to_buffer(
                cb,
                output.image.image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                host.buffer,
                &[vk::BufferImageCopy::default()
                    .buffer_row_length(w)
                    .buffer_image_height(h)
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D {
                        width: w,
                        height: h,
                        depth: 1,
                    })],
            );
            crate::vk::barrier(
                &gpu.device,
                cb,
                output.image.image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        })?;
        let bytes = unsafe { host.bytes() }[..size as usize].to_vec();
        host.destroy(gpu);
        Ok(bytes)
    }

    pub fn destroy(&self, gpu: &Gpu) {
        if let Some(output) = &self.output {
            output.pass.destroy(gpu);
            unsafe {
                gpu.device
                    .destroy_framebuffer(output.image.framebuffer, None);
                gpu.device.destroy_image_view(output.image.view, None);
                gpu.device.destroy_image(output.image.image, None);
                gpu.device.free_memory(output.memory, None);
                gpu.device.destroy_sampler(output.sampler, None);
            }
        }
        unsafe { gpu.device.destroy_fence(self.fence, None) };
    }
}

/// Write an RGBA8 buffer out as a PNG.
pub fn write_png(path: &std::path::Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(())
}

/// Read an RGBA8 PNG back.
pub fn read_png(path: &std::path::Path) -> Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buffer)?;
    buffer.truncate(info.buffer_size());
    Ok((info.width, info.height, buffer))
}
