//! **FLUSH** (HUNT-ESCAPE; Wrysk 2026-10-10, role call 8, option B: keep catching perched
//! bellwings, but usually miss — "bellwing would be able to react very quickly, so catching
//! one shouldnt be all that easy"): a flyer's own escape from a ground hunter it senses
//! coming. A body reflex under every controller, run in the flyer's tick, so its timing is
//! the world's, not its controller period's.
//!
//! - **What it senses** is its looming sense (`senses.loom_m`, no form vision): a body on
//!   whose prey list it is ([`crate::is_prey_of`], the perception's threat class) whose
//!   **strike** has begun — the crouch and the lunge, the jaw rising at it within its strike
//!   band — with the jaw within `loom_m × hunter.strike_warning_scale` of its eye;
//!   a short arm grab gives less visual warning than a whole-body lunge. Or it is **closing** within
//!   `loom_m` fast enough to reach it within [`crate::LightConfig::loom_tau_s`] (the loom's
//!   own rule, measured to the body's middle).
//! - **The reaction**: `flush_reaction_s` after, it takes off (a perched one pays its
//!   takeoff as any takeoff is paid) and **bursts** for `flush_burst_s` at
//!   `flush_speed_factor` × its cruise, sideways off the threat's line of travel (its
//!   heading) and rising at `flush_climb_fraction` of the burst's speed. Whether the snap
//!   closes on it or on air is the strike's own capsule test, made where the flyer is when
//!   the lunge ends: reaction and burst against the striker's windup, lunge and reach.
//! - **After**: until `flush_s` after the takeoff it flies on along its escape heading,
//!   climbing, neither sipping nor choosing to land; then its controller has it back.
//! - **No flush** when it is asleep, falling, grounded (rain or its rest hour: torpid), or
//!   cannot pay the takeoff; a burst that runs into a face ends there (hemmed in). It
//!   senses nothing beyond `loom_m`, and nothing still and not striking.
//! - **The lure keeps working**: a flyer drawn to a lure ([`crate::light::drawn_to_lure`])
//!   reads it as a flower and does not track the lit lanternjaw behind it, and a strike
//!   that began on a lured flyer ([`crate::Strike::lure`]) is not sensed by it.

use crate::predation::StrikePhase;
use crate::{Animal, Fauna, Founder};

/// One flyer's escape, kept in [`Fauna::flushes`] (session state).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Flush {
    /// The threat it flushed from.
    pub from: u64,
    /// Where the threat was and its heading when sensed: the line it escapes off when the
    /// threat is gone by the takeoff.
    pub from_x: f64,
    pub from_z: f64,
    pub from_heading: f64,
    /// The tick its burst begins (after the reaction), the tick the burst ends, and the
    /// tick it may come down again.
    pub go: u64,
    pub burst_until: u64,
    pub until: u64,
    /// Its escape heading, set when the burst begins.
    pub heading: Option<f64>,
}

/// What a flushing flyer does this tick ([`step`]): its heading, its speed (× cruise) and
/// its climb (× cruise).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Escape {
    pub heading: f64,
    pub speed: f64,
    pub climb: f64,
    /// The burst's first tick: a perched body takes off now.
    pub takeoff: bool,
}

fn ticks(s: f64) -> u64 {
    if s.is_finite() && s > 0.0 { (s / cubarium_voxel::DT).round().max(1.0) as u64 } else { 0 }
}

/// The nearest body among `hunters` (indices into the animals) that the flyer `a` senses
/// looming at it now (module note), `None` when there is none.
pub(crate) fn sensed_threat(fauna: &Fauna, a: &Animal, hunters: &[usize], v: f64, ring: f64) -> Option<usize> {
    let founder = a.founder?;
    let table = fauna.config.founder(founder).senses;
    if !(table.loom_m > 0.0) {
        return None;
    }
    let eye = crate::light::eye_of(&fauna.config, a, v)?;
    let lured = crate::light::drawn_to_lure(fauna, a.id);
    let tau = fauna.config.light.loom_tau_s;
    let mut best: Option<(f64, usize)> = None;
    for &k in hunters {
        let o = &fauna.animals[k];
        if o.id == a.id || o.attached.is_some() || o.founder == Some(founder) {
            continue;
        }
        let Some(og) = crate::body_of(&fauna.config, o) else { continue };
        let (dx, dz) = crate::pose::wrapped_delta(eye.0, eye.2, o.pose.x, o.pose.z, ring);
        let span = table.loom_m + og.length_m;
        if dx.abs() > span || dz.abs() > span {
            continue;
        }
        if !crate::is_prey_of(&fauna.config, o, a) {
            continue;
        }
        // The lure: the lit lanternjaw behind the flower it is drawn to is no threat to it.
        if lured && o.founder_state.lure > 0.0 {
            continue;
        }
        let striking = o.strike.filter(|s| s.phase != StrikePhase::Recovery);
        let d = match striking {
            Some(s) if s.prey == a.id && s.lure => continue,
            Some(_) => {
                // The strike looms from its start: the jaw, within its band, nearest the eye.
                let Some(h) = fauna.config.founder(o.founder?).active_hunter() else { continue };
                let jaw = crate::step::jaw_of(o.pose.x, o.pose.z, o.pose.heading_rad, og.length_m, h.jaw_forward_length_fraction);
                let (jx, jz) = crate::pose::wrapped_delta(eye.0, eye.2, jaw.0, jaw.1, ring);
                let s0 = crate::perception::base_m(o, v);
                let dy = eye.1.clamp(s0, s0 + h.strike_height_m(&og)) - eye.1;
                let d = (jx * jx + dy * dy + jz * jz).sqrt();
                if !(h.strike_warning_scale > 0.0) || d > table.loom_m * h.strike_warning_scale {
                    continue;
                }
                d
            }
            None => {
                if !crate::moved_recently(&fauna.config.predation, o) {
                    continue;
                }
                let dy = crate::perception::base_m(o, v) + 0.5 * og.height_m - eye.1;
                let d = (dx * dx + dy * dy + dz * dz).sqrt();
                let h = (dx * dx + dz * dz).sqrt();
                let (vx, vz) = (o.speed_m_s * o.pose.heading_rad.sin(), o.speed_m_s * o.pose.heading_rad.cos());
                let toward = if h > 0.0 { -(vx * dx + vz * dz) / h } else { 0.0 };
                if !(toward > 0.0) || (tau > 0.0 && d >= tau * toward) {
                    continue;
                }
                d
            }
        };
        if d <= table.loom_m && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, k));
        }
    }
    best.map(|(_, k)| k)
}

