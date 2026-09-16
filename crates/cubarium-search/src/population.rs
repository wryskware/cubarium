//! The **population-level** comparison: a trained forager put into a whole calibrated world
//! beside the ordinary founders, against the same world with ordinary founders in its place.
//!
//! A held-out layout says whether one policy keeps one body alive on a painted patch. It says
//! nothing about what a population of that policy does to a world that has to feed it, breed
//! it and be eaten by it. This module runs the second question, as a **matched pair**:
//!
//! - the *neural* arm: the calibrated world with its own founders, plus `N` copies of one
//!   exported policy founded at tick 0 through [`World::found_neural_animal`];
//! - the *legacy* arm: the same world, same seed, same tick, plus `N` copies of the **same
//!   body** founded through [`World::found_training_animal`] and left on the ordinary
//!   controller.
//!
//! Both arms therefore import exactly the same material, the same genotype and the same
//! population; the only difference between them is which controller drives four bodies. Every
//! other condition — reproduction, mutation, the apex arms and their introduction tick, the
//! horizon — is the ecology v1 calibration screen's, unchanged, so the numbers here sit beside
//! that screen's rather than beneath it.
//!
//! # Provenance is read, never assumed
//!
//! Every body's controller comes from the world: a body is neural exactly when
//! `World::neural()` holds an entry for its full id. That is read at tick 0 for the founders
//! and the copies, and again for every child the moment its birth event is drained, so the
//! answer to "does a neural parent have a neural child?" is a measurement of this build, not a
//! reading of a document. The counts are reported by controller either way; if offspring turn
//! out to be legacy-controlled, the arm is a *mixed* population and this module says so in the
//! same row rather than calling it a neural population.
//!
//! # What is measured per controller, and what is not
//!
//! Alive-over-time, births, deaths by cause, travel and the spatial range are per body, so
//! they are reported per controller exactly. **Intake by food is not**: the core's
//! `IntakeDiagnostics` is world-scoped — one set of sums for the whole world — and there is no
//! per-organism intake accessor. Adding one would be a change to the core this brief excludes.
//! So intake by food is reported **per arm**, and the controller's effect on food use is the
//! difference between the matched pair, which is the population-level quantity the comparison
//! is actually about. The limitation is stated here, in the row, and in the result note.

use cubarium_surface::{Scale, Topology};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_core::config::WorldConfig;
use cubarium_core::hunter::FixedHunterProfile;
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::DeathCause;
use cubarium_core::{LifeEvent, World};
use cubarium_surface::{CellId, Face, Vec2, cell_of};
use serde::{Deserialize, Serialize};

use crate::es::export::PolicyFile;
use crate::es::fixture::Ecology;
use crate::evaluate::{
    BUILD_ID, DEPLETION_FRACTION, PROBE_EVERY, RECOVERY_FRACTION, WINDOWS, apex_targets,
    base_config,
};

/// How often the world is validated inside a trial: 18 checks over a 180,000-tick horizon,
/// the same cadence the ES episode uses.
const VALIDATE_EVERY: u64 = 10_000;

/// Which controller drives the imported copies in this arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mix {
    /// `N` copies through `found_neural_animal`, carrying the exported policy.
    Neural,
    /// `N` copies of the same body through `found_training_animal`, on the ordinary
    /// controller. The matched control.
    Legacy,
}

impl Mix {
    pub fn label(self) -> &'static str {
        match self {
            Mix::Neural => "neural",
            Mix::Legacy => "legacy",
        }
    }
}

/// A count split by the controller the body is actually on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByController {
    pub neural: u64,
    pub legacy: u64,
}

impl ByController {
    fn add(&mut self, neural: bool, n: u64) {
        if neural {
            self.neural += n;
        } else {
            self.legacy += n;
        }
    }

    pub fn total(&self) -> u64 {
        self.neural + self.legacy
    }
}

/// A mean split by controller, with the denominators kept so an empty side is visibly empty
/// rather than zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeanByController {
    pub neural: f64,
    pub neural_n: u64,
    pub legacy: f64,
    pub legacy_n: u64,
}

impl MeanByController {
    fn of(sums: (f64, u64), other: (f64, u64)) -> MeanByController {
        MeanByController {
            neural: if sums.1 > 0 { sums.0 / sums.1 as f64 } else { 0.0 },
            neural_n: sums.1,
            legacy: if other.1 > 0 { other.0 / other.1 as f64 } else { 0.0 },
            legacy_n: other.1,
        }
    }
}

/// The pursuit stopping rule a report written before 2026-09-16 implies: the forward
/// half-space, which was the only rule this workspace had. Named rather than taken from
/// `PursuitStop::default()` so that adopting a new shipped rule cannot relabel a retained
/// report (`design/7_Research/ecology-v1-predicate-adoption-2026-09-16.md`).
fn half_space_name() -> String {
    cubarium_core::hunter::PursuitStop::ForwardHalfSpace.as_str().to_string()
}

/// Everything one population comparison ran under.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PopulationPlan {
    pub build_id: String,
    pub config: String,
    pub config_hash: String,
    pub policy_file: String,
    pub policy_generation: u64,
    pub policy_digest: String,
    pub protocol_hash: String,
    pub copies: usize,
    pub horizon_ticks: u64,
    pub sample_every: u64,
    pub apex_introduce_tick: u64,
    /// The pursuit stopping rule every world of this comparison ran
    /// (`cubarium_core::hunter::PursuitStop`), by name.
    ///
    /// **This report depends on it.** Every arm above zero introduces an apex cohort at
    /// `apex_introduce_tick`, so the rule the hunt-intent pass evaluates is a variable of the
    /// experiment, not decoration — which is why it is recorded here, once, for the whole
    /// report: one report, one rule.
    ///
    /// Absent on every report written before this field existed, and what that absence means is
    /// **`forward_half_space`** — the rule this workspace shipped until 2026-09-16, which is
    /// what those runs in fact evaluated. Deliberately not `PursuitStop::default()`, which is
    /// the envelope now: reading a retained report under today's default would relabel a
    /// comparison it never ran.
    #[serde(default = "half_space_name")]
    pub pursuit_stop: String,
    pub seeds: Vec<u64>,
    pub arms: Vec<u32>,
    pub mixes: Vec<String>,
    pub trials: usize,
    pub workers: usize,
    pub wall_seconds_cap: u64,
}

