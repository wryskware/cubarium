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
}

impl PanelImage {
    pub fn new(gpu: &Gpu, size: (u32, u32)) -> Result<Self> {
        let target = Headless::new(gpu, size, 1)?;
        let mut scanout = ScanoutBuffer::new(Path::new("/dev/dri/card0"), size.0, size.1)?;
        scanout.clear();
        Ok(Self {
            target,
            scanout,
            size,
        })
    }

    pub fn export(&self) -> Result<OwnedFd> {
        self.scanout.export()
    }

    pub fn pitch(&self) -> u32 {
        self.scanout.pitch()
    }

    pub fn rgba(&self) -> Vec<u8> {
        self.scanout.read_rgba()
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
        let mut completed = false;
        let mut copied = Ok(());
        self.target.readback.drain_rows(gpu, |_, rows| {
            completed = true;
            copied = self.scanout.write_rows(rows.data, rows.stride);
        })?;
        ensure!(completed, "panel frame was not submitted");
        copied
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
        let result = self.wait(gpu);
        source.frame_retired();
        result?;
        Ok(source.gpu_ms(gpu))
    }
}
