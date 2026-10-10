//! **The animal half** (`design/webmodel/fauna.md`): every founder lineage as a
//! stage-structured population — eggs (and the lanternjaw's dormant bank), juveniles,
//! adults, parasite riders — built from the sim's own individual rules and the numbers in
//! the config the sim reads.
//!
//! - [`config`]: the scenario file with its `base` chain, resolved through the sim's own
//!   registry ([`cubarium_voxel_sim::params`]).
//! - [`laws`]: the individual budget, each rule with its `file:line` in the fauna crate.
//! - [`fits`]: the rates that emerge from behaviour in space, fitted from telemetry.
//! - [`model`]: the cohorts, the step, the hunt, births and deaths; the mean field and the
//!   stochastic twin on one code path ([`rng::Draws`]).

pub mod config;
pub mod behavior;
mod sync;
pub mod fits;
pub mod laws;
pub mod model;
pub mod rng;

pub use fits::{Fits, LineageFit, Response};
pub use laws::{Body, Flows};
pub use model::{Census, Cause, Cohort, EggCohort, FaunaModel, LineageLedger};
pub use rng::{Draws, Rng};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_size_not_mass;
#[cfg(test)]
mod tests_held;
