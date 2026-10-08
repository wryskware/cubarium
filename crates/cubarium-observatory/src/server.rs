//! **The HTTP + server-sent-events transport** and the embedded dashboard.
//!
//! `std::net` only, like the web sink (`crates/cubarium/src/sink/web`): the surface is a
//! dozen read routes, one push stream and one command route, and every request but the
//! stream is `Connection: close`. No async runtime: a reader costs one short-lived thread
//! per request (capped) and one parked thread per open stream (capped), which is nothing
//! for a dashboard or two on a home LAN, and a runtime would be the biggest dependency in
//! the binary for it.
//!
//! **Server-sent events, not WebSocket**: the push is one-way (the world to the page), and
//! the commands are rare and fit a `POST`. SSE is plain HTTP — no handshake hashing, no
//! framing — and the browser's `EventSource` reconnects by itself.
//!
//! Threads: the **ingest** thread receives [`Publication`]s from the loop over a channel
//! (the loop's send never blocks), files the life events and the history sample, and
//! serializes the snapshot once for every reader. The **accept** thread hands each
//! connection to a handler thread. Nothing here holds or sees the world.
//!
//! Routes (all JSON unless said):
//! - `GET /` the dashboard (HTML)
//! - `GET /snapshot` summary, clock, run, populations and environment, and the history
//!   sample taken with it (`null` when none was)
//! - `GET /summary`, `/clock`, `/run`, `/populations`, `/environment` one part each
//! - `GET /history[?since=TICK]` the history ring's samples, oldest first
//! - `GET /animals` every living body's row, with its parent and offspring count, and the
//!   most recent deaths
//! - `GET /animals/{id}` one body's detail and life history; a dead body's history while
//!   the book keeps it
//! - `GET /stream` server-sent events: an `snapshot` event (the `/snapshot` body) per
//!   publication
//! - `POST /control` `{"speed": 4}`, `{"paused": true}`, `{"step": "faster"|"slower"}`

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::history::{History, HistoryConfig, Sample};
use crate::life::{LifeBook, LifeRecord};
use crate::observer::{Publication, Snapshot};
use crate::query::AnimalRow;
use crate::Command;

/// The dashboard page, embedded so a running host has no asset to find.
pub const DASHBOARD_HTML: &str = include_str!("dashboard.html");

/// Handler threads alive at once (streams included); more connections are closed at once.
const MAX_HANDLERS: usize = 32;
/// Open event streams at once.
const MAX_STREAMS: usize = 16;
/// Longest request head, and longest command body.
const MAX_HEAD: usize = 8 * 1024;
const MAX_BODY: usize = 4 * 1024;
/// A whole request must arrive within this.
const REQUEST_DEADLINE: Duration = Duration::from_secs(5);
/// A stream writes a comment line this often when nothing is published (a paused host
/// still publishes; a stopped one does not), so a dead peer is found.
const KEEPALIVE: Duration = Duration::from_secs(15);
/// Deaths listed under `/animals`.
const RECENT_DEATHS: usize = 50;

/// What every reader reads, behind one lock that only the ingest thread writes.
struct State {
    seq: u64,
    snapshot: Option<Arc<Snapshot>>,
    /// The `/snapshot` body, serialized once per publication.
    live: Arc<String>,
    history: History,
    book: LifeBook,
    stopping: bool,
}

struct Hub {
    state: Mutex<State>,
    changed: Condvar,
    commands: Mutex<Sender<Command>>,
    handlers: AtomicUsize,
    streams: AtomicUsize,
    stop: AtomicBool,
}

/// The `/snapshot` body: the snapshot without its animal list, and the sample.
#[derive(Serialize)]
struct Live<'a> {
    seq: u64,
    summary: &'a crate::query::WorldSummary,
    clock: &'a crate::query::WorldClock,
    run: &'a crate::observer::RunState,
    populations: &'a crate::query::Populations,
    environment: &'a crate::query::Environment,
    animals_alive: usize,
    history: HistoryInfo,
    sample: Option<&'a Sample>,
}

