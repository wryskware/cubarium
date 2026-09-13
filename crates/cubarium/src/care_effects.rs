//! Small, receipt-driven acknowledgements in the world image, not new resources.
//!
//! The runner observes only durably applied commands. Feed crumbs enter, settle and
//! disappear; cleanup flecks lift away. The real material remains solely in core fields.
//! Rain already has a physical renderer, so a *scheduled* shower paints nothing here.
//! No world reference, RNG, wall clock or transport is used. A single root-owned surface
//! query keeps each event coherent at seams and clips, rather than reflects, at the rim.

use std::collections::VecDeque;

use cubarium_core::DT;
use cubarium_core::care::{
    CLEAN_MATERIAL, CareApplied, CareCommand, CareKind, CareReceipt, FEED_MATERIAL,
};
use cubarium_render::{Canvas, srgb_decode};
use cubarium_surface::{CELL_COUNT, Face, PixelImage, SurfacePoint, Vec2, unfold_pixels};

use crate::art_present::present_seconds;

const MAX_EVENTS: usize = 8;
// Every moving center stays within 6 px; each separable kernel has radius <1.21 px.
const QUERY_RADIUS: f64 = 8.0;
const KERNEL_RADIUS: f64 = 0.85;
const CRUMBS: [Vec2; 6] = [
    Vec2::new(-2.8, 0.4),
    Vec2::new(-1.5, -0.8),
    Vec2::new(-0.2, 0.65),
    Vec2::new(1.15, -0.45),
    Vec2::new(2.65, 0.6),
    Vec2::new(0.2, -1.2),
];

#[derive(Clone, Copy, Debug)]
struct Effect {
    target: SurfacePoint,
    boundary: u64,
    kind: CareKind,
    strength: f64,
}

impl Effect {
    fn duration(self) -> f64 {
        match self.kind {
            CareKind::Feed => 2.8,
            CareKind::Clean => 1.8,
            CareKind::Rain => 0.0,
        }
    }

    fn crumb(self, age: f64, i: usize) -> (Vec2, f64) {
        let delay = i as f64 * 0.055;
        let t = age - delay;
        let fade = ease((self.duration() - age) / 0.9);
        let attack = ease(t / 0.18);
        match self.kind {
            CareKind::Feed => {
                let fall = ease(t / 0.9);
                let offset = Vec2::new((-0.5 + i as f64 * 0.2) * (1.0 - fall), -3.1 * (1.0 - fall));
                (CRUMBS[i] + offset, 0.8 * self.strength * attack * fade)
            }
            CareKind::Clean => {
                let lift = ease(t / 1.4);
                let offset = Vec2::new(0.0, -2.2 * lift);
                (
                    CRUMBS[i] * (1.0 + 0.4 * lift) + offset,
                    0.55 * self.strength * attack * fade,
                )
            }
            CareKind::Rain => (Vec2::ZERO, 0.0),
        }
    }
}

/// Bounded presentation history. Duplicate/stale sequence numbers cannot replay a flourish.
/// Draw never consumes events: common instants are identical at any draw frequency, even
/// when a diagnostic samples an earlier instant. New receipts evict the oldest at the cap.
#[derive(Default)]
pub(super) struct CareEffects {
    events: VecDeque<Effect>,
    high_water: u64,
    latest_boundary: u64,
    scratch: Vec<PixelImage>,
}

