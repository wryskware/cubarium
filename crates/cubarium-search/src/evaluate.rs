//! One candidate, one seed, one real `cubarium-core` world, run headless to a hard horizon.
//!
//! Nothing here is a simplified ecology: the world is built by [`cubarium_core::World::new`]
//! and advanced by [`cubarium_core::World::step`], with ordinary founders that move, feed and
//! reproduce, the ordinary initial resource inventories, and the real conservation audits.
//! The only staging is the apex cohort, which is an explicit, accounted input placed once at
//! the start and never restocked.

use cubarium_surface::{Scale, Topology};
use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;
use std::time::Instant;

use cubarium_core::encounter::ApexEncounterEvent;
use cubarium_core::fields::CellClass;
use cubarium_core::habitat::Habitat;
use cubarium_core::hunter::{FixedHunterProfile, HunterEvent, HunterTarget};
use cubarium_core::organism::{DeathCause, Mode};
use cubarium_core::{ApexDormancyEvent, FOLIAGE, LifeEvent, OrganismId, World, WorldConfig};
use cubarium_surface::cell_of;
use serde::{Deserialize, Serialize};

use crate::depletion::{DepletionTracker, PlantConstants, ServedAttributor, monod_reference};
use crate::metrics::{Components, EcoMeasures, FORMS_POSSIBLE, Sample, guild_of};
use crate::movement::{
    BodyTrack, CensusBuilder, CensusKey, CrossingCounter, CrossingKind, FormSpatial, FounderBroods,
    MarginAccumulator, Movement, SKIMMER_FORM, SpatialAccumulator, StoreSum, Stores, diet_bin,
};
use crate::params;
use crate::plant_budget::{CellSample, PlantBudgetTracker};
use crate::rng;

/// How often the recorder walks the cells for foliage depletion and the organisms for their
/// position. One second of simulated time: a body cruising at the calibrated 1 BL/s crosses
/// about a quarter of a 4 × 4 px cell in that time, so a probe this often cannot miss a cell
/// a body actually occupied. Cheap next to the step: 1,280 comparisons and one position read
/// per body every twenty ticks.
pub const PROBE_EVERY: u64 = 20;

/// How many equal windows the horizon is cut into for the spatial measure. The last one is
/// also the **late window** every `Components::late` figure is taken over, so "last 20 % of
/// the horizon" and "the last spatial window" are the same interval.
pub const WINDOWS: u64 = 5;

/// A cell counts as **depleted** when its foliage falls below this fraction of what it held
/// at tick 0, and as **recovered** when it later climbs back above [`RECOVERY_FRACTION`] of
/// the same reference.
///
/// The reference is the cell's own opening foliage, `P_0 = initial_fraction · min(P_max, α·W_0)`
/// with `W_0 ∝ L_0·μ_0` (`design/ecology-v1-contract.md` §11), so it is already proportional
/// to the cell's light band and needs no separate per-band baseline run. It is **not** a
/// steady state — B0 measured bright stands still climbing at the 30-minute horizon — so a
/// depletion event means "this cell lost three quarters of the foliage it was given", which
/// is what a grazed-out cell looks like, and not "this cell fell below its equilibrium".
/// Cells that opened bare are not watched; they are counted as bare cells instead.
pub const DEPLETION_FRACTION: f64 = 0.25;
/// See [`DEPLETION_FRACTION`].
pub const RECOVERY_FRACTION: f64 = 0.5;

/// A body is counted in a spatial window only if it was seen on at least this fraction of the
/// window's probes, so the measure is a foraging range and not a lifespan.
pub const SPATIAL_COVERAGE: f64 = 0.9;

/// The build that produced a row, stamped by `build.rs`.
pub const BUILD_ID: &str = env!("CUBARIUM_SEARCH_BUILD");

/// How often the world's own invariants are checked while it is being preconditioned. Ten
/// simulated minutes: the same order as a run's own `sample_every`, and cheap against 180,000
/// plant-only ticks.
pub const PRECONDITION_CHECK_EVERY: u64 = 12_000;

/// How much of a preconditioned comparison arm's opening is recorded point by point: one
/// simulated hour (`3,600 s / DT`), which is the interval the status-quo world's uniform
/// green-then-fade happens over and therefore the interval the comparison has to show.
pub const OPENING_TRAJECTORY_TICKS: u64 = 72_000;

/// One point of that trajectory. Recorded only on a run that founded through
/// [`World::found_roster`] (`RunOptions::precondition`), so no row produced before workstream
/// S carries one and none of them change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpeningPoint {
    /// Ticks since the roster was founded, so age 0 and age 180,000 are on the same axis.
    pub ticks_since_founding: u64,
    /// The whole-field stocks: `Σ P`, `Σ W`, `Σ Q`, `Σ N`.
    pub foliage: f64,
    pub wood: f64,
    pub plant_reserve: f64,
    pub nutrient: f64,
    pub population: u32,
    /// Cumulative since founding, summed over every cell of the per-cell plant record: what
    /// the plants put into foliage, what they took back out, the gross income that funded it,
    /// and what mouths withdrew. All zero when the record is off.
    pub foliage_in: f64,
    pub foliage_out: f64,
    pub income: f64,
    pub withdrawal: f64,
}

/// How an evaluation is run. Fixed across a whole search, so candidates are comparable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Protocol {
    /// Hard tick limit for one evaluation. Enforced by the loop, not by a timer.
    pub horizon_ticks: u64,
    /// Ticks between observations. Also the cadence of the full `check_invariants` call.
    pub sample_every: u64,
    /// Apex founders placed at the start: the accounted input. `0` runs a prey-only world.
    pub apex_founders: u32,
    /// Tick at which they are placed. `0` means "into the opening world".
    pub apex_introduce_tick: u64,
}

impl Default for Protocol {
    fn default() -> Self {
        Protocol {
            horizon_ticks: 2_000,
            sample_every: 100,
            apex_founders: 2,
            apex_introduce_tick: 0,
        }
    }
}

impl Protocol {
    pub fn validate(&self) -> Result<(), String> {
        if self.horizon_ticks == 0 {
            return Err("protocol.horizon_ticks is zero".into());
        }
        if self.sample_every == 0 {
            return Err("protocol.sample_every is zero".into());
        }
        if self.sample_every > self.horizon_ticks {
            return Err(format!(
                "protocol.sample_every {} exceeds horizon_ticks {}",
                self.sample_every, self.horizon_ticks
            ));
        }
        if self.apex_founders > 2 {
            return Err(format!(
                "protocol.apex_founders {} exceeds the two the core's interactive \
                 introduction accepts",
                self.apex_founders
            ));
        }
        if self.apex_introduce_tick >= self.horizon_ticks {
            return Err(format!(
                "protocol.apex_introduce_tick {} is not inside horizon_ticks {}",
                self.apex_introduce_tick, self.horizon_ticks
            ));
        }
        Ok(())
    }
}

/// Why an evaluation produced no metrics. Recorded, never silently discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The world ran to the horizon, or to a terminal collapse, with its audits intact.
    Completed,
    /// The core refused the configuration. A real region of the search box, not an error.
    Invalid,
    /// The simulation itself broke: an invariant failed, an audit drifted, or it panicked.
    Failed,
}

/// One evaluated `(candidate, seed)` pair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    pub status: Status,
    /// Present for `Invalid` and `Failed`, absent for `Completed`.
    pub reason: Option<String>,
    pub seed: u64,
    pub protocol: Protocol,
    pub build_id: String,
    pub elapsed_ms: u64,
    /// Material and energy the apex cohort imported from outside, so the input is on the record.
    pub apex_material_in: f64,
    pub apex_energy_in: f64,
    pub metrics: Option<Components>,
    /// The spatial-coupling measures and the variety census added for workstream F. Absent on
    /// a refused or failed run, exactly as `metrics` is.
    #[serde(default)]
    pub movement: Option<Movement>,
    /// The opening hour's whole-field trajectory, on a preconditioned arm only (workstream S).
    /// `None` everywhere else, including on every row produced before S.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening: Option<Vec<OpeningPoint>>,
    /// `state_hash` of the world the instant after [`World::found_roster`] placed the cohort,
    /// on a preconditioned arm only. It names the opening this row was run from, so the arm's
    /// opening can be checked against the state the field stage saved for the same age
    /// independently of anything the run went on to do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub founding_state_hash: Option<u64>,
}

