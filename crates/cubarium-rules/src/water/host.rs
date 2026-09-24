//! The host's drivers of the exchange: the same per-column rules in [`super`], run
//! serially ([`Serial`]) or across a rayon pool ([`Parallel`], feature `par`). Each is an
//! [`ExchangeBackend`], which is also what a GPU backend implements, so the world calls
//! one interface whoever runs the phases.

use super::{
    Field, Grid, HEAD_PASSES, Params, column_credits, column_drive_pass, column_edges, column_heads, commit, moved,
};

/// One substep's exchange over a world's water. `free` is the fills in the world's index
/// order; `void` and `wet` its per-column row masks (non-solid rows; rows with `free >
/// 0`), as they stand before the exchange. Every cell whose wetness changed is pushed to
/// `flips` as `(cell, now_wet)` so the caller can keep its wet set.
pub trait ExchangeBackend {
    fn exchange(
        &mut self,
        grid: Grid,
        free: &mut [f64],
        void: &[u128],
        wet: &[u128],
        p: &Params,
        flips: &mut Vec<(usize, bool)>,
    );
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
        wet: &[u128],
        p: &Params,
        flips: &mut Vec<(usize, bool)>,
    ) {
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
                        raised |= column_drive_pass(grid, col, wet, full, src, |i, h| dst[i] = h);
                    }
                }
                cur ^= 1;
                if !raised {
                    break;
                }
            }
        }
        let field = Field { grid, free, void, wet, full, head, drive: &drive[cur] };

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
        // D: commit.
        for col in 0..plane {
            let mut rows = wet[col] | touched[col];
            touched[col] = 0;
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
                    if (old > 0.0) != (new > 0.0) {
                        flips.push((i, new > 0.0));
                    }
                }
            }
        }
    }
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
        column_heads, commit, moved,
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
            wet: &[u128],
            p: &Params,
            flips: &mut Vec<(usize, bool)>,
        ) {
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
                let field = Field { grid, free, void, wet, full, head, drive: &drive[cur] };

                // B: the proposals, added atomically into whichever column they land in,
                // and kept per chunk of giver columns.
                {
                    let prop = as_atomic_u64(proposed);
                    let tw = as_atomic_u128_halves(touched);
                    edges.par_iter_mut().enumerate().for_each(|(c, list)| {
                        list.clear();
                        for col in c * grain..((c + 1) * grain).min(plane) {
                            if wet[col] != 0 {
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
                // D: commit, collecting the wet set's flips per chunk.
                let (fr, d, pr) = (
                    Cells(free.as_mut_ptr()),
                    Cells(delta.as_mut_ptr()),
                    Cells(proposed.as_mut_ptr()),
                );
                let chunks: Vec<Vec<(usize, bool)>> = touched
                    .par_chunks_mut(grain)
                    .enumerate()
                    .map(|(c, tch)| {
                        let mut out = Vec::new();
                        for (k, t) in tch.iter_mut().enumerate() {
                            let col = c * grain + k;
                            let mut rows = wet[col] | *t;
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
                                        if (old > 0.0) != (new > 0.0) {
                                            out.push((i, new > 0.0));
                                        }
                                    }
                                }
                            }
                        }
                        out
                    })
                    .collect();
                for c in chunks {
                    flips.extend(c);
                }
            });
        }
    }
}
