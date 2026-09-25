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
/// and contain everything under them. Everything else is a leaf — except the water
/// leaves' **parts** (`SkyTops`, `VoidRuns`, `InfiltrateSet`, `FallSet`, `Exchange*`),
/// which are inside the leaf that calls them and must not be added to it. `Begin` also
/// runs inside `SkyTops`. `Census` is the water census's own cost ([`census`]), outside
/// every other phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    WorldStep,
    Begin,
    Rain,
    Evaporate,
    SkyTops,
    VoidRuns,
    Substeps,
    Infiltrate,
    InfiltrateSet,
    Fall,
    FallSet,
    Exchange,
    ExchangeSet,
    ExchangeScan,
    ExchangeHeads,
    ExchangeOffers,
    ExchangeApply,
    Drain,
    WaterTable,
    Spring,
    Outlet,
    Census,
    FloraStep,
    Prune,
    SkyCache,
    Drown,
    Light,
    Drink,
    Feed,
    Grow,
    Cover,
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
    /// The overlapped tick's water leg: rain through outlet, on the calling thread while
    /// the plants and animals run beside it. Contains the water leaves.
    WaterLeg,
    /// The overlapped tick's water copied back from the read copy, inside the water leg.
    ReadCopy,
    /// The overlapped tick's barrier: the plants' planned drink applied.
    Settle,
}

impl Phase {
    pub const ALL: [Phase; 44] = [
        Phase::WorldStep,
        Phase::Begin,
        Phase::Rain,
        Phase::Evaporate,
        Phase::SkyTops,
        Phase::VoidRuns,
        Phase::Substeps,
        Phase::Infiltrate,
        Phase::InfiltrateSet,
        Phase::Fall,
        Phase::FallSet,
        Phase::Exchange,
        Phase::ExchangeSet,
        Phase::ExchangeScan,
        Phase::ExchangeHeads,
        Phase::ExchangeOffers,
        Phase::ExchangeApply,
        Phase::Drain,
        Phase::WaterTable,
        Phase::Spring,
        Phase::Outlet,
        Phase::Census,
        Phase::FloraStep,
        Phase::Prune,
        Phase::SkyCache,
        Phase::Drown,
        Phase::Light,
        Phase::Drink,
        Phase::Feed,
        Phase::Grow,
        Phase::Cover,
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
        Phase::WaterLeg,
        Phase::ReadCopy,
        Phase::Settle,
    ];
    pub const COUNT: usize = Phase::ALL.len();

    pub fn index(self) -> usize {
        self as usize
    }

    /// Whether this phase's time is inside another phase's.
    pub fn is_total(self) -> bool {
        matches!(
            self,
            Phase::WorldStep
                | Phase::FloraStep
                | Phase::FaunaStep
                | Phase::Substeps
                | Phase::WaterLeg
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            Phase::WorldStep => "World::step (total)",
            Phase::Begin => "  begin (active-set check; also inside sky tops)",
            Phase::Rain => "  rain",
            Phase::Evaporate => "  evaporate",
            Phase::SkyTops => "    sky tops (part of rain/evaporate)",
            Phase::VoidRuns => "    void-run cache rebuild (part of its caller)",
            Phase::Substeps => "  substep loop (total)",
            Phase::Infiltrate => "    infiltrate",
            Phase::InfiltrateSet => "      wet snapshot (part of infiltrate)",
            Phase::Fall => "    fall",
            Phase::FallSet => "      wet snapshot (part of fall)",
            Phase::Exchange => "    exchange",
            Phase::ExchangeSet => "      set, masks, columns (part of exchange)",
            Phase::ExchangeScan => "      column heads (part of exchange)",
            Phase::ExchangeHeads => "      head passes (part of exchange)",
            Phase::ExchangeOffers => "      proposals (part of exchange)",
            Phase::ExchangeApply => "      accept and apply (part of exchange)",
            Phase::Drain => "  drain",
            Phase::WaterTable => "  water_table",
            Phase::Spring => "  spring",
            Phase::Outlet => "  outlet",
            Phase::Census => "  water census (measurement, outside every phase)",
            Phase::FloraStep => "Flora::step (total)",
            Phase::Prune => "  prune_unsupported",
            Phase::SkyCache => "  sky cache",
            Phase::Drown => "  drown",
            Phase::Light => "  light",
            Phase::Drink => "  drink",
            Phase::Feed => "  feed (substrate)",
            Phase::Grow => "  grow",
            Phase::Cover => "  latticevine cover",
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
            Phase::WaterLeg => "water leg (total, overlapped tick)",
            Phase::ReadCopy => "  water copy-back from the read copy (part of the water leg)",
            Phase::Settle => "drink settle (overlapped tick's barrier)",
        }
    }
}