impl Evaluation {
    fn refused(
        status: Status,
        reason: String,
        seed: u64,
        protocol: Protocol,
        elapsed: u64,
    ) -> Self {
        Evaluation {
            status,
            reason: Some(reason),
            seed,
            protocol,
            build_id: BUILD_ID.to_string(),
            elapsed_ms: elapsed,
            apex_material_in: 0.0,
            apex_energy_in: 0.0,
            metrics: None,
            movement: None,
            opening: None,
            founding_state_hash: None,
        }
    }
}

/// The base world every candidate starts from: the shipped defaults, with the host-side
/// event log off. Founders, capacities and initial stocks are untouched, so every candidate
/// gets the same starting resources and the same landscape for a given seed.
pub fn base_config(seed: u64) -> WorldConfig {
    let mut config = WorldConfig {
        seed,
        ..WorldConfig::default()
    };
    config.capacity.event_log = false;
    config
}

/// Deterministic apex founder placements for a seed: distinct faces, away from the seams.
pub(crate) fn apex_targets(seed: u64, count: u32) -> Vec<HunterTarget> {
    (0..count)
        .map(|i| {
            let face = (rng::draw(seed, rng::stream::APEX_PLACEMENT, u64::from(i), 0) % 5) as u8;
            let u = 6.0 + rng::unit(seed, rng::stream::APEX_PLACEMENT, u64::from(i), 1) * 52.0;
            let v = 6.0 + rng::unit(seed, rng::stream::APEX_PLACEMENT, u64::from(i), 2) * 52.0;
            HunterTarget { face, u, v }
        })
        .collect()
}

/// The audited energy total: `Σ_cells (e_v·(P + W + Q + Wd) + e_f·F + De + Ce) +
/// Σ_organisms (E + e_r·R) + Σ_escrow (e_r·(S+R) + E) + Σ hunter guts`
/// (`design/ecology-v1-contract.md` §10). Recomputed here because the core's own copy is
/// compiled out of release builds, and this milestone needs the identity checked in release.
pub(crate) fn stored_energy(world: &World) -> f64 {
    let state = &world.state;
    let e_v = state.config.plant.energy_density;
    let e_f = state.config.fruit.energy_density;
    let e_r = state.config.organism.reserve_energy_density;
    let cells: f64 = state.fields.p.iter().map(|p| e_v * p).sum::<f64>()
        + state.fields.f.iter().map(|f| e_f * f).sum::<f64>()
        + state.fields.de.iter().sum::<f64>()
        + state.ecology.stored_energy(e_v);
    let organisms: f64 = state
        .organisms
        .iter()
        .map(|(_, o)| {
            o.energy
                + e_r * o.reserve
                + o.escrow
                    .as_ref()
                    .map_or(0.0, |e| e_r * (e.structure + e.reserve) + e.energy)
        })
        .sum();
    cells + organisms + state.hunters.gut_energy_total()
}

/// What a run records **beside** its ordinary metrics.
///
/// Deliberately not a [`Protocol`] field: `Protocol` is serialised into every retained row and
/// is constructed by the trainer and the genetic search as well, and neither of those wants
/// the ledger's throughput cost. A row says what it recorded through `movement.ledger_on`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunOptions {
    /// Turn `World::record_body_budgets` on, so the net energy margin per body is measured
    /// rather than inferred (workstream E's ledger). Off is the shipped path and is what
    /// every row before workstream I was produced under.
    pub ledger: bool,
    /// Turn `World::record_plant_budgets` on, so every §4 plant flow and every §6.4 consumer
    /// withdrawal is booked per cell where it happens (workstream M's record). Off is the
    /// shipped path and is what every row before workstream M was produced under.
    pub plant_record: bool,
    /// Found **no animals at all**: `founders.kinds` empty and `founders.count` zero, so the
    /// world is plants, water, weather and nothing else (`world/lifecycle.rs:52-63`).
    ///
    /// This is workstream M's herbivore-absent arm. It changes the configuration, so it
    /// changes the world and its hash deliberately — unlike the two recorders above, which
    /// must not. A run under it is also exempt from the empty-world stop below: a world with
    /// no population by construction has not collapsed.
    pub no_animals: bool,
    /// Found the roster **after** this many plant-only ticks: workstream S's fixed-age
    /// preconditioned opening (option A with a declared procedure). The world is built with an
    /// empty roster, [`precondition`] advances the ordinary §4 dynamics with nothing eating,
    /// the roster is written back into the config, and [`World::found_roster`] places the same
    /// 24 founders the constructor would have placed — into the field the plants have grown.
    ///
    /// `None` is the shipped path: the constructor founds the roster itself and nothing here
    /// runs, which is what every row before workstream S was produced under. `Some(0)` is the
    /// **status-quo arm of the comparison**: the door founds at tick 0 into a world that was
    /// built with an empty roster, which the core's own test proves is the constructor's world
    /// bit for bit (`crates/cubarium-core/tests/found_roster.rs`) — running the age-0 arm
    /// through the door rather than around it is what makes the campaign's reproduction of
    /// workstream M's retained `final_state_hash`es evidence about the door.
    ///
    /// Not compatible with `no_animals`, which never founds at all.
    pub precondition: Option<u64>,
    /// **Which motor contract the run's world uses**
    /// (`cubarium_core::MotorModel`, `crate::World::set_motor_model`). The default is
    /// `Sweep`, the shipped contract, and a run under it is byte-identical to every row
    /// produced before the switch existed — the two arms of workstream T's pairing differ in
    /// exactly this field (`design/7_Research/ecology-v1-motor-inertial-2026-09-16.md`).
    ///
    /// It is a transient on the world, not a configuration: it does not enter
    /// [`crate::calibrate::config_hash`] and no retained row's hash moves because of it.
    pub motor: cubarium_core::MotorModel,
    /// **Which pursuit stopping rule the run's world uses**
    /// (`cubarium_core::hunter::PursuitStop`, `crate::World::set_pursuit_stop`). The default is
    /// the shipped rule, `ReachEnvelope`, adopted on 2026-09-16
    /// (`design/7_Research/ecology-v1-predicate-adoption-2026-09-16.md`).
    ///
    /// A row produced before that date ran `ForwardHalfSpace`, so **reproducing a retained
    /// `final_state_hash` means asking for it by name** — which is what `--pursuit-stop
    /// half-space` is for. Like `motor`, it is a transient on the world: it does not enter
    /// [`crate::calibrate::config_hash`] and no configuration's hash moves because of it.
    pub pursuit_stop: cubarium_core::hunter::PursuitStop,
}

/// Advance a world's plant dynamics with no animals in it, the ordinary §4 tick and nothing
/// else. The world must already have been built with an empty roster; this function does not
/// remove anybody.
///
/// One definition, used by [`run`] inside a comparison arm and by
/// [`crate::precondition`]'s field stage, so "the world at age T" is the same object in both
/// and the two can be checked against each other by `state_hash`.
///
/// `check_every` ticks, the world's own invariants are checked; a violation stops the
/// preconditioning by name rather than founding into a world that has already gone wrong.
pub fn precondition(world: &mut World, ticks: u64, check_every: u64) -> Result<(), String> {
    if world.population() > 0 {
        return Err(format!(
            "preconditioning is plant-only, but this world already holds {} organisms",
            world.population()
        ));
    }
    let every = check_every.max(1);
    for _ in 0..ticks {
        world.step();
        // Nothing with a mouth is in the world, so these are empty every tick; drained anyway
        // so the founding starts from the clean queue an ordinary run starts from.
        world.drain_events();
        world.drain_hunter_events();
        world.drain_apex_dormancy_events();
        world.drain_apex_encounter_events();
        world.drain_quiet_events();
        if world.tick() % every == 0 {
            world.check_invariants().map_err(|e| {
                format!(
                    "invariant violated while preconditioning at tick {}: {e}",
                    world.tick()
                )
            })?;
        }
    }
    Ok(())
}

