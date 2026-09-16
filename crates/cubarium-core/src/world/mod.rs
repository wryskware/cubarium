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

use cubarium_surface::{CELL_COUNT, CellId, ChartImage, FieldGraph, ScalarField, Travel};

use crate::diagnostic::ScriptedIntent;
use crate::dormancy::ApexDormancyEvent;
use crate::encounter::{ApexEncounterEvent, ApexOpportunity};
use crate::events::LifeEvent;
use crate::habitat::Habitat;
use crate::hunter::HunterEvent;
use crate::pairs::NeighborLists;
use crate::quiet::QuietEvent;

pub use budget::{
    BodyBudget, BudgetRecorder, CARRION, CHANNEL_NAMES, CHANNELS, FOLIAGE, FRUIT, IntakeLimit,
    IntakeTick, LITTER, MAX_CLOSED_RECORDS, MAX_TRACE_ROWS, MOUTH_GRAZE, MOUTH_FRUIT,
    MOUTH_NAMES, MOUTH_SCAVENGE, MOUTHS,
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
    pub(crate) images: [Vec<ChartImage>; 5],
    pub(crate) light: Box<[f64; CELL_COUNT]>,
    pub(crate) moisture: Box<[f64; CELL_COUNT]>,
    pub(crate) rain_source: Box<[f64; CELL_COUNT]>,
    pub(crate) rain: Box<[f32; CELL_COUNT]>,
    pub(crate) manual_rain: Box<[f64; CELL_COUNT]>,
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
