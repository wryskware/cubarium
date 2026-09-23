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

use rustc_hash::FxHashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use cubarium_voxel_fauna::{
    Animal, BlindForager, BrowserForager, Controller, Departure, FaunaConfig, Food, Founder,
    FounderPhysiology, Pose, Response, StartingStores,
};
use cubarium_voxel_sim::{ArenaGrid, Placement, Sim, SimConfig, Site, edible_stock, populate};
use serde::{Deserialize, Serialize};

use super::controller::EpisodeDriver;
use super::landscape::PreparedLandscape;
use super::score::ScoreComponents;
use super::task::{self, DEPLETION_FRACTION, Prepared, PreparedArena};

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
    /// Wrapped metres between the two patches' face centres. Pure fixture geometry,
    /// fixed when the layout was built.
    #[serde(default)]
    pub separation_m: f64,
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
    /// The founder's closest approach to the successor's face centre, in metres, over
    /// the ticks from the depletion tick onward. `None` when the initial patch never
    /// ran out — there is no "after depletion" to measure in.
    #[serde(default)]
    pub min_successor_distance_m: Option<f64>,
    /// Ticks from the depletion tick onward spent within the founder's sensed radius of
    /// the successor ([`super::task::sensed_radius_m`]): blind 1.5 m, browser 2.0 m.
    /// Zero when the founder never came that close, and when it never depleted.
    #[serde(default)]
    pub sense_ticks: u64,
    /// The radius those `sense_ticks` were counted against, so a row states its own
    /// threshold instead of the reader having to look it up.
    #[serde(default)]
    pub sensed_radius_m: f64,
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

    /// Fold one post-step reading of the body's distance to the successor into the
    /// departure counters. A no-op until the initial patch has been called empty:
    /// before that the founder has no reason to leave, so "how close did it get" would
    /// measure the layout rather than the policy. The depletion tick itself counts.
    pub fn observe_successor_distance(&mut self, tick: u64, distance_m: f64) {
        if !self.depleted_tick.is_some_and(|d| tick >= d) {
            return;
        }
        self.min_successor_distance_m = Some(
            self.min_successor_distance_m
                .map_or(distance_m, |m: f64| m.min(distance_m)),
        );
        if distance_m <= self.sensed_radius_m {
            self.sense_ticks += 1;
        }
    }
}

/// One acting body's own accounting over an episode (P5-B items 4–5).
///
/// Everything here is read off the body — its stores, its pose and the per-interval
/// feedback the fauna tick records on it — never off the layer's ledger, which since P5-B
/// also carries bystanders and, on a landscape, seven other acting bodies. Nothing here
/// enters an observation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BodyDiagnostics {
    pub id: u64,
    /// Structure at the first tick: the size D11 drew for this episode.
    pub start_body: f64,
    /// Organic matter (structure plus reserve) at the first tick.
    pub start_organic: f64,
    /// Organic matter at the last tick it was seen alive.
    pub end_organic: f64,
    pub alive: bool,
    /// Ticks it was alive for.
    pub ticks_alive: u64,
    /// How many times the fauna sampled the candidate for this body.
    pub samples: u64,
    /// Settled assimilated intake: the organic its bites placed into body and reserve.
    pub intake_organic: f64,
    /// What its paid motion respired.
    pub motor_organic: f64,
    /// Basal upkeep, by conservation: `intake − motor − Δorganic`.
    pub maintenance_organic: f64,
    /// Metres actually covered along the heading.
    pub walked_m: f64,
    /// Distinct columns its pose stood over, times the cell area.
    pub unique_area_m2: f64,
    /// The motor cost of the displacement it asked for and was refused: per tick,
    /// `motor · (attempted − delivered) / attempted`.
    pub blocked_motor_organic: f64,
    /// Ticks spent within one body length of a drop or the strip's edge
    /// ([`near_drop_or_edge`]).
    pub near_drop_or_edge_ticks: u64,
    /// Why it left the world, when it did: the ledger's own cause (`starved`,
    /// `drowned`, `removed`).
    pub death: Option<String>,
}

/// The per-episode diagnostics (P5-B item 5), over the acting bodies. Never part of the
/// ordering; the score is unchanged (D9).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Diagnostics {
    /// Gross organic the acting lineage ate, by food class in `Food::ALL` order (litter,
    /// cap tissue, carrion, foliage). Read off the ledger's class split: the other
    /// lineage never eats the acting one's classes, so the split is the lineage's own.
    pub intake_by_food: [f64; Food::COUNT],
    /// Mean metres walked per acting body.
    pub walked_m: f64,
    /// Mean distinct area covered per acting body, m².
    pub unique_area_m2: f64,
    /// The share of the acting bodies' motor cost that paid for refused displacement.
    pub blocked_motor_share: f64,
    /// The share of acting body-ticks spent within one body length of a drop or edge.
    pub near_drop_or_edge_share: f64,
    /// The acting bodies' departures by cause, in `Departure::ALL` order.
    pub deaths_by_cause: [u64; Departure::COUNT],
}

