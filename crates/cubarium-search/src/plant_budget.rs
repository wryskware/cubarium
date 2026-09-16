//! Workstream M: the counter split by **exact** withdrawal, and the plant budget of the cells
//! that cross.
//!
//! Workstream I split its 1,355 depletion crossings by whether a prey body was ever *observed*
//! in the cell at a one-second probe, and read the 971 that were not as cells seeded above
//! what they can hold. Astra's round-2 review, P2: an animal can cross or feed between probes,
//! bites were attributed to a sampled position rather than recorded at the withdrawal site,
//! and the `(L·μ)_crit` the reading leans on is a static proxy evaluated at a reference
//! nutrient. None of the three is a measurement of the thing claimed.
//!
//! This module replaces all three with what the core now records
//! (`cubarium_core::fields::PlantBudgetRecord`): every metre of foliage a mouth withdrew,
//! booked in the cell the mouth stood in at the site the stock lost it, and every term of the
//! §4 plant budget booked where the plant step computed it. The primary split becomes
//!
//! > **crossed with exact withdrawal since the last recovery** — some mouth took foliage out
//! > of this cell between the last time it was above the recovery threshold and the moment it
//! > crossed below the depletion threshold — **or crossed without**.
//!
//! I's probe-based "ever visited" is kept beside it, unchanged, so the two can be compared on
//! the same crossings rather than across campaigns.
//!
//! Nothing here steps a world. It is arithmetic over a sequence of per-cell cumulative
//! samples and a sequence of crossings, which is why
//! `crates/cubarium-search/tests/plant_budget_measures.rs` can check every definition on
//! hand-built sequences.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Anything smaller than this is not a withdrawal: the same epsilon the watched-cell rule and
/// I's per-cell record use.
pub const EPSILON: f64 = 1e-9;

/// The cadence the per-cell cumulative snapshots are kept at, and the same cadence I samples
/// its `P/P₀` trajectory at. Must divide the probe interval's multiples: every snapshot is
/// taken at a probe tick.
pub const WINDOW_EVERY: u64 = 600;

/// Astra's window: "the measured plant budget over the 6,000 ticks before the crossing".
pub const BUDGET_WINDOW_TICKS: u64 = 6_000;

/// Snapshots kept per cell: the current boundary plus the ten before it.
pub const WINDOW_SLOTS: usize = (BUDGET_WINDOW_TICKS / WINDOW_EVERY) as usize + 1;

/// The most crossing rows one run retains. A storage guard, never silently zero: I's largest
/// run opened 66 per-cell records and counted at most a few hundred crossings.
pub const MAX_CROSSINGS: usize = 4_096;

/// One cell's cumulative plant budget as the core records it, reduced to the six numbers this
/// module needs. Cumulative from the tick the record was opened, never a rate and never a
/// difference: every quantity here is monotone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellSample {
    /// `foliage_from_income + foliage_from_reserve + foliage_from_propagule` (§4.4, §4.8).
    pub foliage_in: f64,
    /// `senescence + ripened + death_foliage` (§4.5, 3c, §4.7). **No consumer term.**
    pub foliage_out: f64,
    /// Exact §6.4 consumer withdrawal from this cell's `P`.
    pub withdrawal: f64,
    /// `A` (§4.1): the gross material the cell drew out of `N`. The income the whole stand
    /// lives on, so `income` against the maintenance it could not pay says *why* a budget is
    /// negative rather than only that it is.
    pub income: f64,
    /// §4.3 maintenance nothing paid, and §4.7 foliage a dying stand dropped. A crossing in a
    /// cell with `death_foliage > 0` is a stand that died, not a stand that thinned.
    pub maintenance_unpaid: f64,
    pub death_foliage: f64,
    /// `Σ W⁻` over the ticks the cell was alive: maintenance is `m_w · W`, so the wood a cell
    /// carries is what its income has to fund.
    pub wood_sum: f64,
    /// `Σ` effective light and `Σ N` over the ticks the cell was alive, and that count, so a
    /// window mean is a difference of two samples divided by a difference of two counts.
    pub light_sum: f64,
    pub nutrient_sum: f64,
    pub ticks_alive: u64,
}

