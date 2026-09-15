use cubarium_surface::{CELL_COUNT, CellId, cell_of};

use crate::config::WorldConfig;
use crate::events::LifeEvent;
use crate::ids::OrganismId;
use crate::organism::Mode;
use crate::telemetry::Telemetry;
use crate::view::{FieldDump, OrganismView, RenderView};
use crate::{DT, snapshot};

use super::*;

use super::lifecycle::ticks_from_seconds;
use super::state::TickCounters;

impl World {
    /// The segments this organism was **actually transported along** during the last step,
    /// borrowed rather than cloned.
    ///
    /// This is the same data [`World::render_view`] publishes in
    /// [`crate::view::OrganismView::moved`], reachable without building a whole view — that call
    /// clones four full fields and every organism's lobes, which is right for a renderer and
    /// wrong for an observer that wants one number every tick. Read-only: it borrows what the
    /// tick already computed, consumes no draw, and changes nothing.
    ///
    /// A seam crossing appears here as the several straight pieces it really was, which is why
    /// an endpoint difference in chart coordinates is not a substitute: at a seam it is
    /// meaningless, and at a reflective rim it is not even the distance travelled.
    ///
    /// Indexed by **slot**, exactly as the view is: a handle whose generation has been retired
    /// resolves to whatever now occupies that slot. Callers iterate live organisms.
    pub fn moved_segments(&self, id: OrganismId) -> &[cubarium_surface::PathSegment] {
        self.moved.get(id.slot as usize).map_or(&[], Vec::as_slice)
    }