/// The outcome of one episode. Every field is measured on every episode.
///
/// On a **landscape** the raw measurements below are the **means over the acting
/// bodies**, so the score is their mean score (D7); on an arena there is one acting body
/// and they are its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub founder: String,
    /// The arena's layout seed, or the landscape's seed base.
    pub layout_seed: u64,
    /// `arena` or `landscape`.
    #[serde(default)]
    pub kind: String,
    /// The fixture's label: the layout seed (`f` for the 0.125 m grid) or
    /// `preset/base/water`.
    #[serde(default)]
    pub fixture: String,
    /// The seed the episode drew its body sizes from (D11).
    #[serde(default)]
    pub episode_seed: u64,
    /// Which driver ran: `gru`, or one of the disclosed control names.
    pub driver: String,
    pub horizon: u64,
    /// Ticks actually simulated: the horizon, or the tick the last acting body was found
    /// gone. Ordinary death is a completed episode, not an error.
    pub ticks: u64,
    /// Every acting body survived the ticks simulated.
    pub alive: bool,
    /// The score and its separately reported components.
    pub score: ScoreComponents,
    /// Which counter state the components were measured in (see [`ScoreCounters`]).
    pub counters: ScoreCounters,
    // --- raw measurements behind the components ---
    /// Organic matter at the first tick.
    pub start_organic: f64,
    /// Organic matter at the last tick seen alive.
    pub end_organic: f64,
    /// The corpse deposits of the acting lineage over the episode.
    pub corpse_organic: f64,
    /// Settled assimilated intake: organic that became tissue before maintenance and
    /// motor charges, never the gross bite withdrawal.
    pub intake_organic: f64,
    /// Organic matter the founder's motion charged.
    pub motor_organic: f64,
    /// Basal upkeep respiration over the episode.
    pub maintenance_organic: f64,
    /// Digestion respiration over the episode: the gross share of the lineage's bites
    /// that did not settle (`eaten − intake`).
    pub digestion_organic: f64,
    /// Maintenance, motor and digestion together.
    pub respired_organic: f64,
    /// The gross organic that crossed the acting lineage's mouths through real `Taken`
    /// withdrawals: the ledger's class split over the lineage's food classes.
    pub eaten_organic: f64,
    // --- diagnostics, never part of the ordering ---
    pub start_energy: f64,
    pub end_energy: f64,
    /// Controller samplings the episode's cadence implies: `ticks / cadence_ticks`.
    pub updates: u64,
    /// Time-average of the held actions over the ticks each body was alive.
    pub mean_forward: f64,
    pub mean_turn_abs: f64,
    pub mean_feed: f64,
    /// The first acting body's terminal pose, metres and radians, read-only.
    pub pose_x: f64,
    pub pose_z: f64,
    pub heading_rad: f64,
    /// Stage B's patch accounting; `None` on a Stage-A layout and on a landscape.
    #[serde(default)]
    pub reacquisition: Option<Reacquisition>,
    /// Each acting body's own accounting.
    #[serde(default)]
    pub bodies: Vec<BodyDiagnostics>,
    /// P5-B's per-episode diagnostics.
    #[serde(default)]
    pub diagnostics: Diagnostics,
}

