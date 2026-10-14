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
//!   [`hidden_food`]: a crowned vaulttree (nuts, larvae or both) or a latticevine with leaves
//!   (larvae; ECO-INT). A vine check walks to its nearest leafy face, as a fruit walk does,
//!   and looks from there;
//! - **the memory** ([`TreeChecks`], saved per body): each host it looked into, when, and
//!   whether it held food then, kept while perched in the crown so a tree it eats out is
//!   remembered empty. It does not check a remembered-empty host again until the memory
//!   lapses (`Glider::check_memory_s`) and goes first to one remembered holding food
//!   ([`order`]): the trap-lining of tamarins and marmosets, coarse;
//! - **the readout** ([`CheckTally`]).
//!
//! The rule shapes no reward and enters no observation: a brain sees only what it sees.

use serde::{Deserialize, Serialize};

use crate::larva::LarvaHost;
use crate::{Fauna, Food, FounderPhysiology, MobilityMode, Perch, PerchAnchor};
use cubarium_voxel_flora::{CoveredFace, FloraView, Stand, Vine};

/// The most hosts a body remembers at once; the oldest is forgotten first. A memory bound,
/// not a knob.
pub const CHECK_SLOTS: usize = 16;

/// One host a body looked into: its key (a stand or a vine, never confused: ECO-INT), the
/// tick it last looked, and whether it held food then.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checked {
    pub host: LarvaHost,
    pub tick: u64,
    pub held_food: bool,
}

/// **A body's tree-check memory** (saved): the hosts it looked into (at most
/// [`CHECK_SLOTS`]), the host its check walk is taking it to and the tick it set out, the
/// host whose crown it is in, and how many checks it has made.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TreeChecks {
    pub seen: Vec<Checked>,
    pub aim: Option<LarvaHost>,
    pub aim_since: u64,
    pub here: Option<LarvaHost>,
    pub checks: u64,
}

/// **The tree checks' readout** per world (session state): check walks set out on (a new
/// host aimed at), of those the ones that replaced an aim not reached, arrivals in a host
/// crown (any), arrivals at the host a check walk aimed at, those that found food, and the
/// bodies that made a first check.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CheckTally {
    pub walks: u64,
    pub switches: u64,
    pub visits: u64,
    pub checks: u64,
    pub found: u64,
    pub checkers: u64,
    /// SP-PROTEIN: check walks given up past their walking bound ([`walk_bound`]).
    pub give_ups: u64,
}

/// **SP-PROTEIN: a check walk under way** (session state, not saved): the last tick the walk
/// steered toward its aim, and the ticks of walking it has spent on that aim. A load starts
/// every walk afresh.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CheckWalk {
    pub last: u64,
    pub walked: u64,
}

/// **SP-PROTEIN: a check walk's walking bound**, ticks, for body `a`: `check_walk_factor` ×
/// the longest route a reflex walk takes to a host at the edge of its search
/// (`teachers.route_detour` × `check_search_m`), at its own cruise (its lineage's body
/// lengths per second × its own length). A walk that has spent this long walking toward its
/// aim without arriving is given up; one not walked for this long has lapsed, and the next
/// walk to the same host starts afresh. `None` without a glider, a search or a pace.
pub(crate) fn walk_bound(fauna: &Fauna, a: &crate::Animal) -> Option<u64> {
    let phys = fauna.config.founder(a.founder?);
    let glider = phys.glider?;
    let length = crate::body_of(&fauna.config, a)?.length_m;
    let cruise = phys.cruise_body_lengths_per_s * length;
    let route = fauna.config.teachers.route_detour.max(1.0) * glider.check_search_m;
    let s = glider.check_walk_factor * route / cruise;
    (s > 0.0 && s.is_finite()).then(|| cubarium_voxel::ticks_at_least_one(s))
}

/// A plant as the check finds it: a stand, or a latticevine at one of its covered faces.
#[derive(Clone, Copy, Debug)]
pub enum Host<'a> {
    Stand(&'a Stand),
    Vine(&'a Vine, &'a CoveredFace),
}

impl Host<'_> {
    /// The key it is remembered by: a stand's id and a vine's id live in separate spaces.
    pub fn key(&self) -> LarvaHost {
        match self {
            Host::Stand(s) => LarvaHost::Stand(s.id),
            Host::Vine(v, _) => LarvaHost::Vine(v.id),
        }
    }
}

/// Whether an eater with physiology `eater` looks for larvae: it eats them and some lineage
/// lays them.
pub(crate) fn seeks_larvae(fauna: &Fauna, eater: &FounderPhysiology) -> bool {
    eater.eating_diet().eats(Food::Larva) && fauna.config.founders.iter().any(|p| p.larva.is_some())
}

