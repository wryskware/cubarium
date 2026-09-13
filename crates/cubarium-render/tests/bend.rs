//! Independent tests for the rooted wind deformation of `cubarium_render::sprite`, written
//! from the public doc comments of `Bend`, `Sprite::bend_headroom`, `stamp_layers_bent`,
//! `stamp_layers_bent_with_radius` and `Mask` — never from their bodies.
//!
//! Every expectation here is either recomputed from the normative formulas in those doc
//! comments (the Hermite profile, each mask's coverage), rebuilt a second way through the
//! same public API (a hand-shifted sprite stamped without a bend), or a property a wrong
//! implementation would visibly break: a root that skates, a tip that moves the wrong way or
//! not at all, a radial reveal that cuts a bent plant against a stationary circle, light lost
//! at a seam, or a filter tail clipped by the nine-pixel footprint the moment the wind blows.
//!
//! The shipped-pack sweeps (every authored plant and column frame at its measured budget) and
//! the synthetic tall column live in `crates/cubarium/tests/art_wind.rs` instead: they need
//! `cubarium::art::ArtPack` and the presenter's documented column geometry, and this crate is
//! below that one in the dependency graph.

use cube_proto::Face;
use cubarium_render::{
    Bend, Canvas, Mask, Pose, Sprite, stamp_layers, stamp_layers_bent,
    stamp_layers_bent_with_radius,
};
use cubarium_surface::{SurfacePoint, Vec2};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

/// Rows and columns of an authored plant tile.
const TILE: usize = 16;
/// The pivot every authored plant and trunk tile carries: the tile centre.
const PIVOT: Vec2 = Vec2 { x: 8.0, y: 8.0 };

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL
        .into_iter()
        .flat_map(|face| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (face, x, y))))
}

/// Bit-for-bit equality, which is what "exactly" and "bit for bit" in the docs mean.
fn assert_identical(a: &Canvas, b: &Canvas, what: &str) {
    let wrong = every_pixel().find(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y));
    if let Some((f, x, y)) = wrong {
        panic!("{what}: ({f:?}, {x}, {y}) is {:?} vs {:?}", a.get(f, x, y), b.get(f, x, y));
    }
}

fn max_diff(a: &Canvas, b: &Canvas) -> f32 {
    every_pixel()
        .flat_map(|(f, x, y)| {
            let (p, q) = (a.get(f, x, y), b.get(f, x, y));
            (0..3).map(move |c| (p[c] - q[c]).abs())
        })
        .fold(0.0, f32::max)
}

fn peak(image: &Canvas) -> f32 {
    every_pixel().flat_map(|(f, x, y)| image.get(f, x, y)).fold(0.0, f32::max)
}

/// Total light on the whole cube, in linear channel units.
fn total_light(image: &Canvas) -> f64 {
    every_pixel()
        .flat_map(|(f, x, y)| image.get(f, x, y))
        .map(f64::from)
        .sum()
}

/// An anchor that lands every texel of a `(8, 8)`-pivot tile on exactly one face pixel: with
/// an integer chart position, scale 1 and heading `+x`, texel `(tx, ty)` is sampled with
/// bilinear weights `(1, 0, 0, 0)` at face pixel `(tx + 24, ty + 24)`, and the destination
/// pixel's tile-local coordinate is exactly `(tx + 0.5, ty + 0.5)`.
fn aligned_anchor() -> SurfacePoint {
    SurfacePoint::new(Face::Front, 32.0, 32.0)
}

/// The face pixel texel `(tx, ty)` of a tile at [`aligned_anchor`] lands on.
fn texel_pixel(tx: usize, ty: usize) -> (Face, u8, u8) {
    (Face::Front, (tx + 24) as u8, (ty + 24) as u8)
}

/// The tile-local `x` of the centre of face pixel column `x`, for a tile at
/// [`aligned_anchor`].
fn tile_x(x: u8) -> f64 {
    f64::from(x) + 0.5 - 32.0 + PIVOT.x
}