/// Run one candidate on one seed. Never panics: a panic inside the core is caught and
/// reported as [`Status::Failed`].
pub fn evaluate(values: &[f64], seed: u64, protocol: Protocol) -> Evaluation {
    evaluate_with(values, seed, protocol, RunOptions::default())
}

/// [`evaluate`] with the optional recorders named explicitly.
pub fn evaluate_with(
    values: &[f64],
    seed: u64,
    protocol: Protocol,
    options: RunOptions,
) -> Evaluation {
    let start = Instant::now();
    match std::panic::catch_unwind(AssertUnwindSafe(|| {
        run(values, seed, protocol, options, start)
    })) {
        Ok(evaluation) => evaluation,
        Err(payload) => {
            let what = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_string());
            Evaluation::refused(
                Status::Failed,
                format!("simulation panicked: {what}"),
                seed,
                protocol,
                start.elapsed().as_millis() as u64,
            )
        }
    }
}

fn run(
    values: &[f64],
    seed: u64,
    protocol: Protocol,
    options: RunOptions,
    start: Instant,
) -> Evaluation {
    let ms = |start: Instant| start.elapsed().as_millis() as u64;
    if let Err(e) = protocol.validate() {
        return Evaluation::refused(Status::Invalid, e, seed, protocol, ms(start));
    }

    if options.no_animals && options.precondition.is_some() {
        return Evaluation::refused(
            Status::Invalid,
            "a run cannot both found no animals and found them after a plant-only prefix"
                .to_string(),
            seed,
            protocol,
            ms(start),
        );
    }
    let mut config = base_config(seed);
    // The roster this world's founding will use, kept aside before the plant-only prefix
    // clears it. Writing it back before `found_roster` is what makes a preconditioned world's
    // *config* — and therefore its hash — the ordinary one.
    let roster = config.founders.clone();
    if options.no_animals || options.precondition.is_some() {
        // Before `params::apply`, so a searched parameter can still refuse an invalid world,
        // and before `validate`, so an empty roster is validated like any other.
        config.founders.kinds.clear();
        config.founders.count = 0;
    }
    // The profile is derived from the *base* config so that the genome it carries does not
    // shift under the searched drive parameters; only the two searched apex fields move.
    let mut profile = FixedHunterProfile::lanternjaw_trial(&config);
    if let Err(e) = params::apply(values, &mut config, &mut profile) {
        return Evaluation::refused(Status::Invalid, e, seed, protocol, ms(start));
    }
    if let Err(e) = config.validate() {
        return Evaluation::refused(
            Status::Invalid,
            format!("config rejected: {e}"),
            seed,
            protocol,
            ms(start),
        );
    }
    if let Err(e) = profile.validate() {
        return Evaluation::refused(
            Status::Invalid,
            format!("hunter profile rejected: {e}"),
            seed,
            protocol,
            ms(start),
        );
    }

    let mut world = match World::new(config) {
        Ok(w) => w,
        Err(e) => {
            return Evaluation::refused(
                Status::Invalid,
                format!("world creation refused: {e}"),
                seed,
                protocol,
                ms(start),
            );
        }
    };

    // The motor contract and the pursuit stopping rule, before the first tick. Both are
    // transients on the world, so the config and its hash are untouched; `MotorModel::Sweep`
    // and `PursuitStop::ReachEnvelope` are the shipped defaults and naming them changes
    // nothing. Naming `ForwardHalfSpace` is how a row retained before 2026-09-16 is reproduced.
    world.set_motor_model(options.motor);
    world.set_pursuit_stop(options.pursuit_stop);
    // Workstream S's fixed-age preconditioned opening, before anything is introduced or
    // recorded: the plants run alone to the declared age, then the ordinary roster is founded
    // into the field they grew. At age 0 nothing is stepped and the door reproduces the
    // constructor, so this branch is the status-quo arm too.
    if let Some(age) = options.precondition {
        if let Err(e) = precondition(&mut world, age, PRECONDITION_CHECK_EVERY) {
            return Evaluation::refused(Status::Failed, e, seed, protocol, ms(start));
        }
        world.state.config.founders = roster;
        if let Err(e) = world.found_roster() {
            return Evaluation::refused(
                Status::Invalid,
                format!("founding the roster refused: {e}"),
                seed,
                protocol,
                ms(start),
            );
        }
    }
    let founding_state_hash = options
        .precondition
        .map(|_| cubarium_core::snapshot::state_hash(&world.state));
    // Every tick the recorder measures is counted from here, not from world creation, so a
    // preconditioned run's windows, horizon and late window are the same intervals a
    // status-quo run's are. At age 0 this is 0 and nothing below changes.
    let origin_tick = world.tick();

    let mut apex_material_in = 0.0;
    let mut apex_energy_in = 0.0;
    let mut apex_introduced = 0;
    if protocol.apex_founders > 0 && protocol.apex_introduce_tick == 0 {
        match introduce(&mut world, &profile, seed, protocol.apex_founders) {
            Ok((m, e, n)) => {
                apex_material_in = m;
                apex_energy_in = e;
                apex_introduced = n;
            }
            Err(e) => return Evaluation::refused(Status::Invalid, e, seed, protocol, ms(start)),
        }
    }

    // Before the first step, so every body's record opens on its own opening inventory.
    // The flag is read only at sites that add to a counter; the campaign's row check against
    // workstream F's ledger-off rows is what tests that claim rather than repeating it.
    world.record_body_budgets(options.ledger);
    // Opened before the first step, so the record's `P₀` is the world's own tick-0 foliage and
    // the closing identity spans the whole horizon.
    world.record_plant_budgets(options.plant_record);
    let mut recorder = Recorder::new(&world, protocol, apex_introduced, options);
    // Founder placement emits nothing, but drain anyway so the run starts from a clean queue.
    world.drain_events();
    world.drain_hunter_events();
    world.drain_apex_dormancy_events();
    world.drain_apex_encounter_events();
    world.drain_quiet_events();
    recorder.sample(&world);

    for _ in 0..protocol.horizon_ticks {
        world.step();
        if protocol.apex_founders > 0
            && apex_introduced == 0
            && world.tick() == origin_tick + protocol.apex_introduce_tick
        {
            match introduce(&mut world, &profile, seed, protocol.apex_founders) {
                Ok((m, e, n)) => {
                    apex_material_in = m;
                    apex_energy_in = e;
                    apex_introduced = n;
                    recorder.apex_introduced = n;
                }
                Err(e) => {
                    return Evaluation::refused(Status::Invalid, e, seed, protocol, ms(start));
                }
            }
        }
        recorder.absorb(&mut world);
        if (world.tick() - origin_tick) % protocol.sample_every == 0 {
            if let Err(e) = world.check_invariants() {
                return Evaluation::refused(
                    Status::Failed,
                    format!("invariant violated at tick {}: {e}", world.tick()),
                    seed,
                    protocol,
                    ms(start),
                );
            }
            recorder.sample(&world);
        }
        if world.population() == 0 && !options.no_animals {
            // Nothing is ever created from nothing: an empty world cannot recover, so the
            // horizon stops here and the collapse tick is the honest survival time. A
            // deliberately animal-free arm is not that world: it is exempt by the flag, not
            // by an exception the ordinary path could take.
            recorder.collapsed_at = Some(world.tick() - origin_tick);
            break;
        }
    }

    let (metrics, movement, opening) = recorder.finish(&mut world);
    Evaluation {
        status: Status::Completed,
        reason: None,
        seed,
        protocol,
        build_id: BUILD_ID.to_string(),
        elapsed_ms: ms(start),
        apex_material_in,
        apex_energy_in,
        metrics: Some(metrics),
        movement: Some(movement),
        opening,
        founding_state_hash,
    }
}

