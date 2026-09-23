//! **Phase-one sensing**: the detritus cue field, the chem/light receptors, the material
//! cone, and the shared occupancy query they read (`design/voxel-senses.md` §1, §3, §4;
//! `design/voxel-senses-phase1-plan.md`, "Initial cue field settings").
//!
//! Phase one has exactly **one cue channel**. Its field is a set of support-layer
//! nodes — exposed support faces, keyed by the cell of the solid voxel whose top face they
//! are — connected only to same-height orthogonal neighbours through the open near-surface
//! medium. Roof and floor never share a node, and there is no vertical transport. Source
//! strength is the **actual detritus stock** per site — the litter and the carrion on
//! it (decisions §3) — so a depleted patch stops emitting while its residue decays. The
//! shredder's third food, glowcap cap tissue, is found at the mouth and is not a source
//! ([`DetritusField`]). It was the litter stock alone until 2026-09-22, and on a world
//! holding only litter it still is; the observation channel is `Chem(detritus)` since
//! contract v2 (it was `Chem(litter)` under schema 1), in the same slot.
//!
//! # The update, in one place
//!
//! Every [`UPDATE_TICKS`] ticks (0.5 s at 20 Hz), in this fixed order — the same order in
//! settling and live updates:
//!
//! 1. **Emission**: each node whose site holds detritus gains `min(detritus / M_EMIT, 1) ·
//!    UPDATE_S` cue units.
//! 2. **Decay**: every worked node decays with half-life [`HALF_LIFE_S`].
//! 3. **Diffusion**: a convex nearest-neighbour mix of [`DIFFUSE_FRACTION`]; a missing or
//!    blocked neighbour reflects the local value.
//! 4. **Discard**: values below [`THRESHOLD`] are dropped.
//!
//! Only the active set (nodes holding a value or a source) plus their immediate neighbours
//! is processed — never a per-animal emitter scan. Terrain connectivity is cached and
//! rebuilt only when the world's [`terrain_version`](VoxelView) or shape changes.
//!
//! # Reading it back
//!
//! `Chem` is sampled by **connectivity-aware bilinear interpolation** among the same-layer
//! nodes of the receptor's cell; a corner that is not a support face (a wall, another
//! floor) is excluded and the weights renormalize, so a reading never interpolates through
//! a wall or off one floor onto another. An unsupported receptor is invalid. `Light` is
//! uniform sky illumination × terrain exposure (no canopy, no emission — a glowcap is
//! invisible). The browser's `Cone` runs a fixed ray fan against a shared occupancy map of
//! stands, surface resources and animal bodies, with terrain/water/wood/litter occluding
//! without being exposed as classes.
//!
//! # Occlusion, contract v2 (D3, D4)
//!
//! Terrain stops a ray. **Free water stops it only below its surface**: a cell holding
//! fill `f` is water from its floor up to `f × cell height`, and air above that — no new
//! constant. **A ground pool stops it only below its physical height**, `organic / (cell
//! area × `[`POOL_BULK_DENSITY`]`)` over the face, which may be less than a cell or span
//! several. **The strip's `z` edge is solid**: a ray leaving the `z` range is a hit, not
//! a miss, because a body cannot go there. A ray leaving the top of the world is open sky.
//! The same clauses are [`crate::manifest::OCCLUSION_RULE`], in the digest.
//!
//! A ray is walked **cell by cell** (Amanatides–Woo, [`Dda`]): every cell it passes
//! through, once, in order; a hit is at the ray's entry into the struck cell, and water
//! and a pool are entered where the ray crosses their surface plane
//! ([`crate::manifest::CONE_TRAVERSAL`], in the browser's digest).
//!
//! # Ownership
//!
//! A [`Senses`] is **per-arena state**, owned by the arena that builds it and settled
//! before the first observation. The static tick receives it through
//! [`Fauna::step_with_senses`](crate::Fauna::step_with_senses) (a new method; the live
//! [`Fauna::step`](crate::Fauna::step) signature is untouched and has no senses). There is
//! no per-read energy bill: sensing is computed, not billed.

use rustc_hash::{FxHashMap, FxHashSet};

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FloraView, Ground, Site, StandLayer};

use crate::Pose;
use crate::body::M_EMIT;
use crate::manifest::Manifest;

/// Field updates run every this many ticks (0.5 s at 20 Hz).
pub const UPDATE_TICKS: u64 = 10;
/// Seconds of emission and decay each update covers.
pub const UPDATE_S: f64 = 0.5;
/// Cue half-life, seconds.
pub const HALF_LIFE_S: f64 = 2.0;
/// Convex nearest-neighbour diffusion mix.
pub const DIFFUSE_FRACTION: f64 = 0.4;
/// Discard: values below this are dropped after an update.
pub const THRESHOLD: f64 = 1e-5;
/// Settling budget: at most this many field updates.
pub const SETTLE_MAX_UPDATES: usize = 120;
/// Settling convergence: stop when the max per-node change stays below this for
/// [`SETTLE_STABLE`] consecutive updates.
const SETTLE_EPS: f64 = 1e-4;
const SETTLE_STABLE: u32 = 8;

/// The bulk density of a ground pool of litter, carrion and dead wood, in organic units
/// per cubic metre (D3): a pool of organic `m` on a face of area `A` stands `m / (A ·
/// density)` tall. **An authored placeholder** (`design/backlog.md` §1): at 80 a
/// reference litter tile (`M_EMIT` = 0.05) is a centimetre deep on a 0.25 m face and four
/// on a 0.125 m one, so a ground pool is a film to an eye, not a wall (decisions §6).
pub const POOL_BULK_DENSITY: f64 = 80.0;

fn decay_factor() -> f64 {
    (-(std::f64::consts::LN_2 * UPDATE_S / HALF_LIFE_S)).exp()
}

/// The support-layer adjacency graph for one terrain geometry.
///
/// A node is one exposed support face (the solid voxel whose top face is exposed). Two
/// nodes at the **same height** are neighbours when their columns are orthogonally
/// adjacent — a column with no support face at that height is a barrier, so a wall breaks
/// the layer and a roof never meets the floor beneath it. Pure geometry: rebuilt only when
/// `terrain_version` or the shape changes.
#[derive(Clone, Debug, Default)]
struct Connectivity {
    version: u64,
    width: u32,
    height: u32,
    depth: u32,
    /// Node cells (`Config::index` of the support face's solid voxel), sorted.
    nodes: Vec<usize>,
    /// Node cell -> its same-height orthogonal neighbour cells, sorted.
    neighbors: FxHashMap<usize, Vec<usize>>,
}

impl Connectivity {
    fn current(&self, view: &VoxelView<'_>) -> bool {
        let c = view.config;
        self.version == view.terrain_version
            && self.width == c.width
            && self.height == c.height
            && self.depth == c.depth
    }

    fn build(view: &VoxelView<'_>) -> Connectivity {
        let c = view.config;
        let mut is_node = vec![false; c.cells()];
        let mut nodes: Vec<usize> = Vec::new();
        for z in 0..c.depth {
            for x in 0..c.width as i64 {
                for y in view.supports_in_column(x, z) {
                    let cell = c.index(x, y, z);
                    nodes.push(cell);
                    is_node[cell] = true;
                }
            }
        }
        nodes.sort_unstable();
        let mut neighbors: FxHashMap<usize, Vec<usize>> =
            FxHashMap::with_capacity_and_hasher(nodes.len(), Default::default());
        for &cell in &nodes {
            let (x, y, z) = c.coords(cell);
            let mut nb: Vec<usize> = Vec::with_capacity(4);
            for (dx, dz) in [(-1i64, 0i64), (1i64, 0i64), (0, -1), (0, 1)] {
                let nz = i64::from(z) + dz;
                if nz < 0 || nz >= i64::from(c.depth) {
                    continue;
                }
                let ncell = c.index(i64::from(x) + dx, y, nz as u32);
                if is_node[ncell] {
                    nb.push(ncell);
                }
            }
            nb.sort_unstable();
            neighbors.insert(cell, nb);
        }
        Connectivity {
            version: view.terrain_version,
            width: c.width,
            height: c.height,
            depth: c.depth,
            nodes,
            neighbors,
        }
    }

    fn has(&self, cell: usize) -> bool {
        self.neighbors.contains_key(&cell)
    }
}

/// The **detritus** cue field: one value per support-layer node.
///
/// Named for what it carries since decisions §3: the source at a face is the **litter
/// and carrion** on it — matter that decays and smells — with the litter field's own
/// emission curve, transport, decay and threshold
/// (`design/handoffs/voxel-diets-2026-09-22.md`). On a world holding only litter it is
/// the litter field it has always been, value for value.
///
/// **Glowcap cap tissue is a shredder's food and is not one of this field's sources**
/// (decisions §3, amended 2026-09-22 on the measurement below): a cap is *found*, by
/// contact and taste at the mouth, and not *smelled*. It was a source for one build and
/// the measurement rejected it — emission is `min(stock / M_EMIT, 1)` with `M_EMIT`
/// 0.05, so on `default` each of the nine caps (0.106) saturated exactly as each of the
/// eight litter tiles (0.2) does, nearly tripling the source count with a food a
/// shredder empties in a bite; the lineage went extinct at about minute 300 against 10
/// alive at six hours without it, and 17 shredders were alive at 60 minutes with the
/// caps out of the cue against 4 with them in
/// (`design/7_Research/voxel-census-2026-09-20.md`, "Diets, 2026-09-22"). Weighting a
/// source by its class was rejected as a knob.
///
/// The observation channel it feeds is `Chem(detritus)` (contract v2; `Chem(litter)`
/// under schema 1), in the same slot and width.
#[derive(Clone, Debug, Default)]
struct DetritusField {
    /// Cue units per node cell.
    value: FxHashMap<usize, f64>,
    graph: Connectivity,
}

/// The detritus stock at one support face — its litter plus its carrion — and
/// therefore the emitter strength.
///
/// Two things a shredder can reach are deliberately **not** in it. A glowcap cap is its
/// food and is found at the mouth rather than smelled across a floor
/// ([`DetritusField`]); dead wood is a glowcap's substrate and not a shredder's food at
/// all, so a cue over a log would send the animal to something it cannot eat.
fn detritus_at(fv: &FloraView<'_>, site: Site) -> f64 {
    fv.ground_at(site).map_or(0.0, |g| g.litter + g.carrion)
}

