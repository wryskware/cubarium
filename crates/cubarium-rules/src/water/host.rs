//! The host's drivers of the water rules in [`super`]: the exchange, serially
//! ([`Serial`]) or across a rayon pool (`Parallel`, feature `par`), each an
//! [`ExchangeBackend`] (which is also what a GPU backend would implement); and the column
//! phases ([`fall`], [`infiltrate`], [`drain`], [`water_table`]) on whichever [`Exec`] the
//! caller hands them: its own thread, or a pool.
//!
//! **How the pool is shared safely.** Every per-column phase writes only the cells of the
//! columns it was given, and the columns are split between tasks, so no two threads ever
//! write one element; the one exception, the exchange's proposals into a neighbour's
//! cells, is an atomic add. The per-cell arrays are strided by column (a cell is `y ·
//! plane + col`), so a task cannot be handed a sub-slice: it writes through a raw pointer
//! (`Ptr`, private to this module) and the drivers here are the only code that makes
//! one.

use core::marker::PhantomData;
use core::ops::Range;

use super::column::{self, Cells, Ground, Rows, Sets, Store, Table, Words};
use super::{
    Field, Grid, HEAD_PASSES, Params, column_credits, column_drive_pass, column_edges, column_heads, commit, moved,
};

/// One substep's exchange over a world's water. `free` is the fills in the world's index
/// order; `void` and `wet` its per-column row masks (non-solid rows; rows with `free >
/// 0`), as they stand before the exchange. `wet` is updated in place to the rows that
/// hold water after it; the return is the change in the number of wet cells.
pub trait ExchangeBackend {
    fn exchange(
        &mut self,
        grid: Grid,
        free: &mut [f64],
        void: &[u128],
        wet: &mut [u128],
        p: &Params,
    ) -> isize;
}

/// The per-cell and per-column buffers the phases pass between them, sized to the world
/// on first use. Every per-cell entry a phase reads was written in this substep; the
/// proposal and delta buffers are left zero at the end of each exchange.
#[derive(Default)]
struct Scratch {
    full: Vec<u128>,
    touched: Vec<u128>,
    head: Vec<f64>,
    drive: [Vec<f64>; 2],
    proposed: Vec<f64>,
    delta: Vec<f64>,
    /// Phase B's proposals, `(from, to, q)`, kept for phase C's debits. The parallel
    /// driver keeps one list per chunk of columns.
    edges: Vec<Vec<(u32, u32, f64)>>,
}

impl Scratch {
    fn ensure(&mut self, grid: Grid) {
        let (n, plane) = (grid.cells(), grid.plane());
        if self.head.len() != n {
            self.head = vec![0.0; n];
            self.drive = [vec![0.0; n], vec![0.0; n]];
            self.proposed = vec![0.0; n];
            self.delta = vec![0.0; n];
        }
        if self.full.len() != plane {
            self.full = vec![0; plane];
            self.touched = vec![0; plane];
        }
    }
}

/// The exchange on one thread.
#[derive(Default)]
pub struct Serial {
    sc: Scratch,
}

