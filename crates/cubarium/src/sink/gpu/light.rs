//! The lit tier's two planes, built on the CPU for `voxel.frag` (package L,
//! `design/handoffs/presentation-plan-2026-09-24.md`).
//!
//! **The ecology's light model is the source of truth.** The picture may shade more
//! finely than the model's sky fan, never differently, so neither plane is a second
//! light model:
//!
//! * the **sky plane** is the core's own [`VoxelView::sky_visibility`], called unchanged,
//!   at every open cell. Open cell `(x, y, z)` stores `sky_visibility(x, y − 1, z)`, the
//!   fan from the foot of the open cell: a top face (whose open cell is the one above it)
//!   reads exactly the model's number, and a wall reads the fan from the foot of the cell
//!   in front of it. It is a function of the terrain alone, so it is keyed on the terrain
//!   like the roof table and computed on a thread of its own ([`SkyWorker`]);
//! * the **canopy plane** is the flora model's foliage layers, attenuating straight down as
//!   the shade model applies them (`crates/cubarium-voxel-flora/src/step.rs`,
//!   `shade_layers_into`): each layer is a disc of `radius_v` cells about its stand's
//!   column at its own drawn cell, and lets `exp(−k · (1 − porosity) · stock / area)`
//!   through to anything under it. Rebuilt every pack ([`Canopy`]).
//!
//! And one that is not the model's, because the model has no emitters: the **glow
//! volume** ([`Glow`]), the local light of the parts the dossiers name as luminous. It is
//! built at a quarter of the world's resolution from the frame's emitter list, spread a
//! few cells through open terrain and stopped by solid terrain, and added to the ambient
//! before the shader snaps it to the ladder.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

use cubarium_gpu::voxel::{CANOPY_STEPS, GLOW_CELL};
use cubarium_voxel::{Config, Ledger, Material, VoxelView};
use cubarium_voxel_flora::FloraView;

#[inline]
fn quantise(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The sky plane of `material` into `out`, in the voxel texture's order
/// (`(z · height + y) · width + x`), over `threads` threads.
///
/// Every air cell gets its value; a solid cell is never an open cell and gets 0. A cell on
/// the floor row has no cell under it and takes its own top face's fan. A cell whose foot
/// is at or above the highest solid anywhere is open sky without asking: every ray in the
/// fan climbs, so none of them can meet a solid, and the answer is exactly 1.
#[cfg(test)]
pub(crate) fn sky_plane(config: &Config, material: &[Material], threads: usize, out: &mut [u8]) {
    sky_plane_until(config, material, threads, out, &AtomicBool::new(false));
}

/// The sky plane above, given up as soon as `cancel` is set: every thread looks at it before each
/// cell, so it stops within one cell's fan of being asked. Returns whether the plane is
/// whole (`false`: cancelled, and `out` is partly written).
pub(crate) fn sky_plane_until(
    config: &Config,
    material: &[Material],
    threads: usize,
    out: &mut [u8],
    cancel: &AtomicBool,
) -> bool {
    let (w, h, d) = (
        config.width as usize,
        config.height as usize,
        config.depth as usize,
    );
    assert_eq!(material.len(), w * h * d, "one material per voxel");
    assert_eq!(out.len(), w * h * d, "one sky byte per voxel");
    let ledger = Ledger::default();
    let view = VoxelView::of_terrain(config, material, &ledger);
    let top = material
        .iter()
        .enumerate()
        .filter(|(_, m)| m.is_solid())
        .map(|(i, _)| config.coords(i).1)
        .max();
    // Rows `y0..` of slab `z`, into `rows` (whole rows of that slab).
    let band = |z: u32, y0: usize, rows: &mut [u8]| -> bool {
        for (r, row) in rows.chunks_mut(w).enumerate() {
            let y = (y0 + r) as u32;
            for (x, cell) in row.iter_mut().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    return false;
                }
                if view.material_at(x as i64, y, z).is_solid() {
                    *cell = 0;
                    continue;
                }
                let foot = y.saturating_sub(1);
                *cell = if top.is_none_or(|t| foot >= t) {
                    255
                } else {
                    quantise(view.sky_visibility(x as i64, foot, z))
                };
            }
        }
        true
    };
    // The work is cut into bands of a few rows of one slab, and each thread takes the next
    // band left until none are: a slab's rows cost very differently (open sky above the
    // highest solid costs nothing), and whole slabs dealt out in turn left most threads
    // idle while the few with one slab more finished. The plane is the same either way.
    let threads = threads.max(1);
    let rows_per_band = SKY_BAND_ROWS.min(h.max(1));
    let mut bands: Vec<(u32, usize, &mut [u8])> = Vec::new();
    for (z, slab) in out.chunks_mut(w * h).enumerate() {
        for (b, rows) in slab.chunks_mut(w * rows_per_band).enumerate() {
            bands.push((z as u32, b * rows_per_band, rows));
        }
    }
    // Reversed, so `pop` hands out the front slabs first.
    bands.reverse();
    let threads = threads.min(bands.len().max(1));
    let queue = Mutex::new(bands);
    std::thread::scope(|s| {
        for _ in 0..threads {
            let (band, queue) = (&band, &queue);
            s.spawn(move || {
                loop {
                    let next = queue.lock().unwrap_or_else(|e| e.into_inner()).pop();
                    let Some((z, y0, rows)) = next else { return };
                    if !band(z, y0, rows) {
                        return;
                    }
                }
            });
        }
    });
    !cancel.load(Ordering::Relaxed)
}

/// Rows in one band of the sky plane's work ([`sky_plane_until`]).
const SKY_BAND_ROWS: usize = 8;

/// Sky planes already computed, kept for a terrain that comes back: a candidate's thumbnail,
/// then the same candidate opened large, then the world founded on it, each open a renderer
/// of their own over the same terrain. The plane is a function of the world's shape and which
/// voxels are solid alone ([`sky_plane_until`]), so that is the key, compared exactly. The
/// newest few are kept ([`PLANE_CACHE_BYTES`]).
static PLANES: Mutex<VecDeque<KnownPlane>> = Mutex::new(VecDeque::new());
/// At most this many bytes of planes are kept (a wide world's plane is about 8 MB).
const PLANE_CACHE_BYTES: usize = 64 << 20;

struct KnownPlane {
    dims: (u32, u32, u32),
    solid: Vec<u64>,
    plane: Arc<Vec<u8>>,
}

/// What a sky plane depends on: one bit per voxel, solid or not.
fn solid_bits(material: &[Material]) -> Vec<u64> {
    let mut bits = vec![0u64; material.len().div_ceil(64)];
    for (i, m) in material.iter().enumerate() {
        if m.is_solid() {
            bits[i / 64] |= 1 << (i % 64);
        }
    }
    bits
}

fn dims_of(c: &Config) -> (u32, u32, u32) {
    (c.width, c.height, c.depth)
}

/// The sky plane of a terrain computed before, if it is still kept.
fn known_plane(dims: (u32, u32, u32), solid: &[u64]) -> Option<Arc<Vec<u8>>> {
    let mut known = PLANES.lock().unwrap_or_else(|e| e.into_inner());
    let at = known.iter().position(|k| k.dims == dims && k.solid == solid)?;
    // The newest at the back.
    let k = known.remove(at)?;
    let plane = k.plane.clone();
    known.push_back(k);
    Some(plane)
}