impl DetritusField {
    fn ensure_graph(&mut self, view: &VoxelView<'_>) {
        if !self.graph.current(view) {
            let old_nodes: FxHashSet<usize> = self.graph.nodes.iter().copied().collect();
            self.graph = Connectivity::build(view);
            // Terrain changed: drop values for nodes that no longer exist and keep the
            // rest. The fresh graph is what defines the nodes from here on.
            self.value.retain(|cell, _| self.graph.has(*cell));
            let _ = old_nodes;
        }
    }

    /// The cue concentration at the receptor, interpolated among the connected same-layer
    /// nodes of its cell. `None` when the pose is off the strip, unsupported, or no corner
    /// is a node (e.g. standing inside a solid — a wall — which is out of medium).
    fn sample(&self, view: &VoxelView<'_>, pose: &Pose, standing_y: u32) -> Option<f64> {
        let c = view.config;
        let (cx, cz) = pose.column(c.voxel_m, c.depth)?;
        let wx = cx.rem_euclid(i64::from(c.width));
        if !view.is_support(wx, standing_y, cz) {
            return None;
        }
        if !self.graph.current(view) || !self.graph.has(c.index(wx, standing_y, cz)) {
            return None;
        }
        // A node's value sits at its face centre, so the bilinear stencil is centred on
        // `pose / v - 0.5`: a receptor at a face centre reads that node alone, and one
        // between two centres blends them. Flooring the raw pose put the stencil's corner
        // on the receptor and shifted the whole sampled field half a voxel (P2-T finding).
        let v = c.voxel_m;
        let xf = pose.x / v - 0.5;
        let zf = pose.z / v - 0.5;
        let x0 = xf.floor() as i64;
        let z0 = zf.floor() as i64;
        let (dx, dz) = (xf - x0 as f64, zf - z0 as f64);
        let mut sum = 0.0;
        let mut wsum = 0.0;
        for (odx, odz, w) in [
            (0i64, 0i64, (1.0 - dx) * (1.0 - dz)),
            (1, 0, dx * (1.0 - dz)),
            (0, 1, (1.0 - dx) * dz),
            (1, 1, dx * dz),
        ] {
            let z = z0 + odz;
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            let cell = c.index(x0 + odx, standing_y, z as u32);
            if !self.graph.has(cell) {
                continue;
            }
            if let Some(&value) = self.value.get(&cell) {
                sum += w * value;
            }
            wsum += w;
        }
        if wsum <= 0.0 { None } else { Some(sum / wsum) }
    }

    /// One field update. Returns the largest per-node absolute change (the settling
    /// criterion).
    fn update(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>) -> f64 {
        self.ensure_graph(view);
        let c = view.config;
        let graph = &self.graph;

        // Sources: every site holding detritus — litter or carrion on the ground.
        // Deterministic (the flora's ground is site-sorted). A fungal cap standing on a
        // face is food and not a source ([`detritus_at`]).
        let mut sources: Vec<usize> = Vec::new();
        for g in fv
            .ground
            .iter()
            .filter(|g| g.litter > 0.0 || g.carrion > 0.0)
        {
            let cell = c.index(g.site.x as i64, g.site.y, g.site.z);
            if graph.has(cell) {
                sources.push(cell);
            }
        }
        sources.sort_unstable();
        sources.dedup();

        // Working set: active (nodes holding a value) + sources + exactly their
        // immediate same-height neighbours. Deduplicate the roots before traversing
        // them: walking a growing work list would follow the cyclic graph transitively
        // and never terminate.
        let mut work: Vec<usize> = self.value.keys().copied().collect();
        work.extend(sources.iter().copied());
        work.sort_unstable();
        work.dedup();
        let roots = work.clone();
        for cell in roots {
            if let Some(nb) = graph.neighbors.get(&cell) {
                work.extend_from_slice(nb);
            }
        }
        work.sort_unstable();
        work.dedup();

        // Order 1+2: emission then decay, into a scratch prime map.
        let decay = decay_factor();
        let mut prime: Vec<(usize, f64)> = Vec::with_capacity(work.len());
        for &cell in &work {
            let old = self.value.get(&cell).copied().unwrap_or(0.0);
            let emit = if sources.binary_search(&cell).is_ok() {
                let (x, y, z) = c.coords(cell);
                let stock = detritus_at(fv, Site { x: x as u32, y, z });
                (stock / M_EMIT).min(1.0) * UPDATE_S
            } else {
                0.0
            };
            prime.push((cell, (old + emit) * decay));
        }
        let by_cell: FxHashMap<usize, f64> = prime.iter().copied().collect();

        // Order 3: convex nearest-neighbour diffusion. A missing/blocked neighbour
        // (a direction with no node) reflects the local value, so the mix is always over
        // four directions.
        let mut max_change = 0.0f64;
        let mut next: FxHashMap<usize, f64> =
            FxHashMap::with_capacity_and_hasher(work.len(), Default::default());
        for &(cell, p) in &prime {
            let (x, y, z) = c.coords(cell);
            let mut total = 0.0;
            for (dx, dz) in [(-1i64, 0i64), (1i64, 0i64), (0, -1), (0, 1)] {
                let nz = i64::from(z) + dz;
                let nb_value = if nz < 0 || nz >= i64::from(c.depth) {
                    p // the world's non-wrapping z boundary reflects
                } else {
                    let ncell = c.index(i64::from(x) + dx, y, nz as u32);
                    if graph
                        .neighbors
                        .get(&cell)
                        .is_some_and(|nb| nb.contains(&ncell))
                    {
                        // A connected but inactive node has concentration zero. It is
                        // deliberately outside `by_cell` when it lies beyond the one-hop
                        // working fringe, and must not be mistaken for a blocked edge.
                        by_cell.get(&ncell).copied().unwrap_or(0.0)
                    } else {
                        p // blocked/missing support reflects the local value
                    }
                };
                total += nb_value;
            }
            let mean = total / 4.0;
            let updated = (1.0 - DIFFUSE_FRACTION) * p + DIFFUSE_FRACTION * mean;
            let old = self.value.get(&cell).copied().unwrap_or(0.0);
            max_change = max_change.max((updated - old).abs());
            // Order 4: discard below the threshold.
            if updated >= THRESHOLD {
                next.insert(cell, updated);
            }
        }
        self.value = next;
        max_change
    }
}

/// One body's chem trend state: the smoothed encoded response.
#[derive(Clone, Copy, Debug, Default)]
struct ChemTrend {
    smoothed: f64,
}

/// Per-arena sensory state: the settled detritus field plus the per-body trend stores, which
/// are episode-private (a fresh arena starts a fresh `Senses`).
#[derive(Clone, Debug, Default)]
pub struct Senses {
    field: DetritusField,
    chem_trend: FxHashMap<u64, ChemTrend>,
    /// The static episode's held cone occupancy ([`Senses::hold_cone`]); `None` builds
    /// one per controller stage.
    cone: Option<Box<HeldCone>>,
    /// The static episode's light memo ([`Senses::hold_light`]); `None` computes the
    /// sky fan at every observation.
    light: Option<Box<LightMemo>>,
}

impl Senses {
    /// An empty, unsettled senses handle.
    pub fn new() -> Senses {
        Senses::default()
    }

    /// Empty the per-body trend stores. They are **episode-private** state (a trend
    /// history is a controller's memory, not the field's), so a driver reusing a settled
    /// field for an unchanged source layout calls this on its `clone()` before the first
    /// observation — otherwise a new episode's animal ids could pick up a dead episode's
    /// trend and read a false gradient.
    pub fn reset_trends(&mut self) {
        self.chem_trend.clear();
    }

    /// Hold the browser cone's occupancy across controller stages instead of building
    /// it per stage: the **static** schedule's choice, where terrain, water and growth are
    /// frozen and only bodies, bites and deaths move ([`HeldCone`]). The held grid always
    /// equals a fresh build, so this changes no reading; it is a cost choice, and the
    /// live world, whose plants grow every tick, does not make it. A clone carries the
    /// grid it holds.
    pub fn hold_cone(&mut self) {
        if self.cone.is_none() {
            self.cone = Some(Box::default());
        }
    }

    /// Memo the `Light` receptor for the episode: the static schedule's choice. The
    /// reading is [`VoxelView::sky_visibility`] of the standing face, which is pure
    /// terrain geometry — no water, no plants, no bodies — so on a frozen world it is a
    /// function of the face alone. The memo keys on the world's `terrain_version` and
    /// shape and recomputes when either moves, so it always returns the direct call's
    /// value, bit for bit.
    pub fn hold_light(&mut self) {
        if self.light.is_none() {
            self.light = Some(Box::default());
        }
    }

    /// Stop holding the cone occupancy: every controller stage builds its own window
    /// again (a schedule whose plants grow).
    pub fn release_cone(&mut self) {
        self.cone = None;
    }

    /// Stop memoing the light receptor.
    pub fn release_light(&mut self) {
        self.light = None;
    }

    /// The standing face's sky visibility through the held memo, or `None` when this
    /// senses handle does not hold one.
    pub(crate) fn held_sky(&mut self, view: &VoxelView<'_>, x: i64, y: u32, z: u32) -> Option<f64> {
        self.light.as_deref_mut().map(|m| m.sky(view, x, y, z))
    }

    /// Lend the held occupancy out for a controller stage (see [`Senses::return_cone`]).
    pub(crate) fn take_cone(&mut self) -> Option<Box<HeldCone>> {
        self.cone.take()
    }

    pub(crate) fn return_cone(&mut self, cone: Option<Box<HeldCone>>) {
        if cone.is_some() {
            self.cone = cone;
        }
    }

    /// One field update (`view` + current flora as read, `dt = UPDATE_S`).
    pub(crate) fn update(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>) {
        self.field.update(view, fv);
    }

