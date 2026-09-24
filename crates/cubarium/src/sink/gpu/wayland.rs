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

use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;

use anyhow::{Context, Result};
use wayland_client::protocol::{wl_compositor, wl_keyboard, wl_seat, wl_surface};
use wayland_client::{Display, EventQueue, GlobalManager, Main};
use wayland_protocols::xdg_shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

/// evdev's `KEY_ESC`: the key is the only one the window listens for, so no keymap is
/// needed to read it.
const KEY_ESC: u32 = 1;

/// The app id, which is also the class a compositor rule matches.
const APP_ID: &str = "cubarium";

/// What the event callbacks leave for the loop to read.
#[derive(Default)]
struct Events {
    /// The size the compositor last asked for; 0 on an axis leaves it to the window.
    configured: Option<(u32, u32)>,
    closed: bool,
    escape: bool,
}

/// One toplevel on its own connection to the compositor.
pub struct WaylandWindow {
    // Declared first, dropped first: the role objects go before the surface, and all of
    // them before the connection.
    toplevel: Main<xdg_toplevel::XdgToplevel>,
    xdg_surface: Main<xdg_surface::XdgSurface>,
    surface: Main<wl_surface::WlSurface>,
    _wm_base: Main<xdg_wm_base::XdgWmBase>,
    _seat: Option<Main<wl_seat::WlSeat>>,
    _keyboard: Rc<RefCell<Option<Main<wl_keyboard::WlKeyboard>>>>,
    events: Rc<RefCell<Events>>,
    queue: EventQueue,
    display: Display,
    size: (u32, u32),
}

impl WaylandWindow {
    /// Open a toplevel titled `title` at `size` pixels, until the compositor says
    /// otherwise. Fails where there is no Wayland display or no xdg-shell, and the caller
    /// falls back.
    ///
    /// `check` is handed the `wl_display *` as soon as the connection exists, and before
    /// any surface does: whatever can refuse this window without one (can the GPU present
    /// here at all?) refuses it there, so that a fallback window is this process's first
    /// window and not its second (`cubarium_gpu::target::window::check_wayland`).
    pub fn open(
        title: &str,
        size: (u32, u32),
        check: impl FnOnce(*mut c_void) -> Result<()>,
    ) -> Result<WaylandWindow> {
        // An agent's window only through `scripts/hidden.sh`, checked before anything is
        // committed (`sink::hidden`).
        crate::sink::hidden::window_allowed()?;
        let display = Display::connect_to_env().context("connecting to the Wayland display")?;
        check(display.get_display_ptr() as *mut c_void)?;
        let mut queue = display.create_event_queue();
        let attached = (*display).clone().attach(queue.token());
        let globals = GlobalManager::new(&attached);
        queue
            .sync_roundtrip(&mut (), |_, _, _| {})
            .context("reading the Wayland registry")?;
        let compositor = globals
            .instantiate_range::<wl_compositor::WlCompositor>(1, 4)
            .context("the compositor offers no wl_compositor")?;
        let wm_base = globals
            .instantiate_range::<xdg_wm_base::XdgWmBase>(1, 2)
            .context("the compositor offers no xdg_wm_base")?;
        wm_base.quick_assign(|base, event, _| {
            if let xdg_wm_base::Event::Ping { serial } = event {
                base.pong(serial);
            }
        });
        let events = Rc::new(RefCell::new(Events::default()));
        let surface = compositor.create_surface();
        let xdg_surface = wm_base.get_xdg_surface(&surface);
        // Acknowledged as it arrives. The swapchain commits whenever it presents, and a
        // configure is a request for the window's next size, which the loop passes on.
        xdg_surface.quick_assign(|s, event, _| {
            if let xdg_surface::Event::Configure { serial } = event {
                s.ack_configure(serial);
            }
        });
        let toplevel = xdg_surface.get_toplevel();
        {
            let events = events.clone();
            toplevel.quick_assign(move |_, event, _| match event {
                xdg_toplevel::Event::Configure { width, height, .. } => {
                    events.borrow_mut().configured =
                        Some((width.max(0) as u32, height.max(0) as u32));
                }
                xdg_toplevel::Event::Close => events.borrow_mut().closed = true,
                _ => {}
            });
        }
        // **Before the first commit**, so that a rule matching either is applied when the
        // window maps.
        toplevel.set_title(title.to_string());
        toplevel.set_app_id(APP_ID.to_string());

        let keyboard = Rc::new(RefCell::new(None));
        let seat = globals.instantiate_range::<wl_seat::WlSeat>(1, 5).ok();
        if let Some(seat) = &seat {
            let (events, keyboard) = (events.clone(), keyboard.clone());
            seat.quick_assign(move |seat, event, _| {
                let wl_seat::Event::Capabilities { capabilities } = event else {
                    return;
                };
                let mut held = keyboard.borrow_mut();
                if !capabilities.contains(wl_seat::Capability::Keyboard) || held.is_some() {
                    return;
                }
                let k = seat.get_keyboard();
                let events = events.clone();
                k.quick_assign(move |_, event, _| match event {
                    wl_keyboard::Event::Key { key, state, .. }
                        if key == KEY_ESC && state == wl_keyboard::KeyState::Pressed =>
                    {
                        events.borrow_mut().escape = true;
                    }
                    // The keymap arrives as a descriptor this side owns; it is not needed.
                    wl_keyboard::Event::Keymap { fd, .. } => close(fd),
                    _ => {}
                });
                *held = Some(k);
            });
        }

        // The initial commit carries no buffer; the compositor answers with the first
        // configure, which is acknowledged before the swapchain attaches anything — the
        // order xdg-shell requires.
        surface.commit();
        queue
            .sync_roundtrip(&mut (), |_, _, _| {})
            .context("the first configure")?;
        let _ = display.flush();
        let mut window = WaylandWindow {
            toplevel,
            xdg_surface,
            surface,
            _wm_base: wm_base,
            _seat: seat,
            _keyboard: keyboard,
            events,
            queue,
            display,
            size,
        };
        window.take_size();
        Ok(window)
    }

