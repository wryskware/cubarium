//! Web sink: a minimal HTTP/1.1 server on the loopback that serves a viewer page at `/`
//! and the newest encoded frame at `/frame`. It exists so the world can be watched and
//! screenshotted without the cube.
//!
//! Same newest-frame mailbox as the shim sink: `submit` replaces the frame in a
//! `Mutex<Option<..>>` and returns; it never touches a socket, so the simulation loop
//! never waits on a browser. A viewer that polls slower than the host renders simply
//! sees fewer, newer frames; a viewer that polls faster re-reads the same tick.
//!
//! `std::net` only — no HTTP crate. The surface is three routes and `Connection: close`
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

/// The viewer page, embedded so a running host has no runtime asset dependency.
pub const INDEX_HTML: &str = include_str!("web/index.html");

/// Length of a `/frame` body: an 8-byte little-endian tick then the encoded frame.
pub const FRAME_BODY_BYTES: usize = 8 + FRAME_BYTES;

/// How long the accept loop sleeps between polls of a non-blocking listener. Bounds how
/// long `finish` waits for the server thread to notice the stop flag.
const ACCEPT_POLL: Duration = Duration::from_millis(10);
/// Read and write timeout on an accepted connection, so one stalled client cannot pin a
/// handler thread forever.
const CONN_TIMEOUT: Duration = Duration::from_secs(5);
/// Longest request head accepted. A `GET` from the viewer page is a few hundred bytes.
const MAX_REQUEST_BYTES: usize = 8 * 1024;

/// The newest frame and the tick it was submitted under.
type Slot = Option<Arc<(u64, Frame)>>;

struct Shared {
    /// Newest-frame mailbox. Unlike the shim's, the server does not *take* the frame: a
    /// viewer polling at its own rate must always find the latest one here.
    slot: Mutex<Slot>,
    stop: AtomicBool,
    /// Frames submitted since start; also the tick of the next one.
    ticks: AtomicU64,
    /// Requests answered on `/frame`.
    served: AtomicU64,
}

impl Shared {
    fn newest(&self) -> Slot {
        self.slot.lock().expect("web mailbox poisoned").clone()
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

    /// Frames submitted since start. The newest frame's tick is this minus one.
    pub fn submitted(&self) -> u64 {
        self.shared.ticks.load(Ordering::Relaxed)
    }

    /// `/frame` requests answered with a frame.
    pub fn served(&self) -> u64 {
        self.shared.served.load(Ordering::Relaxed)
    }

    /// The newest frame in the mailbox and its tick, if one has been submitted.
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
        let tick = self.shared.ticks.fetch_add(1, Ordering::Relaxed);
        let next = Arc::new((tick, frame.clone()));
        let mut slot = self.shared.slot.lock().expect("web mailbox poisoned");
        // The newest frame always wins; nothing here waits on a client.
        *slot = Some(next);
        Ok(())
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

/// Accept connections until the stop flag is set, handing each to a short-lived thread so
/// one slow client cannot delay the next.
fn accept_loop(shared: &Arc<Shared>, listener: &TcpListener) {
    while !shared.stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let shared = Arc::clone(shared);
                let spawned = std::thread::Builder::new()
                    .name("cubarium-web-conn".into())
                    .spawn(move || handle(&shared, stream));
                if spawned.is_err() {
                    // Out of threads: drop the connection rather than stall the loop.
                    std::thread::sleep(ACCEPT_POLL);
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(ACCEPT_POLL),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => std::thread::sleep(ACCEPT_POLL),
        }
    }
}

/// Answer exactly one request, then close.
fn handle(shared: &Shared, mut stream: TcpStream) {
    // The listener is non-blocking; accepted sockets must not inherit that.
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(CONN_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CONN_TIMEOUT));
    let _ = stream.set_nodelay(true);

    let head = match read_head(&mut stream) {
        Some(h) => h,
        None => return,
    };
    let route = request_path(&head);

    let _ = match route {
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
        _ => respond(&mut stream, "404 Not Found", "text/plain; charset=utf-8", "", b"not found\n"),
    };
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/// The `/frame` body: 8-byte little-endian tick then the frame bytes. With no frame yet,
/// tick 0 and a black frame, so the page has something valid to draw immediately.
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

/// Read the request head (everything before the blank line). `None` on a timeout, a
/// closed connection, or a head that runs past [`MAX_REQUEST_BYTES`].
fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::with_capacity(512);
    let mut chunk = [0u8; 512];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if let Some(end) = find_head_end(&buf) {
                    return String::from_utf8(buf[..end].to_vec()).ok();
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
        assert_eq!(body.len(), FRAME_BODY_BYTES, "8-byte tick plus {FRAME_BYTES} frame bytes");
        assert_eq!(u64::from_le_bytes(body[..8].try_into().unwrap()), 0, "the first frame is tick 0");
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
        let (tick, held) = sink.newest().expect("a frame in the mailbox");
        assert_eq!(tick, 7, "eight submits, ticks 0..8, newest is 7");
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
    fn a_query_string_still_reaches_the_frame_route() {
        let mut sink = WebSink::new(0).expect("binding an ephemeral port");
        sink.submit(&distinct_frame(9)).unwrap();
        let (status, _, body) = get(sink.addr(), "/frame?t=12345");
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert_eq!(body.len(), FRAME_BODY_BYTES);
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