impl ExchangeBackend for Serial {
    fn exchange(
        &mut self,
        grid: Grid,
        free: &mut [f64],
        void: &[u128],
        wet: &mut [u128],
        p: &Params,
    ) -> isize {
        let sc = &mut self.sc;
        sc.ensure(grid);
        let plane = grid.plane();
        if sc.edges.is_empty() {
            sc.edges.push(Vec::new());
        }
        let Scratch { full, touched, head, drive, proposed, delta, edges } = sc;
        let edges = &mut edges[0];
        edges.clear();

        // A: heads, full masks, both drive buffers.
        let mut any_full = false;
        for col in 0..plane {
            let w = wet[col];
            if w == 0 {
                full[col] = 0;
                continue;
            }
            let [d0, d1] = drive;
            full[col] = column_heads(grid, col, w, free, |i, h| {
                head[i] = h;
                d0[i] = h;
                d1[i] = h;
            });
            any_full |= full[col] != 0;
        }
        // Head through submerged water: Jacobi, the buffers alternating.
        let mut cur = 0;
        if any_full {
            for _ in 0..HEAD_PASSES {
                let (a, b) = drive.split_at_mut(1);
                let (src, dst) = if cur == 0 { (&a[0], &mut b[0]) } else { (&b[0], &mut a[0]) };
                let mut raised = false;
                for col in 0..plane {
                    if full[col] != 0 {
                        raised |= column_drive_pass(grid, col, &*wet, full, src, |i, h| dst[i] = h);
                    }
                }
                cur ^= 1;
                if !raised {
                    break;
                }
            }
        }
        let field = Field { grid, free, void, wet: &*wet, full, head, drive: &drive[cur] };

        // B: the proposals, kept.
        for col in 0..plane {
            if wet[col] != 0 {
                column_edges(&field, p, col, |from, to_col, to_row, q| {
                    let t = to_row * plane + to_col;
                    proposed[t] += q;
                    touched[to_col] |= 1u128 << to_row;
                    edges.push(((from * plane + col) as u32, t as u32, q));
                });
            }
        }
        // C: every cell's net change: credits by column, debits by kept proposal.
        for col in 0..plane {
            if touched[col] != 0 {
                column_credits(&field, col, touched[col], proposed, |i, d| delta[i] += d);
            }
        }
        for &(from, to, q) in edges.iter() {
            let m = moved(q, proposed[to as usize], free[to as usize]);
            if m > 0.0 {
                delta[from as usize] -= m;
            }
        }
        // D: commit, keeping the wet masks.
        let mut count = 0isize;
        for col in 0..plane {
            let mut rows = wet[col] | touched[col];
            touched[col] = 0;
            let mut word = wet[col];
            while rows != 0 {
                let y = rows.trailing_zeros() as usize;
                rows &= rows - 1;
                let i = y * plane + col;
                let d = delta[i];
                delta[i] = 0.0;
                proposed[i] = 0.0;
                if d != 0.0 {
                    let old = free[i];
                    let new = commit(old, d);
                    free[i] = new;
                    flip_wet(&mut word, &mut count, y, old, new);
                }
            }
            wet[col] = word;
        }
        count
    }
}

/// Keep a column's wet word through one commit: set or clear row `y` if the cell's
/// wetness changed from `old` to `new`, and count it.
#[inline(always)]
fn flip_wet(word: &mut u128, count: &mut isize, y: usize, old: f64, new: f64) {
    if (old > 0.0) != (new > 0.0) {
        if new > 0.0 {
            *word |= 1u128 << y;
            *count += 1;
        } else {
            *word &= !(1u128 << y);
            *count -= 1;
        }
    }
}

// ------------------------------------------------------------------ the column phases

/// Where the column phases run: on the calling thread, or split into chunks of columns
/// across a rayon pool (feature `par`).
#[derive(Clone, Copy)]
pub struct Exec<'a> {
    #[cfg(feature = "par")]
    pool: Option<&'a rayon::ThreadPool>,
    _pool: PhantomData<&'a ()>,
}

/// Columns per task on a pool.
#[cfg_attr(not(feature = "par"), allow(dead_code))]
const GRAIN: usize = 64;

impl<'a> Exec<'a> {
    /// Every column on the calling thread.
    pub fn serial() -> Exec<'a> {
        Exec {
            #[cfg(feature = "par")]
            pool: None,
            _pool: PhantomData,
        }
    }

    /// Chunks of columns across `pool`.
    #[cfg(feature = "par")]
    pub fn on(pool: &'a rayon::ThreadPool) -> Exec<'a> {
        Exec {
            pool: Some(pool),
            _pool: PhantomData,
        }
    }

    fn is_serial(&self) -> bool {
        #[cfg(feature = "par")]
        return self.pool.is_none();
        #[cfg(not(feature = "par"))]
        true
    }

    /// `f` over every column, in chunks; the chunks' results in column order, so a sum
    /// folded over them has the same order whichever thread finished first.
    fn columns<R: Send>(&self, plane: usize, f: impl Fn(Range<usize>) -> R + Sync) -> Vec<R> {
        #[cfg(feature = "par")]
        if let Some(pool) = self.pool {
            use rayon::prelude::*;
            let chunks = plane.div_ceil(GRAIN);
            return pool.install(|| {
                (0..chunks)
                    .into_par_iter()
                    .map(|c| f(c * GRAIN..((c + 1) * GRAIN).min(plane)))
                    .collect()
            });
        }
        vec![f(0..plane)]
    }
}

