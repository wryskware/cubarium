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
    /// `Σ F` over all cells (`design/fauna-v2.md` "Fruit").
    pub fruit: f64,
    pub organism_material: f64,
    pub organism_energy: f64,
    pub light_in: f64,
    pub heat_out: f64,
    pub mass_residual: f64,
    pub population_by_face: [u32; 5],
    /// `Σ P` and `Σ D` over each face's 256 cells, in `Face` order.
    pub producer_by_face: [f64; 5],
    pub detritus_by_face: [f64; 5],
    /// `Σ w` over all cells and per face (`design/water.md`), and this sample's rain and
    /// evaporation totals; `Δ water == rain_in − evap_out` between samples to rounding.
    pub water: f64,
    pub water_by_face: [f64; 5],
    pub rain_in: f64,
    pub evap_out: f64,
    pub occupied_cells: u32,
    pub travel_fallbacks: u32,
    pub travel_ties: u32,
    pub pairs_considered: u64,
    pub pairs_unfolded: u64,
    pub neighbor_truncations: u64,
    pub mode_resting: u32,
    pub mode_seeking: u32,
    pub mode_feeding: u32,
    /// Live organisms per heritable `form` (rig index), `design/fauna-v2.md`.
    pub population_by_form: [u32; 8],
    /// Mean embedded height (Top = 1, rim = −1) of the live organisms of each form; zero
    /// for a form with none.
    pub mean_height_by_form: [f64; 8],
    /// FNV-1a over the postcard encoding of the state, for same-build replay checks.
    pub state_hash: u64,
    /// FNV-1a over the schema 7 projection (everything but `care`), so a care run and a
    /// matched no-care run are directly comparable and a migrated schema 7 world with zero
    /// care hashes as the pre-care build did. Every field below defaults, so a reader of
    /// older telemetry is unaffected.
    #[serde(default)]
    pub ecology_hash: u64,
    /// The highest care sequence number the world has consumed
    /// (`design/7_Research/care-contract-2026-09-12.md`).
    #[serde(default)]
    pub care_admitted_seq: u64,
    /// The six cumulative care ledgers, in the mass, energy and water identities.
    #[serde(default)]
    pub care_feed_material_in: f64,
    #[serde(default)]
    pub care_feed_energy_in: f64,
    #[serde(default)]
    pub care_rain_depth_in: f64,
    #[serde(default)]
    pub care_clean_material_out: f64,
    #[serde(default)]
    pub care_clean_energy_out: f64,
    #[serde(default)]
    pub care_allowance_used: f64,
}
