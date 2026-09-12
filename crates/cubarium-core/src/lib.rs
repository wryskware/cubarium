//! cubarium-core — the deterministic fixed-step world.
//!
//! Implements `design/m2-world-spec.md`. The step function uses no wall clock, I/O,
//! network, display, or ambient randomness: given a `WorldState` and a `WorldConfig`,
//! `World::step` is a pure function of its inputs plus counter-based keyed draws.
//! Rendering reads `RenderView` snapshots; persistence encodes `WorldState` with an
//! explicit schema version.
//!
//! Module map:
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

pub mod config;
pub mod controller;
pub mod events;
pub mod fields;
pub mod genome;
pub mod habitat;
pub mod ids;
pub mod organism;
pub mod pairs;
pub mod rng;
pub mod snapshot;
pub mod telemetry;
pub mod view;
pub mod water;
pub mod world;

pub use config::WorldConfig;
pub use events::LifeEvent;
pub use ids::OrganismId;
pub use snapshot::{SCHEMA_VERSION, SnapshotError, decode_snapshot, encode_snapshot};
pub use telemetry::Telemetry;
pub use view::RenderView;
pub use world::{World, WorldState};

/// Simulation ticks per second.
pub const TICK_HZ: u32 = 20;
/// Seconds per tick.
pub const DT: f64 = 1.0 / TICK_HZ as f64;
