//! FW-6: the canvas by topology, written from `design/flat-world-plan-2026-09-16.md` §3
//! ("`Canvas::new(topo)`, a flat `Vec<[f32;3]>` of `w·h`, same `get/set/add` on
//! `(Face::Front, x, y)` … one new `Canvas::pixels()` … add
//! `Canvas::encode_raster(&mut Raster)`; sRGB encode is shared and untouched"), §9's FW-3
//! row ("same-seed cube canvas bit-identical", "deterministic row-band parallel hook")
//! and the FW-1 freeze's `u16` pixel indices.

use cubarium_render::{Canvas, srgb_decode, srgb_encode};
use cubarium_surface::{Scale, Topology};
use cube_proto::{FACE_SIZE, Face, Frame, Raster};

const W: u16 = 320;
const H: u16 = 180;

fn ring() -> Topology {
    Topology::Ring { w: W, h: H }
}

/// The sRGB transfer function (IEC 61966-2-1), written out here so the comparison is
/// against the standard and not against the crate's own arithmetic or its lookup table.
fn srgb_reference(linear: f32) -> u8 {
    if !linear.is_finite() || linear <= 0.0 {
        return 0;
    }
    let l = f64::from(linear).min(1.0);
    let e = if l <= 0.003_130_8 {
        12.92 * l
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0).round() as u8
}

/// A deterministic spread of linear values, including out-of-range and NaN.
fn pattern(i: usize) -> [f32; 3] {
    let mut s = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        ((s >> 11) as f64 / 9_007_199_254_740_992.0) as f32
    };
    let raw = [next() * 1.2 - 0.1, next(), next() * 0.004];
    if i % 997 == 0 {
        [f32::NAN, raw[1], raw[2]]
    } else {
        raw
    }
}