fn introduce(
    world: &mut World,
    profile: &FixedHunterProfile,
    seed: u64,
    count: u32,
) -> Result<(f64, f64, u32), String> {
    let targets = apex_targets(seed, count);
    let receipts = world
        .introduce_hunters(profile.clone(), &targets)
        .map_err(|e| format!("apex introduction refused: {e}"))?;
    let material = receipts.iter().map(|r| r.material_in).sum();
    let energy = receipts.iter().map(|r| r.energy_in).sum();
    Ok((material, energy, receipts.len() as u32))
}

/// Accumulates every component metric while the world runs.
struct Recorder {
    protocol: Protocol,
    samples: Vec<Sample>,
    apex_introduced: u32,

    founders: BTreeSet<OrganismId>,
    apex_ids: BTreeSet<OrganismId>,
    born_in_run: BTreeSet<OrganismId>,
    parent_of: BTreeMap<OrganismId, OrganismId>,
    matured: BTreeSet<OrganismId>,
    reproduced: BTreeSet<OrganismId>,
    parents_seen: BTreeSet<OrganismId>,

    prey_births: u64,
    prey_deaths: u64,
    apex_births: u64,
    apex_deaths: u64,
    deaths_by_cause: [u64; 4],
    apex_attacks: u64,
    apex_captures: u64,
    apex_matings: u64,
    apex_emergences: u64,
    apex_exhausted: u64,

    // --- ecology v1: guild census, depletion, foraging range ------------------------
    /// What a body **is**, recorded when it is first seen and never revised: `diet` mutates at
    /// conception and never afterwards and `form` is immutable under mutation, so
    /// `births = deaths + Δpopulation` holds per guild and per census cell.
    key_of_id: BTreeMap<OrganismId, CensusKey>,
    prey_births_guild: [u64; 3],
    prey_deaths_guild: [u64; 3],
    /// Foliage depletion and recovery, per cell, on A's thresholds and A's hysteresis.
    crossings: CrossingCounter,
    opening_foliage: f64,
    opening_wood: f64,
    opening_litter: f64,
    opening_alive_cells: u32,
    watched_cells: u32,
    /// The spatial measures: one online visit track per prey body inside the current window.
    tracks: BTreeMap<OrganismId, BodyTrack>,
    probes_this_window: u64,
    window_ticks: u64,
    window_index: u64,
    /// One accumulator per completed window, and the same split by visual form.
    spatial_windows: Vec<SpatialAccumulator>,
    spatial_by_form: Vec<BTreeMap<u8, SpatialAccumulator>>,
    spatial_bodies: u64,

    // --- workstream F: the price of travel, terminal stores and the census -----------
    /// The price this world charged per structure-pixel, echoed into the row.
    move_cost: f64,
    /// `organism.reserve_energy_density`, so a terminal store can be reported as usable energy.
    e_r: f64,
    /// Each prey body's stores at the last probe it was seen on: at most one probe interval
    /// before its death event, which is why the row calls them *last observed*, not *final*.
    last_stores: BTreeMap<OrganismId, Stores>,
    terminal_by_cause: [StoreSum; 4],
    census: CensusBuilder,

    // --- workstream I: the per-depleted-cell record, the brood gate, the margin -------
    options: RunOptions,
    /// Why a depleted cell stays depleted: habitat quality, trajectory, pressure.
    depletion: DepletionTracker,
    /// E's per-body `served[FOLIAGE]` turned into a per-cell attribution.
    served: ServedAttributor,
    /// The cell each prey body was last observed in, so a body that dies between probes can
    /// still have its last bites credited somewhere.
    last_cell: BTreeMap<OrganismId, u16>,
    /// Served material attributed between probes — a dead body's tail — waiting for the next
    /// probe to hand it to the tracker.
    pending_served: BTreeMap<u16, f64>,
    /// Completed broods by tick-0 founders: the ladder's gate.
    broods: FounderBroods,
    /// Founders that have already produced a brood, so parents are counted once.
    founder_parents: BTreeSet<OrganismId>,
    /// Net energy margin per body, by `(form, diet bin)`.
    margins: MarginAccumulator,

    // --- workstream M: the counter split by exact withdrawal --------------------------
    /// Open only when `RunOptions::plant_record` is on, because it reads a record the core
    /// only keeps when asked.
    plant: Option<PlantBudgetTracker>,
    /// Reused per probe so a 180,000-tick run allocates the per-cell view once.
    plant_scratch: Vec<CellSample>,
    plant_occupied: Vec<u16>,

    first_predation_tick: Option<u64>,
    prey_before_first_predation: u32,
    prey_min_after_predation: u32,

    collapsed_at: Option<u64>,
    /// The tick recording opened at: 0 for a status-quo run, the founding tick for a
    /// preconditioned one. Every interval the recorder reports is measured from here.
    origin_tick: u64,
    /// [`OpeningPoint`]s for the first [`OPENING_TRAJECTORY_TICKS`], on a preconditioned run.
    opening: Vec<OpeningPoint>,
    opening_energy: f64,
    opening_net_in: f64,
    max_mass: f64,
    max_water: f64,
    max_energy: f64,
    peak_population: u32,
    min_population: u32,
}

