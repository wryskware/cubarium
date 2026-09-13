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
use cube_proto::{FRAME_BYTES, Frame};

use super::FrameSink;
use crate::care::{self, CareKind, CareShared, CareTarget};

/// The viewer page, embedded so a running host has no runtime asset dependency.
pub const INDEX_HTML: &str = include_str!("web/index.html");

/// Length of a `/frame` body: an 8-byte little-endian render sequence then the encoded
/// frame.
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

/// The newest frame and the render sequence it was submitted under.
type Slot = Option<Arc<(u64, Frame)>>;

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
        let speed = serde_json::Number::from_f64(self.speed)
            .map_or("null".to_string(), |n| n.to_string());
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
}

impl Shared {
    fn newest(&self) -> Slot {
        self.slot.lock().expect("web mailbox poisoned").clone()
    }

    /// The `/status` body. Read straight off the atomics: it never takes the mailbox lock,
    /// so a status poll cannot delay `submit` either.
    fn status_json(&self) -> String {
        let submitted = self.ticks.load(Ordering::Relaxed);
        format!(
            r#"{{"world_tick":{},"render_seq":{},"frames_served":{},"source":{}}}"#,
            self.world_tick.load(Ordering::Relaxed),
            // The render sequence of the newest frame, which is exactly the number in the
            // `/frame` prefix; before the first submit both read 0.
            submitted.saturating_sub(1),
            self.served.load(Ordering::Relaxed),
            self.source.to_json(),
        )
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
        let note = note.into();
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
            .with_context(|| format!("binding the web viewer to 127.0.0.1:{port}"))?;
        let addr = listener.local_addr().context("reading the web viewer's local address")?;
        listener
            .set_nonblocking(true)
            .context("putting the web viewer's listener in non-blocking mode")?;

        let shared = Arc::new(Shared {
            slot: Mutex::new(None),
            stop: AtomicBool::new(false),
            ticks: AtomicU64::new(0),
            served: AtomicU64::new(0),
            world_tick: AtomicU64::new(0),
            handlers: AtomicU64::new(0),
            refused: AtomicU64::new(0),
            note,
            source,
            port: addr.port(),
            care,
        });
        let server = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("cubarium-web".into())
                .spawn(move || accept_loop(&shared, &listener))
                .context("spawning the web viewer thread")?
        };
        Ok(WebSink { shared, server: Some(server), addr })
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

    /// The newest frame in the mailbox and its render sequence, if one has been submitted.
    pub fn newest(&self) -> Option<(u64, Frame)> {
        self.shared.newest().map(|a| (a.0, a.1.clone()))
    }

    fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(s) = self.server.take() {
            let _ = s.join();
        }
    }
}

impl FrameSink for WebSink {
    fn submit(&mut self, frame: &Frame) -> Result<()> {
        let seq = self.shared.ticks.fetch_add(1, Ordering::Relaxed);
        let next = Arc::new((seq, frame.clone()));
        let mut slot = self.shared.slot.lock().expect("web mailbox poisoned");
        // The newest frame always wins; nothing here waits on a client.
        *slot = Some(next);
        Ok(())
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
        Some(("POST", "/care/register")) => {
            care_route(&mut stream, shared, &head, buffered, CareRoute::Register, deadline)
        }
        Some(("POST", "/care")) => {
            care_route(&mut stream, shared, &head, buffered, CareRoute::Submit, deadline)
        }
        Some(("GET", "/")) | Some(("GET", "/index.html")) => respond(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            "",
            INDEX_HTML.as_bytes(),
        ),
        Some(("GET", "/frame")) => {
            let body = frame_body(&shared.newest());
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
        _ => respond(&mut stream, "404 Not Found", "text/plain; charset=utf-8", "", b"not found\n"),
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
    json(stream, status, &format!(r#"{{"error":{}}}"#, serde_json::Value::from(message)))
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
        None => return Some(("403 Forbidden", "a care request needs a Host header".to_string())),
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
                &format!("a care request body must be at most {} bytes", care::MAX_BODY_BYTES),
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
        CareRoute::Submit => match parse_care_request(&body) {
            Err(message) => json_error(stream, "400 Bad Request", message),
            Ok((client, request, kind, target)) => {
                let outcome = care.submit(&client, request, kind, target);
                json(stream, outcome.http_status(), &outcome.body())
            }
        },
    }
}

/// `{"client": "...", "request": N, "kind": "feed", "target": {"face": f, "u": u, "v": v}}`.
/// Every field is required and nothing is guessed at: a request the host cannot read
/// exactly is a `400`, never a command aimed at a cell nobody chose.
fn parse_care_request(body: &[u8]) -> Result<(String, u64, CareKind, CareTarget), &'static str> {
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
    let kind = value.get("kind").and_then(|v| v.as_str()).ok_or("`kind` must be a string")?;
    let kind = CareKind::parse(kind).ok_or("`kind` must be feed, rain or clean")?;
    let target = value.get("target").ok_or("`target` must be {face, u, v}")?;
    let component = |key: &str| -> Result<u8, &'static str> {
        target
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u8::try_from(n).ok())
            .ok_or("`target` must be {face, u, v} with small non-negative integers")
    };
    let target =
        CareTarget { face: component("face")?, u: component("u")?, v: component("v")? };
    target.validate()?;
    Ok((client, request, kind, target))
}

