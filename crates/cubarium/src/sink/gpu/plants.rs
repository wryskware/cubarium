//! The plant pass's cells (`[figures] plants = "models"`, `cubarium_gpu::plants`), packed
//! once a tick beside the voxels.
//!
//! **What a plant cell is**: every trunk or crown cell a stand stamped
//! ([`Stands`]), the same cells the walk then lets its ray through when they are dry. Each
//! names its plant: the flora stand that placed it ([`Stands::owner`]), rooted at the cell
//! above the stand's (drawn) support face. A cell no stand owns (a vine cell) is plant 0,
//! which never moves.
//!
//! **Sway**: a plant's amplitude grows with its height ([`PlantConfig::tip_amplitude`]) and a
//! cell's share of it with its height in the plant ([`PlantConfig::weight`]); the renderer
//! takes the breeze at the plant's root column every frame.
//!
//! **Grow-in**: a cell stamped this tick and not the last **arrives**, one stamped the last
//! tick and not this one **leaves**. Each starts its change at a moment in the window
//! `foliage_fade_s − grow_s` after the tick: its place in the plant's order ([`GrowOrder`]:
//! root up, or from the axis out; leaving reversed) mixed with a per-cell draw by `jitter`,
//! and then scales over `grow_s`. A leaving cell is drawn from the texel it was stamped
//! with (its style slot held in the palette until it is gone). A change reversed half way
//! goes back from the size it has, with no wait. The first tick of a world, and a jump of
//! the clock, draw every cell as it stands.

use rustc_hash::FxHashMap;

use cubarium_gpu::plants::{PlantList, PlantRec, SETTLED, cell_scale, pack_cell, pack_faces, unsmooth};
use cubarium_gpu::voxel::{VoxelStaging, VoxelTexel};
use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::FloraView;

use crate::voxel::plant_motion::{GrowOrder, PlantConfig};
use crate::voxel::stand::{Part, Stands, drawn_site};

use super::super::pack_timer::lap;
use super::super::palette::{Palette, StyleKey};

/// A tick further from the last than this is a jump (a load, a seek): nothing grows in.
const SETTLE_GAP: u64 = 100;

#[derive(Clone, Copy, Debug)]
struct Cell {
    /// Texture index, `(z · height + y) · width + x`.
    index: u32,
    xyz: [u32; 3],
    plant: u32,
    stand: Option<u64>,
    weight: f32,
    /// Its top and bottom faces' weights ([`pack_faces`]): where a drape cord's joins move.
    faces: u32,
    /// Its place in the plant's order, 0..1 (0 first to arrive).
    rank: f32,
}

#[derive(Clone, Copy, Debug)]
struct Plant {
    stand: u64,
    species: cubarium_voxel_flora::Species,
    rec: PlantRec,
    root: [i64; 3],
    /// The highest cell's height over the root, and the farthest cell from the axis.
    top: i64,
    radius: f64,
}

#[derive(Clone, Copy, Debug)]
struct Change {
    dying: bool,
    /// Frame-clock seconds the cell starts arriving or leaving.
    time: f64,
    xyz: [u32; 3],
    /// Leaving: the texel it was stamped with, and the style it holds in the palette.
    texel: VoxelTexel,
    key: Option<StyleKey>,
    stand: Option<u64>,
    rec: PlantRec,
    weight: f32,
}

/// The cells between ticks: see the module.
pub(super) struct PlantTracker {
    cfg: PlantConfig,
    fade_s: f64,
    width: u32,
    height: u32,
    cur: Vec<Cell>,
    prev: Vec<Cell>,
    now_bits: Vec<u64>,
    was_bits: Vec<u64>,
    plants: Vec<Plant>,
    /// Flora stand index → this tick's plant (dense, by index), and stand id → plant.
    slot_of: Vec<u32>,
    by_stand: FxHashMap<u64, u32>,
    prev_plants: FxHashMap<u64, Plant>,
    /// The cells arriving or leaving, by texture index (a few hundred at most: sparse).
    changes: FxHashMap<u32, Change>,
    last_tick: Option<u64>,
    now_s: f64,
    /// The last list built, while it holds for this tick too (nothing moved, nothing
    /// changing): copied instead of built.
    cache: PlantList,
    cache_valid: bool,
    scratch: Vec<u32>,
}

