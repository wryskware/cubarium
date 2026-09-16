//! The apex **opportunity** audit: why no two introduced adults ever mate.
//!
//! The calibration screen ran 180 apex-bearing runs and recorded zero matings, zero births,
//! zero emergences and zero final survivors, then named the 10 px mating radius as the reason.
//! The predicate has six other terms. The screen recorded no minimum pair distance, no
//! simultaneously-ready time and no failure count, so early death, never being ready and never
//! meeting are one undivided outcome (`design/7_Research/ecology-v1-next-review-2026-09-15.md`,
//! finding 5 and next step 5). Asking the owner to move a constant on that evidence would be
//! asking them to guess.
//!
//! This command re-runs **exactly the two-apex arm the screen ran** — the same candidate
//! configuration, the same seeds, the same deterministic placements, the same introduction at
//! tick 6,000, never restocked — with the world's own
//! [`cubarium_core::encounter::ApexOpportunity`] counters on, and reports what each predicate
//! actually did.
//!
//! # Reading the result
//!
//! - `ticks_two_ready == 0` in every run: **no radius can matter.** Two adults were never
//!   simultaneously able to reproduce, so the distance between them was never consulted.
//! - `ticks_two_ready > 0` and `ticks_ready_pair_within_radius == 0`: a ready pair existed and
//!   never closed. The radius, or an encounter policy that brings adults together, is then a
//!   real choice, and `min_ready_distance_px` says how large a change would have to be.
//! - `fail_radius > 0` with `ticks_two_ready > 0`: the pass formed candidate pairs and refused
//!   them on distance alone. The same conclusion, reached from the other side.
//!
//! A candidate pair only exists when the two adults sensed each other; the census counts do
//! not require that. Both are reported, because a world where two ready adults never even
//! entered each other's sensing is a different world from one where they did and stayed 12 px
//! apart.
//!
//! # What this run is not
//!
//! It is not a calibration stage, it scores nothing, it writes no `evals.jsonl`, and it moves
//! no parameter. It changes nothing about the arm it re-runs: the counters are read-only
//! diagnostics with a hash test behind them (`cubarium-core/tests/body_budget.rs`).

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use std::collections::VecDeque;

use cubarium_core::encounter::ApexOpportunity;
use cubarium_core::hunter::{
    self, AttemptOutcome, FixedHunterProfile, HunterEvent, HunterPhase, StrikeClass, StrikeRecord,
};
use cubarium_core::organism::DeathCause;
use cubarium_core::{BodyBudget, OrganismId, World, WorldConfig};
use serde::{Deserialize, Serialize};

use crate::calibrate;
use crate::es::fixture::Ecology;
use crate::evaluate::{self, BUILD_ID};
use crate::search::HELDOUT_SEEDS;

type Boxed = Box<dyn std::error::Error>;

/// How often a running audit re-checks the world's own invariants. The calibration samples
/// every 500 ticks; this run has nothing to sample, so the check is the only reason to stop.
pub const VALIDATE_EVERY: u64 = 5_000;

/// How many ticks of the end of a life the tail window covers. The question the window answers
/// is "was this body still earning when it died", and 2,000 ticks is 100 s — long enough to
/// contain several strike/recovery cycles (`recovery_seconds` 5.0) and short enough that a
/// meal early in the life does not hide in it.
pub const TAIL_TICKS: u64 = 2_000;

/// Ticks one member spent in each hunt phase, and underground, over the life the audit
/// watched. The sum is its `lived_ticks`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseOccupancy {
    pub perched: u64,
    pub stalking: u64,
    pub windup: u64,
    pub strike: u64,
    pub recovering: u64,
    pub handling: u64,
    /// Ticks the member was a concealed offspring rather than an active body. An introduced
    /// founder is never dormant; a descendant begins that way.
    pub dormant: u64,
}

impl PhaseOccupancy {
    fn sample(&mut self, phase: HunterPhase, dormant: bool) {
        if dormant {
            self.dormant += 1;
            return;
        }
        match phase {
            HunterPhase::Perched => self.perched += 1,
            HunterPhase::Stalking => self.stalking += 1,
            HunterPhase::Windup => self.windup += 1,
            HunterPhase::Strike => self.strike += 1,
            HunterPhase::Recovering => self.recovering += 1,
            HunterPhase::Handling => self.handling += 1,
        }
    }
}

/// Paid attempts this member resolved, by outcome, from the world's own hunter events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptTally {
    pub captured: u64,
    pub missed: u64,
    pub out_of_reach: u64,
    pub target_lost: u64,
    pub target_claimed: u64,
    pub ineligible: u64,
    pub grasp_unmapped: u64,
    pub unaffordable: u64,
}

impl AttemptTally {
    fn sample(&mut self, outcome: AttemptOutcome) {
        match outcome {
            AttemptOutcome::Captured => self.captured += 1,
            AttemptOutcome::Missed => self.missed += 1,
            AttemptOutcome::OutOfReach => self.out_of_reach += 1,
            AttemptOutcome::TargetLost => self.target_lost += 1,
            AttemptOutcome::TargetClaimed => self.target_claimed += 1,
            AttemptOutcome::Ineligible => self.ineligible += 1,
            AttemptOutcome::GraspUnmapped => self.grasp_unmapped += 1,
            AttemptOutcome::Unaffordable => self.unaffordable += 1,
        }
    }

    pub fn total(&self) -> u64 {
        self.captured
            + self.missed
            + self.out_of_reach
            + self.target_lost
            + self.target_claimed
            + self.ineligible
            + self.grasp_unmapped
            + self.unaffordable
    }
}

/// `hunter::may_reproduce` taken apart into its nine terms, counted per tick of one member's
/// life, in the predicate's own source order.
///
/// [`ApexOpportunity`] says *whether* a member was ready; this says **which term refused it**,
/// which is the difference between "the age gate binds" and "the age gate binds and so would
/// the stock gate". Exactly one bin is incremented per sampled tick, or `ready` is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessTerms {
    /// Ticks this member was alive and its readiness could be evaluated.
    pub sampled: u64,
    pub ready: u64,
    pub fail_escrow: u64,
    pub fail_carrying: u64,
    pub fail_target: u64,
    pub fail_hunting: u64,
    pub fail_structure: u64,
    pub fail_reserve: u64,
    pub fail_energy: u64,
    pub fail_age: u64,
    pub fail_interval: u64,
    /// Ticks on which the decomposition above disagreed with `hunter::may_reproduce` itself.
    /// It must be zero; if it is not, nothing above it may be read.
    pub mismatch: u64,
}

