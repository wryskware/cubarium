//! The raster strip format (format 2): round trip, every validity rule, and the
//! cutting of an image into datagrams at a size limit.
//!
//! The rule this file exists to protect is that format 2 is **additive**: formats 0
//! and 1 must encode and decode byte-for-byte as they did before it existed. That is
//! asserted here directly as well as by `wire_format.rs` and `python_interop.rs`.

use cube_proto::{
    decode, decode_strip, encode_face, encode_full, encode_raster, Face, Format, Frame, ProtoError,
    Raster, Strip, HEADER_BYTES, MAX_DATAGRAM, MAX_RASTER_DIM, STRIP_HEADER_BYTES,
};

/// A raster whose every pixel is a function of its position, so a misplaced strip or
/// a transposed axis cannot pass unnoticed.
fn patterned(w: u16, h: u16) -> Raster {
    let mut r = Raster::black(w, h);
    for y in 0..h {
        for x in 0..w {
            r.set(
                x,
                y,
                [(x & 0xff) as u8, (y & 0xff) as u8, ((x ^ y) & 0xff) as u8],
            );
        }
    }
    r
}

/// Reassemble an encoded image from its strips, checking each one as it lands.
fn reassemble(datagrams: &[Vec<u8>], seq: u32) -> Raster {
    let mut out: Option<Raster> = None;
    let mut next_y0 = 0u16;
    for dg in datagrams {
        assert!(
            dg.len() <= MAX_DATAGRAM,
            "a datagram of {} bytes exceeds the UDP limit",
            dg.len()
        );
        let (h, payload) = decode(dg).expect("every produced datagram must decode");
        assert_eq!(h.format, Format::RasterStrip);
        assert_eq!(h.face, None);
        assert_eq!(h.seq, seq, "every strip of one image carries the same seq");

        let (strip, pixels) = decode_strip(payload).expect("the strip header must be valid");
        assert_eq!(strip.y0, next_y0, "strips must tile in order with no gap");
        assert_eq!(pixels.len(), strip.pixel_bytes());
        assert_eq!(payload.len(), strip.payload_len());
        assert_eq!(dg.len(), strip.datagram_len());

        let r = out.get_or_insert_with(|| Raster::black(strip.width, strip.height));
        assert_eq!(r.size(), (strip.width, strip.height), "size changed mid-image");
        r.rows_mut(strip.y0, strip.rows)
            .expect("the strip fits")
            .copy_from_slice(pixels);
        next_y0 += strip.rows;
    }
    let r = out.expect("at least one strip");
    assert_eq!(next_y0, r.height(), "the strips must cover every row");
    r
}

// ------------------------------------------------------------------- round trip

#[test]
fn the_shipped_hint_size_round_trips_in_three_datagrams() {
    // 320x180 is the Tachyon's configured raster size: a row is 960 B, so 68 rows
    // fit in a datagram and the image needs 68 + 68 + 44.
    let src = patterned(320, 180);
    let mut out = Vec::new();
    encode_raster(&src, 42, MAX_DATAGRAM, &mut out).expect("320x180 encodes");
    let rows: Vec<u16> = out
        .iter()
        .map(|d| decode_strip(decode(d).unwrap().1).unwrap().0.rows)
        .collect();
    assert_eq!(rows, vec![68, 68, 44]);
    assert_eq!(reassemble(&out, 42), src);
}

#[test]
fn an_image_that_fits_entirely_is_one_datagram() {
    // 64 x 32 is 6,144 B of pixels: comfortably one.
    let src = patterned(64, 32);
    let mut out = Vec::new();
    encode_raster(&src, 3, MAX_DATAGRAM, &mut out).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(reassemble(&out, 3), src);
}

#[test]
fn a_large_image_round_trips_across_many_datagrams() {
    // 1920x1080 is 6.2 MB: definitely many strips.
    let src = patterned(1920, 1080);
    let mut out = Vec::new();
    encode_raster(&src, 7, MAX_DATAGRAM, &mut out).expect("1920x1080 encodes");
    // A row is 5,760 B, so 11 rows fit: 98 full strips and a 2-row remainder.
    assert_eq!(out.len(), 99, "expected 99 strips, got {}", out.len());
    assert_eq!(reassemble(&out, 7), src);
}

