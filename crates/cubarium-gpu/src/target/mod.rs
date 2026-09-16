//! Where a rendered frame goes.
//!
//! * [`Headless`] — no target at all: render the world raster and read it back. The
//!   golden-image test and every desktop screenshot use this, and so does the board
//!   when it is measuring the scene passes without the panel in the way.
//! * [`Scanout`] (feature `scanout`) — the board: a ring of `VK_IMAGE_TILING_LINEAR`
//!   `B8G8R8A8_UNORM` images exported as dma-bufs, imported into KMS with a
//!   modifier-free `AddFB2` and page-flipped at DP-1. Variant (c) of the spike.
//! * [`Window`] (feature `window`) — a `winit` window and an `ash` swapchain, for
//!   development on the desktop.

use anyhow::Result;
use ash::vk;

use crate::render::Renderer;
use crate::scene::Scene;
use crate::vk::Gpu;

#[cfg(feature = "scanout")]
pub mod kms;
#[cfg(feature = "scanout")]
mod scanout;
#[cfg(feature = "window")]
mod window;

#[cfg(feature = "scanout")]
pub use scanout::Scanout;
#[cfg(feature = "window")]
pub use window::Window;

/// Render into the world raster and nothing else.
pub struct Headless {
    command_buffer: vk::CommandBuffer,
    fence: vk::Fence,
}

impl Headless {
    pub fn new(gpu: &Gpu, renderer: &Renderer) -> Result<Headless> {
        let d = &gpu.device;
        let command_buffer = unsafe {
            d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(renderer.command_pool)
                    .command_buffer_count(1),
            )
        }?[0];
        let fence = unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?;
        Ok(Headless { command_buffer, fence })
    }

    /// Draw one frame and wait for it. Returns the GPU milliseconds the timestamps saw.
    pub fn draw(&mut self, gpu: &Gpu, renderer: &mut Renderer, scene: &Scene) -> Result<f64> {
        let d = &gpu.device;
        unsafe { d.reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty()) }?;
        renderer.record(gpu, self.command_buffer, scene, None)?;
        let one = [self.command_buffer];
        unsafe {
            d.reset_fences(&[self.fence])?;
            d.queue_submit(gpu.queue, &[vk::SubmitInfo::default().command_buffers(&one)], self.fence)?;
            d.wait_for_fences(&[self.fence], true, u64::MAX)?;
        }
        Ok(renderer.gpu_ms(gpu))
    }

    /// The world raster as `w × h` RGBA8, already sRGB-encoded by the attachment — the
    /// bytes a PNG wants.
    pub fn read(&self, gpu: &Gpu, renderer: &Renderer) -> Result<Vec<u8>> {
        renderer.read_raster(gpu)
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
