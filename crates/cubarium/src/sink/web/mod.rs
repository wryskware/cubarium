//! Web sink: a minimal HTTP/1.1 server on the loopback that serves a viewer page at `/`
//! and the newest encoded frame at `/frame`. It exists so the world can be watched and
//! screenshotted without the cube.
//!
//! Same newest-frame mailbox as the shim sink: `submit` replaces the frame in a
//! `Mutex<Option<..>>` and returns; it never touches a socket, so the simulation loop
//! never waits on a browser. A viewer that polls slower than the host renders simply
//! sees fewer, newer frames; a viewer that polls faster re-reads the same frame.
//!
//! The 8-byte prefix on `/frame` is the *render sequence* — how many frames this sink has
//! been handed — not the world's tick. The world's tick arrives separately through
//! [`FrameSink::observe_tick`] and is reported, with a read-only description of the host
//! process, at `GET /status`. Optional care routes separately admit bounded, journaled
//! input through the owning runner; frame and status reads cannot change the world.
//!
//! `std::net` only — no HTTP crate. The surface is five routes and `Connection: close`
//! per request, which is all a `fetch` loop from one page on the loopback needs.

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result};
use cube_proto::{FRAME_BYTES, Frame, Raster};

use super::{FrameSink, Output, WorldShape};
use crate::care::{self, CareKind, CareShared, CareTarget};

/// The viewer page, embedded so a running host has no runtime asset dependency.
pub const INDEX_HTML: &str = include_str!("index.html");

/// Length of a `/frame` body on a **cube** world: an 8-byte little-endian render sequence
/// then the encoded frame. A ring world's body is `8 + w·h·3`; the page reads the size it
/// must expect from `/status` rather than from a constant.
pub const FRAME_BODY_BYTES: usize = 8 + FRAME_BYTES;

/// Backoff on resource/socket errors, and the non-Unix fallback wait.
const ACCEPT_POLL: Duration = Duration::from_millis(10);
/// Read and write timeout on one socket operation.
const CONN_TIMEOUT: Duration = Duration::from_secs(5);
/// Absolute deadline for receiving a whole request — head *and* body — measured from the
/// moment the connection was accepted.
///
/// [`CONN_TIMEOUT`] alone does not bound a handler: it is a *per read* timeout, so a peer
/// that sends one byte every 4.9 s renews it forever and keeps its thread for hours. This
/// is the bound that actually makes a handler short-lived. It is deliberately separate
/// from the care service's own five-second wait for a durable acknowledgement, which
/// begins only once a complete, valid request has arrived.
const REQUEST_DEADLINE: Duration = Duration::from_secs(5);
/// Connection handler threads alive at once. Excess connections are closed immediately in
/// the accept loop, without spawning a thread and without the loop waiting for anything:
/// the care limits (four outstanding, 64 clients, eight-deep intake) all apply *after* a
/// request has been read, so none of them can bound the threads doing the reading.
const MAX_HANDLERS: usize = 32;
/// Longest request head accepted. A `GET` from the viewer page is a few hundred bytes.
const MAX_REQUEST_BYTES: usize = 8 * 1024;

/// One rendered image, owned, in the shape the world has. The server hands its bytes to
/// whichever connection asks; it never converts between the two.
pub(crate) enum Image {
    Cube(Frame),
    Ring(Raster),
}

impl Image {
    fn of(out: Output<'_>) -> Image {
        match out {
            Output::Cube(f) => Image::Cube(f.clone()),
            Output::Ring(r) => Image::Ring(r.clone()),
        }
    }

    fn bytes(&self) -> &[u8] {
        match self {
            Image::Cube(f) => f.as_bytes().as_slice(),
            Image::Ring(r) => r.as_bytes(),
        }
    }
}

/// The newest image and the render sequence it was submitted under.
type Slot = Option<Arc<(u64, Image)>>;

/// A read-only description of the host process behind this viewer, fixed for the life of
/// the sink. It answers "whose world am I looking at?" for a viewer that may be one of
/// several tabs on one machine — the state directory, the process, the build, and how the
/// pixels are also leaving the host. Wall time, pids and paths are host facts: nothing
/// here comes from, or reaches, `cubarium-core`.
#[derive(Clone, Debug)]
pub struct Source {
    pub pid: u32,
    /// Absolute (canonical where possible) state directory.
    pub state_dir: String,
    /// `crate::state::build_id()` of the running host.
    pub build_id: String,
    /// The primary sink the same frames are going to (`shim`, `preview`, `png`, `web`).
    pub sink: String,
    /// The snapshot this run resumed from, if any.
    pub resumed_from: Option<String>,
    /// The world tick this run started at.
    pub start_tick: u64,
    /// `--speed`.
    pub speed: f64,
}

