//! The UDP wire format, checked byte-for-byte against the table in
//! `docs/ARCHITECTURE.md` ("cube-proto" → "Wire format").
//!
//! | offset | size | field    | notes                                     |
//! |--------|------|----------|-------------------------------------------|
//! | 0      | 4    | magic    | b"CUBE"                                   |
//! | 4      | 1    | version  | 1                                         |
//! | 5      | 1    | format   | 0 = full frame, 1 = single face           |
//! | 6      | 1    | face     | face index for format 1, 0xFF for format 0|
//! | 7      | 1    | flags    | reserved, must be 0                       |
//! | 8      | 4    | seq      | u32 little-endian                         |
//! | 12     | 4    | reserved | 0                                         |

use cube_proto::{
    decode, encode_face, encode_full, seq_is_newer, Face, Format, Frame, Header, ProtoError,
    FACE_BYTES, FRAME_BYTES, HEADER_BYTES,
};

/// Build the 16 header bytes straight from the table above, with no help from the crate.
fn doc_header(format_code: u8, face_byte: u8, seq: u32) -> [u8; 16] {
    let mut h = [0u8; 16];
    h[0] = b'C';
    h[1] = b'U';
    h[2] = b'B';
    h[3] = b'E';
    h[4] = 1; // version
    h[5] = format_code;
    h[6] = face_byte;
    h[7] = 0; // flags
    // seq, little-endian
    h[8] = (seq & 0xFF) as u8;
    h[9] = ((seq >> 8) & 0xFF) as u8;
    h[10] = ((seq >> 16) & 0xFF) as u8;
    h[11] = ((seq >> 24) & 0xFF) as u8;
    // reserved stays 0
    h
}

#[test]
fn exported_constants_match_the_table() {
    assert_eq!(HEADER_BYTES, 16);
    assert_eq!(cube_proto::MAGIC, b"CUBE");
    assert_eq!(cube_proto::VERSION, 1);
    assert_eq!(cube_proto::NO_FACE, 0xFF);
    assert_eq!(Format::FullFrame.code(), 0);
    assert_eq!(Format::SingleFace.code(), 1);
    assert_eq!(Format::FullFrame.payload_len(), FRAME_BYTES);
    assert_eq!(Format::SingleFace.payload_len(), FACE_BYTES);
}

#[test]
fn full_frame_header_is_byte_for_byte_per_the_doc() {
    // The doctest in clients/python/cubeclient.py spells the seq = 1 header out:
    // 43 55 42 45 01 00 ff 00 01 00 00 00 00 00 00 00
    let literal = [
        0x43, 0x55, 0x42, 0x45, 0x01, 0x00, 0xff, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];
    assert_eq!(doc_header(0, 0xFF, 1), literal, "my table reading is wrong");

    for seq in [0u32, 1, 2, 255, 256, 0x0102_0304, 0xDEAD_BEEF, u32::MAX] {
        let mut buf = Vec::new();
        encode_full(&Frame::black(), seq, &mut buf);
        assert_eq!(buf.len(), 16 + 61440, "full datagram size for seq {seq}");
        assert_eq!(buf.len(), 61456);
        assert_eq!(
            &buf[..16],
            &doc_header(0, 0xFF, seq),
            "full-frame header for seq {seq}"
        );
    }
}

#[test]
fn single_face_header_is_byte_for_byte_per_the_doc() {
    for (fi, face) in Face::ALL.into_iter().enumerate() {
        for seq in [1u32, 0x0102_0304, u32::MAX] {
            let mut buf = Vec::new();
            encode_face(&Frame::black(), face, seq, &mut buf);
            assert_eq!(buf.len(), 16 + 12288, "face datagram size");
            assert_eq!(buf.len(), 12304);
            assert_eq!(
                &buf[..16],
                &doc_header(1, fi as u8, seq),
                "{face:?} header for seq {seq}"
            );
        }
    }
}

#[test]
fn encoders_clear_the_output_buffer_first() {
    let mut buf = vec![0xAAu8; 99];
    encode_full(&Frame::black(), 5, &mut buf);
    assert_eq!(buf.len(), 61456);
    encode_face(&Frame::black(), Face::Top, 5, &mut buf);
    assert_eq!(buf.len(), 12304);
}

