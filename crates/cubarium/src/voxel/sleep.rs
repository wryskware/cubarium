//! **The sleep pose** (WX4): what `State::Asleep` looks like on the voxel grid.
//!
//! An asleep animal **settles** onto the ground by up to [`MAX_DROP_ROWS`] rows, its legs
//! folding under it, and its back **breathes**: the top of the middle of the back falls
//! [`BREATH_AMP_VOXELS`] below where it stood and rises back, on a [`BREATH_PERIOD_S`]
//! cycle. (Falls, not rises: a breath never reaches past the cell's own voxel, so the walk
//! needs no look above the back.) Falling asleep and waking move a
//! per-animal sleep **weight** from 0 to 1 (or back) over [`BLEND_S`].
//!
//! # Where the motion happens
//!
//! Nothing here moves a cell. The body is stamped where it stands awake, and every cell of
//! a posing body carries a [`Class`] (how it moves) and the index of its animal's pose
//! entry (`cubarium_gpu::voxel::PoseEntry`, how far it has moved). The GPU walk
//! (`voxel.frag`, `posedFind`) draws each such cell displaced by a fraction of a voxel, recomputed from the frame's clock, so the
//! settle and the breath are continuous and never add or remove a whole cell:
//!
//! * a **torso** cell (the fullest row and everything above it) comes down by the settle
//!   `D = drop · smooth(w)`;
//! * a **breathing** torso cell (the top of a column of the middle of the back) also has
//!   its top brought down by the breath's dip `B`, its rim row kept on top;
//! * a **leg** cell `d` rows under the torso stays where it is, its top cut off where the
//!   settling torso's bottom has reached (`torso − D`): the legs fold by the same offset.
//!   The empty voxels under the torso (the belly gap) are stamped as faceless leg cells
//!   (glyph 0), so the walk looks for the settling torso from there too.
//!
//! [`span`] is that rule on the CPU, the shader's `posedSpan` port of it, and what the tests
//! read. The pose works in the body's **own frame** (`a` along the body, `c` across it,
//! `row` up), so one classification serves the glyph path and the baked models.

use std::collections::BTreeMap;

use cubarium_gpu::voxel::{Breath, PoseTable};

/// The most rows a body comes down. As far as its own legs allow: a body whose torso is
/// already on the ground (a crawler) does not move.
pub const MAX_DROP_ROWS: i32 = 2;
/// Seconds to fall asleep or wake.
pub const BLEND_S: f64 = 1.0;
/// Seconds per breath.
pub const BREATH_PERIOD_S: f64 = 4.0;
/// How far the back falls at the bottom of a breath, in voxels.
pub const BREATH_AMP_VOXELS: f32 = 0.2;
/// Distinct breath phases: animals share pose entries when their phases match, so the
/// phase is quantised. Eight keeps a herd out of step (1 in 8 share a phase) and the
/// asleep entries to at most `3 · 8`.
pub const BREATH_PHASES: u64 = 8;
/// Pose entries a frame may use, index 0 unused ([`table`]); the GPU uniform's
/// `poseK` array and the texel's six entry bits.
pub const POSE_ENTRIES: usize = 64;
/// The part of the body's length, front to back, that breathes: the middle of the back.
const BREATH_BAND: (f32, f32) = (0.2, 0.8);

/// How one cell of a posing body moves. Packed in the texel's top two bits of `b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Comes down by the settle.
    Torso = 0,
    /// Comes down by the settle and its top rises with the breath.
    Breathing = 1,
    /// One row under the torso: stays, its top cut at the settling torso's bottom.
    Leg1 = 2,
    /// Two rows under the torso.
    Leg2 = 3,
}

impl Class {
    /// The texel byte: class in the top two bits, the pose entry (1..64) below.
    pub fn byte(self, entry: u8) -> u8 {
        debug_assert!(entry != 0 && usize::from(entry) < POSE_ENTRIES);
        (self as u8) << 6 | (entry & 63)
    }
}

