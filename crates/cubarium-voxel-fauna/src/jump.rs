//! **DROP: a trapped walker jumps down, at some risk** (Wrysk, 2026-10-06: "Should a trapped
//! animal be able to jump down a bigger drop than it can normally step, at some risk of
//! injury?" — "yes").
//!
//! A grounded body is **trapped** when it stands off its lineage's route graph with no
//! walking way back on ([`crate::RouteGraph::way_on`]) and no glide off its ledge — EDGE-2's
//! `off_graph_exit` answers `Cornered` — and has been for its lineage's
//! [`DropRule::after_s`]. It then jumps down past its step: the lowest drop within its reach
//! ([`DropRule::max_m`] × its height over the adult's) off an edge of the pocket it can walk,
//! onto a face clear for its body, in water it wades, from which its graph is reachable
//! ([`crate::RouteGraph::drop_way`]). The landing costs a wound of [`DropRule::wound_per_m`]
//! per metre beyond its `step_down_m`, through the predation wound (it slows the body, heals
//! at the predation heal rate, and at 1 the body dies, [`crate::Departure::Fell`]).
//!
//! It is an escape only: the route graph, and every route planned on it, never uses a drop.
//!
//! **Real animals** (the defaults, [`drop_of`]): wolves and big cats jump down 2–4 m (Wrysk's
//! brief); falls injure in proportion to the height — cats falling from buildings (Whitney &
//! Mehlhaff 1987, JAVMA 191:1399, "high-rise syndrome", 132 cats) and dogs (Gordon, Thacher &
//! Kapatkin 1993, JAVMA 202:118, 81 dogs) are hurt more the higher the fall; and the smaller
//! the animal the less a given fall hurts it (Haldane 1926, "On Being the Right Size"). The
//! numbers are placeholders on that shape, for the tuning pass.

use serde::{Deserialize, Serialize};

use super::glider::{OffGraph, off_graph_exit};
use crate::{Actions, Fauna, Founder, FounderPhysiology, Site};
use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::FloraView;

#[cfg(test)]
#[path = "testpass3_drop_tests.rs"]
mod testpass3_drop_tests;

/// **A lineage's jump down** when trapped (DROP). A reach at or under its step is no jump.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DropRule {
    /// Trapped this long (off its route graph, no walking way back on, no glide off) before
    /// it jumps, seconds.
    pub after_s: f64,
    /// The deepest jump down an adult takes, metres; a smaller body's is this × its height
    /// over the adult's.
    pub max_m: f64,
    /// The wound a landing adds per metre dropped beyond its `step_down_m` (a wound of 1
    /// kills).
    pub wound_per_m: f64,
}

impl Default for DropRule {
    /// No jump.
    fn default() -> DropRule {
        DropRule { after_s: 30.0, max_m: 0.0, wound_per_m: 0.0 }
    }
}

impl DropRule {
    /// The deepest jump down the body `body` of lineage `phys` takes, metres.
    pub fn reach_m(&self, phys: &FounderPhysiology, body: &crate::Body) -> f64 {
        let ratio = if phys.adult_height_m > 0.0 { body.height_m / phys.adult_height_m } else { 1.0 };
        let reach = self.max_m * ratio;
        if reach.is_finite() { reach.max(0.0) } else { 0.0 }
    }

    /// The wound a drop of `drop_m` metres adds to a body of lineage `phys`.
    pub fn wound(&self, phys: &FounderPhysiology, drop_m: f64) -> f64 {
        let w = self.wound_per_m * (drop_m - phys.step_down_m).max(0.0);
        if w.is_finite() { w.max(0.0) } else { 0.0 }
    }
}

/// Each lineage's jump down (module docs for the sources): a few body heights for the
/// Veilhound, the lanternjaw and the seedporter, about three for the frondgrazer, little past
/// its long step for the heavy loftstrider; far for the small littershredder and capgnawer,
/// which a fall hardly hurts. The bellwing flies and the ripple-snail does not jump.
pub fn drop_of(founder: Founder) -> DropRule {
    let none = DropRule::default();
    match founder {
        // 0.6 m tall: 3 m, five heights (a wolf's 2–4 m); a 3 m drop wounds 0.19.
        Founder::Chorister => DropRule { max_m: 3.0, wound_per_m: 0.08, ..none },
        // 0.45 m tall: four heights.
        Founder::Lanternjaw => DropRule { max_m: 1.8, wound_per_m: 0.1, ..none },
        // 0.2 m tall, an arboreal climber that lands on its hands: eight heights.
        Founder::Seedporter => DropRule { max_m: 1.6, wound_per_m: 0.1, ..none },
        // The armoured frondgrazer, 0.375 m tall: three heights.
        Founder::Browser => DropRule { max_m: 1.1, wound_per_m: 0.15, ..none },
        // 3.4 m tall on long legs that step 1.15 m: heavy, so a drop hurts it most.
        Founder::Loftstrider => DropRule { max_m: 2.0, wound_per_m: 0.3, ..none },
        // Small bodies a fall hardly hurts.
        Founder::Blind => DropRule { max_m: 1.0, wound_per_m: 0.02, ..none },
        Founder::Capgnawer => DropRule { max_m: 1.0, wound_per_m: 0.03, ..none },
        Founder::Bellwing | Founder::RippleSnail => none,
    }
}

/// A trapped body's chosen jump ([`crate::RouteGraph::drop_way`]), held between its controller
/// samples for the jump ([`take_off`]). Session state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DropPlan {
    /// The pocket face it jumps from.
    pub from: Site,
    /// The face it lands on.
    pub land: Site,
    /// The column it jumps out over (`from`'s neighbour).
    pub edge: Site,
}

