//! One voxel episode: one prepared layout, one placed founder, **one controller attached
//! through the fauna's own controller table**, one single-threaded static schedule, run
//! to horizon or death.
//!
//! An episode is exactly:
//!
//! 1. [`Prepared::episode_arena`] — the immutable layout's private mutable copy: stocks,
//!    ledger and the placed animal all start at the prepared state. The fauna's
//!    controller table clones **empty** ("cloning a body does not clone a mind"), so
//!    every episode installs its own.
//! 2. [`EpisodeDriver::fresh`] — a fresh controller with fresh memory, `reset()` called —
//!    attached through `Fauna::set_controller(animal_id, …)`. Refusal is an experiment
//!    error, never a silent rest episode.
//! 3. `arena.into_sim_prepared(SimConfig { threads: 1 }, senses)` — one **simulation
//!    thread** per episode, seeded from the fixture's settled cue field.
//! 4. [`Sim::step`] until the horizon or the animal is gone. The **fauna tick** owns the
//!    whole controller path: at each due tick it builds the observation, drives the
//!    installed controller, resolves the response through the shared `resolve_actions`
//!    adapter, and holds the actions in the body's `founder_state.held`. The driver
//!    samples nothing, holds nothing and resolves nothing.
//!
//! # Limits are checked *inside* the episode
//!
//! [`Limits`] carries the shared cancellation flag **and** the run's wall-clock deadline,
//! and both are read every [`CANCEL_CHECK_TICKS`] ticks. A cancelled episode reports the
//! ticks it had already simulated, so a discarded generation's work still counts against
//! the budget even though it never reaches the optimizer.
//!
//! # The boundaries this driver holds
//!
//! - **The driver never touches the arena's settlement APIs.** `Arena::resources`,
//!   `resource_stock()` and `take()` are the fixture's privileged stock/settlement
//!   surface. Everything this loop reads is read-only: the placed animal, the fauna view
//!   and its ledger.
//! - **The driver reads outcomes, never controller internals.** The held actions it
//!   reports are the body's own `founder_state.held` — world state the fauna tick
//!   resolved — not a driver-side copy.
//! - **Integrity in release:** the fauna rejects a non-finite observation and the shared
//!   adapter rejects non-finite logits, so the driver checks what remains its own: the
//!   placed animal's numbers, every tick. A violation fails the **experiment** by name,
//!   never a score.
//!
//! # What is measured, and the score-counter state
//!
//! [`Episode`] carries the [`ScoreComponents`] (tests plan §2) and the raw measurements
//! behind them. The fauna ledger splits respiration into maintenance, motor, and
//! digestion. The driver therefore measures settled assimilated intake as
//! `Δorganic + maintenance + motor + corpse`; digestive respiration is deliberately
//! excluded, because it is the share of a bite that never became animal tissue.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use cubarium_voxel_fauna::{Animal, Founder};
use cubarium_voxel_sim::{Sim, SimConfig, Site};
use serde::{Deserialize, Serialize};

use super::controller::EpisodeDriver;
use super::score::ScoreComponents;
use super::task::{DEPLETION_FRACTION, Prepared};

/// How often the shared cancellation flag and the deadline are read inside an episode.
///
/// 64 ticks is 3.2 simulated seconds at 20 Hz — a few hundred microseconds of wall time
/// at the measured static-arena tick rate.
pub const CANCEL_CHECK_TICKS: u64 = 64;

/// The limits one rollout runs under. Both are checked inside the episode.
#[derive(Clone, Copy)]
pub struct Limits<'a> {
    pub cancel: &'a AtomicBool,
    /// When the whole run must stop. `None` means the caller imposed no clock; the
    /// cancellation flag still applies.
    pub deadline: Option<Instant>,
}

