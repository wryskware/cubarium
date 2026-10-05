//! **The live controller stage's occupancy, held between stages and patched** (SPEED-MATURE).
//!
//! A mature world's stage used to lay two whole-ring grids from scratch every tick — every
//! stand's layer discs, every ground pool, every body — although between two plant steps
//! almost nothing in them moves, and a plant step moves only the crowns that crossed a cell.
//! [`LiveGrid`] keeps **one** grid of the stand classes and ground pools and, each stage,
//!
//! - compares every stand with the one it laid (site, species, wood and layer stocks, to
//!   the bit); only a stand whose discs' **cells or class** moved dirties the columns its
//!   old and new discs box, and those columns are re-laid from every stand that reaches
//!   them, in stand order, foliage before trunks — the whole build's order restricted to
//!   them, so they hold exactly what a fresh build holds;
//! - compares the ground entries' organic stocks; a column whose pools' cell runs moved is
//!   re-laid, and the pools' tops are read from a per-column index rebuilt from the ground;
//! - clears and re-lays the bodies, both flavours side by side in their own bits: the v2
//!   cone's one cell per body and the predation occupancy's bodies with volume.
//!
//! A stage with too many dirty columns, a moved terrain or shape, or no current reach index
//! in the plant layer rebuilds everything, on the compute pool. Every reading through a
//! [`LiveView`] equals the reading of [`super::build_occupancy_as`] on the same state (the
//! window aside: the live grid covers the whole ring, which a windowed build's observers
//! read the same). Exact: no knob.

use std::ops::Range;

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FaceLayer, FloraView, Ground, MAX_FOLIAGE_LAYERS, Site, Species};

use super::{
    BODY_BIT, Band, CellOccupancy, ENV_MASK, ENV_TRUNK, Fine, LayerDisc, POOL_BIT, StandDiscs, bands_of,
    pool_top_m,
};

/// A second body in a v2-cone cell.
const MULTI_FLAT: u8 = 0b0001_0000;
/// A body with volume (the predation occupancy) fills the cell.
const BODY_VOLUME: u8 = 0b0010_0000;
/// A second body with volume in the cell.
const MULTI_VOLUME: u8 = 0b0100_0000;

/// Which of the two body layers a [`LiveView`] reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flavour {
    /// One cell per body, the v2 cone's.
    Flat,
    /// Bodies with volume, the predation occupancy's.
    Volume,
}

impl Flavour {
    const fn bits(self) -> (u8, u8) {
        match self {
            Flavour::Flat => (BODY_BIT, MULTI_FLAT),
            Flavour::Volume => (BODY_VOLUME, MULTI_VOLUME),
        }
    }
}

/// A stand as the grid last laid it.
#[derive(Clone, Debug)]
struct Laid {
    key: (Site, FaceLayer),
    species: Species,
    wood: u64,
    stock: [u64; MAX_FOLIAGE_LAYERS],
    /// Its discs in [`LiveGrid::discs`].
    discs: Range<u32>,
    /// The widest disc's half-width in cells: the box its discs can touch.
    span: i64,
}

/// One body's slab as laid, enough to say whether a cell is its own.
#[derive(Clone, Copy, Debug)]
struct Slab {
    id: u64,
    ax: i64,
    az: i64,
    x_m: f64,
    z_m: f64,
    r2: f64,
    span: i64,
    from: i64,
    to: i64,
}

impl Slab {
    /// The same slab, to the bit.
    fn same(&self, o: &Slab) -> bool {
        (self.id, self.ax, self.az, self.span, self.from, self.to) == (o.id, o.ax, o.az, o.span, o.from, o.to)
            && (self.x_m.to_bits(), self.z_m.to_bits(), self.r2.to_bits()) == (o.x_m.to_bits(), o.z_m.to_bits(), o.r2.to_bits())
    }

    /// Every column `(x wrapped, z)` the slab fills, as [`Band::lay_bodies`] walks them.
    fn for_each_column(&self, c: &cubarium_voxel::Config, mut f: impl FnMut(i64, i64)) {
        let v = c.voxel_m;
        for dz in -self.span..=self.span {
            let z = self.az + dz;
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            for dx in -self.span..=self.span {
                let x = self.ax + dx;
                if dx != 0 || dz != 0 {
                    let cx = (x as f64 + 0.5) * v - self.x_m;
                    let cz = (z as f64 + 0.5) * v - self.z_m;
                    if cx * cx + cz * cz > self.r2 {
                        continue;
                    }
                }
                f(x.rem_euclid(i64::from(c.width)), z);
            }
        }
    }

    /// The slab's cells in column `(x, z)`.
    fn column_cells(&self, c: &cubarium_voxel::Config, (x, z): (i64, i64), mut f: impl FnMut(usize)) {
        for layer in self.from.max(1)..self.to.min(i64::from(c.height)) {
            f(c.index(x, layer as u32, z as u32));
        }
    }
}

/// One flavour's bodies as laid: a slab per body, and how many fill each cell, so a stage
/// re-lays only the bodies that moved.
#[derive(Clone, Debug, Default)]
struct Bodies {
    /// Per cell, how many of these bodies fill it; empty until the first lay.
    count: Vec<u8>,
    /// Sorted by id (the animals are).
    slabs: Vec<Slab>,
    /// Scratch: the stage's slabs.
    next: Vec<Slab>,
}

