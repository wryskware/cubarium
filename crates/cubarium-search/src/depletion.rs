//! The per-depleted-cell record: why a cell that lost three quarters of its opening foliage
//! stays there.
//!
//! Workstream F measured that raising `organism.move_cost` multiplies depletion crossings and
//! buys almost no recovery — 13 recovery crossings in 72 raised-price runs — and could not say
//! why, because it recorded neither the depleted cells' own habitat quality and trajectory nor
//! their post-depletion visits and bites (Astra's finding 2,
//! `design/7_Research/ecology-v1-next-steps-review-2026-09-16.md`). This module records
//! exactly those, per cell, so slow regrowth, returning consumers and intrinsically marginal
//! cells can be told apart.
//!
//! Every definition here is the one written into
//! `design/7_Research/ecology-v1-ladder-2026-09-16.md` before the campaign ran, and
//! `crates/cubarium-search/tests/ladder_measures.rs` checks each of them on hand-built
//! samples. Nothing in this module steps a world: it is arithmetic over a probe sequence, a
//! field series and a reconstructed habitat, which is why it is testable without simulating
//! anything.

use std::collections::BTreeMap;

use cubarium_core::ids::OrganismId;
use serde::{Deserialize, Serialize};

/// The most per-cell records one run opens, in the order cells first deplete. A storage guard,
/// not a sampling rule: F's matrix depleted at most 55.9 cells per run at these prices.
pub const MAX_RECORDS: usize = 512;

/// `P/P₀` is sampled at every tick divisible by this, strictly after a record's first
/// depletion. The same cadence the calibration samples its components at.
pub const TRAJECTORY_EVERY: u64 = 600;

/// The most trajectory samples one record keeps: the whole 180,000-tick horizon at
/// [`TRAJECTORY_EVERY`].
pub const MAX_TRAJECTORY: usize = 300;

/// A cell is under **pressure** if the attributed post-depletion take reaches this fraction of
/// its opening foliage. It is the depletion fraction itself, so "pressure" means the
/// post-depletion take is at least as large as the whole standing stock the threshold leaves.
pub const PRESSURE_FRACTION: f64 = crate::evaluate::DEPLETION_FRACTION;

/// The nutrient the contract's §13 arithmetic is quoted at. `(L·μ)_crit` is evaluated there
/// rather than at each cell's own `N`, which makes it permissive: a poorer cell has a higher
/// true threshold.
pub const MONOD_REFERENCE_N: f64 = 0.4;

/// Anything smaller than this is not an observation: the same epsilon the watched-cell rule
/// uses for opening foliage.
pub const EPSILON: f64 = 1e-9;

/// The Monod factor `N/(N + K_N)` at the contract's reference nutrient. `0.615` at the shipped
/// `K_N = 0.25`, which is the number §13 quotes.
pub fn monod_reference(half_saturation: f64) -> f64 {
    MONOD_REFERENCE_N / (MONOD_REFERENCE_N + half_saturation)
}

/// The constants `(L·μ)_crit` is built from, read once per run off the world's own config so
/// a searched configuration moves the threshold with it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlantConstants {
    /// `g`: `producer.growth`.
    pub growth: f64,
    /// `m_p`: `producer.mortality`, foliage senescence.
    pub mortality: f64,
    /// `m_w`: `plant.maintenance`, wood maintenance.
    pub maintenance: f64,
    /// `c_g`: `plant.build`, construction respiration.
    pub build: f64,
    /// `N/(N + K_N)` at [`MONOD_REFERENCE_N`].
    pub monod_ref: f64,
    /// The depletion threshold `P` is measured against, so the breakeven is evaluated at the
    /// point a depleted cell has to climb away from.
    pub deplete_fraction: f64,
}

/// `(L·μ)_crit` for one cell: the contract's §13 breakeven
/// `c·P − m_w·W = (1 + c_g)·m_p·P` with `c = g·L·μ·monod`, solved for `L·μ` at
/// `P = deplete_fraction · P₀`.
///
/// ```text
/// (L·μ)_crit = [ (1 + c_g)·m_p + m_w·W₀ / (deplete_fraction·P₀) ] / ( g · monod_ref )
/// ```
///
/// Permissive by construction, and the note says so in three places: the reference nutrient,
/// the full reserve (no `q_share` cut), and no fruit ripening. A cell this calls marginal is
/// marginal under a generous test. `P₀ ≤ 0` has no breakeven and returns infinity.
pub fn l_mu_crit(p0: f64, w0: f64, k: &PlantConstants) -> f64 {
    let _ = (p0, w0, k);
    unimplemented!("l_mu_crit")
}

