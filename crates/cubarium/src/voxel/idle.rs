//! **The idle pose** (WX4): what an awake animal standing still looks like.
//!
//! A body that is awake, not walking, not feeding and not climbing, and has not moved for
//! [`HOLD_S`], **idles**: a per-animal idle weight goes from 0 to 1 over [`BLEND_S`] (and
//! back when it walks off, falls asleep or feeds), and at that weight
//!
//! * its back **breathes** as a sleeper's does ([`super::sleep`]), [`BREATH_AMP_VOXELS`]
//!   deep on a [`BREATH_PERIOD_S`] cycle at its own phase: the still **base idle**;
//! * now and then it plays one **idle action** (RIG-STATES round 5): a short one-shot move
//!   that leaves the base idle and comes back to it. A rigged founder's actions are its
//!   rig's `idle.<name>` clips (`rig::ClipMode::Action`), several clips of one name being
//!   that action's variants (the side a loftstrider looks to, the frond it scratches with);
//!   a founder drawn on the class path that [`moves_head`] dips its head (the cells come
//!   down [`MOVE_VOXELS`] and rise again over [`MOVE_S`]).
//!
//! # The schedule
//!
//! Actions come one at a time, Sims- or WoW-style: the time from one action's start to the
//! next's is drawn from the founder's `apart_s` range, the action from its `weights`
//! ([`IdleActions`], `[figures.idle.<founder>]`), and never the same one twice in a row
//! when there are more than one ([`schedule`]). The schedule is a pure hash of the
//! animal's id and the slot the time falls in ([`near`], [`between`]): a frame drawn from
//! either of two packs sees the same action, and nothing is kept or saved for it. Which of
//! them play is the presenter's: an action plays only if the idle weight was full at its
//! start (`Animals::idle_full_at`), and one under way when the animal leaves idling eases
//! out with the idle weight.
//!
//! The head is the facing end of the body: the glyph path's head cells, and a baked
//! model's front quarter at or over its torso row ([`head_of_model`]).
//!
//! The class path's motion is the sleep pose's: cells stamped where they stand, displaced
//! on the GPU by whole pixel rows recomputed from the frame's clock
//! (`cubarium_gpu::voxel::pose_rows`). An asleep body does not idle; a body that has just
//! woken idles once its sleep weight is back at 0.

use std::collections::BTreeMap;

use cubarium_gpu::voxel::{Breath, HeadMove};
use cubarium_voxel_fauna::Founder;
use serde::{Deserialize, Serialize};

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
/// Seconds a head move lasts, out and back.
pub const MOVE_S: f64 = 0.5;
/// How far a head dips at its furthest, in voxels.
pub const MOVE_VOXELS: (f32, f32) = (0.3, 0.5);
/// The furthest past its staged tick a pack's pose is drawn (`cubarium_gpu::voxel::pose_at`
/// clamps there): an action due that soon is in the pack.
pub const LOOKAHEAD_TICKS: i64 = 8;

/// Whether a founder's idle body moves its head on the class path. The littershredder
/// (blind, about three voxels long) does not: it only breathes.
pub fn moves_head(founder: Founder) -> bool {
    // Only the browser; an ambusher waits motionless (interim; its body plan is open),
    // and a lineage without its own idle stays still.
    founder == Founder::Browser
}

/// **One founder's idle actions** (`[figures.idle.<founder>]`): how often they come and how
/// likely each is. Presentation only. Wrysk tunes these from the video (RIG-STATES round 5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IdleActions {
    /// Seconds from one idle action's start to the next's, `[shortest, longest]`: each gap
    /// is drawn evenly between them. A shortest under the founder's longest action is
    /// raised to it, so one action never starts before the last has ended.
    pub apart_s: [f64; 2],
    /// Each action's relative chance, by its name (a rig's `idle.<name>` clips; the class
    /// path's head move is `dip`). An action not named here weighs 1; 0 turns it off.
    /// With two actions on, they alternate (never the same one twice in a row).
    pub weights: BTreeMap<String, f64>,
}

impl Default for IdleActions {
    fn default() -> IdleActions {
        IdleActions { apart_s: [6.0, 14.0], weights: BTreeMap::new() }
    }
}

impl IdleActions {
    /// The weight of the action named `name`.
    pub fn weight(&self, name: &str) -> f64 {
        self.weights.get(name).copied().unwrap_or(1.0).max(0.0)
    }
}

