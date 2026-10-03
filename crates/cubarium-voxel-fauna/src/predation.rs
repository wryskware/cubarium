//! **The predation substrate** (`design/predation-and-flight-2026-09-28.md` §2): who
//! is prey and who is a threat to whom, the strike's phases and its dodge, the rest with
//! a reason, and the egg bank's hatching trigger — the pure parts, which the tick
//! ([`crate::step`]) and the heuristics call.
//!
//! # Concurrency
//!
//! A strike's outcome is an **event**, [`Hit`], collected while the act phase runs and
//! applied in one serial pass after it ([`apply_hit`]). No animal writes another during
//! the act: a striker reads its prey, and only the serial pass wounds, wakes or kills it.
//!
//! Every number here is a placeholder for `design/backlog.md`; the note's are marked as
//! the note's, and the others say why where they sit.

use serde::{Deserialize, Serialize};

use crate::manifest::Founder;
use crate::{Animal, FaunaConfig, PredationConfig, State};

/// A strike's phase (§2.3), each a visible pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrikePhase {
    /// A crouch: no movement.
    Windup,
    /// The body moves forward up to its reach under the ordinary step rule; contact is
    /// tested against the prey where it is at the end.
    Lunge,
    /// No strike, half speed.
    Recovery,
}

/// A strike in progress on a carnivore ([`crate::Animal::strike`]).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Strike {
    pub phase: StrikePhase,
    /// Ticks left in this phase, the current one included.
    pub ticks_left: u32,
    /// The prey the strike began on: the lunge's contact test is against its cylinder.
    pub prey: u64,
    /// The striker's centre when the lunge began: the jaw sweeps forward from here.
    pub from_x: f64,
    pub from_z: f64,
}

/// Why a body is at rest ([`crate::Animal::rest`], `Some` exactly while its state is
/// [`State::Asleep`]): the rest state with a reason (§2.8). One reason since the minimal
/// gut removed the carnivores' satiated rest: a fed hunter rests awake, its eating latch
/// off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rest {
    /// Out of its activity phase and not in an emergency, at `sleep_upkeep`.
    Sleep,
}

/// What one body is to another, per observer (§2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// In the observer's prey list and size window.
    Prey,
    /// The observer is in its prey list and size window, and it has moved in the last
    /// 2 s or is within 0.5 m.
    Threat,
    /// Anything else, a still threat further than 0.5 m included.
    Other,
}

/// One strike outcome, applied serially after the act ([`apply_hit`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub prey: u64,
    pub damage: f64,
    pub by: Founder,
}

/// Whether `hunter` would take `prey` (§2.1): its lineage is on the hunter's list and its
/// structure is inside the hunter's size window, which follows the hunter's own body; a
/// glider only on the ground.
pub fn is_prey_of(config: &FaunaConfig, hunter: &Animal, prey: &Animal) -> bool {
    is_prey_in_window(config, hunter, prey, hunter.body)
}

/// [`is_prey_of`] with the size window measured against `window_body` in place of the
/// hunter's own body: the pack seam (roster R3), where a pack member's window is the
/// summed body of its adult packmates nearby (`crate::pack::window_body`).
pub(crate) fn is_prey_in_window(
    config: &FaunaConfig,
    hunter: &Animal,
    prey: &Animal,
    window_body: f64,
) -> bool {
    let (Some(hf), Some(pf)) = (hunter.founder, prey.founder) else {
        return false;
    };
    let Some(h) = config.founder(hf).hunter else {
        return false;
    };
    // A rider is no prey on its own and hunts nothing while it rides (parasite §2).
    if prey.attached.is_some() || hunter.attached.is_some() {
        return false;
    }
    // A glider (F3, the seedporter) is prey only while grounded: climbing, perched or
    // gliding it is out of a walker's reach.
    if config.founder(pf).glider.is_some() && prey.mobility.mode != crate::MobilityMode::Ground {
        return false;
    }
    h.hunts(pf) && prey.body <= h.prey_max_ratio * window_body
}

/// Whether a body has moved faster than `still_speed_bl_per_s` within the last
/// `still_after_s` ([`PredationConfig`]).
pub fn moved_recently(cfg: &PredationConfig, a: &Animal) -> bool {
    a.still_ticks < cfg.still_after_ticks()
}