/// What one `(seed, arm, mix)` world produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PopulationRow {
    pub seed: u64,
    pub arm: u32,
    pub mix: Mix,
    pub status: String,
    pub reason: Option<String>,
    pub elapsed_ms: u64,
    pub ticks_run: u64,
    pub collapsed_at: Option<u64>,

    /// What was imported at tick 0 and how the world classified it.
    pub copies_founded: usize,
    pub copies_neural_at_tick0: usize,
    pub founders_at_tick0: usize,
    pub founders_neural_at_tick0: usize,
    pub imported_material: f64,

    /// Non-apex bodies alive at each sample, and the neural subset of them.
    pub population_series: Vec<u32>,
    pub neural_series: Vec<u32>,
    pub final_population: u32,
    pub final_neural: u32,
    pub peak_neural: u32,
    pub neural_extinct_at: Option<u64>,

    pub births: ByController,
    pub deaths: ByController,
    /// Deaths keyed by `DeathCause`, each split by controller.
    pub deaths_by_cause: BTreeMap<String, ByController>,

    /// **The provenance finding.** Births whose parent this run recorded as neural, and how
    /// many of those children the world itself made neural.
    pub births_of_neural_parents: u64,
    pub neural_children_of_neural_parents: u64,

    /// Mean pixels travelled per body over its whole life, and in body lengths.
    pub travel_px_per_body: MeanByController,
    pub body_lengths_per_body: MeanByController,
    /// Mean distinct cells a body stood in inside one closed window.
    pub distinct_cells_per_body_window: MeanByController,
    pub windows_closed: u64,

    /// **The imported copies themselves**, the four bodies the two arms actually differ in.
    /// The whole-world columns above mix them with twenty-four founders they share; these
    /// compare the same body under the two controllers, one for one.
    pub copy_lifetimes_ticks: Vec<u64>,
    pub copy_alive_final: usize,
    pub copy_deaths_by_cause: BTreeMap<String, u64>,
    /// Children born to an imported copy.
    pub copy_births: u64,
    pub copy_travel_px_mean: f64,
    pub copy_body_lengths_mean: f64,
    pub copy_distinct_cells_per_window: f64,

    /// World-scoped: the core exposes no per-organism intake (see the module header).
    pub intake_producer: f64,
    pub intake_fruit: f64,
    pub intake_litter: f64,
    pub intake_carrion: f64,
    pub producer_growth: f64,

    pub opening_foliage: f64,
    pub late_foliage_mean: f64,
    pub foliage_retention: f64,
    pub depletion_events: u64,
    pub recovery_events: u64,
    pub watched_cells: u32,

    pub apex_introduced: u32,
    pub apex_alive_final: usize,
    pub apex_births: u64,
    pub apex_deaths: u64,
    pub predation_deaths: u64,

    pub max_abs_mass_residual: f64,
    pub max_abs_energy_residual: f64,
}

/// One `(seed, arm)` cell: the neural arm beside its matched legacy arm.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArmSummary {
    pub arm: u32,
    pub seeds: Vec<u64>,
    pub completed_neural: usize,
    pub completed_legacy: usize,
    /// Means over the seeds, neural arm then legacy arm.
    pub final_population: (f64, f64),
    pub final_neural_bodies: f64,
    pub peak_neural_bodies: f64,
    pub births: (f64, f64),
    pub deaths: (f64, f64),
    pub intake_producer: (f64, f64),
    pub intake_detrital: (f64, f64),
    pub foliage_retention: (f64, f64),
    pub depletion_events: (f64, f64),
    pub apex_alive_final: (f64, f64),
    pub predation_deaths: (f64, f64),
    /// The four imported copies, neural arm then legacy arm.
    pub copy_lifetime_ticks: (f64, f64),
    pub copy_alive_final: (f64, f64),
    pub copy_births: (f64, f64),
    pub copy_distinct_cells: (f64, f64),
    pub copy_body_lengths: (f64, f64),
    /// Within the neural arm, per-controller means over the seeds.
    pub neural_arm_distinct_cells: MeanByController,
    pub neural_arm_body_lengths: MeanByController,
    pub neural_arm_births: ByController,
    pub neural_arm_deaths: ByController,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PopulationReport {
    pub plan: PopulationPlan,
    pub wall_seconds: f64,
    pub trials_run: usize,
    pub trials_skipped: usize,
    pub ticks_simulated: u64,
    pub ticks_per_second: f64,
    /// True when every neural arm's offspring of neural parents were themselves neural.
    pub offspring_inherit_the_policy: Option<bool>,
    pub arms: Vec<ArmSummary>,
}

/// Where the imported copies stand at tick 0: deterministic in the seed, on distinct faces,
/// away from the seams, and **identical for both mixes** so the pair really is matched.
///
/// The draws use the apex placement stream under keys no apex cohort can reach (`1000 + i`
/// against the cohort's `0..=1`), so a copy placement and an apex placement never consume each
/// other's randomness however many adults an arm introduces.
fn copy_placements(seed: u64, count: usize) -> Vec<(CellId, Vec2)> {
    use crate::rng::{draw, stream, unit};
    (0..count as u64)
        .map(|i| {
            let key = 1_000 + i;
            let face = (draw(seed, stream::APEX_PLACEMENT, key, 0) % 5) as u8;
            // Cells 2..=13 on both axes: inside the face, never on a seam row.
            let x = 2 + (unit(seed, stream::APEX_PLACEMENT, key, 1) * 12.0) as u16;
            let y = 2 + (unit(seed, stream::APEX_PLACEMENT, key, 2) * 12.0) as u16;
            let angle = std::f64::consts::TAU * unit(seed, stream::APEX_PLACEMENT, key, 3);
            (
                CellId::new(Topology::Cube, Scale::ONE, Face::from_index(face).unwrap_or(Face::Front), x.min(13), y.min(13)),
                Vec2::new(angle.cos(), angle.sin()),
            )
        })
        .collect()
}

/// The recorder: one world's whole history, kept per controller wherever the core lets it be.
struct Recorder {
    /// Provenance, read from the world and never revised: `true` when the world held a neural
    /// entry for that body at the moment it was first seen.
    neural_of: BTreeMap<OrganismId, bool>,
    apex_ids: BTreeSet<OrganismId>,
    extent_of: BTreeMap<OrganismId, f64>,

    population_series: Vec<u32>,
    neural_series: Vec<u32>,
    peak_neural: u32,
    neural_extinct_at: Option<u64>,

