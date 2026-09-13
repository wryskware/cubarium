//! Independent tests for the **Lanternjaw** production rig, written from the public doc
//! comments of `cubarium::lanternjaw` (the module doc's "What changes for production",
//! "Footprint" and body-lattice paragraphs, `Mode`, `PartName` and its layer table,
//! `PartName::layer`, `Part::rig_part`, `ROWS`, the timing constants, `BOUND_*`,
//! `PART_EXTENT_MAX`, `QUERY_RADIUS_MAX`, `envelope`, `hunt_state`, `Lanternjaw::parts` and
//! `Lanternjaw::draw`), of `cubarium_render::{RigPart, rig_radius, stamp_rig}` and of
//! `Sprite::{pivot, texel, extent}`, together with `art/studies/megafauna/fable.md`
//! ("Footprint and timing", "Palette", "Verification") — never from their bodies.
//!
//! Where an expected value exists it is recomputed here a second, independent way: the study's
//! `huntState` schedule and its raised-cosine envelope from the normative formulas; the drawn
//! image from `parts` plus hand-assembled `stamp_rig` calls; the half-turn image from the body
//! frame's own definition. Where no such value exists the assertion is a property a wrong
//! implementation would break: a pose that is not a pure function of `(seconds, mode)`, a part
//! that leaves the body lattice or the footprint contract, a mode whose loop steps, a warm
//! accent leaking into a calm mode, a blink or a strike that cuts instead of easing, a far limb
//! painted over the hull, a fade that ridges where two opaque parts overlap, or an anchor
//! silently rounded to whole pixels.
//!
//! The renderer's own properties — the single root query, the per-layer material sum, the depth
//! order, the rim cut, seam and vertex ownership, the state mixture and the query radius — are
//! tested against synthetic fixtures in `crates/cubarium-render/tests/multipart.rs`.
//!
//! **One brief item is not tested as written.** The work order asks that heading `(−1, 0)`
//! "mirror" heading `(1, 0)` about the anchor *column*. The body frame every stamp uses is
//! `+x = h`, `+y = (−h.y, h.x)`, so negating the heading negates *both* body axes: heading
//! `(−1, 0)` is a half turn about the anchor, not a horizontal reflection (Astra,
//! `astra-lanternjaw-brief-review-2026-09-13.md` §5). The Lanternjaw is deliberately asymmetric
//! about its own mid-line — lanterns above, legs below — so a column-mirror test would reject
//! correct geometry. `heading_minus_x_is_a_half_turn_about_the_anchor` tests the half turn and
//! records that the column mirror does *not* hold.

use cube_proto::Face;
use cubarium::lanternjaw::*;
use cubarium_render::{Canvas, RigPart, Sprite, rig_radius, stamp_rig};
use cubarium_surface::{SurfacePoint, Vec2};

// ---------------------------------------------------------------------------
// sampling and canvas helpers
// ---------------------------------------------------------------------------

/// Presentation frames per second: the rate every timing property below is measured at, and
/// the rate `fable.md` "Verification" measured the study's envelopes at.
const FPS: usize = 60;
/// The sweep length the brief asks for: long enough for two `rest` blinks, two `hunt` cycles
/// and five `move` gaits.
const SWEEP_SECONDS: usize = 12;

fn frames() -> impl Iterator<Item = f64> {
    (0..FPS * SWEEP_SECONDS).map(|i| i as f64 / FPS as f64)
}

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL
        .into_iter()
        .flat_map(|face| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (face, x, y))))
}

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

fn filled(rgb: [f32; 3]) -> Canvas {
    let mut canvas = Canvas::new();
    for (f, x, y) in every_pixel() {
        canvas.set(f, x, y, rgb);
    }
    canvas
}

/// A mid-face anchor on a pixel centre, so the body lattice lands on the face lattice and every
/// half-turn or whole-pixel comparison below is exact rather than approximate.
fn mid() -> SurfacePoint {
    SurfacePoint::pixel_center(Face::Front, 32, 32)
}

fn forward() -> Vec2 {
    Vec2::new(1.0, 0.0)
}

fn body(seconds: f64, mode: Mode) -> Vec<Part> {
    let mut out = Vec::new();
    Lanternjaw::new().parts(seconds, mode, &mut out);
    out
}

fn drawn(seconds: f64, mode: Mode, anchor: SurfacePoint, heading: Vec2, opacity: f32) -> Canvas {
    let mut canvas = Canvas::new();
    Lanternjaw::new().draw(
        &mut canvas,
        anchor,
        heading,
        seconds,
        mode,
        opacity,
        &mut Vec::new(),
        &mut Vec::new(),
    );
    canvas
}

/// The selected parts of one frame, assembled into one `stamp_rig` call at their own layers and
/// composited over `background`. This is how the tests take the body apart: `parts` and
/// `Part::rig_part` are public, so a sub-rig of the real frame is a legitimate second path.
fn stamp_only(
    parts: &[Part],
    keep: &[PartName],
    anchor: SurfacePoint,
    heading: Vec2,
    opacity: f32,
    background: [f32; 3],
) -> Canvas {
    let rig: Vec<RigPart<'_>> =
        parts.iter().filter(|p| keep.contains(&p.name)).map(Part::rig_part).collect();
    let mut canvas = filled(background);
    stamp_rig(&mut canvas, anchor, heading, &[(&rig[..], 1.0)], opacity, &mut Vec::new());
    canvas
}

const HULL: [PartName; 4] = [PartName::Tail, PartName::Abdomen, PartName::Thorax, PartName::Head];

/// The parts that share the **body lattice** per the module doc: "The hull pieces, the underside
/// and the glow … their `offset`s and their sprites' pivots have integer coordinates".
const ON_LATTICE: [PartName; 6] = [
    PartName::Underside,
    PartName::Tail,
    PartName::Abdomen,
    PartName::Thorax,
    PartName::Head,
    PartName::Glow,
];

fn part_of<'a>(parts: &'a [Part], name: PartName) -> &'a Part {
    parts.iter().find(|p| p.name == name).expect("every frame carries every part")
}

/// The body-local centre of texel `(tx, ty)` of `part`: `offset + (texel centre − pivot)`, the
/// footprint contract's own expression.
fn texel_body(part: &Part, tx: usize, ty: usize) -> Vec2 {
    part.offset
        + Vec2::new(
            tx as f64 + 0.5 - part.sprite.pivot().x,
            ty as f64 + 0.5 - part.sprite.pivot().y,
        )
}

fn painted(sprite: &Sprite) -> impl Iterator<Item = (usize, usize)> + '_ {
    (0..sprite.height())
        .flat_map(move |ty| (0..sprite.width()).map(move |tx| (tx, ty)))
        .filter(move |&(tx, ty)| sprite.texel(tx as i32, ty as i32)[3] > 0.0)
}

// ---------------------------------------------------------------------------
// the study's formulas, recomputed here from the doc comments
// ---------------------------------------------------------------------------

/// `fable.js` `smoothstep`: `clamp(u, 0, 1)` through the Hermite `t²(3 − 2t)`.
fn smoothstep(u: f64) -> f64 {
    let c = u.clamp(0.0, 1.0);
    c * c * (3.0 - 2.0 * c)
}

/// The raised-cosine envelope, recomputed from `envelope`'s doc: a cosine ramp over the first
/// 40 % of `u ∈ (0, 1)`, a plateau to 60 %, a cosine ramp down, 0 at and outside both ends.
fn envelope_reference(u: f64) -> f64 {
    if !(u > 0.0 && u < 1.0) {
        return 0.0;
    }
    if u < 0.4 {
        0.5 - 0.5 * (std::f64::consts::PI * u / 0.4).cos()
    } else if u < 0.6 {
        1.0
    } else {
        0.5 - 0.5 * (std::f64::consts::PI * (1.0 - u) / 0.4).cos()
    }
}

/// The normative `hunt_state` schedule, recomputed term by term from its doc comment.
fn hunt_reference(t_h: f64) -> HuntState {
    let mut s = HuntState::default();
    if !t_h.is_finite() {
        return s;
    }
    if t_h >= T_COIL && t_h < T_SNAP {
        let u = smoothstep((t_h - T_COIL) / (T_SNAP - T_COIL));
        s.reach = -0.35 * u;
        s.compress = 1.7 * u;
        s.charge = u;
    } else if t_h >= T_SNAP && t_h < T_OPEN {
        let u = (t_h - T_SNAP) / (T_OPEN - T_SNAP);
        let v = 1.0 - u;
        let e = 1.0 - v * v * v;
        s.reach = -0.35 + 1.35 * e;
        s.compress = 1.7 - 2.0 * e;
        s.lunge = 1.1 * e;
    } else if t_h >= T_OPEN && t_h < T_END {
        let u = smoothstep((t_h - T_OPEN) / (T_END - T_OPEN));
        s.reach = 1.0 - u;
        s.compress = -0.3 + 0.3 * u;
        s.lunge = 1.1 * (1.0 - u);
    }
    if t_h >= T_SNAP {
        s.charge =
            (-(t_h - T_SNAP) / 0.9).exp() * smoothstep((HUNT_PERIOD - t_h) / 0.6);
    }
    s.accent = envelope_reference((t_h - (T_SNAP - 0.02)) / ACCENT_SECONDS);
    s.blink = envelope_reference((t_h - (T_END + 0.3)) / BLINK_SECONDS);
    s
}

