//! cubarium-core — the deterministic fixed-step world.
//!
//! Implements `design/m2-world-spec.md`. The step function uses no wall clock, I/O,
//! network, display, or ambient randomness: given a `WorldState` and a `WorldConfig`,
//! `World::step` is a pure function of its inputs plus counter-based keyed draws.
//! Rendering reads `RenderView` snapshots; persistence encodes `WorldState` with an
//! explicit schema version.
//!
//! Module map:
//! - [`accounting`]: compensated accumulation for the persisted cumulative energy ledgers.
//! - [`care`]: the optional bounded feed / rain / clean commands and their ledgers.
//! - [`hunter`]: the opt-in paid hunter extension (off by default, empty and inert).
//! - [`config`]: every rate and bound, serde-loadable, with the spec's initial values as defaults.
//! - [`rng`]: counter-based keyed draws partitioned by stream.
//! - [`ids`]: generation-checked organism slots.
//! - [`genome`]: bounded genome v1 and phenotype decode.
//! - [`habitat`]: static habitat and slow weather → light/moisture per cell.
//! - [`fields`]: `N`, `P`, `D`, `De` and their reactions.
//! - [`organism`]: organism state, modes, escrow.
//! - [`controller`]: observation → decision (pure).
//! - [`events`]: transient birth/death records for the observer.
//! - [`pairs`]: chord-filtered all-pairs neighbor lists.
//! - [`world`]: the tick, invariants, births/deaths, telemetry.
//! - [`snapshot`]: header + postcard encoding, validation.
//! - [`view`]: immutable render view.

#![forbid(unsafe_code)]

pub mod accounting;
pub mod care;
pub mod config;
pub mod controller;
pub mod events;
pub mod fields;
pub mod genome;
pub mod habitat;
pub mod hunter;
pub mod ids;
pub mod organism;
pub mod pairs;
pub mod quiet;
pub mod rng;
pub mod snapshot;
pub mod telemetry;
pub mod view;
pub mod water;
pub mod world;

pub use accounting::{EnergyCorrection, EnergyLedgers, Ledger};
pub use care::{
    ActiveShower, CareApplied, CareCommand, CareDose, CareKind, CareOutcome, CareReceipt,
    CareState, CareTarget,
};
pub use config::WorldConfig;
pub use events::LifeEvent;
pub use hunter::{
    AttemptOutcome, EscrowKey, FixedHunterProfile, FundingBlocked, HunterControlReceipt,
    HunterEvent, HunterFounderReceipt, HunterMember, HunterPhase, HunterRole, HunterState,
    HunterTarget, HunterView, OxidationPolicy, Reproduction,
};
pub use ids::OrganismId;
pub use snapshot::{
    SCHEMA_V7, SCHEMA_V8, SCHEMA_V9, SCHEMA_V10, SCHEMA_V11, SCHEMA_V12, SCHEMA_VERSION, SnapshotError,
    WorldStateV7, WorldStateV8, WorldStateV9, WorldStateV10, WorldStateV11, WorldStateV12, decode_snapshot,
    ecology_hash, encode_snapshot,
};
pub use telemetry::Telemetry;
pub use view::RenderView;
pub use world::{ChargingDiagnostics, World, WorldState};

/// Simulation ticks per second.
pub const TICK_HZ: u32 = 20;
/// Seconds per tick.
pub const DT: f64 = 1.0 / TICK_HZ as f64;