fn bit(bits: &[u64], i: u32) -> bool {
    bits[(i / 64) as usize] & (1 << (i % 64)) != 0
}

fn set_bit(bits: &mut [u64], i: u32, on: bool) {
    let (w, b) = ((i / 64) as usize, 1u64 << (i % 64));
    if on { bits[w] |= b } else { bits[w] &= !b }
}

/// A draw in `[0, 1)` for a cell of a stand: the same every time.
fn draw(stand: u64, index: u32) -> f64 {
    let mut z = stand ^ u64::from(index).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

impl PlantTracker {
    pub(super) fn new(cfg: PlantConfig, fade_s: f64, width: u32, height: u32, depth: u32) -> PlantTracker {
        let n = width as usize * height as usize * depth as usize;
        PlantTracker {
            cfg,
            fade_s: fade_s.max(0.0),
            width,
            height,
            cur: Vec::new(),
            prev: Vec::new(),
            now_bits: vec![0; n.div_ceil(64)],
            was_bits: vec![0; n.div_ceil(64)],
            plants: Vec::new(),
            slot_of: Vec::new(),
            by_stand: FxHashMap::default(),
            prev_plants: FxHashMap::default(),
            changes: FxHashMap::default(),
            last_tick: None,
            now_s: 0.0,
            cache: PlantList::default(),
            cache_valid: false,
            scratch: Vec::new(),
        }
    }

    fn grow_s(&self) -> f64 {
        self.cfg.grow_s.max(0.0)
    }

    /// When a cell of rank `rank` starts its change after a tick at `now`: arriving in
    /// order, leaving in reverse.
    fn moment(&self, now: f64, stand: Option<u64>, index: u32, rank: f32, leaving: bool) -> f64 {
        let window = (self.fade_s - self.grow_s()).max(0.0);
        let j = self.cfg.jitter.clamp(0.0, 1.0);
        let place = if leaving { 1.0 - f64::from(rank) } else { f64::from(rank) };
        now + window * ((1.0 - j) * place.clamp(0.0, 1.0) + j * draw(stand.unwrap_or(0), index))
    }

    /// This tick's plant cells from the rebuilt stands, and what arrives and leaves: call
    /// after the stands' rebuild and **before** the voxels are filled, while `master` still
    /// holds the last tick's texels (a leaving cell's) and `keyed` says which of them hold a
    /// palette slot. `now` is the frame clock's seconds at this tick.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn detect(
        &mut self,
        view: &VoxelView<'_>,
        flora: FloraView<'_>,
        stands: &Stands,
        master: &[VoxelTexel],
        keyed: impl Fn(usize) -> bool,
        palette: &mut Palette,
        tick: u64,
        now: f64,
    ) {
        self.now_s = now;
        let settle = match self.last_tick {
            None => true,
            Some(last) => tick < last || tick > last + SETTLE_GAP,
        };
        self.last_tick = Some(tick);
        // A change-only rebuild that moved nothing leaves every plant cell, its plant and its
        // owner as they were: most ticks. Gathering reads the stand grid at every plant
        // cell, scattered over the volume (2 ms a tick on the wide world).
        let (w, h) = (self.width, self.height);
        let now_bits = &self.now_bits;
        let plant_cell = |g: u32| {
            let (x, y, z) = stands.xyz(g);
            matches!(stands.at_index(g), Part::Trunk(_) | Part::Crown { .. })
                || bit(now_bits, VoxelStaging::index(w, h, x, y, z) as u32)
        };
        // Only a dirty cell that is, or was, a plant cell moves a plant (not a mark).
        let moved = settle || !stands.incremental() || stands.dirty().iter().any(|&g| plant_cell(g));
        self.cache_valid &= !moved;
        if moved {
            // The last tick's cells become `prev`; `now_bits` is cleared for this tick's.
            for c in &self.prev {
                set_bit(&mut self.was_bits, c.index, false);
            }
            std::mem::swap(&mut self.prev, &mut self.cur);
            std::mem::swap(&mut self.was_bits, &mut self.now_bits);
            self.cur.clear();
            lap!("plants");
            self.gather(view, flora, stands);
            lap!("plant gather");
        }
        if settle {
            for (_, ch) in self.changes.drain() {
                if let Some(key) = ch.key {
                    release_key(palette, &key);
                }
            }
        } else {
            let grow = self.grow_s();
            let as_f32 = |s: f64| s as f32;
            // Arrivals.
            for k in 0..if moved { self.cur.len() } else { 0 } {
                let c = self.cur[k];
                if bit(&self.was_bits, c.index) {
                    continue;
                }
                let start = match self.changes.get(&c.index) {
                    // Leaving still: back from the size it has.
                    Some(ch) if ch.dying => {
                        let s = cell_scale(as_f32(now - ch.time), 0.0, true, as_f32(grow));
                        if let Some(key) = ch.key {
                            release_key(palette, &key);
                        }
                        now - f64::from(unsmooth(s)) * grow
                    }
                    Some(ch) => ch.time,
                    None => self.moment(now, c.stand, c.index, c.rank, false),
                };
                self.changes.insert(
                    c.index,
                    Change {
                        dying: false,
                        time: start,
                        xyz: c.xyz,
                        texel: VoxelTexel::default(),
                        key: None,
                        stand: c.stand,
                        rec: PlantRec::default(),
                        weight: c.weight,
                    },
                );
            }
            // Departures.
            for k in 0..if moved { self.prev.len() } else { 0 } {
                let c = self.prev[k];
                if bit(&self.now_bits, c.index) {
                    continue;
                }
                let start = match self.changes.get(&c.index) {
                    Some(ch) if !ch.dying => {
                        let s = cell_scale(as_f32(now - ch.time), 0.0, false, as_f32(grow));
                        if s <= 0.0 {
                            // It never showed: it simply is not there.
                            self.changes.remove(&c.index);
                            continue;
                        }
                        now - f64::from(unsmooth(1.0 - s)) * grow
                    }
                    Some(ch) => ch.time,
                    None => self.moment(now, c.stand, c.index, c.rank, true),
                };
                let texel = master[c.index as usize];
                let key = if keyed(c.index as usize) {
                    palette.key(texel.style()).copied().inspect(|k| {
                        palette.acquire(k);
                    })
                } else {
                    None
                };
                let rec = c
                    .stand
                    .and_then(|s| self.prev_plants.get(&s))
                    .map_or(PlantRec::default(), |p| p.rec);
                self.changes.insert(
                    c.index,
                    Change { dying: true, time: start, xyz: c.xyz, texel, key, stand: c.stand, rec, weight: c.weight },
                );
            }
            // Changes done.
            let mut done = Vec::new();
            for (&i, ch) in &self.changes {
                if now >= ch.time + grow {
                    done.push(i);
                }
            }
            for i in done {
                if let Some(ch) = self.changes.remove(&i)
                    && let Some(key) = ch.key
                {
                    release_key(palette, &key);
                }
            }
        }
        if moved {
            self.prev_plants.clear();
            self.prev_plants.extend(self.plants[1..].iter().map(|p| (p.stand, *p)));
        }
    }

    /// The stamped plant cells, their plants and their weights and ranks.
    fn gather(&mut self, view: &VoxelView<'_>, flora: FloraView<'_>, stands: &Stands) {
        let (w, h) = (self.width, self.height);
        self.slot_of.clear();
        self.slot_of.resize(flora.stands.len(), u32::MAX);
        self.by_stand.clear();
        self.plants.clear();
        self.plants.push(Plant {
            stand: u64::MAX,
            species: cubarium_voxel_flora::Species::Glowcap,
            rec: PlantRec::default(),
            root: [0; 3],
            top: 0,
            radius: 0.0,
        });
        let ring = i64::from(w.max(1));
        let wrap = |dx: i64| (dx + ring / 2).rem_euclid(ring) - ring / 2;
        // In grid order: the grid and its owners are read front to back, not scattered.
        self.scratch.clear();
        self.scratch.extend(stands.stamped());
        self.scratch.sort_unstable();
        for k in 0..self.scratch.len() {
            let g = self.scratch[k];
            if !matches!(stands.at_index(g), Part::Trunk(_) | Part::Crown { .. }) {
                continue;
            }
            let (x, y, z) = stands.xyz(g);
            let index = VoxelStaging::index(w, h, x, y, z) as u32;
            let (plant, stand) = match stands.owner(i64::from(x), y, z).filter(|&o| o < flora.stands.len()) {
                Some(o) => {
                    if self.slot_of[o] == u32::MAX {
                        let s = &flora.stands[o];
                        let site = drawn_site(view, flora, s);
                        self.slot_of[o] = self.plants.len() as u32;
                        self.by_stand.insert(s.id, self.plants.len() as u32);
                        self.plants.push(Plant {
                            stand: s.id,
                            species: s.species,
                            rec: PlantRec { x: site.x as f32, amp: 0.0, lag: 0.0 },
                            root: [i64::from(site.x), i64::from(site.y) + 1, i64::from(site.z)],
                            top: 0,
                            radius: 0.0,
                        });
                    }
                    let k = self.slot_of[o];
                    (k, Some(flora.stands[o].id))
                }
                None => (0, None),
            };
            if plant != 0 {
                let p = &mut self.plants[plant as usize];
                p.top = p.top.max(i64::from(y) - p.root[1]);
                let (dx, dz) = (wrap(i64::from(x) - p.root[0]), i64::from(z) - p.root[2]);
                p.radius = p.radius.max(((dx * dx + dz * dz) as f64).sqrt());
            }
            set_bit(&mut self.now_bits, index, true);
            self.cur.push(Cell { index, xyz: [x, y, z], plant, stand, weight: 0.0, faces: pack_faces(0.0, 0.0), rank: 0.0 });
        }
        // Each plant's sway, each cell's share and place in the order.
        for p in self.plants.iter_mut().skip(1) {
            let height = (p.top + 1) as f64;
            p.rec.amp = self.cfg.tip_amplitude(height, p.species) as f32;
            p.rec.lag = (self.cfg.lag_s_per_voxel * height) as f32;
        }
        for c in &mut self.cur {
            if c.plant == 0 {
                continue;
            }
            let p = &self.plants[c.plant as usize];
            let hgt = i64::from(c.xyz[1]) - p.root[1];
            c.weight = self.cfg.weight(hgt, p.top);
            c.faces = pack_faces(
                self.cfg.face_weight(hgt as f64 + 0.5, p.top),
                self.cfg.face_weight(hgt as f64 - 0.5, p.top),
            );
            c.rank = match self.cfg.order {
                GrowOrder::RootUp => {
                    if p.top > 0 { (hgt.clamp(0, p.top) as f64 / p.top as f64) as f32 } else { 0.0 }
                }
                GrowOrder::CentreOut => {
                    let (dx, dz) = (wrap(i64::from(c.xyz[0]) - p.root[0]), i64::from(c.xyz[2]) - p.root[2]);
                    let r = ((dx * dx + dz * dz) as f64).sqrt();
                    if p.radius > 0.0 { (r / p.radius) as f32 } else { 0.0 }
                }
            };
        }
    }

    /// Write the tick's list: every plant cell, and every leaving cell with its texel. `full`: the fill packed in full, which reset the palette, so the
    /// leaving cells take their styles' slots again.
    pub(super) fn build(&mut self, palette: &mut Palette, full: bool, list: &mut PlantList) {
        if self.cache_valid && self.changes.is_empty() {
            list.clone_from(&self.cache);
            list.epoch = self.now_s;
            return;
        }
        let epoch = self.now_s;
        list.cells.clear();
        list.plants.clear();
        list.plants.extend(self.plants.iter().map(|p| p.rec));
        list.epoch = epoch;
        let grow = self.grow_s();
        let mut settled_at = f64::NEG_INFINITY;
        for c in &self.cur {
            let time = match self.changes.get(&c.index) {
                Some(ch) if !ch.dying => {
                    settled_at = settled_at.max(ch.time + grow);
                    (ch.time - epoch) as f32
                }
                _ => SETTLED,
            };
            // A cell in the volume draws the volume's texel; only a leaving one carries its
            // own. The others carry their faces' weights in its place.
            list.cells.push(pack_cell(c.xyz, c.plant, c.weight, false, time, c.faces));
        }
        let mut ghosts: FxHashMap<u64, u32> = FxHashMap::default();
        for ch in self.changes.values_mut().filter(|ch| ch.dying) {
            if full && let Some(key) = &ch.key {
                // The palette started again: hold the style anew, at whatever slot it has.
                ch.texel.0[3] = palette.acquire(key).unwrap_or(0);
            }
            let plant = match ch.stand {
                None => 0,
                Some(s) => match self.by_stand.get(&s) {
                    Some(&k) => k,
                    None => *ghosts.entry(s).or_insert_with(|| {
                        list.plants.push(ch.rec);
                        (list.plants.len() - 1) as u32
                    }),
                },
            };
            settled_at = settled_at.max(ch.time + grow);
            list.cells.push(pack_cell(
                ch.xyz,
                plant,
                ch.weight,
                true,
                (ch.time - epoch) as f32,
                u32::from_le_bytes(ch.texel.0),
            ));
        }
        list.settled_at = settled_at;
        if self.changes.is_empty() {
            self.cache.clone_from(list);
            self.cache_valid = true;
        } else {
            self.cache_valid = false;
        }
    }
}

