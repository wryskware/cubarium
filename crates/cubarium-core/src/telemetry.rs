//! Bounded per-sample telemetry; observer-side only.

use serde::{Deserialize, Serialize};

/// One telemetry sample (emitted every `telemetry_seconds`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Telemetry {
    pub tick: u64,
    pub population: u32,
    pub births: u32,
    pub deaths_starvation: u32,
    pub deaths_age: u32,
    pub deaths_collapse: u32,
    pub escrows: u32,
    pub cap_rejections: u32,
    pub nutrient: f64,
    pub producer: f64,
    pub detritus: f64,
    pub detritus_energy: f64,
    pub organism_material: f64,
    pub organism_energy: f64,
    pub light_in: f64,
    pub heat_out: f64,
    pub mass_residual: f64,
    pub population_by_face: [u32; 5],
    /// `Σ P` and `Σ D` over each face's 256 cells, in `Face` order.
    pub producer_by_face: [f64; 5],
    pub detritus_by_face: [f64; 5],
    pub occupied_cells: u32,
    pub travel_fallbacks: u32,
    pub travel_ties: u32,
    pub pairs_considered: u64,
    pub pairs_unfolded: u64,
    pub neighbor_truncations: u64,
    pub mode_resting: u32,
    pub mode_seeking: u32,
    pub mode_feeding: u32,
    /// FNV-1a over the postcard encoding of the state, for same-build replay checks.
    pub state_hash: u64,
}