/// A pointer a task reads and writes a strided array through. See the module doc: the
/// drivers hand every column to exactly one task, and a column phase touches only its own
/// column's cells, so no element is ever reached from two threads at once.
#[derive(Clone, Copy)]
struct Ptr<T> {
    at: *mut T,
    len: usize,
}
unsafe impl<T: Send> Send for Ptr<T> {}
unsafe impl<T: Send> Sync for Ptr<T> {}

impl<T: Copy> Ptr<T> {
    fn new(v: &mut [T]) -> Ptr<T> {
        Ptr {
            at: v.as_mut_ptr(),
            len: v.len(),
        }
    }
    #[inline(always)]
    fn load(&self, i: usize) -> T {
        debug_assert!(i < self.len);
        // SAFETY: in bounds (the grid's cells and columns), and owned by the calling task.
        unsafe { *self.at.add(i) }
    }
    #[inline(always)]
    fn store(&mut self, i: usize, v: T) {
        debug_assert!(i < self.len);
        // SAFETY: as `load`.
        unsafe { *self.at.add(i) = v }
    }
}

impl Store for Ptr<f64> {
    #[inline(always)]
    fn get(&self, i: usize) -> f64 {
        self.load(i)
    }
    #[inline(always)]
    fn set(&mut self, i: usize, v: f64) {
        self.store(i, v)
    }
}

/// A world's water as the column phases work on it: the free and pore fills in the
/// world's index order, and the three active sets as per-column row masks (at most
/// [`super::MASK_ROWS`] rows).
pub struct Water<'a> {
    pub grid: Grid,
    /// One cell's volume, m³.
    pub voxel: f64,
    pub free: &'a mut [f64],
    pub pore: &'a mut [f64],
    pub wet: &'a mut [u128],
    pub damp: &'a mut [u128],
    pub drainable: &'a mut [u128],
}

/// [`Water`] as the tasks share it.
#[derive(Clone, Copy)]
struct Shared {
    plane: usize,
    height: usize,
    voxel: f64,
    free: Ptr<f64>,
    pore: Ptr<f64>,
    wet: Ptr<u128>,
    damp: Ptr<u128>,
    drainable: Ptr<u128>,
}

impl Water<'_> {
    fn share(&mut self) -> Shared {
        let plane = self.grid.plane();
        assert!(self.free.len() == self.grid.cells() && self.pore.len() == self.grid.cells());
        assert!(self.wet.len() == plane && self.damp.len() == plane && self.drainable.len() == plane);
        assert!(self.grid.height <= super::MASK_ROWS);
        Shared {
            plane,
            height: self.grid.height,
            voxel: self.voxel,
            free: Ptr::new(self.free),
            pore: Ptr::new(self.pore),
            wet: Ptr::new(self.wet),
            damp: Ptr::new(self.damp),
            drainable: Ptr::new(self.drainable),
        }
    }
}

