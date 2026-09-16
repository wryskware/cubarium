//! Workstream R — F's 150-minute variety census with **one locus of the founder roster moved**.
//!
//! [Workstream O](../../../design/7_Research/ecology-v1-depth-factorial-2026-09-16.md) showed
//! that `depth` — which decodes only to the preferred embedded height `h_pref = −1 + 2·depth`
//! in the steering term (`genome.rs:434`, `controller.rs:234`), carrying no capacity, no rate
//! and no bill — is what kills the roster skimmer at founding: sterile cold-founded clones at
//! `depth = 0.10` starved in 32 of 32 lives and at 0.55 reached the horizon in 26 of 32. Those
//! clones never bred. This module re-runs
//! [workstream F's census](../../../design/7_Research/ecology-v1-movement-2026-09-16.md) —
//! reproduction and mutation on for everyone, 180,000 ticks, six seeds, three apex arms, two
//! configurations — with the roster skimmer's `depth` written to the treatment value at tick 0
//! and nothing else touched.
//!
//! **The override is search-side and post-build.** `World::new` founds the ordinary 24
//! founders; then, before the first `step`, every founder carrying the roster skimmer's genome
//! gets `genome.depth = depth` and a re-decoded phenotype. No `WorldConfig` field is added —
//! adding one would change `calibrate::config_hash` for every existing TOML and invalidate
//! every retained row — and the control level writes the value that was already there, which
//! is why the 24 control rows must reproduce A's, I's and M's retained `final_state_hash`es
//! exactly.
//!
//! **Why this module carries its own run loop.** The override has to happen between
//! `World::new` and the first `World::step`, and [`crate::evaluate::run`] has no seam there.
//! Rather than change a file this round's other workers own, the loop is rebuilt here out of
//! [`crate::movement`]'s public accumulators — the same `CensusBuilder`, `FounderBroods`,
//! `MarginAccumulator` and `CrossingCounter` `evaluate` uses — and
//! `a_control_run_reproduces_the_ordinary_harness_world` pins it to `evaluate_with`'s own
//! hash, census, broods, crossings and margins. It is the ordinary loop or the test fails.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_core::encounter::ApexEncounterEvent;
use cubarium_core::genome::decode;
use cubarium_core::hunter::{FixedHunterProfile, HunterEvent};
use cubarium_core::organism::DeathCause;
use cubarium_core::{BodyBudget, LifeEvent, OrganismId, World};
use cubarium_surface::cell_of;
use serde::{Deserialize, Serialize};

use crate::calibrate;
use crate::evaluate::{BUILD_ID, DEPLETION_FRACTION, PROBE_EVERY, Protocol, RECOVERY_FRACTION};
use crate::factorial::{Roster, SKIMMER, form_name};
use crate::metrics::{DEATH_CAUSES, guild_of};
use crate::movement::{
    Census, CensusBuilder, CensusKey, Crossings, CrossingCounter, FounderBroods, MarginAccumulator,
    Margins, SKIMMER_FORM, StoreSum, Stores, diet_bin,
};
use crate::params;

/// The roster skimmer's own `depth` (`config.rs:519`). Writing it is a no-op by construction,
/// which is what makes the control arm a control.
pub const DEPTH_CONTROL: f32 = 0.10;

/// The roster grazer's own `depth` (`config.rs:513`), `h_pref = +0.1`: just above the equator,
/// so the steering term pushes the body off the rim rather than merely pulling it there less
/// hard. O's choice, unchanged, so the two campaigns measure the same treatment.
pub const DEPTH_TREATMENT: f32 = 0.55;

/// A's two configurations: the shipped §11 defaults and A's selected candidate.
pub const CONFIGURATIONS: [&str; 2] = ["baseline", "fast-leaf"];

/// A's three matched apex arms.
pub const ARMS: [u32; 3] = [0, 1, 2];

/// How many equal windows the horizon is cut into for the composition series. Five, so the
/// last one is F's and A's late window and the six boundaries are 0, 36k, 72k, 108k, 144k,
/// 180k ticks.
pub const WINDOWS: u64 = 5;

/// Water-depth bands under a body, O's own edges (`factorial::DEPTH_BANDS`): dry, damp,
/// standing water, the deep part of a pool. The last is open so no depth falls outside.
pub const DEPTH_BANDS: [f64; 4] = [1e-3, 0.05, 0.15, f64::INFINITY];

/// Which band a water depth falls in. The edges are inclusive upper bounds.
pub fn depth_band(depth: f64) -> usize {
    DEPTH_BANDS.iter().position(|hi| depth <= *hi).unwrap_or(DEPTH_BANDS.len() - 1)
}

// ---------------------------------------------------------------------------------------
// Workstream Y's ladder — declared here as stubs, so the definition tests in
// `tests/depth_ladder.rs` can be committed and run *before* anything implements them.
// Nothing below this line does any work yet.
// ---------------------------------------------------------------------------------------

/// The ladder. STUB.
pub const DEPTH_LEVELS: [f32; 6] = [0.0; 6];

/// The one apex arm this campaign runs.
pub const LADDER_ARM: u32 = 0;

pub const GENERATIONS: usize = 2;
pub const FOUNDER: usize = 0;
pub const DESCENDANT: usize = 1;

/// STUB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ServedProfile {
    pub bodies: u64,
    pub served: [f64; 4],
}

impl ServedProfile {
    pub fn add(&mut self, _budget: &BodyBudget) {}
    pub fn merge(&mut self, _other: &ServedProfile) {}
    pub fn mean(&self, _channel: usize) -> f64 {
        0.0
    }
    pub fn mean_total(&self) -> f64 {
        0.0
    }
}

/// STUB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationMarginBin {
    pub generation: u8,
    pub form: u8,
    pub diet_bin: u8,
    pub bodies: u64,
    pub deaths: u64,
    pub alive: u64,
    pub margin_mean: f64,
    pub margin_rate_mean: f64,
    pub served_total_mean: f64,
    pub recorded_seconds_mean: f64,
}

/// STUB.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationMargins {
    pub bins: Vec<GenerationMarginBin>,
}

impl GenerationMargins {
    pub fn bin(&self, _generation: u8, _form: u8, _diet_bin: u8) -> Option<&GenerationMarginBin> {
        None
    }
}

/// STUB.
#[derive(Clone, Debug, Default)]
pub struct GenerationMarginAccumulator {
    _unused: (),
}

impl GenerationMarginAccumulator {
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        _generation: usize,
        _key: CensusKey,
        _budget: &BodyBudget,
        _reserve_energy_density: f64,
        _now_tick: u64,
        _dt: f64,
        _alive: bool,
    ) {
    }

    pub fn finish(self) -> GenerationMargins {
        GenerationMargins::default()
    }
}

// ---------------------------------------------------------------------------------------
// The override
// ---------------------------------------------------------------------------------------

/// What the override actually wrote, carried into every row so a reader never has to trust
/// that it happened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    /// Founders whose genome was the roster skimmer's. The ordinary roster has five.
    pub bodies: u64,
    pub depth_before: f32,
    pub depth_after: f32,
    pub h_pref_before: f64,
    pub h_pref_after: f64,
}

/// Set every roster skimmer's `genome.depth` and re-decode its phenotype, changing nothing
/// else.
///
/// The bodies are found by **genome equality** against [`Roster::of`], not by a form number:
/// the roster is a set of kinds, and "the roster skimmer" means the body carrying that kind's
/// genome. At tick 0 the two agree, and the row records both so the agreement is visible.
///
/// A `depth` outside the genome's declared `0..=1` is **refused**, not clamped: the box is a
/// declaration, and running outside it silently would make the declaration prose.
pub fn apply_depth_override(world: &mut World, depth: f32) -> Result<Applied, String> {
    if !depth.is_finite() || !(0.0..=1.0).contains(&depth) {
        return Err(format!(
            "depth {depth} is outside the genome's declared bounds 0..=1 (genome.rs:267); a \
             treatment has to be a legal genotype"
        ));
    }
    let roster = Roster::of(world)?;
    let target = roster.genome(SKIMMER).clone();
    let organism_cfg = world.config().organism.clone();
    let before = decode(&target, &organism_cfg);
    let mut after_h_pref = before.h_pref;
    let mut bodies = 0u64;
    for (_, o) in world.state.organisms.iter_mut() {
        if o.genome != target {
            continue;
        }
        o.genome.depth = depth;
        o.phenotype = decode(&o.genome, &organism_cfg);
        after_h_pref = o.phenotype.h_pref;
        bodies += 1;
    }
    Ok(Applied {
        bodies,
        depth_before: target.depth,
        depth_after: depth,
        h_pref_before: before.h_pref,
        h_pref_after: after_h_pref,
    })
}

