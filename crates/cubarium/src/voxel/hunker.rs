//! **The hunker pose** (HUNKER, Wrysk 2026-10-06): what a frondgrazer clamped down under
//! its shield looks like on the voxel grid, and how it gets there.
//!
//! The pose is authored in Blender on the frondgrazer's one rig
//! (`scripts/blender/animal_models.py`, `FG_POSES`) and baked in the founder's own size
//! bins as the specimen `frondgrazer-hunker` ([`model::hunker_model`]): the shield's rim on
//! the ground line all round, the neck and scoop folded into the front notch, the eye
//! stalks in.
//!
//! # How it moves, and why the standing model is what is drawn
//!
//! The live pose system cannot swap one model for another without a pop: a cell can only
//! be drawn lowered by a fraction of a voxel or cut from its top, recomputed on every
//! frame's clock ([`super::sleep`]); none appears and none moves sideways. So the hunker is
//! drawn as the **standing model posed toward the baked hunker**, at a per-animal hunker
//! weight that goes 0 → 1 over the transition time (`VoxelConfig::hunker_s`) while
//! [`Animal::hunkered`](cubarium_voxel_fauna::Animal::hunkered) holds, and back after:
//!
//! * the shield (the torso row and every cell over it, [`sleep::classify`]) comes down by
//!   the body's settle (its rows over the ground, as the sleep's), the legs under it cut off
//!   at its bottom, so the rim reaches the ground;
//! * a cell over the torso whose lowered place is **not** in the baked hunker (the neck,
//!   the scoop and the eye stalks standing out of the front, a shoulder the baked shield
//!   no longer reaches) is cut from its top as the shield comes down
//!   ([`sleep::Class::Leg1`]), so it folds away by the time the shield is down.
//!
//! At full weight the drawn cells are the standing model's own, lowered, inside the baked
//! hunker's footprint; they keep their own colours, as the rigid shield carries its own.
//! Coming up is the same path backwards. A body seen for the first time hunkered is drawn
//! down, as an asleep one is drawn asleep.

use rustc_hash::FxHashSet;

use super::model::ModelCell;
use super::sleep::{self, Class, Classified, Local};

/// Seconds to go down or come up, by default (`VoxelConfig::hunker_s`).
pub const DEFAULT_S: f64 = 0.75;

/// The weight's change per tick of `dt_s` seconds over a transition of `hunker_s`.
pub fn rate_per_tick(dt_s: f64, hunker_s: f64) -> f32 {
    if hunker_s > 0.0 {
        (dt_s / hunker_s) as f32
    } else {
        1.0
    }
}

/// The weight one step on, toward down (1) or up (0), never more than a step from where it
/// was, so it cannot jump.
pub fn advance_blend(blend: f32, down: bool, dt_s: f64, hunker_s: f64) -> f32 {
    let step = if hunker_s > 0.0 {
        (dt_s / hunker_s) as f32
    } else {
        1.0
    };
    if down {
        (blend + step).min(1.0)
    } else {
        (blend - step).max(0.0)
    }
}

/// Classify a standing body toward its hunker: [`sleep::classify`]'s torso, settle and
/// legs, and every cell at or over the torso row whose place `drop` rows down is not a cell
/// of `target` (the baked hunker, offsets `[a, row, c]`) cut instead of carried.
pub fn classify(cells: &[Local], target: &[ModelCell]) -> Classified {
    let mut k = sleep::classify(cells);
    let down: FxHashSet<[i32; 3]> = target.iter().map(|m| m.offset).collect();
    for (l, class) in cells.iter().zip(k.class.iter_mut()) {
        if l.row >= k.torso && !down.contains(&[l.a, l.row - k.drop, l.c]) {
            *class = Some(Class::Leg1);
        }
    }
    k
}

/// Where a cell of `class` at row `sy` is drawn at hunker weight `w`, as `(bottom, top)`
/// pixel rows at `s` pixels a voxel (the shader's `posedSpan` at `pose_at`'s settle): the
/// CPU picture of the pose, for the tests.
pub fn span_at(class: Class, sy: i32, drop: i32, w: f32, s: i32) -> (i32, i32) {
    sleep::span(class, sy, drop as f32 * sleep::smooth(w), 0.0, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_weight_steps_toward_its_target_and_stops_there() {
        let dt = 0.05;
        let mut w = 0.0;
        let mut ups = 0;
        while w < 1.0 {
            let next = advance_blend(w, true, dt, 0.75);
            assert!(next > w && next - w <= (dt / 0.75) as f32 + 1e-6);
            w = next;
            ups += 1;
        }
        assert_eq!(ups, (0.75f64 / dt).ceil() as usize);
        assert_eq!(advance_blend(1.0, true, dt, 0.75), 1.0);
        assert!(advance_blend(1.0, false, dt, 0.75) < 1.0);
        assert_eq!(
            advance_blend(0.3, true, dt, 0.0),
            1.0,
            "no transition time: at once"
        );
    }
}