impl Recorder {
    fn new(world: &World, protocol: Protocol, apex_introduced: u32, options: RunOptions) -> Self {
        let founders: BTreeSet<_> = world.state.organisms.iter().map(|(id, _)| id).collect();
        let apex_ids: BTreeSet<_> = world.hunters().members.iter().map(|m| m.id).collect();
        // Every body alive at tick 0 is classified now, so a founder's death is attributed to
        // the guild it was born into rather than to "unknown".
        let mut key_of_id = BTreeMap::new();
        let mut census = CensusBuilder::default();
        let mut broods = FounderBroods::default();
        for (id, o) in world.state.organisms.iter() {
            if !apex_ids.contains(&id) {
                let key = census_key(o);
                key_of_id.insert(id, key);
                census.found(key);
                broods.found(key.form);
            }
        }
        let alive_min = world.config().plant.alive_min;
        let p_ref: Vec<f64> = world.state.fields.p.clone();
        let crossings = CrossingCounter::new(&p_ref, DEPLETION_FRACTION, RECOVERY_FRACTION);
        let watched_cells = crossings.watched();

        // The static habitat, rebuilt from the same `(config.habitat, seed)` pair `World::new`
        // builds it from, because `World::habitat` is not public. The reconstruction is not
        // assumed: `fields::initial_wood` from it must reproduce the world's own tick-0 wood in
        // every cell, and the largest error is carried into the row.
        let cfg = world.config().clone();
        // The search crate is cube-only (`design/flat-world-plan-2026-09-16.md` §1), and the
        // world it rebuilds the habitat for is the world it was handed: name the pair the
        // world's own config carries rather than assuming it.
        let habitat = Habitat::new(&cfg.habitat, cfg.seed, cfg.topology, cfg.world_scale);
        let light: Vec<f64> = habitat.light_base.to_vec();
        let moisture: Vec<f64> = habitat.moisture_base.to_vec();
        let wood0: Vec<f64> = world.state.ecology.wood.clone();
        let habitat_error = light
            .iter()
            .zip(&moisture)
            .zip(&wood0)
            .map(|((l, m), w)| (cubarium_core::fields::initial_wood(&cfg, *l, *m) - w).abs())
            .fold(0.0f64, f64::max);
        let constants = PlantConstants {
            growth: cfg.producer.growth,
            mortality: cfg.producer.mortality,
            maintenance: cfg.plant.maintenance,
            build: cfg.plant.build,
            monod_ref: monod_reference(cfg.nutrient.half_saturation),
            deplete_fraction: DEPLETION_FRACTION,
        };
        let mut depletion = DepletionTracker::new(&p_ref, &light, &moisture, &wood0, &constants);
        depletion.set_habitat_check(habitat_error);
        // The static habitat product I classified on, carried into M's rows for comparison
        // only: M's own light and nutrient come from the plant step itself.
        let l_mu: Vec<f64> = light.iter().zip(&moisture).map(|(l, m)| l * m).collect();
        let plant = options
            .plant_record
            .then(|| PlantBudgetTracker::new(&p_ref, &l_mu, world.tick()));
        let opening_alive_cells = world
            .state
            .ecology
            .wood
            .iter()
            .filter(|w| CellClass::of(**w, alive_min) == CellClass::Alive)
            .count() as u32;
        Recorder {
            key_of_id,
            census,
            prey_births_guild: [0; 3],
            prey_deaths_guild: [0; 3],
            opening_foliage: p_ref.iter().sum(),
            opening_wood: world.state.ecology.wood.iter().sum(),
            opening_litter: world.state.fields.d.iter().sum(),
            opening_alive_cells,
            watched_cells,
            crossings,
            tracks: BTreeMap::new(),
            probes_this_window: 0,
            window_ticks: (protocol.horizon_ticks / WINDOWS).max(1),
            window_index: 0,
            spatial_windows: Vec::new(),
            spatial_by_form: Vec::new(),
            spatial_bodies: 0,
            move_cost: world.config().organism.move_cost,
            e_r: world.config().organism.reserve_energy_density,
            last_stores: BTreeMap::new(),
            terminal_by_cause: [StoreSum::default(); 4],
            options,
            depletion,
            served: ServedAttributor::default(),
            last_cell: BTreeMap::new(),
            pending_served: BTreeMap::new(),
            broods,
            founder_parents: BTreeSet::new(),
            margins: MarginAccumulator::default(),
            plant,
            plant_scratch: Vec::new(),
            plant_occupied: Vec::new(),
            protocol,
            samples: Vec::new(),
            apex_introduced,
            founders,
            apex_ids,
            born_in_run: BTreeSet::new(),
            parent_of: BTreeMap::new(),
            matured: BTreeSet::new(),
            reproduced: BTreeSet::new(),
            parents_seen: BTreeSet::new(),
            prey_births: 0,
            prey_deaths: 0,
            apex_births: 0,
            apex_deaths: 0,
            deaths_by_cause: [0; 4],
            apex_attacks: 0,
            apex_captures: 0,
            apex_matings: 0,
            apex_emergences: 0,
            apex_exhausted: 0,
            first_predation_tick: None,
            prey_before_first_predation: 0,
            prey_min_after_predation: u32::MAX,
            collapsed_at: None,
            origin_tick: world.tick(),
            opening: Vec::new(),
            opening_energy: stored_energy(world),
            opening_net_in: world.state.net_energy_in_corrected(),
            max_mass: 0.0,
            max_water: 0.0,
            max_energy: 0.0,
            peak_population: world.population() as u32,
            min_population: world.population() as u32,
        }
    }

    /// Drain one tick's events. Apex membership is read from the world itself, so a child is
    /// classified by what the core committed, not by the order this observer sees events in.
    fn absorb(&mut self, world: &mut World) {
        for m in &world.hunters().members {
            self.apex_ids.insert(m.id);
        }
        for event in world.drain_hunter_events() {
            match event {
                // Paid attempts only: an unpaid refusal consumes no counter and is not a
                // hunt the ecology had to fund.
                HunterEvent::Attempt {
                    attack_counter: Some(_),
                    ..
                } => self.apex_attacks += 1,
                HunterEvent::Capture { .. } => self.apex_captures += 1,
                HunterEvent::Offspring { child, .. } => {
                    self.apex_ids.insert(child);
                }
                _ => {}
            }
        }
        for event in world.drain_apex_encounter_events() {
            match event {
                ApexEncounterEvent::Mated { .. } => self.apex_matings += 1,
                ApexEncounterEvent::Born { child, .. } => {
                    self.apex_ids.insert(child);
                }
                _ => {}
            }
        }
        for event in world.drain_apex_dormancy_events() {
            match event {
                ApexDormancyEvent::Emerged { .. } => self.apex_emergences += 1,
                ApexDormancyEvent::Exhausted { .. } => self.apex_exhausted += 1,
                _ => {}
            }
        }
        world.drain_quiet_events();

        for event in world.drain_events() {
            match event {
                LifeEvent::Birth {
                    tick, id, parent, ..
                } => {
                    self.parent_of.insert(id, parent);
                    self.parents_seen.insert(parent);
                    if self.apex_ids.contains(&id) {
                        self.apex_births += 1;
                    } else {
                        self.prey_births += 1;
                        self.born_in_run.insert(id);
                        if self.born_in_run.contains(&parent) {
                            self.reproduced.insert(parent);
                        }
                        // The child's guild and diet bin are read from the phenotype the
                        // world just decoded, not from the parent's: `diet` mutates at
                        // conception, and that is exactly how a guild census changes.
                        let key =
                            world
                                .state
                                .organisms
                                .get(id)
                                .map(census_key)
                                .unwrap_or(CensusKey {
                                    form: 0,
                                    diet_bin: 1,
                                    guild: 2,
                                });
                        self.key_of_id.insert(id, key);
                        self.census.born(key);
                        self.prey_births_guild[usize::from(key.guild)] += 1;
                        // The ladder's gate. A `Birth` is emitted when gestation completes and
                        // the child is committed, so this **is** a completed brood; nothing is
                        // inferred from a reserve or an age. The parent's own form is used,
                        // not the child's, so a mutated child cannot move a founder's brood
                        // into another rig's column — `form` is immutable anyway, which makes
                        // the two agree and is why the choice is stated rather than hidden.
                        if self.founders.contains(&parent) {
                            let form = self.key_of_id.get(&parent).map_or(0, |k| k.form);
                            let first = self.founder_parents.insert(parent);
                            self.broods.brood(form, tick, first);
                        }
                    }
                }
                LifeEvent::Death {
                    id,
                    cause,
                    tick,
                    age_ticks,
                    ..
                } => {
                    let slot = match cause {
                        DeathCause::Starvation => 0,
                        DeathCause::Age => 1,
                        DeathCause::Collapse => 2,
                        DeathCause::Predation => 3,
                    };
                    self.deaths_by_cause[slot] += 1;
                    if self.apex_ids.contains(&id) {
                        self.apex_deaths += 1;
                    } else {
                        self.prey_deaths += 1;
                        // A body removed in step 9 is already gone from the slot map, so both
                        // its census key and its stores come from the records made while it
                        // was alive: the key from when it was first seen, the stores from the
                        // last probe before this event.
                        let stores = self.last_stores.remove(&id);
                        if let Some(key) = self.key_of_id.get(&id).copied() {
                            self.prey_deaths_guild[usize::from(key.guild)] += 1;
                            self.census.died(
                                key,
                                slot,
                                crate::metrics::DEATH_CAUSES[slot],
                                age_ticks,
                                stores.as_ref(),
                            );
                        }
                        if let Some(s) = stores.as_ref() {
                            self.terminal_by_cause[slot].add(s);
                        }
                    }
                    if cause == DeathCause::Predation && self.first_predation_tick.is_none() {
                        self.first_predation_tick = Some(tick);
                        self.prey_before_first_predation =
                            self.samples.last().map(|s| s.prey).unwrap_or(0);
                    }
                }
            }
        }

        self.drain_budgets(world);

        if world.tick() % PROBE_EVERY == 0 {
            self.probe(world);
        }
        // A window closes the first time the tick crosses its boundary. A partial final
        // window — the world collapsed inside it — is deliberately not closed: a foraging
        // range measured over part of a window is not the same quantity.
        let index = (world.tick() - self.origin_tick) / self.window_ticks;
        if index > self.window_index {
            self.close_window();
            self.window_index = index;
        }
    }

