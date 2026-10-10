//! **FORAGE-LEAVE: a forager leaves a depleted patch** (giving-up time / giving-up density,
//! the marginal value theorem's coarse form).
//!
//! The heuristics that stop to feed — the blind forager (littershredder, capgnawer,
//! ripple-snail), the seedporter's fruit forager and the bellwing's hover-sip — stayed while
//! their mouth tasted anything over `feed_gate`. Taste saturates, so a crown holding a
//! thousandth of its fruit tasted like food, and a hungry seedporter nibbled it for tens of
//! minutes (LOAD-CLIMB, 2026-10-08). The grazing browsers (frondgrazer, loftstrider) stop
//! to feed too since JUVENILE-FOOD; the lanternjaw's patrol has its own fruitless-patch
//! memory.
//!
//! The rule, over those heuristics only (a trained policy behaves only through the world):
//!
//! - **What the body knows.** At every bite the body compares what the patch offered its
//!   mouth (the gut's bite before the stomach's room caps it, `want · E / (E + K)`) with the
//!   intake that fills its stomach against digestion (`full_tolerance · K_stomach / τ` of
//!   that food over the bite's period) — its own gut and its own mouth, no world query.
//!   Below 1, the patch cannot end a meal: the stomach settles under the full line and the
//!   eating latch never lets go, however long it stays.
//! - **A stay** starts when the body, wanting food, holds still to feed; it lasts while it
//!   keeps doing so within `give_up_zone_bl` body lengths of where it began. Each controller
//!   sample folds the interval's mean offer (zero when nothing could be bitten) into a
//!   trailing average of time constant `give_up_window_s`.
//! - **Giving up.** After `give_up_min_s` on the patch, a trailing offer under
//!   `give_up_fraction` marks the patch depleted for `give_up_shun_s`: inside its zone the
//!   body's stop-to-feed answer becomes a walk (glide, climb, flight) on without feeding —
//!   for the first `give_up_leave_s` any feeding answer does, held straight on, so a
//!   heuristic that turns to the food it sees does not turn straight back — and the stand it fed from (a seedporter's crown) joins the reflex walks' shun
//!   list, so the fruit search takes the next one. Everything else is the body's normal
//!   search; the flee rule and the other reflexes after it still win.
//! - `give_up_fraction` 0 turns the rule off; the stays are still watched for the readout
//!   ([`ForageTally`]).
//! - An interval that a later layer (a flight, a reflex walk) spent elsewhere bit nothing
//!   on the patch and is not counted against it.

use crate::controller::Actions;
use crate::{Fauna, Food, Founder, FounderPhysiology};
use cubarium_voxel::DT;

/// One body's patch record (session state, not saved).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Patch {
    /// The stay under way: where it began (x, z, y metres), the tick it began and the
    /// trailing offer (`None` until its first whole interval).
    stay: Option<Stay>,
    /// The patch given up: its centre and the tick it may be fed on again.
    shun: Option<((f64, f64, f64), u64)>,
    /// The tick the walk off a patch just given up ends (`give_up_leave_s`).
    leave_until: u64,
    /// This interval's bites: the sum of their offers and their count, and the stand the
    /// last one came from.
    offer: f64,
    bites: u32,
    key: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
struct Stay {
    at: (f64, f64, f64),
    since: u64,
    rate: Option<f64>,
    key: Option<u64>,
}

/// **The forage readout** per lineage (session state for probes and studies, not saved):
/// the patch stays the rule watched, their offers and the give-ups.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ForageTally {
    /// Stays ended (a give-up, a meal finished, a move or a flight).
    pub stays: u64,
    /// Controller samples spent in a stay, ended or not, and the seconds they cover.
    pub stay_samples: u64,
    pub stay_s: f64,
    /// The longest ended stay, seconds.
    pub stay_max_s: f64,
    /// The interval offers sampled in stays (fractions of the fill rate): how many, their
    /// sum, and how many were under `give_up_fraction`.
    pub offer_samples: u64,
    pub offer_sum: f64,
    pub offer_low: u64,
    /// Patches given up, and controller samples the shun walked the body off one.
    pub give_ups: u64,
    pub leave_samples: u64,
}

