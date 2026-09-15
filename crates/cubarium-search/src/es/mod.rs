//! The recurrent-policy trainer: antithetic Gaussian evolution strategies over the **real**
//! `cubarium-core` tick loop.
//!
//! This module is a second, independent entry point inside `cubarium-search`. It shares the
//! crate's build stamping, its worker pattern and its discipline about keyed randomness, and
//! it shares **nothing** with the M1 ecological parameter search: no neural weight enters
//! `params`, no `Feeding`-mode fitness is reused, and every M1 command behaves exactly as it
//! did. The two live together because a search harness is a search harness, not because a
//! policy is an ecology parameter.
//!
//! - [`bits`]: exact float persistence, because plain JSON numbers are not exact here.
//! - [`rng`]: the trainer's own positional randomness, separate from the world's.
//! - [`tensor`]: the flatten/unflatten order and the seeded centre.
//! - [`optimizer`]: centred-rank utilities, the antithetic gradient estimate, Adam ascent.
//! - [`fixture`]: the frozen training and held-out layouts, and how one becomes a world.
//! - [`episode`]: one rollout through `World::step`, plus the three disclosed controls.
//! - [`trainer`]: stable job identities, bounded workers, the reduction and the checkpoint.
//! - [`export`]: a trained centre as a self-contained policy the core can attach.
//! - [`commands`]: the development commands behind the `es-*` subcommands.
//!
//! Nothing here attaches a policy to the display world, migrates a world, or trains during a
//! world's ordinary life. The trainer builds its own isolated worlds and throws them away.

pub mod bits;
pub mod commands;
pub mod episode;
pub mod export;
pub mod fixture;
pub mod optimizer;
pub mod rng;
pub mod tensor;
pub mod trainer;

pub use episode::{Control, Detail, Driver, Episode};
pub use fixture::{HORIZON_TICKS, Layout, holdout_layouts, training_layouts};
pub use optimizer::{Adam, SIGMA};
pub use tensor::PARAMS;
pub use trainer::{Checkpoint, GenerationReport, Plan, Protocol, STORE_WEIGHT, run_generation, score};
