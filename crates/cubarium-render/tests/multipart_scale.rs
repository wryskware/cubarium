//! Independent tests for **`cubarium_render::stamp_rig_scaled`**, written from its public doc
//! comment (and from `stamp_rig`, `rig_radius`, `RIG_MARGIN`, `stamp_rig_with_radius`, `RigPart`
//! and `Sprite::{from_premultiplied, extent}`) — never from their bodies.
//!
//! The normative claims of `stamp_rig_scaled` are exactly three, and each is tested against a
//! second, independent path or against arithmetic recomputed here:
//!
//! * **`scale` must be finite and positive**, anything else "panics in every build" — five
//!   rejected scales, each its own `#[should_panic]`.
//! * **every destination pixel's body coordinate is divided by `scale`**, so "a part at offset
//!   `o` appears at `scale · o` from the root and its texels `scale` pixels apart". Measured two
//!   ways: the light a half-scale body carries (a quarter of the adult's, exactly, for the
//!   lattice-aligned fixture below) and the light *centroid* of a part placed six body pixels
//!   ahead (three chart pixels ahead at half scale, and it rotates with the heading).
//! * **the query radius is `rig_radius · scale`, validated against `MAX_LOCAL_RADIUS` after
//!   scaling**. Two sharp consequences: a rig whose *unscaled* radius is illegal draws fine at
//!   half scale (so the validation really happens after the multiply), and a rig whose *scaled*
//!   radius is illegal panics even though its unscaled radius is legal. That the scaled query
//!   also loses no support is checked the way the unscaled suite checks it — the same rig drawn
//!   with a deliberately more generous query must be the same image, here by padding the rig
//!   with a fully transparent part far from the root (which enlarges `rig_radius` and contributes
//!   no light).
//!
//! Plus the two properties that a scaled rig must keep from the unscaled one, because they are
//! what the single root-owned query buys: a scaled part over the open rim is **cut**, never
//! reflected, and a scaled body straddling a seam carries the same light it carries mid-face at
//! a matched lattice phase.
//!
//! `scale = 1` being "`stamp_rig` bit for bit" is checked first, over ten anchors, five headings
//! and a two-layer rig, because every other test here leans on it.
//!
//! The fixtures are copied from `tests/multipart.rs` rather than imported (integration tests are
//! separate crates), with one addition: an `n × n` opaque full-brightness square whose pivot is
//! its **geometric centre**, which is what makes the light claims exact instead of approximate
//! (see `a_half_scale_body_carries_a_quarter_of_the_light`).

use cubarium_render::{
    Canvas, RigPart, Sprite, rig_radius, stamp_rig, stamp_rig_scaled, stamp_rig_with_radius,
};
use cubarium_surface::{MAX_LOCAL_RADIUS, SurfacePoint, Vec2};
use cube_proto::Face;

// ---------------------------------------------------------------------------
// canvas helpers (copied from tests/multipart.rs)
// ---------------------------------------------------------------------------

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL
        .into_iter()
        .flat_map(|face| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (face, x, y))))
}

