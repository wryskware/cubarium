//! Observation → decision. Pure: no world mutation, no draws except the OU noise the
//! caller supplies.

use cubarium_surface::Vec2;

use crate::organism::{Mode, Organism};

/// What an organism senses this tick, all in its own chart.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Observation {
    /// `P` and `D` in the organism's own cell.
    pub p_here: f64,
    pub d_here: f64,
    /// Normalized gradients `Σ (value_i − value_0) · dir_i` over the graph neighbors of the
    /// own cell, with `dir_i` the unit vector toward neighbor i's center via `unfold`.
    pub grad_p: Vec2,
    pub grad_d: Vec2,
    /// Crowding repulsion: `Σ (own − other)/|own − other|² · (extent_sum)` over neighbors
    /// closer than the sum of body extents plus 1 px (positions from the pair pass).
    pub repulsion: Vec2,
    /// Standard-normal 2-vector for this tick's OU update (from `Stream::OrganismTurn`).
    pub noise: Vec2,
}

/// What the organism requests this tick; physiology and settlement apply limits.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub mode: Mode,
    /// New unit heading after the bounded turn.
    pub heading: Vec2,
    /// Updated OU vector.
    pub ou: Vec2,
    /// Movement effort in `[0, 1]`.
    pub effort: f64,
    /// Grazing and scavenging intake efforts in `[0, 1]` (zero unless Feeding).
    pub graze_effort: f64,
    pub scavenge_effort: f64,
    /// Request to begin gestation (capacity and escrow checks happen in the world).
    pub bud: bool,
    pub hunger_memory: f64,
}

/// Normative rules (`design/m2-world-spec.md` "Controller"):
/// - `h = hunger()`, `m_h += (1 − exp(−dt/τ)) (h − m_h)`.
/// - mode: Resting→Seeking when `m_h > seek_on`; Seeking/Feeding→Resting when `m_h < seek_off`;
///   Seeking→Feeding when `p_here ≥ feed_min` (grazing on) or `d_here ≥ feed_min` (scavenging on);
///   Feeding→Seeking when neither holds.
/// - OU: `ou' = ou · (1 − dt/τ_ou) + noise · turn_noise · sqrt(dt)`, with `τ_ou = 2 s`.
/// - steering `s = w_food · h · grad_p + w_detritus · h · grad_d + w_crowd · repulsion + w_persist · ou'`;
///   if `|s| < 1e-9` keep the heading; else rotate the heading toward `s` by at most
///   `turn_rate_max · dt` (radians) and renormalize.
/// - effort: Seeking 1, Feeding `feed_effort`, Resting `rest_effort`.
/// - intake efforts: Feeding with `p_here ≥ feed_min` → `graze_effort = 1`; Feeding with
///   `d_here ≥ feed_min` → `scavenge_effort = 1` (both may be 1).
/// - `bud` when `reserve ≥ bud_reserve · R_max`, `energy ≥ bud_energy · E_max`,
///   `age ≥ bud_min_age`, no escrow.
pub fn decide(org: &Organism, obs: &Observation, now: u64, dt: f64, grazing: bool, scavenging: bool) -> Decision {
    let _ = (org, obs, now, dt, grazing, scavenging);
    todo!("decide")
}