/// Keep a whole sky plane for its terrain, dropping the oldest past [`PLANE_CACHE_BYTES`].
fn keep_plane(dims: (u32, u32, u32), solid: Vec<u64>, plane: Arc<Vec<u8>>) {
    let mut known = PLANES.lock().unwrap_or_else(|e| e.into_inner());
    known.retain(|k| !(k.dims == dims && k.solid == solid));
    known.push_back(KnownPlane { dims, solid, plane });
    let bytes = |k: &KnownPlane| k.plane.len() + 8 * k.solid.len();
    while known.len() > 1 && known.iter().map(bytes).sum::<usize>() > PLANE_CACHE_BYTES {
        known.pop_front();
    }
}

/// The sky plane, computed off the loop thread.
///
/// The packer submits the terrain whenever it moves and polls for the plane each pack; a
/// newer submission supersedes an older one still waiting, and a finished plane for a
/// terrain that has since moved is thrown away by its id.
///
/// Dropping the worker cancels the plane it is computing ([`sky_plane_until`]) and joins
/// its thread, so none of its threads outlives it: the Godot extension's library is
/// unloaded at exit, and a thread still running its code then faults.
pub(crate) struct SkyWorker {
    /// `None` once the worker is dropping: the thread's `recv` then fails and it ends.
    jobs: Option<Sender<(u64, Config, Vec<Material>)>>,
    done: Receiver<(u64, Vec<u8>)>,
    /// A plane [`SkyWorker::wait`] saw finish, kept for the next poll.
    landed: Option<(u64, Vec<u8>)>,
    /// Set on drop: the plane being computed is given up.
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SkyWorker {
    pub(crate) fn spawn() -> SkyWorker {
        // Every core the process may use but one, which the loop keeps: this runs once
        // at start-up and again only when the terrain is edited.
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .saturating_sub(1)
            .max(1);
        SkyWorker::with_threads(threads)
    }

    fn with_threads(threads: usize) -> SkyWorker {
        let (jobs, inbox) = mpsc::channel::<(u64, Config, Vec<Material>)>();
        let (outbox, done) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let thread = std::thread::Builder::new()
            .name("cubarium-sky".into())
            .spawn(move || {
                while let Ok(mut job) = inbox.recv() {
                    while let Ok(newer) = inbox.try_recv() {
                        job = newer;
                    }
                    let (id, config, material) = job;
                    let (dims, solid) = (dims_of(&config), solid_bits(&material));
                    let plane = match known_plane(dims, &solid) {
                        Some(known) => known.as_ref().clone(),
                        None => {
                            let mut plane = vec![0u8; material.len()];
                            if !sky_plane_until(&config, &material, threads, &mut plane, &stop) {
                                break;
                            }
                            keep_plane(dims, solid, Arc::new(plane.clone()));
                            plane
                        }
                    };
                    if outbox.send((id, plane)).is_err() {
                        break;
                    }
                }
            })
            .expect("spawning the sky plane's thread");
        SkyWorker {
            jobs: Some(jobs),
            done,
            landed: None,
            cancel,
            thread: Some(thread),
        }
    }

    /// Compute the sky plane of this terrain, under `id`.
    pub(crate) fn submit(&self, id: u64, config: &Config, material: &[Material]) {
        // A worker that has gone away leaves the plane as it was; nothing else breaks.
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send((id, config.clone(), material.to_vec()));
        }
    }

    /// The newest plane finished since the last poll, with its id.
    pub(crate) fn poll(&mut self) -> Option<(u64, Vec<u8>)> {
        let mut newest = self.landed.take();
        while let Ok(done) = self.done.try_recv() {
            newest = Some(done);
        }
        newest
    }

    /// Wait up to `timeout` for a plane to finish, without taking it: whether one has
    /// since the last poll (the next [`SkyWorker::poll`] returns it). A still world's stager
    /// waits here instead of packing the world again and again to poll.
    pub(crate) fn wait(&mut self, timeout: Duration) -> bool {
        if self.landed.is_some() {
            return true;
        }
        match self.done.recv_timeout(timeout) {
            Ok(done) => {
                self.landed = Some(done);
                true
            }
            Err(RecvTimeoutError::Timeout) => false,
            // The thread has ended (its drop is under way): nothing will come, but a caller
            // waiting in a loop still waits its `timeout`, never spins.
            Err(RecvTimeoutError::Disconnected) => {
                std::thread::park_timeout(timeout);
                false
            }
        }
    }
}

impl Drop for SkyWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.jobs = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// A fraction in `0..=1` as a nibble.
#[inline]
fn nibble(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 15.0).round() as u8
}

/// The step each foliage layer's transmission is rounded to before the canopy uses it:
/// 1/1024, four times finer than the plane's 8-bit step, in the full pack and the
/// change-only pack alike. A layer whose stock creeps between two steps then leaves the
/// canopy as it was, so a tick whose rounded transmissions did not move recomputes no
/// column. Its visible effect is at most a one-step shift of a plane byte or a nibble at
/// the moment it would round anyway.
pub(crate) const TRANSMISSION_STEP: f64 = 1.0 / 1024.0;

/// One foliage layer's disc as [`Canopy`] keeps it: the stand's column, the disc's reach
/// and radius squared, its drawn cell, and what it lets through.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Disc {
    x: i64,
    z: i64,
    reach: i64,
    r2: f64,
    cell: i64,
    t: f32,
}

/// One foliage step over a column: the drawn cell of the layer's disc and what it lets
/// through.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Step {
    cell: u8,
    t: f32,
}

/// The canopy plane: per column, the foliage layers over it as up to [`CANOPY_STEPS`]
/// steps, highest first, each carrying the product of its own transmission and every one
/// above it.
///
/// A face of a voxel at height `y` is under a layer when the layer's disc is drawn in a
/// cell above `y`: the ground and the trunk under a crown, and never the crown's own cells
/// (the shader's `canopyAt`). A column under more than [`CANOPY_STEPS`] distinct layers
/// merges its two lowest into the lower one's cell, which keeps the product — and so the
/// ground under all of them — exact and lets only a face between those two layers see
/// through the upper one.
///
/// Built whole every tick ([`Canopy::build`]) or only where a stand's discs moved
/// ([`Canopy::update`]): a column is recomputed from every disc over it, in stand order,
/// so the two agree to the bit.
pub(crate) struct Canopy {
    width: u32,
    depth: u32,
    steps: Vec<[Step; CANOPY_STEPS + 1]>,
    count: Vec<u8>,
    /// Every stand's foliage discs, by slot, and each stand's slot by its index in
    /// `FloraView::stands`: what [`Canopy::plant_byte`] divides a stand's own shade out
    /// with. The full build's slot is the index.
    discs: Vec<Vec<Disc>>,
    slot_of: Vec<u32>,
    /// Crown cells drawn in each column, `z · width + x` ([`Canopy::count_crowns`]), and
    /// the pass nibble they share, worked out once per column that has any.
    crowns: Vec<u16>,
    pass: Vec<u8>,
    /// The change-only update's state.
    inc: CanopyInc,
}

/// What [`Canopy::update`] keeps between ticks.
#[derive(Default)]
struct CanopyInc {
    by_id: rustc_hash::FxHashMap<u64, u32>,
    seen: Vec<u64>,
    k_of: Vec<u32>,
    free: Vec<u32>,
    tick: u64,
    /// Per column, the slots whose discs reach it; per slot, the columns its discs reach.
    cover: Vec<Vec<u32>>,
    foot: Vec<Vec<u32>>,
    dirty: Vec<u32>,
    marked: Vec<bool>,
    scratch: Vec<Disc>,
    started: bool,
}