/// Bit-for-bit equality, which is what "bit for bit" in the doc means.
fn assert_identical(a: &Canvas, b: &Canvas, what: &str) {
    if let Some((f, x, y)) = every_pixel().find(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y)) {
        panic!(
            "{what}: ({f:?}, {x}, {y}) is {:?} vs {:?}",
            a.get(f, x, y),
            b.get(f, x, y)
        );
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

/// Total premultiplied light on the whole cube, in linear channel units.
fn total_light(image: &Canvas) -> f64 {
    every_pixel()
        .flat_map(|(f, x, y)| image.get(f, x, y))
        .map(f64::from)
        .sum()
}

fn lit_faces(image: &Canvas) -> usize {
    Face::ALL
        .into_iter()
        .filter(|&f| (0..64u8).any(|y| (0..64u8).any(|x| image.get(f, x, y) != [0.0; 3])))
        .count()
}

/// A non-black canvas, so a scaled rig that let the background through, or composited twice,
/// cannot hide behind a black backdrop.
fn ground() -> Canvas {
    let mut canvas = Canvas::new();
    for (f, x, y) in every_pixel() {
        let k = f64::from(x) * 0.011 + f64::from(y) * 0.007 + (f as usize as f64) * 0.03;
        canvas.set(
            f,
            x,
            y,
            [0.08 + 0.04 * k as f32, 0.05, 0.19 - 0.02 * k as f32],
        );
    }
    canvas
}

// ---------------------------------------------------------------------------
// sprite fixtures
// ---------------------------------------------------------------------------

/// The premultiplied linear RGBA of one texel of the test material (copied from
/// `tests/multipart.rs`): `0 ≤ r, g, b ≤ a ≤ 1`, every texel painted so an opened joint shows.
fn material(x: usize, y: usize, opaque: bool) -> [f32; 4] {
    let a = if opaque {
        1.0
    } else {
        0.20 + 0.06 * ((x + 3 * y) % 11) as f32
    };
    [
        a * (0.13 + 0.05 * (x % 7) as f32),
        a * (0.07 + 0.06 * (y % 5) as f32),
        a * (0.21 + 0.04 * ((x + y) % 6) as f32),
        a,
    ]
}

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
    pieces
        .iter()
        .map(|(sprite, offset)| RigPart {
            sprite,
            offset: *offset,
            layer,
        })
        .collect()
}

/// An `n × n` fully opaque, full-brightness square whose pivot is the image's **geometric
/// centre** (`n / 2`, which for an even `n` is a texel corner and for an odd `n` a texel centre).
///
/// This is the fixture the exact light claims rest on. With the root on a pixel centre, a
/// destination pixel's body displacement `d` is an integer vector, so the art coordinate a
/// half-scale draw reads is `2 d` — the samples are a spacing-2 decimation of the art. For an
/// all-ones `n × n` box the 1-D bilinear profile is a trapezoid of total mass `n` (1 across the
/// texel centres, ramping to 0 half a texel outside), and a spacing-2 sum of that trapezoid is
/// exactly `n / 2` whatever the phase, because the two half-weighted end samples replace exactly
/// the one interior sample the coarser lattice drops. Separably, the whole-square sum is
/// `(n / 2)² = n² / 4`: *exactly* a quarter of the adult's `n²`.
fn square(n: usize) -> Sprite {
    Sprite::from_premultiplied(
        n,
        n,
        Vec2::new(n as f64 / 2.0, n as f64 / 2.0),
        vec![[1.0; 4]; n * n],
    )
    .expect("the fixture must be inside the nine-pixel material budget")
}

/// A one-texel, fully **transparent** sprite: it can enlarge `rig_radius` (through its offset)
/// without contributing any light, which is how the tests below hand the renderer a deliberately
/// generous query.
fn invisible() -> Sprite {
    Sprite::from_premultiplied(1, 1, Vec2::new(0.5, 0.5), vec![[0.0; 4]])
        .expect("a transparent texel is a valid premultiplied sprite")
}