impl Default for Source {
    fn default() -> Source {
        Source {
            pid: std::process::id(),
            state_dir: String::new(),
            build_id: crate::state::build_id(),
            sink: "web".to_string(),
            resumed_from: None,
            start_tick: 0,
            speed: 0.0,
        }
    }
}

impl Source {
    /// The `source` object of `/status`, hand-built so the key order is the documented
    /// one rather than `serde_json`'s sorted map.
    fn to_json(&self) -> String {
        let s = |v: &str| serde_json::Value::from(v).to_string();
        let resumed = match &self.resumed_from {
            Some(p) => s(p),
            None => "null".to_string(),
        };
        // `--speed` is validated finite before a run starts; a non-finite one would have
        // no JSON spelling, so it is reported as null rather than as a lie.
        let speed =
            serde_json::Number::from_f64(self.speed).map_or("null".to_string(), |n| n.to_string());
        format!(
            r#"{{"pid":{},"state_dir":{},"build_id":{},"sink":{},"resumed_from":{resumed},"start_tick":{},"speed":{speed}}}"#,
            self.pid,
            s(&self.state_dir),
            s(&self.build_id),
            s(&self.sink),
            self.start_tick,
        )
    }
}

struct Shared {
    /// Newest-frame mailbox. Unlike the shim's, the server does not *take* the frame: a
    /// viewer polling at its own rate must always find the latest one here.
    slot: Mutex<Slot>,
    stop: AtomicBool,
    /// Frames submitted since start; also the render sequence of the next one.
    ticks: AtomicU64,
    /// Requests answered on `/frame`.
    served: AtomicU64,
    /// The world's tick as of the last completed tick the host reported.
    world_tick: AtomicU64,
    /// The living population as of that same tick.
    population: AtomicU64,
    /// How many of those carry a recurrent policy (`run --neural`). Zero in every ordinary
    /// world, which is what makes a seeded one checkable from outside.
    neural_animals: AtomicU64,
    /// Connection handler threads currently alive; the permit is released on every exit
    /// path by [`HandlerPermit`]'s `Drop`.
    handlers: AtomicU64,
    /// Connections closed without a handler because [`MAX_HANDLERS`] was already reached.
    refused: AtomicU64,
    /// A short line the viewer's HUD appends, fixed for the life of the sink. The host
    /// puts the simulation speed here so a reviewer can tell 1× from 8× on sight.
    note: String,
    source: Source,
    /// The port actually bound, so the `Host` and `Origin` guards on the care routes can
    /// check what a browser sent against what this server actually is.
    port: u16,
    /// Present only with `--care`. The one thing served here that is *not* read-only, and
    /// it still cannot reach the world: it can only put a validated request in a bounded
    /// queue the simulation owner drains at a boundary of its own choosing.
    care: Option<Arc<CareShared>>,
    /// The world's shape, fixed for the life of the sink. `/status` reports it so the page
    /// can pick its mode and size its canvas before the first `/frame` answer, and the
    /// care routes validate a target against it.
    shape: WorldShape,
}

impl Shared {
    fn newest(&self) -> Slot {
        self.slot.lock().expect("web mailbox poisoned").clone()
    }

    /// The `/status` body. Read straight off the atomics: it never takes the mailbox lock,
    /// so a status poll cannot delay `submit` either.
    fn status_json(&self) -> String {
        let submitted = self.ticks.load(Ordering::Relaxed);
        let (w, h) = self.shape.chart_size();
        // `--world-scale` is validated finite and positive before a run starts.
        let scale = serde_json::Number::from_f64(self.shape.scale.world())
            .map_or("null".to_string(), |n| n.to_string());
        format!(
            r#"{{"world_tick":{},"population":{},"neural_animals":{},"render_seq":{},"frames_served":{},"topology":"{}","w":{w},"h":{h},"scale":{scale},"source":{}}}"#,
            self.world_tick.load(Ordering::Relaxed),
            self.population.load(Ordering::Relaxed),
            self.neural_animals.load(Ordering::Relaxed),
            // The render sequence of the newest frame, which is exactly the number in the
            // `/frame` prefix; before the first submit both read 0.
            submitted.saturating_sub(1),
            self.served.load(Ordering::Relaxed),
            self.shape.name(),
            self.source.to_json(),
        )
    }

