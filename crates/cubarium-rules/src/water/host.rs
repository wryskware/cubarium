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
//! plane + col`), so a task cannot be handed a sub-slice: the drivers view each array,
//! for the length of a phase, as `AtomicU64`s over its own memory (`F64s`, `Words128`,
//! private to this module) and every task reads and writes its columns through relaxed
//! loads and stores, which compile to the plain moves a pointer would. No `unsafe`: the
//! crate forbids it (Wrysk, 2026-10-08).

use core::marker::PhantomData;
use core::ops::Range;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering::Relaxed;

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

/// A per-cell `f64` array as the tasks share it: the array's own memory viewed as
/// `AtomicU64`s holding the floats' bits, for as long as the view borrows it mutably.
/// Every access is a relaxed load or store (a plain `mov` on x86-64 and aarch64) and
/// bounds-checked. The drivers hand every column to exactly one task and a column phase
/// touches only its own column's cells, so the relaxed accesses never actually race; the
/// atomics are what lets safe code say so.
#[derive(Clone, Copy)]
struct F64s<'a>(&'a [AtomicU64]);

impl<'a> F64s<'a> {
    fn new(v: &'a mut [f64]) -> F64s<'a> {
        F64s(AtomicU64::from_mut_slice(bytemuck::cast_slice_mut(v)))
    }
    #[inline(always)]
    fn load(&self, i: usize) -> f64 {
        f64::from_bits(self.0[i].load(Relaxed))
    }
    #[inline(always)]
    fn store(&self, i: usize, v: f64) {
        self.0[i].store(v.to_bits(), Relaxed)
    }
    /// `f64` addition on the cell: what a proposal into another column's cell is. Order
    /// across threads is whatever the adds land in.
    #[inline(always)]
    #[cfg(feature = "par")]
    fn add(&self, i: usize, q: f64) {
        let cell = &self.0[i];
        let mut cur = cell.load(Relaxed);
        loop {
            let new = (f64::from_bits(cur) + q).to_bits();
            match cell.compare_exchange_weak(cur, new, Relaxed, Relaxed) {
                Ok(_) => return,
                Err(seen) => cur = seen,
            }
        }
    }
}

impl Store for F64s<'_> {
    #[inline(always)]
    fn get(&self, i: usize) -> f64 {
        self.load(i)
    }
    #[inline(always)]
    fn set(&mut self, i: usize, v: f64) {
        self.store(i, v)
    }
}

/// The exchange's per-column `touched` row masks as the tasks share them: each word as
/// its two `u64` halves in `AtomicU64`s, the same way as [`F64s`], so a task can mark a
/// row of another column's word.
#[cfg(feature = "par")]
#[derive(Clone, Copy)]
struct Words128<'a>(&'a [AtomicU64]);

#[cfg(feature = "par")]
impl<'a> Words128<'a> {
    fn new(v: &'a mut [u128]) -> Words128<'a> {
        Words128(AtomicU64::from_mut_slice(bytemuck::cast_slice_mut(v)))
    }
    /// Set row `y` of column `col`'s word, from any thread: the `u64` holding bits
    /// `64·h .. 64·h + 64` of a word is its `h`-th in memory on a little-endian host.
    #[inline(always)]
    fn set_row(&self, col: usize, y: usize) {
        let h = y >> 6;
        let k = if cfg!(target_endian = "little") { h } else { 1 - h };
        self.0[2 * col + k].fetch_or(1u64 << (y & 63), Relaxed);
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

/// [`Water`]'s per-cell arrays as the tasks share them.
#[derive(Clone, Copy)]
struct Shared<'a> {
    plane: usize,
    height: usize,
    voxel: f64,
    free: F64s<'a>,
    pore: F64s<'a>,
}

/// [`Water`]'s three per-column set arrays (wet, damp, drainable), or one task's chunk of
/// them: columns are contiguous here, so each task is handed its own sub-slices.
type SetArrays<'s> = [&'s mut [u128]; 3];

impl Water<'_> {
    /// The per-cell arrays shared, and the set arrays to hand out by chunk.
    fn split(&mut self) -> (Shared<'_>, SetArrays<'_>) {
        let plane = self.grid.plane();
        assert!(self.free.len() == self.grid.cells() && self.pore.len() == self.grid.cells());
        assert!(self.wet.len() == plane && self.damp.len() == plane && self.drainable.len() == plane);
        assert!(self.grid.height <= super::MASK_ROWS);
        (
            Shared {
                plane,
                height: self.grid.height,
                voxel: self.voxel,
                free: F64s::new(self.free),
                pore: F64s::new(self.pore),
            },
            [&mut *self.wet, &mut *self.damp, &mut *self.drainable],
        )
    }
}