#[derive(Serialize, Clone, Copy)]
struct HistoryInfo {
    sample_s: f64,
    span_s: f64,
    samples: usize,
}

/// A running server. Dropping it stops the accept loop; the threads serving open streams
/// end at their next write.
pub struct Server {
    hub: Arc<Hub>,
    tx: Option<Sender<Publication>>,
    addr: SocketAddr,
    accept: Option<JoinHandle<()>>,
    ingest: Option<JoinHandle<()>>,
}

impl Server {
    /// Bind `addr` (`127.0.0.1:7878`, `0.0.0.0:7878`; port 0 picks one) and start serving.
    /// Commands from readers are sent on `commands`.
    pub fn start(addr: &str, history: HistoryConfig, commands: Sender<Command>) -> std::io::Result<Server> {
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidInput, format!("{addr:?} names no address")))?;
        let listener = TcpListener::bind(addr)?;
        let addr = listener.local_addr()?;
        let hub = Arc::new(Hub {
            state: Mutex::new(State {
                seq: 0,
                snapshot: None,
                live: Arc::new("null".to_string()),
                history: History::new(history),
                book: LifeBook::default(),
                stopping: false,
            }),
            changed: Condvar::new(),
            commands: Mutex::new(commands),
            handlers: AtomicUsize::new(0),
            streams: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
        });
        let (tx, rx) = mpsc::channel::<Publication>();
        let ingest = {
            let hub = hub.clone();
            std::thread::Builder::new()
                .name("observatory-ingest".into())
                .spawn(move || ingest_loop(&hub, rx))?
        };
        let accept = {
            let hub = hub.clone();
            std::thread::Builder::new()
                .name("observatory-accept".into())
                .spawn(move || accept_loop(&hub, &listener))?
        };
        Ok(Server { hub, tx: Some(tx), addr, accept: Some(accept), ingest: Some(ingest) })
    }

    /// The address actually bound.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The dashboard's URL as a browser on this machine would open it.
    pub fn url(&self) -> String {
        let host = if self.addr.ip().is_unspecified() { Ipv4Addr::LOCALHOST.into() } else { self.addr.ip() };
        format!("http://{}/", SocketAddr::new(host, self.addr.port()))
    }

    /// Hand a publication over. Never blocks: the ingest thread takes it from here.
    pub fn publish(&self, publication: Publication) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(publication);
        }
    }

    /// The publications filed so far (for tests and probes).
    pub fn published(&self) -> u64 {
        self.hub.state.lock().map_or(0, |s| s.seq)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(h) = self.ingest.take() {
            let _ = h.join();
        }
        self.hub.stop.store(true, Ordering::SeqCst);
        // Wake the blocking accept with a connection of our own.
        let wake = SocketAddr::new(
            if self.addr.ip().is_unspecified() { Ipv4Addr::LOCALHOST.into() } else { self.addr.ip() },
            self.addr.port(),
        );
        let _ = TcpStream::connect_timeout(&wake, Duration::from_millis(200));
        if let Some(h) = self.accept.take() {
            let _ = h.join();
        }
    }
}

