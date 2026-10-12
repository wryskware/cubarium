//! **Rest physiology** (CALLS-REST, behaviour-AI §3.4 and §11.4; Wrysk's topic 5,
//! 2026-10-12: "would you recommend B? it sounds more interesting to watch").
//!
//! A lineage whose [`RestTable::light`] is on rests **mostly lightly**: a body in its rest
//! bout ([`crate::State::Asleep`], whatever put it there: its rhythm, a brain's or a
//! heuristic's [`crate::RestAsk`]) lies still in **light rest**, sensing at `rest_sense` of its
//! awake ranges, and falls into **deep sleep** only in short bouts (`deep_min_s`..`deep_max_s`,
//! about 1–2 min), each after at least `light_min_s` of light rest, at `deep_per_min` per
//! minute of light rest past that. Only deep sleep is blind: a deep sleeper does not sense,
//! and hears only loud things ([`crate::rhythm::wakes`]).
//!
//! - **Sleeps lighter** (`deep_alone_scale`): while it has had no sight of a relative up or in
//!   light rest within `kin_awake_s` (its own sight tracks of its lineage, [`Doze::kin_awake`]:
//!   visual evidence only, never an unseen animal's true state), its deep-bout rate is scaled
//!   by this. A guard emerges from what each one sees; no rota.
//! - **The sleep-need ceiling** (`need_s`): a body that has had no deep bout for this long
//!   (awake or resting) takes one as soon as its light-rest interval allows, whatever it sees.
//!   The rhythm stays the backstop that puts it to rest at all.
//! - The light-or-deep choice is the body's: brains and heuristics only ask to rest.
//! - Off (every other lineage), the whole rest is deep, as before.
//!
//! Wild models: giraffe deep-sleep bouts last about a minute (median 55 s, mean 2 min) inside
//! longer light rest, with at most three of a group in deep sleep at once (Tobler & Schwierin
//! 1996, *J. Sleep Res.* 5:21; Sicks 2016, *Zool. Garten* 85:120); ducks at the exposed end of
//! a row sleep with one eye open more often (Rattenborg, Lima & Amlaner 1999, *Nature*
//! 397:397).

use serde::{Deserialize, Serialize};

use crate::{Animal, Fauna, Founder, State};

/// **A lineage's rest physiology** (module note). Off by default: the whole rest is deep.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RestTable {
    /// Rest is mostly light with short deep bouts (a switch, no registry row; off: the whole
    /// rest is deep and blind, as before).
    pub light: bool,
    /// In light rest it senses at this share of its awake ranges (sight, hearing, tremor).
    /// Fraction.
    pub rest_sense: f64,
    /// At least this much light rest before a deep bout and between two, seconds.
    pub light_min_s: f64,
    /// Past that, a deep bout begins at this rate per minute of light rest.
    pub deep_per_min: f64,
    /// A deep bout lasts a uniform draw between these, seconds.
    pub deep_min_s: f64,
    pub deep_max_s: f64,
    /// **Sleeps lighter**: the deep-bout rate is multiplied by this while it has seen no
    /// relative up or in light rest within `kin_awake_s`. Factor (1: no effect).
    pub deep_alone_scale: f64,
    /// A sight of a relative up or in light rest counts this long, seconds.
    pub kin_awake_s: f64,
    /// **The sleep-need ceiling**: a body with no deep bout for this long takes one at its next
    /// chance, seconds (0: no ceiling).
    pub need_s: f64,
    /// **Waking** (step 4): a body moving faster than this × its lineage's cruise is rushing:
    /// its footfall wakes a sleeper of this lineage that hears it as it would a fleeing prey's.
    /// Factor (0: rushes wake no one, as before).
    pub rush_over_cruise: f64,
    /// **Rest sites** (step 5): a heuristic body about to rest walks down the hunter-scent
    /// gradient while the hunter scent where it stands is at least this × its smell floor
    /// (`senses.smell_min`). Factor (0: it rests where it stands, as before).
    pub bed_scent: f64,
    /// ...for at most this long from the rest's onset, then rests where it is, seconds.
    pub bed_search_s: f64,
}

impl Default for RestTable {
    /// Off: the whole rest is deep, rests wherever it stands, and no rush wakes it.
    fn default() -> RestTable {
        RestTable {
            light: false,
            rest_sense: 0.5,
            light_min_s: 60.0,
            deep_per_min: 0.5,
            deep_min_s: 60.0,
            deep_max_s: 120.0,
            deep_alone_scale: 1.0,
            kin_awake_s: 60.0,
            need_s: 0.0,
            rush_over_cruise: 0.0,
            bed_scent: 0.0,
            bed_search_s: 0.0,
        }
    }
}