    /// Settle the field against a layout: at most [`SETTLE_MAX_UPDATES`] updates, stopping
    /// early once the max per-node change stays below [`SETTLE_EPS`] for
    /// [`SETTLE_STABLE`] consecutive updates. Returns `(updates run, converged)`; the
    /// caller charges the setup to the benchmark and reuses the result only for unchanged
    /// source layouts.
    pub fn settle(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>) -> (usize, bool) {
        let mut stable = 0u32;
        for updates in 1..=SETTLE_MAX_UPDATES {
            let change = self.field.update(view, fv);
            if change < SETTLE_EPS {
                stable += 1;
                if stable >= SETTLE_STABLE {
                    return (updates, true);
                }
            } else {
                stable = 0;
            }
        }
        (SETTLE_MAX_UPDATES, false)
    }

    /// The encoded cue concentration at a receptor, or `None` when the receptor is
    /// unsupported or out of the connected medium.
    pub(crate) fn sample_cue(
        &self,
        view: &VoxelView<'_>,
        pose: &Pose,
        standing_y: u32,
    ) -> Option<f64> {
        self.field.sample(view, pose, standing_y)
    }

    /// Advance a body's trend state and return the trend channel for this sample. The
    /// first sample of a body (a birth, or the start of a fresh episode) initializes the
    /// history from itself and emits **zero** — no false gradient.
    pub(crate) fn advance_chem_trend(
        &mut self,
        id: u64,
        response: f64,
        manifest: &Manifest,
    ) -> f64 {
        let dt = manifest.controller_period_s.max(1e-9);
        let tau = manifest.tunings.chem_tau_s.max(1e-9);
        let scale = manifest.tunings.trend_scale_s;
        let entry = self
            .chem_trend
            .entry(id)
            .or_insert_with(|| ChemTrend { smoothed: response });
        let prev = entry.smoothed;
        let alpha = 1.0 - (-(dt / tau)).exp();
        let s = prev + alpha * (response - prev);
        if !s.is_finite() {
            return 0.0;
        }
        entry.smoothed = s;
        let rate = (s - prev) / dt;
        (rate * scale).clamp(-1.0, 1.0)
    }
}

/// What the cone's first hit is. Terrain and water are read off the world directly;
/// everything else comes from the occupancy map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    /// A stand's live crown cell — exposed as `foliage`.
    Foliage,
    /// An animal body's occupied cell — exposed as `body`.
    Body,
    /// Wood, litter/carrion/dead-wood pools, terrain, water: they occlude but are not
    /// exposed as a class.
    Occluder,
}

/// What the ray actually struck, before [`Class`] collapses four of these into one
/// `Occluder`. Diagnosis only: the policy never sees a `Fine`, and
/// [`Fine::coarse`] is the single place the two are related, so a cone reading cannot
/// drift from the census that explains it.
///
/// It exists because a blank cone has four unrelated causes — a slope, standing water or
/// a shower film, a crown someone has already stripped, and a ground pool of
/// litter/carrion/dead wood — and `Class::Occluder` cannot tell them apart
/// (`design/handoffs/voxel-cone-autopsy-2026-09-21.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fine {
    /// A solid voxel: terrain, which on a landscape is usually the slope ahead.
    Terrain,
    /// A cell holding free water: the lake, a puddle, or a film left by a shower.
    Water,
    /// A stand's crown cell whose `foliage` is zero — grazed bare, still occluding.
    StrippedCrown,
    /// A stand's **trunk** cell: structure, never food, always an occluder. Its own
    /// name because a blank cone with a stem in it is a different fact from a blank
    /// cone with a stripped crown in it, and since plants have layers a trunk is in
    /// the occupancy map at all.
    Trunk,
    /// A stand's crown cell with foliage standing in it.
    FoliageCrown,
    /// A ground pool of litter, carrion or dead wood, below its physical height.
    GroundPool,
    /// Another animal's body cell.
    Body,
}

impl Fine {
    /// The class the policy's [`SectorReading`] is built from. **The only mapping**:
    /// every coarse reading in the crate comes through here.
    pub(crate) const fn coarse(self) -> Class {
        match self {
            Fine::FoliageCrown => Class::Foliage,
            Fine::Body => Class::Body,
            Fine::Terrain | Fine::Water | Fine::StrippedCrown | Fine::Trunk | Fine::GroundPool => {
                Class::Occluder
            }
        }
    }
}

/// One cone sector's accumulated reading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SectorReading {
    pub clear: f64,
    pub all_proximity: f64,
    pub foliage_fraction: f64,
    pub foliage_proximity: f64,
    pub body_fraction: f64,
    pub body_proximity: f64,
}

impl SectorReading {
    const ZERO: SectorReading = SectorReading {
        clear: 0.0,
        all_proximity: 0.0,
        foliage_fraction: 0.0,
        foliage_proximity: 0.0,
        body_fraction: 0.0,
        body_proximity: 0.0,
    };
}

/// The three-sector cone reading, in manifest order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ConeReading {
    pub valid: bool,
    pub sectors: [SectorReading; 3],
}

/// What the march asks of an occupancy, one cell at a time: another body than the
/// observer's, a stand's class, a ground pool's top. [`ConeOccupancy`] is the live one;
/// the march is generic so a test can run the very same march against a reference.
pub(crate) trait CellOccupancy {
    /// Whether anything at all may be in `cell` — a cheap pre-check the march may skip
    /// the three queries on. `true` is always a safe answer.
    fn any(&self, _cell: usize) -> bool {
        true
    }
    /// A body other than `observer_id` occupies `cell`.
    fn other_body(&self, cell: usize, observer_id: u64) -> bool;
    /// The stand class claiming `cell`, if any.
    fn environment(&self, cell: usize) -> Option<Fine>;
    /// The top, in metres, of the ground pool reaching into `cell`, if any.
    fn pool_top(&self, cell: usize) -> Option<f64>;
}

/// The two low bits of a [`ConeOccupancy`] cell: which stand class claims it.
const ENV_MASK: u8 = 0b0011;
const ENV_FOLIAGE: u8 = 1;
const ENV_STRIPPED: u8 = 2;
const ENV_TRUNK: u8 = 3;
/// A ground pool reaches into the cell; its top is in [`ConeOccupancy::pools`].
const POOL_BIT: u8 = 0b0100;
/// At least one body stands in the cell; which ones are in [`ConeOccupancy::bodies`].
const BODY_BIT: u8 = 0b1000;

const fn env_code(fine: Fine) -> u8 {
    match fine {
        Fine::FoliageCrown => ENV_FOLIAGE,
        Fine::StrippedCrown => ENV_STRIPPED,
        _ => ENV_TRUNK,
    }
}

/// Occupancy shared by every cone sampled in one controller stage. Environment geometry
/// and bodies are indexed once; a ray ignores only its observer's id, so another body in
/// the same cell remains visible.
///
/// **Dense**: one byte per world cell ([`cubarium_voxel::Config::index`]) holds the stand
/// class claiming it, whether a ground pool reaches into it, and whether a body stands in
/// it; the pools' tops and the bodies' ids sit in short side lists sorted by cell, read
/// only when the byte says so. A ray step is one byte load (cone-speed item 2; it was
/// three SipHash map lookups).
///
/// A **windowed** build ([`cone_occupancy_window`]) indexes only the columns a set of
/// observers' rays can reach; a ray from one of those observers reads exactly what it
/// would read against the full build, because nothing outside the window is on its path.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ConeOccupancy {
    cells: Vec<u8>,
    /// `(cell, top in metres)` for every cell carrying [`POOL_BIT`], sorted by cell.
    pools: Vec<(u32, f64)>,
    /// `(cell, body id)` for every indexed body, sorted by cell, in animal order within
    /// a cell.
    bodies: Vec<(u32, u64)>,
}

impl CellOccupancy for ConeOccupancy {
    #[inline]
    fn any(&self, cell: usize) -> bool {
        self.byte(cell) != 0
    }

    #[inline]
    fn other_body(&self, cell: usize, observer_id: u64) -> bool {
        if self.byte(cell) & BODY_BIT == 0 {
            return false;
        }
        let cell = cell as u32;
        let from = self.bodies.partition_point(|&(c, _)| c < cell);
        self.bodies[from..]
            .iter()
            .take_while(|&&(c, _)| c == cell)
            .any(|&(_, id)| id != observer_id)
    }

    #[inline]
    fn environment(&self, cell: usize) -> Option<Fine> {
        match self.byte(cell) & ENV_MASK {
            0 => None,
            ENV_FOLIAGE => Some(Fine::FoliageCrown),
            ENV_STRIPPED => Some(Fine::StrippedCrown),
            _ => Some(Fine::Trunk),
        }
    }

    #[inline]
    fn pool_top(&self, cell: usize) -> Option<f64> {
        if self.byte(cell) & POOL_BIT == 0 {
            return None;
        }
        let cell = cell as u32;
        self.pools
            .binary_search_by_key(&cell, |&(c, _)| c)
            .ok()
            .map(|i| self.pools[i].1)
    }
}

impl ConeOccupancy {
    fn empty(cells: usize) -> ConeOccupancy {
        ConeOccupancy {
            cells: vec![0; cells],
            pools: Vec::new(),
            bodies: Vec::new(),
        }
    }

    #[inline]
    fn byte(&self, cell: usize) -> u8 {
        self.cells.get(cell).copied().unwrap_or(0)
    }

    /// Every layer of every profile, not one disc per stand: a foliage-bearing layer
    /// holding stock is foliage to an eye, one holding none is a stripped crown, and a
    /// trunk is structure. **Porosity does not enter here**: it is a light property, and
    /// a porous canopy is still leaves to an eye — a known simplification, recorded in
    /// `design/voxel-encounter-contract-2026-09-21.md` §8.
    ///
    /// Foliage is laid down **before** structure, over both passes, so that where a
    /// rosette wraps its own stem the eye reads the food and not the stick. The first
    /// claim on a cell keeps it, as it always has. `claim` sees every foliage claim as
    /// `(cell, stand index, layer position)` — the held occupancy's record of who owns a
    /// crown cell, which a bite can re-class but never move.
    fn lay_stands(
        &mut self,
        view: &VoxelView<'_>,
        fv: &FloraView<'_>,
        window: Option<&[bool]>,
        mut claim: impl FnMut(usize, usize, usize),
    ) {
        for foliage_pass in [true, false] {
            for (si, stand) in fv.stands.iter().enumerate() {
                for (li, layer) in fv.profile_layers(stand).iter().enumerate() {
                    if layer.kind.bears_foliage() != foliage_pass {
                        continue;
                    }
                    let code = env_code(layer_class(layer));
                    for_each_layer_cell(view, stand.site, layer, window, |cell| {
                        let b = &mut self.cells[cell];
                        if *b & ENV_MASK == 0 {
                            *b |= code;
                            if foliage_pass {
                                claim(cell, si, li);
                            }
                        }
                    });
                }
            }
        }
    }

