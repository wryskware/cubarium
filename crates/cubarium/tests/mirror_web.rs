//! `--mirror-web`: one world, one render, one encode — and the identical bytes reaching
//! the shim and the loopback viewer at the same time.
//!
//! The three things worth proving, and nothing else: the two sinks receive the same
//! bytes (not "both look right"), the viewer can name the world's tick, and mirroring
//! leaves the simulation's state exactly where a headless run of the same seed leaves it.

mod support;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;

use cube_proto::{FRAME_BYTES, Face, Format, Frame, HEADER_BYTES, decode};
use cubarium::sink::web::FRAME_BODY_BYTES;
use cubarium::sink::{FanOutSink, FrameSink, ShimSink, WebSink};

/// A frame whose bytes are unique per index, touching all five faces. Same shape as the
/// fixture in `shim_sink.rs`, so a byte comparison here means something.
fn distinctive(i: u8) -> Frame {
    let mut f = Frame::black();
    for (k, face) in Face::ALL.into_iter().enumerate() {
        f.fill_face(face, [i, i.wrapping_mul(3).wrapping_add(k as u8), 255 - i]);
        f.set(face, usize::from(i) % 64, k, [i, 200, k as u8]);
    }
    f
}

/// One HTTP/1.1 GET over a real socket, returning the body bytes.
fn get(addr: SocketAddr, path: &str) -> Vec<u8> {
    let mut s = TcpStream::connect(addr).expect("connecting to the mirrored viewer");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(s, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    s.flush().unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).expect("reading the response");
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a complete response head");
    raw[end + 4..].to_vec()
}

#[test]
fn the_shim_and_the_viewer_receive_the_identical_frame_bytes() {
    let server = UdpSocket::bind("127.0.0.1:0").expect("binding the test receiver");
    server.set_read_timeout(Some(Duration::from_secs(5))).expect("setting a read timeout");
    let shim_addr = server.local_addr().expect("receiver address");

    let web = WebSink::new(0).expect("binding an ephemeral viewer port");
    let web_addr = web.addr();
    let mut fan = FanOutSink::new(vec![
        Box::new(ShimSink::new(shim_addr.to_string())) as Box<dyn FrameSink>,
        Box::new(web),
    ]);
    assert_eq!(fan.len(), 2, "the fan-out feeds the shim and the viewer");

    let mut rx = vec![0u8; HEADER_BYTES + FRAME_BYTES + 64];
    for i in 0..5u8 {
        let frame = distinctive(i);
        fan.submit(&frame).expect("submit never fails");

        // Receive before the next submit, so the shim's newest-frame mailbox has no
        // chance to coalesce and every submitted frame reaches the wire.
        let n = server.recv(&mut rx).expect("a datagram arrives");
        let (header, payload) = decode(&rx[..n]).expect("a valid cube-proto datagram");
        assert_eq!(header.format, Format::FullFrame, "frame {i}");
        assert_eq!(payload.len(), FRAME_BYTES, "frame {i} payload length");

        let body = get(web_addr, "/frame");
        assert_eq!(body.len(), FRAME_BODY_BYTES, "frame {i} /frame body length");
        assert_eq!(
            u64::from_le_bytes(body[..8].try_into().unwrap()),
            u64::from(i),
            "the /frame prefix is the render sequence, one per submitted frame"
        );

        // The three-way identity this flag exists for: what the cube gets, what the
        // browser gets, and what the host encoded are one and the same bytes.
        assert_eq!(payload, &body[8..], "frame {i}: the shim and the viewer disagree");
        assert_eq!(payload, frame.as_bytes().as_slice(), "frame {i}: the shim altered the frame");
        assert_eq!(
            &body[8..],
            frame.as_bytes().as_slice(),
            "frame {i}: the viewer altered the frame"
        );
    }

    fan.finish().expect("clean shutdown of both sinks");
}

#[test]
fn the_world_tick_reaches_the_mirrored_viewers_status_route() {
    let web = WebSink::new(0).expect("binding an ephemeral viewer port");
    let web_addr = web.addr();
    // Port 9 discards; this run is about the tick, not the wire.
    let mut fan = FanOutSink::new(vec![
        Box::new(ShimSink::new("127.0.0.1:9")) as Box<dyn FrameSink>,
        Box::new(web),
    ]);

    fan.observe_tick(4242);
    let body = get(web_addr, "/status");
    let status: serde_json::Value =
        serde_json::from_slice(&body).expect("the status route answers JSON");
    assert_eq!(status["world_tick"], 4242, "{status}");
    // The viewer describes the host, never the other way around.
    assert_eq!(status["source"]["pid"], serde_json::Value::from(std::process::id()));

    fan.finish().expect("clean shutdown");
}

#[test]
fn mirroring_a_shim_run_renders_frames_and_changes_nothing_about_the_world() {
    // A receiver that is drained, so the shim worker reports no errors and the kernel
    // buffer cannot fill during the run.
    let server = UdpSocket::bind("127.0.0.1:0").expect("binding the test receiver");
    server.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
    let shim_addr = server.local_addr().unwrap().to_string();
    let draining = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let flag = std::sync::Arc::clone(&draining);
    let drain = std::thread::spawn(move || {
        let mut rx = vec![0u8; HEADER_BYTES + FRAME_BYTES + 64];
        while flag.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = server.recv(&mut rx);
        }
    });

    let mirrored_state = support::Scratch::new("mirror-web-mirrored");
    let mirrored = support::run(&[
        "--sink", "shim", "--addr", &shim_addr, "--mirror-web", "--web-port", "0",
        "--speed", "20", "--seconds", "10", "--fresh", "--seed", "5",
        "--state", mirrored_state.path().to_str().unwrap(),
    ]);

    draining.store(false, std::sync::atomic::Ordering::Relaxed);
    drain.join().expect("the drain thread");

    assert!(mirrored.frames > 0, "a mirrored run must have rendered: {mirrored:?}");
    assert_eq!(mirrored.final_tick, 200, "10 simulated seconds at 20 Hz");

    // The same world, run headless with no rendering at all, must end in the same state:
    // observing is not participating, and mirroring adds no second world.
    let headless_state = support::Scratch::new("mirror-web-headless");
    let headless = support::run(&[
        "--sink", "none", "--speed", "0", "--seconds", "10", "--fresh", "--seed", "5",
        "--state", headless_state.path().to_str().unwrap(),
    ]);
    assert_eq!(headless.frames, 0, "a headless run renders nothing");
    assert_eq!(
        mirrored.state_hash, headless.state_hash,
        "rendering to two sinks changed the world: mirrored {:?} vs headless {:?}",
        mirrored.state_hash, headless.state_hash
    );
    assert_eq!(mirrored.final_tick, headless.final_tick);
    assert_eq!(mirrored.population, headless.population);
}