impl Shared {
    /// Run `f` on column `col` with its three set words loaded, store them back, and
    /// return its result and the sets' count changes. The caller owns the column.
    #[inline(always)]
    fn column<R>(
        &self,
        col: usize,
        f: impl FnOnce(&mut Cells<'_, Ptr<f64>, Ptr<f64>, Words>, Words) -> R,
    ) -> (R, [isize; 3]) {
        let (mut free, mut pore) = (self.free, self.pore);
        let (mut wet, mut damp, mut drainable) = (self.wet, self.damp, self.drainable);
        let before = Words {
            wet: wet.load(col),
            damp: damp.load(col),
            drainable: drainable.load(col),
            delta: [0; 3],
        };
        let mut words = before;
        let r = {
            let mut c = Cells {
                free: &mut free,
                pore: &mut pore,
                sets: &mut words,
                voxel: self.voxel,
                plane: self.plane,
            };
            f(&mut c, before)
        };
        wet.store(col, words.wet);
        damp.store(col, words.damp);
        drainable.store(col, words.drainable);
        (r, words.delta)
    }
}

fn add3(a: &mut [isize; 3], b: [isize; 3]) {
    for k in 0..3 {
        a[k] += b[k];
    }
}

/// [`column::fall`] over every column: the wet rows at the start of the call, bottom-up.
/// Returns the change in the three sets' member counts (wet, damp, drainable).
pub fn fall<G: Fn(usize) -> Ground + Sync>(exec: Exec<'_>, mut w: Water<'_>, ground: &G) -> [isize; 3] {
    let sh = w.share();
    let mut total = [0; 3];
    for d in exec.columns(sh.plane, |cols| {
        let mut delta = [0; 3];
        for col in cols {
            if sh.wet.load(col) == 0 {
                continue;
            }
            let ((), d) = sh.column(col, |c, start| column::fall(c, col, Rows(start.wet), ground));
            add3(&mut delta, d);
        }
        delta
    }) {
        add3(&mut total, d);
    }
    total
}

/// [`column::infiltrate`] over every column, from its wet rows at the start of the call.
/// `ground`'s flux is over the substep. Returns the sets' count changes.
pub fn infiltrate<G: Fn(usize) -> Ground + Sync>(exec: Exec<'_>, mut w: Water<'_>, ground: &G) -> [isize; 3] {
    let sh = w.share();
    let mut total = [0; 3];
    for d in exec.columns(sh.plane, |cols| {
        let mut delta = [0; 3];
        for col in cols {
            if sh.wet.load(col) == 0 {
                continue;
            }
            let ((), d) = sh.column(col, |c, start| column::infiltrate(c, col, Rows(start.wet), ground));
            add3(&mut delta, d);
        }
        delta
    }) {
        add3(&mut total, d);
    }
    total
}

/// [`column::drain`] over every column, from its drainable rows at the start of the call,
/// against the table `table` (metres) as it stood when the phase began. Returns the sets'
/// count changes and what the aquifer gained, m³, summed over chunks in column order.
pub fn drain<G: Fn(usize) -> Ground + Sync>(
    exec: Exec<'_>,
    mut w: Water<'_>,
    ground: &G,
    voxel_m: f64,
    table: f64,
) -> ([isize; 3], f64) {
    let sh = w.share();
    let mut total = ([0; 3], 0.0);
    for (d, gained) in exec.columns(sh.plane, |cols| {
        let (mut delta, mut gained) = ([0; 3], 0.0);
        for col in cols {
            if sh.drainable.load(col) == 0 {
                continue;
            }
            let (g, d) = sh.column(col, |c, start| {
                column::drain(c, col, Rows(start.drainable), ground, voxel_m, table)
            });
            gained += g;
            add3(&mut delta, d);
        }
        (delta, gained)
    }) {
        add3(&mut total.0, d);
        total.1 += gained;
    }
    total
}

/// The whole arrays' set masks, for a pass that walks cells in index order rather than
/// column by column.
struct Masks {
    plane: usize,
    wet: Ptr<u128>,
    damp: Ptr<u128>,
    drainable: Ptr<u128>,
    delta: [isize; 3],
}

impl Masks {
    #[inline(always)]
    fn flip(p: &mut Ptr<u128>, count: &mut isize, col: usize, y: usize, member: bool) {
        let mut word = p.load(col);
        let bit = 1u128 << y;
        if member == (word & bit != 0) {
            return;
        }
        if member {
            word |= bit;
            *count += 1;
        } else {
            word &= !bit;
            *count -= 1;
        }
        p.store(col, word);
    }
}

impl Sets for Masks {
    #[inline(always)]
    fn wet(&mut self, i: usize, y: usize, member: bool) {
        Masks::flip(&mut self.wet, &mut self.delta[0], i - y * self.plane, y, member);
    }
    #[inline(always)]
    fn damp(&mut self, i: usize, y: usize, member: bool) {
        Masks::flip(&mut self.damp, &mut self.delta[1], i - y * self.plane, y, member);
    }
    #[inline(always)]
    fn drainable(&mut self, i: usize, y: usize, member: bool) {
        Masks::flip(&mut self.drainable, &mut self.delta[2], i - y * self.plane, y, member);
    }
}

/// **The water table** over every cell it reaches ([`column::saturate`], then
/// [`column::seep`]), out of an aquifer holding `charged` m³. Returns the sets' count
/// changes and what the aquifer gave up, m³.
///
/// The aquifer is shared in **index order by rule**: a cell earlier in the world's order is
/// filled first when the stock cannot pay for everyone. On one thread that is the walk
/// itself. On a pool, each half first sums what every cell would take from an unlimited
/// stock (read only, in parallel); when the stock covers that, nobody's share depends on
/// anybody else's, and the half runs column by column. When it does not, that half and
/// what follows run in index order on the calling thread, which is the rule exactly.
pub fn water_table<G: Fn(usize) -> Ground + Sync>(
    exec: Exec<'_>,
    mut w: Water<'_>,
    ground: &G,
    t: Table,
    charged: f64,
) -> ([isize; 3], f64) {
    let sh = w.share();
    if exec.is_serial() {
        return table_in_order(&sh, ground, t, charged, 0.0, true);
    }
    let sum = |v: Vec<f64>| v.into_iter().sum::<f64>();

    // Saturation: rows 0..band.
    let demand = sum(exec.columns(sh.plane, |cols| {
        let (mut free, mut pore) = (sh.free, sh.pore);
        let mut none = Words::default();
        let c = Cells {
            free: &mut free,
            pore: &mut pore,
            sets: &mut none,
            voxel: sh.voxel,
            plane: sh.plane,
        };
        let mut d = 0.0;
        for y in 0..t.band {
            if !t.submerged(y) {
                continue;
            }
            for col in cols.clone() {
                let i = y * sh.plane + col;
                d += column::saturate_demand(&c, i, ground(i));
            }
        }
        d
    }));
    if !(demand <= charged) {
        return table_in_order(&sh, ground, t, charged, 0.0, true);
    }
    let mut total = [0; 3];
    let mut taken = 0.0;
    for (d, took) in exec.columns(sh.plane, |cols| {
        let (mut delta, mut took) = ([0; 3], 0.0);
        for col in cols {
            let (t1, d) = sh.column(col, |c, _| {
                let mut took = 0.0;
                for y in 0..t.band {
                    if t.submerged(y) {
                        let i = y * sh.plane + col;
                        took += column::saturate(c, (i, y, ground(i)), f64::INFINITY);
                    }
                }
                took
            });
            took += t1;
            add3(&mut delta, d);
        }
        (delta, took)
    }) {
        add3(&mut total, d);
        taken += took;
    }

    // Seepage: rows 1..seep_top, read after saturation.
    let demand = sum(exec.columns(sh.plane, |cols| {
        let (mut free, mut pore) = (sh.free, sh.pore);
        let mut none = Words::default();
        let c = Cells {
            free: &mut free,
            pore: &mut pore,
            sets: &mut none,
            voxel: sh.voxel,
            plane: sh.plane,
        };
        let mut d = 0.0;
        for y in 1..t.seep_top {
            let (level, top_row) = (t.level(y), y + 1 == sh.height);
            for col in cols.clone() {
                d += column::seep_demand(&c, y * sh.plane + col, level, top_row, ground);
            }
        }
        d
    }));
    if !(demand <= charged - taken) {
        let (d, taken) = table_in_order(&sh, ground, t, charged, taken, false);
        add3(&mut total, d);
        return (total, taken);
    }
    for (d, took) in exec.columns(sh.plane, |cols| {
        let (mut delta, mut took) = ([0; 3], 0.0);
        for col in cols {
            let (t2, d) = sh.column(col, |c, _| {
                let mut took = 0.0;
                for y in 1..t.seep_top {
                    let (level, top_row) = (t.level(y), y + 1 == sh.height);
                    took += column::seep(c, y * sh.plane + col, y, level, top_row, ground, f64::INFINITY);
                }
                took
            });
            took += t2;
            add3(&mut delta, d);
        }
        (delta, took)
    }) {
        add3(&mut total, d);
        taken += took;
    }
    (total, taken)
}

/// The water table in index order on the calling thread, from `taken` already given up:
/// both halves, or (`saturation` false) seepage only.
fn table_in_order<G: Fn(usize) -> Ground>(
    sh: &Shared,
    ground: &G,
    t: Table,
    charged: f64,
    mut taken: f64,
    saturation: bool,
) -> ([isize; 3], f64) {
    let (mut free, mut pore) = (sh.free, sh.pore);
    let mut sets = Masks {
        plane: sh.plane,
        wet: sh.wet,
        damp: sh.damp,
        drainable: sh.drainable,
        delta: [0; 3],
    };
    let mut c = Cells {
        free: &mut free,
        pore: &mut pore,
        sets: &mut sets,
        voxel: sh.voxel,
        plane: sh.plane,
    };
    if saturation {
        for y in 0..t.band {
            if !t.submerged(y) {
                continue;
            }
            for col in 0..sh.plane {
                let i = y * sh.plane + col;
                taken += column::saturate(&mut c, (i, y, ground(i)), charged - taken);
            }
        }
    }
    for y in 1..t.seep_top {
        let (level, top_row) = (t.level(y), y + 1 == sh.height);
        for col in 0..sh.plane {
            taken += column::seep(&mut c, y * sh.plane + col, y, level, top_row, ground, charged - taken);
        }
    }
    (sets.delta, taken)
}

#[cfg(feature = "par")]
pub use par::{Parallel, pool};

#[cfg(feature = "par")]
mod par {
    use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use rayon::prelude::*;

