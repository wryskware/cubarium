//! **DROP: a trapped walker jumps down, at some risk** (Wrysk, 2026-10-06: "Should a trapped
//! animal be able to jump down a bigger drop than it can normally step, at some risk of
//! injury?" — "yes").
//!
//! A grounded body is **trapped** when it stands off its lineage's route graph with no
//! walking way back on ([`crate::RouteGraph::way_on`]) and no glide off its ledge — EDGE-2's
//! `off_graph_exit` answers `Cornered` — and has been for its lineage's
//! [`DropRule::after_s`]. Off the graph includes standing on an island of it, or by a node
//! its motor cannot walk to from where it stands ([`crate::RouteGraph::node_under`]); a
//! brief time off its feet (a climb or a cling) does not restart the clock (SP-STUCK). It then jumps down past its step: the lowest drop within its reach
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
use rustc_hash::FxHashSet;
use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::FloraView;

#[cfg(test)]
#[path = "testpass3_drop_tests.rs"]
mod testpass3_drop_tests;

#[cfg(test)]
#[path = "sp_stuck_tests.rs"]
mod sp_stuck_tests;

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

/// **A lineage's wade-out** (WADE-OUT, [`wade_reflex`]): whether a walker held in water past
/// its depth, or in a pocket ringed by it, swims the least-deep way out within its drown
/// clock.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WadeRule {
    /// It swims out (true), or stays where it stands (false).
    pub swim_out: bool,
}

impl Default for WadeRule {
    /// Swims out: most animals can swim (Wrysk 2026-10-10: "deffo swim out").
    fn default() -> WadeRule {
        WadeRule { swim_out: true }
    }
}

