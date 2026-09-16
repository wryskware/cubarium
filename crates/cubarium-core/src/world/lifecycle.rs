use std::f64::consts::TAU;

use cubarium_surface::{
    CELL_COUNT, CellId, FACE_EXTENT, Face, FieldGraph, MAX_SEAMS, ScalarField, SurfacePoint,
    Travel, Vec2, chart_images, face_frame,
};

use crate::accounting::EnergyCorrection;
use crate::care::{self, CareState};
use crate::config::{FounderKind, WorldConfig};
use crate::dormancy::ApexDormancyState;
use crate::encounter::ApexEncounterState;
use crate::fields::{EcoScratch, EcologyV1State, Fields};
use crate::genome::{Genome, decode};
use crate::habitat::{Habitat, Weather};
use crate::hunter::HunterState;
use crate::ids::Slots;
use crate::organism::{Mode, Organism, Origin};
use crate::pairs::NeighborLists;
use crate::quiet::QuietState;
use crate::rng::{Counter, Stream, unit};

use super::*;

use super::state::{ChargingDiagnostics, IntakeDiagnostics, TickCounters, WorldState};

impl World {
    /// Create a new world: validate config, build habitat/weather, initial fields, and
    /// `founders.count` founders placed uniformly by area (rejection sampling on the five
    /// faces from `Stream::Founders`, key = index, counters 0..3 for face/u/v/heading and
    /// 4 for hue), each with the founder genome, `S = S_adult`,
    /// `R = initial_reserve_fraction · R_max`, `E = initial_energy_fraction · E_max`.
    /// Founder material is recorded in `external_material_in`.
    ///
    /// The five faces have equal area, so a uniform face draw followed by uniform `(u, v)`
    /// is already uniform by area; no rejection is needed.
    pub fn new(config: WorldConfig) -> Result<World, String> {
        config.validate()?;
        let habitat = Habitat::new(&config.habitat, config.seed);
        let weather = Weather::new(&config.weather, config.seed);
        let fields = Fields::new(&config, &habitat.light_base, &habitat.moisture_base);
        let ecology =
            EcologyV1State::new(&config, &habitat.light_base, &habitat.moisture_base);

        let cap = config.capacity.max_organisms;
        let mut organisms = Slots::with_capacity(cap as usize);
        let mut external_material_in = 0.0;
        for (index, kind) in roster_keys(&config).into_iter().take(cap as usize) {
            let founder = founder_body(&config, index, kind, 0);
            external_material_in += founder.structure + founder.reserve;
            organisms.insert(founder);
        }

        // The residual is measured against the field material present at creation; the
        // founders arrive from outside and are booked in `external_material_in`. Ecology v1's
        // wood and reserve are field material like any other: seeded, not imported.
        let initial_material = fields.total_material() + ecology.total_material();
        let state = WorldState {
            config,
            tick: 0,
            fields,
            weather,
            organisms,
            births_total: 0,
            deaths_total: [0; 3],
            cap_rejections_total: 0,
            external_material_in,
            light_in_total: 0.0,
            heat_out_total: 0.0,
            rain_in_total: 0.0,
            evap_out_total: 0.0,
            care: CareState::default(),
            energy_correction: EnergyCorrection::default(),
            hunters: HunterState::default(),
            quiet: QuietState::default(),
            apex_dormancy: ApexDormancyState::default(),
            apex_encounters: ApexEncounterState::default(),
            neural: crate::neural::NeuralState::default(),
            ecology,
        };
        Ok(World::assemble(state, habitat, initial_material))
    }