impl ReadinessTerms {
    /// Add another member's tally into this one.
    pub fn add(&mut self, other: &ReadinessTerms) {
        self.sampled += other.sampled;
        self.ready += other.ready;
        self.fail_escrow += other.fail_escrow;
        self.fail_carrying += other.fail_carrying;
        self.fail_target += other.fail_target;
        self.fail_hunting += other.fail_hunting;
        self.fail_structure += other.fail_structure;
        self.fail_reserve += other.fail_reserve;
        self.fail_energy += other.fail_energy;
        self.fail_age += other.fail_age;
        self.fail_interval += other.fail_interval;
        self.mismatch += other.mismatch;
    }

    /// The term that refused this member most often, and how many ticks it refused on.
    pub fn dominant_failure(&self) -> (&'static str, u64) {
        [
            ("gestating", self.fail_escrow),
            ("carrying a carcass", self.fail_carrying),
            ("holding a target", self.fail_target),
            ("hunting", self.fail_hunting),
            ("below adult structure", self.fail_structure),
            ("reserve below the stock fraction", self.fail_reserve),
            ("energy below the stock fraction", self.fail_energy),
            ("below the minimum reproduction age", self.fail_age),
            ("inside the reproduction interval", self.fail_interval),
        ]
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .unwrap_or(("none", 0))
    }

    /// Sample one tick. The term list is `hunter::may_reproduce`'s own conjunction, in its
    /// order, and the result is checked against the function rather than trusted.
    fn sample(
        &mut self,
        profile: &FixedHunterProfile,
        o: &cubarium_core::organism::Organism,
        member: &cubarium_core::hunter::HunterMember,
        now: u64,
        dt: f64,
    ) {
        self.sampled += 1;
        let terms = [
            o.escrow.is_none(),
            !member.carrying(),
            member.target.is_none(),
            !member.phase.hunting(),
            o.structure >= o.phenotype.structure_adult - hunter::TOLERANCE,
            o.reserve >= profile.reproduce_reserve_fraction * o.phenotype.reserve_max,
            o.energy >= profile.reproduce_energy_fraction * o.phenotype.energy_max,
            o.age_ticks(now) as f64 * dt >= profile.reproduce_min_age_seconds,
            now >= member.next_reproduction_tick,
        ];
        let truth = hunter::may_reproduce(profile, o, member, now, dt);
        if terms.iter().all(|t| *t) != truth {
            self.mismatch += 1;
        }
        match terms.iter().position(|t| !t) {
            None => self.ready += 1,
            Some(0) => self.fail_escrow += 1,
            Some(1) => self.fail_carrying += 1,
            Some(2) => self.fail_target += 1,
            Some(3) => self.fail_hunting += 1,
            Some(4) => self.fail_structure += 1,
            Some(5) => self.fail_reserve += 1,
            Some(6) => self.fail_energy += 1,
            Some(7) => self.fail_age += 1,
            _ => self.fail_interval += 1,
        }
    }
}

/// The difference between two [`BodyBudget`] readings of the same body: what it earned and
/// what it owed over the ticks between them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BudgetWindow {
    /// Ticks the window actually covers, which is `TAIL_TICKS` or the whole life if shorter.
    pub ticks: u64,
    pub served_total: f64,
    pub reserve_credit: f64,
    pub gut_reserve_credit: f64,
    pub battery_credit: f64,
    pub gut_battery_credit: f64,
    pub oxidation_reserve_burned: f64,
    pub oxidation_battery_credit: f64,
    pub bill_total: f64,
    pub bill_paid: f64,
    pub upkeep_billed: f64,
    pub motor_billed: f64,
    pub other_energy_paid: f64,
}

impl BudgetWindow {
    fn between(from_tick: u64, from: &BodyBudget, to_tick: u64, to: &BodyBudget) -> BudgetWindow {
        BudgetWindow {
            ticks: to_tick.saturating_sub(from_tick),
            served_total: to.served_total() - from.served_total(),
            reserve_credit: to.reserve_credit_total() - from.reserve_credit_total(),
            gut_reserve_credit: to.gut_reserve_credit - from.gut_reserve_credit,
            battery_credit: to.battery_credit_total() - from.battery_credit_total(),
            gut_battery_credit: to.gut_battery_credit - from.gut_battery_credit,
            oxidation_reserve_burned: to.oxidation_reserve_burned - from.oxidation_reserve_burned,
            oxidation_battery_credit: to.oxidation_battery_credit - from.oxidation_battery_credit,
            bill_total: to.bill_total - from.bill_total,
            bill_paid: to.bill_paid - from.bill_paid,
            upkeep_billed: to.upkeep_billed - from.upkeep_billed,
            motor_billed: (to.motor_translation_billed + to.motor_turn_billed)
                - (from.motor_translation_billed + from.motor_turn_billed),
            other_energy_paid: to.other_energy_paid - from.other_energy_paid,
        }
    }
}

/// One introduced adult's life in the run that introduced it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApexLife {
    pub id: OrganismId,
    pub introduced_tick: u64,
    /// The age it was placed at, in ticks: `0` for the door as it has always behaved.
    pub introduced_age_ticks: u64,
    /// The first tick the id no longer resolved. `None` if it outlived the horizon.
    pub gone_tick: Option<u64>,
    /// Ticks it was alive inside this run.
    pub lived_ticks: u64,
    /// `introduced_age_ticks + lived_ticks`: the age `may_reproduce` compared, at the end.
    pub age_at_end_ticks: u64,
    /// Why the world removed it, from the ledger's closed record. `None` if it survived, or if
    /// the ledger was off.
    pub death_cause: Option<DeathCause>,
    /// Its whole store budget: the closed record if it died, the live one at the horizon
    /// otherwise. `None` when the ledger was off.
    pub budget: Option<BodyBudget>,
    /// The last [`TAIL_TICKS`] of that budget, so a body that starved after earning nothing is
    /// distinguishable from one that was earning and still lost.
    pub tail: Option<BudgetWindow>,
    pub phases: PhaseOccupancy,
    pub readiness: ReadinessTerms,
    pub attempts: AttemptTally,
    /// Captures, and the material and energy they actually put into the gut.
    pub captures: u64,
    pub capture_material: f64,
    pub capture_energy: f64,
    /// The highest fraction of `R_max` and `E_max` this body ever held, against the 0.8 and
    /// 0.75 `may_reproduce` demands, and what it held at the end.
    pub max_reserve_fraction: f64,
    pub max_energy_fraction: f64,
    pub end_reserve_fraction: f64,
    pub end_energy_fraction: f64,
}