// ---------------------------------------------------------------------------------------
// The design
// ---------------------------------------------------------------------------------------

/// One cell of the census: a configuration, a depth level, a seed and an apex arm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Job {
    pub candidate: &'static str,
    pub depth: f32,
    pub seed: u64,
    pub arm: u32,
}

/// The whole design, in a fixed order, so the same matrix always produces the same rows in the
/// same places whatever order the workers finish in.
pub fn plan(seeds: &[u64]) -> Vec<Job> {
    let mut jobs = Vec::with_capacity(seeds.len() * 12);
    for depth in [DEPTH_CONTROL, DEPTH_TREATMENT] {
        for candidate in CONFIGURATIONS {
            for seed in seeds {
                for arm in ARMS {
                    jobs.push(Job { candidate, depth, seed: *seed, arm });
                }
            }
        }
    }
    jobs
}

// ---------------------------------------------------------------------------------------
// The row
// ---------------------------------------------------------------------------------------

/// Where the bodies of one visual form stood, over every probe of a run.
///
/// O found the binary wet fraction barely moves under this treatment (67 % → 65 %) while the
/// **depth of the water** moves six-fold, so the mean depth and the band histogram are the
/// measures and the wet fraction is reported beside them rather than instead of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DepthProfile {
    pub probes: u64,
    pub depth_sum: f64,
    pub wet_probes: u64,
    pub algae_band_probes: u64,
    /// Probes in each of [`DEPTH_BANDS`].
    pub bands: [u64; 4],
}

impl DepthProfile {
    fn observe(&mut self, depth: f64, algae_band: f64) {
        self.probes += 1;
        self.depth_sum += depth;
        if depth > 0.0 {
            self.wet_probes += 1;
        }
        if depth >= algae_band {
            self.algae_band_probes += 1;
        }
        self.bands[depth_band(depth)] += 1;
    }

    pub fn mean_depth(&self) -> f64 {
        if self.probes == 0 { 0.0 } else { self.depth_sum / self.probes as f64 }
    }

    pub fn wet_fraction(&self) -> f64 {
        if self.probes == 0 { 0.0 } else { self.wet_probes as f64 / self.probes as f64 }
    }

    pub fn algae_fraction(&self) -> f64 {
        if self.probes == 0 { 0.0 } else { self.algae_band_probes as f64 / self.probes as f64 }
    }

    pub fn merge(&mut self, other: &DepthProfile) {
        self.probes += other.probes;
        self.depth_sum += other.depth_sum;
        self.wet_probes += other.wet_probes;
        self.algae_band_probes += other.algae_band_probes;
        for (a, b) in self.bands.iter_mut().zip(other.bands) {
            *a += b;
        }
    }
}

/// The tick-0 founders of one visual form, followed individually.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FounderLives {
    pub founders: u64,
    pub alive_final: u64,
    pub deaths: u64,
    pub deaths_by_cause: [u64; 4],
    /// Σ age at death, exact from `LifeEvent::Death { age_ticks }`.
    pub death_age_ticks: u64,
    /// Σ (age at death, or the whole run for a survivor): **censored**, and named as such.
    pub lifetime_ticks: u64,
    /// Σ `births` over the founders of this form.
    pub births: u64,
}

impl FounderLives {
    /// Mean lifetime in seconds, counting a survivor at the length of the run.
    pub fn mean_lifetime_seconds(&self) -> f64 {
        if self.founders == 0 {
            0.0
        } else {
            self.lifetime_ticks as f64 * cubarium_core::DT / self.founders as f64
        }
    }
}

/// The live composition at one window boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub population: u64,
    pub by_form: [u64; 5],
    pub by_diet_bin: [u64; 3],
    pub foliage: f64,
    pub litter: f64,
}

/// One run of the census.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Row {
    // --- identity ---------------------------------------------------------------------
    pub candidate: String,
    pub seed: u64,
    pub arm: u32,
    /// The roster skimmer's `depth` in this run.
    pub depth: f64,
    /// `control` or `treatment`.
    pub level: String,
    pub build_id: String,
    pub protocol: Protocol,
    pub move_cost: f64,
    pub config_hash: u64,
    pub elapsed_ms: u64,

    // --- the override, recorded -------------------------------------------------------
    pub override_bodies: u64,
    pub override_h_pref_before: f64,
    pub override_h_pref_after: f64,
    /// Founders whose *form* is the skimmer's, so "found by genome" can be checked against
    /// "found by form" in every row rather than only in the test.
    pub skimmer_founders_by_form: u64,

    // --- the world --------------------------------------------------------------------
    pub ticks_run: u64,
    pub collapsed: bool,
    pub final_state_hash: u64,
    pub final_population: u64,
    pub prey_births: u64,
    pub prey_deaths: u64,
    /// Starvation, age, collapse, predation — [`DEATH_CAUSES`] order, over every body
    /// including the apex, exactly as `Components` counts them.
    pub deaths_by_cause: [u64; 4],
    pub opening_foliage: f64,
    pub final_foliage: f64,
    pub mean_foliage: f64,
    pub opening_litter: f64,
    pub final_litter: f64,
    pub mean_litter: f64,
    pub mean_population: f64,
    pub worst_material_residual: f64,
    pub worst_energy_residual: f64,
    pub ledger_records_dropped: u64,

    // --- the census -------------------------------------------------------------------
    pub census: Census,
    pub founder_broods: FounderBroods,
    pub crossings: Crossings,
    pub margins: Margins,
    pub terminal_by_cause: [StoreSum; 4],

    // --- workstream R's own measures --------------------------------------------------
    pub entered_by_form: [u64; 5],
    pub births_by_form: [u64; 5],
    pub deaths_by_form: [u64; 5],
    pub alive_by_form: [u64; 5],
    /// Form-3 bodies entered, by diet bin: O's predicted drift toward foliage.
    pub skimmer_entered_by_diet_bin: [u64; 3],
    pub skimmer_alive_by_diet_bin: [u64; 3],
    pub founder_lives: [FounderLives; 5],
    /// Where the bodies of each form stood, over every probe.
    pub depth_by_form: [DepthProfile; 5],
    /// The tick-0 skimmers only, so a founder's habitat is separable from its descendants'.
    pub founder_skimmer_depth: DepthProfile,
    pub series: Vec<Snapshot>,
}

// ---------------------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------------------

/// What a body is, for the census: `evaluate::census_key`, which is private to that module.
fn census_key(o: &cubarium_core::organism::Organism) -> CensusKey {
    CensusKey {
        form: o.phenotype.form.min(4),
        diet_bin: diet_bin(o.phenotype.diet) as u8,
        guild: guild_of(o.phenotype.cap_foliage, o.phenotype.cap_detrital) as u8,
    }
}

struct Recorder {
    // the ordinary harness's census state, in the ordinary harness's order
    founders: BTreeSet<OrganismId>,
    apex_ids: BTreeSet<OrganismId>,
    key_of_id: BTreeMap<OrganismId, CensusKey>,
    census: CensusBuilder,
    broods: FounderBroods,
    founder_parents: BTreeSet<OrganismId>,
    margins: MarginAccumulator,
    crossings: CrossingCounter,
    last_stores: BTreeMap<OrganismId, Stores>,
    terminal_by_cause: [StoreSum; 4],
    e_r: f64,
    algae_band: f64,
    prey_births: u64,
    prey_deaths: u64,
    deaths_by_cause: [u64; 4],

    // workstream R's own
    founder_skimmers: BTreeSet<OrganismId>,
    founder_form: BTreeMap<OrganismId, u8>,
    founder_lives: [FounderLives; 5],
    entered_by_form: [u64; 5],
    births_by_form: [u64; 5],
    deaths_by_form: [u64; 5],
    skimmer_entered_by_diet_bin: [u64; 3],
    depth_by_form: [DepthProfile; 5],
    founder_skimmer_depth: DepthProfile,
    series: Vec<Snapshot>,
    window_ticks: u64,
    next_window: u64,
    samples: u64,
    foliage_sum: f64,
    litter_sum: f64,
    population_sum: f64,
    collapsed_at: Option<u64>,
    worst_material_residual: f64,
    worst_energy_residual: f64,
}

