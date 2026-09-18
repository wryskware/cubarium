//! **Measurement only, and only with the `profile` feature.** Per-phase wall time and the
//! counts that explain it, for one process, in one place.
//!
//! Nothing here is part of the model: no rule reads it, no number depends on it, and with
//! the feature off the module does not exist and every timer call site is `#[cfg]`-ed away.
//! It lives in this crate rather than in three because the tick spans three layers and one
//! table of it is worth more than three — the [`Phase`] list therefore names the water
//! phases, the plant phases and the animal phases together, and each crate's own `profile`
//! feature turns its own timers on.
//!
//! **What the numbers are.** A [`Timer`] is one `Instant::now()` at the start of a phase
//! and one at its end, accumulated into a global `u64` of nanoseconds; a nested phase's
//! time is therefore also inside its parent's (the substep loop contains infiltrate, fall
//! and equalize). The timer itself costs about 40 ns per phase per tick on this machine,
//! which is 1e-3 of a 15 ms tick and 2e-2 of the 50 µs target — worth knowing when the
//! target is reached, negligible now, and the same for every phase.
//!
//! **Single-threaded by construction.** The accumulators are atomics so the API needs no
//! `&mut`, not because the phases are parallel. A threaded tick would need per-thread
//! accumulators before these numbers meant anything again.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// One timed phase of the coupled tick, in the order the tick runs them.
///
/// `Substeps` is the whole `water_substeps` loop and contains `Infiltrate`, `Fall` and
/// `Equalize`; `WorldStep`, `FloraStep` and `FaunaStep` are the three layers' whole steps
/// and contain everything under them. Everything else is a leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    WorldStep,
    Rain,
    Evaporate,
    Substeps,
    Infiltrate,
    Fall,
    Exchange,
    Drain,
    WaterTable,
    Spring,
    Outlet,
    FloraStep,
    Prune,
    SkyCache,
    Drown,
    Light,
    Drink,
    Feed,
    Grow,
    Decompose,
    SeedBank,
    Propagate,
    FaunaStep,
    FaunaTerrain,
    FaunaMaintenance,
    FaunaSense,
    FaunaAct,
    FaunaBirths,
    FaunaDeaths,
}

impl Phase {
    pub const ALL: [Phase; 29] = [
        Phase::WorldStep,
        Phase::Rain,
        Phase::Evaporate,
        Phase::Substeps,
        Phase::Infiltrate,
        Phase::Fall,
        Phase::Exchange,
        Phase::Drain,
        Phase::WaterTable,
        Phase::Spring,
        Phase::Outlet,
        Phase::FloraStep,
        Phase::Prune,
        Phase::SkyCache,
        Phase::Drown,
        Phase::Light,
        Phase::Drink,
        Phase::Feed,
        Phase::Grow,
        Phase::Decompose,
        Phase::SeedBank,
        Phase::Propagate,
        Phase::FaunaStep,
        Phase::FaunaTerrain,
        Phase::FaunaMaintenance,
        Phase::FaunaSense,
        Phase::FaunaAct,
        Phase::FaunaBirths,
        Phase::FaunaDeaths,
    ];
    pub const COUNT: usize = Phase::ALL.len();

    pub fn index(self) -> usize {
        self as usize
    }

    /// Whether this phase's time is inside another phase's.
    pub fn is_total(self) -> bool {
        matches!(
            self,
            Phase::WorldStep | Phase::FloraStep | Phase::FaunaStep | Phase::Substeps
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            Phase::WorldStep => "World::step (total)",
            Phase::Rain => "  rain",
            Phase::Evaporate => "  evaporate",
            Phase::Substeps => "  substep loop (total)",
            Phase::Infiltrate => "    infiltrate",
            Phase::Fall => "    fall",
            Phase::Exchange => "    exchange",
            Phase::Drain => "  drain",
            Phase::WaterTable => "  water_table",
            Phase::Spring => "  spring",
            Phase::Outlet => "  outlet",
            Phase::FloraStep => "Flora::step (total)",
            Phase::Prune => "  prune_unsupported",
            Phase::SkyCache => "  sky cache",
            Phase::Drown => "  drown",
            Phase::Light => "  light",
            Phase::Drink => "  drink",
            Phase::Feed => "  feed (substrate)",
            Phase::Grow => "  grow",
            Phase::Decompose => "  decompose",
            Phase::SeedBank => "  seed_bank",
            Phase::Propagate => "  propagate",
            Phase::FaunaStep => "Fauna::step (total)",
            Phase::FaunaTerrain => "  terrain",
            Phase::FaunaMaintenance => "  maintenance",
            Phase::FaunaSense => "  sense",
            Phase::FaunaAct => "  act",
            Phase::FaunaBirths => "  births",
            Phase::FaunaDeaths => "  deaths",
        }
    }
}

