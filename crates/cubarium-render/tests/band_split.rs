//! FW-3: the two things that are only true if the *stamps* respect the split and the
//! cache — that a band-split composite of a real stamped scene is bit-identical to the
//! serial one for any number of bands, and that a cached footprint draws the same image
//! as an uncached one.
//!
//! `ring_canvas.rs` (FW-6) proves the split for a pass that writes pixels directly. This
//! file proves it for the path that goes through `unfold_pixels`, `Canvas::band_pixels`
//! and the source-over composite, which is where a band could lose a pixel at a seam, at
//! the ring's wrap, or on a band boundary a stamp straddles.

use cubarium_render::{
    Bend, Canvas, Mask, Pose, Shade, Sprite, Tone, Unfolds, stamp_layers_bent_toned,
    stamp_layers_cached,
};
use cubarium_surface::{Face, Scale, SurfacePoint, Topology, Vec2};

/// A 9×13 tile with a painted stem and two lobes, pivoted low so it stands on its anchor:
/// asymmetric in both axes, so a lost or duplicated pixel shows.
fn plant() -> Sprite {
    let (w, h) = (9usize, 13usize);
    let mut bytes = vec![0u8; w * h * 4];
    let mut put = |x: usize, y: usize, rgba: [u8; 4]| {
        let i = (y * w + x) * 4;
        bytes[i..i + 4].copy_from_slice(&rgba);
    };
    for y in 3..h {
        put(4, y, [40, (200 - y * 8) as u8, 60, 255]);
    }
    for (x, y) in [
        (2, 4),
        (3, 3),
        (5, 3),
        (6, 5),
        (3, 8),
        (6, 9),
        (4, 2),
        (5, 7),
    ] {
        put(x, y, [(30 + x * 20) as u8, 180, (90 + y * 6) as u8, 255]);
    }
    Sprite::from_rgba(w, h, Vec2::new(4.5, 8.0), &bytes).expect("inside the nine-pixel budget")
}

/// Anchors spread over the whole surface: mid-chart, on every rim, at the seams and, on a
/// ring, right on the wrap.
fn anchors(topo: Topology) -> Vec<SurfacePoint> {
    match topo {
        Topology::Cube => {
            let mut v = Vec::new();
            for face in Face::ALL {
                for (u, w) in [
                    (0.5, 0.5),
                    (63.5, 0.5),
                    (0.5, 63.5),
                    (63.5, 63.5),
                    (31.5, 47.5),
                ] {
                    v.push(SurfacePoint::new(face, u, w));
                }
            }
            // Deliberately on and around the band cuts of a 2- and 4-way split of 320 rows.
            for y in [0.5, 39.5, 40.5, 63.5] {
                v.push(SurfacePoint::new(Face::Back, 17.5, y));
            }
            v
        }
        Topology::Ring { w, h } => {
            let mut v = Vec::new();
            for y in [
                0.5,
                1.5,
                f64::from(h) / 2.0 + 0.5,
                f64::from(h) - 1.5,
                f64::from(h) - 0.5,
            ] {
                for x in [0.5, 1.5, 7.5, f64::from(w) / 2.0, f64::from(w) - 0.5] {
                    v.push(SurfacePoint::new(Face::Front, x, y));
                }
            }
            v
        }
    }
}

fn wind(i: usize) -> Bend {
    Bend {
        amplitude: ((i % 7) as f64 - 3.0) * 0.4,
        base: 0.0,
        root: 1.0,
        length: 10.0,
    }
}

/// One drawing pass: every anchor stamped, bent by its own wind, some of them toned.
fn draw_plants(canvas: &mut Canvas) {
    let sprite = plant();
    let topo = canvas.topology();
    let mut scratch = Vec::new();
    for (i, anchor) in anchors(topo).into_iter().enumerate() {
        let tone = Tone {
            colour: [0.4, 0.25, 0.1],
            shade: Shade {
                floor: 0.2,
                reference: 0.6,
            },
            mix: if i % 3 == 0 { 0.5 } else { 0.0 },
        };
        let mask = if i % 4 == 0 {
            Mask::Axial { reveal: 9.0 }
        } else {
            Mask::None
        };
        stamp_layers_bent_toned(
            canvas,
            anchor,
            Vec2::new(0.0, -1.0),
            &[(Pose::still(&sprite), 1.0)],
            1.0,
            0.9,
            mask,
            wind(i),
            tone,
            &mut scratch,
        );
    }
}

/// The same pass through the footprint cache.
fn draw_plants_cached(canvas: &mut Canvas, unfolds: &mut Unfolds) {
    let sprite = plant();
    let topo = canvas.topology();
    for (i, anchor) in anchors(topo).into_iter().enumerate() {
        let tone = Tone {
            colour: [0.4, 0.25, 0.1],
            shade: Shade {
                floor: 0.2,
                reference: 0.6,
            },
            mix: if i % 3 == 0 { 0.5 } else { 0.0 },
        };
        let mask = if i % 4 == 0 {
            Mask::Axial { reveal: 9.0 }
        } else {
            Mask::None
        };
        stamp_layers_cached(
            canvas,
            anchor,
            Vec2::new(0.0, -1.0),
            &[(Pose::still(&sprite), 1.0)],
            1.0,
            0.9,
            mask,
            wind(i),
            tone,
            unfolds,
        );
    }
}

fn topologies() -> [Topology; 2] {
    [Topology::Cube, Topology::Ring { w: 320, h: 180 }]
}