// ---------------------------------------------------------------- the reach diagnostic (N)

/// How many raw per-attempt records one row keeps. The aggregates below are computed over
/// **every** record; this cap only bounds the artifact, and what it left out is counted.
pub const MAX_KEPT_RECORDS: usize = 4_000;

/// One quantity over a set of attempts: enough to report a mean without hiding the spread, and
/// the extremes a verdict's deciding rows come from. `n` counts only the attempts that
/// actually carried the quantity, so a mean is never diluted by a missing frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Series {
    pub n: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
}

impl Series {
    fn push(&mut self, v: f64) {
        if !v.is_finite() {
            return;
        }
        if self.n == 0 {
            self.min = v;
            self.max = v;
        } else {
            self.min = self.min.min(v);
            self.max = self.max.max(v);
        }
        self.n += 1;
        self.sum += v;
    }

    fn push_opt(&mut self, v: Option<f64>) {
        if let Some(v) = v {
            self.push(v);
        }
    }

    pub fn add(&mut self, other: &Series) {
        if other.n == 0 {
            return;
        }
        if self.n == 0 {
            *self = *other;
            return;
        }
        self.n += other.n;
        self.sum += other.sum;
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    pub fn mean(&self) -> Option<f64> {
        (self.n > 0).then(|| self.sum / self.n as f64)
    }
}

/// Everything one bucket of attempts — one class, or one outcome — says about reach.
///
/// Separations are `effector_distance`: the surface distance from the scaled grasp centre to
/// the prey. `overshoot` is that minus the contact tolerance, so zero is the edge of the grasp
/// and positive is out of it. Speeds are realised displacement over the phase's own duration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StrikeStats {
    pub count: u64,
    /// The strike cost these attempts actually paid, summed.
    pub energy_paid: f64,
    /// Attempts whose intent frame said the prey was already inside the grasp.
    pub began_in_reach: u64,
    /// Attempts whose resolution frame said so.
    pub resolved_in_reach: u64,
    /// Attempts in which the prey, or the hunter, changed cube face during the attempt.
    pub target_crossed_face: u64,
    pub hunter_crossed_face: u64,
    pub advertised_reach: Series,
    pub tolerance: Series,
    pub intent_separation: Series,
    pub intent_overshoot: Series,
    pub strike_separation: Series,
    pub resolution_separation: Series,
    pub resolution_overshoot: Series,
    /// `resolution − strike`: how far the gap moved across the paid burst. Positive is the
    /// prey pulling away.
    pub separation_change_over_strike: Series,
    /// `resolution − intent`: across the whole attempt, windup included.
    pub separation_change_total: Series,
    pub prey_speed_windup: Series,
    pub prey_speed_strike: Series,
    pub hunter_speed_windup: Series,
    pub hunter_speed_strike: Series,
    pub prey_turn_strike_deg: Series,
}

impl StrikeStats {
    fn sample(&mut self, r: &StrikeRecord) {
        self.count += 1;
        self.energy_paid += r.energy_paid;
        if r.intent.is_some_and(|f| f.in_reach) {
            self.began_in_reach += 1;
        }
        if r.resolution.in_reach {
            self.resolved_in_reach += 1;
        }
        if r.target_crossed_face {
            self.target_crossed_face += 1;
        }
        if r.hunter_crossed_face {
            self.hunter_crossed_face += 1;
        }
        self.advertised_reach.push(r.resolution.advertised_reach);
        self.tolerance.push_opt(r.resolution.tolerance);
        self.intent_separation.push_opt(r.intent.and_then(|f| f.effector_distance));
        self.intent_overshoot.push_opt(r.intent.and_then(|f| f.overshoot()));
        self.strike_separation.push_opt(r.strike.and_then(|f| f.effector_distance));
        self.resolution_separation.push_opt(r.resolution.effector_distance);
        self.resolution_overshoot.push_opt(r.resolution.overshoot());
        self.separation_change_over_strike.push_opt(r.separation_change_over_strike());
        self.separation_change_total.push_opt(r.separation_change_total());
        self.prey_speed_windup.push_opt(r.target_speed_windup);
        self.prey_speed_strike.push_opt(r.target_speed_strike);
        self.hunter_speed_windup.push_opt(r.hunter_speed_windup);
        self.hunter_speed_strike.push_opt(r.hunter_speed_strike);
        self.prey_turn_strike_deg.push_opt(r.target_turn_strike.map(f64::to_degrees));
    }

    pub fn add(&mut self, other: &StrikeStats) {
        self.count += other.count;
        self.energy_paid += other.energy_paid;
        self.began_in_reach += other.began_in_reach;
        self.resolved_in_reach += other.resolved_in_reach;
        self.target_crossed_face += other.target_crossed_face;
        self.hunter_crossed_face += other.hunter_crossed_face;
        self.advertised_reach.add(&other.advertised_reach);
        self.tolerance.add(&other.tolerance);
        self.intent_separation.add(&other.intent_separation);
        self.intent_overshoot.add(&other.intent_overshoot);
        self.strike_separation.add(&other.strike_separation);
        self.resolution_separation.add(&other.resolution_separation);
        self.resolution_overshoot.add(&other.resolution_overshoot);
        self.separation_change_over_strike.add(&other.separation_change_over_strike);
        self.separation_change_total.add(&other.separation_change_total);
        self.prey_speed_windup.add(&other.prey_speed_windup);
        self.prey_speed_strike.add(&other.prey_speed_strike);
        self.hunter_speed_windup.add(&other.hunter_speed_windup);
        self.hunter_speed_strike.add(&other.hunter_speed_strike);
        self.prey_turn_strike_deg.add(&other.prey_turn_strike_deg);
    }
}

/// Every paid attempt of one run, split the two ways the diagnostic asks for: by the mechanism
/// its own geometry implicates, and by the outcome the world gave it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StrikeAudit {
    /// Paid attempts recorded. Equals the sum of `AttemptTally` over the followed members
    /// minus its `unaffordable`, which is refused before payment and is not an attempt.
    pub recorded: u64,
    /// Records the recorder dropped undrained, and records that resolved with no intent frame.
    /// Both must be zero for the table below to be complete.
    pub dropped: u64,
    pub unreadable: u64,
    /// Raw records omitted from `records` by [`MAX_KEPT_RECORDS`]; the aggregates saw them.
    pub omitted: u64,
    pub by_class: Vec<(StrikeClass, StrikeStats)>,
    pub by_outcome: Vec<(AttemptOutcome, StrikeStats)>,
    pub records: Vec<StrikeRecord>,
}