    /// Ground pools occlude up to their physical height over the face (D3): organic /
    /// (cell area × bulk density), from the face's top — a film under a cell, or a heap
    /// spanning several. A stand's claim on a cell comes first, and the first pool (in
    /// site order) to reach a cell keeps it.
    fn lay_pools<'g>(
        &mut self,
        view: &VoxelView<'_>,
        ground: impl IntoIterator<Item = &'g Ground>,
        window: Option<&[bool]>,
    ) {
        let c = view.config;
        for g in ground {
            let Some(top) = pool_top_m(g, c.voxel_m) else {
                continue;
            };
            if !column_seen(c, window, i64::from(g.site.x)) {
                continue;
            }
            let mut y = g.site.y + 1;
            while y < c.height && f64::from(y) * c.voxel_m < top {
                let cell = c.index(i64::from(g.site.x), y, g.site.z);
                let b = &mut self.cells[cell];
                if *b & (ENV_MASK | POOL_BIT) == 0 {
                    *b |= POOL_BIT;
                    self.pools.push((cell as u32, top));
                }
                y += 1;
            }
        }
    }

    /// Clear every body's bit and forget the list, for a re-index.
    fn clear_bodies(&mut self) {
        for &(cell, _) in &self.bodies {
            self.cells[cell as usize] &= !BODY_BIT;
        }
        self.bodies.clear();
    }

    /// Every body's occupied cell (the layer over its standing face), in animal order.
    fn lay_bodies(
        &mut self,
        view: &VoxelView<'_>,
        fauna: &crate::FaunaView<'_>,
        window: Option<&[bool]>,
    ) {
        let c = view.config;
        for a in fauna.animals {
            if let Some((ax, az)) = a.pose.column(c.voxel_m, c.depth) {
                if !column_seen(c, window, ax) {
                    continue;
                }
                let layer = i64::from(a.site.y) + 1;
                if layer > 0 && layer < i64::from(c.height) {
                    let cell = c.index(ax, layer as u32, az);
                    self.cells[cell] |= BODY_BIT;
                    self.bodies.push((cell as u32, a.id));
                }
            }
        }
        // Stable: bodies sharing a cell stay in animal order.
        self.bodies.sort_by_key(|&(cell, _)| cell);
    }
}

/// The stand class a profile layer shows an eye.
fn layer_class(layer: &StandLayer) -> Fine {
    if !layer.kind.bears_foliage() {
        Fine::Trunk
    } else if layer.stock > 0.0 {
        Fine::FoliageCrown
    } else {
        Fine::StrippedCrown
    }
}

/// A ground entry's pool top in metres, or `None` when it holds no organic matter.
fn pool_top_m(g: &Ground, voxel_m: f64) -> Option<f64> {
    let organic = g.litter + g.carrion + g.dead_wood;
    (organic > 0.0).then(|| {
        crate::surface_m(g.site.y, voxel_m) + organic / (voxel_m * voxel_m * POOL_BULK_DENSITY)
    })
}

fn column_seen(c: &cubarium_voxel::Config, window: Option<&[bool]>, cx: i64) -> bool {
    window.is_none_or(|w| w[cx.rem_euclid(i64::from(c.width)) as usize])
}

/// Every cell a profile layer's disc covers — its rows, the integer disc of its radius
/// around the stand's column, clipped to the strip's `z` range and the window.
fn for_each_layer_cell(
    view: &VoxelView<'_>,
    site: Site,
    layer: &StandLayer,
    window: Option<&[bool]>,
    mut f: impl FnMut(usize),
) {
    let c = view.config;
    let span = layer.radius_v.ceil() as i64;
    let r2 = layer.radius_v * layer.radius_v;
    let root = i64::from(site.x);
    if window.is_some() && !(root - span..=root + span).any(|cx| column_seen(c, window, cx)) {
        return;
    }
    for cell_y in layer.cells.0..=layer.cells.1 {
        if cell_y <= 0 || cell_y as u32 >= c.height {
            continue;
        }
        for dz in -span..=span {
            for dx in -span..=span {
                if (dx * dx + dz * dz) as f64 > r2 {
                    continue;
                }
                let z = i64::from(site.z) + dz;
                if z < 0 || z >= i64::from(c.depth) {
                    continue;
                }
                let cx = (root + dx).rem_euclid(i64::from(c.width));
                if !column_seen(c, window, cx) {
                    continue;
                }
                f(c.index(cx, cell_y as u32, z as u32));
            }
        }
    }
}

pub(crate) fn cone_occupancy(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    fauna: &crate::FaunaView<'_>,
) -> ConeOccupancy {
    cone_occupancy_with(view, fv, fauna, true)
}

/// [`cone_occupancy`] with the ground-pool occluder made optional.
///
/// The live cone always passes `true`: a litter, carrion or dead-wood pool occludes up
/// to its physical height (D3). `false` leaves pools out altogether, for a diagnostic
/// arm that asks what the eye would see with no pools at all.
pub(crate) fn cone_occupancy_with(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    fauna: &crate::FaunaView<'_>,
    pools_occlude: bool,
) -> ConeOccupancy {
    build_occupancy(view, fv, fauna, pools_occlude, None)
}

/// The live occupancy for **one controller stage**: indexed only over the columns the
/// observers at `observer_x_m` can see within `range_m`, plus a column of margin either
/// side (P5-A item 7). A ray from one of those observers never leaves that window
/// horizontally, so it reads exactly what the full build would give it; the cost is
/// proportional to the observers' share of the ring instead of to the whole world.
pub(crate) fn cone_occupancy_window(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    fauna: &crate::FaunaView<'_>,
    observer_x_m: impl IntoIterator<Item = f64>,
    range_m: f64,
) -> ConeOccupancy {
    let c = view.config;
    let width = i64::from(c.width);
    let mut window = vec![false; c.width as usize];
    for x in observer_x_m {
        if !x.is_finite() || !range_m.is_finite() {
            // Nothing sane to bound: fall back to the whole ring.
            return build_occupancy(view, fv, fauna, true, None);
        }
        let lo = ((x - range_m) / c.voxel_m).floor() as i64 - 1;
        let hi = ((x + range_m) / c.voxel_m).floor() as i64 + 1;
        if hi - lo + 1 >= width {
            return build_occupancy(view, fv, fauna, true, None);
        }
        for cx in lo..=hi {
            window[cx.rem_euclid(width) as usize] = true;
        }
    }
    build_occupancy(view, fv, fauna, true, Some(&window))
}

/// The one occupancy builder: every stand layer, ground pool and body, restricted to the
/// columns `window` marks when there is one.
pub(crate) fn build_occupancy(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    fauna: &crate::FaunaView<'_>,
    pools_occlude: bool,
    window: Option<&[bool]>,
) -> ConeOccupancy {
    let mut occ = ConeOccupancy::empty(view.config.cells());
    occ.lay_stands(view, fv, window, |_, _, _| {});
    if pools_occlude {
        occ.lay_pools(view, fv.ground.iter(), window);
        occ.pools.sort_unstable_by_key(|&(cell, _)| cell);
    }
    occ.lay_bodies(view, fauna, window);
    occ
}

/// The `Light` receptor's memo for a frozen world (cone-speed follow-up): one entry per
/// `(x, z)` column holding the last standing face's `y` and its sky visibility. A body
/// standing on another face of the same column (a ledge over a floor) recomputes and
/// replaces the entry — still the direct call's value. Dense, no hashing.
#[derive(Clone, Debug, Default)]
pub(crate) struct LightMemo {
    shape: (u64, u32, u32, u32),
    /// `(face y, sky)` per column `z * width + x`; `y == u32::MAX` is empty.
    columns: Vec<(u32, f64)>,
}

impl LightMemo {
    pub(crate) fn sky(&mut self, view: &VoxelView<'_>, x: i64, y: u32, z: u32) -> f64 {
        let c = view.config;
        let shape = (view.terrain_version, c.width, c.height, c.depth);
        let n = c.width as usize * c.depth as usize;
        if self.shape != shape || self.columns.len() != n {
            self.shape = shape;
            self.columns.clear();
            self.columns.resize(n, (u32::MAX, 0.0));
        }
        if z >= c.depth {
            return view.sky_visibility(x, y, z);
        }
        let wx = x.rem_euclid(i64::from(c.width)) as usize;
        let entry = &mut self.columns[z as usize * c.width as usize + wx];
        if entry.0 != y {
            *entry = (y, view.sky_visibility(x, y, z));
        }
        entry.1
    }
}