/// `[figures.idle]`: each founder's idle actions ([`IdleActions`]); a founder not named
/// here takes [`IdleActions::default`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IdleConfig {
    /// The frondgrazer's single head dip (round 1's 21 s loop of three dips, one at a time),
    /// 4 to 10 s apart as the class path's always were.
    pub frondgrazer: IdleActions,
    /// The loftstrider's `look` (to one side, the whole body in it) and `scratch` (one
    /// frond scratches the body, then is cleaned at the mouth).
    pub loftstrider: IdleActions,
}

impl Default for IdleConfig {
    fn default() -> IdleConfig {
        IdleConfig {
            frondgrazer: IdleActions { apart_s: [4.0, 10.0], weights: BTreeMap::from([("dip".to_string(), 1.0)]) },
            loftstrider: IdleActions {
                apart_s: [8.0, 18.0],
                weights: BTreeMap::from([("look".to_string(), 1.0), ("scratch".to_string(), 1.0)]),
            },
        }
    }
}

impl IdleConfig {
    /// `founder`'s idle actions.
    pub fn of(&self, founder: Founder) -> IdleActions {
        match founder {
            Founder::Browser => self.frondgrazer.clone(),
            Founder::Loftstrider => self.loftstrider.clone(),
            _ => IdleActions::default(),
        }
    }
}

/// One kind of idle action in a founder's set: its weight (relative chance; 0 never), how
/// many variants it has (one is drawn evenly each time), and its length in seconds (its
/// longest variant's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kind {
    pub weight: f64,
    pub variants: usize,
    pub length_s: f64,
}

/// One scheduled idle action of one animal: its slot `k`, its start tick and length in
/// ticks, which kind (an index into the set) and variant, and its hash (for anything else
/// the action draws, like the class path's dip depth).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Action {
    pub k: i64,
    pub start_tick: i64,
    pub ticks: i64,
    pub kind: usize,
    pub variant: usize,
    pub hash: u64,
}

/// One head move of one animal (the class path's `dip` action): a dip of `dip` voxels.
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

/// A set's slots: their length in ticks and each start's jitter either way, so that two
/// starts are `apart_s` apart, never closer than the longest kind.
fn slots(apart_s: [f64; 2], kinds: &[Kind], dt_s: f64) -> (i64, i64) {
    let longest = kinds.iter().filter(|k| k.weight > 0.0).map(|k| k.length_s).fold(0.0, f64::max);
    let lo = ((apart_s[0].max(longest) / dt_s).ceil() as i64).max(1);
    let hi = ((apart_s[1] / dt_s).floor() as i64).max(lo);
    let d = hi - lo;
    (lo + d / 2, d / 4)
}

fn offset(id: u64, every: i64) -> i64 {
    (splitmix(id ^ 0x1D1E_0FF5_E700_0001) % every as u64) as i64
}