fn draw_at(
    sprite: &Sprite,
    anchor: SurfacePoint,
    heading: Vec2,
    mask: Mask,
    bend: Bend,
) -> Canvas {
    let mut image = Canvas::new();
    stamp_layers_bent(
        &mut image,
        anchor,
        heading,
        &[(Pose::still(sprite), 1.0)],
        1.0,
        1.0,
        mask,
        bend,
        &mut Vec::new(),
    );
    image
}

fn draw(sprite: &Sprite, mask: Mask, bend: Bend) -> Canvas {
    draw_at(sprite, aligned_anchor(), Vec2::new(1.0, 0.0), mask, bend)
}

/// A 16×16 tile painting one opaque texel per row of `rows` in tile column `column`: a stem
/// standing up the tile, the thing a rooted bend exists for. Its extent stays inside the
/// nine-pixel footprint bound.
fn stem(rows: std::ops::Range<usize>, column: usize, color: impl Fn(usize) -> [u8; 4]) -> Sprite {
    let mut bytes = vec![0u8; TILE * TILE * 4];
    for ty in rows {
        let at = (ty * TILE + column) * 4;
        bytes[at..at + 4].copy_from_slice(&color(ty));
    }
    Sprite::from_rgba(TILE, TILE, PIVOT, &bytes).unwrap()
}

fn white_stem(rows: std::ops::Range<usize>) -> Sprite {
    stem(rows, 8, |_| [255, 255, 255, 255])
}

/// A 16×16 tile whose painted texels form a disc of `radius` around the pivot, so every row
/// carries paint at several distances from the pivot (a solid tile is over the footprint
/// budget and `Sprite::from_rgba` rejects it). The authored plant tiles have this shape.
fn disc(radius: f64, color: impl Fn(usize, usize) -> [u8; 4]) -> Sprite {
    let mut bytes = vec![0u8; TILE * TILE * 4];
    for ty in 0..TILE {
        for tx in 0..TILE {
            let (dx, dy) = (tx as f64 + 0.5 - PIVOT.x, ty as f64 + 0.5 - PIVOT.y);
            if dx.hypot(dy) <= radius {
                let at = (ty * TILE + tx) * 4;
                bytes[at..at + 4].copy_from_slice(&color(tx, ty));
            }
        }
    }
    Sprite::from_rgba(TILE, TILE, PIVOT, &bytes).unwrap()
}

// ---------------------------------------------------------------------------
// the normative formulas, recomputed here from the doc comments
// ---------------------------------------------------------------------------