fn scaled(
    root: SurfacePoint,
    heading: Vec2,
    parts: &[RigPart<'_>],
    scale: f64,
    background: bool,
) -> Canvas {
    let mut canvas = if background { ground() } else { Canvas::new() };
    stamp_rig_scaled(
        &mut canvas,
        root,
        heading,
        &[(parts, 1.0)],
        scale,
        1.0,
        &mut Vec::new(),
    );
    canvas
}

fn unscaled(root: SurfacePoint, heading: Vec2, parts: &[RigPart<'_>], background: bool) -> Canvas {
    let mut canvas = if background { ground() } else { Canvas::new() };
    stamp_rig(
        &mut canvas,
        root,
        heading,
        &[(parts, 1.0)],
        1.0,
        &mut Vec::new(),
    );
    canvas
}

/// The light centroid of a `Front`-only image, in chart pixels **relative to `root`**: `Σ light ·
/// (pixel centre − root.chart()) / Σ light`. The tests that use it assert first that nothing
/// reached another face, so the centroid is the whole body's.
fn front_centroid(image: &Canvas, root: SurfacePoint) -> Vec2 {
    let origin = root.chart();
    let mut mass = 0.0f64;
    let mut sum = Vec2::ZERO;
    for y in 0..64u8 {
        for x in 0..64u8 {
            let light: f64 = image
                .get(Face::Front, x, y)
                .into_iter()
                .map(f64::from)
                .sum();
            if light <= 0.0 {
                continue;
            }
            mass += light;
            sum = sum
                + Vec2::new(
                    (f64::from(x) + 0.5 - origin.x) * light,
                    (f64::from(y) + 0.5 - origin.y) * light,
                );
        }
    }
    assert!(mass > 0.0, "the centroid of an empty image is not defined");
    Vec2::new(sum.x / mass, sum.y / mass)
}

fn only_front(image: &Canvas, what: &str) {
    for face in Face::ALL {
        if face == Face::Front {
            continue;
        }
        assert!(
            (0..64u8).all(|y| (0..64u8).all(|x| image.get(face, x, y) == [0.0; 3])),
            "{what}: the body reached {face:?}, so a Front-only measurement would miss part of it"
        );
    }
}

// ---------------------------------------------------------------------------
// the anchor and heading suites (copied from tests/multipart.rs)
// ---------------------------------------------------------------------------

const ANCHORS: [(&str, SurfacePoint); 10] = [
    (
        "mid-face, integer",
        SurfacePoint::new(Face::Front, 32.0, 32.0),
    ),
    (
        "mid-face, pixel centre",
        SurfacePoint::new(Face::Front, 32.5, 32.5),
    ),
    (
        "mid-face, subpixel",
        SurfacePoint::new(Face::Front, 32.3, 31.7),
    ),
    (
        "a side/side seam",
        SurfacePoint::new(Face::Front, 63.5, 32.5),
    ),
    (
        "a side/side seam, subpixel",
        SurfacePoint::new(Face::Front, 63.2, 20.7),
    ),
    (
        "a side/side seam, from Right",
        SurfacePoint::new(Face::Right, 0.5, 41.5),
    ),
    ("a side/top seam", SurfacePoint::new(Face::Front, 27.5, 0.5)),
    (
        "a side/top seam, from Top",
        SurfacePoint::new(Face::Top, 38.3, 63.5),
    ),
    (
        "a top vertex, from Top",
        SurfacePoint::new(Face::Top, 0.5, 0.5),
    ),
    (
        "a top vertex, from a side",
        SurfacePoint::new(Face::Front, 0.5, 0.5),
    ),
];

const HEADINGS: [(&str, Vec2); 5] = [
    ("+x", Vec2 { x: 1.0, y: 0.0 }),
    ("+y", Vec2 { x: 0.0, y: 1.0 }),
    ("-x", Vec2 { x: -1.0, y: 0.0 }),
    ("a shallow diagonal", Vec2 { x: 0.6, y: -0.8 }),
    ("45 degrees", Vec2 { x: 1.0, y: 1.0 }),
];

// ---------------------------------------------------------------------------
// 1. scale 1 is the unscaled rig
// ---------------------------------------------------------------------------

/// "`scale = 1` is `stamp_rig` bit for bit." Ten anchors — mid-face at integer, pixel-centre and
/// subpixel phases, both kinds of seam from both sides, and a top vertex from Top and from a
/// side — five headings, and two rigs: a three-piece material in one layer, and that material
/// with an opaque piece on a second layer over it (so the layer sum, the source-over and the
/// single composite are all exercised, not just one sample).
///
/// Everything else in this file is a comparison against a `scale = 1` image or against arithmetic
/// that assumes this, so it is checked first and it is checked bit for bit over a lit background.
#[test]
fn scale_one_is_the_unscaled_rig_bit_for_bit() {
    let pieces = chain(true);
    let one_layer = rig_parts(&pieces, 0);
    let translucent = patch(2, 0, 5, 5, Vec2::new(2.5, 2.5), false);
    let mut two_layer = one_layer.clone();
    two_layer.push(RigPart {
        sprite: &translucent,
        offset: Vec2::new(2.0, 1.0),
        layer: 3,
    });

    let mut compared = 0usize;
    for (where_, root) in ANCHORS {
        for (which, heading) in HEADINGS {
            for (what, parts) in [("one layer", &one_layer), ("two layers", &two_layer)] {
                let plain = unscaled(root, heading, parts, true);
                let at_one = scaled(root, heading, parts, 1.0, true);
                assert_identical(
                    &plain,
                    &at_one,
                    &format!("{what} at {where_} heading {which}, scale 1 vs stamp_rig"),
                );
                assert!(
                    max_diff(&plain, &ground()) > 0.01,
                    "{what} at {where_} heading {which}: nothing was drawn, so nothing was compared"
                );
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 100, "ten anchors x five headings x two rigs");
}

// ---------------------------------------------------------------------------
// 2. the body coordinate is divided by the scale: area
// ---------------------------------------------------------------------------

/// "Every destination pixel's body coordinate is divided by `scale` … its texels `scale` pixels
/// apart." At half scale an opaque 8 × 8 square therefore covers a 4 × 4 area, and the light it
/// carries is a quarter of the adult's.
///
/// With the root on a pixel centre the claim is **exact**, not approximate — see `square`: the
/// half-scale samples are a spacing-2 decimation of the art's trapezoidal bilinear profile, whose
/// sum is exactly half the adult's per axis. So the assertion is a quarter within a floating-point
/// hair, and the spec's "within 15 %" is reported as measured slack rather than needed slack. The
/// 15 % band is kept as the assertion for the *subpixel* root, where the decimation phase is not
/// lattice-aligned and the trapezoid rule really does carry an error term.
///
/// The second half of the claim is the footprint: no pixel farther than `scale · extent + 1` from
/// the root may be painted (the `+ 1` is the bilinear filter's own reach, one *art* pixel, which
/// is `scale ≤ 1` chart pixels — so the bound is conservative). A renderer that scaled only the
/// sprite images, or that forgot to scale the query, would paint outside it.
#[test]
fn a_half_scale_body_carries_a_quarter_of_the_light_inside_a_half_size_footprint() {
    let sprite = square(8);
    let parts = [RigPart {
        sprite: &sprite,
        offset: Vec2::ZERO,
        layer: 0,
    }];
    let extent = sprite.extent();

    // Lattice-aligned: exact.
    let root = SurfacePoint::new(Face::Front, 32.5, 32.5);
    let adult = scaled(root, Vec2::new(1.0, 0.0), &parts, 1.0, false);
    let juvenile = scaled(root, Vec2::new(1.0, 0.0), &parts, 0.5, false);
    only_front(&adult, "the adult square");
    only_front(&juvenile, "the half-scale square");
    let (a, j) = (total_light(&adult), total_light(&juvenile));
    assert!(
        (a - 3.0 * 64.0).abs() < 1e-6,
        "the adult 8x8 opaque square should carry 3 x 64 = 192 light, not {a}"
    );
    assert!(
        (j - a / 4.0).abs() / a < 1e-6,
        "the half-scale square carried {j} where a quarter of the adult's {a} is {}",
        a / 4.0
    );

    // The painted box halves too. A sample is non-zero exactly while its art coordinate can
    // still read a painted texel, i.e. while `p = d / scale + pivot − 0.5 ∈ (−1, 8)`: at scale 1
    // that is `d ∈ {−4 … 4}` (the art's eight columns plus the filter's one-pixel ring, and the
    // pixel-centre root puts `p` on half-integers), and at scale 0.5 it is `d ∈ {−2 … 2}`.
    let painted_span = |c: &Canvas| {
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        for y in 0..64u8 {
            for x in 0..64u8 {
                if c.get(Face::Front, x, y) != [0.0; 3] {
                    lo = lo.min(i32::from(x) - 32);
                    hi = hi.max(i32::from(x) - 32);
                }
            }
        }
        (lo, hi)
    };
    assert_eq!(
        painted_span(&juvenile),
        (-2, 2),
        "a half-scale 8x8 square spans five columns"
    );
    assert_eq!(painted_span(&adult), (-4, 4), "the adult square spans nine");

    // And nothing past the scaled footprint, at a lattice-aligned and at a subpixel root.
    for (what, root) in [
        (
            "a pixel-centre root",
            SurfacePoint::new(Face::Front, 32.5, 32.5),
        ),
        (
            "a subpixel root",
            SurfacePoint::new(Face::Front, 32.3, 31.7),
        ),
    ] {
        for scale in [0.5f64, 0.75, 1.0] {
            let image = scaled(root, Vec2::new(0.6, -0.8), &parts, scale, false);
            let origin = root.chart();
            let bound = scale * extent + 1.0;
            for (f, x, y) in every_pixel() {
                if image.get(f, x, y) == [0.0; 3] {
                    continue;
                }
                assert_eq!(
                    f,
                    Face::Front,
                    "{what} at scale {scale}: the body left Front"
                );
                let d = Vec2::new(f64::from(x) + 0.5 - origin.x, f64::from(y) + 0.5 - origin.y);
                assert!(
                    d.length() <= bound,
                    "{what} at scale {scale}: Front ({x}, {y}) is {} from the root, past the \
                     scaled footprint {bound} (scale x extent {extent} + 1)",
                    d.length()
                );
            }
        }
        // The unlattice-aligned light claim, with the trapezoid rule's own slack.
        let a = total_light(&scaled(root, Vec2::new(1.0, 0.0), &parts, 1.0, false));
        let j = total_light(&scaled(root, Vec2::new(1.0, 0.0), &parts, 0.5, false));
        assert!(
            (j - a / 4.0).abs() / (a / 4.0) < 0.15,
            "{what}: the half-scale square carried {j}, more than 15 % off the quarter {}",
            a / 4.0
        );
    }
}

// ---------------------------------------------------------------------------
// 3. the body coordinate is divided by the scale: offsets
// ---------------------------------------------------------------------------

/// "A part at offset `o` appears at `scale · o` from the root." A second part six body pixels
/// ahead of the root must draw its light three chart pixels ahead at half scale, and six at full
/// scale — a renderer that scaled only the sprite images would leave it at six either way, which
/// is exactly the "juvenile's head/arms detached at adult distances" failure the contract names.
///
/// The fixture is an opaque 7 × 7 square whose pivot is a texel centre, so at a pixel-centre root
/// and an axis-aligned heading every sample lands exactly on a texel centre and the sampled set is
/// symmetric about the part's centre: the measured centroid is then the scaled offset to
/// floating-point rounding, not to a decimation bias. The four axis-aligned headings are asserted
/// to 0.02 px; the rotated ones get the decimation bound (half a sample spacing, `scale` chart
/// pixels) and the sharper comparison that the scaled centroid is near `scale · o` and nowhere
/// near `o`.
#[test]
fn a_scaled_part_sits_at_its_scaled_offset_and_rotates_with_the_heading() {
    let sprite = square(7);
    const AHEAD: f64 = 6.0;
    let parts = [RigPart {
        sprite: &sprite,
        offset: Vec2::new(AHEAD, 0.0),
        layer: 0,
    }];
    let root = SurfacePoint::new(Face::Front, 32.5, 32.5);

    for (which, heading) in HEADINGS {
        let h = Vec2::new(heading.x, heading.y);
        let unit = {
            let len = h.length();
            Vec2::new(h.x / len, h.y / len)
        };
        for scale in [0.5f64, 1.0] {
            let image = scaled(root, heading, &parts, scale, false);
            only_front(&image, &format!("heading {which} at scale {scale}"));
            let centroid = front_centroid(&image, root);
            let want = Vec2::new(unit.x * AHEAD * scale, unit.y * AHEAD * scale);
            let off = (centroid - want).length();
            let axis_aligned = unit.x == 0.0 || unit.y == 0.0;
            let bound = if axis_aligned { 0.02 } else { scale + 1e-9 };
            assert!(
                off <= bound,
                "heading {which} at scale {scale}: the part's light centroid is \
                 ({:.4}, {:.4}), {off:.4} from the scaled offset ({:.4}, {:.4})",
                centroid.x,
                centroid.y,
                want.x,
                want.y
            );
            if scale < 1.0 {
                let unscaled_place = Vec2::new(unit.x * AHEAD, unit.y * AHEAD);
                assert!(
                    (centroid - unscaled_place).length() > AHEAD * (1.0 - scale) - bound,
                    "heading {which}: the half-scale part is still sitting at its adult distance"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 4. the query radius is rig_radius x scale, validated after the multiply
// ---------------------------------------------------------------------------

/// "The query radius is `rig_radius · scale`, validated against `MAX_LOCAL_RADIUS` **after**
/// scaling." Three consequences, all observable:
///
/// * a rig whose *unscaled* `rig_radius` is past `MAX_LOCAL_RADIUS` (a part 40 body pixels out)
///   draws without complaint at half scale, where its radius is 23-ish — if the validation ran
///   before the multiply this would panic, and every juvenile of a long-bodied rig would be
///   undrawable;
/// * the scaled query still loses no support: padding the same rig with a fully transparent part
///   far from the root enlarges `rig_radius` (and so the query) fourfold and must not change one
///   bit of the image;
/// * at `scale = 1` the radius is the rig's own, which `stamp_rig_with_radius` at a deliberately
///   larger legal radius must match bit for bit (the unscaled suite's "own radius paints what a
///   larger legal radius paints", restated here as the `scale = 1` case of the scaled rule).
#[test]
fn the_scaled_query_radius_is_validated_after_scaling_and_loses_no_support() {
    let sprite = square(7);
    let far = invisible();

    // A rig that is illegal unscaled and legal at half scale.
    let long = [RigPart {
        sprite: &sprite,
        offset: Vec2::new(40.0, 0.0),
        layer: 0,
    }];
    let radius = rig_radius(&[(&long[..], 1.0)]);
    assert!(
        radius > MAX_LOCAL_RADIUS,
        "the fixture is meant to be illegal unscaled, but its radius is only {radius}"
    );
    assert!(
        radius * 0.5 <= MAX_LOCAL_RADIUS,
        "the fixture is meant to be legal at half scale, but {} is still illegal",
        radius * 0.5
    );
    let juvenile = scaled(
        SurfacePoint::new(Face::Front, 32.5, 32.5),
        Vec2::new(1.0, 0.0),
        &long,
        0.5,
        false,
    );
    assert!(
        total_light(&juvenile) > 1.0,
        "the half-scale long rig drew nothing, so the radius claim is vacuous"
    );

    // The scaled query loses no support: a generous query paints the same image.
    let body = [RigPart {
        sprite: &sprite,
        offset: Vec2::new(3.0, 1.0),
        layer: 0,
    }];
    let padded = [
        body[0],
        RigPart {
            sprite: &far,
            offset: Vec2::new(24.0, 0.0),
            layer: 0,
        },
    ];
    let tight = rig_radius(&[(&body[..], 1.0)]);
    let generous = rig_radius(&[(&padded[..], 1.0)]);
    assert!(
        generous > tight * 2.0,
        "the padding was meant to enlarge the query ({tight} -> {generous})"
    );
    for (where_, root) in ANCHORS {
        for (which, heading) in HEADINGS {
            for scale in [0.5f64, 0.6, 1.0] {
                assert_identical(
                    &scaled(root, heading, &body, scale, true),
                    &scaled(root, heading, &padded, scale, true),
                    &format!(
                        "{where_} heading {which} at scale {scale}: the rig's own scaled query \
                         painted less than a generous one"
                    ),
                );
            }
        }
    }

    // And at scale 1 the radius is the rig's own, which a larger legal radius must reproduce.
    for (where_, root) in ANCHORS {
        for (which, heading) in HEADINGS {
            let own = scaled(root, heading, &body, 1.0, true);
            let mut wide = ground();
            stamp_rig_with_radius(
                &mut wide,
                root,
                heading,
                &[(&body[..], 1.0)],
                1.0,
                24.0,
                &mut Vec::new(),
            );
            assert_identical(
                &own,
                &wide,
                &format!("{where_} heading {which}: rig_radius x 1 vs an explicit radius of 24"),
            );
        }
    }
}

/// The other side of "validated … after scaling": a rig whose radius is legal unscaled and
/// illegal once magnified is a configuration error and panics in every build. A part 20 body
/// pixels out has a radius of about 24 — legal — and twice that is 48, past the surface's 32.
#[test]
#[should_panic(expected = "MAX_LOCAL_RADIUS")]
fn a_rig_whose_scaled_radius_passes_the_surface_bound_panics() {
    let sprite = square(7);
    let parts = [RigPart {
        sprite: &sprite,
        offset: Vec2::new(20.0, 0.0),
        layer: 0,
    }];
    assert!(
        rig_radius(&[(&parts[..], 1.0)]) <= MAX_LOCAL_RADIUS,
        "the fixture must be legal *unscaled*, or it proves nothing about the multiply"
    );
    scaled(
        SurfacePoint::new(Face::Front, 32.5, 32.5),
        Vec2::new(1.0, 0.0),
        &parts,
        2.0,
        false,
    );
}

// ---------------------------------------------------------------------------
// 5. an inadmissible scale is a configuration error
// ---------------------------------------------------------------------------

/// "`scale` must be finite and positive — anything else is a rig configuration error that panics
/// in every build, exactly like an illegal radius." Four rejected values, one test each so that a
/// single swallowed case cannot hide behind another's panic; a renderer that clamped instead would
/// silently draw an adult where a zero or a NaN was asked for.
macro_rules! rejects_scale {
    ($name:ident, $scale:expr) => {
        #[test]
        #[should_panic(expected = "rig scale")]
        fn $name() {
            let sprite = square(7);
            let parts = [RigPart {
                sprite: &sprite,
                offset: Vec2::ZERO,
                layer: 0,
            }];
            scaled(
                SurfacePoint::new(Face::Front, 32.5, 32.5),
                Vec2::new(1.0, 0.0),
                &parts,
                $scale,
                false,
            );
        }
    };
}

rejects_scale!(a_scale_of_zero_panics, 0.0);
rejects_scale!(a_negative_scale_panics, -0.5);
rejects_scale!(a_nan_scale_panics, f64::NAN);
rejects_scale!(an_infinite_scale_panics, f64::INFINITY);
rejects_scale!(a_negative_infinite_scale_panics, f64::NEG_INFINITY);

// ---------------------------------------------------------------------------
// 6. the rim still cuts a scaled part
// ---------------------------------------------------------------------------

/// "The root, heading, ownership and depth rules are those of `stamp_rig`" — so a scaled part
/// hanging over the open rim is **cut**, never reflected or clamped inward.
///
/// The expectation is built the second way the unscaled suite builds it: the *same* scaled rig
/// thirty rows up the same face, where nothing is near the rim. Both roots have integer chart
/// coordinates exactly thirty pixels apart, so the rim image's pixel `(x, y)` and the flat image's
/// `(x, y − 30)` have the *same double* for `d` and therefore the same body coordinate after the
/// division by `scale`: the comparison is bit for bit.
///
/// A part carried to its own anchor would have been reflected back up the face (light on rows the
/// flat body does not have); one clamped inward would have squeezed against the last row.
#[test]
fn a_scaled_part_over_the_open_rim_is_cut_and_never_reflected() {
    let sprite = square(8);
    // Six body pixels below the root: at every scale below some of the square is still on the
    // face and some of it is past the rim, which is what makes the row-by-row comparison sharp.
    let parts = [RigPart {
        sprite: &sprite,
        offset: Vec2::new(0.0, 6.0),
        layer: 0,
    }];
    let heading = Vec2::new(1.0, 0.0);

    for scale in [0.5f64, 0.6, 1.0] {
        let rim = scaled(
            SurfacePoint::new(Face::Front, 32.0, 60.0),
            heading,
            &parts,
            scale,
            false,
        );
        let flat = scaled(
            SurfacePoint::new(Face::Front, 32.0, 30.0),
            heading,
            &parts,
            scale,
            false,
        );

        assert!(
            (34..64u8).any(|y| (0..64u8).any(|x| flat.get(Face::Front, x, y) != [0.0; 3])),
            "at scale {scale} the fixture does not reach past where the rim is, so it proves nothing"
        );
        assert!(
            (0..64u8).any(|x| rim.get(Face::Front, x, 63) != [0.0; 3]),
            "at scale {scale} the rim body painted nothing on the last row that exists"
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
                "at scale {scale}, over the rim, ({f:?}, {x}, {y}) is {:?} where the same rig \
                 thirty rows up has {want:?} — a reflected or clamped part, not a cut one",
                rim.get(f, x, y)
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 7. a scaled body across a seam is still one body
// ---------------------------------------------------------------------------

/// A scaled body straddling a seam carries the light it carries mid-face. The argument is the
/// unscaled suite's, and the division by `scale` does not touch it: one root-owned query gives
/// every pixel within the radius exactly once, and at a **matched lattice phase** (fractional
/// chart coordinates `(0.5, 0.5)`, invariant under the quarter turn a side/top seam applies) the
/// set of body displacements `d` is literally the same at both anchors — so the sampled art
/// coordinates `d / scale` are the same too and the totals must agree to floating-point rounding.
///
/// A scaled rig that re-derived its own chart images per part, or that queried a radius it had
/// forgotten to scale, would lose or double material exactly here.
#[test]
fn a_scaled_rig_straddling_a_seam_carries_the_same_light_as_mid_face() {
    let pieces = chain(true);
    let parts = rig_parts(&pieces, 0);
    let mid = SurfacePoint::new(Face::Front, 32.5, 32.5);

    for scale in [0.5f64, 0.6, 1.0] {
        for (which, heading) in HEADINGS {
            let middle = total_light(&scaled(mid, heading, &parts, scale, false));
            assert!(
                middle > 1.0,
                "at scale {scale} the fixture must carry real light, got {middle}"
            );
            for (where_, root) in [
                (
                    "a Front/Right seam",
                    SurfacePoint::new(Face::Front, 63.5, 32.5),
                ),
                (
                    "a Front/Right seam, from Right",
                    SurfacePoint::new(Face::Right, 0.5, 41.5),
                ),
                (
                    "a Right/Top seam, a quarter turn",
                    SurfacePoint::new(Face::Right, 32.5, 0.5),
                ),
                (
                    "a Front/Top seam, untwisted",
                    SurfacePoint::new(Face::Front, 27.5, 0.5),
                ),
            ] {
                let canvas = scaled(root, heading, &parts, scale, false);
                let light = total_light(&canvas);
                assert!(
                    (light - middle).abs() / middle < 1e-6,
                    "{where_} heading {which} at scale {scale}: the rig carried {light} where \
                     mid-face at the same lattice phase it carries {middle}"
                );
                assert!(
                    lit_faces(&canvas) >= 2,
                    "{where_} heading {which} at scale {scale}: the rig did not reach the \
                     neighbouring face, so no seam was crossed"
                );
            }
        }
    }
}