fn slot_hash(id: u64, k: i64) -> u64 {
    splitmix(id ^ (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x1D1E_4EAD_0000_0002)
}

/// A pick among `kinds` (those `allowed`) by their weights, from `h`.
fn weighted(kinds: &[Kind], h: u64, allowed: impl Fn(usize) -> bool) -> Option<usize> {
    let total: f64 = (0..kinds.len()).filter(|&i| allowed(i)).map(|i| kinds[i].weight).sum();
    if !(total > 0.0) {
        return None;
    }
    let mut u = (h >> 11) as f64 / (1u64 << 53) as f64 * total;
    let mut last = None;
    for i in (0..kinds.len()).filter(|&i| allowed(i) && kinds[i].weight > 0.0) {
        if u < kinds[i].weight {
            return Some(i);
        }
        u -= kinds[i].weight;
        last = Some(i);
    }
    last
}

/// The kind of slot `k` of animal `id`. With one kind on, always it. With more, an even
/// slot's is drawn by weight (with exactly two, the even slots all draw the same one) and an
/// odd slot's by weight among the kinds neither neighbouring even slot drew: so no kind
/// comes twice in a row, and every slot is still a pure hash of the id and `k`.
fn kind_of(id: u64, k: i64, kinds: &[Kind]) -> Option<usize> {
    let on = kinds.iter().filter(|k| k.weight > 0.0).count();
    let draw = |k: i64| splitmix(slot_hash(id, k) ^ 0x1D1E_C401_CE00_0004);
    let even = |k: i64| -> Option<usize> {
        let h = if on == 2 { splitmix(id ^ 0x1D1E_E7E0_0000_0005) } else { draw(k) };
        weighted(kinds, h, |_| true)
    };
    match on {
        0 => None,
        1 => weighted(kinds, 0, |_| true),
        _ if k.rem_euclid(2) == 0 => even(k),
        _ => {
            let (before, after) = (even(k - 1), even(k + 1));
            weighted(kinds, draw(k), |i| Some(i) != before && Some(i) != after)
        }
    }
}

/// Idle action `k` of animal `id` from the set `kinds` spaced by `apart_s`, at `dt_s`
/// seconds a tick; `None` when no kind is on.
pub fn schedule(id: u64, k: i64, dt_s: f64, apart_s: [f64; 2], kinds: &[Kind]) -> Option<Action> {
    let (every, jitter) = slots(apart_s, kinds, dt_s);
    let h = slot_hash(id, k);
    let j = (h % (2 * jitter as u64 + 1)) as i64 - jitter;
    let kind = kind_of(id, k, kinds)?;
    let variant = (splitmix(h ^ 0x1D1E_7A21_0000_0006) % kinds[kind].variants.max(1) as u64) as usize;
    Some(Action {
        k,
        start_tick: k * every + offset(id, every) + j,
        ticks: ticks(kinds[kind].length_s, dt_s).max(1),
        kind,
        variant,
        hash: h,
    })
}

/// The slot of animal `id` that `tick` falls in.
fn slot(id: u64, tick: i64, every: i64) -> i64 {
    (tick - offset(id, every)).div_euclid(every)
}

/// The idle actions of animal `id` a pack staged at `tick` draws: one under way at the
/// tick, and one due before the pack's frames run out ([`LOOKAHEAD_TICKS`]), in order.
pub fn near(id: u64, tick: u64, dt_s: f64, apart_s: [f64; 2], kinds: &[Kind]) -> Vec<Action> {
    let (every, _) = slots(apart_s, kinds, dt_s);
    let t = tick as i64;
    let k0 = slot(id, t, every);
    (k0 - 1..=k0 + 1)
        .filter_map(|k| schedule(id, k, dt_s, apart_s, kinds))
        .filter(|a| a.start_tick + a.ticks > t && a.start_tick <= t + LOOKAHEAD_TICKS)
        .collect()
}

/// Every idle action of animal `id` starting in `from..to` (ticks), in order.
pub fn between(id: u64, from: u64, to: u64, dt_s: f64, apart_s: [f64; 2], kinds: &[Kind]) -> Vec<Action> {
    let (every, _) = slots(apart_s, kinds, dt_s);
    let (a, b) = (from as i64, to as i64);
    (slot(id, a, every) - 1..=slot(id, b, every) + 1)
        .filter_map(|k| schedule(id, k, dt_s, apart_s, kinds))
        .filter(|m| m.start_tick >= a && m.start_tick < b)
        .collect()
}

/// The class path's set: one kind, the head's `dip`.
fn dips(actions: &IdleActions) -> [Kind; 1] {
    [Kind { weight: actions.weight("dip"), variants: 1, length_s: MOVE_S }]
}

fn dip_of(a: Action) -> Move {
    let dip = MOVE_VOXELS.0 + (MOVE_VOXELS.1 - MOVE_VOXELS.0) * ((a.hash >> 20) & 0xFF) as f32 / 255.0;
    Move { start_tick: a.start_tick, ticks: a.ticks, dip }
}

/// Head move `k` of animal `id` (the class path's dips, [`schedule`]d by `actions`).
pub fn move_k(id: u64, k: i64, dt_s: f64, actions: &IdleActions) -> Option<Move> {
    schedule(id, k, dt_s, actions.apart_s, &dips(actions)).map(dip_of)
}

/// The head move of animal `id` that a pack staged at `tick` draws: one under way at the
/// tick, or due before the pack's frames run out.
pub fn move_near(id: u64, tick: u64, dt_s: f64, actions: &IdleActions) -> Option<Move> {
    near(id, tick, dt_s, actions.apart_s, &dips(actions)).first().copied().map(dip_of)
}

/// Every head move of animal `id` starting in `from..to` (ticks), in order.
pub fn moves_between(id: u64, from: u64, to: u64, dt_s: f64, actions: &IdleActions) -> Vec<Move> {
    between(id, from, to, dt_s, actions.apart_s, &dips(actions)).into_iter().map(dip_of).collect()
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

    fn dips() -> IdleActions {
        IdleConfig::default().frondgrazer
    }

    fn kinds(weights: &[f64], lengths: &[f64]) -> Vec<Kind> {
        weights.iter().zip(lengths).map(|(&weight, &length_s)| Kind { weight, variants: 3, length_s }).collect()
    }

    /// Each start is `apart_s` from the last, and never closer than the longest action
    /// lasts, whatever the knob says: one action at a time.
    #[test]
    fn actions_come_apart_by_the_knob_and_never_overlap() {
        let set = kinds(&[1.0, 2.0, 1.0], &[3.0, 5.5, 2.0]);
        for apart_s in [[8.0f64, 18.0], [2.0, 3.0], [6.0, 6.0]] {
            let lo = apart_s[0].max(5.5);
            let hi = apart_s[1].max(lo);
            for id in 0..12u64 {
                let all = between(id, 0, 20 * 1200, DT, apart_s, &set);
                assert!(all.len() > 10);
                for w in all.windows(2) {
                    let gap = (w[1].start_tick - w[0].start_tick) as f64 * DT;
                    assert!(gap >= lo - 1e-9 && gap <= hi + 1e-9, "{apart_s:?}: gap {gap}");
                    assert!(w[0].start_tick + w[0].ticks <= w[1].start_tick, "overlap");
                }
            }
        }
    }

    /// With several actions on, one never follows itself; each on action comes, each
    /// variant of it too, and an action weighted 0 never does. With one, it is every one.
    #[test]
    fn the_next_action_is_drawn_by_weight_and_never_repeats() {
        for weights in [vec![1.0, 1.0], vec![3.0, 1.0, 1.0], vec![1.0, 0.0, 2.0, 1.0]] {
            let set = kinds(&weights, &vec![2.0; weights.len()]);
            for id in 0..12u64 {
                let all = between(id, 0, 20 * 3600, DT, [4.0, 10.0], &set);
                for w in all.windows(2) {
                    assert_ne!(w[0].kind, w[1].kind, "{weights:?}: a repeat");
                }
                for (i, &wt) in weights.iter().enumerate() {
                    let n = all.iter().filter(|a| a.kind == i).count();
                    assert_eq!(n > 0, wt > 0.0, "{weights:?}: kind {i} came {n} times");
                    if wt > 0.0 {
                        for v in 0..3 {
                            assert!(all.iter().any(|a| a.kind == i && a.variant == v), "variant {v}");
                        }
                    }
                }
            }
        }
        let one = kinds(&[1.0, 0.0], &[2.0, 2.0]);
        assert!(between(3, 0, 20 * 600, DT, [4.0, 10.0], &one).iter().all(|a| a.kind == 0));
        assert!(schedule(3, 1, DT, [4.0, 10.0], &kinds(&[0.0], &[2.0])).is_none(), "none on");
    }

    /// A pack staged at any tick of an action carries it; one staged just before carries
    /// it when it is due within the pack's frames.
    #[test]
    fn every_pack_over_an_action_carries_it() {
        let set = kinds(&[1.0, 1.0], &[4.0, 6.0]);
        for id in 0..8u64 {
            for a in between(id, 400, 4000, DT, [8.0, 18.0], &set) {
                for tick in a.start_tick - LOOKAHEAD_TICKS..a.start_tick + a.ticks {
                    assert!(near(id, tick as u64, DT, [8.0, 18.0], &set).contains(&a), "tick {tick}");
                }
                assert!(!near(id, (a.start_tick + a.ticks) as u64, DT, [8.0, 18.0], &set).contains(&a));
            }
        }
    }

    #[test]
    fn head_dips_come_apart_by_the_frondgrazers_knob() {
        let d = dips();
        for id in 0..16u64 {
            let moves = moves_between(id, 0, 20 * 600, DT, &d);
            assert!(moves.len() > 20);
            for w in moves.windows(2) {
                let gap = (w[1].start_tick - w[0].start_tick) as f64 * DT;
                assert!((d.apart_s[0]..=d.apart_s[1]).contains(&gap), "gap {gap}");
            }
            assert!(moves.iter().all(|m| (MOVE_VOXELS.0..=MOVE_VOXELS.1).contains(&m.dip)));
        }
    }

    /// Every pack between a move's start and end carries it, and the frame it draws at a
    /// given time is the same from either of two packs.
    #[test]
    fn the_head_move_is_bounded_eased_and_returns_to_zero() {
        let id = 7;
        let d = dips();
        let m = moves_between(id, 400, 2000, DT, &d)[0];
        let t0 = m.start_tick as u64;
        let at = |tick: u64, frac: f32| {
            let head = move_near(id, tick, DT, &d).map_or(HeadMove::NONE, |m| m.relative_to(tick));
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
        let d = dips();
        assert_ne!(move_k(1, 3, DT, &d).unwrap().start_tick, move_k(2, 3, DT, &d).unwrap().start_tick);
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
