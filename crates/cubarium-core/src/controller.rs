//! Observation → decision. Pure: no world mutation, no draws except the OU noise the
//! caller supplies. Controller v2 (`design/fauna-v2.md`): diet-weighted food gradients, the
//! fruit channel, a depth preference, and a per-mode turn gate.

use cubarium_surface::Vec2;

use crate::config::OrganismConfig;
use crate::organism::{Mode, Organism};

/// What an organism senses this tick, all in its own chart.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Observation {
    /// `P`, fruit `F` and edible detritus in the organism's own cell. Edible detritus is
    /// `D_eff = D · min(1, ρ / e_r)` with `ρ = De / D`: detritus too energy-poor to fuel
    /// reserve storage is not food, so `d_here` carries `D_eff`, never raw `D`.
    pub p_here: f64,
    pub f_here: f64,
    pub d_here: f64,
    /// Normalized gradients `Σ (value_i − value_0) · dir_i / dist_i` over the graph cells
    /// within the organism's sensing depth (`design/fauna-v2.md` "Controller v2"), with
    /// `dir_i` the unit vector toward cell i's center via `unfold` and `dist_i` its surface
    /// distance. `grad_d` is the gradient of edible detritus, matching `d_here`.
    pub grad_p: Vec2,
    pub grad_f: Vec2,
    pub grad_d: Vec2,
    /// Crowding repulsion: `Σ (own − other)/|own − other|² · (extent_sum)` over neighbors
    /// closer than the sum of body extents plus 1 px (positions from the pair pass).
    pub repulsion: Vec2,
    /// The embedded height of the organism's position (Top = 1, rim = −1) and the unit
    /// chart direction of increasing height; `up` is zero on the level top face.
    pub height: f64,
    pub up: Vec2,
    /// Standard-normal 2-vector for this tick's OU update (from `Stream::OrganismTurn`).
    pub noise: Vec2,
}

/// What the organism requests this tick; physiology and settlement apply limits.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub mode: Mode,
    /// The **ordinary** mode this tick's hysteresis produced, before any quiet override.
    ///
    /// Equal to `mode` whenever no override applies, which is every tick of an Off world. While
    /// a `post_birth_pause_v1` pause holds, `mode` is the imposed `Resting` and this is what the
    /// controller would otherwise have chosen — the value the pause carries forward so release
    /// resumes ordinary hysteresis instead of latching in the rest band (`crate::quiet`).
    pub underlying_mode: Mode,
    /// New unit heading after the bounded turn.
    pub heading: Vec2,
    /// Updated OU vector.
    pub ou: Vec2,
    /// Movement effort in `[0, 1]`.
    pub effort: f64,
    /// Fruit, grazing and scavenging intake efforts in `[0, 1]` (zero unless Feeding).
    pub fruit_effort: f64,
    pub graze_effort: f64,
    pub scavenge_effort: f64,
    /// Request to begin gestation (capacity and escrow checks happen in the world).
    pub bud: bool,
    pub hunger_memory: f64,
}

/// The per-mode turn gate `k` (spec: "Turning is gated by mode"). Seeking is always 1; the
/// other two modes carry the world config's `organism.rest_turn_fraction` and
/// `organism.feed_turn_fraction`. Both at 1 is the ungated behaviour of the E2 batches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnGate {
    pub feed: f64,
    pub rest: f64,
}

impl TurnGate {
    /// Full turning in every mode: the pre-gating controller.
    pub const UNGATED: TurnGate = TurnGate { feed: 1.0, rest: 1.0 };

    /// The gate a world config asks for.
    pub fn from_config(cfg: &OrganismConfig) -> Self {
        TurnGate { feed: cfg.feed_turn_fraction, rest: cfg.rest_turn_fraction }
    }

    /// `k` for the mode decided this tick.
    fn fraction(self, mode: Mode) -> f64 {
        match mode {
            Mode::Seeking => 1.0,
            Mode::Feeding => self.feed,
            Mode::Resting => self.rest,
        }
    }
}