impl Canopy {
    pub(crate) fn new(width: u32, depth: u32) -> Canopy {
        let n = width as usize * depth as usize;
        Canopy {
            width,
            depth,
            steps: vec![[Step::default(); CANOPY_STEPS + 1]; n],
            count: vec![0; n],
            discs: Vec::new(),
            slot_of: Vec::new(),
            crowns: vec![0; n],
            pass: vec![0; n],
            inc: CanopyInc::default(),
        }
    }

    /// One stand's foliage discs, each layer's transmission rounded to
    /// [`TRANSMISSION_STEP`].
    fn stand_discs(flora: FloraView<'_>, stand: &cubarium_voxel_flora::Stand, out: &mut Vec<Disc>) {
        out.clear();
        let k = flora.config.shade_k_per_m2;
        for layer in flora.layers(stand) {
            let t = (-k * (1.0 - layer.porosity) * layer.stock / layer.area_m2).exp();
            let t = (t / TRANSMISSION_STEP).round() * TRANSMISSION_STEP;
            if t.is_nan() || t >= 1.0 || layer.cell <= 0 {
                continue;
            }
            let r = layer.radius_v.max(0.0);
            out.push(Disc {
                x: i64::from(stand.site.x),
                z: i64::from(stand.site.z),
                reach: r.floor() as i64,
                // Only whole squared offsets are ever compared with it, so its floor
                // reaches exactly the cells `r²` does, and moves only when they do.
                r2: (r * r).floor(),
                cell: i64::from(layer.cell.min(255) as u8),
                t: t as f32,
            });
        }
    }

    /// Every column a disc reaches, with each `(dx, dz)` that lands on it, in the full
    /// build's order.
    fn each_column(width: u32, depth: u32, disc: &Disc, mut f: impl FnMut(usize)) {
        let (w, d) = (i64::from(width), i64::from(depth));
        for dz in -disc.reach..=disc.reach {
            let z = disc.z + dz;
            if z < 0 || z >= d {
                continue;
            }
            for dx in -disc.reach..=disc.reach {
                if ((dx * dx + dz * dz) as f64) > disc.r2 {
                    continue;
                }
                let x = (disc.x + dx).rem_euclid(w);
                f((z * w + x) as usize);
            }
        }
    }

    /// How many of a disc's `(dx, dz)` land on column `col` ([`Canopy::each_column`]'s
    /// count of it): one, or more only for a disc wider than the ring.
    fn hits(width: u32, disc: &Disc, col: usize) -> usize {
        let w = i64::from(width);
        let (x, z) = ((col as i64) % w, (col as i64) / w);
        let dz = z - disc.z;
        if dz.abs() > disc.reach {
            return 0;
        }
        // The smallest dx ≥ −reach with disc.x + dx ≡ x (mod w), then every w on.
        let mut dx = -disc.reach + (x - disc.x + disc.reach).rem_euclid(w);
        let mut n = 0;
        while dx <= disc.reach {
            if ((dx * dx + dz * dz) as f64) <= disc.r2 {
                n += 1;
            }
            dx += w;
        }
        n
    }

    /// Rebuild from `flora`'s foliage layers and write the plane into `out`
    /// ([`cubarium_gpu::voxel::VoxelStaging::canopy`]'s layout).
    pub(crate) fn build(&mut self, flora: FloraView<'_>, out: &mut [u8]) {
        self.count.fill(0);
        self.slot_of.clear();
        self.discs.resize_with(flora.stands.len().max(self.discs.len()), Vec::new);
        let mut scratch = std::mem::take(&mut self.inc.scratch);
        for (k, stand) in flora.stands.iter().enumerate() {
            Canopy::stand_discs(flora, stand, &mut scratch);
            self.slot_of.push(k as u32);
            let (w, d) = (self.width, self.depth);
            for disc in &scratch {
                let (cell, t) = (disc.cell as u8, disc.t);
                Canopy::each_column(w, d, disc, |col| self.add(col, cell, t));
            }
            self.discs[k].clone_from(&scratch);
        }
        self.inc.scratch = scratch;
        self.write(out);
    }

    /// The change-only build: each stand's rounded discs against last tick's, and every
    /// column a moved disc reaches (before or after) recomputed and written into `out`.
    /// Returns those columns, `z · width + x`. The first call builds every column.
    pub(crate) fn update(&mut self, flora: FloraView<'_>, out: &mut [u8]) -> &[u32] {
        let n = self.count.len();
        let mut inc = std::mem::take(&mut self.inc);
        inc.dirty.clear();
        if !inc.started {
            inc = CanopyInc {
                cover: vec![Vec::new(); n],
                marked: vec![false; n],
                started: true,
                ..CanopyInc::default()
            };
            self.discs.clear();
            self.count.fill(0);
            for col in 0..n {
                inc.marked[col] = true;
                inc.dirty.push(col as u32);
            }
        }
        inc.tick += 1;
        self.slot_of.clear();
        let mut scratch = std::mem::take(&mut inc.scratch);
        for (k, stand) in flora.stands.iter().enumerate() {
            let slot = match inc.by_id.get(&stand.id) {
                Some(&s) => s,
                None => {
                    let s = inc.free.pop().unwrap_or_else(|| {
                        self.discs.push(Vec::new());
                        inc.seen.push(0);
                        inc.k_of.push(0);
                        inc.foot.push(Vec::new());
                        (self.discs.len() - 1) as u32
                    });
                    inc.by_id.insert(stand.id, s);
                    s
                }
            };
            let si = slot as usize;
            inc.seen[si] = inc.tick;
            inc.k_of[si] = k as u32;
            self.slot_of.push(slot);
            Canopy::stand_discs(flora, stand, &mut scratch);
            if scratch != self.discs[si] {
                self.move_discs(&mut inc, slot, &scratch);
            }
        }
        inc.scratch = scratch;
        let gone: Vec<(u64, u32)> = inc
            .by_id
            .iter()
            .filter(|(_, s)| inc.seen[**s as usize] != inc.tick)
            .map(|(&id, &s)| (id, s))
            .collect();
        for (id, slot) in gone {
            inc.by_id.remove(&id);
            self.move_discs(&mut inc, slot, &[]);
            inc.free.push(slot);
        }
        // Each moved column from every disc over it, in stand order.
        let mut slots: Vec<u32> = Vec::new();
        for &col in &inc.dirty {
            let col = col as usize;
            inc.marked[col] = false;
            self.count[col] = 0;
            slots.clear();
            slots.extend_from_slice(&inc.cover[col]);
            slots.sort_unstable_by_key(|&s| inc.k_of[s as usize]);
            for &slot in &slots {
                for i in 0..self.discs[slot as usize].len() {
                    let disc = self.discs[slot as usize][i];
                    let (cell, t) = (disc.cell as u8, disc.t);
                    for _ in 0..Canopy::hits(self.width, &disc, col) {
                        self.add(col, cell, t);
                    }
                }
            }
            self.write_column(col, out);
        }
        self.inc = inc;
        &self.inc.dirty
    }

    /// Give `slot` the discs `new`: its old and new columns are dirty.
    fn move_discs(&mut self, inc: &mut CanopyInc, slot: u32, new: &[Disc]) {
        let si = slot as usize;
        let old_foot = std::mem::take(&mut inc.foot[si]);
        for &col in &old_foot {
            inc.cover[col as usize].retain(|&s| s != slot);
            if !inc.marked[col as usize] {
                inc.marked[col as usize] = true;
                inc.dirty.push(col);
            }
        }
        let mut foot: Vec<u32> = Vec::new();
        for disc in new {
            Canopy::each_column(self.width, self.depth, disc, |col| foot.push(col as u32));
        }
        foot.sort_unstable();
        foot.dedup();
        for &col in &foot {
            inc.cover[col as usize].push(slot);
            if !inc.marked[col as usize] {
                inc.marked[col as usize] = true;
                inc.dirty.push(col);
            }
        }
        inc.foot[si] = foot;
        self.discs[si].clear();
        self.discs[si].extend_from_slice(new);
    }