impl Bodies {
    /// Forget every body, for a grid laid from zero.
    fn reset(&mut self) {
        self.count.fill(0);
        self.slabs.clear();
    }

    /// Add or take away one slab's bodies, cell by cell, and set each cell's bits from its
    /// count.
    fn apply(&mut self, cells: &mut [u8], c: &cubarium_voxel::Config, slab: &Slab, add: bool, bits: (u8, u8)) {
        let mut columns = Vec::new();
        slab.for_each_column(c, |x, z| columns.push((x, z)));
        for col in columns {
            self.apply_column(cells, c, slab, col, add, bits);
        }
    }

    /// A body moved from slab `o` to slab `n`: when both span the same layers, only the
    /// columns one fills and the other does not change; else all of both.
    fn shift(&mut self, cells: &mut [u8], c: &cubarium_voxel::Config, o: &Slab, n: &Slab, bits: (u8, u8)) {
        if (o.from, o.to) != (n.from, n.to) {
            self.apply(cells, c, o, false, bits);
            self.apply(cells, c, n, true, bits);
            return;
        }
        let (mut was, mut now) = (Vec::new(), Vec::new());
        o.for_each_column(c, |x, z| was.push((x, z)));
        n.for_each_column(c, |x, z| now.push((x, z)));
        was.sort_unstable();
        now.sort_unstable();
        for &col in &was {
            if now.binary_search(&col).is_err() {
                self.apply_column(cells, c, o, col, false, bits);
            }
        }
        for &col in &now {
            if was.binary_search(&col).is_err() {
                self.apply_column(cells, c, n, col, true, bits);
            }
        }
    }

    fn apply_column(
        &mut self,
        cells: &mut [u8],
        c: &cubarium_voxel::Config,
        slab: &Slab,
        col: (i64, i64),
        add: bool,
        (body, multi): (u8, u8),
    ) {
        let count = &mut self.count;
        slab.column_cells(c, col, |cell| {
            let n = &mut count[cell];
            *n = if add { n.saturating_add(1) } else { n.saturating_sub(1) };
            let b = &mut cells[cell];
            *b &= !(body | multi);
            if *n >= 1 {
                *b |= body;
            }
            if *n >= 2 {
                *b |= multi;
            }
        });
    }
}

/// Per column `z * width + x`, the ground pools' cell runs `(from, to, top)` in ground order.
#[derive(Clone, Debug, Default)]
struct PoolIndex {
    col_start: Vec<u32>,
    runs: Vec<(u32, u32, f64)>,
    /// Per ground entry, its run's slot in `runs`, or `u32::MAX` for none.
    slot: Vec<u32>,
}

/// The cells `from..to` a ground entry's pool reaches and its top, as [`Band::lay_pools`]
/// walks them; `None` for no pool or none of its cells.
fn pool_run(c: &cubarium_voxel::Config, g: &Ground, density: f64) -> Option<(u32, u32, f64)> {
    let top = pool_top_m(g, c.voxel_m, density)?;
    if g.site.z >= c.depth || g.site.x >= c.width {
        return None;
    }
    let from = g.site.y + 1;
    let mut to = from;
    while to < c.height && f64::from(to) * c.voxel_m < top {
        to += 1;
    }
    (to > from).then_some((from, to, top))
}

impl PoolIndex {
    fn build(view: &VoxelView<'_>, ground: &[Ground], density: f64) -> PoolIndex {
        let c = view.config;
        let (width, plane) = (c.width as usize, c.width as usize * c.depth as usize);
        let mut runs: Vec<(usize, usize, (u32, u32, f64))> = Vec::new();
        for (gi, g) in ground.iter().enumerate() {
            if let Some(run) = pool_run(c, g, density) {
                runs.push((g.site.z as usize * width + g.site.x as usize, gi, run));
            }
        }
        // A counting sort by column keeps ground order within a column.
        let mut col_start = vec![0u32; plane + 1];
        for &(col, _, _) in &runs {
            col_start[col + 1] += 1;
        }
        for k in 0..plane {
            col_start[k + 1] += col_start[k];
        }
        let mut fill = col_start.clone();
        let mut out = vec![(0, 0, 0.0); runs.len()];
        let mut slot = vec![u32::MAX; ground.len()];
        for (col, gi, run) in runs {
            out[fill[col] as usize] = run;
            slot[gi] = fill[col];
            fill[col] += 1;
        }
        PoolIndex { col_start, runs: out, slot }
    }

    #[inline]
    fn column(&self, col: usize) -> &[(u32, u32, f64)] {
        match (self.col_start.get(col), self.col_start.get(col + 1)) {
            (Some(&a), Some(&b)) => &self.runs[a as usize..b as usize],
            _ => &[],
        }
    }
}