/// Whether the pose byte of a **faceless** cell (an empty voxel a posing body moves into) is
/// a soft mark: class torso, the voxels under an idle body's dipping head
/// ([`super::idle`]). A soft mark gives way to a plant or a vine in its voxel (the head is
/// then hidden there) and is air to the walk's creases and shadows (`voxel.frag`,
/// `softMark`). The sleep's belly cells are class leg and settle over what is there.
pub fn is_soft_mark(byte: u8) -> bool {
    byte != 0 && byte >> 6 < Class::Leg1 as u8
}

/// One cell of a body in its own frame. `bump` says a breath may raise it (the back, not a
/// head or a sense patch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Local {
    pub a: i32,
    pub c: i32,
    pub row: i32,
    pub bump: bool,
}

/// A body's classification: its torso row, how many rows it settles, and each cell's
/// class (`None` for a cell that never moves: a leg further under the torso than it comes
/// down).
#[derive(Clone, Debug, PartialEq)]
pub struct Classified {
    pub torso: i32,
    pub drop: i32,
    pub class: Vec<Option<Class>>,
}

impl Classified {
    /// The class of an empty voxel `d` rows under the torso row: a faceless leg.
    pub fn under(d: i32) -> Option<Class> {
        match d {
            1 => Some(Class::Leg1),
            2 => Some(Class::Leg2),
            _ => None,
        }
    }
}

/// Classify a body. The torso is the fullest row, the lowest of a tie; the rows under it
/// are legs.
pub fn classify(cells: &[Local]) -> Classified {
    if cells.is_empty() {
        return Classified {
            torso: 0,
            drop: 0,
            class: Vec::new(),
        };
    }
    let mut per_row: BTreeMap<i32, usize> = BTreeMap::new();
    for l in cells {
        *per_row.entry(l.row).or_insert(0) += 1;
    }
    let most = per_row.values().copied().max().unwrap_or(0);
    let torso = per_row
        .iter()
        .find(|(_, n)| **n == most)
        .map(|(r, _)| *r)
        .unwrap_or(0);
    let lowest = *per_row.keys().next().expect("a cell");
    let drop = (torso - lowest).clamp(0, MAX_DROP_ROWS);
    let (lo, hi) = cells
        .iter()
        .fold((i32::MAX, i32::MIN), |(lo, hi), l| (lo.min(l.a), hi.max(l.a)));
    let span = (hi - lo + 1) as f32;
    let mut top: BTreeMap<(i32, i32), i32> = BTreeMap::new();
    for l in cells {
        let t = top.entry((l.a, l.c)).or_insert(l.row);
        *t = (*t).max(l.row);
    }
    let class = cells
        .iter()
        .map(|l| {
            if l.row < torso {
                return match torso - l.row {
                    1 if drop >= 1 => Some(Class::Leg1),
                    2 if drop >= 2 => Some(Class::Leg2),
                    _ => None,
                };
            }
            let f = ((l.a - lo) as f32 + 0.5) / span;
            let breathes = l.bump
                && l.row == top[&(l.a, l.c)]
                && f >= BREATH_BAND.0
                && f <= BREATH_BAND.1;
            Some(if breathes {
                Class::Breathing
            } else {
                Class::Torso
            })
        })
        .collect();
    Classified { torso, drop, class }
}

/// The weight's change per tick of `dt_s` seconds while falling asleep.
pub fn rate_per_tick(dt_s: f64) -> f32 {
    (dt_s / BLEND_S) as f32
}

/// The sleep breath at the staged `tick`: its phase then (0..1, so the clock's wrap never
/// shows), its period in ticks, its dip in voxels.
pub fn breath(tick: u64, dt_s: f64) -> Breath {
    let period = BREATH_PERIOD_S / dt_s;
    Breath {
        base: (tick as f64 / period).fract() as f32,
        period: period as f32,
        amp: BREATH_AMP_VOXELS,
    }
}