fn ingest_loop(hub: &Hub, rx: Receiver<Publication>) {
    while let Ok(mut p) = rx.recv() {
        // A backlog (a reader-starved machine) is filed in order, and only the newest
        // snapshot is kept.
        let mut events = std::mem::take(&mut p.events);
        let mut samples = Vec::new();
        let mut pubs = vec![p];
        while let Ok(more) = rx.try_recv() {
            pubs.push(more);
        }
        for q in &mut pubs {
            events.append(&mut q.events);
            let s = &q.snapshot;
            samples.push(Sample::of(&s.clock, &s.populations, &s.environment));
        }
        let snapshot = Arc::new(pubs.pop().expect("one at least").snapshot);
        let Ok(mut st) = hub.state.lock() else { return };
        st.book.apply(&events);
        let mut taken = None;
        for sample in samples {
            if st.history.offer(sample.clone()) {
                taken = Some(sample);
            }
        }
        let info = HistoryInfo {
            sample_s: st.history.config().sample_s,
            span_s: st.history.config().span_s,
            samples: st.history.len(),
        };
        let live = Live {
            seq: snapshot.seq,
            summary: &snapshot.summary,
            clock: &snapshot.clock,
            run: &snapshot.run,
            populations: &snapshot.populations,
            environment: &snapshot.environment,
            animals_alive: snapshot.animals.len(),
            history: info,
            sample: taken.as_ref(),
        };
        let json = serde_json::to_string(&live).unwrap_or_else(|_| "null".into());
        st.seq = snapshot.seq;
        st.live = Arc::new(json);
        st.snapshot = Some(snapshot);
        drop(st);
        hub.changed.notify_all();
    }
    if let Ok(mut st) = hub.state.lock() {
        st.stopping = true;
    }
    hub.changed.notify_all();
}

/// A slot among the handler threads, given back on every exit path.
struct Permit<'a>(&'a AtomicUsize);

impl<'a> Permit<'a> {
    fn take(counter: &'a AtomicUsize, cap: usize) -> Option<Permit<'a>> {
        counter
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < cap).then_some(n + 1))
            .ok()
            .map(|_| Permit(counter))
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn accept_loop(hub: &Arc<Hub>, listener: &TcpListener) {
    for conn in listener.incoming() {
        if hub.stop.load(Ordering::SeqCst) {
            return;
        }
        let Ok(stream) = conn else {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        };
        if hub.handlers.load(Ordering::SeqCst) >= MAX_HANDLERS {
            let _ = stream.shutdown(Shutdown::Both);
            continue;
        }
        let hub = hub.clone();
        let _ = std::thread::Builder::new().name("observatory-conn".into()).spawn(move || {
            if let Some(_permit) = Permit::take(&hub.handlers, MAX_HANDLERS) {
                handle(&hub, stream);
            }
        });
    }
}

/// A parsed request head.
struct Request {
    method: String,
    path: String,
    query: String,
    head: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let deadline = Instant::now() + REQUEST_DEADLINE;
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > MAX_HEAD || Instant::now() >= deadline {
            return None;
        }
        let left = deadline.saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
        stream.set_read_timeout(Some(left)).ok()?;
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    };
    let head = String::from_utf8_lossy(&buf[..end]).into_owned();
    let mut words = head.split_whitespace();
    let method = words.next()?.to_string();
    let target = words.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let (path, query) = (path.to_string(), query.to_string());
    let length: usize = header(&head, "content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    if length > MAX_BODY {
        return None;
    }
    let mut body = buf[end..].to_vec();
    while body.len() < length {
        if Instant::now() >= deadline {
            return None;
        }
        let left = deadline.saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
        stream.set_read_timeout(Some(left)).ok()?;
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    body.truncate(length);
    Some(Request { method, path, query, head, body })
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|line| {
        let (k, v) = line.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         Access-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn json(stream: &mut TcpStream, status: &str, body: &str) {
    respond(stream, status, "application/json", body.as_bytes());
}

fn error(stream: &mut TcpStream, status: &str, message: &str) {
    json(stream, status, &serde_json::json!({ "error": message }).to_string());
}

fn handle(hub: &Hub, mut stream: TcpStream) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let Some(req) = read_request(&mut stream) else {
        return;
    };
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => {
            respond(&mut stream, "200 OK", "text/html; charset=utf-8", DASHBOARD_HTML.as_bytes())
        }
        ("GET", "/snapshot") => {
            let live = hub.state.lock().map(|s| s.live.clone()).unwrap_or_default();
            json(&mut stream, "200 OK", &live);
        }
        ("GET", part @ ("/summary" | "/clock" | "/run" | "/populations" | "/environment")) => {
            let Some(s) = hub.state.lock().ok().and_then(|s| s.snapshot.clone()) else {
                return error(&mut stream, "503 Service Unavailable", "nothing published yet");
            };
            let body = match part {
                "/summary" => serde_json::to_string(&s.summary),
                "/clock" => serde_json::to_string(&s.clock),
                "/run" => serde_json::to_string(&s.run),
                "/populations" => serde_json::to_string(&s.populations),
                _ => serde_json::to_string(&s.environment),
            };
            json(&mut stream, "200 OK", &body.unwrap_or_default());
        }
        ("GET", "/history") => {
            let since = req
                .query
                .split('&')
                .find_map(|kv| kv.strip_prefix("since="))
                .and_then(|v| v.parse::<u64>().ok());
            let body = {
                let Ok(st) = hub.state.lock() else { return };
                let samples: Vec<&Sample> = st.history.since(since).collect();
                serde_json::json!({
                    "sample_s": st.history.config().sample_s,
                    "span_s": st.history.config().span_s,
                    "samples": samples,
                })
                .to_string()
            };
            json(&mut stream, "200 OK", &body);
        }
        ("GET", "/animals") => {
            let body = {
                let Ok(st) = hub.state.lock() else { return };
                animals_body(&st)
            };
            json(&mut stream, "200 OK", &body);
        }
        ("GET", p) if p.starts_with("/animals/") => {
            let Ok(id) = p["/animals/".len()..].parse::<u64>() else {
                return error(&mut stream, "400 Bad Request", "an animal id is a whole number");
            };
            let body = {
                let Ok(st) = hub.state.lock() else { return };
                animal_body(&st, id)
            };
            match body {
                Some(b) => json(&mut stream, "200 OK", &b),
                None => error(&mut stream, "404 Not Found", "no living or recorded animal has that id"),
            }
        }
        ("GET", "/stream") => stream_events(hub, stream),
        ("POST", "/control") => control(hub, &mut stream, &req),
        ("OPTIONS", _) => respond(&mut stream, "204 No Content", "text/plain", b""),
        ("GET", _) => error(&mut stream, "404 Not Found", "no such route"),
        _ => error(&mut stream, "405 Method Not Allowed", "GET, or POST /control"),
    }
}

/// A row with its family, for `/animals`.
#[derive(Serialize)]
struct Listed<'a> {
    #[serde(flatten)]
    row: &'a AnimalRow,
    parent: Option<u64>,
    offspring: usize,
}