// ---------------------------------------------------------------------------
// colour probes derived from `fable.md` "Palette"
// ---------------------------------------------------------------------------

/// **The warm-accent hue test.** `fable.md` "Palette" says the orange "never appears on its own
/// — it only tints the pink at the peak of the strike envelope, and it is absent from `rest`,
/// `move` and `bud` entirely". The study's `JAW_HOT` is `mix(PINK, ORANGE, 0.45)` =
/// sRGB `(255, 93, 175)`: it is the **only** colour in the whole palette whose red is above its
/// blue by any margin. Every other entry is a violet, indigo or cyan with blue dominant; the
/// single exception is the eye's pure `PINK` `(255, 42, 252)`, whose decoded premultiplied red
/// exceeds its blue by only `0.027`.
///
/// So "nearer `JAW_HOT` than to any rest colour" is decidable as a hue: premultiplied
/// `r − b > 0.06`, twice the eye's margin and reached by the accent mix at roughly 33 % of the
/// envelope. Premultiplication only scales the margin down, so a texel that passes carries real
/// warm coverage and not a filter tail.
const WARM: f32 = 0.06;

/// The warmest texel of the whole body: `max(r − b)` in premultiplied linear over every part.
fn warmest(parts: &[Part]) -> f32 {
    let mut worst = f32::NEG_INFINITY;
    for part in parts {
        for (tx, ty) in painted(&part.sprite) {
            let t = part.sprite.texel(tx as i32, ty as i32);
            worst = worst.max(t[0] - t[2]);
        }
    }
    worst
}

/// **The eye reading.** The eye is the template's single `e` cell, at column 15 of `ROWS[3]` —
/// body-local `(COL_LEFT + 15, ROW_TOP + 3) = (6, −1)` before the frame's own `(dx, dy)`. Its
/// colour walks `PINK → EYE_SHUT` (`mix(BG, VIOLET, 0.55)`) with the blink closure, and that is
/// a swing of about `0.95` in decoded `r − g`.
///
/// The reading is the **signed** sum of `r − g` over the `Head` texels whose body centre lies
/// within 1.5 px of `(6, −1)`, which is:
///
/// * exactly linear in the eye's own colour, hence exactly linear in the closure, because the
///   rasterizer *adds* a cell's bilinear splat into the buffer and the splat's four weights sum
///   to 1 — so sub-pixel motion of the eye does not move the reading at all;
/// * free of the one other thing in the head that changes colour over time, the brow lamp `L`
///   at column 13 (body `(4, −2)`, 2.24 px away and therefore outside the window), whose cyan
///   has `r − g < 0` and whose pulse would otherwise contaminate both the swing and the step;
/// * carrying a *constant* offset from the neighbouring `c`, `d` and `v` cells inside the
///   window, which cancels out of both `max − min` and `|Δ|`.
fn eye_reading(parts: &[Part]) -> f64 {
    let head = part_of(parts, PartName::Head);
    let eye = Vec2::new(COL_LEFT + 15.0, ROW_TOP + 3.0);
    let mut sum = 0.0f64;
    for ty in 0..head.sprite.height() {
        for tx in 0..head.sprite.width() {
            if (texel_body(head, tx, ty) - eye).length() <= 1.5 {
                let t = head.sprite.texel(tx as i32, ty as i32);
                sum += f64::from(t[0]) - f64::from(t[1]);
            }
        }
    }
    sum
}

// ---------------------------------------------------------------------------
// 1. the envelope and the hunt schedule
// ---------------------------------------------------------------------------

/// `envelope` is "0 at and outside both ends, with zero slope there", 1 on the plateau, and
/// symmetric. The last two assertions are the ones the study's "nothing in this body is allowed
/// to switch on for a single frame" rests on: a linear ramp has a kink at each end, and a step
/// has a slope of 1 there.
#[test]
fn the_accent_envelope_is_the_studys_raised_cosine() {
    assert_eq!(envelope(0.0), 0.0);
    assert_eq!(envelope(1.0), 0.0);
    assert_eq!(envelope(-0.3), 0.0);
    assert_eq!(envelope(1.7), 0.0);
    assert_eq!(envelope(f64::NAN), 0.0);
    assert_eq!(envelope(f64::INFINITY), 0.0);
    for u in [0.4, 0.45, 0.5, 0.55, 0.599] {
        assert_eq!(envelope(u), 1.0, "the plateau runs from 40 % to 60 %");
    }
    for k in 0..=1000 {
        let u = f64::from(k) / 1000.0;
        let v = envelope(u);
        assert!((0.0..=1.0).contains(&v), "envelope({u}) = {v}");
        assert!(
            (v - envelope_reference(u)).abs() < 1e-12,
            "envelope({u}) = {v}, not the raised cosine's {}",
            envelope_reference(u)
        );
        // Symmetric about the middle of its window.
        assert!((v - envelope(1.0 - u)).abs() < 1e-12, "envelope is not symmetric at {u}");
    }
    // Zero slope at both ends: the first and last hundredth move far less than the steepest
    // hundredth in the middle of a ramp.
    let step = |u: f64| (envelope(u + 0.01) - envelope(u)).abs();
    let steep = step(0.2);
    assert!(step(0.0) < steep / 10.0, "the envelope has a kink at its start");
    assert!(step(0.99) < steep / 10.0, "the envelope has a kink at its end");
}

/// `hunt_state` is the study's `huntState`, "unrounded": the coil's smoothstep, the snap's cubic
/// ease-out, the recoil's smoothstep, the charge's exponential bleed forced to zero before the
/// cycle wraps, and the two envelopes. Recomputed term by term, and checked at the instants that
/// distinguish the phases — including the three boundaries and the very end of the cycle, where
/// a charge that did not decay to zero would step the loop.
#[test]
fn the_hunt_schedule_is_the_studys_hunt_state() {
    let probes = [
        0.0, 1.0, 3.0, T_COIL, 3.16, T_SNAP - 1e-9, T_SNAP, 3.28, T_OPEN - 1e-9, T_OPEN, 3.44,
        T_END - 1e-9, T_END, 3.84, 4.5, 5.9, 5.999, HUNT_PERIOD - 1e-9,
    ];
    for t in probes {
        let got = hunt_state(t);
        let want = hunt_reference(t);
        for (name, g, w) in [
            ("reach", got.reach, want.reach),
            ("compress", got.compress, want.compress),
            ("lunge", got.lunge, want.lunge),
            ("charge", got.charge, want.charge),
            ("accent", got.accent, want.accent),
            ("blink", got.blink, want.blink),
        ] {
            assert!(
                (g - w).abs() < 1e-9,
                "hunt_state({t}).{name} = {g}, not the schedule's {w}"
            );
        }
    }
    // A non-finite phase is the default, all zero.
    assert_eq!(hunt_state(f64::NAN), HuntState::default());
    assert_eq!(hunt_state(f64::INFINITY), HuntState::default());

    // The named beats really are what the schedule says: cocked at the coil, fully extended at
    // full extension, folded and quiet again by the end.
    assert!(hunt_state(3.0).reach == 0.0 && hunt_state(3.0).lunge == 0.0, "stalking is still");
    assert!(hunt_state(3.16).reach < -0.1, "the coil pulls the limbs back");
    assert!((hunt_state(T_OPEN).reach - 1.0).abs() < 1e-9, "full extension is reach 1");
    assert_eq!(hunt_state(T_END).reach, 0.0, "the recoil ends folded");
    // "forced to zero before the cycle wraps so the loop has no step".
    assert!(
        hunt_state(HUNT_PERIOD - 1e-9).charge.abs() < 1e-9,
        "the charge must reach zero before the cycle wraps, not {}",
        hunt_state(HUNT_PERIOD - 1e-9).charge
    );
    // The accent opens with the forelimbs and is gone before the recoil ends.
    assert_eq!(hunt_state(T_SNAP - 0.02).accent, 0.0);
    assert!(hunt_state(T_SNAP + 0.1).accent > 0.5);
    assert_eq!(hunt_state(T_SNAP - 0.02 + ACCENT_SECONDS).accent, 0.0);
    assert!(T_SNAP - 0.02 + ACCENT_SECONDS < T_END, "the accent ends before the recoil does");
}

// ---------------------------------------------------------------------------
// 2. the rig's shape: eight parts, their layers, the body lattice
// ---------------------------------------------------------------------------

