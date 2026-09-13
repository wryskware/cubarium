//! The bounded, **non-intervening** post-intake opportunity check
//! (`design/7_Research/root-post-intake-shadow-scope-2026-09-13.md`).
//!
//! This is a measurement, not a behaviour. Nothing here writes an organism, a field, a stock, a
//! mode, an escrow, a snapshot or an RNG counter. It reads what the tick already computed, keeps a
//! small transient per-live-ID record beside it, and emits records a harness drains. A world with
//! the shadow off is the same world it was before this module existed, operation for operation;
//! a world with it on takes the identical trajectory and additionally says what *could* have been
//! offered.
//!
//! ## What it measures, and what that is not
//!
//! At a completed boundary `B` the shadow asks one fixed question of one organism: **had the rule
//! been running, would this animal have been admitted to a 30-decision pause?** If yes, it opens a
//! *hypothetical window* covering decisions `B..B+29` and re-tests, at every later boundary, that
//! the still-freely-feeding baseline would have stayed compatible with holding it.
//!
//! The window is **not** a pause. The animal keeps feeding, moving, growing, gestating and dying
//! exactly as it would have. So:
//!
//! * the window's length is *baseline-compatible window time*, not rest time, and not a claim that
//!   the animal would have rested that long under an intervention;
//! * the baseline's intake during a window is recorded separately and is **not** "food the pause
//!   would have cost". A real pause would have changed the patch, the neighbours' shares, the
//!   animal's position and every later stock in either direction;
//! * none of this is an upper bound on anything. An intervention has a different future.
//!
//! ## The fixed rule
//!
//! * **Credit** is actual material credited to reserve by field settlement — the `to_reserve`
//!   terms the settlement stage really applied — never raw food, a `fed` flag or a reserve delta.
//!   It accumulates within an episode and is capped at `Q`.
//! * **Episode expiry** is evaluated at a settlement boundary, *before* that boundary's new
//!   settlement is added: an episode whose last positive settlement is [`EPISODE_EXPIRY_TICKS`] or
//!   more ticks old is dropped, and the new settlement opens a fresh one.
//! * **The quota** is the diet-permitted channel sum
//!   `Q = QUOTA_SECONDS · η_m · (I_g·graze_rate + I_f·graze_rate + I_s·scavenge_rate)`, with the
//!   controller's own [`DIET_GATE`]/[`FRUIT_DIET`] gates and the world's own `η_m`. It is a
//!   **diet-permitted unconstrained rate ceiling**, not an attainable meal rate: density, headroom,
//!   food energy and mode all reduce what actually settles.
//! * **An attempt** fires only when a fresh positive settlement at this boundary carries the
//!   episode to `Q`, no window is open, and no refractory deadline is outstanding. Nothing fires
//!   from a timer. Every attempt consumes the credit, refusal included.
//! * **Admission** is tested after the completed tick's own physiology, funding and death checks,
//!   with the unchanged [`Budget::of`]/[`Budget::affordable`] over [`WINDOW_DECISIONS`] decisions
//!   plus the existing safety tick.
//! * **Refractory** runs [`REFRACTORY_TICKS`] ticks from a release, an abort or a refused attempt.
//!   Credit may accumulate during it; it simply cannot fire.
//!
//! ## Bounds
//!
//! Entries are keyed by full generational [`OrganismId`], so a reused slot is a different animal
//! and inherits no credit and no cooldown. Entries are removed when the organism is removed, and
//! an idle entry — no episode, no window, no future deadline — is removed at the next commit. The
//! map never exceeds the world's live organism capacity.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::config::OrganismConfig;
use crate::controller::{DIET_GATE, FRUIT_DIET};
use crate::ids::{OrganismId, Slots};
use crate::organism::{Mode, Organism};
use crate::quiet::{Budget, horizon_seconds};

/// Wire version of the records this module emits, so an artifact states which rule produced it.
pub const SHADOW_VERSION: u32 = 1;

/// The quota's normalisation window: two seconds of the diet-permitted unconstrained rate.
pub const QUOTA_SECONDS: f64 = 2.0;

/// An episode ends after this many ticks without a positive settlement. One second at 20 Hz.
pub const EPISODE_EXPIRY_TICKS: u64 = 20;

/// The hypothetical window's length in decisions: `B..B+29`, releasing at `B + 30`.
pub const WINDOW_DECISIONS: u64 = 30;

/// Ticks of refractory after a release, an abort, or a refused attempt.
pub const REFRACTORY_TICKS: u64 = 600;

/// Why an attempt was refused, or why a hypothetical window ended before its release boundary.
///
/// A window that ends early ends for a fact about the **baseline**, which was never held and never
/// paused. It is deliberately not spelled like a production policy's abort.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ShadowReason {
    /// The organism died at this very boundary; the completed physiology does not qualify it.
    Dead,
    /// A hunter member is never offered this ordinary behaviour in this slice.
    HunterMember,
    /// Below actual adult structure.
    Juvenile,
    /// Holding an escrow, including one funded at this same boundary.
    Gestating,
    /// The tick's ordinary mode was neither Seeking nor Feeding.
    InactiveMode,
    /// One or more of the budget's inputs was not a finite nonnegative number.
    InvalidInputs,
    /// The conservative budget was not met at admission. Strict: landing on it is not meeting it.
    Unaffordable,
    /// The tick arithmetic would overflow.
    Overflow,
    /// The per-ID bound was full, so no record could be kept for this organism.
    Bounded,
    /// The budget stopped being met at a later baseline decision boundary.
    UnaffordableRemaining,
    /// The baseline organism was removed while a window was open.
    BaselineGone,
    /// The baseline funded an escrow while a window was open.
    BaselineEscrow,
    /// The baseline fell below adult structure while a window was open.
    BaselineJuvenile,
    /// The measurement ended with the window still open. Censoring, not an outcome.
    RunEnd,
}

impl ShadowReason {
    pub fn as_str(self) -> &'static str {
        match self {
            ShadowReason::Dead => "dead",
            ShadowReason::HunterMember => "hunter_member",
            ShadowReason::Juvenile => "juvenile",
            ShadowReason::Gestating => "gestating",
            ShadowReason::InactiveMode => "inactive_mode",
            ShadowReason::InvalidInputs => "invalid_inputs",
            ShadowReason::Unaffordable => "unaffordable",
            ShadowReason::Overflow => "overflow",
            ShadowReason::Bounded => "bounded",
            ShadowReason::UnaffordableRemaining => "unaffordable_remaining",
            ShadowReason::BaselineGone => "baseline_gone",
            ShadowReason::BaselineEscrow => "baseline_escrow",
            ShadowReason::BaselineJuvenile => "baseline_juvenile",
            ShadowReason::RunEnd => "run_end",
        }
    }

    /// Every reason, in a fixed order, so a per-reason table always has the same rows — including
    /// the zeros, which is what makes "never happened" readable instead of absent.
    pub const ALL: [ShadowReason; 14] = [
        ShadowReason::Dead,
        ShadowReason::HunterMember,
        ShadowReason::Juvenile,
        ShadowReason::Gestating,
        ShadowReason::InactiveMode,
        ShadowReason::InvalidInputs,
        ShadowReason::Unaffordable,
        ShadowReason::Overflow,
        ShadowReason::Bounded,
        ShadowReason::UnaffordableRemaining,
        ShadowReason::BaselineGone,
        ShadowReason::BaselineEscrow,
        ShadowReason::BaselineJuvenile,
        ShadowReason::RunEnd,
    ];
}

/// The unchanged [`Budget`]'s five numbers plus the horizon they were taken over, in a shape an
/// artifact can carry. Recorded, never used to decide anything: the decision is `Budget::affordable`
/// on the real struct.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BudgetRecord {
    pub horizon_seconds: f64,
    pub per_second: f64,
    pub growth: f64,
    pub oxidation: f64,
    pub energy: f64,
    pub material: f64,
}

