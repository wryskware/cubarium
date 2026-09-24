//! Where `--sink gpu`'s frames go: the panel, a development window, or nowhere.

use std::ffi::CStr;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use clap::ValueEnum;

use cubarium_gpu::present::FrameSource;
use cubarium_gpu::target::Headless;
use cubarium_gpu::target::window::{NativeWindow, SURFACE_EXTENSIONS, WindowPresenter};
use cubarium_gpu::vk::Gpu;

use super::wayland::WaylandWindow;

/// `--gpu-target`.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuTargetKind {
    /// The board: dma-bufs handed to `cube-screen-shim` over its frame socket.
    Shim,
    /// The desktop: a window showing the world raster at the largest whole upscale that
    /// fits — through a swapchain on its own thread for the voxel strip, read back from
    /// the GPU each frame for the ring (and wherever a swapchain cannot be made).
    Window,
    /// Render and drop. For measuring the renderer, and for `--gpu-capture`.
    Headless,
}

impl GpuTargetKind {
    /// The Vulkan instance extensions this target wants from `Gpu::open`: a window's
    /// surface extensions, and nothing for the others, so the panel's instance is exactly
    /// what it was. `Gpu::open` enables only the ones the loader has.
    pub fn instance_extensions(self) -> &'static [&'static CStr] {
        match self {
            GpuTargetKind::Window => &SURFACE_EXTENSIONS,
            _ => &[],
        }
    }

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
    /// The desktop through a swapchain presented on its own thread: the voxel strip's
    /// window. `minifb` keeps the window and its events on the loop; the thread has the
    /// surface.
    WindowThread(Box<SwapchainWindow>),
    /// The desktop through a readback on the loop: the ring's window, and the fallback
    /// wherever a swapchain cannot be made.
    Window(Box<WindowTarget>),
    Headless(Box<Headless>),
}