/// "`Lanternjaw::parts` always returns exactly the eight parts of `PartName::ALL`", in painting
/// order, and "`Part::rig_part` … at its name's layer". A frame that dropped a part when its
/// pose went out of budget, or that reordered the list, is what this rules out; the layer
/// equality is what makes the far limb stay behind the hull and the hull's four pieces stay one
/// material.
#[test]
fn every_frame_carries_the_eight_parts_in_painting_order_at_their_own_layers() {
    let rig = Lanternjaw::new();
    let mut out = Vec::new();
    // A dirty buffer: `out` is "cleared first", so a stale part may not survive.
    out.push(Part {
        name: PartName::Head,
        sprite: Sprite::from_premultiplied(1, 1, Vec2::new(0.5, 0.5), vec![[1.0; 4]]).unwrap(),
        offset: Vec2::new(99.0, 99.0),
    });
    for mode in Mode::ALL {
        for seconds in frames() {
            rig.parts(seconds, mode, &mut out);
            assert_eq!(out.len(), 8, "{mode:?} at {seconds}: {} parts", out.len());
            for (part, name) in out.iter().zip(PartName::ALL) {
                assert_eq!(part.name, name, "{mode:?} at {seconds}: out of painting order");
                assert_eq!(
                    part.rig_part().layer,
                    name.layer(),
                    "{mode:?} at {seconds}: {name:?} was handed to the renderer at the wrong depth"
                );
            }
        }
    }
    // The layer table of the doc comment, read back: the far limb behind everything, the
    // underside under the hull, the hull's four pieces one material, the glow over them and the
    // near limb in front.
    assert_eq!(PartName::FarLimb.layer(), 0);
    assert_eq!(PartName::Underside.layer(), 1);
    for hull in HULL {
        assert_eq!(hull.layer(), 2, "{hull:?} is a hull piece and must share one layer");
    }
    assert_eq!(PartName::Glow.layer(), 3);
    assert_eq!(PartName::NearLimb.layer(), 4);
    // Ascending painting order is ascending depth.
    let layers: Vec<u8> = PartName::ALL.into_iter().map(PartName::layer).collect();
    assert!(layers.windows(2).all(|w| w[0] <= w[1]), "painting order must ascend in depth: {layers:?}");
}