impl Exec<'_> {
    /// `f` over every column, in chunks, each handed its own columns' set words
    /// (`sets[k][j]` is column `cols.start + j`); the chunks' results in column order.
    fn columns_with_sets<R: Send>(
        &self,
        plane: usize,
        sets: SetArrays<'_>,
        f: impl Fn(Range<usize>, SetArrays<'_>) -> R + Sync,
    ) -> Vec<R> {
        let [wet, damp, drainable] = sets;
        #[cfg(feature = "par")]
        if let Some(pool) = self.pool {
            use rayon::prelude::*;
            return pool.install(|| {
                wet.par_chunks_mut(GRAIN)
                    .zip(damp.par_chunks_mut(GRAIN))
                    .zip(drainable.par_chunks_mut(GRAIN))
                    .enumerate()
                    .map(|(c, ((w, d), dr))| f(c * GRAIN..c * GRAIN + w.len(), [w, d, dr]))
                    .collect()
            });
        }
        vec![f(0..plane, [wet, damp, drainable])]
    }
}

impl<'a> Shared<'a> {
    /// Run `f` on one column with its three set words (its entries in the task's
    /// chunk), store them back, and return its result and the sets' count changes.
    #[inline(always)]
    fn column<R>(
        &self,
        words: [&mut u128; 3],
        f: impl FnOnce(&mut Cells<'_, F64s<'a>, F64s<'a>, Words>, Words) -> R,
    ) -> (R, [isize; 3]) {
        let (mut free, mut pore) = (self.free, self.pore);
        let [wet, damp, drainable] = words;
        let before = Words {
            wet: *wet,
            damp: *damp,
            drainable: *drainable,
            delta: [0; 3],
        };
        let mut sets = before;
        let r = {
            let mut c = Cells {
                free: &mut free,
                pore: &mut pore,
                sets: &mut sets,
                voxel: self.voxel,
                plane: self.plane,
            };
            f(&mut c, before)
        };
        *wet = sets.wet;
        *damp = sets.damp;
        *drainable = sets.drainable;
        (r, sets.delta)
    }
}

/// The task's column `k` of its chunk, as [`Shared::column`] takes it.
#[inline(always)]
fn words_at<'s>(sets: &'s mut SetArrays<'_>, k: usize) -> [&'s mut u128; 3] {
    let [w, d, dr] = sets;
    [&mut w[k], &mut d[k], &mut dr[k]]
}

fn add3(a: &mut [isize; 3], b: [isize; 3]) {
    for k in 0..3 {
        a[k] += b[k];
    }
}