    pub fn render_view(&self) -> RenderView {
        // The same derivation the tick uses for the escrow's due date, so what a renderer
        // shows as "nearly born" is the tick the world will actually commit the birth on.
        let gestation_ticks = ticks_from_seconds(self.state.config.organism.gestation_seconds, DT);
        let tick = self.state.tick;
        let organisms = self
            .state
            .organisms
            .iter()
            .filter(|(id, _)| !self.state.apex_dormancy.contains(*id))
            .map(|(id, o)| OrganismView {
                id,
                pos: o.pos,
                heading: o.heading,
                lobes: o.phenotype.lobes.clone(),
                hue: o.phenotype.hue,
                mode: o.mode,
                fed: o.fed_this_tick,
                juvenile: o.structure < 0.7 * o.phenotype.structure_adult,
                form: o.phenotype.form,
                gestation: o.escrow.as_ref().map(|e| {
                    // A zero-tick gestation (a config that rounds below one tick) is due
                    // the moment it starts, so it reads as complete rather than as NaN.
                    if gestation_ticks == 0 {
                        1.0
                    } else {
                        (tick.saturating_sub(e.started_tick) as f64 / gestation_ticks as f64)
                            .clamp(0.0, 1.0) as f32
                    }
                }),
                moved: self
                    .moved
                    .get(id.slot as usize)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();
        RenderView {
            tick: self.state.tick,
            producer: self.state.fields.p.clone(),
            detritus: self.state.fields.d.clone(),
            fruit: self.state.fields.f.clone(),
            wood: self.state.ecology.wood.clone(),
            plant_reserve: self.state.ecology.plant_reserve.clone(),
            dead_wood: self.state.ecology.dead_wood.clone(),
            carrion: self.state.ecology.carrion.clone(),
            water: self.state.fields.w.clone(),
            rain: self.rain.to_vec(),
            producer_max: self.state.config.producer.max,
            organisms,
        }
    }

    /// Take the births and deaths committed since the last drain, in commit order (a tick's
    /// deaths before its births, each in slot order). The buffer is transient: nothing in the
    /// world reads it back, and a host that never drains it simply lets it grow.
    pub fn drain_events(&mut self) -> Vec<LifeEvent> {
        std::mem::take(&mut self.events)
    }

    /// Every cell's material fields plus its live organism count, for the observer's
    /// field dump. Cells are in `CellId` index order, 1,280 entries each.
    pub fn field_dump(&self) -> FieldDump {
        let mut organisms = vec![0u16; CELL_COUNT];
        for (_, o) in self.state.organisms.iter() {
            organisms[cell_of(&o.pos).index()] += 1;
        }
        let fields = &self.state.fields;
        let eco = &self.state.ecology;
        FieldDump {
            tick: self.state.tick,
            n: fields.n.clone(),
            p: fields.p.clone(),
            d: fields.d.clone(),
            de: fields.de.clone(),
            w: fields.w.clone(),
            f: fields.f.clone(),
            wood: eco.wood.clone(),
            plant_reserve: eco.plant_reserve.clone(),
            dead_wood: eco.dead_wood.clone(),
            carrion: eco.carrion.clone(),
            carrion_energy: eco.carrion_energy.clone(),
            organisms,
        }
    }

    /// Each cell's four graph neighbors in `Edge` order (`Top, Right, Bottom, Left`), as raw
    /// cell indices with `None` at the open rim. Static for the life of the world: an
    /// analyzer reads it once and computes graph distances without this crate.
    pub fn cell_neighbors(&self) -> Vec<[Option<u16>; 4]> {
        CellId::all()
            .map(|cell| {
                let n = self.graph.neighbors(cell);
                std::array::from_fn(|e| n[e].map(|c| c.0))
            })
            .collect()
    }

    /// Produce a telemetry sample and reset the per-sample counters.
    pub fn telemetry(&mut self) -> Telemetry {
        let mass_residual = self.mass_residual();
        let (mut hunters, mut hunter_juveniles) = (0u32, 0u32);
        for m in &self.state.hunters.members {
            if let Some(o) = self.state.organisms.get(m.id) {
                hunters += 1;
                if o.structure < 0.7 * o.phenotype.structure_adult {
                    hunter_juveniles += 1;
                }
            }
        }
        let mut population_by_face = [0u32; 5];
        let mut occupied = vec![false; CELL_COUNT];
        let mut occupied_cells = 0;
        let mut escrows = 0;
        let (mut organism_material, mut organism_energy) = (0.0, 0.0);
        let (mut mode_resting, mut mode_seeking, mut mode_feeding) = (0, 0, 0);
        let mut population_by_form = [0u32; 8];
        let mut height_by_form = [0.0f64; 8];
        for (_, o) in self.state.organisms.iter() {
            population_by_face[o.pos.face.index()] += 1;
            let form = (o.phenotype.form as usize).min(7);
            population_by_form[form] += 1;
            height_by_form[form] += o.pos.embed()[1];
            let cell = cell_of(&o.pos).index();
            if !occupied[cell] {
                occupied[cell] = true;
                occupied_cells += 1;
            }
            if o.escrow.is_some() {
                escrows += 1;
            }
            organism_material += o.material();
            organism_energy += o.energy;
            match o.mode {
                Mode::Resting => mode_resting += 1,
                Mode::Seeking => mode_seeking += 1,
                Mode::Feeding => mode_feeding += 1,
            }
        }
        let fields = &self.state.fields;
        let mut producer_by_face = [0.0f64; 5];
        let mut detritus_by_face = [0.0f64; 5];
        let mut water_by_face = [0.0f64; 5];
        for cell in CellId::all() {
            let face = cell.face().index();
            producer_by_face[face] += fields.p[cell.index()];
            detritus_by_face[face] += fields.d[cell.index()];
            water_by_face[face] += fields.w[cell.index()];
        }
        let mean_height_by_form = std::array::from_fn(|i| {
            if population_by_form[i] > 0 {
                height_by_form[i] / f64::from(population_by_form[i])
            } else {
                0.0
            }
        });
        let eco = &self.state.ecology;
        let alive_min = self.state.config.plant.alive_min;
        let (mut bare_cells, mut establishing_cells) = (0u32, 0u32);
        for &w in &eco.wood {
            match crate::fields::CellClass::of(w, alive_min) {
                crate::fields::CellClass::Bare => bare_cells += 1,
                crate::fields::CellClass::Establishing => establishing_cells += 1,
                crate::fields::CellClass::Alive => {}
            }
        }
        let sample = Telemetry {
            tick: self.state.tick,
            population: self.state.organisms.len() as u32,
            births: self.counters.births,
            deaths_starvation: self.counters.deaths[0],
            deaths_age: self.counters.deaths[1],
            deaths_collapse: self.counters.deaths[2],
            escrows,
            cap_rejections: self.counters.cap_rejections,
            nutrient: fields.n.iter().sum(),
            producer: fields.p.iter().sum(),
            detritus: fields.d.iter().sum(),
            detritus_energy: fields.de.iter().sum(),
            fruit: fields.f.iter().sum(),
            organism_material,
            organism_energy,
            light_in: self.counters.light_in,
            heat_out: self.counters.heat_out,
            mass_residual,
            population_by_face,
            producer_by_face,
            detritus_by_face,
            water: fields.w.iter().sum(),
            water_by_face,
            rain_in: self.counters.rain_in,
            evap_out: self.counters.evap_out,
            occupied_cells,
            travel_fallbacks: self.counters.travel_fallbacks,
            travel_ties: self.counters.travel_ties,
            pairs_considered: self.neighbors.pairs_considered,
            pairs_unfolded: self.neighbors.pairs_unfolded,
            neighbor_truncations: self.neighbors.lists_truncated,
            mode_resting,
            mode_seeking,
            mode_feeding,
            population_by_form,
            mean_height_by_form,
            state_hash: snapshot::state_hash(&self.state),
            ecology_hash: snapshot::ecology_hash(&self.state),
            care_admitted_seq: self.state.care.admitted_seq,
            care_feed_material_in: self.state.care.feed_material_in,
            care_feed_energy_in: self.state.care.feed_energy_in,
            care_rain_depth_in: self.state.care.rain_depth_in,
            care_clean_material_out: self.state.care.clean_material_out,
            care_clean_energy_out: self.state.care.clean_energy_out,
            care_allowance_used: self.state.care.allowance_used,
            hunters,
            hunter_juveniles,
            hunter_attacks: self.counters.hunter_attacks,
            hunter_captures: self.counters.hunter_captures,
            deaths_predation: self.counters.deaths_predation,
            hunter_gut_material: self.state.hunters.gut_material_total(),
            hunter_gut_energy: self.state.hunters.gut_energy_total(),
            hunter_attacks_total: self.state.hunters.attacks_total,
            hunter_captures_total: self.state.hunters.captures_total,
            predation_deaths_total: self.state.hunters.predation_deaths_total,
            hunter_births_total: self.state.hunters.hunter_births_total,
            hunter_deaths_total: self.state.hunters.hunter_deaths_total,
            wood: eco.wood.iter().sum(),
            plant_reserve: eco.plant_reserve.iter().sum(),
            dead_wood: eco.dead_wood.iter().sum(),
            carrion: eco.carrion.iter().sum(),
            carrion_energy: eco.carrion_energy.iter().sum(),
            bare_cells,
            establishing_cells,
            plant_deaths: eco.plant_deaths_total,
            recolonisations: eco.recolonisations_total,
            hunter_material_in: self.state.hunters.imported_material(),
            hunter_energy_in: self.state.hunters.imported_energy(),
        };
        self.counters = TickCounters::default();
        self.neighbors.pairs_considered = 0;
        self.neighbors.pairs_unfolded = 0;
        self.neighbors.lists_truncated = 0;
        sample
    }

    pub fn config(&self) -> &WorldConfig {
        &self.state.config
    }

    pub fn tick(&self) -> u64 {
        self.state.tick
    }

    pub fn population(&self) -> usize {
        self.state.organisms.len()
    }

    /// The 70-scalar observation this body's policy would see on its next controller tick,
    /// built by the world's own sampler from the current state.
    ///
    /// A development and fixture accessor: it takes no step, consumes no draw, changes
    /// nothing, and works for **any** live ordinary body, neural or not — what it reports is a
    /// property of where the body is standing, not of whether a policy is attached. The
    /// neighbour list it reads is the one the last pair pass built, so a fixture that has just
    /// moved bodies by hand should step once (or call it after a step) to see them.
    ///
    /// Feedback channels (indices 64–69) come from the animal's accumulated interval when it
    /// has one, and are the birth-tick values otherwise.
    pub fn neural_observation(
        &self,
        id: crate::ids::OrganismId,
    ) -> Option<crate::neural::Observation70> {
        let o = self.state.organisms.get(id)?;
        let cfg = &self.state.config;
        let dt = crate::DT;
        let here = cubarium_surface::cell_of(&o.pos).index();
        let wading = 1.0 + self.state.fields.w[here] * (1.0 - o.phenotype.swim);
        let bill = crate::motor::MotorBill::of(o, cfg);
        let u_full =
            (o.phenotype.speed_max / wading).min(bill.affordable_motor(o.energy, dt));
        let radius_px = crate::motor::turn_radius_px(o, None);
        let feedback = self.state.neural.get(id).map_or([0.0, 0.0, 0.0, 0.0, 0.0, 1.0], |a| {
            a.feedback.channels(o.phenotype.mouth_rate, o.phenotype.speed_max, radius_px, dt)
        });
        let mut cells = Vec::new();
        let mut bodies = Vec::new();
        Some(super::step::sample_observation(
            o,
            self.state.tick,
            dt,
            cfg,
            cfg.organism.reserve_energy_density,
            &self.state.fields,
            &self.state.ecology,
            &self.light,
            &self.images,
            &self.sense_rings[here],
            self.neighbors.lists.get(id.slot as usize).map_or(&[][..], |l| &l[..]),
            u_full,
            feedback,
        &mut cells,
            &mut bodies,
        ))
    }

    /// The recurrent extension, for a development tool or a fixture to read.
    pub fn neural(&self) -> &crate::neural::NeuralState {
        &self.state.neural
    }

    /// How many **live** organisms carry a recurrent policy. Counted against the organism
    /// slots rather than read off the extension's length, so an entry whose body has died
    /// can never be reported as an animal that is still out there.
    pub fn neural_population(&self) -> usize {
        self.state
            .neural
            .animals
            .iter()
            .filter(|(id, _)| self.state.organisms.get(*id).is_some())
            .count()
    }

    /// **Explicit** development access: attach `policy` to one live ordinary body, giving it
    /// fresh private state at the current tick.
    ///
    /// Nothing in the world ever calls this. Loading a snapshot never does it either: a world
    /// becomes neural only because a tool or a fixture said so, by name, for a named body.
    /// The refusals are the contract's: a foreign digest, an apex member, an enabled quiet
    /// policy, and a body that is not there.
    pub fn attach_neural_policy(
        &mut self,
        id: crate::ids::OrganismId,
        policy: crate::neural::Policy,
    ) -> Result<(), String> {
        policy.validate()?;
        if self.state.organisms.get(id).is_none() {
            return Err(format!("organism {}:{} is not alive", id.slot, id.generation));
        }
        if self.state.hunters.contains(id) {
            return Err(format!(
                "organism {}:{} is an apex member: the apex sensory and action extensions do \
                 not exist in this interface version",
                id.slot, id.generation
            ));
        }
        if self.state.quiet.policy.enabled() {
            return Err(
                "the ordinary quiet extension and neural animals cannot be enabled together"
                    .into(),
            );
        }
        let index = self.state.neural.intern(policy);
        let birth_tick = self
            .state
            .organisms
            .get(id)
            .map_or(self.state.tick, |o| o.born_tick);
        self.state
            .neural
            .insert(id, crate::neural::AnimalState::fresh(birth_tick, index));
        self.state.validate()
    }
}