fn release_key(palette: &mut Palette, key: &StyleKey) {
    if let Some(slot) = palette.find(key) {
        palette.release(slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_gpu::plants::{SETTLED, cell_scale, unpack_cell};
    use cubarium_voxel::{Command, Config, Material, World};
    use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species};

    const FADE: f64 = 2.0;
    const GROW: f64 = 0.4;

    struct Rig {
        world: World,
        flora: Flora,
        stands: Stands,
        tracker: PlantTracker,
        palette: Palette,
        master: Vec<VoxelTexel>,
        list: PlantList,
    }

    impl Rig {
        fn new(cfg: PlantConfig) -> Rig {
            let c = Config { width: 24, height: 32, depth: 8, ..Config::default() };
            let mut world = World::empty(c.clone());
            for x in 0..i64::from(c.width) {
                for z in 0..c.depth {
                    world.apply(Command::SetMaterial { x, y: 0, z, material: Material::Soil });
                }
            }
            let fc = FloraConfig::default();
            let mut flora = Flora::new(fc.clone());
            let wood = fc.species(Species::Vaulttree).wood_max;
            assert!(flora.apply(&world, FloraCommand::Seed { x: 12, z: 4, species: Species::Vaulttree, wood }));
            let mut stands = Stands::empty(c.width, c.height, c.depth);
            stands.track_owners();
            let n = (c.width * c.height * c.depth) as usize;
            Rig {
                tracker: PlantTracker::new(cfg, FADE, c.width, c.height, c.depth),
                world,
                flora,
                stands,
                palette: Palette::new(),
                master: vec![VoxelTexel::default(); n],
                list: PlantList::default(),
            }
        }

        /// One tick at `now`, the stand there or not; the list's cells as (height, absolute
        /// time, dying, weight).
        fn tick(&mut self, tick: u64, now: f64, present: bool) -> Vec<(u32, f64, bool, f32)> {
            let view = self.world.view();
            let fv = self.flora.view();
            let none = FloraView { stands: &[], ..fv };
            let fv = if present { fv } else { none };
            self.stands.rebuild(&view, fv);
            self.tracker.detect(&view, fv, &self.stands, &self.master, |_| false, &mut self.palette, tick, now);
            self.tracker.build(&mut self.palette, false, &mut self.list);
            let epoch = self.list.epoch;
            self.list
                .cells
                .iter()
                .map(|&c| {
                    let (xyz, _, w, dying, t) = unpack_cell(c);
                    let abs = if t == SETTLED { f64::NEG_INFINITY } else { epoch + f64::from(t) };
                    (xyz[1], abs, dying, w)
                })
                .collect()
        }
    }

    fn still_order() -> PlantConfig {
        PlantConfig { jitter: 0.0, grow_s: GROW, ..PlantConfig::default() }
    }

    fn scale(at: f64, t: f64, dying: bool) -> f32 {
        cell_scale((at - t) as f32, 0.0, dying, GROW as f32)
    }

    #[test]
    fn the_first_tick_shows_every_cell_as_it_stands() {
        let mut r = Rig::new(still_order());
        let cells = r.tick(1, 10.0, true);
        assert!(cells.len() > 4, "a vaulttree of several cells");
        assert!(cells.iter().all(|c| c.1 == f64::NEG_INFINITY && !c.2), "nothing grows in on a world's first tick");
    }

    #[test]
    fn an_arriving_plant_grows_in_from_the_root_up_and_is_whole_at_the_end() {
        let mut r = Rig::new(still_order());
        assert!(r.tick(1, 0.0, false).is_empty());
        let cells = r.tick(2, 0.05, true);
        assert!(cells.iter().all(|c| c.1 >= 0.05 && !c.2), "every cell arrives, from the tick on");
        // Root up: a higher cell never starts before a lower one; the lowest first.
        let mut by_h = cells.clone();
        by_h.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        for w in by_h.windows(2) {
            assert!(w[1].1 >= w[0].1 - 1e-6, "h {} starts at {} before h {} at {}", w[1].0, w[1].1, w[0].0, w[0].1);
        }
        assert!(by_h.last().unwrap().1 > by_h[0].1, "the top waits for the root");
        // Mid-change some voxels are whole and some not yet there; at the end all whole.
        let mid = 0.05 + FADE / 2.0;
        let shown: Vec<f32> = cells.iter().map(|c| scale(mid, c.1, false)).collect();
        assert!(shown.iter().any(|&s| s == 1.0) && shown.iter().any(|&s| s < 1.0), "partial mid-change: {shown:?}");
        assert!(cells.iter().all(|c| scale(0.05 + FADE + 1e-3, c.1, false) == 1.0), "whole after foliage_fade_s");
        assert!((r.list.settled_at - (0.05 + FADE)).abs() < 1e-3, "moving until {}", r.list.settled_at);
    }

    #[test]
    fn a_leaving_plant_shrinks_from_the_top_down_and_is_then_gone() {
        let mut r = Rig::new(still_order());
        let whole = r.tick(1, 0.0, true).len();
        let cells = r.tick(2, 1.0, false);
        assert_eq!(cells.len(), whole, "every cell still drawn, leaving");
        assert!(cells.iter().all(|c| c.2), "all leaving");
        let mut by_h = cells.clone();
        by_h.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
        for w in by_h.windows(2) {
            assert!(w[1].1 <= w[0].1 + 1e-6, "top first: h {} at {} after h {} at {}", w[1].0, w[1].1, w[0].0, w[0].1);
        }
        assert!(cells.iter().all(|c| scale(1.0 + FADE + 1e-3, c.1, true) == 0.0), "gone after foliage_fade_s");
        assert!(r.tick(3, 1.0 + FADE + 0.1, false).is_empty(), "and out of the list");
    }

    #[test]
    fn a_change_reversed_half_way_goes_back_from_the_size_it_has() {
        let mut r = Rig::new(still_order());
        r.tick(1, 0.0, false);
        r.tick(2, 0.0, true);
        let at = 1.0;
        let key = |c: &[u32; 4]| unpack_cell(*c).0;
        let arriving: FxHashMap<[u32; 3], f64> = r
            .list
            .cells
            .iter()
            .map(|c| (key(c), r.list.epoch + f64::from(unpack_cell(*c).4)))
            .collect();
        let shown = arriving.values().filter(|&&t| scale(at, t, false) > 0.0).count();
        r.tick(3, at, false);
        // Each cell that had begun leaves from the size it had; one not begun is gone.
        assert_eq!(r.list.cells.len(), shown, "the cells that had begun to show");
        assert!(shown > 0 && shown < arriving.len(), "reversed mid-change");
        for c in &r.list.cells {
            let (xyz, _, _, dying, t) = unpack_cell(*c);
            assert!(dying);
            let was = scale(at, arriving[&xyz], false);
            let now = scale(at, r.list.epoch + f64::from(t), true);
            assert!((was - now).abs() < 1e-3, "{xyz:?} at {was} jumped to {now}");
            assert!(scale(at + 0.05, r.list.epoch + f64::from(t), true) < now, "and shrinks from there");
        }
    }

    #[test]
    fn the_root_never_moves_and_a_cell_moves_more_the_higher_it_is() {
        let mut r = Rig::new(still_order());
        let cells = r.tick(1, 0.0, true);
        let root = cells.iter().map(|c| c.0).min().unwrap();
        for c in &cells {
            if c.0 == root {
                assert_eq!(c.3, 0.0, "a root cell's weight");
            }
            for d in &cells {
                if d.0 > c.0 {
                    assert!(d.3 > c.3, "h {} weight {} vs h {} weight {}", d.0, d.3, c.0, c.3);
                }
            }
        }
        assert!(r.list.plants.iter().skip(1).any(|p| p.amp > 0.0), "the tree sways");
        assert_eq!(r.list.plants[0].amp, 0.0, "plant 0 is still");
    }
}
