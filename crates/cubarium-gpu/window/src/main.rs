//! `Target::Window`: the synthetic ring in a desktop window, for development.
//!
//! ```text
//! cargo run --release -- --art ../../../assets/atelier --ring 320x180 --scale 1 --zoom 3
//! ```
//!
//! The scene, the passes and the present transform are the board's, exactly; only the
//! attachment differs — a `winit` window and a safe wgpu surface instead of a dma-buf
//! flipped at DP-1. The upscale is the same nearest-neighbour integer blow-up the panel
//! gets, so what this window shows is what the panel shows, at a smaller `k`.

#![forbid(unsafe_code)]

use std::sync::Arc;

#[path = "../../../cubarium/src/sink/hidden.rs"]
mod hidden;

use anyhow::{Context, Result, anyhow, bail};
use cubarium_gpu::atlas::Atlas;
use cubarium_gpu::scene::RingLayout;
use cubarium_gpu::synthetic::SyntheticWorld;
use cubarium_gpu::wgpu_backend::{FrameSource, Gpu, flat::Renderer, window::WindowPresenter};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const TICK_HZ: f64 = 20.0;

fn main() -> Result<()> {
    let mut art = std::path::PathBuf::from("assets/atelier");
    let mut layout = RingLayout::RING_320;
    let mut zoom = 3u32;
    let mut seed = 1u64;
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| anyhow!("{flag} wants a value"));
        match flag.as_str() {
            "--art" => art = value()?.into(),
            "--ring" => {
                let v = value()?;
                let (w, h) = v.split_once('x').ok_or_else(|| anyhow!("--ring WxH"))?;
                layout.w = w.parse()?;
                layout.h = h.parse()?;
            }
            "--scale" => layout.scale = value()?.parse()?,
            "--zoom" => zoom = value()?.parse()?,
            "--seed" => seed = value()?.parse()?,
            other => bail!("unknown flag {other}"),
        }
    }
    let atlas = Atlas::load(&art).with_context(|| format!("load {}", art.display()))?;
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        atlas,
        layout,
        zoom,
        seed,
        state: None,
        start: std::time::Instant::now(),
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    atlas: Atlas,
    layout: RingLayout,
    zoom: u32,
    seed: u64,
    state: Option<State>,
    start: std::time::Instant,
}

struct State {
    window: Arc<Window>,
    gpu: Arc<Gpu>,
    renderer: Renderer,
    world: SyntheticWorld,
    presenter: WindowPresenter,
    frame: u64,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match State::new(event_loop, &self.atlas, self.layout, self.zoom, self.seed) {
            Ok(state) => self.state = Some(state),
            Err(e) => {
                eprintln!("could not open a window: {e:#}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) => {
                if let Err(e) = state.rebuild_swapchain() {
                    eprintln!("swapchain: {e:#}");
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                let seconds = self.start.elapsed().as_secs_f64();
                if let Err(e) = state.draw(&self.atlas, seconds) {
                    eprintln!("draw: {e:#}");
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

impl State {
    fn new(
        event_loop: &ActiveEventLoop,
        atlas: &Atlas,
        layout: RingLayout,
        zoom: u32,
        seed: u64,
    ) -> Result<State> {
        hidden::window_allowed()?;
        let size = winit::dpi::LogicalSize::new(layout.w * zoom, layout.h * zoom);
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("cubarium ring")
                    .with_inner_size(size),
            )?,
        );
        let gpu = Arc::new(Gpu::open()?);
        println!("device: {}", gpu.name);
        let renderer = Renderer::new(&gpu, atlas, layout)?;
        let world = SyntheticWorld::new(layout, seed);
        let size = window.inner_size();
        let presenter =
            WindowPresenter::open(gpu.clone(), window.clone(), (size.width, size.height))?;
        let mut state = State {
            window,
            gpu,
            renderer,
            world,
            presenter,
            frame: 0,
        };
        state.rebuild_swapchain()?;
        Ok(state)
    }

    fn rebuild_swapchain(&mut self) -> Result<()> {
        let size = self.window.inner_size();
        self.presenter.resize((size.width, size.height));
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let format = self.presenter.format();
        let fit = cubarium_gpu::present::WindowFit::fit(
            self.renderer.raster_size(),
            (size.width, size.height),
            !format.is_srgb(),
        );
        println!(
            "swapchain {}x{} {:?}, upscale x{}, sRGB by {}",
            size.width,
            size.height,
            format,
            1.0 / fit.step,
            if fit.encode_srgb {
                "the shader"
            } else {
                "the attachment"
            },
        );
        Ok(())
    }

    fn draw(&mut self, atlas: &Atlas, seconds: f64) -> Result<()> {
        let Some(frame) = self.presenter.acquire()? else {
            return Ok(());
        };
        let tick = (seconds * TICK_HZ).floor() as u64 + 1;
        let f = (seconds * TICK_HZ).fract();
        let present_seconds = |phase: f64| (tick as f64 - 1.0 + phase) / TICK_HZ;
        if tick != self.frame {
            self.world.tick(tick, present_seconds(0.0));
            self.frame = tick;
        }
        let scene = self.world.frame(atlas, present_seconds(f), f as f32);
        let mut encoder = self.gpu.encoder("synthetic window frame");
        self.renderer
            .record_frame(&self.gpu, &mut encoder, scene, None)?;
        if let Err(error) = self.presenter.record_present(
            &mut encoder,
            self.renderer.raster_view(),
            self.renderer.raster_size(),
            &frame,
        ) {
            self.renderer.frame_discarded();
            return Err(error);
        }
        let submission = match self.presenter.submit(frame, encoder) {
            Ok(submission) => submission,
            Err(error) => {
                self.renderer.frame_discarded();
                return Err(error);
            }
        };
        // One frame at a time, as with the original window's fence. Retirement
        // follows completion, so the renderer never reuses a pending upload.
        self.gpu.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(30)),
        })?;
        self.renderer.frame_retired();
        Ok(())
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.renderer.destroy(&self.gpu);
    }
}