impl CellSample {
    /// The plant's own net over the interval between two samples: what the plant put into its
    /// foliage minus what the plant took back out, with no consumer term. This is the quantity
    /// the verdict turns on — negative means the cell cannot hold what it is carrying.
    pub fn budget_since(&self, earlier: &CellSample) -> f64 {
        (self.foliage_in - earlier.foliage_in) - (self.foliage_out - earlier.foliage_out)
    }

    /// Mean over the ticks the cell was alive between two samples, or `None` when it was alive
    /// for none of them — a dead or bare cell has no light the plant step read.
    pub fn mean_since(&self, earlier: &CellSample, sum: fn(&CellSample) -> f64) -> Option<f64> {
        let ticks = self.ticks_alive.saturating_sub(earlier.ticks_alive);
        (ticks > 0).then(|| (sum(self) - sum(earlier)) / ticks as f64)
    }
}

/// Which side of the split a crossing falls on. Exhaustive and disjoint: the exact withdrawal
/// since the last recovery is either above [`EPSILON`] or it is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossingClass {
    /// Some mouth withdrew foliage from this cell since it was last above the recovery
    /// threshold. Grazing is part of this crossing's history, whatever else is.
    WithWithdrawal,
    /// No mouth withdrew anything at all. Whatever took the foliage, it was not a mouth.
    WithoutWithdrawal,
}

impl CrossingClass {
    pub fn name(self) -> &'static str {
        match self {
            CrossingClass::WithWithdrawal => "with_withdrawal",
            CrossingClass::WithoutWithdrawal => "without_withdrawal",
        }
    }
}

/// One depletion crossing, with the exact withdrawal that preceded it and the plant budget of
/// the window before it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CrossingRow {
    pub cell: u16,
    /// The probe tick the crossing was detected at — the same tick I's counter reports.
    pub tick: u64,
    /// Which crossing of this cell this is, counting from zero.
    pub index: u32,
    /// The primary split.
    pub class: Option<CrossingClass>,
    /// Exact withdrawal from this cell since its last recovery, or since the record opened.
    pub withdrawal_since_recovery: f64,
    /// Exact withdrawal from this cell over the whole run so far.
    pub withdrawal_cumulative: f64,
    /// I's probe-based split, kept for comparison: was a prey body ever observed in this cell
    /// at a probe before this crossing?
    pub ever_visited: bool,

    /// The measured plant budget over the window before the crossing, and the window's actual
    /// length — shorter than [`BUDGET_WINDOW_TICKS`] only near the start of a run.
    pub budget_window_ticks: u64,
    pub budget_in: f64,
    pub budget_out: f64,
    /// `budget_in − budget_out`: the plant's own net over the window.
    pub budget_net: f64,
    /// Exact consumer withdrawal over the same window, reported beside the budget rather than
    /// inside it, so "the plant could not hold it" and "a mouth took it" stay separable.
    pub withdrawal_window: f64,
    /// The ticks the cell was alive inside the window, and the means the plant step actually
    /// used over them. `None` when the cell was alive for none of the window.
    pub window_ticks_alive: u64,
    pub light_effective: Option<f64>,
    pub nutrient: Option<f64>,
    pub wood: Option<f64>,
    /// Gross income, unpaid maintenance and death-dropped foliage over the same window: why
    /// the budget is what it is.
    pub income: f64,
    pub maintenance_unpaid: f64,
    pub death_foliage: f64,

    /// The cell's opening foliage and the foliage standing at the crossing.
    pub p_open: f64,
    pub p_at_crossing: f64,
}

impl CrossingRow {
    /// Astra's confirmation test for one crossing: no mouth took anything and the plant's own
    /// budget over the window before it was negative.
    pub fn is_plant_budget_failure(&self) -> bool {
        self.class == Some(CrossingClass::WithoutWithdrawal) && self.budget_net < 0.0
    }
}