/// A count that explains a phase's cost: how much work it was handed, not how long it
/// took.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    /// Wet cells `fall` visited after the bottom row, once per substep.
    FallCells,
    /// Wet cells `infiltrate` offered to the ground, once per substep.
    InfiltrateCells,
    /// Wet cells the local exchange offered water from, per substep.
    ExchangeWet,
    /// Columns it walked for heads and displacement targets, per substep.
    ExchangeColumns,
    /// Cells of the saturated band `water_table` scanned.
    WaterTableCells,
    /// Cells `drain` looked at: the drainable set since package D.
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
    /// Full cells (`free >= 1 - ROOM_EPS`) in the exchange's active set, per substep: the
    /// cells the head passes walk.
    ExchangeFull,
    /// Head passes the exchange actually ran (the early exit included), per substep.
    HeadPasses,
    /// Full-cell visits those passes made: full cells times passes run.
    HeadPassCells,
    /// Proposals the exchange made (edges, displacement splits counted separately).
    ExchangeEdges,
    /// Of those, the edges whose accepted volume was nonzero.
    ExchangeMoved,
    /// Cells whose free water the exchange's settlement changed (net per substep).
    ExchangeNet,
    /// Of the proposals, those landing **above** the giver's own row: a push displaced up
    /// a neighbour's run.
    ExchangeRaised,
    /// Cells `fall` actually moved water out of.
    FallMoved,
    /// Rebuilds of the terrain-owned void-run cache (terrain edits and fresh worlds).
    VoidRunRebuilds,
    /// Damp cells `drain` found over their field capacity and above the water table.
    DrainOver,
    /// Of those, the cells it actually moved pore water out of.
    DrainMoved,
}

impl Count {
    pub const ALL: [Count; 27] = [
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
        Count::ExchangeFull,
        Count::HeadPasses,
        Count::HeadPassCells,
        Count::ExchangeEdges,
        Count::ExchangeMoved,
        Count::ExchangeNet,
        Count::ExchangeRaised,
        Count::FallMoved,
        Count::VoidRunRebuilds,
        Count::DrainOver,
        Count::DrainMoved,
    ];
    pub const COUNT: usize = Count::ALL.len();

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Count::FallCells => "fall: wet cells visited (snapshot)",
            Count::InfiltrateCells => "infiltrate: wet cells visited",
            Count::ExchangeWet => "exchange: wet cells offering water",
            Count::ExchangeColumns => "exchange: columns walked",
            Count::WaterTableCells => "water_table: band cells scanned",
            Count::DrainCells => "drain: drainable cells scanned",
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
            Count::ExchangeFull => "exchange: full cells in the active set",
            Count::HeadPasses => "exchange: head passes run",
            Count::HeadPassCells => "exchange: full-cell visits by the head passes",
            Count::ExchangeEdges => "exchange: proposals (edges)",
            Count::ExchangeMoved => "exchange: edges that moved water",
            Count::ExchangeNet => "exchange: cells whose water changed (net)",
            Count::ExchangeRaised => "exchange: proposals above the giver's row",
            Count::FallMoved => "fall: cells that moved water down",
            Count::VoidRunRebuilds => "void-run cache rebuilds",
            Count::DrainOver => "drain: damp cells over field capacity",
            Count::DrainMoved => "drain: cells that moved pore water",
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
    Timer {
        phase,
        at: Instant::now(),
    }
}

/// One phase's timing, accumulated on drop.
pub struct Timer {
    phase: Phase,
    at: Instant,
}