    /// The `/frame` body's length for this world: the 8-byte sequence plus the image.
    fn frame_body_bytes(&self) -> usize {
        let (w, h) = self.shape.chart_size();
        match self.shape.topology {
            cubarium_surface::Topology::Cube => FRAME_BODY_BYTES,
            cubarium_surface::Topology::Ring { .. } => 8 + usize::from(w) * usize::from(h) * 3,
        }
    }
}

/// Serves the newest frame over HTTP on `127.0.0.1:<port>`.
pub struct WebSink {
    shared: Arc<Shared>,
    server: Option<JoinHandle<()>>,
    addr: SocketAddr,
}

impl WebSink {
    /// Bind `127.0.0.1:port` and start the server thread. Port 0 binds an ephemeral port;
    /// read the real one back from [`WebSink::port`].
    pub fn new(port: u16) -> Result<WebSink> {
        WebSink::with_note(port, String::new())
    }

    /// [`WebSink::new`] with a HUD note served at `GET /note`. An empty note leaves the
    /// HUD exactly as it is; a non-empty one is appended to the live label.
    pub fn with_note(port: u16, note: impl Into<String>) -> Result<WebSink> {
        WebSink::with_source(port, note, Source::default())
    }

    /// [`WebSink::with_note`] plus the read-only host identity `GET /status` reports.
    pub fn with_source(port: u16, note: impl Into<String>, source: Source) -> Result<WebSink> {
        WebSink::with_care(port, note, source, None)
    }

    /// [`WebSink::with_source`] plus the care service, when `--care` asked for one. With
    /// `None`, `/care/status` answers `{"enabled": false}` and the two `POST` routes answer
    /// `503 care disabled` — the page can tell "this world does not offer care" from "this
    /// host has never heard of the route".
    pub fn with_care(
        port: u16,
        note: impl Into<String>,
        source: Source,
        care: Option<Arc<CareShared>>,
    ) -> Result<WebSink> {
        WebSink::with_world(port, note, source, care, WorldShape::CUBE)
    }

    /// [`WebSink::with_care`] for a world of a named shape. The shape is what `/status`
    /// reports as `topology`, `w`, `h` and `scale`, what sizes the `/frame` body, and what
    /// a care target is validated against — all of which the page needs before it has
    /// polled a frame, so it is given here and never inferred from the first image.
    pub fn with_world(
        port: u16,
        note: impl Into<String>,
        source: Source,
        care: Option<Arc<CareShared>>,
        shape: WorldShape,
    ) -> Result<WebSink> {
        let note = note.into();
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
            .with_context(|| format!("binding the web viewer to 127.0.0.1:{port}"))?;
        let addr = listener
            .local_addr()
            .context("reading the web viewer's local address")?;
        listener
            .set_nonblocking(true)
            .context("putting the web viewer's listener in non-blocking mode")?;

        let shared = Arc::new(Shared {
            slot: Mutex::new(None),
            stop: AtomicBool::new(false),
            ticks: AtomicU64::new(0),
            served: AtomicU64::new(0),
            world_tick: AtomicU64::new(0),
            population: AtomicU64::new(0),
            neural_animals: AtomicU64::new(0),
            handlers: AtomicU64::new(0),
            refused: AtomicU64::new(0),
            note,
            source,
            port: addr.port(),
            care,
            shape,
        });
        let server = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("cubarium-web".into())
                .spawn(move || accept_loop(&shared, &listener))
                .context("spawning the web viewer thread")?
        };
        Ok(WebSink {
            shared,
            server: Some(server),
            addr,
        })
    }

    /// The address actually bound (the resolved port when 0 was requested).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The port actually bound.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// The URL to open in a browser.
    pub fn url(&self) -> String {
        format!("http://{}/", self.addr)
    }

    /// Frames submitted since start. The newest frame's render sequence is this minus one.
    pub fn submitted(&self) -> u64 {
        self.shared.ticks.load(Ordering::Relaxed)
    }

    /// The world tick last reported through [`FrameSink::observe_tick`].
    pub fn world_tick(&self) -> u64 {
        self.shared.world_tick.load(Ordering::Relaxed)
    }

    /// The host identity served at `/status`.
    pub fn source(&self) -> &Source {
        &self.shared.source
    }

    /// `/frame` requests answered with a frame.
    pub fn served(&self) -> u64 {
        self.shared.served.load(Ordering::Relaxed)
    }

    /// Connection handler threads alive right now. Never above [`MAX_HANDLERS`].
    pub fn handlers(&self) -> u64 {
        self.shared.handlers.load(Ordering::Relaxed)
    }

    /// Connections closed in the accept loop because the handler cap was already reached.
    pub fn refused_connections(&self) -> u64 {
        self.shared.refused.load(Ordering::Relaxed)
    }

    /// The HUD note this sink serves at `/note` (empty when there is none).
    pub fn note(&self) -> &str {
        &self.shared.note
    }

    /// The world shape this viewer reports and serves.
    pub fn shape(&self) -> WorldShape {
        self.shared.shape
    }

    /// The newest cube frame in the mailbox and its render sequence, if one has been
    /// submitted. `None` on a ring world, whose images are rasters.
    pub fn newest(&self) -> Option<(u64, Frame)> {
        self.shared.newest().and_then(|a| match &a.1 {
            Image::Cube(f) => Some((a.0, f.clone())),
            Image::Ring(_) => None,
        })
    }

    /// The newest raster and its render sequence, on a ring world.
    pub fn newest_raster(&self) -> Option<(u64, Raster)> {
        self.shared.newest().and_then(|a| match &a.1 {
            Image::Ring(r) => Some((a.0, r.clone())),
            Image::Cube(_) => None,
        })
    }

    fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(s) = self.server.take() {
            let _ = s.join();
        }
    }
}