    /// One second of simulated time: foliage depletion and recovery per cell, and where each
    /// prey body is standing. Reads only; writes nothing the world can see.
    fn probe(&mut self, world: &World) {
        let tick = world.tick();
        let probe_index = self.probes_this_window;
        self.probes_this_window += 1;
        let mut skimmer_alive = false;
        // Positions and attributed bites are read **before** the crossings, which is what makes
        // "the last consumer visit before depletion" well defined: a body standing in a cell as
        // it crosses is the last visit before, at zero ticks.
        let mut occupancy: BTreeMap<u16, u32> = BTreeMap::new();
        let mut served = std::mem::take(&mut self.pending_served);
        for (id, o) in world.state.organisms.iter() {
            if self.apex_ids.contains(&id) {
                continue;
            }
            let cell = cell_of(Topology::Cube, Scale::ONE, &o.pos).index() as u16;
            self.tracks
                .entry(id)
                .or_default()
                .observe(probe_index, cell);
            *occupancy.entry(cell).or_insert(0) += 1;
            self.last_cell.insert(id, cell);
            if self.options.ledger
                && let Some(budget) = world.body_budget(id)
            {
                self.served
                    .observe(id, cell, budget.served[FOLIAGE], &mut served);
            }
            self.last_stores.insert(
                id,
                Stores {
                    energy: o.energy,
                    reserve: o.reserve,
                    structure: o.structure,
                    usable: o.energy + self.e_r * o.reserve,
                    hunger: o.hunger(),
                },
            );
            if o.phenotype.form == SKIMMER_FORM {
                skimmer_alive = true;
            }
        }
        if skimmer_alive {
            self.census.skimmer_seen(tick);
        }

        let p = &world.state.fields.p;
        self.depletion.note_probe(tick, p, &occupancy, &served);
        // Workstream M's cumulative per-cell samples, taken **before** this probe's crossings,
        // so "since the last recovery" has the detection ticks themselves as its endpoints.
        if let Some(plant) = self.plant.as_mut()
            && let Some(rec) = world.plant_budget()
        {
            self.plant_scratch.clear();
            self.plant_scratch
                .extend(rec.cells.iter().map(|c| CellSample {
                    foliage_in: c.foliage_in(),
                    foliage_out: c.foliage_out(),
                    withdrawal: c.withdrawal_foliage,
                    income: c.income,
                    maintenance_unpaid: c.maintenance_unpaid,
                    death_foliage: c.death_foliage,
                    wood_sum: c.w_sum,
                    light_sum: c.light_effective_sum,
                    nutrient_sum: c.nutrient_sum,
                    ticks_alive: c.ticks_alive,
                }));
            self.plant_occupied.clear();
            self.plant_occupied.extend(occupancy.keys().copied());
            plant.observe(tick, &self.plant_scratch, &self.plant_occupied);
        }
        // One rule, one place: the per-cell record is driven by the same counter A's aggregate
        // numbers come out of, through the closure form, so the two cannot drift. M's split is
        // driven from the same closure for the same reason.
        let crossings = &mut self.crossings;
        let depletion = &mut self.depletion;
        let plant = self.plant.as_mut();
        let mut plant = plant;
        crossings.observe_reporting(p, |cell, kind| match kind {
            CrossingKind::Depleted => {
                depletion.depleted(cell, tick);
                if let Some(plant) = plant.as_mut() {
                    plant.depleted(cell, tick, p.get(cell).copied().unwrap_or(0.0));
                }
            }
            CrossingKind::Recovered => {
                depletion.recovered(cell, tick);
                if let Some(plant) = plant.as_mut() {
                    plant.recovered(cell, tick);
                }
            }
        });
        self.depletion.sample(tick, p);
    }

    /// Take the ledger's closed records: the bodies that died since the last drain. Their
    /// served tail is credited to the cell they were last observed in and waits for the next
    /// probe; their margin goes into the bins.
    fn drain_budgets(&mut self, world: &mut World) {
        if !self.options.ledger {
            return;
        }
        let now = world.tick();
        let (closed, dropped) = world.drain_body_budgets();
        if dropped > 0 {
            self.margins.note_dropped(dropped);
        }
        for budget in closed {
            let Some(key) = self.key_of_id.get(&budget.id).copied() else {
                // An apex member: it has a record but no census key, and its bites are not
                // foliage. Forget it so a reused slot cannot inherit its reading.
                self.served.forget(budget.id);
                continue;
            };
            match self.last_cell.remove(&budget.id) {
                Some(cell) => self.served.close(
                    budget.id,
                    cell,
                    budget.served[FOLIAGE],
                    &mut self.pending_served,
                ),
                None => self.served.forget(budget.id),
            }
            self.margins
                .add(key, &budget, self.e_r, now, cubarium_core::DT, false);
        }
    }

    /// Close one spatial window: the mean distinct cells visited by every body that was
    /// present for [`SPATIAL_COVERAGE`] of its probes.
    fn close_window(&mut self) {
        let need = ((self.probes_this_window as f64) * SPATIAL_COVERAGE).ceil() as u64;
        let mut window = SpatialAccumulator::default();
        let mut by_form: BTreeMap<u8, SpatialAccumulator> = BTreeMap::new();
        for (id, track) in &mut self.tracks {
            track.finish();
            if track.probes_seen() < need.max(1) {
                continue;
            }
            window.add(track, PROBE_EVERY);
            let form = self.key_of_id.get(id).map_or(0, |k| k.form);
            by_form.entry(form).or_default().add(track, PROBE_EVERY);
        }
        self.spatial_bodies += window.bodies();
        self.spatial_windows.push(window);
        self.spatial_by_form.push(by_form);
        self.tracks.clear();
        self.probes_this_window = 0;
    }

    /// One [`OpeningPoint`] per sample over the first [`OPENING_TRAJECTORY_TICKS`] after
    /// founding, on a preconditioned run only. The whole-field stocks come from the state; the
    /// four flows are the per-cell plant record summed over every cell, which is cumulative
    /// since the record opened — and the record opens at founding, so the point's flows and
    /// its tick share an origin.
    fn note_opening(&mut self, world: &World) {
        if self.options.precondition.is_none() {
            return;
        }
        let since = world.tick() - self.origin_tick;
        if since > OPENING_TRAJECTORY_TICKS {
            return;
        }
        if self.opening.last().map(|o| o.ticks_since_founding) == Some(since) {
            return;
        }
        let state = &world.state;
        let mut point = OpeningPoint {
            ticks_since_founding: since,
            foliage: state.fields.p.iter().sum(),
            wood: state.ecology.wood.iter().sum(),
            plant_reserve: state.ecology.plant_reserve.iter().sum(),
            nutrient: state.fields.n.iter().sum(),
            population: world.population() as u32,
            ..OpeningPoint::default()
        };
        if let Some(rec) = world.plant_budget() {
            for c in &rec.cells {
                point.foliage_in += c.foliage_in();
                point.foliage_out += c.foliage_out();
                point.income += c.income;
                point.withdrawal += c.withdrawal_foliage;
            }
        }
        self.opening.push(point);
    }