    births: ByController,
    deaths: ByController,
    deaths_by_cause: BTreeMap<String, ByController>,
    births_of_neural_parents: u64,
    neural_children_of_neural_parents: u64,
    predation_deaths: u64,
    apex_births: u64,
    apex_deaths: u64,

    travel_px: BTreeMap<OrganismId, f64>,

    /// The bodies the arm imported at tick 0, and what became of each.
    imported: BTreeSet<OrganismId>,
    copy_death_tick: BTreeMap<OrganismId, u64>,
    copy_deaths_by_cause: BTreeMap<String, u64>,
    copy_births: u64,
    copy_cells: (f64, u64),

    p_ref: Vec<f64>,
    depleted: Vec<bool>,
    depletion_events: u64,
    recovery_events: u64,
    opening_foliage: f64,
    watched_cells: u32,

    /// Distinct cells per body inside the current window, and the closed windows' sums.
    visited: BTreeMap<OrganismId, BTreeSet<u16>>,
    window_ticks: u64,
    window_index: u64,
    windows_closed: u64,
    cells_neural: (f64, u64),
    cells_legacy: (f64, u64),

    /// The late window's foliage samples, for retention.
    late_from_tick: u64,
    late_foliage: Vec<f64>,

    max_mass: f64,
    max_energy: f64,
    opening_energy: f64,
    opening_net_in: f64,
    collapsed_at: Option<u64>,
}

impl Recorder {
    fn new(world: &World, horizon: u64, imported: BTreeSet<OrganismId>) -> Recorder {
        let apex_ids: BTreeSet<_> = world.hunters().members.iter().map(|m| m.id).collect();
        let mut neural_of = BTreeMap::new();
        let mut extent_of = BTreeMap::new();
        for (id, o) in world.state.organisms.iter() {
            if apex_ids.contains(&id) {
                continue;
            }
            // Read from the world, not from how the body was founded.
            neural_of.insert(id, world.neural().contains(id));
            extent_of.insert(id, o.phenotype.extent);
        }
        let p_ref: Vec<f64> = world.state.fields.p.clone();
        let watched_cells = p_ref.iter().filter(|p| **p > 1e-9).count() as u32;
        let window_ticks = (horizon / WINDOWS).max(1);
        Recorder {
            neural_of,
            apex_ids,
            extent_of,
            population_series: Vec::new(),
            neural_series: Vec::new(),
            peak_neural: 0,
            neural_extinct_at: None,
            births: ByController::default(),
            deaths: ByController::default(),
            deaths_by_cause: BTreeMap::new(),
            births_of_neural_parents: 0,
            neural_children_of_neural_parents: 0,
            predation_deaths: 0,
            apex_births: 0,
            apex_deaths: 0,
            travel_px: BTreeMap::new(),
            imported,
            copy_death_tick: BTreeMap::new(),
            copy_deaths_by_cause: BTreeMap::new(),
            copy_births: 0,
            copy_cells: (0.0, 0),
            opening_foliage: p_ref.iter().sum(),
            depleted: vec![false; p_ref.len()],
            p_ref,
            depletion_events: 0,
            recovery_events: 0,
            watched_cells,
            visited: BTreeMap::new(),
            window_ticks,
            window_index: 0,
            windows_closed: 0,
            cells_neural: (0.0, 0),
            cells_legacy: (0.0, 0),
            late_from_tick: horizon.saturating_sub(window_ticks),
            late_foliage: Vec::new(),
            max_mass: 0.0,
            max_energy: 0.0,
            opening_energy: crate::evaluate::stored_energy(world),
            opening_net_in: world.state.net_energy_in_corrected(),
            collapsed_at: None,
        }
    }

    /// Per tick: the travel every body actually resolved, then the drained events.
    fn absorb(&mut self, world: &mut World) {
        for m in &world.hunters().members {
            self.apex_ids.insert(m.id);
        }
        for (id, _) in world.state.organisms.iter() {
            if self.apex_ids.contains(&id) {
                continue;
            }
            let moved: f64 = world
                .moved_segments(id)
                .iter()
                .map(cubarium_surface::PathSegment::length)
                .sum();
            if moved > 0.0 {
                *self.travel_px.entry(id).or_insert(0.0) += moved;
            }
        }
        for event in world.drain_hunter_events() {
            if let cubarium_core::hunter::HunterEvent::Offspring { child, .. } = event {
                self.apex_ids.insert(child);
            }
        }
        for event in world.drain_apex_encounter_events() {
            if let cubarium_core::encounter::ApexEncounterEvent::Born { child, .. } = event {
                self.apex_ids.insert(child);
            }
        }
        world.drain_apex_dormancy_events();
        world.drain_quiet_events();

        for event in world.drain_events() {
            match event {
                LifeEvent::Birth { id, parent, .. } => {
                    if self.apex_ids.contains(&id) {
                        self.apex_births += 1;
                        continue;
                    }
                    // The world decides, here and nowhere else, whether this child is neural.
                    let child_neural = world.neural().contains(id);
                    let parent_neural = self.neural_of.get(&parent).copied().unwrap_or(false);
                    if parent_neural {
                        self.births_of_neural_parents += 1;
                        if child_neural {
                            self.neural_children_of_neural_parents += 1;
                        }
                    }
                    if self.imported.contains(&parent) {
                        self.copy_births += 1;
                    }
                    self.neural_of.insert(id, child_neural);
                    if let Some(o) = world.state.organisms.get(id) {
                        self.extent_of.insert(id, o.phenotype.extent);
                    }
                    self.births.add(child_neural, 1);
                }
                LifeEvent::Death { id, cause, .. } => {
                    if cause == DeathCause::Predation {
                        self.predation_deaths += 1;
                    }
                    if self.apex_ids.contains(&id) {
                        self.apex_deaths += 1;
                        continue;
                    }
                    // The neural entry is removed with the body, so the controller comes from
                    // the record made when the body was first seen.
                    let was_neural = self.neural_of.get(&id).copied().unwrap_or(false);
                    if self.imported.contains(&id) {
                        self.copy_death_tick.insert(id, world.tick());
                        *self
                            .copy_deaths_by_cause
                            .entry(format!("{cause:?}").to_lowercase())
                            .or_default() += 1;
                    }
                    self.deaths.add(was_neural, 1);
                    self.deaths_by_cause
                        .entry(format!("{cause:?}").to_lowercase())
                        .or_default()
                        .add(was_neural, 1);
                }
            }
        }

        if world.tick().is_multiple_of(PROBE_EVERY) {
            self.probe(world);
        }
        let index = world.tick() / self.window_ticks;
        if index > self.window_index {
            self.close_window();
            self.window_index = index;
        }
    }

