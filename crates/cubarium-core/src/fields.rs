//! Material fields and their reactions.

use serde::{Deserialize, Serialize};

use cubarium_surface::{CELL_COUNT, FieldGraph, ScalarField};

use crate::config::WorldConfig;

/// The four checkpointed per-cell quantities.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fields {
    pub n: Vec<f64>,
    pub p: Vec<f64>,
    pub d: Vec<f64>,
    pub de: Vec<f64>,
}

/// Energy ledger for one tick's field reactions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FieldLedger {
    /// Energy that entered through producer growth.
    pub light_in: f64,
    /// Energy dissipated by mortality clamping and decomposition.
    pub heat_out: f64,
}

impl Fields {
    /// Initial fields per the spec: `N = initial`, `P = initial_fraction · P_max · L₀ · W₀`,
    /// `D = De = 0`.
    pub fn new(cfg: &WorldConfig, light0: &[f64; CELL_COUNT], moisture0: &[f64; CELL_COUNT]) -> Fields {
        let _ = (cfg, light0, moisture0);
        todo!("Fields::new")
    }

    pub fn total_material(&self) -> f64 {
        self.n.iter().sum::<f64>() + self.p.iter().sum::<f64>() + self.d.iter().sum::<f64>()
    }

    /// One tick of reactions, all from pre-tick values, per cell:
    /// 1. growth `Δ = min(g · L · W · P · (1 − P/P_max) · dt, f_max · N · dt, N)` (Δ ≥ 0):
    ///    `N −= Δ`, `P += Δ`, `light_in += e_p · Δ`.
    /// 2. mortality `Δ = m_p · P · dt`: `P −= Δ`, `D += Δ`, `De += e_p · Δ`, then if
    ///    `De > e_d_max · D` the excess goes to `heat_out` and `De` is clamped.
    /// 3. decomposition `Δ = k_d · D · dt`: `D −= Δ`, `N += Δ`, `De` reduced by the same
    ///    fraction with the removed energy in `heat_out`.
    ///
    /// Then `N` diffuses with `cubarium_surface::diffuse` at `diffusion · dt` using
    /// `scratch`. Never produces negatives; non-finite input is a bug (debug_assert).
    pub fn react(&mut self, cfg: &WorldConfig, light: &[f64; CELL_COUNT], moisture: &[f64; CELL_COUNT], graph: &FieldGraph, scratch: &mut (ScalarField, ScalarField)) -> FieldLedger {
        let _ = (cfg, light, moisture, graph, scratch);
        todo!("Fields::react")
    }

    /// Debug/telemetry check: finite and nonnegative everywhere, `De ≤ e_d_max · D + 1e-9`.
    pub fn check(&self, energy_cap: f64) -> Result<(), String> {
        let _ = energy_cap;
        todo!("Fields::check")
    }
}
