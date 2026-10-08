//! A bare Wayland toplevel for the swapchain window: a `wl_surface` with an xdg role and
//! nothing else — no buffers of its own, since the swapchain draws every one.
//!
//! # Why not `minifb`'s window
//!
//! The swapchain was first made on `minifb`'s window, and `minifb` 0.28's Wayland backend
//! fights it in three ways:
//!
//! * it attaches a buffer and commits — maps the window — **before** it sets the title,
//!   and never sets an app id. A compositor rule matching either (`run-voxel.sh
//!   --background`'s float-and-don't-focus rule, `class ^(cubarium)$` in the Hyprland
//!   config) can never apply, and the window tiles into the person's workspace;
//! * it acknowledges a configure only when it attaches a buffer of its own, which a
//!   swapchain window never does, so a resize was never acknowledged;
//! * its display handle is a proxy wrapper that is freed before it is returned.
//!
//! This window sets the title and the app id `cubarium` before its first commit,
//! acknowledges every configure as it arrives, and reads its socket without blocking —
//! the swapchain's WSI on the presenting thread reads the same socket.

use std::ffi::c_void;
use std::os::fd::AsFd;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use cubarium_gpu::target::window::WindowScale;
use wayland_client::protocol::{
    wl_compositor, wl_keyboard, wl_output, wl_registry, wl_seat, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

const KEY_ESC: u32 = 1;
const KEY_S: u32 = 31;
const MOD_CONTROL: u32 = 1 << 2;
const APP_ID: &str = "cubarium";

#[derive(Default)]
struct Output {
    name: u32,
    scale: u32,
    physical: Option<(u32, u32)>,
    rotated: bool,
    entered: bool,
}

/// State written by protocol events. Scale selection is independent of size configures:
/// moving a window between monitors can change its pixels without changing its geometry.
#[derive(Default)]
struct Events {
    globals: Vec<(u32, String, u32)>,
    outputs: Vec<(wl_output::WlOutput, Output)>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    configured: Option<(u32, u32)>,
    preferred_fractional: Option<u32>,
    preferred_integer: Option<u32>,
    closed: bool,
    escape: bool,
    control: bool,
    save: bool,
}

impl Events {
    fn bind<I: Proxy + 'static>(
        &self,
        registry: &wl_registry::WlRegistry,
        qh: &QueueHandle<Self>,
        max: u32,
    ) -> Option<I>
    where
        Self: Dispatch<I, ()>,
    {
        let (name, _, version) = self
            .globals
            .iter()
            .find(|(_, interface, _)| *interface == I::interface().name)?;
        Some(registry.bind(*name, (*version).min(max), qh, ()))
    }

    fn scale(&self, fractional: bool, surface_version: u32) -> WindowScale {
        surface_scale(
            fractional,
            self.preferred_fractional,
            surface_version,
            self.preferred_integer,
            self.outputs
                .iter()
                .filter(|(_, o)| o.entered)
                .map(|(_, o)| o.scale)
                .max(),
        )
    }
}