impl Timer {
    /// Book the phase so far and start timing `phase` from here: consecutive parts of one
    /// phase with one clock read between them.
    pub fn next(&mut self, phase: Phase) {
        let now = Instant::now();
        let ns = now.duration_since(self.at).as_nanos() as u64;
        NANOS[self.phase.index()].fetch_add(ns, Ordering::Relaxed);
        CALLS[self.phase.index()].fetch_add(1, Ordering::Relaxed);
        self.phase = phase;
        self.at = now;
    }
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

/// **The water census**: which cells each water phase changed in one tick, and the shape
/// of the water the tick left (`design/handoffs/voxel-water-algorithms-2026-09-22.md`,
/// "Proposed next work"). Measurement only, like everything in this module.
///
/// A driver opens a tick with [`census::begin_tick`] and closes it with
/// [`census::end_tick`]; in between, every water phase call is bracketed by
/// [`census::before`] and [`census::after`], which snapshot the wet set's free water and
/// compare, so a cell counts for a phase when that call left it different (a cell a call
/// fills and empties again is not seen: that is gross movement inside one call, and the
/// exchange's own edge counts are where gross movement is measured). Outside an open tick
/// every bracket is one thread-local flag read, and nothing is stored. All of it is timed
/// as [`Phase::Census`] and none of it inside any other phase.
///
/// Per tick, at the close: wet and full cells, vertical wet runs, distinct wet columns, the
/// free-depth histogram, and which phases changed which cells — so the cells no phase
/// touched (sleeping's ceiling per cell) and the 8 × 8-column tiles none of whose water
/// was touched (its ceiling per tile) fall out of the same pass.
pub mod census {
    use std::cell::{Cell, RefCell};

    use super::{Phase, start};
    use crate::World;

    /// The water phases, as the census attributes changes to them. A rain pulse
    /// (`Command::RainPulse`) counts as `Rain`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Tag {
        Rain,
        Evaporate,
        Infiltrate,
        Fall,
        Exchange,
        Drain,
        WaterTable,
        Spring,
        Outlet,
    }

    impl Tag {
        pub const ALL: [Tag; 9] = [
            Tag::Rain,
            Tag::Evaporate,
            Tag::Infiltrate,
            Tag::Fall,
            Tag::Exchange,
            Tag::Drain,
            Tag::WaterTable,
            Tag::Spring,
            Tag::Outlet,
        ];
        pub const COUNT: usize = Tag::ALL.len();

        pub fn index(self) -> usize {
            self as usize
        }

        fn bit(self) -> u16 {
            1 << self as u16
        }