impl StrikeAudit {
    /// File one closed record: one class bucket, one outcome bucket, and the raw record if
    /// the cap allows.
    pub fn sample(&mut self, r: StrikeRecord) {
        self.recorded += 1;
        match self.by_class.iter_mut().find(|(c, _)| *c == r.class) {
            Some((_, s)) => s.sample(&r),
            None => {
                let mut s = StrikeStats::default();
                s.sample(&r);
                self.by_class.push((r.class, s));
            }
        }
        match self.by_outcome.iter_mut().find(|(o, _)| *o == r.outcome) {
            Some((_, s)) => s.sample(&r),
            None => {
                let mut s = StrikeStats::default();
                s.sample(&r);
                self.by_outcome.push((r.outcome, s));
            }
        }
        if self.records.len() < MAX_KEPT_RECORDS {
            self.records.push(r);
        } else {
            self.omitted += 1;
        }
    }

    pub fn add(&mut self, other: &StrikeAudit) {
        self.recorded += other.recorded;
        self.dropped += other.dropped;
        self.unreadable += other.unreadable;
        self.omitted += other.omitted;
        for (c, s) in &other.by_class {
            match self.by_class.iter_mut().find(|(k, _)| k == c) {
                Some((_, into)) => into.add(s),
                None => self.by_class.push((*c, *s)),
            }
        }
        for (o, s) in &other.by_outcome {
            match self.by_outcome.iter_mut().find(|(k, _)| k == o) {
                Some((_, into)) => into.add(s),
                None => self.by_outcome.push((*o, *s)),
            }
        }
    }

    /// The classes in the fixed reporting order, so two runs' tables line up.
    pub fn classes_in_order(&self) -> Vec<(StrikeClass, StrikeStats)> {
        const ORDER: [StrikeClass; 6] = [
            StrikeClass::ResolvedInReach,
            StrikeClass::BeganInReachResolvedOut,
            StrikeClass::PreyOutran,
            StrikeClass::BeganOutOfReach,
            StrikeClass::TargetLost,
            StrikeClass::Unreadable,
        ];
        ORDER
            .iter()
            .filter_map(|c| self.by_class.iter().find(|(k, _)| k == c).copied())
            .collect()
    }

    pub fn stats_for_outcome(&self, outcome: AttemptOutcome) -> Option<StrikeStats> {
        self.by_outcome.iter().find(|(o, _)| *o == outcome).map(|(_, s)| *s)
    }
}

/// One `(configuration, seed)` audit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditRow {
    pub config: String,
    pub config_hash: String,
    /// Whether the loaded configuration is bit-for-bit the declared screen candidate of the
    /// same name at this seed. `None` when no candidate carries that name.
    pub matches_screen_candidate: Option<bool>,
    pub seed: u64,
    pub apex_founders: u32,
    pub introduce_tick: u64,
    /// The age the founders were introduced at (s). `0.0` is the door as the screen ran it.
    pub founder_age_seconds: f64,
    /// Whether the per-body store ledger was recording. When it is off, every `budget` and
    /// `tail` below is `None` and no `death_cause` is known.
    pub ledger: bool,
    pub horizon_ticks: u64,
    /// Ticks actually simulated: the horizon, or fewer if the world emptied.
    pub ticks: u64,
    pub collapsed_at: Option<u64>,
    pub apex_material_in: f64,
    pub apex_energy_in: f64,
    pub lives: Vec<ApexLife>,
    pub opportunity: ApexOpportunity,
    /// Every paid attempt of this run, classified (workstream N). Empty when the strike
    /// recorder was off.
    pub strikes: StrikeAudit,
    /// The world's own conservation residuals at the end, so a run that drifted is visible.
    pub mass_residual: f64,
    pub final_population: usize,
    pub elapsed_ms: u64,
    /// The profile terms `hunter::may_reproduce` tests, carried on the row so a reader can see
    /// what "never ready" was measured against without opening the source.
    pub reproduce_min_age_ticks: u64,
    pub reproduce_reserve_fraction: f64,
    pub reproduce_energy_fraction: f64,
    pub mating_radius_px: f64,
}

impl AuditRow {
    /// Did any two ready adults ever stand within the mating radius?
    pub fn ready_pair_ever_met(&self) -> bool {
        self.opportunity.ticks_ready_pair_within_radius > 0
    }