/// The offer at one bite: the patch's bite before the stomach's room caps it, over the
/// intake that holds the stomach at its full line against digestion of `food` — both over
/// the controller period. Zero when the gut has no rate.
pub(crate) fn offer(a: &crate::Animal, phys: &FounderPhysiology, food: Food, period_s: f64, effort: f64, stock: f64) -> f64 {
    let speed = phys.gut.throughput_factor(a.body, phys.core.body_max);
    let want = phys.gut.bite_per_body_s * speed * a.body * period_s * effort;
    let k = phys.bite_half_stock_for(food);
    let offered = if k > 0.0 { want * stock.max(0.0) / (stock.max(0.0) + k) } else { want };
    let tau = phys.gut.digestion_s[food.index()] / speed;
    let fill = phys.gut.full_tolerance * phys.gut.capacity(a.body) * period_s / tau;
    if fill > 0.0 && fill.is_finite() && offered.is_finite() { offered / fill } else { 0.0 }
}

/// A bite taken by body `i` (or tried: `stock` may be zero): its offer joins the interval's,
/// when the body has a patch record. `key` is the stand it came from, if any.
#[allow(clippy::too_many_arguments)]
pub(crate) fn note_bite(fauna: &mut Fauna, i: usize, phys: &FounderPhysiology, food: Food, period_s: f64, effort: f64, stock: f64, key: Option<u64>) {
    let a = &fauna.animals[i];
    if !a.eating {
        return;
    }
    let q = offer(a, phys, food, period_s, effort, stock);
    if let Some(p) = fauna.forage.get_mut(&a.id) {
        p.offer += q;
        p.bites += 1;
        if key.is_some() {
            p.key = key;
        }
    }
}