/// Each lineage's wade-out: on for every walker but the lanternjaw.
pub fn wade_of(founder: Founder) -> WadeRule {
    match founder {
        // Insectoid: no swimming (Wrysk 2026-10-10).
        Founder::Lanternjaw => WadeRule { swim_out: false },
        _ => WadeRule::default(),
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
    /// Where in `land`'s column it comes down ([`crate::DropWay::at`]).
    pub at: u8,
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
    if reach <= step.down {
        fauna.trapped.remove(&a.id);
        fauna.drop_plans.remove(&a.id);
        return None;
    }
    let now = fauna.tick;
    if !grounded {
        // SP-STUCK: off its feet a moment (up a vine it cannot top and back down, a cling)
        // is no way out, and the clock keeps running. A seedporter on a shelf climbed the
        // covered riser behind it every few seconds and slid back, and each climb restarted
        // its clock, so it never jumped. Off the ground longer than the wait, its clock
        // starts afresh when it is next read on its feet.
        fauna.drop_plans.remove(&a.id);
        if let Some(t) = fauna.trapped.get_mut(&a.id) {
            let aloft = *t.aloft.get_or_insert(now);
            if (now.saturating_sub(aloft) as f64) * cubarium_voxel::DT > rule.after_s {
                fauna.trapped.remove(&a.id);
            }
        }
        return None;
    }
    if let Some(t) = fauna.trapped.get_mut(&a.id) {
        t.aloft = None;
    }
    // Trapped: re-read only when it has moved to another face or the graph may have changed.
    let every = cubarium_voxel::ticks_at_least_one(fauna.config.teachers.route_refresh_s);
    let cornered = match fauna.trapped.get(&a.id) {
        Some(t) if t.site == a.site && now.saturating_sub(t.checked) < every => t.cornered,
        _ => {
            let cornered = off_graph_exit(fauna, view, fv, &a, (0.0, 0.0)) == OffGraph::Cornered;
            let since = match fauna.trapped.get(&a.id) {
                Some(t) if t.cornered && cornered => t.since,
                _ => now,
            };
            fauna.trapped.insert(a.id, Trapped { since, site: a.site, checked: now, cornered, aloft: None });
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
    let Some(way) = graph.drop_way(view, a.site, (a.pose.x, a.pose.z), r, body.headroom_voxels(v), step, reach, phys.wade_depth_m(&body), own, cols) else {
        fauna.drop_plans.remove(&a.id);
        return None;
    };
    fauna.drop_plans.insert(a.id, DropPlan { from: way.from, land: way.land, edge: way.edge, at: way.at });
    let aim = if a.site == way.from { way.landing(v, view.config.width) } else { ((f64::from(way.first.x) + 0.5) * v, (f64::from(way.first.z) + 0.5) * v) };
    let ring = f64::from(view.config.width) * v;
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, aim.0, aim.1, ring);
    Some(super::glider::walk_toward(&a, dx, dz, 0.0))
}

/// The trapped clock of one body ([`Fauna::trapped`]): when it was first found cornered, the
/// face and tick of the last reading, and the tick it last left its feet (SP-STUCK: `None`
/// while it stands). Session state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Trapped {
    pub since: u64,
    pub site: Site,
    pub checked: u64,
    pub cornered: bool,
    pub aloft: Option<u64>,
}

/// **The jump**, after the body at `i` walked this tick: standing on the face its plan leaves
/// from, held forward and facing the landing (within a right angle), with its disc reaching
/// over the edge (its centre within its radius of the edge column), it comes down on the
/// plan's landing point ([`crate::DropWay::landing`]: the landing's centre, or pressed to a
/// side of its column; out over the edge, a disc wider than half a voxel jumps clear of the
/// riser). The wound, the ledger, and the plan and the clock cleared.
pub(crate) fn take_off(fauna: &mut Fauna, i: usize, founder: Founder, view: &VoxelView<'_>, held: Actions) {
    let a = fauna.animals[i];
    let Some(&plan) = fauna.drop_plans.get(&a.id) else { return };
    if a.site != plan.from || !(held.forward > 0.0) || plan.land.y >= plan.from.y {
        return;
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let (lx, lz) = crate::encounter::drop_landing(plan.land, plan.at, v, view.config.width);
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

/// **WADE-OUT: a walker held in water it may not wade walks out through it** (SP-FREEZE2). A
/// body's wade depth ([`FounderPhysiology::wade_depth_m`], a quarter of a seedporter's height)
/// is under the depth it drowns in (half its height), and a juvenile's both scale with its
/// height: a juvenile seedporter 0.23 m tall refuses 6 cm of water. Gliding down onto a
/// flooded shore it lands on a few faces of water it wades, ringed by water it does not (a
/// pocket: of its lineage's route graph, a component of a handful of nodes; or of its own,
/// where the adult the graph is built for wades on), or in water past its depth whose nearest
/// shallows lie across deeper water (the motor's "never deeper" escape cannot reach them), and
/// stands there for good: the graph says it is on it, so DROP never finds it cornered.
///
/// A grounded walker (not aquatic, not climbing, not a flyer) is **held** when it stands in
/// water deeper than it wades, or in a pocket: the faces it walks to through water it wades all
/// lie within the window DROP's pocket search spans (`⌈teachers.way_on_m / voxel⌉` columns
/// round it; [`crate::RouteGraph::wade_pocket`], read only beside water it does not wade) —
/// for a body as large as the graph's adult, its graph component has fewer nodes than that
/// window holds. Held for its lineage's [`DropRule::after_s`] (the shallows reflex's and its
/// own walk's time to get out; none under water it drowns in), it takes the way out with the
/// fewest faces of water it would drown in, then the fewest faces
/// ([`crate::RouteGraph::wade_way`], out to `life.shallows_search_m`), to a face it wades on
/// an open part of its graph and outside every pocket — crossing water it is under only for
/// as long as its drown clock allows (`drown_after_s`, less what has run, at a quarter of its
/// cruise: a short swim) — its motor admitting the way's deepest water
/// ([`Fauna::wade_limit`]) until it gets there, leaves the way, or runs out of the way's time.
/// A body with no such way in reach looks again at `teachers.route_refresh_s`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Wade {
    /// The tick it was first found held; `None` while it is not.
    pub since: Option<u64>,
    /// The tick it last looked for a way out.
    pub searched: Option<u64>,
    /// The way it is wading, if any.
    pub way: Option<WadeWay>,
    /// Its last pocket reading: the tick, whether it was a pocket, and the faces it walked
    /// (all of them for a pocket; those found before it reached the window's edge otherwise).
    /// Reused while it stands on one of them, until `teachers.route_refresh_s`.
    pub read: Option<(u64, bool, FxHashSet<Site>)>,
}

/// A held body's way out ([`Wade`]): the faces from where it set out to the goal, the deepest
/// water on them (its motor's wade limit while it walks it), and the tick it gives the way up.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WadeWay {
    pub faces: Vec<Site>,
    pub wade_m: f64,
    pub until: u64,
}

/// The way's time, in multiples of walking it at cruise, and the margin added: a bound
/// against a way the motor cannot finish, not a knob.
const WADE_TIME_FACTOR: f64 = 4.0;
const WADE_TIME_MARGIN_S: f64 = 10.0;

/// **The wade-out reflex** for the due body at `i` ([`Wade`]): `None` unless it is held, has
/// been for its lineage's `drop.after_s`, and has (or now finds) a way out; then the walk to
/// the way's next face.
pub(crate) fn wade_reflex(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let grounded = a.mobility.mode == crate::MobilityMode::Ground
        && a.mobility.climb.is_none()
        && a.attached.is_none()
        && phys.flight.is_none()
        && !phys.aquatic;
    if !grounded || !phys.wade.swim_out {
        fauna.wading.remove(&a.id);
        return None;
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let now = fauna.tick;
    let aim = |a: &crate::Animal, f: Site| {
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, (f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v, ring);
        super::glider::walk_toward(a, dx, dz, 0.0)
    };
    // On its way: the next face, until the goal, off the way, or out of time.
    if let Some(way) = fauna.wading.get(&a.id).and_then(|w| w.way.clone()) {
        match way.faces.iter().position(|&f| f == a.site) {
            Some(k) if now < way.until && k + 1 < way.faces.len() => return Some(aim(&a, way.faces[k + 1])),
            Some(k) if k + 1 == way.faces.len() => {
                fauna.wading.remove(&a.id);
                fauna.wade_outs[founder.index()] += 1;
                return None;
            }
            // Off the way (a corner cut onto the next column) it looks again now; out of time,
            // at the next refresh.
            off => {
                if let Some(w) = fauna.wading.get_mut(&a.id) {
                    w.way = None;
                    if off.is_none() {
                        w.searched = None;
                    }
                }
            }
        }
    }
    let tc = fauna.config.teachers;
    let body = phys.body_at(a.body);
    let wade = phys.wade_depth_m(&body);
    let graph = fauna.routes(view, founder, tc.route_refresh_s);
    let r = body.footprint_radius();
    let own = (r / v).ceil().max(0.0) as u32;
    let headroom = body.headroom_voxels(v);
    let step = phys.step_limits(v);
    let cols = if tc.way_on_m.is_finite() && tc.way_on_m > 0.0 { (tc.way_on_m / v).ceil() as u32 } else { 0 };
    let side = 2 * cols as usize + 1;
    let open_min = side * side;
    let every = cubarium_voxel::ticks_at_least_one(tc.route_refresh_s);
    let water = |f: Site| view.water_depth_m(i64::from(f.x), f.y, f.z);
    let here = water(a.site);
    let deep = here > wade;
    // Under water it drowns in, it does not wait out the shallows reflex's time.
    let under = here > phys.drown_depth_m(&body);
    // A body smaller than the graph's adult wades shallower water: its own pockets.
    let small = wade < phys.wade_depth_m(&phys.adult_body());
    let beside = || {
        [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dz)| {
            let nz = i64::from(a.site.z) + dz;
            if nz < 0 || nz >= i64::from(view.config.depth) {
                return false;
            }
            let nx = (i64::from(a.site.x) + dx).rem_euclid(i64::from(view.config.width));
            view.nearest_support(nx, nz as u32, a.site.y, step.down, step.up)
                .is_some_and(|ny| view.water_depth_m(nx, ny, nz as u32) > wade)
        })
    };
    let pocket = if deep {
        false
    } else if small {
        let cached = fauna.wading.get(&a.id).and_then(|w| w.read.as_ref()).and_then(|(t, closed, faces)| {
            (now.saturating_sub(*t) < every && faces.contains(&a.site)).then_some(*closed)
        });
        match cached {
            Some(closed) => closed,
            None if beside() => {
                let (closed, faces) = graph.wade_pocket(view, a.site, r, headroom, step, wade, cols);
                let w = fauna.wading.entry(a.id).or_insert(Wade { since: None, searched: None, way: None, read: None });
                w.read = Some((now, closed, faces.into_iter().collect()));
                closed
            }
            None => false,
        }
    } else {
        graph.node_near(a.site, (a.pose.x, a.pose.z), own).is_some_and(|n| graph.component_len(n) < open_min)
    };
    if !(deep || pocket) {
        // Not held: keep only a pocket reading worth reusing.
        match fauna.wading.get_mut(&a.id) {
            Some(w) if w.read.is_some() => {
                w.since = None;
                w.searched = None;
            }
            Some(_) => {
                fauna.wading.remove(&a.id);
            }
            None => {}
        }
        return None;
    }
    let w = fauna.wading.entry(a.id).or_insert(Wade { since: None, searched: None, way: None, read: None });
    let since = *w.since.get_or_insert(now);
    if !under && (now.saturating_sub(since) as f64) * cubarium_voxel::DT < phys.drop.after_s {
        return None;
    }
    if w.searched.is_some_and(|t| now.saturating_sub(t) < every) {
        return None;
    }
    w.searched = Some(now);
    let reach = fauna.config.life.shallows_search_m;
    let reach_cols = if reach.is_finite() && reach > 0.0 { (reach / v).ceil() as u32 } else { 0 };
    // The faces under it may cross: the way's time (`WADE_TIME_FACTOR` × its walk at cruise)
    // under water must end before the drown clock it has left runs out.
    let cruise = a_cruise(&phys);
    let left_s = phys.drown_ticks().saturating_sub(a.mobility.submerged_ticks) as f64 * cubarium_voxel::DT;
    let max_under = (left_s * cruise / (WADE_TIME_FACTOR * v)).floor().clamp(0.0, f64::from(u32::MAX)) as u32;
    // The goal stands in water a level step shallower than it wades
    // (`body::LEVEL_TOLERANCE_M`): a flowing sheet's depth moves by millimetres, and a goal on
    // the very edge of its depth is out of it again on arrival. For a smaller body, the goal
    // lies outside its own pocket and any other it finds at that depth.
    let goal_wade = if wade > crate::body::LEVEL_TOLERANCE_M { wade - crate::body::LEVEL_TOLERANCE_M } else { wade };
    let mut closed: FxHashSet<Site> = match fauna.wading.get(&a.id).and_then(|w| w.read.as_ref()) {
        Some((_, true, faces)) => faces.clone(),
        _ => FxHashSet::default(),
    };
    let mut accept = |f: Site| {
        if !small {
            return true;
        }
        if closed.contains(&f) {
            return false;
        }
        let (shut, faces) = graph.wade_pocket(view, f, r, headroom, step, goal_wade, cols);
        if shut {
            closed.extend(faces);
        }
        !shut
    };
    let (faces, wade_m, _) = graph.wade_way(view, a.site, r, headroom, step, (goal_wade, phys.drown_depth_m(&body), max_under), own, open_min, reach_cols, &mut accept)?;
    let walk_s = (faces.len() as f64) * v / cruise;
    let until = now + cubarium_voxel::ticks_at_least_one(WADE_TIME_FACTOR * walk_s + WADE_TIME_MARGIN_S);
    let next = faces[1];
    if let Some(w) = fauna.wading.get_mut(&a.id) {
        w.way = Some(WadeWay { faces, wade_m: wade_m.max(wade), until });
    }
    Some(aim(&a, next))
}

/// The lineage's walking cruise, m/s (never zero).
fn a_cruise(phys: &FounderPhysiology) -> f64 {
    let c = phys.cruise_body_lengths_per_s * phys.adult_length_m;
    if c.is_finite() && c > 0.0 { c } else { 1.0 }
}