/// The `/frame` body: 8-byte little-endian render sequence then the frame bytes. With no
/// frame yet, sequence 0 and a black frame, so the page has something valid to draw
/// immediately. The layout is fixed: the page checks this length.
fn frame_body(slot: &Slot) -> Vec<u8> {
    let mut body = Vec::with_capacity(FRAME_BODY_BYTES);
    match slot {
        Some(entry) => {
            body.extend_from_slice(&entry.0.to_le_bytes());
            body.extend_from_slice(entry.1.as_bytes());
        }
        None => {
            body.extend_from_slice(&0u64.to_le_bytes());
            body.resize(FRAME_BODY_BYTES, 0);
        }
    }
    debug_assert_eq!(body.len(), FRAME_BODY_BYTES);
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
                    let body_start = if buf[end..].starts_with(b"\r\n\r\n") { end + 4 } else { end + 2 };
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
        return Err(BodyError::Invalid("a chunked body is not accepted; send Content-Length"));
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
            return Err(BodyError::Invalid("the request body did not arrive in time"));
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
    buf.windows(4).position(|w| w == b"\r\n\r\n").or_else(|| buf.windows(2).position(|w| w == b"\n\n"))
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
mod tests {
    use super::*;
    use cube_proto::{FACE_BYTES, Face};
    use cubarium_surface::face_frame;

    /// One request over a real socket, returning the status line, the headers and the
    /// body bytes.
    fn get(addr: SocketAddr, path: &str) -> (String, String, Vec<u8>) {
        let mut s = TcpStream::connect(addr).expect("connecting to the web sink");
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
        s.flush().unwrap();
        let mut raw = Vec::new();
        s.read_to_end(&mut raw).expect("reading the response");
        let end = find_head_end(&raw).expect("a complete response head");
        let head = String::from_utf8_lossy(&raw[..end]).into_owned();
        let body_start = if raw[end..].starts_with(b"\r\n\r\n") { end + 4 } else { end + 2 };
        let status = head.lines().next().unwrap_or_default().to_string();
        (status, head, raw[body_start..].to_vec())
    }

    /// A frame whose bytes are a recognizable function of `seed`.
    fn distinct_frame(seed: u8) -> Frame {
        let mut f = Frame::black();
        let bytes = f.as_bytes_mut();
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(seed);
        }
        f
    }

    #[test]
    fn the_index_route_serves_the_embedded_page() {
        let sink = WebSink::new(0).expect("binding an ephemeral port");
        let (status, head, body) = get(sink.addr(), "/");
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert!(head.contains("Content-Type: text/html; charset=utf-8"), "{head}");
        assert_eq!(body, INDEX_HTML.as_bytes(), "the body is the embedded page verbatim");
    }

    #[test]
    fn the_frame_route_serves_the_tick_and_the_submitted_frame() {
        let mut sink = WebSink::new(0).expect("binding an ephemeral port");
        let frame = distinct_frame(3);
        sink.submit(&frame).unwrap();

        let (status, head, body) = get(sink.addr(), "/frame");
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert!(head.contains("Content-Type: application/octet-stream"), "{head}");
        assert!(head.contains("Cache-Control: no-store"), "{head}");
        assert_eq!(body.len(), FRAME_BODY_BYTES, "8-byte sequence plus {FRAME_BYTES} frame bytes");
        assert_eq!(
            u64::from_le_bytes(body[..8].try_into().unwrap()),
            0,
            "the first frame is render sequence 0"
        );
        assert_eq!(&body[8..], frame.as_bytes().as_slice(), "the payload is the submitted frame");
    }

    #[test]
    fn the_frame_route_answers_before_any_frame_is_submitted() {
        let sink = WebSink::new(0).expect("binding an ephemeral port");
        let (status, _, body) = get(sink.addr(), "/frame");
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert_eq!(body.len(), FRAME_BODY_BYTES);
        assert!(body[8..].iter().all(|&b| b == 0), "a black frame before the host submits one");
    }

    #[test]
    fn the_mailbox_keeps_only_the_newest_frame() {
        let mut sink = WebSink::new(0).expect("binding an ephemeral port");
        for seed in 0..8u8 {
            sink.submit(&distinct_frame(seed)).unwrap();
        }
        let newest = distinct_frame(7);
        let (seq, held) = sink.newest().expect("a frame in the mailbox");
        assert_eq!(seq, 7, "eight submits, sequences 0..8, newest is 7");
        assert_eq!(held.as_bytes().as_slice(), newest.as_bytes().as_slice());

        // And the same is what the route hands out, twice: reading does not consume it.
        for _ in 0..2 {
            let (_, _, body) = get(sink.addr(), "/frame");
            assert_eq!(u64::from_le_bytes(body[..8].try_into().unwrap()), 7);
            assert_eq!(&body[8..], newest.as_bytes().as_slice());
        }
        assert_eq!(sink.submitted(), 8);
    }

    #[test]
    fn submit_never_blocks_on_a_client() {
        let mut sink = WebSink::new(0).expect("binding an ephemeral port");
        let frame = distinct_frame(1);
        let t0 = std::time::Instant::now();
        for _ in 0..200 {
            sink.submit(&frame).unwrap();
        }
        assert!(t0.elapsed() < Duration::from_millis(500), "submit blocked on I/O");
    }

    /// A browser tab that opens a socket, sends half a request head and never reads must
    /// not be able to hold the simulation up: nothing in `submit` touches a socket.
    #[test]
    fn a_stalled_client_never_stalls_submit() {
        let mut sink = WebSink::new(0).expect("binding an ephemeral port");
        let mut stalled = TcpStream::connect(sink.addr()).expect("connecting to the web sink");
        // A partial head: no blank line, so the handler thread stays in `read_head` until
        // its own five-second timeout. The client never reads the response either.
        write!(stalled, "GET /frame HTTP/1.1\r\nHost: localhost\r\n").unwrap();
        stalled.flush().unwrap();

        let frame = distinct_frame(5);
        let t0 = std::time::Instant::now();
        for _ in 0..200 {
            sink.submit(&frame).unwrap();
        }
        let elapsed = t0.elapsed();
        assert!(elapsed < Duration::from_millis(500), "submit waited on a client: {elapsed:?}");
        assert_eq!(sink.submitted(), 200);
        // And the sink still answers a well-behaved client while that one hangs.
        let (status, _, body) = get(sink.addr(), "/frame");
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert_eq!(&body[8..], frame.as_bytes().as_slice());
        drop(stalled);
    }

    #[test]
    fn the_status_route_reports_the_world_tick_the_sequence_and_the_source() {
        let source = Source {
            pid: 4321,
            state_dir: "/tmp/cubarium-status-test/state".to_string(),
            build_id: "0.1.0+abcdef1".to_string(),
            sink: "shim".to_string(),
            resumed_from: Some("/tmp/cubarium-status-test/state/world-100.cubw".to_string()),
            start_tick: 100,
            speed: 2.5,
        };
        let mut sink = WebSink::with_source(0, "2.5× time", source).expect("binding a port");
        sink.submit(&distinct_frame(2)).unwrap();
        sink.submit(&distinct_frame(3)).unwrap();
        sink.observe_tick(4242);

        let (status, head, body) = get(sink.addr(), "/status");
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert!(head.contains("Content-Type: application/json; charset=utf-8"), "{head}");
        assert!(head.contains("Cache-Control: no-store"), "{head}");
        let text = String::from_utf8(body).expect("the status body is UTF-8");
        let v: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("status is not JSON: {e}\n{text}"));
        assert_eq!(v["world_tick"], 4242);
        assert_eq!(v["render_seq"], 1, "two submits: the newest frame is sequence 1");
        assert_eq!(v["frames_served"], 0, "no /frame request has been answered yet");
        let s = &v["source"];
        assert_eq!(s["pid"], 4321);
        assert_eq!(s["state_dir"], "/tmp/cubarium-status-test/state");
        assert_eq!(s["build_id"], "0.1.0+abcdef1");
        assert_eq!(s["sink"], "shim");
        assert_eq!(s["resumed_from"], "/tmp/cubarium-status-test/state/world-100.cubw");
        assert_eq!(s["start_tick"], 100);
        assert_eq!(s["speed"], 2.5);

        // `render_seq` is the number the `/frame` prefix carries, and serving one frame is
        // what `frames_served` counts.
        let (_, _, frame_body) = get(sink.addr(), "/frame");
        assert_eq!(u64::from_le_bytes(frame_body[..8].try_into().unwrap()), 1);
        let (_, _, body) = get(sink.addr(), "/status");
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["frames_served"], 1);
    }

    #[test]
    fn a_plain_sink_still_answers_status_with_a_null_resume_and_this_process() {
        let sink = WebSink::new(0).expect("binding an ephemeral port");
        let (status, _, body) = get(sink.addr(), "/status");
        assert_eq!(status, "HTTP/1.1 200 OK");
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["world_tick"], 0);
        assert_eq!(v["render_seq"], 0, "before any submit the sequence reads 0, like /frame");
        assert!(v["source"]["resumed_from"].is_null());
        assert_eq!(v["source"]["pid"], serde_json::Value::from(std::process::id()));
        assert_eq!(v["source"]["build_id"], crate::state::build_id());
        assert_eq!(v["source"]["sink"], "web");
    }

    /// The page must actually ask for the status, or the HUD can never name the world.
    #[test]
    fn the_page_fetches_the_status_route() {
        assert!(INDEX_HTML.contains("fetch(\"/status\""), "the page never fetches /status");
        assert!(INDEX_HTML.contains("world_tick"), "the page never reads the world tick");
        assert!(INDEX_HTML.contains("state_dir"), "the page never names the source state dir");
    }

    #[test]
    fn the_note_route_serves_the_hosts_note_and_is_empty_by_default() {
        let plain = WebSink::new(0).expect("binding an ephemeral port");
        let (status, head, body) = get(plain.addr(), "/note");
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert!(head.contains("Content-Type: text/plain; charset=utf-8"), "{head}");
        assert!(body.is_empty(), "`new` serves no note: {body:?}");
        assert_eq!(plain.note(), "");

        let noted = WebSink::with_note(0, "8× time").expect("binding an ephemeral port");
        let (status, _, body) = get(noted.addr(), "/note");
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert_eq!(String::from_utf8(body).unwrap(), "8× time");
        assert_eq!(noted.note(), "8× time");
    }

    /// The page must actually ask for the note, or the HUD can never show it.
    #[test]
    fn the_page_fetches_the_note_route() {
        assert!(INDEX_HTML.contains("fetch(\"/note\""), "the page never fetches /note");
    }

    #[test]
    fn every_other_route_is_a_404() {
        let sink = WebSink::new(0).expect("binding an ephemeral port");
        for path in ["/nope", "/frame/extra", "/../etc/passwd", "/favicon.ico"] {
            let (status, _, _) = get(sink.addr(), path);
            assert_eq!(status, "HTTP/1.1 404 Not Found", "path {path}");
        }
    }

    #[test]
    fn finishing_stops_the_server() {
        let mut sink = WebSink::new(0).expect("binding an ephemeral port");
        let addr = sink.addr();
        sink.finish().unwrap();
        drop(sink);
        // The listener is closed with the server thread, so connecting must now fail.
        let mut failed = false;
        for _ in 0..50 {
            if TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_err() {
                failed = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(failed, "the port is still accepting after finish()");
    }

    #[test]
    fn idle_and_connected_shutdowns_do_not_need_a_wakeup_client() {
        for with_client in [false, true] {
            let mut sink = WebSink::new(0).unwrap();
            let client = with_client.then(|| TcpStream::connect(sink.addr()).unwrap());
            // Let the accept thread enter its idle readiness wait (or handle a
            // partial request); no incoming connection is needed to release it.
            std::thread::sleep(Duration::from_millis(30));
            let start = std::time::Instant::now();
            sink.finish().unwrap();
            assert!(start.elapsed() < Duration::from_secs(1));
            // Existing partial handlers are separately bounded by their deadline.
            drop(client);
            sink.finish().unwrap();
        }
    }

    #[test]
    fn a_query_string_still_reaches_the_frame_route() {
        let mut sink = WebSink::new(0).expect("binding an ephemeral port");
        sink.submit(&distinct_frame(9)).unwrap();
        let (status, _, body) = get(sink.addr(), "/frame?t=12345");
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert_eq!(body.len(), FRAME_BODY_BYTES);
    }

    // -- the care surface over a real socket --------------------------------------

    use crate::care::{CareService, JournalStatus};

    /// A sink with care attached, plus the service the runner would own.
    fn care_sink() -> (WebSink, CareService) {
        let service = CareService::new("epoch-http", Arc::new(JournalStatus::default()));
        let sink =
            WebSink::with_care(0, "", Source::default(), Some(service.shared())).expect("binding");
        (sink, service)
    }

    /// One raw request, written verbatim so a test can send exactly the head it means to.
    fn raw(addr: SocketAddr, request: &str) -> (String, String, String) {
        let mut s = TcpStream::connect(addr).expect("connecting");
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        s.write_all(request.as_bytes()).unwrap();
        s.flush().unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).expect("reading the response");
        let end = find_head_end(&buf).expect("a complete response head");
        let head = String::from_utf8_lossy(&buf[..end]).into_owned();
        let start = if buf[end..].starts_with(b"\r\n\r\n") { end + 4 } else { end + 2 };
        let status = head.lines().next().unwrap_or_default().to_string();
        (status, head, String::from_utf8_lossy(&buf[start..]).into_owned())
    }

    /// A well-formed care POST: every header the contract requires.
    fn care_post(addr: SocketAddr, path: &str, body: &str) -> (String, String, String) {
        raw(
            addr,
            &format!(
                "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:{}\r\n\
                 Content-Type: application/json\r\nX-Cubarium-Care: 1\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                addr.port(),
                addr.port(),
                body.len(),
            ),
        )
    }

    #[test]
    fn a_host_without_care_reports_it_disabled_and_refuses_the_post_routes() {
        let sink = WebSink::new(0).expect("binding");
        let (status, _, body) = get(sink.addr(), "/care/status");
        assert_eq!(status, "HTTP/1.1 200 OK");
        let v: serde_json::Value = serde_json::from_str(&body_text(&body)).unwrap();
        assert_eq!(v["enabled"], false, "the page must be able to tell care is off");
        assert_eq!(v["care"], "disabled");

        let (status, _, body) = care_post(sink.addr(), "/care/register", "{}");
        assert_eq!(status, "HTTP/1.1 503 Service Unavailable");
        assert!(body.contains("care disabled"), "{body}");
    }

    fn body_text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    #[test]
    fn registering_then_posting_reaches_the_service_and_nothing_touches_the_world() {
        let (sink, service) = care_sink();
        let addr = sink.addr();

        let (status, head, body) = care_post(addr, "/care/register", "{}");
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert!(head.contains("Content-Type: application/json"), "{head}");
        // The refusal that matters most is the one that is *absent*: no CORS header, ever.
        assert!(
            !head.to_ascii_lowercase().contains("access-control-"),
            "a CORS header would let any page on the internet water this world: {head}"
        );
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let client = v["client"].as_str().expect("an issued identity").to_string();
        assert!(client.starts_with("epoch-http."), "the id embeds the epoch: {client}");
        assert_eq!(v["epoch"], "epoch-http");

        // A submit, answered as soon as the "runner" commits it.
        let posted = std::thread::spawn(move || {
            care_post(
                addr,
                "/care",
                &format!(
                    r#"{{"client":"{client}","request":1,"kind":"feed","target":{{"face":2,"u":31,"v":7}}}}"#
                ),
            )
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let planned = loop {
            let planned = service.drain_prepared(1, 60);
            if !planned.is_empty() {
                break planned;
            }
            assert!(std::time::Instant::now() < deadline, "the request never reached the FIFO");
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(planned[0].kind.as_str(), "feed");
        assert_eq!(planned[0].target.face, 2);
        assert_eq!((planned[0].target.u, planned[0].target.v), (31, 7));
        service.commit_accepted(&planned);

        let (status, _, body) = posted.join().unwrap();
        assert_eq!(status, "HTTP/1.1 202 Accepted");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["seq"], 1);
        assert_eq!(v["apply_after_tick"], 60);
        drop(sink);
    }

    #[test]
    fn a_cross_origin_page_is_refused_and_so_is_a_missing_custom_header() {
        let (sink, _service) = care_sink();
        let addr = sink.addr();
        let port = addr.port();

        // The header a cross-origin `fetch` cannot set without a preflight.
        let (status, _, body) = raw(
            addr,
            &format!(
                "POST /care/register HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Content-Type: application/json\r\nContent-Length: 2\r\n\
                 Connection: close\r\n\r\n{{}}"
            ),
        );
        assert_eq!(status, "HTTP/1.1 403 Forbidden");
        assert!(body.contains("X-Cubarium-Care"), "{body}");

        // An Origin that is not this viewer.
        for origin in
            ["http://evil.example", "http://localhost:1", "null", "https://127.0.0.1"]
        {
            let (status, _, body) = raw(
                addr,
                &format!(
                    "POST /care/register HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                     Origin: {origin}\r\nContent-Type: application/json\r\n\
                     X-Cubarium-Care: 1\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
                ),
            );
            assert_eq!(status, "HTTP/1.1 403 Forbidden", "origin {origin}");
            assert!(body.contains("cross-origin"), "origin {origin}: {body}");
        }

        // A Host naming some other server (a DNS-rebinding page resolving to 127.0.0.1).
        let (status, _, body) = raw(
            addr,
            &format!(
                "POST /care/register HTTP/1.1\r\nHost: rebind.example:{port}\r\n\
                 Content-Type: application/json\r\nX-Cubarium-Care: 1\r\n\
                 Content-Length: 2\r\nConnection: close\r\n\r\n{{}}"
            ),
        );
        assert_eq!(status, "HTTP/1.1 403 Forbidden");
        assert!(body.contains("not rebind.example"), "{body}");

        // A preflight is never answered, so the browser never sends the real request.
        let (status, head, _) = raw(
            addr,
            &format!(
                "OPTIONS /care HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Origin: http://evil.example\r\n\
                 Access-Control-Request-Method: POST\r\nConnection: close\r\n\r\n"
            ),
        );
        assert_eq!(status, "HTTP/1.1 404 Not Found");
        assert!(!head.to_ascii_lowercase().contains("access-control-"), "{head}");

        // And the wrong content type.
        let (status, _, _) = raw(
            addr,
            &format!(
                "POST /care/register HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Content-Type: text/plain\r\nX-Cubarium-Care: 1\r\n\
                 Content-Length: 2\r\nConnection: close\r\n\r\n{{}}"
            ),
        );
        assert_eq!(status, "HTTP/1.1 415 Unsupported Media Type");
    }

    #[test]
    fn a_mutation_through_get_is_405_and_never_registers_anything() {
        let (sink, _service) = care_sink();
        for path in ["/care", "/care/register"] {
            let (status, head, _) = get(sink.addr(), path);
            assert_eq!(status, "HTTP/1.1 405 Method Not Allowed", "{path}");
            assert!(head.contains("Allow: POST"), "{head}");
        }
        // Nothing was issued by those requests.
        let (_, _, body) = get(sink.addr(), "/care/status");
        let v: serde_json::Value = serde_json::from_str(&body_text(&body)).unwrap();
        assert!(v["receipts"].as_array().unwrap().is_empty(), "{v}");
    }

    #[test]
    fn an_oversize_body_is_413_and_a_malformed_one_is_400() {
        let (sink, _service) = care_sink();
        let addr = sink.addr();
        let port = addr.port();

        // Declared past the 4 KiB bound: refused without reading the body at all.
        let (status, _, body) = raw(
            addr,
            &format!(
                "POST /care HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Content-Type: application/json\r\nX-Cubarium-Care: 1\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n",
                care::MAX_BODY_BYTES + 1
            ),
        );
        assert_eq!(status, "HTTP/1.1 413 Payload Too Large");
        assert!(body.contains("4096"), "{body}");

        // No Content-Length at all.
        let (status, _, _) = raw(
            addr,
            &format!(
                "POST /care HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Content-Type: application/json\r\nX-Cubarium-Care: 1\r\n\
                 Connection: close\r\n\r\n"
            ),
        );
        assert_eq!(status, "HTTP/1.1 400 Bad Request");

        // Well-formed transport, unusable payloads.
        for body in [
            "not json",
            r#"{"request":1,"kind":"feed","target":{"face":0,"u":0,"v":0}}"#,
            r#"{"client":"x","request":1,"kind":"polish","target":{"face":0,"u":0,"v":0}}"#,
            r#"{"client":"x","request":1,"kind":"feed","target":{"face":9,"u":0,"v":0}}"#,
            r#"{"client":"x","request":1,"kind":"feed","target":{"face":0,"u":64,"v":0}}"#,
        ] {
            let (status, _, answer) = care_post(addr, "/care", body);
            assert_eq!(status, "HTTP/1.1 400 Bad Request", "body {body} answered {answer}");
        }
    }

    #[test]
    fn an_unknown_identity_is_409_retired_and_a_full_journal_is_503() {
        let status = Arc::new(JournalStatus::default());
        let service = CareService::new("epoch-http", Arc::clone(&status));
        let sink = WebSink::with_care(0, "", Source::default(), Some(service.shared())).unwrap();

        let (code, _, body) = care_post(
            sink.addr(),
            "/care",
            r#"{"client":"epoch-http.99","request":1,"kind":"feed","target":{"face":0,"u":1,"v":1}}"#,
        );
        assert_eq!(code, "HTTP/1.1 409 Conflict");
        assert!(body.contains("retired"), "{body}");

        // Register properly, then fill the journal.
        let (_, _, body) = care_post(sink.addr(), "/care/register", "{}");
        let client = serde_json::from_str::<serde_json::Value>(&body).unwrap()["client"]
            .as_str()
            .unwrap()
            .to_string();
        status.set_for_test(crate::care::JOURNAL_LIMIT, 0);
        let (code, _, body) = care_post(
            sink.addr(),
            "/care",
            &format!(
                r#"{{"client":"{client}","request":1,"kind":"feed","target":{{"face":0,"u":1,"v":1}}}}"#
            ),
        );
        assert_eq!(code, "HTTP/1.1 503 Service Unavailable");
        assert!(body.contains("journal full"), "{body}");
    }

    #[test]
    fn care_is_503_replaying_until_the_recovered_schedule_is_exhausted() {
        let (sink, service) = care_sink();
        service.gate_until_replayed();
        let (_, _, body) = care_post(sink.addr(), "/care/register", "{}");
        let client = serde_json::from_str::<serde_json::Value>(&body).unwrap()["client"]
            .as_str()
            .unwrap()
            .to_string();
        let payload = format!(
            r#"{{"client":"{client}","request":1,"kind":"feed","target":{{"face":0,"u":1,"v":1}}}}"#
        );
        let (code, _, body) = care_post(sink.addr(), "/care", &payload);
        assert_eq!(code, "HTTP/1.1 503 Service Unavailable");
        // A state, not an error: the page retries rather than reporting a failure.
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap()["care"],
            "replaying",
            "{body}"
        );

        service.open_intake();
        let addr = sink.addr();
        let posted = std::thread::spawn(move || care_post(addr, "/care", &payload));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let planned = service.drain_prepared(1, 20);
            if !planned.is_empty() {
                service.commit_accepted(&planned);
                break;
            }
            assert!(std::time::Instant::now() < deadline, "intake never opened");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(posted.join().unwrap().0, "HTTP/1.1 202 Accepted");
        drop(sink);
    }

    /// Astra's bound: partial connections beyond the cap must not raise the number of live
    /// handlers, and permits must come back when the request deadline expires.
    #[test]
    fn partial_connections_cannot_push_live_handlers_past_the_cap() {
        let sink = WebSink::new(0).expect("binding");
        let addr = sink.addr();
        // Twice the cap, each sending a head that never ends.
        let mut held = Vec::new();
        for _ in 0..(MAX_HANDLERS * 2) {
            match TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
                Ok(mut s) => {
                    let _ = write!(s, "GET /frame HTTP/1.1\r\nHost: localhost\r\n");
                    let _ = s.flush();
                    held.push(s);
                }
                Err(_) => break,
            }
        }
        // Let the accept loop work through them.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while sink.refused_connections() == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            sink.handlers() <= MAX_HANDLERS as u64,
            "live handlers {} above the cap {MAX_HANDLERS}",
            sink.handlers()
        );
        assert!(sink.refused_connections() > 0, "the excess must be closed, not queued");

        // The permits come back once the absolute deadline expires, even though every one
        // of those peers is still connected and would renew a per-read timeout forever.
        let deadline = std::time::Instant::now() + REQUEST_DEADLINE + Duration::from_secs(5);
        while sink.handlers() > 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(sink.handlers(), 0, "a permit was not released on some exit path");
        // And the sink still answers.
        let (status, _, _) = get(addr, "/status");
        assert_eq!(status, "HTTP/1.1 200 OK");
        drop(held);
    }

    #[test]
    fn a_head_that_crosses_the_cap_in_one_read_is_still_refused() {
        let sink = WebSink::new(0).expect("binding");
        let mut s = TcpStream::connect(sink.addr()).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        // One write: an oversized head and its terminating blank line together, so the
        // delimiter is found in the same read that crosses `MAX_REQUEST_BYTES`.
        let padding = "x".repeat(MAX_REQUEST_BYTES * 2);
        let _ = write!(s, "GET /status HTTP/1.1\r\nHost: localhost\r\nX-Pad: {padding}\r\n\r\n");
        let _ = s.flush();
        let mut buf = Vec::new();
        let _ = s.read_to_end(&mut buf);
        assert!(buf.is_empty(), "an oversized head must be dropped, not answered: {} bytes", buf.len());
    }

    // -- the page's face table cannot drift from the Rust one --------------------

    /// Pull `key: [a, b, c]` out of one `FACE_FRAMES` row.
    fn parse_vec(row: &str, key: &str) -> [f64; 3] {
        let at = row
            .find(&format!("{key}: ["))
            .unwrap_or_else(|| panic!("row is missing `{key}`: {row}"));
        let rest = &row[at + key.len() + 3..];
        let end = rest.find(']').unwrap_or_else(|| panic!("unterminated `{key}`: {row}"));
        let nums: Vec<f64> = rest[..end]
            .split(',')
            .map(|n| n.trim().parse::<f64>().unwrap_or_else(|_| panic!("bad number in {row}")))
            .collect();
        assert_eq!(nums.len(), 3, "`{key}` must have three components: {row}");
        [nums[0], nums[1], nums[2]]
    }

    /// The five `FACE_FRAMES` rows of the embedded page, in order.
    fn html_face_rows() -> Vec<String> {
        let start = INDEX_HTML
            .find("const FACE_FRAMES = [")
            .expect("the page must declare `const FACE_FRAMES = [`");
        let rest = &INDEX_HTML[start..];
        let end = rest.find("\n];").expect("the FACE_FRAMES literal must end with a line `];`");
        let rows: Vec<String> = rest[..end]
            .lines()
            .filter(|l| l.contains("face:"))
            .map(|l| l.trim().to_string())
            .collect();
        assert_eq!(rows.len(), 5, "the page's table must have exactly five rows");
        rows
    }

    #[test]
    fn the_pages_face_table_matches_cubarium_surface_face_frame() {
        let rows = html_face_rows();
        for (i, face) in Face::ALL.iter().enumerate() {
            let row = &rows[i];
            let f = face_frame(*face);
            assert!(
                row.contains(&format!("face: {i},")),
                "row {i} must be frame index {i} ({face:?}): {row}"
            );
            assert!(
                row.contains(&format!("name: \"{face:?}\"")),
                "row {i} must be named {face:?}: {row}"
            );
            assert_eq!(parse_vec(row, "center"), f.center, "center of {face:?}");
            assert_eq!(parse_vec(row, "u"), f.tangent_u, "tangent_u of {face:?}");
            assert_eq!(parse_vec(row, "v"), f.tangent_v, "tangent_v of {face:?}");
            assert_eq!(parse_vec(row, "n"), f.normal, "normal of {face:?}");
        }
    }

    /// The page's UV construction, replayed in Rust: for a cube vertex `p` on face `f`,
    /// the chart coordinates are `((p . u) + 1) / 2` and `((p . v) + 1) / 2`. Checking it
    /// against the embedding proves the page draws the documented corners at the
    /// documented cube corners.
    #[test]
    fn the_uv_rule_in_the_page_inverts_the_documented_embedding() {
        let corners: [(f64, f64); 3] = [(0.0, 0.0), (63.0, 0.0), (0.0, 63.0)];
        for face in Face::ALL {
            let f = face_frame(face);
            for (u, v) in corners {
                // Where the embedding puts this chart pixel's center on the cube.
                let (a, b) = (u / 32.0 - 1.0, v / 32.0 - 1.0);
                let p = [
                    f.center[0] + a * f.tangent_u[0] + b * f.tangent_v[0],
                    f.center[1] + a * f.tangent_u[1] + b * f.tangent_v[1],
                    f.center[2] + a * f.tangent_u[2] + b * f.tangent_v[2],
                ];
                // The page's rule, run backwards from that point.
                let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
                let su = (dot(p, f.tangent_u) + 1.0) / 2.0 * 64.0;
                let sv = (dot(p, f.tangent_v) + 1.0) / 2.0 * 64.0;
                assert!(
                    (su - u).abs() < 1e-9 && (sv - v).abs() < 1e-9,
                    "{face:?} chart ({u}, {v}) round-tripped to ({su}, {sv})"
                );
                // And the vertex the marker touches is a genuine cube corner.
                assert!(p.iter().all(|c| c.abs() > 0.9), "{face:?} ({u}, {v}) is not near a corner");
            }
        }
    }

    /// The page's documented corner table, checked against the embedding. Each entry is
    /// the cube corner a chart corner's marker must land on.
    #[test]
    fn the_documented_self_test_corners_are_the_ones_the_embedding_gives() {
        // (face, chart u, chart v, cube corner), straight out of the comment in the page.
        let expected: [(Face, f64, f64, [f64; 3]); 15] = [
            (Face::Front, 0.0, 0.0, [-1.0, 1.0, 1.0]),
            (Face::Front, 63.0, 0.0, [1.0, 1.0, 1.0]),
            (Face::Front, 0.0, 63.0, [-1.0, -1.0, 1.0]),
            (Face::Right, 0.0, 0.0, [1.0, 1.0, 1.0]),
            (Face::Right, 63.0, 0.0, [1.0, 1.0, -1.0]),
            (Face::Right, 0.0, 63.0, [1.0, -1.0, 1.0]),
            (Face::Back, 0.0, 0.0, [1.0, 1.0, -1.0]),
            (Face::Back, 63.0, 0.0, [-1.0, 1.0, -1.0]),
            (Face::Back, 0.0, 63.0, [1.0, -1.0, -1.0]),
            (Face::Left, 0.0, 0.0, [-1.0, 1.0, -1.0]),
            (Face::Left, 63.0, 0.0, [-1.0, 1.0, 1.0]),
            (Face::Left, 0.0, 63.0, [-1.0, -1.0, -1.0]),
            (Face::Top, 0.0, 0.0, [-1.0, 1.0, -1.0]),
            (Face::Top, 63.0, 0.0, [1.0, 1.0, -1.0]),
            (Face::Top, 0.0, 63.0, [-1.0, 1.0, 1.0]),
        ];
        for (face, u, v, corner) in expected {
            let f = face_frame(face);
            let (a, b) = (u / 32.0 - 1.0, v / 32.0 - 1.0);
            for axis in 0..3 {
                let p = f.center[axis] + a * f.tangent_u[axis] + b * f.tangent_v[axis];
                assert_eq!(
                    p.signum(),
                    corner[axis],
                    "{face:?} chart ({u}, {v}) axis {axis}: got {p}, page documents {corner:?}"
                );
                assert!(p.abs() > 0.9, "{face:?} chart ({u}, {v}) axis {axis} is not at the rim");
            }
        }
    }

    /// The page's net layout must be the same grid the PNG sink and preview use.
    #[test]
    fn the_pages_net_cells_match_the_hosts_net_layout() {
        let at = INDEX_HTML.find("const NET_CELL = [").expect("the page declares NET_CELL");
        let rest = &INDEX_HTML[at..];
        let line = rest.lines().next().unwrap();
        for face in Face::ALL {
            let (col, row) = crate::net::net_cell(face);
            let want = format!("[{col}, {row}]");
            assert!(line.contains(&want), "{face:?} cell {want} missing from: {line}");
        }
    }

    /// The page's click-to-target mapping must be built from the same two constants the
    /// drawing code uses, or a click would land on a different cell than the one under the
    /// cursor — and the world would be fed somewhere nobody chose.
    #[test]
    fn the_pages_click_mapping_is_built_from_net_cell_and_face_size() {
        let start = INDEX_HTML.find("function netTarget(").expect("the page maps clicks to cells");
        let end = INDEX_HTML[start..].find("\n}").expect("netTarget must end") + start;
        let body = &INDEX_HTML[start..end];
        assert!(body.contains("NET_CELL[f][0]") && body.contains("NET_CELL[f][1]"),
            "the mapping must use the same NET_CELL table the net is drawn from: {body}");
        assert!(body.contains("FACE_SIZE * NET_SCALE"), "{body}");
        assert!(body.contains("col * FACE_SIZE") && body.contains("row * FACE_SIZE"), "{body}");
        // Chart coordinates stay inside the face, so a click on a seam cannot address a
        // pixel of some other face.
        assert!(body.contains("u >= FACE_SIZE") && body.contains("v >= FACE_SIZE"), "{body}");
        // The marker is drawn on the overlay, never into the frame bytes.
        let mark = INDEX_HTML.find("function drawMark(").expect("the page draws a marker");
        let mark_end = INDEX_HTML[mark..].find("\n}").expect("drawMark must end") + mark;
        assert!(
            !INDEX_HTML[mark..mark_end].contains("frameBytes"),
            "the target marker must never be written into the frame"
        );
        assert!(INDEX_HTML.contains(r#"markCtx.clearRect"#), "the overlay is redrawn, not stacked");
    }

    /// The care panel: closed by default, registers on load, and posts only with the
    /// header a cross-origin page cannot send.
    #[test]
    fn the_page_declares_a_closed_care_panel_that_registers_and_posts() {
        assert!(INDEX_HTML.contains(r#"<details class="panel" id="carePanel">"#));
        assert!(
            !INDEX_HTML.contains(r#"id="carePanel" open"#)
                && !INDEX_HTML.contains(r#"<details open class="panel" id="carePanel">"#),
            "the care panel must be closed by default"
        );
        assert!(INDEX_HTML.contains("<summary>Care (optional)</summary>"));
        for label in ["Scatter food", "Shower", "Clean up litter"] {
            assert!(INDEX_HTML.contains(label), "the panel is missing the button {label:?}");
        }
        assert!(INDEX_HTML.contains(r#""X-Cubarium-Care": "1""#), "the custom header is the lock");
        assert!(INDEX_HTML.contains(r#"fetch("/care/register""#), "the page registers on load");
        assert!(INDEX_HTML.contains(r#"fetch("/care", { method: "POST""#));
        assert!(INDEX_HTML.contains(r#"fetch("/care/status""#));
        // Every state the contract asks a request row to show.
        for state in ["queued", "accepted", "applied", "partial", "rejected"] {
            assert!(INDEX_HTML.contains(&format!(".st-{state}")), "no style for {state}");
        }
        assert!(INDEX_HTML.contains("holding at tick"), "the hold must be visible");
        assert!(INDEX_HTML.contains("care failed at tick"), "the failure must be visible");
        assert!(
            INDEX_HTML.contains("earlier requests are unknown, check the receipts"),
            "a restart must be reported honestly rather than guessed at"
        );
    }

    /// Two lifetime bugs the browser rollout review caught: registration that fails once
    /// (the world was replaying, or the host was not listening yet) must be retried rather
    /// than leaving the controls dead until a reload, and the request-row map must be
    /// bounded alongside the DOM rather than retaining every detached element.
    #[test]
    fn the_page_retries_registration_and_bounds_its_row_map() {
        assert!(
            INDEX_HTML.contains("if (careRegistering || careClient !== null) return;"),
            "registration must guard against racing itself"
        );
        assert!(
            INDEX_HTML.contains("careRetryAt = performance.now() + CARE_RETRY_MS;"),
            "a failed registration must become retryable at a bounded cadence"
        );
        assert!(
            INDEX_HTML
                .contains("if (careClient === null && s.care !== \"failed\" && performance.now() >= careRetryAt)"),
            "the status poll must retry registration once the host is reachable"
        );
        // The row map is trimmed with the DOM, keyed by the request number on the element.
        assert!(INDEX_HTML.contains("careRows.delete(Number(gone.dataset.request));"), "unbounded row map");
        assert!(INDEX_HTML.contains("careRowsEl.children.length > CARE_ROWS_MAX"));
    }

    /// Guard the page's other load-bearing constants and its one external dependency.
    #[test]
    fn the_page_declares_the_expected_sizes_and_one_external_script() {
        assert!(INDEX_HTML.contains("const FRAME_BYTES = NUM_FACES * FACE_BYTES;"));
        assert!(INDEX_HTML.contains("const FACE_SIZE = 64;"));
        assert!(INDEX_HTML.contains("8 + FRAME_BYTES"), "the page checks the /frame body length");
        assert_eq!(FACE_BYTES * 5, FRAME_BYTES);
        let scripts: Vec<&str> = INDEX_HTML.match_indices("<script src=").map(|(_, s)| s).collect();
        assert_eq!(scripts.len(), 1, "exactly one external script");
        assert!(
            INDEX_HTML.contains(
                "https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"
            ),
            "three.js r128 from cdnjs"
        );
        assert!(INDEX_HTML.contains("#0B0525"), "the dark background of the room's palette");
    }
}