/// A count that explains a phase's cost: how much work it was handed, not how long it
/// took.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    /// Cells `fall` walked: the wet columns' cells, once per substep.
    FallCells,
    /// Wet cells `infiltrate` offered to the ground, once per substep.
    InfiltrateCells,
    /// Wet cells the local exchange offered water from, per substep.
    ExchangeWet,
    /// Columns it walked for heads and displacement targets, per substep.
    ExchangeColumns,
    /// Cells of the saturated band `water_table` scanned.
    WaterTableCells,
    /// Damp cells `drain` looked at.
    DrainCells,
    /// Columns scanned by `rain` and `evaporate` (sky/open-water search included).
    ColumnScans,
    /// Cells the column searches of `rain`, `evaporate`, `drain` and `spring` walked.
    ColumnSearchCells,
    /// Stands the plant layer stepped.
    Stands,
    /// Sites with a `Ground` the plant layer walked.
    GroundSites,
    /// Root-box or mycelium-box voxel reads the plant layer made.
    BoxVoxels,
    /// Germination predicates the plant layer evaluated.
    GatesEvaluated,
    /// Sky-visibility rays cast (cache misses only).
    SkyRays,
    /// Animals the animal layer stepped.
    Animals,
    /// `reachable_foliage` queries the animal layer made.
    ReachQueries,
    /// Candidate faces the animal layer scored.
    CandidateFaces,
}

impl Count {
    pub const ALL: [Count; 16] = [
        Count::FallCells,
        Count::InfiltrateCells,
        Count::ExchangeWet,
        Count::ExchangeColumns,
        Count::WaterTableCells,
        Count::DrainCells,
        Count::ColumnScans,
        Count::ColumnSearchCells,
        Count::Stands,
        Count::GroundSites,
        Count::BoxVoxels,
        Count::GatesEvaluated,
        Count::SkyRays,
        Count::Animals,
        Count::ReachQueries,
        Count::CandidateFaces,
    ];
    pub const COUNT: usize = Count::ALL.len();

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Count::FallCells => "fall: cells visited (wet columns)",
            Count::InfiltrateCells => "infiltrate: wet cells visited",
            Count::ExchangeWet => "exchange: wet cells offering water",
            Count::ExchangeColumns => "exchange: columns walked",
            Count::WaterTableCells => "water_table: band cells scanned",
            Count::DrainCells => "drain: damp cells scanned",
            Count::ColumnScans => "column scans (rain, evaporate, ...)",
            Count::ColumnSearchCells => "cells walked by those column searches",
            Count::Stands => "stands stepped",
            Count::GroundSites => "sites with a Ground",
            Count::BoxVoxels => "root/mycelium box voxel reads",
            Count::GatesEvaluated => "germination predicates evaluated",
            Count::SkyRays => "sky-visibility rays cast",
            Count::Animals => "animals stepped",
            Count::ReachQueries => "reachable_foliage queries",
            Count::CandidateFaces => "candidate faces scored",
        }
    }
}

#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU64 = AtomicU64::new(0);
static NANOS: [AtomicU64; Phase::COUNT] = [ZERO; Phase::COUNT];
static CALLS: [AtomicU64; Phase::COUNT] = [ZERO; Phase::COUNT];
static COUNTS: [AtomicU64; Count::COUNT] = [ZERO; Count::COUNT];

/// Start timing a phase. The time is accumulated when the returned value is dropped, so
/// the phase is the value's scope.
pub fn start(phase: Phase) -> Timer {
    Timer { phase, at: Instant::now() }
}

/// One phase's timing, accumulated on drop.
pub struct Timer {
    phase: Phase,
    at: Instant,
}

impl Drop for Timer {
    fn drop(&mut self) {
        let ns = self.at.elapsed().as_nanos() as u64;
        NANOS[self.phase.index()].fetch_add(ns, Ordering::Relaxed);
        CALLS[self.phase.index()].fetch_add(1, Ordering::Relaxed);
    }
}

/// Add to a count. `n` is work units, not a timing.
pub fn add(count: Count, n: u64) {
    COUNTS[count.index()].fetch_add(n, Ordering::Relaxed);
}

pub fn nanos(phase: Phase) -> u64 {
    NANOS[phase.index()].load(Ordering::Relaxed)
}

pub fn calls(phase: Phase) -> u64 {
    CALLS[phase.index()].load(Ordering::Relaxed)
}

pub fn count(count: Count) -> u64 {
    COUNTS[count.index()].load(Ordering::Relaxed)
}

/// Forget everything: what a warm-up is separated from the measured window with.
pub fn reset() {
    for p in Phase::ALL {
        NANOS[p.index()].store(0, Ordering::Relaxed);
        CALLS[p.index()].store(0, Ordering::Relaxed);
    }
    for c in Count::ALL {
        COUNTS[c.index()].store(0, Ordering::Relaxed);
    }
}
