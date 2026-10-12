//! **Daily rhythms and the stop-and-go pace** (RHYTHMS-CADENCE, Wrysk 2026-10-09;
//! `design/training-plan-2026-10-08.md` §6 item 18).
//!
//! # A leaning, not a switch ([`Rhythm`])
//!
//! A lineage whose [`crate::Activity`] is [`crate::Activity::Leaning`] has a **chance of being
//! awake** by the time of day ([`Rhythm::chance`]): one value with the sun well up, one with it
//! well down, and one at sunrise and sunset, eased into the other two over `twilight_h` hours of
//! the day's 24-hour clock either side of the horizon ([`Horizon`]). A dawn and dusk peak is a
//! twilight value above both; a flat curve is three equal values.
//!
//! Each body **rests in bouts** drawn from that chance, so no two bodies share a schedule and no
//! body keeps a clock-perfect cycle ([`advance`]): awake, a rest begins at random with the hazard
//! `(1 − p) / (p · rest_mean_h)` per hour; a rest lasts `rest_min_h` plus an exponential draw with
//! mean `rest_mean_h − rest_min_h`. The awake bouts then average `rest_mean_h · p / (1 − p)`, so a
//! body is awake a share `p` of the time wherever the chance holds still, and a body whose
//! chance falls at dusk soon rests, waking briefly between rests through the night (a grazer's
//! sentinel effect). The state is one counter per body ([`crate::Animal::rest_ticks`]); with the
//! day clock off no body rests (every rhythm is awake), as before.
//!
//! `shade_is_night` keeps the dark-active reading where it matters (the detritus web §3.1B): by
//! day on a face whose light (`sky × daylight`) is under the day line it takes the night chance —
//! a littershredder in shade or damp under an overhang is as likely awake as at night (real
//! millipedes dry out in the sun), a capgnawer in a cave keeps the night.
//!
//! Wild models: lions rest about 20 of 24 hours and hunt at any hour (Schaller 1972, *The
//! Serengeti Lion*); grazing ungulates sleep in short bouts through the night with herd members
//! awake at staggered times (Ruckebusch 1972, *Physiol. Behav.* 9:13; Lima et al. 2005, *Neurosci.
//! Biobehav. Rev.* 29:1); many moths and other insects fly in a dusk peak and a smaller dawn one
//! (Lewis & Taylor 1965, *Trans. R. Entomol. Soc.* 116:393).
//!
//! # Stop and go ([`Cadence`])
//!
//! A walker or climber searching or foraging on its own heuristic moves in **bouts**: each bout
//! at its own drawn share of the cruise, with a short **pause** to look and listen between bouts
//! ([`Cadence::at`]). Flight from a threat keeps a steady run; a flyer in the air is not paced.
//! The draws are keyed by the world seed, the body and the bout, so the pace is stateless and the
//! same across a save. Wild model: **saltatory search** — many foragers move in short runs
//! separated by scanning pauses, and detect prey and predators mostly while stopped (O'Brien,
//! Browman & Evans 1990, *Am. Sci.* 78:152; Kramer & McLaughlin 2001, *Am. Zool.* 41:137).
//!
//! What makes the pause pay is the world, not a score: a body that is itself still senses
//! farther ([`crate::SenseTable::still_gain`], read by the perception core), and a still body is
//! harder to see ([`crate::SenseTable::sight_still_fraction`]). Nothing rewards a pause.
//!
//! # Waking ([`wakes`], CALLS-REST: one rule)
//!
//! A sleeping body whose lineage hears wakes to a **running** body (fleeing or bolting: the alarm
//! wave, [`crate::vigil`]) — or, where its lineage's `rest.rush_over_cruise` says so, a
//! **rushing** one — it would hear at `sleep_hearing` of its awake sensitivity
//! ([`crate::Vigilance::sleep_hearing`]) through the terrain between them; to a heeded **call**;
//! and, in light rest, to a **pressing threat** it senses. It is kept awake for the predation
//! table's `alert_s` and hears the wave at its next sensing.