impl BudgetRecord {
    fn of(b: &Budget, horizon_seconds: f64) -> BudgetRecord {
        BudgetRecord {
            horizon_seconds,
            per_second: b.per_second,
            growth: b.growth,
            oxidation: b.oxidation,
            energy: b.energy,
            material: b.material,
        }
    }
}

/// The joint per-ID state at the boundary a record was written, read from the organism after the
/// tick's own physiology, funding and death checks. Historical feeding flags and lifetime flow
/// sums cannot reconstruct these together, which is the whole reason the hook exists.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowSample {
    pub form: u8,
    pub structure: f64,
    pub structure_adult: f64,
    pub reserve: f64,
    pub energy: f64,
    pub reserve_max: f64,
    pub energy_max: f64,
    /// The tick's **ordinary** mode, as the controller decided it.
    pub mode: Mode,
    pub gestating: bool,
    /// The tick the open escrow started, when there is one: a gestation's identity, so repeat
    /// refusals can be de-duplicated per gestation without deleting them from the record.
    pub escrow_started_tick: Option<u64>,
    pub hunter_member: bool,
    pub age_ticks: u64,
    /// The diet-permitted rate ceiling. `None` when the diet permits no channel or an input was
    /// not a finite nonnegative number — no opportunity, never a division by zero.
    pub quota: Option<f64>,
    pub graze_rate: f64,
    pub scavenge_rate: f64,
    pub diet: f64,
    pub budget: Option<BudgetRecord>,
}

impl ShadowSample {
    fn read(
        o: &Organism,
        cfg: &OrganismConfig,
        grazing: bool,
        scavenging: bool,
        boundary: u64,
        hunter_member: bool,
        budget: Option<BudgetRecord>,
    ) -> ShadowSample {
        ShadowSample {
            form: o.phenotype.form,
            structure: o.structure,
            structure_adult: o.phenotype.structure_adult,
            reserve: o.reserve,
            energy: o.energy,
            reserve_max: o.phenotype.reserve_max,
            energy_max: o.phenotype.energy_max,
            mode: o.mode,
            gestating: o.escrow.is_some(),
            escrow_started_tick: o.escrow.as_ref().map(|e| e.started_tick),
            hunter_member,
            age_ticks: o.age_ticks(boundary),
            quota: quota(o, cfg, grazing, scavenging),
            graze_rate: o.phenotype.graze_rate,
            scavenge_rate: o.phenotype.scavenge_rate,
            diet: o.phenotype.diet,
            budget,
        }
    }
}

/// The episode that earned the credit an attempt consumed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EpisodeRecord {
    /// The boundary the episode's first positive settlement landed on.
    pub start_tick: u64,
    /// The boundary of its last positive settlement.
    pub last_positive_tick: u64,
    /// Boundaries within the episode that carried a positive settlement. Intake is gappy; this is
    /// not the episode's length.
    pub positive_ticks: u64,
    /// The episode's elapsed length in ticks, `last_positive_tick − start_tick`.
    pub elapsed_ticks: u64,
    /// Every unit of material the episode credited to reserve, **uncapped**.
    pub assimilated: f64,
    /// What the rule actually held, capped at `Q`.
    pub credit: f64,
}

/// Transient measurement records, drained by the observer exactly as life and quiet events are:
/// never checkpointed, never hashed, never read back by the tick, never shown on the display.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShadowEvent {
    /// A fresh positive settlement carried an episode to `Q` with no window open and no refractory
    /// outstanding. `admitted` says whether a hypothetical window opened. Repeats are kept: a
    /// gestating adult that re-earns `Q` every few seconds really does attempt every few seconds,
    /// and deleting those would misreport the rule rather than improve it.
    Attempt {
        tick: u64,
        id: OrganismId,
        admitted: bool,
        reason: Option<ShadowReason>,
        episode: EpisodeRecord,
        sample: ShadowSample,
        /// The release boundary of the window this attempt opened, when it opened one.
        window_end_tick: Option<u64>,
        /// The cooldown deadline this entry carries as the attempt ends. A **refusal** arms it
        /// here, at `tick + REFRACTORY_TICKS`. An **admission** does not: the window itself is
        /// what blocks a further attempt, and the operative deadline arrives with the close, so
        /// what an admission reports here is the spent previous deadline (always `None` or at or
        /// before `tick`), kept because it says what the animal had just come out of.
        refractory_until: Option<u64>,
    },
    /// A hypothetical window reached its release boundary with the baseline compatible throughout.
    /// This is baseline-compatible window time, not observed rest.
    Release {
        tick: u64,
        id: OrganismId,
        window_start_tick: u64,
        held_decisions: u64,
        /// What the freely feeding baseline actually assimilated across the window. Recorded
        /// separately and deliberately: it is **not** food a pause would have cost.
        baseline_intake_material: f64,
        baseline_intake_ticks: u64,
        /// The baseline died at this same boundary, after the window's last hypothetical decision.
        baseline_died_at_release: bool,
        sample: ShadowSample,
        refractory_until: Option<u64>,
    },
    /// The baseline stopped being compatible with holding the window before its release boundary.
    Abort {
        tick: u64,
        id: OrganismId,
        window_start_tick: u64,
        completed_decisions: u64,
        reason: ShadowReason,
        baseline_intake_material: f64,
        baseline_intake_ticks: u64,
        sample: Option<ShadowSample>,
        refractory_until: Option<u64>,
    },
    /// The measurement ended with the window still open. A censored interval, never a release.
    Censor {
        tick: u64,
        id: OrganismId,
        window_start_tick: u64,
        completed_decisions: u64,
        baseline_intake_material: f64,
        baseline_intake_ticks: u64,
    },
}

impl ShadowEvent {
    pub fn tick(&self) -> u64 {
        match self {
            ShadowEvent::Attempt { tick, .. }
            | ShadowEvent::Release { tick, .. }
            | ShadowEvent::Abort { tick, .. }
            | ShadowEvent::Censor { tick, .. } => *tick,
        }
    }

    pub fn id(&self) -> OrganismId {
        match self {
            ShadowEvent::Attempt { id, .. }
            | ShadowEvent::Release { id, .. }
            | ShadowEvent::Abort { id, .. }
            | ShadowEvent::Censor { id, .. } => *id,
        }
    }
}

/// The diet-permitted unconstrained rate ceiling for one organism, or `None` when the diet permits
/// no channel at all or an input is not a finite nonnegative number.
///
/// `Q = QUOTA_SECONDS · η_m · (I_g·graze_rate + I_f·graze_rate + I_s·scavenge_rate)`, where `I_g`
/// needs grazing enabled and `diet ≥ DIET_GATE`, `I_f` needs grazing enabled and
/// `diet ≥ FRUIT_DIET`, and `I_s` needs scavenging enabled and `diet ≤ 1 − DIET_GATE` — the
/// controller's own gates, read from the controller, not restated as approximate numbers here.
///
/// Two channels open on `graze_rate` because producer and fruit are two separate requests the
/// settlement really serves, and both are gated separately. This is a normalisation and not a
/// claim that any diet attains it: density, headroom, food energy density and mode all reduce
/// what actually settles.
pub fn quota(o: &Organism, cfg: &OrganismConfig, grazing: bool, scavenging: bool) -> Option<f64> {
    let p = &o.phenotype;
    let inputs = [cfg.assimilation_material, p.graze_rate, p.scavenge_rate, p.diet];
    if !inputs.iter().all(|x| x.is_finite() && *x >= 0.0) {
        return None;
    }
    let graze_channel = grazing && p.diet >= DIET_GATE;
    let fruit_channel = grazing && p.diet >= FRUIT_DIET;
    let scavenge_channel = scavenging && p.diet <= 1.0 - DIET_GATE;
    let mut sum = 0.0;
    if graze_channel {
        sum += p.graze_rate;
    }
    if fruit_channel {
        sum += p.graze_rate;
    }
    if scavenge_channel {
        sum += p.scavenge_rate;
    }
    let q = QUOTA_SECONDS * cfg.assimilation_material * sum;
    (q.is_finite() && q > 0.0).then_some(q)
}