impl<'a> Limits<'a> {
    /// No clock, only the flag.
    pub fn new(cancel: &'a AtomicBool) -> Limits<'a> {
        Limits {
            cancel,
            deadline: None,
        }
    }

    /// Flag and clock.
    pub fn until(cancel: &'a AtomicBool, deadline: Instant) -> Limits<'a> {
        Limits {
            cancel,
            deadline: Some(deadline),
        }
    }

    /// True when the run must stop now. A passed deadline **sets** the shared flag, so
    /// one worker noticing the clock stops every other worker's episode at its next
    /// check rather than each discovering it separately.
    pub fn expired(&self) -> bool {
        if self.cancel.load(Ordering::Relaxed) {
            return true;
        }
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            self.cancel.store(true, Ordering::Relaxed);
            return true;
        }
        false
    }
}

/// The score components are measured from the fauna ledger's split counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScoreCounters {
    /// The driver reads maintenance and motor respiration separately, so intake is
    /// settled assimilated organic rather than gross bite organic.
    #[default]
    Landed,
}

impl ScoreCounters {
    pub fn as_str(self) -> &'static str {
        match self {
            ScoreCounters::Landed => "landed (split respiration ledger)",
        }
    }
}

/// Why an episode produced no score.
///
/// Neither is a low fitness. A cancelled episode was stopped by the run's limits; an
/// invalid one is an **experiment error** and names the job it happened in.
#[derive(Clone, Debug, PartialEq)]
pub enum EpisodeError {
    /// The run's cancellation flag or deadline stopped this rollout. Carries the ticks
    /// already simulated, so discarded work is still counted.
    Cancelled { ticks: u64 },
    /// A fixture or state invariant failed. The experiment fails here.
    Invalid { ticks: u64, detail: String },
}

impl EpisodeError {
    pub fn ticks(&self) -> u64 {
        match self {
            EpisodeError::Cancelled { ticks } | EpisodeError::Invalid { ticks, .. } => *ticks,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, EpisodeError::Cancelled { .. })
    }
}

impl std::fmt::Display for EpisodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EpisodeError::Cancelled { ticks } => write!(f, "cancelled after {ticks} ticks"),
            EpisodeError::Invalid { ticks, detail } => {
                write!(f, "invalid after {ticks} ticks: {detail}")
            }
        }
    }
}

impl std::error::Error for EpisodeError {}

/// Stage B's accounting, measured **outside** the controller boundary.
///
/// The two patch sites come from `Arena::build_reacquisition`'s fixture metadata
/// (`ReacquisitionArena::into_parts`), which the arena keeps precisely so an evaluator
/// can say what happened without the policy ever being told where food is. Every number
/// here is read from the plant layer's live stock at those two sites; nothing here
/// enters an observation and nothing here enters the score. The score stays the plan's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Reacquisition {
    /// The stock each patch held at the first tick.
    pub initial_start: f64,
    pub successor_start: f64,
    /// What it held at the last tick.
    pub initial_end: f64,
    pub successor_end: f64,
    /// Gross organic taken off each patch: the intake split by patch. Nothing else
    /// consumes a static arena's stock, so the drop *is* what crossed the mouth.
    pub initial_taken: f64,
    pub successor_taken: f64,
    /// The stated depletion threshold, as a fraction of the initial patch's start.
    pub depletion_fraction: f64,
    /// The first tick the initial patch was below that threshold.
    pub depleted_tick: Option<u64>,
    /// The first tick the successor patch's stock fell at all.
    pub successor_first_bite_tick: Option<u64>,
    /// The initial patch was depleted and the founder then fed at the successor. A bite
    /// taken at the successor *before* the first patch ran out is a second patch found
    /// early, not a reacquisition, and is reported through the two ticks rather than
    /// counted here.
    pub reacquired: bool,
}

impl Reacquisition {
    /// The order the boolean requires: the first patch ran out, and only then did the
    /// founder feed at the successor. A successor bitten first is a second patch found
    /// early — worth seeing in the two ticks, not worth calling reacquisition.
    pub fn is_reacquisition(depleted_tick: Option<u64>, first_bite_tick: Option<u64>) -> bool {
        match (depleted_tick, first_bite_tick) {
            (Some(depleted), Some(bite)) => bite >= depleted,
            _ => false,
        }
    }
}