/// **The host test** (one predicate): whether `host` is a plant that may hold food `eater`
/// cannot see from afar. A crowned stand (`profile_stage >= 1`, the crop's own gate) of a
/// species that bears fruit seen only from close by
/// ([`cubarium_voxel_flora::SpeciesConfig::fruit_sight_m`]: the vaulttree's nuts); and, for
/// an eater that seeks larvae, a larva host ([`crate::larva::may_hold_hidden_larvae`]: a
/// crowned vaulttree with leaves, a latticevine at a leafy face). A vaulttree counts for its
/// nuts, its larvae or both. Whether it holds any is not read: that is what a check finds out.
pub fn may_hold_hidden_food(fauna: &Fauna, fv: &FloraView<'_>, eater: &FounderPhysiology, host: Host<'_>) -> bool {
    let nuts = match host {
        Host::Stand(stand) => {
            let sc = fv.config.species(stand.species);
            sc.fruit_per_s > 0.0 && sc.fruit_sight_m > 0.0 && stand.profile_stage >= 1
        }
        Host::Vine(..) => false,
    };
    nuts || seeks_larvae(fauna, eater) && crate::larva::may_hold_hidden_larvae(fv, host)
}

/// **What a host holds** for a body that looks: a stand's hanging fruit, and the hatched
/// larvae it would see on a stand or a vine ([`crate::larva::visible`]).
pub fn hidden_food(fauna: &Fauna, _fv: &FloraView<'_>, host: Host<'_>) -> bool {
    let fruit = matches!(host, Host::Stand(s) if s.fruit > 0.0);
    fruit || crate::larva::visible(fauna, host.key()).is_some()
}

/// What body `id` remembers of `host` within `memory` ticks of `now`.
pub(crate) fn recall(fauna: &Fauna, id: u64, host: LarvaHost, now: u64, memory: u64) -> Option<Checked> {
    fauna
        .tree_checks
        .get(&id)?
        .seen
        .iter()
        .find(|c| c.host == host && now.saturating_sub(c.tick) < memory)
        .copied()
}

/// **The check's order** over `cands` (`(distance, host, …)`, nearest first): a host
/// remembered empty within the memory is dropped; of the rest, the host it is already
/// walking to (`aim`) comes first, so a walk is not given up for another tree that drew
/// level on the way; then those remembered holding food, then the unchecked (or forgotten),
/// each nearest first. `recall` answers what the body remembers of a host.
pub(crate) fn order<H: Copy + PartialEq, T: Copy>(cands: &[(f64, H, T)], aim: Option<H>, recall: impl Fn(H) -> Option<Checked>) -> Vec<(f64, H, T)> {
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
    if let Some(k) = aim.and_then(|h| fed.iter().position(|c| c.1 == h)) {
        let c = fed.remove(k);
        fed.insert(0, c);
    }
    fed
}

/// The host body `id`'s check walk is taking it to, if any.
pub(crate) fn aim_of(fauna: &Fauna, id: u64) -> Option<LarvaHost> {
    fauna.tree_checks.get(&id)?.aim
}

/// A check walk for body `id` takes it to `host` this sample: a new aim is a walk set out on.
/// **SP-PROTEIN**: the walk's time on its aim accrues between its samples; a walk last walked
/// longer ago than `bound` ticks ([`walk_bound`]; with no record, as after a load) has lapsed,
/// so this sample sets out afresh (its `aim_since` now), and replacing a lapsed aim is not a
/// switch. It was: an aim was dropped only on arrival, so a body that ate, slept or fled on
/// the way kept its first aim's clock for hours.
pub(crate) fn aim(fauna: &mut Fauna, id: u64, host: LarvaHost, bound: u64) {
    let now = fauna.tick;
    let lapsed = fauna.check_walks.get(&id).is_none_or(|w| now.saturating_sub(w.last) > bound);
    let m = fauna.tree_checks.entry(id).or_default();
    if m.aim != Some(host) || lapsed {
        fauna.check_tally.switches += u64::from(m.aim.is_some_and(|h| h != host) && !lapsed);
        m.aim = Some(host);
        m.aim_since = now;
        fauna.check_tally.walks += 1;
        fauna.check_walks.insert(id, CheckWalk { last: now, walked: 0 });
    } else if let Some(w) = fauna.check_walks.get_mut(&id) {
        w.walked += now.saturating_sub(w.last);
        w.last = now;
    }
}

