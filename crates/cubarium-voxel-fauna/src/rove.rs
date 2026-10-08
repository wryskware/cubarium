//! **HUNGRY-SEARCH: a hungry forager with nothing in sense goes looking** (area-restricted
//! search near the last food, then ranging: a correlated walk with long straight legs away
//! from ground already covered).
//!
//! The foraging heuristics' no-signal searches are local by design: the browser's and the
//! fruit forager's wander swings its heading back and forth every few samples, the blind
//! forager's correlated walk turns every few seconds, and a scent with no gradient (an even
//! fungal field) holds one heading until a wall turns the body back. In a mature world
//! hungry bodies paced so for minutes (STUCK-AUDIT, JUVENILE-FOOD, 2026-10-08: 100–1,800
//! body lengths of path for under one of net way in five minutes; capgnawer juveniles
//! shuttling between two walls in an even scent), while food stood within reach of a body
//! that travelled.
//!
//! The rule, over those heuristics only (a trained policy behaves only through the world;
//! [`crate::Controller::seek`] says what each answer was for):
//!
//! - **Memory.** At every controller sample the body marks the ground it stands on: a cell
//!   `search_cell_bl` of its current lengths across, weighted by the time spent there and
//!   fading with half-life `search_memory_half_life_s`. Its own memory of its own path
//!   (session state, not saved), as any animal remembers where it has been.
//! - **Patience.** For `search_patience_s` after its last taste of food while it wants food
//!   (its eating or emergency latch on), the heuristic's own local search is the search
//!   (area-restricted search near where food was). Food at its mouth, or the latch letting
//!   go, starts the patience over. A lead it steers by (food in view, a scent rising or
//!   falling) is followed as before but does not: one that never reaches a bite is no food
//!   (capgnawer juveniles shuttled between two walls in a fungal scent, rising one way and
//!   falling the other, for minutes).
//! - **Legs.** After that, at each sample its heuristic spends on its own search (nothing to
//!   eat in sense, nothing in the way, no lead) it is on a leg: of evenly spread headings starting from the
//!   one it faces, the one with the least remembered ground ahead of it (nearer ground
//!   counting more; ties to the straightest), held until it has made `search_leg_bl` of its
//!   lengths of net way or twice the time that takes at cruise. Its heuristic's pace is
//!   kept (a flyer's lift too: it searches in flight).
//! - **In the way.** A sample its heuristic spends on something in its way (an escape from
//!   a refused motor or a drop, a blocked turn) is left to it, as the wall rules are; a leg
//!   it interrupts ends, the ground just ahead joins the memory as covered, and the next
//!   leg turns at least square to the one that ran into it. A later layer that overrides
//!   a leg's answer (the flee rule, a reflex walk to water, a roost or fruit, a drop) ends
//!   it the same way, so a leg never tugs against it.
//! - Legs steer a body walking, flying or gliding; on a terrain face, a stem or a perch its
//!   own climb or take-off runs.
//! - `search_leg_bl` 0 turns the rule off.

use crate::controller::{Actions, Seek};
use crate::Fauna;
use cubarium_voxel::DT;

/// The most cells a body remembers: a memory bound, not a knob; the faintest goes first.
pub(crate) const ROVE_CELLS: usize = 48;

/// The headings a leg is chosen from, evenly spread: a resolution, not a knob.
const HEADINGS: usize = 16;

/// One body's search record (session state, not saved).
#[derive(Clone, Debug, Default)]
pub(crate) struct Rove {
    cells: Vec<Cell>,
    /// The tick its answers began to be its own search while it wanted food.
    quiet_since: Option<u64>,
    leg: Option<Leg>,
    /// The heading of the leg something in the way ended: the next leg turns at least
    /// square to it (the obstacle's side is closed), once.
    blocked: Option<f64>,
}

/// A remembered cell: its centre (x, z metres), its weight (seconds, faded to `at`).
#[derive(Clone, Copy, Debug)]
struct Cell {
    x: f64,
    z: f64,
    w: f64,
    at: u64,
}

#[derive(Clone, Copy, Debug)]
struct Leg {
    heading: f64,
    from: (f64, f64),
    until: u64,
}

/// `w` faded from `at` to `now` at `half_ticks`.
fn faded(w: f64, at: u64, now: u64, half_ticks: f64) -> f64 {
    if half_ticks > 0.0 {
        w * 0.5f64.powf(now.saturating_sub(at) as f64 / half_ticks)
    } else {
        0.0
    }
}

