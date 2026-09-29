//! **The idle pose** (WX4): what an awake animal standing still looks like.
//!
//! A body that is awake, not walking, not feeding and not climbing, and has not moved for
//! [`HOLD_S`], **idles**: a per-animal idle weight goes from 0 to 1 over [`BLEND_S`] (and
//! back when it walks off, falls asleep or feeds), and at that weight
//!
//! * its back **breathes** as a sleeper's does ([`super::sleep`]), [`BREATH_AMP_VOXELS`]
//!   deep on a [`BREATH_PERIOD_S`] cycle at its own phase;
//! * every 4–10 s its **head dips** once, out and back over [`MOVE_S`]: the head cells come
//!   down [`MOVE_VOXELS`] and rise again. Only a founder [`moves_head`] says so does this (a
//!   head turn waits for rigged models).
//!
//! The head is the facing end of the body: the glyph path's head cells, and a baked
//! model's front quarter at or over its torso row ([`head_of_model`]). The head move's
//! schedule is a pure hash of the animal's id and the tick ([`move_near`]), so a frame
//! drawn from either of two packs sees the same head, and nothing is kept for it.
//!
//! The motion is the sleep pose's: cells stamped where they stand, displaced on the GPU by
//! whole pixel rows recomputed from the frame's clock
//! (`cubarium_gpu::voxel::pose_rows`). An asleep body does not idle; a body that has just
//! woken idles once its sleep weight is back at 0.

use cubarium_gpu::voxel::{Breath, HeadMove};
use cubarium_voxel_fauna::Founder;

use super::sleep::{self, splitmix};

/// Seconds to start or stop idling.
pub const BLEND_S: f64 = 0.5;
/// Seconds a body stands still before it idles: two controller periods, so a feeding body
/// (which feeds on one tick of each) and a walker between steps never do.
pub const HOLD_S: f64 = 0.5;
/// Seconds per idle breath.
pub const BREATH_PERIOD_S: f64 = 2.5;
/// How far the back falls at the bottom of an idle breath, in voxels.
pub const BREATH_AMP_VOXELS: f32 = 0.1;
/// Seconds between one head move and the next: this, each move jittered by up to
/// [`MOVE_JITTER_S`] either way, so 4 to 10 s.
pub const MOVE_EVERY_S: f64 = 7.0;
pub const MOVE_JITTER_S: f64 = 1.5;
/// Seconds a head move lasts, out and back.
pub const MOVE_S: f64 = 0.5;
/// How far a head dips at its furthest, in voxels.
pub const MOVE_VOXELS: (f32, f32) = (0.3, 0.5);
/// The furthest past its staged tick a pack's pose is drawn (`cubarium_gpu::voxel::pose_at`
/// clamps there): a move due that soon is in the pack.
const LOOKAHEAD_TICKS: i64 = 8;

/// Whether a founder's idle body moves its head. The littershredder (blind, about three
/// voxels long) does not: it only breathes.
pub fn moves_head(founder: Founder) -> bool {
    match founder {
        Founder::Browser => true,
        // An ambusher waits motionless (interim; its body plan is open).
        Founder::Blind | Founder::Lanternjaw => false,
    }
}

/// One head move of one animal: a dip of `dip` voxels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub start_tick: i64,
    pub ticks: i64,
    pub dip: f32,
}

impl Move {
    /// The move as a pose entry carries it, relative to the staged `tick`.
    pub fn relative_to(self, tick: u64) -> HeadMove {
        HeadMove {
            start: (self.start_tick - tick as i64) as f32,
            ticks: self.ticks as f32,
            dip: self.dip,
        }
    }
}

fn ticks(s: f64, dt_s: f64) -> i64 {
    (s / dt_s).round() as i64
}