/// **What `other` is to `observer`** (§2.2), `distance_m` apart: prey (in the observer's
/// list and window), threat (the observer in the other's list and window — and moving,
/// or within 0.5 m), or other.
pub fn kind_of(
    config: &FaunaConfig,
    observer: &Animal,
    other: &Animal,
    distance_m: f64,
) -> BodyKind {
    if is_prey_of(config, observer, other) {
        return BodyKind::Prey;
    }
    if is_prey_of(config, other, observer) {
        let cfg = &config.predation;
        return if moved_recently(cfg, other) || distance_m <= cfg.still_threat_near_m {
            BodyKind::Threat
        } else {
            BodyKind::Other
        };
    }
    BodyKind::Other
}

/// **The dodge chance** (§2.3): `agility × awareness × aspect × condition`, aspect
/// `dodge_rear_aspect` (0.5) when struck from the rear half, condition the reserve
/// fraction clamped to `[dodge_condition_floor, 1]`, awareness the table's
/// ([`PredationConfig::awareness`]).
pub fn dodge_chance(
    cfg: &PredationConfig,
    agility: f64,
    state: State,
    struck_from_rear: bool,
    reserve_fraction: f64,
) -> f64 {
    let aspect = if struck_from_rear { cfg.dodge_rear_aspect } else { 1.0 };
    let condition = if reserve_fraction.is_finite() {
        reserve_fraction.clamp(cfg.dodge_condition_floor, 1.0)
    } else {
        cfg.dodge_condition_floor
    };
    (agility * cfg.awareness(state) * aspect * condition).clamp(0.0, 1.0)
}

/// Stream key for the dodge draw, apart from the step's own.
const DOMAIN_DODGE: u64 = 0xD0D6E;

/// **The dodge draw**, keyed by the world seed, the striker, the prey and the tick like
/// every draw in the step: the same strike always resolves the same way.
pub fn dodges(chance: f64, seed: u64, striker: u64, prey: u64, tick: u64) -> bool {
    if !(chance > 0.0) {
        return false;
    }
    let mut rng = crate::step::Rng::keyed(DOMAIN_DODGE, seed, striker.rotate_left(32) ^ prey, tick);
    rng.unit() < chance
}

/// **The hatching trigger** (§2.13, Wrysk's choice, behind one function): the egg's
/// footfall over the last clock-day has reached its threshold, it is dusk, and no adult of
/// its lineage is within `hatch_suppress_m` ([`PredationConfig`]).
pub fn egg_hatches(
    cfg: &PredationConfig,
    threshold: u32,
    footfall: u32,
    dusk: bool,
    nearest_adult_m: Option<f64>,
) -> bool {
    footfall >= threshold && dusk && nearest_adult_m.is_none_or(|d| d > cfg.hatch_suppress_m)
}

/// Stream key for the per-dusk hatch draw.
const DOMAIN_HATCH: u64 = 0x4A7C4;

/// **The per-dusk hatch draw**, beside [`egg_hatches`]'s gate: true with `chance` (the
/// config's [`crate::FaunaConfig::hatch_chance_per_dusk`]), keyed by the world seed, the
/// egg's own serial ([`Dormancy::serial`]) and the dusk's tick, so each egg draws once at
/// each dusk and the same egg at the same dusk always draws the same.
pub fn hatch_draw(chance: f64, seed: u64, egg_serial: u64, dusk_tick: u64) -> bool {
    let mut rng = crate::step::Rng::keyed(DOMAIN_HATCH, seed, egg_serial, dusk_tick);
    rng.unit() < chance
}

/// Apply one [`Hit`] event: the serial pass's act on the prey. The wound adds (armour was
/// in the damage); the hit **wakes** the prey, ending any rest; at a wound of 1 the prey
/// dies [`crate::Departure::Killed`], its whole body going to carrion on its face with a
/// carcass record. A prey already gone — killed by an earlier hit this tick — is left
/// alone, so it dies once.
pub fn apply_hit(
    fauna: &mut crate::Fauna,
    flora: &mut cubarium_voxel_flora::Flora,
    prey: u64,
    damage: f64,
    by: Founder,
) {
    crate::step::apply_hit(fauna, flora, prey, damage, by);
}

/// The tremor one body makes at a point `d` metres away: `mass × speed / d²`, its mass
/// its organic matter (§2.2); `d` is read as at least `near_m`
/// ([`PredationConfig::tremor_near_m`]).
pub fn tremor_of(a: &Animal, d: f64, near_m: f64) -> f64 {
    let speed = if a.speed_m_s.is_finite() {
        a.speed_m_s
    } else {
        0.0
    };
    if !(speed > 0.0) {
        return 0.0;
    }
    let d = d.max(near_m);
    a.organic() * speed / (d * d)
}

