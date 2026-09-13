//! Cubarium host library: clock, deterministic fixture scenes, output sinks, the
//! preview's cube ray-caster, and the persistent M2 world loop (state directory,
//! checkpoint worker, presentation). The binary in `src/main.rs` is a thin wrapper
//! around [`run`]; everything else lives here so the integration tests can exercise it
//! without a subprocess.
//!
//! See `crates/cubarium/README.md` for the command contract this implements. The host
//! owns no geometry: every spatial operation goes through `cubarium-surface` and every
//! pixel through `cubarium-render`.

// `deny` rather than `forbid`: exactly one place in this crate needs `unsafe`, the
// `flock(2)` call in [`state::StateLock`], and it carries its own `#[allow]` with the
// argument for why it is sound. Everything else in the host stays unsafe-free.
#![deny(unsafe_code)]

pub mod art;
pub mod art_present;
pub mod care;
pub mod cli;
pub mod clock;
pub mod net;
pub mod present;
pub mod raycast;
pub mod rng;
pub mod runner;
pub mod scene;
pub mod sink;
pub mod state;

mod run;

pub use run::run;
pub use runner::{RunOutcome, run_world, run_world_until};