impl CareEffects {
    /// Observe one authoritative receipt after application, for fresh and replayed input.
    /// Identity/boundary/target mismatch is inert. Matched rejected/zero receipts consume
    /// their sequence without adding pixels; malformed quantities are never visualized.
    pub(super) fn observe(&mut self, command: &CareCommand, receipt: &CareReceipt) {
        if command.seq == 0
            || command.seq <= self.high_water
            || command.seq != receipt.seq
            || command.apply_after_tick != receipt.tick
            || receipt.tick < self.latest_boundary
            || command.target.resolve().is_none()
        {
            return;
        }
        self.high_water = command.seq;
        self.latest_boundary = receipt.tick;
        let Some(applied) = receipt.outcome.applied() else {
            return;
        };
        if !valid_applied(command.kind, receipt.tick, applied) {
            return;
        }
        let strength = match command.kind {
            CareKind::Feed => applied.material_in / FEED_MATERIAL,
            CareKind::Clean => applied.material_out / CLEAN_MATERIAL,
            // A receipt schedules water; actual delivery already drives physical rain.
            CareKind::Rain => return,
        }
        .clamp(0.0, 1.0);
        if strength <= 0.0 {
            return;
        }
        if self.events.len() == MAX_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(Effect {
            target: SurfacePoint::new(
                Face::from_index(command.target.face).expect("target validated"),
                command.target.u,
                command.target.v,
            ),
            boundary: receipt.tick,
            kind: command.kind,
            strength,
        });
    }

    /// Composite over the presenter's linear canvas before its one encode. Boundary B
    /// starts at B*DT, so a durable hold at (tick=B,f=1) stays at the invisible onset.
    /// Tick zero and non-finite fractions follow the existing presenter clock exactly.
    pub(super) fn draw(&mut self, tick: u64, fraction: f64, canvas: &mut Canvas) {
        if self.events.is_empty() {
            return;
        }
        let seconds = present_seconds(tick, fraction);
        for event in &self.events {
            let age = seconds - event.boundary as f64 * DT;
            if age <= 0.0 || age >= event.duration() {
                continue;
            }
            let color = match event.kind {
                CareKind::Feed => [255, 175, 91],
                CareKind::Clean => [183, 117, 173],
                CareKind::Rain => continue,
            }
            .map(srgb_decode);
            let crumbs: [_; 6] = std::array::from_fn(|i| event.crumb(age, i));
            unfold_pixels(event.target, QUERY_RADIUS, &mut self.scratch);
            for pixel in &self.scratch {
                let local = pixel.local - event.target.chart();
                // One receipt layer, not additive sparkle: overlap cannot build a halo.
                let mut alpha = 0.0_f64;
                for (center, opacity) in crumbs {
                    let d = local - center;
                    let coverage = ease(1.0 - d.x.abs() / KERNEL_RADIUS)
                        * ease(1.0 - d.y.abs() / KERNEL_RADIUS);
                    alpha = alpha.max(coverage * opacity);
                }
                if alpha <= 0.0 {
                    continue;
                }
                let a = alpha as f32;
                let old = canvas.get(pixel.face, pixel.x, pixel.y);
                canvas.set(
                    pixel.face,
                    pixel.x,
                    pixel.y,
                    std::array::from_fn(|c| old[c] * (1.0 - a) + color[c] * a),
                );
            }
        }
    }
}

fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn valid_applied(kind: CareKind, boundary: u64, q: &CareApplied) -> bool {
    if [
        q.material_in,
        q.energy_in,
        q.water_depth,
        q.material_out,
        q.energy_out,
    ]
    .into_iter()
    .any(|v| !v.is_finite() || v < 0.0)
        || q.cells == 0
        || q.cells as usize > CELL_COUNT
    {
        return false;
    }
    match kind {
        CareKind::Feed => {
            q.water_depth == 0.0
                && q.material_out == 0.0
                && q.energy_out == 0.0
                && q.ends_tick.is_none()
        }
        CareKind::Clean => {
            q.water_depth == 0.0
                && q.material_in == 0.0
                && q.energy_in == 0.0
                && q.ends_tick.is_none()
        }
        CareKind::Rain => {
            q.material_in == 0.0
                && q.energy_in == 0.0
                && q.material_out == 0.0
                && q.energy_out == 0.0
                && q.ends_tick.is_some_and(|end| end > boundary)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::care::{CareOutcome, CareTarget};
    use cube_proto::Frame;

    fn pair(kind: CareKind, seq: u64, boundary: u64) -> (CareCommand, CareReceipt) {
        let command = CareCommand {
            seq,
            apply_after_tick: boundary,
            kind,
            target: CareTarget {
                face: 0,
                u: 32.0,
                v: 32.0,
            },
        };
        let mut q = CareApplied {
            cells: 5,
            ..CareApplied::default()
        };
        match kind {
            CareKind::Feed => {
                q.material_in = FEED_MATERIAL;
                q.energy_in = 1.0;
            }
            CareKind::Clean => {
                q.material_out = CLEAN_MATERIAL;
                q.energy_out = 1.0;
            }
            CareKind::Rain => {
                q.water_depth = 4.0;
                q.ends_tick = Some(boundary + 120);
            }
        }
        (
            command,
            CareReceipt {
                seq,
                tick: boundary,
                outcome: CareOutcome::Applied(q),
            },
        )
    }

    fn frame(effects: &mut CareEffects, tick: u64, fraction: f64) -> Frame {
        let mut canvas = Canvas::new();
        effects.draw(tick, fraction, &mut canvas);
        let mut frame = Frame::black();
        canvas.encode(&mut frame);
        frame
    }

    fn light(canvas: &Canvas) -> f64 {
        Face::ALL
            .into_iter()
            .flat_map(|face| (0..64).flat_map(move |y| (0..64).map(move |x| (face, x, y))))
            .map(|(face, x, y)| f64::from(canvas.get(face, x, y)[0]))
            .sum()
    }

    #[test]
    fn no_input_is_byte_identical_even_on_a_nonempty_canvas() {
        let mut canvas = Canvas::new();
        for face in Face::ALL {
            for y in 0..64 {
                for x in 0..64 {
                    canvas.set(face, x, y, [f32::from(x) / 64.0, 0.12, f32::from(y) / 64.0]);
                }
            }
        }
        let mut before = Frame::black();
        canvas.encode(&mut before);
        let mut effects = CareEffects::default();
        for (tick, f) in [(0, 1.0), (1, f64::NAN), (1000, 0.5)] {
            effects.draw(tick, f, &mut canvas);
        }
        let mut after = Frame::black();
        canvas.encode(&mut after);
        assert_eq!(before.as_bytes(), after.as_bytes());
    }

    #[test]
    fn rejects_mismatch_invalid_zero_rejected_and_duplicate_receipts() {
        let (command, receipt) = pair(CareKind::Feed, 1, 0);
        let mut effects = CareEffects::default();
        let mut wrong = receipt.clone();
        wrong.seq = 2;
        effects.observe(&command, &wrong);
        wrong = receipt.clone();
        wrong.tick = 1;
        effects.observe(&command, &wrong);
        for target in [
            CareTarget {
                face: 5,
                ..command.target
            },
            CareTarget {
                u: f64::NAN,
                ..command.target
            },
            CareTarget {
                v: 64.0,
                ..command.target
            },
        ] {
            effects.observe(&CareCommand { target, ..command }, &receipt);
        }
        assert_eq!(effects.high_water, 0);
        effects.observe(&command, &receipt);
        effects.observe(&command, &receipt);
        assert_eq!(effects.events.len(), 1);
        for (i, outcome) in [
            CareOutcome::Rejected("allowance exhausted".into()),
            CareOutcome::Applied(CareApplied {
                cells: 5,
                ..CareApplied::default()
            }),
            CareOutcome::Applied(CareApplied {
                material_in: f64::NAN,
                cells: 5,
                ..CareApplied::default()
            }),
            CareOutcome::Applied(CareApplied {
                material_in: -1.0,
                cells: 5,
                ..CareApplied::default()
            }),
            CareOutcome::Applied(CareApplied {
                material_in: 1.0,
                material_out: 1.0,
                cells: 5,
                ..CareApplied::default()
            }),
            CareOutcome::Applied(CareApplied {
                material_in: 1.0,
                cells: 0,
                ..CareApplied::default()
            }),
        ]
        .into_iter()
        .enumerate()
        {
            let seq = i as u64 + 2;
            effects.observe(
                &CareCommand { seq, ..command },
                &CareReceipt {
                    seq,
                    tick: 0,
                    outcome,
                },
            );
        }
        assert_eq!(effects.events.len(), 1);
        let (rain, scheduled) = pair(CareKind::Rain, 20, 0);
        effects.observe(&rain, &scheduled);
        assert_eq!(
            effects.events.len(),
            1,
            "scheduled water is not a fake rain overlay"
        );
    }

    #[test]
    fn history_and_scratch_are_bounded_and_expired_draw_is_inert() {
        let mut effects = CareEffects::default();
        for seq in 1..=1000 {
            let (command, receipt) = pair(CareKind::Feed, seq, seq);
            effects.observe(&command, &receipt);
        }
        assert_eq!(effects.events.len(), MAX_EVENTS);
        assert_eq!(effects.high_water, 1000);
        let (old, receipt) = pair(CareKind::Feed, 1, 1);
        effects.observe(&old, &receipt);
        assert_eq!(effects.events.len(), MAX_EVENTS);
        let mut canvas = Canvas::new();
        effects.draw(1001, 0.5, &mut canvas);
        assert!(effects.scratch.len() < 400);
        canvas.clear();
        effects.draw(2000, 0.5, &mut canvas);
        assert_eq!(light(&canvas), 0.0);
    }

    #[test]
    fn boundary_hold_onset_end_and_repeated_draw_are_continuous() {
        for kind in [CareKind::Feed, CareKind::Clean] {
            let (command, receipt) = pair(kind, 1, 20);
            let mut effects = CareEffects::default();
            effects.observe(&command, &receipt);
            assert!(
                frame(&mut effects, 20, 1.0)
                    .as_bytes()
                    .iter()
                    .all(|&b| b == 0)
            );
            assert!(
                frame(&mut effects, 21, 0.0)
                    .as_bytes()
                    .iter()
                    .all(|&b| b == 0)
            );
            let mut tiny = Canvas::new();
            effects.draw(21, 1e-4, &mut tiny);
            assert!(light(&tiny) < 1e-7);
            let moving = frame(&mut effects, 29, 0.2);
            assert!(moving.as_bytes().iter().any(|&b| b > 0));
            assert_eq!(moving.as_bytes(), frame(&mut effects, 29, 0.2).as_bytes());
            assert_ne!(moving.as_bytes(), frame(&mut effects, 29, 0.8).as_bytes());
            let last = 20 + (effects.events[0].duration() / DT).round() as u64;
            let mut almost = Canvas::new();
            effects.draw(last, 1.0 - 1e-4, &mut almost);
            assert!(light(&almost) < 1e-7);
            assert!(
                frame(&mut effects, last, 1.0)
                    .as_bytes()
                    .iter()
                    .all(|&b| b == 0)
            );
        }
        let (command, receipt) = pair(CareKind::Feed, 1, 0);
        let mut effects = CareEffects::default();
        effects.observe(&command, &receipt);
        assert_eq!(
            frame(&mut effects, 0, 0.0).as_bytes(),
            frame(&mut effects, 0, 1.0).as_bytes()
        );
    }

    #[test]
    fn cleanup_strength_tracks_actual_removal_and_draw_rate_does_not_advance_it() {
        let (command, full) = pair(CareKind::Clean, 1, 0);
        let mut half = full.clone();
        let mut q = *half.outcome.applied().unwrap();
        q.material_out *= 0.5;
        q.energy_out *= 0.5;
        half.outcome = CareOutcome::Partial(q);
        let mut a = CareEffects::default();
        a.observe(&command, &full);
        let mut b = CareEffects::default();
        b.observe(&command, &half);
        let mut ca = Canvas::new();
        a.draw(11, 0.5, &mut ca);
        let mut cb = Canvas::new();
        b.draw(11, 0.5, &mut cb);
        assert!((light(&cb) / light(&ca) - 0.5).abs() < 1e-6);
        let expected = frame(&mut a, 11, 0.5);
        for rate in [30, 60, 120] {
            for i in 0..rate {
                let time = i as f64 / rate as f64;
                let ticks = time / DT;
                let _ = frame(&mut a, ticks.floor() as u64 + 1, ticks.fract());
            }
            assert_eq!(expected.as_bytes(), frame(&mut a, 11, 0.5).as_bytes());
        }
    }

    #[test]
    fn finite_local_coverage_on_all_faces_seams_vertices_and_rims() {
        for face in Face::ALL {
            for (u, v) in [
                (32.0, 32.0),
                (0.1, 32.0),
                (63.9, 32.0),
                (32.0, 0.1),
                (0.1, 0.1),
                (63.9, 0.1),
                (32.0, 63.9),
            ] {
                for kind in [CareKind::Feed, CareKind::Clean] {
                    let (mut command, receipt) = pair(kind, 1, 0);
                    command.target = CareTarget {
                        face: face.index() as u8,
                        u,
                        v,
                    };
                    let mut effects = CareEffects::default();
                    effects.observe(&command, &receipt);
                    let mut canvas = Canvas::new();
                    effects.draw(15, 0.5, &mut canvas);
                    assert!(light(&canvas) > 0.0, "{face:?} {u},{v} {kind:?}");
                    for f in Face::ALL {
                        for y in 0..64 {
                            for x in 0..64 {
                                assert!(
                                    canvas
                                        .get(f, x, y)
                                        .iter()
                                        .all(|c| c.is_finite() && (0.0..=1.0).contains(c))
                                );
                            }
                        }
                    }
                    for i in 0..6 {
                        for k in 0..200 {
                            let (center, _) = effects.events[0].crumb(k as f64 * 0.02, i);
                            assert!(
                                center.length() + KERNEL_RADIUS * 2.0_f64.sqrt() < QUERY_RADIUS
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rim_is_a_crop_not_a_reflection_and_side_seam_is_registered() {
        let (mut command, receipt) = pair(CareKind::Feed, 1, 0);
        let mut flat = CareEffects::default();
        flat.observe(&command, &receipt);
        let mut reference = Canvas::new();
        flat.draw(23, 0.5, &mut reference);
        command.target.v = 63.0;
        let mut rim = CareEffects::default();
        rim.observe(&command, &receipt);
        let mut cropped = Canvas::new();
        rim.draw(23, 0.5, &mut cropped);
        for y in 31..64 {
            for x in 0..64 {
                assert_eq!(
                    cropped.get(Face::Front, x, y),
                    reference.get(Face::Front, x, y - 31)
                );
            }
        }
        command.target.v = 32.0;
        command.target.u = 63.0;
        let mut seam = CareEffects::default();
        seam.observe(&command, &receipt);
        let mut across = Canvas::new();
        seam.draw(23, 0.5, &mut across);
        for y in 0..64 {
            for x in 25..40 {
                let shifted = x + 31;
                let (face, sx) = if shifted < 64 {
                    (Face::Front, shifted)
                } else {
                    (Face::Right, shifted - 64)
                };
                assert_eq!(across.get(face, sx, y), reference.get(Face::Front, x, y));
            }
        }
    }

    #[test]
    fn observing_and_drawing_real_receipts_never_changes_ecology() {
        let mut world = cubarium_core::World::new(cubarium_core::WorldConfig::default()).unwrap();
        let (command, _) = pair(CareKind::Feed, 1, 0);
        let receipt = world.apply_care(&command);
        assert!(receipt.outcome.applied().is_some());
        let before = cubarium_core::ecology_hash(&world.state);
        let mut effects = CareEffects::default();
        effects.observe(&command, &receipt);
        for tick in 0..70 {
            let _ = frame(&mut effects, tick, 0.5);
        }
        assert_eq!(before, cubarium_core::ecology_hash(&world.state));
    }
}