impl Recorder {
    fn new(world: &World, protocol: Protocol) -> Recorder {
        let founders: BTreeSet<_> = world.state.organisms.iter().map(|(id, _)| id).collect();
        let apex_ids: BTreeSet<_> = world.hunters().members.iter().map(|m| m.id).collect();
        let mut key_of_id = BTreeMap::new();
        let mut census = CensusBuilder::default();
        let mut broods = FounderBroods::default();
        let mut founder_skimmers = BTreeSet::new();
        let mut founder_form = BTreeMap::new();
        let mut founder_lives = [FounderLives::default(); 5];
        let mut entered_by_form = [0u64; 5];
        let mut skimmer_entered_by_diet_bin = [0u64; 3];
        for (id, o) in world.state.organisms.iter() {
            if apex_ids.contains(&id) {
                continue;
            }
            let key = census_key(o);
            key_of_id.insert(id, key);
            census.found(key);
            broods.found(key.form);
            let f = usize::from(key.form).min(4);
            entered_by_form[f] += 1;
            founder_lives[f].founders += 1;
            founder_form.insert(id, key.form);
            if key.form == SKIMMER_FORM {
                founder_skimmers.insert(id);
                skimmer_entered_by_diet_bin[usize::from(key.diet_bin).min(2)] += 1;
            }
        }
        let p_ref: Vec<f64> = world.state.fields.p.clone();
        Recorder {
            founders,
            apex_ids,
            key_of_id,
            census,
            broods,
            founder_parents: BTreeSet::new(),
            margins: MarginAccumulator::default(),
            crossings: CrossingCounter::new(&p_ref, DEPLETION_FRACTION, RECOVERY_FRACTION),
            last_stores: BTreeMap::new(),
            terminal_by_cause: [StoreSum::default(); 4],
            e_r: world.config().organism.reserve_energy_density,
            algae_band: 0.5 * world.config().water.algae_depth,
            prey_births: 0,
            prey_deaths: 0,
            deaths_by_cause: [0; 4],
            founder_skimmers,
            founder_form,
            founder_lives,
            entered_by_form,
            births_by_form: [0; 5],
            deaths_by_form: [0; 5],
            skimmer_entered_by_diet_bin,
            depth_by_form: [DepthProfile::default(); 5],
            founder_skimmer_depth: DepthProfile::default(),
            series: Vec::new(),
            window_ticks: (protocol.horizon_ticks / WINDOWS).max(1),
            next_window: 0,
            samples: 0,
            foliage_sum: 0.0,
            litter_sum: 0.0,
            population_sum: 0.0,
            collapsed_at: None,
            worst_material_residual: 0.0,
            worst_energy_residual: 0.0,
        }
    }

    /// One tick's events, in `evaluate::Recorder::absorb`'s order — apex membership from the
    /// world itself first, then the life events, then the ledger, then the probe. The order is
    /// what makes the census reproduce the ordinary harness's, so it is copied and not
    /// improved.
    fn absorb(&mut self, world: &mut World) {
        for m in &world.hunters().members {
            self.apex_ids.insert(m.id);
        }
        for event in world.drain_hunter_events() {
            if let HunterEvent::Offspring { child, .. } = event {
                self.apex_ids.insert(child);
            }
        }
        for event in world.drain_apex_encounter_events() {
            if let ApexEncounterEvent::Born { child, .. } = event {
                self.apex_ids.insert(child);
            }
        }
        for _ in world.drain_apex_dormancy_events() {}
        world.drain_quiet_events();

        for event in world.drain_events() {
            match event {
                LifeEvent::Birth { tick, id, parent, .. } => {
                    if self.apex_ids.contains(&id) {
                        continue;
                    }
                    self.prey_births += 1;
                    let key = world
                        .state
                        .organisms
                        .get(id)
                        .map(census_key)
                        .unwrap_or(CensusKey { form: 0, diet_bin: 1, guild: 2 });
                    self.key_of_id.insert(id, key);
                    self.census.born(key);
                    let f = usize::from(key.form).min(4);
                    self.births_by_form[f] += 1;
                    self.entered_by_form[f] += 1;
                    if key.form == SKIMMER_FORM {
                        self.skimmer_entered_by_diet_bin[usize::from(key.diet_bin).min(2)] += 1;
                    }
                    if self.founders.contains(&parent) {
                        let form = self.key_of_id.get(&parent).map_or(0, |k| k.form);
                        let first = self.founder_parents.insert(parent);
                        self.broods.brood(form, tick, first);
                        if let Some(pf) = self.founder_form.get(&parent).copied() {
                            self.founder_lives[usize::from(pf).min(4)].births += 1;
                        }
                    }
                }
                LifeEvent::Death { id, cause, age_ticks, .. } => {
                    let slot = match cause {
                        DeathCause::Starvation => 0,
                        DeathCause::Age => 1,
                        DeathCause::Collapse => 2,
                        DeathCause::Predation => 3,
                    };
                    self.deaths_by_cause[slot] += 1;
                    if self.apex_ids.contains(&id) {
                        continue;
                    }
                    self.prey_deaths += 1;
                    let stores = self.last_stores.remove(&id);
                    if let Some(key) = self.key_of_id.get(&id).copied() {
                        self.census.died(
                            key,
                            slot,
                            DEATH_CAUSES[slot],
                            age_ticks,
                            stores.as_ref(),
                        );
                        self.deaths_by_form[usize::from(key.form).min(4)] += 1;
                    }
                    if let Some(s) = stores.as_ref() {
                        self.terminal_by_cause[slot].add(s);
                    }
                    if let Some(form) = self.founder_form.get(&id).copied() {
                        let lives = &mut self.founder_lives[usize::from(form).min(4)];
                        lives.deaths += 1;
                        lives.deaths_by_cause[slot] += 1;
                        lives.death_age_ticks += age_ticks;
                        lives.lifetime_ticks += age_ticks;
                    }
                }
            }
        }

        self.drain_budgets(world);

        if world.tick() % PROBE_EVERY == 0 {
            self.probe(world);
        }
    }

    fn probe(&mut self, world: &World) {
        let tick = world.tick();
        let mut skimmer_alive = false;
        for (id, o) in world.state.organisms.iter() {
            if self.apex_ids.contains(&id) {
                continue;
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
            let depth = world
                .state
                .fields
                .w
                .get(cell_of(&o.pos).index())
                .copied()
                .unwrap_or(0.0);
            let f = usize::from(o.phenotype.form).min(4);
            self.depth_by_form[f].observe(depth, self.algae_band);
            if self.founder_skimmers.contains(&id) {
                self.founder_skimmer_depth.observe(depth, self.algae_band);
            }
            if o.phenotype.form == SKIMMER_FORM {
                skimmer_alive = true;
            }
        }
        if skimmer_alive {
            self.census.skimmer_seen(tick);
        }
        self.crossings.observe(&world.state.fields.p);
    }

    fn drain_budgets(&mut self, world: &mut World) {
        let now = world.tick();
        let (closed, dropped) = world.drain_body_budgets();
        if dropped > 0 {
            self.margins.note_dropped(dropped);
        }
        for budget in closed {
            self.note_residuals(&budget);
            let Some(key) = self.key_of_id.get(&budget.id).copied() else { continue };
            self.margins.add(key, &budget, self.e_r, now, cubarium_core::DT, false);
        }
    }

    fn note_residuals(&mut self, budget: &BodyBudget) {
        self.worst_material_residual = self.worst_material_residual.max(budget.material_residual().abs());
        self.worst_energy_residual = self.worst_energy_residual.max(budget.energy_residual().abs());
    }

    /// One observation of the whole world at the sample cadence, and a composition snapshot
    /// when the tick crosses a window boundary.
    fn sample(&mut self, world: &World) {
        let foliage: f64 = world.state.fields.p.iter().sum();
        let litter: f64 = world.state.fields.d.iter().sum();
        self.samples += 1;
        self.foliage_sum += foliage;
        self.litter_sum += litter;
        self.population_sum += world.population() as f64;
        if world.tick() >= self.next_window {
            self.snapshot(world, foliage, litter);
            self.next_window = self.next_window.saturating_add(self.window_ticks);
        }
    }

    fn snapshot(&mut self, world: &World, foliage: f64, litter: f64) {
        let mut by_form = [0u64; 5];
        let mut by_diet_bin = [0u64; 3];
        let mut population = 0u64;
        for (id, o) in world.state.organisms.iter() {
            if self.apex_ids.contains(&id) {
                continue;
            }
            population += 1;
            let key = self.key_of_id.get(&id).copied().unwrap_or_else(|| census_key(o));
            by_form[usize::from(key.form).min(4)] += 1;
            by_diet_bin[usize::from(key.diet_bin).min(2)] += 1;
        }
        self.series.push(Snapshot {
            tick: world.tick(),
            population,
            by_form,
            by_diet_bin,
            foliage,
            litter,
        });
    }

    fn finish(mut self, world: &mut World) -> Finished {
        self.drain_budgets(world);
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
                self.note_residuals(&budget);
                self.margins.add(key, &budget, self.e_r, now, cubarium_core::DT, true);
            }
        }

