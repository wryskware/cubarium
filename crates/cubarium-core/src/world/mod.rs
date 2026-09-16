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
    /// Per-attempt apex strike records (`crate::hunter::StrikeRecorder`). Off by default;
    /// transient, never persisted, never hashed, never read back by the tick.
    pub(crate) strikes: crate::hunter::StrikeRecorder,
    /// Why two adult apex members did or did not mate (`crate::encounter::ApexOpportunity`).
    /// Written only inside the apex-encounter pass; transient like every field above.
    pub(crate) apex_opportunity: ApexOpportunity,
    /// Transient timing of the recurrent stage (`crate::neural`). Development measurement
    /// only: never persisted, never hashed, never read by the tick.
    pub(crate) neural_timing: NeuralTiming,
    /// Transient diagnostic intent overrides (`crate::diagnostic`). Empty in every ordinary
    /// world, never persisted, never set by the world itself.
    pub(crate) scripted: Vec<(crate::ids::OrganismId, ScriptedIntent)>,
    /// The motor contract this world runs (`crate::motor::MotorModel`). Transient like every
    /// field above: never persisted, never hashed, never in [`WorldConfig`]. The default is
    /// [`crate::motor::MotorModel::Sweep`], the shipped contract, so a world that never names a
    /// model is byte-identical to the build before the switch existed.
    pub(crate) motor_model: crate::motor::MotorModel,
    /// Which radius an apex member's grasp puts in the turn budget under
    /// [`crate::motor::MotorModel::Sweep`] (`crate::motor::ApexTurnRadius`). Transient like
    /// every field above: never persisted, never hashed, never in [`WorldConfig`]. The default
    /// is [`crate::motor::ApexTurnRadius::Grasp`], the shipped rule, so a world that never
    /// names one is byte-identical to the build before the switch existed.
    pub(crate) apex_turn_radius: crate::motor::ApexTurnRadius,
    /// The motor contract a **hunter member** runs, when it is not the world's own
    /// (`crate::motor::model_for_body`). Transient like every field above: never persisted,
    /// never hashed, never in [`WorldConfig`]. `None` — the default — means every body runs
    /// [`World::motor_model`], so a world that never names one is byte-identical to the build
    /// before the override existed.
    pub(crate) apex_motor_model: Option<crate::motor::MotorModel>,
    /// Which action adapter decodes a neural body's raw head this tick
    /// (`crate::neural::ActionAdapter`). Transient like every field above: never persisted,
    /// never hashed, never in [`WorldConfig`]. The default is
    /// [`crate::neural::ActionAdapter::CubAct1`], the shipped adapter, so a world that never
    /// names one is byte-identical to the build before the switch existed. It reaches no
    /// legacy body: only a neural animal has a raw head to decode.
    pub(crate) action_adapter: crate::neural::ActionAdapter,
    pub(crate) initial_material: f64,
}

impl World {
    /// Run this world under a named motor contract
    /// (`design/7_Research/ecology-v1-motor-inertial-2026-09-16.md`).
    ///
    /// Opt-in and transient: nothing in a snapshot records it, so a resumed world runs
    /// [`crate::motor::MotorModel::Sweep`] until it is told otherwise. Set it before the first
    /// tick of an experiment; changing it mid-run is legal but makes one run of two worlds.
    pub fn set_motor_model(&mut self, model: crate::motor::MotorModel) {
        self.motor_model = model;
    }

    /// The motor contract in force, [`crate::motor::MotorModel::Sweep`] unless one was named.
    pub fn motor_model(&self) -> crate::motor::MotorModel {
        self.motor_model
    }

    /// Run this world with the apex's grasp counted, or not counted, as a turn radius
    /// (`design/7_Research/ecology-v1-apex-grasp-2026-09-16.md`).
    ///
    /// Opt-in and transient exactly as [`World::set_motor_model`] is: nothing in a snapshot
    /// records it, so a resumed world runs [`crate::motor::ApexTurnRadius::Grasp`] until it is
    /// told otherwise. It reaches no ordinary body: only a hunter member has the contact
    /// geometry the rule chooses between.
    pub fn set_apex_turn_radius(&mut self, rule: crate::motor::ApexTurnRadius) {
        self.apex_turn_radius = rule;
    }

    /// The rule in force, [`crate::motor::ApexTurnRadius::Grasp`] unless one was named.
    pub fn apex_turn_radius(&self) -> crate::motor::ApexTurnRadius {
        self.apex_turn_radius
    }

    /// Run this world's **apex members** under a motor contract of their own, leaving every
    /// other body on the world's
    /// (`design/7_Research/ecology-v1-apex-motor-isolation-2026-09-16.md`).
    ///
    /// Opt-in and transient exactly as [`World::set_motor_model`] is: nothing in a snapshot
    /// records it, so a resumed world runs `None` — the world's own contract for every body —
    /// until it is told otherwise. It reaches no ordinary body: only a hunter member is handed
    /// the contact geometry [`crate::motor::model_for_body`] selects on.
    ///
    /// This is what makes a paired motor arm a measurement of the apex's envelope alone: the
    /// prey run the world's own contract in both arms, so the world the founders are introduced
    /// into is the same world, row for row.
    pub fn set_apex_motor_model(&mut self, model: Option<crate::motor::MotorModel>) {
        self.apex_motor_model = model;
    }

    /// The contract a member runs when it is not the world's own; `None` unless one was named.
    pub fn apex_motor_model(&self) -> Option<crate::motor::MotorModel> {
        self.apex_motor_model
    }

    /// Run this world's neural animals under a named action adapter
    /// (`design/7_Research/ecology-v1-turn-deadband-2026-09-16.md`).
    ///
    /// Opt-in and transient exactly as [`World::set_motor_model`] is: nothing in a snapshot
    /// records it, so a resumed world runs [`crate::neural::ActionAdapter::CubAct1`] — the
    /// shipped adapter, and the display host's — until it is told otherwise. Set it **before**
    /// attaching a policy: `World::attach_neural_policy` refuses, by name, a policy whose
    /// schema digest is not the adapter in force, so a world that changes adapter after
    /// attaching is refused at the next attachment rather than quietly running the wrong
    /// decode. Changing it mid-run is legal and makes one run of two animals.
    pub fn set_action_adapter(&mut self, adapter: crate::neural::ActionAdapter) {
        self.action_adapter = adapter;
    }

    /// The adapter in force, [`crate::neural::ActionAdapter::CubAct1`] unless one was named.
    pub fn action_adapter(&self) -> crate::neural::ActionAdapter {
        self.action_adapter
    }
}