/// Egg footfall memory: how many distinct prey an egg remembers hearing.
pub const FOOTFALL_SLOTS: usize = 12;

/// A dormant egg (§2.13): its own footfall threshold and viability, and the prey it has
/// heard lately.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dormancy {
    /// Distinct prey heard over the last clock-day it needs before it hatches at dusk:
    /// drawn per egg, uniform on `egg_footfall_min..=egg_footfall_max` (2..=10: mean 6,
    /// spread wide).
    pub threshold: u32,
    /// The tick its viability runs out and it dies to the litter of its face: drawn per
    /// egg, log-uniform over 6–24 h (`egg_viability_median_s` 12 h, `egg_viability_spread` 2).
    pub viable_until_tick: u64,
    /// `(prey id + 1, tick last heard)`, `0` an empty slot. The oldest is forgotten when
    /// it is full.
    pub heard: [(u64, u64); FOOTFALL_SLOTS],
    /// The egg's own serial (laid or buried), no other egg's: what its hatch draw is keyed
    /// by ([`hatch_draw`]).
    pub serial: u64,
}

impl Dormancy {
    /// A dormant egg drawn from a keyed stream: `seed` the world's, `serial` a counter no
    /// other egg shares.
    pub fn draw(cfg: &PredationConfig, seed: u64, serial: u64, tick: u64) -> Dormancy {
        let mut rng = crate::step::Rng::keyed(0xE66, seed, serial, tick);
        let range = u64::from(cfg.egg_footfall_max - cfg.egg_footfall_min) + 1;
        let threshold = cfg.egg_footfall_min + (rng.next_u64() % range) as u32;
        // Log-uniform over [median / spread, median × spread]: median × spread^u, u
        // uniform on [-1, 1] (6–24 h at the defaults).
        let u = 2.0 * rng.unit() - 1.0;
        let span_s = cfg.egg_viability_median_s * cfg.egg_viability_spread.powf(u);
        let ticks = cubarium_voxel::ticks_at_least_one(span_s);
        Dormancy {
            threshold,
            viable_until_tick: tick.saturating_add(ticks),
            heard: [(0, 0); FOOTFALL_SLOTS],
            serial,
        }
    }

    /// Distinct prey heard within the last `window_ticks` up to `now`.
    pub fn footfall(&self, now: u64, window_ticks: u64) -> u32 {
        self.heard
            .iter()
            .filter(|&&(id, t)| id != 0 && now.saturating_sub(t) < window_ticks)
            .count() as u32
    }

    /// Hear prey `id` at `tick`: refresh its slot, or take an empty one, or forget the
    /// oldest.
    pub fn hear(&mut self, id: u64, tick: u64) {
        let key = id.wrapping_add(1);
        if let Some(slot) = self.heard.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = tick;
            return;
        }
        let at = self
            .heard
            .iter()
            .enumerate()
            .min_by_key(|&(_, &(k, t))| if k == 0 { (0, 0) } else { (1, t) })
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.heard[at] = (key, tick);
    }
}

#[cfg(test)]
mod hatch_tests {
    use super::*;

    /// Bet-hedging: over many seeded dusks, a third of a 300-egg bank that is past its
    /// threshold with no adult near hatches at one dusk (1/3 ± 0.06); below the threshold
    /// none does.
    #[test]
    fn a_third_of_a_bank_past_its_threshold_hatches_at_one_dusk() {
        let (mut hatched, mut drawn) = (0u32, 0u32);
        let cfg = PredationConfig::default();
        let chance = crate::FaunaConfig::default().hatch_chance_per_dusk;
        for seed in 1..=40u64 {
            let dusk_tick = 1_000 + 97 * seed;
            for serial in 0..300u64 {
                assert!(egg_hatches(&cfg, 4, 6, true, None));
                drawn += 1;
                if hatch_draw(chance, seed, serial, dusk_tick) {
                    hatched += 1;
                }
                assert!(!egg_hatches(&cfg, 4, 3, true, None), "below its threshold, never");
            }
        }
        let share = f64::from(hatched) / f64::from(drawn);
        assert!(
            (share - chance).abs() <= 0.06,
            "{share} of the bank hatched at one dusk"
        );
    }
}