    /// The `wl_display *` and `wl_surface *` a Vulkan surface is made on.
    pub fn handles(&self) -> (*mut c_void, *mut c_void) {
        (
            self.display.get_display_ptr() as *mut c_void,
            self.surface.as_ref().c_ptr() as *mut c_void,
        )
    }

    /// The window's size in pixels.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Whether the compositor asked to close it.
    pub fn closed(&self) -> bool {
        self.events.borrow().closed
    }

    /// Whether Escape has been pressed in it.
    pub fn escape(&self) -> bool {
        self.events.borrow().escape
    }

    /// Send what is queued and handle whatever has arrived, **without blocking**.
    ///
    /// The swapchain's WSI reads this same socket on the presenting thread, and
    /// libwayland makes a reader that finds another thread prepared to read wait for it.
    /// So the socket is read only when `poll` says there is something to read — then the
    /// other reader wakes too, and nobody waits long. `minifb` read unconditionally.
    pub fn pump(&mut self) -> Result<()> {
        // A full socket is WouldBlock and is retried on the next pump.
        let _ = self.display.flush();
        if let Some(guard) = self.queue.prepare_read() {
            if readable(self.display.get_connection_fd()) {
                if let Err(e) = guard.read_events()
                    && e.kind() != std::io::ErrorKind::WouldBlock
                {
                    return Err(e).context("reading the Wayland socket");
                }
            } else {
                guard.cancel();
            }
        }
        self.queue
            .dispatch_pending(&mut (), |_, _, _| {})
            .context("dispatching Wayland events")?;
        self.take_size();
        Ok(())
    }

    fn take_size(&mut self) {
        if let Some((w, h)) = self.events.borrow_mut().configured.take() {
            // Zero on an axis is the compositor leaving that axis to the window.
            if w > 0 {
                self.size.0 = w;
            }
            if h > 0 {
                self.size.1 = h;
            }
        }
    }
}

impl Drop for WaylandWindow {
    fn drop(&mut self) {
        self.toplevel.destroy();
        self.xdg_surface.destroy();
        self.surface.destroy();
        let _ = self.display.flush();
    }
}

/// Whether `fd` has something to read now.
#[allow(unsafe_code)] // Audited poll(2) wrapper only, zero timeout.
fn readable(fd: std::os::fd::RawFd) -> bool {
    let mut descriptor = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one initialised pollfd, exclusively borrowed for the call; the descriptor is
    // the display connection's, which outlives it. A zero timeout never sleeps.
    let n = unsafe { libc::poll(&mut descriptor, 1, 0) };
    n > 0 && descriptor.revents & libc::POLLIN != 0
}

/// Close a descriptor an event handed over.
#[allow(unsafe_code)] // Audited: closes the keymap fd wl_keyboard transferred to this process.
fn close(fd: std::os::fd::RawFd) {
    // SAFETY: the fd came in the event's SCM_RIGHTS and nothing else holds it.
    unsafe { libc::close(fd) };
}