impl Default for TurnGate {
    /// The default world config's gate, not the ungated one.
    fn default() -> Self {
        TurnGate::from_config(&OrganismConfig::default())
    }
}

/// Grazing (producer and fruit) needs at least this much `diet`; scavenging needs at most
/// `1 − DIET_GATE`.
pub const DIET_GATE: f64 = 0.05;
/// Fruit is eaten only by organisms with `diet ≥ FRUIT_DIET`.
pub const FRUIT_DIET: f64 = 0.5;

/// Normative rules (`design/m2-world-spec.md` "Controller" and `design/fauna-v2.md`
/// "Controller v2"):
/// - `h = hunger()`, `m_h += (1 − exp(−dt/τ)) (h − m_h)`.
/// - food gates: `can_graze` when grazing is on, `diet ≥ 0.05` and `p_here ≥ feed_min`;
///   `can_fruit` when grazing is on, `diet ≥ 0.5` and `f_here ≥ feed_min`; `can_scavenge`
///   when scavenging is on, `diet ≤ 0.95` and `d_here ≥ feed_min` (`d_here` is `D_eff`).
/// - mode: Resting→Seeking when `m_h > seek_on`; Seeking/Feeding→Resting when `m_h < seek_off`;
///   Seeking→Feeding when any food gate holds; Feeding→Seeking when none does.
/// - turn gate: `k = 1` while Seeking, `k = feed_turn_fraction` while Feeding, `k =
///   rest_turn_fraction` while Resting, using the mode decided this tick.
/// - OU: `ou' = ou · (1 − dt/τ_ou) + noise · turn_noise · sqrt(dt) · k`, with `τ_ou = 2 s`.
/// - steering `s = diet · w_food · h · (grad_p + grad_f) + (1 − diet) · w_detritus · h · grad_d +
///   w_crowd · repulsion + w_depth · (h_pref − height) · up + w_persist · ou'`;
///   if `|s| < 1e-9` keep the heading; else rotate the heading toward `s` by at most
///   `turn_rate_max · dt · k` (radians) and renormalize. A zero budget keeps the heading
///   exactly, so a resting body holds its orientation while its noise decays.
/// - effort: Seeking 1, Feeding `feed_effort`, Resting `rest_effort`.
/// - intake efforts: Feeding with `can_fruit` → `fruit_effort = 1`; with `can_graze` →
///   `graze_effort = 1`; with `can_scavenge` → `scavenge_effort = 1` (any may be 1 together).
/// - `bud` when `reserve ≥ bud_reserve · R_max`, `energy ≥ bud_energy · E_max`,
///   `age ≥ bud_min_age`, no escrow.
pub fn decide(
    org: &Organism,
    obs: &Observation,
    now: u64,
    dt: f64,
    grazing: bool,
    scavenging: bool,
    gate: TurnGate,
) -> Decision {
    decide_quiet(org, obs, now, dt, grazing, scavenging, gate, None)
}