    /// Rebuild caches around a validated state (after a snapshot load).
    ///
    /// The residual baseline is re-derived so that it reads zero at load and reports drift
    /// since the load; a snapshot does not carry the pre-load residual.
    pub fn from_state(mut state: WorldState) -> Result<World, String> {
        for (_, o) in state.organisms.iter_mut() {
            // A genome written before fauna v2 is brought to version 2 in place, escrow
            // included; the phenotype is re-decoded so the new loci take effect.
            let upgraded = o.genome.upgrade();
            if let Some(e) = &mut o.escrow {
                e.genome.upgrade();
            }
            if upgraded {
                o.phenotype = decode(&o.genome, &state.config.organism);
            }
            // Repair a heading that drifted, but never touch one that is already unit to
            // rounding: renormalizing it would change the state bit-for-bit and a reloaded
            // world would no longer replay identically to the one that wrote the snapshot.
            let len = o.heading.length();
            if (len - 1.0).abs() > HEADING_REPAIR_FLOOR && (len - 1.0).abs() <= HEADING_TOLERANCE {
                o.heading = o.heading * (1.0 / len);
            }
        }
        state.validate()?;
        let habitat = Habitat::new(&state.config.habitat, state.config.seed);
        let organism_material: f64 = state.organisms.iter().map(|(_, o)| o.material()).sum();
        // The same terms `mass_residual` subtracts, so a loaded world reads zero: care has
        // imported `feed_material_in` and exported `clean_material_out` since creation, the
        // hunter extension has imported its founders (and any budget-matched control deposit),
        // and a carried carcass is material that is still in the world.
        let initial_material = state.fields.total_material()
            + state.ecology.total_material()
            + organism_material
            + state.hunters.gut_material_total()
                - state.external_material_in
                - state.care.feed_material_in
                + state.care.clean_material_out
                - state.hunters.imported_material();
        Ok(World::assemble(state, habitat, initial_material))
    }

    /// Perform the **ordinary founding** — the same roster [`World::new`] would place for this
    /// world's own config and seed — on an existing world, at whatever tick it has reached.
    ///
    /// This is the door workstream M's option A needs (`design/handoffs/
    /// ecology-v1-precondition-opus-2026-09-16.md`, deliverable 1): advance the §4 plant
    /// dynamics with no animals for a declared number of ticks, then found the animals into
    /// that grown field. `World::new` can only found at creation; the two named-genome doors
    /// place a *training* body (`TRAINING_START_RESERVE`, `Mode::Seeking`) rather than a
    /// founder with `founders.initial_reserve_fraction` and `Mode::Resting`, so neither of
    /// them can put the ordinary cohort into a world that has already run.
    ///
    /// The bodies are built by the same private helper [`World::new`] uses, so the two cannot
    /// drift: same `(seed, Stream::Founders, index)` draws, same kind overrides, same adult
    /// stores, same `Origin::Founder`, and `structure + reserve` per founder booked into
    /// `external_material_in` exactly as the constructor books it. The one thing that follows
    /// the clock is `born_tick`, which is the world's current tick: a founder placed at tick
    /// 96,000 was not born at tick 0.
    ///
    /// The roster is read from **this world's** `config.founders`. A caller that emptied it to
    /// run plants-only must write it back before founding — which is also what makes the
    /// founded world's config, and therefore its `state_hash`, the ordinary one. Founding into
    /// an untouched tick-0 world reproduces [`World::new`] bit for bit
    /// (`crates/cubarium-core/tests/found_roster.rs`).
    ///
    /// Refuses, by name and without touching the world, when animals are already present (a
    /// second roster would double the cohort and double-book its material) and when the config
    /// declares no roster at all (a silent empty founding is the exact failure a cleared-and-
    /// not-restored config would produce).
    pub fn found_roster(&mut self) -> Result<Vec<crate::ids::OrganismId>, String> {
        if !self.state.organisms.is_empty() {
            return Err(format!(
                "cannot found the roster: this world already holds {} organisms; the ordinary \
                 founding places a cohort into an empty world",
                self.state.organisms.len()
            ));
        }
        let config = self.state.config.clone();
        let keys = roster_keys(&config);
        if keys.is_empty() {
            return Err(
                "cannot found the roster: this world's config declares no founders (no kinds \
                 and a zero count), so there is no roster to found"
                    .to_string(),
            );
        }
        let cap = config.capacity.max_organisms as usize;
        let tick = self.state.tick;
        let mut ids = Vec::with_capacity(keys.len().min(cap));
        for (index, kind) in keys.into_iter().take(cap) {
            let founder = founder_body(&config, index, kind, tick);
            self.state.external_material_in += founder.structure + founder.reserve;
            ids.push(self.state.organisms.insert(founder));
        }
        self.moved.resize_with(self.state.organisms.slot_count(), Vec::new);
        Ok(ids)
    }