/// **The drop reflex** for the due body at `i`, on its controller sample: `None` unless it is
/// trapped and has been for its lineage's `drop.after_s` and a jump is on offer; then the
/// walk to the edge and over it (held by [`DropPlan`] for the motor).
pub(crate) fn reflex(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let rule = phys.drop;
    let v = view.config.voxel_m;
    let body = phys.body_at(a.body);
    let step = phys.step_limits(v);
    let reach = crate::body::step_voxels(rule.reach_m(&phys, &body), v);
    let grounded = a.mobility.mode == crate::MobilityMode::Ground && a.mobility.climb.is_none() && phys.flight.is_none();
    if !grounded || reach <= step.down {
        fauna.trapped.remove(&a.id);
        fauna.drop_plans.remove(&a.id);
        return None;
    }
    // Trapped: re-read only when it has moved to another face or the graph may have changed.
    let now = fauna.tick;
    let every = cubarium_voxel::ticks_at_least_one(fauna.config.teachers.route_refresh_s);
    let cornered = match fauna.trapped.get(&a.id) {
        Some(t) if t.site == a.site && now.saturating_sub(t.checked) < every => t.cornered,
        _ => {
            let cornered = off_graph_exit(fauna, view, fv, &a, (0.0, 0.0)) == OffGraph::Cornered;
            let since = match fauna.trapped.get(&a.id) {
                Some(t) if t.cornered && cornered => t.since,
                _ => now,
            };
            fauna.trapped.insert(a.id, Trapped { since, site: a.site, checked: now, cornered });
            cornered
        }
    };
    if !cornered {
        fauna.drop_plans.remove(&a.id);
        return None;
    }
    let since = fauna.trapped[&a.id].since;
    if (now.saturating_sub(since) as f64) * cubarium_voxel::DT < rule.after_s {
        return None;
    }
    let tc = fauna.config.teachers;
    let graph = fauna.routes(view, founder, tc.route_refresh_s);
    let r = body.footprint_radius();
    let own = (r / v).ceil().max(0.0) as u32;
    let cols = if tc.way_on_m.is_finite() && tc.way_on_m > 0.0 { (tc.way_on_m / v).ceil() as u32 } else { 0 };
    let Some(way) = graph.drop_way(view, a.site, r, body.headroom_voxels(v), step, reach, phys.wade_depth_m(&body), own, cols) else {
        fauna.drop_plans.remove(&a.id);
        return None;
    };
    fauna.drop_plans.insert(a.id, DropPlan { from: way.from, land: way.land, edge: way.edge });
    let aim = if a.site == way.from { way.land } else { way.first };
    let ring = f64::from(view.config.width) * v;
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, (f64::from(aim.x) + 0.5) * v, (f64::from(aim.z) + 0.5) * v, ring);
    Some(super::glider::walk_toward(&a, dx, dz, 0.0))
}

/// The trapped clock of one body ([`Fauna::trapped`]): when it was first found cornered, and
/// the face and tick of the last reading. Session state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Trapped {
    pub since: u64,
    pub site: Site,
    pub checked: u64,
    pub cornered: bool,
}

/// **The jump**, after the body at `i` walked this tick: standing on the face its plan leaves
/// from, held forward and facing the landing (within a right angle), with its disc reaching
/// over the edge (its centre within its radius of the edge column), it comes down on the
/// landing's centre (out over the edge: a disc wider than half a voxel jumps clear of the
/// riser). The wound, the ledger, and the plan and the clock cleared.
pub(crate) fn take_off(fauna: &mut Fauna, i: usize, founder: Founder, view: &VoxelView<'_>, held: Actions) {
    let a = fauna.animals[i];
    let Some(&plan) = fauna.drop_plans.get(&a.id) else { return };
    if a.site != plan.from || !(held.forward > 0.0) || plan.land.y >= plan.from.y {
        return;
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let (lx, lz) = ((f64::from(plan.land.x) + 0.5) * v, (f64::from(plan.land.z) + 0.5) * v);
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, lx, lz, ring);
    let (fx, fz) = a.pose.forward();
    if fx * dx + fz * dz <= 0.0 {
        return;
    }
    // The distance from the centre to the edge column's square: its disc reaches over it.
    let (ex, ez) = ((f64::from(plan.edge.x) + 0.5) * v, (f64::from(plan.edge.z) + 0.5) * v);
    let (edx, edz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, ex, ez, ring);
    let (sx, sz) = (a.pose.x + edx, a.pose.z + edz);
    let qx = a.pose.x.clamp(sx - 0.5 * v, sx + 0.5 * v);
    let qz = a.pose.z.clamp(sz - 0.5 * v, sz + 0.5 * v);
    let gap = ((a.pose.x - qx).powi(2) + (a.pose.z - qz).powi(2)).sqrt();
    let phys = *fauna.config.founder(founder);
    if gap > phys.body_at(a.body).footprint_radius() + 1e-9 {
        return;
    }
    let layers = plan.from.y - plan.land.y;
    let drop_m = f64::from(layers) * v;
    let w = phys.drop.wound(&phys, drop_m);
    let b = &mut fauna.animals[i];
    b.pose.x = lx.rem_euclid(ring);
    b.pose.z = lz;
    b.site = plan.land;
    b.pose.y_m = crate::surface_m(plan.land.y, v);
    b.wound += w;
    let l = &mut fauna.ledger;
    l.drops_by_founder[founder.index()] += 1;
    l.drop_m_by_founder[founder.index()] += drop_m;
    l.drop_wound_by_founder[founder.index()] += w;
    fauna.drop_plans.remove(&a.id);
    fauna.trapped.remove(&a.id);
}