/// Paint every pixel the canvas owns, in `coords()` order.
fn paint(canvas: &mut Canvas) {
    for (i, (face, x, y)) in canvas.coords().collect::<Vec<_>>().into_iter().enumerate() {
        canvas.set(face, x, y, pattern(i));
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------------------
// Shape
// ---------------------------------------------------------------------------

/// §3: a ring canvas is one `w×h` chart, addressed as `(Face::Front, x, y)`.
#[test]
fn a_ring_canvas_is_one_chart_of_w_by_h() {
    let canvas = Canvas::new(ring(), Scale::ONE);
    assert_eq!(canvas.topology(), ring());
    assert_eq!(canvas.scale(), Scale::ONE);
    assert_eq!((canvas.width(), canvas.height()), (W, H));
    assert_eq!(canvas.charts(), &[Face::Front], "one chart");
    assert_eq!(
        canvas.pixels().len(),
        usize::from(W) * usize::from(H),
        "57,600 pixels"
    );
    assert!(
        canvas.pixels().iter().all(|p| *p == [0.0; 3]),
        "a new canvas is black"
    );
}

/// The same canvas at S = 2 is the 640×360 raster: four times the pixels, same charts, and
/// it carries the stamp budget with it.
#[test]
fn the_double_scale_ring_canvas_is_640_by_360() {
    let canvas = Canvas::new(Topology::Ring { w: 640, h: 360 }, Scale::new(2.0));
    assert_eq!((canvas.width(), canvas.height()), (640, 360));
    assert_eq!(canvas.pixels().len(), 640 * 360);
    assert_eq!(
        canvas.scale().footprint_radius(),
        18.0,
        "9·S travels with the canvas"
    );
}

/// The cube canvas is unchanged: five 64×64 charts in `Face::ALL` order.
#[test]
fn the_cube_canvas_is_still_five_64_by_64_faces() {
    let canvas = Canvas::cube();
    assert_eq!(canvas.topology(), Topology::Cube);
    assert_eq!((canvas.width(), canvas.height()), (64, 64));
    assert_eq!(canvas.charts(), Face::ALL.as_slice());
    assert_eq!(
        canvas.pixels().len(),
        5 * FACE_SIZE * FACE_SIZE,
        "20,480 pixels"
    );
}

/// `coords()` replaces the 129 `for face { for y { for x` loops: every pixel of the
/// topology exactly once, in storage order.
#[test]
fn coords_visits_every_pixel_of_the_topology_exactly_once() {
    for canvas in [Canvas::new(ring(), Scale::ONE), Canvas::cube()] {
        let topo = canvas.topology();
        let seen: Vec<(Face, u16, u16)> = canvas.coords().collect();
        assert_eq!(
            seen.len(),
            canvas.pixels().len(),
            "{topo:?}: one coordinate per pixel"
        );
        let mut sorted = seen.clone();
        sorted.sort_by_key(|(f, x, y)| (f.index(), *y, *x));
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            seen.len(),
            "{topo:?}: a pixel was visited twice"
        );
        let (w, h) = (canvas.width(), canvas.height());
        let expected: Vec<(Face, u16, u16)> = canvas
            .charts()
            .iter()
            .flat_map(|&f| (0..h).flat_map(move |y| (0..w).map(move |x| (f, x, y))))
            .collect();
        assert_eq!(seen, expected, "{topo:?}: charts outermost, then y, then x");
    }
}

/// `get`/`set`/`add` take `u16` pixel indices (§2's forced widening: 320 > 255) and `add`
/// is a plain linear-light sum with no clamping before encode.
#[test]
fn get_set_and_add_are_u16_indexed_linear_light() {
    let mut canvas = Canvas::new(ring(), Scale::ONE);
    canvas.set(Face::Front, 0, 0, [0.25, 0.5, 0.75]);
    assert_eq!(canvas.get(Face::Front, 0, 0), [0.25, 0.5, 0.75]);
    canvas.add(Face::Front, 0, 0, [0.5, 0.5, 0.5]);
    assert_eq!(
        canvas.get(Face::Front, 0, 0),
        [0.75, 1.0, 1.25],
        "no clamp before encode"
    );

    // The last pixel of the last row, at an index no `u8` could hold.
    canvas.set(Face::Front, W - 1, H - 1, [1.0, 0.0, 0.0]);
    assert_eq!(canvas.get(Face::Front, W - 1, H - 1), [1.0, 0.0, 0.0]);
    assert_eq!(
        canvas.get(Face::Front, W - 2, H - 1),
        [0.0; 3],
        "no neighbour was touched"
    );
    let last = usize::from(H - 1) * usize::from(W) + usize::from(W - 1);
    assert_eq!(canvas.pixels()[last], [1.0, 0.0, 0.0], "row-major storage");

    canvas.clear();
    assert_eq!(canvas.get(Face::Front, 0, 0), [0.0; 3]);
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

/// The sRGB encode is the standard curve: clamp to `[0, 1]`, transfer, round. §3 says it
/// is "shared and untouched" between `encode` and `encode_raster`.
#[test]
fn srgb_encode_is_the_iec_61966_curve() {
    for v in 0..=255u8 {
        assert_eq!(
            srgb_encode(srgb_decode(v)),
            v,
            "decode/encode round trip at {v}"
        );
    }
    assert_eq!(srgb_encode(0.0), 0);
    assert_eq!(srgb_encode(1.0), 255);
    assert_eq!(srgb_encode(-0.5), 0, "below range clamps to 0");
    assert_eq!(srgb_encode(1.5), 255, "above range clamps to 255");
    assert_eq!(srgb_encode(f32::NAN), 0, "NaN encodes as 0");
    for i in 0..20_001 {
        let linear = (i as f32) / 10_000.0 - 0.5;
        assert_eq!(
            srgb_encode(linear),
            srgb_reference(linear),
            "at linear {linear}"
        );
    }
}

/// Every encoded byte of a fully painted cube canvas equals the reference transfer of the
/// value written there. §9 asks FW-3 for a bit-identical cube canvas; this pins the bytes
/// against the standard rather than against a snapshot.
#[test]
fn a_cube_canvas_encodes_exactly_the_reference_bytes() {
    let mut canvas = Canvas::cube();
    paint(&mut canvas);
    let mut frame = Frame::black();
    canvas.encode(&mut frame);

    for (i, (face, x, y)) in canvas.coords().enumerate() {
        let want = pattern(i).map(srgb_reference);
        assert_eq!(
            frame.get(face, usize::from(x), usize::from(y)),
            want,
            "{face:?} ({x}, {y})"
        );
    }
    let mut bytes = Vec::with_capacity(5 * FACE_SIZE * FACE_SIZE * 3);
    for i in 0..5 * FACE_SIZE * FACE_SIZE {
        bytes.extend_from_slice(&pattern(i).map(srgb_reference));
    }
    assert_eq!(
        fnv1a(frame.as_bytes()),
        fnv1a(&bytes),
        "frame byte layout and digest"
    );
}

/// `encode_raster` writes `w·h·3` bytes with the same transfer, 1:1 when the raster is the
/// canvas's own size.
#[test]
fn encode_raster_writes_the_same_bytes_at_the_canvas_size() {
    let mut canvas = Canvas::new(ring(), Scale::ONE);
    paint(&mut canvas);
    let mut raster = Raster::black(W, H);
    canvas.encode_raster(&mut raster);

    assert_eq!(
        raster.as_bytes().len(),
        usize::from(W) * usize::from(H) * 3,
        "w·h·3"
    );
    assert_eq!(raster.row_bytes(), usize::from(W) * 3);
    for (i, (_, x, y)) in canvas.coords().enumerate() {
        assert_eq!(
            raster.get(x, y),
            pattern(i).map(srgb_reference),
            "pixel ({x}, {y})"
        );
    }
}

/// A whole-number upscale repeats each pixel exactly and filters nothing — the property
/// the panel's integer upscale depends on.
#[test]
fn encode_raster_repeats_each_pixel_at_a_whole_number_upscale() {
    let mut canvas = Canvas::new(ring(), Scale::ONE);
    paint(&mut canvas);
    let mut small = Raster::black(W, H);
    canvas.encode_raster(&mut small);
    let mut big = Raster::black(W * 2, H * 2);
    canvas.encode_raster(&mut big);

    for y in 0..H {
        for x in 0..W {
            let want = small.get(x, y);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                assert_eq!(
                    big.get(x * 2 + dx, y * 2 + dy),
                    want,
                    "({x}, {y}) + ({dx}, {dy})"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The deterministic row-band split
// ---------------------------------------------------------------------------

/// A drawing pass that does not know about bands: discs that straddle the wrap and the
/// band cuts. Every band drops the writes outside its own rows.
fn draw_discs(canvas: &mut Canvas) {
    let (w, h) = (canvas.width(), canvas.height());
    let discs: [(i32, i32, i32, [f32; 3]); 6] = [
        (0, 0, 12, [0.10, 0.02, 0.00]),
        (30, 45, 30, [0.00, 0.11, 0.03]),
        (63, 63, 15, [0.02, 0.00, 0.13]),
        (45, 20, 9, [0.07, 0.07, 0.00]),
        (10, 40, 20, [0.00, 0.05, 0.05]),
        (33, 44, 5, [0.03, 0.00, 0.09]),
    ];
    for face in canvas.charts().to_vec() {
        for (cx, cy, r, rgb) in discs {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy > r * r {
                        continue;
                    }
                    let y = cy + dy;
                    if y < 0 || y >= i32::from(h) {
                        continue;
                    }
                    let x = (cx + dx).rem_euclid(i32::from(w));
                    canvas.add(face, x as u16, y as u16, rgb);
                }
            }
        }
    }
}

#[test]
fn a_band_split_composite_is_bit_identical_to_the_serial_one() {
    for topo in [ring(), Topology::Cube] {
        let mut serial = Canvas::new(topo, Scale::ONE);
        draw_discs(&mut serial);
        assert!(
            serial.pixels().iter().any(|p| *p != [0.0; 3]),
            "{topo:?}: the pass drew"
        );

        for n in [1usize, 2, 4] {
            let mut split = Canvas::new(topo, Scale::ONE);
            let mut bands = split.bands(n);
            assert_eq!(bands.len(), n, "{topo:?}: {n} bands");
            split.for_each_band(&mut bands, draw_discs);
            assert_eq!(
                split.pixels(),
                serial.pixels(),
                "{topo:?}: the {n}-band composite differs from serial"
            );

            let mut par = Canvas::new(topo, Scale::ONE);
            let mut bands = par.bands(n);
            par.par_each_band(&mut bands, draw_discs);
            assert_eq!(
                par.pixels(),
                serial.pixels(),
                "{topo:?}: the {n}-band parallel composite differs from serial"
            );
        }
    }
}

/// The bands partition the rows: contiguous, in order, as even as the row count allows,
/// cut in the same place every time, and each one drops writes outside its rows.
#[test]
fn bands_partition_the_rows_and_drop_writes_outside_them() {
    let canvas = Canvas::new(ring(), Scale::ONE);
    for n in [1usize, 2, 3, 4, 7] {
        let bands = canvas.bands(n);
        let mut next = 0u32;
        let mut sizes = Vec::new();
        for band in bands.iter() {
            let r = band.band();
            assert_eq!(r.start, next, "{n} bands: rows are contiguous and in order");
            assert!(r.end > r.start, "{n} bands: no empty band");
            sizes.push(r.end - r.start);
            next = r.end;
        }
        assert_eq!(
            next,
            u32::from(H),
            "{n} bands: the rows are covered exactly once"
        );
        let (lo, hi) = (sizes.iter().min().unwrap(), sizes.iter().max().unwrap());
        assert!(
            hi - lo <= 1,
            "{n} bands: as even as the row count allows, got {sizes:?}"
        );
        let again: Vec<u32> = canvas.bands(n).iter().map(|b| b.band().start).collect();
        let first: Vec<u32> = bands.iter().map(|b| b.band().start).collect();
        assert_eq!(again, first, "{n} bands: the cut is deterministic");
    }

    let mut bands = canvas.bands(2);
    let top = bands.iter_mut().next().expect("two bands");
    let below = top.band().end as u16;
    assert!(
        top.owns(Face::Front, below - 1),
        "the band claims its own last row"
    );
    assert!(!top.owns(Face::Front, below), "and not the next one");
    top.set(Face::Front, 5, below, [1.0, 1.0, 1.0]);
    assert!(
        top.pixels().iter().all(|p| *p == [0.0; 3]),
        "a write below the band's last row must be dropped"
    );
}
