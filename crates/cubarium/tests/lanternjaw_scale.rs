//! The core-admitted whole-rig scales of the Lanternjaw, drawn: the trial's `body_scale_min`
//! 0.2 (`SCALE_MIN`), the valid child of `child_structure_fraction = 0.1` (`sqrt(0.1)` =
//! 0.31622776601683794), the default child of fraction 0.4 (`sqrt(0.4)` = 0.6324555320336759)
//! and the adult. Written from `Lanternjaw::draw_living`, `effectors`, `SCALE_MIN`/`SCALE_MAX`,
//! the module footprint contract and `cubarium_render::stamp_rig_scaled`'s doc, after Astra's
//! `astra-hunter-geometry-review-2026-09-13.md` gate 2 ("include a below-0.5 valid juvenile and
//! the smallest admitted scale").
//!
//! For every admitted scale the body draws mid-face, across a side/side and a side/top seam, at
//! a top vertex and into the open rim; its painted footprint lies inside the scaled bound; the
//! total light rises with the scale and stays near the area law; the named effectors scale
//! linearly and the near claw lands on the drawn near limb; a seam conserves light at matched
//! lattice phase; the rim cuts and never reflects; and the scale gate admits exactly
//! `SCALE_MIN..=SCALE_MAX`.

use cubarium::lanternjaw::*;
use cubarium_render::{Canvas, SUPERSAMPLE_REACH, stamp_rig_scaled};
use cubarium_surface::{SurfacePoint, Vec2};
use cube_proto::Face;

/// The four core-admitted scales this suite sweeps.
const SCALES: [f64; 4] = [
    SCALE_MIN,
    0.31622776601683794,
    0.6324555320336759,
    SCALE_MAX,
];

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL
        .into_iter()
        .flat_map(|f| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (f, x, y))))
}

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

/// A strike held at settlement: the claws out, the pose every contact check is about.
fn settled(ambient: f64) -> LivingPose {
    LivingPose {
        ambient,
        movement: 0.0,
        attack: Some(AttackEpisode {
            phase: AttackPhase::Strike,
            elapsed: 1.5,
            duration: 1.0,
            from: Reach {
                near: -0.35,
                far: -0.35,
                compress: 1.7,
                lunge: 0.0,
                charge: 1.0,
            },
        }),
        gut: 0.0,
        cocoon: None,
    }
}

fn draw(pose: &LivingPose, anchor: SurfacePoint, heading: Vec2, scale: f64) -> Canvas {
    let mut canvas = Canvas::new();
    Lanternjaw::new().draw_living(
        &mut canvas,
        anchor,
        heading,
        pose,
        scale,
        1.0,
        &mut Vec::new(),
        &mut Vec::new(),
    );
    canvas
}

/// Only the named part, through the same scaled stamp the body uses.
fn draw_part(pose: &LivingPose, name: PartName, anchor: SurfacePoint, scale: f64) -> Canvas {
    let mut parts = Vec::new();
    Lanternjaw::new().parts_living(pose, &mut parts);
    let part = parts
        .iter()
        .find(|p| p.name == name)
        .expect("the part exists");
    let rig = [part.rig_part()];
    let mut canvas = Canvas::new();
    stamp_rig_scaled(
        &mut canvas,
        anchor,
        Vec2::new(1.0, 0.0),
        &[(&rig[..], 1.0)],
        scale,
        1.0,
        &mut Vec::new(),
    );
    canvas
}

fn mid() -> SurfacePoint {
    SurfacePoint::pixel_center(Face::Front, 32, 32)
}

#[test]
fn the_admitted_scales_are_the_cores_and_the_gate_admits_exactly_them() {
    assert_eq!(SCALE_MIN, 0.2, "the trial profile's body_scale_min");
    assert_eq!(SCALE_MAX, 1.0);
    for &s in &SCALES {
        assert!((SCALE_MIN..=SCALE_MAX).contains(&s));
        // Draws without panicking at every admitted scale.
        let image = draw(&settled(0.0), mid(), Vec2::new(1.0, 0.0), s);
        assert!(total_light(&image) > 0.0, "scale {s} drew nothing");
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let refused = |s: f64| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw(&settled(0.0), mid(), Vec2::new(1.0, 0.0), s);
        }))
        .is_err()
    };
    let below = refused(SCALE_MIN - 1e-9);
    let above = refused(SCALE_MAX + 1e-9);
    let nan = refused(f64::NAN);
    std::panic::set_hook(previous);
    assert!(below, "a scale just below SCALE_MIN must be refused");
    assert!(above, "a scale just above SCALE_MAX must be refused");
    assert!(nan, "a NaN scale must be refused");
}

