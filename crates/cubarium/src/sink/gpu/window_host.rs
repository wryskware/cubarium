//! Development-window events and ownership for safe wgpu surface creation.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::{Window, WindowAttributes, WindowId};

pub(super) struct WindowHost {
    event_loop: EventLoop<()>,
    events: Events,
}

struct Events {
    attributes: WindowAttributes,
    window: Option<Arc<Window>>,
    error: Option<anyhow::Error>,
    modifiers: ModifiersState,
    quit: bool,
    save: bool,
}

impl ApplicationHandler for Events {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.error.is_some() {
            return;
        }
        if let Err(error) = crate::sink::hidden::window_allowed() {
            self.error = Some(error);
            event_loop.exit();
            return;
        }
        match event_loop.create_window(self.attributes.clone()) {
            Ok(window) => self.window = Some(Arc::new(window)),
            Err(error) => {
                self.error = Some(error.into());
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.window.as_ref().is_none_or(|window| window.id() != id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.quit = true;
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => self.quit = true,
                    PhysicalKey::Code(KeyCode::KeyS) if self.modifiers.control_key() => self.save = true,
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

struct DrawEvents<'a, F> {
    events: &'a mut Events,
    draw: F,
    error: Option<anyhow::Error>,
}

impl<F: FnMut() -> Result<()>> ApplicationHandler for DrawEvents<'_, F> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.events.resumed(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested)
            && self.events.window.as_ref().is_some_and(|window| window.id() == id)
        {
            if !self.events.quit && self.error.is_none() {
                self.error = (self.draw)().err();
            }
        } else {
            self.events.window_event(event_loop, id, event);
        }
    }
}

impl WindowHost {
    pub fn open(title: &str, size: (u32, u32)) -> Result<Self> {
        crate::sink::hidden::window_allowed()?;
        let mut builder = EventLoop::builder();
        #[cfg(target_os = "linux")]
        {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_any_thread(true);
        }
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            builder.with_any_thread(true);
        }
        let event_loop = builder.build().context("create development window event loop")?;
        let events = Events {
            attributes: Window::default_attributes()
                .with_title(title)
                .with_inner_size(PhysicalSize::new(size.0.max(1), size.1.max(1)))
                .with_resizable(true),
            window: None,
            error: None,
            modifiers: ModifiersState::empty(),
            quit: false,
            save: false,
        };
        let mut host = Self { event_loop, events };
        host.pump()?;
        if host.events.window.is_none() {
            return Err(anyhow!("development window did not resume during initialization"));
        }
        Ok(host)
    }

    pub fn window(&self) -> Arc<Window> {
        self.events.window.as_ref().expect("initialized development window").clone()
    }

    pub fn size(&self) -> (u32, u32) {
        let size = self.events.window.as_ref().expect("initialized development window").inner_size();
        (size.width, size.height)
    }

    pub fn pump(&mut self) -> Result<bool> {
        if let PumpStatus::Exit(_) = self.event_loop.pump_app_events(Some(Duration::ZERO), &mut self.events) {
            self.events.quit = true;
        }
        if let Some(error) = self.events.error.take() {
            return Err(error);
        }
        Ok(!self.events.quit)
    }

    pub fn take_save(&mut self) -> bool {
        std::mem::take(&mut self.events.save)
    }

    pub fn is_open(&self) -> bool {
        !self.events.quit
    }

    /// Paints inside Winit's redraw callback, as macOS requires.
    pub fn draw(&mut self, draw: impl FnMut() -> Result<()>) -> Result<()> {
        self.window().request_redraw();
        let (status, error) = {
            let mut events = DrawEvents { events: &mut self.events, draw, error: None };
            let status = self.event_loop.pump_app_events(Some(Duration::ZERO), &mut events);
            (status, events.error)
        };
        if matches!(status, PumpStatus::Exit(_)) {
            self.events.quit = true;
        }
        if let Some(error) = self.events.error.take() {
            return Err(error);
        }
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
