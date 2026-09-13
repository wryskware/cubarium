//! Independent tests for the pose / layer / reveal-mask compositing surface of
//! `cubarium_render::sprite`, written from the public doc comments of `Pose`, `Mask`,
//! `stamp_pose`, `stamp_layers`, `Sprite::subtract` and `Sprite::paints_like` rather than
//! from their bodies. Every expectation here is either an image the same public API can
//! build a second, independent way (one stamp versus a hand-weighted mix of stamps) or a
//! property a wrong implementation would visibly break: a background leak in a cross-fade,
//! a row popping in whole, a reveal that is not monotone, a zero-weight layer that still
//! costs footprint.

use cube_proto::Face;
use cubarium_render::{Canvas, Mask, Pose, Sprite, stamp_layers, stamp_pose, stamp_sprite};
use cubarium_surface::{SurfacePoint, Vec2};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL
        .into_iter()
        .flat_map(|face| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (face, x, y))))
}

/// The largest per-channel difference between two images, in linear light.
fn max_diff(a: &Canvas, b: &Canvas) -> f32 {
    every_pixel()
        .flat_map(|(f, x, y)| {
            let (p, q) = (a.get(f, x, y), b.get(f, x, y));
            (0..3).map(move |c| (p[c] - q[c]).abs())
        })
        .fold(0.0, f32::max)
}

/// Bit-for-bit equality, which is what "exactly" and "bit for bit" in the docs mean.
fn assert_identical(a: &Canvas, b: &Canvas, what: &str) {
    let wrong = every_pixel().find(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y));
    assert!(
        wrong.is_none(),
        "{what}: {:?} is {:?} vs {:?}",
        wrong.unwrap(),
        a.get(wrong.unwrap().0, wrong.unwrap().1, wrong.unwrap().2),
        b.get(wrong.unwrap().0, wrong.unwrap().1, wrong.unwrap().2),
    );
}

fn peak(image: &Canvas) -> f32 {
    every_pixel()
        .flat_map(|(f, x, y)| image.get(f, x, y))
        .fold(0.0, f32::max)
}

fn filled(rgb: [f32; 3]) -> Canvas {
    let mut canvas = Canvas::new();
    for (f, x, y) in every_pixel() {
        canvas.set(f, x, y, rgb);
    }
    canvas
}

/// One opaque (or not) texel with a centered pivot.
fn dot(rgba: [u8; 4]) -> Sprite {
    Sprite::from_rgba(1, 1, Vec2::new(0.5, 0.5), &rgba).unwrap()
}

/// A 16-row tile whose painted texels form a disc around the pivot, so every row carries
/// paint while the footprint stays inside the nine-pixel surface budget (a *solid* 16×16
/// tile is rejected by `Sprite::from_rgba` at extent 11.8), coloured by `color(row)`.
/// The authored plant and trunk tiles have exactly this shape.
fn row_tile(color: impl Fn(usize) -> [u8; 4]) -> Sprite {
    let mut bytes = vec![0u8; 16 * 16 * 4];
    for ty in 0..16usize {
        for tx in 0..16usize {
            let (dx, dy) = (tx as f64 + 0.5 - 8.0, ty as f64 + 0.5 - 8.0);
            if dx.hypot(dy) <= 7.7 {
                bytes[(ty * 16 + tx) * 4..(ty * 16 + tx) * 4 + 4].copy_from_slice(&color(ty));
            }
        }
    }
    Sprite::from_rgba(16, 16, Vec2::new(8.0, 8.0), &bytes).unwrap()
}

/// A tile anchor that lands every texel on exactly one face pixel: with an integer chart
/// position, a `(8, 8)` pivot, scale 1 and heading `+x`, texel `(tx, ty)` is sampled with
/// bilinear weights `(1, 0, 0, 0)` at face pixel `(tx + 24, ty + 24)`.
const TILE_ANCHOR: (f64, f64) = (32.0, 32.0);

fn tile_anchor() -> SurfacePoint {
    SurfacePoint::new(Face::Front, TILE_ANCHOR.0, TILE_ANCHOR.1)
}

/// The face pixel texel `(tx, ty)` of a tile at [`tile_anchor`] lands on.
fn texel_pixel(tx: usize, ty: usize) -> (Face, u8, u8) {
    (Face::Front, (tx + 24) as u8, (ty + 24) as u8)
}

