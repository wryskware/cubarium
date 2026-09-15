use super::*;
use cubarium_surface::face_frame;
use cube_proto::{FACE_BYTES, Face};

/// One request over a real socket, returning the status line, the headers and the
/// body bytes.
fn get(addr: SocketAddr, path: &str) -> (String, String, Vec<u8>) {
    let mut s = TcpStream::connect(addr).expect("connecting to the web sink");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    s.flush().unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).expect("reading the response");
    let end = find_head_end(&raw).expect("a complete response head");
    let head = String::from_utf8_lossy(&raw[..end]).into_owned();
    let body_start = if raw[end..].starts_with(b"\r\n\r\n") {
        end + 4
    } else {
        end + 2
    };
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
    assert!(
        head.contains("Content-Type: text/html; charset=utf-8"),
        "{head}"
    );
    assert_eq!(
        body,
        INDEX_HTML.as_bytes(),
        "the body is the embedded page verbatim"
    );
}

#[test]
fn the_frame_route_serves_the_tick_and_the_submitted_frame() {
    let mut sink = WebSink::new(0).expect("binding an ephemeral port");
    let frame = distinct_frame(3);
    sink.submit(&frame).unwrap();

    let (status, head, body) = get(sink.addr(), "/frame");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(
        head.contains("Content-Type: application/octet-stream"),
        "{head}"
    );
    assert!(head.contains("Cache-Control: no-store"), "{head}");
    assert_eq!(
        body.len(),
        FRAME_BODY_BYTES,
        "8-byte sequence plus {FRAME_BYTES} frame bytes"
    );
    assert_eq!(
        u64::from_le_bytes(body[..8].try_into().unwrap()),
        0,
        "the first frame is render sequence 0"
    );
    assert_eq!(
        &body[8..],
        frame.as_bytes().as_slice(),
        "the payload is the submitted frame"
    );
}

#[test]
fn the_frame_route_answers_before_any_frame_is_submitted() {
    let sink = WebSink::new(0).expect("binding an ephemeral port");
    let (status, _, body) = get(sink.addr(), "/frame");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert_eq!(body.len(), FRAME_BODY_BYTES);
    assert!(
        body[8..].iter().all(|&b| b == 0),
        "a black frame before the host submits one"
    );
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
    assert!(
        t0.elapsed() < Duration::from_millis(500),
        "submit blocked on I/O"
    );
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
    assert!(
        elapsed < Duration::from_millis(500),
        "submit waited on a client: {elapsed:?}"
    );
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
    assert!(
        head.contains("Content-Type: application/json; charset=utf-8"),
        "{head}"
    );
    assert!(head.contains("Cache-Control: no-store"), "{head}");
    let text = String::from_utf8(body).expect("the status body is UTF-8");
    let v: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("status is not JSON: {e}\n{text}"));
    assert_eq!(v["world_tick"], 4242);
    assert_eq!(
        v["render_seq"], 1,
        "two submits: the newest frame is sequence 1"
    );
    assert_eq!(
        v["frames_served"], 0,
        "no /frame request has been answered yet"
    );
    let s = &v["source"];
    assert_eq!(s["pid"], 4321);
    assert_eq!(s["state_dir"], "/tmp/cubarium-status-test/state");
    assert_eq!(s["build_id"], "0.1.0+abcdef1");
    assert_eq!(s["sink"], "shim");
    assert_eq!(
        s["resumed_from"],
        "/tmp/cubarium-status-test/state/world-100.cubw"
    );
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

/// The seeding control is only checkable from outside if `/status` says how many neural
/// animals are alive; a world that was never seeded must read zero rather than omit it.
#[test]
fn the_status_route_reports_the_population_and_how_many_of_it_is_neural() {
    let mut sink = WebSink::new(0).expect("binding an ephemeral port");
    let (_, _, body) = get(sink.addr(), "/status");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["population"], 0, "before the first tick, zero — not absent");
    assert_eq!(v["neural_animals"], 0, "an ordinary world is never neural");

    sink.observe_tick(200);
    sink.observe_counts(37, 4);
    let (_, _, body) = get(sink.addr(), "/status");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["world_tick"], 200);
    assert_eq!(v["population"], 37);
    assert_eq!(v["neural_animals"], 4);
}