    /// Recount the crown cells of these columns from `crowns_in` (column `(x, z)`'s crown
    /// cells) and work their pass nibbles out again: the change-only
    /// [`Canopy::count_crowns`].
    pub(crate) fn recount(&mut self, cols: impl Iterator<Item = u32>, crowns_in: impl Fn(u32, u32) -> u16) {
        let w = self.width as usize;
        for col in cols {
            let col = col as usize;
            let (x, z) = ((col % w) as u32, (col / w) as u32);
            let n = crowns_in(x, z);
            self.crowns[col] = n;
            if n > 0 {
                self.pass[col] = nibble(self.through(x, z, -1).powf(1.0 / f32::from(n)));
            }
        }
    }

    /// Count the crown cells drawn in each column, for [`Canopy::plant_byte`]'s pass
    /// chance.
    pub(crate) fn count_crowns(&mut self, crowns: impl Iterator<Item = (u32, u32)>) {
        self.crowns.fill(0);
        let w = self.width as usize;
        for (x, z) in crowns {
            let c = &mut self.crowns[z as usize * w + x as usize];
            *c = c.saturating_add(1);
        }
        for col in 0..self.crowns.len() {
            let n = self.crowns[col];
            if n == 0 {
                continue;
            }
            let (x, z) = ((col % w) as u32, (col / w) as u32);
            self.pass[col] = nibble(self.through(x, z, -1).powf(1.0 / f32::from(n)));
        }
    }

    /// What column `(x, z)`'s foliage lets through to a face of a voxel at height `y`:
    /// every layer whose disc is drawn above `y`, as the plane holds it (a `y` below 0 is
    /// the whole column).
    fn through(&self, x: u32, z: u32, y: i64) -> f32 {
        let col = z as usize * self.width as usize + x as usize;
        self.steps[col][..self.count[col] as usize]
            .iter()
            .take_while(|s| i64::from(s.cell) > y)
            .map(|s| s.t)
            .product()
    }

    /// What stand `stand`'s own layers let through to a face of a voxel at `y` in column
    /// `(x, z)`.
    fn own(&self, stand: usize, x: u32, z: u32, y: i64) -> f32 {
        let Some(discs) = self.slot_of.get(stand).and_then(|&s| self.discs.get(s as usize)) else {
            return 1.0;
        };
        let w = i64::from(self.width);
        let mut t = 1.0;
        for disc in discs {
            let mut dx = (i64::from(x) - disc.x).rem_euclid(w);
            if dx > w / 2 {
                dx -= w;
            }
            let dz = i64::from(z) - disc.z;
            if disc.cell > y && ((dx * dx + dz * dz) as f64) <= disc.r2 {
                t *= disc.t;
            }
        }
        t
    }

    /// The lit tier's byte for a plant cell at `(x, y, z)` (the voxel texel's `b`, which a
    /// plant cell's air never uses): in the low nibble, the canopy over the cell's faces
    /// without its own stand's layers (`owner`), since the model never lets a stand shade
    /// itself; in the high nibble, for a crown cell, the chance a shadow ray gets through
    /// it: the column's whole transmission spread evenly over the crown cells drawn in the
    /// column, so a ray straight down through all of them keeps what the model lets
    /// through.
    pub(crate) fn plant_byte(&self, owner: Option<usize>, crown: bool, x: u32, y: u32, z: u32) -> u8 {
        let y = i64::from(y);
        let all = self.through(x, z, y);
        let canopy = match owner {
            Some(stand) => {
                let own = self.own(stand, x, z, y);
                if own > 0.0 { (all / own).min(1.0) } else { 1.0 }
            }
            None => all,
        };
        let col = z as usize * self.width as usize + x as usize;
        let pass = if crown && self.crowns[col] > 0 {
            self.pass[col]
        } else if crown {
            15
        } else {
            0
        };
        pass << 4 | nibble(canopy)
    }

    fn add(&mut self, col: usize, cell: u8, t: f32) {
        let steps = &mut self.steps[col];
        let n = self.count[col] as usize;
        if let Some(s) = steps[..n].iter_mut().find(|s| s.cell == cell) {
            s.t *= t;
            return;
        }
        // Insert keeping the cells descending.
        let at = steps[..n].iter().position(|s| s.cell < cell).unwrap_or(n);
        steps.copy_within(at..n, at + 1);
        steps[at] = Step { cell, t };
        if n + 1 > CANOPY_STEPS {
            let low = steps[CANOPY_STEPS];
            steps[CANOPY_STEPS - 1].t *= low.t;
            steps[CANOPY_STEPS - 1].cell = low.cell;
            self.count[col] = CANOPY_STEPS as u8;
        } else {
            self.count[col] = (n + 1) as u8;
        }
    }

    fn write(&self, out: &mut [u8]) {
        for col in 0..self.steps.len() {
            self.write_column(col, out);
        }
    }

    fn write_column(&self, col: usize, out: &mut [u8]) {
        let w = self.width as usize;
        let per_row = CANOPY_STEPS / 2;
        let steps = &self.steps[col];
        let (x, z) = (col % w, col / w);
        let n = self.count[col] as usize;
        let mut through = 1.0f32;
        for (k, step) in steps[..CANOPY_STEPS].iter().enumerate() {
            let at = ((z * per_row + k / 2) * w + x) * 4 + (k % 2) * 2;
            if k < n {
                through *= step.t;
                out[at] = step.cell;
                out[at + 1] = quantise(f64::from(through));
            } else {
                out[at] = 0;
                out[at + 1] = 0;
            }
        }
    }

    /// The transmission the shader's `canopyAt` reads for a face of a voxel at height
    /// `y` in column `(x, z)`, from a written plane.
    #[cfg(test)]
    pub(crate) fn read(plane: &[u8], width: u32, x: u32, z: u32, y: u32) -> f32 {
        let (w, per_row) = (width as usize, CANOPY_STEPS / 2);
        let mut t = 1.0;
        for k in 0..CANOPY_STEPS {
            let at = ((z as usize * per_row + k / 2) * w + x as usize) * 4 + (k % 2) * 2;
            if u32::from(plane[at]) <= y {
                break;
            }
            t = f32::from(plane[at + 1]) / 255.0;
        }
        t
    }
}

/// The glow volume: the emitters' light on a grid of [`GLOW_CELL`]³ voxels.
///
/// Each emitting voxel adds its emissive colour times `gain` to its glow cell. From every
/// cell that has a source the light spreads cell to cell over the six faces, at most
/// `reach` steps, falling off linearly to nothing one step past `reach`, and sources add.
/// **Terrain stops it:** a cell with no air receives nothing, and a cell less than half air
/// receives but passes nothing on, so a cell behind a wall gets light only by a way round
/// it within the reach. The strip wraps in x as the world does.
///
/// A function of the terrain and of the emitter list alone: [`Glow::build`] recomputes only
/// when either moved, and bumps [`Glow::key`], the key the plane is uploaded under.
pub(crate) struct Glow {
    dims: (u32, u32, u32),
    /// Per glow cell: its voxels inside the world, and how many of them are air.
    total: Vec<u16>,
    air: Vec<u16>,
    /// Bumped when the terrain changes.
    terrain: u64,
    /// The terrain and sources the volume was last built from.
    built: Option<u64>,
    /// The volume as the plane holds it: RGBA8, alpha unused.
    pub(crate) bytes: Vec<u8>,
    pub(crate) key: u64,
    light: Vec<[f32; 3]>,
    sources: Vec<(u32, [f32; 3])>,
    /// The spread's scratch: the source a cell was last reached from, plus one.
    seen: Vec<u32>,
    queue: VecDeque<(u32, u8)>,
}