/// **The giving-up layer**, at body `i`'s controller sample: `actions` is what its own
/// heuristic (and the reflexes before this one) asked. `Some` replaces them: the walk off
/// a patch given up. `ring` is the world's x circumference, metres.
pub(crate) fn leave(fauna: &mut Fauna, i: usize, actions: Actions, ring: f64) -> Option<Actions> {
    let tc = fauna.config.teachers;
    let a = fauna.animals[i];
    let founder = a.founder?;
    let id = a.id;
    let now = fauna.tick;
    let fraction = fauna.config.founder(founder).give_up_fraction.unwrap_or(tc.give_up_fraction);
    let on = fraction > 0.0;
    // A stop to feed: still, not turning (an escape's stop turns away from what refused it,
    // and is never walked on), feeding.
    let stopping = actions.feed > 0.5 && actions.forward <= tc.forage_stay_forward && actions.turn == 0.0;
    if !fauna.forage.contains_key(&id) && !(stopping && a.eating) {
        return None;
    }
    // Whether the interval just ended was spent trying to feed where it stood: one that a
    // later layer turned into a flight or a walk bit nothing here, and says nothing of it.
    let prev = a.founder_state.held;
    let tried = prev.feed > 0.5 && prev.forward <= tc.forage_stay_forward && prev.turn == 0.0;
    let length = crate::body_of(&fauna.config, &a).map_or(0.0, |b| b.length_m);
    let zone = tc.give_up_zone_bl.max(0.0) * length;
    let here = (a.pose.x, a.pose.z, a.pose.y_m);
    let near = |p: (f64, f64, f64)| {
        let (dx, dz) = crate::pose::wrapped_delta(p.0, p.1, here.0, here.1, ring);
        let dy = here.2 - p.2;
        (dx * dx + dz * dz + dy * dy).sqrt() <= zone
    };
    let period_s = founder.manifest().cadence_ticks().max(1) as f64 * DT;
    let ticks = |s: f64| if s.is_finite() && s > 0.0 { cubarium_voxel::ticks_at_least_one(s) } else { 0 };
    let p = fauna.forage.entry(id).or_default();
    // The interval just ended: the mean offer of its bites, zero when it bit nothing.
    let sample = if p.bites > 0 { p.offer / f64::from(p.bites) } else { 0.0 };
    let key = p.key.take();
    p.offer = 0.0;
    p.bites = 0;
    let mut tally = ForageTally::default();
    let end = |stay: Stay, tally: &mut ForageTally| {
        let s = now.saturating_sub(stay.since) as f64 * DT;
        tally.stays += 1;
        tally.stay_max_s = tally.stay_max_s.max(s);
    };
    let mut out = None;
    let mut shun_key = None;
    if let Some((at, until)) = p.shun {
        if now >= until {
            p.shun = None;
        } else if now < p.leave_until && actions.feed > 0.5 && (stopping || actions.forward >= 1.0) {
            // The walk off: straight on, whatever food its heuristic stops at or turns to at
            // full effort. A creep or an escape is its own.
            if let Some(stay) = p.stay.take() {
                end(stay, &mut tally);
            }
            tally.leave_samples += 1;
            out = Some(Actions { forward: 1.0, turn: 0.0, feed: 0.0 });
        } else if stopping && near(at) {
            if let Some(stay) = p.stay.take() {
                end(stay, &mut tally);
            }
            tally.leave_samples += 1;
            out = Some(Actions { forward: 1.0, turn: actions.turn, feed: 0.0 });
        }
    }
    if out.is_none() {
        if !(stopping && a.eating) {
            if let Some(stay) = p.stay.take() {
                end(stay, &mut tally);
            }
        } else {
            let stay = match p.stay {
                Some(s) if near(s.at) && !tried => s,
                Some(s) if near(s.at) => {
                    let w = tc.give_up_window_s;
                    let alpha = if w > 0.0 { (period_s / w).min(1.0) } else { 1.0 };
                    let rate = s.rate.map_or(sample, |r| r + alpha * (sample - r));
                    tally.offer_samples += 1;
                    tally.offer_sum += sample;
                    if sample < fraction {
                        tally.offer_low += 1;
                    }
                    Stay { rate: Some(rate), key: key.or(s.key), ..s }
                }
                old => {
                    if let Some(s) = old {
                        end(s, &mut tally);
                    }
                    // A new stay: the interval before it was not spent here.
                    Stay { at: here, since: now, rate: None, key: None }
                }
            };
            tally.stay_samples += 1;
            tally.stay_s += period_s;
            let on_patch_s = now.saturating_sub(stay.since) as f64 * DT;
            if on && on_patch_s >= tc.give_up_min_s && stay.rate.is_some_and(|r| r < fraction) {
                end(stay, &mut tally);
                p.stay = None;
                p.shun = Some((stay.at, now.saturating_add(ticks(tc.give_up_shun_s))));
                p.leave_until = now.saturating_add(ticks(tc.give_up_leave_s));
                tally.give_ups += 1;
                shun_key = stay.key.map(|k| (k, now.saturating_add(ticks(tc.give_up_shun_s))));
                out = Some(Actions { forward: 1.0, turn: actions.turn, feed: 0.0 });
            } else {
                p.stay = Some(stay);
            }
        }
    }
    if p.stay.is_none() && p.shun.is_none() {
        fauna.forage.remove(&id);
    }
    if let Some((k, until)) = shun_key {
        fauna.shun_target(id, k, until);
    }
    let t = &mut fauna.forage_tally[founder.index()];
    t.stays += tally.stays;
    t.stay_samples += tally.stay_samples;
    t.stay_s += tally.stay_s;
    t.stay_max_s = t.stay_max_s.max(tally.stay_max_s);
    t.offer_samples += tally.offer_samples;
    t.offer_sum += tally.offer_sum;
    t.offer_low += tally.offer_low;
    t.give_ups += tally.give_ups;
    t.leave_samples += tally.leave_samples;
    out
}

impl Fauna {
    /// FORAGE-LEAVE's readout per lineage ([`ForageTally`]), indexed by [`Founder::index`].
    pub fn forage_tally(&self) -> [ForageTally; Founder::COUNT] {
        self.forage_tally
    }

    /// FORAGE-LEAVE: the stays under way, `(id, seconds on the patch so far, trailing offer)`.
    pub fn forage_stays(&self) -> Vec<(u64, f64, Option<f64>)> {
        let mut v: Vec<_> = self
            .forage
            .iter()
            .filter_map(|(&id, p)| p.stay.map(|s| (id, self.tick.saturating_sub(s.since) as f64 * DT, s.rate)))
            .collect();
        v.sort_by_key(|e| e.0);
        v
    }
}

#[cfg(test)]
#[path = "forage_tests.rs"]
mod forage_tests;
