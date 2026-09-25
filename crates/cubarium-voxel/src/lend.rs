//! **The overlapped tick's water lend** (`cubarium-voxel-sim`'s `LaggedWorld`,
//! `design/handoffs/voxel-phase-overlap-2026-09-24.md`). Execution only.
//!
//! At the start of an overlapped tick the live world's free and pore arrays are swapped
//! into a read copy, which the plants and animals then read; the live world is left with
//! the copy's buffers, which hold the water as it stood one tick earlier, and its water leg
//! copies the new values back before it steps. Only cells that can differ need copying:
//!
//! - **free** water is non-zero only in wet cells, so a cell whose free water can differ
//!   was wet one tick ago or is wet now — the live world's wet row masks, then and now;
//! - **pore** water is only ever held by porous cells (a zero capacity takes and gives
//!   nothing), so the copy walks the terrain's porous runs, plane by plane — on the
//!   terrarium 29 % of the cells, where free water is 1-4 %.
//!
//! Anything the lend cannot vouch for — the first lend, a tick skipped (a chained tick in
//! between), a terrain change, a tall world whose sets are member lists — copies both
//! arrays whole.

use crate::World;

/// What [`World::restore_water_from`] keeps between overlapped ticks, owned beside the
/// read copy.
#[derive(Clone, Debug, Default)]
pub struct WaterLend {
    /// The live world's wet row masks at the last lend (the cells its stale buffer can
    /// hold free water in) and at this one.
    wet_then: Vec<u128>,
    wet_now: Vec<u128>,
    /// Per plane (`y`), the porous runs `(start, len)` within it, and the terrain version
    /// they were read at.
    porous: Vec<Vec<(u32, u32)>>,
    porous_version: Option<u64>,
    /// `(tick, terrain_version)` of the live world at the last lend.
    last: Option<(u64, u64)>,
    /// Whether the restore this lend set up may copy only what can have changed.
    partial: bool,
}

impl World {
    /// **The start of an overlapped tick**: make this world, a **read copy**, read exactly
    /// as `live` does through a [`crate::VoxelView`] — without copying the water. `live`'s
    /// free and pore arrays are **swapped** in, which costs nothing, and every other field
    /// a view reads is copied: the stores, the clock, the ledger, the named cells, and —
    /// when `terrain_version` says it moved — the terrain with its solid-column mask. That
    /// is the trust flora's sky cache already places in `terrain_version`.
    ///
    /// `live` is left holding this copy's **stale** water until
    /// [`World::restore_water_from`] copies it back with the same `lend`: the overlapped
    /// tick does that first thing on its water leg, so the copy runs beside the plants and
    /// animals rather than ahead of both. Nothing may read `live` in between. `live`'s
    /// active sets must be current (the tick's `begin` sees to it). The water phases'
    /// caches stay with `live`, where they still describe its water once it is restored;
    /// a read copy is for readers, and stepping or commanding one is a bug.
    pub fn take_readable_from(&mut self, live: &mut World, lend: &mut WaterLend) {
        let terrain = self.terrain_version != live.terrain_version
            || self.material.len() != live.material.len();
        self.config.clone_from(&live.config);
        self.aquifer_m3 = live.aquifer_m3;
        self.atmosphere_m3 = live.atmosphere_m3;
        self.shower_left_m3 = live.shower_left_m3;
        self.next_shower_tick = live.next_shower_tick;
        self.outlet_open = live.outlet_open;
        self.tick = live.tick;
        self.terrain_version = live.terrain_version;
        self.ledger.clone_from(&live.ledger);
        self.outlet_cell = live.outlet_cell;
        self.spring_cell = live.spring_cell;
        self.lake_drain.clone_from(&live.lake_drain);
        self.lake_datum_y = live.lake_datum_y;
        if terrain {
            self.material.clone_from(&live.material);
            self.solid.bits.clone_from(&live.solid.bits);
        }
        std::mem::swap(&mut self.free, &mut live.free);
        std::mem::swap(&mut self.pore, &mut live.pore);

        // The lend: partial only one tick after the last, on unchanged terrain, with row
        // masks this tick and last.
        let key = (live.tick, live.terrain_version);
        let follows = lend
            .last
            .is_some_and(|(t, v)| t + 1 == key.0 && v == key.1);
        std::mem::swap(&mut lend.wet_then, &mut lend.wet_now);
        lend.wet_now.clear();
        match live.wet.columns() {
            Some(cols) if !live.wet.needs_rebuild(live.config.cells()) => {
                lend.wet_now.extend_from_slice(cols);
                lend.partial = follows && lend.wet_then.len() == cols.len();
            }
            _ => lend.partial = false,
        }
        lend.last = Some(key);
    }