/// The **body lattice** rule: "The hull pieces, the underside and the glow share the body
/// lattice: their `offset`s and their sprites' pivots have integer coordinates, so every texel
/// centre of those parts sits at a half-integer body coordinate exactly as a template cell does.
/// This is what makes the hull's four pieces sum to the uncut hull under bilinear sampling."
///
/// A fractional offset or pivot on any one of those parts would put its texels on a *different*
/// lattice from its neighbours, and the per-layer sum would then reconstruct a blurred, brighter
/// joint instead of the uncut hull — a defect that is invisible at a single instant and obvious
/// in motion. The limbs are exempt: they are their own depths, not pieces of one material.
#[test]
fn the_hull_the_underside_and_the_glow_stay_on_the_body_lattice() {
    let rig = Lanternjaw::new();
    let mut out = Vec::new();
    for mode in Mode::ALL {
        for seconds in frames() {
            rig.parts(seconds, mode, &mut out);
            for part in out.iter().filter(|p| ON_LATTICE.contains(&p.name)) {
                for (label, v) in
                    [("offset", part.offset), ("pivot", part.sprite.pivot())]
                {
                    assert_eq!(
                        (v.x.fract(), v.y.fract()),
                        (0.0, 0.0),
                        "{mode:?} at {seconds}: {:?}'s {label} {v:?} is off the body lattice",
                        part.name
                    );
                }
            }
            // And all four hull pieces really do share one lattice: the difference between any
            // two pieces' (offset − pivot) is integral, which is the condition under which
            // their bilinear samples add up to the uncut material.
            let base: Vec<Vec2> = HULL
                .iter()
                .map(|&n| {
                    let p = part_of(&out, n);
                    p.offset - p.sprite.pivot()
                })
                .collect();
            for (n, b) in HULL.iter().zip(&base) {
                let d = *b - base[0];
                assert_eq!(
                    (d.x.fract(), d.y.fract()),
                    (0.0, 0.0),
                    "{mode:?} at {seconds}: {n:?} is on a different lattice from the tail ({d:?})"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 3. the footprint contract
// ---------------------------------------------------------------------------

/// The module doc's **normative** footprint: "In every mode at every time, every painted texel
/// centre of every part, in body-local pixels from the anchor … lies within
/// `x ∈ [−BOUND_BACK, BOUND_FRONT]`, `y ∈ [−BOUND_ABOVE, BOUND_BELOW]`; every part's sprite,
/// from its own pivot, has an extent of at most `PART_EXTENT_MAX`; and the rig's query radius
/// … is at most `QUERY_RADIUS_MAX`."
///
/// All three bounds matter for a different reason. The body-local bounds are the contract a
/// caller places the animal by. `PART_EXTENT_MAX` is the per-*material* nine-pixel budget with
/// slack for the true bilinear corner (`8 + 0.207 < 9`, Astra §6.3), and the sweep over every
/// sampled frame is what keeps an interpolated extremum — a coil, a lunge, a wave crest — from
/// quietly exceeding it. `QUERY_RADIUS_MAX` is what `Lanternjaw::draw`'s own `rig_radius` must
/// stay under so the single root query is never clamped.
///
/// The sweep is every 1/60 s for 12 s in all four modes, which covers two whole `hunt` cycles,
/// five gaits and every wave and pulse phase the modes use.
#[test]
fn every_frame_of_every_mode_holds_the_footprint_the_extent_and_the_query_radius() {
    let rig = Lanternjaw::new();
    let mut out = Vec::new();
    let mut worst_x = (0.0f64, 0.0f64);
    let mut worst_y = (0.0f64, 0.0f64);
    let mut worst_extent = 0.0f64;
    let mut worst_radius = 0.0f64;
    let mut any_painted = false;

    for mode in Mode::ALL {
        for seconds in frames() {
            rig.parts(seconds, mode, &mut out);
            for part in &out {
                assert!(
                    part.sprite.extent() <= PART_EXTENT_MAX + 1e-9,
                    "{mode:?} at {seconds}: {:?}'s extent {} exceeds PART_EXTENT_MAX",
                    part.name,
                    part.sprite.extent()
                );
                worst_extent = worst_extent.max(part.sprite.extent());
                for (tx, ty) in painted(&part.sprite) {
                    any_painted = true;
                    let b = texel_body(part, tx, ty);
                    assert!(
                        b.x >= -BOUND_BACK - 1e-9 && b.x <= BOUND_FRONT + 1e-9,
                        "{mode:?} at {seconds}: {:?} texel ({tx}, {ty}) is at body x {} — \
                         outside [{}, {}]",
                        part.name,
                        b.x,
                        -BOUND_BACK,
                        BOUND_FRONT
                    );
                    assert!(
                        b.y >= -BOUND_ABOVE - 1e-9 && b.y <= BOUND_BELOW + 1e-9,
                        "{mode:?} at {seconds}: {:?} texel ({tx}, {ty}) is at body y {} — \
                         outside [{}, {}]",
                        part.name,
                        b.y,
                        -BOUND_ABOVE,
                        BOUND_BELOW
                    );
                    worst_x = (worst_x.0.min(b.x), worst_x.1.max(b.x));
                    worst_y = (worst_y.0.min(b.y), worst_y.1.max(b.y));
                }
            }
            let states: Vec<RigPart<'_>> = out.iter().map(Part::rig_part).collect();
            let radius = rig_radius(&[(&states[..], 1.0)]);
            assert!(
                radius <= QUERY_RADIUS_MAX,
                "{mode:?} at {seconds}: the rig needs a query radius of {radius}, past \
                 QUERY_RADIUS_MAX"
            );
            worst_radius = worst_radius.max(radius);
        }
    }
    assert!(any_painted, "the sweep never saw a painted texel");
    // The bounds must be *used*, or they are not evidence of anything: the study's hull is
    // 18 px long and its strike reaches 12 px ahead, so the body has to fill most of its box.
    assert!(
        worst_x.0 < -8.0 && worst_x.1 > 8.0,
        "the sweep only reached body x {worst_x:?}; the hull alone spans −9 … +8"
    );
    assert!(worst_y.0 < -2.0 && worst_y.1 > 2.0, "the sweep only reached body y {worst_y:?}");
    eprintln!(
        "footprint over 4 modes x {SWEEP_SECONDS} s at {FPS} fps: x {worst_x:?} y {worst_y:?}, \
         extent {worst_extent:.3} / {PART_EXTENT_MAX}, radius {worst_radius:.3} / \
         {QUERY_RADIUS_MAX}"
    );
}

// ---------------------------------------------------------------------------
// 4. purity
// ---------------------------------------------------------------------------

/// "A pure function of `(seconds, mode)`: the same arguments give the same sprites texel for
/// texel, whatever was asked before, so a repeated draw is the same image, pausing holds the
/// pose, and nothing here restarts on a frame." Also: "A non-finite `seconds` reads as 0."
///
/// Everything downstream depends on this — a host at 30, 60 or 144 fps shows the same picture at
/// the same instant, a paused viewer holds still, and every other test in this file is
/// reproducible. A rig that advanced an internal cursor per call would fail on the very first
/// repeat.
#[test]
fn parts_and_draw_are_pure_functions_of_the_time_and_the_mode() {
    let rig = Lanternjaw::new();
    let mut a = Vec::new();
    let mut b = Vec::new();
    // Interleave the probes so a hidden cursor cannot accidentally line up.
    let probes = [0.0, 3.32, 1.0 / 60.0, 7.5, 3.32, 0.0, 11.99, 7.5];
    for mode in Mode::ALL {
        for &t in &probes {
            rig.parts(t, mode, &mut a);
            // Ask for a different frame in between, then come back.
            rig.parts(t + 0.37, mode, &mut b);
            rig.parts(t, mode, &mut b);
            assert_parts_identical(&a, &b, &format!("{mode:?} at {t}"));
        }
    }
    // A whole second rig object agrees with the first.
    let other = Lanternjaw::new();
    other.parts(3.32, Mode::Hunt, &mut b);
    rig.parts(3.32, Mode::Hunt, &mut a);
    assert_parts_identical(&a, &b, "two independently constructed rigs");

    // A non-finite time reads as 0.
    rig.parts(0.0, Mode::Move, &mut a);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        rig.parts(bad, Mode::Move, &mut b);
        assert_parts_identical(&a, &b, &format!("a seconds of {bad} must read as 0"));
    }

    // And the drawn image repeats, including over a non-trivial background and at a seam.
    for (what, anchor) in [
        ("mid-face", mid()),
        ("a side seam", SurfacePoint::new(Face::Front, 63.5, 32.5)),
        ("a top vertex", SurfacePoint::new(Face::Top, 0.5, 0.5)),
    ] {
        let mut parts = Vec::new();
        let mut scratch = Vec::new();
        let mut first = filled([0.03, 0.02, 0.07]);
        let mut second = filled([0.03, 0.02, 0.07]);
        rig.draw(&mut first, anchor, Vec2::new(0.6, -0.8), 3.32, Mode::Hunt, 0.9, &mut parts, &mut scratch);
        // Draw something else with the same buffers, then repeat the original.
        let mut junk = Canvas::new();
        rig.draw(&mut junk, mid(), forward(), 1.0, Mode::Bud, 1.0, &mut parts, &mut scratch);
        rig.draw(&mut second, anchor, Vec2::new(0.6, -0.8), 3.32, Mode::Hunt, 0.9, &mut parts, &mut scratch);
        assert_identical(&first, &second, &format!("a repeated draw at {what}"));
        assert!(max_diff(&first, &filled([0.03, 0.02, 0.07])) > 0.01, "{what}: nothing was drawn");
    }
}

fn assert_parts_identical(a: &[Part], b: &[Part], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: part counts differ");
    for (p, q) in a.iter().zip(b) {
        assert_eq!(p.name, q.name, "{what}");
        assert_eq!(p.offset, q.offset, "{what}: {:?}'s offset", p.name);
        assert_eq!(p.sprite.pivot(), q.sprite.pivot(), "{what}: {:?}'s pivot", p.name);
        assert_eq!(
            (p.sprite.width(), p.sprite.height()),
            (q.sprite.width(), q.sprite.height()),
            "{what}: {:?}'s image size",
            p.name
        );
        for ty in 0..p.sprite.height() {
            for tx in 0..p.sprite.width() {
                assert_eq!(
                    p.sprite.texel(tx as i32, ty as i32),
                    q.sprite.texel(tx as i32, ty as i32),
                    "{what}: {:?} texel ({tx}, {ty})",
                    p.name
                );
            }
        }
    }
}

/// "Every sub-rhythm divides it, so `Hunt` at `t` and `t + 6` are the same picture." `fable.md`
/// makes the same claim of the study ("the frame at t and at t+6 are pixel-identical"), and the
/// charge's forced decay is what buys it. A loop that stepped would be visible as a twitch once
/// every six seconds forever.
#[test]
fn the_hunt_cycle_is_exactly_periodic_at_six_seconds() {
    let rig = Lanternjaw::new();
    let mut a = Vec::new();
    let mut b = Vec::new();
    for i in 0..FPS * 6 {
        let t = i as f64 / FPS as f64;
        rig.parts(t, Mode::Hunt, &mut a);
        rig.parts(t + HUNT_PERIOD, Mode::Hunt, &mut b);
        assert_parts_identical(&a, &b, &format!("hunt at {t} vs {} ", t + HUNT_PERIOD));
    }
    // And the drawn image too, at the same 60 fps instants: bit for bit one cycle on.
    for i in [0usize, 60, 193, 199, 212, 330] {
        let t = i as f64 / FPS as f64;
        assert_identical(
            &drawn(t, Mode::Hunt, mid(), forward(), 1.0),
            &drawn(t + HUNT_PERIOD, Mode::Hunt, mid(), forward(), 1.0),
            &format!("hunt at {t} vs one cycle later"),
        );
    }
    // At an *arbitrary* instant the claim is the same picture to within the phase reduction's
    // own rounding, not bit for bit: `seconds mod HUNT_PERIOD` is not exact in binary floating
    // point (`15.22 − 12` is `3.2199999999999989`, not `3.22`), so two frames a cycle apart
    // can differ in the last bits of a colour ramp. The study could claim pixel identity
    // because it rounded every position to a whole pixel; fractional motion cannot, and the
    // measured divergence below is about 1e-18 — twenty orders of magnitude under a visible
    // step.
    for t in [3.22, 3.32, 3.54, 5.5] {
        let d = max_diff(
            &drawn(t, Mode::Hunt, mid(), forward(), 1.0),
            &drawn(t + 2.0 * HUNT_PERIOD, Mode::Hunt, mid(), forward(), 1.0),
        );
        assert!(d <= 1e-6, "hunt at {t} vs two cycles later differs by {d}");
    }
    // Non-vacuity: the cycle is not a still image.
    assert!(
        max_diff(
            &drawn(3.0, Mode::Hunt, mid(), forward(), 1.0),
            &drawn(3.32, Mode::Hunt, mid(), forward(), 1.0)
        ) > 0.05,
        "the strike must change the picture, or periodicity is trivial"
    );
}

// ---------------------------------------------------------------------------
// 5. the warm strike accent
// ---------------------------------------------------------------------------

/// "The warm accent still appears only at the peak of the hunt's 240 ms strike envelope", and
/// `fable.md`: "the strike's warm accent never appears in `rest`, `move` or `bud`" — verified
/// there over 1 800 frames. The same sweep here, 720 frames per mode, through the hue test
/// derived above.
///
/// In `hunt` the accent is `envelope((t_h − (T_SNAP − 0.02)) / ACCENT_SECONDS)`, so its whole
/// window is `t_h ∈ (3.20, 3.44)` and it can only clear the hue threshold inside that. The test
/// asserts the containment (a warm pixel one frame outside the window is a leak), that the
/// accent actually *reaches* its plateau, and that it eases: several frames land strictly
/// between the threshold and the peak, where a flash would give none.
#[test]
fn the_warm_strike_accent_appears_only_inside_the_hunts_strike_envelope() {
    let rig = Lanternjaw::new();
    let mut out = Vec::new();

    for mode in [Mode::Rest, Mode::Move, Mode::Bud] {
        for seconds in frames() {
            rig.parts(seconds, mode, &mut out);
            let w = warmest(&out);
            assert!(
                w <= WARM,
                "{mode:?} at {seconds}: a texel is warm ({w} of red over blue, past the {WARM} \
                 hue threshold) — the strike accent leaked into a calm mode"
            );
        }
    }

    // One whole hunt cycle, frame by frame.
    let mut warm_frames: Vec<(f64, f32)> = Vec::new();
    let mut peak = f32::NEG_INFINITY;
    for i in 0..FPS * 6 {
        let t = i as f64 / FPS as f64;
        rig.parts(t, Mode::Hunt, &mut out);
        let w = warmest(&out);
        peak = peak.max(w);
        if w > WARM {
            warm_frames.push((t, w));
        }
    }
    assert!(peak > 0.2, "the accent never reached its plateau: peak {peak}");
    assert!(!warm_frames.is_empty(), "the hunt never showed the warm accent at all");
    for &(t, w) in &warm_frames {
        assert!(
            t > T_SNAP - 0.02 && t < T_SNAP - 0.02 + ACCENT_SECONDS,
            "a warm texel at t = {t} ({w}) is outside the strike envelope's window \
             ({}, {})",
            T_SNAP - 0.02,
            T_SNAP - 0.02 + ACCENT_SECONDS
        );
    }
    // The 240 ms envelope is 14.4 frames at 60 fps; the study measured 12 frames above its own
    // threshold. Several of them must be part-way up the ramp, not all at the plateau: an
    // accent that switched on for one frame is what the raised cosine exists to prevent.
    let easing = warm_frames.iter().filter(|&&(_, w)| w < 0.9 * peak).count();
    assert!(
        warm_frames.len() >= 6,
        "the accent showed on only {} frames of a 240 ms envelope",
        warm_frames.len()
    );
    assert!(easing >= 4, "only {easing} of the accent's frames are part-way up its ramp");
    eprintln!(
        "strike accent: {} warm frames from {:.3} to {:.3} s, peak {peak:.3}, {easing} easing",
        warm_frames.len(),
        warm_frames[0].0,
        warm_frames[warm_frames.len() - 1].0
    );
}

// ---------------------------------------------------------------------------
// 6. the blink eases
// ---------------------------------------------------------------------------

/// `fable.md`: the blink is a 280 ms raised cosine, "the eye walks to the lid colour and back
/// over ~17 frames", and the study measured "a largest single-frame step of 0.23 of its full
/// swing — a hard cut would be 1.0 with zero intermediate frames". The brief's bound is 0.3.
///
/// The derivation, so the bound is not a tuned number: the envelope's steepest slope is
/// `d/du (0.5 − 0.5·cos(πu/0.4)) = 1.25π ≈ 3.93` per unit `u`, one frame at 60 fps advances
/// `u` by `(1/60) / 0.28 = 0.0595`, so the largest per-frame change of the *closure* is
/// `3.93 × 0.0595 = 0.234` of its full range.
///
/// The reading is a monotone but **not affine** function of the closure: the eye's mix runs
/// `PINK → EYE_SHUT` in sRGB and is decoded once into linear light, and `srgb_decode` is convex,
/// so the reading's steepest slope is about 1.18 times its average and the largest per-frame
/// step lands near `0.234 × 1.18 = 0.276` of the swing rather than at 0.234. That is still
/// comfortably inside the brief's 0.3, and an order of magnitude inside the 1.0 a hard cut
/// would give; the measured value is printed so the margin is visible rather than assumed.
#[test]
fn the_blink_eases_through_intermediate_values_at_sixty_frames_a_second() {
    let rig = Lanternjaw::new();
    let mut out = Vec::new();
    for (mode, period) in [
        (Mode::Rest, BLINK_PERIOD_REST),
        (Mode::Move, BLINK_PERIOD_MOVE),
        (Mode::Bud, BLINK_PERIOD_BUD),
    ] {
        let mut readings = Vec::new();
        for seconds in frames() {
            rig.parts(seconds, mode, &mut out);
            readings.push(eye_reading(&out));
        }
        let lo = readings.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = readings.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let swing = hi - lo;
        assert!(
            swing > 0.3,
            "{mode:?}: the eye's reading only moved by {swing} over {SWEEP_SECONDS} s — with a \
             blink every {period} s it must close at least once"
        );
        let step = readings
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f64, f64::max);
        assert!(
            step <= 0.3 * swing,
            "{mode:?}: the eye moved {step} in one frame, more than 0.3 of its {swing} swing \
             (the raised cosine's own largest step is 0.234 of the swing; a hard cut is 1.0)"
        );
        // And it really does pass through the middle: a cut would show none of these.
        let middle = readings
            .iter()
            .filter(|&&v| v > lo + 0.15 * swing && v < hi - 0.15 * swing)
            .count();
        assert!(
            middle >= 8,
            "{mode:?}: only {middle} frames of {SWEEP_SECONDS} s sit part-way through the \
             blink; a 280 ms raised cosine should show about 17 per blink"
        );
        eprintln!("{mode:?} blink: swing {swing:.3}, largest step {step:.4} ({:.3} of the swing), {middle} intermediate frames", step / swing);
    }
}

// ---------------------------------------------------------------------------
// 7. depth ownership at the strike
// ---------------------------------------------------------------------------

/// "Overlap is painting order", made explicit as layers: the far forelimb is *under* the hull
/// and the near one *over* it. Checked at the strike, where the limbs are extended, lit and
/// unambiguously across the hull.
///
/// Three independent readings, all through the public `parts` / `rig_part` / `stamp_rig` path:
///
/// * **the far limb loses.** The hull's own coverage `α` at a pixel is measured exactly, without
///   reading any private state, by stamping the four hull pieces over black and over white:
///   `α = 1 − (white − black)`. Adding the far limb *underneath* can then change the pixel by at
///   most `(1 − α) · L`, with `L` the far limb's largest premultiplied channel — that is what
///   "underneath" means arithmetically. (Fractional motion means the hull is never *exactly*
///   opaque: its peak coverage at this frame is about 0.98, and where it overlaps the far limb
///   about 0.91, so the bound there is a couple of percent of the limb's own light rather than
///   zero. The control below shows what the wrong depth costs by comparison.)
/// * **the near limb wins.** At the pixel where the near limb paints most, the hull also paints,
///   and the whole body differs from the body-without-the-near-limb: the limb is in front.
/// * **the depth order is the composite.** With `opacity` 1 the whole body equals the first
///   seven parts composited and then the near limb stamped on top — the exact decomposition
///   ascending layer order means — while the opposite order is a different picture, so the order
///   is doing work rather than being unobservable.
#[test]
fn at_the_strike_the_near_limb_is_over_the_hull_and_the_far_limb_under_it() {
    // Full extension plus a frame: the limbs are at their longest and the accent is lit.
    let seconds = T_OPEN;
    let parts = body(seconds, Mode::Hunt);
    let anchor = mid();
    let heading = forward();
    let all: Vec<PartName> = PartName::ALL.to_vec();
    let without = |skip: PartName| -> Vec<PartName> {
        PartName::ALL.into_iter().filter(|&n| n != skip).collect()
    };

    let whole_body = stamp_only(&parts, &all, anchor, heading, 1.0, [0.0; 3]);
    let no_far = stamp_only(&parts, &without(PartName::FarLimb), anchor, heading, 1.0, [0.0; 3]);
    let no_near = stamp_only(&parts, &without(PartName::NearLimb), anchor, heading, 1.0, [0.0; 3]);
    let far_only = stamp_only(&parts, &[PartName::FarLimb], anchor, heading, 1.0, [0.0; 3]);
    let near_only = stamp_only(&parts, &[PartName::NearLimb], anchor, heading, 1.0, [0.0; 3]);
    let hull_black = stamp_only(&parts, &HULL, anchor, heading, 1.0, [0.0; 3]);
    let hull_white = stamp_only(&parts, &HULL, anchor, heading, 1.0, [1.0; 3]);

    assert!(max_diff(&far_only, &Canvas::new()) > 0.01, "the far limb painted nothing");
    assert!(max_diff(&near_only, &Canvas::new()) > 0.01, "the near limb painted nothing");

    // The hull's coverage, exactly: over black the pixel is `c`, over white it is
    // `c + (1 − c.a)`, so `c.a = 1 − (white − black)`.
    let hull_coverage = |f: Face, x: u8, y: u8| -> f32 {
        (0..3)
            .map(|c| 1.0 - (hull_white.get(f, x, y)[c] - hull_black.get(f, x, y)[c]))
            .fold(f32::MAX, f32::min)
    };
    // The most light the far limb's own material can carry into a pixel.
    let far_light = part_of(&parts, PartName::FarLimb);
    let l_far = painted(&far_light.sprite)
        .map(|(tx, ty)| {
            let t = far_light.sprite.texel(tx as i32, ty as i32);
            (0..3).map(|c| t[c]).fold(0.0f32, f32::max)
        })
        .fold(0.0f32, f32::max);
    assert!(l_far > 0.02, "the far limb carries only {l_far} of light, so hiding it proves little");

    // The wrong depth, as a control: the very same far-limb sprite handed to the renderer at a
    // layer above everything else.
    let mut wrong_depth: Vec<RigPart<'_>> = parts
        .iter()
        .filter(|p| p.name != PartName::FarLimb)
        .map(Part::rig_part)
        .collect();
    wrong_depth.push(RigPart {
        sprite: &far_light.sprite,
        offset: far_light.offset,
        layer: 9,
    });
    let mut far_on_top = Canvas::new();
    stamp_rig(&mut far_on_top, anchor, heading, &[(&wrong_depth[..], 1.0)], 1.0, &mut Vec::new());

    let mut hidden = 0usize;
    let mut worst_right = 0.0f32;
    let mut best_wrong = 0.0f32;
    for (f, x, y) in every_pixel() {
        let alpha = hull_coverage(f, x, y);
        if far_only.get(f, x, y) == [0.0; 3] || alpha <= 0.5 {
            continue;
        }
        hidden += 1;
        let right = (0..3)
            .map(|c| (whole_body.get(f, x, y)[c] - no_far.get(f, x, y)[c]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            right <= (1.0 - alpha) * l_far + 1e-6,
            "({f:?}, {x}, {y}): the hull covers {alpha} of this pixel, so a far limb *under* it \
             can change it by at most {} — but it changed it by {right}, from {:?} to {:?}",
            (1.0 - alpha) * l_far,
            no_far.get(f, x, y),
            whole_body.get(f, x, y)
        );
        worst_right = worst_right.max(right);
        let wrong = (0..3)
            .map(|c| (far_on_top.get(f, x, y)[c] - no_far.get(f, x, y)[c]).abs())
            .fold(0.0f32, f32::max);
        // Per pixel, so the comparison is like for like: the same sprite, the same destination,
        // only the declared depth changed.
        best_wrong = best_wrong.max(wrong / (right + 1e-4));
    }
    assert!(
        hidden >= 3,
        "only {hidden} mostly-opaque hull pixels overlap the far limb at the strike, so the \
         ownership claim is not being exercised"
    );
    assert!(
        best_wrong >= 3.0,
        "the far limb's depth is unobservable: on no overlap pixel does putting the same sprite \
         in front of the hull move it more than {best_wrong:.2}× as far as leaving it behind"
    );
    eprintln!(
        "far limb under the hull: {hidden} overlap pixels, moved by at most {worst_right:.5}; \
         the same sprite in front moves one of them {best_wrong:.1}× as far"
    );

    // The near limb where it crosses the hull — at full extension the arm runs from the
    // shoulder at body (4, 1), over the carapace, out to the claw at (12.3, 0.6), so the pixels
    // that matter are the ones the limb and the hull both paint. The brightest of those is the
    // test point, and the control is the same sprite declared *behind* everything.
    let mut behind: Vec<RigPart<'_>> = parts
        .iter()
        .filter(|p| p.name != PartName::NearLimb)
        .map(Part::rig_part)
        .collect();
    let near_part = part_of(&parts, PartName::NearLimb);
    behind.insert(0, RigPart { sprite: &near_part.sprite, offset: near_part.offset, layer: 0 });
    let mut near_at_back = Canvas::new();
    stamp_rig(&mut near_at_back, anchor, heading, &[(&behind[..], 1.0)], 1.0, &mut Vec::new());

    let overlap: Vec<(Face, u8, u8)> = every_pixel()
        .filter(|&(f, x, y)| {
            near_only.get(f, x, y) != [0.0; 3] && hull_coverage(f, x, y) > 0.5
        })
        .collect();
    assert!(
        overlap.len() >= 3,
        "only {} pixels have the near limb over mostly-opaque hull at the strike",
        overlap.len()
    );
    let (f, x, y) = *overlap
        .iter()
        .max_by(|&&a, &&b| {
            let l = |p: (Face, u8, u8)| {
                near_only.get(p.0, p.1, p.2).into_iter().map(f64::from).sum::<f64>()
            };
            l(a).partial_cmp(&l(b)).unwrap()
        })
        .unwrap();
    let in_front = (0..3)
        .map(|c| (whole_body.get(f, x, y)[c] - no_near.get(f, x, y)[c]).abs())
        .fold(0.0f32, f32::max);
    let at_back = (0..3)
        .map(|c| (near_at_back.get(f, x, y)[c] - no_near.get(f, x, y)[c]).abs())
        .fold(0.0f32, f32::max);
    assert!(
        in_front > 0.01,
        "({f:?}, {x}, {y}): the near limb changed this hull pixel (coverage {}) by only \
         {in_front} — the arm is behind the carapace, not in front of it",
        hull_coverage(f, x, y)
    );
    assert!(
        in_front >= 3.0 * at_back,
        "({f:?}, {x}, {y}): the same near-limb sprite moves this pixel {in_front} in front of \
         the hull and {at_back} behind it — the near limb's depth is barely observable, so this \
         reading proves little"
    );
    eprintln!(
        "near limb over the hull: {} overlap pixels; at ({f:?}, {x}, {y}) the arm moves the \
         carapace by {in_front:.5} in front and {at_back:.5} behind",
        overlap.len()
    );

    // Ascending layer order *is* the composite: at opacity 1, seven parts then the eighth.
    let mut staged = stamp_only(&parts, &without(PartName::NearLimb), anchor, heading, 1.0, [0.0; 3]);
    let near: Vec<RigPart<'_>> = parts
        .iter()
        .filter(|p| p.name == PartName::NearLimb)
        .map(Part::rig_part)
        .collect();
    stamp_rig(&mut staged, anchor, heading, &[(&near[..], 1.0)], 1.0, &mut Vec::new());
    assert!(
        max_diff(&staged, &whole_body) <= 1e-6,
        "the body is not its first seven parts with the near limb stamped over them: {}",
        max_diff(&staged, &whole_body)
    );
    // Stamping the near limb *first* is a different picture, so the order is observable.
    let mut wrong = stamp_only(&parts, &[PartName::NearLimb], anchor, heading, 1.0, [0.0; 3]);
    let rest: Vec<RigPart<'_>> = parts
        .iter()
        .filter(|p| p.name != PartName::NearLimb)
        .map(Part::rig_part)
        .collect();
    stamp_rig(&mut wrong, anchor, heading, &[(&rest[..], 1.0)], 1.0, &mut Vec::new());
    assert!(
        max_diff(&wrong, &whole_body) > 0.005,
        "the near limb's depth is unobservable in this frame, so the test proves nothing"
    );
}

// ---------------------------------------------------------------------------
// 8. opacity belongs to the whole animal
// ---------------------------------------------------------------------------

/// `stamp_rig` applies `opacity` **once** to the assembled body: "the body sample `c` is
/// composited once: `canvas = c · opacity + canvas · (1 − c.a · opacity)`". Astra §4 names the
/// failure this rules out: "two overlapping opaque parts drawn at opacity 0.5 yield 0.75
/// coverage, whereas fading their assembled opaque body yields 0.5. Do not accidentally build
/// fade-dependent ridges into the hull."
///
/// The Lanternjaw has such overlaps by construction — the near limb crosses the hull at the
/// strike, and the hull's own pieces overlap where the coil compresses it — so the property is
/// read off the real body. Effective coverage is measured exactly, without reading any private
/// state: drawing over black gives `c · opacity` and over white gives `c · opacity + 1 − c.a ·
/// opacity`, so `c.a · opacity = 1 − (white − black)`. At `opacity` 0.5 that can never exceed
/// 0.5 anywhere, and must reach it wherever the body is opaque.
#[test]
fn opacity_fades_the_assembled_body_once_and_never_ridges_an_overlap() {
    for (what, seconds, mode) in
        [("the strike", T_OPEN, Mode::Hunt), ("a gait", 0.4, Mode::Move), ("a cocoon", 1.3, Mode::Bud)]
    {
        // First: the body really does have overlapping material at different depths, or a
        // per-part fade could not ridge and the test would be vacuous. Each layer's own
        // coverage is measured exactly (over black the pixel is `c`, over white it is
        // `c + (1 − c.a)`), and their sum exceeding 1 is precisely "two depths both cover this
        // pixel" — the configuration in which fading each part separately leaves
        // `1 − (1 − 0.5·α₁)(1 − 0.5·α₂)` instead of `0.5 · α`.
        let parts = body(seconds, mode);
        let groups: [&[PartName]; 5] = [
            &[PartName::FarLimb],
            &[PartName::Underside],
            &HULL,
            &[PartName::Glow],
            &[PartName::NearLimb],
        ];
        let maps: Vec<(Canvas, Canvas)> = groups
            .iter()
            .map(|g| {
                (
                    stamp_only(&parts, g, mid(), forward(), 1.0, [0.0; 3]),
                    stamp_only(&parts, g, mid(), forward(), 1.0, [1.0; 3]),
                )
            })
            .collect();
        let stacked = every_pixel()
            .map(|(f, x, y)| {
                maps.iter()
                    .map(|(b, w)| {
                        (0..3)
                            .map(|c| 1.0 - (w.get(f, x, y)[c] - b.get(f, x, y)[c]))
                            .fold(f32::MAX, f32::min)
                    })
                    .sum::<f32>()
            })
            .fold(0.0f32, f32::max);
        assert!(
            stacked >= 1.2,
            "{what}: the depths never stack past {stacked} of coverage on one pixel, so a \
             per-part fade could not have ridged and this test proves nothing"
        );

        let opacity = 0.5f32;
        let black = drawn(seconds, mode, mid(), forward(), opacity);
        let white = {
            let mut canvas = filled([1.0; 3]);
            Lanternjaw::new().draw(
                &mut canvas,
                mid(),
                forward(),
                seconds,
                mode,
                opacity,
                &mut Vec::new(),
                &mut Vec::new(),
            );
            canvas
        };
        let mut most = 0.0f32;
        for (f, x, y) in every_pixel() {
            for c in 0..3 {
                let coverage = 1.0 - (white.get(f, x, y)[c] - black.get(f, x, y)[c]);
                assert!(
                    coverage <= opacity + 1e-6,
                    "{what}: ({f:?}, {x}, {y}) channel {c} has effective coverage {coverage} at \
                     opacity {opacity} — two overlapping opaque parts faded separately would \
                     read 0.75, one faded body reads 0.5"
                );
                most = most.max(coverage);
            }
        }
        // Fractional motion means no pixel is *exactly* opaque (the body's peak coverage is
        // about 0.98), so the non-vacuity bound is that the body is nearly opaque somewhere: at
        // a pixel where one depth covers 0.98 and another 0.5, separate half-opacity stamps
        // would read 0.74 against the 0.49 one faded body reads.
        assert!(
            most >= 0.95 * opacity,
            "{what}: the most opaque pixel only reached coverage {most} of a possible \
             {opacity}, so the body is nowhere solid and the ridge could not have shown"
        );
        // And the fade is linear in the opacity over black: `c · opacity`.
        let full = drawn(seconds, mode, mid(), forward(), 1.0);
        for (f, x, y) in every_pixel() {
            for c in 0..3 {
                assert!(
                    (black.get(f, x, y)[c] - 0.5 * full.get(f, x, y)[c]).abs() <= 1e-6,
                    "{what}: ({f:?}, {x}, {y}) channel {c} at opacity 0.5 is {}, not half of \
                     the opaque body's {}",
                    black.get(f, x, y)[c],
                    full.get(f, x, y)[c]
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 9. the body frame
// ---------------------------------------------------------------------------

/// The body frame is "`+x` forward, `+y` down when facing image-right". A frame whose `+y`
/// pointed the other way would draw the animal upside down and still satisfy every symmetric
/// bound in this file (`BOUND_ABOVE` and `BOUND_BELOW` are both 5.5), so the convention needs
/// its own assertion, and the rig carries two parts that pin it unambiguously:
///
/// * `Glow` is "the lantern halos (**one row up**) and glows (**two rows up**) of every
///   lantern", so every one of its painted texels must be at negative body `y`;
/// * `Underside` is "the three walking-leg pairs and, in `Bud`, the cocoon" — the study's legs
///   are at body `y = 2` and `3` and its cocoon at `2` and `3` — so all of its painted texels
///   must be at positive body `y`.
///
/// Those are statements about `parts`; the second half of the test ties them to the chart by
/// stamping each alone and checking that the glow lands *above* the anchor's own pixel row and
/// the underside *below* it, which is what "`+y` down when the heading is image-right" means.
#[test]
fn the_body_frame_puts_the_glow_above_the_mid_line_and_the_legs_below_it() {
    let rig = Lanternjaw::new();
    let mut out = Vec::new();
    for mode in Mode::ALL {
        for seconds in frames() {
            rig.parts(seconds, mode, &mut out);
            for (name, sign, why) in [
                (PartName::Glow, -1.0f64, "the lantern halo is one row up and its glow two"),
                (PartName::Underside, 1.0, "the walking legs and the cocoon are under the hull"),
            ] {
                let part = part_of(&out, name);
                for (tx, ty) in painted(&part.sprite) {
                    let b = texel_body(part, tx, ty);
                    assert!(
                        b.y * sign > 0.0,
                        "{mode:?} at {seconds}: {name:?} texel ({tx}, {ty}) is at body y {} — \
                         {why}, so the body frame's +y is the wrong way round",
                        b.y
                    );
                }
            }
        }
    }

    // And on the chart, with the heading image-right, +y is image-down. The glow only exists
    // while a lantern is past its halo threshold, so the instant is chosen as the brightest
    // glow of the `rest` sweep rather than assumed.
    let brightest_glow = frames()
        .max_by(|&a, &b| {
            let light = |t: f64| {
                let parts = body(t, Mode::Rest);
                let glow = part_of(&parts, PartName::Glow);
                painted(&glow.sprite)
                    .map(|(tx, ty)| f64::from(glow.sprite.texel(tx as i32, ty as i32)[3]))
                    .sum::<f64>()
            };
            light(a).partial_cmp(&light(b)).unwrap()
        })
        .unwrap();
    let parts = body(brightest_glow, Mode::Rest);
    let centroid = |name: PartName| -> f64 {
        let image = stamp_only(&parts, &[name], mid(), forward(), 1.0, [0.0; 3]);
        let mut light = 0.0f64;
        let mut moment = 0.0f64;
        for (f, x, y) in every_pixel() {
            let l: f64 = image.get(f, x, y).into_iter().map(f64::from).sum();
            assert!(l == 0.0 || f == Face::Front, "the mid-face part reached {f:?}");
            light += l;
            moment += l * f64::from(y);
        }
        assert!(light > 1e-3, "{name:?} painted nothing, so its centroid proves nothing");
        moment / light
    };
    let glow = centroid(PartName::Glow);
    let legs = centroid(PartName::Underside);
    // The anchor sits at the centre of pixel row 32.
    assert!(glow < 32.0, "the glow's light is centred on chart row {glow}, not above the anchor");
    assert!(legs > 32.0, "the legs' light is centred on chart row {legs}, not below the anchor");
    eprintln!("body frame: glow centred on chart row {glow:.2}, underside on {legs:.2}");
}

/// Heading `(−1, 0)` is a **half turn** about the anchor, not a horizontal mirror: with
/// `side = (−h.y, h.x)`, negating `h` negates both body axes, so the pixel `(x, y)` of the
/// reversed body is the pixel `(64 − x, 64 − y)` of the forward one about a pixel-centre anchor.
/// With the anchor at a pixel centre every body displacement `d` is an exact half-integer
/// difference and its negation is exact, so the comparison is **bit for bit**.
///
/// The last assertion records why the brief's "mirror about the anchor column" is not the
/// property (Astra §5): the Lanternjaw is asymmetric about its own mid-line, so a column-only
/// reflection of the forward body is a *different* picture from the reversed one, and a test
/// written that way would reject correct transport.
#[test]
fn heading_minus_x_is_a_half_turn_about_the_anchor() {
    for (seconds, mode) in [(0.0, Mode::Rest), (0.4, Mode::Move), (T_OPEN, Mode::Hunt), (1.3, Mode::Bud)] {
        let ahead = drawn(seconds, mode, mid(), forward(), 1.0);
        let behind = drawn(seconds, mode, mid(), Vec2::new(-1.0, 0.0), 1.0);
        // Everything stays on Front: the body is 14 px from a mid-face anchor.
        for face in Face::ALL {
            if face == Face::Front {
                continue;
            }
            assert!(
                (0..64u8).all(|y| (0..64u8).all(|x| ahead.get(face, x, y) == [0.0; 3])),
                "{mode:?} at {seconds}: the mid-face body reached {face:?}"
            );
        }
        let mut turned = 0usize;
        for y in 1..64u8 {
            for x in 1..64u8 {
                let got = behind.get(Face::Front, x, y);
                let want = ahead.get(Face::Front, 64 - x, 64 - y);
                assert_eq!(
                    got, want,
                    "{mode:?} at {seconds}: Front ({x}, {y}) of the reversed body is {got:?}, \
                     where the half turn of the forward body has {want:?}"
                );
                if got != [0.0; 3] {
                    turned += 1;
                }
            }
        }
        assert!(turned > 40, "{mode:?} at {seconds}: only {turned} pixels were compared");

        // The column-only mirror the brief asked for is a different picture.
        let mirrored_differs = (1..64u8)
            .any(|y| (1..64u8).any(|x| behind.get(Face::Front, x, y) != ahead.get(Face::Front, 64 - x, y)));
        assert!(
            mirrored_differs,
            "{mode:?} at {seconds}: reversing the heading happened to mirror the body about the \
             anchor column, which would mean the animal is symmetric about its mid-line"
        );
    }
}

// ---------------------------------------------------------------------------
// 10. fractional motion
// ---------------------------------------------------------------------------

/// "The anchor is whatever fractional `SurfacePoint` the caller passes, so motion reads as
/// fractional brightness at 60 fps the way the world's bodies already do" — against the study's
/// own limitation 5, "the anchor is rounded to whole pixels inside `draw`, so a caller animating
/// a fractional position gets 1 px snapping".
///
/// Three properties, each exactly true:
///
/// * a **whole**-pixel anchor shift is a whole-pixel translation of the image, bit for bit
///   (every destination pixel's body displacement is the same double one column over), which is
///   the reference the sub-pixel case is measured against;
/// * a **half**-pixel shift changes the picture — an implementation that rounded the anchor
///   would give the integer image back, bit for bit, and fail here;
/// * and it changes it by no more than the bilinear weight allows. For one part on one layer
///   over black at opacity 1 the canvas value *is* the clamped bilinear sample, whose slope
///   along body `x` is a convex combination of adjacent-texel differences; so a half-pixel shift
///   can move a pixel by at most `0.5 · J`, with `J` the largest adjacent-texel step of that
///   part's own sprite, measured here including its transparent border. A part re-rasterized at
///   a different pose, or a splat placed a whole texel over, breaks this by a wide margin.
#[test]
fn a_fractional_anchor_moves_the_body_by_sub_pixel_brightness_only() {
    let seconds = 0.4;
    let mode = Mode::Move;

    // A whole pixel across is a whole pixel across.
    let at = |u: f64| drawn(seconds, mode, SurfacePoint::new(Face::Front, u, 32.5), forward(), 1.0);
    let (base, shifted) = (at(32.5), at(33.5));
    for y in 0..64u8 {
        for x in 1..64u8 {
            assert_eq!(
                shifted.get(Face::Front, x, y),
                base.get(Face::Front, x - 1, y),
                "a one-pixel anchor shift must translate the image: Front ({x}, {y})"
            );
        }
    }

    // Half a pixel is a different picture: the anchor is not rounded.
    let half = at(33.0);
    assert!(
        max_diff(&half, &base) > 0.01 && max_diff(&half, &shifted) > 0.01,
        "a half-pixel anchor shift changed nothing — the anchor is being rounded to whole pixels \
         (base {}, shifted {})",
        max_diff(&half, &base),
        max_diff(&half, &shifted)
    );

    // And it is a sub-pixel change, bounded by the sprite's own bilinear slope. Measured on one
    // part alone, where the bound is exact and tight.
    let parts = body(seconds, mode);
    for name in [PartName::Head, PartName::Tail, PartName::NearLimb] {
        let part = part_of(&parts, name);
        let sprite = &part.sprite;
        let mut jump = 0.0f32;
        for ty in 0..sprite.height() as i32 {
            for tx in -1..sprite.width() as i32 {
                let (a, b) = (sprite.texel(tx, ty), sprite.texel(tx + 1, ty));
                for c in 0..3 {
                    jump = jump.max((a[c] - b[c]).abs());
                }
            }
        }
        let one = |u: f64| {
            let rig = [part.rig_part()];
            let mut canvas = Canvas::new();
            stamp_rig(
                &mut canvas,
                SurfacePoint::new(Face::Front, u, 32.5),
                forward(),
                &[(&rig[..], 1.0)],
                1.0,
                &mut Vec::new(),
            );
            canvas
        };
        let (p0, ph) = (one(32.5), one(33.0));
        let moved = max_diff(&p0, &ph);
        assert!(
            moved <= 0.5 * jump + 1e-6,
            "{name:?}: a half-pixel anchor shift moved a pixel by {moved}, more than half its \
             sprite's largest adjacent-texel step of {jump}"
        );
        assert!(moved > 1e-4, "{name:?}: a half-pixel shift moved nothing ({moved})");
    }
}

/// "A non-normalizable heading draws nothing", and the canvas is left exactly as it was.
#[test]
fn a_degenerate_heading_or_opacity_draws_nothing() {
    let bg = [0.04, 0.02, 0.11];
    for (what, heading, opacity) in [
        ("a zero heading", Vec2::ZERO, 1.0f32),
        ("a NaN heading", Vec2::new(f64::NAN, 1.0), 1.0),
        ("an infinite heading", Vec2::new(1.0, f64::INFINITY), 1.0),
        ("a zero opacity", forward(), 0.0),
        ("a negative opacity", forward(), -1.0),
        ("a NaN opacity", forward(), f32::NAN),
    ] {
        for mode in Mode::ALL {
            let mut canvas = filled(bg);
            Lanternjaw::new().draw(
                &mut canvas,
                mid(),
                heading,
                0.4,
                mode,
                opacity,
                &mut Vec::new(),
                &mut Vec::new(),
            );
            assert_identical(&canvas, &filled(bg), &format!("{what} in {mode:?}"));
        }
    }
    // The heading's length is not its magnitude of anything: only its direction is used.
    for scale in [0.1f64, 1.0, 25.0] {
        assert_identical(
            &drawn(0.4, Mode::Move, mid(), Vec2::new(0.6 * scale, -0.8 * scale), 1.0),
            &drawn(0.4, Mode::Move, mid(), Vec2::new(0.6, -0.8), 1.0),
            "the heading's length must not change the picture",
        );
    }
}

// ---------------------------------------------------------------------------
// 11. no per-frame jump
// ---------------------------------------------------------------------------

/// Nothing in `move` may jump between consecutive 60 fps frames. **The bound is derived from
/// the wave and the gait, not tuned** (the pattern `art_growth_clip.rs` uses for its 0.188).
///
/// The fastest-moving material in `move` is a walking leg. The gait is 1.2 s with
/// `swing = 1.3 · sin(2π u)`, so `|d swing/dt| ≤ 1.3 · 2π / 1.2 = 6.81 px/s`; the production
/// leg lift is the continuous bump `sin(π u / 0.38)` over a 1 px rise, so
/// `|d lift/dt| ≤ (π / 0.38) / 1.2 = 6.89 px/s`. One frame at 60 fps is therefore at most
/// `0.1135 px` of swing and `0.1148 px` of lift, and a leg texel moves by at most
/// `δ = 0.1135 + 0.1148 = 0.228 px` measured as `|Δx| + |Δy|`. The hull's own wave (±1.15 px on
/// 2.4 s) is far slower: `1.15 · 2π / 2.4 = 3.01 px/s`, i.e. `0.050 px` per frame.
///
/// A premultiplied channel lies in `[0, 1]`, and a bilinear reconstruction's partial derivatives
/// are bounded by the largest adjacent-texel difference, itself at most 1. So one frame moves
/// any destination pixel of a given material by at most `δ`, and the **mean** over the pixels
/// the body paints is bounded by the same `δ`:
///
/// ```text
/// bound = 0.1135 + 0.1148 = 0.228  ->  0.25 with a little headroom
/// ```
///
/// The study's stepped motion could not satisfy it: a cell that jumps a whole pixel changes both
/// the pixel it left and the pixel it entered by its full colour, which over the body's 70–89
/// painted pixels puts the mean of order 1 on every step frame — four times this bound. The
/// companion assertion is that the mean is not zero: a still image would pass any bound.
#[test]
fn move_never_jumps_more_than_the_wave_and_the_gait_allow_in_one_frame() {
    const BOUND: f64 = 0.25;
    let rig = Lanternjaw::new();
    let mut parts = Vec::new();
    let mut scratch = Vec::new();
    let mut previous: Option<Canvas> = None;
    let mut worst = 0.0f64;
    let mut worst_at = 0.0f64;
    let mut total = 0.0f64;
    let mut counted = 0usize;

    for seconds in frames() {
        let mut canvas = Canvas::new();
        rig.draw(&mut canvas, mid(), forward(), seconds, Mode::Move, 1.0, &mut parts, &mut scratch);
        for face in Face::ALL {
            if face == Face::Front {
                continue;
            }
            assert!(
                (0..64u8).all(|y| (0..64u8).all(|x| canvas.get(face, x, y) == [0.0; 3])),
                "at {seconds} the mid-face body reached {face:?}, so the Front-only scan below \
                 would miss part of it"
            );
        }
        if let Some(before) = &previous {
            let mut sum = 0.0f64;
            let mut painted = 0usize;
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let (a, b) = (before.get(Face::Front, x, y), canvas.get(Face::Front, x, y));
                    if a == [0.0; 3] && b == [0.0; 3] {
                        continue;
                    }
                    painted += 1;
                    sum += (0..3).map(|c| f64::from((a[c] - b[c]).abs())).fold(0.0, f64::max);
                }
            }
            assert!(painted > 30, "at {seconds} the body painted only {painted} pixels");
            let mean = sum / painted as f64;
            if mean > worst {
                worst = mean;
                worst_at = seconds;
            }
            total += mean;
            counted += 1;
        }
        previous = Some(canvas);
    }
    assert!(
        worst <= BOUND,
        "the mean per-painted-pixel change between two 60 fps frames of `move` reached {worst} at \
         {worst_at} s, past the {BOUND} the wave and the gait allow"
    );
    assert!(
        worst > 1e-4,
        "the body never changed between frames ({worst}); `move` must actually move"
    );
    eprintln!(
        "move at {FPS} fps: mean |Δ| per painted pixel averages {:.5}, worst {worst:.5} at \
         {worst_at:.3} s (bound {BOUND})",
        total / counted as f64
    );
}

