//! Opt-in ordinary quiet behaviour: the `post_birth_pause_v1` candidate
//! (`design/7_Research/astra-ordinary-quiet-experiment-proposal-2026-09-13.md`).
//!
//! **Off by default, and Off is inert.** [`QuietState::default`] is policy [`QuietPolicy::Off`]
//! with no entries, and a world carrying it executes the pre-quiet tick operation for operation.
//! Nothing here is a canonical decision, an accepted duration, or a claim that the candidate is
//! viable; it exists so a copied-world experiment can measure one fixed rule.
//!
//! ## What the candidate is
//!
//! After an **actual paid child insertion** completes boundary `B`, the surviving *ordinary*
//! parent may hold exactly [`POST_BIRTH_PAUSE_TICKS`] decisions: `B` through `B + 39`, producing
//! completed Resting intervals `B + 1` through `B + 40`. The decision at `B + 40` is ordinary
//! again. The birth's own tick is never retroactively repainted.
//!
//! It is **interrupted activity**, not satiation, healing, free energy or a rest pose. Every
//! ordinary bill — maintenance, sensing, the actual movement it still performs, growth,
//! oxidation, the death checks — runs in its usual phase at its usual amount. Hunger and hunger
//! memory keep telling the truth, and age is not suspended.
//!
//! ## What it is not allowed to become
//!
//! * **Not a debit.** [`Budget`] is an affordability *test* evaluated at admission and before
//!   every held decision. It charges nothing, escrows nothing and heals nothing.
//! * **Not a hysteresis latch.** Writing `Mode::Resting` into an organism and clearing a timer
//!   would leave it resting inside the `seek_off..seek_on` band after the promised pause. So the
//!   entry retains the [`QuietPause::underlying`] ordinary mode, the ordinary transition is
//!   evaluated from *that* every tick, and release hands the ordinary controller the underlying
//!   value rather than the imposed one.
//! * **Not a second controller.** One ordinary decision is computed per organism per tick, from
//!   the same observation and the same noise draws; the override is applied to its result.
//!
//! Hunter worlds are excluded from this slice by [`QuietState::validate`]: threat and escape
//! interactions need their own explicit contract before the two can be combined.

use serde::{Deserialize, Serialize};

use crate::config::OrganismConfig;
use crate::ids::OrganismId;
use crate::organism::{Mode, Organism};

/// Wire version of [`QuietState`]. A saved extension with any other version is refused rather
/// than reinterpreted.
pub const QUIET_VERSION: u32 = 1;

/// The candidate's one fixed duration: two seconds at 20 Hz. An **unvalidated** candidate, not
/// an accepted default, and deliberately not configurable — a sweep is a different experiment.
pub const POST_BIRTH_PAUSE_TICKS: u64 = 40;

/// The extra ordinary tick the conservative budget always reserves, so the affordability test
/// leaves a small positive margin rather than landing exactly on the boundary.
pub const SAFETY_MARGIN_TICKS: u64 = 1;

/// Which ordinary quiet rule a world runs. Reference is [`QuietPolicy::Off`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuietPolicy {
    /// The unchanged autonomous controller. Inert: no entry may exist, and no code path here
    /// runs.
    #[default]
    Off,
    /// One affordable two-second pause after a real paid birth.
    PostBirthPauseV1,
}

impl QuietPolicy {
    pub fn enabled(self) -> bool {
        !matches!(self, QuietPolicy::Off)
    }

    /// A stable label for a manifest, so a recorded experiment states its policy in words.
    pub fn as_str(self) -> &'static str {
        match self {
            QuietPolicy::Off => "off",
            QuietPolicy::PostBirthPauseV1 => "post_birth_pause_v1",
        }
    }
}

/// Why a pause did not start, or did not finish. Carried on the transient records so a harness
/// can separate "never offered" from "offered and refused" from "started and abandoned".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuietReason {
    /// The conservative budget was not met at admission.
    Unaffordable,
    /// The conservative budget stopped being met before a held decision.
    UnaffordableRemaining,
    /// The parent is no longer alive.
    ParentGone,
    /// The bound on live entries is full.
    Bounded,
    /// A pause for this parent was already open.
    AlreadyPaused,
    /// A hunter member is never offered this ordinary behaviour in this slice.
    HunterMember,
    /// One or more of the budget's inputs was not a finite nonnegative number.
    InvalidInputs,
    /// The tick arithmetic would overflow.
    Overflow,
}

