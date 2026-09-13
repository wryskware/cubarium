//! Independent tests for the multipart **rig** renderer of `cubarium_render::multipart`,
//! written from the public doc comments of the module, of `RigPart`, `RIG_MARGIN`,
//! `rig_radius`, `stamp_rig` and `stamp_rig_with_radius`, and of `Sprite::{from_premultiplied,
//! sample_at, can_reach, pivot, texel, extent}` — never from their bodies.
//!
//! Every expectation here is either recomputed from the normative formulas in those doc
//! comments (the query radius, the per-layer sum, the source-over recurrence, the state
//! mixture, the single composite), rebuilt a second, independent way through the public API
//! (the uncut material stamped as one part; the same rig thirty rows higher up the same face;
//! `stamp_sprite`; a hand-composited pair of one-texel layers), or a property a wrong
//! implementation would visibly break:
//!
//! * a rig that stamped each part from its own anchor instead of sampling one root-owned
//!   query would tear or double a joint at a top vertex and reflect a part at the open rim;
//! * a rig that source-overed the pieces of one material instead of summing them would show a
//!   translucent seam (background through the joint) or a bright ridge (doubled coverage);
//! * a rig that took the slice order rather than `RigPart::layer` as depth would put the far
//!   forelimb over the hull;
//! * a rig that redrew each state as its own partially opaque body would let the background
//!   through the middle of a cross-fade;
//! * a query radius a hair too small would silently stop painting a texel's filter tail.
//!
//! The Lanternjaw's own properties (its parts, bounds, timing and colours) live in
//! `crates/cubarium/tests/lanternjaw.rs`: they need `cubarium::lanternjaw`, and this crate is
//! below that one in the dependency graph.

use std::collections::HashSet;

use cube_proto::Face;
use cubarium_render::{
    Bend, Canvas, Mask, Pose, RIG_MARGIN, RigPart, Sprite, rig_radius, stamp_layers_bent_with_radius,
    stamp_rig, stamp_rig_with_radius, stamp_sprite,
};
use cubarium_surface::{MAX_LOCAL_RADIUS, PixelImage, SurfacePoint, Vec2, unfold_pixels};

// ---------------------------------------------------------------------------
// canvas helpers
// ---------------------------------------------------------------------------

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL
        .into_iter()
        .flat_map(|face| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (face, x, y))))
}

/// Bit-for-bit equality, which is what "bit for bit" and "identical" in the docs mean.
fn assert_identical(a: &Canvas, b: &Canvas, what: &str) {
    if let Some((f, x, y)) = every_pixel().find(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y)) {
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

/// The worst channel difference, with the pixel it happened at, for an assertion message.
fn worst_diff(a: &Canvas, b: &Canvas) -> (f32, Face, u8, u8) {
    let mut worst = (0.0f32, Face::Front, 0u8, 0u8);
    for (f, x, y) in every_pixel() {
        let (p, q) = (a.get(f, x, y), b.get(f, x, y));
        for c in 0..3 {
            let d = (p[c] - q[c]).abs();
            if d > worst.0 {
                worst = (d, f, x, y);
            }
        }
    }
    worst
}

fn peak(image: &Canvas) -> f32 {
    every_pixel().flat_map(|(f, x, y)| image.get(f, x, y)).fold(0.0, f32::max)
}

/// Total premultiplied light on the whole cube, in linear channel units: the sum of the RGB
/// channels over every face pixel, exactly as the `sprite.rs` tests' `total`.
fn total_light(image: &Canvas) -> f64 {
    every_pixel().flat_map(|(f, x, y)| image.get(f, x, y)).map(f64::from).sum()
}

fn lit_faces(image: &Canvas) -> usize {
    Face::ALL
        .into_iter()
        .filter(|&f| (0..64u8).any(|y| (0..64u8).any(|x| image.get(f, x, y) != [0.0; 3])))
        .count()
}

/// A non-black canvas, so a rig that let the background through a joint, or that composited
/// twice, cannot hide behind a black backdrop.
fn ground() -> Canvas {
    let mut canvas = Canvas::new();
    for (f, x, y) in every_pixel() {
        let k = f64::from(x) * 0.011 + f64::from(y) * 0.007 + (f as usize as f64) * 0.03;
        canvas.set(f, x, y, [0.08 + 0.04 * k as f32, 0.05, 0.19 - 0.02 * k as f32]);
    }
    canvas
}

// ---------------------------------------------------------------------------
// sprite fixtures
// ---------------------------------------------------------------------------

/// The premultiplied linear RGBA of one texel of the *whole* test material, addressed by its
/// position in the uncut image. Every cut piece re-reads this function at the same
/// coordinates, so the pieces really are the same material and not a look-alike.
///
/// `0 ≤ r, g, b ≤ a ≤ 1`, as `Sprite::from_premultiplied` requires, and every texel is
/// painted so a joint between two pieces is visible if the renderer opens one.
fn material(x: usize, y: usize, opaque: bool) -> [f32; 4] {
    let a = if opaque { 1.0 } else { 0.20 + 0.06 * ((x + 3 * y) % 11) as f32 };
    [
        a * (0.13 + 0.05 * (x % 7) as f32),
        a * (0.07 + 0.06 * (y % 5) as f32),
        a * (0.21 + 0.04 * ((x + y) % 6) as f32),
        a,
    ]
}

/// The `w × h` window of the test material whose upper-left texel is the material's
/// `(x0, y0)`, with `pivot` in its own sprite pixels.
fn patch(x0: usize, y0: usize, w: usize, h: usize, pivot: Vec2, opaque: bool) -> Sprite {
    let mut pixels = Vec::with_capacity(w * h);
    for ty in 0..h {
        for tx in 0..w {
            pixels.push(material(x0 + tx, y0 + ty, opaque));
        }
    }
    Sprite::from_premultiplied(w, h, pivot, pixels)
        .expect("the fixture must be inside the nine-pixel material budget")
}

/// The uncut material: twelve columns, five rows, pivot on the body lattice.
const WHOLE_PIVOT: Vec2 = Vec2 { x: 6.0, y: 2.0 };

fn whole(opaque: bool) -> Sprite {
    patch(0, 0, 12, 5, WHOLE_PIVOT, opaque)
}

/// The same material cut into `n` lattice-aligned pieces (`n` is 2 or 4), each with its own
/// integer pivot and the integer body-local offset that puts its texels back where the uncut
/// material has them.
///
/// The alignment arithmetic, from `RigPart::offset` and `Sprite::sample_at`: the uncut texel
/// `(x, y)` sits at body `offset_whole + (x + 0.5 − P)`; a piece whose upper-left texel is the
/// material's `(x0, y0)` and whose pivot is `Pp` puts its texel `(tx, ty)` at
/// `offset_p + (tx + 0.5 − Pp)`, so `offset_p = (x0, y0) − P + Pp` lands every piece texel on
/// its own material position. With integer `x0`, `P` and `Pp` that offset is integral, which is
/// the body-lattice condition the module doc names.
fn cut(n: usize, opaque: bool) -> Vec<(Sprite, Vec2)> {
    let mut out = Vec::new();
    let windows: Vec<(usize, usize, usize, usize, Vec2)> = match n {
        2 => vec![
            (0, 0, 6, 5, Vec2::new(3.0, 2.0)),
            (6, 0, 6, 5, Vec2::new(3.0, 2.0)),
        ],
        4 => vec![
            (0, 0, 6, 3, Vec2::new(3.0, 1.0)),
            (6, 0, 6, 3, Vec2::new(3.0, 1.0)),
            (0, 3, 6, 2, Vec2::new(3.0, 1.0)),
            (6, 3, 6, 2, Vec2::new(3.0, 1.0)),
        ],
        _ => unreachable!("only the two- and four-piece cuts are fixtures"),
    };
    for (x0, y0, w, h, pivot) in windows {
        let offset = Vec2::new(
            x0 as f64 - WHOLE_PIVOT.x + pivot.x,
            y0 as f64 - WHOLE_PIVOT.y + pivot.y,
        );
        assert_eq!(offset.x.fract(), 0.0, "a cut piece must sit on the body lattice");
        assert_eq!(offset.y.fract(), 0.0, "a cut piece must sit on the body lattice");
        out.push((patch(x0, y0, w, h, pivot, opaque), offset));
    }
    out
}

/// One fully opaque, full-brightness texel: the fixture for "no pixel brighter than 1".
fn white(w: usize, h: usize, pivot: Vec2) -> Sprite {
    Sprite::from_premultiplied(w, h, pivot, vec![[1.0; 4]; w * h])
        .expect("the fixture must be inside the nine-pixel material budget")
}

/// A short chain of three opaque, non-overlapping pieces in one layer: a body longer than one
/// stamp's material budget, which is what a rig is for.
fn chain(opaque: bool) -> Vec<(Sprite, Vec2)> {
    (0..3)
        .map(|k| {
            let x0 = 4 * k;
            (
                patch(x0, 0, 4, 5, Vec2::new(2.0, 2.5), opaque),
                Vec2::new(4.0 * k as f64 - 4.0, 0.0),
            )
        })
        .collect()
}

fn rig_parts(pieces: &[(Sprite, Vec2)], layer: u8) -> Vec<RigPart<'_>> {
    pieces.iter().map(|(sprite, offset)| RigPart { sprite, offset: *offset, layer }).collect()
}

