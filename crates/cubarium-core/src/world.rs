//! The world: checkpointed state plus transient caches, and the tick.

use serde::{Deserialize, Serialize};

use cubarium_surface::{CELL_COUNT, ChartImage, FieldGraph, ScalarField, Travel};

use crate::config::WorldConfig;
use crate::fields::Fields;
use crate::habitat::{Habitat, Weather};
use crate::ids::Slots;
use crate::organism::{DeathCause, Organism};
use crate::pairs::NeighborLists;
use crate::telemetry::Telemetry;
use crate::view::RenderView;

/// Everything a checkpoint must capture. Transient caches are rebuilt on load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldState {
    pub config: WorldConfig,
    pub tick: u64,
    pub fields: Fields,
    pub weather: Weather,
    pub organisms: Slots<Organism>,
    /// Cumulative counters since world creation.
    pub births_total: u64,
    pub deaths_total: [u64; 3],
    pub cap_rejections_total: u64,
    /// Material admitted from outside (founders and any future stimuli), for the invariant.
    pub external_material_in: f64,
    /// Running energy audit.
    pub light_in_total: f64,
    pub heat_out_total: f64,
}

impl WorldState {
    /// Range checks after decode: finite numbers, nonnegative stocks, canonical positions,
    /// unit headings (renormalize if within 1e-6, else invalid), population ≤ cap, escrows
    /// nonnegative, genome fields in range, config valid.
    pub fn validate(&self) -> Result<(), String> {
        todo!("WorldState::validate")
    }
}

/// Per-tick counters exposed to telemetry and reset each sample.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickCounters {
    pub births: u32,
    pub deaths: [u32; 3],
    pub cap_rejections: u32,
    pub light_in: f64,
    pub heat_out: f64,
    pub travel_fallbacks: u32,
    pub travel_ties: u32,
}

#[allow(dead_code)] // caches are wired by the tick implementation
pub struct World {
    pub state: WorldState,
    graph: FieldGraph,
    habitat: Habitat,
    images: [Vec<ChartImage>; 5],
    light: Box<[f64; CELL_COUNT]>,
    moisture: Box<[f64; CELL_COUNT]>,
    scratch: (ScalarField, ScalarField),
    neighbors: NeighborLists,
    travel_buf: Travel,
    /// This tick's traveled segments per slot, for the render view.
    moved: Vec<Vec<cubarium_surface::PathSegment>>,
    counters: TickCounters,
    initial_material: f64,
}

impl World {
    /// Create a new world: validate config, build habitat/weather, initial fields, and
    /// `founders.count` founders placed uniformly by area (rejection sampling on the five
    /// faces from `Stream::Founders`, key = index, counters 0..3 for face/u/v/heading and
    /// 4 for hue), each with the founder genome, `S = S_adult`,
    /// `R = initial_reserve_fraction · R_max`, `E = initial_energy_fraction · E_max`.
    /// Founder material is recorded in `external_material_in`.
    pub fn new(config: WorldConfig) -> Result<World, String> {
        let _ = config;
        todo!("World::new")
    }

    /// Rebuild caches around a validated state (after a snapshot load).
    pub fn from_state(state: WorldState) -> Result<World, String> {
        let _ = state;
        todo!("World::from_state")
    }

    /// One tick in the normative order of `design/m2-world-spec.md` "Tick order".
    /// Returns the per-tick counters (also accumulated internally for telemetry).
    pub fn step(&mut self) -> &TickCounters {
        todo!("World::step")
    }

    /// Mass invariant: `Σ fields + Σ organisms (incl. escrow) − external_material_in − initial`.
    /// Should stay within `1e-9 · initial` per hour of simulated time; telemetry reports it.
    pub fn mass_residual(&self) -> f64 {
        todo!("World::mass_residual")
    }

    /// Full invariant check (fields finite/nonnegative, organisms finite, positions canonical,
    /// escrows consistent, population ≤ cap). Called every tick in debug builds and every
    /// telemetry sample in release; a failure is a fatal implementation error.
    pub fn check_invariants(&self) -> Result<(), String> {
        todo!("World::check_invariants")
    }

    pub fn render_view(&self) -> RenderView {
        todo!("World::render_view")
    }

    /// Produce a telemetry sample and reset the per-sample counters.
    pub fn telemetry(&mut self) -> Telemetry {
        todo!("World::telemetry")
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

    /// Death causes in `deaths_total` order.
    pub const DEATH_CAUSES: [DeathCause; 3] = [DeathCause::Starvation, DeathCause::Age, DeathCause::Collapse];
}