/// The outcome of one episode. Every field is measured on every episode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub founder: String,
    pub layout_seed: u64,
    /// Which driver ran: `gru`, or one of the disclosed control names.
    pub driver: String,
    pub horizon: u64,
    /// Ticks actually simulated. The horizon when the body survived it; the tick the body
    /// was found gone otherwise. Ordinary death is a completed episode, not an error.
    pub ticks: u64,
    pub alive: bool,
    /// The score and its separately reported components.
    pub score: ScoreComponents,
    /// Which counter state the components were measured in (see [`ScoreCounters`]).
    pub counters: ScoreCounters,
    // --- raw measurements behind the components ---
    /// The placed animal's organic matter at the first tick.
    pub start_organic: f64,
    /// What the body still held at the last tick, plus its corpse deposit if it died:
    /// the organic that stayed in the world as that animal's tissue.
    pub end_organic: f64,
    /// The corpse deposit alone (`0.0` for a survivor).
    pub corpse_organic: f64,
    /// Settled assimilated intake: organic that became tissue before maintenance and
    /// motor charges, never the gross bite withdrawal.
    pub intake_organic: f64,
    /// Organic matter the founder's motion charged.
    pub motor_organic: f64,
    /// Basal upkeep respiration over the episode.
    pub maintenance_organic: f64,
    /// Digestion respiration over the episode: the bite share that did not settle.
    pub digestion_organic: f64,
    /// All three respiration categories over the episode.
    pub respired_organic: f64,
    /// The gross organic that crossed the mouth through real `Taken` withdrawals — the
    /// ledger's own boundary reading. It equals settled intake plus digestion.
    pub eaten_organic: f64,
    // --- diagnostics, never part of the ordering ---
    pub start_energy: f64,
    pub end_energy: f64,
    /// Controller samplings the episode's cadence implies: `ticks / cadence_ticks`
    /// (the final partial interval is never sampled). The fauna samples internally;
    /// this is the driver's count of the due ticks.
    pub updates: u64,
    /// Time-average of the body's held actions over the ticks simulated, read from the
    /// body's own `founder_state.held` — rest before the first sampling included.
    pub mean_forward: f64,
    pub mean_turn_abs: f64,
    pub mean_feed: f64,
    /// The terminal pose, metres and radians, read-only.
    pub pose_x: f64,
    pub pose_z: f64,
    pub heading_rad: f64,
    /// Stage B's patch accounting; `None` on a Stage-A layout.
    #[serde(default)]
    pub reacquisition: Option<Reacquisition>,
}

impl Episode {
    /// `ticks / horizon`: the survival term's fraction.
    pub fn survived_fraction(&self) -> f64 {
        (self.ticks as f64 / self.horizon as f64).clamp(0.0, 1.0)
    }
}

/// Run one episode from a fresh arena built from `(founder, layout_seed)`.
///
/// The bench's second setup path; the trainer always runs [`run_prepared`].
pub fn run(
    founder: Founder,
    layout_seed: u64,
    driver: &EpisodeDriver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
) -> Result<Episode, EpisodeError> {
    let prepared = Prepared::build(founder, layout_seed);
    episode_from(&prepared, driver, horizon, limits, job)
}

/// Run one episode from a prepared layout's private mutable copy.
pub fn run_prepared(
    prepared: &Prepared,
    driver: &EpisodeDriver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
) -> Result<Episode, EpisodeError> {
    episode_from(prepared, driver, horizon, limits, job)
}