/// The live stage's occupancy grid, kept on the fauna between stages. Session state, like
/// the controllers: not saved and not cloned.
#[derive(Default)]
pub(crate) struct LiveGrid {
    built: bool,
    shape: (u64, u32, u32, u32, u64, u64),
    cells: Vec<u8>,
    stands: Vec<Laid>,
    discs: Vec<LayerDisc>,
    ground: Vec<(Site, u64)>,
    pools: PoolIndex,
    flat: Bodies,
    volume: Bodies,
    /// Per column, scratch: dirty this stage.
    mask: Vec<bool>,
    /// Statistics for a diagnostic: full rebuilds and patched stages so far.
    pub(crate) rebuilds: u64,
    pub(crate) patches: u64,
    /// Columns re-laid by patches so far.
    pub(crate) relaid: u64,
    /// Patches whose stands were compared in place.
    pub(crate) in_place: u64,
}

impl Clone for LiveGrid {
    fn clone(&self) -> Self {
        LiveGrid::default()
    }
}

impl std::fmt::Debug for LiveGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LiveGrid({} stands, {} rebuilds, {} patches ({} in place), {} columns re-laid)",
            self.stands.len(),
            self.rebuilds,
            self.patches,
            self.in_place,
            self.relaid
        )
    }
}

/// The share of the plane's columns over which a stage rebuilds instead of patching.
const REBUILD_SHARE: usize = 4;
/// The fewest stands one job reads discs for.
const STANDS_PER_JOB: usize = 512;
/// The fewest ground entries one job compares.
const GROUND_PER_JOB: usize = 4096;

/// One stand's discs and the record of it, read through the view's own stand (so its
/// cached crown is used).
fn read_stand(fv: &FloraView<'_>, si: usize, buf: &mut Vec<cubarium_voxel_flora::StandLayer>, discs: &mut Vec<LayerDisc>) -> Laid {
    let stand = &fv.stands[si];
    buf.clear();
    fv.profile_layers_into(stand, buf);
    let from = discs.len() as u32;
    let mut span = 0;
    for (li, layer) in buf.iter().enumerate() {
        let d = LayerDisc::of(stand.site, layer, si, li);
        span = span.max(d.span);
        discs.push(d);
    }
    Laid {
        key: (stand.site, fv.config.face_layer(stand.species)),
        species: stand.species,
        wood: stand.wood.to_bits(),
        stock: stand.layer_stock.map(f64::to_bits),
        discs: from..discs.len() as u32,
        span,
    }
}

/// Whether two disc lists of one stand cover the same cells with the same classes: the
/// same rows, and the same integer disc (`dx² + dz² ≤ r²` over integers is `≤ ⌊r²⌋`).
fn same_discs(a: &[LayerDisc], b: &[LayerDisc]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(p, q)| {
            p.rows == q.rows && p.code == q.code && p.r2.floor() == q.r2.floor() && p.x == q.x && p.z == q.z
        })
}

/// One chunk of the stand comparison: the new records and discs of `fv.stands[new]`, and
/// the boxes `(x, z, span)` whose columns moved, against the old records `old`.
struct DiffPart {
    laid: Vec<Laid>,
    discs: Vec<LayerDisc>,
    boxes: Vec<(i64, i64, i64)>,
}

fn diff_chunk(fv: &FloraView<'_>, new: Range<usize>, old: &[Laid], old_discs: &[LayerDisc]) -> DiffPart {
    let mut part = DiffPart {
        laid: Vec::with_capacity(new.len()),
        discs: Vec::new(),
        boxes: Vec::new(),
    };
    let mut buf = Vec::new();
    let mut k = 0;
    let box_of = |l: &Laid| (i64::from(l.key.0.x), i64::from(l.key.0.z), l.span);
    for si in new {
        let stand = &fv.stands[si];
        let key = (stand.site, fv.config.face_layer(stand.species));
        while k < old.len() && old[k].key < key {
            part.boxes.push(box_of(&old[k]));
            k += 1;
        }
        let same = k < old.len() && old[k].key == key && old[k].species == stand.species;
        if !same {
            if k < old.len() && old[k].key == key {
                part.boxes.push(box_of(&old[k]));
                k += 1;
            }
            let laid = read_stand(fv, si, &mut buf, &mut part.discs);
            part.boxes.push(box_of(&laid));
            part.laid.push(laid);
            continue;
        }
        let was = &old[k];
        k += 1;
        let wood = stand.wood.to_bits();
        let stock = stand.layer_stock.map(f64::to_bits);
        let was_discs = &old_discs[was.discs.start as usize..was.discs.end as usize];
        if was.wood == wood && was.stock == stock {
            let from = part.discs.len() as u32;
            part.discs.extend(was_discs.iter().map(|d| LayerDisc { stand: si as u32, ..*d }));
            part.laid.push(Laid {
                discs: from..part.discs.len() as u32,
                ..was.clone()
            });
            continue;
        }
        let laid = read_stand(fv, si, &mut buf, &mut part.discs);
        let now = &part.discs[laid.discs.start as usize..laid.discs.end as usize];
        if !same_discs(was_discs, now) {
            part.boxes.push(box_of(was));
            part.boxes.push(box_of(&laid));
        }
        part.laid.push(laid);
    }
    part.boxes.extend(old[k..].iter().map(box_of));
    part
}