use serde::{Deserialize, Serialize};

use crate::Founder;

/// **Where in the day it is**: whether the sun is up, and hours on the day's 24-hour clock to
/// the nearer of the day's two horizon crossings (sunrise or sunset). Derived from the world's
/// sun each tick ([`Horizon::of`]), never saved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Horizon {
    pub up: bool,
    pub edge_h: f64,
}

impl Default for Horizon {
    fn default() -> Horizon {
        Horizon::NOON
    }
}

impl Horizon {
    /// The sun well up, no horizon near: the clock off.
    pub const NOON: Horizon = Horizon { up: true, edge_h: f64::INFINITY };

    /// The horizon clock of `sun`.
    pub fn of(sun: &cubarium_voxel::weather::Sun) -> Horizon {
        let day = (sun.set - sun.rise).rem_euclid(1.0);
        let since_rise = (sun.phase - sun.rise).rem_euclid(1.0);
        if since_rise <= day {
            Horizon { up: true, edge_h: 24.0 * since_rise.min(day - since_rise) }
        } else {
            let since_set = since_rise - day;
            let night = 1.0 - day;
            Horizon { up: false, edge_h: 24.0 * since_set.min(night - since_set) }
        }
    }
}

/// **A lineage's daily leaning** (module note): its waking chance by the time of day and the
/// length of its rests. Read only while its activity is [`crate::Activity::Leaning`]. Every
/// number is a knob (`fauna.founders.<lineage>.rhythm.*`); the default is awake at every hour
/// with no rests.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rhythm {
    /// The waking chance with the sun up, beyond `twilight_h` of the horizon.
    pub day: f64,
    /// The waking chance with the sun down, beyond `twilight_h` of the horizon.
    pub night: f64,
    /// The waking chance at sunrise and sunset.
    pub twilight: f64,
    /// Hours on the 24-hour day clock either side of the horizon over which the chance eases
    /// from `twilight` to `day` or `night` (a smoothstep); 0 has no twilight value.
    pub twilight_h: f64,
    /// The mean rest bout, hours on the 24-hour day clock; 0 never rests.
    pub rest_mean_h: f64,
    /// The shortest rest bout, hours on the 24-hour day clock (at most `rest_mean_h`).
    pub rest_min_h: f64,
    /// By day on a dark face (`sky × daylight` under the day line) it takes the night chance
    /// (a switch, no registry row).
    pub shade_is_night: bool,
}

impl Default for Rhythm {
    fn default() -> Rhythm {
        Rhythm {
            day: 1.0,
            night: 1.0,
            twilight: 1.0,
            twilight_h: 0.0,
            rest_mean_h: 0.0,
            rest_min_h: 0.0,
            shade_is_night: false,
        }
    }
}

impl Rhythm {
    /// The waking chance at `h`, `dark_face` when the light at its face is under the day line
    /// while the sun is up (read only with `shade_is_night`). In `[0, 1]`.
    pub fn chance(&self, h: Horizon, dark_face: bool) -> f64 {
        let p = if h.up && dark_face && self.shade_is_night {
            self.night
        } else {
            let base = if h.up { self.day } else { self.night };
            if self.twilight_h > 0.0 && h.edge_h < self.twilight_h {
                let x = (h.edge_h / self.twilight_h).clamp(0.0, 1.0);
                let s = x * x * (3.0 - 2.0 * x);
                self.twilight + (base - self.twilight) * s
            } else {
                base
            }
        };
        if p.is_finite() { p.clamp(0.0, 1.0) } else { 1.0 }
    }

    /// The mean rest bout, hours: `rest_mean_h`, at least `rest_min_h`.
    fn mean_h(&self) -> f64 {
        let m = self.rest_mean_h.max(self.rest_min_h);
        if m.is_finite() { m.max(0.0) } else { 0.0 }
    }

