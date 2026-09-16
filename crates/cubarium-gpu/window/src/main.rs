//! `Target::Window`: the synthetic ring in a desktop window, for development.
//!
//! ```text
//! cargo run --release -- --art ../../../assets/atelier --ring 320x180 --scale 1 --zoom 3
//! ```
//!
//! The scene, the passes and the present transform are the board's, exactly; only the
//! attachment differs — a `winit` window and an `ash` swapchain instead of a dma-buf
//! flipped at DP-1. The upscale is the same nearest-neighbour integer blow-up the panel
//! gets, so what this window shows is what the panel shows, at a smaller `k`.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use ash::vk;
use cubarium_gpu::atlas::Atlas;
use cubarium_gpu::render::{PresentTransform, Renderer, TargetImage};
use cubarium_gpu::scene::RingLayout;
use cubarium_gpu::synthetic::SyntheticWorld;
use cubarium_gpu::vk::Gpu;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
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
    _window: Arc<Window>,
    gpu: Gpu,
    renderer: Renderer,
    world: SyntheticWorld,
    surface_instance: ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    swapchain_device: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    format: vk::Format,
    extent: vk::Extent2D,
    targets: Vec<TargetImage>,
    command_buffers: Vec<vk::CommandBuffer>,
    acquired: vk::Semaphore,
    rendered: vk::Semaphore,
    fence: vk::Fence,
    transform: PresentTransform,
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
            state._window.request_redraw();
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
        let size = winit::dpi::LogicalSize::new(layout.w * zoom, layout.h * zoom);
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("cubarium ring")
                    .with_inner_size(size),
            )?,
        );

        // Only the surface extensions this display server actually needs.
        let display = window.display_handle()?.as_raw();
        let mut extensions = vec![ash::khr::surface::NAME];
        match display {
            RawDisplayHandle::Wayland(_) => extensions.push(ash::khr::wayland_surface::NAME),
            RawDisplayHandle::Xlib(_) => extensions.push(ash::khr::xlib_surface::NAME),
            RawDisplayHandle::Xcb(_) => extensions.push(ash::khr::xcb_surface::NAME),
            other => bail!("no Vulkan surface for {other:?}"),
        }
        let gpu = Gpu::open(&extensions)?;
        println!("device: {}", gpu.name);
        let surface = create_surface(&gpu, display, window.window_handle()?.as_raw())?;
        let surface_instance = ash::khr::surface::Instance::new(&gpu.entry, &gpu.instance);
        if !unsafe {
            surface_instance.get_physical_device_surface_support(gpu.pdev, gpu.queue_family, surface)
        }? {
            bail!("the graphics queue family cannot present to this surface");
        }
        let swapchain_device = ash::khr::swapchain::Device::new(&gpu.instance, &gpu.device);
        let renderer = Renderer::new(&gpu, atlas, layout)?;
        let world = SyntheticWorld::new(layout, seed);
        let d = &gpu.device;
        let semaphore = || unsafe { d.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) };
        let mut state = State {
            _window: window,
            surface_instance,
            surface,
            swapchain_device,
            swapchain: vk::SwapchainKHR::null(),
            format: vk::Format::UNDEFINED,
            extent: vk::Extent2D::default(),
            targets: Vec::new(),
            command_buffers: Vec::new(),
            acquired: semaphore()?,
            rendered: semaphore()?,
            fence: unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?,
            transform: PresentTransform { factor: zoom, quarter_turns: 0, encode_srgb: false },
            frame: 0,
            renderer,
            world,
            gpu,
        };
        state.rebuild_swapchain()?;
        Ok(state)
    }

    fn rebuild_swapchain(&mut self) -> Result<()> {
        unsafe { self.gpu.device.device_wait_idle()? };
        self.destroy_swapchain();
        let pdev = self.gpu.pdev;
        let capabilities = unsafe {
            self.surface_instance
                .get_physical_device_surface_capabilities(pdev, self.surface)
        }?;
        let formats = unsafe {
            self.surface_instance.get_physical_device_surface_formats(pdev, self.surface)
        }?;
        let d = &self.gpu.device;
        // An `_SRGB` surface makes the encode the hardware's, exactly as on the board's
        // preferred route; a UNORM surface falls back to the present shader's own.
        let chosen = formats
            .iter()
            .find(|f| {
                matches!(f.format, vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB)
            })
            .or_else(|| formats.first())
            .copied()
            .ok_or_else(|| anyhow!("the surface offers no formats"))?;
        let srgb = matches!(
            chosen.format,
            vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB
        );
        self.format = chosen.format;
        self.extent = if capabilities.current_extent.width == u32::MAX {
            vk::Extent2D {
                width: self.renderer.layout.w * self.transform.factor,
                height: self.renderer.layout.h * self.transform.factor,
            }
        } else {
            capabilities.current_extent
        };
        if self.extent.width == 0 || self.extent.height == 0 {
            return Ok(());
        }
        self.transform = PresentTransform::fit(
            (self.renderer.layout.w, self.renderer.layout.h),
            (self.extent.width, self.extent.height),
            0,
            !srgb,
        )
        .unwrap_or(PresentTransform { factor: 1, quarter_turns: 0, encode_srgb: !srgb });

        let count = (capabilities.min_image_count + 1)
            .min(if capabilities.max_image_count == 0 { u32::MAX } else { capabilities.max_image_count });
        self.swapchain = unsafe {
            self.swapchain_device.create_swapchain(
                &vk::SwapchainCreateInfoKHR::default()
                    .surface(self.surface)
                    .min_image_count(count)
                    .image_format(self.format)
                    .image_color_space(chosen.color_space)
                    .image_extent(self.extent)
                    .image_array_layers(1)
                    .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .pre_transform(capabilities.current_transform)
                    .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                    .present_mode(vk::PresentModeKHR::FIFO)
                    .clipped(true),
                None,
            )
        }?;
        let images = unsafe { self.swapchain_device.get_swapchain_images(self.swapchain) }?;
        let pass = self
            .renderer
            .present_pass(&self.gpu, self.format, vk::ImageLayout::PRESENT_SRC_KHR)?;
        self.command_buffers = unsafe {
            d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(self.renderer.command_pool)
                    .command_buffer_count(images.len() as u32),
            )
        }?;
        for image in images {
            let view = self.gpu.view(image, self.format)?;
            let framebuffer = cubarium_gpu::render::framebuffer(
                d,
                pass,
                view,
                self.extent.width,
                self.extent.height,
            )?;
            self.targets.push(TargetImage { image, view, framebuffer });
        }
        println!(
            "swapchain {}x{} {:?}, upscale x{}, sRGB by {}",
            self.extent.width,
            self.extent.height,
            self.format,
            self.transform.factor,
            if self.transform.encode_srgb { "the shader" } else { "the attachment" }
        );
        Ok(())
    }

    fn draw(&mut self, atlas: &Atlas, seconds: f64) -> Result<()> {
        if self.swapchain == vk::SwapchainKHR::null() {
            return Ok(());
        }
        let d = &self.gpu.device;
        let index = match unsafe {
            self.swapchain_device.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.acquired,
                vk::Fence::null(),
            )
        } {
            Ok((i, _)) => i as usize,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return self.rebuild_swapchain(),
            Err(e) => return Err(e.into()),
        };
        unsafe {
            d.wait_for_fences(&[self.fence], true, u64::MAX)?;
            d.reset_fences(&[self.fence])?;
        }

        let tick = (seconds * TICK_HZ).floor() as u64 + 1;
        let f = (seconds * TICK_HZ).fract();
        let present_seconds = |phase: f64| (tick as f64 - 1.0 + phase) / TICK_HZ;
        if tick != self.frame {
            self.world.tick(tick, present_seconds(0.0));
            self.frame = tick;
        }
        let scene = self.world.frame(atlas, present_seconds(f), f as f32);

        let cb = self.command_buffers[index];
        unsafe { d.reset_command_buffer(cb, vk::CommandBufferResetFlags::empty()) }?;
        self.renderer.record(
            &self.gpu,
            cb,
            scene,
            Some((
                &self.targets[index],
                (self.extent.width, self.extent.height),
                self.format,
                vk::ImageLayout::PRESENT_SRC_KHR,
                self.transform,
            )),
        )?;
        let wait = [self.acquired];
        let signal = [self.rendered];
        let stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let buffers = [cb];
        unsafe {
            d.queue_submit(
                self.gpu.queue,
                &[vk::SubmitInfo::default()
                    .wait_semaphores(&wait)
                    .wait_dst_stage_mask(&stages)
                    .command_buffers(&buffers)
                    .signal_semaphores(&signal)],
                self.fence,
            )?;
            let swapchains = [self.swapchain];
            let indices = [index as u32];
            match self.swapchain_device.queue_present(
                self.gpu.queue,
                &vk::PresentInfoKHR::default()
                    .wait_semaphores(&signal)
                    .swapchains(&swapchains)
                    .image_indices(&indices),
            ) {
                Ok(_) => {}
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                    self.rebuild_swapchain()?
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn destroy_swapchain(&mut self) {
        let d = &self.gpu.device;
        unsafe {
            for target in self.targets.drain(..) {
                d.destroy_framebuffer(target.framebuffer, None);
                d.destroy_image_view(target.view, None);
            }
            if !self.command_buffers.is_empty() {
                d.free_command_buffers(self.renderer.command_pool, &self.command_buffers);
                self.command_buffers.clear();
            }
            if self.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_device.destroy_swapchain(self.swapchain, None);
                self.swapchain = vk::SwapchainKHR::null();
            }
        }
    }
}

