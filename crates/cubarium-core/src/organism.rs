//! Organism state.

use serde::{Deserialize, Serialize};

use cubarium_surface::{SurfacePoint, Vec2};

use crate::config::OrganismConfig;
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
    /// Consumed by a hunter (`crate::hunter`). **Appended last**: the three natural causes
    /// above keep their order and their index in the persisted `deaths_total` triple, and
    /// predation is counted separately in `HunterState::predation_deaths_total`.
    Predation,
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

    /// The most energy this body could have available for **one** tick: what it holds now,
    /// plus everything one tick of oxidation could convert out of the reserve.
    ///
    /// This is the quantity the starvation predicate compares against
    /// [`crate::motor::MotorBill::upkeep`]. It uses the same rate, density, efficiency and
    /// headroom the physiology pass uses (`crate::world::step`, oxidation), so it can never
    /// promise energy the body would not actually receive. It deliberately does **not** apply
    /// the oxidation *threshold*: the threshold decides when a healthy body tops itself up,
    /// not how much a starving one can raise, and gating on it would kill bodies that the
    /// physiology would have refuelled on the next tick.
    pub fn raisable_energy(&self, cfg: &OrganismConfig, dt: f64) -> f64 {
        let energy = if self.energy.is_finite() { self.energy.max(0.0) } else { 0.0 };
        let reserve = if self.reserve.is_finite() { self.reserve.max(0.0) } else { 0.0 };
        let burned = (cfg.oxidation_rate * dt).max(0.0).min(reserve);
        let released = cfg.reserve_energy_density * burned;
        let room = (self.phenotype.energy_max - energy).max(0.0);
        energy + (released * cfg.oxidation_efficiency).min(room)
    }
}
