//! FW-6: the raster on the wire, written from `design/flat-world-plan-2026-09-16.md` §3
//! (`FrameSink::submit(Output<'_>)`, `ShimSink` "sending a `Raster` in wire format 2
//! (strips)", the PNG and web sinks, and the `PreviewSink` refusal).
//!
//! **Status.** FW-4 has not landed: there is no `Output` enum, no raster sink and no
//! `topology` in `/status`. What is written here is the part that exists — FW-0's vendored
//! `cube-proto`, whose format 2 is what every ring sink will put on the wire — as the brief
//! asks. The rest is a `pending FW-4` list at the foot of the file.

use cube_proto::{
    Format, HEADER_BYTES, MAX_DATAGRAM, ProtoError, Raster, STRIP_HEADER_BYTES, Strip, decode,
    decode_strip, encode_raster,
};

const W: u16 = 320;
const H: u16 = 180;

/// A deterministic image whose every pixel is distinguishable from every other.
fn painted(w: u16, h: u16) -> Raster {
    let mut raster = Raster::black(w, h);
    for y in 0..h {
        for x in 0..w {
            let i = u32::from(y) * u32::from(w) + u32::from(x);
            raster.set(x, y, [(i & 0xFF) as u8, ((i >> 8) & 0xFF) as u8, (x as u8) ^ (y as u8)]);
        }
    }
    raster
}

/// Reassemble an image from its datagrams the way a receiver would, checking every rule the
/// format states along the way.
fn reassemble(datagrams: &[Vec<u8>], seq: u32) -> Raster {
    let mut image: Option<Raster> = None;
    let mut rows_seen: Vec<u16> = Vec::new();
    for dg in datagrams {
        let (header, payload) = decode(dg).expect("a strip decodes");
        assert_eq!(header.format, Format::RasterStrip, "format 2");
        assert_eq!(header.face, None, "a raster names no face");
        assert_eq!(header.seq, seq, "every strip of one image carries the same seq");

        let (strip, pixels) = decode_strip(payload).expect("the strip header parses");
        strip.validate().expect("the geometry is legal");
        assert_eq!(pixels.len(), strip.pixel_bytes(), "rows · width · 3");
        assert_eq!(dg.len(), strip.datagram_len());
        assert!(dg.len() <= MAX_DATAGRAM, "a strip fits in one datagram");

        let out = image.get_or_insert_with(|| Raster::black(strip.width, strip.height));
        assert_eq!(
            out.size(),
            (strip.width, strip.height),
            "every strip describes the whole image, so a receiver can size its buffer from any one"
        );
        for r in 0..strip.rows {
            let y = strip.y0 + r;
            assert!(!rows_seen.contains(&y), "row {y} arrived twice");
            rows_seen.push(y);
            let row_bytes = out.row_bytes();
            let from = usize::from(r) * row_bytes;
            out.rows_mut(y, 1).expect("in range").copy_from_slice(&pixels[from..][..row_bytes]);
        }
    }
    let out = image.expect("at least one strip");
    assert_eq!(rows_seen.len(), usize::from(out.height()), "every row arrived exactly once");
    out
}

#[test]
fn a_320_by_180_raster_round_trips_through_format_2() {
    let raster = painted(W, H);
    let mut datagrams = Vec::new();
    encode_raster(&raster, 7, MAX_DATAGRAM, &mut datagrams).expect("encodes");
    // 320 × 180 × 3 = 172,800 bytes, well over the 65,507-byte datagram, so the image goes
    // out as three strips of 68, 68 and 44 rows. (§3's "a 320×180 image goes out in one" is
    // wrong: one *row* is 960 bytes and only 68 of them fit.)
    assert_eq!(datagrams.len(), 3, "172,800 bytes cannot fit in one 65,507-byte datagram");
    let rows: Vec<u16> = datagrams
        .iter()
        .map(|dg| decode_strip(decode(dg).expect("decodes").1).expect("strip").0.rows)
        .collect();
    assert_eq!(rows, vec![68, 68, 44], "68 rows per strip, then the remainder");

    let back = reassemble(&datagrams, 7);
    assert_eq!(back.size(), (W, H));
    assert_eq!(back.as_bytes(), raster.as_bytes(), "byte for byte");
}

#[test]
fn a_larger_raster_splits_into_strips_that_reassemble_exactly() {
    for (w, h) in [(640u16, 360u16), (1920, 1080), (320, 181)] {
        let raster = painted(w, h);
        let mut datagrams = Vec::new();
        encode_raster(&raster, 42, MAX_DATAGRAM, &mut datagrams).expect("encodes");
        assert!(!datagrams.is_empty());
        let back = reassemble(&datagrams, 42);
        assert_eq!(back.size(), (w, h), "{w}×{h}");
        assert_eq!(back.as_bytes(), raster.as_bytes(), "{w}×{h} byte for byte");
    }
}