        let mut alive_keys: Vec<CensusKey> = Vec::new();
        let mut alive_by_form = [0u64; 5];
        let mut skimmer_alive_by_diet_bin = [0u64; 3];
        for (id, o) in world.state.organisms.iter() {
            if self.apex_ids.contains(&id) {
                continue;
            }
            let key = self.key_of_id.get(&id).copied().unwrap_or_else(|| census_key(o));
            alive_keys.push(key);
            alive_by_form[usize::from(key.form).min(4)] += 1;
            if key.form == SKIMMER_FORM {
                skimmer_alive_by_diet_bin[usize::from(key.diet_bin).min(2)] += 1;
            }
            if let Some(form) = self.founder_form.get(&id).copied() {
                let lives = &mut self.founder_lives[usize::from(form).min(4)];
                lives.alive_final += 1;
                lives.lifetime_ticks += o.age_ticks(now);
            }
        }
        // `founder_lives.births` is counted **only** at the birth event, where every child of
        // a tick-0 founder passes exactly once. Adding a survivor's own `births` counter here
        // as well would count its brood twice, and it is the same brood.

        let foliage: f64 = world.state.fields.p.iter().sum();
        let litter: f64 = world.state.fields.d.iter().sum();
        // The terminal composition is always in the series, whatever tick the run stopped on.
        if self.series.last().map(|s| s.tick) != Some(now) {
            self.snapshot(world, foliage, litter);
        }

        let n = self.samples.max(1) as f64;
        Finished {
            census: self.census.clone().finish(&alive_keys, cubarium_core::DT),
            founder_broods: self.broods,
            crossings: self.crossings.summary(),
            margins: self.margins.clone().finish(true),
            terminal_by_cause: self.terminal_by_cause,
            prey_births: self.prey_births,
            prey_deaths: self.prey_deaths,
            deaths_by_cause: self.deaths_by_cause,
            entered_by_form: self.entered_by_form,
            births_by_form: self.births_by_form,
            deaths_by_form: self.deaths_by_form,
            alive_by_form,
            skimmer_entered_by_diet_bin: self.skimmer_entered_by_diet_bin,
            skimmer_alive_by_diet_bin,
            founder_lives: self.founder_lives,
            depth_by_form: self.depth_by_form,
            founder_skimmer_depth: self.founder_skimmer_depth,
            series: self.series,
            mean_foliage: self.foliage_sum / n,
            mean_litter: self.litter_sum / n,
            mean_population: self.population_sum / n,
            final_foliage: foliage,
            final_litter: litter,
            collapsed_at: self.collapsed_at,
            worst_material_residual: self.worst_material_residual,
            worst_energy_residual: self.worst_energy_residual,
        }
    }
}

struct Finished {
    census: Census,
    founder_broods: FounderBroods,
    crossings: Crossings,
    margins: Margins,
    terminal_by_cause: [StoreSum; 4],
    prey_births: u64,
    prey_deaths: u64,
    deaths_by_cause: [u64; 4],
    entered_by_form: [u64; 5],
    births_by_form: [u64; 5],
    deaths_by_form: [u64; 5],
    alive_by_form: [u64; 5],
    skimmer_entered_by_diet_bin: [u64; 3],
    skimmer_alive_by_diet_bin: [u64; 3],
    founder_lives: [FounderLives; 5],
    depth_by_form: [DepthProfile; 5],
    founder_skimmer_depth: DepthProfile,
    series: Vec<Snapshot>,
    mean_foliage: f64,
    mean_litter: f64,
    mean_population: f64,
    final_foliage: f64,
    final_litter: f64,
    collapsed_at: Option<u64>,
    worst_material_residual: f64,
    worst_energy_residual: f64,
}

/// One run: an ordinary world of 24 founders, the depth override at tick 0, E's ledger on, and
/// 180,000 ticks of ordinary reproduction and mutation.
pub fn run_one(
    candidate: &str,
    seed: u64,
    arm: u32,
    depth: f32,
    protocol: Protocol,
) -> Result<Row, String> {
    let start = Instant::now();
    protocol.validate()?;
    if !ARMS.contains(&arm) {
        return Err(format!("apex arm {arm} is not one of {ARMS:?}"));
    }
    if protocol.apex_founders != arm {
        return Err(format!(
            "the protocol's apex_founders {} is not the arm {arm}",
            protocol.apex_founders
        ));
    }
    let declared = calibrate::candidate(candidate).ok_or_else(|| {
        format!(
            "{candidate} is not a declared candidate; known: {}",
            calibrate::CANDIDATES.iter().map(|c| c.name).collect::<Vec<_>>().join(", ")
        )
    })?;
    let values = declared.vector()?;
    let mut config = crate::evaluate::base_config(seed);
    let mut profile = FixedHunterProfile::lanternjaw_trial(&config);
    params::apply(&values, &mut config, &mut profile)?;
    config.validate().map_err(|e| format!("config rejected: {e}"))?;
    profile.validate().map_err(|e| format!("hunter profile rejected: {e}"))?;
    let config_hash = calibrate::config_hash(&config);
    let move_cost = config.organism.move_cost;

    let mut world = World::new(config).map_err(|e| format!("world creation refused: {e}"))?;
    let skimmer_founders_by_form = world
        .state
        .organisms
        .iter()
        .filter(|(_, o)| o.phenotype.form == SKIMMER_FORM)
        .count() as u64;

    let mut apex_introduced = 0u32;
    if protocol.apex_founders > 0 && protocol.apex_introduce_tick == 0 {
        apex_introduced = introduce(&mut world, &profile, seed, protocol.apex_founders)?;
    }

    // The treatment, written after the ordinary founding and before the first step.
    let applied = apply_depth_override(&mut world, depth)?;

    world.record_body_budgets(true);
    world.record_plant_budgets(false);
    let mut recorder = Recorder::new(&world, protocol);
    world.drain_events();
    world.drain_hunter_events();
    world.drain_apex_dormancy_events();
    world.drain_apex_encounter_events();
    world.drain_quiet_events();
    recorder.sample(&world);
    let opening_foliage: f64 = world.state.fields.p.iter().sum();
    let opening_litter: f64 = world.state.fields.d.iter().sum();

    for _ in 0..protocol.horizon_ticks {
        world.step();
        if protocol.apex_founders > 0
            && apex_introduced == 0
            && world.tick() == protocol.apex_introduce_tick
        {
            apex_introduced = introduce(&mut world, &profile, seed, protocol.apex_founders)?;
        }
        recorder.absorb(&mut world);
        if world.tick() % protocol.sample_every == 0 {
            world
                .check_invariants()
                .map_err(|e| format!("invariant violated at tick {}: {e}", world.tick()))?;
            recorder.sample(&world);
        }
        if world.population() == 0 {
            recorder.collapsed_at = Some(world.tick());
            break;
        }
    }

    let ticks_run = world.tick();
    let final_population = world.population() as u64;
    let final_state_hash = cubarium_core::snapshot::state_hash(&world.state);
    let f = recorder.finish(&mut world);

    Ok(Row {
        candidate: candidate.to_string(),
        seed,
        arm,
        depth: f64::from(depth),
        level: if depth == DEPTH_CONTROL { "control".into() } else { "treatment".into() },
        build_id: BUILD_ID.to_string(),
        protocol,
        move_cost,
        config_hash,
        elapsed_ms: start.elapsed().as_millis() as u64,
        override_bodies: applied.bodies,
        override_h_pref_before: applied.h_pref_before,
        override_h_pref_after: applied.h_pref_after,
        skimmer_founders_by_form,
        ticks_run,
        collapsed: f.collapsed_at.is_some(),
        final_state_hash,
        final_population,
        prey_births: f.prey_births,
        prey_deaths: f.prey_deaths,
        deaths_by_cause: f.deaths_by_cause,
        opening_foliage,
        final_foliage: f.final_foliage,
        mean_foliage: f.mean_foliage,
        opening_litter,
        final_litter: f.final_litter,
        mean_litter: f.mean_litter,
        mean_population: f.mean_population,
        worst_material_residual: f.worst_material_residual,
        worst_energy_residual: f.worst_energy_residual,
        ledger_records_dropped: f.margins.records_dropped,
        census: f.census,
        founder_broods: f.founder_broods,
        crossings: f.crossings,
        margins: f.margins,
        terminal_by_cause: f.terminal_by_cause,
        entered_by_form: f.entered_by_form,
        births_by_form: f.births_by_form,
        deaths_by_form: f.deaths_by_form,
        alive_by_form: f.alive_by_form,
        skimmer_entered_by_diet_bin: f.skimmer_entered_by_diet_bin,
        skimmer_alive_by_diet_bin: f.skimmer_alive_by_diet_bin,
        founder_lives: f.founder_lives,
        depth_by_form: f.depth_by_form,
        founder_skimmer_depth: f.founder_skimmer_depth,
        series: f.series,
    })
}