impl LiveGrid {
    /// Bring the grid up to the world, the plant layer and the bodies as they stand, with
    /// the body flavours a stage will read (`flat`, `volume`) laid and the others cleared:
    /// a [`LiveView`] of a flavour not laid by the last refresh reads no bodies.
    pub(crate) fn refresh(
        &mut self,
        view: &VoxelView<'_>,
        fv: &FloraView<'_>,
        fauna: &crate::FaunaView<'_>,
        threads: usize,
        (flat, volume): (bool, bool),
    ) {
        let c = view.config;
        let density = fauna.config.cue.pool_bulk_density;
        let shape = (
            view.terrain_version,
            c.width,
            c.height,
            c.depth,
            c.voxel_m.to_bits(),
            density.to_bits(),
        );
        #[cfg(feature = "profile")]
        let mut tm = cubarium_voxel::profile::start(cubarium_voxel::profile::Phase::FaunaWindowRead);
        if !self.built || self.shape != shape || !self.patch(view, fv, density, threads) {
            self.rebuild(view, fv, density, threads);
            self.shape = shape;
            self.built = true;
            self.rebuilds += 1;
        } else {
            self.patches += 1;
        }
        #[cfg(feature = "profile")]
        tm.next(cubarium_voxel::profile::Phase::FaunaWindowLay);
        for (flavour, wanted) in [(Flavour::Flat, flat), (Flavour::Volume, volume)] {
            if wanted {
                self.lay_bodies(view, fauna, flavour);
            } else {
                self.clear_bodies(view.config, flavour);
            }
        }
    }

    /// What the stage reads, in one flavour.
    pub(crate) fn view<'a>(&'a self, view: &VoxelView<'_>, flavour: Flavour) -> LiveView<'a> {
        let c = view.config;
        LiveView {
            grid: self,
            flavour,
            width: c.width,
            depth: c.depth,
            voxel_m: c.voxel_m,
        }
    }