// ---------------------------------------------------------------- per-ID state

/// One productive episode: the credit the rule holds, and what earned it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Episode {
    start_tick: u64,
    last_positive_tick: u64,
    positive_ticks: u64,
    assimilated: f64,
    credit: f64,
}

impl Episode {
    fn record(&self) -> EpisodeRecord {
        EpisodeRecord {
            start_tick: self.start_tick,
            last_positive_tick: self.last_positive_tick,
            positive_ticks: self.positive_ticks,
            elapsed_ticks: self.last_positive_tick.saturating_sub(self.start_tick),
            assimilated: self.assimilated,
            credit: self.credit,
        }
    }
}

/// One open hypothetical window. The baseline is not held and not paused; this records which
/// decisions the rule *would* have claimed and what the still-feeding baseline did meanwhile.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Window {
    start_tick: u64,
    end_tick: u64,
    /// The form the admitted organism wore. Carried so a window closed away from its organism —
    /// the commit sweep's removal, or the run's end — is still attributed to a form instead of
    /// quietly landing only in the whole-world total.
    form: u8,
    baseline_material: f64,
    baseline_ticks: u64,
}

/// Everything the rule keeps for one live full ID. Credit, window and deadline are independent:
/// dropping expired credit never drops an outstanding cooldown, and the entry survives as long as
/// any one of the three is live.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Entry {
    episode: Option<Episode>,
    window: Option<Window>,
    refractory_until: Option<u64>,
}

impl Entry {
    /// No credit, no window, no future deadline: nothing about this ID is still being tracked.
    fn idle(&self, boundary: u64) -> bool {
        self.episode.is_none()
            && self.window.is_none()
            && self.refractory_until.is_none_or(|until| boundary >= until)
    }
}

// ---------------------------------------------------------------- counters

/// Exposure and outcome tallies for one form, or for the whole world. Every field is a count of
/// boundaries or of records; nothing here is inferred from a difference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Exposure {
    /// Living organism-boundaries observed.
    pub organism_ticks: u64,
    /// ... of which at or above actual adult structure.
    pub adult_ticks: u64,
    /// ... of which adult and holding no escrow.
    pub adult_unencumbered_ticks: u64,
    /// ... of which also in an ordinary Seeking or Feeding mode and not a hunter member. This is
    /// the population the rule can ever ask about; affordability is **not** included, because it
    /// is tested only where the rule actually tests it.
    pub eligible_ticks: u64,
    /// Boundaries carrying a positive field assimilation for this organism.
    pub settlement_ticks: u64,
    /// Material actually credited to reserve by field settlement.
    pub assimilated: f64,
    /// Settlement boundaries at which the diet permitted no quota at all.
    pub settlement_ticks_without_quota: u64,
    /// Episodes dropped because [`EPISODE_EXPIRY_TICKS`] passed with no positive settlement.
    pub episodes_expired: u64,
    pub attempts: u64,
    pub admissions: u64,
    pub releases: u64,
    pub aborts: u64,
    pub censored: u64,
    /// Hypothetical decisions completed inside windows that closed, by whatever route.
    pub completed_window_decisions: u64,
    /// Material the freely feeding baseline assimilated while a window was open. Recorded, never
    /// presented as food a pause would have cost.
    pub baseline_intake_in_windows: f64,
    /// Refusals by [`ShadowReason::ALL`] index, repeats included.
    pub refusals: [u64; 14],
}

impl Exposure {
    fn refuse(&mut self, reason: ShadowReason) {
        let at = ShadowReason::ALL.iter().position(|r| *r == reason).expect("every reason is listed");
        self.refusals[at] += 1;
    }
}

/// Per-form and whole-world tallies. Forms are the phenotype's eight rig indices and every one is
/// always present, so a form that never ate keeps a visible row of zeros rather than vanishing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShadowCounters {
    pub total: Exposure,
    pub by_form: [Exposure; 8],
    /// Boundaries at which at least one window was open anywhere in the world.
    pub world_window_ticks: u64,
    /// Boundaries observed at all.
    pub world_ticks: u64,
    /// The largest the per-ID map ever grew.
    pub peak_entries: usize,
    /// Attempts that could not be recorded because the per-ID bound was full.
    pub bounded_refusals: u64,
}

impl ShadowCounters {
    fn each(&mut self, form: u8) -> [&mut Exposure; 2] {
        let index = (form as usize).min(7);
        let (total, by_form) = (&mut self.total, &mut self.by_form);
        [total, &mut by_form[index]]
    }
}

// ---------------------------------------------------------------- the shadow

/// The transient observer. Constructed by [`crate::world::World::enable_post_intake_shadow`],
/// dropped with the process, absent from every snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct PostIntakeShadow {
    /// The world's live organism capacity: the per-ID map may never exceed it.
    cap: usize,
    entries: BTreeMap<OrganismId, Entry>,
    events: Vec<ShadowEvent>,
    counters: ShadowCounters,
}

impl PostIntakeShadow {
    pub fn new(cap: usize) -> PostIntakeShadow {
        PostIntakeShadow {
            cap,
            entries: BTreeMap::new(),
            events: Vec::new(),
            counters: ShadowCounters::default(),
        }
    }

    pub fn counters(&self) -> &ShadowCounters {
        &self.counters
    }

    pub fn entries(&self) -> usize {
        self.entries.len()
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Open hypothetical windows right now. Not paused animals: nothing is held.
    pub fn open_windows(&self) -> usize {
        self.entries.values().filter(|e| e.window.is_some()).count()
    }

    pub fn drain_events(&mut self) -> Vec<ShadowEvent> {
        std::mem::take(&mut self.events)
    }

    /// A positive field-assimilation settlement of `assimilated` material credited to reserve for
    /// `id` at completed boundary `boundary`.
    ///
    /// Called from the settlement stage, from the transfers it really applied. Expiry is evaluated
    /// here, **before** this boundary's material is added, which is the whole difference between
    /// "brief nibble gaps are one meal" and "an old meal banks a rest forever".
    ///
    /// While a window is open the baseline's intake is recorded on the window and **not** credited:
    /// the animal was never actually held, so treating its continuing meal as earned credit would
    /// be measuring a counterfactual that did not happen.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_settlement(
        &mut self,
        id: OrganismId,
        o: &Organism,
        cfg: &OrganismConfig,
        grazing: bool,
        scavenging: bool,
        boundary: u64,
        assimilated: f64,
    ) {
        if !(assimilated.is_finite() && assimilated > 0.0) {
            return;
        }
        let form = o.phenotype.form;
        for e in self.counters.each(form) {
            e.settlement_ticks += 1;
            e.assimilated += assimilated;
        }
        let quota = quota(o, cfg, grazing, scavenging);
        let Some(entry) = Self::slot(&mut self.entries, self.cap, &mut self.counters, id) else {
            return;
        };
        if let Some(window) = entry.window.as_mut() {
            window.baseline_material += assimilated;
            window.baseline_ticks += 1;
            for e in self.counters.each(form) {
                e.baseline_intake_in_windows += assimilated;
            }
            return;
        }
        let Some(quota) = quota else {
            // No channel the diet can open, or an input that describes no rate: no opportunity.
            // Recorded so a per-form zero reads as "the rule never applied" and not as biology.
            for e in self.counters.each(form) {
                e.settlement_ticks_without_quota += 1;
            }
            return;
        };
        if Self::expire(entry, boundary) {
            for e in self.counters.each(form) {
                e.episodes_expired += 1;
            }
        }
        match entry.episode.as_mut() {
            Some(episode) => {
                episode.last_positive_tick = boundary;
                episode.positive_ticks += 1;
                episode.assimilated += assimilated;
                episode.credit = (episode.credit + assimilated).min(quota);
            }
            None => {
                entry.episode = Some(Episode {
                    start_tick: boundary,
                    last_positive_tick: boundary,
                    positive_ticks: 1,
                    assimilated,
                    credit: assimilated.min(quota),
                });
            }
        }
    }