/// The apex cohort at the deterministic seeded targets: `evaluate::introduce`, which is
/// private to that module.
fn introduce(
    world: &mut World,
    profile: &FixedHunterProfile,
    seed: u64,
    count: u32,
) -> Result<u32, String> {
    let targets = crate::evaluate::apex_targets(seed, count);
    let receipts = world
        .introduce_hunters(profile.clone(), &targets)
        .map_err(|e| format!("apex introduction refused: {e}"))?;
    Ok(receipts.len() as u32)
}

// ---------------------------------------------------------------------------------------
// Astra's rule
// ---------------------------------------------------------------------------------------

/// What the rule reads out of one run. Deliberately small and free of the world: the rule is
/// testable without simulating anything, which is how its thresholds were checked against F's
/// measured control before the campaign ran.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunFacts {
    pub seed: u64,
    pub arm: u32,
    pub skimmer_alive_final: u64,
    pub skimmer_births: u64,
    pub founder_forms_alive: u64,
    pub top_form_share: f64,
    pub alive_by_form: [u64; 5],
    pub founder_skimmer_mean_lifetime_seconds: f64,
    pub skimmer_entered: u64,
    pub skimmer_bin2_entered: u64,
}

impl RunFacts {
    pub fn of(row: &Row) -> RunFacts {
        let total: u64 = row.alive_by_form.iter().sum();
        let top = row.alive_by_form.iter().copied().max().unwrap_or(0);
        RunFacts {
            seed: row.seed,
            arm: row.arm,
            skimmer_alive_final: row.alive_by_form[usize::from(SKIMMER_FORM)],
            skimmer_births: row.births_by_form[usize::from(SKIMMER_FORM)],
            founder_forms_alive: row.alive_by_form.iter().filter(|n| **n > 0).count() as u64,
            top_form_share: if total == 0 { 0.0 } else { top as f64 / total as f64 },
            alive_by_form: row.alive_by_form,
            founder_skimmer_mean_lifetime_seconds: row.founder_lives
                [usize::from(SKIMMER_FORM)]
            .mean_lifetime_seconds(),
            skimmer_entered: row.skimmer_entered_by_diet_bin.iter().sum(),
            skimmer_bin2_entered: row.skimmer_entered_by_diet_bin[2],
        }
    }
}

/// One clause of the rule, with the agreement it rests on spelled out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Clause {
    pub name: String,
    pub holds: bool,
    pub runs_agreeing: usize,
    pub seeds_agreeing: usize,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Confirmed,
    Refuted,
    #[default]
    Partial,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Confirmed => "CONFIRMED",
            Verdict::Refuted => "REFUTED",
            Verdict::Partial => "PARTIAL",
        }
    }
}

/// One configuration's treatment cell judged against its own control cell.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Assessment {
    pub runs: usize,
    pub seeds: usize,
    pub runs_needed: usize,
    pub seeds_needed: usize,
    pub lineage: Clause,
    pub monoculture: Clause,
    pub variety_harmed: Clause,
    pub rescue_gone: Clause,
    /// O's predicted drift toward foliage: reported, never read by the verdict.
    pub diet_drift: Clause,
    pub verdict: Verdict,

    // --- workstream Y: the number the acceptance turns on, reported beside every rung ---
    /// The grazer's mean horizon population in this cell. STUB.
    pub grazer_mean: f64,
    /// The same in the 0.10 control cell. STUB.
    pub grazer_control_mean: f64,
    /// `grazer_mean / grazer_control_mean`, 0 when the control had none. STUB.
    pub grazer_ratio: f64,
}

impl Assessment {
    /// Astra's rule: a depth is acceptable if the lineage establishes and neither the
    /// monoculture clause nor the variety clause holds. STUB.
    pub fn acceptable(&self) -> bool {
        false
    }
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let mut n = 0u64;
    let mut sum = 0.0;
    for v in values {
        n += 1;
        sum += v;
    }
    if n == 0 { 0.0 } else { sum / n as f64 }
}

/// How many runs satisfy a predicate, and how many seeds have at least two runs that do.
fn agreement(facts: &[RunFacts], pred: impl Fn(&RunFacts) -> bool) -> (usize, usize) {
    let mut runs = 0usize;
    let mut by_seed: BTreeMap<u64, usize> = BTreeMap::new();
    for f in facts {
        let ok = pred(f);
        if ok {
            runs += 1;
        }
        *by_seed.entry(f.seed).or_insert(0) += usize::from(ok);
    }
    (runs, by_seed.values().filter(|n| **n >= 2).count())
}