impl QuietReason {
    pub fn as_str(self) -> &'static str {
        match self {
            QuietReason::Unaffordable => "unaffordable",
            QuietReason::UnaffordableRemaining => "unaffordable_remaining",
            QuietReason::ParentGone => "parent_gone",
            QuietReason::Bounded => "bounded",
            QuietReason::AlreadyPaused => "already_paused",
            QuietReason::HunterMember => "hunter_member",
            QuietReason::InvalidInputs => "invalid_inputs",
            QuietReason::Overflow => "overflow",
        }
    }
}

/// Transient measurement records. Drained by the observer exactly as life and hunter events are:
/// never checkpointed, never hashed, never read back by the tick.
///
/// These are for a harness, not for the display. `design/0_Canon` keeps the ambient image free of
/// analytical overlays; normal art may use the actual `Resting` mode and nothing here.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum QuietEvent {
    /// A pause was admitted. `end_tick` is the first ordinary decision boundary again.
    Begin {
        tick: u64,
        parent: OrganismId,
        child: OrganismId,
        end_tick: u64,
        underlying: Mode,
    },
    /// A pause was offered by a real birth and not admitted. The opportunity is forgotten; a
    /// hungry parent is never made to wait for permission to act.
    Refuse {
        tick: u64,
        parent: OrganismId,
        child: OrganismId,
        reason: QuietReason,
    },
    /// The pause ran its full length and released at its expiry boundary.
    End {
        tick: u64,
        parent: OrganismId,
        child: OrganismId,
        completed_ticks: u64,
        underlying: Mode,
    },
    /// The pause stopped early. The parent uses the ordinary controller on this same tick.
    Abort {
        tick: u64,
        parent: OrganismId,
        child: OrganismId,
        completed_ticks: u64,
        reason: QuietReason,
    },
}

/// One held pause. Full generational IDs on both sides, the originating birth identity, the
/// timer and the retained ordinary mode — everything a restart needs to continue exactly.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuietPause {
    /// The parent, by slot **and** generation: a stale handle never resolves to a reused slot.
    pub parent: OrganismId,
    /// The child whose actual insertion offered this pause.
    pub child: OrganismId,
    /// The completed boundary `B` the birth finished at, and the first held decision.
    pub start_tick: u64,
    /// `B + POST_BIRTH_PAUSE_TICKS`: the first ordinary decision boundary again. The decision at
    /// this tick is **not** held.
    pub end_tick: u64,
    /// The ordinary mode the controller would be in, carried across the pause so release does
    /// not read the imposed `Resting` back out of the organism.
    pub underlying: Mode,
}

impl QuietPause {
    /// Held decisions remaining at decision boundary `now`, including this one. Zero once the
    /// pause has expired.
    pub fn remaining(&self, now: u64) -> u64 {
        self.end_tick.saturating_sub(now.max(self.start_tick))
    }

    /// Held decisions already completed before boundary `now`.
    pub fn completed(&self, now: u64) -> u64 {
        now.saturating_sub(self.start_tick).min(POST_BIRTH_PAUSE_TICKS)
    }

    /// Whether the decision at boundary `now` is held by this entry.
    pub fn holds(&self, now: u64) -> bool {
        (self.start_tick..self.end_tick).contains(&now)
    }
}

/// The versioned, opt-in extension. Appended to `WorldState` in schema 13, outside every legacy
/// nested organism and config payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuietState {
    pub version: u32,
    pub policy: QuietPolicy,
    /// Open pauses, bounded by the world's live organism capacity and kept in ascending parent
    /// order so the encoding is a property of the set and not of insertion history.
    pub pauses: Vec<QuietPause>,
}

impl Default for QuietState {
    fn default() -> Self {
        QuietState { version: QUIET_VERSION, policy: QuietPolicy::Off, pauses: Vec::new() }
    }
}

impl QuietState {
    /// The candidate, chosen explicitly by an experiment initializer before the world is built.
    ///
    /// There is deliberately no setter on `World`: the proposal asks for the policy to be
    /// chosen once, snapshotted, and never hot-toggled, so a caller writes this into a
    /// `WorldState` and constructs from it.
    pub fn post_birth_pause_v1() -> QuietState {
        QuietState {
            version: QUIET_VERSION,
            policy: QuietPolicy::PostBirthPauseV1,
            pauses: Vec::new(),
        }
    }