/// An empty pose table for a pack staged at `tick`: the tick wrapped as the frame clock
/// wraps it, and the sleep and idle breaths then.
pub fn table(tick: u64, tick_wrap: u64, dt_s: f64) -> PoseTable {
    PoseTable {
        tick: (tick % tick_wrap) as f32,
        sleep: breath(tick, dt_s),
        idle: super::idle::breath(tick, dt_s),
        ..PoseTable::EMPTY
    }
}

/// The breath phase of animal `id`, one of [`BREATH_PHASES`], out of step between animals.
pub fn phase(id: u64) -> f32 {
    (splitmix(id ^ 0x5EED_B8EA_7400_0007) % BREATH_PHASES) as f32 / BREATH_PHASES as f32
}

/// The blend one step on: toward asleep or awake at `1 / BLEND_S` per second. Never more
/// than `dt_s / BLEND_S` from where it was, so it cannot jump.
pub fn advance_blend(blend: f32, asleep: bool, dt_s: f64) -> f32 {
    let step = (dt_s / BLEND_S) as f32;
    let target = if asleep { 1.0 } else { 0.0 };
    if blend < target {
        (blend + step).min(target)
    } else {
        (blend - step).max(target)
    }
}

/// The settle's ease: a smoothstep of the weight, so the body starts and stops gently.
pub fn smooth(w: f32) -> f32 {
    let w = w.clamp(0.0, 1.0);
    w * w * (3.0 - 2.0 * w)
}

/// Where a posing cell at row `sy` is drawn, as `(bottom, top)` pixel rows over the
/// bottom of row 0 at `s` pixels a voxel, under a settle `d` and a breath's dip `b`
/// (voxels). The shader's `posedSpan`. A leg's top may fall to its bottom: it is folded
/// away. Nothing reaches above its own voxel's top.
pub fn span(class: Class, sy: i32, d: f32, b: f32, s: i32) -> (i32, i32) {
    let dp = (d * s as f32 + 0.5).floor() as i32;
    match class {
        Class::Torso | Class::Breathing => {
            let bp = if class == Class::Breathing {
                (b * s as f32 + 0.5).floor() as i32
            } else {
                0
            };
            (sy * s - dp, (sy + 1) * s - dp - bp)
        }
        Class::Leg1 | Class::Leg2 => {
            let dist = if class == Class::Leg1 { 1 } else { 2 };
            (sy * s, ((sy + 1) * s).min((sy + dist) * s - dp).max(sy * s))
        }
    }
}