impl Rove {
    /// Mark `here` as covered: the nearest cell within half `cell_m` (else a new one, the
    /// faintest cell making room) gains `add` seconds and weighs at least `floor`.
    #[allow(clippy::too_many_arguments)]
    fn mark(&mut self, here: (f64, f64), cell_m: f64, add: f64, floor: f64, now: u64, half_ticks: f64, ring: f64) {
        let mut near: Option<(usize, f64)> = None;
        for (k, c) in self.cells.iter().enumerate() {
            let (dx, dz) = crate::pose::wrapped_delta(c.x, c.z, here.0, here.1, ring);
            let d = dx.hypot(dz);
            if d <= 0.5 * cell_m && near.is_none_or(|(_, b)| d < b) {
                near = Some((k, d));
            }
        }
        match near {
            Some((k, _)) => {
                let c = &mut self.cells[k];
                c.w = (faded(c.w, c.at, now, half_ticks) + add).max(floor);
                c.at = now;
            }
            None => {
                if self.cells.len() >= ROVE_CELLS {
                    let faintest = (0..self.cells.len())
                        .min_by(|&a, &b| {
                            let (ca, cb) = (self.cells[a], self.cells[b]);
                            faded(ca.w, ca.at, now, half_ticks).total_cmp(&faded(cb.w, cb.at, now, half_ticks))
                        })
                        .unwrap_or(0);
                    self.cells.swap_remove(faintest);
                }
                self.cells.push(Cell { x: here.0, z: here.1, w: add.max(floor), at: now });
            }
        }
    }

    /// Of the evenly spread headings starting at `facing`, the one with the least
    /// remembered ground ahead: each cell weighs its faded time, by how squarely it lies
    /// ahead (the cosine: ground behind counts against) and how near it is
    /// (`1 / (1 + d / leg_m)`). So it heads away from what it has covered, nearer ground
    /// counting more; with nothing remembered, ties go to the straightest. After an
    /// obstacle (`blocked`, the heading it ended) only headings square to it or turned away
    /// from it are taken.
    #[allow(clippy::too_many_arguments)]
    fn least_covered(&self, here: (f64, f64), facing: f64, blocked: Option<f64>, leg_m: f64, now: u64, half_ticks: f64, ring: f64) -> f64 {
        let mut best = (f64::INFINITY, facing);
        for k in 0..HEADINGS {
            // Straight on first, then alternately right and left, widening.
            let step = k.div_ceil(2) as f64 * std::f64::consts::TAU / HEADINGS as f64;
            let h = if k % 2 == 1 { facing + step } else { facing - step };
            // The obstacle's side is closed: only headings square to it or away from it.
            if blocked.is_some_and(|b| (h - b).cos() > 1e-9) {
                continue;
            }
            let (fx, fz) = (h.sin(), h.cos());
            let mut score = 0.0;
            for c in &self.cells {
                let (dx, dz) = crate::pose::wrapped_delta(here.0, here.1, c.x, c.z, ring);
                let d = dx.hypot(dz);
                if d <= f64::EPSILON {
                    continue;
                }
                let ahead = (fx * dx + fz * dz) / d;
                score += faded(c.w, c.at, now, half_ticks) * ahead / (1.0 + d / leg_m);
            }
            if score < best.0 * (1.0 - 1e-9) {
                best = (score, h);
            }
        }
        best.1.rem_euclid(std::f64::consts::TAU)
    }
}

