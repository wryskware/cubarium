//! **The pack off the critical path** (H3 of `design/handoffs/hybrid-renderer-2026-10-07.md`):
//! what the GPU pack reads of one tick's world, copied off the live world so the pack can
//! run on another thread while the next tick steps.
//!
//! # The three pieces
//!
//! - **[`PackSnapshot`]**: one tick's copy. It owns everything the packer reads, except
//!   the two dense water grids, which are far too large to copy every tick (a wide world's
//!   free and pore water are 63 MB each):
//!   - **free water** travels as the world's wet set and the values there. Free water is
//!     zero outside the wet set (it is the set of void cells holding free water), so the
//!     list is the whole grid. A world whose set is not trusted yet (loaded, not stepped)
//!     sends its whole grid once.
//!   - **pore water** travels as one window of the grid per snapshot: a sweep that covers
//!     the grid once every [`crate::voxel::PackConfig::pore_sweep`] snapshots, as the Godot
//!     feed's `pore_sweep` does. Soil wetness on screen catches up over that many ticks.
//!     A new world or a terrain edit sends the whole grid.
//!   - the terrain (material) is shared, not copied, while it stands still: one `Arc` per
//!     terrain version.
//! - **[`SnapshotCopier`]**: fills snapshots from the live world, on the simulation's
//!   thread. It keeps the sweep's cursor and the terrain's `Arc`.
//! - **[`Mirror`]**: the pack's side. It keeps the dense grids the snapshots update (its own
//!   free and pore water), and makes the views the packer reads: the same
//!   [`VoxelView`], [`FloraView`] and [`FaunaView`] the live world would give, over the
//!   snapshot's data.
//!
//! A snapshot is owned by one side at a time (filled by the copier, then handed to the
//! pack, then handed back to be refilled), so a pack never reads a snapshot being written.

use std::sync::Arc;

use cubarium_voxel::weather::WeatherView;
use cubarium_voxel::{Config, Ledger, Material, VoxelView};
use cubarium_voxel_fauna::{Animal, Clutch, FaunaConfig, FaunaLedger, FaunaView};
use cubarium_voxel_flora::{Burr, Cover, CrownCache, FloraConfig, FloraLedger, FloraView, Ground, Stand};

/// The default sweep ([`crate::voxel::PackConfig::pore_sweep`]): the pack's pore water covers
/// the grid once every this many snapshots.
pub const PORE_SWEEP: u32 = 16;

/// One tick's world as the pack reads it ([`self`]).
pub struct PackSnapshot {
    /// Which world it is of, for a host that installs worlds while snapshots are in flight
    /// (the hosted renderer's install count). The copier leaves it alone.
    pub generation: u64,
    world: WorldPart,
    flora: FloraPart,
    fauna: FaunaPart,
    /// The world's wet set (`Config::index`), and the free water at each; or, when the
    /// world's set is not trusted, `free_all`: the whole grid.
    wet: Vec<usize>,
    wet_free: Vec<f64>,
    wet_trusted: bool,
    free_all: Vec<f64>,
    /// Pore water: `(start, len)` ranges of world indices, and their values one after the
    /// other.
    pore_ranges: Vec<(usize, usize)>,
    pore: Vec<f64>,
    /// A mirror has taken this snapshot in. One that was never taken in and is filled again
    /// keeps its pore windows (and takes the new one too), so no window is lost.
    applied: bool,
    /// Milliseconds the last fill took.
    pub(crate) copy_ms: f64,
}

struct WorldPart {
    config: Config,
    material: Arc<Vec<Material>>,
    tick: u64,
    terrain_version: u64,
    ledger: Ledger,
    aquifer_m3: f64,
    atmosphere_m3: f64,
    outlet_open: bool,
    outlet: Option<(u32, u32, u32)>,
    spring: Option<(u32, u32, u32)>,
    weather: WeatherView,
}

struct FloraPart {
    config: FloraConfig,
    tick: u64,
    stands: Vec<Stand>,
    ground: Vec<Ground>,
    ledger: FloraLedger,
    cover: Cover,
    crowns: CrownCache,
    burrs: Vec<Burr>,
}

struct FaunaPart {
    config: FaunaConfig,
    tick: u64,
    animals: Vec<Animal>,
    clutches: Vec<Clutch>,
    ledger: FaunaLedger,
}

impl Default for PackSnapshot {
    fn default() -> PackSnapshot {
        PackSnapshot {
            generation: 0,
            world: WorldPart {
                config: Config::default(),
                material: Arc::new(Vec::new()),
                tick: 0,
                terrain_version: 0,
                ledger: Ledger::default(),
                aquifer_m3: 0.0,
                atmosphere_m3: 0.0,
                outlet_open: false,
                outlet: None,
                spring: None,
                weather: WeatherView::CLEAR_NOON,
            },
            flora: FloraPart {
                config: FloraConfig::default(),
                tick: 0,
                stands: Vec::new(),
                ground: Vec::new(),
                ledger: FloraLedger::default(),
                cover: Cover::default(),
                crowns: CrownCache::default(),
                burrs: Vec::new(),
            },
            fauna: FaunaPart {
                config: FaunaConfig::default(),
                tick: 0,
                animals: Vec::new(),
                clutches: Vec::new(),
                ledger: FaunaLedger::default(),
            },
            wet: Vec::new(),
            wet_free: Vec::new(),
            wet_trusted: false,
            free_all: Vec::new(),
            pore_ranges: Vec::new(),
            pore: Vec::new(),
            applied: true,
            copy_ms: 0.0,
        }
    }
}

