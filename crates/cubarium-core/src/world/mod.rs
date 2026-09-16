//! The world: checkpointed state plus transient caches, and the tick.

mod budget;
mod care;
mod hunter;
mod invariants;
mod lifecycle;
mod state;
mod step;
#[cfg(test)]
mod tests;
mod view;

use cubarium_surface::{CellId, ChartImage, FieldGraph, ScalarField, Scale, Topology, Travel};

use crate::diagnostic::ScriptedIntent;
use crate::dormancy::ApexDormancyEvent;
use crate::encounter::{ApexEncounterEvent, ApexOpportunity};
use crate::events::LifeEvent;
use crate::habitat::Habitat;
use crate::hunter::HunterEvent;
use crate::pairs::NeighborLists;
use crate::quiet::QuietEvent;

pub use budget::{
    BodyBudget, BudgetRecorder, CARRION, CHANNEL_NAMES, CHANNELS, FOLIAGE, FRUIT, LITTER,
    MAX_CLOSED_RECORDS,
};
pub use crate::fields::{CellClass, EcologyV1State};
pub use lifecycle::{TRAINING_FOUNDER_HUE, TRAINING_START_ENERGY, TRAINING_START_RESERVE};
pub(crate) use state::check_genome;
pub use state::{ChargingDiagnostics, IntakeDiagnostics, NeuralTiming, TickCounters, WorldState};

pub(super) const CELL_UNFOLD_RADIUS: f64 = 20.0;
pub(super) const SENSE_DEPTH_MAX: usize = 3;
pub(super) const BIRTH_DRAWS: u64 = 16;
/// The per-tick energy/water audits that read this are debug-only.
#[cfg(debug_assertions)]
pub(super) const AUDIT_TOLERANCE: f64 = 1e-9;
pub(super) const GRADIENT_EPS: f64 = 1e-9;
pub(super) const HEADING_TOLERANCE: f64 = 1e-6;
pub(super) const HEADING_REPAIR_FLOOR: f64 = 1e-12;

pub struct World {
    pub state: WorldState,
    pub(crate) graph: FieldGraph,
    pub(crate) habitat: Habitat,
    /// One entry per chart of the topology, indexed by [`Topology::chart_index`]: five on
    /// the cube, one on a ring.
    pub(crate) images: Vec<Vec<ChartImage>>,
    /// The five per-cell weather and habitat caches. Sized at runtime from
    /// `topology.cell_count(scale)`: `1,280 = 2^8·5` is the cube's count, not the world's
    /// (`design/flat-world-plan-2026-09-16.md` §2).
    pub(crate) light: Box<[f64]>,
    pub(crate) moisture: Box<[f64]>,
    pub(crate) rain_source: Box<[f64]>,
    pub(crate) rain: Box<[f32]>,
    pub(crate) manual_rain: Box<[f64]>,
    pub(crate) rain_envelope: [f64; crate::care::RAIN_SAMPLES],
    pub(crate) scratch: (ScalarField, ScalarField),
    /// Reusable working storage for the ecology v1 cross-cell subphases 3f and 3h, so a long
    /// headless run allocates none of it per tick.
    pub(crate) eco_scratch: crate::fields::EcoScratch,
    pub(crate) water_scratch: ScalarField,
    pub(crate) sense_rings: Vec<[Vec<CellId>; SENSE_DEPTH_MAX]>,
    pub(crate) neighbors: NeighborLists,
    pub(crate) travel_buf: Travel,
    pub(crate) moved: Vec<Vec<cubarium_surface::PathSegment>>,
    pub(crate) events: Vec<LifeEvent>,
    pub(crate) hunter_events: Vec<HunterEvent>,
    pub(crate) quiet_events: Vec<QuietEvent>,
    pub(crate) apex_dormancy_events: Vec<ApexDormancyEvent>,
    pub(crate) apex_encounter_events: Vec<ApexEncounterEvent>,
    pub(crate) counters: TickCounters,
    pub(crate) charging: ChargingDiagnostics,
    pub(crate) intake: IntakeDiagnostics,
    /// Per-organism store budgets (`budget`). Off by default; transient, never persisted,
    /// never hashed, never read back by the tick.
    pub(crate) budgets: BudgetRecorder,
    /// Why two adult apex members did or did not mate (`crate::encounter::ApexOpportunity`).
    /// Written only inside the apex-encounter pass; transient like every field above.
    pub(crate) apex_opportunity: ApexOpportunity,
    /// Transient timing of the recurrent stage (`crate::neural`). Development measurement
    /// only: never persisted, never hashed, never read by the tick.
    pub(crate) neural_timing: NeuralTiming,
    /// Transient diagnostic intent overrides (`crate::diagnostic`). Empty in every ordinary
    /// world, never persisted, never set by the world itself.
    pub(crate) scripted: Vec<(crate::ids::OrganismId, ScriptedIntent)>,
    pub(crate) initial_material: f64,
}

impl World {
    /// The shape of this world's surface. Every geometric call in the crate reads it rather
    /// than assuming the cube (`design/flat-world-plan-2026-09-16.md` §2).
    #[inline]
    pub fn topology(&self) -> Topology {
        self.state.config.topology
    }

    /// This world's scale `S`. A cube is always [`Scale::ONE`]; a ring may be any validated
    /// multiplier, and it is what turns pixels into cells and embedded units.
    #[inline]
    pub fn scale(&self) -> Scale {
        self.state.config.world_scale
    }

    /// Cells over the whole surface: `topology().cell_count(scale())`, and the length of
    /// every per-cell vector this world holds.
    #[inline]
    pub fn cell_count(&self) -> usize {
        self.graph.cell_count()
    }
}