    /// The chance a rest begins within `hours` of awake time at waking chance `p`: the hazard
    /// `(1 − p) / (p · mean)` per hour (module note). 0 with no rests or `p` at 1.
    pub fn onset(&self, p: f64, hours: f64) -> f64 {
        let mean = self.mean_h();
        if !(mean > 0.0) || p >= 1.0 || !(hours > 0.0) {
            return 0.0;
        }
        if p <= 0.0 {
            return 1.0;
        }
        let hazard = (1.0 - p) / (p * mean);
        (1.0 - (-hazard * hours).exp()).clamp(0.0, 1.0)
    }

    /// A rest bout's length, hours, from a uniform draw `u` in `[0, 1)`: `rest_min_h` plus an
    /// exponential with mean `mean − rest_min_h`.
    pub fn bout_h(&self, u: f64) -> f64 {
        let mean = self.mean_h();
        let min = self.rest_min_h.clamp(0.0, mean);
        let e = -(1.0 - u.clamp(0.0, 1.0 - 1e-12)).ln();
        min + (mean - min) * e
    }
}

/// The keyed stream's domains.
const DOMAIN_REST: u64 = 0x2E57_B0A7;
const DOMAIN_CADENCE: u64 = 0xCADE_0CE5;

/// **One tick of a body's rest bouts** (module note): `rest_ticks` is what is left of its rest
/// (0 awake), `p` its waking chance now, `hours` the tick's length on the 24-hour day clock;
/// `seed`, `id` and `tick` key the draws. Returns the rest left after this tick: a resting body
/// counts down, an awake one starts a rest at random.
pub fn advance(r: &Rhythm, rest_ticks: u32, p: f64, hours: f64, seed: u64, id: u64, tick: u64) -> u32 {
    if rest_ticks > 0 {
        return rest_ticks - 1;
    }
    let mut rng = cubarium_voxel::KeyedRng::keyed(DOMAIN_REST, seed, id, tick);
    if rng.unit() >= r.onset(p, hours) {
        return 0;
    }
    let ticks = r.bout_h(rng.unit()) / hours;
    if ticks.is_finite() { ticks.round().clamp(1.0, f64::from(u32::MAX)) as u32 } else { u32::MAX }
}

/// Hours on the 24-hour day clock one tick takes under `day`.
pub fn hours_per_tick(day: &cubarium_voxel::weather::DayClock) -> f64 {
    let s = day.day_s();
    if s > 0.0 && s.is_finite() { 24.0 * crate::DT / s } else { 0.0 }
}

/// **A lineage's stop-and-go pace** (module note): every number a knob
/// (`fauna.founders.<lineage>.cadence.*`). The default is off (a steady cruise).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cadence {
    /// One bout and its pause, seconds: each such span holds one pause at a random point in
    /// it, so the moving runs between pauses vary from nothing to twice this. 0 is off.
    pub cycle_s: f64,
    /// A pause's shortest and longest, seconds (a uniform draw; at most `cycle_s`).
    pub pause_min_s: f64,
    pub pause_max_s: f64,
    /// A bout's pace, × the forward its heuristic asked: a uniform draw in this range.
    pub speed_min: f64,
    pub speed_max: f64,
}

impl Default for Cadence {
    fn default() -> Cadence {
        Cadence { cycle_s: 0.0, pause_min_s: 0.0, pause_max_s: 0.0, speed_min: 1.0, speed_max: 1.0 }
    }
}

/// The pace at one tick ([`Cadence::at`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pace {
    pub paused: bool,
    /// The bout's share of the asked forward.
    pub speed: f64,
}

impl Cadence {
    /// Paced at all.
    pub fn on(&self) -> bool {
        self.cycle_s.is_finite() && self.cycle_s > 0.0
    }