#[derive(Serialize)]
struct Death<'a> {
    id: u64,
    species: &'a str,
    label: &'a str,
    tick: u64,
    cause: &'a str,
    by: Option<u64>,
}

fn animals_body(st: &State) -> String {
    let Some(s) = &st.snapshot else { return "{\"tick\":0,\"animals\":[],\"recent_deaths\":[]}".into() };
    let rows: Vec<Listed> = s
        .animals
        .iter()
        .map(|a| {
            let rec = st.book.record(a.row.id);
            Listed {
                row: &a.row,
                parent: rec.and_then(|r| r.parent),
                offspring: rec.map_or(0, |r| r.offspring.len()),
            }
        })
        .collect();
    let mut dead: Vec<&LifeRecord> = st.book.records().filter(|r| r.death.is_some()).collect();
    dead.sort_by_key(|r| std::cmp::Reverse(r.death.as_ref().map_or(0, |d| d.tick)));
    let deaths: Vec<Death> = dead
        .into_iter()
        .take(RECENT_DEATHS)
        .filter_map(|r| {
            let d = r.death.as_ref()?;
            let crate::life::What::Died { cause, by } = &d.what else { return None };
            Some(Death { id: r.id, species: &r.species, label: &r.label, tick: d.tick, cause, by: *by })
        })
        .collect();
    serde_json::json!({ "tick": s.clock.tick, "animals": rows, "recent_deaths": deaths }).to_string()
}