    /// Read every stand's discs and lay everything but the bodies from scratch.
    fn rebuild(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>, density: f64, threads: usize) {
        let c = view.config;
        let plane = c.width as usize * c.depth as usize;
        // The stands' discs, in chunks across the pool.
        let ranges = crate::par::ranges(fv.stands.len(), threads, STANDS_PER_JOB);
        let jobs: Vec<crate::par::Job<'_, DiffPart>> = ranges
            .into_iter()
            .map(|r| -> crate::par::Job<'_, DiffPart> {
                Box::new(move || {
                    let mut part = DiffPart { laid: Vec::with_capacity(r.len()), discs: Vec::new(), boxes: Vec::new() };
                    let mut buf = Vec::new();
                    for si in r {
                        let laid = read_stand(fv, si, &mut buf, &mut part.discs);
                        part.laid.push(laid);
                    }
                    part
                })
            })
            .collect();
        self.stands.clear();
        self.discs.clear();
        for part in crate::par::run(threads, jobs) {
            self.append(part);
        }
        self.ground = ground_keys(fv);
        self.pools = PoolIndex::build(view, fv.ground, density);
        self.flat.reset();
        self.volume.reset();
        self.mask.clear();
        self.mask.resize(plane, false);
        // Lay in interleaved bands of cell layers, as the stage's grids always were.
        let bands = if cfg!(feature = "parallel") {
            threads.clamp(1, (c.height as usize).max(1))
        } else {
            1
        };
        let mut sorted = StandDiscs {
            parts: vec![(vec![Vec::new(); bands], vec![Vec::new(); bands])],
        };
        for d in &self.discs {
            let lists = if d.code == ENV_TRUNK { &mut sorted.parts[0].1 } else { &mut sorted.parts[0].0 };
            bands_of(d.rows.0, d.rows.1, c.height, bands, |b| lists[b].push(*d));
        }
        let mut cells = std::mem::take(&mut self.cells);
        cells.resize(c.cells(), 0);
        {
            let (sorted, pools) = (&sorted, &self.pools);
            let jobs: Vec<crate::par::Job<'_, ()>> = Band::interleaved(&mut cells, view, bands)
                .into_iter()
                .map(|mut band| -> crate::par::Job<'_, ()> {
                    Box::new(move || {
                        for row in band.rows.iter_mut().flatten() {
                            row.fill(0);
                        }
                        band.lay_stands(view, sorted, None, |_, _, _| {});
                        band.lay_pool_runs(view, pools);
                    })
                })
                .collect();
            crate::par::run(threads, jobs);
        }
        self.cells = cells;
    }

    fn append(&mut self, part: DiffPart) {
        let base = self.discs.len() as u32;
        self.discs.extend(part.discs);
        self.stands.extend(part.laid.into_iter().map(|mut l| {
            l.discs = l.discs.start + base..l.discs.end + base;
            l
        }));
    }

    /// Patch the grid from what moved since the last stage; `false` when it must be rebuilt
    /// instead (left as it was).
    fn patch(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>, density: f64, threads: usize) -> bool {
        let c = view.config;
        let (width, depth) = (i64::from(c.width), i64::from(c.depth));
        let plane = c.width as usize * c.depth as usize;
        // The stands: in place when the list kept its shape (the usual stage: bites and a
        // plant step's growth), else re-recorded whole. Either way the records are the
        // new ones from here, and a `false` below rebuilds them all again.
        let boxes = match self.diff_in_place(fv, threads) {
            Some(boxes) => {
                self.in_place += 1;
                boxes
            }
            None => self.diff_general(fv, threads),
        };
        // The dirty columns.
        self.mask.resize(plane, false);
        let mut dirty: Vec<usize> = Vec::new();
        let mark = |mask: &mut Vec<bool>, col: usize, dirty: &mut Vec<usize>| {
            if !mask[col] {
                mask[col] = true;
                dirty.push(col);
            }
        };
        for &(x, z, span) in &boxes {
            let (x0, xn) = if 2 * span + 1 >= width { (0, width) } else { (x - span, 2 * span + 1) };
            for zz in (z - span).max(0)..=(z + span).min(depth - 1) {
                for xx in x0..x0 + xn {
                    mark(&mut self.mask, zz as usize * width as usize + xx.rem_euclid(width) as usize, &mut dirty);
                }
            }
        }
        let env_dirty = dirty.len();
        let clear_mask = |mask: &mut Vec<bool>, dirty: &[usize]| {
            for &col in dirty {
                mask[col] = false;
            }
        };
        if env_dirty * REBUILD_SHARE > plane {
            clear_mask(&mut self.mask, &dirty);
            return false;
        }
        // Who reaches the dirty columns, ascending: the plant layer's reach index.
        // Marked in a bit per stand and read back in order: a growing plant step dirties
        // thousands of columns, each listing the same few dozen stands.
        let mut marks = vec![0u64; fv.stands.len().div_ceil(64)];
        for &col in &dirty {
            let (x, z) = ((col % width as usize) as i64, (col / width as usize) as u32);
            match fv.stands_near(x, z, c.width, c.depth) {
                Some(s) => {
                    for &si in s {
                        marks[si as usize / 64] |= 1 << (si % 64);
                    }
                }
                None => {
                    clear_mask(&mut self.mask, &dirty);
                    return false;
                }
            }
        }
        let mut near: Vec<u32> = Vec::new();
        for (w, &word) in marks.iter().enumerate() {
            let mut bits = word;
            while bits != 0 {
                near.push((w * 64) as u32 + bits.trailing_zeros());
                bits &= bits - 1;
            }
        }
        // Ground: the pools' tops in place while no pool's cells moved, else the runs
        // re-indexed and the columns whose runs moved re-laid.
        if !self.ground_in_place(view, fv, density, threads) {
            let ground = ground_keys(fv);
            let pools = PoolIndex::build(view, fv.ground, density);
            // The columns whose runs moved: the old and new entries walked side by side
            // (both site-sorted), an entry's column marked where its run's cells differ,
            // it came or it went.
            let cells_of = |p: &PoolIndex, k: usize| {
                let slot = p.slot[k];
                (slot != u32::MAX).then(|| {
                    let r = p.runs[slot as usize];
                    (r.0, r.1)
                })
            };
            let col_of = |site: Site| site.z as usize * c.width as usize + site.x as usize;
            let (mut i, mut j) = (0, 0);
            while i < self.ground.len() || j < ground.len() {
                let (moved, site) = match (self.ground.get(i), ground.get(j)) {
                    (Some(o), Some(n)) if o.0 == n.0 => {
                        let moved = cells_of(&self.pools, i) != cells_of(&pools, j);
                        i += 1;
                        j += 1;
                        (moved, n.0)
                    }
                    (Some(o), n) if n.is_none_or(|n| o.0 < n.0) => {
                        i += 1;
                        (cells_of(&self.pools, i - 1).is_some(), o.0)
                    }
                    (_, Some(n)) => {
                        j += 1;
                        (cells_of(&pools, j - 1).is_some(), n.0)
                    }
                    (_, None) => unreachable!("the walk ends when both lists do"),
                };
                if moved && site.x < c.width && site.z < c.depth {
                    mark(&mut self.mask, col_of(site), &mut dirty);
                }
            }
            self.pools = pools;
            self.ground = ground;
        }
        if dirty.is_empty() {
            return true;
        }
        self.relaid += dirty.len() as u64;
        // Re-lay the stand classes over the stand-dirty columns, then every dirty column's
        // pools.
        let height = c.height as usize;
        for &col in &dirty[..env_dirty] {
            for y in 0..height {
                self.cells[y * plane + col] &= !(ENV_MASK | POOL_BIT);
            }
        }
        for foliage_pass in [true, false] {
            for &si in &near {
                let l = &self.stands[si as usize];
                for d in &self.discs[l.discs.start as usize..l.discs.end as usize] {
                    if (d.code != ENV_TRUNK) == foliage_pass {
                        lay_disc_in(c, d, &self.mask, &mut self.cells);
                    }
                }
            }
        }
        for &col in &dirty {
            for y in 0..height {
                self.cells[y * plane + col] &= !POOL_BIT;
            }
            for &(from, to, _) in self.pools.column(col) {
                for y in from..to {
                    let b = &mut self.cells[y as usize * plane + col];
                    if *b & (ENV_MASK | POOL_BIT) == 0 {
                        *b |= POOL_BIT;
                    }
                }
            }
        }
        clear_mask(&mut self.mask, &dirty);
        true
    }

    /// The stands compared in place, when the list kept its length and every position
    /// its stand (site, face layer, species): each stand whose wood or stocks moved is
    /// read again, and its record and discs overwritten where they lie. The boxes whose
    /// columns moved, or `None` — nothing touched — when the list changed shape or a
    /// stand's disc count did.
    fn diff_in_place(&mut self, fv: &FloraView<'_>, threads: usize) -> Option<Vec<(i64, i64, i64)>> {
        let n = fv.stands.len();
        if n != self.stands.len() || n == 0 {
            return None;
        }
        type Changed = Option<Vec<(usize, Laid, Vec<LayerDisc>)>>;
        let parts: Vec<Changed> = {
            let old = &self.stands;
            let jobs: Vec<crate::par::Job<'_, Changed>> = crate::par::ranges(n, threads, STANDS_PER_JOB * 4)
                .into_iter()
                .map(|r| -> crate::par::Job<'_, Changed> {
                    Box::new(move || {
                        let mut changed = Vec::new();
                        let mut buf = Vec::new();
                        for si in r {
                            let (stand, was) = (&fv.stands[si], &old[si]);
                            if was.key != (stand.site, fv.config.face_layer(stand.species)) || was.species != stand.species {
                                return None;
                            }
                            if was.wood == stand.wood.to_bits() && was.stock == stand.layer_stock.map(f64::to_bits) {
                                continue;
                            }
                            let mut discs = Vec::new();
                            let laid = read_stand(fv, si, &mut buf, &mut discs);
                            if discs.len() != was.discs.len() {
                                return None;
                            }
                            changed.push((si, laid, discs));
                        }
                        Some(changed)
                    })
                })
                .collect();
            crate::par::run(threads, jobs)
        };
        // Nothing is written until every chunk said yes: a record must always describe
        // what the grid holds.
        if parts.iter().any(Option::is_none) {
            return None;
        }
        let box_of = |l: &Laid| (i64::from(l.key.0.x), i64::from(l.key.0.z), l.span);
        let mut boxes = Vec::new();
        for (si, mut laid, discs) in parts.into_iter().flatten().flatten() {
            let was = &self.stands[si];
            let slot = was.discs.start as usize..was.discs.end as usize;
            if !same_discs(&self.discs[slot.clone()], &discs) {
                boxes.push(box_of(was));
                boxes.push(box_of(&laid));
            }
            self.discs[slot].copy_from_slice(&discs);
            laid.discs = was.discs.clone();
            self.stands[si] = laid;
        }
        Some(boxes)
    }

    /// The stands re-recorded whole against the old records, chunk by chunk across the
    /// pool (each chunk of the new stands against the old records whose keys fall in its
    /// range); the boxes whose columns moved.
    fn diff_general(&mut self, fv: &FloraView<'_>, threads: usize) -> Vec<(i64, i64, i64)> {
        let box_of = |l: &Laid| (i64::from(l.key.0.x), i64::from(l.key.0.z), l.span);
        if fv.stands.is_empty() {
            let boxes = self.stands.iter().map(box_of).collect();
            self.stands.clear();
            self.discs.clear();
            return boxes;
        }
        let ranges = crate::par::ranges(fv.stands.len(), threads, STANDS_PER_JOB * 4);
        let starts: Vec<usize> = ranges
            .iter()
            .map(|r| {
                if r.start == 0 {
                    return 0;
                }
                let s = &fv.stands[r.start];
                let key = (s.site, fv.config.face_layer(s.species));
                self.stands.partition_point(|l| l.key < key)
            })
            .chain(std::iter::once(self.stands.len()))
            .collect();
        let parts = {
            let (old, old_discs) = (&self.stands, &self.discs);
            let jobs: Vec<crate::par::Job<'_, DiffPart>> = ranges
                .into_iter()
                .enumerate()
                .map(|(j, r)| -> crate::par::Job<'_, DiffPart> {
                    let mine = &old[starts[j]..starts[j + 1]];
                    Box::new(move || diff_chunk(fv, r, mine, old_discs))
                })
                .collect();
            crate::par::run(threads, jobs)
        };
        self.stands.clear();
        self.discs.clear();
        let mut boxes = Vec::new();
        for mut part in parts {
            boxes.append(&mut part.boxes);
            self.append(part);
        }
        boxes
    }

    /// The ground compared in place: `true` when it kept its sites and no pool's cells
    /// moved, its tops then updated where they lie; `false`, nothing touched, otherwise.
    /// Compared in chunks across the pool: the ground is long and its entries wide.
    fn ground_in_place(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>, density: f64, threads: usize) -> bool {
        let n = fv.ground.len();
        if n != self.ground.len() || self.pools.slot.len() != n {
            return false;
        }
        let c = view.config;
        /// `(entry, its organic bits, its pool's new top)`, or `None` for a moved shape.
        type Moves = Option<Vec<(usize, u64, Option<f64>)>>;
        let parts: Vec<Moves> = {
            let (keys, slots, runs) = (&self.ground, &self.pools.slot, &self.pools.runs);
            let jobs: Vec<crate::par::Job<'_, Moves>> = crate::par::ranges(n, threads, GROUND_PER_JOB)
                .into_iter()
                .map(|r| -> crate::par::Job<'_, Moves> {
                    Box::new(move || {
                        let mut moves = Vec::new();
                        for gi in r {
                            let (key, g) = (&keys[gi], &fv.ground[gi]);
                            if key.0 != g.site {
                                return None;
                            }
                            let organic = (g.litter + g.carrion + g.dead_wood).to_bits();
                            if key.1 == organic {
                                continue;
                            }
                            let slot = slots[gi];
                            match (slot != u32::MAX, pool_run(c, g, density)) {
                                (false, None) => moves.push((gi, organic, None)),
                                (true, Some(run)) => {
                                    let was = &runs[slot as usize];
                                    if (was.0, was.1) != (run.0, run.1) {
                                        return None;
                                    }
                                    moves.push((gi, organic, Some(run.2)));
                                }
                                _ => return None,
                            }
                        }
                        Some(moves)
                    })
                })
                .collect();
            crate::par::run(threads, jobs)
        };
        if parts.iter().any(Option::is_none) {
            return false;
        }
        for (gi, organic, top) in parts.into_iter().flatten().flatten() {
            self.ground[gi].1 = organic;
            if let Some(top) = top {
                self.pools.runs[self.pools.slot[gi] as usize].2 = top;
            }
        }
        true
    }

    /// Clear one flavour's bodies.
    fn clear_bodies(&mut self, c: &cubarium_voxel::Config, flavour: Flavour) {
        let bits = flavour.bits();
        let bodies = match flavour {
            Flavour::Flat => &mut self.flat,
            Flavour::Volume => &mut self.volume,
        };
        let slabs = std::mem::take(&mut bodies.slabs);
        for slab in &slabs {
            bodies.apply(&mut self.cells, c, slab, false, bits);
        }
    }

    /// Bring one flavour's bodies up to the animals, as [`Band::lay_bodies`] lays them
    /// over a whole grid: a body whose slab is the one laid stays, and only the others
    /// are taken away and laid again.
    fn lay_bodies(&mut self, view: &VoxelView<'_>, fauna: &crate::FaunaView<'_>, flavour: Flavour) {
        let c = view.config;
        let v = c.voxel_m;
        let bits = flavour.bits();
        let bodies = match flavour {
            Flavour::Flat => &mut self.flat,
            Flavour::Volume => &mut self.volume,
        };
        if bodies.count.len() != self.cells.len() {
            // The grid was laid from zero at a new size: nothing of these bodies is in it.
            bodies.count = vec![0; self.cells.len()];
            bodies.slabs.clear();
        }
        let mut next = std::mem::take(&mut bodies.next);
        next.clear();
        let height = i64::from(c.height);
        for a in fauna.animals {
            let Some((ax, az)) = a.pose.column(v, c.depth) else {
                continue;
            };
            let first = i64::from(a.sense_layer(v)) + 1;
            let geometry = if flavour == Flavour::Volume { crate::body_of(fauna.config, a) } else { None };
            let (radius, tall) = match geometry {
                Some(b) => (
                    b.footprint_radius(),
                    (b.height_m / v).ceil().max(1.0).min(f64::from(c.height)) as i64,
                ),
                None => (0.0, 1),
            };
            let (from, to) = (first, first + tall);
            if !(from..to).any(|y| y > 0 && y < height) {
                continue;
            }
            next.push(Slab {
                id: a.id,
                ax,
                az: i64::from(az),
                x_m: a.pose.x,
                z_m: a.pose.z,
                r2: radius * radius,
                span: (radius / v).ceil() as i64,
                from,
                to,
            });
        }
        // The animals are sorted by id; keep the walk and the lookup sound regardless.
        if !next.is_sorted_by_key(|s| s.id) {
            next.sort_by_key(|s| s.id);
        }
        let old = std::mem::take(&mut bodies.slabs);
        let (mut i, mut j) = (0, 0);
        while i < old.len() || j < next.len() {
            match (old.get(i), next.get(j)) {
                (Some(o), Some(n)) if o.id == n.id => {
                    if !o.same(n) {
                        bodies.shift(&mut self.cells, c, o, n, bits);
                    }
                    i += 1;
                    j += 1;
                }
                (Some(o), n) if n.is_none_or(|n| o.id < n.id) => {
                    bodies.apply(&mut self.cells, c, o, false, bits);
                    i += 1;
                }
                (_, Some(n)) => {
                    bodies.apply(&mut self.cells, c, n, true, bits);
                    j += 1;
                }
                (_, None) => unreachable!("the walk ends when both lists do"),
            }
        }
        bodies.slabs = next;
        bodies.next = old;
    }
}