    /// The pace of body `id` at `tick` (module note): stateless, keyed by `seed`, the body and
    /// its bout.
    pub fn at(&self, seed: u64, id: u64, tick: u64) -> Pace {
        if !self.on() {
            return Pace { paused: false, speed: 1.0 };
        }
        let slot = cubarium_voxel::ticks_at_least_one(self.cycle_s).max(2);
        let offset = cubarium_voxel::mix(seed ^ cubarium_voxel::mix(id ^ DOMAIN_CADENCE)) % slot;
        let t = tick.wrapping_add(offset);
        let (k, pos) = (t / slot, t % slot);
        let mut rng = cubarium_voxel::KeyedRng::keyed(DOMAIN_CADENCE, seed, id, k);
        let (u_len, u_at, u_speed) = (rng.unit(), rng.unit(), rng.unit());
        let lo = self.pause_min_s.max(0.0);
        let hi = self.pause_max_s.max(lo);
        let len_s = lo + (hi - lo) * u_len;
        let len = if len_s > 0.0 { cubarium_voxel::ticks_at_least_one(len_s).min(slot - 1) } else { 0 };
        let start = ((slot - len + 1) as f64 * u_at).floor() as u64;
        let (s_lo, s_hi) = (self.speed_min.max(0.0), self.speed_max.max(self.speed_min.max(0.0)));
        Pace { paused: len > 0 && pos >= start && pos < start + len, speed: s_lo + (s_hi - s_lo) * u_speed }
    }
}

/// **Each lineage's leaning** (module note), from Wrysk's words of 2026-10-09; the probabilities
/// are plausible starts, untuned. One day-clock hour is five minutes of a 2-hour world day.
/// The lanternjaw and the ripple-snail keep their own rules (their activity is not
/// [`crate::Activity::Leaning`], so this is unread for them).
pub fn rhythm_of(founder: Founder) -> Rhythm {
    let r = Rhythm::default();
    match founder {
        // "randomly always active … just take random breaks. but no real cycle": flat, frequent
        // short rests; shade (or night) a little more awake than open sun.
        Founder::Blind => Rhythm {
            day: 0.7,
            night: 0.8,
            twilight: 0.8,
            twilight_h: 1.0,
            rest_mean_h: 0.5,
            rest_min_h: 0.1,
            shade_is_night: true,
        },
        // Night-leaning, drawn to glow: mostly awake in the dark, in a cave by day too.
        Founder::Capgnawer => Rhythm {
            day: 0.15,
            night: 0.85,
            twilight: 0.6,
            twilight_h: 1.0,
            rest_mean_h: 1.0,
            rest_min_h: 0.25,
            shade_is_night: true,
        },
        // "mostly a day thing but … stay vigilant at night": a night share awake, in short
        // staggered wakes between rests (the sentinel effect).
        Founder::Browser => Rhythm {
            day: 0.9,
            night: 0.15,
            twilight: 0.6,
            twilight_h: 1.0,
            rest_mean_h: 0.75,
            rest_min_h: 0.1,
            ..r
        },
        // More active by day.
        Founder::Loftstrider => Rhythm {
            day: 0.85,
            night: 0.25,
            twilight: 0.6,
            twilight_h: 1.0,
            rest_mean_h: 1.0,
            rest_min_h: 0.25,
            ..r
        },
        // "more active at night … still active during the day occasionally. huge swarms at
        // dawn/dusk": night-leaning, a twilight peak over both.
        Founder::Bellwing => Rhythm {
            day: 0.25,
            night: 0.7,
            twilight: 0.95,
            twilight_h: 1.0,
            rest_mean_h: 0.5,
            rest_min_h: 0.1,
            ..r
        },
        // Diurnal, roosting at night (its roost reflex takes it up before it sleeps).
        Founder::Seedporter => Rhythm {
            day: 0.9,
            night: 0.05,
            twilight: 0.5,
            twilight_h: 1.0,
            rest_mean_h: 1.5,
            rest_min_h: 0.25,
            ..r
        },
        // "can probably hunt any time of day, and like lions tend to sleep or play a lot when
        // they arent hunting": any hour, long rests (its emergency latch wakes it hungry).
        Founder::Chorister => Rhythm {
            day: 0.35,
            night: 0.35,
            twilight: 0.35,
            twilight_h: 0.0,
            rest_mean_h: 2.0,
            rest_min_h: 0.5,
            ..r
        },
        Founder::Lanternjaw | Founder::RippleSnail => r,
    }
}