impl Glow {
    pub(crate) fn new(width: u32, height: u32, depth: u32) -> Glow {
        let dims = (
            width.div_ceil(GLOW_CELL),
            height.div_ceil(GLOW_CELL),
            depth.div_ceil(GLOW_CELL),
        );
        let n = dims.0 as usize * dims.1 as usize * dims.2 as usize;
        Glow {
            dims,
            total: vec![0; n],
            air: vec![0; n],
            terrain: 0,
            built: None,
            bytes: vec![0; n * 4],
            key: 0,
            light: vec![[0.0; 3]; n],
            sources: Vec::new(),
            seen: vec![0; n],
            queue: VecDeque::new(),
        }
    }

    /// The glow cell of voxel `(x, y, z)`, in the plane's order.
    #[inline]
    fn cell_of(&self, x: u32, y: u32, z: u32) -> u32 {
        let (cx, cy, cz) = (x / GLOW_CELL, y / GLOW_CELL, z / GLOW_CELL);
        (cz * self.dims.1 + cy) * self.dims.0 + cx
    }

    /// Count each cell's air from `material` (the world's `(y · depth + z) · width + x`
    /// order). Called when the terrain moves.
    pub(crate) fn set_terrain(&mut self, config: &Config, material: &[Material]) {
        let (w, h, d) = (config.width, config.height, config.depth);
        self.total.fill(0);
        self.air.fill(0);
        for y in 0..h {
            for z in 0..d {
                let row = ((y * d + z) * w) as usize;
                for x in 0..w {
                    let c = self.cell_of(x, y, z) as usize;
                    self.total[c] += 1;
                    if !material[row + x as usize].is_solid() {
                        self.air[c] += 1;
                    }
                }
            }
        }
        self.terrain += 1;
    }