#[test]
fn the_extreme_shapes_round_trip() {
    for (w, h) in [
        (1u16, 1u16),
        (1, MAX_RASTER_DIM),
        (MAX_RASTER_DIM, 1),
        (MAX_RASTER_DIM, 7),
        (3, 4096),
    ] {
        let src = patterned(w, h);
        let mut out = Vec::new();
        encode_raster(&src, 1, MAX_DATAGRAM, &mut out).unwrap_or_else(|e| panic!("{w}x{h}: {e}"));
        assert_eq!(reassemble(&out, 1), src, "{w}x{h}");
    }
}

// --------------------------------------------------------------- strip cutting

#[test]
fn strips_take_the_largest_row_count_that_fits() {
    // One row of a 4096-wide image is 12,288 B; with 24 B of headers, five rows fit
    // in 65,507 and six do not.
    let src = patterned(MAX_RASTER_DIM, 12);
    let mut out = Vec::new();
    encode_raster(&src, 1, MAX_DATAGRAM, &mut out).unwrap();
    let rows: Vec<u16> = out
        .iter()
        .map(|d| decode_strip(decode(d).unwrap().1).unwrap().0.rows)
        .collect();
    assert_eq!(rows, vec![5, 5, 2], "full strips first, remainder last");
    for d in &out[..2] {
        assert_eq!(d.len(), HEADER_BYTES + STRIP_HEADER_BYTES + 5 * 4096 * 3);
        assert!(d.len() <= MAX_DATAGRAM);
        // ...and one more row genuinely would not fit.
        assert!(d.len() + 4096 * 3 > MAX_DATAGRAM);
    }
}

#[test]
fn a_smaller_datagram_limit_cuts_smaller_strips() {
    let src = patterned(100, 20);
    let row = 100 * 3;
    for limit in [
        HEADER_BYTES + STRIP_HEADER_BYTES + row,     // exactly one row
        HEADER_BYTES + STRIP_HEADER_BYTES + row + 1, // still one row
        HEADER_BYTES + STRIP_HEADER_BYTES + 3 * row, // three rows
        MAX_DATAGRAM,
    ] {
        let mut out = Vec::new();
        encode_raster(&src, 5, limit, &mut out).unwrap_or_else(|e| panic!("limit {limit}: {e}"));
        for d in &out {
            assert!(d.len() <= limit, "limit {limit}: datagram of {} B", d.len());
        }
        assert_eq!(reassemble(&out, 5), src, "limit {limit}");
    }
}

#[test]
fn a_limit_too_small_for_one_row_is_an_error_not_a_bad_datagram() {
    let src = patterned(100, 20);
    let row = 100 * 3;
    let mut out = Vec::new();
    let err = encode_raster(&src, 1, HEADER_BYTES + STRIP_HEADER_BYTES + row - 1, &mut out)
        .expect_err("one row must not fit");
    assert!(
        matches!(err, ProtoError::DatagramTooLarge { .. }),
        "got {err:?}"
    );
    // Zero and tiny limits fail the same way rather than looping forever.
    assert!(encode_raster(&src, 1, 0, &mut out).is_err());
    assert!(encode_raster(&src, 1, 10, &mut out).is_err());
}

#[test]
fn encode_raster_clears_the_output_first() {
    let src = patterned(8, 8);
    let mut out = vec![vec![0xffu8; 4]; 9];
    encode_raster(&src, 1, MAX_DATAGRAM, &mut out).unwrap();
    assert_eq!(out.len(), 1, "stale datagrams must not survive");
    assert_eq!(reassemble(&out, 1), src);
}

// ------------------------------------------------------------- validity rules