impl Assessment {
    /// The pre-registered rule, evaluated on one configuration's matched cells.
    ///
    /// Thresholds scale with the cell: a clause needs two thirds of the runs and five sixths
    /// of the seeds, which at the campaign's 18 runs over 6 seeds is 12 and 5.
    pub fn of(control: &[RunFacts], treatment: &[RunFacts]) -> Assessment {
        let n = treatment.len();
        let seeds: BTreeSet<u64> = treatment.iter().map(|f| f.seed).collect();
        let s = seeds.len();
        let runs_needed = (2 * n).div_ceil(3);
        let seeds_needed = (5 * s).div_ceil(6);

        // L — the lineage establishes: alive at the horizon *and* descendants.
        let (l_runs, l_seeds) =
            agreement(treatment, |f| f.skimmer_alive_final >= 1 && f.skimmer_births > 0);
        let lineage = Clause {
            name: "L: the skimmer lineage establishes".into(),
            holds: l_runs >= runs_needed && l_seeds >= seeds_needed,
            runs_agreeing: l_runs,
            seeds_agreeing: l_seeds,
            detail: format!(
                "alive at the horizon and breeding in {l_runs}/{n} runs (need {runs_needed}), \
                 {l_seeds}/{s} seeds (need {seeds_needed}); the control had \
                 {}/{} runs",
                agreement(control, |f| f.skimmer_alive_final >= 1 && f.skimmer_births > 0).0,
                control.len()
            ),
        };

        // M — a new monoculture.
        let kinds_t = mean(treatment.iter().map(|f| f.founder_forms_alive as f64));
        let kinds_c = mean(control.iter().map(|f| f.founder_forms_alive as f64));
        let share_t = mean(treatment.iter().map(|f| f.top_form_share));
        let share_c = mean(control.iter().map(|f| f.top_form_share));
        let mono = kinds_t < kinds_c || (share_t >= 0.80 && share_c < 0.80);
        let (m_runs, m_seeds) = agreement(treatment, |f| f.top_form_share >= 0.80);
        let monoculture = Clause {
            name: "M: the world became a new monoculture".into(),
            holds: mono,
            runs_agreeing: m_runs,
            seeds_agreeing: m_seeds,
            detail: format!(
                "kinds at the horizon {kinds_t:.2} against the control's {kinds_c:.2}; top-form \
                 share {share_t:.2} against {share_c:.2}"
            ),
        };

        // V — variety harmed: a kind that had the horizon in the control loses it here.
        let mut harmed: Vec<String> = Vec::new();
        let mut v_runs = 0usize;
        let mut v_seeds = 0usize;
        for form in 0..5usize {
            let c = mean(control.iter().map(|f| f.alive_by_form[form] as f64));
            let t = mean(treatment.iter().map(|f| f.alive_by_form[form] as f64));
            if c >= 1.0 && t < 0.60 * c {
                harmed.push(format!("{} {t:.1} vs {c:.1}", form_name(form as u8)));
            }
            let (present_c, _) = agreement(control, |f| f.alive_by_form[form] > 0);
            let (absent_t, absent_seeds) = agreement(treatment, |f| f.alive_by_form[form] == 0);
            if present_c * 2 >= control.len() && absent_t >= runs_needed {
                harmed.push(format!("{} absent in {absent_t}/{n}", form_name(form as u8)));
                v_runs = v_runs.max(absent_t);
                v_seeds = v_seeds.max(absent_seeds);
            }
        }
        let variety_harmed = Clause {
            name: "V: another kind lost ground".into(),
            holds: !harmed.is_empty(),
            runs_agreeing: v_runs,
            seeds_agreeing: v_seeds,
            detail: if harmed.is_empty() {
                "no founder kind fell below 0.6x its control horizon population".into()
            } else {
                harmed.join("; ")
            },
        };

        // F — the founding rescue disappears under reproduction.
        let life_t = mean(treatment.iter().map(|f| f.founder_skimmer_mean_lifetime_seconds));
        let life_c = mean(control.iter().map(|f| f.founder_skimmer_mean_lifetime_seconds));
        let (gone_runs, gone_seeds) = agreement(treatment, |f| f.skimmer_alive_final == 0);
        let rescue_gone = Clause {
            name: "F: the founding rescue disappeared".into(),
            holds: life_t <= 1.10 * life_c || gone_runs >= runs_needed,
            runs_agreeing: gone_runs,
            seeds_agreeing: gone_seeds,
            detail: format!(
                "founder skimmer mean lifetime {life_t:.0} s against the control's {life_c:.0} s; \
                 no skimmer at the horizon in {gone_runs}/{n} runs"
            ),
        };

        // D — the diet drift, reported and not read by the verdict.
        let share = |f: &[RunFacts]| {
            let entered: u64 = f.iter().map(|r| r.skimmer_entered).sum();
            let bin2: u64 = f.iter().map(|r| r.skimmer_bin2_entered).sum();
            if entered == 0 { 0.0 } else { bin2 as f64 / entered as f64 }
        };
        let drift_t = share(treatment);
        let drift_c = share(control);
        let control_by_seed: BTreeMap<u64, f64> = seeds
            .iter()
            .map(|s| {
                let mine: Vec<RunFacts> =
                    control.iter().copied().filter(|f| f.seed == *s).collect();
                (*s, share(&mine))
            })
            .collect();
        let (d_runs, d_seeds) = agreement(treatment, |f| {
            let c = control_by_seed.get(&f.seed).copied().unwrap_or(0.0);
            f.skimmer_entered > 0
                && f.skimmer_bin2_entered as f64 / f.skimmer_entered as f64 > c
        });
        let diet_drift = Clause {
            name: "D: the diet drifts toward foliage".into(),
            holds: drift_t > drift_c && d_seeds >= seeds_needed,
            runs_agreeing: d_runs,
            seeds_agreeing: d_seeds,
            detail: format!(
                "{:.0} % of form-3 entrants in the foliage bin against the control's {:.0} %",
                100.0 * drift_t,
                100.0 * drift_c
            ),
        };

        let verdict = if lineage.holds && !monoculture.holds && !variety_harmed.holds {
            Verdict::Confirmed
        } else if rescue_gone.holds || variety_harmed.holds {
            Verdict::Refuted
        } else {
            Verdict::Partial
        };

        Assessment {
            runs: n,
            seeds: s,
            runs_needed,
            seeds_needed,
            lineage,
            monoculture,
            variety_harmed,
            rescue_gone,
            diet_drift,
            verdict,
            // STUB: workstream Y's reported grazer ratio.
            grazer_mean: 0.0,
            grazer_control_mean: 0.0,
            grazer_ratio: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------------------
// The campaign
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CampaignReport {
    pub build_id: String,
    pub protocol: Protocol,
    pub seeds: Vec<u64>,
    pub trials: usize,
    pub completed: usize,
    pub skipped: usize,
    pub workers: usize,
    pub wall_seconds: f64,
    pub simulated_ticks: u64,
    pub worst_material_residual: f64,
    pub worst_energy_residual: f64,
    pub ledger_records_dropped: u64,
    pub reproduction: Vec<Reproduction>,
    pub assessments: Vec<(String, Assessment)>,
}

/// One control row checked against a retained row of the same `(candidate, seed, arm)`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reproduction {
    pub source: String,
    pub checked: usize,
    pub matched: usize,
    pub missing: usize,
    pub mismatches: Vec<String>,
}

/// Read `(candidate, seed, arm) -> final_state_hash` out of a retained `evals.jsonl`, keeping
/// only rows at the shipped movement price so a ladder file contributes its control rung only.
fn retained_hashes(path: &Path) -> Result<BTreeMap<(String, u64, u32), u64>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value =
            serde_json::from_str(line).map_err(|e| format!("{}: {e}", path.display()))?;
        let price = v.get("move_cost").and_then(|p| p.as_f64()).unwrap_or(calibrate::DEFAULT_MOVE_COST);
        if (price - calibrate::DEFAULT_MOVE_COST).abs() > f64::EPSILON {
            continue;
        }
        let (Some(c), Some(s), Some(a), Some(h)) = (
            v.get("candidate").and_then(|c| c.as_str()),
            v.get("seed").and_then(|s| s.as_u64()),
            v.get("arm").and_then(|a| a.as_u64()),
            v.pointer("/metrics/final_state_hash").and_then(|h| h.as_u64()),
        ) else {
            continue;
        };
        out.insert((c.to_string(), s, a as u32), h);
    }
    Ok(out)
}

fn check_against(rows: &[Row], path: &Path) -> Reproduction {
    let mut r = Reproduction {
        source: path.display().to_string(),
        checked: 0,
        matched: 0,
        missing: 0,
        mismatches: Vec::new(),
    };
    let retained = match retained_hashes(path) {
        Ok(h) => h,
        Err(e) => {
            r.mismatches.push(format!("unreadable: {e}"));
            return r;
        }
    };
    for row in rows.iter().filter(|r| r.level == "control") {
        let key = (row.candidate.clone(), row.seed, row.arm);
        match retained.get(&key) {
            None => r.missing += 1,
            Some(h) => {
                r.checked += 1;
                if *h == row.final_state_hash {
                    r.matched += 1;
                } else {
                    r.mismatches.push(format!(
                        "{}/{}/arm {}: {} != retained {h}",
                        row.candidate, row.seed, row.arm, row.final_state_hash
                    ));
                }
            }
        }
    }
    r
}

/// The census subcommand's arguments. Declared here rather than in `main.rs` so this
/// workstream's footprint on the binary is the one module line and the one dispatch line the
/// brief allows.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// How many training seeds, from the front. The campaign's design is six.
    #[arg(long, default_value_t = 6)]
    pub seeds: usize,
    #[arg(long, default_value_t = 180_000)]
    pub ticks: u64,
    #[arg(long, default_value_t = 600)]
    pub sample_every: u64,
    /// The tick the apex cohort is introduced at, matching A's screen.
    #[arg(long, default_value_t = 6_000)]
    pub introduce_tick: u64,
    #[arg(long, default_value_t = 8)]
    pub workers: usize,
    /// A hard cap: a trial not started by it is recorded as skipped, and no horizon is
    /// shortened.
    #[arg(long, default_value_t = 900)]
    pub wall_seconds: u64,
    /// Retained `evals.jsonl` files the control rows must reproduce by `final_state_hash`.
    #[arg(
        long,
        default_value = "runs/ecology-v1-calibration/screen/evals.jsonl,runs/ecology-v1-ladder/ladder/evals.jsonl,runs/ecology-v1-plant-budget/present-off/evals.jsonl"
    )]
    pub retained: String,
    #[arg(long, default_value = "runs/ecology-v1-depth-census")]
    pub out: PathBuf,
}

/// The one dispatch entry point: parse the arguments and run the campaign.
pub fn run_command(args: Args) -> Result<(), String> {
    let protocol = Protocol {
        horizon_ticks: args.ticks,
        sample_every: args.sample_every,
        apex_founders: 0,
        apex_introduce_tick: args.introduce_tick,
    };
    let retained: Vec<PathBuf> = args
        .retained
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect();
    command(args.seeds, args.workers, protocol, args.wall_seconds, &retained, &args.out)
}