    /// Rebuild from this frame's emitters, each a voxel and its emissive colour in linear
    /// light, if they or the terrain moved since the last build. `gain` is one emitting
    /// voxel's light at its own cell, in units of full ambient light; `reach` the steps
    /// it spreads. Returns whether the volume changed.
    pub(crate) fn build(
        &mut self,
        emitters: impl Iterator<Item = ((u32, u32, u32), [f32; 3])>,
        gain: f32,
        reach: u32,
    ) -> bool {
        self.sources.clear();
        for ((x, y, z), rgb) in emitters {
            let c = self.cell_of(x, y, z);
            self.sources.push((c, rgb.map(|k| k * gain)));
        }
        self.sources.sort_unstable_by_key(|s| s.0);
        self.sources.dedup_by(|b, a| {
            if a.0 == b.0 {
                for k in 0..3 {
                    a.1[k] += b.1[k];
                }
                true
            } else {
                false
            }
        });
        let mut h = rustc_hash::FxHasher::default();
        self.terrain.hash(&mut h);
        reach.hash(&mut h);
        gain.to_bits().hash(&mut h);
        for (c, rgb) in &self.sources {
            c.hash(&mut h);
            rgb.map(|k| (k * 4096.0) as u32).hash(&mut h);
        }
        let id = h.finish();
        if self.built == Some(id) {
            return false;
        }
        self.built = Some(id);
        self.spread(reach);
        for (px, l) in self.bytes.chunks_exact_mut(4).zip(&self.light) {
            for k in 0..3 {
                px[k] = (l[k].clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            px[3] = 255;
        }
        self.key += 1;
        true
    }

    fn spread(&mut self, reach: u32) {
        self.light.fill([0.0; 3]);
        self.seen.fill(0);
        let (dw, dh, dd) = (self.dims.0 as i64, self.dims.1 as i64, self.dims.2 as i64);
        let reach = reach.min(254) as u8;
        let fall = 1.0 / (f32::from(reach) + 1.0);
        for (k, &(src, rgb)) in self.sources.iter().enumerate() {
            let mark = k as u32 + 1;
            self.queue.clear();
            self.queue.push_back((src, 0));
            self.seen[src as usize] = mark;
            while let Some((c, steps)) = self.queue.pop_front() {
                let ci = c as usize;
                let f = 1.0 - f32::from(steps) * fall;
                for (l, v) in self.light[ci].iter_mut().zip(rgb) {
                    *l += v * f;
                }
                // A cell under half air takes light but passes none on; the source's own
                // cell always does.
                if steps >= reach || (steps > 0 && u32::from(self.air[ci]) * 2 < u32::from(self.total[ci])) {
                    continue;
                }
                let (x, y, z) = (
                    i64::from(c) % dw,
                    (i64::from(c) / dw) % dh,
                    i64::from(c) / (dw * dh),
                );
                for (nx, ny, nz) in [
                    (x - 1, y, z),
                    (x + 1, y, z),
                    (x, y - 1, z),
                    (x, y + 1, z),
                    (x, y, z - 1),
                    (x, y, z + 1),
                ] {
                    if !(0..dh).contains(&ny) || !(0..dd).contains(&nz) {
                        continue;
                    }
                    let n = ((nz * dh + ny) * dw + nx.rem_euclid(dw)) as usize;
                    if self.seen[n] == mark || self.air[n] == 0 {
                        continue;
                    }
                    self.seen[n] = mark;
                    self.queue.push_back((n as u32, steps + 1));
                }
            }
        }
    }

    /// The light the plane holds at glow cell `(cx, cy, cz)`.
    #[cfg(test)]
    fn at(&self, cx: u32, cy: u32, cz: u32) -> [u8; 3] {
        let i = (((cz * self.dims.1 + cy) * self.dims.0 + cx) * 4) as usize;
        [self.bytes[i], self.bytes[i + 1], self.bytes[i + 2]]
    }
}

/// The glow volume split by candle curve: one grid for the steady emitters (curve 0:
/// every non-glowcap emitter, and every glowcap with the candle off) and one for each
/// candle curve's lips at full brightness. A grid is spread again only when its own
/// emitters or the terrain change; each tick the candle's gains mix them
/// ([`GlowSet::mix`]): `steady + Σ gain_k · curve_k`, which is the one-grid build at
/// those gains up to float rounding, since the spread is linear in its sources.
pub(crate) struct GlowSet {
    /// The volume as the plane holds it: RGBA8, alpha unused.
    pub(crate) bytes: Vec<u8>,
    pub(crate) key: u64,
    /// Grid 0 steady, grid `k` candle curve `k`, each at full brightness.
    grids: Vec<Glow>,
    /// Whether each grid holds any light, and the glow cells a curve grid lights.
    live: Vec<bool>,
    support: Vec<Vec<u32>>,
    /// The cells any curve grid lights, ascending, and the gains last mixed at.
    union: Vec<u32>,
    gains: Vec<f32>,
    /// Whether the steady grid or the supports moved since the last mix.
    stale: bool,
    buckets: Vec<Vec<((u32, u32, u32), [f32; 3])>>,
    /// Scratch: the curve cells' sums.
    sum: Vec<[f32; 3]>,
}

impl GlowSet {
    pub(crate) fn new(width: u32, height: u32, depth: u32) -> GlowSet {
        let n = usize::from(crate::voxel::candle::CLASSES) + 1;
        let grids: Vec<Glow> = (0..n).map(|_| Glow::new(width, height, depth)).collect();
        let cells = grids[0].light.len();
        GlowSet {
            bytes: vec![0; cells * 4],
            key: 0,
            grids,
            live: vec![false; n],
            support: vec![Vec::new(); n],
            union: Vec::new(),
            gains: Vec::new(),
            stale: true,
            buckets: vec![Vec::new(); n],
            sum: vec![[0.0; 3]; cells],
        }
    }

    /// The terrain moved: every grid spreads again at its next update.
    pub(crate) fn set_terrain(&mut self, config: &Config, material: &[Material]) {
        self.grids[0].set_terrain(config, material);
        let (total, air, terrain) = (self.grids[0].total.clone(), self.grids[0].air.clone(), self.grids[0].terrain);
        for g in &mut self.grids[1..] {
            g.total.clone_from(&total);
            g.air.clone_from(&air);
            g.terrain = terrain;
        }
        // An unlit grid stays unlit whatever the terrain; a lit one spreads again.
        self.stale = true;
    }

    /// This tick's emitters, each a voxel, its candle curve (0 steady) and its emissive
    /// colour at full brightness. Spreads the grids whose emitters or terrain moved and
    /// returns how many it spread.
    pub(crate) fn update(
        &mut self,
        emitters: impl Iterator<Item = ((u32, u32, u32), u8, [f32; 3])>,
        gain: f32,
        reach: u32,
    ) -> usize {
        for b in &mut self.buckets {
            b.clear();
        }
        let n = self.grids.len();
        for (v, k, rgb) in emitters {
            self.buckets[usize::from(k).min(n - 1)].push((v, rgb));
        }
        let mut spread = 0;
        for k in 0..n {
            let bucket = &self.buckets[k];
            if bucket.is_empty() && !self.live[k] {
                continue;
            }
            if !self.grids[k].build(bucket.iter().copied(), gain, reach) {
                continue;
            }
            spread += 1;
            self.live[k] = !bucket.is_empty();
            self.stale = true;
            if k > 0 {
                let support = &mut self.support[k];
                support.clear();
                support.extend(
                    self.grids[k]
                        .light
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.iter().any(|&x| x != 0.0))
                        .map(|(c, _)| c as u32),
                );
            }
        }
        if self.stale {
            self.union.clear();
            for s in &self.support[1..] {
                self.union.extend_from_slice(s);
            }
            self.union.sort_unstable();
            self.union.dedup();
        }
        spread
    }

    /// Mix the grids at the curves' gains (`gains[k]` for curve `k`; `gains[0]` unused)
    /// into [`GlowSet::bytes`], bumping [`GlowSet::key`] when a byte changed. Returns
    /// whether one did.
    pub(crate) fn mix(&mut self, gains: &[f32]) -> bool {
        let quantise = |l: [f32; 3]| l.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        let mut changed = false;
        let mut put = |bytes: &mut [u8], c: usize, rgb: [u8; 3]| {
            let px = &mut bytes[c * 4..c * 4 + 4];
            if px[..3] != rgb || px[3] != 255 {
                px[..3].copy_from_slice(&rgb);
                px[3] = 255;
                changed = true;
            }
        };
        let steady = &self.grids[0].light;
        if self.stale {
            // Every cell from the steady grid; the curves' cells below.
            for (c, l) in steady.iter().enumerate() {
                put(&mut self.bytes, c, quantise(*l));
            }
        } else if gains == self.gains.as_slice() {
            return false;
        }
        let gain = |k: usize| gains.get(k).copied().unwrap_or(1.0);
        // Sum into the curve cells' scratch, each grid over its own lit cells only, in
        // grid order (the order a cell's terms are added in does not depend on the gains).
        for &c in &self.union {
            self.sum[c as usize] = steady[c as usize];
        }
        for k in 1..self.grids.len() {
            if !self.live[k] {
                continue;
            }
            let g = gain(k);
            let light = &self.grids[k].light;
            for &c in &self.support[k] {
                let (l, v) = (&mut self.sum[c as usize], light[c as usize]);
                for i in 0..3 {
                    l[i] += v[i] * g;
                }
            }
        }
        for &c in &self.union {
            put(&mut self.bytes, c as usize, quantise(self.sum[c as usize]));
        }
        self.stale = false;
        self.gains.clear();
        self.gains.extend_from_slice(gains);
        if changed {
            self.key += 1;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel::{Command, World};
    use std::time::Instant;
    use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species};

    fn config() -> Config {
        Config {
            width: 24,
            height: 12,
            depth: 6,
            ..Config::default()
        }
    }

    /// An emitter lights the open glow cell beside it, and nothing behind a wall of
    /// terrain: not the wall's own cells, not the cell past it, and not by the way round
    /// the ring, which is longer than the reach.
    #[test]
    fn an_emitter_lights_its_open_neighbour_and_not_behind_terrain() {
        let c = Config {
            width: 32,
            height: 8,
            depth: 12,
            ..Config::default()
        };
        let material: Vec<Material> = (0..c.width * c.height * c.depth)
            .map(|i| {
                let x = i % c.width;
                if (12..16).contains(&x) { Material::Rock } else { Material::Air }
            })
            .collect();
        let mut glow = Glow::new(c.width, c.height, c.depth);
        glow.set_terrain(&c, &material);
        let cyan = [0.05, 0.56, 0.94];
        assert!(glow.build(std::iter::once(((6, 4, 6), cyan)), 0.5, 3));
        let lit = glow.at(2, 1, 1);
        assert!(lit[2] > 0 && lit[1] > 0, "the open neighbour is lit: {lit:?}");
        assert!(glow.at(1, 1, 1)[2] > lit[2], "and less than the source's own cell");
        for cy in 0..2 {
            for cz in 0..3 {
                assert_eq!(glow.at(3, cy, cz), [0; 3], "the wall");
                assert_eq!(glow.at(4, cy, cz), [0; 3], "behind the wall");
            }
        }
        assert!(!glow.build(std::iter::once(((6, 4, 6), cyan)), 0.5, 3), "nothing moved");
    }

    fn set(world: &mut World, x: i64, y: u32, z: u32, material: Material) {
        world.apply(Command::SetMaterial { x, y, z, material });
    }

    /// Ground at y = 0..2, a wall at the back, and a roofed passage: open cells under open
    /// sky, against a wall and under a roof.
    fn world(c: &Config) -> World {
        let mut world = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                set(&mut world, x, 0, z, Material::Bedrock);
                set(&mut world, x, 1, z, Material::Soil);
            }
        }
        for x in 0..i64::from(c.width) {
            for y in 2..4 {
                set(&mut world, x, y, c.depth - 1, Material::Rock);
            }
        }
        for x in 10..16i64 {
            for z in 0..c.depth {
                set(&mut world, x, 5, z, Material::Rock);
            }
        }
        world
    }