impl FrameSink for WebSink {
    fn submit(&mut self, out: Output<'_>) -> Result<()> {
        let seq = self.shared.ticks.fetch_add(1, Ordering::Relaxed);
        let next = Arc::new((seq, Image::of(out)));
        let mut slot = self.shared.slot.lock().expect("web mailbox poisoned");
        // The newest frame always wins; nothing here waits on a client.
        *slot = Some(next);
        Ok(())
    }

    fn observe_counts(&mut self, population: usize, neural: usize) {
        self.shared
            .population
            .store(population as u64, Ordering::Relaxed);
        self.shared
            .neural_animals
            .store(neural as u64, Ordering::Relaxed);
    }

    fn observe_tick(&mut self, tick: u64) {
        self.shared.world_tick.store(tick, Ordering::Relaxed);
    }

    fn finish(&mut self) -> Result<()> {
        let (addr, submitted, served) = (self.addr, self.submitted(), self.served());
        self.stop();
        eprintln!("cubarium: web {addr}: {submitted} frames submitted, {served} served");
        Ok(())
    }
}

impl Drop for WebSink {
    fn drop(&mut self) {
        self.stop();
    }
}

/// One live connection handler. Taking the permit is a non-blocking compare-and-swap, and
/// dropping it — on *every* exit path, including a panicking handler — gives it back.
struct HandlerPermit(Arc<Shared>);

impl HandlerPermit {
    /// A permit if one is free, `None` if [`MAX_HANDLERS`] are already alive. Never waits.
    fn take(shared: &Arc<Shared>) -> Option<HandlerPermit> {
        shared
            .handlers
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < MAX_HANDLERS as u64).then(|| n + 1)
            })
            .ok()
            .map(|_| HandlerPermit(Arc::clone(shared)))
    }
}

