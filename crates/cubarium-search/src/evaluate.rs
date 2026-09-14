//! One candidate, one seed, one real `cubarium-core` world, run headless to a hard horizon.
//!
//! Nothing here is a simplified ecology: the world is built by [`cubarium_core::World::new`]
//! and advanced by [`cubarium_core::World::step`], with ordinary founders that move, feed and
//! reproduce, the ordinary initial resource inventories, and the real conservation audits.
//! The only staging is the apex cohort, which is an explicit, accounted input placed once at
//! the start and never restocked.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;
use std::time::Instant;

use cubarium_core::encounter::ApexEncounterEvent;
use cubarium_core::hunter::{FixedHunterProfile, HunterEvent, HunterTarget};
use cubarium_core::organism::{DeathCause, Mode};
use cubarium_core::{ApexDormancyEvent, LifeEvent, OrganismId, World, WorldConfig};
use serde::{Deserialize, Serialize};

use crate::metrics::{Components, FORMS_POSSIBLE, Sample};
use crate::params;
use crate::rng;

/// The build that produced a row, stamped by `build.rs`.
pub const BUILD_ID: &str = env!("CUBARIUM_SEARCH_BUILD");

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
}

impl Evaluation {
    fn refused(status: Status, reason: String, seed: u64, protocol: Protocol, elapsed: u64) -> Self {
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
        }
    }
}

/// The base world every candidate starts from: the shipped defaults, with the host-side
/// event log off. Founders, capacities and initial stocks are untouched, so every candidate
/// gets the same starting resources and the same landscape for a given seed.
pub fn base_config(seed: u64) -> WorldConfig {
    let mut config = WorldConfig { seed, ..WorldConfig::default() };
    config.capacity.event_log = false;
    config
}

/// Deterministic apex founder placements for a seed: distinct faces, away from the seams.
fn apex_targets(seed: u64, count: u32) -> Vec<HunterTarget> {
    (0..count)
        .map(|i| {
            let face = (rng::draw(seed, rng::stream::APEX_PLACEMENT, u64::from(i), 0) % 5) as u8;
            let u = 6.0 + rng::unit(seed, rng::stream::APEX_PLACEMENT, u64::from(i), 1) * 52.0;
            let v = 6.0 + rng::unit(seed, rng::stream::APEX_PLACEMENT, u64::from(i), 2) * 52.0;
            HunterTarget { face, u, v }
        })
        .collect()
}