#[test]
fn a_stamped_scene_splits_into_bands_bit_for_bit() {
    for topo in topologies() {
        let mut serial = Canvas::new(topo, Scale::ONE);
        draw_plants(&mut serial);
        let lit = serial.pixels().iter().filter(|p| **p != [0.0; 3]).count();
        assert!(lit > 200, "{topo:?}: the pass drew only {lit} pixels");

        for n in [1usize, 2, 4, 5, 16] {
            let mut split = Canvas::new(topo, Scale::ONE);
            let mut bands = split.bands(n);
            split.for_each_band(&mut bands, draw_plants);
            assert_eq!(
                split.pixels(),
                serial.pixels(),
                "{topo:?}: the {n}-band composite differs from the serial image"
            );

            let mut par = Canvas::new(topo, Scale::ONE);
            let mut bands = par.bands(n);
            par.par_each_band(&mut bands, draw_plants);
            assert_eq!(
                par.pixels(),
                serial.pixels(),
                "{topo:?}: the {n}-band threaded composite differs from the serial image"
            );
        }
    }
}

/// A band that is drawn on twice in a row composites the second pass over the first,
/// exactly as the serial canvas does — the split must not lose the background a
/// source-over stamp reads.
#[test]
fn a_band_reads_the_background_the_serial_canvas_would_have() {
    for topo in topologies() {
        let mut serial = Canvas::new(topo, Scale::ONE);
        draw_plants(&mut serial);
        draw_plants(&mut serial);

        let mut split = Canvas::new(topo, Scale::ONE);
        let mut bands = split.bands(4);
        split.for_each_band(&mut bands, draw_plants);
        split.for_each_band(&mut bands, draw_plants);
        assert_eq!(
            split.pixels(),
            serial.pixels(),
            "{topo:?}: two passes over four bands"
        );
    }
}

#[test]
fn a_cached_footprint_draws_the_uncached_image() {
    for topo in topologies() {
        let mut plain = Canvas::new(topo, Scale::ONE);
        draw_plants(&mut plain);

        let mut unfolds = Unfolds::cached();
        // Four frames: the first fills the cache, the rest must hit it and still draw the
        // same image. The bend changes the radius frame to frame in the real presenter;
        // here the same radii recur, which is what the cache is keyed to survive.
        for frame in 0..4 {
            let mut cached = Canvas::new(topo, Scale::ONE);
            draw_plants_cached(&mut cached, &mut unfolds);
            assert_eq!(
                cached.pixels(),
                plain.pixels(),
                "{topo:?}: frame {frame} differs"
            );
        }
        let (hits, misses, entries, pixels) = unfolds.stats();
        assert!(
            hits >= 3 * misses,
            "{topo:?}: {hits} hits against {misses} misses"
        );
        assert_eq!(
            entries,
            anchors(topo).len(),
            "{topo:?}: one entry per anchor"
        );
        assert!(pixels > 0, "{topo:?}: {pixels} pooled");

        // And the cache composes with the split: same image again, four bands, cached.
        let mut split = Canvas::new(topo, Scale::ONE);
        let mut bands = split.bands(4);
        let mut per_band: Vec<Unfolds> = (0..bands.len()).map(|_| Unfolds::cached()).collect();
        split.split_into(&mut bands);
        for (band, unfolds) in bands.iter_mut().zip(per_band.iter_mut()) {
            draw_plants_cached(band, unfolds);
        }
        split.gather(&bands);
        assert_eq!(
            split.pixels(),
            plain.pixels(),
            "{topo:?}: cached, four bands"
        );
    }
}

/// A cached entry serves every smaller radius the same anchor asks for afterwards, which
/// is the property that makes one entry per anchor enough while the wind moves.
#[test]
fn a_widened_entry_still_draws_the_narrow_image() {
    let sprite = plant();
    let anchor = SurfacePoint::new(Face::Front, 63.5, 63.5);
    let narrow = Bend {
        amplitude: 0.0,
        base: 0.0,
        root: 1.0,
        length: 10.0,
    };
    let wide = Bend {
        amplitude: 1.6,
        base: 0.0,
        root: 1.0,
        length: 10.0,
    };
    let flat = Tone {
        colour: [0.0; 3],
        shade: Shade::FLAT,
        mix: 0.0,
    };

    let mut want = Canvas::cube();
    stamp_layers_bent_toned(
        &mut want,
        anchor,
        Vec2::new(0.0, -1.0),
        &[(Pose::still(&sprite), 1.0)],
        1.0,
        1.0,
        Mask::None,
        narrow,
        flat,
        &mut Vec::new(),
    );

    let mut unfolds = Unfolds::cached();
    // Ask for the wide footprint first, so the entry is wider than the narrow stamp needs.
    let mut scrap = Canvas::cube();
    stamp_layers_cached(
        &mut scrap,
        anchor,
        Vec2::new(0.0, -1.0),
        &[(Pose::still(&sprite), 1.0)],
        1.0,
        1.0,
        Mask::None,
        wide,
        flat,
        &mut unfolds,
    );
    let mut got = Canvas::cube();
    stamp_layers_cached(
        &mut got,
        anchor,
        Vec2::new(0.0, -1.0),
        &[(Pose::still(&sprite), 1.0)],
        1.0,
        1.0,
        Mask::None,
        narrow,
        flat,
        &mut unfolds,
    );
    assert_eq!(unfolds.stats().0, 1, "the narrow stamp hit the wide entry");
    assert_eq!(
        got.pixels(),
        want.pixels(),
        "a widened entry drew a different narrow image"
    );
}