/// The platform surface, without `ash-window`: three of them, chosen by the handle the
/// window hands back.
fn create_surface(
    gpu: &Gpu,
    display: RawDisplayHandle,
    window: RawWindowHandle,
) -> Result<vk::SurfaceKHR> {
    unsafe {
        match (display, window) {
            (RawDisplayHandle::Wayland(d), RawWindowHandle::Wayland(w)) => {
                let instance = ash::khr::wayland_surface::Instance::new(&gpu.entry, &gpu.instance);
                Ok(instance.create_wayland_surface(
                    &vk::WaylandSurfaceCreateInfoKHR::default()
                        .display(d.display.as_ptr())
                        .surface(w.surface.as_ptr()),
                    None,
                )?)
            }
            (RawDisplayHandle::Xlib(d), RawWindowHandle::Xlib(w)) => {
                let instance = ash::khr::xlib_surface::Instance::new(&gpu.entry, &gpu.instance);
                Ok(instance.create_xlib_surface(
                    &vk::XlibSurfaceCreateInfoKHR::default()
                        .dpy(d.display.ok_or_else(|| anyhow!("no X display"))?.as_ptr().cast())
                        .window(w.window),
                    None,
                )?)
            }
            (RawDisplayHandle::Xcb(d), RawWindowHandle::Xcb(w)) => {
                let instance = ash::khr::xcb_surface::Instance::new(&gpu.entry, &gpu.instance);
                Ok(instance.create_xcb_surface(
                    &vk::XcbSurfaceCreateInfoKHR::default()
                        .connection(d.connection.ok_or_else(|| anyhow!("no XCB connection"))?.as_ptr())
                        .window(w.window.get()),
                    None,
                )?)
            }
            (d, w) => bail!("no Vulkan surface for {d:?} / {w:?}"),
        }
    }
}