/// [`column::fall`] over every column: the wet rows at the start of the call, bottom-up.
/// Returns the change in the three sets' member counts (wet, damp, drainable).
pub fn fall<G: Fn(usize) -> Ground + Sync>(exec: Exec<'_>, mut w: Water<'_>, ground: &G) -> [isize; 3] {
    let (sh, sets) = w.split();
    let mut total = [0; 3];
    for d in exec.columns_with_sets(sh.plane, sets, |cols, mut sets| {
        let mut delta = [0; 3];
        for (k, col) in cols.enumerate() {
            if sets[0][k] == 0 {
                continue;
            }
            let ((), d) = sh.column(words_at(&mut sets, k), |c, start| {
                column::fall(c, col, Rows(start.wet), ground)
            });
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
    let (sh, sets) = w.split();
    let mut total = [0; 3];
    for d in exec.columns_with_sets(sh.plane, sets, |cols, mut sets| {
        let mut delta = [0; 3];
        for (k, col) in cols.enumerate() {
            if sets[0][k] == 0 {
                continue;
            }
            let ((), d) = sh.column(words_at(&mut sets, k), |c, start| {
                column::infiltrate(c, col, Rows(start.wet), ground)
            });
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
    let (sh, sets) = w.split();
    let mut total = ([0; 3], 0.0);
    for (d, gained) in exec.columns_with_sets(sh.plane, sets, |cols, mut sets| {
        let (mut delta, mut gained) = ([0; 3], 0.0);
        for (k, col) in cols.enumerate() {
            if sets[2][k] == 0 {
                continue;
            }
            let (g, d) = sh.column(words_at(&mut sets, k), |c, start| {
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
/// column by column (on the calling thread).
struct Masks<'s> {
    plane: usize,
    sets: SetArrays<'s>,
    delta: [isize; 3],
}

impl Masks<'_> {
    #[inline(always)]
    fn flip(&mut self, k: usize, i: usize, y: usize, member: bool) {
        let word = &mut self.sets[k][i - y * self.plane];
        let count = &mut self.delta[k];
        let bit = 1u128 << y;
        if member == (*word & bit != 0) {
            return;
        }
        if member {
            *word |= bit;
            *count += 1;
        } else {
            *word &= !bit;
            *count -= 1;
        }
    }
}

impl Sets for Masks<'_> {
    #[inline(always)]
    fn wet(&mut self, i: usize, y: usize, member: bool) {
        self.flip(0, i, y, member);
    }
    #[inline(always)]
    fn damp(&mut self, i: usize, y: usize, member: bool) {
        self.flip(1, i, y, member);
    }
    #[inline(always)]
    fn drainable(&mut self, i: usize, y: usize, member: bool) {
        self.flip(2, i, y, member);
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
    let (sh, mut sets) = w.split();
    if exec.is_serial() {
        return table_in_order(&sh, sets, ground, t, charged, 0.0, true);
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
        return table_in_order(&sh, sets, ground, t, charged, 0.0, true);
    }
    let mut total = [0; 3];
    let mut taken = 0.0;
    let [a, b, c] = &mut sets;
    for (d, took) in exec.columns_with_sets(sh.plane, [&mut **a, &mut **b, &mut **c], |cols, mut sets| {
        let (mut delta, mut took) = ([0; 3], 0.0);
        for (k, col) in cols.enumerate() {
            let (t1, d) = sh.column(words_at(&mut sets, k), |c, _| {
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
        let (d, taken) = table_in_order(&sh, sets, ground, t, charged, taken, false);
        add3(&mut total, d);
        return (total, taken);
    }
    for (d, took) in exec.columns_with_sets(sh.plane, sets, |cols, mut sets| {
        let (mut delta, mut took) = ([0; 3], 0.0);
        for (k, col) in cols.enumerate() {
            let (t2, d) = sh.column(words_at(&mut sets, k), |c, _| {
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
    sh: &Shared<'_>,
    sets: SetArrays<'_>,
    ground: &G,
    t: Table,
    charged: f64,
    mut taken: f64,
    saturation: bool,
) -> ([isize; 3], f64) {
    let (mut free, mut pore) = (sh.free, sh.pore);
    let mut sets = Masks {
        plane: sh.plane,
        sets,
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
    use core::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use rayon::prelude::*;

    use super::{
        ExchangeBackend, F64s, Field, Grid, HEAD_PASSES, Params, Scratch, Words128, column_credits, column_drive_pass,
        column_edges, column_heads, commit, flip_wet, moved,
    };

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
            assert!(free.len() == grid.cells() && void.len() == plane && wet.len() == plane);
            assert!(u32::try_from(grid.cells()).is_ok(), "kept proposals index cells as u32");
            let chunks = plane.div_ceil(grain);
            sc.edges.resize_with(chunks, Vec::new);
            let Scratch { full, touched, head, drive, proposed, delta, edges } = sc;
            self.pool.install(|| {
                // A: heads, full masks, both drive buffers.
                let any_full = AtomicBool::new(false);
                {
                    let wet: &[u128] = wet;
                    let [d0, d1] = drive;
                    let (h, d0, d1) = (F64s::new(head), F64s::new(d0), F64s::new(d1));
                    let free: &[f64] = free;
                    full.par_iter_mut().enumerate().with_min_len(grain).for_each(|(col, fm)| {
                        let w = wet[col];
                        *fm = if w == 0 {
                            0
                        } else {
                            column_heads(grid, col, w, free, |i, v| {
                                h.store(i, v);
                                d0.store(i, v);
                                d1.store(i, v);
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
                        let [a, b] = &mut *drive;
                        let (src, dst) = if cur == 0 { (&*a, b) } else { (&*b, a) };
                        let (src, dst) = (&src[..], F64s::new(dst));
                        let full: &[u128] = full;
                        let wet: &[u128] = wet;
                        let raised = (0..plane)
                            .into_par_iter()
                            .with_min_len(grain)
                            .filter(|&col| full[col] != 0)
                            .map(|col| column_drive_pass(grid, col, wet, full, src, |i, v| dst.store(i, v)))
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
                    let prop = F64s::new(proposed);
                    let tw = Words128::new(touched);
                    edges.par_iter_mut().enumerate().for_each(|(c, list)| {
                        list.clear();
                        for col in c * grain..((c + 1) * grain).min(plane) {
                            if field.wet[col] != 0 {
                                column_edges(&field, p, col, |from, to_col, to_row, q| {
                                    let t = to_row * plane + to_col;
                                    prop.add(t, q);
                                    tw.set_row(to_col, to_row);
                                    list.push(((from * plane + col) as u32, t as u32, q));
                                });
                            }
                        }
                    });
                }
                // C: every cell's net change, into its own chunk's cells: the credits of
                // its columns, the debits of its kept proposals.
                {
                    let d = F64s::new(delta);
                    let proposed: &[f64] = proposed;
                    let touched: &[u128] = touched;
                    let free: &[f64] = field.free;
                    edges.par_iter().enumerate().for_each(|(c, list)| {
                        for col in c * grain..((c + 1) * grain).min(plane) {
                            if touched[col] != 0 {
                                column_credits(&field, col, touched[col], proposed, |i, v| {
                                    d.store(i, d.load(i) + v);
                                });
                            }
                        }
                        for &(from, to, q) in list {
                            let m = moved(q, proposed[to as usize], free[to as usize]);
                            if m > 0.0 {
                                d.store(from as usize, d.load(from as usize) - m);
                            }
                        }
                    });
                }
                // D: commit, keeping each chunk's wet words and counting its flips.
                let (fr, d, pr) = (F64s::new(free), F64s::new(delta), F64s::new(proposed));
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
                                let dv = d.load(i);
                                d.store(i, 0.0);
                                pr.store(i, 0.0);
                                if dv != 0.0 {
                                    let old = fr.load(i);
                                    let new = commit(old, dv);
                                    fr.store(i, new);
                                    flip_wet(word, &mut count, y, old, new);
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