    /// Was there ever a moment at which two adults could both have reproduced?
    pub fn readiness_overlap(&self) -> bool {
        self.opportunity.ticks_two_ready > 0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditReport {
    pub build: String,
    pub configs: Vec<String>,
    pub seeds: Vec<u64>,
    pub apex_founders: u32,
    pub introduce_tick: u64,
    pub founder_age_seconds: f64,
    pub ledger: bool,
    pub horizon_ticks: u64,
    pub workers: usize,
    pub wall_seconds: f64,
    /// The one sentence the audit exists to produce.
    pub verdict: String,
    /// Every paid attempt of every row, classified together (workstream N).
    pub strikes: StrikeAudit,
    pub rows: Vec<AuditRow>,
}

/// What one arm runs: the screen's own settings, plus the two this workstream added.
#[derive(Clone, Copy, Debug)]
pub struct Arm {
    pub apex: u32,
    pub horizon: u64,
    pub introduce_tick: u64,
    /// The age the founders are placed at (s). `0.0` is the door the screen used.
    pub founder_age_seconds: f64,
    /// Whether to record the per-body store ledger for the run.
    pub ledger: bool,
}

/// Run one `(configuration, seed)` arm of the audit.
fn run_one(eco: &Ecology, seed: u64, arm: Arm) -> Result<AuditRow, String> {
    let Arm { apex, horizon, introduce_tick, founder_age_seconds, ledger } = arm;
    let start = Instant::now();
    let mut config: WorldConfig = (*eco.base).clone();
    config.seed = seed;
    // The host-side event log is off in every headless run; leaving it on would grow a queue
    // this command never reads over 180,000 ticks.
    config.capacity.event_log = false;
    let hash = calibrate::config_hash(&config);
    // Provenance: is this the declared screen candidate of the same name, at this seed?
    let matches = calibrate::candidate(&eco.label)
        .map(|c| c.config(seed).map(|c| calibrate::config_hash(&c) == hash).unwrap_or(false));

    // The profile is derived from the **base** configuration, exactly as the screen derives it,
    // so the apex genome does not shift under a candidate's parameters.
    let profile = FixedHunterProfile::lanternjaw_trial(&evaluate::base_config(seed));
    profile.validate().map_err(|e| format!("hunter profile rejected: {e}"))?;
    config.validate().map_err(|e| format!("config rejected: {e}"))?;
    let mut world = World::new(config).map_err(|e| format!("world creation refused: {e}"))?;
    world.record_body_budgets(ledger);
    // The per-attempt strike record follows the ledger flag rather than a new one: this whole
    // command is a diagnostic, the record is inert by the same hash test the ledger is, and a
    // second CLI switch would only be a second way to run the audit half-instrumented.
    world.record_strike_attempts(ledger);
    let dt = cubarium_core::DT;
    let age_ticks = (founder_age_seconds / dt).round().max(0.0) as u64;

    let mut lives: Vec<ApexLife> = Vec::new();
    // One rolling window of `(tick, budget)` readings per life, capped at `TAIL_TICKS`, so the
    // end of a life can be differenced against its own state 2,000 ticks earlier.
    let mut tails: Vec<VecDeque<(u64, BodyBudget)>> = Vec::new();
    let (mut material_in, mut energy_in) = (0.0, 0.0);
    let mut strikes = StrikeAudit::default();
    let mut introduced = false;
    let mut collapsed_at = None;
    let mut ticks = 0;

    for _ in 0..horizon {
        world.step();
        ticks = world.tick();
        // The queues are drained every tick rather than accumulated: a 180,000-tick run would
        // otherwise hold every event it ever emitted. The hunt queue is read on the way past,
        // for the attempts and captures of the members this audit follows.
        world.drain_events();
        for event in world.drain_hunter_events() {
            match event {
                HunterEvent::Attempt { hunter, outcome, .. } => {
                    if let Some(life) = lives.iter_mut().find(|l| l.id == hunter) {
                        life.attempts.sample(outcome);
                    }
                }
                HunterEvent::Capture { hunter, material, energy, .. } => {
                    if let Some(life) = lives.iter_mut().find(|l| l.id == hunter) {
                        life.captures += 1;
                        life.capture_material += material;
                        life.capture_energy += energy;
                    }
                }
                _ => {}
            }
        }
        world.drain_apex_dormancy_events();
        world.drain_apex_encounter_events();
        world.drain_quiet_events();
        // The ledger is drained every tick too, for the same reason, and the records of the
        // members this audit follows are kept. Everything else the world buried is dropped.
        // The strike records are drained on the same schedule and for the same reason: a
        // 180,000-tick run would otherwise hold every attempt it ever made.
        let (records, strike_dropped, strike_unreadable) = world.drain_strike_records();
        strikes.dropped += strike_dropped;
        strikes.unreadable += strike_unreadable;
        for record in records {
            strikes.sample(record);
        }
        let (closed, dropped) = world.drain_body_budgets();
        if dropped > 0 {
            return Err(format!("the ledger dropped {dropped} closed record(s)"));
        }
        for record in closed {
            if let Some(life) = lives.iter_mut().find(|l| l.id == record.id) {
                life.death_cause = record.death_cause;
                life.budget = Some(record);
            }
        }

        if !introduced && apex > 0 && world.tick() == introduce_tick {
            let targets = evaluate::apex_targets(seed, apex);
            let receipts = world
                .introduce_hunters_with_age(profile.clone(), &targets, founder_age_seconds)
                .map_err(|e| format!("apex introduction refused: {e}"))?;
            material_in = receipts.iter().map(|r| r.material_in).sum();
            energy_in = receipts.iter().map(|r| r.energy_in).sum();
            lives = receipts
                .iter()
                .map(|r| ApexLife {
                    id: r.id,
                    introduced_tick: world.tick(),
                    introduced_age_ticks: age_ticks,
                    gone_tick: None,
                    lived_ticks: 0,
                    age_at_end_ticks: age_ticks,
                    death_cause: None,
                    budget: None,
                    tail: None,
                    phases: PhaseOccupancy::default(),
                    readiness: ReadinessTerms::default(),
                    attempts: AttemptTally::default(),
                    captures: 0,
                    capture_material: 0.0,
                    capture_energy: 0.0,
                    max_reserve_fraction: 0.0,
                    max_energy_fraction: 0.0,
                    end_reserve_fraction: 0.0,
                    end_energy_fraction: 0.0,
                })
                .collect();
            tails = lives.iter().map(|_| VecDeque::new()).collect();
            introduced = true;
        }
        let now = world.tick();
        for (life, tail) in lives.iter_mut().zip(tails.iter_mut()) {
            if life.gone_tick.is_some() {
                continue;
            }
            let Some(o) = world.state.organisms.get(life.id) else {
                life.gone_tick = Some(now);
                // The record the ledger closed this tick is the whole life; the window is its
                // last `TAIL_TICKS`, differenced against this member's own earlier reading.
                if let (Some(end), Some((from_tick, from))) = (life.budget.as_ref(), tail.front()) {
                    life.tail = Some(BudgetWindow::between(*from_tick, from, now, end));
                }
                continue;
            };
            life.lived_ticks += 1;
            life.age_at_end_ticks = life.introduced_age_ticks + life.lived_ticks;
            let reserve_fraction = o.reserve / o.phenotype.reserve_max;
            let energy_fraction = o.energy / o.phenotype.energy_max;
            life.max_reserve_fraction = life.max_reserve_fraction.max(reserve_fraction);
            life.max_energy_fraction = life.max_energy_fraction.max(energy_fraction);
            life.end_reserve_fraction = reserve_fraction;
            life.end_energy_fraction = energy_fraction;
            if let Some(member) = world.hunters().member(life.id) {
                life.phases.sample(member.phase, world.state.apex_dormancy.contains(life.id));
                life.readiness.sample(&profile, o, member, now, dt);
            }
            if let Some(budget) = world.body_budget(life.id) {
                tail.push_back((now, *budget));
                while tail.len() as u64 > TAIL_TICKS {
                    tail.pop_front();
                }
            }
        }

        if world.tick().is_multiple_of(VALIDATE_EVERY)
            && let Err(e) = world.check_invariants()
        {
            return Err(format!("invariant violated at tick {}: {e}", world.tick()));
        }
        if world.population() == 0 {
            collapsed_at = Some(world.tick());
            break;
        }
    }

    // A member that outlived the horizon has no closed record: its budget is the live one, and
    // its window ends where the run did.
    let end_tick = world.tick();
    for (life, tail) in lives.iter_mut().zip(tails.iter()) {
        if life.gone_tick.is_some() {
            continue;
        }
        if let Some(budget) = world.body_budget(life.id) {
            if let Some((from_tick, from)) = tail.front() {
                life.tail = Some(BudgetWindow::between(*from_tick, from, end_tick, budget));
            }
            life.budget = Some(*budget);
        }
    }

    Ok(AuditRow {
        reproduce_min_age_ticks: (profile.reproduce_min_age_seconds / cubarium_core::DT) as u64,
        reproduce_reserve_fraction: profile.reproduce_reserve_fraction,
        reproduce_energy_fraction: profile.reproduce_energy_fraction,
        mating_radius_px: cubarium_core::encounter::MATING_RADIUS_PX,
        config: eco.label.clone(),
        config_hash: format!("{hash:016x}"),
        matches_screen_candidate: matches,
        seed,
        apex_founders: apex,
        introduce_tick,
        founder_age_seconds,
        ledger,
        horizon_ticks: horizon,
        ticks,
        collapsed_at,
        apex_material_in: material_in,
        apex_energy_in: energy_in,
        lives,
        opportunity: world.drain_apex_opportunity(),
        strikes,
        mass_residual: world.mass_residual(),
        final_population: world.population(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

/// The one sentence the audit produces, from the rows it produced.
fn verdict(rows: &[AuditRow]) -> String {
    let with_two_adults = rows.iter().filter(|r| r.opportunity.ticks_two_adults > 0).count();
    let with_overlap = rows.iter().filter(|r| r.readiness_overlap()).count();
    let with_meeting = rows.iter().filter(|r| r.ready_pair_ever_met()).count();
    let candidates: u64 = rows.iter().map(|r| r.opportunity.pair_candidates).sum();
    let radius_failures: u64 = rows.iter().map(|r| r.opportunity.fail_radius).sum();
    if with_overlap == 0 {
        // When no member is ever ready, say what the readiness gate it never passed actually
        // is. The oldest apex any run produced against the age `may_reproduce` demands is a
        // comparison the rows already carry, and it is the difference between "the radius is
        // the wrong knob" and "the radius is the wrong knob and here is the right one".
        let oldest = rows
            .iter()
            .flat_map(|r| r.lives.iter().map(|l| l.age_at_end_ticks))
            .max()
            .unwrap_or(0);
        let min_age = rows.iter().map(|r| r.reproduce_min_age_ticks).max().unwrap_or(0);
        let age = if min_age > 0 && oldest < min_age {
            format!(
                " No member ever became eligible at all: the oldest apex in any run reached \
                 {oldest} ticks and `may_reproduce` requires {min_age}, so every member died at \
                 {:.0}% of its own minimum reproduction age.",
                100.0 * oldest as f64 / min_age as f64
            )
        } else if min_age > 0 {
            // The age gate was reachable in this arm, so it is not what refused them. Name the
            // term that did, from the members' own per-tick decomposition.
            let mut terms = ReadinessTerms::default();
            for r in rows {
                for l in &r.lives {
                    terms.add(&l.readiness);
                }
            }
            let (name, ticks) = terms.dominant_failure();
            let mismatch = if terms.mismatch > 0 {
                format!(
                    " WARNING: the term decomposition disagreed with `may_reproduce` on \
                     {} tick(s) and must not be read.",
                    terms.mismatch
                )
            } else {
                String::new()
            };
            format!(
                " The age gate is not what refused them here: members reached {oldest} ticks \
                 against the {min_age} `may_reproduce` requires. Of {} member-ticks watched, \
                 the first term to refuse was `{name}` on {ticks} ({:.1}%).{mismatch}",
                terms.sampled,
                100.0 * ticks as f64 / terms.sampled.max(1) as f64,
            )
        } else {
            String::new()
        };
        format!(
            "Readiness overlap is zero in all {} runs: two adults were alive together in {} of \
             them, but never simultaneously able to reproduce, so the {} candidate pair(s) the \
             pass formed never reached the distance test as the deciding term. Changing the 10 \
             px mating radius cannot produce a mating in this arm.{age}",
            rows.len(),
            with_two_adults,
            candidates
        )
    } else if with_meeting == 0 {
        format!(
            "Readiness overlap exists in {with_overlap} of {} runs and no ready pair ever closed \
             to the 10 px mating radius ({radius_failures} candidate pair(s) were refused on \
             distance). The radius, or an encounter policy that brings ready adults together, \
             is a real owner-facing choice here.",
            rows.len()
        )
    } else {
        format!(
            "A ready pair stood inside the mating radius in {with_meeting} of {} runs, so \
             neither readiness nor distance is the whole explanation; read the per-run failure \
             histogram for what refused them.",
            rows.len()
        )
    }
}

/// Run the audit over every configuration and seed and write its record.
#[allow(clippy::too_many_arguments)]
pub fn run(
    configs: Vec<PathBuf>,
    seeds: usize,
    arm: Arm,
    workers: usize,
    wall_seconds: u64,
    out: PathBuf,
) -> Result<(), Boxed> {
    let Arm { apex, horizon, introduce_tick, founder_age_seconds, ledger } = arm;
    if configs.is_empty() {
        return Err("--config must name at least one world configuration TOML".into());
    }
    if !founder_age_seconds.is_finite() || founder_age_seconds < 0.0 {
        return Err("--founder-age-seconds must be finite and not negative".into());
    }
    if seeds == 0 || seeds > HELDOUT_SEEDS.len() {
        return Err(format!("--seeds must be between 1 and {}", HELDOUT_SEEDS.len()).into());
    }
    let ecologies: Vec<Ecology> = configs
        .iter()
        .map(|p| Ecology::load(p))
        .collect::<Result<_, _>>()?;
    let seeds: Vec<u64> = HELDOUT_SEEDS[..seeds].to_vec();

    println!("# apex opportunity audit");
    println!("# build {BUILD_ID}, {apex} adults introduced at tick {introduce_tick}, never restocked");
    println!(
        "# founders placed at age {founder_age_seconds} s, per-body ledger {}",
        if ledger { "on" } else { "off" }
    );
    println!("# horizon {horizon} ticks, held-out seeds {seeds:?}, {workers} workers");
    for e in &ecologies {
        println!("# config {} (hash {})", e.label, e.hex());
    }

    let jobs: Vec<(usize, u64)> = ecologies
        .iter()
        .enumerate()
        .flat_map(|(i, _)| seeds.iter().map(move |s| (i, *s)))
        .collect();
    let cursor = AtomicUsize::new(0);
    let rows: Mutex<Vec<AuditRow>> = Mutex::new(Vec::new());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let started = Instant::now();

    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    let Some(&(e, seed)) = jobs.get(i) else { return };
                    if started.elapsed().as_secs() >= wall_seconds {
                        failures
                            .lock()
                            .expect("failures")
                            .push(format!("{}/{seed}: not started inside the wall cap", ecologies[e].label));
                        continue;
                    }
                    match run_one(&ecologies[e], seed, arm) {
                        Ok(row) => rows.lock().expect("rows").push(row),
                        Err(err) => failures
                            .lock()
                            .expect("failures")
                            .push(format!("{}/{seed}: {err}", ecologies[e].label)),
                    }
                }
            });
        }
    });

