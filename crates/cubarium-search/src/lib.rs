//! Headless whole-ecosystem parameter search over the real `cubarium-core` tick loop.
//!
//! This crate owns the search harness and nothing else: it never touches the viewer, the
//! host's controls, or the shipped world parameters. It builds ordinary worlds with
//! [`cubarium_core::World::new`], advances them with [`cubarium_core::World::step`], and reads
//! the same conservation audits the host does.
//!
//! - [`params`]: the joint parameter vector, its bounds, and what is deliberately excluded.
//! - [`evaluate`]: one candidate on one seed, to a hard tick horizon, with component metrics.
//! - [`metrics`]: the component metrics and the scalar rank derived from them.
//! - [`search`]: the bounded genetic search and every limit it runs under.
//! - [`rng`]: the search's own keyed randomness, independent of the world's.

#![forbid(unsafe_code)]

pub mod evaluate;
pub mod metrics;
pub mod params;
pub mod rng;
pub mod search;

pub use evaluate::{Evaluation, Protocol, Status, evaluate};
pub use metrics::{Components, Objectives, Scoring};
pub use search::{Budget, SearchReport, StopReason, Variation};