#[test]
fn payloads_carry_the_right_bytes() {
    let mut frame = Frame::black();
    frame.set(Face::Back, 3, 4, [1, 2, 3]);
    frame.set(Face::Top, 63, 63, [10, 20, 30]);

    let mut buf = Vec::new();
    encode_full(&frame, 7, &mut buf);
    assert_eq!(&buf[16..], frame.as_bytes().as_slice());
    // Back (index 2) pixel (3,4) is at payload byte 2*12288 + (4*64+3)*3.
    let o = 16 + 2 * 12288 + (4 * 64 + 3) * 3;
    assert_eq!(&buf[o..o + 3], &[1, 2, 3]);

    encode_face(&frame, Face::Top, 8, &mut buf);
    assert_eq!(&buf[16..], frame.face(Face::Top));
    let o = 16 + (63 * 64 + 63) * 3;
    assert_eq!(&buf[o..o + 3], &[10, 20, 30]);
}

#[test]
fn decode_accepts_valid_full_and_single_face_datagrams() {
    let mut frame = Frame::black();
    frame.set(Face::Right, 1, 2, [4, 5, 6]);

    let mut buf = Vec::new();
    encode_full(&frame, 42, &mut buf);
    let (h, payload) = decode(&buf).expect("valid full frame");
    assert_eq!(
        h,
        Header {
            format: Format::FullFrame,
            face: None,
            seq: 42
        }
    );
    assert_eq!(payload.len(), FRAME_BYTES);
    assert_eq!(payload, frame.as_bytes().as_slice());

    for (fi, face) in Face::ALL.into_iter().enumerate() {
        encode_face(&frame, face, 1000 + fi as u32, &mut buf);
        let (h, payload) = decode(&buf).expect("valid single face");
        assert_eq!(
            h,
            Header {
                format: Format::SingleFace,
                face: Some(face),
                seq: 1000 + fi as u32
            }
        );
        assert_eq!(payload.len(), FACE_BYTES);
        assert_eq!(payload, frame.face(face));
    }
}

fn good_full() -> Vec<u8> {
    let mut v = Vec::new();
    encode_full(&Frame::black(), 1, &mut v);
    v
}

fn good_face() -> Vec<u8> {
    let mut v = Vec::new();
    encode_face(&Frame::black(), Face::Front, 1, &mut v);
    v
}

#[test]
fn decode_rejects_bad_magic() {
    for i in 0..4 {
        let mut d = good_full();
        d[i] ^= 0xFF;
        assert!(
            matches!(decode(&d), Err(ProtoError::BadMagic(_))),
            "byte {i} of the magic was corrupted but decode accepted it"
        );
    }
}

#[test]
fn decode_rejects_wrong_version() {
    for v in [0u8, 2, 0xFF] {
        let mut d = good_full();
        d[4] = v;
        assert_eq!(decode(&d), Err(ProtoError::BadVersion(v)));
    }
}

#[test]
fn decode_rejects_unknown_format() {
    for f in [2u8, 3, 0xFF] {
        let mut d = good_full();
        d[5] = f;
        assert_eq!(decode(&d), Err(ProtoError::BadFormat(f)));
    }
}

#[test]
fn decode_rejects_format0_with_a_face_byte_other_than_0xff() {
    for face_byte in [0u8, 1, 4, 5, 0xFE] {
        let mut d = good_full();
        d[6] = face_byte;
        assert_eq!(
            decode(&d),
            Err(ProtoError::BadFace(face_byte)),
            "full frame must carry face byte 0xFF, got {face_byte:#04x}"
        );
    }
}

#[test]
fn decode_rejects_format1_with_face_index_5_or_more() {
    for face_byte in [5u8, 6, 0xFE, 0xFF] {
        let mut d = good_face();
        d[6] = face_byte;
        assert_eq!(decode(&d), Err(ProtoError::BadFace(face_byte)));
    }
    // …and accepts 0..=4.
    for face_byte in 0u8..5 {
        let mut d = good_face();
        d[6] = face_byte;
        let (h, _) = decode(&d).expect("face index in range");
        assert_eq!(h.face, Face::from_index(face_byte));
    }
}

