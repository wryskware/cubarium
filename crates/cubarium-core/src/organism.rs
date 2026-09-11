//! Organism state.

use serde::{Deserialize, Serialize};

use cubarium_surface::{SurfacePoint, Vec2};

use crate::genome::{Genome, Phenotype};
use crate::ids::OrganismId;
use crate::rng::Counter;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Resting,
    Seeking,
    Feeding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    Founder,
    Descendant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathCause {
    Starvation,
    Age,
    Collapse,
}

/// Escrowed offspring material and energy held by a gestating parent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Escrow {
    pub structure: f64,
    pub reserve: f64,
    pub energy: f64,
    pub started_tick: u64,
    pub genome: Genome,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Organism {
    pub pos: SurfacePoint,
    /// Unit heading in the chart of `pos`.
    pub heading: Vec2,
    /// Ornstein–Uhlenbeck turn-noise vector in the chart of `pos`; transported with the heading.
    pub ou: Vec2,
    pub structure: f64,
    pub reserve: f64,
    pub energy: f64,
    pub born_tick: u64,
    pub hunger_memory: f64,
    pub mode: Mode,
    pub escrow: Option<Escrow>,
    pub births: u32,
    pub genome: Genome,
    pub phenotype: Phenotype,
    pub parent: Option<OrganismId>,
    pub origin: Origin,
    pub turn_counter: Counter,
    /// Presentation-only, derived each tick: true while intake was nonzero this tick.
    pub fed_this_tick: bool,
}

impl Organism {
    pub fn material(&self) -> f64 {
        self.structure + self.reserve + self.escrow.as_ref().map_or(0.0, |e| e.structure + e.reserve)
    }

    pub fn age_ticks(&self, now: u64) -> u64 {
        now.saturating_sub(self.born_tick)
    }

    pub fn hunger(&self) -> f64 {
        (1.0 - self.reserve / self.phenotype.reserve_max).clamp(0.0, 1.0)
    }
}