impl Episode {
    /// The survival term's fraction: the acting bodies' mean `ticks_alive / horizon`.
    pub fn survived_fraction(&self) -> f64 {
        let ticks = if self.bodies.is_empty() {
            self.ticks as f64
        } else {
            self.bodies
                .iter()
                .map(|b| b.ticks_alive as f64)
                .sum::<f64>()
                / self.bodies.len() as f64
        };
        (ticks / self.horizon as f64).clamp(0.0, 1.0)
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
    run_prepared(&prepared, driver, horizon, limits, job)
}

/// Run one episode from a fixture's private mutable copy, drawing its body sizes from
/// the fixture's own seed ([`run_prepared_seeded`]).
pub fn run_prepared(
    prepared: &Prepared,
    driver: &EpisodeDriver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
) -> Result<Episode, EpisodeError> {
    run_prepared_seeded(
        prepared,
        driver,
        horizon,
        limits,
        job,
        prepared.layout_seed(),
    )
}

/// Run one episode, drawing each acting body's size from `episode_seed` (D11): uniform
/// in structure between the lineage's newborn `body_min` and its adult `body_max`, with
/// no reserve. The same seed on the same fixture is the same episode; the trainer gives
/// every candidate of one generation the same seed, so both signs of a pair still run
/// identical worlds.
pub fn run_prepared_seeded(
    prepared: &Prepared,
    driver: &EpisodeDriver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
    episode_seed: u64,
) -> Result<Episode, EpisodeError> {
    match prepared {
        Prepared::Arena(arena) => arena_episode(arena, driver, horizon, limits, job, episode_seed),
        Prepared::Landscape(land) => {
            landscape_episode(land, driver, horizon, limits, job, episode_seed).map(|(e, _)| e)
        }
    }
}

/// A landscape episode that also hands back its simulator, for a caller that has to
/// read the world it ended in (the frozen-water test, a diagnostic).
pub fn run_landscape_sim(
    prepared: &PreparedLandscape,
    driver: &EpisodeDriver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
    episode_seed: u64,
) -> Result<(Episode, Sim), EpisodeError> {
    landscape_episode(prepared, driver, horizon, limits, job, episode_seed)
}

/// The stores the `k`-th acting body of a fixture arrives with in an episode (D11).
pub fn sampled_stores(
    founder: Founder,
    episode_seed: u64,
    fixture_seed: u64,
    k: usize,
) -> StartingStores {
    let sc = FaunaConfig::default().founder(founder).core;
    let newborn = (sc.body_min / sc.body_max).clamp(0.0, 1.0);
    let u = unit(
        episode_seed
            ^ fixture_seed.rotate_left(29)
            ^ (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ BODY_SALT,
    );
    StartingStores {
        body: newborn + u * (1.0 - newborn),
        reserve: 0.0,
    }
}

/// The body-size draw's own salt.
const BODY_SALT: u64 = 0x_D11B_0D1E_5A3F_1E00;

/// A uniform `[0, 1)` from one splitmix64 step of `seed`.
fn unit(seed: u64) -> f64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// The candidate's controller, counting its samplings for [`BodyDiagnostics::samples`].
struct Counted {
    inner: Box<dyn Controller>,
    count: Arc<AtomicU64>,
}

impl Controller for Counted {
    fn drive(&mut self, observation: &[f64]) -> Response {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.inner.drive(observation)
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}

/// A bystander's own observation-only heuristic.
fn heuristic(founder: Founder) -> Box<dyn Controller> {
    match founder {
        Founder::Blind => Box::new(BlindForager::new()),
        Founder::Browser => Box::new(BrowserForager::new()),
    }
}

/// An arena episode: the fixture's private copy, its founder refounded at the sampled
/// size, the candidate attached to it and the bystanders on their heuristics.
fn arena_episode(
    prepared: &PreparedArena,
    driver: &EpisodeDriver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
    episode_seed: u64,
) -> Result<Episode, EpisodeError> {
    let founder = prepared.founder;
    check_founder(driver, founder, job)?;
    let mut arena = prepared.episode_arena();
    arena.refound(sampled_stores(
        founder,
        episode_seed,
        prepared.layout_seed,
        0,
    ));
    let Some(id) = arena.animal_id else {
        return invalid(job, 0, "the arena placed no founder body".into());
    };
    let bystanders: Vec<(u64, Founder)> = arena
        .bystander_ids
        .iter()
        .zip(&arena.bystanders)
        .map(|(&id, p)| (id, p.founder))
        .collect();
    let grid = prepared.grid;
    let mut sim = arena.into_sim_prepared(SimConfig { threads: 1 }, prepared.episode_senses());
    let stage_b = prepared.patches().map(|(initial, successor)| StageB {
        founder,
        grid,
        initial,
        successor,
        r: Reacquisition {
            separation_m: task::site_separation_m(initial, successor, grid),
            initial_start: patch_stock(founder, &sim, grid, initial),
            successor_start: patch_stock(founder, &sim, grid, successor),
            depletion_fraction: DEPLETION_FRACTION,
            sensed_radius_m: task::sensed_radius_m(founder),
            ..Reacquisition::default()
        },
    });
    let mut episode = drive(
        &mut sim,
        driver,
        &[id],
        &bystanders,
        horizon,
        limits,
        job,
        stage_b,
    )?;
    episode.layout_seed = prepared.layout_seed;
    episode.kind = "arena".into();
    episode.fixture = Prepared::Arena(prepared.clone()).label();
    episode.episode_seed = episode_seed;
    Ok(episode)
}

/// A landscape episode: the frozen world and plants cloned into a static simulator, a
/// fresh animal layer with every placed founder — the acting lineage at sampled sizes
/// running the candidate, the other on its heuristics — births off.
fn landscape_episode(
    prepared: &PreparedLandscape,
    driver: &EpisodeDriver,
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
    episode_seed: u64,
) -> Result<(Episode, Sim), EpisodeError> {
    let founder = prepared.founder;
    check_founder(driver, founder, job)?;
    let acting: Vec<Placement> = prepared.acting().copied().collect();
    let others: Vec<Placement> = prepared.bystanders().copied().collect();
    if acting.is_empty() {
        return invalid(
            job,
            0,
            "the landscape placed no founder of this lineage".into(),
        );
    }
    let bodies = acting
        .iter()
        .enumerate()
        .map(|(k, p)| {
            (
                *p,
                sampled_stores(founder, episode_seed, prepared.seed_base, k),
            )
        })
        .chain(others.iter().map(|p| (*p, StartingStores::HUNGRY)));
    let world = prepared.world().clone();
    let (fauna, ids) = populate(&world, FaunaConfig::default(), bodies);
    let (acting_ids, other_ids) = ids.split_at(acting.len());
    let Some(acting_ids) = acting_ids.iter().copied().collect::<Option<Vec<u64>>>() else {
        return invalid(job, 0, "the fauna refused an acting founder".into());
    };
    let bystanders: Vec<(u64, Founder)> = other_ids
        .iter()
        .zip(&others)
        .filter_map(|(id, p)| id.map(|id| (id, p.founder)))
        .collect();
    let mut sim = Sim::new_static_prepared(
        world,
        prepared.flora().clone(),
        fauna,
        SimConfig { threads: 1 },
        prepared.episode_senses(),
    );
    let mut episode = drive(
        &mut sim,
        driver,
        &acting_ids,
        &bystanders,
        horizon,
        limits,
        job,
        None,
    )?;
    episode.layout_seed = prepared.seed_base;
    episode.kind = "landscape".into();
    episode.fixture = prepared.label();
    episode.episode_seed = episode_seed;
    Ok((episode, sim))
}

fn invalid<T>(job: &str, ticks: u64, detail: String) -> Result<T, EpisodeError> {
    Err(EpisodeError::Invalid {
        ticks,
        detail: format!("job {job}: {detail}"),
    })
}

fn check_founder(driver: &EpisodeDriver, founder: Founder, job: &str) -> Result<(), EpisodeError> {
    if driver.founder() != founder {
        return invalid(
            job,
            0,
            format!(
                "driver is for {} but the fixture places {}",
                driver.founder().name(),
                founder.name()
            ),
        );
    }
    Ok(())
}

/// Stage B's evaluator-side accounting on an arena. The two sites are the fixture's,
/// read here and nowhere the controller can reach.
struct StageB {
    founder: Founder,
    grid: ArenaGrid,
    initial: Site,
    successor: Site,
    r: Reacquisition,
}

/// Edible stock on an arena site in a running sim: [`cubarium_voxel_sim::edible_stock`],
/// the arena's own reading.
fn patch_stock(founder: Founder, sim: &Sim, grid: ArenaGrid, site: Site) -> f64 {
    edible_stock(founder, sim.flora(), grid.voxel_m(), grid.ground_y(), site)
}

/// One acting body, followed over the episode.
struct Tracked {
    id: u64,
    cadence: u64,
    phys: FounderPhysiology,
    samples: Arc<AtomicU64>,
    start_body: f64,
    start_organic: f64,
    start_energy: f64,
    last_organic: f64,
    last_energy: f64,
    last_pose: Pose,
    /// The feedback accumulator as it stood after the previous tick: intake, motor,
    /// attempted, delivered, forward.
    prev: [f64; 5],
    intake: f64,
    motor: f64,
    walked: f64,
    blocked_motor: f64,
    visited: FxHashSet<(i64, u32)>,
    near_ticks: u64,
    ticks_alive: u64,
    alive: bool,
    death: Option<String>,
    forward_sum: f64,
    turn_abs_sum: f64,
    feed_sum: f64,
}

impl Tracked {
    /// Fold one post-step reading of the living body in.
    ///
    /// The feedback accumulator is zeroed at a controller sampling, which the fauna makes
    /// on exactly the ticks where `age_ticks` is a multiple of the cadence, **before**
    /// that tick's motion and bite are added. So after such a tick it holds that tick
    /// alone, and otherwise it holds the previous reading plus this tick.
    fn observe(&mut self, a: &Animal, view: &cubarium_voxel::VoxelView<'_>) {
        let fb = a.founder_state.feedback;
        let now = [
            fb.intake,
            fb.motor_respiration,
            fb.attempted_equivalent,
            fb.delivered_equivalent,
            fb.delivered_forward,
        ];
        let sampled = a.age_ticks.is_multiple_of(self.cadence);
        let d: [f64; 5] = std::array::from_fn(|i| {
            if sampled {
                now[i]
            } else {
                now[i] - self.prev[i]
            }
        });
        self.prev = now;
        self.intake += d[0];
        self.motor += d[1];
        self.walked += d[4];
        if d[2] > 0.0 {
            self.blocked_motor += d[1] * ((d[2] - d[3]) / d[2]).clamp(0.0, 1.0);
        }
        let v = view.config.voxel_m;
        if let Some((cx, cz)) = a.pose.column(v, view.config.depth) {
            self.visited
                .insert((cx.rem_euclid(i64::from(view.config.width)), cz));
        }
        let body = self.phys.body_at(a.body);
        if near_drop_or_edge(view, &a.pose, a.site.y, body.length_m, &self.phys) {
            self.near_ticks += 1;
        }
        let held = a.founder_state.held;
        self.forward_sum += held.forward;
        self.turn_abs_sum += held.turn.abs();
        self.feed_sum += held.feed;
        self.last_organic = a.organic();
        self.last_energy = a.energy;
        self.last_pose = a.pose;
        self.ticks_alive += 1;
    }

    fn diagnostics(&self, cell_area: f64) -> BodyDiagnostics {
        BodyDiagnostics {
            id: self.id,
            start_body: self.start_body,
            start_organic: self.start_organic,
            end_organic: self.last_organic,
            alive: self.alive,
            ticks_alive: self.ticks_alive,
            samples: self.samples.load(Ordering::Relaxed),
            intake_organic: self.intake,
            motor_organic: self.motor,
            maintenance_organic: (self.intake
                - self.motor
                - (self.last_organic - self.start_organic))
                .max(0.0),
            walked_m: self.walked,
            unique_area_m2: self.visited.len() as f64 * cell_area,
            blocked_motor_organic: self.blocked_motor,
            near_drop_or_edge_ticks: self.near_ticks,
            death: self.death.clone(),
        }
    }
}

/// Whether a body standing at `pose` on layer `standing_y` is within `reach_m` of the
/// strip's `z` edge, or of a **drop**: a column whose centre is within `reach_m` where
/// the cell at head height is open (not a wall) and there is no support face within the
/// lineage's climb of the body's own layer. A wall is not a drop; water is not a drop.
pub fn near_drop_or_edge(
    view: &cubarium_voxel::VoxelView<'_>,
    pose: &Pose,
    standing_y: u32,
    reach_m: f64,
    phys: &FounderPhysiology,
) -> bool {
    let c = view.config;
    let v = c.voxel_m;
    let depth_m = f64::from(c.depth) * v;
    if pose.z < reach_m || pose.z > depth_m - reach_m {
        return true;
    }
    let climb = i64::from(cubarium_voxel_fauna::climb_voxels(phys, v));
    let span = (reach_m / v).ceil() as i64 + 1;
    let (px, pz) = ((pose.x / v).floor() as i64, (pose.z / v).floor() as i64);
    let y = i64::from(standing_y);
    for dz in -span..=span {
        let z = pz + dz;
        if z < 0 || z >= i64::from(c.depth) {
            continue;
        }
        for dx in -span..=span {
            let x = px + dx;
            let (cx, cz) = ((x as f64 + 0.5) * v, (z as f64 + 0.5) * v);
            if (cx - pose.x).hypot(cz - pose.z) > reach_m {
                continue;
            }
            let head = y + 1;
            if head >= i64::from(c.height) || view.material_at(x, head as u32, z as u32).is_solid()
            {
                continue;
            }
            let supported = (y - climb..=y + climb)
                .filter(|&yy| yy >= 0 && yy < i64::from(c.height))
                .any(|yy| view.is_support(x, yy as u32, z as u32));
            if !supported {
                return true;
            }
        }
    }
    false
}

/// The episode loop both fixtures share: attach the candidate to every acting body and
/// the heuristics to the bystanders, step the static schedule to the horizon or until
/// every acting body is gone, and read the result off the bodies.
#[allow(clippy::too_many_arguments)]
fn drive(
    sim: &mut Sim,
    driver: &EpisodeDriver,
    acting: &[u64],
    bystanders: &[(u64, Founder)],
    horizon: u64,
    limits: Limits<'_>,
    job: &str,
    mut stage_b: Option<StageB>,
) -> Result<Episode, EpisodeError> {
    let founder = driver.founder();
    let manifest = founder.manifest();
    let cadence = manifest.cadence_ticks().max(1);
    let phys = *FaunaConfig::default().founder(founder);

    let mut tracked: Vec<Tracked> = Vec::with_capacity(acting.len());
    for &id in acting {
        let samples = Arc::new(AtomicU64::new(0));
        // Fresh memory by construction, `reset()` by contract. The fauna table clones
        // empty, so this is the only mind the body gets.
        let controller = Counted {
            inner: driver.fresh(),
            count: Arc::clone(&samples),
        };
        if !sim.fauna_mut().set_controller(id, Box::new(controller)) {
            return invalid(job, 0, "the fauna refused the episode's controller".into());
        }
        let Some(a) = sim.fauna().view().animal(id).copied() else {
            return invalid(job, 0, "an acting animal is already gone".into());
        };
        tracked.push(Tracked {
            id,
            cadence,
            phys,
            samples,
            start_body: a.body,
            start_organic: a.organic(),
            start_energy: a.energy,
            last_organic: a.organic(),
            last_energy: a.energy,
            last_pose: a.pose,
            prev: [0.0; 5],
            intake: 0.0,
            motor: 0.0,
            walked: 0.0,
            blocked_motor: 0.0,
            visited: FxHashSet::default(),
            near_ticks: 0,
            ticks_alive: 0,
            alive: true,
            death: None,
            forward_sum: 0.0,
            turn_abs_sum: 0.0,
            feed_sum: 0.0,
        });
    }
    for &(id, other) in bystanders {
        // A bystander the layer lost is no error: it was never the candidate.
        let _ = sim.fauna_mut().set_controller(id, heuristic(other));
    }

    // The acting lineage's food classes (decisions §3).
    let classes: Vec<usize> = match founder {
        Founder::Blind => vec![Food::Litter, Food::CapTissue, Food::Carrion],
        Founder::Browser => vec![Food::Foliage],
    }
    .into_iter()
    .map(Food::index)
    .collect();
    let (eaten_start, deaths_start) = {
        let l = sim.fauna().view().ledger;
        (l.eaten_by_food, l.deaths_by_founder_cause[founder.index()])
    };

    let mut ticks = 0u64;
    for tick in 0..horizon {
        if tick.is_multiple_of(CANCEL_CHECK_TICKS) && limits.expired() {
            return Err(EpisodeError::Cancelled { ticks: tick });
        }
        // The pre-step finiteness gate, every acting body, every tick, in release too.
        {
            let view = sim.fauna().view();
            for t in tracked.iter().filter(|t| t.alive) {
                if let Some(a) = view.animal(t.id)
                    && !(a.body.is_finite()
                        && a.reserve.is_finite()
                        && a.energy.is_finite()
                        && a.mineral.is_finite()
                        && a.pose.is_finite())
                {
                    return invalid(job, tick, format!("animal {} is non-finite", t.id));
                }
            }
        }
        let deaths_before = sim.fauna().view().ledger.deaths_by_founder_cause[founder.index()];

        sim.step();
        ticks = tick + 1;

        if let Some(b) = stage_b.as_mut() {
            let now_initial = patch_stock(b.founder, sim, b.grid, b.initial);
            let now_successor = patch_stock(b.founder, sim, b.grid, b.successor);
            if b.r.depleted_tick.is_none()
                && now_initial < b.r.depletion_fraction * b.r.initial_start
            {
                b.r.depleted_tick = Some(ticks);
            }
            if b.r.successor_first_bite_tick.is_none() && now_successor < b.r.successor_start {
                b.r.successor_first_bite_tick = Some(ticks);
            }
        }

        let (world, _, fauna) = sim.layers();
        let view = world.view();
        let fv = fauna.view();
        let mut gone: Vec<usize> = Vec::new();
        for (k, t) in tracked.iter_mut().enumerate().filter(|(_, t)| t.alive) {
            match fv.animal(t.id) {
                Some(a) => {
                    t.observe(a, &view);
                    // Post-depletion approach to the successor, read after the step and
                    // never summed into the score.
                    if k == 0
                        && let Some(b) = stage_b.as_mut()
                    {
                        let d = task::distance_to_site_m(a.pose.x, a.pose.z, b.successor, b.grid);
                        b.r.observe_successor_distance(ticks, d);
                    }
                }
                None => {
                    t.alive = false;
                    gone.push(k);
                }
            }
        }
        if !gone.is_empty() {
            // The ledger's own cause for each body that left this tick, in cause order.
            let after = fv.ledger.deaths_by_founder_cause[founder.index()];
            let mut causes: Vec<Departure> = Departure::ALL
                .iter()
                .flat_map(|d| {
                    std::iter::repeat_n(*d, (after[d.index()] - deaths_before[d.index()]) as usize)
                })
                .collect();
            causes.reverse();
            for k in gone {
                tracked[k].death = Some(
                    causes
                        .pop()
                        .map_or("gone".to_string(), |d| d.name().to_string()),
                );
            }
        }
        if tracked.iter().all(|t| !t.alive) {
            break;
        }
    }

    // Epilogue: every number off the bodies, plus the lineage's class split.
    let view = sim.fauna().view();
    let ledger = view.ledger;
    let n = tracked.len() as f64;
    let cell_area = sim.world().config().cell_area();
    let bodies: Vec<BodyDiagnostics> = tracked.iter().map(|t| t.diagnostics(cell_area)).collect();
    for b in &bodies {
        if !(b.intake_organic.abs() < 1e-9 || b.intake_organic >= 0.0) {
            return invalid(
                job,
                ticks,
                format!(
                    "body {} settled intake measured {:+e}: the feedback reading broke",
                    b.id, b.intake_organic
                ),
            );
        }
    }
    let mean = |f: &dyn Fn(&BodyDiagnostics) -> f64| bodies.iter().map(f).sum::<f64>() / n;
    let intake = mean(&|b| b.intake_organic.max(0.0));
    let motor = mean(&|b| b.motor_organic);
    let maintenance = mean(&|b| b.maintenance_organic);
    let mut intake_by_food = [0.0; Food::COUNT];
    for &i in &classes {
        intake_by_food[i] = ledger.eaten_by_food[i] - eaten_start[i];
    }
    let eaten = intake_by_food.iter().sum::<f64>() / n;
    let deaths_end = ledger.deaths_by_founder_cause[founder.index()];
    let mut deaths_by_cause = [0u64; Departure::COUNT];
    for d in Departure::ALL {
        deaths_by_cause[d.index()] = deaths_end[d.index()] - deaths_start[d.index()];
    }
    let body_ticks: u64 = tracked.iter().map(|t| t.ticks_alive).sum();
    let motor_total: f64 = bodies.iter().map(|b| b.motor_organic).sum();
    let blocked_total: f64 = bodies.iter().map(|b| b.blocked_motor_organic).sum();
    let near_total: u64 = bodies.iter().map(|b| b.near_drop_or_edge_ticks).sum();
    let per_tick = |f: &dyn Fn(&Tracked) -> f64| {
        tracked
            .iter()
            .map(|t| f(t) / t.ticks_alive.max(1) as f64)
            .sum::<f64>()
            / n
    };
    let first = &tracked[0];
    let digestion = (eaten - intake).max(0.0);
    let mut episode = Episode {
        founder: founder.name().to_string(),
        layout_seed: 0,
        kind: String::new(),
        fixture: String::new(),
        episode_seed: 0,
        driver: driver.name(),
        horizon,
        ticks,
        alive: tracked.iter().all(|t| t.alive),
        score: ScoreComponents::default(),
        counters: ScoreCounters::Landed,
        start_organic: mean(&|b| b.start_organic),
        end_organic: mean(&|b| b.end_organic),
        corpse_organic: tracked
            .iter()
            .filter(|t| !t.alive)
            .map(|t| t.last_organic)
            .sum::<f64>()
            / n,
        intake_organic: intake,
        motor_organic: motor,
        maintenance_organic: maintenance,
        digestion_organic: digestion,
        respired_organic: maintenance + motor + digestion,
        eaten_organic: eaten,
        start_energy: tracked.iter().map(|t| t.start_energy).sum::<f64>() / n,
        end_energy: tracked.iter().map(|t| t.last_energy).sum::<f64>() / n,
        updates: ticks / cadence,
        mean_forward: per_tick(&|t| t.forward_sum),
        mean_turn_abs: per_tick(&|t| t.turn_abs_sum),
        mean_feed: per_tick(&|t| t.feed_sum),
        pose_x: first.last_pose.x,
        pose_z: first.last_pose.z,
        heading_rad: first.last_pose.heading_rad,
        reacquisition: None,
        diagnostics: Diagnostics {
            intake_by_food,
            walked_m: mean(&|b| b.walked_m),
            unique_area_m2: mean(&|b| b.unique_area_m2),
            blocked_motor_share: if motor_total > 0.0 {
                blocked_total / motor_total
            } else {
                0.0
            },
            near_drop_or_edge_share: near_total as f64 / body_ticks.max(1) as f64,
            deaths_by_cause,
        },
        bodies,
    };
    episode.score = ScoreComponents::of(
        episode.intake_organic,
        episode.motor_organic,
        episode.survived_fraction(),
        manifest.body_reference,
    );
    if let Some(mut b) = stage_b {
        b.r.initial_end = patch_stock(b.founder, sim, b.grid, b.initial);
        b.r.successor_end = patch_stock(b.founder, sim, b.grid, b.successor);
        b.r.initial_taken = (b.r.initial_start - b.r.initial_end).max(0.0);
        b.r.successor_taken = (b.r.successor_start - b.r.successor_end).max(0.0);
        b.r.reacquired =
            Reacquisition::is_reacquisition(b.r.depleted_tick, b.r.successor_first_bite_tick);
        episode.reacquisition = Some(b.r);
    }
    Ok(episode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::voxel::controller::VoxelControl;
    use crate::es::voxel::task::{Prepared, PreparedArena, Stage, TRAINING_LAYOUT_SEEDS};

    fn gru(founder: Founder) -> EpisodeDriver {
        let theta = if founder == Founder::Blind {
            crate::es::tensor::initial_center_shape::<23, 3>(5)
        } else {
            crate::es::tensor::initial_center_shape::<37, 3>(5)
        };
        EpisodeDriver::gru(&theta, founder).expect("a valid centre")
    }

    /// The per-body intake reading is the ledger's own. An arena's only body of the
    /// candidate's lineage is the founder (the bystanders are the other lineage), so the
    /// intake the driver folds out of the body's interval feedback — zeroed at every
    /// sampling — must equal the ledger's per-lineage assimilation over the episode.
    #[test]
    fn the_per_body_intake_is_the_ledgers_lineage_assimilation() {
        let cancel = AtomicBool::new(false);
        for founder in Founder::ALL {
            for seed in TRAINING_LAYOUT_SEEDS.into_iter().take(3) {
                let prepared = PreparedArena::build_stage(founder, seed, Stage::A);
                let mut arena = prepared.episode_arena();
                arena.refound(sampled_stores(founder, seed, seed, 0));
                let id = arena.animal_id.expect("placed");
                let mut sim =
                    arena.into_sim_prepared(SimConfig { threads: 1 }, prepared.episode_senses());
                let before = sim.fauna().view().ledger.assimilated_by_founder[founder.index()];
                drop(sim.fauna_mut().take_controller(id));
                let teacher = EpisodeDriver::control(VoxelControl::Heuristic, founder);
                let e = drive(
                    &mut sim,
                    &teacher,
                    &[id],
                    &[],
                    200,
                    Limits::new(&cancel),
                    "t",
                    None,
                )
                .expect("ok");
                let after = sim.fauna().view().ledger.assimilated_by_founder[founder.index()];
                assert!(
                    (e.bodies[0].intake_organic - (after - before)).abs() < 1e-12,
                    "{founder:?} seed {seed}: body reading {} vs ledger {}",
                    e.bodies[0].intake_organic,
                    after - before
                );
                assert_eq!(e.intake_organic, e.bodies[0].intake_organic);
            }
        }
    }

    /// An arena episode draws its founder's size from the episode seed (D11), inside
    /// `[body_min, body_max]`, and the bystanders are there and not scored.
    #[test]
    fn an_arena_episode_samples_the_body_and_scores_only_the_founder() {
        let cancel = AtomicBool::new(false);
        for founder in Founder::ALL {
            let prepared = Prepared::build(founder, TRAINING_LAYOUT_SEEDS[0]);
            let rest = EpisodeDriver::control(VoxelControl::NoIntake, founder);
            let sc = FaunaConfig::default().founder(founder).core;
            let mut sizes = Vec::new();
            for seed in 0..6u64 {
                let e = run_prepared_seeded(&prepared, &rest, 20, Limits::new(&cancel), "t", seed)
                    .expect("ok");
                assert_eq!(e.bodies.len(), 1, "one acting body in an arena");
                let b = e.bodies[0].start_body;
                assert!((sc.body_min..=sc.body_max).contains(&b), "{founder:?}: {b}");
                sizes.push(b);
            }
            sizes.dedup();
            assert!(
                sizes.len() > 1,
                "{founder:?}: the size is drawn per episode"
            );
        }
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
            ((organic_delta + e.maintenance_organic + e.motor_organic) - e.intake_organic).abs()
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
        assert!(e.intake_organic.abs() < 1e-12);
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
            // The departure accounting (P3-B step 1). The separation is the layout's own
            // wrapped geometry, so it equals the fixture arithmetic exactly and is at
            // least the landed band's 8 columns (2 m). Nothing depleted, so there is no
            // "after depletion" to measure in and the two counters say so.
            assert_eq!(
                r.separation_m,
                task::site_separation_m(initial, successor, ArenaGrid::Standard)
            );
            assert!(
                r.separation_m >= 2.0 - 1e-12,
                "{founder:?}: landed separation {} m",
                r.separation_m
            );
            assert_eq!(r.min_successor_distance_m, None);
            assert_eq!(r.sense_ticks, 0);
            assert_eq!(r.sensed_radius_m, task::sensed_radius_m(founder));
        }
    }

    /// The departure counters are arithmetic on the post-step distance, and they are
    /// silent until the initial patch has been called empty: before that the founder
    /// has no reason to leave and "closest approach" would measure the layout.
    #[test]
    fn the_departure_counters_start_at_the_depletion_tick() {
        let mut r = Reacquisition {
            sensed_radius_m: 1.5,
            ..Reacquisition::default()
        };
        // Before depletion: walking right past the successor is not counted.
        for tick in 1..=10 {
            r.observe_successor_distance(tick, 0.1);
        }
        assert_eq!(r.min_successor_distance_m, None);
        assert_eq!(r.sense_ticks, 0);

        r.depleted_tick = Some(11);
        // The depletion tick itself counts, and only readings at or inside 1.5 m do.
        for (tick, d) in [(11, 3.0), (12, 1.5), (13, 1.5001), (14, 0.75), (15, 2.0)] {
            r.observe_successor_distance(tick, d);
        }
        assert_eq!(r.min_successor_distance_m, Some(0.75));
        assert_eq!(r.sense_ticks, 2, "1.5 m is inside, 1.5001 m is not");
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
