//! **NUTS: checking trees for food it cannot see** (Wrysk 2026-10-12: "somehow seedporters
//! should know to liek \"check\" trees and stuff cuz they wont be able to see larvae or nuts
//! from the ground in a tall vaulttree"; `design/larva-nuts-plan-2026-10-12.md`).
//!
//! A hungry grounded glider on its heuristic that sees no food walks to a nearby **host**, a
//! plant that may hold food it cannot see from where it is ([`may_hold_hidden_food`]), climbs
//! into its crown and looks (the glider's walk, `step::glider`). This module holds the parts
//! that are not the walk:
//!
//! - **the host test**, one predicate, [`may_hold_hidden_food`], and what a host holds,
//!   [`hidden_food`] (LARVA extends both: latticevines and vaulttrees holding larvae);
//! - **the memory** ([`TreeChecks`], saved per body): each host it looked into, when, and
//!   whether it held food then, kept while perched in the crown so a tree it eats out is
//!   remembered empty. It does not check a remembered-empty host again until the memory
//!   lapses (`Glider::check_memory_s`) and goes first to one remembered holding food
//!   ([`order`]): the trap-lining of tamarins and marmosets, coarse;
//! - **the readout** ([`CheckTally`]).
//!
//! The rule shapes no reward and enters no observation: a brain sees only what it sees.

use serde::{Deserialize, Serialize};

use crate::{Fauna, MobilityMode, Perch, PerchAnchor};
use cubarium_voxel_flora::{FloraView, Stand};

/// The most hosts a body remembers at once; the oldest is forgotten first. A memory bound,
/// not a knob.
pub const CHECK_SLOTS: usize = 16;

/// One host a body looked into: its id (a stand's), the tick it last looked, and whether it
/// held food then.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Checked {
    pub host: u64,
    pub tick: u64,
    pub held_food: bool,
}

/// **A body's tree-check memory** (saved): the hosts it looked into (at most
/// [`CHECK_SLOTS`]), the host its check walk is taking it to and the tick it set out, the
/// host whose crown it is in, and how many checks it has made.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TreeChecks {
    pub seen: Vec<Checked>,
    pub aim: Option<u64>,
    pub aim_since: u64,
    pub here: Option<u64>,
    pub checks: u64,
}

/// **The tree checks' readout** per world (session state): check walks set out on (a new
/// host aimed at), arrivals in a host crown (any), arrivals at the host a check walk aimed
/// at, those that found food, and the bodies that made a first check.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CheckTally {
    pub walks: u64,
    pub visits: u64,
    pub checks: u64,
    pub found: u64,
    pub checkers: u64,
}

/// **The host test** (one predicate; LARVA adds latticevines and vaulttrees holding larvae):
/// whether `stand` is a plant that may hold food an eater cannot see from afar. Here: a
/// crowned stand (`profile_stage >= 1`, the crop's own gate) of a species that bears fruit
/// seen only from close by ([`cubarium_voxel_flora::SpeciesConfig::fruit_sight_m`]): the
/// vaulttree. Whether it holds any is not read: that is what a check finds out.
pub fn may_hold_hidden_food(_fauna: &Fauna, fv: &FloraView<'_>, stand: &Stand) -> bool {
    let sc = fv.config.species(stand.species);
    sc.fruit_per_s > 0.0 && sc.fruit_sight_m > 0.0 && stand.profile_stage >= 1
}

/// **What a host holds** for a body in its crown (LARVA adds its larvae): its hanging fruit.
pub fn hidden_food(_fauna: &Fauna, _fv: &FloraView<'_>, stand: &Stand) -> bool {
    stand.fruit > 0.0
}

/// What body `id` remembers of `host` within `memory` ticks of `now`.
pub(crate) fn recall(fauna: &Fauna, id: u64, host: u64, now: u64, memory: u64) -> Option<Checked> {
    fauna
        .tree_checks
        .get(&id)?
        .seen
        .iter()
        .find(|c| c.host == host && now.saturating_sub(c.tick) < memory)
        .copied()
}