/// The cone occupancy **held across controller stages** of a static episode (cone-speed
/// item 4). In a static episode the terrain, the water and the plants' growth are
/// frozen; what changes is bodies (every tick) and what they eat or leave behind (a bite
/// strips a crown or shrinks a pool; a death lays carrion). So the grid is built over
/// the whole world once, and each stage then
///
/// - re-indexes the bodies,
/// - re-classes the crown cells of any stand whose layers' stocked/bare pattern moved —
///   a bite can turn a crown cell bare but never moves it, so the cell's claimant (which
///   stand and layer laid it, recorded at the build) is unchanged, and
/// - re-lays the pool cells of any column whose ground entries changed.
///
/// What changed is found by **comparing** the plant layer with the snapshot the grid was
/// built from — every stand's site, species, wood and stocked-layer mask, every ground
/// entry's organic total — not by trusting the caller to report its bites: a training
/// world holds tens of stands and ground entries, so the comparison is cheap, and any
/// change it cannot patch (a stand added, removed or grown; the terrain or the shape
/// moved) is a full rebuild. A held grid therefore always equals a fresh build; holding
/// it is only a question of cost, which is why the live world (whose plants grow every
/// tick) does not.
#[derive(Clone, Debug, Default)]
pub(crate) struct HeldCone {
    occ: ConeOccupancy,
    built: bool,
    shape: (u64, u32, u32, u32, u64),
    stands: Vec<HeldStand>,
    /// `(site, organic bits)` for every ground entry, in the plant layer's site order.
    ground: Vec<(Site, u64)>,
    /// Per cell: `0`, or `1 + (stand index << 8 | layer position)` of the foliage layer
    /// that claimed it.
    claims: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HeldStand {
    site: Site,
    species: cubarium_voxel_flora::Species,
    wood: u64,
    /// Bit `i`: foliage layer `i` holds stock.
    stocked: u8,
}

impl HeldStand {
    fn of(stand: &cubarium_voxel_flora::Stand) -> HeldStand {
        let mut stocked = 0u8;
        for (i, &s) in stand.layer_stock.iter().enumerate() {
            if s > 0.0 {
                stocked |= 1 << i;
            }
        }
        HeldStand {
            site: stand.site,
            species: stand.species,
            wood: stand.wood.to_bits(),
            stocked,
        }
    }

    fn same_geometry(&self, other: &HeldStand) -> bool {
        self.site == other.site && self.species == other.species && self.wood == other.wood
    }
}

fn claim_code(stand: usize, layer: usize) -> u32 {
    1 + ((stand as u32) << 8 | layer as u32)
}

impl HeldCone {
    /// Bring the held grid up to date with the world, the plant layer and the bodies as
    /// they stand, and hand it out. Equal to [`cone_occupancy`] on the same state.
    pub(crate) fn refresh(
        &mut self,
        view: &VoxelView<'_>,
        fv: &FloraView<'_>,
        fauna: &crate::FaunaView<'_>,
    ) -> &ConeOccupancy {
        let c = view.config;
        let shape = (
            view.terrain_version,
            c.width,
            c.height,
            c.depth,
            c.voxel_m.to_bits(),
        );
        let stands: Vec<HeldStand> = fv.stands.iter().map(HeldStand::of).collect();
        let geometry_held = self.built
            && self.shape == shape
            && self.stands.len() == stands.len()
            && self
                .stands
                .iter()
                .zip(&stands)
                .all(|(a, b)| a.same_geometry(b));
        if !geometry_held {
            self.rebuild(view, fv, fauna, shape, stands);
            return &self.occ;
        }
        // Bites: re-class the crown cells of every stand whose stocked mask moved.
        for (si, (old, new)) in self.stands.iter_mut().zip(&stands).enumerate() {
            if old.stocked == new.stocked {
                continue;
            }
            *old = *new;
            let stand = &fv.stands[si];
            for (li, layer) in fv.profile_layers(stand).iter().enumerate() {
                if !layer.kind.bears_foliage() {
                    continue;
                }
                let (code, claim) = (env_code(layer_class(layer)), claim_code(si, li));
                let (cells, claims) = (&mut self.occ.cells, &self.claims);
                for_each_layer_cell(view, stand.site, layer, None, |cell| {
                    if claims[cell] == claim {
                        cells[cell] = (cells[cell] & !ENV_MASK) | code;
                    }
                });
            }
        }
        // Meals and deaths: re-lay the pools of every column whose ground moved.
        let ground: Vec<(Site, u64)> = fv
            .ground
            .iter()
            .map(|g| (g.site, (g.litter + g.carrion + g.dead_wood).to_bits()))
            .collect();
        if ground != self.ground {
            let mut dirty: Vec<(u32, u32)> = Vec::new();
            let (mut i, mut j) = (0, 0);
            while i < self.ground.len() || j < ground.len() {
                let (a, b) = (self.ground.get(i), ground.get(j));
                match (a, b) {
                    (Some(a), Some(b)) if a.0 == b.0 => {
                        if a.1 != b.1 {
                            dirty.push((a.0.x, a.0.z));
                        }
                        i += 1;
                        j += 1;
                    }
                    (Some(a), Some(b)) if a.0 < b.0 => {
                        dirty.push((a.0.x, a.0.z));
                        i += 1;
                    }
                    (Some(_), Some(b)) | (None, Some(b)) => {
                        dirty.push((b.0.x, b.0.z));
                        j += 1;
                    }
                    (Some(a), None) => {
                        dirty.push((a.0.x, a.0.z));
                        i += 1;
                    }
                    (None, None) => unreachable!(),
                }
            }
            dirty.sort_unstable();
            dirty.dedup();
            self.relay_pools(view, fv, &dirty);
            self.ground = ground;
        }
        self.occ.clear_bodies();
        self.occ.lay_bodies(view, fauna, None);
        &self.occ
    }

    fn rebuild(
        &mut self,
        view: &VoxelView<'_>,
        fv: &FloraView<'_>,
        fauna: &crate::FaunaView<'_>,
        shape: (u64, u32, u32, u32, u64),
        stands: Vec<HeldStand>,
    ) {
        let cells = view.config.cells();
        let mut occ = ConeOccupancy::empty(cells);
        let mut claims = std::mem::take(&mut self.claims);
        claims.clear();
        claims.resize(cells, 0);
        occ.lay_stands(view, fv, None, |cell, si, li| {
            claims[cell] = claim_code(si, li);
        });
        occ.lay_pools(view, fv.ground.iter(), None);
        occ.pools.sort_unstable_by_key(|&(cell, _)| cell);
        occ.lay_bodies(view, fauna, None);
        *self = HeldCone {
            occ,
            built: true,
            shape,
            stands,
            ground: fv
                .ground
                .iter()
                .map(|g| (g.site, (g.litter + g.carrion + g.dead_wood).to_bits()))
                .collect(),
            claims,
        };
    }

    /// Clear and re-lay the pool cells of the `(x, z)` columns in `dirty` (sorted), from
    /// every ground entry in those columns in site order — only an entry in a column can
    /// reach a cell of it, so this is exactly what a fresh build lays there.
    fn relay_pools(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>, dirty: &[(u32, u32)]) {
        if dirty.is_empty() {
            return;
        }
        let c = view.config;
        for &(x, z) in dirty {
            for y in 0..c.height {
                self.occ.cells[c.index(i64::from(x), y, z)] &= !POOL_BIT;
            }
        }
        self.occ.pools.retain(|&(cell, _)| {
            let (x, _, z) = c.coords(cell as usize);
            dirty.binary_search(&(x, z)).is_err()
        });
        for &(x, z) in dirty {
            let from = fv.ground.partition_point(|g| g.site.x < x);
            let column = fv.ground[from..]
                .iter()
                .take_while(|g| g.site.x == x)
                .filter(|g| g.site.z == z);
            self.occ.lay_pools(view, column, None);
        }
        self.occ.pools.sort_unstable_by_key(|&(cell, _)| cell);
    }
}

/// March one ray from `origin` (metres) along unit `dir`, returning the first hit's
/// distance and **fine** class within `range`. The traversal is **cell-exact** ([`Dda`]):
/// every cell the ray passes through is visited once, in order, and a hit's distance is
/// the ray's entry into the struck cell — or, for water and a ground pool, its entry
/// into the part below the surface, solved against the plane. A ray that leaves the top
/// or the bottom of the world is a clear ray, and one that leaves the strip's `z` range
/// **hits the edge** as terrain (D4), at the edge.
///
/// The order of the tests below is the occlusion policy: the world's bounds, terrain,
/// free water below its surface (D3), a body, a stand's cell, then a ground pool below
/// its height (D3). Only the returned class is finer — [`Fine::coarse`] turns it back
/// into what the policy reads.
pub(crate) fn ray_first_hit(
    view: &VoxelView<'_>,
    occupancy: &ConeOccupancy,
    observer_id: u64,
    origin: (f64, f64, f64),
    dir: (f64, f64, f64),
    range: f64,
) -> Option<(f64, Fine)> {
    ray_first_hit_cell(view, occupancy, observer_id, origin, dir, range)
        .map(|(distance, fine, _)| (distance, fine))
}

/// [`ray_first_hit`] with the **cell index** it struck, so a diagnostic can say which
/// stand's crown a ray found rather than only that it found foliage. The march is the
/// live one; [`ray_first_hit`] is this with the cell dropped.
pub(crate) fn ray_first_hit_cell(
    view: &VoxelView<'_>,
    occupancy: &ConeOccupancy,
    observer_id: u64,
    origin: (f64, f64, f64),
    dir: (f64, f64, f64),
    range: f64,
) -> Option<(f64, Fine, usize)> {
    ray_first_hit_cell_in(view, occupancy, observer_id, origin, dir, range)
}

/// The march itself, over any [`CellOccupancy`].
pub(crate) fn ray_first_hit_cell_in<O: CellOccupancy>(
    view: &VoxelView<'_>,
    occupancy: &O,
    observer_id: u64,
    origin: (f64, f64, f64),
    dir: (f64, f64, f64),
    range: f64,
) -> Option<(f64, Fine, usize)> {
    let c = view.config;
    let v = c.voxel_m;
    let (width, height, depth) = (i64::from(c.width), i64::from(c.height), i64::from(c.depth));
    for step in Dda::new(v, origin, dir, range) {
        let t = step.t_in;
        if step.y < 0 || step.y >= height {
            return None; // escaped upward or below the world: clear
        }
        let (y, wx) = (step.y as u32, step.x.rem_euclid(width));
        if step.z < 0 || step.z >= depth {
            // The edge of the world is a wall (D4): the cell reported is the last one
            // inside the strip, at the height the ray reached the edge.
            let edge = step.z.clamp(0, depth - 1) as u32;
            return Some((t, Fine::Terrain, c.index(wx, y, edge)));
        }
        let cell = c.index(wx, y, step.z as u32);
        if view.material[cell].is_solid() {
            return Some((t, Fine::Terrain, cell));
        }
        // Water is a surface, not a cell (D3): a cell holding fill `f` is water from its
        // floor to `f × cell height`, and the ray passes through the air above.
        let fill = view.free[cell];
        if fill > 0.0 {
            let surface = (f64::from(y) + fill.min(1.0)) * v;
            if let Some(te) = entry_below(origin.1, dir.1, &step, surface, range) {
                return Some((te, Fine::Water, cell));
            }
        }
        if !occupancy.any(cell) {
            continue;
        }
        if occupancy.other_body(cell, observer_id) {
            return Some((t, Fine::Body, cell));
        }
        if let Some(class) = occupancy.environment(cell) {
            return Some((t, class, cell));
        }
        if let Some(top) = occupancy.pool_top(cell) {
            if let Some(te) = entry_below(origin.1, dir.1, &step, top, range) {
                return Some((te, Fine::GroundPool, cell));
            }
        }
    }
    None
}

/// Where the ray's stretch through `step` first lies **below** the horizontal plane
/// `surface` (metres), within `range`: the cell entry if it is already under it, else
/// the plane crossing on the way down, else nowhere.
#[inline]
fn entry_below(oy: f64, dy: f64, step: &DdaCell, surface: f64, range: f64) -> Option<f64> {
    if oy + dy * step.t_in < surface {
        return Some(step.t_in);
    }
    if dy < 0.0 {
        let ts = (surface - oy) / dy;
        if ts < step.t_out && ts <= range {
            return Some(ts.max(step.t_in));
        }
    }
    None
}

/// One cell a ray passes through: its **unwrapped** coordinates (the march wraps `x` and
/// checks the bounds) and the ray parameter, metres, at which it enters and leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DdaCell {
    pub x: i64,
    pub y: i64,
    pub z: i64,
    pub t_in: f64,
    pub t_out: f64,
}

