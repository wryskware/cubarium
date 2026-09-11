//! The contract's required shim test: a local UDP receiver, the shim sink driving it,
//! `cube_proto::decode` on the datagrams, and byte-equality with the encoded `Frame`
//! for several frames with increasing sequence numbers.

use std::net::UdpSocket;
use std::time::Duration;

use cube_proto::{FRAME_BYTES, Face, Format, Frame, HEADER_BYTES, decode};
use cubarium::sink::{FrameSink, ShimSink};

/// Build a frame whose bytes are unique per index, touching all five faces.
fn distinctive(i: u8) -> Frame {
    let mut f = Frame::black();
    for (k, face) in Face::ALL.into_iter().enumerate() {
        f.fill_face(face, [i, i.wrapping_mul(3).wrapping_add(k as u8), 255 - i]);
        f.set(face, usize::from(i) % 64, k, [i, 200, k as u8]);
    }
    f
}

#[test]
fn the_shim_sink_sends_the_exact_encoded_frame_bytes_with_increasing_sequence() {
    let server = UdpSocket::bind("127.0.0.1:0").expect("binding the test receiver");
    server
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("setting a read timeout");
    let addr = server.local_addr().expect("receiver address");

    let mut sink = ShimSink::new(addr.to_string());
    let mut rx = vec![0u8; HEADER_BYTES + FRAME_BYTES + 64];

    for i in 0..6u8 {
        let frame = distinctive(i);
        sink.submit(&frame).expect("submit never fails");

        // Receive before submitting the next frame, so the newest-frame mailbox has no
        // chance to coalesce and every submitted frame reaches the wire.
        let n = server.recv(&mut rx).expect("a datagram arrives");
        let (header, payload) = decode(&rx[..n]).expect("a valid cube-proto datagram");

        assert_eq!(header.format, Format::FullFrame, "frame {i}");
        assert_eq!(header.face, None, "a full frame carries no face index");
        assert_eq!(header.seq, u32::from(i) + 1, "sequence numbers start at 1 and increase");
        assert_eq!(payload.len(), FRAME_BYTES, "frame {i} payload length");
        assert_eq!(payload, frame.as_bytes().as_slice(), "frame {i} payload bytes");
    }

    assert_eq!(sink.errors(), 0, "a listening receiver must produce no send errors");
    sink.finish().expect("clean shutdown");
}

#[test]
fn the_mailbox_coalesces_without_inventing_or_corrupting_frames() {
    let server = UdpSocket::bind("127.0.0.1:0").expect("binding the test receiver");
    server.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
    let addr = server.local_addr().unwrap();

    // Drain continuously on another thread so the kernel receive buffer never overflows.
    let collector = std::thread::spawn(move || {
        let mut rx = vec![0u8; HEADER_BYTES + FRAME_BYTES + 64];
        let mut out: Vec<(u32, Vec<u8>)> = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        while std::time::Instant::now() < deadline {
            match server.recv(&mut rx) {
                Ok(n) => {
                    let (h, p) = decode(&rx[..n]).expect("every datagram decodes");
                    out.push((h.seq, p.to_vec()));
                }
                Err(_) => {
                    if !out.is_empty() {
                        break;
                    }
                }
            }
        }
        out
    });

    let mut sink = ShimSink::new(addr.to_string());
    let submitted: Vec<Frame> = (0..40u8).map(distinctive).collect();
    for f in &submitted {
        sink.submit(f).unwrap();
    }
    assert!(sink.wait_for_sent(1, Duration::from_secs(3)), "the worker sent nothing");
    sink.finish().unwrap();

    let got = collector.join().expect("collector thread");
    assert!(!got.is_empty(), "at least one datagram arrives");
    assert!(got.len() <= submitted.len(), "the mailbox must not invent frames");
    let seqs: Vec<u32> = got.iter().map(|(s, _)| *s).collect();
    assert_eq!(seqs[0], 1, "sequence numbers start at 1");
    assert!(seqs.windows(2).all(|w| w[1] > w[0]), "sequence numbers increase: {seqs:?}");
    // Every datagram is byte-identical to one of the frames that was submitted: the
    // mailbox replaces whole frames, it never blends or truncates them.
    for (seq, payload) in &got {
        assert!(
            submitted.iter().any(|f| f.as_bytes().as_slice() == payload.as_slice()),
            "datagram seq {seq} does not match any submitted frame"
        );
    }
}

#[test]
fn a_dead_shim_is_not_fatal() {
    // Nothing is listening on this port; the loop must keep running and finish cleanly.
    let mut sink = ShimSink::new("127.0.0.1:9");
    let frame = Frame::black();
    let t0 = std::time::Instant::now();
    for _ in 0..100 {
        sink.submit(&frame).expect("submit never fails when the shim is down");
    }
    assert!(t0.elapsed() < Duration::from_secs(1), "submit must not block on the socket");
    sink.finish().expect("clean shutdown with a dead shim");
}