#[test]
fn decode_rejects_nonzero_flags() {
    for flags in [1u8, 0x80, 0xFF] {
        let mut d = good_full();
        d[7] = flags;
        assert_eq!(decode(&d), Err(ProtoError::BadFlags(flags)));
    }
}

#[test]
fn decode_accepts_nonzero_reserved_bytes_for_forward_compat() {
    for i in 12..16 {
        let mut d = good_full();
        d[i] = 0xA5;
        let (h, payload) = decode(&d).expect("reserved bytes must be ignored on receive");
        assert_eq!(h.seq, 1);
        assert_eq!(payload.len(), FRAME_BYTES);
    }
    let mut d = good_face();
    d[12..16].copy_from_slice(&[1, 2, 3, 4]);
    assert!(decode(&d).is_ok());
}

#[test]
fn decode_rejects_payload_off_by_one_in_either_direction() {
    let d = good_full();
    assert_eq!(
        decode(&d[..d.len() - 1]),
        Err(ProtoError::BadPayloadLen {
            expected: FRAME_BYTES,
            got: FRAME_BYTES - 1
        })
    );
    let mut long = d.clone();
    long.push(0);
    assert_eq!(
        decode(&long),
        Err(ProtoError::BadPayloadLen {
            expected: FRAME_BYTES,
            got: FRAME_BYTES + 1
        })
    );

    let d = good_face();
    assert_eq!(
        decode(&d[..d.len() - 1]),
        Err(ProtoError::BadPayloadLen {
            expected: FACE_BYTES,
            got: FACE_BYTES - 1
        })
    );
    let mut long = d.clone();
    long.push(0);
    assert_eq!(
        decode(&long),
        Err(ProtoError::BadPayloadLen {
            expected: FACE_BYTES,
            got: FACE_BYTES + 1
        })
    );
    // A full-frame payload in a single-face datagram is also wrong.
    let mut mixed = good_full();
    mixed[5] = 1;
    mixed[6] = 0;
    assert_eq!(
        decode(&mixed),
        Err(ProtoError::BadPayloadLen {
            expected: FACE_BYTES,
            got: FRAME_BYTES
        })
    );
}

#[test]
fn decode_rejects_datagrams_shorter_than_the_header() {
    let d = good_full();
    for len in 0..HEADER_BYTES {
        assert_eq!(
            decode(&d[..len]),
            Err(ProtoError::TooShort { len }),
            "a {len}-byte datagram must be rejected as too short"
        );
    }
    // Exactly HEADER_BYTES is long enough to parse, but has an empty payload.
    assert_eq!(
        decode(&d[..HEADER_BYTES]),
        Err(ProtoError::BadPayloadLen {
            expected: FRAME_BYTES,
            got: 0
        })
    );
}

#[test]
fn seq_is_newer_handles_wraparound_equality_and_backward_jumps() {
    assert!(seq_is_newer(2, 1));
    assert!(seq_is_newer(1, 0));
    assert!(!seq_is_newer(1, 1), "equal is not newer");
    assert!(!seq_is_newer(0, 0));
    assert!(!seq_is_newer(u32::MAX, u32::MAX));

    // Wraparound: u32::MAX → 0 → 1 are all newer.
    assert!(seq_is_newer(0, u32::MAX));
    assert!(seq_is_newer(1, u32::MAX));
    assert!(seq_is_newer(1_000, u32::MAX - 5));

    // Large backward jumps are not newer.
    assert!(!seq_is_newer(u32::MAX, 0));
    assert!(!seq_is_newer(1, 1_000_000));
    assert!(!seq_is_newer(u32::MAX - 5, 1_000));

    // Half-range boundary: exactly 2^31 ahead is ambiguous and must not be "newer";
    // one less is newer.
    assert!(seq_is_newer(0x7FFF_FFFF, 0));
    assert!(!seq_is_newer(0x8000_0000, 0));
}