    /// One organism's completed boundary, called from the physiology stage **after** that
    /// organism's oxidation, growth, gestation/funding and death checks have all run.
    ///
    /// `mode` is the tick's ordinary decided mode, `dying` says the death check named a cause at
    /// this boundary, and `hunter_member` whether the extension owns it. Nothing here is written
    /// back: the organism is read.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_boundary(
        &mut self,
        id: OrganismId,
        o: &Organism,
        cfg: &OrganismConfig,
        grazing: bool,
        scavenging: bool,
        boundary: u64,
        dt: f64,
        mode: Mode,
        dying: bool,
        hunter_member: bool,
    ) {
        let form = o.phenotype.form;
        let adult = o.structure >= o.phenotype.structure_adult;
        let unencumbered = adult && o.escrow.is_none();
        let active = matches!(mode, Mode::Seeking | Mode::Feeding);
        for e in self.counters.each(form) {
            e.organism_ticks += 1;
            e.adult_ticks += u64::from(adult);
            e.adult_unencumbered_ticks += u64::from(unencumbered);
            e.eligible_ticks += u64::from(unencumbered && active && !hunter_member);
        }

        let Some(entry) = self.entries.get(&id).copied() else {
            return;
        };
        if entry.window.is_some() {
            self.step_window(id, o, cfg, grazing, scavenging, boundary, dt, dying, hunter_member);
            return;
        }
        // A dying animal's completed physiology does not qualify it; if it had reached Q this
        // boundary the attempt is still recorded, because the opportunity really was evaluated.
        self.try_attempt(id, o, cfg, grazing, scavenging, boundary, dt, mode, dying, hunter_member);
    }

    /// Re-test an open window against the baseline at this boundary, or release it.
    #[allow(clippy::too_many_arguments)]
    fn step_window(
        &mut self,
        id: OrganismId,
        o: &Organism,
        cfg: &OrganismConfig,
        grazing: bool,
        scavenging: bool,
        boundary: u64,
        dt: f64,
        dying: bool,
        hunter_member: bool,
    ) {
        let entry = self.entries.get(&id).copied().expect("checked by the caller");
        let window = entry.window.expect("checked by the caller");
        let form = o.phenotype.form;
        // The release boundary is reached only after every one of the window's decisions is
        // behind it. A death at the release boundary is recorded on the release, not used to
        // retract decisions that had already completed.
        if boundary >= window.end_tick {
            let sample =
                ShadowSample::read(o, cfg, grazing, scavenging, boundary, hunter_member, None);
            let refractory = self.close(id, boundary, WINDOW_DECISIONS);
            for e in self.counters.each(form) {
                e.releases += 1;
                e.completed_window_decisions += WINDOW_DECISIONS;
            }
            self.events.push(ShadowEvent::Release {
                tick: boundary,
                id,
                window_start_tick: window.start_tick,
                held_decisions: WINDOW_DECISIONS,
                baseline_intake_material: window.baseline_material,
                baseline_intake_ticks: window.baseline_ticks,
                baseline_died_at_release: dying,
                sample,
                refractory_until: refractory,
            });
            return;
        }
        let remaining = window.end_tick - boundary;
        let (reason, budget) = if dying {
            (Some(ShadowReason::BaselineGone), None)
        } else if o.escrow.is_some() {
            (Some(ShadowReason::BaselineEscrow), None)
        } else if o.structure < o.phenotype.structure_adult {
            (Some(ShadowReason::BaselineJuvenile), None)
        } else if hunter_member {
            (Some(ShadowReason::HunterMember), None)
        } else {
            match horizon_seconds(remaining, dt) {
                None => (Some(ShadowReason::Overflow), None),
                Some(horizon) => match Budget::of(o, cfg, horizon) {
                    None => (Some(ShadowReason::InvalidInputs), None),
                    Some(budget) => {
                        let record = BudgetRecord::of(&budget, horizon);
                        if budget.affordable(o) {
                            (None, Some(record))
                        } else {
                            (Some(ShadowReason::UnaffordableRemaining), Some(record))
                        }
                    }
                },
            }
        };
        let Some(reason) = reason else {
            let _ = budget;
            return;
        };
        let completed = boundary.saturating_sub(window.start_tick).min(WINDOW_DECISIONS);
        let sample =
            ShadowSample::read(o, cfg, grazing, scavenging, boundary, hunter_member, budget);
        let refractory = self.close(id, boundary, completed);
        for e in self.counters.each(form) {
            e.aborts += 1;
            e.completed_window_decisions += completed;
            e.refuse(reason);
        }
        self.events.push(ShadowEvent::Abort {
            tick: boundary,
            id,
            window_start_tick: window.start_tick,
            completed_decisions: completed,
            reason,
            baseline_intake_material: window.baseline_material,
            baseline_intake_ticks: window.baseline_ticks,
            sample: Some(sample),
            refractory_until: refractory,
        });
    }

    /// Evaluate one attempt, if this boundary earned one.
    #[allow(clippy::too_many_arguments)]
    fn try_attempt(
        &mut self,
        id: OrganismId,
        o: &Organism,
        cfg: &OrganismConfig,
        grazing: bool,
        scavenging: bool,
        boundary: u64,
        dt: f64,
        mode: Mode,
        dying: bool,
        hunter_member: bool,
    ) {
        let entry = self.entries.get(&id).copied().expect("checked by the caller");
        // Nothing fires from a timer: the trigger is *this* boundary's fresh positive settlement.
        let Some(episode) = entry.episode.filter(|e| e.last_positive_tick == boundary) else {
            return;
        };
        let Some(quota) = quota(o, cfg, grazing, scavenging) else {
            return;
        };
        if episode.credit < quota {
            return;
        }
        // A cooldown suppresses the attempt; it does not consume the credit. Credit that keeps
        // accruing through a cooldown is the point of the rule, so the first fresh settlement
        // after the deadline fires, with no extra earning period demanded of the animal.
        if entry.refractory_until.is_some_and(|until| boundary < until) {
            return;
        }

        let form = o.phenotype.form;
        let record = episode.record();
        let adult = o.structure >= o.phenotype.structure_adult;
        let active = matches!(mode, Mode::Seeking | Mode::Feeding);
        let mut budget_record = None;
        let reason = if dying {
            Some(ShadowReason::Dead)
        } else if hunter_member {
            Some(ShadowReason::HunterMember)
        } else if !adult {
            Some(ShadowReason::Juvenile)
        } else if o.escrow.is_some() {
            Some(ShadowReason::Gestating)
        } else if !active {
            Some(ShadowReason::InactiveMode)
        } else {
            match horizon_seconds(WINDOW_DECISIONS, dt) {
                None => Some(ShadowReason::Overflow),
                Some(horizon) => match Budget::of(o, cfg, horizon) {
                    None => Some(ShadowReason::InvalidInputs),
                    Some(budget) => {
                        budget_record = Some(BudgetRecord::of(&budget, horizon));
                        if budget.affordable(o) { None } else { Some(ShadowReason::Unaffordable) }
                    }
                },
            }
        };
        let end_tick = boundary.checked_add(WINDOW_DECISIONS);
        let reason = match (reason, end_tick) {
            (Some(reason), _) => Some(reason),
            (None, None) => Some(ShadowReason::Overflow),
            (None, Some(_)) => None,
        };

        // The attempt consumes the credit whatever it decides. A hungry animal is never left
        // waiting on a hidden latch; the next attempt needs new food.
        let sample = ShadowSample::read(
            o,
            cfg,
            grazing,
            scavenging,
            boundary,
            hunter_member,
            budget_record,
        );
        let mut window_end = None;
        let refractory;
        {
            let entry = self.entries.get_mut(&id).expect("checked by the caller");
            entry.episode = None;
            match (reason, end_tick) {
                (None, Some(end)) => {
                    entry.window = Some(Window {
                        start_tick: boundary,
                        end_tick: end,
                        form,
                        baseline_material: 0.0,
                        baseline_ticks: 0,
                    });
                    window_end = Some(end);
                    // A window's cooldown is set when it closes, from the boundary it closed at.
                    refractory = entry.refractory_until;
                }
                _ => {
                    refractory = boundary.checked_add(REFRACTORY_TICKS);
                    entry.refractory_until = refractory;
                }
            }
        }
        for e in self.counters.each(form) {
            e.attempts += 1;
            match reason {
                None => e.admissions += 1,
                Some(reason) => e.refuse(reason),
            }
        }
        self.events.push(ShadowEvent::Attempt {
            tick: boundary,
            id,
            admitted: reason.is_none(),
            reason,
            episode: record,
            sample,
            window_end_tick: window_end,
            refractory_until: refractory,
        });
    }

    /// Close an open window and arm the cooldown from the boundary it closed at.
    fn close(&mut self, id: OrganismId, boundary: u64, _completed: u64) -> Option<u64> {
        let entry = self.entries.get_mut(&id).expect("checked by the caller");
        entry.window = None;
        entry.refractory_until = boundary.checked_add(REFRACTORY_TICKS);
        entry.refractory_until
    }

    /// At the world's commit stage, after every removal path has run: drop the entries of
    /// organisms that are gone, and the entries that are tracking nothing.
    ///
    /// An open window whose organism was removed by a path that never reached the physiology loop
    /// — a settled capture, say — is closed here with its own baseline reason rather than
    /// disappearing.
    pub fn prune_removed(&mut self, organisms: &Slots<Organism>, boundary: u64) {
        let mut gone: Vec<(OrganismId, Option<Window>)> = Vec::new();
        for (id, entry) in self.entries.iter_mut() {
            if organisms.get(*id).is_none() {
                gone.push((*id, entry.window));
                continue;
            }
            if Self::expire(entry, boundary) {
                // Counted against no form on purpose: the organism is alive and its form is
                // knowable, but this sweep deliberately reads only its own map.
                self.counters.total.episodes_expired += 1;
            }
        }
        for (id, window) in gone {
            self.entries.remove(&id);
            let Some(window) = window else { continue };
            let completed = boundary.saturating_sub(window.start_tick).min(WINDOW_DECISIONS);
            for e in self.counters.each(window.form) {
                e.aborts += 1;
                e.completed_window_decisions += completed;
                e.refuse(ShadowReason::BaselineGone);
            }
            self.events.push(ShadowEvent::Abort {
                tick: boundary,
                id,
                window_start_tick: window.start_tick,
                completed_decisions: completed,
                reason: ShadowReason::BaselineGone,
                baseline_intake_material: window.baseline_material,
                baseline_intake_ticks: window.baseline_ticks,
                sample: None,
                refractory_until: None,
            });
        }
        // Only now, when credit, window and deadline are all absent, does the entry go.
        self.entries.retain(|_, entry| !entry.idle(boundary));
        self.counters.world_ticks += 1;
        if self.entries.values().any(|e| e.window.is_some()) {
            self.counters.world_window_ticks += 1;
        }
        self.counters.peak_entries = self.counters.peak_entries.max(self.entries.len());
        debug_assert!(self.entries.len() <= self.cap, "the per-ID map exceeded the live capacity");
    }

    /// End the measurement. Every open window is censored where it stands — a censored interval,
    /// never a release, and never silently dropped so the run looks tidy.
    pub fn censor(&mut self, boundary: u64) {
        let open: Vec<(OrganismId, Window)> = self
            .entries
            .iter()
            .filter_map(|(id, e)| e.window.map(|w| (*id, w)))
            .collect();
        for (id, window) in open {
            let completed = boundary.saturating_sub(window.start_tick).min(WINDOW_DECISIONS);
            // Right-censored, not discarded: the decisions the baseline really did stay
            // compatible with are counted, and the interval is reported as censored so nobody
            // reads a truncated window as a short one.
            for e in self.counters.each(window.form) {
                e.censored += 1;
                e.completed_window_decisions += completed;
                e.refuse(ShadowReason::RunEnd);
            }
            self.events.push(ShadowEvent::Censor {
                tick: boundary,
                id,
                window_start_tick: window.start_tick,
                completed_decisions: completed,
                baseline_intake_material: window.baseline_material,
                baseline_intake_ticks: window.baseline_ticks,
            });
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.window = None;
            }
        }
    }

    /// Drop an episode whose last positive settlement is [`EPISODE_EXPIRY_TICKS`] or more ticks
    /// old. The refractory deadline is untouched: expiring credit is not serving a cooldown.
    fn expire(entry: &mut Entry, boundary: u64) -> bool {
        let stale = entry
            .episode
            .is_some_and(|e| boundary.saturating_sub(e.last_positive_tick) >= EPISODE_EXPIRY_TICKS);
        if stale {
            entry.episode = None;
        }
        stale
    }

    /// The entry for `id`, created if the bound allows it. The map never exceeds the world's live
    /// organism capacity; a refused insert is counted rather than silently growing the map.
    fn slot<'a>(
        entries: &'a mut BTreeMap<OrganismId, Entry>,
        cap: usize,
        counters: &mut ShadowCounters,
        id: OrganismId,
    ) -> Option<&'a mut Entry> {
        if !entries.contains_key(&id) {
            if entries.len() >= cap {
                counters.bounded_refusals += 1;
                counters.total.refuse(ShadowReason::Bounded);
                return None;
            }
            entries.insert(id, Entry::default());
            counters.peak_entries = counters.peak_entries.max(entries.len());
        }
        entries.get_mut(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DT;
    use crate::config::WorldConfig;
    use crate::genome::{Genome, decode};
    use crate::organism::{Escrow, Origin};
    use crate::rng::Counter;
    use cubarium_surface::{Face, SurfacePoint, Vec2};

    fn cfg() -> OrganismConfig {
        WorldConfig::default().organism
    }

    fn organism(structure: f64, reserve: f64, energy: f64) -> Organism {
        let world = WorldConfig::default();
        let mut genome = Genome::founder(0.5, &world.drives);
        genome.clamp();
        let phenotype = decode(&genome, &world.organism);
        Organism {
            pos: SurfacePoint::new(Face::Top, 10.0, 10.0),
            heading: Vec2::new(1.0, 0.0),
            ou: Vec2::ZERO,
            structure,
            reserve,
            energy,
            born_tick: 0,
            hunger_memory: 0.5,
            mode: Mode::Feeding,
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

    /// A comfortable adult: well above every bound the strict budget can ask for.
    fn adult() -> Organism {
        let mut o = organism(0.0, 0.0, 0.0);
        o.structure = o.phenotype.structure_adult;
        o.reserve = 0.5 * o.phenotype.reserve_max;
        o.energy = 0.9 * o.phenotype.energy_max;
        o
    }

    fn id(slot: u32, generation: u32) -> OrganismId {
        OrganismId { slot, generation }
    }

    /// Feed `id` one positive settlement at `boundary`, then let the boundary complete.
    fn tick(
        s: &mut PostIntakeShadow,
        id: OrganismId,
        o: &Organism,
        boundary: u64,
        assimilated: f64,
    ) {
        if assimilated > 0.0 {
            s.observe_settlement(id, o, &cfg(), true, true, boundary, assimilated);
        }
        s.observe_boundary(id, o, &cfg(), true, true, boundary, DT, o.mode, false, false);
    }

    fn events(s: &mut PostIntakeShadow) -> Vec<ShadowEvent> {
        s.drain_events()
    }

    #[test]
    fn the_quota_is_the_diet_permitted_channel_sum_and_nothing_else() {
        let cfg = cfg();
        let mut o = adult();
        o.phenotype.graze_rate = 0.03;
        o.phenotype.scavenge_rate = 0.02;

        // A pure grazer below FRUIT_DIET opens one channel; at or above it, two.
        o.phenotype.diet = FRUIT_DIET - 1e-9;
        let one = quota(&o, &cfg, true, false).unwrap();
        assert_eq!(one, QUOTA_SECONDS * cfg.assimilation_material * 0.03);
        o.phenotype.diet = FRUIT_DIET;
        let two = quota(&o, &cfg, true, false).unwrap();
        assert_eq!(two, QUOTA_SECONDS * cfg.assimilation_material * (0.03 + 0.03));

        // The scavenge gate is the controller's, from the other end of the diet axis.
        o.phenotype.diet = 1.0 - DIET_GATE;
        let with = quota(&o, &cfg, false, true).unwrap();
        assert_eq!(with, QUOTA_SECONDS * cfg.assimilation_material * 0.02);
        o.phenotype.diet = 1.0 - DIET_GATE + 1e-9;
        assert_eq!(quota(&o, &cfg, false, true), None);

        // A mechanism that is off opens nothing, and a diet below DIET_GATE cannot graze.
        o.phenotype.diet = 0.5;
        assert_eq!(quota(&o, &cfg, false, false), None);
        o.phenotype.diet = DIET_GATE - 1e-9;
        assert_eq!(quota(&o, &cfg, true, false), None);
    }

    #[test]
    fn an_invalid_rate_is_no_opportunity_rather_than_a_nan() {
        let cfg = cfg();
        let mut o = adult();
        for bad in [f64::NAN, f64::INFINITY, -1.0] {
            let mut broken = o.clone();
            broken.phenotype.graze_rate = bad;
            assert_eq!(quota(&broken, &cfg, true, true), None, "graze_rate {bad}");
            let mut broken = o.clone();
            broken.phenotype.diet = bad;
            assert_eq!(quota(&broken, &cfg, true, true), None, "diet {bad}");
        }
        // A zero maximum rate is no opportunity, not a division.
        o.phenotype.graze_rate = 0.0;
        o.phenotype.scavenge_rate = 0.0;
        assert_eq!(quota(&o, &cfg, true, true), None);

        // And nothing is ever recorded for it beyond the explicit zero-quota exposure.
        let mut s = PostIntakeShadow::new(8);
        tick(&mut s, id(1, 1), &o, 100, 1.0);
        assert!(events(&mut s).is_empty());
        assert_eq!(s.counters().total.settlement_ticks_without_quota, 1);
        assert_eq!(s.counters().total.attempts, 0);
        assert_eq!(s.entries(), 1);
    }

    #[test]
    fn the_episode_expires_at_twenty_ticks_and_not_at_nineteen() {
        let o = adult();
        let q = quota(&o, &cfg(), true, true).unwrap();
        let bite = q / 4.0;
        for (gap, expected) in [(19u64, 2.0 * bite), (20, bite)] {
            let mut s = PostIntakeShadow::new(8);
            let who = id(3, 1);
            tick(&mut s, who, &o, 100, bite);
            tick(&mut s, who, &o, 100 + gap, bite);
            let episode = s.entries.get(&who).unwrap().episode.unwrap();
            assert_eq!(episode.credit, expected, "gap {gap}");
            assert_eq!(episode.start_tick, if gap == 19 { 100 } else { 100 + gap }, "gap {gap}");
        }
    }

    #[test]
    fn credit_is_capped_at_the_quota_and_every_attempt_consumes_it() {
        let o = adult();
        let q = quota(&o, &cfg(), true, true).unwrap();
        let mut s = PostIntakeShadow::new(8);
        let who = id(2, 7);
        // Two bites, the second far larger than the remaining headroom.
        tick(&mut s, who, &o, 500, q / 2.0);
        assert!(events(&mut s).is_empty(), "half the quota must not fire");
        tick(&mut s, who, &o, 501, q * 10.0);
        let drained = events(&mut s);
        assert_eq!(drained.len(), 1);
        let ShadowEvent::Attempt { admitted, episode, window_end_tick, .. } = &drained[0] else {
            panic!("expected an attempt, got {drained:?}");
        };
        assert!(admitted, "a comfortable adult in an active mode must be admitted");
        assert_eq!(episode.credit, q, "credit is capped at the quota");
        assert_eq!(episode.assimilated, q / 2.0 + q * 10.0, "the uncapped total is kept too");
        assert_eq!(episode.start_tick, 500);
        assert_eq!(episode.elapsed_ticks, 1);
        assert_eq!(*window_end_tick, Some(501 + WINDOW_DECISIONS));
        assert!(s.entries.get(&who).unwrap().episode.is_none(), "the attempt consumed the credit");
    }

    #[test]
    fn the_window_runs_exactly_thirty_decisions_and_releases_at_its_boundary() {
        let o = adult();
        let q = quota(&o, &cfg(), true, true).unwrap();
        let mut s = PostIntakeShadow::new(8);
        let who = id(0, 1);
        tick(&mut s, who, &o, 1000, q);
        assert!(matches!(events(&mut s)[0], ShadowEvent::Attempt { admitted: true, .. }));
        for boundary in 1001..1000 + WINDOW_DECISIONS {
            s.observe_boundary(who, &o, &cfg(), true, true, boundary, DT, o.mode, false, false);
            assert!(events(&mut s).is_empty(), "boundary {boundary} must stay inside the window");
            assert_eq!(s.open_windows(), 1);
        }
        let release = 1000 + WINDOW_DECISIONS;
        s.observe_boundary(who, &o, &cfg(), true, true, release, DT, o.mode, false, false);
        let drained = events(&mut s);
        let [ShadowEvent::Release { tick, held_decisions, window_start_tick, refractory_until, .. }] =
            &drained[..]
        else {
            panic!("expected one release, got {drained:?}");
        };
        assert_eq!((*tick, *held_decisions, *window_start_tick), (release, WINDOW_DECISIONS, 1000));
        assert_eq!(*refractory_until, Some(release + REFRACTORY_TICKS));
        assert_eq!(s.open_windows(), 0);
    }

    #[test]
    fn the_baselines_intake_during_a_window_is_recorded_and_never_credited() {
        let o = adult();
        let q = quota(&o, &cfg(), true, true).unwrap();
        let mut s = PostIntakeShadow::new(8);
        let who = id(0, 1);
        tick(&mut s, who, &o, 1000, q);
        let _ = events(&mut s);
        for boundary in 1001..1000 + WINDOW_DECISIONS {
            tick(&mut s, who, &o, boundary, q);
        }
        assert!(events(&mut s).is_empty(), "a window may not fire a second attempt");
        s.observe_boundary(who, &o, &cfg(), true, true, 1030, DT, o.mode, false, false);
        let drained = events(&mut s);
        let [ShadowEvent::Release { baseline_intake_material, baseline_intake_ticks, .. }] =
            &drained[..]
        else {
            panic!("expected one release, got {drained:?}");
        };
        assert_eq!(*baseline_intake_ticks, WINDOW_DECISIONS - 1);
        // The running sum the window really accumulated, add for add: a closed-form product is a
        // different float and would make this assertion about arithmetic rather than about the rule.
        let expected = (0..WINDOW_DECISIONS - 1).fold(0.0, |acc, _| acc + q);
        assert_eq!(*baseline_intake_material, expected);
        assert!(
            s.entries.get(&who).unwrap().episode.is_none(),
            "window intake must not have become credit"
        );
    }

    #[test]
    fn no_attempt_fires_before_the_refractory_deadline_and_credit_survives_it() {
        let o = adult();
        let q = quota(&o, &cfg(), true, true).unwrap();
        let mut s = PostIntakeShadow::new(8);
        let who = id(5, 2);
        tick(&mut s, who, &o, 2000, q);
        let release = 2000 + WINDOW_DECISIONS;
        for boundary in 2001..=release {
            s.observe_boundary(who, &o, &cfg(), true, true, boundary, DT, o.mode, false, false);
        }
        let _ = events(&mut s);
        let deadline = release + REFRACTORY_TICKS;

        // Credit forms during the cooldown and is not consumed by it: no attempt is even made.
        tick(&mut s, who, &o, deadline - 1, q);
        assert!(events(&mut s).is_empty(), "an attempt fired one tick early");
        assert_eq!(s.entries.get(&who).unwrap().episode.unwrap().credit, q);

        // The deadline alone fires nothing; only fresh assimilation can.
        s.observe_boundary(who, &o, &cfg(), true, true, deadline, DT, o.mode, false, false);
        assert!(events(&mut s).is_empty(), "a timer fired an attempt without fresh assimilation");

        // The very next positive settlement fires, with no extra earning period demanded.
        tick(&mut s, who, &o, deadline + 1, q / 1000.0);
        let drained = events(&mut s);
        let [ShadowEvent::Attempt { admitted: true, episode, .. }] = &drained[..] else {
            panic!("expected one admitted attempt, got {drained:?}");
        };
        assert_eq!(episode.start_tick, deadline - 1, "the cooldown episode carried through");
    }

    #[test]
    fn a_refusal_consumes_the_credit_and_arms_the_same_cooldown() {
        let mut o = adult();
        o.escrow = Some(Escrow {
            structure: 0.1,
            reserve: 0.1,
            energy: 0.1,
            started_tick: 40,
            genome: o.genome.clone(),
        });
        let q = quota(&o, &cfg(), true, true).unwrap();
        let mut s = PostIntakeShadow::new(8);
        let who = id(9, 3);
        tick(&mut s, who, &o, 100, q);
        let drained = events(&mut s);
        let [ShadowEvent::Attempt { admitted: false, reason, refractory_until, sample, .. }] =
            &drained[..]
        else {
            panic!("expected one refusal, got {drained:?}");
        };
        assert_eq!(*reason, Some(ShadowReason::Gestating));
        assert_eq!(*refractory_until, Some(100 + REFRACTORY_TICKS));
        assert_eq!(sample.escrow_started_tick, Some(40), "the gestation is identifiable");
        assert!(s.entries.get(&who).unwrap().episode.is_none());

        // A gestating adult keeps eating and really does attempt again after each cooldown. The
        // repeats are the rule's behaviour and are kept, not collapsed.
        tick(&mut s, who, &o, 100 + REFRACTORY_TICKS, q);
        let again = events(&mut s);
        assert_eq!(again.len(), 1, "the repeat attempt is preserved");
        assert_eq!(s.counters().total.attempts, 2);
        let at = ShadowReason::ALL.iter().position(|r| *r == ShadowReason::Gestating).unwrap();
        assert_eq!(s.counters().total.refusals[at], 2);
    }

    #[test]
    fn exact_budget_equality_is_refused_because_the_test_is_strict() {
        let mut o = adult();
        let horizon = horizon_seconds(WINDOW_DECISIONS, DT).unwrap();
        let budget = Budget::of(&o, &cfg(), horizon).unwrap();
        o.energy = budget.energy;
        o.reserve = budget.material + 1.0;
        let q = quota(&o, &cfg(), true, true).unwrap();
        let mut s = PostIntakeShadow::new(8);
        tick(&mut s, id(1, 1), &o, 100, q);
        let drained = events(&mut s);
        let [ShadowEvent::Attempt { admitted: false, reason, sample, .. }] = &drained[..] else {
            panic!("expected one refusal, got {drained:?}");
        };
        assert_eq!(*reason, Some(ShadowReason::Unaffordable));
        assert_eq!(sample.budget.unwrap().energy, budget.energy, "the tested bound is recorded");
    }

    #[test]
    fn the_admission_reasons_are_ordered_and_every_one_is_reachable() {
        let q = quota(&adult(), &cfg(), true, true).unwrap();
        let juvenile = {
            let mut o = adult();
            o.structure = 0.5 * o.phenotype.structure_adult;
            o
        };
        let gestating = {
            let mut o = adult();
            o.escrow = Some(Escrow {
                structure: 0.1,
                reserve: 0.1,
                energy: 0.1,
                started_tick: 1,
                genome: o.genome.clone(),
            });
            o
        };
        let broke = {
            let mut o = adult();
            o.energy = 0.0;
            o.reserve = 0.4 * o.phenotype.reserve_max;
            o
        };
        // (organism, dying, member, mode, expected reason)
        let cases: [(Organism, bool, bool, Mode, ShadowReason); 6] = [
            (adult(), true, false, Mode::Feeding, ShadowReason::Dead),
            (adult(), false, true, Mode::Feeding, ShadowReason::HunterMember),
            (juvenile, false, false, Mode::Feeding, ShadowReason::Juvenile),
            (gestating, false, false, Mode::Feeding, ShadowReason::Gestating),
            (adult(), false, false, Mode::Resting, ShadowReason::InactiveMode),
            (broke, false, false, Mode::Feeding, ShadowReason::Unaffordable),
        ];
        for (o, dying, member, mode, expected) in cases {
            let mut s = PostIntakeShadow::new(8);
            let who = id(1, 1);
            s.observe_settlement(who, &o, &cfg(), true, true, 100, q);
            s.observe_boundary(who, &o, &cfg(), true, true, 100, DT, mode, dying, member);
            let drained = events(&mut s);
            let [ShadowEvent::Attempt { admitted: false, reason, .. }] = &drained[..] else {
                panic!("expected a refusal for {expected:?}, got {drained:?}");
            };
            assert_eq!(*reason, Some(expected));
        }
    }

    #[test]
    fn a_same_boundary_funding_or_death_refuses_on_its_own_terms() {
        let q = quota(&adult(), &cfg(), true, true).unwrap();
        // Funded an escrow at this very boundary: the completed physiology no longer qualifies it,
        // and the child it just paid for is never deprioritised for this.
        let mut funded = adult();
        funded.escrow = Some(Escrow {
            structure: 0.1,
            reserve: 0.1,
            energy: 0.1,
            started_tick: 300,
            genome: funded.genome.clone(),
        });
        let mut s = PostIntakeShadow::new(8);
        tick(&mut s, id(1, 1), &funded, 300, q);
        let drained = events(&mut s);
        assert!(matches!(
            drained[..],
            [ShadowEvent::Attempt { reason: Some(ShadowReason::Gestating), .. }]
        ));

        // Died at this very boundary: the attempt is recorded rather than vanishing.
        let mut s = PostIntakeShadow::new(8);
        let who = id(1, 1);
        let o = adult();
        s.observe_settlement(who, &o, &cfg(), true, true, 300, q);
        s.observe_boundary(who, &o, &cfg(), true, true, 300, DT, Mode::Feeding, true, false);
        let drained = events(&mut s);
        assert!(matches!(
            drained[..],
            [ShadowEvent::Attempt { reason: Some(ShadowReason::Dead), .. }]
        ));
    }

    #[test]
    fn a_window_ends_on_the_baselines_own_change_with_its_own_reason() {
        let o = adult();
        let q = quota(&o, &cfg(), true, true).unwrap();
        // The baseline funds an escrow mid-window: the compatible window ends, and the record says
        // the baseline changed rather than claiming a policy aborted a pause that never ran.
        let mut s = PostIntakeShadow::new(8);
        let who = id(4, 1);
        tick(&mut s, who, &o, 700, q);
        let _ = events(&mut s);
        let mut later = o.clone();
        later.escrow = Some(Escrow {
            structure: 0.1,
            reserve: 0.1,
            energy: 0.1,
            started_tick: 710,
            genome: o.genome.clone(),
        });
        s.observe_boundary(who, &later, &cfg(), true, true, 710, DT, Mode::Feeding, false, false);
        let drained = events(&mut s);
        let [ShadowEvent::Abort { reason, completed_decisions, .. }] = &drained[..] else {
            panic!("expected one abort, got {drained:?}");
        };
        assert_eq!((*reason, *completed_decisions), (ShadowReason::BaselineEscrow, 10));

        // And the strict budget is re-tested at every later boundary, not only at admission.
        let mut s = PostIntakeShadow::new(8);
        tick(&mut s, who, &o, 700, q);
        let _ = events(&mut s);
        let mut starving = o.clone();
        starving.energy = 0.0;
        s.observe_boundary(who, &starving, &cfg(), true, true, 705, DT, Mode::Feeding, false, false);
        let drained = events(&mut s);
        let [ShadowEvent::Abort { reason, completed_decisions, sample, .. }] = &drained[..] else {
            panic!("expected one abort, got {drained:?}");
        };
        assert_eq!((*reason, *completed_decisions), (ShadowReason::UnaffordableRemaining, 5));
        assert!(sample.unwrap().budget.is_some(), "the re-tested bound is recorded");
    }

    #[test]
    fn a_reused_slot_inherits_no_credit_and_no_cooldown() {
        let o = adult();
        let q = quota(&o, &cfg(), true, true).unwrap();
        let mut s = PostIntakeShadow::new(8);
        let first = id(6, 1);
        tick(&mut s, first, &o, 100, q);
        assert!(matches!(events(&mut s)[..], [ShadowEvent::Attempt { admitted: true, .. }]));

        // The same slot, a new generation, at the very next boundary: a different animal.
        let second = id(6, 2);
        tick(&mut s, second, &o, 101, q);
        let drained = events(&mut s);
        let [ShadowEvent::Attempt { admitted: true, id: who, .. }] = &drained[..] else {
            panic!("expected the new occupant's own admitted attempt, got {drained:?}");
        };
        assert_eq!(*who, second);
        assert_eq!(s.entries(), 2, "the two generations are separate entries");
        assert!(s.entries.get(&first).unwrap().window.is_some());
        assert!(s.entries.get(&second).unwrap().window.is_some());
    }

    #[test]
    fn dropping_expired_credit_never_drops_an_outstanding_cooldown() {
        let mut entry = Entry {
            episode: Some(Episode {
                start_tick: 10,
                last_positive_tick: 10,
                positive_ticks: 1,
                assimilated: 1.0,
                credit: 1.0,
            }),
            window: None,
            refractory_until: Some(900),
        };
        assert!(PostIntakeShadow::expire(&mut entry, 10 + EPISODE_EXPIRY_TICKS));
        assert!(entry.episode.is_none(), "the stale episode is gone");
        assert_eq!(entry.refractory_until, Some(900), "the cooldown survived it");
        assert!(!entry.idle(30), "an entry with a future deadline is not idle");
        assert!(entry.idle(900), "and is idle once the deadline has passed");
    }

    #[test]
    fn the_final_open_window_is_censored_and_never_counted_as_a_release() {
        let o = adult();
        let q = quota(&o, &cfg(), true, true).unwrap();
        let mut s = PostIntakeShadow::new(8);
        let who = id(7, 1);
        tick(&mut s, who, &o, 100, q);
        let _ = events(&mut s);
        s.censor(110);
        let drained = events(&mut s);
        let [ShadowEvent::Censor { tick, completed_decisions, window_start_tick, .. }] = &drained[..]
        else {
            panic!("expected one censor, got {drained:?}");
        };
        assert_eq!((*tick, *completed_decisions, *window_start_tick), (110, 10, 100));
        assert_eq!(s.counters().total.releases, 0, "a censored interval is not a release");
        assert_eq!(s.counters().total.censored, 1);
    }

    #[test]
    fn the_per_id_map_is_bounded_by_the_live_capacity() {
        let o = adult();
        let q = quota(&o, &cfg(), true, true).unwrap();
        let mut s = PostIntakeShadow::new(2);
        for slot in 0..5u32 {
            s.observe_settlement(id(slot, 1), &o, &cfg(), true, true, 100, q / 8.0);
        }
        assert_eq!(s.entries(), 2, "the map stopped at the capacity");
        assert_eq!(s.counters().bounded_refusals, 3);
        assert!(s.counters().peak_entries <= s.capacity());
    }

    #[test]
    fn an_idle_entry_is_removed_only_when_nothing_is_still_tracked() {
        let o = adult();
        let q = quota(&o, &cfg(), true, true).unwrap();
        let mut arena: Slots<Organism> = Slots::with_capacity(4);
        let live = arena.insert(o.clone());
        let mut s = PostIntakeShadow::new(4);

        // Credit alone keeps the entry; expiring it with no window and no deadline removes it.
        s.observe_settlement(live, &o, &cfg(), true, true, 100, q / 8.0);
        s.prune_removed(&arena, 100);
        assert_eq!(s.entries(), 1);
        s.prune_removed(&arena, 100 + EPISODE_EXPIRY_TICKS);
        assert_eq!(s.entries(), 0, "an idle entry is dropped");

        // A removed organism's entry goes with it, and an open window is closed, not lost.
        s.observe_settlement(live, &o, &cfg(), true, true, 200, q);
        s.observe_boundary(live, &o, &cfg(), true, true, 200, DT, Mode::Feeding, false, false);
        let _ = events(&mut s);
        arena.remove(live);
        s.prune_removed(&arena, 205);
        let drained = events(&mut s);
        let [ShadowEvent::Abort { reason, completed_decisions, .. }] = &drained[..] else {
            panic!("expected one abort for the removed organism, got {drained:?}");
        };
        assert_eq!((*reason, *completed_decisions), (ShadowReason::BaselineGone, 5));
        assert_eq!(s.entries(), 0);
    }

    /// A window that closes away from its organism — censored at the run's end, or swept when the
    /// organism was removed — still belongs to a form. Counting it only in the whole-world total
    /// makes the per-form rows disagree with the event stream, which is exactly the disagreement
    /// an independent reduction is for.
    #[test]
    fn a_window_closed_away_from_its_organism_still_lands_in_its_forms_row() {
        let o = adult();
        let form = (o.phenotype.form as usize).min(7);
        let q = quota(&o, &cfg(), true, true).unwrap();

        let mut s = PostIntakeShadow::new(8);
        let who = id(2, 1);
        tick(&mut s, who, &o, 100, q);
        let _ = events(&mut s);
        s.censor(112);
        let c = s.counters();
        assert_eq!(c.total.censored, 1);
        assert_eq!(c.by_form[form].censored, 1, "the censored window lost its form");
        assert_eq!(c.total.completed_window_decisions, 12);
        assert_eq!(
            c.by_form[form].completed_window_decisions, 12,
            "a right-censored window's compatible decisions are real and belong to the form"
        );

        let mut arena: Slots<Organism> = Slots::with_capacity(4);
        let live = arena.insert(o.clone());
        let mut s = PostIntakeShadow::new(4);
        tick(&mut s, live, &o, 200, q);
        let _ = events(&mut s);
        arena.remove(live);
        s.prune_removed(&arena, 207);
        let c = s.counters();
        assert_eq!(c.by_form[form].aborts, 1, "the swept window lost its form");
        assert_eq!(c.by_form[form].completed_window_decisions, 7);
    }

    #[test]
    fn no_intake_produces_no_record_at_all() {
        let o = adult();
        let mut s = PostIntakeShadow::new(8);
        for boundary in 0..200 {
            s.observe_boundary(id(1, 1), &o, &cfg(), true, true, boundary, DT, o.mode, false, false);
        }
        assert!(events(&mut s).is_empty());
        assert_eq!(s.counters().total.attempts, 0);
        assert_eq!(s.counters().total.organism_ticks, 200);
        assert_eq!(s.counters().total.eligible_ticks, 200);
        assert_eq!(s.entries(), 0, "an organism that never ate is never tracked");
    }
}