fn ground_keys(fv: &FloraView<'_>) -> Vec<(Site, u64)> {
    fv.ground
        .iter()
        .map(|g| (g.site, (g.litter + g.carrion + g.dead_wood).to_bits()))
        .collect()
}

/// Lay one disc's cells that fall in a masked column, keeping the first claim, as
/// [`super::for_each_disc_cell`] walks them.
fn lay_disc_in(c: &cubarium_voxel::Config, d: &LayerDisc, mask: &[bool], cells: &mut [u8]) {
    let (width, depth) = (i64::from(c.width), i64::from(c.depth));
    let plane = (width * depth) as usize;
    let (y0, y1) = (d.rows.0.max(1), d.rows.1.min(i64::from(c.height) - 1));
    if y0 > y1 {
        return;
    }
    for dz in -d.span..=d.span {
        let z = d.z + dz;
        if z < 0 || z >= depth {
            continue;
        }
        for dx in -d.span..=d.span {
            if (dx * dx + dz * dz) as f64 > d.r2 {
                continue;
            }
            let col = (z * width + (d.x + dx).rem_euclid(width)) as usize;
            if !mask[col] {
                continue;
            }
            for y in y0..=y1 {
                let b = &mut cells[y as usize * plane + col];
                if *b & ENV_MASK == 0 {
                    *b |= d.code;
                }
            }
        }
    }
}