/// Run the campaign, write its rows and report, print the tables the note carries.
#[allow(clippy::too_many_arguments)]
pub fn command(
    seed_count: usize,
    workers: usize,
    protocol: Protocol,
    wall_seconds: u64,
    retained: &[PathBuf],
    out: &Path,
) -> Result<(), String> {
    if workers == 0 || workers > 8 {
        return Err("--workers must be in 1..=8 for this brief's compute cap".into());
    }
    let seeds = calibrate::SeedSet::Training.seeds(seed_count)?;
    let jobs = plan(&seeds);
    println!("build {BUILD_ID}");
    println!("seeds {seeds:?}   arms {ARMS:?}   configurations {CONFIGURATIONS:?}");
    println!(
        "depth {DEPTH_CONTROL} (control) vs {DEPTH_TREATMENT} (treatment)   trials {}",
        jobs.len()
    );
    println!(
        "horizon {} sample/{} apex at {}   ledger on",
        protocol.horizon_ticks, protocol.sample_every, protocol.apex_introduce_tick
    );

    std::fs::create_dir_all(out).map_err(|e| format!("creating {}: {e}", out.display()))?;
    let rows_path = out.join("runs.jsonl");
    let file = std::fs::File::create(&rows_path)
        .map_err(|e| format!("creating {}: {e}", rows_path.display()))?;
    let writer = Mutex::new(std::io::BufWriter::new(file));
    let start = Instant::now();
    let deadline = start + std::time::Duration::from_secs(wall_seconds);
    let cursor = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let results: Mutex<Vec<(usize, Row)>> = Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= jobs.len() {
                        return;
                    }
                    if Instant::now() >= deadline {
                        skipped.fetch_add(1, Ordering::SeqCst);
                        continue;
                    }
                    let job = jobs[i];
                    let protocol = Protocol { apex_founders: job.arm, ..protocol };
                    match run_one(job.candidate, job.seed, job.arm, job.depth, protocol) {
                        Ok(row) => {
                            if let Ok(mut w) = writer.lock()
                                && let Ok(text) = serde_json::to_string(&row)
                            {
                                let _ = writeln!(w, "{text}");
                            }
                            results.lock().expect("results mutex").push((i, row));
                        }
                        Err(e) => failures.lock().expect("failure mutex").push(format!(
                            "{}/{}/arm {}/depth {}: {e}",
                            job.candidate, job.seed, job.arm, job.depth
                        )),
                    }
                }
            });
        }
    });

    let wall = start.elapsed().as_secs_f64();
    if let Ok(mut w) = writer.lock() {
        let _ = w.flush();
    }
    let failures = failures.into_inner().map_err(|e| format!("failure mutex: {e}"))?;
    if !failures.is_empty() {
        return Err(format!("{} runs failed:\n{}", failures.len(), failures.join("\n")));
    }
    let mut results = results.into_inner().map_err(|e| format!("results mutex: {e}"))?;
    results.sort_by_key(|(i, _)| *i);
    let rows: Vec<Row> = results.into_iter().map(|(_, r)| r).collect();

    // Read the rows back from the file that was just written rather than from memory: a table
    // in the note and the rows on disk cannot then disagree.
    let text = std::fs::read_to_string(&rows_path).map_err(|e| format!("{}: {e}", rows_path.display()))?;
    let mut disk: Vec<Row> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{}: {e}", rows_path.display()))?;
    disk.sort_by(|a, b| {
        (a.level.clone(), a.candidate.clone(), a.seed, a.arm).cmp(&(
            b.level.clone(),
            b.candidate.clone(),
            b.seed,
            b.arm,
        ))
    });
    if disk.len() != rows.len() {
        return Err(format!("{} rows in memory, {} on disk", rows.len(), disk.len()));
    }

    let reproduction: Vec<Reproduction> =
        retained.iter().map(|p| check_against(&disk, p)).collect();
    print_reproduction(&reproduction);
    let clean = reproduction.iter().all(|r| r.mismatches.is_empty() && r.matched > 0);

    let mut assessments = Vec::new();
    for candidate in CONFIGURATIONS {
        let facts = |level: &str| -> Vec<RunFacts> {
            disk.iter()
                .filter(|r| r.candidate == candidate && r.level == level)
                .map(RunFacts::of)
                .collect()
        };
        assessments.push((candidate.to_string(), Assessment::of(&facts("control"), &facts("treatment"))));
    }

    if clean {
        print_tables(&disk);
        print_assessments(&assessments);
    } else {
        println!(
            "\nSTOP: a control row does not reproduce its retained world. The treatment arms \
             are not interpreted."
        );
    }

    let report = CampaignReport {
        build_id: BUILD_ID.to_string(),
        protocol,
        seeds,
        trials: jobs.len(),
        completed: disk.len(),
        skipped: skipped.into_inner(),
        workers,
        wall_seconds: wall,
        simulated_ticks: disk.iter().map(|r| r.ticks_run).sum(),
        worst_material_residual: disk
            .iter()
            .map(|r| r.worst_material_residual)
            .fold(0.0f64, f64::max),
        worst_energy_residual: disk.iter().map(|r| r.worst_energy_residual).fold(0.0f64, f64::max),
        ledger_records_dropped: disk.iter().map(|r| r.ledger_records_dropped).sum(),
        reproduction,
        assessments,
    };
    let summary = out.join("summary.json");
    std::fs::write(
        &summary,
        serde_json::to_string_pretty(&report).map_err(|e| format!("summary: {e}"))?,
    )
    .map_err(|e| format!("{}: {e}", summary.display()))?;

    println!(
        "\nruns {} ({} skipped) in {:.1} s on {workers} workers   {:.0} ticks/s",
        report.completed,
        report.skipped,
        wall,
        report.simulated_ticks as f64 / wall.max(1e-9)
    );
    println!(
        "worst |material residual| {:.3e}   worst |energy residual| {:.3e}   dropped ledger \
         records {}",
        report.worst_material_residual, report.worst_energy_residual, report.ledger_records_dropped
    );
    println!("rows    {}", rows_path.display());
    println!("summary {}", summary.display());
    Ok(())
}

fn print_reproduction(checks: &[Reproduction]) {
    println!("\n## The reproduction check, before anything is interpreted\n");
    println!("| retained rows | control rows checked | matched | not in that file |");
    println!("| --- | --- | --- | --- |");
    for c in checks {
        println!("| {} | {} | {} | {} |", c.source, c.checked, c.matched, c.missing);
        for m in &c.mismatches {
            println!("| | | **{m}** | |");
        }
    }
}

fn cell_rows<'a>(rows: &'a [Row], candidate: &str, level: &str) -> Vec<&'a Row> {
    rows.iter().filter(|r| r.candidate == candidate && r.level == level).collect()
}

fn mean_of(rows: &[&Row], f: impl Fn(&Row) -> f64) -> f64 {
    if rows.is_empty() { 0.0 } else { rows.iter().map(|r| f(r)).sum::<f64>() / rows.len() as f64 }
}