fn animal_body(st: &State, id: u64) -> Option<String> {
    let live = st.snapshot.as_ref().and_then(|s| s.animals.iter().find(|a| a.row.id == id));
    let record = st.book.record(id);
    if live.is_none() && record.is_none() {
        return None;
    }
    Some(
        serde_json::json!({
            "id": id,
            "alive": live.is_some(),
            "tick": st.snapshot.as_ref().map_or(0, |s| s.clock.tick),
            "detail": live,
            "history": record,
        })
        .to_string(),
    )
}

fn stream_events(hub: &Hub, mut stream: TcpStream) {
    let Some(_slot) = Permit::take(&hub.streams, MAX_STREAMS) else {
        return error(&mut stream, "503 Service Unavailable", "too many open streams");
    };
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\
                Access-Control-Allow-Origin: *\r\nConnection: keep-alive\r\n\r\nretry: 2000\n\n";
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let mut sent = 0u64;
    loop {
        let (seq, live) = {
            let Ok(mut st) = hub.state.lock() else { return };
            let start = Instant::now();
            while st.seq == sent && !st.stopping && start.elapsed() < KEEPALIVE {
                let left = KEEPALIVE.saturating_sub(start.elapsed());
                st = match hub.changed.wait_timeout(st, left) {
                    Ok((g, _)) => g,
                    Err(_) => return,
                };
            }
            if st.stopping || hub.stop.load(Ordering::SeqCst) {
                return;
            }
            (st.seq, st.live.clone())
        };
        let msg = if seq == sent {
            ": keepalive\n\n".to_string()
        } else {
            format!("event: snapshot\nid: {seq}\ndata: {live}\n\n")
        };
        if stream.write_all(msg.as_bytes()).and_then(|_| stream.flush()).is_err() {
            return;
        }
        sent = seq;
    }
}

/// Parse a `/control` body into commands.
pub fn parse_control(body: &[u8]) -> Result<Vec<Command>, &'static str> {
    let v: serde_json::Value = serde_json::from_slice(body).map_err(|_| "the body is not JSON")?;
    let o = v.as_object().ok_or("the body is a JSON object")?;
    let mut out = Vec::new();
    if let Some(s) = o.get("speed") {
        let s = s.as_f64().ok_or("speed is a number")?;
        if !(s.is_finite() && s > 0.0) {
            return Err("speed is a positive number (pause with {\"paused\": true})");
        }
        out.push(Command::SetSpeed(s));
    }
    if let Some(p) = o.get("paused") {
        out.push(if p.as_bool().ok_or("paused is true or false")? { Command::Pause } else { Command::Resume });
    }
    if let Some(s) = o.get("step") {
        out.push(match s.as_str() {
            Some("faster") => Command::Faster,
            Some("slower") => Command::Slower,
            _ => return Err("step is \"faster\" or \"slower\""),
        });
    }
    if out.is_empty() {
        return Err("nothing to do: give speed, paused or step");
    }
    Ok(out)
}

fn control(hub: &Hub, stream: &mut TcpStream, req: &Request) {
    // Only a page this server served (or a client that sends no Origin, like curl) may
    // change the run: a page elsewhere on the web cannot drive it through the browser.
    if let Some(origin) = header(&req.head, "origin") {
        let host = header(&req.head, "host").unwrap_or("");
        let origin_host = origin.split_once("://").map_or(origin, |(_, h)| h);
        if origin_host != host {
            return error(stream, "403 Forbidden", "commands come from the dashboard's own origin");
        }
    }
    let is_json = header(&req.head, "content-type").is_some_and(|t| t.starts_with("application/json"));
    if !is_json {
        return error(stream, "415 Unsupported Media Type", "send application/json");
    }
    match parse_control(&req.body) {
        Err(why) => error(stream, "400 Bad Request", why),
        Ok(commands) => {
            let sent = hub
                .commands
                .lock()
                .map(|tx| commands.iter().all(|c| tx.send(*c).is_ok()))
                .unwrap_or(false);
            if sent {
                json(stream, "202 Accepted", &serde_json::json!({ "accepted": commands }).to_string());
            } else {
                error(stream, "503 Service Unavailable", "the run is not taking commands");
            }
        }
    }
}