    use super::{
        ExchangeBackend, Field, Grid, HEAD_PASSES, Params, Scratch, column_credits, column_drive_pass, column_edges,
        column_heads, commit, flip_wet, moved,
    };

    /// A pointer the phases write through from many threads. Every phase writes only the
    /// cells of the column it is given, and the columns are split between threads, so no
    /// two threads ever write one element, and nothing reads an element a phase writes.
    #[derive(Clone, Copy)]
    struct Cells<T>(*mut T);
    unsafe impl<T> Send for Cells<T> {}
    unsafe impl<T> Sync for Cells<T> {}
    impl<T: Copy> Cells<T> {
        /// # Safety
        /// `i` is in bounds and belongs to the calling task's column.
        #[inline(always)]
        unsafe fn set(self, i: usize, v: T) {
            unsafe { *self.0.add(i) = v }
        }
        #[inline(always)]
        unsafe fn get(self, i: usize) -> T {
            unsafe { *self.0.add(i) }
        }
    }

    /// `f64` addition on an `AtomicU64` holding its bits: what a proposal into another
    /// column's cell is. Order across threads is whatever the adds land in.
    #[inline(always)]
    fn add_f64(cell: &AtomicU64, q: f64) {
        let mut cur = cell.load(Ordering::Relaxed);
        loop {
            let new = (f64::from_bits(cur) + q).to_bits();
            match cell.compare_exchange_weak(cur, new, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return,
                Err(seen) => cur = seen,
            }
        }
    }