    fn sample(&mut self, world: &World) {
        self.note_opening(world);
        let state = &world.state;
        let dormant: BTreeSet<_> = state.apex_dormancy.dormant.iter().map(|d| d.id).collect();
        let live_apex: BTreeSet<_> = world
            .hunters()
            .members
            .iter()
            .map(|m| m.id)
            .filter(|id| state.organisms.get(*id).is_some())
            .collect();

        let mut forms = [0u32; 8];
        let (mut feeding, mut seeking, mut resting) = (0u32, 0u32, 0u32);
        let mut hunger = 0.0;
        let mut population = 0u32;
        let mut guild = [0u32; 3];
        let mut form_counts = [0u32; 5];
        for (id, o) in state.organisms.iter() {
            population += 1;
            forms[(o.phenotype.form as usize).min(7)] += 1;
            form_counts[(o.phenotype.form as usize).min(4)] += 1;
            if !live_apex.contains(&id) {
                guild[guild_of(o.phenotype.cap_foliage, o.phenotype.cap_detrital)] += 1;
            }
            match o.mode {
                Mode::Feeding => feeding += 1,
                Mode::Seeking => seeking += 1,
                Mode::Resting => resting += 1,
            }
            hunger += o.hunger();
        }
        let prey = population - live_apex.len() as u32;
        let present = forms.iter().filter(|c| **c > 0).count() as u32;
        let entropy = if population > 0 {
            let total = f64::from(population);
            -forms
                .iter()
                .filter(|c| **c > 0)
                .map(|c| {
                    let p = f64::from(*c) / total;
                    p * p.ln()
                })
                .sum::<f64>()
        } else {
            0.0
        };
        let evenness = (entropy / FORMS_POSSIBLE.ln()).clamp(0.0, 1.0);

        let mass = world.mass_residual();
        let water = world.water_residual();
        let energy = (stored_energy(world) - self.opening_energy)
            - (state.net_energy_in_corrected() - self.opening_net_in);
        self.max_mass = self.max_mass.max(mass.abs());
        self.max_water = self.max_water.max(water.abs());
        self.max_energy = self.max_energy.max(energy.abs());
        self.peak_population = self.peak_population.max(population);
        self.min_population = self.min_population.min(population);

        // Maturation is observed, not inferred: a descendant counts once it is seen at full
        // adult structure while alive.
        for (id, o) in state.organisms.iter() {
            if self.born_in_run.contains(&id)
                && !self.matured.contains(&id)
                && o.structure + 1e-9 >= o.phenotype.structure_adult
            {
                self.matured.insert(id);
            }
        }
        if self.first_predation_tick.is_some() {
            self.prey_min_after_predation = self.prey_min_after_predation.min(prey);
        }

        let alive_min = state.config.plant.alive_min;
        let (mut alive_cells, mut establishing_cells, mut bare_cells) = (0u32, 0u32, 0u32);
        for w in &state.ecology.wood {
            match CellClass::of(*w, alive_min) {
                CellClass::Alive => alive_cells += 1,
                CellClass::Establishing => establishing_cells += 1,
                CellClass::Bare => bare_cells += 1,
            }
        }

        self.samples.push(Sample {
            tick: state.tick,
            population,
            prey,
            apex_active: live_apex.difference(&dormant).count() as u32,
            apex_dormant: live_apex.intersection(&dormant).count() as u32,
            producer: state.fields.p.iter().sum(),
            detritus: state.fields.d.iter().sum(),
            fruit: state.fields.f.iter().sum(),
            nutrient: state.fields.n.iter().sum(),
            water: state.fields.w.iter().sum(),
            form_evenness: evenness,
            forms_present: present,
            feeding,
            seeking,
            resting,
            mean_hunger: if population > 0 {
                hunger / f64::from(population)
            } else {
                0.0
            },
            mass_residual: mass,
            water_residual: water,
            energy_residual: energy,

            wood: state.ecology.wood.iter().sum(),
            plant_reserve: state.ecology.plant_reserve.iter().sum(),
            dead_wood: state.ecology.dead_wood.iter().sum(),
            carrion: state.ecology.carrion.iter().sum(),
            carrion_energy: state.ecology.carrion_energy.iter().sum(),

            alive_cells,
            establishing_cells,
            bare_cells,
            depleted_cells: self.crossings.depleted_now(),

            guild,
            forms: form_counts,

            cum_prey_births: self.prey_births,
            cum_prey_births_guild: self.prey_births_guild,
            cum_prey_deaths: self.prey_deaths,
            cum_prey_deaths_guild: self.prey_deaths_guild,
            cum_deaths_cause: self.deaths_by_cause,
            cum_apex: [
                self.apex_attacks,
                self.apex_captures,
                self.apex_births,
                self.apex_deaths,
                self.apex_matings,
                self.apex_emergences,
                self.apex_exhausted,
            ],
            cum_plant_deaths: state.ecology.plant_deaths_total,
            cum_recolonisations: state.ecology.recolonisations_total,
            cum_depletion_events: self.crossings.depletions(),
            cum_recovery_events: self.crossings.recoveries(),
            intake: world.intake_diagnostics(),
        });
    }

    /// Resolve an organism to the founder at the root of its parent chain.
    fn root(&self, mut id: OrganismId) -> OrganismId {
        // The chain is finite: every entry was inserted when the child was born, and a child
        // is always born after its parent, so the walk strictly decreases in birth order.
        for _ in 0..self.parent_of.len() + 1 {
            match self.parent_of.get(&id) {
                Some(parent) => id = *parent,
                None => break,
            }
        }
        id
    }

    fn depth(&self, mut id: OrganismId) -> u32 {
        let mut depth = 0;
        for _ in 0..self.parent_of.len() + 1 {
            match self.parent_of.get(&id) {
                Some(parent) => {
                    depth += 1;
                    id = *parent;
                }
                None => break,
            }
        }
        depth
    }