        pub fn name(self) -> &'static str {
            match self {
                Tag::Rain => "rain (and rain pulses)",
                Tag::Evaporate => "evaporation",
                Tag::Infiltrate => "infiltration",
                Tag::Fall => "fall",
                Tag::Exchange => "exchange",
                Tag::Drain => "drain",
                Tag::WaterTable => "water table",
                Tag::Spring => "spring and re-entry",
                Tag::Outlet => "outlet",
            }
        }
    }

    /// The depth histogram's lower edges, in cell units: bin 0 is below `EDGES[0]`, bin
    /// `k` is `[EDGES[k-1], EDGES[k])`, bin 9 is `[1e-1, full)` and bin 10 is full
    /// (`free >= 1 - 1e-12`, the exchange's own `ROOM_EPS`).
    pub const EDGES: [f64; 9] = [1e-9, 1e-8, 1e-7, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1];
    pub const BINS: usize = EDGES.len() + 2;
    const FULL: f64 = 1.0 - 1e-12;

    /// A tile of columns for the per-tile sleeping ceiling: `TILE` × `TILE` columns.
    pub const TILE: usize = 8;

    const FLOW: u16 = (1 << Tag::Fall as u16) | (1 << Tag::Exchange as u16);
    const EVAPORATION: u16 = 1 << Tag::Evaporate as u16;

    /// Sums over the census ticks; divide by `ticks` (or `substeps`) for a mean.
    #[derive(Clone, Debug, Default)]
    pub struct Totals {
        /// Ticks the census closed.
        pub ticks: u64,
        /// Wet cells at the close of each tick.
        pub wet: u64,
        /// Of those, full ones.
        pub full: u64,
        /// Vertical runs of wet cells (a wet cell whose cell below is dry or solid starts
        /// one).
        pub runs: u64,
        /// Columns holding any wet cell.
        pub columns: u64,
        /// Wet cells by free depth, [`EDGES`].
        pub depth: [u64; BINS],
        /// Wet cells within 1e-6 of full and not full: a packed cell a sideways debit left
        /// short, which the next `fall` repacks.
        pub nearly_full: u64,
        /// Cells any phase changed in the tick (wet at its start or its close).
        pub touched: u64,
        /// Of those, cells whose free water at the close differs from the start: net.
        pub net: u64,
        /// Cells each phase changed in the tick, however many of its calls did.
        pub by_tag: [u64; Tag::COUNT],
        /// Cells each phase changed, summed over its calls (four substeps count four).
        pub per_call: [u64; Tag::COUNT],
        /// Wet cells at the close that no phase changed: sleeping's per-cell ceiling.
        pub untouched_wet: u64,
        /// Wet cells at the close that only evaporation changed.
        pub evaporation_only: u64,
        /// Wet cells at the close that neither fall nor exchange changed.
        pub flow_quiet: u64,
        /// Tiles holding water at the close.
        pub tiles_wet: u64,
        /// Of those, tiles none of whose cells any phase changed, and their wet cells.
        pub tiles_asleep: u64,
        pub wet_in_asleep_tiles: u64,
        /// The same with evaporation's changes ignored.
        pub tiles_asleep_but_evaporation: u64,
        pub wet_in_asleep_but_evaporation_tiles: u64,
        /// Exchange substeps seen, and per substep (summed): its active wet cells, those
        /// with no nonzero accepted edge in or out, and the wet runs and wet columns of
        /// its masks.
        pub substeps: u64,
        pub exchange_wet: u64,
        pub exchange_no_edge: u64,
        pub exchange_runs: u64,
        pub exchange_columns: u64,
        /// Per substep (summed): active wet cells not full under a wet cell, whose head
        /// is their run's top.
        pub exchange_unpacked: u64,
        /// Per substep (summed): the exchange's proposals by the giver's free depth,
        /// [`EDGES`] — the work a depth threshold would remove, not just the cells.
        pub edges_by_depth: [u64; BINS],
        /// The same, from **unpacked** givers only.
        pub edges_by_depth_unpacked: [u64; BINS],
        /// Net-changed cells by the size of their change over the tick, `|end - start|`
        /// binned on [`EDGES`] — how far a sleep tolerance would have to reach.
        pub net_by_size: [u64; BINS],
    }

    /// One exchange substep's shape, handed over from inside the exchange.
    pub struct Substep {
        pub wet: u64,
        pub no_edge: u64,
        pub runs: u64,
        pub columns: u64,
        pub unpacked: u64,
        pub edge_depth: [u64; BINS],
        pub edge_depth_unpacked: [u64; BINS],
    }

    /// The histogram bin of a free depth, [`EDGES`].
    #[inline]
    pub fn bin(f: f64) -> usize {
        if f >= FULL {
            BINS - 1
        } else {
            EDGES.iter().take_while(|&&e| f >= e).count()
        }
    }

    #[derive(Default)]
    struct State {
        stamp: u32,
        mark: Vec<u32>,
        snap: Vec<(u32, f64)>,
        mask: Vec<u16>,
        first: Vec<f64>,
        list: Vec<u32>,
        column: Vec<u32>,
        tile_wet: Vec<u32>,
        tile_awake: Vec<u16>,
        totals: Totals,
    }

    impl State {
        fn ensure(&mut self, n: usize, plane: usize, tiles: usize) {
            if self.mark.len() != n {
                self.mark = vec![0; n];
                self.mask = vec![0; n];
                self.first = vec![0.0; n];
                self.list.clear();
            }
            if self.column.len() != plane {
                self.column = vec![0; plane];
            }
            if self.tile_wet.len() != tiles {
                self.tile_wet = vec![0; tiles];
                self.tile_awake = vec![0; tiles];
            }
        }

        fn next_stamp(&mut self) -> u32 {
            self.stamp = self.stamp.wrapping_add(1);
            if self.stamp == 0 {
                self.mark.fill(0);
                self.column.fill(0);
                self.stamp = 1;
            }
            self.stamp
        }

        fn touch(&mut self, i: usize, tag: Tag, old: f64) {
            if self.mask[i] == 0 {
                self.list.push(i as u32);
                self.first[i] = old;
            }
            self.mask[i] |= tag.bit();
        }
    }

    thread_local! {
        static OPEN: Cell<bool> = const { Cell::new(false) };
        static STATE: RefCell<State> = RefCell::new(State::default());
    }

    fn dims(w: &World) -> (usize, usize, usize, usize) {
        let width = w.config.width as usize;
        let plane = width * w.config.depth as usize;
        let tiles_x = width.div_ceil(TILE);
        let tiles = tiles_x * (w.config.depth as usize).div_ceil(TILE);
        (plane, width, tiles_x, tiles)
    }

    /// Whether a census tick is open on this thread.
    #[inline]
    pub fn open() -> bool {
        OPEN.with(Cell::get)
    }

    /// Open a census tick: everything the water phases do until [`end_tick`] is counted.
    /// A rain pulse applied after this and before the step counts in this tick.
    pub fn begin_tick(w: &World) {
        let _t = start(Phase::Census);
        let (plane, _, _, tiles) = dims(w);
        STATE.with(|s| s.borrow_mut().ensure(w.config.cells(), plane, tiles));
        OPEN.with(|o| o.set(true));
    }

    /// Snapshot the wet set before a phase call. `None` (nothing stored) outside a tick.
    pub fn before(w: &World, tag: Tag) -> Option<Tag> {
        if !open() {
            return None;
        }
        let _t = start(Phase::Census);
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            let stamp = s.next_stamp();
            s.snap.clear();
            for i in w.wet.members() {
                s.mark[i] = stamp;
                s.snap.push((i as u32, w.free[i]));
            }
        });
        Some(tag)
    }

    /// Compare after the call [`before`] opened: every snapshot cell whose water changed,
    /// and every cell that is wet now and was not, is the phase's.
    pub fn after(w: &World, tag: Option<Tag>) {
        let Some(tag) = tag else {
            return;
        };
        let _t = start(Phase::Census);
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            let stamp = s.stamp;
            let mut changed = 0u64;
            let snap = std::mem::take(&mut s.snap);
            for &(i, old) in &snap {
                let i = i as usize;
                if w.free[i] != old {
                    s.touch(i, tag, old);
                    changed += 1;
                }
            }
            s.snap = snap;
            for i in w.wet.members() {
                if s.mark[i] != stamp {
                    s.touch(i, tag, 0.0);
                    changed += 1;
                }
            }
            s.totals.per_call[tag.index()] += changed;
        });
    }

    /// One exchange substep's gross-movement shape, from inside the exchange.
    pub fn exchange_substep(sub: Substep) {
        let _t = start(Phase::Census);
        STATE.with(|s| {
            let t = &mut s.borrow_mut().totals;
            t.substeps += 1;
            t.exchange_wet += sub.wet;
            t.exchange_no_edge += sub.no_edge;
            t.exchange_runs += sub.runs;
            t.exchange_columns += sub.columns;
            t.exchange_unpacked += sub.unpacked;
            for k in 0..BINS {
                t.edges_by_depth[k] += sub.edge_depth[k];
                t.edges_by_depth_unpacked[k] += sub.edge_depth_unpacked[k];
            }
        });
    }

    /// Close the tick: the water's shape now, and who changed what.
    pub fn end_tick(w: &World) {
        if !open() {
            return;
        }
        let _t = start(Phase::Census);
        OPEN.with(|o| o.set(false));
        let (plane, width, tiles_x, tiles) = dims(w);
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            let stamp = s.next_stamp();
            let t = &mut s.totals;
            t.ticks += 1;
            s.tile_wet.fill(0);
            s.tile_awake.fill(0);
            let tile_of = |col: usize| {
                let (z, x) = (col / width, col % width);
                (z / TILE) * tiles_x + x / TILE
            };
            for i in w.wet.members() {
                let f = w.free[i];
                t.wet += 1;
                let b = bin(f);
                if b == BINS - 1 {
                    t.full += 1;
                } else if f >= 1.0 - 1e-6 {
                    t.nearly_full += 1;
                }
                t.depth[b] += 1;
                let col = i % plane;
                if i < plane || !(w.free[i - plane] > 0.0 && !w.material[i - plane].is_solid()) {
                    t.runs += 1;
                }
                if s.column[col] != stamp {
                    s.column[col] = stamp;
                    t.columns += 1;
                }
                s.tile_wet[tile_of(col)] += 1;
                let m = s.mask[i];
                if m == 0 {
                    t.untouched_wet += 1;
                } else if m == EVAPORATION {
                    t.evaporation_only += 1;
                }
                if m & FLOW == 0 {
                    t.flow_quiet += 1;
                }
            }
            for &i in &s.list {
                let i = i as usize;
                let m = s.mask[i];
                s.mask[i] = 0;
                t.touched += 1;
                if w.free[i] != s.first[i] {
                    t.net += 1;
                    t.net_by_size[bin((w.free[i] - s.first[i]).abs())] += 1;
                }
                for tag in Tag::ALL {
                    if m & tag.bit() != 0 {
                        t.by_tag[tag.index()] += 1;
                    }
                }
                s.tile_awake[tile_of(i % plane)] |= m;
            }
            s.list.clear();
            for k in 0..tiles {
                let wet = u64::from(s.tile_wet[k]);
                if wet == 0 {
                    continue;
                }
                t.tiles_wet += 1;
                if s.tile_awake[k] == 0 {
                    t.tiles_asleep += 1;
                    t.wet_in_asleep_tiles += wet;
                }
                if s.tile_awake[k] & !EVAPORATION == 0 {
                    t.tiles_asleep_but_evaporation += 1;
                    t.wet_in_asleep_but_evaporation_tiles += wet;
                }
            }
        });
    }

    /// Everything the census has summed since the last [`reset`].
    pub fn totals() -> Totals {
        STATE.with(|s| s.borrow().totals.clone())
    }

    /// Forget the sums (the per-cell buffers are kept, and are clean between ticks).
    pub fn reset() {
        STATE.with(|s| s.borrow_mut().totals = Totals::default());
    }
}