/// **The check's order** over `cands` (`(distance, host, …)`, nearest first): a host
/// remembered empty within the memory is dropped; of the rest, those remembered holding food
/// come first, then the unchecked (or forgotten), each nearest first. `recall` answers what
/// the body remembers of a host.
pub(crate) fn order<T: Copy>(cands: &[(f64, u64, T)], recall: impl Fn(u64) -> Option<Checked>) -> Vec<(f64, u64, T)> {
    let mut fed = Vec::new();
    let mut fresh = Vec::new();
    for &c in cands {
        match recall(c.1) {
            Some(k) if k.held_food => fed.push(c),
            Some(_) => {}
            None => fresh.push(c),
        }
    }
    fed.extend(fresh);
    fed
}

/// A check walk for body `id` takes it to `host` this sample: a new aim is a walk set out on.
pub(crate) fn aim(fauna: &mut Fauna, id: u64, host: u64) {
    let now = fauna.tick;
    let m = fauna.tree_checks.entry(id).or_default();
    if m.aim != Some(host) {
        m.aim = Some(host);
        m.aim_since = now;
        fauna.check_tally.walks += 1;
    }
}

/// **The look** (each tick of a glider's act): on a host — perched in its crown, or climbing
/// its stem with the crown's food in its mouth's reach — it remembers whether the host holds
/// food now; the first look since it got onto the host is a visit, and a check when its check
/// walk aimed there. Off the host's stem and crown it is on none. Nothing is kept while the
/// check is off (`check_search_m` 0).
pub(crate) fn note(fauna: &mut Fauna, i: usize, view: &cubarium_voxel::VoxelView<'_>, fv: &FloraView<'_>) {
    let a = fauna.animals[i];
    let Some(founder) = a.founder else { return };
    let Some(glider) = fauna.config.founder(founder).glider else { return };
    if !(glider.check_search_m > 0.0) {
        return;
    }
    prune(fauna);
    let host = match (a.mobility.mode, a.mobility.perch) {
        (MobilityMode::Perched | MobilityMode::Climb, Some(Perch { anchor: PerchAnchor::Stand { site, .. }, .. })) => {
            fv.stand_at(site).filter(|s| may_hold_hidden_food(fauna, fv, s))
        }
        _ => None,
    };
    let Some(stand) = host else {
        if let Some(m) = fauna.tree_checks.get_mut(&a.id) {
            m.here = None;
        }
        return;
    };
    // On the stem below the crown's food it cannot look yet.
    let looks = a.mobility.mode == MobilityMode::Perched
        || crate::step::glider_fruit_in_reach(view, fv, &a, fauna.config.founder(founder))
            .is_some_and(|(b, _)| b == crate::Bloom::Stand(stand.site));
    if !looks {
        return;
    }
    let food = hidden_food(fauna, fv, stand);
    let now = fauna.tick;
    let tally = &mut fauna.check_tally;
    let m = fauna.tree_checks.entry(a.id).or_default();
    if m.here != Some(stand.id) {
        m.here = Some(stand.id);
        tally.visits += 1;
        if m.aim == Some(stand.id) {
            m.aim = None;
            tally.checks += 1;
            tally.found += u64::from(food);
            if m.checks == 0 {
                tally.checkers += 1;
            }
            m.checks += 1;
        }
    }
    match m.seen.iter_mut().find(|c| c.host == stand.id) {
        Some(c) => {
            c.tick = now;
            c.held_food = food;
        }
        None => {
            if m.seen.len() >= CHECK_SLOTS
                && let Some(k) = m.seen.iter().enumerate().min_by_key(|(_, c)| c.tick).map(|(k, _)| k)
            {
                m.seen.swap_remove(k);
            }
            m.seen.push(Checked { host: stand.id, tick: now, held_food: food });
        }
    }
}

/// The dead's memories go, at most once a tick and every 256 ticks.
fn prune(fauna: &mut Fauna) {
    let now = fauna.tick;
    if now % 256 != 0 || fauna.check_pruned == Some(now) || fauna.tree_checks.is_empty() {
        return;
    }
    fauna.check_pruned = Some(now);
    let alive: rustc_hash::FxHashSet<u64> = fauna.animals.iter().map(|a| a.id).collect();
    fauna.tree_checks.retain(|id, _| alive.contains(id));
}