/// Head move `k` of animal `id`, at `dt_s` seconds a tick: one per [`MOVE_EVERY_S`] slot,
/// jittered by up to [`MOVE_JITTER_S`], so two moves are 4 to 10 s apart.
pub fn move_k(id: u64, k: i64, dt_s: f64) -> Move {
    let every = ticks(MOVE_EVERY_S, dt_s);
    let jitter = ticks(MOVE_JITTER_S, dt_s);
    let offset = (splitmix(id ^ 0x1D1E_0FF5_E700_0001) % every as u64) as i64;
    let h = splitmix(id ^ (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x1D1E_4EAD_0000_0002);
    let j = (h % (2 * jitter as u64 + 1)) as i64 - jitter;
    let dip = MOVE_VOXELS.0 + (MOVE_VOXELS.1 - MOVE_VOXELS.0) * ((h >> 20) & 0xFF) as f32 / 255.0;
    Move {
        start_tick: k * every + offset + j,
        ticks: ticks(MOVE_S, dt_s).max(1),
        dip,
    }
}

/// The slot of animal `id` that `tick` falls in.
fn slot(id: u64, tick: i64, dt_s: f64) -> i64 {
    let every = ticks(MOVE_EVERY_S, dt_s);
    let offset = (splitmix(id ^ 0x1D1E_0FF5_E700_0001) % every as u64) as i64;
    (tick - offset).div_euclid(every)
}

/// The head move of animal `id` that a pack staged at `tick` draws: one under way at the
/// tick, or due before the pack's frames run out. At most one, since moves are 4 s apart.
pub fn move_near(id: u64, tick: u64, dt_s: f64) -> Option<Move> {
    let t = tick as i64;
    let k0 = slot(id, t, dt_s);
    (k0 - 1..=k0 + 1)
        .map(|k| move_k(id, k, dt_s))
        .find(|m| m.start_tick + m.ticks > t && m.start_tick <= t + LOOKAHEAD_TICKS)
}

/// Every head move of animal `id` starting in `from..to` (ticks), in order.
pub fn moves_between(id: u64, from: u64, to: u64, dt_s: f64) -> Vec<Move> {
    let (a, b) = (from as i64, to as i64);
    (slot(id, a, dt_s) - 1..=slot(id, b, dt_s) + 1)
        .map(|k| move_k(id, k, dt_s))
        .filter(|m| m.start_tick >= a && m.start_tick < b)
        .collect()
}

/// The idle breath phase of animal `id`, one of [`sleep::BREATH_PHASES`]: out of step
/// between animals, and not the sleep breath's.
pub fn phase(id: u64) -> f32 {
    (splitmix(id ^ 0x1D1E_B8EA_7400_0003) % sleep::BREATH_PHASES) as f32
        / sleep::BREATH_PHASES as f32
}

/// The idle breath at the staged `tick`: its phase then, its period in ticks, its dip.
pub fn breath(tick: u64, dt_s: f64) -> Breath {
    let period = BREATH_PERIOD_S / dt_s;
    Breath {
        base: (tick as f64 / period).fract() as f32,
        period: period as f32,
        amp: BREATH_AMP_VOXELS,
    }
}

/// The weight's change per tick of `dt_s` seconds while starting to idle.
pub fn rate_per_tick(dt_s: f64) -> f32 {
    (dt_s / BLEND_S) as f32
}

/// The idle weight one step on, toward idling or not at `1 / BLEND_S` per second.
pub fn advance_blend(blend: f32, idle: bool, dt_s: f64) -> f32 {
    let step = (dt_s / BLEND_S) as f32;
    if idle {
        (blend + step).min(1.0)
    } else {
        (blend - step).max(0.0)
    }
}

/// Which cells of a baked model are its head: its facing end (the model faces `+a`), the
/// front quarter of its length (one slice at least) at or over its torso row.
pub fn head_of_model(local: &[sleep::Local], torso: i32) -> Vec<bool> {
    let (lo, hi) = local
        .iter()
        .fold((i32::MAX, i32::MIN), |(lo, hi), l| (lo.min(l.a), hi.max(l.a)));
    let slices = ((hi - lo + 1) / 4).max(1);
    local
        .iter()
        .map(|l| l.a > hi - slices && l.row >= torso)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_gpu::voxel::{PoseEntry, PoseTable, pose_at};

    const DT: f64 = 0.05;

    fn table(tick: u64) -> PoseTable {
        PoseTable {
            tick: tick as f32,
            sleep: Breath::ZERO,
            idle: breath(tick, DT),
            entries: [PoseEntry::ZERO; cubarium_gpu::voxel::POSE_ENTRIES],
        }
    }

    #[test]
    fn head_dips_come_every_four_to_ten_seconds() {
        for id in 0..16u64 {
            let moves = moves_between(id, 0, 20 * 600, DT);
            assert!(moves.len() >= 55, "about one in 7 s: {}", moves.len());
            for w in moves.windows(2) {
                let gap = (w[1].start_tick - w[0].start_tick) as f64 * DT;
                assert!((4.0..=10.0).contains(&gap), "gap {gap}");
            }
            assert!(moves.iter().all(|m| (MOVE_VOXELS.0..=MOVE_VOXELS.1).contains(&m.dip)));
        }
    }

    /// Every pack between a move's start and end carries it, and the frame it draws at a
    /// given time is the same from either of two packs.
    #[test]
    fn the_head_move_is_bounded_eased_and_returns_to_zero() {
        let id = 7;
        let m = moves_between(id, 400, 2000, DT)[0];
        let t0 = m.start_tick as u64;
        let at = |tick: u64, frac: f32| {
            let head = move_near(id, tick, DT).map_or(HeadMove::NONE, |m| m.relative_to(tick));
            let e = PoseEntry {
                idle: 1.0,
                head,
                ..PoseEntry::ZERO
            };
            pose_at(&table(tick), &e, frac).settle
        };
        let mut peak = 0.0f32;
        let mut last = 0.0f32;
        for tick in t0 - 5..t0 + m.ticks as u64 + 5 {
            for f in 0..3 {
                let v = at(tick, f as f32 / 3.0);
                assert!(v <= 0.5 + 1e-6, "bounded at half a voxel");
                assert!((v - last).abs() < 0.06, "eased: no frame jumps");
                // The next pack draws the same instant the same.
                if f == 0 && tick > t0 - 5 {
                    assert!((v - at(tick - 1, 1.0)).abs() < 1e-5);
                }
                peak = peak.max(v);
                last = v;
            }
        }
        assert!(peak >= MOVE_VOXELS.0 - 0.02, "it moves: {peak}");
        assert_eq!(at(t0, 0.0), 0.0, "it leaves from rest");
        assert_eq!(at(t0 + m.ticks as u64, 0.0), 0.0, "and comes back to it");
    }

    #[test]
    fn two_animals_idle_out_of_step() {
        assert_ne!(phase(1), phase(2));
        assert_ne!(move_k(1, 3, DT).start_tick, move_k(2, 3, DT).start_tick);
    }

    #[test]
    fn the_littershredder_moves_no_head_and_the_browser_does() {
        assert!(!moves_head(Founder::Blind));
        assert!(moves_head(Founder::Browser));
    }

    #[test]
    fn the_head_of_a_model_is_its_front_quarter_over_the_legs() {
        // An eight-long body facing +a, legs under a = -3 and 3.
        let mut local = Vec::new();
        for a in -3..=4 {
            local.push(sleep::Local { a, c: 0, row: 1, bump: true });
        }
        for a in [-3, 3] {
            local.push(sleep::Local { a, c: 0, row: 0, bump: false });
        }
        let head = head_of_model(&local, 1);
        let heads: Vec<(i32, i32)> = local
            .iter()
            .zip(&head)
            .filter(|(_, h)| **h)
            .map(|(l, _)| (l.a, l.row))
            .collect();
        assert_eq!(heads, vec![(3, 1), (4, 1)]);
    }
}