/// One cell's whole-run aggregate: the coarse all-cell summary kept beside the crossing rows,
/// so a cell that crossed in one arm can be looked up in the other arm even when it never
/// crossed there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellTotals {
    pub cell: u16,
    pub p_open: f64,
    pub p_final: f64,
    /// Whole-run plant budget and exact withdrawal.
    pub budget_net: f64,
    pub withdrawal: f64,
    pub ticks_alive: u64,
    pub light_effective: Option<f64>,
    pub nutrient: Option<f64>,
    pub wood: Option<f64>,
    pub income: f64,
    pub maintenance_unpaid: f64,
    pub death_foliage: f64,
    /// The static habitat product I classified on, carried for comparison only.
    pub l_mu: f64,
    pub crossings: u32,
    pub recoveries: u32,
}

/// Everything one run's split and budget amount to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlantBudgetSummary {
    pub crossings: Vec<CrossingRow>,
    /// Crossings past [`MAX_CROSSINGS`], never silently zero.
    pub crossings_dropped: u64,
    pub total_crossings: u64,
    pub with_withdrawal: u64,
    pub without_withdrawal: u64,
    pub fraction_with_withdrawal: f64,
    /// I's split on the same crossings, for comparison.
    pub ever_visited: u64,
    pub never_visited: u64,
    /// Astra's confirmation count: crossings with no withdrawal whose window budget is
    /// negative, and the same among crossings that did have withdrawal.
    pub without_withdrawal_negative_budget: u64,
    pub with_withdrawal_negative_budget: u64,
    /// Distinct cells that crossed at least once, ascending. The overlap between arms is
    /// computed from these.
    pub crossing_cells: Vec<u16>,
    /// The coarse all-cell summary: whole-run aggregates, no time resolution.
    pub cells: Vec<CellTotals>,
    /// The core record's own identity residual at the horizon, carried into the row so a run
    /// whose accounting did not close cannot be read as a measurement.
    pub max_identity_residual: f64,
    /// Ticks the record covered, and the tick it was opened at.
    pub ticks: u64,
    pub opened_at: u64,
}

impl PlantBudgetSummary {
    /// The cells that crossed in both summaries, ascending.
    pub fn overlap(&self, other: &PlantBudgetSummary) -> Vec<u16> {
        let mine: std::collections::BTreeSet<u16> = self.crossing_cells.iter().copied().collect();
        other.crossing_cells.iter().copied().filter(|c| mine.contains(c)).collect()
    }
}

/// Accumulates [`CrossingRow`]s while a run proceeds.
///
/// Driven in one fixed order per probe, which is what makes "since the last recovery" well
/// defined: the cumulative samples are taken first ([`Self::observe`]), then this probe's
/// crossings ([`Self::depleted`] / [`Self::recovered`]). Both crossing calls read the samples
/// taken at the same tick, so the withdrawal interval's endpoints are the detection ticks
/// themselves and nothing is attributed across them.
#[derive(Clone, Debug, Default)]
pub struct PlantBudgetTracker {
    /// The cell's opening foliage and static `L·μ`, for the rows.
    p_ref: Vec<f64>,
    l_mu: Vec<f64>,
    /// This probe's cumulative sample per cell.
    now: Vec<CellSample>,
    now_tick: u64,
    /// The last [`WINDOW_SLOTS`] boundary snapshots, oldest first, and the tick of each.
    ring: Vec<Vec<CellSample>>,
    ring_ticks: Vec<u64>,
    /// The cumulative withdrawal each cell stood at when it last recovered, or `0.0`.
    since_recovery: Vec<f64>,
    /// Cells a prey body has been observed in, at any probe.
    visited: Vec<bool>,
    crossings: Vec<CrossingRow>,
    dropped: u64,
    counts: BTreeMap<u16, u32>,
    recoveries: BTreeMap<u16, u32>,
    total: u64,
    /// The tick the core record was opened at: the start of the first, shortened window.
    opened_at: u64,
}