#[test]
fn a_plain_sink_still_answers_status_with_a_null_resume_and_this_process() {
    let sink = WebSink::new(0).expect("binding an ephemeral port");
    let (status, _, body) = get(sink.addr(), "/status");
    assert_eq!(status, "HTTP/1.1 200 OK");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["world_tick"], 0);
    assert_eq!(
        v["render_seq"], 0,
        "before any submit the sequence reads 0, like /frame"
    );
    assert!(v["source"]["resumed_from"].is_null());
    assert_eq!(
        v["source"]["pid"],
        serde_json::Value::from(std::process::id())
    );
    assert_eq!(v["source"]["build_id"], crate::state::build_id());
    assert_eq!(v["source"]["sink"], "web");
}

/// The page must actually ask for the status, or the HUD can never name the world.
#[test]
fn the_page_fetches_the_status_route() {
    assert!(
        INDEX_HTML.contains("fetch(\"/status\""),
        "the page never fetches /status"
    );
    assert!(
        INDEX_HTML.contains("world_tick"),
        "the page never reads the world tick"
    );
    assert!(
        INDEX_HTML.contains("state_dir"),
        "the page never names the source state dir"
    );
}

#[test]
fn the_note_route_serves_the_hosts_note_and_is_empty_by_default() {
    let plain = WebSink::new(0).expect("binding an ephemeral port");
    let (status, head, body) = get(plain.addr(), "/note");
    assert_eq!(status, "HTTP/1.1 200 OK");
    assert!(
        head.contains("Content-Type: text/plain; charset=utf-8"),
        "{head}"
    );
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
    assert!(
        INDEX_HTML.contains("fetch(\"/note\""),
        "the page never fetches /note"
    );
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
    let start = if buf[end..].starts_with(b"\r\n\r\n") {
        end + 4
    } else {
        end + 2
    };
    let status = head.lines().next().unwrap_or_default().to_string();
    (
        status,
        head,
        String::from_utf8_lossy(&buf[start..]).into_owned(),
    )
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
    assert_eq!(
        v["enabled"], false,
        "the page must be able to tell care is off"
    );
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
    let client = v["client"]
        .as_str()
        .expect("an issued identity")
        .to_string();
    assert!(
        client.starts_with("epoch-http."),
        "the id embeds the epoch: {client}"
    );
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
        assert!(
            std::time::Instant::now() < deadline,
            "the request never reached the FIFO"
        );
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
fn apex_count_reaches_the_control_fifo_and_invalid_counts_are_400() {
    let (sink, service) = care_sink();
    let addr = sink.addr();
    let (_, _, body) = care_post(addr, "/care/register", "{}");
    let client = serde_json::from_str::<serde_json::Value>(&body).unwrap()["client"]
        .as_str()
        .unwrap()
        .to_string();
    for count in [0, 3] {
        let payload =
            format!(r#"{{"client":"{client}","request":1,"kind":"spawn_apex","count":{count}}}"#);
        let (status, _, body) = care_post(addr, "/care", &payload);
        assert_eq!(status, "HTTP/1.1 400 Bad Request");
        assert!(body.contains("count"), "{body}");
    }
    let payload = format!(r#"{{"client":"{client}","request":1,"kind":"spawn_apex","count":2}}"#);
    let posted = std::thread::spawn(move || care_post(addr, "/care", &payload));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let planned = loop {
        let planned = service.drain_prepared(1, 90);
        if !planned.is_empty() {
            break planned;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the apex request never arrived"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(planned.len(), 1);
    assert_eq!(planned[0].kind, CareKind::SpawnApex);
    assert!(planned[0].second_target.is_some());
    service.commit_accepted(&planned);
    assert_eq!(posted.join().unwrap().0, "HTTP/1.1 202 Accepted");
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
    for origin in [
        "http://evil.example",
        "http://localhost:1",
        "null",
        "https://127.0.0.1",
    ] {
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
    assert!(
        !head.to_ascii_lowercase().contains("access-control-"),
        "{head}"
    );

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
        assert_eq!(
            status, "HTTP/1.1 400 Bad Request",
            "body {body} answered {answer}"
        );
    }
}

/// The one new HTTP property. Omission is the standard dose; anything *present* must be an
/// integer inside the documented range. An explicit `null`, a fractional or negative
/// number, a string or an out-of-range value is a `400` — never clamped, and never read as
/// omission. Every one of these is refused before any identity is even consulted, so a
/// rejected amount cannot burn a request number.
#[test]
fn an_unusable_dose_is_400_and_is_never_clamped_or_read_as_omission() {
    let (sink, _service) = care_sink();
    let addr = sink.addr();
    let with = |dose: &str| {
        format!(
            r#"{{"client":"x","request":1,"kind":"feed","target":{{"face":0,"u":0,"v":0}},"dose_permille":{dose}}}"#
        )
    };
    for dose in [
        "null",
        "1500.5",
        "-500",
        "0",
        "249",
        "2001",
        "65536",
        "99999999999999999999",
        "\"1500\"",
        "true",
        "[1500]",
        "{}",
    ] {
        let body = with(dose);
        let (status, _, answer) = care_post(addr, "/care", &body);
        assert_eq!(
            status, "HTTP/1.1 400 Bad Request",
            "dose {dose} answered {answer}"
        );
        assert!(
            answer.contains("dose_permille"),
            "the refusal names the field: {answer}"
        );
    }
    // The bounds themselves are not refused: they get as far as the identity check, which
    // is the next gate and a different answer.
    for dose in ["250", "1000", "2000"] {
        let (status, _, answer) = care_post(addr, "/care", &with(dose));
        assert_eq!(
            status, "HTTP/1.1 409 Conflict",
            "dose {dose} answered {answer}"
        );
    }
}

/// A valid amount reaches the planned command verbatim, and an omitted one is the standard
/// dose — the two paths a viewer can take, end to end through the actual HTTP route.
#[test]
fn a_posted_dose_reaches_the_planned_command_and_omission_is_standard() {
    for (property, want) in [(r#","dose_permille":1500"#, 1500u16), ("", 1000)] {
        let (sink, service) = care_sink();
        let addr = sink.addr();
        let (_, _, body) = care_post(addr, "/care/register", "{}");
        let client = serde_json::from_str::<serde_json::Value>(&body).unwrap()["client"]
            .as_str()
            .unwrap()
            .to_string();
        let payload = format!(
            r#"{{"client":"{client}","request":1,"kind":"rain","target":{{"face":2,"u":31,"v":7}}{property}}}"#
        );
        let posted = std::thread::spawn(move || care_post(addr, "/care", &payload));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let planned = loop {
            let planned = service.drain_prepared(1, 60);
            if !planned.is_empty() {
                break planned;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the request never arrived"
            );
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(planned[0].dose.permille(), want, "property {property:?}");
        service.commit_accepted(&planned);
        let (status, _, _) = posted.join().unwrap();
        assert_eq!(status, "HTTP/1.1 202 Accepted");

        // And the amount is in the row the page reads back.
        let (_, _, body) = get(sink.addr(), "/care/status");
        let v: serde_json::Value = serde_json::from_str(&body_text(&body)).unwrap();
        assert_eq!(v["receipts"][0]["dose_permille"], want);
        assert_eq!(v["dose"]["version"], 1);
    }
}

/// The capability block a viewer gates amount selection on, served over the real route —
/// and absent from a host that offers no care at all.
#[test]
fn the_status_route_advertises_the_dose_capability_only_when_care_is_enabled() {
    let (sink, _service) = care_sink();
    let (status, _, body) = get(sink.addr(), "/care/status");
    assert_eq!(status, "HTTP/1.1 200 OK");
    let v: serde_json::Value = serde_json::from_str(&body_text(&body)).unwrap();
    assert_eq!(v["enabled"], true);
    assert_eq!(
        v["dose"],
        serde_json::json!({
            "version": 1,
            "min_permille": 250,
            "max_permille": 2000,
            "default_permille": 1000,
        })
    );

    let plain = WebSink::new(0).expect("binding");
    let (_, _, body) = get(plain.addr(), "/care/status");
    let v: serde_json::Value = serde_json::from_str(&body_text(&body)).unwrap();
    assert_eq!(v["enabled"], false);
    assert!(v["dose"].is_null(), "no care means no advertised amounts");
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
    assert!(
        sink.refused_connections() > 0,
        "the excess must be closed, not queued"
    );

    // The permits come back once the absolute deadline expires, even though every one
    // of those peers is still connected and would renew a per-read timeout forever.
    let deadline = std::time::Instant::now() + REQUEST_DEADLINE + Duration::from_secs(5);
    while sink.handlers() > 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        sink.handlers(),
        0,
        "a permit was not released on some exit path"
    );
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
    let _ = write!(
        s,
        "GET /status HTTP/1.1\r\nHost: localhost\r\nX-Pad: {padding}\r\n\r\n"
    );
    let _ = s.flush();
    let mut buf = Vec::new();
    let _ = s.read_to_end(&mut buf);
    assert!(
        buf.is_empty(),
        "an oversized head must be dropped, not answered: {} bytes",
        buf.len()
    );
}

// -- the page's face table cannot drift from the Rust one --------------------

/// Pull `key: [a, b, c]` out of one `FACE_FRAMES` row.
fn parse_vec(row: &str, key: &str) -> [f64; 3] {
    let at = row
        .find(&format!("{key}: ["))
        .unwrap_or_else(|| panic!("row is missing `{key}`: {row}"));
    let rest = &row[at + key.len() + 3..];
    let end = rest
        .find(']')
        .unwrap_or_else(|| panic!("unterminated `{key}`: {row}"));
    let nums: Vec<f64> = rest[..end]
        .split(',')
        .map(|n| {
            n.trim()
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("bad number in {row}"))
        })
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
    let end = rest
        .find("\n];")
        .expect("the FACE_FRAMES literal must end with a line `];`");
    let rows: Vec<String> = rest[..end]
        .lines()
        .filter(|l| l.contains("face:"))
        .map(|l| l.trim().to_string())
        .collect();
    assert_eq!(
        rows.len(),
        5,
        "the page's table must have exactly five rows"
    );
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
            assert!(
                p.iter().all(|c| c.abs() > 0.9),
                "{face:?} ({u}, {v}) is not near a corner"
            );
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
            assert!(
                p.abs() > 0.9,
                "{face:?} chart ({u}, {v}) axis {axis} is not at the rim"
            );
        }
    }
}

/// The page's net layout must be the same grid the PNG sink and preview use.
#[test]
fn the_pages_net_cells_match_the_hosts_net_layout() {
    let at = INDEX_HTML
        .find("const NET_CELL = [")
        .expect("the page declares NET_CELL");
    let rest = &INDEX_HTML[at..];
    let line = rest.lines().next().unwrap();
    for face in Face::ALL {
        let (col, row) = crate::net::net_cell(face);
        let want = format!("[{col}, {row}]");
        assert!(
            line.contains(&want),
            "{face:?} cell {want} missing from: {line}"
        );
    }
}

/// The page's click-to-target mapping must be built from the same two constants the
/// drawing code uses, or a click would land on a different cell than the one under the
/// cursor — and the world would be fed somewhere nobody chose.
#[test]
fn the_pages_click_mapping_is_built_from_net_cell_and_face_size() {
    let start = INDEX_HTML
        .find("function netTarget(")
        .expect("the page maps clicks to cells");
    let end = INDEX_HTML[start..].find("\n}").expect("netTarget must end") + start;
    let body = &INDEX_HTML[start..end];
    assert!(
        body.contains("NET_CELL[f][0]") && body.contains("NET_CELL[f][1]"),
        "the mapping must use the same NET_CELL table the net is drawn from: {body}"
    );
    assert!(body.contains("FACE_SIZE * NET_SCALE"), "{body}");
    assert!(
        body.contains("col * FACE_SIZE") && body.contains("row * FACE_SIZE"),
        "{body}"
    );
    // Chart coordinates stay inside the face, so a click on a seam cannot address a
    // pixel of some other face.
    assert!(
        body.contains("u >= FACE_SIZE") && body.contains("v >= FACE_SIZE"),
        "{body}"
    );
    // The marker is drawn on the overlay, never into the frame bytes.
    let mark = INDEX_HTML
        .find("function drawMark(")
        .expect("the page draws a marker");
    let mark_end = INDEX_HTML[mark..].find("\n}").expect("drawMark must end") + mark;
    assert!(
        !INDEX_HTML[mark..mark_end].contains("frameBytes"),
        "the target marker must never be written into the frame"
    );
    assert!(
        INDEX_HTML.contains(r#"markCtx.clearRect"#),
        "the overlay is redrawn, not stacked"
    );
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
    for label in [
        "Scatter food",
        "Shower",
        "Clean up litter",
        "Spawn 1 apex",
        "Spawn 2 apex",
    ] {
        assert!(
            INDEX_HTML.contains(label),
            "the panel is missing the button {label:?}"
        );
    }
    assert!(INDEX_HTML.contains(r#"kind: "spawn_apex", count: count"#));
    assert!(
        INDEX_HTML.contains(r#""X-Cubarium-Care": "1""#),
        "the custom header is the lock"
    );
    assert!(
        INDEX_HTML.contains(r#"fetch("/care/register""#),
        "the page registers on load"
    );
    assert!(INDEX_HTML.contains(r#"fetch("/care", { method: "POST""#));
    assert!(INDEX_HTML.contains(r#"fetch("/care/status""#));
    // Every state the contract asks a request row to show.
    for state in ["queued", "accepted", "applied", "partial", "rejected"] {
        assert!(
            INDEX_HTML.contains(&format!(".st-{state}")),
            "no style for {state}"
        );
    }
    assert!(
        INDEX_HTML.contains("holding at tick"),
        "the hold must be visible"
    );
    assert!(
        INDEX_HTML.contains("care failed at tick"),
        "the failure must be visible"
    );
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
        INDEX_HTML.contains(
            "if (careClient === null && s.care !== \"failed\" && performance.now() >= careRetryAt)"
        ),
        "the status poll must retry registration once the host is reachable"
    );
    // The row map is trimmed with the DOM, keyed by the request number on the element.
    assert!(
        INDEX_HTML.contains("careRows.delete(Number(gone.dataset.request));"),
        "unbounded row map"
    );
    assert!(INDEX_HTML.contains("careRowsEl.children.length > CARE_ROWS_MAX"));
}

/// Guard the page's other load-bearing constants and its one external dependency.
#[test]
fn the_page_declares_the_expected_sizes_and_one_external_script() {
    assert!(INDEX_HTML.contains("const FRAME_BYTES = NUM_FACES * FACE_BYTES;"));
    assert!(INDEX_HTML.contains("const FACE_SIZE = 64;"));
    assert!(
        INDEX_HTML.contains("8 + FRAME_BYTES"),
        "the page checks the /frame body length"
    );
    assert_eq!(FACE_BYTES * 5, FRAME_BYTES);
    let scripts: Vec<&str> = INDEX_HTML
        .match_indices("<script src=")
        .map(|(_, s)| s)
        .collect();
    assert_eq!(scripts.len(), 1, "exactly one external script");
    assert!(
        INDEX_HTML.contains("https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"),
        "three.js r128 from cdnjs"
    );
    assert!(
        INDEX_HTML.contains("#0B0525"),
        "the dark background of the room's palette"
    );
}
