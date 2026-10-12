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
    Field, Grid, HEAD_PASSES, Layout, MASK_ROWS, One, Params, Stride, column_credits_words,
    column_drive_pass, column_edges, column_heads_words, commit, moved,
};

/// One substep's exchange over a world's water. `free` is the fills in the world's index
/// order; `void` and `wet` its per-column row masks (non-solid rows; rows with `free >
/// 0`), with `grid.mask_words()` contiguous words per column. `wet` is updated in place to the rows that
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
    /// Bytes of buffer held (capacity).
    fn bytes(&self) -> usize {
        16 * (self.full.capacity() + self.touched.capacity())
            + 8 * (self.head.capacity()
                + self.drive[0].capacity()
                + self.drive[1].capacity()
                + self.proposed.capacity()
                + self.delta.capacity())
            + self
                .edges
                .iter()
                .map(|e| e.capacity() * core::mem::size_of::<(u32, u32, f64)>())
                .sum::<usize>()
    }

    fn ensure(&mut self, grid: Grid) {
        let (n, plane) = (grid.cells(), grid.plane());
        if self.head.len() != n {
            self.head = vec![0.0; n];
            self.drive = [vec![0.0; n], vec![0.0; n]];
            self.proposed = vec![0.0; n];
            self.delta = vec![0.0; n];
        }
        let masks = plane * grid.mask_words();
        if self.full.len() != masks {
            self.full = vec![0; masks];
            self.touched = vec![0; masks];
        }
    }
}

/// The exchange on one thread.
#[derive(Default)]
pub struct Serial {
    sc: Scratch,
}

impl Serial {
    /// Bytes of working buffer this driver holds (capacity).
    pub fn scratch_bytes(&self) -> usize {
        self.sc.bytes()
    }
}

/// `$f(layout, args…)` with the grid's mask layout: [`One`] when a column fits one word,
/// else [`Stride`]. The choice is made once per call, and each layout runs its own
/// monomorphized copy of the one generic body.
macro_rules! by_layout {
    ($grid:expr, $f:ident($($arg:expr),* $(,)?)) => {{
        let words = $grid.mask_words();
        if words == 1 {
            $f(One, $($arg),*)
        } else {
            $f(Stride(words), $($arg),*)
        }
    }};
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
        by_layout!(
            grid,
            serial_exchange(&mut self.sc, grid, free, void, wet, p)
        )
    }
}

fn serial_exchange<L: Layout>(
    layout: L,
    sc: &mut Scratch,
    grid: Grid,
    free: &mut [f64],
    void: &[u128],
    wet: &mut [u128],
    p: &Params,
) -> isize {
    sc.ensure(grid);
    let plane = grid.plane();
    let words = layout.words();
    if sc.edges.is_empty() {
        sc.edges.push(Vec::new());
    }
    let Scratch {
        full,
        touched,
        head,
        drive,
        proposed,
        delta,
        edges,
    } = sc;
    let edges = &mut edges[0];
    edges.clear();

    // A: heads, full masks, both drive buffers.
    let mut any_full = false;
    for col in 0..plane {
        let w = layout.column(wet, col);
        let fm = &mut full[col * words..(col + 1) * words];
        if w.iter().all(|&m| m == 0) {
            fm.fill(0);
            continue;
        }
        let [d0, d1] = drive;
        column_heads_words(grid, layout, col, w, fm, free, |i, h| {
            head[i] = h;
            d0[i] = h;
            d1[i] = h;
        });
        any_full |= fm.iter().any(|&m| m != 0);
    }
    // Head through submerged water: Jacobi, the buffers alternating.
    let mut cur = 0;
    if any_full {
        for _ in 0..HEAD_PASSES {
            let (a, b) = drive.split_at_mut(1);
            let (src, dst) = if cur == 0 {
                (&a[0], &mut b[0])
            } else {
                (&b[0], &mut a[0])
            };
            let mut raised = false;
            for col in 0..plane {
                if layout.column(full, col).iter().any(|&m| m != 0) {
                    raised |=
                        column_drive_pass(grid, layout, col, &*wet, full, src, |i, h| dst[i] = h);
                }
            }
            cur ^= 1;
            if !raised {
                break;
            }
        }
    }
    let field = Field {
        grid,
        layout,
        free,
        void,
        wet: &*wet,
        full,
        head,
        drive: &drive[cur],
    };

    // B: the proposals, kept.
    for col in 0..plane {
        if layout.column(wet, col).iter().any(|&m| m != 0) {
            column_edges(&field, p, col, |from, to_col, to_row, q| {
                let t = to_row * plane + to_col;
                proposed[t] += q;
                touched[layout.index(to_col, to_row)] |= 1u128 << (to_row % MASK_ROWS);
                edges.push(((from * plane + col) as u32, t as u32, q));
            });
        }
    }
    // C: every cell's net change: credits by column, debits by kept proposal.
    for col in 0..plane {
        let t = layout.column(touched, col);
        if t.iter().any(|&m| m != 0) {
            column_credits_words(&field, col, t, proposed, |i, d| delta[i] += d);
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
        for k in 0..words {
            let at = col * words + k;
            let mut rows = wet[at] | touched[at];
            touched[at] = 0;
            let word = &mut wet[at];
            while rows != 0 {
                let bit = rows.trailing_zeros() as usize;
                let y = k * MASK_ROWS + bit;
                rows &= rows - 1;
                let i = y * plane + col;
                let d = delta[i];
                delta[i] = 0.0;
                proposed[i] = 0.0;
                if d != 0.0 {
                    let old = free[i];
                    let new = commit(old, d);
                    free[i] = new;
                    flip_wet(word, &mut count, bit, old, new);
                }
            }
        }
    }
    count
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

/// A world's water as the column phases work on it: the free and pore fills in the
/// world's index order, and the three active sets as contiguous mask words per column.
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
struct Shared<'a, L> {
    plane: usize,
    height: usize,
    /// The set arrays' layout, chosen once per phase ([`by_layout`]).
    layout: L,
    voxel: f64,
    free: F64s<'a>,
    pore: F64s<'a>,
}

/// [`Water`]'s three per-column set arrays (wet, damp, drainable), or one task's chunk of
/// them: columns are contiguous here, so each task is handed its own sub-slices.
type SetArrays<'s> = [&'s mut [u128]; 3];

