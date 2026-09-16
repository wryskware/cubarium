//! Headless whole-ecosystem parameter search over the real `cubarium-core` tick loop.
//!
//! This crate owns the search harness and nothing else: it never touches the viewer, the
//! host's controls, or the shipped world parameters. It builds ordinary worlds with
//! [`cubarium_core::World::new`], advances them with [`cubarium_core::World::step`], and reads
//! the same conservation audits the host does.
//!
//! - [`params`]: the joint parameter vector, its bounds, and what is deliberately excluded.
//! - [`apex_audit`]: why two introduced apex adults never mate, counted at the predicate.
//! - [`census`]: workstream R's depth census — F's 150-minute variety census with the roster
//!   skimmer's `depth` overridden search-side at tick 0 and nothing else changed.
//! - [`calibrate`]: the ecology v1 calibration matrix — declared candidates × seeds × matched
//!   zero/one/two-apex arms, with the ecology v1 component vector and a config export.
//! - [`depletion`]: the per-depleted-cell record — habitat quality, foliage trajectory,
//!   post-depletion pressure, and the four-way reading of why a cell stays depleted.
//! - [`evaluate`]: one candidate on one seed, to a hard tick horizon, with component metrics.
//! - [`factorial`]: the controlled form × diet factorial — cloned founders at matched cells
//!   with mutation and reproduction off, measured with the core's per-body ledger.
//! - [`metrics`]: the component metrics and the scalar rank derived from them.
//! - [`plant_budget`]: workstream M's counter split by **exact** consumer withdrawal, and the
//!   measured plant budget of the cells that cross.
//! - [`movement`]: the spatial-coupling measures — visits, residence, revisit intervals, the
//!   per-cell depletion/recovery crossing counter, and the variety census.
//! - [`search`]: the bounded genetic search and every limit it runs under.
//! - [`rng`]: the search's own keyed randomness, independent of the world's.
//!
//! It also hosts the **recurrent-policy trainer**, which is a different search over the same
//! tick loop:
//!
//! - [`es`]: antithetic evolution strategies over a GRU policy's weights (R2a). It shares this
//!   crate's build stamp and worker pattern and nothing else; every M1 command above is
//!   untouched by it.

#![forbid(unsafe_code)]

pub mod apex_audit;
pub mod calibrate;
pub mod census;
pub mod depletion;
pub mod es;
pub mod evaluate;
pub mod factorial;
pub mod metrics;
pub mod movement;
pub mod params;
pub mod plant_budget;
pub mod population;
pub mod rng;
pub mod search;

pub use evaluate::{Evaluation, Protocol, Status, evaluate};
pub use metrics::{Components, Objectives, Scoring};
pub use search::{Budget, SearchReport, StopReason, Variation};