impl Band<'_> {
    /// [`Band::lay_pools`] from a [`PoolIndex`], column by column; within a column the runs
    /// are in ground order, and only an entry in a column reaches its cells, so this is
    /// the ground-order lay.
    fn lay_pool_runs(&mut self, view: &VoxelView<'_>, pools: &PoolIndex) {
        let c = view.config;
        let plane = self.plane;
        for col in 0..plane {
            for &(from, to, _) in pools.column(col) {
                for y in from..to {
                    if !self.owns[y as usize] {
                        continue;
                    }
                    let b = Band::byte(&mut self.rows, plane, y as usize * plane + col, y);
                    if *b & (ENV_MASK | POOL_BIT) == 0 {
                        *b |= POOL_BIT;
                    }
                }
            }
        }
        let _ = c;
    }
}

/// The live grid as one stage's occupancy reads it.
#[derive(Clone, Copy)]
pub(crate) struct LiveView<'a> {
    grid: &'a LiveGrid,
    flavour: Flavour,
    width: u32,
    depth: u32,
    voxel_m: f64,
}

impl LiveView<'_> {
    #[inline]
    fn byte(&self, cell: usize) -> u8 {
        self.grid.cells.get(cell).copied().unwrap_or(0)
    }

    fn bodies(&self) -> &Bodies {
        match self.flavour {
            Flavour::Flat => &self.grid.flat,
            Flavour::Volume => &self.grid.volume,
        }
    }
}