impl GpuTarget {
    /// Open a target for `src`. `title` names the development window and is ignored by
    /// every other target, and `present_thread` asks for the panel or the window to be
    /// presented on its own thread (the voxel strip does; the ring, whose renderer writes
    /// its instance buffers every frame, does not). The headless path ignores it.
    ///
    /// A window that cannot have a swapchain — no surface extension, a queue that cannot
    /// present to it, a renderer that does not lend its raster — says why and reads the
    /// raster back instead.
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
            GpuTargetKind::Window if present_thread => {
                // A refused window is not a swapchain that failed: it must not fall back
                // to the readback window, and it must not look like it did.
                crate::sink::hidden::window_allowed()?;
                match SwapchainWindow::open(gpu, src, title) {
                    Ok(window) => Ok(GpuTarget::WindowThread(Box::new(window))),
                    // The readback window takes over the window that is already open, if
                    // there is one: a second window from this process would escape
                    // `scripts/hidden.sh`, whose rule reaches only the first.
                    Err((e, left)) => {
                        let open = match left {
                            Leftover::Nothing => None,
                            Leftover::Minifb(window) => Some(window),
                            // A toplevel existed, though it never had a buffer. Whether the
                            // compositor counted it is not ours to know, so a hidden launch
                            // (`sink::hidden`'s marker) stops here rather than risk a
                            // second window that its rule would not reach.
                            Leftover::Toplevel
                                if std::env::var("CUBARIUM_HIDDEN_LAUNCH").as_deref()
                                    == Ok("1") =>
                            {
                                return Err(e.context(
                                    "no swapchain for the development window, and a \
                                     readback window now would be this process's second",
                                ));
                            }
                            Leftover::Toplevel => None,
                        };
                        eprintln!(
                            "cubarium: no swapchain for the development window ({e:#}); \
                             reading the raster back instead"
                        );
                        Ok(GpuTarget::Window(Box::new(WindowTarget::open(
                            gpu, src, title, open,
                        )?)))
                    }
                }
            }
            GpuTargetKind::Window => Ok(GpuTarget::Window(Box::new(WindowTarget::open(
                gpu, src, title, None,
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
            // The timestamps are the last *retired* frame's, and none has retired for the
            // first frame or two: that is 0 here, not a NaN that turns the sink's average
            // into NaN for the whole run.
            GpuTarget::WindowThread(t) => t
                .draw(gpu, src, frame)
                .map(|ms| if ms.is_finite() { ms } else { 0.0 }),
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
            GpuTarget::WindowThread(t) => {
                Some((t.presenter.presented(), t.presenter.skipped()))
            }
            _ => None,
        }
    }

    /// Where the presenting thread's time went, cumulatively. `None` for a target that
    /// presents on the calling thread and has no separate story to tell.
    pub fn sample(&self) -> Option<cubarium_gpu::target::presenter::PresentSample> {
        match self {
            GpuTarget::ShimThread(t) => Some(t.sample()),
            GpuTarget::WindowThread(t) => Some(t.presenter.sample()),
            _ => None,
        }
    }

    pub fn should_quit(&mut self) -> bool {
        match self {
            GpuTarget::Window(t) => !t.open,
            GpuTarget::WindowThread(t) => !t.open,
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
            GpuTarget::WindowThread(t) => t.destroy(gpu),
            GpuTarget::Window(t) => t.destroy(gpu),
            GpuTarget::Headless(t) => t.destroy(gpu),
        }
    }
}

/// The development window, presented through a swapchain on a thread of its own.
///
/// On Wayland the window is this crate's own bare toplevel ([`WaylandWindow`]): a
/// surface with a title and the app id `cubarium` set before it maps, so the compositor's
/// rules can place it. Elsewhere it is `minifb`'s Xlib window. Either way nothing here
/// draws into it — the swapchain does. The loop's part is pumping the window's events and
/// recording the world; everything that waits — the fence, the acquire, the present — is
/// on the presenting thread (`cubarium_gpu::target::window`).
pub struct SwapchainWindow {
    /// `None` only once a presenting thread stuck in the driver has been left behind,
    /// and the window with it: its surface is still on the window.
    host: Option<Host>,
    presenter: WindowPresenter,
    pub open: bool,
    /// The event pump, which is the one thing about the window left on the loop: calls,
    /// and total and worst nanoseconds. It reads the socket the presenting thread's WSI
    /// also reads, so it is measured rather than assumed to be free.
    pumps: u64,
    pump_ns: u64,
    pump_max_ns: u64,
}

/// What a swapchain window that could not be made leaves for the readback window.
enum Leftover {
    /// No window was made: the readback window will be this process's first.
    Nothing,
    /// A `minifb` window, open and mapped, for the readback window to take over.
    Minifb(minifb::Window),
    /// A Wayland toplevel was made and dropped without ever having had a buffer.
    Toplevel,
}

/// The native window under the swapchain.
enum Host {
    Wayland(WaylandWindow),
    Xlib(minifb::Window),
}

impl Host {
    fn size(&self) -> (u32, u32) {
        match self {
            Host::Wayland(w) => w.size(),
            Host::Xlib(w) => {
                let (w, h) = w.get_size();
                (w as u32, h as u32)
            }
        }
    }

    /// Handle whatever has arrived; false once the window has been closed or Escape
    /// pressed in it.
    fn pump(&mut self) -> Result<bool> {
        Ok(match self {
            Host::Wayland(w) => {
                w.pump()?;
                !w.closed() && !w.escape()
            }
            Host::Xlib(w) => {
                w.update();
                w.is_open() && !w.is_key_down(minifb::Key::Escape)
            }
        })
    }
}

impl SwapchainWindow {
    /// Open the window and its presenter. On failure the error comes back with the
    /// `minifb` window, if one was opened, for the readback window to take over: one
    /// process, one window (`scripts/hidden.sh` hides only the first).
    fn open<S: FrameSource>(
        gpu: &Arc<Gpu>,
        src: &mut S,
        title: &str,
    ) -> std::result::Result<SwapchainWindow, (anyhow::Error, Leftover)> {
        // Before anything is committed or mapped (`sink::hidden`).
        crate::sink::hidden::window_allowed().map_err(|e| (e, Leftover::Nothing))?;
        let (rw, rh) = src.raster_size();
        // The size it opens at is the readback window's; after that it is whatever the
        // person or the compositor makes it, and the picture follows.
        let zoom = ((1600 / rw).min(900 / rh)).max(1);
        let size = (rw * zoom, rh * zoom);
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty());
        let (host, native) = if wayland {
            // Whether this GPU can present here is asked of the connection, before a
            // surface exists; a toplevel that then fails has never had a buffer.
            let window = WaylandWindow::open(title, size, |display| {
                cubarium_gpu::target::window::check_wayland(gpu, display)
            })
            .map_err(|e| (e, Leftover::Nothing))?;
            let (display, surface) = window.handles();
            (Host::Wayland(window), NativeWindow::Wayland { display, surface })
        } else {
            let mut window = minifb::Window::new(
                title,
                size.0 as usize,
                size.1 as usize,
                minifb::WindowOptions {
                    resize: true,
                    ..minifb::WindowOptions::default()
                },
            )
            .context("opening the --gpu-target window")
            .map_err(|e| (e, Leftover::Nothing))?;
            // Pump events only: `minifb` otherwise sleeps in `update` to pace a window
            // this path does not draw with, on the loop.
            window.set_target_fps(0);
            match xlib_window(&window) {
                Ok(native) => (Host::Xlib(window), native),
                Err(e) => return Err((e, Leftover::Minifb(window))),
            }
        };
        let presenter = match WindowPresenter::open(gpu.clone(), src, native, host.size()) {
            Ok(presenter) => presenter,
            Err(e) => {
                return Err((
                    e,
                    match host {
                        Host::Xlib(window) => Leftover::Minifb(window),
                        Host::Wayland(_) => Leftover::Toplevel,
                    },
                ));
            }
        };
        eprintln!(
            "cubarium: development window presented on its own thread — {}",
            presenter.describe()
        );
        Ok(SwapchainWindow {
            host: Some(host),
            presenter,
            open: true,
            pumps: 0,
            pump_ns: 0,
            pump_max_ns: 0,
        })
    }

    fn draw<S: FrameSource>(&mut self, gpu: &Gpu, src: &mut S, frame: S::Frame<'_>) -> Result<f64> {
        let host = self
            .host
            .as_mut()
            .expect("the window is only let go of at destroy");
        let at = Instant::now();
        let open = host.pump()?;
        let ns = at.elapsed().as_nanos() as u64;
        self.pumps += 1;
        self.pump_ns += ns;
        self.pump_max_ns = self.pump_max_ns.max(ns);
        if !open {
            self.open = false;
            return Ok(src.gpu_ms(gpu));
        }
        let size = host.size();
        self.presenter.draw(gpu, src, frame, size)
    }

    fn destroy(&mut self, gpu: &Gpu) {
        if self.pumps > 0 {
            eprintln!(
                "cubarium: the window's event pump took {:.3} ms a frame on the loop \
                 (worst {:.1} ms over {} frames)",
                self.pump_ns as f64 / self.pumps as f64 / 1e6,
                self.pump_max_ns as f64 / 1e6,
                self.pumps
            );
        }
        if !self.presenter.destroy(gpu) {
            // The thread is inside the driver with the surface: keep the window under it.
            std::mem::forget(self.host.take());
        }
    }
}

/// The Xlib window under a `minifb` window, for a Vulkan surface. (On Wayland the
/// swapchain window is [`WaylandWindow`]; `minifb`'s Wayland backend maps before it is
/// titled and hands out a display handle that is already freed.)
fn xlib_window(window: &minifb::Window) -> Result<NativeWindow> {
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
    let handle = window
        .window_handle()
        .map_err(|e| anyhow!("the window's handle: {e}"))?
        .as_raw();
    let RawWindowHandle::Xlib(h) = handle else {
        bail!("no Vulkan surface for a {handle:?} minifb window");
    };
    let display = window
        .display_handle()
        .map_err(|e| anyhow!("the window's display: {e}"))?
        .as_raw();
    let RawDisplayHandle::Xlib(d) = display else {
        bail!("an Xlib window on a {display:?} display");
    };
    let display = d
        .display
        .ok_or_else(|| anyhow!("the Xlib display handle is null"))?
        .as_ptr();
    Ok(NativeWindow::Xlib {
        display,
        window: h.window as u64,
    })
}

/// A development window drawn by reading the raster back: the ring's window, and the
/// fallback for the voxel strip's wherever [`SwapchainWindow`] cannot be made.
///
/// One `read_raster`, a CPU upscale and `minifb`'s shm copy per frame, all on the loop:
/// about a millisecond at ring resolution, but 17 ms a frame at the voxel strip's 13 px
/// (3328 × 2048), which is why that one has a swapchain now. The board's target reads
/// nothing back.
pub struct WindowTarget {
    window: minifb::Window,
    headless: Headless,
    buffer: Vec<u32>,
    zoom: usize,
    size: (usize, usize),
    pub open: bool,
}

impl WindowTarget {
    /// Open the readback window, or take over `open` — a `minifb` window a swapchain
    /// could not be made on — rather than open a second one.
    fn open<S: FrameSource>(
        gpu: &Gpu,
        src: &mut S,
        title: &str,
        open: Option<minifb::Window>,
    ) -> Result<WindowTarget> {
        let (rw, rh) = src.raster_size();
        let (w, h) = (rw as usize, rh as usize);
        // The largest whole upscale that fits a 1,600 x 900 desktop area: nearest and
        // integer, exactly as the panel gets it.
        let zoom = ((1600 / w).min(900 / h)).max(1);
        let size = (w * zoom, h * zoom);
        crate::sink::hidden::window_allowed()?;
        let window = match open {
            Some(window) => window,
            None => minifb::Window::new(title, size.0, size.1, minifb::WindowOptions::default())
                .context("opening the --gpu-target window")?,
        };
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