fn draw_layers(
    background: &Canvas,
    anchor: SurfacePoint,
    layers: &[(Pose, f32)],
    scale: f64,
    mask: Mask,
) -> Canvas {
    let mut image = background.clone();
    stamp_layers(
        &mut image,
        anchor,
        Vec2::new(1.0, 0.0),
        layers,
        scale,
        1.0,
        mask,
        &mut Vec::new(),
    );
    image
}

fn draw_tile(sprite: &Sprite, mask: Mask, background: &Canvas) -> Canvas {
    draw_layers(background, tile_anchor(), &[(Pose::still(sprite), 1.0)], 1.0, mask)
}

/// The pixels a tile at [`tile_anchor`] can reach: its footprint is under the nine-pixel
/// budget, so everything it touches lies well inside this window. Reveal sweeps only have
/// to look here, which is what keeps a four-hundred-step sweep cheap.
fn tile_pixels() -> impl Iterator<Item = (Face, u8, u8)> {
    (16..48u8).flat_map(move |y| (16..48u8).map(move |x| (Face::Front, x, y)))
}

/// The largest reveal step a sweep takes, and the most a pixel's coverage may change over
/// it. **Derived from the normative formulas**, not from the implementation: each variant's
/// coverage is a product of at most two factors that depend on the reveal, each a unit ramp
/// of slope 1 per tile pixel (`clamp(reveal − h + 0.5)` with the start envelope
/// `clamp(reveal)`, or `clamp(reveal − floor)`). For `f, g ∈ [0, 1]`, `(f + δ)(g + δ) − fg
/// ≤ 2δ + δ²`. A mask that popped a row in whole would move a pixel by 1.
const REVEAL_STEP: f64 = 0.05;
const REVEAL_LIPSCHITZ: f32 = (2.0 * REVEAL_STEP + REVEAL_STEP * REVEAL_STEP) as f32;