    fn assemble(state: WorldState, habitat: Habitat, initial_material: f64) -> World {
        let mut world = World {
            state,
            graph: FieldGraph::new(),
            habitat,
            images: std::array::from_fn(|i| {
                let mut v = Vec::new();
                chart_images(
                    Face::from_index(i as u8).expect("five faces"),
                    MAX_SEAMS,
                    &mut v,
                );
                v
            }),
            light: Box::new([0.0; CELL_COUNT]),
            moisture: Box::new([0.0; CELL_COUNT]),
            rain_source: Box::new([0.0; CELL_COUNT]),
            rain: Box::new([0.0; CELL_COUNT]),
            manual_rain: Box::new([0.0; CELL_COUNT]),
            rain_envelope: care::rain_envelope(),
            scratch: (ScalarField::zeros(), ScalarField::zeros()),
            eco_scratch: EcoScratch::default(),
            water_scratch: ScalarField::zeros(),
            sense_rings: Vec::new(),
            neighbors: NeighborLists::default(),
            travel_buf: Travel::default(),
            moved: Vec::new(),
            events: Vec::new(),
            hunter_events: Vec::new(),
            quiet_events: Vec::new(),
            apex_dormancy_events: Vec::new(),
            apex_encounter_events: Vec::new(),
            counters: TickCounters::default(),
            charging: ChargingDiagnostics::default(),
            intake: IntakeDiagnostics::default(),
            budgets: crate::world::BudgetRecorder::default(),
            strikes: crate::hunter::StrikeRecorder::default(),
            apex_opportunity: crate::encounter::ApexOpportunity::default(),
            neural_timing: crate::world::state::NeuralTiming::default(),
            scripted: Vec::new(),
            motor_model: crate::motor::MotorModel::default(),
            apex_turn_radius: crate::motor::ApexTurnRadius::default(),
            apex_motor_model: None,
            action_adapter: crate::neural::ActionAdapter::default(),
            initial_material,
        };
        // Make the derived light/moisture readable before the first tick advances weather.
        let cfg = &world.state.config;
        world.state.weather.sample(
            &cfg.weather,
            &world.habitat,
            &mut world.light,
            &mut world.moisture,
            &mut world.rain_source,
            cfg.habitat.moisture_min,
        );
        world
            .moved
            .resize_with(world.state.organisms.slot_count(), Vec::new);
        world.sense_rings = sense_rings(&world.graph);
        world
    }
}

// --- the shared roster -------------------------------------------------------------------
//
// One definition of "the founder roster this config describes", used by `World::new` at
// creation and by `World::found_roster` afterwards. They must be the *same* body: a
// preconditioned opening founded from a second implementation would be a different cohort
// from the one every result so far was measured in.

/// The founder roster as `(draw key, kind)` per founder. With kinds, the kind index is folded
/// into the high bits of the key so each kind's draws are their own stream and a seed stays
/// reproducible when another kind's count changes; without kinds the v1 path keeps
/// `key = index`.
fn roster_keys(config: &WorldConfig) -> Vec<(u64, Option<&FounderKind>)> {
    if config.founders.kinds.is_empty() {
        (0..u64::from(config.founders.count)).map(|i| (i, None)).collect()
    } else {
        config
            .founders
            .kinds
            .iter()
            .enumerate()
            .flat_map(|(k, kind)| {
                (0..u64::from(kind.count)).map(move |j| (((k as u64) << 32) | j, Some(kind)))
            })
            .collect()
    }
}