/// The module contract: a destination pixel is painted only if `b / scale` is within one pixel
/// of a painted texel centre inside the footprint, so every painted pixel is inside
/// `scale · (BOUND + 1)` — and light follows the area within a generous band, monotone in the
/// scale (bilinear sampling of adult art at a fifth of its size aliases, so the band is wide).
#[test]
fn every_admitted_scale_stays_inside_its_footprint_and_light_follows_the_area() {
    let origin = mid().chart();
    let mut last = 0.0f64;
    let adult = total_light(&draw(&settled(2.0), mid(), Vec2::new(1.0, 0.0), SCALE_MAX));
    for &s in &SCALES {
        let image = draw(&settled(2.0), mid(), Vec2::new(1.0, 0.0), s);
        let (mut front, mut back, mut across) = (f64::NEG_INFINITY, f64::INFINITY, 0.0f64);
        for (f, x, y) in every_pixel() {
            if image.get(f, x, y) == [0.0; 3] {
                continue;
            }
            assert_eq!(f, Face::Front, "scale {s}: a mid-face body left its face");
            let b = Vec2::new(f64::from(x) + 0.5 - origin.x, f64::from(y) + 0.5 - origin.y);
            front = front.max(b.x);
            back = back.min(b.x);
            across = across.max(b.y.abs());
        }
        // Below scale 1 the stamp is box-filtered, so a painted pixel centre may sit
        // `SUPERSAMPLE_REACH` further out than the point-sampled bound.
        let reach = if s < 1.0 { SUPERSAMPLE_REACH } else { 0.0 };
        assert!(
            front <= s * (BOUND_FRONT + 1.0) + reach + 1e-9,
            "scale {s}: front {front}"
        );
        assert!(
            back >= -s * (BOUND_BACK + 1.0) - reach - 1e-9,
            "scale {s}: back {back}"
        );
        assert!(
            across <= s * (BOUND_ABOVE + 1.0) + reach + 1e-9,
            "scale {s}: across {across}"
        );
        // The claws are out at settlement at every scale: the front reaches past the jaw.
        assert!(
            front > s * 9.6,
            "scale {s}: the front {front} is not past the scaled jaw"
        );
        let light = total_light(&image);
        let ratio = light / adult;
        assert!(
            light > last,
            "scale {s}: light {light} did not rise from {last}"
        );
        assert!(
            ratio >= 0.35 * s * s && ratio <= 2.2 * s * s,
            "scale {s}: light ratio {ratio:.4} is far from the area law {:.4}",
            s * s
        );
        eprintln!("scale {s}: front {front:.3} back {back:.3} across {across:.3} ratio {ratio:.4}");
        last = light;
    }
}

/// `effectors(scale)` is linear in the scale to the last bit, and at every admitted scale the
/// named near claw lies on the drawn near limb: within one pixel of a painted pixel of the
/// `NearLimb` part alone, at the smallest scale within the limb's own scaled reach.
#[test]
fn the_named_effectors_are_linear_and_land_on_the_drawn_claw_at_every_admitted_scale() {
    let adult = effectors(SCALE_MAX);
    assert_eq!(
        adult.near_claw.x, 13.279411764705882,
        "the agreed exact near-claw x"
    );
    assert_eq!(adult.near_claw.y, 1.1, "the agreed exact near-claw y");
    assert_eq!(adult.mouth.x, 9.6);
    let origin = mid().chart();
    for &s in &SCALES {
        let e = effectors(s);
        for (name, got, want) in [
            ("mouth.x", e.mouth.x, s * adult.mouth.x),
            ("mouth.y", e.mouth.y, s * adult.mouth.y),
            ("near_claw.x", e.near_claw.x, s * adult.near_claw.x),
            ("near_claw.y", e.near_claw.y, s * adult.near_claw.y),
            ("far_claw.x", e.far_claw.x, s * adult.far_claw.x),
            ("far_claw.y", e.far_claw.y, s * adult.far_claw.y),
        ] {
            assert!(
                (got - want).abs() <= 1e-15 * (1.0 + want.abs()),
                "scale {s}: {name} is {got}, linear scaling gives {want}"
            );
        }
        // The claw is where the effector says, in chart pixels from the root: the pixel whose
        // area contains the named point carries the near limb's light at every admitted scale
        // (box-filtered minification gives a sub-pixel claw its share of the pixel's area
        // instead of hitting or missing it), and the nearest painted pixel centre is within
        // one pixel — never a tolerance that grows with the body.
        let image = draw_part(&settled(0.0), PartName::NearLimb, mid(), s);
        let mut nearest = f64::INFINITY;
        let mut at_claw = 0.0f32;
        for (f, x, y) in every_pixel() {
            let p = image.get(f, x, y);
            if p == [0.0; 3] {
                continue;
            }
            assert_eq!(f, Face::Front);
            let b = Vec2::new(f64::from(x) + 0.5 - origin.x, f64::from(y) + 0.5 - origin.y);
            nearest = nearest.min((b - e.near_claw).length());
            if (b.x - e.near_claw.x).abs() <= 0.5 && (b.y - e.near_claw.y).abs() <= 0.5 {
                at_claw = p.iter().copied().fold(0.0, f32::max);
            }
        }
        eprintln!(
            "scale {s}: nearest painted near-limb pixel {nearest:.3} px from the claw; the \
             pixel over the claw carries {at_claw:.4}"
        );
        assert!(
            nearest <= 1.0,
            "scale {s}: the nearest painted near-limb pixel is {nearest} px from the named claw \
             ({}, {})",
            e.near_claw.x,
            e.near_claw.y
        );
        assert!(
            at_claw > 0.0,
            "scale {s}: the pixel over the named claw carries no near-limb light"
        );
        // And the mouth is not the claw: the jaw sits behind the claw by the same scaled gap.
        assert!(
            e.near_claw.x - e.mouth.x > 3.0 * s,
            "scale {s}: claw and mouth are not distinct"
        );
    }
}