impl PackSnapshot {
    /// The world's tick this snapshot is of.
    pub fn tick(&self) -> u64 {
        self.world.tick
    }

    /// The terrain version this snapshot is of.
    pub fn terrain_version(&self) -> u64 {
        self.world.terrain_version
    }

    /// The world's material, one entry per voxel in `Config::index` order.
    pub fn material(&self) -> &[Material] {
        &self.world.material
    }

    /// The world's config.
    pub fn config(&self) -> &Config {
        &self.world.config
    }

    /// Milliseconds the last fill took on the copier's thread.
    pub fn copy_ms(&self) -> f64 {
        self.copy_ms
    }
}

/// Fills [`PackSnapshot`]s from the live world, on the simulation's thread ([`self`]).
pub struct SnapshotCopier {
    sweep: u32,
    cursor: usize,
    /// The terrain the shared material is of: its version and the grid's size.
    terrain: Option<(u64, usize)>,
    material: Arc<Vec<Material>>,
    /// The tick of the last fill: a world whose tick runs backwards is another world.
    last_tick: Option<u64>,
    /// The next fill carries the whole pore grid.
    whole: bool,
}

impl SnapshotCopier {
    /// A copier whose pore sweep covers the grid once every `sweep` fills (at least one).
    pub fn new(sweep: u32) -> SnapshotCopier {
        SnapshotCopier {
            sweep: sweep.max(1),
            cursor: 0,
            terrain: None,
            material: Arc::new(Vec::new()),
            last_tick: None,
            whole: true,
        }
    }

    /// Another world: the next fill carries everything.
    pub fn reset(&mut self) {
        self.terrain = None;
        self.last_tick = None;
        self.cursor = 0;
        self.whole = true;
    }

    /// Copy what the pack reads of this tick's world into `snap`.
    pub fn fill(&mut self, view: &VoxelView<'_>, flora: FloraView<'_>, fauna: FaunaView<'_>, snap: &mut PackSnapshot) {
        let at = std::time::Instant::now();
        let n = view.material.len();
        if self.last_tick.is_some_and(|t| view.tick < t) {
            self.reset();
        }
        self.last_tick = Some(view.tick);
        if self.terrain != Some((view.terrain_version, n)) {
            self.material = Arc::new(view.material.to_vec());
            self.terrain = Some((view.terrain_version, n));
            self.whole = true;
        }
        let w = &mut snap.world;
        w.config.clone_from(view.config);
        w.material = self.material.clone();
        w.tick = view.tick;
        w.terrain_version = view.terrain_version;
        w.ledger.clone_from(view.ledger);
        w.aquifer_m3 = view.aquifer_m3;
        w.atmosphere_m3 = view.atmosphere_m3;
        w.outlet_open = view.outlet_open;
        w.outlet = view.outlet;
        w.spring = view.spring;
        w.weather = view.weather;

        // Free water: the wet set and its values, or the whole grid.
        snap.wet_trusted = view.wet_cells(&mut snap.wet);
        snap.wet_free.clear();
        snap.free_all.clear();
        if snap.wet_trusted {
            snap.wet_free.extend(snap.wet.iter().map(|&i| view.free[i]));
        } else {
            snap.free_all.extend_from_slice(view.free);
        }

        // Pore water: this snapshot's window of the sweep (and any window a snapshot that
        // was never taken in still carries), or the whole grid.
        if !snap.applied && !self.whole {
            // Refilled before a mirror took it: its windows stand, refreshed below.
        } else {
            snap.pore_ranges.clear();
        }
        if self.whole || n == 0 {
            snap.pore_ranges.clear();
            snap.pore_ranges.push((0, n));
            self.cursor = 0;
            self.whole = false;
        } else {
            // `share <= n`, so a window that wraps ends before it began.
            let share = n.div_ceil(self.sweep as usize).clamp(1, n);
            let start = self.cursor % n;
            let first = share.min(n - start);
            snap.pore_ranges.push((start, first));
            if first < share {
                snap.pore_ranges.push((0, share - first));
            }
            self.cursor = (start + share) % n;
        }
        snap.pore.clear();
        for &(start, len) in &snap.pore_ranges {
            snap.pore.extend_from_slice(&view.pore[start..start + len]);
        }

        let f = &mut snap.flora;
        f.config.clone_from(flora.config);
        f.tick = flora.tick;
        f.stands.clear();
        f.stands.extend_from_slice(flora.stands);
        f.ground.clone_from_slice_or_extend(flora.ground);
        f.ledger.clone_from(flora.ledger);
        f.cover.clone_from(flora.cover);
        f.crowns.clone_from(flora.crowns);
        f.burrs.clear();
        f.burrs.extend_from_slice(flora.burrs);

        let a = &mut snap.fauna;
        a.config.clone_from(fauna.config);
        a.tick = fauna.tick;
        a.animals.clear();
        a.animals.extend_from_slice(fauna.animals);
        a.clutches.clear();
        a.clutches.extend_from_slice(fauna.clutches);
        a.ledger = *fauna.ledger;

        snap.applied = false;
        snap.copy_ms = at.elapsed().as_secs_f64() * 1e3;
    }
}

