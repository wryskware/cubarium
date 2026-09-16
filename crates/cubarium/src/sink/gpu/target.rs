//! Where `--sink gpu`'s frames go: the panel, a development window, or nowhere.

use anyhow::{Context, Result};
use clap::ValueEnum;

use cubarium_gpu::render::Renderer;
use cubarium_gpu::scene::Scene;
use cubarium_gpu::target::Headless;
use cubarium_gpu::vk::Gpu;

/// `--gpu-target`.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuTargetKind {
    /// The board: dma-bufs handed to `cube-screen-shim` over its frame socket.
    Shim,
    /// The desktop: a window showing the world raster at the largest whole upscale that
    /// fits, read back from the GPU each frame.
    Window,
    /// Render and drop. For measuring the renderer, and for `--gpu-capture`.
    Headless,
}

impl GpuTargetKind {
    /// What `--sink gpu` picks when `--gpu-target` is not given: the panel where the
    /// daemon's socket exists, and a window where it does not.
    pub fn detect() -> GpuTargetKind {
        if std::path::Path::new(cubarium_gpu::target::SHIM_SOCKET).exists() {
            GpuTargetKind::Shim
        } else {
            GpuTargetKind::Window
        }
    }
}

/// One open target.
pub enum GpuTarget {
    Shim(Box<cubarium_gpu::target::ShimScanout>),
    Window(Box<WindowTarget>),
    Headless(Box<Headless>),
}

impl GpuTarget {
    pub fn open(kind: GpuTargetKind, gpu: &Gpu, renderer: &mut Renderer) -> Result<GpuTarget> {
        match kind {
            GpuTargetKind::Shim => {
                let shim = cubarium_gpu::target::ShimScanout::open(gpu, renderer, 1)
                    .context("attaching to cube-screen-shim's frame socket")?;
                Ok(GpuTarget::Shim(Box::new(shim)))
            }
            GpuTargetKind::Window => Ok(GpuTarget::Window(Box::new(WindowTarget::open(
                gpu, renderer,
            )?))),
            GpuTargetKind::Headless => {
                Ok(GpuTarget::Headless(Box::new(Headless::new(gpu, renderer)?)))
            }
        }
    }

    /// Draw one scene. Returns the GPU milliseconds the timestamps saw.
    pub fn draw(&mut self, gpu: &Gpu, renderer: &mut Renderer, scene: &Scene) -> Result<f64> {
        match self {
            GpuTarget::Shim(t) => Ok(t.draw(gpu, renderer, scene)?.0),
            GpuTarget::Window(t) => t.draw(gpu, renderer, scene),
            GpuTarget::Headless(t) => t.draw(gpu, renderer, scene),
        }
    }

    pub fn should_quit(&mut self) -> bool {
        match self {
            GpuTarget::Window(t) => !t.open,
            _ => false,
        }
    }

    pub fn finish(&mut self, _gpu: &Gpu, _renderer: &mut Renderer) -> Result<()> {
        Ok(())
    }

    pub fn destroy(&mut self, gpu: &Gpu) {
        match self {
            GpuTarget::Shim(t) => t.destroy(gpu),
            GpuTarget::Window(t) => t.destroy(gpu),
            GpuTarget::Headless(t) => t.destroy(gpu),
        }
    }
}

/// A development window.
///
/// It is **`minifb`, not a Vulkan swapchain**, and that is deliberate. `winit` resolves
/// 110 packages — wayland, x11rb, objc2, android — into the workspace's shared lockfile
/// for a window the board never opens, and `minifb` is already a dependency of
/// `PreviewSink`. The price is one `read_raster` per frame: about a millisecond on a
/// desktop GPU at ring resolution, on a path whose whole purpose is that a person is
/// looking at it. The board's target reads nothing back.
pub struct WindowTarget {
    window: minifb::Window,
    headless: Headless,
    buffer: Vec<u32>,
    zoom: usize,
    size: (usize, usize),
    pub open: bool,
}

impl WindowTarget {
    fn open(gpu: &Gpu, renderer: &mut Renderer) -> Result<WindowTarget> {
        let (w, h) = (renderer.layout.w as usize, renderer.layout.h as usize);
        // The largest whole upscale that fits a 1,600 x 900 desktop area: nearest and
        // integer, exactly as the panel gets it.
        let zoom = ((1600 / w).min(900 / h)).max(1);
        let size = (w * zoom, h * zoom);
        let window = minifb::Window::new(
            "cubarium — ring (GPU)",
            size.0,
            size.1,
            minifb::WindowOptions::default(),
        )
        .context("opening the --gpu-target window")?;
        Ok(WindowTarget {
            window,
            headless: Headless::new(gpu, renderer)?,
            buffer: vec![0; size.0 * size.1],
            zoom,
            size,
            open: true,
        })
    }

    fn draw(&mut self, gpu: &Gpu, renderer: &mut Renderer, scene: &Scene) -> Result<f64> {
        let ms = self.headless.draw(gpu, renderer, scene)?;
        let rgba = renderer.read_raster(gpu)?;
        let (w, h) = (renderer.layout.w as usize, renderer.layout.h as usize);
        for y in 0..self.size.1 {
            let sy = y / self.zoom;
            for x in 0..self.size.0 {
                let i = (sy * w + x / self.zoom) * 4;
                self.buffer[y * self.size.0 + x] = u32::from(rgba[i]) << 16
                    | u32::from(rgba[i + 1]) << 8
                    | u32::from(rgba[i + 2]);
            }
        }
        let _ = h;
        if !self.window.is_open() || self.window.is_key_down(minifb::Key::Escape) {
            self.open = false;
            return Ok(ms);
        }
        self.window
            .update_with_buffer(&self.buffer, self.size.0, self.size.1)
            .context("updating the --gpu-target window")?;
        Ok(ms)
    }

    fn destroy(&mut self, gpu: &Gpu) {
        self.headless.destroy(gpu);
    }
}