/// **SP-PROTEIN: the give-up**: body `id`'s aim, when its walk has spent at least `bound`
/// ticks walking toward it ([`walk_bound`]) without arriving, is given up: the aim goes, and
/// the host is remembered as one not to check again within the memory, as an empty one is
/// ([`order`]). `true` when it gave up.
pub(crate) fn give_up_if_overdue(fauna: &mut Fauna, id: u64, bound: u64) -> bool {
    let Some(host) = aim_of(fauna, id) else { return false };
    if fauna.check_walks.get(&id).is_none_or(|w| w.walked < bound) {
        return false;
    }
    if let Some(m) = fauna.tree_checks.get_mut(&id) {
        m.aim = None;
    }
    fauna.check_walks.remove(&id);
    fauna.check_tally.give_ups += 1;
    remember(fauna, id, host, false);
    true
}

/// **The look** (each tick of a glider's act): on a host it remembers whether the host holds
/// food now; the first look since it got onto the host is a visit, and a check when its check
/// walk aimed there. On a stand host it is perched in the crown, or climbing the stem with the
/// crown's food in its mouth's reach; on a vine host its mouth reaches a leafy face of that
/// vine (where it would bite the vine's larvae, [`crate::step::glider_larva_in_reach`]). Off
/// every host it is on none. Nothing is kept while the check is off (`check_search_m` 0).
pub(crate) fn note(fauna: &mut Fauna, i: usize, view: &cubarium_voxel::VoxelView<'_>, fv: &FloraView<'_>) {
    let a = fauna.animals[i];
    let Some(founder) = a.founder else { return };
    let phys = *fauna.config.founder(founder);
    let Some(glider) = phys.glider else { return };
    if !(glider.check_search_m > 0.0) {
        return;
    }
    prune(fauna);
    let stand = match (a.mobility.mode, a.mobility.perch) {
        (MobilityMode::Perched | MobilityMode::Climb, Some(Perch { anchor: PerchAnchor::Stand { site, .. }, .. })) => {
            fv.stand_at(site).filter(|s| may_hold_hidden_food(fauna, fv, &phys, Host::Stand(s)))
        }
        _ => None,
    };
    let host = match stand {
        // On the stem below the crown's food it cannot look yet.
        Some(s) => {
            let looks = a.mobility.mode == MobilityMode::Perched
                || crate::step::glider_fruit_in_reach(view, fv, &a, &phys).is_some_and(|(b, _)| b == crate::Bloom::Stand(s.site))
                || crate::step::glider_larva_in_reach(fauna, view, fv, &a, &phys).is_some_and(|(h, _)| h == LarvaHost::Stand(s.id));
            if !looks {
                return;
            }
            Some(Host::Stand(s))
        }
        None if a.mobility.mode != MobilityMode::Glide && seeks_larvae(fauna, &phys) => {
            crate::step::leafy_vine_in_reach(view, fv, &a, &phys).filter(|&h| may_hold_hidden_food(fauna, fv, &phys, h))
        }
        None => None,
    };
    let Some(host) = host else {
        if let Some(m) = fauna.tree_checks.get_mut(&a.id) {
            m.here = None;
        }
        return;
    };
    let food = hidden_food(fauna, fv, host);
    let key = host.key();
    let tally = &mut fauna.check_tally;
    let m = fauna.tree_checks.entry(a.id).or_default();
    if m.here != Some(key) {
        m.here = Some(key);
        tally.visits += 1;
        if m.aim == Some(key) {
            m.aim = None;
            fauna.check_walks.remove(&a.id);
            tally.checks += 1;
            tally.found += u64::from(food);
            if m.checks == 0 {
                tally.checkers += 1;
            }
            m.checks += 1;
        }
    }
    remember(fauna, a.id, key, food);
}

/// Body `id` remembers now that `host` held food (`food`) or not: its entry is refreshed, or
/// a new one takes the oldest slot once all [`CHECK_SLOTS`] are full.
pub(crate) fn remember(fauna: &mut Fauna, id: u64, host: LarvaHost, food: bool) {
    let now = fauna.tick;
    let m = fauna.tree_checks.entry(id).or_default();
    match m.seen.iter_mut().find(|c| c.host == host) {
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
            m.seen.push(Checked { host, tick: now, held_food: food });
        }
    }
}

/// The dead's memories go, at most once a tick and every 256 ticks.
pub(crate) fn prune(fauna: &mut Fauna) {
    let now = fauna.tick;
    if now % 256 != 0 || fauna.check_pruned == Some(now) || fauna.tree_checks.is_empty() {
        return;
    }
    fauna.check_pruned = Some(now);
    let alive: rustc_hash::FxHashSet<u64> = fauna.animals.iter().map(|a| a.id).collect();
    fauna.tree_checks.retain(|id, _| alive.contains(id));
    fauna.check_walks.retain(|id, _| alive.contains(id));
}