/// One founder of the roster: placed uniformly by area from `Stream::Founders` at `index`,
/// carrying the founder genome with the kind's fixed loci written over it, adult, with
/// `R = initial_reserve_fraction · R_max` and `E = initial_energy_fraction · E_max`.
///
/// `born_tick` is the caller's: tick 0 from the constructor, the world's current tick from
/// [`World::found_roster`].
fn founder_body(
    config: &WorldConfig,
    index: u64,
    kind: Option<&FounderKind>,
    born_tick: u64,
) -> Organism {
    let seed = config.seed;
    let face_index = (unit(seed, Stream::Founders, index, 0) * 5.0).floor();
    let face = Face::from_index(face_index as u8).unwrap_or(Face::Front);
    let u = unit(seed, Stream::Founders, index, 1) * FACE_EXTENT;
    let v = unit(seed, Stream::Founders, index, 2) * FACE_EXTENT;
    let heading = Vec2::from_screen_angle(unit(seed, Stream::Founders, index, 3) * TAU);
    let hue = unit(seed, Stream::Founders, index, 4) as f32;

    let mut genome = Genome::founder(kind.and_then(|k| k.hue).unwrap_or(hue), &config.drives);
    // Founders sense at the configured radius; the genome bounds still apply.
    genome.sense = config.organism.sense_radius as f32;
    if let Some(k) = kind {
        // A kind fixes the loci it names; the rest keep the v1 founder values. The rig follows
        // the kind, or the hue tercile when the kind leaves it open.
        if let Some(x) = k.diet {
            genome.diet = x;
        }
        if let Some(x) = k.depth {
            genome.depth = x;
        }
        if let Some(x) = k.speed {
            genome.speed = x;
        }
        if let Some(x) = k.size {
            genome.size = x;
        }
        if let Some(x) = k.metabolism {
            genome.metabolism = x;
        }
        if let Some(x) = k.swim {
            genome.swim = x;
        }
        if let Some(x) = k.form {
            genome.form = x;
        } else {
            genome.form = crate::genome::form_of_hue(genome.hue);
        }
    }
    genome.clamp();
    let phenotype = decode(&genome, &config.organism);
    let structure = phenotype.structure_adult;
    let reserve = config.founders.initial_reserve_fraction * phenotype.reserve_max;
    let energy = config.founders.initial_energy_fraction * phenotype.energy_max;
    Organism {
        pos: SurfacePoint::new(face, u, v).canonicalize(),
        heading,
        ou: Vec2::ZERO,
        structure,
        reserve,
        energy,
        born_tick,
        hunger_memory: (1.0 - reserve / phenotype.reserve_max).clamp(0.0, 1.0),
        mode: Mode::Resting,
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

// --- the shared training body ---------------------------------------------------------
//
// One founding routine, used by the recurrent training fixtures and by the display's
// seeding control. They must be the *same* body: a policy trained on one and run on the
// other would otherwise be scored against a different animal than it drives.

/// The genotype every recurrent episode — and every seeded display animal — uses: the unit
/// adult founder R0a, R0b and R0d measured. [`Genome::founder`] takes a *hue*; everything
/// else about the genome is that constructor's fixed unit adult, so one number names the
/// whole genotype.
pub const TRAINING_FOUNDER_HUE: f32 = 0.5;

/// Starting reserve as a fraction of `reserve_max` (headroom to store, so intake is never
/// refused for a reason that has nothing to do with behaviour).
pub const TRAINING_START_RESERVE: f64 = 0.5;

/// Starting energy as a fraction of `energy_max`.
pub const TRAINING_START_ENERGY: f64 = 0.75;

impl World {
    /// Found one **training body** at `pos` facing `heading`, with no policy attached.
    ///
    /// This is the single definition of that body: the [`TRAINING_FOUNDER_HUE`] unit adult,
    /// `S = S_adult`, `R = TRAINING_START_RESERVE · R_max`, `E = TRAINING_START_ENERGY ·
    /// E_max`, `Mode::Seeking`, a full hunger memory, `Origin::Founder`, born at the current
    /// tick, and `structure + reserve` booked into `external_material_in` because the body
    /// arrived from outside the world's material box.
    ///
    /// It refuses rather than exceed `capacity.max_organisms`: a founding that silently did
    /// nothing, or that pushed the population past the cap the invariants check, would be a
    /// worse answer than a named error.
    pub fn found_training_animal(
        &mut self,
        pos: SurfacePoint,
        heading: Vec2,
    ) -> Result<crate::ids::OrganismId, String> {
        let genome = Genome::founder(TRAINING_FOUNDER_HUE, &self.state.config.drives);
        self.found_body(pos, heading, genome)
    }

    /// Found one **adult with a named genome** at `pos` facing `heading`: the same body
    /// [`World::found_training_animal`] founds, with the genome chosen by the caller instead
    /// of fixed to the unit adult.
    ///
    /// This is the door a matched experiment needs. The roster path inside [`World::new`]
    /// can only run at creation and only over the configured kinds; `found_training_animal`
    /// fixes the genome. Neither can put a *stated* genome — the roster's skimmer with only
    /// `diet` moved, say — into a live world at a chosen cell and tick, which is what
    /// cloning a founder means (`design/handoffs/ecology-v1-diet-factorial-opus-2026-09-16.md`).
    ///
    /// Everything else is identical to the training door and shares its code: adult at
    /// founding, `R = TRAINING_START_RESERVE · R_max`, `E = TRAINING_START_ENERGY · E_max`,
    /// `Mode::Seeking`, a full hunger memory, [`Origin::Founder`], `born_tick` = the current
    /// tick, and `structure + reserve` booked into `external_material_in` because the body
    /// arrived from outside the material box.
    ///
    /// Three refusals, each by name and each leaving the world exactly as it found it:
    /// capacity, a zero heading, and a genome outside the genome's own bounds. The third is
    /// a refusal rather than a clamp on purpose: a caller asking for `size = 9` has stated
    /// something the genome cannot express, and silently founding a `size = 2` animal in a
    /// *controlled* experiment would corrupt the control without saying so.
    pub fn found_animal_with_genome(
        &mut self,
        pos: SurfacePoint,
        heading: Vec2,
        genome: Genome,
    ) -> Result<crate::ids::OrganismId, String> {
        let mut checked = genome.clone();
        if checked.clamp() {
            return Err(
                "a founding genome must already lie inside the genome's own bounds; this one \
                 does not (`Genome::clamp` would move at least one locus)"
                    .to_string(),
            );
        }
        self.found_body(pos, heading, genome)
    }

    /// The one definition of a founded adult body, shared by both doors above. Private: the
    /// public doors own what genome reaches it and what they refuse before it does.
    fn found_body(
        &mut self,
        pos: SurfacePoint,
        heading: Vec2,
        genome: Genome,
    ) -> Result<crate::ids::OrganismId, String> {
        let cap = self.state.config.capacity.max_organisms as usize;
        if self.state.organisms.len() >= cap {
            return Err(format!(
                "cannot found another animal: the world already holds {} of its {cap} organisms",
                self.state.organisms.len()
            ));
        }
        let heading = heading
            .normalized()
            .ok_or_else(|| "a founding heading must be nonzero".to_string())?;
        let cfg = self.state.config.clone();
        let phenotype = decode(&genome, &cfg.organism);
        let structure = phenotype.structure_adult;
        let reserve = TRAINING_START_RESERVE * phenotype.reserve_max;
        let energy = TRAINING_START_ENERGY * phenotype.energy_max;
        let id = self.state.organisms.insert(Organism {
            pos: pos.canonicalize(),
            heading,
            ou: Vec2::ZERO,
            structure,
            reserve,
            energy,
            born_tick: self.state.tick,
            hunger_memory: 1.0,
            mode: Mode::Seeking,
            escrow: None,
            births: 0,
            genome,
            phenotype,
            parent: None,
            origin: Origin::Founder,
            turn_counter: Counter::default(),
            fed_this_tick: false,
        });
        self.state.external_material_in += structure + reserve;
        self.moved.resize_with(self.state.organisms.slot_count(), Vec::new);
        Ok(id)
    }

    /// [`World::found_training_animal`] and then [`World::attach_neural_policy`]: the whole
    /// door a development tool needs to put a trained forager into a live world.
    ///
    /// Every refusal `attach_neural_policy` makes — a foreign digest, an enabled quiet
    /// extension — is made here too, and the body is removed again first, so a refused seed
    /// leaves the world exactly as it found it rather than half-founded.
    pub fn found_neural_animal(
        &mut self,
        pos: SurfacePoint,
        heading: Vec2,
        policy: crate::neural::Policy,
    ) -> Result<crate::ids::OrganismId, String> {
        // The two refusals that do not depend on the body are made *before* it exists, so
        // the ordinary case never founds and unwinds. The digest is checked against the
        // adapter **this world** runs, not against the build's default, so a `cub-act-2`
        // policy is refused by name by the display host and vice versa.
        policy.validate_in(self.action_adapter)?;
        if self.state.quiet.policy.enabled() {
            return Err(
                "the ordinary quiet extension and neural animals cannot be enabled together"
                    .into(),
            );
        }
        let id = self.found_training_animal(pos, heading)?;
        if let Err(e) = self.attach_neural_policy(id, policy) {
            self.state.neural.remove(id);
            if let Some(o) = self.state.organisms.remove(id) {
                self.state.external_material_in -= o.structure + o.reserve;
            }
            return Err(e);
        }
        Ok(id)
    }
}

/// The unit chart direction of increasing embedded height at a point of `face`: the chart
/// gradient of `y`, `(tangent_u.y, tangent_v.y)` normalized. Zero on the level top face,
/// `−v` on the four side faces.
pub(super) fn up_direction(face: Face) -> Vec2 {
    let frame = face_frame(face);
    normalize_or_zero(Vec2::new(frame.tangent_u[1], frame.tangent_v[1]))
}

/// Sensing depth in graph hops for a sensing radius: `ceil(r_sense / 4)`, at least 1 and at
/// most `SENSE_DEPTH_MAX` (`design/fauna-v2.md` "Controller v2").
pub(super) fn sense_depth(sense_radius: f64) -> usize {
    let hops = (sense_radius / cubarium_surface::CELL_PIXELS).ceil();
    if hops.is_finite() {
        (hops as usize).clamp(1, SENSE_DEPTH_MAX)
    } else {
        1
    }
}

/// For every cell, the cells at graph distance exactly 1, 2 and 3 (breadth-first over the
/// field graph, seams included, never across the open rim).
pub(super) fn sense_rings(graph: &FieldGraph) -> Vec<[Vec<CellId>; SENSE_DEPTH_MAX]> {
    CellId::all()
        .map(|origin| {
            let mut seen = vec![false; CELL_COUNT];
            seen[origin.index()] = true;
            let mut rings: [Vec<CellId>; SENSE_DEPTH_MAX] = Default::default();
            let mut frontier = vec![origin];
            for ring in rings.iter_mut() {
                let mut next = Vec::new();
                for cell in &frontier {
                    for n in graph.neighbors(*cell).iter().flatten() {
                        if !seen[n.index()] {
                            seen[n.index()] = true;
                            next.push(*n);
                        }
                    }
                }
                next.sort_unstable_by_key(|c| c.index());
                *ring = next.clone();
                frontier = next;
            }
            rings
        })
        .collect()
}

/// Unit gradient, or zero when the gradient carries no direction.
pub(super) fn normalize_or_zero(v: Vec2) -> Vec2 {
    if v.length() > GRADIENT_EPS {
        v.normalized().unwrap_or(Vec2::ZERO)
    } else {
        Vec2::ZERO
    }
}

/// The proportional share each request receives when the cell cannot serve them all.
pub(super) fn share(requested: f64, available: f64) -> f64 {
    if requested > available && requested > 0.0 {
        available / requested
    } else {
        1.0
    }
}

pub(crate) fn ticks_from_seconds(seconds: f64, dt: f64) -> u64 {
    let ticks = (seconds / dt).round();
    if ticks.is_finite() && ticks > 0.0 {
        ticks as u64
    } else {
        0
    }
}