    /// True when this extension can do anything at all. An Off world with no entries never
    /// enters a quiet code path.
    pub fn active(&self) -> bool {
        self.policy.enabled()
    }

    pub fn find(&self, parent: OrganismId) -> Option<&QuietPause> {
        self.pauses.iter().find(|p| p.parent == parent)
    }

    /// Range checks after decode, called from `WorldState::validate`.
    ///
    /// `tick` is the state's own tick and `cap` its organism capacity; `slots` resolves a live
    /// organism, so a stale or reused handle is caught rather than silently re-paused. `hunters`
    /// says whether the world carries a hunter extension, which this slice refuses to combine
    /// with an enabled policy.
    pub fn validate(
        &self,
        tick: u64,
        cap: usize,
        hunters_active: bool,
        alive: impl Fn(OrganismId) -> bool,
    ) -> Result<(), String> {
        if self.version != QUIET_VERSION {
            return Err(format!("quiet extension version {} is not {QUIET_VERSION}", self.version));
        }
        if !self.policy.enabled() {
            // Off is inert, and an Off world carrying entries is a contradiction rather than a
            // harmless leftover: something wrote a timer nothing will ever run.
            if !self.pauses.is_empty() {
                return Err(format!(
                    "quiet policy is off but {} pause(s) are held",
                    self.pauses.len()
                ));
            }
            return Ok(());
        }
        if hunters_active {
            return Err(
                "an enabled ordinary quiet policy and a hunter extension are not combined in \
                 this slice; threat and escape interactions need their own contract"
                    .to_string(),
            );
        }
        if self.pauses.len() > cap {
            return Err(format!(
                "quiet holds {} pauses, more than the world's {cap} organism capacity",
                self.pauses.len()
            ));
        }
        let mut previous: Option<OrganismId> = None;
        for p in &self.pauses {
            // Ascending and strictly increasing: ordering is the canonical form, and it also
            // makes a duplicate parent impossible to express.
            if let Some(before) = previous
                && (before.slot, before.generation) >= (p.parent.slot, p.parent.generation)
            {
                return Err(format!(
                    "quiet pauses are not in ascending parent order at {:?}",
                    p.parent
                ));
            }
            previous = Some(p.parent);
            if p.parent == p.child {
                return Err(format!("quiet pause {:?} is its own child", p.parent));
            }
            if !alive(p.parent) {
                return Err(format!("quiet pause names {:?}, which is not alive", p.parent));
            }
            if p.end_tick != p.start_tick.saturating_add(POST_BIRTH_PAUSE_TICKS) {
                return Err(format!(
                    "quiet pause {:?} runs {} ticks, not {POST_BIRTH_PAUSE_TICKS}",
                    p.parent,
                    p.end_tick.wrapping_sub(p.start_tick)
                ));
            }
            if p.start_tick.checked_add(POST_BIRTH_PAUSE_TICKS).is_none() {
                return Err(format!("quiet pause {:?} overflows its end tick", p.parent));
            }
            // A pause that started after now would apply a timer the world has not reached.
            if p.start_tick > tick {
                return Err(format!(
                    "quiet pause {:?} starts at {} after tick {tick}",
                    p.parent, p.start_tick
                ));
            }
            // `end_tick == tick` is **valid and necessary**: the last held decision has been
            // made, the fortieth interval is complete, and the entry is now carrying exactly one
            // thing — the underlying ordinary mode the release at this boundary must resume
            // from. It is consumed by that decision and gone afterwards. A world snapshotted at
            // its expiry boundary would otherwise be unloadable, and its release would have to
            // read the imposed `Resting` back out of the organism, which is the latch this
            // design exists to avoid. Only a pause the world has already stepped *past* is
            // stale.
            if p.end_tick < tick {
                return Err(format!(
                    "quiet pause {:?} expired at {} and was not released by tick {tick}",
                    p.parent, p.end_tick
                ));
            }
        }
        Ok(())
    }
}