/// Every pixel of `image` is at least `before`'s and at most `REVEAL_LIPSCHITZ` above it.
fn assert_reveal_step(image: &Canvas, before: &Canvas, what: &str) {
    for (f, x, y) in tile_pixels() {
        let (now, last) = (image.get(f, x, y), before.get(f, x, y));
        for c in 0..3 {
            assert!(
                now[c] >= last[c] - 1e-6,
                "{what}: coverage fell at ({f:?}, {x}, {y}), {} -> {}",
                last[c],
                now[c]
            );
            assert!(
                now[c] - last[c] <= REVEAL_LIPSCHITZ + 1e-6,
                "{what}: a {REVEAL_STEP}-pixel step moved ({f:?}, {x}, {y}) by {}",
                now[c] - last[c]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 1. stamp_pose of a still sprite *is* stamp_sprite
// ---------------------------------------------------------------------------

/// A wrong implementation that always ran the blend arithmetic, or sized its footprint
/// from both sprites, would drift from `stamp_sprite` in the last bits — which is exactly
/// what the decided M2 image comparisons in the host's tests cannot tolerate.
#[test]
fn a_still_pose_without_a_mask_stamps_bit_identically_to_stamp_sprite() {
    let sprite = row_tile(|ty| [(16 * ty + 8) as u8, 255 - (13 * ty) as u8, 90, 255]);
    let anchors = [
        ("mid-face", SurfacePoint::new(Face::Front, 32.25, 32.25), 1),
        ("across a side seam", SurfacePoint::new(Face::Front, 63.5, 32.5), 2),
        ("at a top-face vertex", SurfacePoint::new(Face::Top, 0.25, 0.25), 3),
    ];
    for (what, anchor, faces) in anchors {
        for heading in [Vec2::new(1.0, 0.0), Vec2::new(0.6, -0.8)] {
            let mut expected = Canvas::new();
            stamp_sprite(&mut expected, anchor, heading, &sprite, 1.0, 0.85, &mut Vec::new());
            let mut actual = Canvas::new();
            stamp_pose(
                &mut actual,
                anchor,
                heading,
                Pose::still(&sprite),
                1.0,
                0.85,
                Mask::None,
                &mut Vec::new(),
            );
            assert!(peak(&expected) > 0.05, "{what}: the fixture must paint something");
            let touched = Face::ALL
                .into_iter()
                .filter(|&f| {
                    (0..64u8).any(|y| (0..64u8).any(|x| expected.get(f, x, y) != [0.0; 3]))
                })
                .count();
            assert!(
                touched >= faces,
                "{what}: the fixture lit {touched} faces, not the {faces} the unfold must span"
            );
            assert_identical(&actual, &expected, what);
        }
    }
}

// ---------------------------------------------------------------------------
// 2. weighted layers are an exact lerp, not two partial stamps
// ---------------------------------------------------------------------------

/// The documented reason `stamp_layers` exists: a pixel opaque in every layer stays
/// opaque, where the same fade drawn as two half-opacity stamps lets a quarter of the
/// background through.
#[test]
fn two_opaque_layers_at_half_weight_replace_the_background_exactly() {
    let red = dot([255, 0, 0, 255]);
    let blue = dot([0, 0, 255, 255]);
    let anchor = SurfacePoint::pixel_center(Face::Front, 32, 32);
    for background in [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.4, 0.9, 0.2]] {
        let image = draw_layers(
            &filled(background),
            anchor,
            &[(Pose::still(&red), 0.5), (Pose::still(&blue), 0.5)],
            1.0,
            Mask::None,
        );
        let pixel = image.get(Face::Front, 32, 32);
        assert!(
            (pixel[0] - 0.5).abs() < 1e-6 && pixel[1] == 0.0 && (pixel[2] - 0.5).abs() < 1e-6,
            "over {background:?} the mix was {pixel:?}, not an exact (0.5, 0, 0.5)"
        );
    }

    // Non-vacuity: the same fade as two separate half-opacity stamps does leak.
    let background = filled([0.0, 1.0, 0.0]);
    let mut leaky = background.clone();
    for sprite in [&red, &blue] {
        stamp_sprite(&mut leaky, anchor, Vec2::new(1.0, 0.0), sprite, 1.0, 0.5, &mut Vec::new());
    }
    let leaked = leaky.get(Face::Front, 32, 32);
    assert!(
        leaked[1] > 0.2,
        "the fixture must show why layers exist; two stamps left only {leaked:?}"
    );
}

/// A layer at weight 0 is not sampled *and* does not pay footprint: the second sprite here
/// is over the nine-pixel budget at this scale, so an implementation that took its extent
/// into account would draw nothing at all.
#[test]
fn a_weight_of_one_is_the_first_layer_alone_and_a_zero_weight_layer_costs_no_footprint() {
    let small = dot([255, 220, 40, 255]);
    let large = Sprite::from_rgba(5, 1, Vec2::new(2.5, 0.5), &[255; 20]).unwrap();
    let scale = 3.0;
    assert!(large.extent() * scale > 9.0, "the fixture's wide sprite must break the budget");
    assert!(small.extent() * scale < 9.0);
    let anchor = SurfacePoint::new(Face::Front, 32.25, 32.25);
    let background = filled([0.1, 0.0, 0.3]);

    let alone = draw_layers(&background, anchor, &[(Pose::still(&small), 1.0)], scale, Mask::None);
    assert!(max_diff(&alone, &background) > 0.1, "the fixture drew nothing");

    for weights in [(1.0f32, 0.0f32), (1.0, -1.0), (1.0, f32::NAN)] {
        let mixed = draw_layers(
            &background,
            anchor,
            &[(Pose::still(&small), weights.0), (Pose::still(&large), weights.1)],
            scale,
            Mask::None,
        );
        assert_identical(&mixed, &alone, &format!("weights {weights:?}"));
    }
}

#[test]
fn three_layers_at_equal_weights_give_the_exact_mean_of_their_samples() {
    let red = dot([255, 0, 0, 255]);
    let green = dot([0, 255, 0, 255]);
    let blue = dot([0, 0, 255, 255]);
    let third = 1.0f32 / 3.0;
    let image = draw_layers(
        &filled([0.8, 0.8, 0.8]),
        SurfacePoint::pixel_center(Face::Front, 20, 40),
        &[
            (Pose::still(&red), third),
            (Pose::still(&green), third),
            (Pose::still(&blue), third),
        ],
        1.0,
        Mask::None,
    );
    let pixel = image.get(Face::Front, 20, 40);
    for c in 0..3 {
        assert!(
            (pixel[c] - third).abs() < 1e-6,
            "three equal layers gave {pixel:?}, not the mean"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. every layer keeps its own temporal blend
// ---------------------------------------------------------------------------

/// A layer is a `Pose`, not a frame: an implementation that dropped the temporal mix while
/// cross-fading (the "fade drops the sway back to held frames" failure the docs name)
/// would show the first sample instead of the blend.
#[test]
fn a_layer_contributes_its_temporal_blend_and_not_a_held_frame() {
    let red = dot([255, 0, 0, 255]);
    let blue = dot([0, 0, 255, 255]);
    let green = dot([0, 255, 0, 255]);
    let anchor = SurfacePoint::pixel_center(Face::Left, 10, 10);
    let swaying = Pose { first: &red, second: &blue, mix: 0.5 };

    // One layer at weight 1: the pose's own lerp, (0.5, 0, 0.5).
    let solo = draw_layers(&Canvas::new(), anchor, &[(swaying, 1.0)], 1.0, Mask::None);
    let pixel = solo.get(Face::Left, 10, 10);
    assert!(
        (pixel[0] - 0.5).abs() < 1e-6 && pixel[1] == 0.0 && (pixel[2] - 0.5).abs() < 1e-6,
        "a mid-clip pose drew {pixel:?}"
    );

    // Half of that blend against half of a still layer: (0.25, 0.5, 0.25).
    let faded = draw_layers(
        &Canvas::new(),
        anchor,
        &[(swaying, 0.5), (Pose::still(&green), 0.5)],
        1.0,
        Mask::None,
    );
    let pixel = faded.get(Face::Left, 10, 10);
    for (c, want) in [0.25f32, 0.5, 0.25].into_iter().enumerate() {
        assert!(
            (pixel[c] - want).abs() < 1e-6,
            "a cross-fade of a blended pose drew {pixel:?}, not (0.25, 0.5, 0.25)"
        );
    }
    // And it is not the held first frame of the swaying layer, which would be (0.5, 0.5, 0).
    assert!(pixel[2] > 0.2, "the layer lost its temporal blend: {pixel:?}");
}

// ---------------------------------------------------------------------------
// 4. Mask::Strip owns exactly its rows
// ---------------------------------------------------------------------------

/// Single ownership is what keeps a tall plant's overlapping trunk tiles from compositing
/// a row twice. A half-row-off implementation, or one that let the ramp bleed a row, shows
/// up here as a row that is painted when it should be bare.
#[test]
fn a_strip_paints_exactly_the_whole_rows_between_its_floor_and_reveal() {
    let tile = row_tile(|ty| [(16 * ty + 8) as u8, 200, 60, 255]);
    let background = filled([0.02, 0.0, 0.05]);
    let whole = draw_tile(&tile, Mask::None, &background);

    for (floor, reveal, rows) in [(12.0, 16.0, 0..4usize), (9.0, 16.0, 0..7)] {
        let strip = draw_tile(&tile, Mask::Strip { floor, reveal }, &background);
        for ty in 0..16usize {
            let inside = rows.contains(&ty);
            let mut painted = 0;
            for tx in 0..16usize {
                let (f, x, y) = texel_pixel(tx, ty);
                let want = if inside { whole.get(f, x, y) } else { background.get(f, x, y) };
                assert_eq!(
                    strip.get(f, x, y),
                    want,
                    "Strip {{ floor: {floor}, reveal: {reveal} }} at tile texel ({tx}, {ty})"
                );
                if whole.get(f, x, y) != background.get(f, x, y) {
                    painted += 1;
                }
            }
            assert!(painted > 0, "the fixture paints nothing in row {ty}");
        }
    }

    // An empty strip draws nothing at all.
    for (floor, reveal) in [(12.0, 12.0), (12.0, 11.0), (0.0, 0.0)] {
        let empty = draw_tile(&tile, Mask::Strip { floor, reveal }, &background);
        assert_identical(&empty, &background, &format!("Strip {{ {floor}, {reveal} }}"));
    }

    // Two strips that partition the tile's rows composite to one unmasked stamp: every row
    // is owned by exactly one of them, so no row is painted twice and none is missed.
    let mut split = background.clone();
    for mask in [Mask::Strip { floor: 0.0, reveal: 12.0 }, Mask::Strip { floor: 12.0, reveal: 16.0 }] {
        stamp_layers(
            &mut split,
            tile_anchor(),
            Vec2::new(1.0, 0.0),
            &[(Pose::still(&tile), 1.0)],
            1.0,
            1.0,
            mask,
            &mut Vec::new(),
        );
    }
    assert!(max_diff(&split, &whole) < 1e-6, "{}", max_diff(&split, &whole));
}

/// The reveal is a continuous function of the strip's height: a twentieth-of-a-pixel step
/// may move a pixel only by [`REVEAL_LIPSCHITZ`], and a strip that has only just left its
/// floor is invisible. A row-at-a-time implementation would step by a whole row.
#[test]
fn a_growing_strip_fades_its_first_row_in_and_never_jumps_a_whole_row() {
    let tile = row_tile(|_| [255, 255, 255, 255]);
    let floor = 9.0;
    let empty = Canvas::new();

    // Over black with an opaque white tile the pixel value *is* the mask coverage.
    let mut previous = draw_tile(&tile, Mask::Strip { floor, reveal: floor }, &empty);
    assert_eq!(peak(&previous), 0.0, "a strip at its floor must be empty");
    let mut reveal = floor + REVEAL_STEP;
    while reveal <= 17.0 {
        let image = draw_tile(&tile, Mask::Strip { floor, reveal }, &empty);
        assert_reveal_step(&image, &previous, &format!("strip reveal {reveal}"));
        previous = image;
        reveal += REVEAL_STEP;
    }
    assert!(peak(&previous) > 0.9, "the fixture never filled a row");

    let born = draw_tile(&tile, Mask::Strip { floor, reveal: floor + 1e-6 }, &empty);
    assert!(
        peak(&born) < 1e-4,
        "a strip one millionth of a pixel tall already shows {}",
        peak(&born)
    );
}

// ---------------------------------------------------------------------------
// 5. the axial and radial start envelopes
// ---------------------------------------------------------------------------

/// A reveal sweep, which is what a plant coming up out of the ground actually is: the image
/// only ever gains light, gains at most a pixel's worth per pixel of reveal, starts from
/// nothing and ends exactly at the unmasked stamp.
#[test]
fn axial_and_radial_reveals_are_monotone_from_nothing_to_the_whole_tile() {
    let tile = row_tile(|_| [255, 255, 255, 255]);
    let empty = Canvas::new();
    let whole = draw_tile(&tile, Mask::None, &empty);
    assert!(peak(&whole) > 0.9);

    for name in ["axial", "radial"] {
        let mask_at = |reveal: f64| {
            if name == "axial" { Mask::Axial { reveal } } else { Mask::Radial { reveal } }
        };
        let born = draw_tile(&tile, mask_at(1e-6), &empty);
        assert!(peak(&born) < 1e-4, "{name} popped at reveal 1e-6: {}", peak(&born));

        let mut previous = draw_tile(&tile, mask_at(0.0), &empty);
        assert_eq!(peak(&previous), 0.0, "{name} at reveal 0 must be empty");
        let mut steps = 0;
        let mut reveal = REVEAL_STEP;
        while reveal <= 20.0 {
            let image = draw_tile(&tile, mask_at(reveal), &empty);
            assert_reveal_step(&image, &previous, &format!("{name} reveal {reveal}"));
            previous = image;
            steps += 1;
            reveal += REVEAL_STEP;
        }
        assert!(steps > 350);
        // Nothing the sweep asserted on is outside the window it looked at.
        for (f, x, y) in every_pixel() {
            if f == Face::Front && (16..48).contains(&x) && (16..48).contains(&y) {
                continue;
            }
            assert_eq!(previous.get(f, x, y), [0.0; 3], "the tile reached ({f:?}, {x}, {y})");
        }
        assert_identical(&previous, &whole, &format!("{name} at a reveal past the tile"));
    }
}

// ---------------------------------------------------------------------------
// 6. Sprite::subtract and Sprite::paints_like
// ---------------------------------------------------------------------------

/// Four 4×4 sprites over the same grid: `base` is the "crown", `other` the "trunk" whose
/// shared texels must go, and `expected` the hand-written result of clearing exactly the
/// texels `other` paints identically inside the rows.
fn quad(texels: [[u8; 4]; 16]) -> Sprite {
    let mut bytes = Vec::with_capacity(64);
    for texel in texels {
        bytes.extend_from_slice(&texel);
    }
    Sprite::from_rgba(4, 4, Vec2::new(2.0, 2.0), &bytes).unwrap()
}

fn stamped(sprite: &Sprite) -> Canvas {
    let mut image = filled([0.0, 0.15, 0.0]);
    stamp_sprite(
        &mut image,
        SurfacePoint::new(Face::Front, 32.0, 32.0),
        Vec2::new(1.0, 0.0),
        sprite,
        1.0,
        1.0,
        &mut Vec::new(),
    );
    image
}

#[test]
fn subtract_clears_only_the_texels_other_paints_identically_inside_the_rows() {
    const A: [u8; 4] = [255, 40, 40, 255];
    const B: [u8; 4] = [40, 255, 40, 255];
    const C: [u8; 4] = [40, 40, 255, 255];
    const NONE: [u8; 4] = [0, 0, 0, 0];
    // Rows 0..4; the cleared texels are exactly the ones that agree inside rows 1..3.
    let base = quad([
        A, B, C, A, // row 0: outside the rows, kept even where it agrees
        A, B, NONE, C, // row 1
        B, A, C, NONE, // row 2
        A, A, A, A, // row 3: outside the rows
    ]);
    let other = quad([
        A, B, C, A, // row 0 agrees, but is outside the rows
        A, C, C, NONE, // row 1: texel 0 agrees -> cleared; 1 differs; 3 unpainted
        B, NONE, C, A, // row 2: texels 0 and 2 agree -> cleared
        A, A, A, A, // row 3 agrees, outside the rows
    ]);
    let expected = quad([
        A, B, C, A, //
        NONE, B, NONE, C, //
        NONE, A, NONE, NONE, //
        A, A, A, A, //
    ]);

    let cut = base.subtract(&other, 1..3);
    assert_identical(&stamped(&cut), &stamped(&expected), "subtract over rows 1..3");
    assert!(
        !stamped(&cut)
            .get(Face::Front, 30, 31)
            .eq(&stamped(&base).get(Face::Front, 30, 31)),
        "the fixture must actually clear something"
    );

    // The extent shrinks to what remains and never grows.
    assert!(cut.extent() <= base.extent());
    let corner = quad([
        A, NONE, NONE, A, //
        NONE, NONE, NONE, NONE, //
        NONE, NONE, NONE, NONE, //
        A, NONE, NONE, A, //
    ]);
    let inner = quad([
        NONE, NONE, NONE, NONE, //
        NONE, A, A, NONE, //
        NONE, A, A, NONE, //
        NONE, NONE, NONE, NONE, //
    ]);
    let trimmed = corner.subtract(&corner, 0..4);
    assert!(trimmed.extent() < corner.extent(), "clearing the outer texels must shrink the extent");
    assert_eq!(peak(&stamped(&trimmed)), peak(&filled([0.0, 0.15, 0.0])));
    assert!(inner.extent() < corner.extent());

    // Mismatched dimensions: an unchanged copy.
    let wide = Sprite::from_rgba(5, 4, Vec2::new(2.0, 2.0), &[255; 80]).unwrap();
    let untouched = base.subtract(&wide, 0..4);
    assert_eq!(untouched.width(), base.width());
    assert_eq!(untouched.height(), base.height());
    assert_eq!(untouched.extent(), base.extent());
    assert_identical(&stamped(&untouched), &stamped(&base), "subtract with wrong dimensions");

    // Rows beyond the sprite are clamped rather than panicking, and clear what they reach.
    let all = base.subtract(&base, 0..99);
    assert_eq!(all.extent(), 0.0, "subtracting a sprite from itself leaves nothing");
}

#[test]
fn paints_like_is_true_only_where_every_painted_texel_of_other_matches() {
    const A: [u8; 4] = [255, 40, 40, 255];
    const B: [u8; 4] = [40, 255, 40, 255];
    const NONE: [u8; 4] = [0, 0, 0, 0];
    let base = quad([
        A, A, A, A, //
        A, B, A, B, //
        B, A, B, A, //
        B, B, B, B, //
    ]);
    // Painted only where it agrees with `base` in rows 1..3, and differing in row 3.
    let tail = quad([
        NONE, NONE, NONE, NONE, //
        A, NONE, A, NONE, //
        NONE, A, NONE, A, //
        A, A, A, A, //
    ]);

    assert!(base.paints_like(&base, 0..4));
    assert!(base.paints_like(&tail, 1..3), "every painted texel of the tail agrees there");
    assert!(!base.paints_like(&tail, 0..4), "row 3 disagrees");
    assert!(base.paints_like(&tail, 0..1), "an all-transparent row compares vacuously true");
    // A transparent texel of `other` is never compared, so a sprite painting nothing in the
    // rows always agrees.
    let blank = quad([[NONE; 1][0]; 16]);
    assert!(base.paints_like(&blank, 0..4));
    // Mismatched dimensions are false, whatever the pixels.
    let wide = Sprite::from_rgba(5, 4, Vec2::new(2.0, 2.0), &[255; 80]).unwrap();
    assert!(!base.paints_like(&wide, 0..4));

    // And the two functions agree: what `subtract` cleared no longer paints like `other`.
    let cut = base.subtract(&tail, 1..3);
    assert!(!cut.paints_like(&tail, 1..3), "the cleared texels must no longer match");
}