    /// The other half of [`World::take_readable_from`]: copy the water arrays back from
    /// the read copy `copy`, split by plane across `threads` workers of the water pool
    /// (the `parallel` feature; `1` copies on the caller) — only the cells `lend` says
    /// can differ when it can say so ([`WaterLend`]), else everything.
    pub fn restore_water_from(&mut self, copy: &World, lend: &mut WaterLend, threads: usize) {
        if self.free.len() != copy.free.len() {
            self.free = copy.free.clone();
            self.pore = copy.pore.clone();
            lend.partial = false;
            return;
        }
        let plane = self.config.width as usize * self.config.depth as usize;
        if !lend.partial || plane == 0 {
            crate::world::copy_dense(&mut self.free, &copy.free, threads);
            crate::world::copy_dense(&mut self.pore, &copy.pore, threads);
            return;
        }
        if lend.porous_version != Some(self.terrain_version) {
            lend.porous = porous_runs(self, plane);
            lend.porous_version = Some(self.terrain_version);
        }
        // Free water: the cells wet then or now.
        let rows: Vec<u128> = lend
            .wet_then
            .iter()
            .zip(&lend.wet_now)
            .map(|(a, b)| a | b)
            .collect();
        let any = rows.iter().fold(0u128, |a, r| a | r);
        let free = |y: usize, d: &mut [f64], s: &[f64]| {
            if (any >> y) & 1 == 0 {
                return;
            }
            for (c, r) in rows.iter().enumerate() {
                if (r >> y) & 1 != 0 {
                    d[c] = s[c];
                }
            }
        };
        let porous = &lend.porous;
        let pore = |y: usize, d: &mut [f64], s: &[f64]| {
            for &(start, len) in &porous[y] {
                let (a, b) = (start as usize, (start + len) as usize);
                d[a..b].copy_from_slice(&s[a..b]);
            }
        };
        by_plane(&mut self.free, &copy.free, plane, threads, free);
        by_plane(&mut self.pore, &copy.pore, plane, threads, pore);
    }
}

/// Every plane's porous runs, from `world`'s terrain.
fn porous_runs(world: &World, plane: usize) -> Vec<Vec<(u32, u32)>> {
    world
        .material
        .chunks(plane)
        .map(|row| {
            let mut runs = Vec::new();
            let mut c = 0;
            while c < row.len() {
                if row[c].pore_capacity() > 0.0 {
                    let start = c;
                    while c < row.len() && row[c].pore_capacity() > 0.0 {
                        c += 1;
                    }
                    runs.push((start as u32, (c - start) as u32));
                } else {
                    c += 1;
                }
            }
            runs
        })
        .collect()
}

/// `f(y, dst plane y, src plane y)` for every plane, split across the water pool.
fn by_plane(
    dst: &mut [f64],
    src: &[f64],
    plane: usize,
    threads: usize,
    f: impl Fn(usize, &mut [f64], &[f64]) + Sync,
) {
    #[cfg(feature = "parallel")]
    if threads > 1 {
        use rayon::prelude::*;
        let pool = cubarium_rules::water::host::pool(threads);
        pool.install(|| {
            dst.par_chunks_mut(plane)
                .zip(src.par_chunks(plane))
                .enumerate()
                .for_each(|(y, (d, s))| f(y, d, s));
        });
        return;
    }
    let _ = threads;
    for (y, (d, s)) in dst.chunks_mut(plane).zip(src.chunks(plane)).enumerate() {
        f(y, d, s);
    }
}