/// The split is by whole rows and takes the largest number that still fits, so the only
/// short strip is the last one.
#[test]
fn every_strip_but_the_last_carries_the_same_number_of_rows() {
    let raster = painted(1920, 1080);
    let mut datagrams = Vec::new();
    encode_raster(&raster, 1, MAX_DATAGRAM, &mut datagrams).expect("encodes");
    let strips: Vec<Strip> = datagrams
        .iter()
        .map(|dg| decode_strip(decode(dg).expect("decodes").1).expect("strip").0)
        .collect();
    assert!(strips.len() > 1, "a 1920×1080 image needs several datagrams");
    let full = strips[0].rows;
    for (i, s) in strips.iter().enumerate() {
        assert_eq!(
            s.y0,
            strips[..i].iter().map(|p| p.rows).sum::<u16>(),
            "strip {i} starts where {} ended",
            i.saturating_sub(1)
        );
        if i + 1 < strips.len() {
            assert_eq!(s.rows, full, "strip {i} is a full strip");
        } else {
            assert!(s.rows <= full && s.rows > 0, "the last strip is the remainder");
        }
    }
    // The largest whole number of rows that fits: the budget is the datagram less both headers.
    let row_bytes = raster.row_bytes();
    assert_eq!(full, ((MAX_DATAGRAM - HEADER_BYTES - STRIP_HEADER_BYTES) / row_bytes) as u16);
}

/// A caller-chosen limit too small for a single row is the one failure the encoder has.
#[test]
fn a_datagram_limit_below_one_row_is_refused() {
    let raster = painted(W, H);
    let mut datagrams = Vec::new();
    let row = raster.row_bytes();
    assert_eq!(
        encode_raster(&raster, 0, HEADER_BYTES + STRIP_HEADER_BYTES + row - 1, &mut datagrams),
        Err(ProtoError::DatagramTooLarge {
            len: HEADER_BYTES + STRIP_HEADER_BYTES + row,
            max: HEADER_BYTES + STRIP_HEADER_BYTES + row - 1
        })
    );
    // Exactly one row's worth is enough, and then there is one strip per row.
    encode_raster(&raster, 0, HEADER_BYTES + STRIP_HEADER_BYTES + row, &mut datagrams)
        .expect("fits");
    assert_eq!(datagrams.len(), usize::from(H), "one strip per row");
    assert_eq!(reassemble(&datagrams, 0).as_bytes(), raster.as_bytes());
}

/// Hostile geometry is refused rather than trusted: the decoder checks the strip's own
/// header before it believes any of its bytes.
#[test]
fn a_strip_with_impossible_geometry_is_refused() {
    let bad = [
        Strip { width: 0, height: 180, y0: 0, rows: 1 },
        Strip { width: 320, height: 0, y0: 0, rows: 1 },
        Strip { width: 320, height: 180, y0: 0, rows: 0 },
        Strip { width: 320, height: 180, y0: 179, rows: 2 },
        Strip { width: 320, height: 180, y0: 180, rows: 1 },
        Strip { width: 5000, height: 180, y0: 0, rows: 1 },
    ];
    for strip in bad {
        assert!(strip.validate().is_err(), "{strip:?} must be refused");
    }
    assert!(Strip { width: 320, height: 180, y0: 179, rows: 1 }.validate().is_ok(), "the last row");

    // And a payload whose length disagrees with its header is refused too.
    let raster = painted(8, 4);
    let mut datagrams = Vec::new();
    encode_raster(&raster, 0, MAX_DATAGRAM, &mut datagrams).expect("encodes");
    let (_, payload) = decode(&datagrams[0]).expect("decodes");
    let mut truncated = payload.to_vec();
    truncated.pop();
    assert!(decode_strip(&truncated).is_err(), "a short payload is refused");
}

/// Format 2 is a third format beside the two cube ones, not a replacement: a cube frame
/// still encodes as format 0.
#[test]
fn the_raster_format_is_the_third_one() {
    assert_eq!(Format::RasterStrip.code(), 2);
    assert_eq!(Format::FullFrame.code(), 0);
    assert_eq!(Format::SingleFace.code(), 1);
    assert_eq!(Format::RasterStrip.fixed_payload_len(), None, "a strip is variable length");
    assert_eq!(Format::RasterStrip.payload_len(), STRIP_HEADER_BYTES, "and that is its minimum");
}

// --- pending FW-4 -----------------------------------------------------------------
//
// Not written yet, because the interface does not exist in this worktree:
//
// * `enum Output<'a> { Cube(&'a Frame), Ring(&'a Raster) }` and
//   `FrameSink::submit(&mut self, out: Output<'_>)`, with the trait still object-safe —
//   `FanOutSink(Vec<Box<dyn FrameSink>>)` must still compile and still fan out;
// * `ShimSink` sending a ring world as format 2 strips through the same newest-frame
//   mailbox and backoff, with the cube path unchanged;
// * `PngSink` writing a `w×h` PNG for a ring and leaving `net.rs` untouched for the cube;
// * `WebSink`: `/frame` = 8-byte seq followed by the raster bytes, and `/status` carrying
//   `"topology"` so the page can pick its mode;
// * `PreviewSink` **refusing** a ring world with a clear error, and `--sink preview` with a
//   ring topology refused at argument validation next to the `--fresh`/`--require-resume`
//   check;
// * `open_world` refusing a resume whose `--config` topology differs from the snapshot's,
//   by name, like every other schema refusal;
// * the TOML: `topology = "cube"` and `topology = { ring = { w = 320, h = 180 } }`, plus
//   `world_scale`.