pub(crate) fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A browser-like body: two legs two rows under a 6 x 3 torso.
    fn body() -> Vec<Local> {
        let mut v = Vec::new();
        for a in 0..6 {
            for c in 0..3 {
                v.push(Local {
                    a,
                    c,
                    row: 2,
                    bump: a != 5,
                });
                if a == 0 || a == 5 {
                    for row in 0..2 {
                        v.push(Local {
                            a,
                            c,
                            row,
                            bump: false,
                        });
                    }
                }
            }
        }
        v
    }

    #[test]
    fn a_standing_body_settles_two_rows_on_legs_that_fold() {
        let b = body();
        let k = classify(&b);
        assert_eq!(k.drop, 2);
        for (l, c) in b.iter().zip(&k.class) {
            match l.row {
                0 => assert_eq!(*c, Some(Class::Leg2)),
                1 => assert_eq!(*c, Some(Class::Leg1)),
                _ => assert!(matches!(c, Some(Class::Torso | Class::Breathing))),
            }
        }
        // Fully settled: the torso sits on the ground row and the legs are folded away.
        let s = 10;
        assert_eq!(span(Class::Torso, 2, 2.0, 0.0, s), (0, s));
        assert_eq!(span(Class::Leg1, 1, 2.0, 0.0, s).1, s);
        let (lo, hi) = span(Class::Leg1, 1, 2.0, 0.0, s);
        assert_eq!(lo, hi, "a folded leg has no height left");
        assert_eq!(span(Class::Leg2, 0, 2.0, 0.0, s), (0, 0));
        // Halfway: the torso's bottom and the top leg's top meet.
        assert_eq!(span(Class::Torso, 2, 0.5, 0.0, s).0, span(Class::Leg1, 1, 0.5, 0.0, s).1);
    }

    #[test]
    fn only_the_middle_of_the_back_breathes() {
        let b = body();
        let k = classify(&b);
        for (l, c) in b.iter().zip(&k.class) {
            if *c == Some(Class::Breathing) {
                assert!(l.row == 2 && (1..=4).contains(&l.a));
            }
        }
        assert!(k.class.contains(&Some(Class::Breathing)));
        // A crawler already on the ground settles nowhere but still breathes.
        let crawler: Vec<Local> = (0..5)
            .map(|a| Local {
                a,
                c: 0,
                row: 0,
                bump: true,
            })
            .collect();
        let k = classify(&crawler);
        assert_eq!(k.drop, 0);
        assert!(k.class.contains(&Some(Class::Breathing)));
    }

    use cubarium_gpu::voxel::{PoseEntry, pose_at};

    fn asleep(weight: f32, rate: f32, drop: f32, phase: f32) -> PoseEntry {
        PoseEntry {
            sleep: weight,
            sleep_rate: rate,
            drop,
            sleep_phase: phase,
            ..PoseEntry::ZERO
        }
    }

    fn settle_and_breath(e: PoseEntry, tick: u64, ticks: f32) -> (f32, f32) {
        let p = pose_at(&table(tick, 1 << 20, 0.05), &e, ticks);
        (p.settle, p.breath)
    }

    /// Weight 0 is no offset at all: every class draws exactly its own voxel.
    #[test]
    fn an_awake_weight_is_no_offset() {
        let e = asleep(0.0, 0.0, 2.0, 0.5);
        for f in [0.0, 0.3, 0.9] {
            let (d, b) = settle_and_breath(e, 1234, f);
            assert_eq!((d, b), (0.0, 0.0));
            for class in [Class::Torso, Class::Breathing, Class::Leg1, Class::Leg2] {
                assert_eq!(span(class, 3, d, b, 7), (21, 28));
            }
        }
    }

    #[test]
    fn the_breath_has_a_four_second_period_and_dips_a_fifth_of_a_voxel() {
        let e = asleep(1.0, 0.0, 0.0, 0.25);
        let at = |ticks: f32| settle_and_breath(e, 40, ticks).1;
        assert!((at(0.0) - settle_and_breath(e, 40 + 80, 0.0).1).abs() < 1e-5);
        // Over one period, a staged tick at a time (a frame only extrapolates a few ticks).
        let peak = (40..120u64)
            .map(|t| settle_and_breath(e, t, 0.0).1)
            .fold(0.0f32, f32::max);
        assert!(peak <= BREATH_AMP_VOXELS + 1e-6 && peak > 0.9 * BREATH_AMP_VOXELS);
        // A breathing cell never reaches above its own voxel.
        let (_, top) = span(Class::Breathing, 2, 0.0, at(0.0), 10);
        assert!(top <= 30);
        // Across the staged tick the table carries the phase on: tick 41's frame 0 is tick
        // 40's frame at +1.
        assert!((settle_and_breath(e, 41, 0.0).1 - at(1.0)).abs() < 1e-5);
    }

    #[test]
    fn two_animals_breathe_out_of_step() {
        let phases: std::collections::BTreeSet<u32> =
            (0..32u64).map(|id| (phase(id) * 8.0) as u32).collect();
        assert!(phases.len() > 4, "ids spread over the phases");
        assert_ne!(phase(1), phase(2));
    }

    #[test]
    fn the_blend_moves_at_one_over_the_blend_time_and_never_jumps() {
        let mut b = 0.0;
        for _ in 0..10 {
            let next = advance_blend(b, true, 0.05);
            assert!(next - b <= (0.05 / BLEND_S) as f32 + 1e-6);
            b = next;
        }
        assert!((b - 0.5).abs() < 1e-5);
        for _ in 0..40 {
            b = advance_blend(b, true, 0.05);
        }
        assert_eq!(b, 1.0);
        for _ in 0..40 {
            b = advance_blend(b, false, 0.05);
        }
        assert_eq!(b, 0.0);
    }
}
