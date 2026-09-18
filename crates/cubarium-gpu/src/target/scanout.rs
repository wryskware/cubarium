//! The board, taking DRM master directly — the spike's own path, kept as a fallback.
//!
//! **Prefer [`super::ShimScanout`].** Since GS-2 landed, `cube-screen-shim` accepts
//! dma-buf frames over a socket and keeps DRM master itself, so the production route
//! neither stops the service nor competes for mastership. This path is here because it
//! is the one `gpu-scanout-spike-2026-09-16.md` measured end to end, so it is the
//! control when a socket frame looks wrong: it answers "is it the renderer or the
//! handoff?" without changing anything else.
//!
//! It takes DRM master, so `cube-screen-shim` must be stopped first and started again
//! afterwards. It must never be used while another worker is on the display.

use std::path::Path;
use std::time::Instant;

use anyhow::{Result, anyhow, bail};
use ash::vk;
use drm::buffer::DrmFourcc;

use super::dmabuf::{self, LinearImage};
use super::kms::Output;
use crate::present::FrameSource;
use crate::render::{PresentTransform, TargetImage, framebuffer};
use crate::vk::Gpu;

const RING: usize = 2;

struct Frame {
    image: LinearImage,
    target: TargetImage,
    fb: drm::control::framebuffer::Handle,
    command_buffer: vk::CommandBuffer,
    fence: vk::Fence,
}

pub struct Scanout {
    output: Output,
    frames: Vec<Frame>,
    transform: PresentTransform,
    view_format: vk::Format,
    index: usize,
}

impl Scanout {
    /// Take DRM master, allocate the scanout ring and set the mode.
    pub fn open<S: FrameSource>(
        gpu: &Gpu,
        src: &mut S,
        connector: &str,
        quarter_turns: u32,
    ) -> Result<Scanout> {
        if !gpu.has_dma_buf {
            bail!("this device has no VK_EXT_external_memory_dma_buf; scanout is impossible");
        }
        dmabuf::linear_export_supported(gpu)?;
        let shader_encode = !dmabuf::srgb_view_supported(gpu);
        let mut output = Output::open(Path::new("/dev/dri/card0"), connector)?;
        let (w, h) = (output.width, output.height);
        let raster = src.raster_size();
        let transform = PresentTransform::fit(raster, (w, h), quarter_turns, shader_encode)
            .ok_or_else(|| {
                anyhow!(
                    "a {}x{} raster does not fit {w}x{h} at {quarter_turns} quarter turn(s)",
                    raster.0,
                    raster.1
                )
            })?;
        let view_format = if shader_encode {
            dmabuf::FORMAT
        } else {
            vk::Format::B8G8R8A8_SRGB
        };
        let pass = src.present_pass(gpu, view_format, vk::ImageLayout::GENERAL)?;

        let d = &gpu.device;
        let command_buffers = unsafe {
            d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(src.command_pool())
                    .command_buffer_count(RING as u32),
            )
        }?;
        let mut frames = Vec::with_capacity(RING);
        for i in 0..RING {
            let mut image = dmabuf::export_linear(gpu, w, h, shader_encode)?;
            let fd = image
                .fd
                .take()
                .expect("a freshly exported image has its fd");
            let fb =
                output.import_dmabuf(fd, w, h, DrmFourcc::Xrgb8888, image.pitch, image.offset)?;
            frames.push(Frame {
                target: TargetImage {
                    image: image.image,
                    view: image.view,
                    framebuffer: framebuffer(d, pass, image.view, w, h)?,
                },
                image,
                fb,
                command_buffer: command_buffers[i],
                fence: unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?,
            });
        }
        output.set_crtc(frames[0].fb)?;
        println!(
            "direct scanout: {RING} linear dma-bufs, AddFB2 accepted at pitch {}, sRGB encode by {}",
            frames[0].image.pitch,
            if shader_encode {
                "the present shader"
            } else {
                "the _SRGB attachment"
            }
        );
        Ok(Scanout {
            output,
            frames,
            transform,
            view_format,
            index: 0,
        })
    }

    pub fn width(&self) -> u32 {
        self.output.width
    }

    pub fn height(&self) -> u32 {
        self.output.height
    }

    pub fn refresh_hz(&self) -> f64 {
        self.output.refresh_hz()
    }

    pub fn transform(&self) -> PresentTransform {
        self.transform
    }

    /// Render into the next image of the ring and flip it.
    ///
    /// Returns `(GPU ms, submit..fence ms, flip queue..complete ms)`.
    pub fn draw<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        src: &mut S,
        frame: S::Frame<'_>,
    ) -> Result<(f64, f64, f64)> {
        let d = &gpu.device;
        let i = self.index;
        self.index = (self.index + 1) % RING;
        let slot = &self.frames[i];
        let start = Instant::now();
        unsafe {
            d.reset_command_buffer(slot.command_buffer, vk::CommandBufferResetFlags::empty())
        }?;
        src.record_frame(
            gpu,
            slot.command_buffer,
            frame,
            Some((
                &slot.target,
                (self.output.width, self.output.height),
                self.view_format,
                vk::ImageLayout::GENERAL,
                self.transform,
            )),
        )?;
        let one = [slot.command_buffer];
        unsafe {
            d.reset_fences(&[slot.fence])?;
            d.queue_submit(
                gpu.queue,
                &[vk::SubmitInfo::default().command_buffers(&one)],
                slot.fence,
            )?;
            d.wait_for_fences(&[slot.fence], true, u64::MAX)?;
        }
        let submitted = Instant::now();
        self.output.flip(slot.fb)?;
        let flipped = Instant::now();
        Ok((
            src.gpu_ms(gpu),
            (submitted - start).as_secs_f64() * 1e3,
            (flipped - submitted).as_secs_f64() * 1e3,
        ))
    }

    /// The last scanned-out image, read back through the GPU as RGBA8.
    pub fn read_scanout<S: FrameSource>(&self, gpu: &Gpu, src: &S) -> Result<(u32, u32, Vec<u8>)> {
        let i = (self.index + RING - 1) % RING;
        let rgba = dmabuf::read_back(
            gpu,
            src.command_pool(),
            &self.frames[i].image,
            self.output.width,
            self.output.height,
            vk::ImageLayout::GENERAL,
        )?;
        Ok((self.output.width, self.output.height, rgba))
    }

    pub fn destroy(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            for frame in &self.frames {
                d.destroy_fence(frame.fence, None);
                d.destroy_framebuffer(frame.target.framebuffer, None);
                d.destroy_image_view(frame.image.view, None);
                d.destroy_image(frame.image.image, None);
                d.free_memory(frame.image.memory, None);
            }
        }
        self.frames.clear();
    }
}
