//! Where `--sink gpu`'s frames go: the panel, a development window, or nowhere.

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::ValueEnum;

use cubarium_gpu::present::FrameSource;
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

/// One open target, for either renderer.
///
/// Nothing here names `Renderer` or `VoxelRenderer`: a target asks its frame source
/// (`cubarium_gpu::present::FrameSource`) for the raster's size, a command pool, the
/// present render pass and one `record_frame`, which is all a window or a panel needs.
/// That is why `--sink gpu` on the ring and on the voxel strip share this file.
pub enum GpuTarget {
    /// The panel, presented on this thread: record, submit, wait, present, return.
    Shim(Box<cubarium_gpu::target::ShimScanout>),
    /// The panel, presented on its own thread — everything after the recording is
    /// waiting, and the run loop has a world to simulate. `--sink gpu` on the voxel strip
    /// asks for this; the ring keeps the synchronous one, because its renderer writes its
    /// instance buffers every frame and has no ring to keep them off a frame in flight.
    ShimThread(Box<cubarium_gpu::target::ShimPresenter>),
    /// The desktop. **Not threaded**: `minifb`'s window is not `Send`, and a person
    /// looking at a development window is not waiting on 60 fps.
    Window(Box<WindowTarget>),
    Headless(Box<Headless>),
}

impl GpuTarget {
    /// Open a target for `src`. `title` names the development window and is ignored by
    /// every other target, and `present_thread` asks the panel to be presented on its own thread. It is ignored
    /// by every other target: the window is not `Send` and the headless path has nothing
    /// to present to.
    pub fn open<S: FrameSource>(
        kind: GpuTargetKind,
        gpu: &Arc<Gpu>,
        src: &mut S,
        title: &str,
        present_thread: bool,
    ) -> Result<GpuTarget> {
        match kind {
            GpuTargetKind::Shim if present_thread => {
                let shim = cubarium_gpu::target::ShimPresenter::open(gpu.clone(), src, 1)
                    .context("attaching to cube-screen-shim's frame socket")?;
                Ok(GpuTarget::ShimThread(Box::new(shim)))
            }
            GpuTargetKind::Shim => {
                let shim = cubarium_gpu::target::ShimScanout::open(gpu, src, 1)
                    .context("attaching to cube-screen-shim's frame socket")?;
                Ok(GpuTarget::Shim(Box::new(shim)))
            }
            GpuTargetKind::Window => Ok(GpuTarget::Window(Box::new(WindowTarget::open(
                gpu, src, title,
            )?))),
            GpuTargetKind::Headless => Ok(GpuTarget::Headless(Box::new(Headless::new(gpu, src)?))),
        }
    }

    /// Draw one frame. Returns the GPU milliseconds the timestamps saw.
    pub fn draw<S: FrameSource>(
        &mut self,
        gpu: &Gpu,
        src: &mut S,
        frame: S::Frame<'_>,
    ) -> Result<f64> {
        match self {
            GpuTarget::Shim(t) => Ok(t.draw(gpu, src, frame)?.0),
            GpuTarget::ShimThread(t) => t.draw(gpu, src, frame),
            GpuTarget::Window(t) => t.draw(gpu, src, frame),
            GpuTarget::Headless(t) => t.draw(gpu, src, frame),
        }
    }

    /// Frames the panel has actually been shown, where that is not the same as the frames
    /// the loop drew: a presenting thread is the one that knows. `None` for a target that
    /// presents every frame it is given.
    pub fn presented(&self) -> Option<(u64, u64)> {
        match self {
            GpuTarget::ShimThread(t) => Some((t.presented(), t.skipped())),
            _ => None,
        }
    }

    /// Where the presenting thread's time went, cumulatively. `None` for a target that
    /// presents on the calling thread and has no separate story to tell.
    pub fn sample(&self) -> Option<cubarium_gpu::target::presenter::PresentSample> {
        match self {
            GpuTarget::ShimThread(t) => Some(t.sample()),
            _ => None,
        }
    }

    pub fn should_quit(&mut self) -> bool {
        match self {
            GpuTarget::Window(t) => !t.open,
            _ => false,
        }
    }

    pub fn finish(&mut self, _gpu: &Gpu) -> Result<()> {
        Ok(())
    }

    pub fn destroy(&mut self, gpu: &Gpu) {
        match self {
            GpuTarget::Shim(t) => t.destroy(gpu),
            GpuTarget::ShimThread(t) => t.destroy(gpu),
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
    fn open<S: FrameSource>(gpu: &Gpu, src: &mut S, title: &str) -> Result<WindowTarget> {
        let (rw, rh) = src.raster_size();
        let (w, h) = (rw as usize, rh as usize);
        // The largest whole upscale that fits a 1,600 x 900 desktop area: nearest and
        // integer, exactly as the panel gets it.
        let zoom = ((1600 / w).min(900 / h)).max(1);
        let size = (w * zoom, h * zoom);
        let window = minifb::Window::new(title, size.0, size.1, minifb::WindowOptions::default())
            .context("opening the --gpu-target window")?;
        Ok(WindowTarget {
            window,
            headless: Headless::new(gpu, src)?,
            buffer: vec![0; size.0 * size.1],
            zoom,
            size,
            open: true,
        })
    }

    fn draw<S: FrameSource>(&mut self, gpu: &Gpu, src: &mut S, frame: S::Frame<'_>) -> Result<f64> {
        let ms = self.headless.draw(gpu, src, frame)?;
        let rgba = src.read_raster(gpu)?;
        upscale(&rgba, src.raster_size().0 as usize, self.zoom, &mut self.buffer);
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

/// `rgba` (`w` pixels a row) into minifb's `0RGB` words at `zoom`× nearest: each source
/// row converted once, and its zoomed copies taken from the first.
fn upscale(rgba: &[u8], w: usize, zoom: usize, out: &mut [u32]) {
    let row = w * zoom;
    for (src, band) in rgba.chunks_exact(w * 4).zip(out.chunks_exact_mut(row * zoom)) {
        let (first, rest) = band.split_at_mut(row);
        for (p, dst) in src.chunks_exact(4).zip(first.chunks_exact_mut(zoom)) {
            dst.fill(u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2]));
        }
        for copy in rest.chunks_exact_mut(row) {
            copy.copy_from_slice(first);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_window_upscale_is_nearest_at_the_zoom() {
        // 2 x 1 pixels, red then blue, at 2x: a 4 x 2 block of each colour's word.
        let rgba = [255, 0, 0, 255, 0, 0, 255, 255];
        let mut out = vec![0u32; 8];
        super::upscale(&rgba, 2, 2, &mut out);
        let (r, b) = (0x00FF_0000, 0x0000_00FF);
        assert_eq!(out, [r, r, b, b, r, r, b, b]);
        let mut one = vec![0u32; 2];
        super::upscale(&rgba, 2, 1, &mut one);
        assert_eq!(one, [r, b]);
    }
}
