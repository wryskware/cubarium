//! Cubarium host library: clock, deterministic fixture scenes, output sinks, and the
//! preview's cube ray-caster. The binary in `src/main.rs` is a thin wrapper around
//! [`run`]; everything else lives here so the integration tests can exercise it.
//!
//! See `crates/cubarium/README.md` for the command contract this implements. The host
//! owns no geometry: every spatial operation goes through `cubarium-surface` and every
//! pixel through `cubarium-render`.

#![forbid(unsafe_code)]

pub mod clock;
pub mod cli;
pub mod net;
pub mod raycast;
pub mod rng;
pub mod scene;
pub mod sink;

mod run;

pub use run::run;