impl Drop for HandlerPermit {
    fn drop(&mut self) {
        self.0.handlers.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Accept connections until the stop flag is set, handing each to a short-lived thread so
/// one slow client cannot delay the next.
///
/// "Short-lived" is enforced, not assumed: a handler needs one of [`MAX_HANDLERS`] permits,
/// and it has [`REQUEST_DEADLINE`] to receive a whole request. Beyond the cap the loop
/// closes the connection immediately and goes back to accepting — it never waits for a
/// permit, because a loop that blocks is a loop the next connection cannot reach either.
fn accept_loop(shared: &Arc<Shared>, listener: &TcpListener) {
    while !shared.stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let Some(permit) = HandlerPermit::take(shared) else {
                    shared.refused.fetch_add(1, Ordering::Relaxed);
                    // Closed without a byte of response: a client holding sockets open gets
                    // nothing from them, and the loop is back at `accept` immediately.
                    drop(stream);
                    continue;
                };
                let shared = Arc::clone(shared);
                let spawned = std::thread::Builder::new()
                    .name("cubarium-web-conn".into())
                    .spawn(move || {
                        let _permit = permit;
                        handle(&shared, stream);
                    });
                if spawned.is_err() {
                    // Out of threads: drop the connection rather than stall the loop. The
                    // permit went into the closure that was never created, so it is already
                    // dropped and the count is correct.
                    std::thread::sleep(ACCEPT_POLL);
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => wait_for_connection(listener),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => std::thread::sleep(ACCEPT_POLL),
        }
    }
}

/// Wait for readiness rather than quantizing every browser request to a 10ms poll.
/// The listener stays nonblocking: readiness is only a hint, and the next accept
/// can still return WouldBlock. A finite idle timeout bounds shutdown even if no
/// client ever connects, without allocating a wakeup socket or spinning at 1kHz.
#[cfg(unix)]
#[allow(unsafe_code)] // Audited poll(2) wrapper only; all HTTP handling stays safe Rust.
fn wait_for_connection(listener: &TcpListener) {
    use std::os::fd::AsRawFd;
    let mut descriptor = libc::pollfd {
        fd: listener.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one initialized pollfd remains exclusively borrowed for the call;
    // its descriptor is owned by the listener, which outlives this wait. poll
    // neither takes ownership nor reads beyond the one-element array. The finite
    // 100ms timeout keeps the thread responsive to its atomic shutdown flag.
    let result = unsafe { libc::poll(&mut descriptor, 1, 100) };
    if result < 0 || (result > 0 && descriptor.revents & libc::POLLIN == 0) {
        // Avoid a busy loop on an OS error, including POLLNVAL/POLLERR/POLLHUP.
        std::thread::sleep(ACCEPT_POLL);
    }
}

#[cfg(not(unix))]
fn wait_for_connection(_listener: &TcpListener) {
    std::thread::sleep(ACCEPT_POLL);
}

/// Answer exactly one request, then close.
fn handle(shared: &Shared, mut stream: TcpStream) {
    // The listener is non-blocking; accepted sockets must not inherit that.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(CONN_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CONN_TIMEOUT));
    let _ = stream.set_nodelay(true);
    let deadline = std::time::Instant::now() + REQUEST_DEADLINE;

    let (head, buffered) = match read_head(&mut stream, deadline) {
        Some(h) => h,
        None => return,
    };
    let route = request_path(&head);

    let _ = match route {
        // --- care ----------------------------------------------------------------
        Some(("GET", "/care/status")) => respond(
            &mut stream,
            "200 OK",
            "application/json; charset=utf-8",
            "Cache-Control: no-store\r\n",
            match &shared.care {
                Some(care) => care.status_json(),
                None => CareShared::disabled_status_json(),
            }
            .as_bytes(),
        ),
        // A mutation through GET is refused outright. It is the one request a
        // cross-origin page can make without a preflight, so it must never do anything.
        Some(("GET", "/care")) | Some(("GET", "/care/register")) => respond(
            &mut stream,
            "405 Method Not Allowed",
            "application/json; charset=utf-8",
            "Allow: POST\r\nCache-Control: no-store\r\n",
            br#"{"error":"care is POST only"}"#,
        ),
        Some(("POST", "/care/register")) => care_route(
            &mut stream,
            shared,
            &head,
            buffered,
            CareRoute::Register,
            deadline,
        ),
        Some(("POST", "/care")) => care_route(
            &mut stream,
            shared,
            &head,
            buffered,
            CareRoute::Submit,
            deadline,
        ),
        Some(("GET", "/")) | Some(("GET", "/index.html")) => respond(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            "",
            INDEX_HTML.as_bytes(),
        ),
        Some(("GET", "/frame")) => {
            let body = frame_body(&shared.newest(), shared.frame_body_bytes());
            shared.served.fetch_add(1, Ordering::Relaxed);
            respond(
                &mut stream,
                "200 OK",
                "application/octet-stream",
                "Cache-Control: no-store\r\n",
                &body,
            )
        }
        Some(("GET", "/status")) => respond(
            &mut stream,
            "200 OK",
            "application/json; charset=utf-8",
            "Cache-Control: no-store\r\n",
            shared.status_json().as_bytes(),
        ),
        Some(("GET", "/note")) => respond(
            &mut stream,
            "200 OK",
            "text/plain; charset=utf-8",
            "Cache-Control: no-store\r\n",
            shared.note.as_bytes(),
        ),
        _ => respond(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "",
            b"not found\n",
        ),
    };
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

// --- the care routes -----------------------------------------------------------------

/// Which of the two mutating routes is being served.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CareRoute {
    Register,
    Submit,
}

/// One JSON answer. There are deliberately **no** `Access-Control-*` headers anywhere in
/// this file: without them a browser will not hand a cross-origin page the response, and
/// the preflight that would be needed to send `X-Cubarium-Care` is never answered either
/// (it falls through to the 404 arm). Those two facts together are what keep a random page
/// on the internet from watering someone's world.
fn json(stream: &mut TcpStream, status: &str, body: &str) -> std::io::Result<()> {
    respond(
        stream,
        status,
        "application/json; charset=utf-8",
        "Cache-Control: no-store\r\n",
        body.as_bytes(),
    )
}

fn json_error(stream: &mut TcpStream, status: &str, message: &str) -> std::io::Result<()> {
    json(
        stream,
        status,
        &format!(r#"{{"error":{}}}"#, serde_json::Value::from(message)),
    )
}

/// Everything a care request must satisfy before the service ever sees it.
///
/// The custom header is the load-bearing one: a cross-origin page cannot set
/// `X-Cubarium-Care` on a `fetch` without triggering a CORS preflight, and this server
/// answers no preflight. `Host` and `Origin` are checked against the port actually bound
/// so a DNS-rebinding page that resolves some other name to 127.0.0.1 is refused too.
///
/// The statuses here are the ordinary HTTP ones for each refusal; the contract enumerates
/// the *care* outcomes (`400/409/429/503`) and leaves these transport checks unnumbered.
fn care_guard(head: &str, port: u16) -> Option<(&'static str, String)> {
    if header_value(head, "x-cubarium-care") != Some("1") {
        return Some((
            "403 Forbidden",
            "a care request must carry the header X-Cubarium-Care: 1".to_string(),
        ));
    }
    let expected = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    match header_value(head, "host") {
        Some(host) if expected.iter().any(|e| e == host) => {}
        Some(host) => {
            return Some((
                "403 Forbidden",
                format!("this server is 127.0.0.1:{port}, not {host}"),
            ));
        }
        None => {
            return Some((
                "403 Forbidden",
                "a care request needs a Host header".to_string(),
            ));
        }
    }
    // `Origin` is optional (a same-origin `fetch` from a page loaded over http may omit
    // it); when it is there it must be *exactly* one of this server's two origins. Not
    // `https://`: this server has no TLS, so an `https` origin is by definition some other
    // server that a browser has been talked into believing is this one.
    if let Some(origin) = header_value(head, "origin") {
        let ok = expected.iter().any(|e| origin == format!("http://{e}"));
        if !ok {
            return Some((
                "403 Forbidden",
                format!("cross-origin care is refused: {origin} is not this viewer"),
            ));
        }
    }
    match header_value(head, "content-type") {
        Some(ct) if ct.split(';').next().unwrap_or("").trim() == "application/json" => {}
        _ => {
            return Some((
                "415 Unsupported Media Type",
                "a care request must be Content-Type: application/json".to_string(),
            ));
        }
    }
    None
}

/// Serve `POST /care/register` or `POST /care`.
fn care_route(
    stream: &mut TcpStream,
    shared: &Shared,
    head: &str,
    buffered: Vec<u8>,
    route: CareRoute,
    deadline: std::time::Instant,
) -> std::io::Result<()> {
    if let Some((status, message)) = care_guard(head, shared.port) {
        return json_error(stream, status, &message);
    }
    let body = match read_body(stream, head, buffered, deadline) {
        Ok(body) => body,
        Err(BodyError::TooLarge) => {
            return json_error(
                stream,
                "413 Payload Too Large",
                &format!(
                    "a care request body must be at most {} bytes",
                    care::MAX_BODY_BYTES
                ),
            );
        }
        Err(BodyError::Invalid(message)) => {
            return json_error(stream, "400 Bad Request", message);
        }
    };
    // Only now, with the request proven well-formed and same-origin, is the service told
    // anything at all.
    let Some(care) = shared.care.as_ref() else {
        return json_error(stream, "503 Service Unavailable", "care disabled");
    };
    match route {
        CareRoute::Register => {
            let outcome = care.register();
            json(stream, outcome.http_status(), &outcome.body())
        }
        CareRoute::Submit => match parse_care_request(&body, shared.shape.topology) {
            Err(message) => json_error(stream, "400 Bad Request", message),
            Ok(CareRequest::Care {
                client,
                request,
                kind,
                target,
                dose,
            }) => {
                let outcome = care.submit_dosed(&client, request, kind, target, dose);
                json(stream, outcome.http_status(), &outcome.body())
            }
            Ok(CareRequest::SpawnApex {
                client,
                request,
                count,
            }) => {
                let outcome = care.submit_apex(&client, request, count);
                json(stream, outcome.http_status(), &outcome.body())
            }
        },
    }
}

/// One parsed `POST /care` body.
enum CareRequest {
    Care {
        client: String,
        request: u64,
        kind: CareKind,
        target: CareTarget,
        dose: care::CareDose,
    },
    SpawnApex {
        client: String,
        request: u64,
        count: u8,
    },
}

/// `{"client": "...", "request": N, "kind": "feed", "target": {"face": f, "u": u, "v": v}}`,
/// with one optional property: `"dose_permille": N`.
///
/// Every other field is required and nothing is guessed at: a request the host cannot read
/// exactly is a `400`, never a command aimed at a cell nobody chose.
///
/// The amount is the same rule applied to a value that may be absent. **Absent** means exactly
/// the standard dose — that is what every request meant before this property existed. Anything
/// *present* must be an integer inside the documented range: an explicit `null`, a fractional
/// or negative number, a string, or a value outside 250..=2000 is a `400`. None of them is
/// clamped, and none of them is treated as omission — a page that sent `null` because its
/// selector was empty asked a question this host cannot answer, and telling it so is the only
/// answer that cannot deliver an amount nobody chose.
fn parse_care_request(
    body: &[u8],
    topology: cubarium_surface::Topology,
) -> Result<CareRequest, &'static str> {
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| "the request body is not JSON")?;
    let client = value
        .get("client")
        .and_then(|v| v.as_str())
        .ok_or("`client` must be the identity from /care/register")?
        .to_string();
    let request = value
        .get("request")
        .and_then(serde_json::Value::as_u64)
        .ok_or("`request` must be this client's monotonic request number")?;
    let kind_name = value
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or("`kind` must be a string")?;
    if kind_name == "spawn_apex" {
        let count = value
            .get("count")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u8::try_from(n).ok())
            .filter(|n| (1..=2).contains(n))
            .ok_or("`count` must be 1 or 2 for spawn_apex")?;
        if value.get("target").is_some() || value.get("dose_permille").is_some() {
            return Err("spawn_apex chooses random locations and accepts no target or dose");
        }
        return Ok(CareRequest::SpawnApex {
            client,
            request,
            count,
        });
    }
    let kind =
        CareKind::parse(kind_name).ok_or("`kind` must be feed, rain, clean or spawn_apex")?;
    let target = value.get("target").ok_or("`target` must be {face, u, v}")?;
    // `u` and `v` are JSON integers and always were; only the accepted range grew, from a
    // byte to the world's own extent. A page that sent `{"face":0,"u":12,"v":34}` before
    // the widening sends exactly the same bytes and gets exactly the same cell.
    let component = |key: &str| -> Result<u16, &'static str> {
        target
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u16::try_from(n).ok())
            .ok_or("`target` must be {face, u, v} with small non-negative integers")
    };
    let target = CareTarget {
        face: u8::try_from(component("face")?)
            .map_err(|_| "`target` must be {face, u, v} with small non-negative integers")?,
        u: component("u")?,
        v: component("v")?,
    };
    // Against *this* world: a 320-pixel ring accepts u = 200, a cube refuses it.
    target.validate_on(topology)?;
    let dose = match value.get("dose_permille") {
        None => care::CareDose::STANDARD,
        Some(raw) => {
            let n = raw
                .as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .ok_or("`dose_permille` must be an integer, or omitted for the standard dose")?;
            care::CareDose::new(n)
                .map_err(|_| "`dose_permille` must be an integer from 250 to 2000")?
        }
    };
    Ok(CareRequest::Care {
        client,
        request,
        kind,
        target,
        dose,
    })
}