    fn probe(&mut self, world: &World) {
        let p = &world.state.fields.p;
        for i in 0..self.p_ref.len() {
            let reference = self.p_ref[i];
            if reference <= 1e-9 {
                continue;
            }
            let now = p[i];
            if self.depleted[i] {
                if now > RECOVERY_FRACTION * reference {
                    self.depleted[i] = false;
                    self.recovery_events += 1;
                }
            } else if now < DEPLETION_FRACTION * reference {
                self.depleted[i] = true;
                self.depletion_events += 1;
            }
        }
        for (id, o) in world.state.organisms.iter() {
            if self.apex_ids.contains(&id) {
                continue;
            }
            self.visited.entry(id).or_default().insert(cell_of(Topology::Cube, Scale::ONE, &o.pos).index() as u16);
        }
    }

    fn close_window(&mut self) {
        let visited = std::mem::take(&mut self.visited);
        for (id, cells) in visited {
            let neural = self.neural_of.get(&id).copied().unwrap_or(false);
            let slot = if neural { &mut self.cells_neural } else { &mut self.cells_legacy };
            slot.0 += cells.len() as f64;
            slot.1 += 1;
            if self.imported.contains(&id) {
                self.copy_cells.0 += cells.len() as f64;
                self.copy_cells.1 += 1;
            }
        }
        self.windows_closed += 1;
    }

    fn sample(&mut self, world: &World) {
        let mut population = 0u32;
        for (id, _) in world.state.organisms.iter() {
            if !self.apex_ids.contains(&id) {
                population += 1;
            }
        }
        let neural = world.neural().animals.len() as u32;
        self.population_series.push(population);
        self.neural_series.push(neural);
        self.peak_neural = self.peak_neural.max(neural);
        if neural == 0 && self.neural_extinct_at.is_none() && world.tick() > 0 {
            self.neural_extinct_at = Some(world.tick());
        }
        if world.tick() >= self.late_from_tick {
            self.late_foliage.push(world.state.fields.p.iter().sum::<f64>());
        }
        self.max_mass = self.max_mass.max(world.mass_residual().abs());
        let energy = (crate::evaluate::stored_energy(world) - self.opening_energy)
            - (world.state.net_energy_in_corrected() - self.opening_net_in);
        self.max_energy = self.max_energy.max(energy.abs());
    }
}

/// Run one `(seed, arm, mix)` world. Never panics: a panic inside the core is caught and
/// recorded, the way the calibration harness records one.
#[allow(clippy::too_many_arguments)]
fn trial(
    ecology: &Ecology,
    policy: &cubarium_core::neural::Policy,
    seed: u64,
    arm: u32,
    mix: Mix,
    copies: usize,
    horizon: u64,
    sample_every: u64,
    introduce_tick: u64,
    pursuit_stop: cubarium_core::hunter::PursuitStop,
) -> PopulationRow {
    let start = Instant::now();
    let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_trial(
            ecology, policy, seed, arm, mix, copies, horizon, sample_every, introduce_tick,
            pursuit_stop, start,
        )
    }));
    match done {
        Ok(row) => row,
        Err(payload) => {
            let what = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_string());
            refused(seed, arm, mix, "failed", format!("simulation panicked: {what}"), start)
        }
    }
}