/// [`decide`] with the optional ordinary-quiet override of `crate::quiet`.
///
/// `None` is the ordinary controller, byte for byte: every expression below reduces to what it
/// was before this parameter existed, and an Off world always passes `None`.
///
/// With `Some(q)`, two things change and nothing else:
///
/// 1. the hysteresis reads `q.underlying` instead of `org.mode`, because during a pause the
///    organism's public mode is the imposed `Resting` and feeding that back in would latch it;
/// 2. when `q.hold` is set, the ordinary result is computed first and recorded in
///    [`Decision::underlying_mode`], and *then* `Resting` is imposed — **before** the turn gate,
///    the effort and the intake/bud selection all read the mode. The release tick passes
///    `hold: false`, which substitutes the underlying mode and imposes nothing.
///
/// The noise pair in `obs` is drawn by the caller either way, so an override consumes no extra
/// RNG and shifts no stream.
#[allow(clippy::too_many_arguments)]
pub fn decide_quiet(
    org: &Organism,
    obs: &Observation,
    now: u64,
    dt: f64,
    grazing: bool,
    scavenging: bool,
    gate: TurnGate,
    quiet: Option<crate::quiet::QuietOverride>,
) -> Decision {
    let d = &org.phenotype.drives;
    let h = org.hunger();
    let diet = org.phenotype.diet;

    // Hunger memory: a first-order lag toward the instantaneous hunger.
    let tau = f64::from(d.tau_hunger_seconds).max(1e-9);
    let hunger_memory = org.hunger_memory + (1.0 - (-dt / tau).exp()) * (h - org.hunger_memory);

    // Mode with hysteresis, then the feeding sub-state from what the own cell holds and
    // what this diet can digest.
    let feed_min = f64::from(d.feed_min);
    let can_graze = grazing && diet >= DIET_GATE && obs.p_here >= feed_min;
    let can_fruit = grazing && diet >= FRUIT_DIET && obs.f_here >= feed_min;
    let can_scavenge = scavenging && diet <= 1.0 - DIET_GATE && obs.d_here >= feed_min;
    let any_food = can_graze || can_fruit || can_scavenge;
    let mut mode = quiet.map_or(org.mode, |q| q.underlying);
    if mode == Mode::Resting {
        if hunger_memory > f64::from(d.seek_on) {
            mode = Mode::Seeking;
        }
    } else if hunger_memory < f64::from(d.seek_off) {
        mode = Mode::Resting;
    }
    if mode == Mode::Seeking && any_food {
        mode = Mode::Feeding;
    } else if mode == Mode::Feeding && !any_food {
        mode = Mode::Seeking;
    }

    // The ordinary answer is settled here, and it is the one the pause carries forward. Only
    // now may the override repaint the mode the rest of this function reads.
    let underlying_mode = mode;
    let held = quiet.is_some_and(|q| q.hold);
    if held {
        mode = Mode::Resting;
    }

    // Turning is gated by the mode decided this tick: the noise injection and the turn
    // budget are both scaled by `k`, so a resting body holds its heading while the noise
    // it already carries decays.
    let k = gate.fraction(mode);

    // Ornstein–Uhlenbeck turn noise; the caller supplies the standard normal pair.
    let ou = org.ou * (1.0 - dt / OU_TAU) + obs.noise * (f64::from(d.turn_noise) * dt.sqrt() * k);

    let steer = (obs.grad_p + obs.grad_f) * (diet * f64::from(d.w_food) * h)
        + obs.grad_d * ((1.0 - diet) * f64::from(d.w_detritus) * h)
        + obs.repulsion * f64::from(d.w_crowd)
        + obs.up * (f64::from(d.w_depth) * (org.phenotype.h_pref - obs.height))
        + ou * f64::from(d.w_persist);
    let heading =
        turn_toward(org.heading, steer, f64::from(d.turn_rate_max_deg).to_radians() * dt * k);

    let effort = match mode {
        Mode::Seeking => 1.0,
        Mode::Feeding => f64::from(d.feed_effort),
        Mode::Resting => f64::from(d.rest_effort),
    };
    let feeding = mode == Mode::Feeding;
    // A held tick cannot be Feeding, so these are already zero; the explicit guard states the
    // contract rather than relying on that, and covers a future mode the override might impose.
    let fruit_effort = if feeding && can_fruit && !held { 1.0 } else { 0.0 };
    let graze_effort = if feeding && can_graze && !held { 1.0 } else { 0.0 };
    let scavenge_effort = if feeding && can_scavenge && !held { 1.0 } else { 0.0 };

    let age_seconds = org.age_ticks(now) as f64 * dt;
    // New budding requests are suppressed until release. The completed child's stores and birth
    // event are untouched; this refuses a *new* gestation, it does not cancel one.
    let bud = !held
        && org.escrow.is_none()
        && org.reserve >= f64::from(d.bud_reserve) * org.phenotype.reserve_max
        && org.energy >= f64::from(d.bud_energy) * org.phenotype.energy_max
        && age_seconds >= f64::from(d.bud_min_age_seconds);

    Decision {
        mode,
        underlying_mode,
        heading,
        ou,
        effort,
        fruit_effort,
        graze_effort,
        scavenge_effort,
        bud,
        hunger_memory,
    }
}