/// A legal one-strip datagram for a `w`x`h` image, which the tests then corrupt.
fn strip_datagram(w: u16, h: u16, y0: u16, rows: u16) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(b"CUBE");
    d.push(1); // version
    d.push(2); // format: raster strip
    d.push(0xFF); // face
    d.push(0); // flags
    d.extend_from_slice(&1u32.to_le_bytes()); // seq
    d.extend_from_slice(&0u32.to_le_bytes()); // reserved
    d.extend_from_slice(&w.to_le_bytes());
    d.extend_from_slice(&h.to_le_bytes());
    d.extend_from_slice(&y0.to_le_bytes());
    d.extend_from_slice(&rows.to_le_bytes());
    d.resize(d.len() + rows as usize * w as usize * 3, 0);
    d
}

#[test]
fn a_hand_built_strip_datagram_decodes() {
    let d = strip_datagram(4, 3, 1, 2);
    assert_eq!(d.len(), HEADER_BYTES + STRIP_HEADER_BYTES + 2 * 4 * 3);
    let (h, payload) = decode(&d).expect("a well-formed strip decodes");
    assert_eq!(h.format, Format::RasterStrip);
    assert_eq!(h.seq, 1);
    let (strip, pixels) = decode_strip(payload).unwrap();
    assert_eq!(
        strip,
        Strip {
            width: 4,
            height: 3,
            y0: 1,
            rows: 2
        }
    );
    assert_eq!(pixels.len(), 24);
}

#[test]
fn a_zero_or_oversized_image_side_is_refused() {
    for (w, h) in [
        (0u16, 4u16),
        (4, 0),
        (0, 0),
        (MAX_RASTER_DIM + 1, 4),
        (4, MAX_RASTER_DIM + 1),
    ] {
        let d = strip_datagram(w.max(1), h.max(1), 0, 1);
        // Rebuild the geometry fields with the illegal values, keeping the payload.
        let mut d = d;
        d[HEADER_BYTES..HEADER_BYTES + 2].copy_from_slice(&w.to_le_bytes());
        d[HEADER_BYTES + 2..HEADER_BYTES + 4].copy_from_slice(&h.to_le_bytes());
        match decode(&d) {
            Err(ProtoError::BadRasterSize { width, height }) => {
                assert_eq!((width, height), (w, h));
            }
            // A changed width also changes the expected payload length; either
            // complaint is a refusal, which is what matters.
            Err(ProtoError::BadPayloadLen { .. }) => {}
            other => panic!("{w}x{h} was not refused: {other:?}"),
        }
    }
}

#[test]
fn zero_rows_is_refused() {
    let d = strip_datagram(4, 3, 0, 0);
    assert_eq!(
        decode(&d),
        Err(ProtoError::BadStripRange {
            y0: 0,
            rows: 0,
            height: 3
        })
    );
}

#[test]
fn a_strip_that_runs_past_the_bottom_is_refused() {
    // y0 + rows must be <= height.
    for (y0, rows) in [(2u16, 2u16), (3, 1), (0, 4), (300, 1)] {
        let d = strip_datagram(4, 3, y0, rows);
        assert_eq!(
            decode(&d),
            Err(ProtoError::BadStripRange {
                y0,
                rows,
                height: 3
            }),
            "y0 {y0} rows {rows}"
        );
    }
    // The exactly-fitting cases are legal.
    for (y0, rows) in [(0u16, 3u16), (1, 2), (2, 1)] {
        assert!(decode(&strip_datagram(4, 3, y0, rows)).is_ok(), "y0 {y0} rows {rows}");
    }
}

#[test]
fn a_payload_that_does_not_match_the_strip_header_is_refused() {
    let good = strip_datagram(4, 3, 0, 2);
    let want = STRIP_HEADER_BYTES + 2 * 4 * 3;

    let short = &good[..good.len() - 1];
    assert_eq!(
        decode(short),
        Err(ProtoError::BadPayloadLen {
            expected: want,
            got: want - 1
        })
    );

    let mut long = good.clone();
    long.push(0);
    assert_eq!(
        decode(&long),
        Err(ProtoError::BadPayloadLen {
            expected: want,
            got: want + 1
        })
    );

    // A payload too short to even hold the strip header.
    let stub = &good[..HEADER_BYTES + 4];
    assert_eq!(
        decode(stub),
        Err(ProtoError::BadPayloadLen {
            expected: STRIP_HEADER_BYTES,
            got: 4
        })
    );
}