#[cfg(test)]
mod tests {
    use super::census::{self, Tag};
    use crate::{Command, Config, Material, World, water};

    /// A bedrock floor, a soil cell, a two-cell column of water beside a dry cell and a
    /// film on the soil, under evaporation: every kind of change the census attributes.
    fn pond() -> World {
        let mut w = World::empty(Config {
            width: 4,
            height: 4,
            depth: 1,
            voxel_m: 1.0,
            seed: 7,
            evaporation_m_per_s: 1e-3,
            ..Config::default()
        });
        w.apply(Command::SetMaterial {
            x: 3,
            y: 1,
            z: 0,
            material: Material::Soil,
        });
        for (x, y, volume_m3) in [(0, 1, 1.0), (0, 2, 0.5), (3, 2, 1e-6)] {
            w.apply(Command::AddWater {
                x,
                y,
                z: 0,
                volume_m3,
            });
        }
        w
    }

    #[test]
    fn a_census_tick_counts_the_water_and_leaves_the_world_as_it_was() {
        let mut plain = pond();
        let mut counted = pond();
        census::reset();
        for _ in 0..3 {
            water::step(&mut plain, 1);
            census::begin_tick(&counted);
            water::step(&mut counted, 1);
            census::end_tick(&counted);
        }
        assert_eq!(plain, counted, "the census changed the world");
        assert!(!census::open(), "end_tick closes the tick");

        let t = census::totals();
        assert_eq!(t.ticks, 3);
        assert_eq!(t.depth.iter().sum::<u64>(), t.wet);
        assert!(t.full <= t.wet && t.runs <= t.wet && t.columns <= t.runs);
        assert!(t.net <= t.touched && t.untouched_wet <= t.wet);
        // The column spreads into its dry neighbour, the film soaks in, the open surfaces
        // evaporate: each phase is seen, and nothing a closed budget does not run is.
        for tag in [Tag::Fall, Tag::Exchange, Tag::Evaporate, Tag::Infiltrate] {
            assert!(t.by_tag[tag.index()] > 0, "{} saw nothing", tag.name());
        }
        assert_eq!(t.by_tag[Tag::Rain.index()], 0);
        assert_eq!(t.substeps, 12, "four substeps a tick");
        assert!(t.exchange_no_edge <= t.exchange_wet);

        // Outside a tick nothing is stored.
        water::step(&mut counted, 1);
        assert_eq!(census::totals().ticks, 3);
        assert_eq!(census::totals().substeps, 12);
    }
}