/// The `/frame` body: 8-byte little-endian render sequence then the image bytes — the
/// cube's 61,440 encoded frame bytes, or a ring's `w·h·3` raster bytes. With no image yet,
/// sequence 0 and a black one of the right size, so the page has something valid to draw
/// immediately.
///
/// `expect` is the world's own body length, from [`Shared::frame_body_bytes`]. The page
/// checks the length it computes from `/status` against what arrives, so a mailbox holding
/// the wrong shape — which nothing can produce, since one sink serves one world — would be
/// visible rather than drawn as garbage.
fn frame_body(slot: &Slot, expect: usize) -> Vec<u8> {
    let mut body = Vec::with_capacity(expect);
    match slot {
        Some(entry) => {
            body.extend_from_slice(&entry.0.to_le_bytes());
            body.extend_from_slice(entry.1.bytes());
        }
        None => {
            body.extend_from_slice(&0u64.to_le_bytes());
            body.resize(expect, 0);
        }
    }
    debug_assert_eq!(body.len(), expect);
    body
}

/// Read the request head (everything before the blank line), plus whatever bytes of the
/// body arrived in the same read. `None` on a timeout, a closed connection, or a head that
/// runs past [`MAX_REQUEST_BYTES`].
///
/// The over-read matters now that there are `POST` routes: a `fetch` sends the head and a
/// small JSON body in one segment, so the body is usually already in this buffer and
/// [`read_body`] must not go looking for it on the socket.
fn read_head(stream: &mut TcpStream, deadline: std::time::Instant) -> Option<(String, Vec<u8>)> {
    let mut buf = Vec::with_capacity(512);
    let mut chunk = [0u8; 512];
    loop {
        if std::time::Instant::now() >= deadline {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if let Some(end) = find_head_end(&buf) {
                    // Checked on the *resolved* head, not on the buffer before the
                    // delimiter was found: a blank line arriving in the read that crosses
                    // the cap would otherwise admit an oversized head.
                    if end > MAX_REQUEST_BYTES {
                        return None;
                    }
                    let head = String::from_utf8(buf[..end].to_vec()).ok()?;
                    let body_start = if buf[end..].starts_with(b"\r\n\r\n") {
                        end + 4
                    } else {
                        end + 2
                    };
                    let rest = buf.get(body_start..).unwrap_or_default().to_vec();
                    return Some((head, rest));
                }
                if buf.len() > MAX_REQUEST_BYTES {
                    return None;
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
}

/// One header's value, matched case-insensitively as HTTP requires. The first occurrence
/// wins; a duplicated header is not merged, because nothing here takes a list.
fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

/// Why a body could not be taken.
enum BodyError {
    /// Past [`care::MAX_BODY_BYTES`].
    TooLarge,
    Invalid(&'static str),
}

/// Read exactly `Content-Length` bytes, refusing anything past the contract's 4 KiB.
///
/// `Content-Length` is required and `Transfer-Encoding` is refused: the only client is a
/// page on the loopback posting a hundred bytes of JSON, and a chunked reader would be
/// unbounded input handling written for no caller.
fn read_body(
    stream: &mut TcpStream,
    head: &str,
    buffered: Vec<u8>,
    deadline: std::time::Instant,
) -> Result<Vec<u8>, BodyError> {
    if header_value(head, "transfer-encoding").is_some() {
        return Err(BodyError::Invalid(
            "a chunked body is not accepted; send Content-Length",
        ));
    }
    let declared = header_value(head, "content-length")
        .ok_or(BodyError::Invalid("a care request needs a Content-Length"))?;
    let len: usize = declared
        .parse()
        .map_err(|_| BodyError::Invalid("Content-Length is not a number"))?;
    if len > care::MAX_BODY_BYTES {
        return Err(BodyError::TooLarge);
    }
    let mut body = buffered;
    body.truncate(len);
    let mut chunk = [0u8; 512];
    while body.len() < len {
        if std::time::Instant::now() >= deadline {
            return Err(BodyError::Invalid(
                "the request body did not arrive in time",
            ));
        }
        match stream.read(&mut chunk) {
            Ok(0) => return Err(BodyError::Invalid("the request body ended early")),
            Ok(n) => {
                let take = n.min(len - body.len());
                body.extend_from_slice(&chunk[..take]);
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Err(BodyError::Invalid("the request body could not be read")),
        }
    }
    Ok(body)
}

/// Index just past the end of the request head, accepting both CRLFCRLF and LFLF.
fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .or_else(|| buf.windows(2).position(|w| w == b"\n\n"))
}

/// Method and path from a request head, with any query string stripped.
fn request_path(head: &str) -> Option<(&str, &str)> {
    let mut parts = head.lines().next()?.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    let path = target.split(['?', '#']).next().unwrap_or(target);
    Some((method, path))
}

fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    extra_headers: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         {extra_headers}Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

#[cfg(test)]
mod tests;