/// The episode body.
fn episode_from(
    prepared: &Prepared,
    driver: &EpisodeDriver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
) -> Result<Episode, EpisodeError> {
    let founder = prepared.founder;
    if driver.founder() != founder {
        return Err(EpisodeError::Invalid {
            ticks: 0,
            detail: format!(
                "job {job}: driver is for {} but the arena places {}",
                driver.founder().name(),
                founder.name()
            ),
        });
    }
    let manifest = founder.manifest();
    let arena = prepared.episode_arena();

    let invalid = |ticks: u64, detail: String| {
        Err(EpisodeError::Invalid {
            ticks,
            detail: format!("job {job}: {detail}"),
        })
    };

    let Some(id) = arena.animal_id else {
        return invalid(0, "the arena placed no founder body".into());
    };
    let cadence = manifest.cadence_ticks().max(1);

    // One simulation thread per episode: the fauna leg stays off the process-wide pool.
    let mut sim = arena.into_sim_prepared(SimConfig { threads: 1 }, prepared.episode_senses());

    // Attach the episode's controller: fresh memory by construction, `reset()` by
    // contract. The fauna table clones empty, so this is the only mind the body gets.
    let controller = driver.fresh();
    if !sim.fauna_mut().set_controller(id, controller) {
        return invalid(0, "the fauna refused the episode's controller".into());
    }

    let Some(start) = sim.fauna().view().animal(id) else {
        return invalid(0, "the placed animal is already gone".into());
    };
    let mut episode = Episode {
        founder: founder.name().to_string(),
        layout_seed: prepared.layout_seed,
        driver: driver.name(),
        horizon,
        ticks: 0,
        alive: true,
        score: ScoreComponents::default(),
        counters: ScoreCounters::Landed,
        start_organic: start.organic(),
        end_organic: 0.0,
        corpse_organic: 0.0,
        intake_organic: 0.0,
        motor_organic: 0.0,
        maintenance_organic: 0.0,
        digestion_organic: 0.0,
        respired_organic: 0.0,
        eaten_organic: 0.0,
        start_energy: start.energy,
        end_energy: 0.0,
        updates: 0,
        mean_forward: 0.0,
        mean_turn_abs: 0.0,
        mean_feed: 0.0,
        pose_x: start.pose.x,
        pose_z: start.pose.z,
        heading_rad: start.pose.heading_rad,
        reacquisition: None,
    };

    // Stage B's evaluator-side accounting. The two sites are the fixture's, read here
    // and nowhere the controller can reach; the stock readings come off the plant layer
    // exactly as the arena's own `patch_stock` reads them.
    let patch_stock = |sim: &Sim, site: Site| -> f64 {
        let fv = sim.flora().view();
        match founder {
            Founder::Blind => fv.ground_at(site).map_or(0.0, |g| g.litter),
            Founder::Browser => fv.stand_at(site).map_or(0.0, |s| s.foliage),
        }
    };
    let mut patches = prepared.patches().map(|(initial, successor)| {
        (
            initial,
            successor,
            Reacquisition {
                initial_start: patch_stock(&sim, initial),
                successor_start: patch_stock(&sim, successor),
                depletion_fraction: DEPLETION_FRACTION,
                ..Reacquisition::default()
            },
        )
    });

    // The ledger boundary at the first tick, for the epilogue's readings.
    let (
        deposited_start,
        respired_start,
        maintenance_start,
        motor_start,
        digestion_start,
        eaten_start,
    ) = {
        let ledger = sim.fauna().view().ledger;
        (
            ledger.deposited_organic_out,
            ledger.respired_out,
            ledger.respired_maintenance_out,
            ledger.respired_motor_out,
            ledger.respired_digestion_out,
            ledger.eaten_organic_in,
        )
    };
    let mut forward_sum = 0.0f64;
    let mut turn_abs_sum = 0.0f64;
    let mut feed_sum = 0.0f64;

    for tick in 0..horizon {
        if tick.is_multiple_of(CANCEL_CHECK_TICKS) && limits.expired() {
            return Err(EpisodeError::Cancelled { ticks: tick });
        }

        // The pre-step finiteness gate: the placed animal's numbers are checked every
        // tick, in release too. (Observations and actions are the fauna's to reject;
        // its controller stage holds rest on a non-finite observation and the shared
        // adapter zeroes a non-finite logit.)
        if let Some(a) = sim.fauna().view().animal(id)
            && !(a.body.is_finite()
                && a.reserve.is_finite()
                && a.energy.is_finite()
                && a.mineral.is_finite()
                && a.pose.is_finite())
        {
            return invalid(tick, "the placed animal's state is non-finite".into());
        }

        sim.step();
        episode.ticks = tick + 1;

        if let Some((initial, successor, r)) = patches.as_mut() {
            let now_initial = patch_stock(&sim, *initial);
            let now_successor = patch_stock(&sim, *successor);
            if r.depleted_tick.is_none() && now_initial < r.depletion_fraction * r.initial_start {
                r.depleted_tick = Some(episode.ticks);
            }
            if r.successor_first_bite_tick.is_none() && now_successor < r.successor_start {
                r.successor_first_bite_tick = Some(episode.ticks);
            }
        }

        let view = sim.fauna().view();
        match view.animal(id) {
            Some(a) => {
                episode.pose_x = a.pose.x;
                episode.pose_z = a.pose.z;
                episode.heading_rad = a.pose.heading_rad;
                episode.end_energy = a.energy;
                // The body's own held actions, as the fauna resolved them.
                let held = a.founder_state.held;
                forward_sum += held.forward;
                turn_abs_sum += held.turn.abs();
                feed_sum += held.feed;
            }
            None => {
                // Dead: an ordinary completed episode. The corpse carried what the body
                // still held; the epilogue reads it off the ledger boundary below.
                episode.alive = false;
                break;
            }
        }
    }

    // Epilogue: the raw measurements the score's components come from. Read-only —
    // the fauna view and its ledger, never the arena's settlement APIs.
    //
    let view = sim.fauna().view();
    let ledger = view.ledger;
    let corpse = ledger.deposited_organic_out - deposited_start;
    episode.corpse_organic = corpse;
    let end_organic = view.animal(id).map_or(0.0, Animal::organic) + corpse;
    episode.end_organic = end_organic;
    episode.end_energy = view.animal(id).map_or(0.0, |a| a.energy);
    episode.respired_organic = ledger.respired_out - respired_start;
    episode.maintenance_organic = ledger.respired_maintenance_out - maintenance_start;
    episode.motor_organic = ledger.respired_motor_out - motor_start;
    episode.digestion_organic = ledger.respired_digestion_out - digestion_start;
    episode.eaten_organic = ledger.eaten_organic_in - eaten_start;
    debug_assert!(
        episode.respired_organic >= 0.0,
        "the layer cannot unrespire"
    );
    episode.intake_organic = settled_intake(
        episode.start_organic,
        end_organic,
        episode.maintenance_organic,
        episode.motor_organic,
    );
    if !(episode.intake_organic.abs() < 1e-9 || episode.intake_organic >= 0.0) {
        return invalid(
            episode.ticks,
            format!(
                "settled intake measured {:+e}: the epilogue's conservation reading broke",
                episode.intake_organic
            ),
        );
    }
    debug_assert!(
        (episode.intake_organic + episode.digestion_organic - episode.eaten_organic).abs() < 1e-6,
        "settled intake plus digestion must reconcile with gross eaten organic: \
         intake {:+e} + digestion {:+e} vs eaten {:+e}",
        episode.intake_organic,
        episode.digestion_organic,
        episode.eaten_organic
    );
    episode.score = ScoreComponents::of(
        episode.intake_organic,
        episode.motor_organic,
        episode.survived_fraction(),
        manifest.body_reference,
    );
    if let Some((initial, successor, mut r)) = patches {
        r.initial_end = patch_stock(&sim, initial);
        r.successor_end = patch_stock(&sim, successor);
        r.initial_taken = (r.initial_start - r.initial_end).max(0.0);
        r.successor_taken = (r.successor_start - r.successor_end).max(0.0);
        r.reacquired =
            Reacquisition::is_reacquisition(r.depleted_tick, r.successor_first_bite_tick);
        episode.reacquisition = Some(r);
    }
    let ticks = episode.ticks.max(1) as f64;
    episode.updates = episode.ticks / cadence;
    episode.mean_forward = forward_sum / ticks;
    episode.mean_turn_abs = turn_abs_sum / ticks;
    episode.mean_feed = feed_sum / ticks;
    Ok(episode)
}

