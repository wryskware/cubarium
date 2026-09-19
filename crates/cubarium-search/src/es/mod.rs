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
//! - [`antithetic`]: the antithetic-pair reduction over the retained generation reports,
//!   and the deadband occupancy of a sigma-scale perturbation.
//! - [`bits`]: exact float persistence, because plain JSON numbers are not exact here.
//! - [`rng`]: the trainer's own positional randomness, separate from the world's.
//! - [`tensor`]: the flatten/unflatten order and the seeded centre.
//! - [`optimizer`]: centred-rank utilities, the antithetic gradient estimate, Adam ascent.
//! - [`fixture`]: the frozen training and held-out layouts, and how one becomes a world.
//! - [`episode`]: one rollout through `World::step`, plus the three disclosed controls.
//! - [`budget`]: the matched feasibility experiment over the world's per-organism ledger.
//! - [`intake`]: the per-tick intake diagnostic — why a trained controller travels and does
//!   not eat, separated at the sites the tick decides it.
//! - [`scorecheck`]: the two falsification checks the score hypothesis had to survive —
//!   the frozen controller's response to food, and the current score's dwell gradient.
//! - [`trainer`]: stable job identities, bounded workers, the reduction and the checkpoint.
//! - [`export`]: a trained centre as a self-contained policy the core can attach.
//! - [`turnband`]: the paired replay of one weight set under both action adapters, and the
//!   gradient-direction stability of the retained pair contributions.
//! - [`commands`]: the development commands behind the `es-*` subcommands.
//! - [`voxel`]: the **phase-one voxel slice** (P1-D) — the episode driver, controllers and
//!   ES over the static voxel arena's two founder manifests, with its own `voxel-*`
//!   command family. It reuses this module's optimizer, perturbation stream and exact
//!   float persistence; nothing else is shared with the flat trainer below.
//!
//! Nothing here attaches a policy to the display world, migrates a world, or trains during a
//! world's ordinary life. The trainer builds its own isolated worlds and throws them away.

pub mod antithetic;
pub mod bits;
pub mod budget;
pub mod commands;
pub mod episode;
pub mod export;
pub mod fixture;
pub mod intake;
pub mod optimizer;
pub mod rng;
pub mod scorecheck;
pub mod tensor;
pub mod trainer;
pub mod turnband;
pub mod voxel;

pub use episode::{Control, Driver, Episode, EpisodeError, Limits};
pub use fixture::{
    Ecology, HORIZON_TICKS, Layout, holdout_layouts, holdout_layouts_on, training_layouts,
    training_layouts_on,
};
pub use optimizer::{Adam, SIGMA};
pub use tensor::PARAMS;
pub use trainer::{
    Checkpoint, Discarded, GenerationError, GenerationReport, Plan, Protocol, STORE_WEIGHT,
    run_generation, score,
};