/// **Each lineage's rest physiology** (module note), placeholders, untuned:
/// - **the loftstrider** (topic 5): half its ranges in light rest; deep bouts of 1–2 min after
///   at least 1 min of light rest, at 0.5 a minute past that (a bout every ~3 min of light rest
///   with a relative seen up: about a third of rest time deep); seeing no relative up within a
///   minute, a fifth of that rate (deep bouts ~5× rarer); a deep bout at the latest every 20
///   min (four world-day hours at the 2-hour day: one night's worth). A body at more than
///   1.2× its cruise is rushing: a Veilhound's rush (3.5 m/s against its 1.4) wakes it, its
///   stalk (at or under cruise) does not. Rests away from hunter scent it can smell, searching
///   up to 30 s;
/// - **the Veilhound** (topic 5's "also for the Veilhounds at the den"): the same light rest
///   and sleeps-lighter rule, which it mostly takes at its den; rushes and scent are prey's.
pub fn rest_of(founder: Founder) -> RestTable {
    let d = RestTable::default();
    let light = RestTable {
        light: true,
        rest_sense: 0.5,
        light_min_s: 60.0,
        deep_per_min: 0.5,
        deep_min_s: 60.0,
        deep_max_s: 120.0,
        deep_alone_scale: 0.2,
        kin_awake_s: 60.0,
        need_s: 1200.0,
        ..d
    };
    match founder {
        Founder::Loftstrider => RestTable { rush_over_cruise: 1.2, bed_scent: 1.0, bed_search_s: 30.0, ..light },
        Founder::Chorister => light,
        _ => d,
    }
}

/// **A body's rest state** (module note), saved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Doze {
    /// In light rest (asleep and not deep). False awake, and for an asleep body whose lineage
    /// has no light rest: every sleeper is deep unless this is set.
    pub light: bool,
    /// Its light rest began at this tick.
    pub light_since: u64,
    /// Its deep bout ends at this tick.
    pub deep_until: u64,
    /// Its last deep bout ended (or its first rest began) at this tick (0: never yet).
    pub last_deep: u64,
    /// The last tick it saw a relative up or in light rest (0: never).
    pub kin_awake: u64,
    /// **Rest sites** (step 5): the tick its current search for a scent-free bed ends (0: no
    /// search under way).
    pub bed_until: u64,
}

/// Whether `a` is in deep sleep: asleep and not in light rest. Blind and hard of hearing.
pub fn deep_asleep(a: &Animal) -> bool {
    a.state == State::Asleep && !a.doze.light
}

/// Whether `a` is in light rest: asleep, lying still, sensing at its `rest_sense`.
pub fn light_rest(a: &Animal) -> bool {
    a.state == State::Asleep && a.doze.light
}

const DOMAIN_DEEP: u64 = 0xDEE9_5EE9;

fn ticks(s: f64) -> u64 {
    if s.is_finite() && s > 0.0 { cubarium_voxel::ticks_at_least_one(s) } else { 0 }
}

/// **Sight of a relative up** (module note): the latest tick body `i`'s record holds a sight
/// track of its own lineage that is not in deep sleep (up, still, or in light rest).
fn note_kin(fauna: &mut Fauna, i: usize) {
    use crate::perception::{SIGHT, class, posture};
    let id = fauna.animals[i].id;
    let Some(p) = fauna.percepts.get(&id) else { return };
    let seen = p
        .tracks()
        .iter()
        .filter(|t| t.class == class::KIN && t.id != id && t.modes & SIGHT != 0 && t.posture != posture::ASLEEP)
        .map(|t| t.tick)
        .max();
    if let Some(t) = seen {
        let d = &mut fauna.animals[i].doze;
        d.kin_awake = d.kin_awake.max(t);
    }
}

/// **The body's light-or-deep choice** for one tick (module note), body `i` of lineage `f`,
/// after the circadian stage has settled whether it rests: `was_asleep` is its state before.
/// Keyed draws (`seed`, the body, the tick) keep it stateless across a save. Returns whether a
/// deep bout began this tick (for a lineage without light rest: whether it fell asleep).
pub(crate) fn step(fauna: &mut Fauna, i: usize, f: Founder, was_asleep: bool, seed: u64) -> bool {
    let t = fauna.config.founder(f).rest;
    let now = fauna.tick;
    if !t.light {
        let a = &mut fauna.animals[i];
        a.doze.light = false;
        return !was_asleep && a.state == State::Asleep;
    }
    note_kin(fauna, i);
    let a = &mut fauna.animals[i];
    if a.state != State::Asleep {
        a.doze.light = false;
        a.doze.deep_until = 0;
        return false;
    }
    let d = &mut a.doze;
    if !was_asleep {
        // Lying down: light rest first.
        d.light = true;
        d.light_since = now;
        d.deep_until = 0;
        if d.last_deep == 0 {
            d.last_deep = now;
        }
    }
    if !d.light {
        if now >= d.deep_until {
            d.light = true;
            d.light_since = now;
            d.last_deep = now;
        }
        return false;
    }
    if now < d.light_since + ticks(t.light_min_s) {
        return false;
    }
    let need = t.need_s > 0.0 && now >= d.last_deep + ticks(t.need_s);
    let alone = !(d.kin_awake > 0 && now <= d.kin_awake + ticks(t.kin_awake_s));
    let rate = t.deep_per_min.max(0.0) * if alone { t.deep_alone_scale.max(0.0) } else { 1.0 };
    let p = if need { 1.0 } else { 1.0 - (-rate / 60.0 * cubarium_voxel::DT).exp() };
    let mut rng = cubarium_voxel::KeyedRng::keyed(DOMAIN_DEEP, seed, a.id, now);
    if rng.unit() >= p {
        return false;
    }
    let (lo, hi) = (t.deep_min_s.max(0.0), t.deep_max_s.max(t.deep_min_s.max(0.0)));
    let s = lo + (hi - lo) * rng.unit();
    let d = &mut a.doze;
    d.light = false;
    d.deep_until = now + ticks(s).max(1);
    true
}