/// The cell-exact traversal of a ray through the voxel grid (Amanatides–Woo): every
/// cell the ray passes through, once, in order, while its entry is within `range`. The
/// first cell is the one holding the origin, entered at 0 (on a face, the one the ray
/// moves into). Each crossing is solved from the face it crosses, not accumulated, so a
/// distance is the analytic one. A non-finite origin, direction or range yields
/// nothing.
pub(crate) struct Dda {
    cell: [i64; 3],
    step: [i64; 3],
    origin: [f64; 3],
    dir: [f64; 3],
    voxel_m: f64,
    /// The ray parameter at which it crosses the next face on each axis.
    next: [f64; 3],
    /// Entry into the current cell.
    t: f64,
    range: f64,
}

impl Dda {
    pub(crate) fn new(
        voxel_m: f64,
        origin: (f64, f64, f64),
        dir: (f64, f64, f64),
        range: f64,
    ) -> Dda {
        let o = [origin.0, origin.1, origin.2];
        let d = [dir.0, dir.1, dir.2];
        let sane = voxel_m > 0.0
            && voxel_m.is_finite()
            && range.is_finite()
            && o.iter().chain(&d).all(|x| x.is_finite());
        let mut dda = Dda {
            cell: [0; 3],
            step: [0; 3],
            origin: o,
            dir: d,
            voxel_m,
            next: [f64::INFINITY; 3],
            t: if sane { 0.0 } else { f64::INFINITY },
            range: if sane { range } else { f64::NEG_INFINITY },
        };
        if !sane {
            return dda;
        }
        for a in 0..3 {
            let u = o[a] / voxel_m;
            dda.step[a] = if d[a] > 0.0 {
                1
            } else if d[a] < 0.0 {
                -1
            } else {
                0
            };
            // On a face and moving down the axis, the cell is the one below the face.
            dda.cell[a] = if dda.step[a] < 0 {
                u.ceil() as i64 - 1
            } else {
                u.floor() as i64
            };
            dda.next[a] = dda.face(a);
        }
        dda
    }

    /// The ray parameter at the current cell's exit face on axis `a`.
    #[inline]
    fn face(&self, a: usize) -> f64 {
        match self.step[a] {
            0 => f64::INFINITY,
            s => {
                let f = (self.cell[a] + i64::from(s > 0)) as f64 * self.voxel_m;
                (f - self.origin[a]) / self.dir[a]
            }
        }
    }
}

impl Iterator for Dda {
    type Item = DdaCell;

    #[inline]
    fn next(&mut self) -> Option<DdaCell> {
        if !(self.t <= self.range) {
            return None;
        }
        let a = if self.next[0] <= self.next[1] && self.next[0] <= self.next[2] {
            0
        } else if self.next[1] <= self.next[2] {
            1
        } else {
            2
        };
        let out = DdaCell {
            x: self.cell[0],
            y: self.cell[1],
            z: self.cell[2],
            t_in: self.t,
            t_out: self.next[a],
        };
        self.t = self.next[a];
        self.cell[a] += self.step[a];
        self.next[a] = self.face(a);
        Some(out)
    }
}

/// Whether an eye can be sampled from here at all, in [`cone_readings`]' own order:
/// the pose has to be in a column, that column has to be a support face at the standing
/// layer, and the manifest has to declare three sectors. Shared with the diagnostic
/// census so "the cone was invalid" means one thing in this crate.
pub(crate) fn cone_valid(
    view: &VoxelView<'_>,
    pose: &Pose,
    standing_y: u32,
    manifest: &Manifest,
) -> bool {
    let c = view.config;
    let Some((cx, cz)) = pose.column(c.voxel_m, c.depth) else {
        return false;
    };
    let wx = cx.rem_euclid(i64::from(c.width));
    view.is_support(wx, standing_y, cz) && manifest.sector_centres_deg.len() == 3
}

/// Where the eye is, in metres: the body's own column, and `0.8 × body height` over the
/// **standing surface** `(standing_y + 1) · voxel_m` (decisions §6;
/// `design/handoffs/voxel-body-anchors-2026-09-22.md`).
///
/// It used to be one and a half **voxels** over the standing face, so the same browser's
/// eye was twice as high on a 0.25 m world as on a 0.125 m one. It is now a length, and
/// the grid does not appear in it at all. Contract v2 writes the eye fraction into the
/// manifest's digest (D1).
pub(crate) fn cone_origin(
    view: &VoxelView<'_>,
    pose: &Pose,
    standing_y: u32,
    body: &crate::Body,
) -> (f64, f64, f64) {
    (
        pose.x,
        crate::surface_m(standing_y, view.config.voxel_m) + body.eye_m,
        pose.z,
    )
}

/// One ray's unit direction from the heading, its sector centre and its two offsets, all
/// in degrees. The single expression both the reading and the census use.
pub(crate) fn ray_direction(
    heading_rad: f64,
    centre_deg: f64,
    yaw_offset_deg: f64,
    pitch_offset_deg: f64,
) -> (f64, f64, f64) {
    let yaw = heading_rad + centre_deg.to_radians() + yaw_offset_deg.to_radians();
    let pitch = pitch_offset_deg.to_radians();
    (
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    )
}

