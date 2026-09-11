//! `CubeClient` end to end against a real bound UDP socket on 127.0.0.1.

use std::net::UdpSocket;
use std::time::Duration;

use cube_proto::{decode, CubeClient, Face, Format, Frame, FACE_BYTES, FRAME_BYTES, HEADER_BYTES};

fn server() -> (UdpSocket, std::net::SocketAddr) {
    let s = UdpSocket::bind("127.0.0.1:0").expect("bind loopback");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let a = s.local_addr().unwrap();
    (s, a)
}

fn recv(sock: &UdpSocket) -> Vec<u8> {
    let mut buf = vec![0u8; HEADER_BYTES + FRAME_BYTES + 16];
    let n = sock.recv(&mut buf).expect("datagram");
    buf.truncate(n);
    buf
}

#[test]
fn send_and_send_face_produce_decodable_datagrams_with_seq_1_2_3() {
    let (sock, addr) = server();
    let mut client = CubeClient::connect(addr).expect("connect");
    assert_eq!(client.next_seq(), 1, "the doc starts sequence numbers at 1");

    let mut frame = Frame::black();
    // A pixel that must survive the trip at exactly the documented offset.
    frame.set(Face::Left, 5, 6, [0x11, 0x22, 0x33]);
    frame.set(Face::Top, 63, 0, [0x44, 0x55, 0x66]);

    client.send(&frame).unwrap();
    client.send_face(&frame, Face::Top).unwrap();
    client.send(&frame).unwrap();

    // #1: full frame, seq 1.
    let d = recv(&sock);
    assert_eq!(d.len(), 61456);
    let (h, payload) = decode(&d).expect("decodes");
    assert_eq!(h.seq, 1);
    assert_eq!(h.format, Format::FullFrame);
    assert_eq!(h.face, None);
    assert_eq!(payload, frame.as_bytes().as_slice());
    // Left is face index 3: (5,6) → 3*12288 + (6*64+5)*3.
    let o = 3 * 12288 + (6 * 64 + 5) * 3;
    assert_eq!(&payload[o..o + 3], &[0x11, 0x22, 0x33]);

    // #2: single face Top, seq 2.
    let d = recv(&sock);
    assert_eq!(d.len(), 12304);
    let (h, payload) = decode(&d).expect("decodes");
    assert_eq!(h.seq, 2);
    assert_eq!(h.format, Format::SingleFace);
    assert_eq!(h.face, Some(Face::Top));
    assert_eq!(payload.len(), FACE_BYTES);
    assert_eq!(payload, frame.face(Face::Top));
    let o = 63 * 3; // row 0, column 63
    assert_eq!(&payload[o..o + 3], &[0x44, 0x55, 0x66]);

    // #3: full frame, seq 3.
    let (h, _) = decode(&recv(&sock)).expect("decodes");
    assert_eq!(h.seq, 3);
    assert_eq!(client.next_seq(), 4);
}

#[test]
fn sequence_numbers_keep_increasing_across_many_sends() {
    let (sock, addr) = server();
    let mut client = CubeClient::connect(addr).expect("connect");
    let frame = Frame::black();
    for expected in 1..=8u32 {
        client.send_face(&frame, Face::Front).unwrap();
        let (h, _) = decode(&recv(&sock)).expect("decodes");
        assert_eq!(h.seq, expected);
        assert!(cube_proto::seq_is_newer(h.seq, expected.saturating_sub(1)));
    }
}

#[test]
fn connect_accepts_a_host_port_tuple_and_reports_a_usable_socket() {
    let (sock, addr) = server();
    let mut client = CubeClient::connect(("127.0.0.1", addr.port())).expect("connect by tuple");
    assert!(client.socket().local_addr().unwrap().ip().is_ipv4());
    client.send(&Frame::black()).unwrap();
    assert_eq!(recv(&sock).len(), 61456);
}

#[test]
fn connect_to_an_unresolvable_address_is_an_error() {
    assert!(CubeClient::connect("this-host-does-not-exist.invalid:7392").is_err());
}