/// `Vec::clone_from` for a slice source: reuse what is there, element by element.
trait CloneFromSlice<T> {
    fn clone_from_slice_or_extend(&mut self, src: &[T]);
}

impl<T: Clone> CloneFromSlice<T> for Vec<T> {
    fn clone_from_slice_or_extend(&mut self, src: &[T]) {
        self.truncate(src.len());
        let have = self.len();
        for (d, s) in self.iter_mut().zip(src) {
            d.clone_from(s);
        }
        self.extend_from_slice(&src[have..]);
    }
}

/// The pack's side of the snapshots ([`self`]): the dense water grids they update, and the
/// views over a snapshot that the packer reads.
#[derive(Default)]
pub struct Mirror {
    /// Free water, dense, and the cells the last snapshot set (to clear before the next).
    free: Vec<f64>,
    set: Vec<usize>,
    /// Pore water, dense.
    pore: Vec<f64>,
    /// Pore ranges taken in since the last pack that landed; `None`, everything (a new
    /// grid). The packer compares only these against what it holds.
    owed: Option<Vec<(usize, usize)>>,
}

/// What a pack of a snapshot may skip, from the [`Mirror`]: the wet set it would otherwise
/// rebuild from the free-water grid, and the only pore ranges that can have moved.
#[derive(Clone, Copy, Default)]
pub(crate) struct Hint<'a> {
    pub wet: Option<&'a [usize]>,
    pub pore: Option<&'a [(usize, usize)]>,
}

impl Mirror {
    /// Take `snap` in: its free water over the grid's, its pore windows over the grid's.
    /// Taking the same snapshot in twice (a pack refused and tried again) changes nothing.
    pub fn apply(&mut self, snap: &mut PackSnapshot) {
        let n = snap.world.material.len();
        if self.free.len() != n {
            self.free = vec![0.0; n];
            self.pore = vec![0.0; n];
            self.set.clear();
            self.owed = None;
        }
        if snap.wet_trusted {
            for &i in &self.set {
                self.free[i] = 0.0;
            }
            for (&i, &v) in snap.wet.iter().zip(&snap.wet_free) {
                self.free[i] = v;
            }
            self.set.clear();
            self.set.extend_from_slice(&snap.wet);
        } else if snap.free_all.len() == n {
            self.free.copy_from_slice(&snap.free_all);
            self.set.clear();
            self.set.extend((0..n).filter(|&i| self.free[i] != 0.0));
        }
        let mut k = 0;
        for &(start, len) in &snap.pore_ranges {
            self.pore[start..start + len].copy_from_slice(&snap.pore[k..k + len]);
            k += len;
            if start == 0 && len == n {
                self.owed = None;
            } else if let Some(owed) = &mut self.owed {
                owed.push((start, len));
            }
        }
        snap.applied = true;
    }

    /// A pack of what was taken in landed: nothing more is owed.
    pub fn packed(&mut self) {
        match &mut self.owed {
            Some(o) => o.clear(),
            None => self.owed = Some(Vec::new()),
        }
    }

    /// The views over `snap` and this mirror's grids, as the live world's would be.
    pub fn views<'a>(&'a self, snap: &'a PackSnapshot) -> (VoxelView<'a>, FloraView<'a>, FaunaView<'a>) {
        let w = &snap.world;
        let mut view = VoxelView::of_terrain(&w.config, &w.material, &w.ledger);
        view.free = &self.free;
        view.pore = &self.pore;
        view.tick = w.tick;
        view.terrain_version = w.terrain_version;
        view.aquifer_m3 = w.aquifer_m3;
        view.atmosphere_m3 = w.atmosphere_m3;
        view.outlet_open = w.outlet_open;
        view.outlet = w.outlet;
        view.spring = w.spring;
        view.weather = w.weather;
        let f = &snap.flora;
        let flora = FloraView {
            config: &f.config,
            tick: f.tick,
            stands: &f.stands,
            ground: &f.ground,
            ledger: &f.ledger,
            cover: &f.cover,
            crowns: &f.crowns,
            burrs: &f.burrs,
        };
        let a = &snap.fauna;
        let fauna = FaunaView {
            config: &a.config,
            tick: a.tick,
            animals: &a.animals,
            clutches: &a.clutches,
            ledger: &a.ledger,
        };
        (view, flora, fauna)
    }

    /// What the pack of the snapshot last taken in may skip.
    pub(crate) fn hint<'a>(&'a self, snap: &'a PackSnapshot) -> Hint<'a> {
        Hint {
            wet: snap.wet_trusted.then_some(snap.wet.as_slice()),
            pore: self.owed.as_deref(),
        }
    }
}