impl PlantBudgetTracker {
    /// `p_ref` is each cell's foliage when the record opened; `l_mu` the static habitat
    /// product I classified on, carried through for comparison only.
    pub fn new(p_ref: &[f64], l_mu: &[f64], opened_at: u64) -> PlantBudgetTracker {
        let n = p_ref.len();
        PlantBudgetTracker {
            p_ref: p_ref.to_vec(),
            l_mu: l_mu.to_vec(),
            now: vec![CellSample::default(); n],
            now_tick: 0,
            ring: Vec::new(),
            ring_ticks: Vec::new(),
            since_recovery: vec![0.0; n],
            visited: vec![false; n],
            crossings: Vec::new(),
            dropped: 0,
            counts: BTreeMap::new(),
            recoveries: BTreeMap::new(),
            total: 0,
            opened_at,
        }
    }

    /// One probe's cumulative per-cell samples, **before** this probe's crossings. A sample on
    /// a [`WINDOW_EVERY`] boundary also enters the ring.
    pub fn observe(&mut self, tick: u64, samples: &[CellSample], occupied: &[u16]) {
        self.now.clear();
        self.now.extend_from_slice(samples);
        self.now_tick = tick;
        for cell in occupied {
            if let Some(v) = self.visited.get_mut(usize::from(*cell)) {
                *v = true;
            }
        }
        if tick % WINDOW_EVERY == 0 {
            self.ring.push(samples.to_vec());
            self.ring_ticks.push(tick);
            if self.ring.len() > WINDOW_SLOTS {
                self.ring.remove(0);
                self.ring_ticks.remove(0);
            }
        }
    }

    /// The ring slot the window ending at `tick` starts from: the newest boundary at or before
    /// `tick − BUDGET_WINDOW_TICKS`, or the oldest snapshot held when the run is younger than
    /// the window. `None` before the first boundary, where the window starts at the tick the
    /// record was opened and every cumulative sample there is zero by construction.
    fn window_start(&self, tick: u64) -> Option<(usize, u64)> {
        let floor = tick.saturating_sub(BUDGET_WINDOW_TICKS);
        let mut chosen = 0usize;
        for (i, t) in self.ring_ticks.iter().enumerate() {
            if *t <= floor {
                chosen = i;
            }
        }
        self.ring_ticks.get(chosen).map(|t| (chosen, *t))
    }

    /// A cell crossed to depleted at `tick`, with `p` standing in it.
    pub fn depleted(&mut self, cell: usize, tick: u64, p: f64) {
        self.total += 1;
        let key = cell as u16;
        let index = self.counts.entry(key).or_insert(0);
        let row_index = *index;
        *index += 1;
        if self.crossings.len() >= MAX_CROSSINGS {
            self.dropped += 1;
            return;
        }
        let now = self.now.get(cell).copied().unwrap_or_default();
        let since = now.withdrawal - self.since_recovery.get(cell).copied().unwrap_or(0.0);
        let class = if since > EPSILON {
            CrossingClass::WithWithdrawal
        } else {
            CrossingClass::WithoutWithdrawal
        };
        let (start, start_tick) = match self.window_start(tick) {
            Some((i, t)) => (self.ring[i].get(cell).copied().unwrap_or_default(), t),
            // Before the first boundary the window starts where the record was opened, and
            // every cumulative term there is zero.
            None => (CellSample::default(), self.opened_at),
        };
        let row = CrossingRow {
            cell: key,
            tick,
            index: row_index,
            class: Some(class),
            withdrawal_since_recovery: since,
            withdrawal_cumulative: now.withdrawal,
            ever_visited: self.visited.get(cell).copied().unwrap_or(false),
            budget_window_ticks: tick.saturating_sub(start_tick),
            budget_in: now.foliage_in - start.foliage_in,
            budget_out: now.foliage_out - start.foliage_out,
            budget_net: now.budget_since(&start),
            withdrawal_window: now.withdrawal - start.withdrawal,
            window_ticks_alive: now.ticks_alive.saturating_sub(start.ticks_alive),
            light_effective: now.mean_since(&start, |s| s.light_sum),
            nutrient: now.mean_since(&start, |s| s.nutrient_sum),
            wood: now.mean_since(&start, |s| s.wood_sum),
            income: now.income - start.income,
            maintenance_unpaid: now.maintenance_unpaid - start.maintenance_unpaid,
            death_foliage: now.death_foliage - start.death_foliage,
            p_open: self.p_ref.get(cell).copied().unwrap_or(0.0),
            p_at_crossing: p,
        };
        self.crossings.push(row);
    }