#[test]
fn a_raster_strip_must_carry_the_no_face_byte() {
    let mut d = strip_datagram(4, 3, 0, 1);
    d[6] = 0;
    assert_eq!(decode(&d), Err(ProtoError::BadFace(0)));
    d[6] = 4;
    assert_eq!(decode(&d), Err(ProtoError::BadFace(4)));
}

#[test]
fn the_shared_header_rules_still_apply_to_format_2() {
    let good = strip_datagram(4, 3, 0, 1);

    let mut d = good.clone();
    d[0] = b'X';
    assert!(matches!(decode(&d), Err(ProtoError::BadMagic(_))));

    let mut d = good.clone();
    d[4] = 2;
    assert_eq!(decode(&d), Err(ProtoError::BadVersion(2)));

    let mut d = good.clone();
    d[7] = 1;
    assert_eq!(decode(&d), Err(ProtoError::BadFlags(1)));

    // The reserved word is still ignored on receive.
    let mut d = good.clone();
    d[12..16].copy_from_slice(&0xdead_beefu32.to_le_bytes());
    assert!(decode(&d).is_ok(), "the reserved word must stay ignored");
}

#[test]
fn a_datagram_over_the_udp_limit_is_refused() {
    // 4096 wide x 6 rows = 73,728 B of pixels: larger than one UDP payload. The
    // encoder will never produce this; a hostile or buggy sender might.
    let d = strip_datagram(MAX_RASTER_DIM, 6, 0, 6);
    assert!(d.len() > MAX_DATAGRAM);
    assert_eq!(
        decode(&d),
        Err(ProtoError::DatagramTooLarge {
            len: d.len(),
            max: MAX_DATAGRAM
        })
    );
}

// ------------------------------------------------- formats 0 and 1 are untouched

#[test]
fn the_cube_formats_are_byte_identical_to_what_they_always_were() {
    let mut frame = Frame::black();
    frame.set(Face::Top, 1, 2, [9, 8, 7]);

    let mut buf = Vec::new();
    encode_full(&frame, 1, &mut buf);
    assert_eq!(
        &buf[..HEADER_BYTES],
        &[0x43, 0x55, 0x42, 0x45, 0x01, 0x00, 0xff, 0x00, 0x01, 0x00, 0x00, 0x00, 0, 0, 0, 0]
    );
    assert_eq!(buf.len(), HEADER_BYTES + 61440);
    let (h, payload) = decode(&buf).unwrap();
    assert_eq!(h.format, Format::FullFrame);
    assert_eq!(payload, frame.as_bytes().as_slice());

    encode_face(&frame, Face::Top, 2, &mut buf);
    assert_eq!(
        &buf[..HEADER_BYTES],
        // version 1, format 1 (single face), face 4 (Top), flags 0, seq 2.
        &[0x43, 0x55, 0x42, 0x45, 0x01, 0x01, 0x04, 0x00, 0x02, 0x00, 0x00, 0x00, 0, 0, 0, 0]
    );
    assert_eq!(buf.len(), HEADER_BYTES + 12288);
    let (h, payload) = decode(&buf).unwrap();
    assert_eq!(h.format, Format::SingleFace);
    assert_eq!(h.face, Some(Face::Top));
    assert_eq!(payload, frame.face(Face::Top));

    // And the fixed lengths are still what the formats promise.
    assert_eq!(Format::FullFrame.fixed_payload_len(), Some(61440));
    assert_eq!(Format::SingleFace.fixed_payload_len(), Some(12288));
    assert_eq!(Format::RasterStrip.fixed_payload_len(), None);
    assert_eq!(Format::RasterStrip.code(), 2);
}