    fn finish(mut self, world: &mut World) -> (Components, Movement, Option<Vec<OpeningPoint>>) {
        // A final observation, so the terminal state is always in the series.
        if self.samples.last().map(|s| s.tick) != Some(world.tick()) {
            self.sample(world);
        }
        // Everything the ledger still holds: the last closed records, then every body still
        // alive at the horizon, so the margin's denominator is every body and not only the
        // ones that died.
        self.drain_budgets(world);
        if self.options.ledger {
            let now = world.tick();
            let live: Vec<(OrganismId, CensusKey)> = world
                .state
                .organisms
                .iter()
                .filter(|(id, _)| !self.apex_ids.contains(id))
                .filter_map(|(id, _)| self.key_of_id.get(&id).copied().map(|k| (id, k)))
                .collect();
            for (id, key) in live {
                if let Some(budget) = world.body_budget(id) {
                    let budget = *budget;
                    self.margins
                        .add(key, &budget, self.e_r, now, cubarium_core::DT, true);
                }
            }
        }
        let n = self.samples.len().max(1) as f64;
        let mean = |f: fn(&Sample) -> f64| self.samples.iter().map(f).sum::<f64>() / n;

        let ticks_run = world.tick() - self.origin_tick;
        let last = *self.samples.last().expect("at least one sample");
        let capacity = world.config().producer.max * world.state.fields.p.len() as f64;
        let e_p = world.config().plant.energy_density;
        let gross_material = (world.state.light_in_corrected() / e_p).max(0.0);
        let hours = (ticks_run as f64 * cubarium_core::DT / 3600.0).max(1e-9);

        let live: Vec<_> = world.state.organisms.iter().map(|(id, _)| id).collect();
        let lineages: BTreeSet<_> = live
            .iter()
            .filter(|id| !self.apex_ids.contains(id))
            .map(|id| self.root(*id))
            .filter(|root| self.founders.contains(root))
            .collect();
        let max_depth = self
            .born_in_run
            .iter()
            .map(|id| self.depth(*id))
            .max()
            .unwrap_or(0);

        // The ecology v1 windows. The late window is the last `1/WINDOWS` of the **declared**
        // horizon, not of the ticks actually run, so a collapsed world reports `None` rather
        // than a late window measured over its own death throes.
        let late_start = self.origin_tick
            + self
                .protocol
                .horizon_ticks
                .saturating_sub(self.protocol.horizon_ticks / WINDOWS);
        let eco = EcoMeasures::from_samples(&self.samples).unwrap_or_default();
        let late_slice: Vec<Sample> = self
            .samples
            .iter()
            .copied()
            .filter(|s| s.tick >= late_start)
            .collect();
        let late = (late_slice.len() >= 2)
            .then(|| EcoMeasures::from_samples(&late_slice))
            .flatten();

        // The spatial measures. `whole_run` pools every window's bodies, weighted equally per
        // body-window — the weighting A's `cells_per_body_window` already uses — and `late` is
        // the last closed window, the same interval as the late window above.
        let mut pooled = SpatialAccumulator::default();
        for w in &self.spatial_windows {
            pooled.merge(w);
        }
        let dt = cubarium_core::DT;
        let per_window: Vec<_> = self.spatial_windows.iter().map(|w| w.finish(dt)).collect();
        let whole_run = pooled.finish(dt);
        let late_spatial = per_window.last().copied();
        let cells_per_body_window = whole_run.cells_per_body;
        let cells_per_body_window_late = late_spatial.map_or(0.0, |s| s.cells_per_body);
        let late_by_form: Vec<FormSpatial> = self
            .spatial_by_form
            .last()
            .map(|m| {
                m.iter()
                    .map(|(form, acc)| FormSpatial {
                        form: *form,
                        spatial: acc.finish(dt),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let alive_keys: Vec<CensusKey> = world
            .state
            .organisms
            .iter()
            .filter(|(id, _)| !self.apex_ids.contains(id))
            .map(|(id, o)| {
                self.key_of_id
                    .get(&id)
                    .copied()
                    .unwrap_or_else(|| census_key(o))
            })
            .collect();
        let margins = self.margins.clone().finish(self.options.ledger);
        let depletion = self.depletion.clone().finish(&world.state.fields.p);
        let plant_budget = self.plant.take().map(|plant| {
            let residual = world.plant_budget_residual();
            let (samples, ticks) = match world.plant_budget() {
                Some(rec) => (
                    rec.cells
                        .iter()
                        .map(|c| CellSample {
                            foliage_in: c.foliage_in(),
                            foliage_out: c.foliage_out(),
                            withdrawal: c.withdrawal_foliage,
                            income: c.income,
                            maintenance_unpaid: c.maintenance_unpaid,
                            death_foliage: c.death_foliage,
                            wood_sum: c.w_sum,
                            light_sum: c.light_effective_sum,
                            nutrient_sum: c.nutrient_sum,
                            ticks_alive: c.ticks_alive,
                        })
                        .collect::<Vec<_>>(),
                    rec.ticks,
                ),
                None => (Vec::new(), 0),
            };
            plant.finish(&samples, &world.state.fields.p, residual, ticks)
        });
        let movement = Movement {
            move_cost: self.move_cost,
            probe_ticks: PROBE_EVERY,
            windows: per_window,
            late: late_spatial,
            whole_run,
            late_by_form,
            crossings: self.crossings.summary(),
            census: self.census.clone().finish(&alive_keys, dt),
            terminal_by_cause: self.terminal_by_cause,
            // Workstream E's per-organism budget accumulator is what makes this a number
            // rather than a `null`: F's rows carry `null` because it had not landed when F ran.
            net_energy_margin_per_body: self.options.ledger.then_some(margins.margin_mean),
            ledger_on: self.options.ledger,
            founder_broods: self.broods,
            margins: self.options.ledger.then_some(margins),
            depletion: Some(depletion),
            plant_budget,
        };

        let apex_active_samples = self.samples.iter().filter(|s| s.apex_active > 0).count() as f64;
        let recovery =
            if self.first_predation_tick.is_some() && self.prey_min_after_predation < u32::MAX {
                f64::from(last.prey) / f64::from(self.prey_min_after_predation.max(1))
            } else {
                0.0
            };

        let components = Components {
            horizon_ticks: self.protocol.horizon_ticks,
            ticks_run,
            survived_ticks: self.collapsed_at.unwrap_or(ticks_run),
            collapsed: self.collapsed_at.is_some(),
            final_population: last.population,
            final_prey: last.prey,
            peak_population: self.peak_population,
            mean_population: mean(|s| f64::from(s.population)),
            min_population: self.min_population,

            mean_producer: mean(|s| s.producer),
            min_producer: self
                .samples
                .iter()
                .map(|s| s.producer)
                .fold(f64::INFINITY, f64::min),
            final_producer: last.producer,
            mean_detritus: mean(|s| s.detritus),
            mean_nutrient: mean(|s| s.nutrient),
            mean_fruit: mean(|s| s.fruit),
            gross_production_per_hour: gross_material / hours,
            producer_capacity: capacity,

            prey_births: self.prey_births,
            prey_deaths: self.prey_deaths,
            deaths_starvation: self.deaths_by_cause[0],
            deaths_age: self.deaths_by_cause[1],
            deaths_collapse: self.deaths_by_cause[2],
            deaths_predation: self.deaths_by_cause[3],
            descendants_matured: self.matured.len() as u64,
            descendants_reproduced: self.reproduced.len() as u64,
            max_lineage_depth: max_depth,
            founder_lineages_alive: lineages.len() as u32,
            distinct_parents: self.parents_seen.len() as u64,

            mean_form_evenness: mean(|s| s.form_evenness),
            min_forms_present: self
                .samples
                .iter()
                .map(|s| s.forms_present)
                .min()
                .unwrap_or(0),
            final_forms_present: last.forms_present,

            feeding_fraction: mean(|s| {
                if s.population > 0 {
                    f64::from(s.feeding) / f64::from(s.population)
                } else {
                    0.0
                }
            }),
            seeking_fraction: mean(|s| {
                if s.population > 0 {
                    f64::from(s.seeking) / f64::from(s.population)
                } else {
                    0.0
                }
            }),
            resting_fraction: mean(|s| {
                if s.population > 0 {
                    f64::from(s.resting) / f64::from(s.population)
                } else {
                    0.0
                }
            }),
            mean_hunger: mean(|s| s.mean_hunger),

            apex_introduced: self.apex_introduced,
            apex_active_mean: mean(|s| f64::from(s.apex_active)),
            apex_dormant_mean: mean(|s| f64::from(s.apex_dormant)),
            apex_active_sample_fraction: apex_active_samples / n,
            apex_alive_final: last.apex_active + last.apex_dormant,
            apex_attacks: self.apex_attacks,
            apex_captures: self.apex_captures,
            apex_births: self.apex_births,
            apex_deaths: self.apex_deaths,
            apex_matings: self.apex_matings,
            apex_emergences: self.apex_emergences,
            apex_exhausted: self.apex_exhausted,

            predation_occurred: self.first_predation_tick.is_some(),
            prey_before_first_predation: self.prey_before_first_predation,
            prey_min_after_first_predation: if self.prey_min_after_predation == u32::MAX {
                0
            } else {
                self.prey_min_after_predation
            },
            prey_recovery_ratio: recovery,

            max_abs_mass_residual: self.max_mass,
            max_abs_water_residual: self.max_water,
            max_abs_energy_residual: self.max_energy,

            final_ecology_hash: cubarium_core::ecology_hash(&world.state),
            final_state_hash: cubarium_core::snapshot::state_hash(&world.state),

            eco,
            late,
            opening_foliage: self.opening_foliage,
            opening_wood: self.opening_wood,
            opening_litter: self.opening_litter,
            opening_alive_cells: self.opening_alive_cells,
            depletion_cells_watched: self.watched_cells,
            cells_per_body_window,
            cells_per_body_window_late,
            spatial_windows_observed: self.spatial_windows.len() as u64,
            spatial_bodies_observed: self.spatial_bodies,
        };
        let opening = self.options.precondition.is_some().then_some(self.opening);
        (components, movement, opening)
    }
}

/// What a body is, for the census: its visual form, its diet bin and its guild, all decoded
/// from the phenotype the world itself built and none of them revised afterwards.
fn census_key(o: &cubarium_core::organism::Organism) -> CensusKey {
    CensusKey {
        form: o.phenotype.form.min(4),
        diet_bin: diet_bin(o.phenotype.diet) as u8,
        guild: guild_of(o.phenotype.cap_foliage, o.phenotype.cap_detrital) as u8,
    }
}