    /// Every open cell holds the core's own number for the fan from its foot, quantised
    /// to a byte, so a top face reads exactly the model's sky visibility; and the answer
    /// does not depend on how many threads computed it.
    #[test]
    fn the_sky_plane_is_the_cores_sky_visibility_at_each_open_cells_foot() {
        let c = config();
        let world = world(&c);
        let view = world.view();
        let n = view.material.len();
        let (mut one, mut four) = (vec![0u8; n], vec![0u8; n]);
        sky_plane(&c, view.material, 1, &mut one);
        sky_plane(&c, view.material, 4, &mut four);
        assert!(one == four, "the plane does not depend on the thread count");
        let at = |x: u32, y: u32, z: u32| {
            one[(z as usize * c.height as usize + y as usize) * c.width as usize + x as usize]
        };
        let mut checked = 0;
        for z in 0..c.depth {
            for y in 1..c.height {
                for x in 0..c.width {
                    if view.material_at(i64::from(x), y, z).is_solid() {
                        assert_eq!(at(x, y, z), 0, "a solid is no open cell");
                        continue;
                    }
                    let want = quantise(view.sky_visibility(i64::from(x), y - 1, z));
                    assert_eq!(at(x, y, z), want, "open cell ({x}, {y}, {z})");
                    checked += 1;
                }
            }
        }
        assert!(checked > 500);
        // The three places the fixture is for.
        assert_eq!(at(2, 2, 1), 255, "open ground, far from the wall");
        assert!(at(12, 2, 2) < 128, "under the roof: {}", at(12, 2, 2));
        let wall = at(2, 2, c.depth - 2);
        assert!(
            (130..=210).contains(&wall),
            "in front of the wall about two thirds of the sky: {wall}"
        );
    }

