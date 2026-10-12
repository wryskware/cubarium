//! Safe presentation into a DRM allocation for the legacy panel transports.

use std::os::fd::OwnedFd;
use std::path::Path;

use anyhow::{Result, ensure};

use super::scanout_buffer::ScanoutBuffer;
use crate::render::PresentTransform;
use crate::wgpu_backend::{FrameSource, Gpu, Headless, PresentPass, TargetSlot};

/// The GPU target and the CPU-writable scanout allocation have separate ownership.
/// Callers may draw only after the panel has released this image's slot.
pub struct PanelImage {
    target: Headless,
    scanout: ScanoutBuffer,
    size: (u32, u32),
    latest: Vec<u8>,
}

impl PanelImage {
    pub fn new(gpu: &Gpu, size: (u32, u32)) -> Result<Self> {
        let target = Headless::new(gpu, size, 1)?;
        let mut scanout = ScanoutBuffer::new(Path::new("/dev/dri/card0"), size.0, size.1)?;
        let mut latest = vec![0; size.0 as usize * size.1 as usize * 4];
        for pixel in latest.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        scanout.write_rgba(&latest)?;
        Ok(Self {
            target,
            scanout,
            size,
            latest,
        })
    }

    pub fn export(&self) -> Result<OwnedFd> {
        self.scanout.export()
    }

    pub fn pitch(&self) -> u32 {
        self.scanout.pitch()
    }

    pub fn rgba(&self) -> &[u8] {
        &self.latest
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.target.view
    }

    /// Append readback and submit the recorder's owned command encoder.
    pub fn submit(&mut self, gpu: &Gpu, encoder: wgpu::CommandEncoder) -> Result<()> {
        ensure!(
            self.target.readback.can_submit(),
            "panel readback is still in flight"
        );
        ensure!(
            self.target
                .readback
                .submit(gpu, encoder, &self.target.texture, 0)?,
            "panel readback refused a reserved frame"
        );
        Ok(())
    }

    /// Finish readback and the CPU copy before promising a finished buffer to DRM.
    pub fn wait(&mut self, gpu: &Gpu) -> Result<()> {
        let mut completed = None;
        self.target
            .readback
            .drain(gpu, |_, rgba| completed = Some(rgba.to_vec()))?;
        let rgba = completed.ok_or_else(|| anyhow::anyhow!("panel frame was not submitted"))?;
        self.scanout.write_rgba(&rgba)?;
        self.latest = rgba;
        Ok(())
    }

    pub fn draw<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        source: &mut S,
        frame: S::Frame<'_>,
        transform: PresentTransform,
    ) -> Result<f64> {
        ensure!(
            source.frames_in_flight() < source.frame_capacity(),
            "renderer is at frame capacity"
        );
        let mut encoder = gpu.encoder("panel frame");
        let target = TargetSlot {
            view: &self.target.view,
            size: self.size,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            transform: PresentPass::panel_transform(transform, self.size),
        };
        source.record_frame(gpu, &mut encoder, frame, Some(target))?;
        if let Err(error) = self.submit(gpu, encoder) {
            source.frame_discarded();
            return Err(error);
        }
        self.wait(gpu)?;
        source.frame_retired();
        Ok(source.gpu_ms(gpu))
    }
}
