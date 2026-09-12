//! Shared helpers for the independent verification suite.
//!
//! Everything here is derived from `design/m2-world-spec.md` and the public doc comments
//! of `cubarium-core`; nothing reads the implementation bodies. The accounting quantities
//! below are transcriptions of the spec's own formulas, recomputed from `World::state` so
//! that a test never trusts the world's own bookkeeping.

#![allow(dead_code)]

use cubarium_core::{World, WorldConfig};

/// The spec's closed-box material:
/// `M = Σ_cells (N + P + D) + Σ_organisms (S + R) + Σ_escrow (S_child + R_child)`.
///
/// `Organism::material()` is documented to include the escrow, so this is the whole `M`.
pub fn total_material(world: &World) -> f64 {
    let fields = &world.state.fields;
    let cells: f64 = fields.n.iter().sum::<f64>()
        + fields.p.iter().sum::<f64>()
        + fields.d.iter().sum::<f64>();
    let organisms: f64 = world.state.organisms.iter().map(|(_, o)| o.material()).sum();
    cells + organisms
}

/// The spec's stored energy ("Units and quantities"):
/// `E_total = Σ_cells (e_p · P + De) + Σ_organisms (E + e_r · R)
///          + Σ_escrow (e_r · (S_child + R_child) + E_child)`.
pub fn stored_energy(world: &World) -> f64 {
    let cfg = &world.state.config;
    let e_p = cfg.producer.energy_density;
    let e_r = cfg.organism.reserve_energy_density;
    let fields = &world.state.fields;

    let mut total: f64 =
        fields.p.iter().map(|p| e_p * p).sum::<f64>() + fields.de.iter().sum::<f64>();
    for (_, o) in world.state.organisms.iter() {
        total += o.energy + e_r * o.reserve;
        if let Some(e) = &o.escrow {
            total += e_r * (e.structure + e.reserve) + e.energy;
        }
    }
    total
}

/// A world with no light at all: `L₀ = clamp(0 + 0·h + 0·n, 0, 1) = 0` everywhere and no
/// weather amplitude, so `light_in` must be identically zero and no energy can enter.
pub fn no_light_config() -> WorldConfig {
    let mut cfg = WorldConfig::default();
    cfg.habitat.light_base = 0.0;
    cfg.habitat.light_height_gain = 0.0;
    cfg.habitat.light_noise_gain = 0.0;
    cfg.weather.amplitude = 0.0;
    cfg
}

/// A world that starves: producers never grow and the initial standing crop is 2% of
/// `P_max · L₀ · W₀`, so the founders burn their reserves down and die.
pub fn harsh_config() -> WorldConfig {
    let mut cfg = WorldConfig::default();
    cfg.producer.growth = 0.0;
    cfg.producer.initial_fraction = 0.02;
    cfg
}

/// Every material stock the spec names, per the "Units and quantities" table, flattened so
/// a test can assert nonnegativity over all of them at once.
pub fn stocks(world: &World) -> impl Iterator<Item = (&'static str, f64)> + '_ {
    let fields = &world.state.fields;
    let field_stocks = fields
        .n
        .iter()
        .map(|v| ("N", *v))
        .chain(fields.p.iter().map(|v| ("P", *v)))
        .chain(fields.d.iter().map(|v| ("D", *v)))
        .chain(fields.de.iter().map(|v| ("De", *v)));
    let organism_stocks = world.state.organisms.iter().flat_map(|(_, o)| {
        let escrow = o.escrow.as_ref();
        [
            ("S", o.structure),
            ("R", o.reserve),
            ("E", o.energy),
            ("S_child", escrow.map_or(0.0, |e| e.structure)),
            ("R_child", escrow.map_or(0.0, |e| e.reserve)),
            ("E_child", escrow.map_or(0.0, |e| e.energy)),
        ]
    });
    field_stocks.chain(organism_stocks)
}