    /// A world whose every open cell is under a roof, so each one casts its whole fan:
    /// a sky plane that takes a while.
    fn roofed(c: &Config) -> World {
        let mut world = World::empty(c.clone());
        let roof = c.height - 2;
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                set(&mut world, x, 0, z, Material::Bedrock);
                if x % 7 != 0 {
                    set(&mut world, x, roof, z, Material::Rock);
                }
            }
        }
        world
    }

    /// A cancelled plane writes nothing more: with the flag already set, not one cell.
    #[test]
    fn a_cancelled_sky_plane_stops_before_its_next_cell() {
        let c = config();
        let world = world(&c);
        let view = world.view();
        let mut out = vec![7u8; view.material.len()];
        let whole = sky_plane_until(&c, view.material, 4, &mut out, &AtomicBool::new(true));
        assert!(!whole, "a cancelled plane says it is not whole");
        assert!(out.iter().all(|&b| b == 7), "no cell was computed after the cancel");
    }

    /// A still world's stager waits for the plane instead of packing again to poll: the
    /// wait sees it finish, and the next poll still returns it, the plane of that id.
    #[test]
    fn waiting_for_a_plane_keeps_it_for_the_next_poll() {
        let c = Config {
            width: 24,
            height: 16,
            depth: 6,
            ..Config::default()
        };
        let world = roofed(&c);
        let view = world.view();
        let mut whole = vec![0u8; view.material.len()];
        sky_plane(&c, view.material, 1, &mut whole);
        let mut worker = SkyWorker::with_threads(1);
        assert!(!worker.wait(Duration::from_millis(1)), "nothing submitted, nothing finishes");
        worker.submit(7, &c, view.material);
        let deadline = Instant::now() + Duration::from_secs(60);
        while !worker.wait(Duration::from_millis(50)) {
            assert!(Instant::now() < deadline, "the plane never finished");
        }
        assert!(worker.wait(Duration::ZERO), "a plane seen is kept until it is polled");
        assert_eq!(worker.poll(), Some((7, whole)), "the poll returns the plane the wait saw");
        assert_eq!(worker.poll(), None);
        assert!(!worker.wait(Duration::from_millis(1)));
    }

    /// Dropping the worker while it computes a plane gives the plane up and joins the
    /// thread: the drop returns in a small part of the time the whole plane takes, and no
    /// thread of the worker is left holding anything of it.
    #[test]
    fn dropping_the_sky_worker_mid_plane_returns_promptly_and_ends_its_thread() {
        let c = Config {
            width: 96,
            height: 32,
            depth: 16,
            ..Config::default()
        };
        let world = roofed(&c);
        let view = world.view();
        let mut full = vec![0u8; view.material.len()];
        let at = Instant::now();
        sky_plane(&c, view.material, 1, &mut full);
        let whole = at.elapsed();

        let worker = SkyWorker::with_threads(1);
        // The thread holds its own clone of the cancel flag for as long as it runs.
        let thread_alive = Arc::downgrade(&worker.cancel);
        worker.submit(1, &c, view.material);
        std::thread::sleep(whole / 8);
        let at = Instant::now();
        drop(worker);
        let dropped = at.elapsed();
        assert!(
            dropped < whole / 2,
            "the drop gave the plane up ({dropped:?}, the whole plane takes {whole:?})"
        );
        assert!(thread_alive.upgrade().is_none(), "the worker's thread has ended");
    }

    fn flora(world: &World, stands: &[(i64, u32, Species)]) -> Flora {
        let mut flora = Flora::new(FloraConfig::default());
        for &(x, z, species) in stands {
            let wood = flora.config().species(species).wood_max;
            assert!(flora.apply(
                world,
                FloraCommand::Seed {
                    x,
                    z,
                    species,
                    wood
                }
            ));
        }
        flora
    }

    /// The ground under a crown is dimmed by exactly the layers whose discs are drawn
    /// above it, as the shade model multiplies them; the crown's own cells and anything
    /// outside its disc are not; two crowns over one column multiply.
    /// The per-curve grids mixed at a tick's gains are the one-grid build at those gains,
    /// to a byte; and a tick whose emitters and terrain did not move spreads nothing,
    /// whatever the gains do.
    #[test]
    fn the_curve_grids_mix_to_the_one_grid_build_and_spread_only_on_change() {
        let c = Config {
            width: 32,
            height: 8,
            depth: 12,
            ..Config::default()
        };
        let material: Vec<Material> = (0..c.width * c.height * c.depth)
            .map(|i| {
                let x = i % c.width;
                if (12..16).contains(&x) { Material::Rock } else { Material::Air }
            })
            .collect();
        let emitters: Vec<((u32, u32, u32), u8, [f32; 3])> = vec![
            ((6, 4, 6), 0, [0.05, 0.56, 0.94]),
            ((7, 4, 6), 3, [0.9, 0.6, 0.1]),
            ((8, 2, 5), 3, [0.9, 0.6, 0.1]),
            ((20, 1, 2), 1, [0.4, 0.9, 0.2]),
            ((21, 1, 3), 0, [0.2, 0.2, 0.7]),
            ((22, 6, 9), 16, [0.7, 0.3, 0.6]),
        ];
        let mut set = GlowSet::new(c.width, c.height, c.depth);
        set.set_terrain(&c, &material);
        let mut one = Glow::new(c.width, c.height, c.depth);
        one.set_terrain(&c, &material);
        let check = |set: &GlowSet, one: &mut Glow, gains: &[f32]| {
            let scaled = emitters
                .iter()
                .map(|&(v, k, rgb)| (v, rgb.map(|x| x * if k == 0 { 1.0 } else { gains[usize::from(k)] })));
            one.build(scaled, 0.5, 3);
            for (i, (a, b)) in set.bytes.iter().zip(&one.bytes).enumerate() {
                assert!(a.abs_diff(*b) <= 1, "byte {i}: mixed {a}, built {b}");
            }
        };
        let mut gains = vec![1.0f32; 17];
        assert_eq!(set.update(emitters.iter().copied(), 0.5, 3), 4, "steady and curves 1, 3, 16");
        set.mix(&gains);
        check(&set, &mut one, &gains);
        for (k, g) in gains.iter_mut().enumerate() {
            *g = 0.25 + 0.04 * k as f32;
        }
        assert_eq!(set.update(emitters.iter().copied(), 0.5, 3), 0, "nothing moved");
        let key = set.key;
        assert!(set.mix(&gains), "the candle moved the light");
        assert_ne!(set.key, key);
        check(&set, &mut one, &gains);
        assert!(!set.mix(&gains), "the same gains, the same bytes");
        // One curve's emitter moves: only its grid spreads.
        let mut moved = emitters.clone();
        moved[3].0 = (19, 1, 2);
        assert_eq!(set.update(moved.iter().copied(), 0.5, 3), 1);
    }

    #[test]
    fn the_canopy_plane_attenuates_straight_down_by_the_models_layers() {
        let c = Config {
            width: 32,
            height: 40,
            depth: 12,
            ..Config::default()
        };
        let mut world = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                set(&mut world, x, 0, z, Material::Bedrock);
                set(&mut world, x, 1, z, Material::Soil);
            }
        }
        let flora = flora(
            &world,
            &[(8, 6, Species::Umbrellafrond), (9, 6, Species::Bloomcrown)],
        );
        let view = flora.view();
        let mut canopy = Canopy::new(c.width, c.depth);
        let mut plane = vec![0u8; c.width as usize * c.depth as usize * CANOPY_STEPS * 2];
        canopy.build(view, &mut plane);

        // What the shade model would multiply for a receiver on the ground at (x, z).
        let k = view.config.shade_k_per_m2;
        let model = |x: i64, z: i64, y: u32| -> f64 {
            let mut t = 1.0;
            for s in view.stands {
                for l in view.layers(s) {
                    let (dx, dz) = (x - i64::from(s.site.x), z - i64::from(s.site.z));
                    let inside = ((dx * dx + dz * dz) as f64) <= l.radius_v * l.radius_v;
                    if inside && l.cell > i64::from(y) {
                        t *= (-k * (1.0 - l.porosity) * l.stock / l.area_m2).exp();
                    }
                }
            }
            t
        };
        let (mut shaded, mut both) = (0, 0);
        for z in 0..c.depth {
            for x in 0..c.width {
                let want = model(i64::from(x), i64::from(z), 1);
                let got = Canopy::read(&plane, c.width, x, z, 1);
                assert!(
                    (f64::from(got) - want).abs() <= 1.0 / 255.0,
                    "ground at ({x}, {z}): the plane says {got}, the model {want}"
                );
                if want < 0.999 {
                    shaded += 1;
                }
                let stands_over = view
                    .stands
                    .iter()
                    .filter(|s| {
                        view.layers(s).any(|l| {
                            let (dx, dz) = (
                                i64::from(x) - i64::from(s.site.x),
                                i64::from(z) - i64::from(s.site.z),
                            );
                            ((dx * dx + dz * dz) as f64) <= l.radius_v * l.radius_v
                        })
                    })
                    .count();
                if stands_over == 2 {
                    both += 1;
                }
            }
        }
        assert!(shaded > 0, "the crowns shade some ground");
        assert!(both > 0, "the fixture overlaps its two crowns");
        // A crown's own cell is not under itself: at the highest disc's cell, nothing.
        let top = view
            .stands
            .iter()
            .flat_map(|s| view.layers(s).map(|l| l.cell))
            .max()
            .unwrap();
        assert_eq!(Canopy::read(&plane, c.width, 8, 6, top as u32), 1.0);
        // Far from both, open.
        assert_eq!(Canopy::read(&plane, c.width, 24, 1, 1), 1.0);
    }

    /// A stand's own cells are shaded by every other stand's layers above them and never
    /// by its own (the model never lets a stand shade itself); a mark (no stand) takes the
    /// whole column. A crown cell's pass chance spreads the column's transmission over its
    /// crown cells.
    #[test]
    fn a_plant_cell_is_not_shaded_by_its_own_stand() {
        let c = Config {
            width: 32,
            height: 40,
            depth: 12,
            ..Config::default()
        };
        let mut world = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                set(&mut world, x, 0, z, Material::Bedrock);
                set(&mut world, x, 1, z, Material::Soil);
            }
        }
        let flora = flora(
            &world,
            &[(8, 6, Species::Umbrellafrond), (9, 6, Species::Bloomcrown)],
        );
        let view = flora.view();
        let mut canopy = Canopy::new(c.width, c.depth);
        let mut plane = vec![0u8; c.width as usize * c.depth as usize * CANOPY_STEPS * 2];
        canopy.build(view, &mut plane);
        let k = view.config.shade_k_per_m2;
        // The model's product over the layers of every stand but `skip` above `y`.
        let model = |skip: Option<usize>, x: i64, z: i64, y: i64| -> f64 {
            let mut t = 1.0;
            for (i, s) in view.stands.iter().enumerate() {
                if Some(i) == skip {
                    continue;
                }
                for l in view.layers(s) {
                    let (dx, dz) = (x - i64::from(s.site.x), z - i64::from(s.site.z));
                    if ((dx * dx + dz * dz) as f64) <= l.radius_v * l.radius_v && l.cell > y {
                        t *= (-k * (1.0 - l.porosity) * l.stock / l.area_m2).exp();
                    }
                }
            }
            t
        };
        let nibble = |v: f64| (v * 15.0).round() as u8;
        let (x, z, y) = (9u32, 6u32, 2u32);
        let all = model(None, 9, 6, 2);
        assert!(all < 0.9, "the fixture's column is shaded: {all}");
        for stand in 0..view.stands.len() {
            let others = model(Some(stand), 9, 6, 2);
            let byte = canopy.plant_byte(Some(stand), false, x, y, z);
            assert_eq!(byte & 15, nibble(others), "stand {stand}'s own cell");
            assert_eq!(byte >> 4, 0, "not a crown: no pass chance");
            assert!(others > all, "its own layers are divided out");
        }
        assert_eq!(canopy.plant_byte(None, false, x, y, z) & 15, nibble(all));
        // Three crown cells in the column share its transmission.
        canopy.count_crowns([(x, z), (x, z), (x, z)].into_iter());
        let ground = model(None, 9, 6, -1);
        assert_eq!(
            canopy.plant_byte(Some(0), true, x, y, z) >> 4,
            nibble(ground.powf(1.0 / 3.0))
        );
    }

    /// Five distinct layers over one column: the ground under all of them keeps the whole
    /// product, and the steps stay highest first.
    #[test]
    fn a_column_under_more_layers_than_steps_keeps_the_grounds_product() {
        let mut canopy = Canopy::new(1, 1);
        let layers = [(30u8, 0.9f32), (10, 0.8), (20, 0.7), (5, 0.6), (25, 0.5)];
        for (cell, t) in layers {
            canopy.add(0, cell, t);
        }
        let mut plane = vec![0u8; CANOPY_STEPS * 2];
        canopy.write(&mut plane);
        let all: f32 = layers.iter().map(|l| l.1).product();
        assert!((Canopy::read(&plane, 1, 0, 0, 1) - all).abs() <= 1.0 / 255.0);
        assert!((Canopy::read(&plane, 1, 0, 0, 26) - 0.9).abs() <= 1.0 / 255.0);
        assert!((Canopy::read(&plane, 1, 0, 0, 21) - 0.45).abs() <= 1.0 / 255.0);
        assert_eq!(Canopy::read(&plane, 1, 0, 0, 30), 1.0);
        let cells: Vec<u8> = (0..CANOPY_STEPS)
            .map(|k| plane[(k / 2) * 4 + (k % 2) * 2])
            .collect();
        assert!(cells.windows(2).all(|p| p[0] > p[1]), "{cells:?}");
    }
}