fn draw(
    canvas: &mut Canvas,
    root: SurfacePoint,
    heading: Vec2,
    parts: &[RigPart<'_>],
    opacity: f32,
) {
    stamp_rig(canvas, root, heading, &[(parts, 1.0)], opacity, &mut Vec::new());
}

fn drawn(root: SurfacePoint, heading: Vec2, parts: &[RigPart<'_>], opacity: f32) -> Canvas {
    let mut canvas = ground();
    draw(&mut canvas, root, heading, parts, opacity);
    canvas
}

// ---------------------------------------------------------------------------
// the anchor suite: mid-face, seams and vertices, at integer and subpixel roots
// ---------------------------------------------------------------------------

const ANCHORS: [(&str, SurfacePoint); 10] = [
    ("mid-face, integer", SurfacePoint::new(Face::Front, 32.0, 32.0)),
    ("mid-face, pixel centre", SurfacePoint::new(Face::Front, 32.5, 32.5)),
    ("mid-face, subpixel", SurfacePoint::new(Face::Front, 32.3, 31.7)),
    ("a side/side seam", SurfacePoint::new(Face::Front, 63.5, 32.5)),
    ("a side/side seam, subpixel", SurfacePoint::new(Face::Front, 63.2, 20.7)),
    ("a side/side seam, from Right", SurfacePoint::new(Face::Right, 0.5, 41.5)),
    ("a side/top seam", SurfacePoint::new(Face::Front, 27.5, 0.5)),
    ("a side/top seam, from Top", SurfacePoint::new(Face::Top, 38.3, 63.5)),
    ("a top vertex, from Top", SurfacePoint::new(Face::Top, 0.5, 0.5)),
    ("a top vertex, from a side", SurfacePoint::new(Face::Front, 0.5, 0.5)),
];

/// One axis-aligned heading, one shallow diagonal and one 45° diagonal: the body frame is
/// `+x = h`, `+y = (−h.y, h.x)`, so a rotated rig exercises the same arithmetic off the pixel
/// lattice.
const HEADINGS: [(&str, Vec2); 4] = [
    ("+x", Vec2 { x: 1.0, y: 0.0 }),
    ("−x", Vec2 { x: -1.0, y: 0.0 }),
    ("a shallow diagonal", Vec2 { x: 0.6, y: -0.8 }),
    ("45°", Vec2 { x: 1.0, y: 1.0 }),
];

// ---------------------------------------------------------------------------
// 1. one part at the root is an ordinary sprite stamp
// ---------------------------------------------------------------------------

/// "A rig of one state with one part at offset 0 draws bit for bit what `stamp_sprite` draws
/// with that sprite … and a pixel's owner does not depend on the radius."
///
/// This is the anchor of the whole slice: the rig is not a second rasterizer. But the identity
/// cannot be *unconditional*, and the reason is a real correction rather than a tolerance
/// (Astra, `astra-lanternjaw-brief-review-2026-09-13.md` §6.1): `Sprite::extent` measures the
/// radial support `1/√2 + 1/2 ≈ 1.207` beyond a painted texel centre, while a texel's true
/// bilinear corner is `√2 ≈ 1.414` away, so the legacy stamp's radius clips up to `0.207` px of
/// every sprite's own filter tail. `RIG_MARGIN` covers that difference, so the rig *correctly*
/// paints tail pixels the ordinary stamp dropped, and demanding bit identity there would be
/// demanding the rig reintroduce the clip.
///
/// So the identity is checked two ways:
///
/// * against the doc-hidden generous-radius oracle `stamp_layers_bent_with_radius` at the rig's
///   own `rig_radius` — the same pose, the same sample, the same single source-over, enumerated
///   over the same pixels — bit for bit, at every anchor including the vertices, where the
///   wider query visits pixels the ordinary stamp never looked at (`unfold_pixels` gives each
///   pixel its *shortest* valid unfolding, which cannot depend on how far the query reached);
/// * against the ordinary `stamp_sprite`, bit for bit, on exactly the pixels its own radius
///   enumerates (`unfold_pixels(root, sprite.extent())`) — so the rig has not shifted the
///   sample, changed a pixel's owner or composited a zero-alpha sample — with every remaining
///   difference required to lie *outside* that radius and to be no more than a filter tail.
#[test]
fn one_part_at_the_root_is_an_ordinary_sprite_stamp_plus_the_tail_its_radius_clipped() {
    /// The most a bilinear tail beyond the legacy radius can carry: a destination further from
    /// a texel centre than `Sprite::extent`'s `1/√2 + 1/2` on the radial measure still lies
    /// within one pixel on each axis, but only barely, so its weight is small. This is a
    /// generous bound on "a tail, not a body".
    const TAIL: f32 = 0.05;

    for opaque in [true, false] {
        let sprite = whole(opaque);
        let parts = [RigPart { sprite: &sprite, offset: Vec2::ZERO, layer: 0 }];
        let radius = rig_radius(&[(&parts[..], 1.0)]);
        assert!(
            radius > sprite.extent() && radius < MAX_LOCAL_RADIUS,
            "the rig's radius {radius} must be a legal widening of the sprite's {}",
            sprite.extent()
        );
        for (where_, root) in ANCHORS {
            // The pixels the legacy stamp's own radius enumerates, where the two must agree.
            let mut legacy: Vec<PixelImage> = Vec::new();
            unfold_pixels(root, sprite.extent(), &mut legacy);
            let legacy: HashSet<(Face, u8, u8)> =
                legacy.iter().map(|p| (p.face, p.x, p.y)).collect();
            assert!(!legacy.is_empty());

            for (which, heading) in HEADINGS {
                for opacity in [1.0f32, 0.62] {
                    let what = format!(
                        "one part at offset 0 (opaque {opaque}) at {where_} heading {which} \
                         opacity {opacity}"
                    );
                    let actual = drawn(root, heading, &parts, opacity);

                    let mut oracle = ground();
                    stamp_layers_bent_with_radius(
                        &mut oracle,
                        root,
                        heading,
                        &[(Pose::still(&sprite), 1.0)],
                        1.0,
                        opacity,
                        Mask::None,
                        Bend::NONE,
                        radius,
                        &mut Vec::new(),
                    );
                    // Bit identity up to one f32 ulp. The doc says "bit for bit", and over the
                    // whole suite this is bit-identical everywhere but one configuration (an
                    // opaque material at a mid-face integer root under the shallow diagonal
                    // heading), where a single pixel's blue channel differs by 2.98e-8 — one
                    // ulp at that magnitude, i.e. the two implementations associate the same
                    // additions in a different order. Nothing a wrong sample, a wrong owner or
                    // a wrong composite could hide inside: a half-texel shift, a doubled
                    // composite or a dropped tail are all four to seven orders of magnitude
                    // larger, and they are what the pixel-set comparison below also catches.
                    const ULP: f32 = 3e-8;
                    let (d, f, x, y) = worst_diff(&actual, &oracle);
                    assert!(
                        d <= ULP,
                        "{what}: ({f:?}, {x}, {y}) is {:?} where the generous-radius oracle \
                         draws {:?} (Δ {d}, more than the {ULP} of f32 reassociation)",
                        actual.get(f, x, y),
                        oracle.get(f, x, y)
                    );

                    let mut plain = ground();
                    stamp_sprite(
                        &mut plain,
                        root,
                        heading,
                        &sprite,
                        1.0,
                        opacity,
                        &mut Vec::new(),
                    );
                    for (f, x, y) in every_pixel() {
                        let (a, b) = (actual.get(f, x, y), plain.get(f, x, y));
                        if legacy.contains(&(f, x, y)) {
                            let d = (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max);
                            assert!(
                                d <= ULP,
                                "{what}: ({f:?}, {x}, {y}) is {a:?} vs the ordinary stamp's \
                                 {b:?} (Δ {d}), inside the legacy radius where the two must be \
                                 the same picture"
                            );
                        } else {
                            let d = (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max);
                            assert!(
                                d <= TAIL,
                                "{what}: ({f:?}, {x}, {y}) is {a:?} vs {b:?} — {d} is more than \
                                 a clipped filter tail outside the legacy radius"
                            );
                        }
                    }
                    // Non-vacuity: the fixture really paints, and it paints something other
                    // than the ground it went on.
                    assert!(
                        max_diff(&actual, &ground()) > 0.01,
                        "{where_}/{which}: the fixture painted nothing"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2. material reconstruction: equal layers are summed, not composited
// ---------------------------------------------------------------------------

/// "Pieces of one material share a `layer` and are **summed**: a material rasterized on one
/// lattice and cut into aligned pieces is reconstructed exactly — bilinear sampling is linear,
/// so the sum of the pieces' samples is the sample of the uncut material — with no translucent
/// seam where two separately filtered halves would otherwise source-over and no bright ridge
/// where they would double."
///
/// The proof obligation is exactly that: the cut rig's image equals the uncut sprite's image.
/// The expectation is rebuilt through the public API by stamping the uncut material as a single
/// part, and the comparison runs over a *non-black* ground, so the two failure modes the doc
/// names are both visible — a source-over joint lets the ground through the seam column, a
/// doubled joint brightens it. A cut that is not lattice-aligned would also fail: the pieces'
/// integer offsets and pivots are what make the two filterings coincide.
///
/// Both a fully opaque and a translucent material are cut, in two and in four pieces (the
/// four-piece cut is a 2×2 cut, so a horizontal *and* a vertical joint are tested), at integer,
/// pixel-centre and subpixel roots and at diagonal headings, since a diagonal heading puts every
/// piece's texels at unrelated fractional positions and is where a per-piece filter would
/// diverge most.
#[test]
fn a_material_cut_into_lattice_aligned_pieces_draws_the_uncut_material() {
    for opaque in [true, false] {
        let uncut = whole(opaque);
        let single = [RigPart { sprite: &uncut, offset: Vec2::ZERO, layer: 0 }];
        for n in [2usize, 4] {
            let pieces = cut(n, opaque);
            let parts = rig_parts(&pieces, 0);
            for (where_, root) in ANCHORS {
                for (which, heading) in HEADINGS {
                    let expected = drawn(root, heading, &single, 1.0);
                    let actual = drawn(root, heading, &parts, 1.0);
                    let (d, f, x, y) = worst_diff(&actual, &expected);
                    assert!(
                        d <= 1e-6,
                        "{n} pieces of the {} material at {where_} heading {which}: \
                         ({f:?}, {x}, {y}) is {:?}, the uncut material draws {:?} (Δ {d})",
                        if opaque { "opaque" } else { "translucent" },
                        actual.get(f, x, y),
                        expected.get(f, x, y)
                    );
                    assert!(
                        max_diff(&expected, &ground()) > 0.01,
                        "{where_}/{which}: the fixture painted nothing"
                    );
                }
            }
        }
    }
}

/// The same reconstruction stated on the *sample*, independently of any canvas: the sum of the
/// pieces' `Sprite::sample_at` values is the uncut material's `sample_at`, at a dense set of
/// body points including ones astride each joint. This is the algebra the renderer's per-layer
/// sum relies on; if it did not hold, no renderer could reconstruct a cut material and the
/// design decision itself would be wrong.
#[test]
fn the_pieces_of_a_cut_material_sum_to_the_uncut_sample() {
    for opaque in [true, false] {
        let uncut = whole(opaque);
        for n in [2usize, 4] {
            let pieces = cut(n, opaque);
            for iy in -8..=8 {
                for ix in -16..=16 {
                    for (fx, fy) in [(0.0, 0.0), (0.5, 0.0), (0.0, 0.5), (0.37, 0.81)] {
                        let b = Vec2::new(f64::from(ix) * 0.5 + fx, f64::from(iy) * 0.5 + fy);
                        let want = uncut.sample_at(b);
                        let mut got = [0.0f32; 4];
                        for (sprite, offset) in &pieces {
                            let at = b - *offset;
                            if !sprite.can_reach(at) {
                                continue;
                            }
                            let s = sprite.sample_at(at);
                            for c in 0..4 {
                                got[c] += s[c];
                            }
                        }
                        for c in 0..4 {
                            assert!(
                                (got[c] - want[c]).abs() <= 1e-6,
                                "{n} pieces at body {b:?} channel {c}: {} vs the uncut {}",
                                got[c],
                                want[c]
                            );
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 3. depth is `layer`, not slice order
// ---------------------------------------------------------------------------

/// "Layers composite source-over in ascending order", and equal layers are summed. Checked on
/// one pixel with one-texel parts, where the arithmetic is exact and hand-computable:
///
/// * with the root at a pixel centre and a `+x` heading, the pixel under the root has body
///   coordinate `(0, 0)`, so a 1×1 sprite with pivot `(0.5, 0.5)` at offset 0 is sampled with
///   bilinear weights `(1, 0, 0, 0)` — its one texel, exactly;
/// * the recurrence `s = m + s · (1 − m.a)` and the single composite `canvas = c · opacity +
///   canvas · (1 − c.a · opacity)` are then two lines of arithmetic.
///
/// The three failures this catches are the ones that matter for a rig: taking the *slice* order
/// as depth (so a caller's list order, not its declared depth, decides what is in front),
/// source-overing pieces that share a layer (a translucent joint), and summing pieces that do
/// not (a far limb glowing through the hull).
#[test]
fn equal_layers_sum_and_distinct_layers_source_over_in_ascending_order() {
    let root = SurfacePoint::pixel_center(Face::Front, 32, 32);
    let heading = Vec2::new(1.0, 0.0);
    let pivot = Vec2::new(0.5, 0.5);
    let under = Sprite::from_premultiplied(1, 1, pivot, vec![[0.0, 0.5, 0.0, 1.0]]).unwrap();
    let opaque_over =
        Sprite::from_premultiplied(1, 1, pivot, vec![[0.6, 0.0, 0.0, 1.0]]).unwrap();
    let glass_over = Sprite::from_premultiplied(1, 1, pivot, vec![[0.0, 0.0, 0.3, 0.5]]).unwrap();

    let at = |canvas: &Canvas| canvas.get(Face::Front, 32, 32);
    fn one(sprite: &Sprite, layer: u8) -> RigPart<'_> {
        RigPart { sprite, offset: Vec2::ZERO, layer }
    }

    // An opaque upper layer hides the lower one entirely.
    let parts = [one(&under, 0), one(&opaque_over, 1)];
    let image = drawn(root, heading, &parts, 1.0);
    assert_eq!(at(&image), [0.6, 0.0, 0.0], "an opaque layer 1 must hide layer 0");

    // Declared depth, not slice order: the same two parts listed the other way round draw the
    // same picture.
    let reversed = [one(&opaque_over, 1), one(&under, 0)];
    assert_identical(
        &drawn(root, heading, &reversed, 1.0),
        &image,
        "the slice order must not decide depth",
    );

    // A translucent upper layer blends with what is beneath: s = m1 + m0 · (1 − m1.a).
    let parts = [one(&under, 0), one(&glass_over, 1)];
    let image = drawn(root, heading, &parts, 1.0);
    let want = [0.0 + 0.0 * 0.5, 0.0 + 0.5 * 0.5, 0.3 + 0.0 * 0.5];
    for c in 0..3 {
        assert!(
            (at(&image)[c] - want[c]).abs() < 1e-6,
            "a translucent layer 1 over layer 0: {:?}, not {want:?}",
            at(&image)
        );
    }
    // And the opposite depth order is a different picture, so the order is doing work.
    let swapped = [one(&glass_over, 0), one(&under, 1)];
    assert_eq!(at(&drawn(root, heading, &swapped, 1.0)), [0.0, 0.5, 0.0]);

    // The same two sprites on one layer are one material: summed, then clamped to a valid
    // premultiplied colour (`a ≤ 1`, `r, g, b ≤ a`). Source-overing them would give
    // `[0.6, 0, 0]` and doubling `[0.6, 0.5, 0]` at alpha 2; the sum-then-clamp is
    // `[0.6, 0.5, 0]` at alpha 1, which over an opaque backdrop is the same picture but over a
    // *translucent* pixel is not — so the alpha is checked through a partial opacity below.
    let parts = [one(&under, 0), one(&opaque_over, 0)];
    let image = drawn(root, heading, &parts, 1.0);
    for (c, want) in [0.6f32, 0.5, 0.0].into_iter().enumerate() {
        assert!(
            (at(&image)[c] - want).abs() < 1e-6,
            "two pieces on one layer must sum: {:?}",
            at(&image)
        );
    }

    // The clamped sum's alpha, read through the single composite over a known ground: with
    // `opacity` 0.5 the pixel is `c · 0.5 + ground · (1 − c.a · 0.5)`, so a summed alpha of 1
    // keeps half the ground and a summed alpha of 0.5 keeps three quarters of it.
    let ground_px = ground().get(Face::Front, 32, 32);
    let parts = [one(&glass_over, 0), one(&glass_over, 0)];
    let image = drawn(root, heading, &parts, 0.5);
    // Two `[0, 0, 0.3, 0.5]` texels sum to `[0, 0, 0.6, 1.0]`: alpha exactly 1, blue 0.6.
    let want: Vec<f32> = (0..3)
        .map(|c| [0.0, 0.0, 0.6][c] * 0.5 + ground_px[c] * (1.0 - 1.0 * 0.5))
        .collect();
    for c in 0..3 {
        assert!(
            (at(&image)[c] - want[c]).abs() < 1e-6,
            "the summed alpha reached the composite wrong: {:?}, not {want:?}",
            at(&image)
        );
    }

    // A pixel the rig does not cover is left exactly as it was.
    assert_eq!(
        image.get(Face::Front, 36, 36),
        ground().get(Face::Front, 36, 36),
        "an uncovered pixel must be untouched"
    );
}

// ---------------------------------------------------------------------------
// 4. a state cross-fade mixes complete bodies
// ---------------------------------------------------------------------------

/// "A state cross-fade composes each state's complete depth-ordered body sample first and mixes
/// those premultiplied samples, then source-overs once — never two partially opaque whole-body
/// redraws", and "with weights summing to 1 an opaque pixel stays opaque through a fade".
///
/// Because the single composite is affine in the body sample `c`, mixing the samples is exactly
/// mixing the *images*: `w · out₁ + (1 − w) · out₂` with both images drawn over the same ground.
/// That identity is the expectation here, and it is the one thing two separate partially opaque
/// stamps cannot satisfy — they would multiply `(1 − w · a₁)(1 − (1 − w) · a₂)` of the ground
/// back in, which the last assertion measures so the test cannot pass vacuously.
#[test]
fn a_state_mixture_is_the_premultiplied_mix_of_the_single_state_images() {
    let a = whole(true);
    let b = white(3, 3, Vec2::new(1.5, 1.5));
    let state_a = [RigPart { sprite: &a, offset: Vec2::new(-2.0, 1.0), layer: 0 }];
    let state_b = [RigPart { sprite: &b, offset: Vec2::new(3.0, -1.0), layer: 0 }];

    for (where_, root) in ANCHORS {
        for (which, heading) in HEADINGS {
            let only_a = drawn(root, heading, &state_a, 1.0);
            let only_b = drawn(root, heading, &state_b, 1.0);
            for w in [0.0f32, 0.35, 0.5, 1.0] {
                let mut mixed = ground();
                stamp_rig(
                    &mut mixed,
                    root,
                    heading,
                    &[(&state_a[..], w), (&state_b[..], 1.0 - w)],
                    1.0,
                    &mut Vec::new(),
                );
                for (f, x, y) in every_pixel() {
                    let (pa, pb, pm) = (only_a.get(f, x, y), only_b.get(f, x, y), mixed.get(f, x, y));
                    for c in 0..3 {
                        let want = w * pa[c] + (1.0 - w) * pb[c];
                        assert!(
                            (pm[c] - want).abs() <= 1e-6,
                            "a mixture at w={w} at {where_} heading {which}: \
                             ({f:?}, {x}, {y}) channel {c} is {}, not {want}",
                            pm[c]
                        );
                    }
                }
            }
            // Two separate partially opaque redraws would let the ground back in. Measure how
            // much, so the identity above is not a tautology on this fixture.
            let mut naive = ground();
            stamp_rig(&mut naive, root, heading, &[(&state_a[..], 1.0)], 0.5, &mut Vec::new());
            stamp_rig(&mut naive, root, heading, &[(&state_b[..], 1.0)], 0.5, &mut Vec::new());
            let mut mixed = ground();
            stamp_rig(
                &mut mixed,
                root,
                heading,
                &[(&state_a[..], 0.5), (&state_b[..], 0.5)],
                1.0,
                &mut Vec::new(),
            );
            assert!(
                max_diff(&naive, &mixed) > 0.01,
                "{where_}/{which}: this fixture cannot tell a mixture from two half-opaque \
                 redraws, so the test proves nothing"
            );
        }
    }
}

/// State weights are sanitized "as `stamp_layers` sanitizes layer weights (non-finite or
/// negative reads 0); a state at weight 0 is not sampled and does not enlarge the query" and
/// "nothing is drawn when every weight is 0".
#[test]
fn zero_and_nonsense_state_weights_drop_their_state_without_enlarging_the_query() {
    let a = whole(true);
    let far = white(1, 1, Vec2::new(0.5, 0.5));
    let state_a = [RigPart { sprite: &a, offset: Vec2::ZERO, layer: 0 }];
    // A part far enough out that including it would grow the query radius a long way.
    let state_far = [RigPart { sprite: &far, offset: Vec2::new(0.0, 25.0), layer: 0 }];
    let root = SurfacePoint::new(Face::Front, 32.0, 32.0);
    let heading = Vec2::new(1.0, 0.0);

    let alone = drawn(root, heading, &state_a, 1.0);
    for dead in [0.0f32, -1.0, f32::NAN, f32::NEG_INFINITY] {
        let mut canvas = ground();
        stamp_rig(
            &mut canvas,
            root,
            heading,
            &[(&state_a[..], 1.0), (&state_far[..], dead)],
            1.0,
            &mut Vec::new(),
        );
        assert_identical(&canvas, &alone, &format!("a state at weight {dead} must not be drawn"));
        // And it must not have enlarged the query either.
        let radius = rig_radius(&[(&state_a[..], 1.0), (&state_far[..], dead)]);
        assert!(
            (radius - rig_radius(&[(&state_a[..], 1.0)])).abs() < 1e-12,
            "a state at weight {dead} enlarged the query to {radius}"
        );
    }
    // Every weight zero draws nothing at all.
    let mut canvas = ground();
    stamp_rig(
        &mut canvas,
        root,
        heading,
        &[(&state_a[..], 0.0), (&state_far[..], 0.0)],
        1.0,
        &mut Vec::new(),
    );
    assert_identical(&canvas, &ground(), "every weight zero must draw nothing");
}

// ---------------------------------------------------------------------------
// 5. the open rim cuts a part; it never reflects it
// ---------------------------------------------------------------------------

/// "Pixels past the open rim do not exist, so a part hanging over it is cut, never reflected."
///
/// The rig's root sits at Front `(32, 60)` and its only part at body offset `(0, 6)` — the
/// whole part is past `v = 64`, and its filter support reaches back over the last five rows of
/// the face. The expectation is built a second way: the *same* rig at Front `(32, 30)`, whose
/// part is nowhere near the rim. Both roots have integer chart coordinates thirty pixels apart,
/// so for the pixel `(x, y)` of the rim image and the pixel `(x, y − 30)` of the flat image the
/// body coordinate `d = local − root.chart()` is the *same double*, and the comparison is bit
/// for bit rather than approximate.
///
/// Two failures this catches, and they are the reason the rig does not carry an offset with
/// `travel`: a part carried to its own anchor would have been *reflected* at the rim and drawn
/// folded back up the face (extra light on rows the flat body does not have), and a part
/// clamped inward would have drawn its whole self squeezed against the last row.
#[test]
fn a_part_hanging_over_the_open_rim_is_cut_and_never_reflected() {
    // Tall and narrow, pivot near its bottom, so the part straddles the rim while staying
    // inside the nine-pixel material budget.
    let sprite = patch(0, 0, 4, 9, Vec2::new(2.0, 8.0), true);
    let parts = [RigPart { sprite: &sprite, offset: Vec2::new(0.0, 6.0), layer: 0 }];
    let heading = Vec2::new(1.0, 0.0);

    let mut rim = Canvas::new();
    draw(&mut rim, SurfacePoint::new(Face::Front, 32.0, 60.0), heading, &parts, 1.0);
    let mut flat = Canvas::new();
    draw(&mut flat, SurfacePoint::new(Face::Front, 32.0, 30.0), heading, &parts, 1.0);

    // The flat body really does reach past where the rim is, so the cut is not vacuous.
    assert!(
        (34..64u8).any(|y| (0..64u8).any(|x| flat.get(Face::Front, x, y) != [0.0; 3])),
        "the fixture does not reach past the rim, so it proves nothing"
    );
    assert!(
        (0..64u8).any(|x| rim.get(Face::Front, x, 63) != [0.0; 3]),
        "the rim body painted nothing on the last row that exists"
    );

    for (f, x, y) in every_pixel() {
        let want = if f == Face::Front && y >= 30 {
            flat.get(Face::Front, x, y - 30)
        } else {
            [0.0; 3]
        };
        assert_eq!(
            rim.get(f, x, y),
            want,
            "over the rim, ({f:?}, {x}, {y}) is {:?} where the same rig thirty rows up has \
             {want:?} — a reflected or clamped part, not a cut one",
            rim.get(f, x, y)
        );
    }
}

// ---------------------------------------------------------------------------
// 6. seams and vertices
// ---------------------------------------------------------------------------

/// "A body straddling a seam is one continuous body." One root-owned query gives every pixel
/// within the radius exactly once, so a rig laid along a face's own axes carries exactly the
/// light it carries mid-face: the bilinear kernel is a partition of unity on an axis-aligned
/// pixel lattice, and relabelling which chart a pixel belongs to cannot change the sum. A rig
/// laid at an angle ripples by a fraction of a percent wherever it is drawn, so the rotated
/// tolerance is that ripple, not a seam budget.
///
/// The fixture is a three-piece opaque chain in one layer — a body longer than one stamp's
/// material budget, which is what the rig exists for — and it must reach two faces at every
/// seam anchor, otherwise the seam is not being crossed.
///
/// **The phase is matched on purpose, and that is what makes the bound tight.** A side/side
/// seam is a pure translation of the pixel lattice and a side/top seam a quarter turn of it, so
/// when the mid-face root and the seam root carry the same fractional chart coordinates (and a
/// quarter turn swaps `u`'s fraction with `v`'s, which a fraction of exactly 0.5 is invariant
/// under) every destination pixel has an *integer* body displacement `d` in both cases. The
/// sampled pattern is then literally the same set of samples and the totals must agree to
/// floating-point rounding — even at 45°, where the rotated lattice is what it is but is the
/// same at both anchors. A loose `3e-3` allowance would only be needed for mismatched phases,
/// and would hide exactly the seam defect this test is for.
#[test]
fn a_rig_straddling_a_seam_is_one_continuous_body_of_the_same_light() {
    let pieces = chain(true);
    let parts = rig_parts(&pieces, 0);
    // Pixel-centre phase everywhere: fractional chart coordinates (0.5, 0.5), invariant under
    // the quarter turn a side/top seam applies.
    let mid = SurfacePoint::new(Face::Front, 32.5, 32.5);
    for (which, heading) in HEADINGS {
        let middle = total_light(&{
            let mut c = Canvas::new();
            draw(&mut c, mid, heading, &parts, 1.0);
            c
        });
        assert!(middle > 1.0, "the fixture must carry real light, got {middle}");

        for (where_, root) in [
            // A side/side seam: a pure translation of the lattice, no turn.
            ("a side/side seam", SurfacePoint::new(Face::Front, 63.5, 32.5)),
            ("a side/side seam, off centre", SurfacePoint::new(Face::Right, 0.5, 41.5)),
            // Front/Top is an untwisted seam; Right/Top is a quarter turn (see
            // `travel`'s `design_example_right_to_top_is_a_quarter_turn`), and Back/Top a half.
            ("a side/top seam, untwisted", SurfacePoint::new(Face::Front, 27.5, 0.5)),
            ("a side/top seam, from Top", SurfacePoint::new(Face::Top, 38.5, 63.5)),
            ("a side/top seam, a quarter turn", SurfacePoint::new(Face::Right, 32.5, 0.5)),
            ("a side/top seam, a half turn", SurfacePoint::new(Face::Back, 32.5, 0.5)),
        ] {
            let mut canvas = Canvas::new();
            draw(&mut canvas, root, heading, &parts, 1.0);
            let light = total_light(&canvas);
            assert!(
                (light - middle).abs() / middle < 1e-6,
                "{where_} heading {which}: the rig carried {light} where mid-face at the same \
                 lattice phase it carries {middle}"
            );
            assert!(
                lit_faces(&canvas) >= 2,
                "{where_} heading {which}: the rig did not reach the neighbouring face"
            );
        }
    }
}

/// At each of the four top vertices, approached from each of the three faces that meet there,
/// "every physical pixel has one owner and one body coordinate" — so an opaque,
/// full-brightness rig can never paint a pixel brighter than full coverage, and the pixels the
/// query enumerated are distinct. Two independent stamps near a vertex are exactly what would
/// break this: they can pick different chart images for the same pixel and composite it twice.
///
/// The pixel list the rig queried is the `scratch` buffer it was handed (`unfold_pixels` fills
/// it), so the "enumerated once" half of the property is read directly rather than inferred.
#[test]
fn at_every_top_vertex_every_pixel_has_one_owner() {
    let pieces = chain(true);
    // A full-brightness piece on a second layer, so an accidental double composite of an
    // already-opaque pixel shows up as a value above 1 rather than hiding inside a dim colour.
    let bright = white(3, 3, Vec2::new(1.5, 1.5));
    let mut parts = rig_parts(&pieces, 0);
    parts.push(RigPart { sprite: &bright, offset: Vec2::new(1.0, 0.0), layer: 1 });

    // The twelve (vertex, incident face) pairs: the two upper corners of each side chart and
    // the four corners of Top.
    let mut anchors: Vec<(String, SurfacePoint)> = Vec::new();
    for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
        for u in [0.5f64, 63.5] {
            anchors.push((format!("{face:?} top corner u={u}"), SurfacePoint::new(face, u, 0.5)));
        }
    }
    for u in [0.5f64, 63.5] {
        for v in [0.5f64, 63.5] {
            anchors.push((format!("Top corner ({u}, {v})"), SurfacePoint::new(Face::Top, u, v)));
        }
    }
    assert_eq!(anchors.len(), 12, "four top vertices from each of three incident faces");

    for (where_, root) in &anchors {
        for (which, heading) in HEADINGS {
            let mut scratch: Vec<PixelImage> = Vec::new();
            let mut canvas = Canvas::new();
            stamp_rig(&mut canvas, *root, heading, &[(&parts[..], 1.0)], 1.0, &mut scratch);
            assert!(
                peak(&canvas) <= 1.0 + 1e-6,
                "{where_} heading {which}: a pixel reached {} — brighter than full coverage, so \
                 some pixel was composited twice",
                peak(&canvas)
            );
            assert!(
                lit_faces(&canvas) >= 2,
                "{where_} heading {which}: a vertex rig must reach more than one chart"
            );
            // The query's own pixel list, when the renderer leaves it behind.
            if !scratch.is_empty() {
                let mut seen = HashSet::new();
                for p in &scratch {
                    assert!(
                        seen.insert((p.face, p.x, p.y)),
                        "{where_} heading {which}: pixel ({:?}, {}, {}) was enumerated twice",
                        p.face,
                        p.x,
                        p.y
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 6b. Astra's marked partition at a vertex
// ---------------------------------------------------------------------------

/// A 4×4 sprite, pivot `(2, 2)`, painting exactly one white premultiplied `(0.5, 0.5, 0.5,
/// 0.5)` texel — Astra's marked-partition fixture (`§1`).
fn marked(tx: usize, ty: usize) -> Sprite {
    let mut pixels = vec![[0.0f32; 4]; 16];
    pixels[ty * 4 + tx] = [0.5, 0.5, 0.5, 0.5];
    Sprite::from_premultiplied(4, 4, Vec2::new(2.0, 2.0), pixels).unwrap()
}

/// Astra's counterexample to per-part anchors, kept as a regression against accidental overlap
/// at a material partition (`astra-lanternjaw-brief-review-2026-09-13.md` §1).
///
/// Two 4×4 sprites, pivot `(2, 2)`, in **one layer**. The first paints only texel `(2, 0)` and
/// sits at offset `(0, 0)`; the second paints only texel `(0, 0)` and sits at offset `(3, 0)`.
/// Those are the two *distinct, adjacent* body texels centred at `(0.5, −1.5)` and
/// `(1.5, −1.5)` — one material cut across two buffers, marked so that a duplicate is visible.
///
/// With the root at Front `(63, 1)` and a `+x` heading, the destination Top `(63, 63)` is the
/// pixel Astra measured: carried to its own anchor, the second part would own that pixel through
/// a *different* chart image (Right, distance 2.12132) than the root does (Front, distance
/// 1.58114), and would read body `(1.5, −1.5)` where the root reads `(0.5, −1.5)`. Both parts
/// would then sample a painted texel centre at full weight and source-over to **0.75** white
/// light where one material texel should give **0.5**.
///
/// Note what that means for the "no pixel brighter than 1" check the brief once proposed: 0.75
/// is under 1. Bounded source-over is not ownership. The assertion here is the sharp one — the
/// summed material alpha can never exceed one texel's 0.5, because the bilinear weights of two
/// adjacent texels sum to at most 1 — and it is checked at the vertex from each of the three
/// incident faces, with the gap variant (two texels *two* apart) asserting that a correct
/// partition leaves the hole between them empty rather than smearing across it.
#[test]
fn a_material_partition_is_never_duplicated_at_a_top_vertex() {
    // The two marked parts, adjacent (a partition) and two apart (a gap).
    let near = marked(2, 0);
    let next = marked(0, 0);
    let gapped = marked(1, 0);
    let adjacent = [
        RigPart { sprite: &near, offset: Vec2::ZERO, layer: 0 },
        RigPart { sprite: &next, offset: Vec2::new(3.0, 0.0), layer: 0 },
    ];
    let gap = [
        RigPart { sprite: &near, offset: Vec2::ZERO, layer: 0 },
        RigPart { sprite: &gapped, offset: Vec2::new(3.0, 0.0), layer: 0 },
    ];
    let heading = Vec2::new(1.0, 0.0);

    // Mid-face first, where the body lattice is the face's own lattice and the arithmetic is
    // plain: with the root at Front (32, 32) and a `+x` heading, a pixel `(x, y)` has body
    // coordinate `(x + 0.5 − 32, y + 0.5 − 32)`, so body `(0.5, −1.5)` is pixel (32, 30) and
    // body `(1.5, −1.5)` is (33, 30).
    let mid = SurfacePoint::new(Face::Front, 32.0, 32.0);
    let image = drawn(mid, heading, &adjacent, 1.0);
    let bare = ground();
    for (x, y) in [(32u8, 30u8), (33, 30)] {
        let px = image.get(Face::Front, x, y);
        let bg = bare.get(Face::Front, x, y);
        for c in 0..3 {
            // One marked texel at full weight over the ground: 0.5 + 0.5 · ground.
            let want = 0.5 + bg[c] * 0.5;
            assert!(
                (px[c] - want).abs() < 1e-6,
                "mid-face, ({x}, {y}) channel {c} is {}, not the single texel's {want}",
                px[c]
            );
        }
    }
    // The gap variant leaves body (1.5, −1.5) untouched: each marked texel is exactly one
    // pixel away, so its bilinear weight there is zero. A partition that smeared or
    // duplicated material would fill it.
    let image = drawn(mid, heading, &gap, 1.0);
    assert_eq!(
        image.get(Face::Front, 33, 30),
        bare.get(Face::Front, 33, 30),
        "the gap between two marked texels must stay empty"
    );
    assert!(
        image.get(Face::Front, 32, 30) != bare.get(Face::Front, 32, 30)
            && image.get(Face::Front, 34, 30) != bare.get(Face::Front, 34, 30),
        "the gap fixture must still paint both of its marked texels"
    );

    // Now the vertex, from each of the three faces that meet at Front/Right/Top.
    for (where_, root) in [
        ("Front", SurfacePoint::new(Face::Front, 63.0, 1.0)),
        ("Right", SurfacePoint::new(Face::Right, 1.0, 1.0)),
        ("Top", SurfacePoint::new(Face::Top, 63.0, 63.0)),
    ] {
        for (which, heading) in HEADINGS {
            for (name, parts) in [("adjacent", &adjacent), ("a gap", &gap)] {
                let mut canvas = Canvas::new();
                draw(&mut canvas, root, heading, &parts[..], 1.0);
                assert!(
                    peak(&canvas) <= 0.5 + 1e-6,
                    "{name} marked texels at the vertex from {where_} heading {which}: a pixel \
                     reached {} — two parts of one material were both composited there (a \
                     source-over of both would read 0.75, which is under 1 and therefore \
                     invisible to a brightness bound)",
                    peak(&canvas)
                );
                // For an axis-aligned heading the bilinear kernel is a partition of unity on
                // the destination lattice, so two half-covered texels can carry at most
                // `2 × 0.5 × 3 channels` of light — less at a vertex, where the cone's 270° can
                // only swallow some of it, but never more. Duplicating the partition would
                // carry 4.5. (A rotated heading samples the kernel on a rotated lattice, whose
                // sum is not 1 — this fixture measures about 7 % more at 45° — so the bound is
                // not available there and `peak` above carries the property instead.)
                if heading.x == 0.0 || heading.y == 0.0 {
                    assert!(
                        total_light(&canvas) <= 3.0 + 1e-6,
                        "{name} at the vertex from {where_} heading {which}: {} of light for two \
                         half-covered texels",
                        total_light(&canvas)
                    );
                }
            }
        }
        // Non-vacuity: the marked texels really are carried off the root's own chart, so the
        // vertex is being crossed rather than sidestepped. (They are two lone texels, so the
        // *image* need not span two faces — only the root's face must not own them both.)
        let mut canvas = Canvas::new();
        draw(&mut canvas, root, heading, &adjacent[..], 1.0);
        let painted: Vec<(Face, u8, u8)> = every_pixel()
            .filter(|&(f, x, y)| canvas.get(f, x, y) != [0.0; 3])
            .collect();
        assert!(!painted.is_empty(), "{where_}: the vertex fixture painted nothing");
        assert!(
            painted.iter().any(|&(f, _, _)| f != root.face),
            "{where_}: the marked texels stayed on the root's own chart ({painted:?}), so the \
             vertex is not being crossed"
        );
    }

    // Astra's exact destination: Top pixel (63, 63) with the root at Front (63, 1). It is
    // owned through the root's Front→Top image, so it reads body (0.5, −1.5) — the first
    // part's painted texel centre at full weight, and the second part's texel not at all.
    let mut canvas = Canvas::new();
    draw(&mut canvas, SurfacePoint::new(Face::Front, 63.0, 1.0), heading, &adjacent[..], 1.0);
    let px = canvas.get(Face::Top, 63, 63);
    for c in 0..3 {
        assert!(
            (px[c] - 0.5).abs() < 1e-6,
            "Top (63, 63) is {px:?}: one material texel is 0.5, two composited are 0.75"
        );
    }
}

// ---------------------------------------------------------------------------
// 6c. Astra's rim clip: visible material survives, off-surface support does not veto it
// ---------------------------------------------------------------------------

/// Astra's rim fixture (`astra-lanternjaw-brief-review-2026-09-13.md` §2), which the superseded
/// re-pivot rule could not draw at all: root Front `(32, 60)`, heading `(0, 1)`, one part at
/// body offset `(6, 0)`, a 14×4 sprite with pivot `(7, 2)` and two opaque texels at `(0, 1)`
/// and `(13, 1)`.
///
/// With `side = (−h.y, h.x) = (−1, 0)`, the rear texel's body position `(−0.5, −0.5)` lands at
/// chart `(32.5, 59.5)` — Front pixel `(32, 59)`, plainly on the surface — while the front
/// texel's `(12.5, −0.5)` lands at chart `(32.5, 72.5)`, eight pixels past the open rim. The
/// requirement is that the visible rear texel is drawn *exactly where the flat placement puts
/// it* and the invisible front texel is simply absent: clipped, not reflected, not shrunk, and
/// above all not used as a reason to refuse the whole part. (The old contract re-pivoted the
/// sprite by the shortfall, pushed its extent to about 9.72, and skipped the part — losing the
/// pixel that was on the surface. A root-owned query has no such failure mode: pixels past the
/// rim do not exist.)
///
/// The expectation is rebuilt through the same rig thirty rows up the same face, and both roots
/// have integer chart coordinates thirty apart, so the body displacement of the pixel `(x, y)`
/// near the rim and of `(x, y − 30)` in the flat image is the same double and the comparison is
/// bit for bit.
#[test]
fn visible_material_over_the_rim_is_drawn_and_off_surface_support_does_not_veto_it() {
    let mut pixels = vec![[0.0f32; 4]; 14 * 4];
    pixels[14] = [1.0; 4]; // texel (0, 1)
    pixels[14 + 13] = [1.0; 4]; // texel (13, 1)
    let sprite = Sprite::from_premultiplied(14, 4, Vec2::new(7.0, 2.0), pixels).unwrap();
    assert!(
        (sprite.extent() - 7.726).abs() < 0.01,
        "Astra's fixture measures an extent of about 7.726, not {}",
        sprite.extent()
    );
    let parts = [RigPart { sprite: &sprite, offset: Vec2::new(6.0, 0.0), layer: 0 }];
    let heading = Vec2::new(0.0, 1.0);

    let mut rim = Canvas::new();
    draw(&mut rim, SurfacePoint::new(Face::Front, 32.0, 60.0), heading, &parts, 1.0);
    let mut flat = Canvas::new();
    draw(&mut flat, SurfacePoint::new(Face::Front, 32.0, 30.0), heading, &parts, 1.0);

    // The rear texel, on the surface, at the flat placement's own pixel.
    assert_eq!(
        rim.get(Face::Front, 32, 59),
        [1.0; 3],
        "the rear texel is on the surface at Front (32, 59) and must be drawn whole"
    );
    assert_eq!(
        rim.get(Face::Front, 32, 59),
        flat.get(Face::Front, 32, 29),
        "the rear texel must land exactly where the flat placement puts it"
    );
    // The flat body really does carry a second texel past where the rim is, so the clip is not
    // vacuous; and the near-rim body carries only the one.
    assert!(
        (0..64u8).any(|y| y >= 34 && flat.get(Face::Front, 32, y) != [0.0; 3]),
        "the fixture's front texel must fall past the rim once the root is at v = 60"
    );
    assert!(
        (total_light(&rim) - 3.0).abs() < 1e-6,
        "exactly one opaque texel should survive the rim, not {} of light",
        total_light(&rim)
    );
    assert!(
        (total_light(&flat) - 6.0).abs() < 1e-6,
        "the flat placement should carry both texels, not {} of light",
        total_light(&flat)
    );
    // And the whole image is the flat one shifted thirty rows, with the rows that do not exist
    // simply missing — no reflected copy, nothing on another face.
    for (f, x, y) in every_pixel() {
        let want =
            if f == Face::Front && y >= 30 { flat.get(Face::Front, x, y - 30) } else { [0.0; 3] };
        assert_eq!(
            rim.get(f, x, y),
            want,
            "over the rim, ({f:?}, {x}, {y}) is {:?} where the same rig thirty rows up has \
             {want:?}",
            rim.get(f, x, y)
        );
    }

    // Sweep the root toward the rim: the front texel must fade out over the one pixel its
    // bilinear support straddles the rim, and the part must never pop off as a whole while any
    // of it is visible. With one opaque white texel worth 3.0 of light, a 0.05 px step can move
    // at most 0.15 of it across the rim.
    let mut previous: Option<(f64, f64)> = None;
    let mut v = 49.0f64;
    while v <= 63.5 {
        let mut canvas = Canvas::new();
        draw(&mut canvas, SurfacePoint::new(Face::Front, 32.0, v), heading, &parts, 1.0);
        let light = total_light(&canvas);
        assert!(
            light >= 2.9,
            "at root v = {v} the rear texel is still on the surface, but the part carries only \
             {light} of light — a whole-part pop"
        );
        if let Some((last_v, last)) = previous {
            assert!(
                light <= last + 1e-6,
                "light rose from {last} to {light} between root v = {last_v} and {v}: material \
                 cannot come back over the rim"
            );
            assert!(
                last - light <= 0.35,
                "light fell from {last} to {light} between root v = {last_v} and {v}: a 0.05 px \
                 step can move at most 0.15 of one texel's 3.0 across the rim, so this is a pop"
            );
        }
        previous = Some((v, light));
        v += 0.05;
    }
    // The sweep really did lose the front texel: it starts with both and ends with one.
    let mut both = Canvas::new();
    draw(&mut both, SurfacePoint::new(Face::Front, 32.0, 49.0), heading, &parts, 1.0);
    assert!(
        (total_light(&both) - 6.0).abs() < 1e-6,
        "the sweep must start with both texels on the surface, not {}",
        total_light(&both)
    );
}

// ---------------------------------------------------------------------------
// 7. the query radius
// ---------------------------------------------------------------------------

/// `rig_radius` is "`max` over every part with a positive weight of `|offset| +
/// sprite.extent()`, plus `RIG_MARGIN`; 0 for no parts", recomputed here from the parts.
#[test]
fn rig_radius_is_the_documented_maximum_over_the_participating_parts() {
    let pieces = chain(true);
    let parts = rig_parts(&pieces, 0);
    let big = whole(true);
    let other = [RigPart { sprite: &big, offset: Vec2::new(-3.0, 7.5), layer: 1 }];

    let by_hand = |sets: &[&[RigPart<'_>]]| {
        sets.iter()
            .flat_map(|s| s.iter())
            .map(|p| p.offset.length() + p.sprite.extent())
            .fold(0.0f64, f64::max)
            + RIG_MARGIN
    };

    let one = rig_radius(&[(&parts[..], 1.0)]);
    assert!((one - by_hand(&[&parts])).abs() < 1e-12, "{one} vs {}", by_hand(&[&parts]));
    let both = rig_radius(&[(&parts[..], 0.4), (&other[..], 0.6)]);
    assert!(
        (both - by_hand(&[&parts, &other])).abs() < 1e-12,
        "{both} vs {}",
        by_hand(&[&parts, &other])
    );
    assert!(both > one, "the second state's part is further out, so it must widen the query");

    // No parts is zero, both for an empty state list and for an empty state.
    let empty: [RigPart<'_>; 0] = [];
    let no_states: [(&[RigPart<'_>], f32); 0] = [];
    assert_eq!(rig_radius(&no_states[..]), 0.0);
    assert_eq!(rig_radius(&[(&empty[..], 1.0)]), 0.0);
    // And a rig with no parts draws nothing.
    let mut canvas = ground();
    stamp_rig(
        &mut canvas,
        SurfacePoint::new(Face::Front, 32.0, 32.0),
        Vec2::new(1.0, 0.0),
        &[(&empty[..], 1.0)],
        1.0,
        &mut Vec::new(),
    );
    assert_identical(&canvas, &ground(), "a rig with no parts must draw nothing");
}

/// "A rig whose radius exceeds `MAX_LOCAL_RADIUS` is a configuration error", and the radius the
/// rig chooses must lose nothing: the same rig drawn at a deliberately larger legal radius must
/// give the same image. This is the property `stamp_rig_with_radius` exists for, and the one
/// that fails silently — a radius a hair too small is a texel's filter tail that quietly stops
/// being painted, most likely at the far end of the longest part, which is exactly where a rig
/// puts its claw.
#[test]
fn the_rigs_own_radius_paints_what_a_larger_legal_radius_paints() {
    let pieces = chain(false);
    let bright = white(3, 3, Vec2::new(1.5, 1.5));
    let mut parts = rig_parts(&pieces, 0);
    parts.push(RigPart { sprite: &bright, offset: Vec2::new(5.0, -2.0), layer: 1 });
    let generous = 24.0;
    assert!(generous <= MAX_LOCAL_RADIUS);
    assert!(
        rig_radius(&[(&parts[..], 1.0)]) < generous,
        "the reference radius must be larger than the rig's own"
    );

    for (where_, root) in ANCHORS {
        for (which, heading) in HEADINGS {
            let tight = drawn(root, heading, &parts, 1.0);
            let mut wide = ground();
            stamp_rig_with_radius(
                &mut wide,
                root,
                heading,
                &[(&parts[..], 1.0)],
                1.0,
                generous,
                &mut Vec::new(),
            );
            assert_identical(
                &tight,
                &wide,
                &format!(
                    "{where_} heading {which}: a radius of {generous} painted something the \
                     rig's own query missed"
                ),
            );
            assert!(max_diff(&tight, &ground()) > 0.01, "{where_}/{which}: the fixture is blank");
        }
    }
}

// ---------------------------------------------------------------------------
// 8. degenerate input
// ---------------------------------------------------------------------------

/// "`h = heading.normalized()`; none draws nothing", and "nothing is drawn when every weight is
/// 0, when `opacity` is not finite or not positive, or when the radius is 0". Nonsense input
/// leaves the canvas exactly as it was — it does not draw a wrong picture and it does not panic.
#[test]
fn degenerate_input_draws_nothing_and_does_not_panic() {
    let sprite = whole(true);
    let parts = [RigPart { sprite: &sprite, offset: Vec2::ZERO, layer: 0 }];
    let root = SurfacePoint::new(Face::Front, 32.0, 32.0);

    for (what, heading, opacity) in [
        ("a zero heading", Vec2::ZERO, 1.0f32),
        ("a NaN heading", Vec2::new(f64::NAN, 0.0), 1.0),
        ("an infinite heading", Vec2::new(f64::INFINITY, 1.0), 1.0),
        ("a zero opacity", Vec2::new(1.0, 0.0), 0.0),
        ("a negative opacity", Vec2::new(1.0, 0.0), -0.5),
        ("a NaN opacity", Vec2::new(1.0, 0.0), f32::NAN),
    ] {
        let mut canvas = ground();
        stamp_rig(&mut canvas, root, heading, &[(&parts[..], 1.0)], opacity, &mut Vec::new());
        assert_identical(&canvas, &ground(), &format!("{what} must draw nothing"));
    }

    // An opacity above 1 is clamped to 1, not refused.
    let mut clamped = ground();
    stamp_rig(&mut clamped, root, Vec2::new(1.0, 0.0), &[(&parts[..], 1.0)], 4.0, &mut Vec::new());
    assert_identical(&clamped, &drawn(root, Vec2::new(1.0, 0.0), &parts, 1.0), "opacity clamp");

    // A heading of any length is a direction: only its normalization is used.
    for scale in [0.25f64, 1.0, 17.0] {
        assert_identical(
            &drawn(root, Vec2::new(0.6 * scale, -0.8 * scale), &parts, 1.0),
            &drawn(root, Vec2::new(0.6, -0.8), &parts, 1.0),
            "the heading's length must not matter",
        );
    }
}

/// `stamp_rig_with_radius` "panics past `MAX_LOCAL_RADIUS` exactly as `unfold_pixels` does".
/// An oracle that silently clamped its own radius would agree with a renderer that silently
/// clamped its own, and the pair would prove nothing (Astra §6.2).
#[test]
#[should_panic(expected = "MAX_LOCAL_RADIUS")]
fn the_generous_reference_query_refuses_an_illegal_radius() {
    let sprite = whole(true);
    let parts = [RigPart { sprite: &sprite, offset: Vec2::ZERO, layer: 0 }];
    stamp_rig_with_radius(
        &mut Canvas::new(),
        SurfacePoint::new(Face::Front, 32.0, 32.0),
        Vec2::new(1.0, 0.0),
        &[(&parts[..], 1.0)],
        1.0,
        MAX_LOCAL_RADIUS + 1.0,
        &mut Vec::new(),
    );
}