/// Which of Astra's four readings a depleted cell falls under. Exhaustive and disjoint by
/// construction: [`DepletedCell::classify`] tests them in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepletedClass {
    /// It crossed back above the recovery threshold at least once. Recovery is possible here
    /// and the question is its rate: the *time-scale* reading.
    Recovered,
    /// It did not, and mouths took at least [`PRESSURE_FRACTION`] of its opening foliage out
    /// of it **after** it was already depleted: the *continued bites* reading.
    Pressure,
    /// It did not, the take was below that, and its habitat is adequate: the *plant equation
    /// or parameters* reading.
    PlantLimited,
    /// It did not, the take was below that, and its habitat is below `(L·μ)_crit`: the
    /// *intrinsically poor cell* reading.
    Marginal,
}

/// The four classes in report order.
pub const CLASSES: [DepletedClass; 4] = [
    DepletedClass::Recovered,
    DepletedClass::Pressure,
    DepletedClass::PlantLimited,
    DepletedClass::Marginal,
];

impl DepletedClass {
    pub fn index(self) -> usize {
        match self {
            DepletedClass::Recovered => 0,
            DepletedClass::Pressure => 1,
            DepletedClass::PlantLimited => 2,
            DepletedClass::Marginal => 3,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            DepletedClass::Recovered => "recovered",
            DepletedClass::Pressure => "pressure",
            DepletedClass::PlantLimited => "plant_limited",
            DepletedClass::Marginal => "marginal",
        }
    }
}

/// One cell that crossed the depletion threshold, from the probe it first crossed to the
/// horizon. Opened once and never re-opened; later crossings increment its counters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DepletedCell {
    pub cell: u16,
    /// Static habitat light and moisture, and their product — **not** this tick's light, which
    /// carries the weather perturbation and the algae floor on top.
    pub light: f64,
    pub moisture: f64,
    pub l_mu: f64,
    pub l_mu_crit: f64,
    /// `l_mu < l_mu_crit`.
    pub marginal: bool,
    /// The cell's own tick-0 foliage and wood.
    pub p0: f64,
    pub w0: f64,

    pub first_depletion_tick: u64,
    pub depletions: u32,
    pub recoveries: u32,
    pub first_recovery_tick: Option<u64>,
    pub first_redepletion_tick: Option<u64>,

    /// The probe tick of the last visit **before** the first depletion, and the foliage
    /// standing at it. `None` for a cell no prey body was ever observed in.
    pub last_visit_tick: Option<u64>,
    pub last_visit_stock: Option<f64>,
    /// `first_depletion_tick − last_visit_tick`; `0` when a body was standing in the cell as
    /// it crossed.
    pub ticks_since_visit_at_depletion: Option<u64>,

    /// Probes strictly after `first_depletion_tick` at which at least one prey body was
    /// observed here, and the sum over those probes of how many were.
    pub post_visit_probes: u64,
    pub post_visit_body_probes: u64,
    /// Attributed post-depletion served foliage (m), and the same over `p0`.
    pub post_served_foliage: f64,
    pub post_served_over_p0: f64,

    pub p_final_over_p0: f64,
    pub p_max_over_p0: f64,
    pub p_min_over_p0: f64,
    /// `P/P₀` at every tick divisible by [`TRAJECTORY_EVERY`] strictly after the first
    /// depletion, in order, capped at [`MAX_TRAJECTORY`].
    pub trajectory: Vec<f32>,

    pub class: Option<DepletedClass>,
}

impl DepletedCell {
    /// Whether mouths took at least [`PRESSURE_FRACTION`] of the opening foliage after the
    /// cell was already depleted.
    pub fn under_pressure(&self) -> bool {
        let _ = self;
        unimplemented!("under_pressure")
    }

    /// Astra's four readings, in priority order. See [`DepletedClass`].
    pub fn classify(&self) -> DepletedClass {
        let _ = self;
        unimplemented!("classify")
    }
}

/// The per-run summary of every record.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DepletionSummary {
    pub records: Vec<DepletedCell>,
    /// Cells that would have opened a record past [`MAX_RECORDS`]. Never silently zero.
    pub records_dropped: u64,
    pub cells_with_record: u32,
    /// Counts and fractions in [`CLASSES`] order.
    pub by_class: [u32; 4],
    pub fraction_by_class: [f64; 4],
    /// Over **all** records, so the priority order can be re-sliced without re-running.
    pub fraction_marginal: f64,
    pub fraction_any_post_bite: f64,
    pub fraction_no_post_visit: f64,
    pub mean_l_mu: f64,
    pub mean_l_mu_crit: f64,
    /// Over the recovered records only, in ticks.
    pub mean_recovery_latency_ticks: f64,
    /// The habitat reconstruction check: `fields::initial_wood(config, L₀, μ₀)` against the
    /// world's own tick-0 wood, over all cells.
    pub habitat_reconstruction_ok: bool,
    pub habitat_max_wood_error: f64,
}

