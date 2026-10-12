//! **Kin-weighted training** (KIN-TRAIN, the trainer half of package 8 of
//! `design/behaviour-ai-2026-10-11.md`; §0 topics 2 and 10, §10.3, §10.4, §11.7).
//!
//! - [`roster`]: who counts as whose relative over a window or episode.

pub mod roster;

#[cfg(test)]
mod tests;

pub use roster::{KinRecord, Relation, Roster};