impl Slab {
    /// Whether `(x, y, z)` (x wrapped) is in this body's slab, by the lay's own test.
    fn contains(&self, x: i64, y: i64, z: i64, width: i64, v: f64) -> bool {
        if y < self.from || y >= self.to {
            return false;
        }
        let dz = z - self.az;
        if dz.abs() > self.span {
            return false;
        }
        let mut dx = (x - self.ax).rem_euclid(width);
        if dx > width / 2 {
            dx -= width;
        }
        if dx.abs() > self.span {
            return false;
        }
        if dx != 0 || dz != 0 {
            let cx = ((self.ax + dx) as f64 + 0.5) * v - self.x_m;
            let cz = ((self.az + dz) as f64 + 0.5) * v - self.z_m;
            if cx * cx + cz * cz > self.r2 {
                return false;
            }
        }
        true
    }
}

impl CellOccupancy for LiveView<'_> {
    #[inline]
    fn any(&self, cell: usize) -> bool {
        let (body, _) = self.flavour.bits();
        self.byte(cell) & (ENV_MASK | POOL_BIT | body) != 0
    }

    #[inline]
    fn other_body(&self, cell: usize, observer_id: u64) -> bool {
        let (body, multi) = self.flavour.bits();
        let b = self.byte(cell);
        if b & body == 0 {
            return false;
        }
        if b & multi != 0 {
            return true;
        }
        // One body here: another's, unless the cell is in the observer's own slab.
        let slabs = &self.bodies().slabs;
        let Ok(k) = slabs.binary_search_by_key(&observer_id, |s| s.id) else {
            return true;
        };
        let (w, d) = (self.width as usize, self.depth as usize);
        let (x, rest) = (cell % w, cell / w);
        let (z, y) = (rest % d, rest / d);
        !slabs[k].contains(x as i64, y as i64, z as i64, i64::from(self.width), self.voxel_m)
    }

    #[inline]
    fn environment(&self, cell: usize) -> Option<Fine> {
        match self.byte(cell) & ENV_MASK {
            0 => None,
            super::ENV_FOLIAGE => Some(Fine::FoliageCrown),
            super::ENV_STRIPPED => Some(Fine::StrippedCrown),
            _ => Some(Fine::Trunk),
        }
    }

    #[inline]
    fn pool_top(&self, cell: usize) -> Option<f64> {
        if self.byte(cell) & POOL_BIT == 0 {
            return None;
        }
        let plane = self.width as usize * self.depth as usize;
        let (col, y) = (cell % plane, (cell / plane) as u32);
        self.grid
            .pools
            .column(col)
            .iter()
            .find(|r| r.0 <= y && y < r.1)
            .map(|r| r.2)
    }
}
