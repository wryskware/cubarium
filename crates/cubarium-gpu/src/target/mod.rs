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
//! * A desktop **window** — a `winit` window and an `ash` swapchain — which lives in
//!   the standalone `crates/cubarium-gpu/window` crate. It drives exactly the passes
//!   below through [`Renderer::record`](crate::render::Renderer::record); it is out of
//!   this crate only so that `winit`'s 110-package tail stays out of the workspace's
//!   shared lockfile and off the board.

use anyhow::Result;
use ash::vk;

use crate::present::FrameSource;
use crate::vk::Gpu;

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
pub use shim::{SOCKET as SHIM_SOCKET, ShimScanout};

/// Render into the world raster and nothing else.
pub struct Headless {
    command_buffer: vk::CommandBuffer,
    fence: vk::Fence,
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
        Ok(Headless { command_buffer, fence })
    }

    /// Draw one frame and wait for it. Returns the GPU milliseconds the timestamps saw.
    pub fn draw<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        src: &mut S,
        frame: S::Frame<'_>,
    ) -> Result<f64> {
        let d = &gpu.device;
        unsafe { d.reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty()) }?;
        src.record_frame(gpu, self.command_buffer, frame, None)?;
        let one = [self.command_buffer];
        unsafe {
            d.reset_fences(&[self.fence])?;
            d.queue_submit(gpu.queue, &[vk::SubmitInfo::default().command_buffers(&one)], self.fence)?;
            d.wait_for_fences(&[self.fence], true, u64::MAX)?;
        }
        Ok(src.gpu_ms(gpu))
    }

    /// The world raster as `w × h` RGBA8, already sRGB-encoded by the attachment — the
    /// bytes a PNG wants.
    pub fn read<S: FrameSource>(&self, gpu: &Gpu, src: &S) -> Result<Vec<u8>> {
        src.read_raster(gpu)
    }

    pub fn destroy(&self, gpu: &Gpu) {
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