    /// A cell crossed back to recovered at `tick`: the withdrawal clock restarts here.
    pub fn recovered(&mut self, cell: usize, _tick: u64) {
        let key = cell as u16;
        *self.recoveries.entry(key).or_insert(0) += 1;
        let now = self.now.get(cell).map(|s| s.withdrawal).unwrap_or(0.0);
        if let Some(slot) = self.since_recovery.get_mut(cell) {
            *slot = now;
        }
    }

    /// Close the run with the horizon's per-cell samples, the horizon's foliage field and the
    /// core record's own identity residual.
    pub fn finish(
        mut self,
        samples: &[CellSample],
        p_final: &[f64],
        max_identity_residual: f64,
        ticks: u64,
    ) -> PlantBudgetSummary {
        let (mut with, mut without) = (0u64, 0u64);
        let (mut visited, mut unvisited) = (0u64, 0u64);
        let (mut with_neg, mut without_neg) = (0u64, 0u64);
        for r in &self.crossings {
            match r.class {
                Some(CrossingClass::WithWithdrawal) => {
                    with += 1;
                    if r.budget_net < 0.0 {
                        with_neg += 1;
                    }
                }
                _ => {
                    without += 1;
                    if r.budget_net < 0.0 {
                        without_neg += 1;
                    }
                }
            }
            if r.ever_visited {
                visited += 1;
            } else {
                unvisited += 1;
            }
        }
        let mut crossing_cells: Vec<u16> = self.counts.keys().copied().collect();
        crossing_cells.sort_unstable();
        let cells = (0..self.p_ref.len())
            .map(|i| {
                let s = samples.get(i).copied().unwrap_or_default();
                let zero = CellSample::default();
                CellTotals {
                    cell: i as u16,
                    p_open: self.p_ref[i],
                    p_final: p_final.get(i).copied().unwrap_or(0.0),
                    budget_net: s.budget_since(&zero),
                    withdrawal: s.withdrawal,
                    ticks_alive: s.ticks_alive,
                    light_effective: s.mean_since(&zero, |s| s.light_sum),
                    nutrient: s.mean_since(&zero, |s| s.nutrient_sum),
                    wood: s.mean_since(&zero, |s| s.wood_sum),
                    income: s.income,
                    maintenance_unpaid: s.maintenance_unpaid,
                    death_foliage: s.death_foliage,
                    l_mu: self.l_mu.get(i).copied().unwrap_or(0.0),
                    crossings: self.counts.get(&(i as u16)).copied().unwrap_or(0),
                    recoveries: self.recoveries.get(&(i as u16)).copied().unwrap_or(0),
                }
            })
            .collect();
        let denominator = (with + without).max(1) as f64;
        self.crossings.shrink_to_fit();
        PlantBudgetSummary {
            crossings: self.crossings,
            crossings_dropped: self.dropped,
            total_crossings: self.total,
            with_withdrawal: with,
            without_withdrawal: without,
            fraction_with_withdrawal: with as f64 / denominator,
            ever_visited: visited,
            never_visited: unvisited,
            without_withdrawal_negative_budget: without_neg,
            with_withdrawal_negative_budget: with_neg,
            crossing_cells,
            cells,
            max_identity_residual,
            ticks,
            opened_at: self.opened_at,
        }
    }
}