/// **The hungry search** at body `i`'s controller sample: `actions` is what its heuristic
/// (and nothing yet over it) asked, and `seek` what that answer was for. `Some` replaces
/// the actions: the leg's heading at the heuristic's own pace. `period_s` is the body's
/// controller period, `ring` the world's x circumference, metres.
pub(crate) fn search(fauna: &mut Fauna, i: usize, actions: Actions, seek: Seek, period_s: f64, ring: f64) -> Option<Actions> {
    let tc = fauna.config.teachers;
    let a = fauna.animals[i];
    let id = a.id;
    if !(tc.search_leg_bl > 0.0) || seek == Seek::Unknown && !fauna.rove.contains_key(&id) {
        fauna.rove.remove(&id);
        return None;
    }
    let founder = a.founder?;
    let length = crate::body_of(&fauna.config, &a).map_or(0.0, |b| b.length_m);
    if !(length > 0.0) {
        return None;
    }
    let now = fauna.tick;
    let here = (a.pose.x, a.pose.z);
    let half_ticks = tc.search_memory_half_life_s.max(0.0) / DT;
    let leg_m = tc.search_leg_bl * length;
    let r = fauna.rove.entry(id).or_default();
    let cell_m = tc.search_cell_bl.max(0.0) * length;
    r.mark(here, cell_m, period_s, 0.0, now, half_ticks, ring);
    if !(a.eating || a.emergency) {
        r.quiet_since = None;
        r.leg = None;
        r.blocked = None;
        return None;
    }
    // A heading means something walking, flying or gliding; on a face, a stem or a perch the
    // body's own climb or take-off runs (a seedporter clinging to a stem is not searching).
    let steers = matches!(a.mobility.mode, crate::MobilityMode::Ground | crate::MobilityMode::Fly | crate::MobilityMode::Glide);
    match seek {
        Seek::Unknown => None,
        Seek::Search if !steers => None,
        Seek::Taste => {
            r.quiet_since = None;
            r.leg = None;
            r.blocked = None;
            None
        }
        // A lead is followed as before; the patience runs on, as a lead that never
        // reaches a bite is no food (the capgnawer shuttling in a scent).
        Seek::Lead => {
            r.quiet_since.get_or_insert(now);
            None
        }
        Seek::Obstacle => {
            // The leg it was on ran into something: the ground just ahead counts as
            // covered, as much as the most covered cell it remembers.
            if let Some(leg) = r.leg.take() {
                r.blocked = Some(leg.heading);
                let (fx, fz) = a.pose.forward();
                let most = r.cells.iter().map(|c| faded(c.w, c.at, now, half_ticks)).fold(period_s, f64::max);
                r.mark((here.0 + fx * cell_m, here.1 + fz * cell_m), cell_m, 0.0, most, now, half_ticks, ring);
            }
            None
        }
        Seek::Search => {
            let since = *r.quiet_since.get_or_insert(now);
            if (now.saturating_sub(since) as f64) * DT < tc.search_patience_s {
                return None;
            }
            let lapsed = r.leg.is_none_or(|leg| {
                let (dx, dz) = crate::pose::wrapped_delta(leg.from.0, leg.from.1, here.0, here.1, ring);
                now >= leg.until || dx.hypot(dz) >= leg_m
            });
            if lapsed {
                let blocked = r.blocked.take();
                let heading = r.least_covered(here, a.pose.heading_rad, blocked, leg_m, now, half_ticks, ring);
                let cruise = fauna.config.founder(founder).cruise_body_lengths_per_s;
                let leg_s = if cruise > 0.0 { 2.0 * tc.search_leg_bl / cruise } else { 0.0 };
                let until = now.saturating_add(cubarium_voxel::ticks_at_least_one(leg_s.max(DT)));
                r.leg = Some(Leg { heading, from: here, until });
            }
            let heading = r.leg.map_or(a.pose.heading_rad, |leg| leg.heading);
            let mut go = super::glider::walk_toward(&a, heading.sin(), heading.cos(), actions.feed);
            go.forward = if go.forward > 0.0 { actions.forward } else { 0.0 };
            Some(go)
        }
    }
}

/// A later layer (a flight, a reflex walk: to water, to a roost, off a ledge) replaced the
/// leg's answer at body `id`'s sample: the leg ends as if something were in its way, and the
/// next one turns at least square to it — so a leg never tugs against the layer that
/// overrode it.
pub(crate) fn overridden(fauna: &mut Fauna, id: u64) {
    if let Some(r) = fauna.rove.get_mut(&id)
        && let Some(leg) = r.leg.take()
    {
        r.blocked = Some(leg.heading);
    }
}

impl Fauna {
    /// HUNGRY-SEARCH's readout: what a body's controller said its last answer was for
    /// ([`crate::Controller::seek`]).
    pub fn seek_of(&self, id: u64) -> Seek {
        self.controllers.seek(id)
    }

    /// HUNGRY-SEARCH: the leg a body is on, `(heading radians, tick it lapses)`, if any.
    pub fn search_leg(&self, id: u64) -> Option<(f64, u64)> {
        self.rove.get(&id).and_then(|r| r.leg).map(|leg| (leg.heading, leg.until))
    }
}

#[cfg(test)]
#[path = "rove_tests.rs"]
mod rove_tests;
