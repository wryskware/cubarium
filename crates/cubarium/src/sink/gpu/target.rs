//! Safe targets for the shared wgpu frame boundary.
#![forbid(unsafe_code)]

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::ValueEnum;
use cubarium_gpu::wgpu_backend::{FrameSource, Gpu};
use cubarium_gpu::target::Headless;

use super::window_target::WgpuWindowTarget;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuTargetKind {
    Shim,
    Window,
    Headless,
}

impl GpuTargetKind {
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        if std::path::Path::new(cubarium_gpu::target::SHIM_SOCKET).exists() {
            return Self::Shim;
        }
        Self::Window
    }
}

pub enum GpuTarget {
    #[cfg(target_os = "linux")]
    Shim(Box<cubarium_gpu::target::ShimScanout>),
    #[cfg(target_os = "linux")]
    ShimThread(Box<cubarium_gpu::target::ShimPresenter>),
    WindowThread(Box<WgpuWindowTarget>),
    Window(Box<WindowTarget>),
    Headless(Box<Headless>),
}

impl GpuTarget {
    /// Select the device against the owned display surface before creating any
    /// renderer resources. Readback fallback still uses a headless device.
    pub fn prepare(kind: GpuTargetKind, title: &str, raster_size: (u32, u32), required_sampled_textures: u32)
        -> Result<(Arc<Gpu>, Option<WgpuWindowTarget>)>
    {
        if kind == GpuTargetKind::Window {
            crate::sink::hidden::window_allowed()?;
            match WgpuWindowTarget::prepare(title, raster_size) {
                Ok((gpu, window)) => {
                    let available = gpu.device.limits().max_sampled_textures_per_shader_stage;
                    if available >= required_sampled_textures {
                        return Ok((gpu, Some(window)));
                    }
                    eprintln!("cubarium: GPU development window supports {available} sampled textures per stage, renderer needs {required_sampled_textures}; reading the raster back instead");
                }
                Err(error) => eprintln!("cubarium: no GPU development window ({error:#}); reading the raster back instead"),
            }
        }
        if kind == GpuTargetKind::Window && std::env::var_os("CUBARIUM_HIDDEN_LAUNCH").is_some() {
            anyhow::bail!("hidden window initialization failed; refusing a second fallback window");
        }
        Ok((Arc::new(crate::sink::gpu::open_device().context("opening the GPU device for --sink gpu")?), None))
    }

    pub fn open<S: FrameSource>(
        kind: GpuTargetKind, gpu: &Arc<Gpu>, src: &mut S, title: &str, present_thread: bool,
        window: Option<WgpuWindowTarget>,
    ) -> Result<Self> {
        match kind {
            #[cfg(target_os = "linux")]
            GpuTargetKind::Shim if present_thread => {
                Ok(Self::ShimThread(Box::new(cubarium_gpu::target::ShimPresenter::open(gpu.clone(), src, 1)
                    .context("attaching to cube-screen-shim's frame socket")?)))
            }
            #[cfg(target_os = "linux")]
            GpuTargetKind::Shim => {
                Ok(Self::Shim(Box::new(cubarium_gpu::target::ShimScanout::open(gpu, src, 1)
                    .context("attaching to cube-screen-shim's frame socket")?)))
            }
            #[cfg(not(target_os = "linux"))]
            GpuTargetKind::Shim => {
                let _ = present_thread;
                anyhow::bail!("GPU hardware-panel output is supported only on Linux")
            }
            GpuTargetKind::Window => {
                crate::sink::hidden::window_allowed()?;
                match window {
                    Some(window) => Ok(Self::WindowThread(Box::new(window))),
                    None => Ok(Self::Window(Box::new(WindowTarget::open(gpu, src, title)?))),
                }
            }
            GpuTargetKind::Headless => Ok(Self::Headless(Box::new(Headless::new(gpu, src)?))),
        }
    }

