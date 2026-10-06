//! **The whole food web as stocks and flows** (`design/webmodel/README.md`): a fast
//! mean-field model of the voxel ecology. It reads every rule-level number from the same
//! config the sim reads, and fits only behaviour-emergent rates from sim telemetry.
//!
//! Two halves meet at [`exchange`]: the producers and the detritus offer a [`Supply`];
//! the animals return a [`Take`] and what they give back ([`Returns`]). Each half can run
//! on its own against the other half's recorded sim values (open loop), then together.

pub mod exchange;
pub mod fauna;
pub mod flora;
pub mod web;

pub use exchange::{Returns, Supply, Take};