/// The conservative no-food budget of the proposal, evaluated over a horizon `h` **in seconds**.
///
/// ```text
/// Sbound = max(S, Sa)
/// B = m·Sbound + move_cost·Sbound·(f·vmax) + sense_cost·r      [energy/second]
/// G = min(growth_rate·h, max(0, Sa − S))                       [material]
/// Q = oxidation_rate·h                                         [material]
/// admit/continue only if  E > B·h + build_cost·G  and  R > Q + G
/// ```
///
/// It is deliberately pessimistic on both sides. `Sbound` assumes the body may grow to adult
/// before it pays; `f·vmax` is the fastest a resting body could move, because wading only ever
/// divides speed; `G` assumes growth runs at its ceiling the whole time; `Q` assumes oxidation
/// burns at its ceiling — and the energy oxidation *returns* is not counted. The result is a
/// bound on the cost of missing food for `h` seconds under the present ordinary core, not a
/// survival proof: age death is still possible, and nothing here is charged or reserved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Budget {
    /// Energy per second of upkeep, movement at rest effort, and sensing.
    pub per_second: f64,
    /// Material that growth could consume over the horizon.
    pub growth: f64,
    /// Material that oxidation could burn over the horizon.
    pub oxidation: f64,
    /// Energy the horizon could cost in total.
    pub energy: f64,
    /// Material the horizon could cost in total.
    pub material: f64,
}

impl Budget {
    /// The budget for `horizon_seconds`, or `None` when any input is not a finite nonnegative
    /// number. A world that cannot describe its own costs does not get to claim they are small.
    pub fn of(org: &Organism, cfg: &OrganismConfig, horizon_seconds: f64) -> Option<Budget> {
        let d = &org.phenotype.drives;
        let inputs = [
            horizon_seconds,
            org.structure,
            org.phenotype.structure_adult,
            org.phenotype.maintenance,
            org.phenotype.speed_max,
            org.phenotype.sense_radius,
            f64::from(d.rest_effort),
            cfg.move_cost,
            cfg.sense_cost,
            cfg.growth_rate,
            cfg.oxidation_rate,
            cfg.build_cost,
        ];
        if !inputs.iter().all(|x| x.is_finite() && *x >= 0.0) {
            return None;
        }
        let h = horizon_seconds;
        let sa = org.phenotype.structure_adult;
        let s_bound = org.structure.max(sa);
        let per_second = org.phenotype.maintenance * s_bound
            + cfg.move_cost * s_bound * (f64::from(d.rest_effort) * org.phenotype.speed_max)
            + cfg.sense_cost * org.phenotype.sense_radius;
        let growth = (cfg.growth_rate * h).min((sa - org.structure).max(0.0));
        let oxidation = cfg.oxidation_rate * h;
        let energy = per_second * h + cfg.build_cost * growth;
        let material = oxidation + growth;
        let budget = Budget { per_second, growth, oxidation, energy, material };
        // A bound that is not a finite nonnegative number bounds nothing.
        if ![per_second, growth, oxidation, energy, material]
            .iter()
            .all(|x| x.is_finite() && *x >= 0.0)
        {
            return None;
        }
        Some(budget)
    }

    /// Whether this organism can cover the horizon. Strict inequalities, as the proposal states:
    /// landing exactly on the bound is not covering it.
    pub fn affordable(&self, org: &Organism) -> bool {
        org.energy.is_finite()
            && org.reserve.is_finite()
            && org.energy > self.energy
            && org.reserve > self.material
    }
}

/// The horizon a pause must be able to cover before decision boundary `now`: the held decisions
/// remaining, plus [`SAFETY_MARGIN_TICKS`], in seconds.
///
/// `None` when the tick arithmetic would overflow, which is a refusal rather than a wrap.
pub fn horizon_seconds(remaining_ticks: u64, dt: f64) -> Option<f64> {
    let ticks = remaining_ticks.checked_add(SAFETY_MARGIN_TICKS)?;
    let seconds = ticks as f64 * dt;
    (seconds.is_finite() && seconds >= 0.0).then_some(seconds)
}