/// A scaled body across the Front/Right seam and the Right/Top seam carries the same light as
/// mid-face at a matched lattice phase (the seam only relabels pixels), and reaches two faces.
#[test]
fn every_admitted_scale_crosses_a_seam_as_one_body() {
    let pose = settled(1.0);
    for &s in &SCALES {
        let reference = total_light(&draw(
            &pose,
            SurfacePoint::new(Face::Front, 32.5, 32.5),
            Vec2::new(1.0, 0.0),
            s,
        ));
        for (anchor, heading) in [
            (
                SurfacePoint::new(Face::Front, 63.5, 32.5),
                Vec2::new(1.0, 0.0),
            ),
            (
                SurfacePoint::new(Face::Right, 32.5, 0.5),
                Vec2::new(0.0, -1.0),
            ),
        ] {
            let image = draw(&pose, anchor, heading, s);
            let light = total_light(&image);
            assert!(
                (light - reference).abs() <= 1e-6 * reference.max(1.0),
                "scale {s} at {anchor:?}: {light} vs mid-face {reference}"
            );
            assert!(
                lit_faces(&image) >= 2,
                "scale {s} at {anchor:?}: the body did not cross"
            );
        }
        // And at a top vertex every pixel has one owner: nothing brighter than one.
        let vertex = draw(
            &pose,
            SurfacePoint::new(Face::Top, 1.5, 1.5),
            Vec2::new(0.6, 0.8),
            s,
        );
        for (f, x, y) in every_pixel() {
            let p = vertex.get(f, x, y);
            assert!(
                p.iter().all(|&c| c <= 1.0 + 1e-6),
                "scale {s}: vertex pixel {p:?} over one"
            );
        }
    }
}

/// A scaled body heading into the open rim is cut where the surface ends and never reflected:
/// on the rows that exist it is the same body drawn thirty rows up, shifted.
#[test]
fn every_admitted_scale_is_cut_at_the_open_rim_and_never_reflected() {
    let pose = settled(0.5);
    let down = Vec2::new(0.0, 1.0);
    for &s in &SCALES {
        let rim = draw(&pose, SurfacePoint::new(Face::Front, 32.5, 62.5), down, s);
        let flat = draw(&pose, SurfacePoint::new(Face::Front, 32.5, 32.5), down, s);
        let mut compared = 0usize;
        for (f, x, y) in every_pixel() {
            let p = rim.get(f, x, y);
            if f != Face::Front {
                assert_eq!(
                    p, [0.0; 3],
                    "scale {s}: light on {f:?} — a reflection or a leak"
                );
                continue;
            }
            let want = if y >= 30 {
                flat.get(Face::Front, x, y - 30)
            } else {
                [0.0; 3]
            };
            assert_eq!(
                p, want,
                "scale {s}: pixel ({x}, {y}) differs from the flat body shifted"
            );
            if p != [0.0; 3] {
                compared += 1;
            }
        }
        assert!(compared > 0, "scale {s}: nothing survived above the rim");
        // The rim body carries strictly less light than the flat one: part of it is cut.
        assert!(
            total_light(&rim) < total_light(&flat),
            "scale {s}: nothing was cut at the rim"
        );
    }
}