/// **Each lineage's pace** (module note): walkers and climbers; a bout every 6–12 s with a
/// 0.5–3 s pause and a bout pace of 0.7–1 of the asked forward. Plausible starts, untuned. The
/// bellwing (a flyer), the lanternjaw (an ambusher on its patrol) and the ripple-snail (a
/// gliding film grazer) are not paced.
pub fn cadence_of(founder: Founder) -> Cadence {
    let paced = |cycle_s: f64, pause_min_s: f64, pause_max_s: f64| Cadence {
        cycle_s,
        pause_min_s,
        pause_max_s,
        speed_min: 0.7,
        speed_max: 1.0,
    };
    match founder {
        Founder::Blind | Founder::Capgnawer => paced(8.0, 0.5, 2.5),
        Founder::Browser | Founder::Loftstrider => paced(10.0, 1.0, 3.0),
        Founder::Seedporter => paced(8.0, 0.5, 2.0),
        Founder::Chorister => paced(12.0, 1.0, 3.0),
        Founder::Bellwing | Founder::Lanternjaw | Founder::RippleSnail => Cadence::default(),
    }
}

/// Why a sleeper woke ([`wakes`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wake {
    /// A loud body heard: a running prey (fleeing or bolting) or, for a lineage with
    /// `rest.rush_over_cruise`, a rushing one.
    Footfall,
    /// A heeded call of its lineage heard since it lay down.
    Call,
    /// In light rest, a pressing threat in its own record.
    Sight,
}