/// Conservation reading for organic that assimilated into the body.
///
/// Digestive respiration is intentionally absent: it is gross bite organic that never
/// settled into tissue, and must not reward a policy.
fn settled_intake(start_organic: f64, end_organic: f64, maintenance: f64, motor: f64) -> f64 {
    (end_organic - start_organic) + maintenance + motor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::voxel::controller::VoxelControl;
    use crate::es::voxel::task::{Prepared, Stage, TRAINING_LAYOUT_SEEDS};

    fn gru(founder: Founder) -> EpisodeDriver {
        let theta = if founder == Founder::Blind {
            crate::es::tensor::initial_center_shape::<23, 3>(5)
        } else {
            crate::es::tensor::initial_center_shape::<37, 3>(5)
        };
        EpisodeDriver::gru(&theta, founder).expect("a valid centre")
    }

    /// The seam is live: the same driver on the same layout, once with actions attached
    /// and once without, must differ. The GRU centre cruises, and a cruising body pays
    /// motor respiration — visible in the terminal pose and split ledger.
    #[test]
    fn an_attached_controller_actually_moves_the_body() {
        let cancel = AtomicBool::new(false);
        let limits = Limits::new(&cancel);
        let prepared = Prepared::build(Founder::Blind, TRAINING_LAYOUT_SEEDS[0]);
        let gru = gru(Founder::Blind);
        let rest = EpisodeDriver::control(VoxelControl::NoIntake, Founder::Blind);
        let moving = run_prepared(&prepared, &gru, 120, limits, "gru").expect("ok");
        let still = run_prepared(&prepared, &rest, 120, limits, "rest").expect("ok");
        let start = prepared.fixture_arena().animal_pose().expect("placed");
        // The GRU centre holds real forward effort, so the body left its start pose.
        assert!(
            (moving.pose_x - start.x).abs() + (moving.pose_z - start.z).abs() > 1e-6,
            "the attached GRU must move the body: {:?} vs {start:?}",
            (moving.pose_x, moving.pose_z)
        );
        // Rest is rest: no action, no motion.
        assert!(
            (still.pose_x - start.x).abs() + (still.pose_z - start.z).abs() == 0.0,
            "the no-intake control holds rest and must not move"
        );
        assert!(
            moving.motor_organic > still.motor_organic,
            "motion is paid: {} > {}",
            moving.motor_organic,
            still.motor_organic
        );
        // The held-action statistics come from the body, not the driver.
        assert!(moving.mean_forward > 0.1, "{moving:?}");
        assert_eq!(still.mean_forward, 0.0);
        assert_eq!(still.mean_feed, 0.0);
    }

    /// Both founders run a full horizon with the GRU and the heuristic slot through the
    /// fauna's own controller stage, at the manifest cadence.
    #[test]
    fn an_episode_runs_the_controller_and_survives_the_horizon() {
        let cancel = AtomicBool::new(false);
        let limits = Limits::new(&cancel);
        for founder in Founder::ALL {
            let prepared = Prepared::build(founder, TRAINING_LAYOUT_SEEDS[0]);
            for driver in [
                gru(founder),
                EpisodeDriver::control(VoxelControl::Heuristic, founder),
                EpisodeDriver::control(VoxelControl::NoIntake, founder),
                EpisodeDriver::control(VoxelControl::StationaryFeeding, founder),
            ] {
                let e = run_prepared(&prepared, &driver, 120, limits, "t").expect("ok");
                assert!(e.alive && e.ticks == 120, "{e:?}");
                assert_eq!(e.founder, founder.name());
                assert_eq!(e.updates, 120 / founder.manifest().cadence_ticks());
                assert_eq!(e.counters, ScoreCounters::Landed);
                assert!(e.score.score.is_finite());
                assert!(
                    e.respired_organic > 0.0,
                    "a living body is billed for living"
                );
            }
        }
    }

    /// An expired limit is observed before the first tick, without relying on wall-clock
    /// calibration or a long rollout.
    #[test]
    fn an_expired_deadline_stops_an_episode_before_it_steps() {
        let cancel = AtomicBool::new(false);
        let prepared = Prepared::build(Founder::Blind, TRAINING_LAYOUT_SEEDS[0]);
        let gru = gru(Founder::Blind);
        let out = run_prepared(
            &prepared,
            &gru,
            120,
            Limits::until(&cancel, Instant::now()),
            "t",
        );
        let err = out.expect_err("the elapsed deadline must stop it");
        assert!(err.is_cancelled());
        assert_eq!(err.ticks(), 0);
        assert!(
            cancel.load(Ordering::Relaxed),
            "and it told the other workers"
        );
    }

    /// Fresh episodes reset controller memory (`reset()`) and all mutable arena data:
    /// the same driver on the same prepared layout produces the identical record, twice,
    /// each with a freshly attached controller and a fresh copy of the arena.
    #[test]
    fn two_fresh_episodes_of_the_same_driver_are_identical() {
        let cancel = AtomicBool::new(false);
        let limits = Limits::new(&cancel);
        let prepared = Prepared::build(Founder::Blind, TRAINING_LAYOUT_SEEDS[2]);
        let driver = gru(Founder::Blind);
        let a = run_prepared(&prepared, &driver, 120, limits, "a").expect("ok");
        let b = run_prepared(&prepared, &driver, 120, limits, "b").expect("ok");
        assert_eq!(a, b, "a fresh episode must not inherit anything");
    }

    /// The epilogue's conservation reading is exact under the current rules: for any
    /// non-feeding body — moving or not — has zero settled intake. The separate motor
    /// term cancels its own charge in the conservation reading.
    #[test]
    fn the_epilogue_reconciles_for_a_non_feeding_body() {
        let cancel = AtomicBool::new(false);
        let prepared = Prepared::build(Founder::Blind, TRAINING_LAYOUT_SEEDS[3]);
        // The no-intake control never feeds, whatever the horizon.
        let e = run_prepared(
            &prepared,
            &EpisodeDriver::control(VoxelControl::NoIntake, Founder::Blind),
            120,
            Limits::new(&cancel),
            "t",
        )
        .expect("ok");
        let organic_delta = e.end_organic - e.start_organic;
        assert!(
            ((organic_delta + e.maintenance_organic + e.motor_organic) - episode_intake(&e)).abs()
                < 1e-12,
            "settled intake = Δorganic + maintenance + motor"
        );
        assert!(
            (organic_delta + e.maintenance_organic).abs() < 1e-12,
            "the resting body pays only maintenance"
        );
        assert_eq!(
            e.eaten_organic, 0.0,
            "rest is rest: nothing crossed the mouth"
        );
        assert!((episode_intake(&e)).abs() < 1e-12);
    }

    /// Settled intake as the epilogue computes it.
    fn episode_intake(e: &Episode) -> f64 {
        settled_intake(
            e.start_organic,
            e.end_organic,
            e.maintenance_organic,
            e.motor_organic,
        )
    }

    #[test]
    fn digestive_respiration_cannot_raise_settled_intake() {
        let settled = settled_intake(1.0, 1.1, 0.02, 0.03);
        assert!((settled - 0.15).abs() < 1e-12);
        // A bite with another 0.07 respired during digestion was gross 0.22, but only
        // 0.15 crossed into the animal and therefore belongs in the score.
        assert!((settled + 0.07 - 0.22).abs() < 1e-12);
    }

    /// The reacquisition boolean is an ordering, not an "it ate twice": the successor
    /// must be bitten at or after the tick the first patch fell below its threshold.
    #[test]
    fn reacquisition_requires_the_first_patch_to_have_run_out_first() {
        assert!(Reacquisition::is_reacquisition(Some(400), Some(400)));
        assert!(Reacquisition::is_reacquisition(Some(400), Some(1_100)));
        assert!(
            !Reacquisition::is_reacquisition(Some(400), Some(120)),
            "a successor bitten before the first patch ran out is not a reacquisition"
        );
        assert!(!Reacquisition::is_reacquisition(Some(400), None));
        assert!(!Reacquisition::is_reacquisition(None, Some(400)));
        assert!(!Reacquisition::is_reacquisition(None, None));
    }

    /// Stage B's accounting exists on a Stage-B layout and nowhere else, reads both
    /// patches' real starting stocks, and stays at zero for a founder that never
    /// reaches food. 120 ticks is six seconds: far too short to deplete a patch, which
    /// is the point — the fields must be honest about that rather than defaulting to
    /// something flattering.
    #[test]
    fn stage_b_reports_both_patches_and_stage_a_reports_nothing() {
        let cancel = AtomicBool::new(false);
        let limits = Limits::new(&cancel);
        for founder in Founder::ALL {
            let a = Prepared::build_stage(founder, TRAINING_LAYOUT_SEEDS[0], Stage::A);
            let e = run_prepared(
                &a,
                &EpisodeDriver::control(VoxelControl::Heuristic, founder),
                60,
                limits,
                "a",
            )
            .expect("ok");
            assert!(
                e.reacquisition.is_none(),
                "Stage A has no patch pair to account for"
            );

            let b = Prepared::build_stage(founder, TRAINING_LAYOUT_SEEDS[0], Stage::B);
            let (initial, successor) = b.patches().expect("Stage B names its patches");
            assert_ne!(initial, successor);
            let e = run_prepared(
                &b,
                &EpisodeDriver::control(VoxelControl::StationaryFeeding, founder),
                120,
                limits,
                "b",
            )
            .expect("ok");
            let r = e.reacquisition.expect("Stage B accounts for its patches");
            assert!(
                r.initial_start > 0.0 && r.successor_start > 0.0,
                "{founder:?}: both patches start stocked: {r:?}"
            );
            // The founder starts off food, so a stationary feeder takes nothing at all.
            assert_eq!(r.initial_taken, 0.0, "{founder:?}: {r:?}");
            assert_eq!(r.successor_taken, 0.0, "{founder:?}: {r:?}");
            assert_eq!(r.depleted_tick, None);
            assert_eq!(r.successor_first_bite_tick, None);
            assert!(!r.reacquired);
            assert_eq!(r.depletion_fraction, DEPLETION_FRACTION);
            assert!(
                e.score.intake_normalized < 1e-12,
                "and it ate nothing: {}",
                e.score.intake_normalized
            );
        }
    }

    /// The digest the driver validates against is the placed founder's manifest digest:
    /// a policy authored against the other founder is refused before any episode runs,
    /// and a driver for the other founder is refused at the episode boundary.
    #[test]
    fn a_policy_for_the_other_founder_is_refused_by_name() {
        let browser_theta = crate::es::tensor::initial_center_shape::<37, 3>(3);
        let err = EpisodeDriver::gru(&browser_theta, Founder::Blind)
            .expect_err("a browser-shaped theta is not a blind policy");
        assert!(err.contains("6915"), "{err}");
        // A right-length theta carrying a non-finite weight is refused too.
        let mut corrupt = crate::es::tensor::initial_center_shape::<23, 3>(3);
        corrupt[0] = f64::NAN;
        let err = EpisodeDriver::gru(&corrupt, Founder::Blind).expect_err("non-finite");
        assert!(err.contains("finite"), "{err}");
        // And a valid driver for the other founder is refused at the episode itself.
        let browser = EpisodeDriver::gru(&browser_theta, Founder::Browser).expect("valid");
        let prepared = Prepared::build(Founder::Blind, TRAINING_LAYOUT_SEEDS[0]);
        let err = run_prepared(
            &prepared,
            &browser,
            10,
            Limits::new(&AtomicBool::new(false)),
            "t",
        )
        .expect_err("founder mismatch");
        assert!(err.to_string().contains("driver is for"), "{err}");
    }
}