/// v6's preference takes precedence over output overlap, and fractional scale takes
/// precedence over both. Old surfaces cannot request an integer buffer scale.
fn surface_scale(
    fractional_available: bool,
    fractional: Option<u32>,
    surface_version: u32,
    integer: Option<u32>,
    output: Option<u32>,
) -> WindowScale {
    if fractional_available {
        WindowScale::Fractional(fractional.unwrap_or(120))
    } else if surface_version >= 3 {
        WindowScale::Integer(integer.or(output).unwrap_or(1).max(1))
    } else {
        WindowScale::Unscaled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_protocols_follow_preference_and_capability() {
        assert_eq!(
            surface_scale(true, Some(150), 6, Some(3), Some(2)),
            WindowScale::Fractional(150)
        );
        assert_eq!(
            surface_scale(true, None, 6, Some(3), Some(2)),
            WindowScale::Fractional(120)
        );
        assert_eq!(
            surface_scale(false, Some(150), 6, Some(3), Some(2)),
            WindowScale::Integer(3)
        );
        assert_eq!(
            surface_scale(false, None, 4, None, Some(2)),
            WindowScale::Integer(2)
        );
        assert_eq!(
            surface_scale(false, None, 4, None, None),
            WindowScale::Integer(1)
        );
        assert_eq!(
            surface_scale(false, None, 2, None, Some(2)),
            WindowScale::Unscaled
        );
    }

    #[test]
    fn scale_change_changes_the_swapchain_size_without_a_logical_resize() {
        let logical = (101, 53);
        let before = surface_scale(true, Some(120), 6, None, None).physical_size(logical);
        let after = surface_scale(true, Some(180), 6, None, None).physical_size(logical);
        assert_ne!(before, after);
        assert_eq!(after, WindowScale::Fractional(180).physical_size(logical));
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Events {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                if interface == "wl_output" {
                    state.outputs.push((
                        registry.bind(name, version.min(4), qh, ()),
                        Output {
                            name,
                            scale: 1,
                            ..Output::default()
                        },
                    ));
                }
                state.globals.push((name, interface, version));
            }
            wl_registry::Event::GlobalRemove { name } => {
                state.globals.retain(|(n, _, _)| *n != name);
                state.outputs.retain(|(_, o)| o.name != name);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for Events {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some((_, o)) = state.outputs.iter_mut().find(|(p, _)| p == output) else {
            return;
        };
        match event {
            wl_output::Event::Scale { factor } => o.scale = factor.max(1) as u32,
            wl_output::Event::Mode {
                flags: WEnum::Value(flags),
                width,
                height,
                ..
            } if flags.contains(wl_output::Mode::Current) && width > 0 && height > 0 => {
                o.physical = Some((width as u32, height as u32));
            }
            wl_output::Event::Geometry {
                transform: WEnum::Value(t),
                ..
            } => {
                o.rotated = matches!(
                    t,
                    wl_output::Transform::_90
                        | wl_output::Transform::_270
                        | wl_output::Transform::Flipped90
                        | wl_output::Transform::Flipped270
                );
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for Events {
    fn event(
        state: &mut Self,
        _: &wl_surface::WlSurface,
        event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_surface::Event::PreferredBufferScale { factor } => {
                state.preferred_integer = Some(factor.max(1) as u32)
            }
            wl_surface::Event::Enter { output } => {
                if let Some((_, o)) = state.outputs.iter_mut().find(|(p, _)| *p == output) {
                    o.entered = true;
                }
            }
            wl_surface::Event::Leave { output } => {
                if let Some((_, o)) = state.outputs.iter_mut().find(|(p, _)| *p == output) {
                    o.entered = false;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, ()> for Events {
    fn event(
        state: &mut Self,
        _: &wp_fractional_scale_v1::WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            if scale > 0 {
                state.preferred_fractional = Some(scale);
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for Events {
    fn event(
        _: &mut Self,
        base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            base.pong(serial);
        }
    }
}
impl Dispatch<xdg_surface::XdgSurface, ()> for Events {
    fn event(
        _: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel, ()> for Events {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                state.configured = Some((width.max(0) as u32, height.max(0) as u32))
            }
            xdg_toplevel::Event::Close => state.closed = true,
            _ => {}
        }
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for Events {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(c),
        } = event
        {
            if c.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}
impl Dispatch<wl_keyboard::WlKeyboard, ()> for Events {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => {
                if key == KEY_ESC {
                    state.escape = true;
                }
                if key == KEY_S {
                    state.save |= state.control;
                }
            }
            wl_keyboard::Event::Modifiers { mods_depressed, .. } => {
                state.control = mods_depressed & MOD_CONTROL != 0
            }
            // The OwnedFd in Keymap is dropped automatically; no keymap is needed.
            _ => {}
        }
    }
}

delegate_noop!(Events: ignore wl_compositor::WlCompositor);
delegate_noop!(Events: ignore wp_viewporter::WpViewporter);
delegate_noop!(Events: ignore wp_viewport::WpViewport);
delegate_noop!(Events: ignore wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);

/// Query physical output modes, without creating any surface or window. This also works
/// on KWin, where neither hyprctl nor Xwayland's logical xrandr sizes describe the panel.
pub fn physical_outputs() -> Option<Vec<(u32, u32)>> {
    // Match the host's choice of Wayland rather than probing a default socket on
    // X11/headless launches (including the private Xvfb used by hidden.sh).
    if !std::env::var_os("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty()) {
        return None;
    }
    let connection = Connection::connect_to_env().ok()?;
    let mut queue = connection.new_event_queue();
    let mut events = Events::default();
    let _registry = connection.display().get_registry(&queue.handle(), ());
    queue.roundtrip(&mut events).ok()?;
    queue.roundtrip(&mut events).ok()?;
    let sizes: Vec<_> = events
        .outputs
        .iter()
        .filter_map(|(_, o)| {
            o.physical
                .map(|(w, h)| if o.rotated { (h, w) } else { (w, h) })
        })
        .collect();
    (!sizes.is_empty()).then_some(sizes)
}

/// One bare toplevel on its own libwayland connection, shared with Vulkan WSI.
pub struct WaylandWindow {
    toplevel: xdg_toplevel::XdgToplevel,
    xdg_surface: xdg_surface::XdgSurface,
    fractional: Option<wp_fractional_scale_v1::WpFractionalScaleV1>,
    viewport: Option<wp_viewport::WpViewport>,
    surface: wl_surface::WlSurface,
    _wm_base: xdg_wm_base::XdgWmBase,
    _seat: Option<wl_seat::WlSeat>,
    events: Events,
    queue: EventQueue<Events>,
    connection: Connection,
    logical: (u32, u32),
    scale: WindowScale,
    /// Copied under a short lock; no WSI or protocol operation holds this lock.
    geometry: Arc<Mutex<((u32, u32), WindowScale)>>,
}

impl WaylandWindow {
    /// `size` is logical. Check presentation support before creating any surface.
    pub fn open(
        title: &str,
        size: (u32, u32),
        check: impl FnOnce(*mut c_void) -> Result<()>,
    ) -> Result<Self> {
        crate::sink::hidden::window_allowed()?;
        let connection =
            Connection::connect_to_env().context("connecting to the Wayland display")?;
        check(connection.backend().display_ptr() as *mut c_void)?;
        let mut queue = connection.new_event_queue();
        let qh = queue.handle();
        let mut events = Events::default();
        let registry = connection.display().get_registry(&qh, ());
        queue
            .roundtrip(&mut events)
            .context("reading the Wayland registry")?;
        let compositor: wl_compositor::WlCompositor = events
            .bind(&registry, &qh, 6)
            .context("the compositor offers no wl_compositor")?;
        let wm_base: xdg_wm_base::XdgWmBase = events
            .bind(&registry, &qh, 2)
            .context("the compositor offers no xdg_wm_base")?;
        let surface = compositor.create_surface(&qh, ());
        let fractional_manager: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1> =
            events.bind(&registry, &qh, 1);
        let viewporter: Option<wp_viewporter::WpViewporter> = events.bind(&registry, &qh, 1);
        let (fractional, viewport) = match (&fractional_manager, &viewporter) {
            (Some(manager), Some(viewporter)) => (
                Some(manager.get_fractional_scale(&surface, &qh, ())),
                Some(viewporter.get_viewport(&surface, &qh, ())),
            ),
            _ => (None, None),
        };
        // Child objects remain valid after destroying these factory objects.
        if let Some(manager) = fractional_manager {
            manager.destroy();
        }
        if let Some(viewporter) = viewporter {
            viewporter.destroy();
        }
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());
        toplevel.set_title(title.to_string());
        toplevel.set_app_id(APP_ID.to_string());
        let seat = events.bind(&registry, &qh, 5);
        // No buffer on the initial commit: acknowledge configure before WSI attaches.
        surface.commit();
        queue
            .roundtrip(&mut events)
            .context("the first configure")?;
        let mut window = Self {
            toplevel,
            xdg_surface,
            fractional,
            viewport,
            surface,
            _wm_base: wm_base,
            _seat: seat,
            events,
            queue,
            connection,
            logical: size,
            scale: WindowScale::Unscaled,
            geometry: Arc::new(Mutex::new((size, WindowScale::Unscaled))),
        };
        window.take_size();
        let _ = window.connection.flush();
        Ok(window)
    }

    pub fn handles(&self) -> (*mut c_void, *mut c_void) {
        (
            self.connection.backend().display_ptr() as *mut c_void,
            self.surface.id().as_ptr() as *mut c_void,
        )
    }
    /// Physical pixels, for swapchain creation and the present pass.
    pub fn size(&self) -> (u32, u32) {
        self.scale.physical_size(self.logical)
    }

    /// The presenter owns all pending buffer-scale/viewport writes, immediately before
    /// the matching swapchain's present. The event loop only publishes geometry.
    pub fn surface_sizing(&self) -> cubarium_gpu::target::window::SurfaceSizing {
        let geometry = self.geometry.clone();
        let surface = self.surface.clone();
        let viewport = self.viewport.clone();
        let mut applied = None;
        Box::new(move || {
            let current @ (logical, scale) = *geometry.lock().expect("window geometry lock");
            if applied != Some(current) {
                if let Some(viewport) = &viewport {
                    if surface.version() >= 3 {
                        surface.set_buffer_scale(1);
                    }
                    viewport.set_destination(logical.0 as i32, logical.1 as i32);
                } else if let WindowScale::Integer(scale) = scale {
                    surface.set_buffer_scale(scale as i32);
                }
                applied = Some(current);
            }
            scale.physical_size(logical)
        })
    }
    pub fn describe(&self) -> String {
        format!(
            "logical {}x{}, scale {:.3} ({:?})",
            self.logical.0,
            self.logical.1,
            self.scale.factor(),
            self.scale
        )
    }
    pub fn closed(&self) -> bool {
        self.events.closed
    }
    pub fn take_save(&mut self) -> bool {
        std::mem::take(&mut self.events.save)
    }
    pub fn escape(&self) -> bool {
        self.events.escape
    }

    /// Read only when the socket is ready: WSI on the other thread also reads it.
    pub fn pump(&mut self) -> Result<()> {
        let _ = self.connection.flush();
        if let Some(guard) = self.queue.prepare_read() {
            if readable(self.connection.as_fd()) {
                if let Err(e) = guard.read() {
                    if !matches!(&e, wayland_client::backend::WaylandError::Io(e) if e.kind() == std::io::ErrorKind::WouldBlock)
                    {
                        return Err(e).context("reading the Wayland socket");
                    }
                }
            } // Dropping an unread guard cancels the read.
        }
        self.queue
            .dispatch_pending(&mut self.events)
            .context("dispatching Wayland events")?;
        self.take_size();
        Ok(())
    }

    fn take_size(&mut self) {
        let old = (self.logical, self.scale);
        if let Some((w, h)) = self.events.configured.take() {
            if w > 0 {
                self.logical.0 = w;
            }
            if h > 0 {
                self.logical.1 = h;
            }
        }
        self.scale = self
            .events
            .scale(self.viewport.is_some(), self.surface.version());
        if old != (self.logical, self.scale) {
            *self.geometry.lock().expect("window geometry lock") = (self.logical, self.scale);
            let physical = self.size();
            eprintln!(
                "cubarium window: {}, requested swapchain {}x{}",
                self.describe(),
                physical.0,
                physical.1
            );
        }
    }
}

impl Drop for WaylandWindow {
    fn drop(&mut self) {
        self.toplevel.destroy();
        self.xdg_surface.destroy();
        if let Some(fractional) = &self.fractional {
            fractional.destroy();
        }
        if let Some(viewport) = &self.viewport {
            viewport.destroy();
        }
        self.surface.destroy();
        let _ = self.connection.flush();
    }
}

/// Whether `fd` has something to read now: poll(2) with a zero timeout, through rustix.
fn readable(fd: std::os::fd::BorrowedFd<'_>) -> bool {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    let mut fds = [PollFd::new(&fd, PollFlags::IN)];
    let now = Timespec { tv_sec: 0, tv_nsec: 0 };
    matches!(poll(&mut fds, Some(&now)), Ok(n) if n > 0) && fds[0].revents().contains(PollFlags::IN)
}