/// The audited energy total: `Σ_cells (e_p·P + e_f·F + De) + Σ_organisms (E + e_r·R) +
/// Σ_escrow (e_r·(S+R) + E) + Σ hunter guts`. Recomputed here because the core's own copy is
/// compiled out of release builds, and this milestone needs the identity checked in release.
fn stored_energy(world: &World) -> f64 {
    let state = &world.state;
    let e_p = state.config.producer.energy_density;
    let e_f = state.config.fruit.energy_density;
    let e_r = state.config.organism.reserve_energy_density;
    let cells: f64 = state.fields.p.iter().map(|p| e_p * p).sum::<f64>()
        + state.fields.f.iter().map(|f| e_f * f).sum::<f64>()
        + state.fields.de.iter().sum::<f64>();
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

/// Run one candidate on one seed. Never panics: a panic inside the core is caught and
/// reported as [`Status::Failed`].
pub fn evaluate(values: &[f64], seed: u64, protocol: Protocol) -> Evaluation {
    let start = Instant::now();
    match std::panic::catch_unwind(AssertUnwindSafe(|| run(values, seed, protocol, start))) {
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

fn run(values: &[f64], seed: u64, protocol: Protocol, start: Instant) -> Evaluation {
    let ms = |start: Instant| start.elapsed().as_millis() as u64;
    if let Err(e) = protocol.validate() {
        return Evaluation::refused(Status::Invalid, e, seed, protocol, ms(start));
    }

    let mut config = base_config(seed);
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

    let mut recorder = Recorder::new(&world, protocol, apex_introduced);
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
            && world.tick() == protocol.apex_introduce_tick
        {
            match introduce(&mut world, &profile, seed, protocol.apex_founders) {
                Ok((m, e, n)) => {
                    apex_material_in = m;
                    apex_energy_in = e;
                    apex_introduced = n;
                    recorder.apex_introduced = n;
                }
                Err(e) => return Evaluation::refused(Status::Invalid, e, seed, protocol, ms(start)),
            }
        }
        recorder.absorb(&mut world);
        if world.tick() % protocol.sample_every == 0 {
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
        if world.population() == 0 {
            // Nothing is ever created from nothing: an empty world cannot recover, so the
            // horizon stops here and the collapse tick is the honest survival time.
            recorder.collapsed_at = Some(world.tick());
            break;
        }
    }

    let metrics = recorder.finish(&mut world);
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

    first_predation_tick: Option<u64>,
    prey_before_first_predation: u32,
    prey_min_after_predation: u32,

    collapsed_at: Option<u64>,
    opening_energy: f64,
    opening_net_in: f64,
    max_mass: f64,
    max_water: f64,
    max_energy: f64,
    peak_population: u32,
    min_population: u32,
}

impl Recorder {
    fn new(world: &World, protocol: Protocol, apex_introduced: u32) -> Self {
        let founders: BTreeSet<_> = world.state.organisms.iter().map(|(id, _)| id).collect();
        let apex_ids: BTreeSet<_> = world.hunters().members.iter().map(|m| m.id).collect();
        Recorder {
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
                HunterEvent::Attempt { attack_counter: Some(_), .. } => self.apex_attacks += 1,
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
                LifeEvent::Birth { id, parent, .. } => {
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
                    }
                }
                LifeEvent::Death { id, cause, tick, .. } => {
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
                    }
                    if cause == DeathCause::Predation && self.first_predation_tick.is_none() {
                        self.first_predation_tick = Some(tick);
                        self.prey_before_first_predation =
                            self.samples.last().map(|s| s.prey).unwrap_or(0);
                    }
                }
            }
        }
    }

    fn sample(&mut self, world: &World) {
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
        for (_, o) in state.organisms.iter() {
            population += 1;
            forms[(o.phenotype.form as usize).min(7)] += 1;
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
            mean_hunger: if population > 0 { hunger / f64::from(population) } else { 0.0 },
            mass_residual: mass,
            water_residual: water,
            energy_residual: energy,
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

    fn finish(mut self, world: &mut World) -> Components {
        // A final observation, so the terminal state is always in the series.
        if self.samples.last().map(|s| s.tick) != Some(world.tick()) {
            self.sample(world);
        }
        let n = self.samples.len().max(1) as f64;
        let mean = |f: fn(&Sample) -> f64| self.samples.iter().map(f).sum::<f64>() / n;

        let ticks_run = world.tick();
        let last = *self.samples.last().expect("at least one sample");
        let capacity = world.config().producer.max * world.state.fields.p.len() as f64;
        let e_p = world.config().producer.energy_density;
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

        let apex_active_samples = self.samples.iter().filter(|s| s.apex_active > 0).count() as f64;
        let recovery = if self.first_predation_tick.is_some()
            && self.prey_min_after_predation < u32::MAX
        {
            f64::from(last.prey) / f64::from(self.prey_min_after_predation.max(1))
        } else {
            0.0
        };

        Components {
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
            min_forms_present: self.samples.iter().map(|s| s.forms_present).min().unwrap_or(0),
            final_forms_present: last.forms_present,

            feeding_fraction: mean(|s| {
                if s.population > 0 { f64::from(s.feeding) / f64::from(s.population) } else { 0.0 }
            }),
            seeking_fraction: mean(|s| {
                if s.population > 0 { f64::from(s.seeking) / f64::from(s.population) } else { 0.0 }
            }),
            resting_fraction: mean(|s| {
                if s.population > 0 { f64::from(s.resting) / f64::from(s.population) } else { 0.0 }
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
        }
    }
}