fn refused(
    seed: u64,
    arm: u32,
    mix: Mix,
    status: &str,
    reason: String,
    start: Instant,
) -> PopulationRow {
    PopulationRow {
        seed,
        arm,
        mix,
        status: status.to_string(),
        reason: Some(reason),
        elapsed_ms: start.elapsed().as_millis() as u64,
        ticks_run: 0,
        collapsed_at: None,
        copies_founded: 0,
        copies_neural_at_tick0: 0,
        founders_at_tick0: 0,
        founders_neural_at_tick0: 0,
        imported_material: 0.0,
        population_series: Vec::new(),
        neural_series: Vec::new(),
        final_population: 0,
        final_neural: 0,
        peak_neural: 0,
        neural_extinct_at: None,
        births: ByController::default(),
        deaths: ByController::default(),
        deaths_by_cause: BTreeMap::new(),
        births_of_neural_parents: 0,
        neural_children_of_neural_parents: 0,
        travel_px_per_body: MeanByController::default(),
        body_lengths_per_body: MeanByController::default(),
        distinct_cells_per_body_window: MeanByController::default(),
        windows_closed: 0,
        copy_lifetimes_ticks: Vec::new(),
        copy_alive_final: 0,
        copy_deaths_by_cause: BTreeMap::new(),
        copy_births: 0,
        copy_travel_px_mean: 0.0,
        copy_body_lengths_mean: 0.0,
        copy_distinct_cells_per_window: 0.0,
        intake_producer: 0.0,
        intake_fruit: 0.0,
        intake_litter: 0.0,
        intake_carrion: 0.0,
        producer_growth: 0.0,
        opening_foliage: 0.0,
        late_foliage_mean: 0.0,
        foliage_retention: 0.0,
        depletion_events: 0,
        recovery_events: 0,
        watched_cells: 0,
        apex_introduced: 0,
        apex_alive_final: 0,
        apex_births: 0,
        apex_deaths: 0,
        predation_deaths: 0,
        max_abs_mass_residual: 0.0,
        max_abs_energy_residual: 0.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_trial(
    ecology: &Ecology,
    policy: &cubarium_core::neural::Policy,
    seed: u64,
    arm: u32,
    mix: Mix,
    copies: usize,
    horizon: u64,
    sample_every: u64,
    introduce_tick: u64,
    pursuit_stop: cubarium_core::hunter::PursuitStop,
    start: Instant,
) -> PopulationRow {
    // The calibrated ecology at this seed. Nothing else about it moves: the 24 founders of
    // `founders.kinds`, reproduction and mutation are the configuration's own.
    let mut config: WorldConfig = (*ecology.base).clone();
    config.seed = seed;
    config.capacity.event_log = false;
    if let Err(e) = config.validate() {
        return refused(seed, arm, mix, "invalid", format!("config rejected: {e}"), start);
    }
    // The apex profile is derived from the **base** config exactly as `evaluate::run` derives
    // it, so the predator in these arms is bit-for-bit the one the calibration screen used.
    let profile = FixedHunterProfile::lanternjaw_trial(&base_config(seed));
    if let Err(e) = profile.validate() {
        return refused(seed, arm, mix, "invalid", format!("hunter profile rejected: {e}"), start);
    }

    let mut world = match World::new(config) {
        Ok(w) => w,
        Err(e) => {
            return refused(seed, arm, mix, "invalid", format!("world creation refused: {e}"), start);
        }
    };
    // The pursuit stopping rule, before the first tick. A transient on the world, so the
    // config and its hash are untouched; the shipped rule is the reach envelope and naming it
    // changes nothing. It reaches the apex arms of this comparison and nothing else.
    world.set_pursuit_stop(pursuit_stop);
    let founders_at_tick0 = world.population();
    let material_before = world.state.external_material_in;

    // The imported copies: same body, same places, same tick in both arms.
    let mut copies_founded = 0usize;
    let mut imported: BTreeSet<OrganismId> = BTreeSet::new();
    for (cell, heading) in copy_placements(seed, copies) {
        let pos = cell.center(Topology::Cube, Scale::ONE);
        let founded = match mix {
            Mix::Neural => world.found_neural_animal(pos, heading, policy.clone()),
            Mix::Legacy => world.found_training_animal(pos, heading),
        };
        match founded {
            Ok(id) => {
                imported.insert(id);
                copies_founded += 1;
            }
            Err(e) => {
                return refused(
                    seed,
                    arm,
                    mix,
                    "invalid",
                    format!("founding copy {copies_founded} refused: {e}"),
                    start,
                );
            }
        }
    }
    let imported_material = world.state.external_material_in - material_before;
    let copies_neural_at_tick0 = world.neural().animals.len();
    let founders_neural_at_tick0 = {
        // How many of the *original* founders the world made neural: zero, by construction,
        // and measured rather than asserted.
        copies_neural_at_tick0.saturating_sub(match mix {
            Mix::Neural => copies_founded,
            Mix::Legacy => 0,
        })
    };

    let mut apex_introduced = 0u32;
    if arm > 0 && introduce_tick == 0 {
        match introduce(&mut world, &profile, seed, arm) {
            Ok(n) => apex_introduced = n,
            Err(e) => return refused(seed, arm, mix, "invalid", e, start),
        }
    }

    let mut rec = Recorder::new(&world, horizon, imported.clone());
    world.drain_events();
    world.drain_hunter_events();
    world.drain_apex_dormancy_events();
    world.drain_apex_encounter_events();
    world.drain_quiet_events();
    rec.sample(&world);

    let mut ticks_run = 0u64;
    for _ in 0..horizon {
        world.step();
        ticks_run = world.tick();
        if arm > 0 && apex_introduced == 0 && world.tick() == introduce_tick {
            match introduce(&mut world, &profile, seed, arm) {
                Ok(n) => apex_introduced = n,
                Err(e) => return refused(seed, arm, mix, "invalid", e, start),
            }
        }
        rec.absorb(&mut world);
        if world.tick().is_multiple_of(VALIDATE_EVERY)
            && let Err(e) = world.check_invariants()
        {
            return refused(
                seed,
                arm,
                mix,
                "failed",
                format!("invariant violated at tick {}: {e}", world.tick()),
                start,
            );
        }
        if world.tick().is_multiple_of(sample_every) {
            rec.sample(&world);
        }
        if world.population() == 0 {
            rec.collapsed_at = Some(world.tick());
            break;
        }
    }
    rec.sample(&world);

    let live_apex: BTreeSet<_> = world
        .hunters()
        .members
        .iter()
        .map(|m| m.id)
        .filter(|id| world.state.organisms.get(*id).is_some())
        .collect();
    let final_population =
        world.state.organisms.iter().filter(|(id, _)| !live_apex.contains(id)).count() as u32;

    let mut travel = ((0.0f64, 0u64), (0.0f64, 0u64));
    let mut lengths = ((0.0f64, 0u64), (0.0f64, 0u64));
    for (id, px) in &rec.travel_px {
        let neural = rec.neural_of.get(id).copied().unwrap_or(false);
        let slot = if neural { &mut travel.0 } else { &mut travel.1 };
        slot.0 += px;
        slot.1 += 1;
        let extent = rec.extent_of.get(id).copied().unwrap_or(0.0);
        if extent > 0.0 {
            let slot = if neural { &mut lengths.0 } else { &mut lengths.1 };
            slot.0 += px / extent;
            slot.1 += 1;
        }
    }

    // The imported copies, one for one.
    let mut copy_lifetimes_ticks = Vec::new();
    let mut copy_alive_final = 0usize;
    let mut copy_travel = (0.0f64, 0u64);
    let mut copy_lengths = (0.0f64, 0u64);
    for id in &imported {
        let lifetime = match rec.copy_death_tick.get(id) {
            Some(t) => *t,
            None => {
                copy_alive_final += 1;
                ticks_run
            }
        };
        copy_lifetimes_ticks.push(lifetime);
        let px = rec.travel_px.get(id).copied().unwrap_or(0.0);
        copy_travel.0 += px;
        copy_travel.1 += 1;
        let extent = rec.extent_of.get(id).copied().unwrap_or(0.0);
        if extent > 0.0 {
            copy_lengths.0 += px / extent;
            copy_lengths.1 += 1;
        }
    }

    let diag = world.intake_diagnostics();
    let late_foliage_mean = if rec.late_foliage.is_empty() {
        0.0
    } else {
        rec.late_foliage.iter().sum::<f64>() / rec.late_foliage.len() as f64
    };

    PopulationRow {
        seed,
        arm,
        mix,
        status: "completed".into(),
        reason: None,
        elapsed_ms: start.elapsed().as_millis() as u64,
        ticks_run,
        collapsed_at: rec.collapsed_at,
        copies_founded,
        copies_neural_at_tick0,
        founders_at_tick0,
        founders_neural_at_tick0,
        imported_material,
        final_population,
        final_neural: world.neural().animals.len() as u32,
        peak_neural: rec.peak_neural,
        neural_extinct_at: rec.neural_extinct_at,
        population_series: rec.population_series,
        neural_series: rec.neural_series,
        births: rec.births,
        deaths: rec.deaths,
        deaths_by_cause: rec.deaths_by_cause,
        births_of_neural_parents: rec.births_of_neural_parents,
        neural_children_of_neural_parents: rec.neural_children_of_neural_parents,
        travel_px_per_body: MeanByController::of(travel.0, travel.1),
        body_lengths_per_body: MeanByController::of(lengths.0, lengths.1),
        distinct_cells_per_body_window: MeanByController::of(rec.cells_neural, rec.cells_legacy),
        windows_closed: rec.windows_closed,
        copy_alive_final,
        copy_deaths_by_cause: rec.copy_deaths_by_cause.clone(),
        copy_births: rec.copy_births,
        copy_travel_px_mean: if copy_travel.1 > 0 { copy_travel.0 / copy_travel.1 as f64 } else { 0.0 },
        copy_body_lengths_mean: if copy_lengths.1 > 0 { copy_lengths.0 / copy_lengths.1 as f64 } else { 0.0 },
        copy_distinct_cells_per_window: if rec.copy_cells.1 > 0 {
            rec.copy_cells.0 / rec.copy_cells.1 as f64
        } else {
            0.0
        },
        copy_lifetimes_ticks,
        intake_producer: diag.producer_eaten,
        intake_fruit: diag.fruit_eaten,
        intake_litter: diag.litter_eaten,
        intake_carrion: diag.carrion_eaten,
        producer_growth: diag.producer_growth,
        opening_foliage: rec.opening_foliage,
        late_foliage_mean,
        foliage_retention: late_foliage_mean / rec.opening_foliage.max(1e-12),
        depletion_events: rec.depletion_events,
        recovery_events: rec.recovery_events,
        watched_cells: rec.watched_cells,
        apex_introduced,
        apex_alive_final: live_apex.len(),
        apex_births: rec.apex_births,
        apex_deaths: rec.apex_deaths,
        predation_deaths: rec.predation_deaths,
        max_abs_mass_residual: rec.max_mass,
        max_abs_energy_residual: rec.max_energy,
    }
}

fn introduce(
    world: &mut World,
    profile: &FixedHunterProfile,
    seed: u64,
    count: u32,
) -> Result<u32, String> {
    let targets = apex_targets(seed, count);
    let receipts = world
        .introduce_hunters(profile.clone(), &targets)
        .map_err(|e| format!("apex introduction refused: {e}"))?;
    Ok(receipts.len() as u32)
}

/// Run the whole matched matrix: `seeds × arms × {neural, legacy}`.
#[allow(clippy::too_many_arguments)]
pub fn run_stage(
    ecology: &Ecology,
    policy_path: &Path,
    seeds: &[u64],
    arms: &[u32],
    copies: usize,
    horizon: u64,
    sample_every: u64,
    introduce_tick: u64,
    pursuit_stop: cubarium_core::hunter::PursuitStop,
    workers: usize,
    wall_seconds: u64,
    out: &Path,
) -> Result<PopulationReport, String> {
    if workers == 0 || workers > 8 {
        return Err("--workers must be in 1..=8 for this brief's compute cap".into());
    }
    if copies == 0 {
        return Err("--copies must be at least 1".into());
    }
    let text = std::fs::read_to_string(policy_path)
        .map_err(|e| format!("reading {}: {e}", policy_path.display()))?;
    let file: PolicyFile = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a policy file: {e}", policy_path.display()))?;
    // The same refusal `es-evaluate` makes, for the same reason: a policy trained in another
    // ecology is not comparable in this one.
    file.check_ecology(ecology)?;
    let policy = file.policy()?;

    let mut jobs: Vec<(u64, u32, Mix)> = Vec::new();
    for seed in seeds {
        for arm in arms {
            for mix in [Mix::Neural, Mix::Legacy] {
                jobs.push((*seed, *arm, mix));
            }
        }
    }

    let plan = PopulationPlan {
        build_id: BUILD_ID.to_string(),
        config: ecology.label.clone(),
        config_hash: ecology.hex(),
        policy_file: policy_path.display().to_string(),
        policy_generation: file.generation,
        policy_digest: format!("{:#018x}", file.policy_digest),
        protocol_hash: format!("{:#018x}", file.protocol_hash),
        copies,
        horizon_ticks: horizon,
        sample_every,
        apex_introduce_tick: introduce_tick,
        pursuit_stop: pursuit_stop.as_str().to_string(),
        seeds: seeds.to_vec(),
        arms: arms.to_vec(),
        mixes: vec!["neural".into(), "legacy".into()],
        trials: jobs.len(),
        workers,
        wall_seconds_cap: wall_seconds,
    };

    std::fs::create_dir_all(out).map_err(|e| format!("creating {}: {e}", out.display()))?;
    let start = Instant::now();
    let deadline = start + std::time::Duration::from_secs(wall_seconds);
    let cursor = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, PopulationRow)>> = Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= jobs.len() {
                        return;
                    }
                    if Instant::now() >= deadline {
                        // Stop at the cap: the trial is not run and is counted, never
                        // extended and never silently shortened.
                        skipped.fetch_add(1, Ordering::SeqCst);
                        continue;
                    }
                    let (seed, arm, mix) = jobs[i];
                    let row = trial(
                        ecology,
                        &policy,
                        seed,
                        arm,
                        mix,
                        copies,
                        horizon,
                        sample_every,
                        introduce_tick,
                        pursuit_stop,
                    );
                    results.lock().expect("population results mutex").push((i, row));
                }
            });
        }
    });

    let wall = start.elapsed().as_secs_f64();
    let mut results = results.into_inner().map_err(|e| format!("results mutex: {e}"))?;
    results.sort_by_key(|(i, _)| *i);
    let rows: Vec<PopulationRow> = results.into_iter().map(|(_, r)| r).collect();
    let ticks: u64 = rows.iter().map(|r| r.ticks_run).sum();

    let rows_path = out.join("rows.jsonl");
    let mut text = String::new();
    for r in &rows {
        text.push_str(&serde_json::to_string(r).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    std::fs::write(&rows_path, text).map_err(|e| format!("writing {}: {e}", rows_path.display()))?;

    let mut arm_summaries = Vec::new();
    let mut inheritance: Option<bool> = None;
    for arm in arms {
        let neural: Vec<&PopulationRow> = rows
            .iter()
            .filter(|r| r.arm == *arm && r.mix == Mix::Neural && r.status == "completed")
            .collect();
        let legacy: Vec<&PopulationRow> = rows
            .iter()
            .filter(|r| r.arm == *arm && r.mix == Mix::Legacy && r.status == "completed")
            .collect();
        if neural.is_empty() && legacy.is_empty() {
            continue;
        }
        for r in &neural {
            if r.births_of_neural_parents > 0 {
                let all = r.neural_children_of_neural_parents == r.births_of_neural_parents;
                inheritance = Some(inheritance.unwrap_or(true) && all);
            }
        }
        let m = |v: &[&PopulationRow], f: &dyn Fn(&PopulationRow) -> f64| -> f64 {
            if v.is_empty() { 0.0 } else { v.iter().map(|r| f(r)).sum::<f64>() / v.len() as f64 }
        };
        let pair = |f: &dyn Fn(&PopulationRow) -> f64| (m(&neural, f), m(&legacy, f));
        let mut nb = ByController::default();
        let mut nd = ByController::default();
        for r in &neural {
            nb.neural += r.births.neural;
            nb.legacy += r.births.legacy;
            nd.neural += r.deaths.neural;
            nd.legacy += r.deaths.legacy;
        }
        arm_summaries.push(ArmSummary {
            arm: *arm,
            seeds: seeds.to_vec(),
            completed_neural: neural.len(),
            completed_legacy: legacy.len(),
            final_population: pair(&|r| f64::from(r.final_population)),
            final_neural_bodies: m(&neural, &|r| f64::from(r.final_neural)),
            peak_neural_bodies: m(&neural, &|r| f64::from(r.peak_neural)),
            births: pair(&|r| r.births.total() as f64),
            deaths: pair(&|r| r.deaths.total() as f64),
            intake_producer: pair(&|r| r.intake_producer),
            intake_detrital: pair(&|r| r.intake_litter + r.intake_carrion),
            foliage_retention: pair(&|r| r.foliage_retention),
            depletion_events: pair(&|r| r.depletion_events as f64),
            apex_alive_final: pair(&|r| r.apex_alive_final as f64),
            predation_deaths: pair(&|r| r.predation_deaths as f64),
            copy_lifetime_ticks: pair(&|r| {
                if r.copy_lifetimes_ticks.is_empty() {
                    0.0
                } else {
                    r.copy_lifetimes_ticks.iter().sum::<u64>() as f64
                        / r.copy_lifetimes_ticks.len() as f64
                }
            }),
            copy_alive_final: pair(&|r| r.copy_alive_final as f64),
            copy_births: pair(&|r| r.copy_births as f64),
            copy_distinct_cells: pair(&|r| r.copy_distinct_cells_per_window),
            copy_body_lengths: pair(&|r| r.copy_body_lengths_mean),
            neural_arm_distinct_cells: MeanByController {
                neural: m(&neural, &|r| r.distinct_cells_per_body_window.neural),
                neural_n: neural.iter().map(|r| r.distinct_cells_per_body_window.neural_n).sum(),
                legacy: m(&neural, &|r| r.distinct_cells_per_body_window.legacy),
                legacy_n: neural.iter().map(|r| r.distinct_cells_per_body_window.legacy_n).sum(),
            },
            neural_arm_body_lengths: MeanByController {
                neural: m(&neural, &|r| r.body_lengths_per_body.neural),
                neural_n: neural.iter().map(|r| r.body_lengths_per_body.neural_n).sum(),
                legacy: m(&neural, &|r| r.body_lengths_per_body.legacy),
                legacy_n: neural.iter().map(|r| r.body_lengths_per_body.legacy_n).sum(),
            },
            neural_arm_births: nb,
            neural_arm_deaths: nd,
        });
    }

    let report = PopulationReport {
        plan,
        wall_seconds: wall,
        trials_run: rows.iter().filter(|r| r.status == "completed").count(),
        trials_skipped: skipped.load(Ordering::SeqCst),
        ticks_simulated: ticks,
        ticks_per_second: if wall > 0.0 { ticks as f64 / wall } else { 0.0 },
        offspring_inherit_the_policy: inheritance,
        arms: arm_summaries,
    };
    let summary_path = out.join("summary.json");
    std::fs::write(
        &summary_path,
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("writing {}: {e}", summary_path.display()))?;
    Ok(report)
}

/// A compact terminal rendering, so the comparison is readable without opening the JSON.
pub fn print_report(report: &PopulationReport) {
    let p = &report.plan;
    println!(
        "# population comparison — {} copies of {} (generation {}) in {} (config hash {})",
        p.copies, p.policy_file, p.policy_generation, p.config, p.config_hash
    );
    println!(
        "# {} trials, {} ticks each, seeds {:?}, arms {:?}, apex at tick {}, {} workers",
        p.trials, p.horizon_ticks, p.seeds, p.arms, p.apex_introduce_tick, p.workers
    );
    println!(
        "# {} completed, {} skipped at the cap, {:.1} s wall, {:.0} ticks/s",
        report.trials_run, report.trials_skipped, report.wall_seconds, report.ticks_per_second
    );
    match report.offspring_inherit_the_policy {
        Some(true) => println!(
            "# provenance: every child of a neural parent was itself neural — the arms are \
             neural *lineages*, not four bodies"
        ),
        Some(false) => println!(
            "# provenance: NOT every child of a neural parent was neural — the neural arms \
             are MIXED populations"
        ),
        None => println!("# provenance: no neural parent reproduced; nothing to inherit"),
    }
    println!();
    println!(
        "{:>3} {:>8} {:>17} {:>15} {:>15} {:>13} {:>13} {:>11}",
        "arm", "mix", "final pop / neural", "births", "deaths", "intake P", "retention", "depletion"
    );
    for a in &report.arms {
        println!(
            "{:>3} {:>8} {:>10.1} / {:<4.1} {:>15.1} {:>15.1} {:>13.2} {:>13.3} {:>11.1}",
            a.arm,
            "neural",
            a.final_population.0,
            a.final_neural_bodies,
            a.births.0,
            a.deaths.0,
            a.intake_producer.0,
            a.foliage_retention.0,
            a.depletion_events.0,
        );
        println!(
            "{:>3} {:>8} {:>10.1} / {:<4} {:>15.1} {:>15.1} {:>13.2} {:>13.3} {:>11.1}",
            a.arm,
            "legacy",
            a.final_population.1,
            "-",
            a.births.1,
            a.deaths.1,
            a.intake_producer.1,
            a.foliage_retention.1,
            a.depletion_events.1,
        );
    }
    println!();
    println!("## the four imported copies themselves — the same body under the two controllers");
    println!(
        "{:>3} {:>8} {:>14} {:>12} {:>10} {:>14} {:>14}",
        "arm", "mix", "mean lifetime", "alive at end", "births", "cells/window", "body lengths"
    );
    for a in &report.arms {
        for (label, v) in [
            ("neural", (a.copy_lifetime_ticks.0, a.copy_alive_final.0, a.copy_births.0, a.copy_distinct_cells.0, a.copy_body_lengths.0)),
            ("legacy", (a.copy_lifetime_ticks.1, a.copy_alive_final.1, a.copy_births.1, a.copy_distinct_cells.1, a.copy_body_lengths.1)),
        ] {
            println!(
                "{:>3} {:>8} {:>14.0} {:>12.1} {:>10.1} {:>14.1} {:>14.0}",
                a.arm, label, v.0, v.1, v.2, v.3, v.4
            );
        }
    }
    println!();
    println!("## inside the neural arm, by the controller the world reports");
    println!(
        "{:>3} {:>12} {:>12} {:>14} {:>14} {:>16} {:>16}",
        "arm", "births n/l", "deaths n/l", "cells/body n", "cells/body l", "body lengths n", "body lengths l"
    );
    for a in &report.arms {
        println!(
            "{:>3} {:>12} {:>12} {:>14.2} {:>14.2} {:>16.0} {:>16.0}",
            a.arm,
            format!("{}/{}", a.neural_arm_births.neural, a.neural_arm_births.legacy),
            format!("{}/{}", a.neural_arm_deaths.neural, a.neural_arm_deaths.legacy),
            a.neural_arm_distinct_cells.neural,
            a.neural_arm_distinct_cells.legacy,
            a.neural_arm_body_lengths.neural,
            a.neural_arm_body_lengths.legacy,
        );
    }
    println!();
    println!(
        "apex: arms {:?}, mean alive at the end neural/legacy {:?}",
        p.arms,
        report
            .arms
            .iter()
            .map(|a| (a.arm, a.apex_alive_final.0, a.apex_alive_final.1))
            .collect::<Vec<_>>()
    );
    println!(
        "intake by food is world-scoped: the core exposes no per-organism intake, so the \
         controller's effect on food use is the neural-minus-legacy difference above."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::tensor;
    use cubarium_core::hunter::PursuitStop;

    /// The two arms must differ in **exactly one thing**: which controller drives the imported
    /// copies. Same seed, same founders, same bodies, same imported material — so a difference
    /// in the result is a difference the controller made, and nothing else.
    #[test]
    fn the_two_arms_are_matched_in_everything_but_the_controller() {
        let eco = Ecology::defaults();
        let policy = tensor::policy(&tensor::initial_center(7)).expect("a policy");
        let neural = run_trial(&eco, &policy, 1001, 0, Mix::Neural, 4, 400, 200, 0, Default::default(), Instant::now());
        let legacy = run_trial(&eco, &policy, 1001, 0, Mix::Legacy, 4, 400, 200, 0, Default::default(), Instant::now());

        assert_eq!(neural.status, "completed", "{:?}", neural.reason);
        assert_eq!(legacy.status, "completed", "{:?}", legacy.reason);
        assert_eq!(neural.founders_at_tick0, legacy.founders_at_tick0);
        assert_eq!(neural.copies_founded, 4);
        assert_eq!(legacy.copies_founded, 4);
        assert!(
            (neural.imported_material - legacy.imported_material).abs() < 1e-12,
            "the arms imported {} and {}",
            neural.imported_material,
            legacy.imported_material
        );
        assert!(neural.max_abs_mass_residual < 1e-6, "{}", neural.max_abs_mass_residual);
        assert!(legacy.max_abs_mass_residual < 1e-6, "{}", legacy.max_abs_mass_residual);
    }

    /// Provenance comes from the world. The neural arm's copies are neural and nothing else is;
    /// the legacy arm holds no neural body at all, however the copies were founded.
    #[test]
    fn a_bodys_controller_is_read_from_the_world_not_from_how_it_was_founded() {
        let eco = Ecology::defaults();
        let policy = tensor::policy(&tensor::initial_center(7)).expect("a policy");
        let neural = run_trial(&eco, &policy, 1001, 0, Mix::Neural, 4, 200, 100, 0, Default::default(), Instant::now());
        let legacy = run_trial(&eco, &policy, 1001, 0, Mix::Legacy, 4, 200, 100, 0, Default::default(), Instant::now());

        assert_eq!(neural.copies_neural_at_tick0, 4, "the four copies are the neural bodies");
        assert_eq!(neural.founders_neural_at_tick0, 0, "and the founders are not");
        assert_eq!(legacy.copies_neural_at_tick0, 0, "the legacy arm holds no neural body");
        assert_eq!(legacy.births.neural, 0);
        assert_eq!(legacy.deaths.neural, 0);
        assert_eq!(legacy.births_of_neural_parents, 0);
    }

    /// The same `(seed, arm, mix)` reproduces: the comparison is a measurement, not a draw.
    #[test]
    fn a_trial_reproduces_from_its_seed() {
        let eco = Ecology::defaults();
        let policy = tensor::policy(&tensor::initial_center(7)).expect("a policy");
        let a = run_trial(&eco, &policy, 1002, 1, Mix::Neural, 2, 400, 200, 100, Default::default(), Instant::now());
        let b = run_trial(&eco, &policy, 1002, 1, Mix::Neural, 2, 400, 200, 100, Default::default(), Instant::now());
        assert_eq!(a.population_series, b.population_series);
        assert_eq!(a.neural_series, b.neural_series);
        assert_eq!(a.births, b.births);
        assert_eq!(a.deaths, b.deaths);
        assert_eq!(a.intake_producer, b.intake_producer);
        assert_eq!(a.copy_lifetimes_ticks, b.copy_lifetimes_ticks);
        assert_eq!(a.apex_introduced, 1, "the arm introduced its cohort");
    }

    /// The copies land inside their faces, never on a seam row, and both mixes get the same
    /// places — that is what makes the pair matched.
    #[test]
    fn the_copy_placements_are_inside_the_faces_and_the_same_for_both_mixes() {
        for seed in [1001u64, 1002, 7] {
            let a = copy_placements(seed, 4);
            let b = copy_placements(seed, 4);
            assert_eq!(a.len(), 4);
            for ((cell, heading), (cell_b, _)) in a.iter().zip(&b) {
                assert_eq!(cell, cell_b);
                let (x, y) = (cell.cx(Topology::Cube, Scale::ONE), cell.cy(Topology::Cube, Scale::ONE));
                assert!((2..=13).contains(&x) && (2..=13).contains(&y), "{x},{y}");
                assert!((heading.length() - 1.0).abs() < 1e-12);
            }
            assert_ne!(a, copy_placements(seed + 1, 4), "another seed places differently");
        }
    }
}