impl Water<'_> {
    /// The per-cell arrays shared, and the set arrays to hand out by chunk.
    fn split<L: Layout>(&mut self, layout: L) -> (Shared<'_, L>, SetArrays<'_>) {
        let plane = self.grid.plane();
        assert!(self.free.len() == self.grid.cells() && self.pore.len() == self.grid.cells());
        let words = self.grid.mask_words();
        assert_eq!(layout.words(), words);
        let masks = plane * words;
        assert!(
            self.wet.len() == masks && self.damp.len() == masks && self.drainable.len() == masks
        );
        (
            Shared {
                plane,
                height: self.grid.height,
                layout,
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
        words: usize,
        sets: SetArrays<'_>,
        f: impl Fn(Range<usize>, SetArrays<'_>) -> R + Sync,
    ) -> Vec<R> {
        let [wet, damp, drainable] = sets;
        #[cfg(feature = "par")]
        if let Some(pool) = self.pool {
            use rayon::prelude::*;
            return pool.install(|| {
                wet.par_chunks_mut(GRAIN * words)
                    .zip(damp.par_chunks_mut(GRAIN * words))
                    .zip(drainable.par_chunks_mut(GRAIN * words))
                    .enumerate()
                    .map(|(c, ((w, d), dr))| f(c * GRAIN..c * GRAIN + w.len() / words, [w, d, dr]))
                    .collect()
            });
        }
        debug_assert_eq!(wet.len(), plane * words);
        vec![f(0..plane, [wet, damp, drainable])]
    }
}

impl<'a, L: Layout> Shared<'a, L> {
    /// Run `f` on one column with its three sets' words (its entries in the task's
    /// chunk), store them back, and return its result and the sets' count changes.
    #[inline(always)]
    fn column<R>(
        &self,
        words: SetArrays<'_>,
        f: impl FnOnce(&mut Cells<'_, F64s<'a>, F64s<'a>, ColumnSets<'_, L>>) -> R,
    ) -> (R, [isize; 3]) {
        let (mut free, mut pore) = (self.free, self.pore);
        let mut sets = ColumnSets {
            low: [words[0][0], words[1][0], words[2][0]],
            sets: words,
            layout: self.layout,
            delta: [0; 3],
        };
        let r = {
            let mut c = Cells {
                free: &mut free,
                pore: &mut pore,
                sets: &mut sets,
                voxel: self.voxel,
                plane: self.plane,
            };
            f(&mut c)
        };
        for k in 0..3 {
            sets.sets[k][0] = sets.low[k];
        }
        (r, sets.delta)
    }
}

/// The task's column `k` of its chunk, as [`Shared::column`] takes it.
#[inline(always)]
fn words_at<'s>(sets: &'s mut SetArrays<'_>, k: usize, words: usize) -> SetArrays<'s> {
    let [w, d, dr] = sets;
    let a = k * words;
    let b = a + words;
    [&mut w[a..b], &mut d[a..b], &mut dr[a..b]]
}

/// One column's words; a transfer may update the word on either side of a boundary.
/// Each set's lowest word — a short world's only one — is held in `low` for the call and
/// stored back after it ([`Shared::column`]), so the one-word build keeps it in registers.
struct ColumnSets<'s, L> {
    sets: SetArrays<'s>,
    low: [u128; 3],
    layout: L,
    delta: [isize; 3],
}

impl<L: Layout> ColumnSets<'_, L> {
    /// Word `w` of set `k` as it stands now.
    #[inline(always)]
    fn word(&self, k: usize, w: usize) -> u128 {
        if w == 0 { self.low[k] } else { self.sets[k][w] }
    }

    #[inline(always)]
    fn flip(&mut self, k: usize, y: usize, member: bool) {
        let word = if self.layout.words() == 1 || y < MASK_ROWS {
            &mut self.low[k]
        } else {
            &mut self.sets[k][y / MASK_ROWS]
        };
        let bit = 1u128 << (y % MASK_ROWS);
        if member != (*word & bit != 0) {
            if member {
                *word |= bit;
                self.delta[k] += 1;
            } else {
                *word &= !bit;
                self.delta[k] -= 1;
            }
        }
    }
}