/// Accumulates [`DepletedCell`] records while a run proceeds.
///
/// The recorder drives it in one fixed order per probe, which is what makes "the last visit
/// **before** depletion" well defined: positions first ([`Self::note_probe`]), then the
/// crossing detection ([`Self::depleted`] / [`Self::recovered`]), then the trajectory sample
/// ([`Self::sample`]).
#[derive(Clone, Debug, Default)]
pub struct DepletionTracker {
    p_ref: Vec<f64>,
    light: Vec<f64>,
    moisture: Vec<f64>,
    crit: Vec<f64>,
    /// Index into `records` for a cell that has one.
    slot: BTreeMap<u16, usize>,
    records: Vec<DepletedCell>,
    dropped: u64,
    last_visit_tick: BTreeMap<u16, u64>,
    last_visit_stock: BTreeMap<u16, f64>,
    habitat_ok: bool,
    habitat_max_wood_error: f64,
}

impl DepletionTracker {
    /// `p_ref` is each cell's tick-0 foliage, `light`/`moisture` the reconstructed static
    /// habitat, `w0` the world's own tick-0 wood.
    pub fn new(p_ref: &[f64], light: &[f64], moisture: &[f64], w0: &[f64], k: &PlantConstants) -> Self {
        let _ = (p_ref, light, moisture, w0, k);
        unimplemented!("DepletionTracker::new")
    }

    /// Record the habitat reconstruction check: the largest absolute difference between
    /// `fields::initial_wood` from the reconstructed habitat and the world's own tick-0 wood.
    pub fn set_habitat_check(&mut self, max_error: f64) {
        let _ = max_error;
        unimplemented!("set_habitat_check")
    }

    /// One probe's positions and attributed bites, **before** this probe's crossings are
    /// detected. `occupancy` maps a cell to how many prey bodies were observed in it;
    /// `served` maps a cell to the attributed served foliage since the previous probe.
    pub fn note_probe(
        &mut self,
        tick: u64,
        p: &[f64],
        occupancy: &BTreeMap<u16, u32>,
        served: &BTreeMap<u16, f64>,
    ) {
        let _ = (tick, p, occupancy, served);
        unimplemented!("note_probe")
    }

    /// A cell crossed to depleted at `tick`.
    pub fn depleted(&mut self, cell: usize, tick: u64) {
        let _ = (cell, tick);
        unimplemented!("depleted")
    }

    /// A cell crossed back to recovered at `tick`.
    pub fn recovered(&mut self, cell: usize, tick: u64) {
        let _ = (cell, tick);
        unimplemented!("recovered")
    }

    /// The trajectory and extremes sample, **after** this probe's crossings.
    pub fn sample(&mut self, tick: u64, p: &[f64]) {
        let _ = (tick, p);
        unimplemented!("sample")
    }

    /// Close the run with the horizon's foliage field.
    pub fn finish(self, p_final: &[f64]) -> DepletionSummary {
        let _ = p_final;
        unimplemented!("finish")
    }
}

/// Turns E's **per-body** `served[FOLIAGE]` into a **per-cell** attribution.
///
/// The ledger records what a body ate, not where. At each probe the difference against that
/// body's previous reading is credited to the cell it is observed in now; a body seen for the
/// first time is differenced against zero, and a body that died between probes is differenced
/// against its closed record and credited to the cell it was last observed in. One second of
/// resolution: the attribution is wrong for whatever a body ate in a cell it left inside the
/// interval. Called *attributed*, never *measured per cell*.
#[derive(Clone, Debug, Default)]
pub struct ServedAttributor {
    last: BTreeMap<OrganismId, f64>,
}

impl ServedAttributor {
    /// A live body observed in `cell` with lifetime served foliage `served_now`.
    pub fn observe(&mut self, id: OrganismId, cell: u16, served_now: f64, out: &mut BTreeMap<u16, f64>) {
        let _ = (id, cell, served_now, out);
        unimplemented!("ServedAttributor::observe")
    }

    /// A body whose record closed, with its final lifetime served foliage, credited to the
    /// cell it was last observed in.
    pub fn close(&mut self, id: OrganismId, last_cell: u16, served_final: f64, out: &mut BTreeMap<u16, f64>) {
        let _ = (id, last_cell, served_final, out);
        unimplemented!("ServedAttributor::close")
    }

    /// Forget a body without crediting anything, for an id whose cell is unknown.
    pub fn forget(&mut self, id: OrganismId) {
        let _ = id;
        unimplemented!("ServedAttributor::forget")
    }
}