/// What the controller is asked to do with one organism's ordinary decision this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuietOverride {
    /// The ordinary mode the hysteresis is evaluated from — the retained underlying mode, never
    /// the imposed `Resting` sitting in the organism.
    pub underlying: Mode,
    /// True while the pause is held: impose `Resting` after the ordinary transition is computed,
    /// and suppress intake and budding. False on the release tick, where the underlying mode is
    /// substituted and nothing is imposed.
    pub hold: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldConfig;
    use crate::genome::{Genome, decode};
    use crate::ids::Slots;
    use crate::organism::{Origin, Organism};
    use crate::rng::Counter;
    use cubarium_surface::{Face, SurfacePoint, Vec2};

    fn organism(structure: f64, reserve: f64, energy: f64) -> Organism {
        let cfg = WorldConfig::default();
        let mut genome = Genome::founder(0.5, &cfg.drives);
        genome.clamp();
        let phenotype = decode(&genome, &cfg.organism);
        Organism {
            pos: SurfacePoint::new(Face::Top, 10.0, 10.0),
            heading: Vec2::new(1.0, 0.0),
            ou: Vec2::ZERO,
            structure,
            reserve,
            energy,
            born_tick: 0,
            hunger_memory: 0.5,
            mode: Mode::Seeking,
            escrow: None,
            births: 0,
            genome,
            phenotype,
            parent: None,
            origin: Origin::Founder,
            turn_counter: Counter::default(),
            fed_this_tick: false,
        }
    }

    fn id(slot: u32, generation: u32) -> OrganismId {
        OrganismId { slot, generation }
    }

    #[test]
    fn off_with_no_entries_is_the_inert_default() {
        let q = QuietState::default();
        assert_eq!(q.version, QUIET_VERSION);
        assert_eq!(q.policy, QuietPolicy::Off);
        assert!(q.pauses.is_empty());
        assert!(!q.active());
        assert_eq!(QuietPolicy::default(), QuietPolicy::Off);
        assert!(!QuietPolicy::Off.enabled());
        assert!(QuietPolicy::PostBirthPauseV1.enabled());
        assert_eq!(QuietPolicy::Off.as_str(), "off");
        assert_eq!(QuietPolicy::PostBirthPauseV1.as_str(), "post_birth_pause_v1");
        q.validate(0, 512, false, |_| true).expect("the default validates");
        // And it validates in a hunter world too: Off never conflicts with anything.
        q.validate(0, 512, true, |_| true).expect("off is compatible with hunters");
    }

    #[test]
    fn the_window_is_exactly_forty_decisions_from_the_birth_boundary() {
        assert_eq!(POST_BIRTH_PAUSE_TICKS, 40);
        let b = 1000;
        let p = QuietPause {
            parent: id(1, 1),
            child: id(2, 1),
            start_tick: b,
            end_tick: b + POST_BIRTH_PAUSE_TICKS,
            underlying: Mode::Seeking,
        };
        assert!(!p.holds(b - 1), "the birth's own tick is not repainted");
        assert!(p.holds(b), "the first held decision is at B");
        assert!(p.holds(b + 39), "the last held decision is at B+39");
        assert!(!p.holds(b + 40), "B+40 is ordinary again");
        assert_eq!((b..b + POST_BIRTH_PAUSE_TICKS).filter(|t| p.holds(*t)).count(), 40);
        assert_eq!(p.remaining(b), 40);
        assert_eq!(p.remaining(b + 1), 39);
        assert_eq!(p.remaining(b + 39), 1);
        assert_eq!(p.remaining(b + 40), 0);
        assert_eq!(p.completed(b), 0);
        assert_eq!(p.completed(b + 1), 1);
        assert_eq!(p.completed(b + 40), 40);
        assert_eq!(p.completed(b + 99), 40, "completion is capped at the window");
    }

    #[test]
    fn the_horizon_is_the_remaining_decisions_plus_one_ordinary_tick() {
        assert_eq!(SAFETY_MARGIN_TICKS, 1);
        assert_eq!(horizon_seconds(40, crate::DT), Some(41.0 * crate::DT));
        assert_eq!(horizon_seconds(1, crate::DT), Some(2.0 * crate::DT));
        assert_eq!(horizon_seconds(0, crate::DT), Some(crate::DT));
        // Overflow is a refusal, never a wrap into a tiny horizon.
        assert_eq!(horizon_seconds(u64::MAX, crate::DT), None);
        assert_eq!(horizon_seconds(10, f64::NAN), None);
        assert_eq!(horizon_seconds(10, f64::INFINITY), None);
        assert_eq!(horizon_seconds(10, -1.0), None);
    }

    #[test]
    fn the_budget_is_the_proposals_formula_and_nothing_else() {
        let cfg = WorldConfig::default();
        let o = organism(0.5, 1.0, 1.0);
        let h = 41.0 * crate::DT;
        let b = Budget::of(&o, &cfg.organism, h).expect("finite inputs");

        let d = &o.phenotype.drives;
        let sa = o.phenotype.structure_adult;
        let s_bound = o.structure.max(sa);
        let want_per_second = o.phenotype.maintenance * s_bound
            + cfg.organism.move_cost * s_bound * (f64::from(d.rest_effort) * o.phenotype.speed_max)
            + cfg.organism.sense_cost * o.phenotype.sense_radius;
        assert_eq!(b.per_second, want_per_second);
        assert_eq!(b.growth, (cfg.organism.growth_rate * h).min((sa - o.structure).max(0.0)));
        assert_eq!(b.oxidation, cfg.organism.oxidation_rate * h);
        assert_eq!(b.energy, b.per_second * h + cfg.organism.build_cost * b.growth);
        assert_eq!(b.material, b.oxidation + b.growth);

        // An adult's growth term is zero, and a bigger body bounds by its own structure.
        let adult = organism(sa, 1.0, 1.0);
        let ab = Budget::of(&adult, &cfg.organism, h).unwrap();
        assert_eq!(ab.growth, 0.0);
        let overgrown = organism(sa * 2.0, 1.0, 1.0);
        let ob = Budget::of(&overgrown, &cfg.organism, h).unwrap();
        assert_eq!(ob.growth, 0.0, "structure past adult never grows");
        assert!(ob.per_second > ab.per_second, "Sbound is max(S, Sa)");
    }

    #[test]
    fn affordability_is_strict_and_never_charges_anything() {
        let cfg = WorldConfig::default();
        let h = 41.0 * crate::DT;
        let probe = organism(0.5, 0.0, 0.0);
        let b = Budget::of(&probe, &cfg.organism, h).unwrap();

        // Exactly on the bound is not covering it, on either currency.
        let exact = organism(0.5, b.material, b.energy);
        assert!(!b.affordable(&exact), "landing on the bound is not affordability");
        let over = organism(0.5, b.material * 1.0001 + 1e-12, b.energy * 1.0001 + 1e-12);
        assert!(b.affordable(&over));
        let thin_energy = organism(0.5, b.material * 2.0, b.energy);
        assert!(!b.affordable(&thin_energy));
        let thin_reserve = organism(0.5, b.material, b.energy * 2.0);
        assert!(!b.affordable(&thin_reserve));

        // And the test leaves the organism untouched — it is a question, not a transaction.
        let before = over.clone();
        assert!(b.affordable(&over));
        assert_eq!(over.reserve, before.reserve);
        assert_eq!(over.energy, before.energy);
    }

    #[test]
    fn a_nonfinite_input_refuses_the_budget_rather_than_bounding_with_a_nan() {
        let cfg = WorldConfig::default();
        let h = 41.0 * crate::DT;
        for poison in [f64::NAN, f64::INFINITY, -1.0] {
            let mut o = organism(0.5, 1.0, 1.0);
            o.structure = poison;
            assert_eq!(Budget::of(&o, &cfg.organism, h), None, "structure {poison}");
            let mut o = organism(0.5, 1.0, 1.0);
            o.phenotype.maintenance = poison;
            assert_eq!(Budget::of(&o, &cfg.organism, h), None, "maintenance {poison}");
            let mut o = organism(0.5, 1.0, 1.0);
            o.phenotype.speed_max = poison;
            assert_eq!(Budget::of(&o, &cfg.organism, h), None, "speed {poison}");
            let mut bad = cfg.organism.clone();
            bad.build_cost = poison;
            assert_eq!(Budget::of(&organism(0.5, 1.0, 1.0), &bad, h), None, "build_cost {poison}");
            assert_eq!(Budget::of(&organism(0.5, 1.0, 1.0), &cfg.organism, poison), None);
        }
        // A finite budget over a nonfinite stock is still not affordable.
        let b = Budget::of(&organism(0.5, 1.0, 1.0), &cfg.organism, h).unwrap();
        let mut nan_stock = organism(0.5, 1.0, 1.0);
        nan_stock.energy = f64::NAN;
        assert!(!b.affordable(&nan_stock));
        nan_stock.energy = f64::INFINITY;
        nan_stock.reserve = f64::NAN;
        assert!(!b.affordable(&nan_stock));
    }

    #[test]
    fn a_hostile_extension_is_refused_one_property_at_a_time() {
        let sound = |start: u64| QuietPause {
            parent: id(3, 1),
            child: id(4, 1),
            start_tick: start,
            end_tick: start + POST_BIRTH_PAUSE_TICKS,
            underlying: Mode::Seeking,
        };
        let state = |pauses: Vec<QuietPause>| QuietState {
            version: QUIET_VERSION,
            policy: QuietPolicy::PostBirthPauseV1,
            pauses,
        };
        // The control really does validate.
        state(vec![sound(100)]).validate(120, 512, false, |_| true).expect("the control is valid");

        // A version this build does not know.
        let mut bad = state(vec![sound(100)]);
        bad.version = 2;
        assert!(bad.validate(120, 512, false, |_| true).is_err());

        // Off carrying an entry is a contradiction.
        let mut off = state(vec![sound(100)]);
        off.policy = QuietPolicy::Off;
        assert!(off.validate(120, 512, false, |_| true).is_err());

        // An enabled policy beside a hunter extension is refused in this slice.
        assert!(state(vec![sound(100)]).validate(120, 512, true, |_| true).is_err());

        // More entries than the world can hold organisms.
        let many: Vec<QuietPause> = (0..5)
            .map(|i| QuietPause { parent: id(i, 1), child: id(100 + i, 1), ..sound(100) })
            .collect();
        assert!(state(many.clone()).validate(120, 4, false, |_| true).is_err());
        state(many.clone()).validate(120, 5, false, |_| true).expect("exactly at the bound is fine");

        // Out of order, and therefore also duplicated.
        let mut swapped = many.clone();
        swapped.swap(0, 1);
        assert!(state(swapped).validate(120, 512, false, |_| true).is_err());
        let duplicate = vec![sound(100), sound(100)];
        assert!(state(duplicate).validate(120, 512, false, |_| true).is_err());

        // A parent that is not alive, and a parent that is its own child.
        assert!(state(vec![sound(100)]).validate(120, 512, false, |_| false).is_err());
        let mut selfish = sound(100);
        selfish.child = selfish.parent;
        assert!(state(vec![selfish]).validate(120, 512, false, |_| true).is_err());

        // A duration that is not the candidate's.
        for wrong in [0u64, 1, 39, 41, 4000] {
            let mut p = sound(100);
            p.end_tick = p.start_tick + wrong;
            assert!(
                state(vec![p]).validate(120, 512, false, |_| true).is_err(),
                "a {wrong}-tick pause must be refused"
            );
        }

        // Impossible ranges against the world's own clock.
        let future = sound(200);
        assert!(state(vec![future]).validate(120, 512, false, |_| true).is_err());
        let expired = sound(50);
        assert!(
            state(vec![expired]).validate(120, 512, false, |_| true).is_err(),
            "a pause the world stepped past should have been released"
        );
        // The exact boundaries. `start == tick` is the first held decision. `end == tick` is the
        // release boundary: the window is complete but the decision that consumes the entry has
        // not been made yet, so a world snapshotted there is valid and resumes exactly.
        state(vec![sound(120)]).validate(120, 512, false, |_| true).expect("start at now is held");
        state(vec![sound(120 - POST_BIRTH_PAUSE_TICKS)])
            .validate(120, 512, false, |_| true)
            .expect("end at now is awaiting its release");
        assert!(state(vec![sound(79)]).validate(120, 512, false, |_| true).is_err());

        // Tick arithmetic that would overflow.
        let mut huge = sound(0);
        huge.start_tick = u64::MAX - 5;
        huge.end_tick = u64::MAX;
        assert!(state(vec![huge]).validate(u64::MAX - 1, 512, false, |_| true).is_err());
    }

    #[test]
    fn a_stale_generation_does_not_resolve_to_a_reused_slot() {
        let mut slots: Slots<Organism> = Slots::with_capacity(8);
        let first = slots.insert(organism(0.5, 1.0, 1.0));
        slots.remove(first);
        let second = slots.insert(organism(0.5, 1.0, 1.0));
        assert_eq!(first.slot, second.slot, "the slot really was reused");
        assert_ne!(first.generation, second.generation);

        let stale = QuietState {
            version: QUIET_VERSION,
            policy: QuietPolicy::PostBirthPauseV1,
            pauses: vec![QuietPause {
                parent: first,
                child: id(9, 1),
                start_tick: 100,
                end_tick: 140,
                underlying: Mode::Seeking,
            }],
        };
        let alive = |q: OrganismId| slots.get(q).is_some();
        assert!(alive(second));
        assert!(!alive(first));
        assert!(
            stale.validate(120, 512, false, alive).is_err(),
            "a pause naming a retired generation must be refused"
        );
    }
}
