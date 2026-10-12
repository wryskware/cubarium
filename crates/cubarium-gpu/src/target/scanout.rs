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

use anyhow::{Result, anyhow};
use drm::buffer::DrmFourcc;

use super::kms::Output;
use super::panel_readback::PanelImage;
use crate::render::PresentTransform;
use crate::wgpu_backend::{FrameSource, Gpu};

const RING: usize = 2;

struct Frame {
    image: PanelImage,
    fb: drm::control::framebuffer::Handle,
}

pub struct Scanout {
    output: Output,
    frames: Vec<Frame>,
    transform: PresentTransform,
    index: usize,
}

impl Scanout {
    /// Take DRM master, allocate the scanout ring and set the mode.
    pub fn open<S: FrameSource>(
        gpu: &Gpu,
        src: &mut S,
        connector: &str,
        quarter_turns: u32,
    ) -> Result<Self> {
        let mut output = Output::open(Path::new("/dev/dri/card0"), connector)?;
        let (w, h) = (output.width, output.height);
        let raster = src.raster_size();
        let transform =
            PresentTransform::fit(raster, (w, h), quarter_turns, false).ok_or_else(|| {
                anyhow!(
                    "a {}x{} raster does not fit {w}x{h} at {quarter_turns} quarter turn(s)",
                    raster.0,
                    raster.1
                )
            })?;
        let mut frames = Vec::with_capacity(RING);
        for _ in 0..RING {
            let image = PanelImage::new(gpu, (w, h))?;
            let fb = output.import_dmabuf(
                image.export()?,
                w,
                h,
                DrmFourcc::Xrgb8888,
                image.pitch(),
                0,
            )?;
            frames.push(Frame { image, fb });
        }
        output.set_crtc(frames[0].fb)?;
        println!(
            "direct scanout: {RING} DRM buffers, AddFB2 accepted at pitch {}, wgpu readback",
            frames[0].image.pitch()
        );
        Ok(Self {
            output,
            frames,
            transform,
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

    /// Render into the next released image and wait for its flip to finish.
    pub fn draw<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        src: &mut S,
        frame: S::Frame<'_>,
    ) -> Result<(f64, f64, f64)> {
        let i = self.index;
        self.index = (self.index + 1) % RING;
        let slot = &mut self.frames[i];
        let start = Instant::now();
        let ms = slot.image.draw(gpu, src, frame, self.transform)?;
        let submitted = Instant::now();
        self.output.flip(slot.fb)?;
        let flipped = Instant::now();
        Ok((
            ms,
            (submitted - start).as_secs_f64() * 1e3,
            (flipped - submitted).as_secs_f64() * 1e3,
        ))
    }

    pub fn read_scanout<S: FrameSource>(
        &self,
        _gpu: &Gpu,
        _src: &S,
    ) -> Result<(u32, u32, Vec<u8>)> {
        let i = (self.index + RING - 1) % RING;
        Ok((
            self.output.width,
            self.output.height,
            self.frames[i].image.rgba().to_vec(),
        ))
    }

    pub fn destroy(&mut self, _gpu: &Gpu) {
        self.frames.clear();
    }
}
