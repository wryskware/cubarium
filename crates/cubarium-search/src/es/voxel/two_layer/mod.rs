//! **The two-layer brain** (TWO-LAYER core; `design/behaviour-ai-2026-10-11.md` §10.1,
//! §10.2, §10.4, §11.3; Wrysk's topic 3): a slow top layer picks and holds a named intent
//! and its subject, and a fast bottom layer — today's GRU — reads the senses plus that
//! intent and steers. A plain brain is a one-item menu and is today's GRU exactly.
//!
//! Everything here takes **plain inputs** (slices and this crate's structs, no fauna
//! types on the brain's side of the seam), so the adapter that joins it to the fauna's
//! intent records is a thin layer written later:
//!
//! - [`shape`]: the runtime dimensions ([`TwoLayerShape`]) and the flat layout with its
//!   ES top/bottom API ([`TwoLayerLayout`], [`Layer`]);
//! - [`net`]: the input assembly and the top's forward pass both runtime and cloning use;
//! - [`arbitration`]: the per-lineage holding knobs ([`IntentArbitration`]) and the rule;
//! - [`brain`]: one body's brain ([`TwoLayerBrain`]) and its saved state ([`BrainState`]);
//! - [`file`]: the versioned two-layer policy file and the plain-file bridge;
//! - [`clone`]: trajectory records and the teacher-forced fit of both layers;
//! - [`adapter`]: the brain behind the fauna's menu seam (`Controller::decide`, saves);
//! - [`scenes`]: the staged mode scenes as recordings, closed-loop runs and competence
//!   checks (BRAIN-NEXT).

pub mod adapter;
pub mod arbitration;
pub mod brain;
pub mod clone;
pub mod file;
pub mod net;
pub mod record;
pub mod scenes;
pub mod shape;

#[cfg(test)]
mod fauna_tests;
#[cfg(test)]
mod tests;

pub use adapter::{BRAIN_MEMORY_TAG, MenuHeads};
pub use arbitration::{EarlyEnd, HoldChange, IntentArbitration};
pub use brain::{
    BrainAnswer, BrainState, CandidateRow, DecisionSample, HeldChoice, ProposalRecord,
    SubjectRef, TwoLayerBrain, TwoLayerNet,
};
pub use file::{BrainFile, TWO_LAYER_POLICY_SCHEMA, TwoLayerPolicyFile, plain_net};
pub use net::SubjectKind;
pub use shape::{ActionTransfer, Layer, TwoLayerLayout, TwoLayerShape, initial_center};