/// Relaxation time of the turn-noise process, in seconds (spec: `τ_ou = 2 s`).
const OU_TAU: f64 = 2.0;

/// Steering strength below which the heading is left alone.
const STEER_EPS: f64 = 1e-9;

/// Rotate `heading` toward `steer` by at most `max_turn` radians, returning a unit vector.
/// A budget of zero (a fully gated mode) returns the heading untouched rather than a
/// round trip through its angle, so a held heading is held exactly.
///
/// The hunter extension's escape term reuses this with its own, larger budget
/// (`crate::hunter`), so a fleeing prey turns by the same rule the controller turns by.
pub(crate) fn turn_toward(heading: Vec2, steer: Vec2, max_turn: f64) -> Vec2 {
    if max_turn <= 0.0 || steer.length() < STEER_EPS {
        return heading;
    }
    let Some(target) = steer.normalized() else {
        return heading;
    };
    let Some(current) = heading.normalized() else {
        return target;
    };
    let delta = wrap_pi(target.screen_angle() - current.screen_angle());
    if delta.abs() <= max_turn {
        return target;
    }
    Vec2::from_screen_angle(current.screen_angle() + delta.signum() * max_turn)
}

/// Wrap an angle into `(-π, π]`.
fn wrap_pi(a: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    let mut a = a % TAU;
    if a > PI {
        a -= TAU;
    } else if a < -PI {
        a += TAU;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    use cubarium_surface::{Face, SurfacePoint};

    use crate::DT;
    use crate::config::DriveConfig;
    use crate::genome::{Drives, Genome, Phenotype};
    use crate::organism::{Escrow, Organism, Origin};
    use crate::rng::Counter;

    /// The gate the default world config asks for (rest 0, feed 0.1).
    fn gate() -> TurnGate {
        TurnGate::default()
    }

    fn drives() -> Drives {
        let c = DriveConfig::default();
        Drives {
            w_food: c.w_food as f32,
            w_detritus: c.w_detritus as f32,
            w_persist: c.w_persist as f32,
            w_crowd: c.w_crowd as f32,
            seek_on: c.seek_on as f32,
            seek_off: c.seek_off as f32,
            feed_min: c.feed_min as f32,
            rest_effort: c.rest_effort as f32,
            feed_effort: c.feed_effort as f32,
            bud_reserve: c.bud_reserve as f32,
            bud_energy: c.bud_energy as f32,
            bud_min_age_seconds: c.bud_min_age_seconds as f32,
            tau_hunger_seconds: c.tau_hunger_seconds as f32,
            turn_rate_max_deg: c.turn_rate_max_deg as f32,
            turn_noise: c.turn_noise as f32,
            w_depth: c.w_depth as f32,
        }
    }

    fn genome() -> Genome {
        Genome {
            version: Genome::VERSION,
            size: 1.0,
            metabolism: 1.0,
            sense: 8.0,
            reserve: 1.0,
            mouth: 1.0,
            speed: 1.0,
            hue: 0.5,
            diet: 1.0,
            depth: 0.5,
            swim: 0.0,
            form: 1,
            drives: drives(),
        }
    }

    fn phenotype() -> Phenotype {
        Phenotype {
            structure_adult: 1.0,
            reserve_max: 1.0,
            energy_max: 2.0,
            speed_max: 1.5,
            mouth_rate: 0.05,
            sense_radius: 8.0,
            maintenance: 0.005,
            lobes: vec![(0.0, 0.0, 1.4)],
            extent: 1.4,
            hue: 0.5,
            // A pure grazer at its preferred height, so the v1 tests read unchanged: the
            // diet weights on `∇P` are 1 and the depth term is zero.
            graze_rate: 0.05,
            scavenge_rate: 0.0,
            diet: 1.0,
            h_pref: 0.0,
            swim: 0.0,
            form: 1,
            drives: drives(),
        }
    }

    fn organism(mode: Mode, reserve: f64, energy: f64, hunger_memory: f64) -> Organism {
        Organism {
            pos: SurfacePoint::new(Face::Front, 10.0, 10.0),
            heading: Vec2::new(1.0, 0.0),
            ou: Vec2::ZERO,
            structure: 1.0,
            reserve,
            energy,
            born_tick: 0,
            hunger_memory,
            mode,
            escrow: None,
            births: 0,
            genome: genome(),
            phenotype: phenotype(),
            parent: None,
            origin: Origin::Founder,
            turn_counter: Counter::default(),
            fed_this_tick: false,
        }
    }

    #[test]
    fn hunger_memory_relaxes_toward_instantaneous_hunger() {
        let org = organism(Mode::Resting, 0.0, 2.0, 0.0);
        let d = decide(&org, &Observation::default(), 0, DT, true, true, gate());
        let expected = (1.0 - (-DT / 10.0f64).exp()) * 1.0;
        assert!((d.hunger_memory - expected).abs() < 1e-15, "{} vs {expected}", d.hunger_memory);
    }

    #[test]
    fn mode_hysteresis_uses_seek_on_and_seek_off() {
        // Starving (h = 1) just below the on-threshold: still resting.
        let org = organism(Mode::Resting, 0.0, 2.0, 0.29);
        assert_eq!(decide(&org, &Observation::default(), 0, DT, true, true, gate()).mode, Mode::Resting);

        // At the threshold the lag pushes the memory above it: resting -> seeking.
        let org = organism(Mode::Resting, 0.0, 2.0, 0.30);
        assert_eq!(decide(&org, &Observation::default(), 0, DT, true, true, gate()).mode, Mode::Seeking);

        // Full (h = 0) above the off-threshold: still seeking.
        let org = organism(Mode::Seeking, 1.0, 2.0, 0.11);
        assert_eq!(decide(&org, &Observation::default(), 0, DT, true, true, gate()).mode, Mode::Seeking);

        // At the off-threshold the lag pushes the memory below it: seeking -> resting.
        let org = organism(Mode::Seeking, 1.0, 2.0, 0.10);
        assert_eq!(decide(&org, &Observation::default(), 0, DT, true, true, gate()).mode, Mode::Resting);
    }

    #[test]
    fn feeding_requires_food_in_the_own_cell_and_its_mechanism() {
        // An omnivore (`diet` 0.5) so both channels pass their diet gates.
        let mut org = organism(Mode::Seeking, 0.0, 2.0, 1.0);
        org.phenotype.diet = 0.5;
        let hungry = Observation { p_here: 1.0, ..Observation::default() };
        let d = decide(&org, &hungry, 0, DT, true, true, gate());
        assert_eq!(d.mode, Mode::Feeding);
        assert_eq!((d.graze_effort, d.scavenge_effort), (1.0, 0.0));

        // The same cell with grazing disabled keeps the organism seeking.
        let d = decide(&org, &hungry, 0, DT, false, true, gate());
        assert_eq!(d.mode, Mode::Seeking);
        assert_eq!((d.graze_effort, d.scavenge_effort), (0.0, 0.0));

        // Both channels can request at once (`d_here` is edible detritus, not raw D).
        let both = Observation { p_here: 1.0, d_here: 1.0, ..Observation::default() };
        let d = decide(&org, &both, 0, DT, true, true, gate());
        assert_eq!((d.graze_effort, d.scavenge_effort), (1.0, 1.0));

        // Feeding falls back to seeking when the cell empties below feed_min.
        let mut feeding = organism(Mode::Feeding, 0.0, 2.0, 1.0);
        feeding.phenotype.diet = 0.5;
        let d = decide(&feeding, &Observation::default(), 0, DT, true, true, gate());
        assert_eq!(d.mode, Mode::Seeking);
    }

    #[test]
    fn effort_follows_mode() {
        // The drives are f32, so the efforts come back as the widened f32 values.
        let resting = organism(Mode::Resting, 1.0, 2.0, 0.0);
        assert_eq!(decide(&resting, &Observation::default(), 0, DT, true, true, gate()).effort, f64::from(0.05f32));

        let seeking = organism(Mode::Seeking, 0.0, 2.0, 1.0);
        assert_eq!(decide(&seeking, &Observation::default(), 0, DT, true, true, gate()).effort, 1.0);

        let obs = Observation { p_here: 1.0, ..Observation::default() };
        assert_eq!(decide(&seeking, &obs, 0, DT, true, true, gate()).effort, f64::from(0.2f32));
    }

    #[test]
    fn turn_is_rate_limited_toward_strong_steering() {
        // Repulsion straight "up" on screen (-v) against a heading along +u.
        let org = organism(Mode::Resting, 1.0, 2.0, 0.0);
        let obs = Observation { repulsion: Vec2::new(0.0, -1000.0), ..Observation::default() };
        let d = decide(&org, &obs, 0, DT, true, true, TurnGate::UNGATED);
        let limit = 90.0f64.to_radians() * DT;
        assert!((d.heading.length() - 1.0).abs() < 1e-12);
        assert!((d.heading.screen_angle() - limit).abs() < 1e-12, "{}", d.heading.screen_angle());

        // Reaching the target within one tick's budget snaps exactly onto it.
        let tiny = Vec2::new(1.0, -0.01);
        let obs = Observation { repulsion: tiny, ..Observation::default() };
        let d = decide(&org, &obs, 0, DT, true, true, TurnGate::UNGATED);
        let expected = tiny.normalized().unwrap();
        assert!((d.heading - expected).length() < 1e-12);

        // Negligible steering leaves the heading untouched.
        let d = decide(&org, &Observation::default(), 0, DT, true, true, gate());
        assert_eq!(d.heading, Vec2::new(1.0, 0.0));
    }

    #[test]
    fn bud_gating() {
        // Thresholds come from the phenotype's drives, never from literals: the config
        // defaults are tuning hypotheses and move between milestones.
        let p = phenotype();
        let reserve_needed = f64::from(p.drives.bud_reserve) * p.reserve_max;
        let energy_needed = f64::from(p.drives.bud_energy) * p.energy_max;
        let age_needed = (f64::from(p.drives.bud_min_age_seconds) / DT).round() as u64;
        let ready = organism(Mode::Resting, reserve_needed, energy_needed, 0.0);
        assert!(decide(&ready, &Observation::default(), age_needed, DT, true, true, gate()).bud);

        // One tick too young.
        assert!(!decide(&ready, &Observation::default(), age_needed - 1, DT, true, true, gate()).bud);
        // Reserve just below bud_reserve · R_max.
        let lean = organism(Mode::Resting, reserve_needed * 0.999, energy_needed, 0.0);
        assert!(!decide(&lean, &Observation::default(), age_needed, DT, true, true, gate()).bud);
        // Energy just below bud_energy · E_max.
        let tired = organism(Mode::Resting, reserve_needed, energy_needed * 0.999, 0.0);
        assert!(!decide(&tired, &Observation::default(), age_needed, DT, true, true, gate()).bud);
        // Already gestating.
        let mut gestating = organism(Mode::Resting, reserve_needed, energy_needed, 0.0);
        gestating.escrow = Some(Escrow {
            structure: 0.4,
            reserve: 0.2,
            energy: 0.5,
            started_tick: 0,
            genome: genome(),
        });
        assert!(!decide(&gestating, &Observation::default(), age_needed, DT, true, true, gate()).bud);
    }
}