    let failures = failures.into_inner().expect("failures");
    for f in &failures {
        println!("FAILED {f}");
    }
    let mut rows = rows.into_inner().expect("rows");
    rows.sort_by(|a, b| a.config.cmp(&b.config).then(a.seed.cmp(&b.seed)));
    if rows.is_empty() {
        return Err("no audit row completed".into());
    }

    println!();
    println!(
        "{:<12} {:>7} {:>9} {:>9} {:>9} {:>9} {:>10} {:>7} {:>7} {:>7}",
        "config", "seed", "life a", "life b", "2 adults", "2 ready", "min px", "cands", "radius", "ready",
    );
    for r in &rows {
        let life = |i: usize| r.lives.get(i).map_or(0, |l| l.lived_ticks);
        println!(
            "{:<12} {:>7} {:>9} {:>9} {:>9} {:>9} {:>10} {:>7} {:>7} {:>7}",
            r.config,
            r.seed,
            life(0),
            life(1),
            r.opportunity.ticks_two_adults,
            r.opportunity.ticks_two_ready,
            r.opportunity
                .min_ready_distance_px
                .map_or_else(|| ">32".to_string(), |d| format!("{d:.2}")),
            r.opportunity.pair_candidates,
            r.opportunity.fail_radius,
            r.opportunity.fail_ready_a + r.opportunity.fail_ready_b,
        );
    }

