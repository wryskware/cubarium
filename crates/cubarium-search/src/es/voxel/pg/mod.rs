//! **PG-PILOT: per-decision policy gradient on the top's intent choice**
//! (`design/handoffs/pg-pilot-2026-10-14.md`; option D of
//! `design/7_Research/training-review-2026-10-14.md` §4).
//!
//! A lineage on [`crate::es::voxel::kin::Comparison::PolicyGradient`] trains from its own
//! live windows, no variants and no twin copies:
//!
//! - [`record`]: during a window every body of the lineage runs the centre with its top
//!   **sampling** its intent proposal at the arbitration temperature
//!   ([`crate::es::voxel::two_layer::Explore`]); each decision is logged with what the top
//!   read (GRU state, held intent, time held), the menu mask, the sampled intent, its
//!   log-probability and its tick; each body's energy is read every tick;
//! - [`reward`]: the per-decision **reward interface**: terms over a body's tick span,
//!   summed per decision. Today's score is two terms, its own net energy (the loss at death
//!   included) and the kin shares of its relatives' (§10.3); more terms (novelty) join here
//!   without touching the learner;
//! - [`learner`]: returns with a time discount, a value baseline on the top's input, GAE,
//!   and a PPO-style clipped advantage-weighted log-prob step on the top's intent head (the
//!   bottom frozen: stage 1, so no backprop through time), plus the split-half cosine
//!   readout (the review's yardstick);
//! - [`knobs`]: the learner's knobs, the kin file's `[<lineage>.pg]` table.

pub mod knobs;
pub mod learner;
pub mod record;
pub mod reward;

#[cfg(test)]
mod tests;

pub use knobs::PgKnobs;
pub use learner::{PgReadout, PgState, Policy, Step, TopInput, Trajectory};
pub use record::{ExplorePlan, Recorder};
pub use reward::{KinEnergy, OwnEnergy, PgBody, PgWorld, RewardTerm};