/// **The one wake rule** (module note; CALLS-REST step 4, behaviour-AI §3.4 and §11.2) for
/// body `i`, asleep:
/// - **footfall**: a body in `loud` (awake, and running or above its cruise) that is running
///   (its flight or bolt: the alarm wave, [`crate::vigil`]) or, for a sleeper whose lineage's
///   `rest.rush_over_cruise` is above 0, moving faster than that × its own lineage's cruise (a
///   rush; a stalk at or under cruise never wakes it), heard at `sleep_hearing` of its awake
///   sensitivity within its `hearing_m` (3D), its loudness through the pair's terrain ray
///   (× `perception.occlusion` when blocked, as waking hearing);
/// - **a call**: a call record of a kind its lineage heeds ([`crate::calls::heeds`]) heard since
///   it lay down — for a deep sleeper only one at least `calls.asleep_loudness` after
///   occlusion is recorded at all ([`crate::calls::hear`]);
/// - **sight**: in light rest ([`crate::sleep`]), a pressing threat (seen moving or near, heard,
///   felt; not one it watches) in its record from a sensing since it lay down.
///
/// This replaces RHYTHMS-CADENCE's alarm wake (a running body's emission with no terrain ray):
/// that is the footfall clause, now through the ray.
pub(crate) fn wakes(fauna: &crate::Fauna, view: &cubarium_voxel::VoxelView<'_>, i: usize, loud: &[usize]) -> Option<Wake> {
    let b = &fauna.animals[i];
    let f = b.founder?;
    let phys = fauna.config.founder(f);
    let since = b.doze.asleep_since;
    // A heeded call heard since it lay down.
    if crate::calls::heard(fauna, b.id)
        .iter()
        .any(|r| r.tick >= since && !phys.calls.sensed_kind(usize::from(r.kind)) && crate::calls::heeds(&fauna.config, f, b, usize::from(r.kind)))
    {
        return Some(Wake::Call);
    }
    // In light rest, a pressing threat it sensed since lying down.
    if crate::sleep::light_rest(b) && crate::vigil::pressing_since(fauna, b, since, view) {
        return Some(Wake::Sight);
    }
    let (table, keep) = (phys.senses, phys.vigilance.sleep_hearing);
    if loud.is_empty() || !table.live || !(table.hearing_m > 0.0) || !(keep > 0.0) {
        return None;
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let threshold = table.hearing_min / keep;
    let pc = fauna.config.perception;
    let near = pc.hearing_near_m.max(1e-9);
    let rush = phys.rest.rush_over_cruise;
    let ear = crate::perception::base_m(b, v) + crate::body_of(&fauna.config, b).map_or(0.0, |g| g.height_m);
    let now = fauna.tick;
    loud.iter()
        .any(|&k| {
            if k == i {
                return false;
            }
            let o = &fauna.animals[k];
            let running = fauna.percepts.get(&o.id).is_some_and(|p| p.vigil.running(now));
            let rushing = rush > 0.0
                && o.founder.is_some_and(|of| o.speed_m_s > rush * fauna.config.manifest(of).cruise_m_per_s);
            if !(running || rushing) {
                return false;
            }
            let (dx, dz) = crate::pose::wrapped_delta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
            let dy = if o.pose.y_m.is_finite() && b.pose.y_m.is_finite() { o.pose.y_m - b.pose.y_m } else { 0.0 };
            let d = (dx * dx + dy * dy + dz * dz).sqrt();
            if d > table.hearing_m {
                return false;
            }
            let e = crate::perception::emission(&fauna.config, o) * crate::perception::run_noise(fauna, o, now);
            if !(e > 0.0 && e / d.max(near) >= threshold) {
                return false;
            }
            let mid = crate::perception::base_m(o, v) + 0.5 * crate::body_of(&fauna.config, o).map_or(0.0, |g| g.height_m);
            crate::perception::line_clear(view, b.id, (b.pose.x, ear, b.pose.z), (o.pose.x, mid, o.pose.z))
                || e * pc.occlusion.max(0.0) / d.max(near) >= threshold
        })
        .then_some(Wake::Footfall)
}

/// The indices of every awake body that is running (fleeing or bolting) or moving faster than
/// its lineage's cruise: the footfalls a sleeper may hear ([`wakes`]).
pub(crate) fn loud_bodies(fauna: &crate::Fauna) -> Vec<usize> {
    let now = fauna.tick;
    fauna
        .animals
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            a.state != crate::State::Asleep
                && (fauna.percepts.get(&a.id).is_some_and(|p| p.vigil.running(now))
                    || a.founder.is_some_and(|f| a.speed_m_s > fauna.config.manifest(f).cruise_m_per_s))
        })
        .map(|(k, _)| k)
        .collect()
}

/// **The rhythms before the leanings** (tests only): every leaning lineage back on the on/off
/// activity it had (the browser, loftstrider, bellwing, seedporter and Veilhound diurnal, the
/// shredder and capgnawer nocturnal), for cases that test the on/off rule and its consumers.
#[cfg(test)]
pub(crate) fn fixed_rhythms(c: &mut crate::FaunaConfig) {
    use crate::Activity::{Diurnal, Nocturnal};
    for (f, a) in [
        (Founder::Blind, Nocturnal),
        (Founder::Capgnawer, Nocturnal),
        (Founder::Browser, Diurnal),
        (Founder::Loftstrider, Diurnal),
        (Founder::Bellwing, Diurnal),
        (Founder::Seedporter, Diurnal),
        (Founder::Chorister, Diurnal),
    ] {
        c.founders[f.index()].active = a;
    }
}

/// [`crate::FaunaConfig::default`] with [`fixed_rhythms`] (tests only).
#[cfg(test)]
pub(crate) fn fixed_config() -> crate::FaunaConfig {
    let mut c = crate::FaunaConfig::default();
    fixed_rhythms(&mut c);
    c
}