    if ledger {
        println!();
        println!(
            "{:<12} {:>7} {:>3} {:>9} {:>9} {:>11} {:>8} {:>8} {:>8} {:>8} {:>8} {:>10}",
            "config",
            "seed",
            "#",
            "lived",
            "age end",
            "cause",
            "captures",
            "credit e",
            "paid e",
            "max R/R*",
            "max E/E*",
            "refused by",
        );
        for r in &rows {
            for (i, l) in r.lives.iter().enumerate() {
                let (credit, paid) = l.budget.as_ref().map_or((0.0, 0.0), |b| {
                    (
                        b.battery_credit_total() + b.gut_battery_credit + b.oxidation_battery_credit,
                        b.bill_paid + b.other_energy_paid,
                    )
                });
                println!(
                    "{:<12} {:>7} {:>3} {:>9} {:>9} {:>11} {:>8} {:>8.3} {:>8.3} {:>8.3} {:>8.3} {:>10}",
                    r.config,
                    r.seed,
                    i,
                    l.lived_ticks,
                    l.age_at_end_ticks,
                    l.death_cause.map_or_else(|| "alive".to_string(), |c| format!("{c:?}")),
                    l.captures,
                    credit,
                    paid,
                    l.max_reserve_fraction,
                    l.max_energy_fraction,
                    l.readiness.dominant_failure().0,
                );
            }
        }
    }

    // The reach diagnostic: every paid attempt of every row, classified by its own geometry.
    let mut strikes = StrikeAudit::default();
    for r in &rows {
        strikes.add(&r.strikes);
    }
    if strikes.recorded > 0 {
        println!();
        println!(
            "# paid attempts {}, dropped {}, unreadable {} (both must be 0)",
            strikes.recorded, strikes.dropped, strikes.unreadable
        );
        println!(
            "{:<28} {:>7} {:>8} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
            "class", "n", "e paid", "sep@int", "sep@str", "sep@res", "over@res", "prey px/s", "hunt px/s",
        );
        let cell = |s: Option<f64>| s.map_or_else(|| "—".to_string(), |v| format!("{v:.2}"));
        for (class, st) in strikes.classes_in_order() {
            println!(
                "{:<28} {:>7} {:>8.3} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
                class.as_str(),
                st.count,
                st.energy_paid,
                cell(st.intent_separation.mean()),
                cell(st.strike_separation.mean()),
                cell(st.resolution_separation.mean()),
                cell(st.resolution_overshoot.mean()),
                cell(st.prey_speed_strike.mean()),
                cell(st.hunter_speed_strike.mean()),
            );
        }
        println!();
        println!(
            "{:<28} {:>7} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
            "outcome", "n", "sep@int", "sep@str", "sep@res", "tol", "reach", "prey px/s", "hunt px/s",
        );
        for (outcome, st) in &strikes.by_outcome {
            println!(
                "{:<28} {:>7} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
                outcome.as_str(),
                st.count,
                cell(st.intent_separation.mean()),
                cell(st.strike_separation.mean()),
                cell(st.resolution_separation.mean()),
                cell(st.tolerance.mean()),
                cell(st.advertised_reach.mean()),
                cell(st.prey_speed_strike.mean()),
                cell(st.hunter_speed_strike.mean()),
            );
        }
    }