/// The heading off a threat's line of travel: square to its heading, on the flyer's side of
/// the line (the flyer's id picks a side when it is on the line itself).
fn escape_heading(a: &Animal, tx: f64, tz: f64, theading: f64, ring: f64) -> f64 {
    let (hx, hz) = (theading.sin(), theading.cos());
    let (dx, dz) = crate::pose::wrapped_delta(tx, tz, a.pose.x, a.pose.z, ring);
    let along = dx * hx + dz * hz;
    let (px, pz) = (dx - along * hx, dz - along * hz);
    let side = if px * px + pz * pz > 1e-12 {
        // Which side of the heading the flyer is on: +1 its right (x of (hz, -hx)).
        if px * hz - pz * hx >= 0.0 { 1.0 } else { -1.0 }
    } else if a.id % 2 == 0 {
        1.0
    } else {
        -1.0
    };
    // Right of a heading θ (x = sin, z = cos) is θ + π/2.
    (theading + side * std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::TAU)
}

/// **The flush's tick** for the flyer at `i` (module note): `calm` when it cannot flush
/// now (asleep, falling, grounded). `None` leaves the flyer to its controller.
pub(crate) fn step(fauna: &mut Fauna, i: usize, flight: &crate::Flight, hunters: &[usize], calm: bool, v: f64, ring: f64) -> Option<Escape> {
    let a = fauna.animals[i];
    let founder: Founder = a.founder?;
    if calm || !(flight.flush_speed_factor > 0.0) || !fauna.config.founder(founder).senses.live || a.attached.is_some() {
        fauna.flushes.remove(&a.id);
        return None;
    }
    let now = fauna.tick;
    if fauna.flushes.get(&a.id).is_some_and(|f| now >= f.until) {
        fauna.flushes.remove(&a.id);
    }
    if !fauna.flushes.contains_key(&a.id) {
        let k = sensed_threat(fauna, &a, hunters, v, ring)?;
        let o = fauna.animals[k];
        let go = now + ticks(flight.flush_reaction_s);
        let burst_until = go + ticks(flight.flush_burst_s).max(1);
        let until = (go + ticks(flight.flush_s)).max(burst_until);
        fauna.flushes.insert(
            a.id,
            Flush { from: o.id, from_x: o.pose.x, from_z: o.pose.z, from_heading: o.pose.heading_rad, go, burst_until, until, heading: None },
        );
    }
    let f = *fauna.flushes.get(&a.id)?;
    if now < f.go {
        return None;
    }
    let (heading, takeoff) = match f.heading {
        Some(h) => (h, false),
        None => {
            // The burst begins: off the threat's line as it is now (or as it was sensed).
            let (tx, tz, th) = fauna
                .animals
                .binary_search_by_key(&f.from, |o| o.id)
                .ok()
                .map(|j| fauna.animals[j])
                .map_or((f.from_x, f.from_z, f.from_heading), |o| (o.pose.x, o.pose.z, o.pose.heading_rad));
            let h = escape_heading(&a, tx, tz, th, ring);
            if let Some(e) = fauna.flushes.get_mut(&a.id) {
                e.heading = Some(h);
            }
            fauna.ledger.flushes_by_founder[founder.index()] += 1;
            (h, true)
        }
    };
    Some(if now < f.burst_until {
        Escape { heading, speed: flight.flush_speed_factor, climb: flight.flush_climb_fraction * flight.flush_speed_factor, takeoff }
    } else {
        Escape { heading, speed: 1.0, climb: flight.lift_speed_fraction, takeoff }
    })
}

/// Whether the body `id` has burst from a threat and is still in its flush (the strike's
/// escape readout).
pub(crate) fn fled(fauna: &Fauna, id: u64) -> bool {
    fauna.flushes.get(&id).is_some_and(|f| f.heading.is_some())
}