/// The browser's material cone: sectors and their offsets as the manifest declares them,
/// 2 m range at the manifest's fixed encoding reference, five pitches −40..+40 (45 rays;
/// decisions §6). Each sector value is a fraction of the whole fan's ray count. It has
/// no memory, no expansion and no body identity — a fresh reading per observation.
pub(crate) fn cone_readings(
    view: &VoxelView<'_>,
    occupancy: &ConeOccupancy,
    observer_id: u64,
    pose: &Pose,
    standing_y: u32,
    manifest: &Manifest,
    body: &crate::Body,
) -> ConeReading {
    let invalid = ConeReading {
        valid: false,
        sectors: [SectorReading::ZERO; 3],
    };
    if !cone_valid(view, pose, standing_y, manifest) {
        return invalid;
    }
    let range = manifest.cone_range_m;
    let origin = cone_origin(view, pose, standing_y, body);
    let heading = pose.heading_rad;
    let rays = manifest.ray_yaw_offsets_deg.len() * manifest.ray_pitch_offsets_deg.len();
    let ray_count = f64::from(rays as u32);
    let mut sectors = [SectorReading::ZERO; 3];
    for (si, &centre_deg) in manifest.sector_centres_deg.iter().enumerate() {
        let (mut clear, mut hit, mut hit_prox) = (0u32, 0u32, 0.0f64);
        let (mut fol, mut fol_prox) = (0u32, 0.0f64);
        let (mut bdy, mut bdy_prox) = (0u32, 0.0f64);
        for &oyaw in manifest.ray_yaw_offsets_deg {
            for &opitch in manifest.ray_pitch_offsets_deg {
                let dir = ray_direction(heading, centre_deg, oyaw, opitch);
                match ray_first_hit(view, occupancy, observer_id, origin, dir, range) {
                    None => clear += 1,
                    Some((dist, fine)) => {
                        let class = fine.coarse();
                        let prox = (1.0 - dist / range).clamp(0.0, 1.0);
                        hit += 1;
                        hit_prox += prox;
                        match class {
                            Class::Foliage => {
                                fol += 1;
                                fol_prox += prox;
                            }
                            Class::Body => {
                                bdy += 1;
                                bdy_prox += prox;
                            }
                            Class::Occluder => {}
                        }
                    }
                }
            }
        }
        sectors[si] = SectorReading {
            clear: f64::from(clear) / ray_count,
            all_proximity: if hit > 0 {
                hit_prox / f64::from(hit)
            } else {
                0.0
            },
            foliage_fraction: f64::from(fol) / ray_count,
            foliage_proximity: if fol > 0 {
                fol_prox / f64::from(fol)
            } else {
                0.0
            },
            body_fraction: f64::from(bdy) / ray_count,
            body_proximity: if bdy > 0 {
                bdy_prox / f64::from(bdy)
            } else {
                0.0
            },
        };
    }
    ConeReading {
        valid: true,
        sectors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Founder;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
    use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig};

    /// A flat world: 8 × 6 × 6 voxels at 0.25 m, soil 1..=2, support at y = 2.
    fn flat_world() -> World {
        let mut world = World::empty(VoxelConfig {
            width: 8,
            height: 6,
            depth: 6,
            voxel_m: 0.25,
            ..VoxelConfig::default()
        });
        for z in 0..6 {
            for x in 0..8 {
                for y in 1..=2 {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        world
    }

    fn site(x: u32, z: u32) -> Site {
        Site { x, y: 2, z }
    }

    fn litter(flora: &mut Flora, x: u32, z: u32, amount: f64) {
        flush_deposit(flora, site(x, z), amount);
    }

    fn flush_deposit(flora: &mut Flora, site: Site, amount: f64) {
        assert!(
            flora.deposit(
                site,
                Deposit {
                    kind: DepositKind::Litter,
                    organic: amount,
                    mineral: amount * 0.02,
                    energy: amount * 2.0,
                },
            ),
            "a deposit on a support face"
        );
    }

    fn pose_at(x_m: f64, z_m: f64) -> Pose {
        Pose {
            x: x_m,
            z: z_m,
            heading_rad: 0.0,
        }
    }

    fn settle_litter() -> (World, Flora, Senses) {
        let world = flat_world();
        let mut flora = Flora::new(FloraConfig::default());
        litter(&mut flora, 2, 2, 0.2);
        let mut senses = Senses::new();
        let (v, fv) = (world.view(), flora.view());
        let (updates, converged) = senses.settle(&v, &fv);
        assert!(converged, "the field converged after {updates} updates");
        (world, flora, senses)
    }

    #[test]
    fn one_update_visits_only_sources_and_their_immediate_neighbors() {
        let world = flat_world();
        let mut flora = Flora::new(FloraConfig::default());
        litter(&mut flora, 2, 2, 0.2);
        let mut senses = Senses::new();
        senses.update(&world.view(), &flora.view());

        assert_eq!(
            senses.field.value.len(),
            5,
            "one source plus four neighbors"
        );
        for &cell in senses.field.value.keys() {
            let (x, y, z) = world.view().config.coords(cell);
            let dx = (i64::from(x) - 2).abs().min((i64::from(x) + 8 - 2).abs());
            let dz = (i64::from(z) - 2).abs();
            assert_eq!(y, 2);
            assert!(dx + dz <= 1, "transitive traversal reached ({x}, {z})");
        }
    }

    #[test]
    fn the_nonwrapping_boundary_reflects_the_local_value() {
        let world = flat_world();
        let mut edge_flora = Flora::new(FloraConfig::default());
        litter(&mut edge_flora, 2, 0, 0.2);
        let mut edge = Senses::new();
        edge.update(&world.view(), &edge_flora.view());
        let edge_cell = world.view().config.index(2, 2, 0);

        let mut middle_flora = Flora::new(FloraConfig::default());
        litter(&mut middle_flora, 2, 2, 0.2);
        let mut middle = Senses::new();
        middle.update(&world.view(), &middle_flora.view());
        let middle_cell = world.view().config.index(2, 2, 2);

        assert!(
            edge.field.value[&edge_cell] > middle.field.value[&middle_cell],
            "the missing z neighbor reflects instead of disappearing from the average"
        );
    }

    /// A source creates a local response; removing the litter stops emission and the
    /// residue decays.
    #[test]
    fn a_source_creates_and_its_removal_decays_the_residue() {
        let (world, mut flora, mut senses) = settle_litter();
        let v = world.view();
        // Standing on the source: a strong local cue, ordered by distance as it falls off.
        let at_source = senses.sample_cue(&v, &pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125), 2);
        let two_away = senses.sample_cue(&v, &pose_at(4.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125), 2);
        let (a, b) = (at_source.expect("supported"), two_away.expect("supported"));
        assert!(
            a > 0.0 && b > 0.0 && b < a,
            "a gradient falls off from the source"
        );

        // Consume the whole tile; the next update stops emitting, so the residue falls.
        assert!(flora.take_litter(site(2, 2), 1.0).is_some());
        let fv = flora.view();
        let after = senses
            .sample_cue(&v, &pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125), 2)
            .expect("still a node");
        let mut fallen = after;
        for _ in 0..20 {
            senses.update(&v, &fv);
            fallen = senses
                .sample_cue(&v, &pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125), 2)
                .expect("still a node");
        }
        assert!(
            fallen < after,
            "with no source the residue decays: {fallen} after {after}"
        );
    }

    /// **The cue is detritus** (`design/handoffs/voxel-diets-2026-09-22.md`,
    /// deliverable 1, written before the rule): a carrion pool on a face with no litter
    /// anywhere raises the field over it, because carrion is now one of the shredder's
    /// three foods and the cue is what it steers by.
    #[test]
    fn a_carrion_pool_with_no_litter_raises_the_cue() {
        let world = flat_world();
        let mut flora = Flora::new(FloraConfig::default());
        assert!(flora.deposit(
            site(2, 2),
            Deposit {
                kind: DepositKind::Carrion,
                organic: 0.2,
                mineral: 0.2 * 0.05,
                energy: 0.2 * 2.0,
            },
        ));
        assert_eq!(
            flora.view().ground_at(site(2, 2)).expect("a pool").litter,
            0.0,
            "the fixture holds no litter at all"
        );
        let mut senses = Senses::new();
        let (v, fv) = (world.view(), flora.view());
        let (updates, converged) = senses.settle(&v, &fv);
        assert!(converged, "the field converged after {updates} updates");
        let at_source = senses
            .sample_cue(&v, &pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125), 2)
            .expect("supported");
        assert!(at_source > 0.0, "a corpse emits a cue: {at_source}");
    }

    /// **A litter-only world reads what it always read, and the source set is litter
    /// and carrion.** The source term is the sum of the matter that decays on a face,
    /// so the field is a function of that sum alone: the same stock as litter or as
    /// carrion settles to the same value, and with only litter present the sum *is* the
    /// litter and the field *is* the litter field.
    ///
    /// Two things a shredder can reach raise nothing. **A glowcap cap is food and not a
    /// source** — decisions §3 as amended 2026-09-22: a cap is found at the mouth, not
    /// smelled across a floor, and the arm that made it a source cost `default` its
    /// shredder lineage. Dead wood is not even food. Together they pin the source set
    /// rather than "anything a mouth could reach".
    ///
    /// Written this way deliberately: a pinned numeric cue would be a golden value of
    /// the transport constants, which this package does not touch.
    #[test]
    fn the_cue_sources_are_litter_and_carrion_and_nothing_else() {
        use cubarium_voxel_flora::{Command as FloraCommand, Species as Plant};

        let world = flat_world();
        let probe = pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125);
        let settled = |flora: &Flora| -> f64 {
            let mut senses = Senses::new();
            let (v, fv) = (world.view(), flora.view());
            let (updates, converged) = senses.settle(&v, &fv);
            assert!(converged, "the field converged after {updates} updates");
            senses.sample_cue(&v, &probe, 2).expect("supported")
        };
        let stock = 0.2;
        let deposited = |kind: DepositKind| -> Flora {
            let mut flora = Flora::new(FloraConfig::default());
            assert!(flora.deposit(
                site(2, 2),
                Deposit {
                    kind,
                    organic: stock,
                    mineral: stock * 0.05,
                    energy: stock * 2.0,
                },
            ));
            flora
        };

        let litter_only = settled(&deposited(DepositKind::Litter));
        assert!(litter_only > 0.0, "the litter fixture emits");
        assert_eq!(
            settled(&deposited(DepositKind::Carrion)),
            litter_only,
            "the same stock as carrion is the same source"
        );

        // A glowcap cap standing on the face: the shredder's food, and no cue.
        let mut fungal = Flora::new(FloraConfig::default());
        let wood_max = fungal.config().species(Plant::Glowcap).wood_max;
        assert!(fungal.apply(
            &world,
            FloraCommand::Seed {
                x: 2,
                z: 2,
                species: Plant::Glowcap,
                wood: wood_max,
            },
        ));
        let cap = fungal
            .view()
            .stand_at(site(2, 2))
            .expect("the seeded glowcap")
            .foliage;
        assert!(cap > 0.0, "the cap holds tissue");
        let quiet = |flora: &Flora, what: &str| {
            let mut senses = Senses::new();
            let (v, fv) = (world.view(), flora.view());
            senses.settle(&v, &fv);
            assert_eq!(
                senses.sample_cue(&v, &probe, 2),
                Some(0.0),
                "{what} is a valid support with no detritus cue on it"
            );
        };
        quiet(&fungal, "a glowcap cap");
        // Dead wood is a glowcap's substrate, not a shredder's food at all.
        quiet(&deposited(DepositKind::DeadWood), "a log");
    }

    /// Roof and floor never share a node: a litter source on a roof does not reach the
    /// floor beneath it, and each layer's adjacency contains only its own height.
    #[test]
    fn a_roof_source_never_reaches_the_floor() {
        let mut world = flat_world();
        // A roofed floor: columns 2..4 carry soil at y = 4..5, so their support faces sit
        // at y = 4 while the ground stays at y = 2, with open air between.
        for z in 1..=3 {
            for x in 2..=4 {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y: 4,
                    z,
                    material: Material::Soil,
                });
            }
        }
        let v = world.view();
        assert!(v.is_support(3, 4, 2) && v.is_support(3, 2, 2));

        let mut flora = Flora::new(FloraConfig::default());
        // Litter on the **roof** floor, y = 4.
        flush_deposit(&mut flora, Site { x: 3, y: 4, z: 2 }, 0.2);
        let mut senses = Senses::new();
        let (v, fv) = (world.view(), flora.view());
        senses.settle(&v, &fv);

        // The graph never connects the two layers.
        let floor_cell = v.config.index(3, 2, 2);
        assert!(
            senses
                .field
                .graph
                .neighbors
                .get(&floor_cell)
                .is_some_and(|nb| nb.iter().all(|&n| {
                    let (_, ny, _) = v.config.coords(n);
                    ny == 2
                })),
            "the floor layer's neighbours are all floor nodes"
        );

        // The floor beneath the roof source reads nothing; the roof itself reads strong.
        let floor_reading =
            senses.sample_cue(&v, &pose_at(3.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125), 2);
        assert_eq!(floor_reading, Some(0.0), "no vertical transport");
        let roof_reading =
            senses.sample_cue(&v, &pose_at(3.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125), 4);
        assert!(roof_reading.expect("roof node") > 0.0);
    }

    /// Two sub-voxel positions inside one gradient cell differ, and a wall (a column that
    /// is not a node at the standing layer) excludes its far-side value from the
    /// interpolation.
    #[test]
    fn sub_voxel_positions_are_interpolated_and_walls_exclude_far_side() {
        let (world, _flora, senses) = settle_litter();
        let v = world.view();
        let near = senses
            .sample_cue(&v, &pose_at(2.0 * 0.25 + 0.01, 2.0 * 0.25 + 0.5), 2)
            .unwrap();
        let far = senses
            .sample_cue(&v, &pose_at(2.0 * 0.25 + 0.49, 2.0 * 0.25 + 0.5), 2)
            .unwrap();
        assert!(
            (near - far).abs() > 1e-4,
            "bilinear: two positions in one cell read different concentrations"
        );

        // A wall column at (4, z) that reaches the body layer: its support face is at y=3,
        // so it is not a node at the standing layer and must not leak its far side.
        let mut walled = flat_world();
        for z in 2..=4 {
            walled.apply(WorldCommand::SetMaterial {
                x: 4,
                y: 3,
                z,
                material: Material::Soil,
            });
        }
        let mut flora = Flora::new(FloraConfig::default());
        litter(&mut flora, 2, 3, 0.2); // far side (west of the wall)
        let mut senses = Senses::new();
        let (v, fv) = (walled.view(), flora.view());
        senses.settle(&v, &fv);
        // Receptor just east of the wall (column 5), at the wall's near edge: its cell's
        // west corner is the wall column, which is excluded, so it reads its own (bare,
        // zero) side — the far-side litter never enters.
        let reading = senses
            .sample_cue(&v, &pose_at(5.0 * 0.25 + 0.01, 3.0 * 0.25 + 0.5), 2)
            .unwrap();
        assert!(
            reading < 0.01,
            "a reading across the wall kept the far-side value: {reading}"
        );
    }

    /// Birth has no false trend: the first sample initializes history from itself (zero
    /// derivative), a rising field makes it positive.
    #[test]
    fn the_first_sample_has_no_trend_and_a_rise_outputs_positive() {
        let manifest = Founder::Blind.manifest();
        let mut senses = Senses::new();
        assert_eq!(
            senses.advance_chem_trend(7, 0.5, &manifest),
            0.0,
            "first sample"
        );
        let rising = senses.advance_chem_trend(7, 0.9, &manifest);
        assert!(
            rising > 0.0,
            "a rising response gives a positive trend, got {rising}"
        );
        let falling = senses.advance_chem_trend(7, 0.2, &manifest);
        assert!(
            falling < 0.0,
            "a falling response gives a negative trend, got {falling}"
        );
        assert!((-1.0..=1.0).contains(&senses.advance_chem_trend(7, 1.0, &manifest)));
    }

    /// `reset_trends` makes a settled field safe to hand to a fresh episode: the dead
    /// episode's trend history cannot reach the new episode's bodies, whose ids start
    /// over — without it, body 0 of the next episode would read a dead body's history as
    /// a false gradient.
    #[test]
    fn reset_trends_clears_the_episode_private_history() {
        let manifest = Founder::Blind.manifest();
        let mut senses = Senses::new();
        assert_eq!(
            senses.advance_chem_trend(0, 0.5, &manifest),
            0.0,
            "first sample"
        );
        assert_ne!(
            senses.advance_chem_trend(0, 0.9, &manifest),
            0.0,
            "a real trend"
        );
        senses.reset_trends();
        assert_eq!(
            senses.advance_chem_trend(0, 0.9, &manifest),
            0.0,
            "the cleared history re-initializes from the first sample"
        );
    }

    /// Light is uniform sky × terrain exposure: open ground reads 1, a cell under a solid
    /// roof reads 0, and nothing emissive exists to detect.
    #[test]
    fn light_an_open_receptor_differs_from_a_covered_one() {
        let mut world = flat_world();
        // A one-voxel roof over column (2, 2) at the body layer.
        world.apply(WorldCommand::SetMaterial {
            x: 2,
            y: 3,
            z: 2,
            material: Material::Soil,
        });
        let v = world.view();
        let open = v.sky_visibility(5, 2, 2);
        assert!((open - 1.0).abs() < 1e-9, "open sky reads {open}");
        let covered = v.sky_visibility(2, 2, 2);
        assert!(
            covered < 1e-9,
            "a roofed cell reads zero sky, got {covered}"
        );
        // Glowcap emission is not modeled: the light channel is sky visibility alone, so a
        // bright fungal source to the side cannot change a receptor that is underground.
        assert_eq!(covered, 0.0, "no emissive contribution exists to read");
    }

    /// Cone: the centre sector sees a stand dead ahead, a foreground rock occludes the
    /// foliage behind it, and an empty direction reads all-zero class slots with the eye
    /// still valid.
    #[test]
    fn a_foreground_rock_blocks_foliage_and_empty_slots_are_zero() {
        const H: f64 = std::f64::consts::FRAC_PI_2; // heading east (+x)
        let world = flat_world();
        let v = world.view();
        let mut flora = Flora::new(FloraConfig::default());
        let wood = 0.5
            * flora
                .config()
                .species(cubarium_voxel_flora::Species::Springturf)
                .wood_max;
        assert!(flora.apply(
            &world,
            cubarium_voxel_flora::Command::Seed {
                x: 5,
                z: 2,
                species: cubarium_voxel_flora::Species::Springturf,
                wood,
            },
        ));
        let fauna = crate::Fauna::new(crate::FaunaConfig::default());
        let manifest = Founder::Browser.manifest();
        let body = crate::FounderPhysiology::frozen(Founder::Browser).adult_body();
        let fv = flora.view();
        let fv_ = fauna.view();

        // Facing the turf from three columns west along +x: the centre sector reports
        // foliage with a positive fraction and proximity.
        let pose = Pose {
            x: 2.0 * 0.25 + 0.125,
            z: 2.0 * 0.25 + 0.125,
            heading_rad: H,
        };
        let occupancy = cone_occupancy(&v, &fv, &fv_);
        let cone = cone_readings(&v, &occupancy, u64::MAX, &pose, 2, &manifest, &body);
        assert!(cone.valid);
        let centre = cone.sectors[1];
        assert!(centre.foliage_fraction > 0.0, "foliage visible ahead");
        assert!(centre.foliage_proximity > 0.0);
        assert!(centre.clear < 1.0);

        // Put a rock wall right in front of the eye: it occludes the foliage behind it.
        let mut walled = flat_world();
        for z in 2..=3u32 {
            walled.apply(WorldCommand::SetMaterial {
                x: 3,
                y: 3,
                z,
                material: Material::Soil,
            });
        }
        let wv = walled.view();
        let mut wflora = Flora::new(FloraConfig::default());
        assert!(wflora.apply(
            &walled,
            cubarium_voxel_flora::Command::Seed {
                x: 5,
                z: 2,
                species: cubarium_voxel_flora::Species::Springturf,
                wood,
            },
        ));
        let occupancy = cone_occupancy(&wv, &wflora.view(), &fv_);
        let cone = cone_readings(&wv, &occupancy, u64::MAX, &pose, 2, &manifest, &body);
        assert_eq!(
            cone.sectors[1].foliage_fraction, 0.0,
            "the wall hides the foliage behind it"
        );
        assert!(
            cone.sectors[1].all_proximity > 0.0,
            "the wall is still an occluding hit"
        );

        // An empty direction (nothing in the world, facing north into open ground): every
        // class slot is zero with the eye valid — a measured miss, never a magic hit.
        let empty = flat_world();
        let ev = empty.view();
        let eflora = Flora::new(FloraConfig::default());
        let pose = Pose {
            x: 0.5,
            z: 0.5,
            heading_rad: 0.0,
        };
        let occupancy = cone_occupancy(&ev, &eflora.view(), &fv_);
        let cone = cone_readings(&ev, &occupancy, u64::MAX, &pose, 2, &manifest, &body);
        assert!(cone.valid);
        for sec in cone.sectors {
            assert_eq!((sec.foliage_fraction, sec.body_fraction), (0.0, 0.0));
            assert_eq!((sec.foliage_proximity, sec.body_proximity), (0.0, 0.0));
        }
    }

    #[test]
    fn a_real_observer_does_not_occlude_itself_but_sees_another_body() {
        const H: f64 = std::f64::consts::FRAC_PI_2;
        let world = flat_world();
        let view = world.view();
        let flora = Flora::new(FloraConfig::default());
        let manifest = Founder::Browser.manifest();
        let mut fauna = crate::Fauna::new(crate::FaunaConfig::default());
        assert!(fauna.apply(
            &world,
            crate::Command::IntroduceFounder {
                x: 2,
                z: 2,
                founder: Founder::Browser,
                stores: crate::StartingStores::FULL,
                heading_rad: H,
            },
        ));
        let observer = fauna.view().animals[0];
        let body = fauna
            .config()
            .founder(Founder::Browser)
            .body_at(observer.body);
        let occupancy = cone_occupancy(&view, &flora.view(), &fauna.view());
        let alone = cone_readings(
            &view,
            &occupancy,
            observer.id,
            &observer.pose,
            observer.site.y,
            &manifest,
            &body,
        );
        assert!(
            alone
                .sectors
                .iter()
                .all(|sector| sector.body_fraction == 0.0),
            "the observer's own occupied cell is excluded"
        );

        assert!(fauna.apply(
            &world,
            crate::Command::IntroduceFounder {
                x: 4,
                z: 2,
                founder: Founder::Browser,
                stores: crate::StartingStores::FULL,
                heading_rad: 0.0,
            },
        ));
        let occupancy = cone_occupancy(&view, &flora.view(), &fauna.view());
        let with_other = cone_readings(
            &view,
            &occupancy,
            observer.id,
            &observer.pose,
            observer.site.y,
            &manifest,
            &body,
        );
        assert!(
            with_other.sectors[1].body_fraction > 0.0,
            "the other body ahead remains visible"
        );
    }
}