    let verdict = verdict(&rows);
    println!();
    println!("{verdict}");
    let report = AuditReport {
        build: BUILD_ID.to_string(),
        configs: ecologies.iter().map(|e| e.label.clone()).collect(),
        seeds,
        apex_founders: apex,
        introduce_tick,
        founder_age_seconds,
        ledger,
        horizon_ticks: horizon,
        workers,
        wall_seconds: started.elapsed().as_secs_f64(),
        verdict,
        strikes,
        rows,
    };
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, serde_json::to_string(&report)?)?;
    println!("wall {:.1} s; wrote {}", report.wall_seconds, out.display());
    if !failures.is_empty() {
        return Err(format!("{} arm(s) did not complete", failures.len()).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(two_adults: u64, two_ready: u64, within: u64, candidates: u64, radius: u64) -> AuditRow {
        AuditRow {
            config: "fast-leaf".into(),
            config_hash: "0".into(),
            matches_screen_candidate: Some(true),
            seed: 1,
            apex_founders: 2,
            introduce_tick: 6_000,
            founder_age_seconds: 0.0,
            ledger: true,
            horizon_ticks: 180_000,
            ticks: 180_000,
            collapsed_at: None,
            apex_material_in: 0.0,
            apex_energy_in: 0.0,
            lives: Vec::new(),
            opportunity: ApexOpportunity {
                ticks_two_adults: two_adults,
                ticks_two_ready: two_ready,
                ticks_ready_pair_within_radius: within,
                pair_candidates: candidates,
                fail_radius: radius,
                ..ApexOpportunity::default()
            },
            strikes: StrikeAudit::default(),
            mass_residual: 0.0,
            final_population: 0,
            elapsed_ms: 0,
            reproduce_min_age_ticks: 24_000,
            reproduce_reserve_fraction: 0.8,
            reproduce_energy_fraction: 0.75,
            mating_radius_px: 10.0,
        }
    }

    /// The three verdicts are decided by the counters, not by the author: no readiness overlap
    /// means the radius is irrelevant, overlap without a meeting makes it a real choice, and a
    /// meeting means neither term alone explains the outcome.
    #[test]
    fn the_verdict_follows_the_counters() {
        let none = verdict(&[row(500, 0, 0, 3, 3)]);
        assert!(none.contains("cannot produce a mating"), "{none}");

        let overlap = verdict(&[row(500, 400, 0, 3, 3)]);
        assert!(overlap.contains("real owner-facing choice"), "{overlap}");

        let met = verdict(&[row(500, 400, 12, 3, 0)]);
        assert!(met.contains("neither readiness nor distance"), "{met}");
    }

    /// An all-zero ledger record. `BodyBudget` is a plain record of sums with no constructor
    /// outside the world that fills it, so a unit test writes one out.
    fn zero_budget() -> BodyBudget {
        BodyBudget {
            id: OrganismId { slot: 0, generation: 1 },
            opened_tick: 0,
            born_tick: 0,
            closed_tick: None,
            death_cause: None,
            start_structure: 0.0,
            start_reserve: 0.0,
            start_energy: 0.0,
            end_structure: 0.0,
            end_reserve: 0.0,
            end_energy: 0.0,
            served: [0.0; 4],
            digestible: [0.0; 4],
            reserve_credit: [0.0; 4],
            battery_credit: [0.0; 4],
            gut_reserve_credit: 0.0,
            gut_battery_credit: 0.0,
            oxidation_reserve_burned: 0.0,
            oxidation_battery_credit: 0.0,
            upkeep_billed: 0.0,
            motor_translation_billed: 0.0,
            motor_turn_billed: 0.0,
            bill_total: 0.0,
            bill_paid: 0.0,
            other_energy_paid: 0.0,
            growth_material: 0.0,
            growth_energy: 0.0,
            reproduction_material: 0.0,
            reproduction_energy: 0.0,
            injury_structure: 0.0,
            billed_ticks: 0,
        }
    }

    fn life(age_at_end: u64, readiness: ReadinessTerms) -> ApexLife {
        ApexLife {
            id: OrganismId { slot: 0, generation: 1 },
            introduced_tick: 6_000,
            introduced_age_ticks: age_at_end,
            gone_tick: None,
            lived_ticks: 0,
            age_at_end_ticks: age_at_end,
            death_cause: Some(DeathCause::Starvation),
            budget: None,
            tail: None,
            phases: PhaseOccupancy::default(),
            readiness,
            attempts: AttemptTally::default(),
            captures: 0,
            capture_material: 0.0,
            capture_energy: 0.0,
            max_reserve_fraction: 0.0,
            max_energy_fraction: 0.0,
            end_reserve_fraction: 0.0,
            end_energy_fraction: 0.0,
        }
    }

    /// When the founders were placed past the age gate and readiness still never opened, the
    /// verdict must stop blaming the age and name the term the members' own decomposition says
    /// refused them.
    #[test]
    fn the_verdict_names_the_term_that_refused_once_the_age_gate_is_open() {
        let terms = ReadinessTerms {
            sampled: 1_000,
            fail_reserve: 900,
            fail_energy: 100,
            ..ReadinessTerms::default()
        };
        let mut r = row(500, 0, 0, 0, 0);
        r.lives = vec![life(30_000, terms)];
        let said = verdict(&[r]);
        assert!(said.contains("cannot produce a mating"), "{said}");
        assert!(said.contains("age gate is not what refused them"), "{said}");
        assert!(said.contains("reserve below the stock fraction"), "{said}");
        assert!(said.contains("90.0%"), "{said}");
        assert!(!said.contains("WARNING"), "{said}");
    }

    /// A decomposition that disagreed with `may_reproduce` says so instead of being read.
    #[test]
    fn a_disagreeing_decomposition_is_flagged_not_reported_quietly() {
        let terms =
            ReadinessTerms { sampled: 10, fail_reserve: 10, mismatch: 3, ..ReadinessTerms::default() };
        let mut r = row(500, 0, 0, 0, 0);
        r.lives = vec![life(30_000, terms)];
        let said = verdict(&[r]);
        assert!(said.contains("WARNING"), "{said}");
    }

    /// The tail window is the difference of two readings of the same body, not a recomputation.
    #[test]
    fn the_tail_window_differences_two_readings() {
        let mut from = zero_budget();
        from.bill_paid = 1.0;
        from.oxidation_reserve_burned = 0.25;
        let mut to = from;
        to.bill_paid = 3.5;
        to.oxidation_reserve_burned = 0.75;
        to.gut_reserve_credit = 2.0;
        let w = BudgetWindow::between(9_000, &from, 11_000, &to);
        assert_eq!(w.ticks, 2_000);
        assert!((w.bill_paid - 2.5).abs() < 1e-12, "{w:?}");
        assert!((w.oxidation_reserve_burned - 0.5).abs() < 1e-12, "{w:?}");
        assert!((w.gut_reserve_credit - 2.0).abs() < 1e-12, "{w:?}");
        assert_eq!(w.served_total, 0.0);
    }

    /// The two row predicates read the counters they claim to read.
    #[test]
    fn the_row_predicates_name_their_counters() {
        let r = row(500, 400, 0, 3, 3);
        assert!(r.readiness_overlap());
        assert!(!r.ready_pair_ever_met());
        let r = row(500, 400, 1, 3, 0);
        assert!(r.ready_pair_ever_met());
    }
}