    fn as_atomic_u64(v: &mut [f64]) -> &[AtomicU64] {
        // SAFETY: f64 and AtomicU64 have the same size and alignment on the hosts this
        // builds for (asserted), and the slice is borrowed mutably for the view's life.
        const { assert!(core::mem::align_of::<AtomicU64>() == core::mem::align_of::<f64>()) };
        unsafe { core::slice::from_raw_parts(v.as_mut_ptr().cast::<AtomicU64>(), v.len()) }
    }

    fn as_atomic_u128_halves(v: &mut [u128]) -> &[AtomicU64] {
        // SAFETY: a u128 is two u64 words (little-endian: low word first), aligned to 16.
        unsafe { core::slice::from_raw_parts(v.as_mut_ptr().cast::<AtomicU64>(), v.len() * 2) }
    }

    /// The process's water pool of `threads` workers: built on the first call for that
    /// count and shared by every later one, for the life of the process.
    ///
    /// Process-global like the schedule's `bevy_tasks` compute pool, and for the same
    /// reason: a tool that runs many simulations side by side, each asking for `k`
    /// threads, gets one pool of `k` between them rather than `k` per simulation. Keyed by
    /// the count (not first-caller-wins) so a bench can compare counts in one process;
    /// nothing live asks for more than one. The workers inherit the CPU affinity of the
    /// thread that first asks, so a host that pins its loop builds the pool before it pins
    /// (`cubarium-voxel-sim`'s `Sim::new` does).
    pub fn pool(threads: usize) -> Arc<rayon::ThreadPool> {
        static POOLS: Mutex<Vec<(usize, Arc<rayon::ThreadPool>)>> = Mutex::new(Vec::new());
        let threads = threads.max(1);
        let mut pools = POOLS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, p)) = pools.iter().find(|(n, _)| *n == threads) {
            return p.clone();
        }
        let p = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .thread_name(|i| format!("water-{i}"))
                .build()
                .expect("a water pool"),
        );
        pools.push((threads, p.clone()));
        p
    }

    /// The exchange across a rayon pool. Columns are handed out in chunks of `grain`.
    pub struct Parallel {
        pool: Arc<rayon::ThreadPool>,
        sc: Scratch,
        grain: usize,
    }

    impl Parallel {
        /// On the process's shared pool of `threads` workers ([`pool`]).
        pub fn new(threads: usize) -> Parallel {
            Parallel::on(pool(threads))
        }

        /// On a pool the caller owns.
        pub fn on(pool: Arc<rayon::ThreadPool>) -> Parallel {
            Parallel {
                pool,
                sc: Scratch::default(),
                grain: 64,
            }
        }

        pub fn threads(&self) -> usize {
            self.pool.current_num_threads()
        }
    }

    impl ExchangeBackend for Parallel {
        fn exchange(
            &mut self,
            grid: Grid,
            free: &mut [f64],
            void: &[u128],
            wet: &mut [u128],
            p: &Params,
        ) -> isize {
            let grain = self.grain;
            let sc = &mut self.sc;
            sc.ensure(grid);
            let plane = grid.plane();
            let chunks = plane.div_ceil(grain);
            sc.edges.resize_with(chunks, Vec::new);
            let Scratch { full, touched, head, drive, proposed, delta, edges } = sc;
            self.pool.install(|| {
                // A: heads, full masks, both drive buffers.
                let any_full = AtomicBool::new(false);
                {
                    let wet: &[u128] = wet;
                    let (h, d0, d1) = (
                        Cells(head.as_mut_ptr()),
                        Cells(drive[0].as_mut_ptr()),
                        Cells(drive[1].as_mut_ptr()),
                    );
                    let free: &[f64] = free;
                    full.par_iter_mut().enumerate().with_min_len(grain).for_each(|(col, fm)| {
                        let w = wet[col];
                        *fm = if w == 0 {
                            0
                        } else {
                            column_heads(grid, col, w, free, |i, v| unsafe {
                                h.set(i, v);
                                d0.set(i, v);
                                d1.set(i, v);
                            })
                        };
                        if *fm != 0 {
                            any_full.store(true, Ordering::Relaxed);
                        }
                    });
                }
                // Head through submerged water.
                let mut cur = 0;
                if any_full.into_inner() {
                    for _ in 0..HEAD_PASSES {
                        let (a, b) = drive.split_at_mut(1);
                        let (src, dst) = if cur == 0 { (&a[0], &mut b[0]) } else { (&b[0], &mut a[0]) };
                        let (src, dst) = (&src[..], Cells(dst.as_mut_ptr()));
                        let full: &[u128] = full;
                        let wet: &[u128] = wet;
                        let raised = (0..plane)
                            .into_par_iter()
                            .with_min_len(grain)
                            .filter(|&col| full[col] != 0)
                            .map(|col| column_drive_pass(grid, col, wet, full, src, |i, v| unsafe { dst.set(i, v) }))
                            .reduce(|| false, |a, b| a | b);
                        cur ^= 1;
                        if !raised {
                            break;
                        }
                    }
                }
                let field = Field { grid, free, void, wet: &*wet, full, head, drive: &drive[cur] };

                // B: the proposals, added atomically into whichever column they land in,
                // and kept per chunk of giver columns.
                {
                    let prop = as_atomic_u64(proposed);
                    let tw = as_atomic_u128_halves(touched);
                    edges.par_iter_mut().enumerate().for_each(|(c, list)| {
                        list.clear();
                        for col in c * grain..((c + 1) * grain).min(plane) {
                            if field.wet[col] != 0 {
                                column_edges(&field, p, col, |from, to_col, to_row, q| {
                                    let t = to_row * plane + to_col;
                                    add_f64(&prop[t], q);
                                    let word = &tw[2 * to_col + (to_row >> 6)];
                                    word.fetch_or(1u64 << (to_row & 63), Ordering::Relaxed);
                                    list.push(((from * plane + col) as u32, t as u32, q));
                                });
                            }
                        }
                    });
                }
                // C: every cell's net change, into its own chunk's cells: the credits of
                // its columns, the debits of its kept proposals.
                {
                    let d = Cells(delta.as_mut_ptr());
                    let proposed: &[f64] = proposed;
                    let touched: &[u128] = touched;
                    let free: &[f64] = field.free;
                    edges.par_iter().enumerate().for_each(|(c, list)| {
                        for col in c * grain..((c + 1) * grain).min(plane) {
                            if touched[col] != 0 {
                                column_credits(&field, col, touched[col], proposed, |i, v| unsafe {
                                    d.set(i, d.get(i) + v);
                                });
                            }
                        }
                        for &(from, to, q) in list {
                            let m = moved(q, proposed[to as usize], free[to as usize]);
                            if m > 0.0 {
                                unsafe { d.set(from as usize, d.get(from as usize) - m) };
                            }
                        }
                    });
                }
                // D: commit, keeping each chunk's wet words and counting its flips.
                let (fr, d, pr) = (
                    Cells(free.as_mut_ptr()),
                    Cells(delta.as_mut_ptr()),
                    Cells(proposed.as_mut_ptr()),
                );
                let counts: Vec<isize> = touched
                    .par_chunks_mut(grain)
                    .zip(wet.par_chunks_mut(grain))
                    .enumerate()
                    .map(|(c, (tch, wt))| {
                        let mut count = 0isize;
                        for (k, (t, word)) in tch.iter_mut().zip(wt.iter_mut()).enumerate() {
                            let col = c * grain + k;
                            let mut rows = *word | *t;
                            *t = 0;
                            while rows != 0 {
                                let y = rows.trailing_zeros() as usize;
                                rows &= rows - 1;
                                let i = y * plane + col;
                                unsafe {
                                    let dv = d.get(i);
                                    d.set(i, 0.0);
                                    pr.set(i, 0.0);
                                    if dv != 0.0 {
                                        let old = fr.get(i);
                                        let new = commit(old, dv);
                                        fr.set(i, new);
                                        flip_wet(word, &mut count, y, old, new);
                                    }
                                }
                            }
                        }
                        count
                    })
                    .collect();
                counts.into_iter().sum()
            })
        }
    }
}
