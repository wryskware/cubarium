//! **Kin-weighted training** (KIN-TRAIN, the trainer half of package 8 of
//! `design/behaviour-ai-2026-10-11.md`; §0 topics 2 and 10, §10.3, §10.4, §11.7).
//!
//! - [`roster`]: who counts as whose relative over a window or episode.
//! - [`config`]: the per-lineage knobs: shares, the comparison switch, the brain stage.
//! - [`score`]: the kin-weighted score and its parts.
//! - [`compare`]: the comparison switch's scorers, family placement, the pair update.
//! - [`matched`]: the matched-copies runner over any scene, and the null pair.

pub mod compare;
pub mod config;
pub mod matched;
pub mod roster;
pub mod score;

#[cfg(test)]
mod tests;

pub use compare::{PairScores, ascend_pairs, place_families};
pub use config::{BrainStage, Comparison, KinConfig, KinShares, LineageKin};
pub use roster::{KinRecord, Relation, Roster};
pub use score::{KinParts, kin_scores};