    pub fn draw<S: FrameSource>(&mut self, gpu: &Gpu, src: &mut S, frame: S::Frame<'_>) -> Result<f64> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Shim(target) => Ok(target.draw(gpu, src, frame)?.0),
            #[cfg(target_os = "linux")]
            Self::ShimThread(target) => target.draw(gpu, src, frame),
            Self::WindowThread(target) => target.draw(src, frame),
            Self::Window(target) => target.draw(gpu, src, frame),
            Self::Headless(target) => target.draw(gpu, src, frame),
        }
    }

    pub fn presented(&self) -> Option<(u64, u64)> {
        match self {
            #[cfg(target_os = "linux")]
            Self::ShimThread(target) => Some((target.presented(), target.skipped())),
            Self::WindowThread(target) => Some(target.presented()),
            _ => None,
        }
    }

    pub fn sample(&self) -> Option<cubarium_gpu::target::presenter::PresentSample> {
        match self {
            #[cfg(target_os = "linux")]
            Self::ShimThread(target) => Some(target.sample()),
            _ => None,
        }
    }

    pub fn should_quit(&mut self) -> bool {
        match self {
            Self::WindowThread(target) => target.should_quit(),
            Self::Window(target) => !target.open,
            _ => false,
        }
    }

    pub fn take_save_request(&mut self) -> bool {
        match self {
            Self::WindowThread(target) => target.take_save(),
            Self::Window(target) => std::mem::take(&mut target.save),
            _ => false,
        }
    }

    pub fn finish<S: FrameSource>(&mut self, gpu: &Gpu, src: &mut S) -> Result<()> {
        match self {
            Self::WindowThread(target) => target.finish(src)?,
            #[cfg(target_os = "linux")]
            Self::ShimThread(target) => target.finish(gpu, src)?,
            _ => {}
        }
        let _ = gpu;
        Ok(())
    }

    pub fn destroy<S: FrameSource>(&mut self, gpu: &Gpu, src: &mut S) {
        if let Err(error) = self.finish(gpu, src) {
            eprintln!("cubarium: development window completion failed: {error:#}");
        }
        match self {
            #[cfg(target_os = "linux")]
            Self::Shim(target) => target.destroy(gpu),
            #[cfg(target_os = "linux")]
            Self::ShimThread(target) => target.destroy(gpu),
            Self::Window(target) => target.headless.destroy(gpu),
            Self::Headless(target) => target.destroy(gpu),
            _ => {}
        }
    }
}

pub struct WindowTarget {
    window: minifb::Window,
    headless: Headless,
    readback: cubarium_gpu::wgpu_backend::ReadbackRing,
    rgba: Vec<u8>,
    buffer: Vec<u32>,
    zoom: usize,
    size: (usize, usize),
    open: bool,
    save: bool,
}

impl WindowTarget {
    fn open<S: FrameSource>(gpu: &Gpu, src: &mut S, title: &str) -> Result<Self> {
        let (rw, rh) = src.raster_size();
        let (w, h) = (rw as usize, rh as usize);
        let zoom = ((1600 / w.max(1)).min(900 / h.max(1))).max(1);
        let size = (w * zoom, h * zoom);
        crate::sink::hidden::window_allowed()?;
        let window = minifb::Window::new(title, size.0, size.1, minifb::WindowOptions::default())
            .context("opening the --gpu-target readback window")?;
        Ok(Self { window, headless: Headless::new(gpu, src)?, readback: cubarium_gpu::wgpu_backend::ReadbackRing::new(gpu, src.raster_size(), 1)?, rgba: Vec::new(), buffer: vec![0; size.0 * size.1], zoom, size, open: true, save: false })
    }

    fn draw<S: FrameSource>(&mut self, gpu: &Gpu, src: &mut S, frame: S::Frame<'_>) -> Result<f64> {
        anyhow::ensure!(self.readback.submit_source(gpu, src, frame, 0)?, "fallback readback has no free slot");
        self.readback.drain_source_rows(gpu, src, |_, rows| rows.copy_into(&mut self.rgba))?;
        let ms = src.gpu_ms(gpu);
        upscale(&self.rgba, src.raster_size().0 as usize, self.zoom, &mut self.buffer);
        self.open = self.window.is_open() && !self.window.is_key_down(minifb::Key::Escape);
        if !self.open { return Ok(ms); }
        self.save |= self.window.is_key_pressed(minifb::Key::S, minifb::KeyRepeat::No)
            && (self.window.is_key_down(minifb::Key::LeftCtrl) || self.window.is_key_down(minifb::Key::RightCtrl));
        self.window.update_with_buffer(&self.buffer, self.size.0, self.size.1)
            .context("updating the --gpu-target readback window")?;
        Ok(ms)
    }
}

fn upscale(rgba: &[u8], w: usize, zoom: usize, out: &mut [u32]) {
    for (i, p) in rgba.chunks_exact(4).enumerate() {
        let colour = u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2]);
        let (x, y) = (i % w, i / w);
        for dy in 0..zoom {
            let row = (y * zoom + dy) * w * zoom + x * zoom;
            out[row..row + zoom].fill(colour);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_window_upscale_is_nearest_at_the_zoom() {
        let rgba = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255];
        let mut out = [0; 16];
        super::upscale(&rgba, 2, 2, &mut out);
        assert_eq!(out, [0xff0000, 0xff0000, 0x00ff00, 0x00ff00, 0xff0000, 0xff0000,
                        0x00ff00, 0x00ff00, 0x0000ff, 0x0000ff, 0xffffff, 0xffffff,
                        0x0000ff, 0x0000ff, 0xffffff, 0xffffff]);
    }
}