/// `smoothstep(clamp((H − root) / length, 0, 1))` with the Hermite `t²(3 − 2t)`; a
/// non-positive or non-finite `length` and a `NaN` height yield 0.
fn profile(root: f64, length: f64, h: f64) -> f64 {
    if length <= 0.0 || length.is_nan() || h.is_nan() {
        return 0.0;
    }
    let t = ((h - root) / length).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `D` of a sample at tile row coordinate `p_y` of a 16-row tile: `H = (16 − p_y) + base`,
/// `D = amplitude · profile(H)`.
fn displacement(bend: Bend, p_y: f64) -> f64 {
    bend.amplitude * profile(bend.root, bend.length, (TILE as f64 - p_y) + bend.base)
}

/// Each mask's normative coverage at tile-local `p`, for a 16-row tile with pivot [`PIVOT`].
fn coverage(mask: Mask, p: Vec2) -> f64 {
    let h = TILE as f64 - p.y;
    let clamp = |v: f64| v.clamp(0.0, 1.0);
    match mask {
        Mask::None => 1.0,
        Mask::Axial { reveal } => clamp(reveal - h + 0.5) * clamp(reveal),
        Mask::Radial { reveal } => {
            let r = (p.x - PIVOT.x).hypot(p.y - PIVOT.y);
            clamp(reveal - r + 0.5) * clamp(reveal)
        }
        Mask::Strip { floor, reveal } => {
            clamp(reveal - h + 0.5) * clamp(h - floor + 0.5) * clamp(reveal - floor)
        }
    }
}

/// A bend with the small-plant shape of the doc comments: the tile stands on the root line.
fn plant_bend(amplitude: f64) -> Bend {
    Bend { amplitude, base: 0.0, root: 1.5, length: 13.0 }
}

// ---------------------------------------------------------------------------
// 1. the identity path
// ---------------------------------------------------------------------------

/// `Bend::NONE`, a zero or non-finite amplitude, a non-positive or non-finite length and a
/// non-finite base or root all take "exactly the path `stamp_layers` has always taken … bit
/// for bit". An implementation that ran the bend arithmetic anyway would differ in the last
/// bits of every resampled pixel, and one that let nonsense wind through would draw a wrong
/// image or nothing at all — and the whole zero-wind guarantee of the slice rests on this.
#[test]
fn every_identity_bend_draws_the_unbent_image_bit_for_bit() {
    let first = disc(7.7, |tx, ty| [(16 * ty + 8) as u8, 255 - (13 * tx) as u8, 90, 255]);
    let second = disc(6.4, |tx, ty| [40, (11 * tx) as u8, (16 * ty) as u8, 200]);
    let layers = [(Pose { first: &first, second: &second, mix: 0.37 }, 1.0)];

    let identities = [
        ("Bend::NONE", Bend::NONE),
        ("a zero amplitude", plant_bend(0.0)),
        ("a negative zero amplitude", plant_bend(-0.0)),
        ("a NaN amplitude", plant_bend(f64::NAN)),
        ("an infinite amplitude", plant_bend(f64::INFINITY)),
        ("a negatively infinite amplitude", plant_bend(f64::NEG_INFINITY)),
        ("a zero length", Bend { amplitude: 0.6, base: 0.0, root: 1.5, length: 0.0 }),
        ("a negative length", Bend { amplitude: 0.6, base: 0.0, root: 1.5, length: -13.0 }),
        ("a NaN length", Bend { amplitude: 0.6, base: 0.0, root: 1.5, length: f64::NAN }),
        ("a NaN base", Bend { amplitude: 0.6, base: f64::NAN, root: 1.5, length: 13.0 }),
        ("a NaN root", Bend { amplitude: 0.6, base: 0.0, root: f64::NAN, length: 13.0 }),
        (
            "an infinite base",
            Bend { amplitude: 0.6, base: f64::INFINITY, root: 1.5, length: 13.0 },
        ),
    ];
    let anchors = [
        ("mid-face", SurfacePoint::new(Face::Front, 32.25, 32.25), 1),
        ("across a side seam", SurfacePoint::new(Face::Front, 63.5, 32.5), 2),
        ("at a top-face vertex", SurfacePoint::new(Face::Top, 0.25, 0.25), 3),
    ];
    let masks = [
        Mask::None,
        Mask::Axial { reveal: 6.4 },
        Mask::Radial { reveal: 3.2 },
        Mask::Strip { floor: 4.0, reveal: 9.0 },
    ];

    for (what, anchor, faces) in anchors {
        for heading in [Vec2::new(1.0, 0.0), Vec2::new(0.6, -0.8)] {
            for mask in masks {
                let mut expected = Canvas::new();
                stamp_layers(
                    &mut expected,
                    anchor,
                    heading,
                    &layers,
                    1.0,
                    0.85,
                    mask,
                    &mut Vec::new(),
                );
                assert!(peak(&expected) > 0.05, "{what}/{mask:?}: the fixture must paint");
                if mask == Mask::None {
                    let touched = Face::ALL
                        .into_iter()
                        .filter(|&f| {
                            (0..64u8).any(|y| (0..64u8).any(|x| expected.get(f, x, y) != [0.0; 3]))
                        })
                        .count();
                    assert!(
                        touched >= faces,
                        "{what}: the fixture lit {touched} faces, not the {faces} the unfold spans"
                    );
                }
                for (name, bend) in identities {
                    assert!(bend.is_identity(), "{name} must be an identity bend");
                    let mut actual = Canvas::new();
                    stamp_layers_bent(
                        &mut actual,
                        anchor,
                        heading,
                        &layers,
                        1.0,
                        0.85,
                        mask,
                        bend,
                        &mut Vec::new(),
                    );
                    assert_identical(&actual, &expected, &format!("{name} {what} {mask:?}"));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2. the root never skates
// ---------------------------------------------------------------------------

/// "Heights `H ≤ root` are fixed (the painted root rows stay still)", for *any* amplitude —
/// including one far past the family's headroom, because the wiring clamps the amplitude but
/// the renderer must never move a root row whatever it is handed. The rows are exact, not
/// close: a root that moved by a hundredth of a pixel would resample and visibly skate.
#[test]
fn no_row_at_or_below_the_root_moves_under_any_amplitude() {
    // The plant shape (root 1.5 above the tile's bottom edge, the centre of the authored root
    // row) and the column shape (root 0, the horizon, with the base tile's bottom edge 8 px
    // below it).
    let shapes = [
        ("a small plant", 0.0f64, 1.5f64, 13.0f64),
        ("a column's base tile", -8.0, 0.0, 48.0),
    ];
    let sprite = disc(7.7, |tx, ty| [(15 * tx + 5) as u8, (15 * ty + 5) as u8, 200, 255]);

    for (what, base, root, length) in shapes {
        let still = draw(&sprite, Mask::None, Bend::NONE);
        // The destination rows whose height is at or below the root: `H = (16 − p_y) + base`
        // and a destination pixel in face row `24 + ty` has `p_y = ty + 0.5`.
        let fixed: Vec<usize> =
            (0..TILE).filter(|&ty| (TILE as f64 - (ty as f64 + 0.5)) + base <= root).collect();
        assert!(!fixed.is_empty(), "{what}: the fixture must have a fixed row");
        let mut moved = false;
        for amplitude in [-50.0, -3.0, -1.0, -0.5, -0.05, 0.05, 0.5, 1.0, 3.0, 50.0] {
            let bend = Bend { amplitude, base, root, length };
            let bent = draw(&sprite, Mask::None, bend);
            for &ty in &fixed {
                for tx in 0..TILE {
                    let (f, x, y) = texel_pixel(tx, ty);
                    assert_eq!(
                        bent.get(f, x, y),
                        still.get(f, x, y),
                        "{what}: row {ty} (H = {}) skated under amplitude {amplitude}",
                        (TILE as f64 - (ty as f64 + 0.5)) + base
                    );
                }
            }
            if max_diff(&still, &bent) > 0.05 {
                moved = true;
            }
        }
        assert!(moved, "{what}: the fixture never moved anything, so it proves nothing");
    }
}

// ---------------------------------------------------------------------------
// 3. the profile: monotone in height, exactly the amplitude past root + length
// ---------------------------------------------------------------------------

/// The displacement of each row, read back from the image as the horizontal centroid of that
/// row's light, must be the normative `amplitude · smoothstep(clamp((H − root) / length, 0,
/// 1))` — recomputed here — and therefore monotone in the row's height and exactly the
/// amplitude at and beyond `root + length`. A linear ramp, a profile with a kink at the root,
/// or one driven by the current row instead of the height above the root all fail here.
#[test]
fn each_rows_displacement_is_the_hermite_profile_of_its_height() {
    let rows = 0..15usize;
    let sprite = white_stem(rows.clone());
    for (base, root, length) in [(0.0, 1.5, 13.0), (0.0, 1.5, 6.0), (-8.0, 0.0, 48.0)] {
        let headroom = sprite.bend_headroom(root, length, base);
        let mut tested = 0;
        for amplitude in [-1.0f64, -0.4, 0.25, 0.7, 1.0] {
            if amplitude.abs() > headroom {
                continue;
            }
            tested += 1;
            let bend = Bend { amplitude, base, root, length };
            let image = draw(&sprite, Mask::None, bend);
            let mut previous: Option<(f64, f64)> = None;
            for ty in rows.clone().rev() {
                let p_y = ty as f64 + 0.5;
                let want = displacement(bend, p_y);
                // The row's light, and its horizontal centre of mass. One opaque white texel
                // resampled bilinearly in x lands on two pixels with weights summing to 1, so
                // the centroid is exactly the displaced texel centre.
                let (f, _, y) = texel_pixel(0, ty);
                let mut light = 0.0f64;
                let mut moment = 0.0f64;
                for x in 16..48u8 {
                    let v = f64::from(image.get(f, x, y)[1]);
                    light += v;
                    moment += v * (f64::from(x) + 0.5);
                }
                assert!(
                    (light - 1.0).abs() < 1e-5,
                    "row {ty} lost light under {bend:?}: {light}"
                );
                let got = moment / light - 32.5;
                assert!(
                    (got - want).abs() < 2e-5,
                    "row {ty} (H = {}) moved by {got}, not the profile's {want} ({bend:?})",
                    (TILE as f64 - p_y) + base
                );
                if let Some((below_h, below_d)) = previous {
                    // The sweep runs from the bottom row upward, so the previous row is the
                    // one below this one. Monotone in height: a higher row is displaced at
                    // least as far, in the amplitude's own direction.
                    let h = (TILE as f64 - p_y) + base;
                    assert!(h > below_h, "the sweep must climb: {h} after {below_h}");
                    assert!(
                        (got - below_d) * amplitude.signum() >= -2e-5,
                        "row {ty} at H {h} moved {got}, less than the row below it at {below_h} ({below_d}) under {bend:?}"
                    );
                }
                previous = Some(((TILE as f64 - p_y) + base, got));
            }
            // And past `root + length` the displacement is the whole amplitude.
            let saturated: Vec<usize> = rows
                .clone()
                .filter(|&ty| (TILE as f64 - (ty as f64 + 0.5)) + base >= root + length)
                .collect();
            for ty in saturated {
                let (f, _, y) = texel_pixel(0, ty);
                let (mut light, mut moment) = (0.0f64, 0.0f64);
                for x in 16..48u8 {
                    let v = f64::from(image.get(f, x, y)[1]);
                    light += v;
                    moment += v * (f64::from(x) + 0.5);
                }
                assert!(
                    (moment / light - 32.5 - amplitude).abs() < 2e-5,
                    "a row above root + length moved {} instead of the full {amplitude}",
                    moment / light - 32.5
                );
            }
        }
        assert!(tested >= 4, "the sweep tried only {tested} amplitudes on this bend shape");
    }
}

/// At an integer amplitude, with the whole sprite above `root + length`, the bend *is* a whole
/// pixel translation of the material — so the bent stamp equals the stamp of a hand-shifted
/// sprite, bit for bit, through a second independent path. A sign error, an off-by-one in the
/// inverse map, or a forward instead of an inverse sample all fail here.
#[test]
fn an_integer_amplitude_past_the_bend_length_shifts_the_material_by_whole_texels() {
    // A short length puts every row past `root + length`, so `D` is the amplitude everywhere.
    let shape = (0.0f64, 0.0f64, 0.4f64);
    for (rows, amplitude) in [(4..12usize, 2.0f64), (4..12, -2.0), (0..15, 1.0), (0..15, -1.0)] {
        let color = |ty: usize| [(17 * ty + 3) as u8, 255 - (9 * ty) as u8, 120, 255];
        let sprite = stem(rows.clone(), 8, color);
        let shifted = stem(rows.clone(), (8.0 + amplitude) as usize, color);
        let bend = Bend { amplitude, base: shape.0, root: shape.1, length: shape.2 };
        assert!(
            sprite.bend_headroom(bend.root, bend.length, bend.base) >= amplitude.abs(),
            "the fixture's amplitude {amplitude} must be inside the sprite's headroom"
        );
        let bent = draw(&sprite, Mask::None, bend);
        let expected = draw(&shifted, Mask::None, Bend::NONE);
        assert!(peak(&expected) > 0.1, "the fixture drew nothing");
        assert_identical(&bent, &expected, &format!("a bend of exactly {amplitude}"));
        // Non-vacuity: the unbent image really is somewhere else.
        assert!(max_diff(&bent, &draw(&sprite, Mask::None, Bend::NONE)) > 0.1);
    }
}

// ---------------------------------------------------------------------------
// 4. masks live in material coordinates
// ---------------------------------------------------------------------------

/// "The mask is evaluated at `p` itself — the *material* row and radius". Coverage is
/// therefore the normative coverage of `q = (p.x − D, p.y)`, recomputed here, times the
/// unmasked bent sample: a growth reveal stays attached to the plant it is revealing, and
/// `Mask::Strip` row ownership is exactly what it was. The non-vacuity assertion at the end is
/// the failure this protects against: a radial reveal evaluated in destination coordinates
/// cuts the bent material against a stationary circle.
#[test]
fn a_bend_reads_every_mask_on_the_material_it_moved() {
    let sprite = disc(6.0, |tx, ty| [(15 * tx + 9) as u8, 250 - (14 * ty) as u8, 70, 255]);
    let bend = plant_bend(0.55);
    assert!(
        sprite.bend_headroom(bend.root, bend.length, bend.base) >= bend.amplitude,
        "the fixture's bend must be inside the sprite's own headroom"
    );
    let unmasked = draw(&sprite, Mask::None, bend);
    assert!(peak(&unmasked) > 0.1);

    for mask in [
        Mask::Axial { reveal: 6.0 },
        Mask::Axial { reveal: 9.4 },
        Mask::Radial { reveal: 3.0 },
        Mask::Radial { reveal: 5.5 },
        Mask::Strip { floor: 4.0, reveal: 9.0 },
        Mask::Strip { floor: 6.0, reveal: 13.5 },
    ] {
        let masked = draw(&sprite, mask, bend);
        let mut wrong_coordinates_would_differ = 0.0f64;
        for ty in 0..TILE {
            for tx in 0..TILE {
                let (f, x, y) = texel_pixel(tx, ty);
                let p = Vec2::new(tile_x(x), ty as f64 + 0.5);
                let q = Vec2::new(p.x - displacement(bend, p.y), p.y);
                let want = coverage(mask, q);
                for c in 0..3 {
                    let whole = f64::from(unmasked.get(f, x, y)[c]);
                    let got = f64::from(masked.get(f, x, y)[c]);
                    assert!(
                        (got - whole * want).abs() < 1e-6,
                        "{mask:?} at texel ({tx}, {ty}) channel {c}: {got}, not {} ({whole} × the material coverage {want})",
                        whole * want
                    );
                    wrong_coordinates_would_differ = wrong_coordinates_would_differ
                        .max((whole * coverage(mask, p) - whole * want).abs());
                }
            }
        }
        if matches!(mask, Mask::Radial { .. }) {
            assert!(
                wrong_coordinates_would_differ > 0.02,
                "{mask:?}: the fixture cannot tell material from destination coordinates"
            );
        } else {
            // Axial and Strip coverage depends on the row alone, so the bend cannot move it:
            // the same rows are owned, windy or calm, which is what keeps a tall column's
            // strips from opening a join.
            assert_eq!(
                wrong_coordinates_would_differ, 0.0,
                "{mask:?} is not a function of the row alone"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 5. light across the seams
// ---------------------------------------------------------------------------

/// A bent stamp that straddles a seam must carry the same light as one in the middle of a
/// face: `unfold_pixels` gives every pixel within the radius exactly once, and the bend only
/// moves light along a row, so nothing is lost or duplicated where two charts meet. A bend
/// applied in face coordinates instead of the tile's own, or one that forgot the seam
/// rotation, would leak light at the seam.
///
/// At a cube *vertex* the surface has only 270° of pixels, so an unfolding of a
/// vertex-straddling stamp cannot be a bijection onto the tile — the doc for `unfold_pixels`
/// says as much ("a shape may show a localized discontinuity at a vertex rather than doubled
/// brightness"). There the requirement is the weaker, but still decisive, one: the wind must
/// not change how much light the vertex costs, so a bent stamp there carries the same light as
/// the unbent stamp at the same anchor.
#[test]
fn a_bent_stamp_keeps_its_light_across_a_seam_and_pays_no_more_at_a_vertex() {
    let sprite = disc(6.0, |tx, ty| [(15 * tx + 9) as u8, 250 - (14 * ty) as u8, 70, 255]);
    let bend = plant_bend(0.55);
    assert!(sprite.bend_headroom(bend.root, bend.length, bend.base) >= bend.amplitude);

    for heading in [Vec2::new(1.0, 0.0), Vec2::new(0.6, -0.8)] {
        let middle = total_light(&draw_at(
            &sprite,
            SurfacePoint::new(Face::Front, 32.0, 32.0),
            heading,
            Mask::None,
            bend,
        ));
        assert!(middle > 1.0, "the fixture must carry real light");
        // A bilinear kernel is a partition of unity on an *axis-aligned* pixel lattice, so a
        // stamp laid along a face's own axes conserves its light exactly, and one laid at an
        // angle ripples by a fraction of a percent wherever it is drawn (this fixture's
        // rotated mid-face stamp already carries 0.6 % more light than its axis-aligned one).
        // The seam must add nothing to that: the tolerance is the ripple, not a seam budget.
        let tolerance = if heading == Vec2::new(1.0, 0.0) { 1e-6 } else { 3e-3 };

        for (what, anchor) in [
            ("a side/side seam", SurfacePoint::new(Face::Front, 63.5, 32.5)),
            ("a side/side seam, off centre", SurfacePoint::new(Face::Right, 0.5, 41.5)),
            ("a side/top seam", SurfacePoint::new(Face::Front, 27.5, 0.5)),
            ("a side/top seam, from Top", SurfacePoint::new(Face::Top, 38.5, 63.5)),
        ] {
            let light = total_light(&draw_at(&sprite, anchor, heading, Mask::None, bend));
            assert!(
                (light - middle).abs() / middle < tolerance,
                "{what}: a bent stamp carried {light} where a mid-face stamp carries {middle}"
            );
        }

        // A cube vertex carries only 270° of surface, so no unfolding of a vertex-straddling
        // stamp can be a bijection onto the tile: `unfold_pixels` keeps one image per pixel
        // and its doc admits "a localized discontinuity at a vertex rather than doubled
        // brightness". Conservation is therefore not available there, and the requirement is
        // the one that matters for the wind: the stamp is still drawn, and the breeze neither
        // drops nor doubles the wedge the cone cannot represent. That wedge is a quarter of a
        // disc centred on the anchor, so a dropped or doubled wedge would move the light by
        // roughly 25 %; this bound is well inside that and well outside the ~4 % the cone's
        // own discontinuity accounts for.
        // The cone's own defect is large — this fixture's disc loses about a fifth of its light
        // at a vertex whether it is bent or not, which is the quarter wedge — so the bound is
        // on what the *wind* does to it, and that must stay far below the wedge's own share.
        const VERTEX_DEFECT: f64 = 0.10;
        for (what, anchor) in [
            ("a Top vertex", SurfacePoint::new(Face::Top, 0.5, 0.5)),
            ("a Top vertex, from a side", SurfacePoint::new(Face::Front, 0.5, 0.5)),
        ] {
            let still = total_light(&draw_at(&sprite, anchor, heading, Mask::None, Bend::NONE));
            let bent = total_light(&draw_at(&sprite, anchor, heading, Mask::None, bend));
            for (name, light) in [("unbent", still), ("bent", bent)] {
                assert!(
                    light > 0.7 * middle && light < 1.05 * middle,
                    "{what}: the {name} stamp carried {light}, not the {middle} of a mid-face stamp less at most the cone's quarter wedge"
                );
            }
            assert!(
                (bent - still).abs() / middle < VERTEX_DEFECT,
                "{what}: the wind moved the light of a stamp from {still} to {bent} (mid-face {middle})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 6. the footprint admits the whole bent support
// ---------------------------------------------------------------------------

/// At its own admitted headroom a sprite's bent stamp must be complete: unfolding a
/// deliberately larger radius finds nothing the nine-pixel footprint missed. This is the
/// property `stamp_layers_bent_with_radius` exists for, and it is the one that fails silently
/// — a clipped filter tail is a texel that quietly stops being drawn when the wind blows.
#[test]
fn at_its_admitted_headroom_a_larger_unfold_radius_draws_the_same_image() {
    let fixtures: [(&str, Sprite); 4] = [
        ("a stem", white_stem(0..15)),
        ("a disc", disc(7.7, |tx, ty| [(15 * tx + 9) as u8, 250 - (14 * ty) as u8, 70, 255])),
        ("a tall off-centre stem", stem(0..16, 9, |ty| [200, (13 * ty) as u8, 255, 255])),
        (
            "one far texel",
            Sprite::from_rgba(1, 1, Vec2::new(0.5, 8.2), &[255, 255, 255, 255]).unwrap(),
        ),
    ];
    let anchors = [
        ("mid-face", SurfacePoint::new(Face::Front, 32.25, 32.25)),
        ("across a side seam", SurfacePoint::new(Face::Front, 63.5, 32.5)),
        ("at a top vertex", SurfacePoint::new(Face::Top, 0.25, 0.25)),
    ];
    for (name, sprite) in &fixtures {
        for (base, root, length) in [(0.0, 1.5, 13.0), (-8.0, 0.0, 48.0), (28.0, 0.0, 48.0)] {
            let headroom = sprite.bend_headroom(root, length, base);
            if !headroom.is_finite() {
                continue;
            }
            for sign in [-1.0, 1.0] {
                let bend = Bend { amplitude: sign * headroom, base, root, length };
                for (what, anchor) in anchors {
                    // A stamp the *unbent* renderer already draws as nothing is a fixture the
                    // cube's vertex cone has swallowed, not a wind failure: only the radius
                    // agreement is meaningful there.
                    let mut still = Canvas::new();
                    stamp_layers_bent(
                        &mut still,
                        anchor,
                        Vec2::new(0.6, -0.8),
                        &[(Pose::still(sprite), 1.0)],
                        1.0,
                        1.0,
                        Mask::None,
                        Bend::NONE,
                        &mut Vec::new(),
                    );
                    let mut tight = Canvas::new();
                    stamp_layers_bent(
                        &mut tight,
                        anchor,
                        Vec2::new(0.6, -0.8),
                        &[(Pose::still(sprite), 1.0)],
                        1.0,
                        1.0,
                        Mask::None,
                        bend,
                        &mut Vec::new(),
                    );
                    let mut wide = Canvas::new();
                    stamp_layers_bent_with_radius(
                        &mut wide,
                        anchor,
                        Vec2::new(0.6, -0.8),
                        &[(Pose::still(sprite), 1.0)],
                        1.0,
                        1.0,
                        Mask::None,
                        bend,
                        14.0,
                        &mut Vec::new(),
                    );
                    assert!(
                        peak(&tight) > 0.0 || peak(&still) == 0.0,
                        "{name} vanished at {what} under an admitted {}",
                        bend.amplitude
                    );
                    assert_identical(
                        &tight,
                        &wide,
                        &format!("{name} at {what}, amplitude {}", bend.amplitude),
                    );
                }
            }
        }
    }
}