fn print_tables(rows: &[Row]) {
    println!("\n## The census\n");
    println!(
        "| cand | depth | late pop | kinds at end | grazer | glider | burrower | **skimmer** | \
         skimmer births | first form-3 brood | foliage x | litter x | worlds lost |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for candidate in CONFIGURATIONS {
        for level in ["control", "treatment"] {
            let cell = cell_rows(rows, candidate, level);
            if cell.is_empty() {
                continue;
            }
            let brood: Vec<u64> = cell
                .iter()
                .filter_map(|r| r.founder_broods.first_brood_tick_by_form[3])
                .collect();
            println!(
                "| {candidate} | {:.2} | {:.1} | {:.2} | {:.1} | {:.1} | {:.1} | **{:.1}** | {:.1} | \
                 {} | {:.2} | {:.2} | {} |",
                cell[0].depth,
                mean_of(&cell, |r| r.final_population as f64),
                mean_of(&cell, |r| r.alive_by_form.iter().filter(|n| **n > 0).count() as f64),
                mean_of(&cell, |r| r.alive_by_form[0] as f64),
                mean_of(&cell, |r| r.alive_by_form[1] as f64),
                mean_of(&cell, |r| r.alive_by_form[2] as f64),
                mean_of(&cell, |r| r.alive_by_form[3] as f64),
                mean_of(&cell, |r| r.births_by_form[3] as f64),
                if brood.is_empty() {
                    "none".to_string()
                } else {
                    format!("{} of {}", brood.len(), cell.len())
                },
                mean_of(&cell, |r| if r.opening_foliage > 0.0 {
                    r.final_foliage / r.opening_foliage
                } else {
                    0.0
                }),
                mean_of(&cell, |r| if r.opening_litter > 0.0 {
                    r.final_litter / r.opening_litter
                } else {
                    0.0
                }),
                cell.iter().filter(|r| r.collapsed).count()
            );
        }
    }

    println!("\n### Where the bodies stood (mean water depth d under the body, over every probe)\n");
    println!(
        "| cand | depth | grazer | glider | burrower | **skimmer** | founder skimmers | \
         skimmer wet % | skimmer algae-band % |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for candidate in CONFIGURATIONS {
        for level in ["control", "treatment"] {
            let cell = cell_rows(rows, candidate, level);
            if cell.is_empty() {
                continue;
            }
            let mut pooled = [DepthProfile::default(); 5];
            let mut founder = DepthProfile::default();
            for r in &cell {
                for (a, b) in pooled.iter_mut().zip(&r.depth_by_form) {
                    a.merge(b);
                }
                founder.merge(&r.founder_skimmer_depth);
            }
            println!(
                "| {candidate} | {:.2} | {:.3} | {:.3} | {:.3} | **{:.3}** | {:.3} | {:.0} % | {:.0} % |",
                cell[0].depth,
                pooled[0].mean_depth(),
                pooled[1].mean_depth(),
                pooled[2].mean_depth(),
                pooled[3].mean_depth(),
                founder.mean_depth(),
                100.0 * pooled[3].wet_fraction(),
                100.0 * pooled[3].algae_fraction()
            );
        }
    }

    println!("\n### Net margin per body by form and diet bin (e, from E's ledger)\n");
    println!("| cand | depth | form | diet bin | bodies | margin (e) | margin (e/s) | served (m) |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for candidate in CONFIGURATIONS {
        for level in ["control", "treatment"] {
            let cell = cell_rows(rows, candidate, level);
            let mut acc: BTreeMap<(u8, u8), (u64, f64, f64, f64)> = BTreeMap::new();
            for r in &cell {
                for b in &r.margins.bins {
                    let e = acc.entry((b.form, b.diet_bin)).or_insert((0, 0.0, 0.0, 0.0));
                    e.0 += b.bodies;
                    e.1 += b.margin_mean * b.bodies as f64;
                    e.2 += b.margin_rate_mean * b.bodies as f64;
                    e.3 += b.served_total_mean * b.bodies as f64;
                }
            }
            for ((form, bin), (bodies, margin, rate, served)) in acc {
                if bodies == 0 {
                    continue;
                }
                let n = bodies as f64;
                println!(
                    "| {candidate} | {:.2} | {} | {} | {bodies} | {:+.4} | {:+.6} | {:.2} |",
                    cell.first().map_or(0.0, |r| r.depth),
                    form_name(form),
                    crate::movement::DIET_BINS[usize::from(bin).min(2)],
                    margin / n,
                    rate / n,
                    served / n
                );
            }
        }
    }

    println!("\n### The composition over time (mean live bodies by form at each window)\n");
    println!("| cand | depth | tick | pop | grazer | glider | burrower | **skimmer** |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for candidate in CONFIGURATIONS {
        for level in ["control", "treatment"] {
            let cell = cell_rows(rows, candidate, level);
            if cell.is_empty() {
                continue;
            }
            let windows = cell.iter().map(|r| r.series.len()).max().unwrap_or(0);
            for w in 0..windows {
                let with: Vec<&Snapshot> = cell.iter().filter_map(|r| r.series.get(w)).collect();
                if with.is_empty() {
                    continue;
                }
                let n = with.len() as f64;
                let m = |f: fn(&Snapshot) -> f64| with.iter().map(|s| f(s)).sum::<f64>() / n;
                println!(
                    "| {candidate} | {:.2} | {} | {:.1} | {:.1} | {:.1} | {:.1} | **{:.1}** |",
                    cell[0].depth,
                    with[0].tick,
                    m(|s| s.population as f64),
                    m(|s| s.by_form[0] as f64),
                    m(|s| s.by_form[1] as f64),
                    m(|s| s.by_form[2] as f64),
                    m(|s| s.by_form[3] as f64),
                );
            }
        }
    }

    for (form, what) in [(3usize, "form-3 bodies"), (0, "form-0 (grazer) bodies")] {
        println!("\n### Per seed: {what} alive at the horizon — arms with any, and the total\n");
        println!("| cand | depth | 1001 | 1002 | 1003 | 1004 | 1005 | 1006 |");
        println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
        for candidate in CONFIGURATIONS {
            for level in ["control", "treatment"] {
                let cell = cell_rows(rows, candidate, level);
                if cell.is_empty() {
                    continue;
                }
                let mut line = format!("| {candidate} | {:.2} ", cell[0].depth);
                let seeds: BTreeSet<u64> = cell.iter().map(|r| r.seed).collect();
                for seed in seeds {
                    let mine: Vec<&&Row> = cell.iter().filter(|r| r.seed == seed).collect();
                    let arms = mine.iter().filter(|r| r.alive_by_form[form] > 0).count();
                    let bodies: u64 = mine.iter().map(|r| r.alive_by_form[form]).sum();
                    line.push_str(&format!("| {arms}/{} ({bodies}) ", mine.len()));
                }
                println!("{line}|");
            }
        }
    }

    println!("\n### The founder skimmers themselves (5 per run, 90 per cell)\n");
    println!(
        "| cand | depth | alive at the horizon | died | mean age at death | mean lifetime \
         (survivors at the horizon) | broods |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for candidate in CONFIGURATIONS {
        for level in ["control", "treatment"] {
            let cell = cell_rows(rows, candidate, level);
            if cell.is_empty() {
                continue;
            }
            let mut lives = FounderLives::default();
            for r in &cell {
                let l = r.founder_lives[3];
                lives.founders += l.founders;
                lives.alive_final += l.alive_final;
                lives.deaths += l.deaths;
                lives.death_age_ticks += l.death_age_ticks;
                lives.lifetime_ticks += l.lifetime_ticks;
                lives.births += l.births;
            }
            println!(
                "| {candidate} | {:.2} | {} of {} | {} | {:.0} s | {:.0} s | {} |",
                cell[0].depth,
                lives.alive_final,
                lives.founders,
                lives.deaths,
                if lives.deaths == 0 {
                    0.0
                } else {
                    lives.death_age_ticks as f64 * cubarium_core::DT / lives.deaths as f64
                },
                lives.mean_lifetime_seconds(),
                lives.births
            );
        }
    }
}

fn print_assessments(assessments: &[(String, Assessment)]) {
    println!("\n## The verdict, by the pre-registered rule\n");
    for (candidate, a) in assessments {
        println!(
            "\n**{candidate}** — {} runs over {} seeds; a clause needs {} runs and {} seeds.\n",
            a.runs, a.seeds, a.runs_needed, a.seeds_needed
        );
        println!("| clause | holds | runs | seeds | detail |");
        println!("| --- | --- | --- | --- | --- |");
        for c in [&a.lineage, &a.monoculture, &a.variety_harmed, &a.rescue_gone, &a.diet_drift] {
            println!(
                "| {} | {} | {}/{} | {}/{} | {} |",
                c.name,
                if c.holds { "**yes**" } else { "no" },
                c.runs_agreeing,
                a.runs,
                c.seeds_agreeing,
                a.seeds,
                c.detail
            );
        }
        println!("\n**{}**", a.verdict.label());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bands_are_increasing_and_the_last_is_open() {
        for pair in DEPTH_BANDS.windows(2) {
            assert!(pair[0] < pair[1]);
        }
        assert!(DEPTH_BANDS[3].is_infinite());
        assert_eq!(depth_band(f64::INFINITY), 3);
    }

    #[test]
    fn a_depth_profile_merges_without_losing_a_probe() {
        let mut a = DepthProfile::default();
        let mut b = DepthProfile::default();
        a.observe(0.0, 0.05);
        a.observe(0.2, 0.05);
        b.observe(0.06, 0.05);
        let (pa, pb) = (a.probes, b.probes);
        a.merge(&b);
        assert_eq!(a.probes, pa + pb);
        assert_eq!(a.bands.iter().sum::<u64>(), a.probes);
        assert_eq!(a.wet_probes, 2);
        assert_eq!(a.algae_band_probes, 2);
        assert!((a.mean_depth() - (0.0 + 0.2 + 0.06) / 3.0).abs() < 1e-12);
    }

    #[test]
    fn a_censored_founder_lifetime_counts_a_survivor_at_the_length_of_the_run() {
        let lives = FounderLives {
            founders: 2,
            alive_final: 1,
            deaths: 1,
            death_age_ticks: 1_000,
            lifetime_ticks: 1_000 + 180_000,
            ..FounderLives::default()
        };
        let expected = (181_000.0 * cubarium_core::DT) / 2.0;
        assert!((lives.mean_lifetime_seconds() - expected).abs() < 1e-9);
    }

    #[test]
    fn the_agreement_rule_needs_two_runs_of_a_seed() {
        let f = |seed, arm, ok| RunFacts {
            seed,
            arm,
            skimmer_alive_final: u64::from(ok),
            ..RunFacts::default()
        };
        let facts = vec![
            f(1, 0, true),
            f(1, 1, false),
            f(1, 2, false),
            f(2, 0, true),
            f(2, 1, true),
            f(2, 2, false),
        ];
        let (runs, seeds) = agreement(&facts, |r| r.skimmer_alive_final > 0);
        assert_eq!(runs, 3);
        assert_eq!(seeds, 1, "one run of a seed is not that seed agreeing");
    }
}
