//! Transient life events: one record per birth and per death, drained by the observer.
//!
//! Events are not part of [`crate::WorldState`]: they are never checkpointed, never hashed,
//! and never read back by the world. They exist so E3 can reconstruct lineages — parent age
//! at birth, time to first reproduction, ancestry depth, reproductive skew — from a log the
//! simulation itself does not depend on.

use serde::Serialize;

use crate::genome::Mutation;
use crate::ids::OrganismId;
use crate::organism::{DeathCause, Origin};

/// One thing that happened to one organism, in the order the world committed it.
///
/// Every field is as of `tick`, which is the tick the world has advanced to when the event
/// is drained: a `Birth`'s `tick` is exactly the child's `born_tick`, and a `Death`'s
/// `age_ticks` is the organism's whole lifespan. `genome` is [`crate::genome::Genome::digest`]
/// of the organism the event is about (the child, for a birth). Founders are placed at world
/// creation, not born, and emit no event.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LifeEvent {
    Birth {
        tick: u64,
        id: OrganismId,
        parent: OrganismId,
        /// The parent's age at `tick`.
        parent_age_ticks: u64,
        /// The parent's total births including this one, so it is never zero.
        parent_births: u32,
        genome: u64,
        origin: Origin,
        /// The loci where the child differs from its parent (`design/fauna-v2.md`
        /// "Mutation"); empty for an exact copy. Recorded here and nowhere else.
        mutations: Vec<Mutation>,
    },
    Death {
        tick: u64,
        id: OrganismId,
        age_ticks: u64,
        cause: DeathCause,
        births: u32,
        genome: u64,
    },
}

impl LifeEvent {
    /// The tick the event was committed on.
    pub fn tick(&self) -> u64 {
        match self {
            LifeEvent::Birth { tick, .. } | LifeEvent::Death { tick, .. } => *tick,
        }
    }

    /// The organism the event is about: the newborn, or the deceased.
    pub fn id(&self) -> OrganismId {
        match self {
            LifeEvent::Birth { id, .. } | LifeEvent::Death { id, .. } => *id,
        }
    }
}