impl<L: Layout> Sets for ColumnSets<'_, L> {
    #[inline(always)]
    fn wet(&mut self, _: usize, y: usize, member: bool) {
        self.flip(0, y, member);
    }
    #[inline(always)]
    fn damp(&mut self, _: usize, y: usize, member: bool) {
        self.flip(1, y, member);
    }
    #[inline(always)]
    fn drainable(&mut self, _: usize, y: usize, member: bool) {
        self.flip(2, y, member);
    }
}

fn add3(a: &mut [isize; 3], b: [isize; 3]) {
    for k in 0..3 {
        a[k] += b[k];
    }
}

/// [`column::fall`] over every column: the wet rows at the start of the call, bottom-up.
/// Returns the change in the three sets' member counts (wet, damp, drainable).
pub fn fall<G: Fn(usize) -> Ground + Sync>(exec: Exec<'_>, w: Water<'_>, ground: &G) -> [isize; 3] {
    by_layout!(w.grid, fall_in(exec, w, ground))
}

fn fall_in<L: Layout, G: Fn(usize) -> Ground + Sync>(
    layout: L,
    exec: Exec<'_>,
    mut w: Water<'_>,
    ground: &G,
) -> [isize; 3] {
    let (sh, sets) = w.split(layout);
    let mut total = [0; 3];
    for d in exec.columns_with_sets(sh.plane, sh.layout.words(), sets, |cols, mut sets| {
        let mut delta = [0; 3];
        for (k, col) in cols.enumerate() {
            if sh.layout.column(sets[0], k).iter().all(|&m| m == 0) {
                continue;
            }
            let ((), d) = sh.column(words_at(&mut sets, k, sh.layout.words()), |c| {
                // Bottom-up transfers only change this word or an earlier one. Taking
                // each word just before visiting it preserves the phase's snapshot.
                for k in 0..sh.layout.words() {
                    let word = c.sets.word(0, k);
                    column::fall(c, col, Rows(word).map(|y| k * MASK_ROWS + y), ground);
                }
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
pub fn infiltrate<G: Fn(usize) -> Ground + Sync>(
    exec: Exec<'_>,
    w: Water<'_>,
    ground: &G,
) -> [isize; 3] {
    by_layout!(w.grid, infiltrate_in(exec, w, ground))
}

fn infiltrate_in<L: Layout, G: Fn(usize) -> Ground + Sync>(
    layout: L,
    exec: Exec<'_>,
    mut w: Water<'_>,
    ground: &G,
) -> [isize; 3] {
    let (sh, sets) = w.split(layout);
    let mut total = [0; 3];
    for d in exec.columns_with_sets(sh.plane, sh.layout.words(), sets, |cols, mut sets| {
        let mut delta = [0; 3];
        for (k, col) in cols.enumerate() {
            if sh.layout.column(sets[0], k).iter().all(|&m| m == 0) {
                continue;
            }
            let ((), d) = sh.column(words_at(&mut sets, k, sh.layout.words()), |c| {
                for k in 0..sh.layout.words() {
                    let word = c.sets.word(0, k);
                    column::infiltrate(c, col, Rows(word).map(|y| k * MASK_ROWS + y), ground);
                }
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
    w: Water<'_>,
    ground: &G,
    voxel_m: f64,
    table: f64,
) -> ([isize; 3], f64) {
    by_layout!(w.grid, drain_in(exec, w, ground, voxel_m, table))
}

fn drain_in<L: Layout, G: Fn(usize) -> Ground + Sync>(
    layout: L,
    exec: Exec<'_>,
    mut w: Water<'_>,
    ground: &G,
    voxel_m: f64,
    table: f64,
) -> ([isize; 3], f64) {
    let (sh, sets) = w.split(layout);
    let mut total = ([0; 3], 0.0);
    for (d, gained) in
        exec.columns_with_sets(sh.plane, sh.layout.words(), sets, |cols, mut sets| {
            let (mut delta, mut gained) = ([0; 3], 0.0);
            for (k, col) in cols.enumerate() {
                if sh.layout.column(sets[2], k).iter().all(|&m| m == 0) {
                    continue;
                }
                let (g, d) = sh.column(words_at(&mut sets, k, sh.layout.words()), |c| {
                    let mut gained = 0.0;
                    for k in 0..sh.layout.words() {
                        let word = c.sets.word(2, k);
                        gained += column::drain(
                            c,
                            col,
                            Rows(word).map(|y| k * MASK_ROWS + y),
                            ground,
                            voxel_m,
                            table,
                        );
                    }
                    gained
                });
                gained += g;
                add3(&mut delta, d);
            }
            (delta, gained)
        })
    {
        add3(&mut total.0, d);
        total.1 += gained;
    }
    total
}

/// The whole arrays' set masks, for a pass that walks cells in index order rather than
/// column by column (on the calling thread).
struct Masks<'s> {
    plane: usize,
    words: usize,
    sets: SetArrays<'s>,
    delta: [isize; 3],
}

impl Masks<'_> {
    #[inline(always)]
    fn flip(&mut self, k: usize, i: usize, y: usize, member: bool) {
        let col = i - y * self.plane;
        let word = &mut self.sets[k][col * self.words + y / MASK_ROWS];
        let count = &mut self.delta[k];
        let bit = 1u128 << (y % MASK_ROWS);
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
    w: Water<'_>,
    ground: &G,
    t: Table,
    charged: f64,
) -> ([isize; 3], f64) {
    by_layout!(w.grid, water_table_in(exec, w, ground, t, charged))
}

fn water_table_in<L: Layout, G: Fn(usize) -> Ground + Sync>(
    layout: L,
    exec: Exec<'_>,
    mut w: Water<'_>,
    ground: &G,
    t: Table,
    charged: f64,
) -> ([isize; 3], f64) {
    let (sh, mut sets) = w.split(layout);
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
    for (d, took) in exec.columns_with_sets(
        sh.plane,
        sh.layout.words(),
        [&mut **a, &mut **b, &mut **c],
        |cols, mut sets| {
            let (mut delta, mut took) = ([0; 3], 0.0);
            for (k, col) in cols.enumerate() {
                let (t1, d) = sh.column(words_at(&mut sets, k, sh.layout.words()), |c| {
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
        },
    ) {
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
    for (d, took) in exec.columns_with_sets(sh.plane, sh.layout.words(), sets, |cols, mut sets| {
        let (mut delta, mut took) = ([0; 3], 0.0);
        for (k, col) in cols.enumerate() {
            let (t2, d) = sh.column(words_at(&mut sets, k, sh.layout.words()), |c| {
                let mut took = 0.0;
                for y in 1..t.seep_top {
                    let (level, top_row) = (t.level(y), y + 1 == sh.height);
                    took += column::seep(
                        c,
                        y * sh.plane + col,
                        y,
                        level,
                        top_row,
                        ground,
                        f64::INFINITY,
                    );
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
fn table_in_order<L: Layout, G: Fn(usize) -> Ground>(
    sh: &Shared<'_, L>,
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
        words: sh.layout.words(),
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
            taken += column::seep(
                &mut c,
                y * sh.plane + col,
                y,
                level,
                top_row,
                ground,
                charged - taken,
            );
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
        ExchangeBackend, F64s, Field, Grid, HEAD_PASSES, Layout, MASK_ROWS, One, Params, Scratch,
        Stride, column_credits_words, column_drive_pass, column_edges, column_heads_words, commit,
        flip_wet, moved,
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

        /// Bytes of working buffer this driver holds (capacity); the pool is shared.
        pub fn scratch_bytes(&self) -> usize {
            self.sc.bytes()
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
            by_layout!(grid, parallel_exchange(self, grid, free, void, wet, p))
        }
    }

    fn parallel_exchange<L: Layout>(
        layout: L,
        this: &mut Parallel,
        grid: Grid,
        free: &mut [f64],
        void: &[u128],
        wet: &mut [u128],
        p: &Params,
    ) -> isize {
        let grain = this.grain;
        let sc = &mut this.sc;
        sc.ensure(grid);
        let plane = grid.plane();
        let words = layout.words();
        assert!(
            free.len() == grid.cells() && void.len() == plane * words && wet.len() == plane * words
        );
        assert!(
            u32::try_from(grid.cells()).is_ok(),
            "kept proposals index cells as u32"
        );
        let chunks = plane.div_ceil(grain);
        sc.edges.resize_with(chunks, Vec::new);
        let Scratch {
            full,
            touched,
            head,
            drive,
            proposed,
            delta,
            edges,
        } = sc;
        this.pool.install(|| {
            // A: heads, full masks, both drive buffers.
            let any_full = AtomicBool::new(false);
            {
                let wet: &[u128] = wet;
                let [d0, d1] = drive;
                let (h, d0, d1) = (F64s::new(head), F64s::new(d0), F64s::new(d1));
                let free: &[f64] = free;
                full.par_chunks_mut(words)
                    .enumerate()
                    .with_min_len(grain)
                    .for_each(|(col, fm)| {
                        let w = layout.column(wet, col);
                        if w.iter().all(|&m| m == 0) {
                            fm.fill(0);
                        } else {
                            column_heads_words(grid, layout, col, w, fm, free, |i, v| {
                                h.store(i, v);
                                d0.store(i, v);
                                d1.store(i, v);
                            });
                        }
                        if fm.iter().any(|&m| m != 0) {
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
                        .filter(|&col| layout.column(full, col).iter().any(|&m| m != 0))
                        .map(|col| {
                            column_drive_pass(grid, layout, col, wet, full, src, |i, v| {
                                dst.store(i, v)
                            })
                        })
                        .reduce(|| false, |a, b| a | b);
                    cur ^= 1;
                    if !raised {
                        break;
                    }
                }
            }
            let field = Field {
                grid,
                layout,
                free,
                void,
                wet: &*wet,
                full,
                head,
                drive: &drive[cur],
            };

            // B: keep proposals per chunk of giver columns, then sum them in giver
            // order, exactly as Serial does. Atomic floating-point adds depended on
            // worker arrival order: rounding changed the pre-roll's water and plant
            // establishment, so even identical seeds could found different worlds.
            {
                edges.par_iter_mut().enumerate().for_each(|(c, list)| {
                    list.clear();
                    for col in c * grain..((c + 1) * grain).min(plane) {
                        if layout.column(field.wet, col).iter().any(|&m| m != 0) {
                            column_edges(&field, p, col, |from, to_col, to_row, q| {
                                let t = to_row * plane + to_col;
                                list.push(((from * plane + col) as u32, t as u32, q));
                            });
                        }
                    }
                });
                for &(_, to, q) in edges.iter().flatten() {
                    let to = to as usize;
                    proposed[to] += q;
                    let y = to / plane;
                    touched[layout.index(to % plane, y)] |= 1u128 << (y % MASK_ROWS);
                }
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
                        let t = layout.column(touched, col);
                        if t.iter().any(|&m| m != 0) {
                            column_credits_words(&field, col, t, proposed, |i, v| {
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
                .par_chunks_mut(grain * words)
                .zip(wet.par_chunks_mut(grain * words))
                .enumerate()
                .map(|(c, (tch, wt))| {
                    let mut count = 0isize;
                    for (k, (t, word)) in tch.iter_mut().zip(wt.iter_mut()).enumerate() {
                        let col = c * grain + k / words;
                        let mut rows = *word | *t;
                        *t = 0;
                        while rows != 0 {
                            let bit = rows.trailing_zeros() as usize;
                            let y = (k % words) * MASK_ROWS + bit;
                            rows &= rows - 1;
                            let i = y * plane + col;
                            let dv = d.load(i);
                            d.store(i, 0.0);
                            pr.store(i, 0.0);
                            if dv != 0.0 {
                                let old = fr.load(i);
                                let new = commit(old, dv);
                                fr.store(i, new);
                                flip_wet(word, &mut count, bit, old, new);
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
