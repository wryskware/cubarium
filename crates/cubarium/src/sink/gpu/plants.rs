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

use cubarium_gpu::plants::{PlantList, PlantRec, SETTLED, cell_scale, pack_cell, unsmooth};
use cubarium_gpu::voxel::{VoxelStaging, VoxelTexel};
use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::FloraView;

use crate::voxel::plant_motion::{GrowOrder, PlantConfig};
use crate::voxel::stand::{Part, Stands, drawn_site};

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
        // The last tick's cells become `prev`; `now_bits` is cleared for this tick's.
        for c in &self.prev {
            set_bit(&mut self.was_bits, c.index, false);
        }
        std::mem::swap(&mut self.prev, &mut self.cur);
        std::mem::swap(&mut self.was_bits, &mut self.now_bits);
        self.cur.clear();
        self.gather(view, flora, stands);

        let settle = match self.last_tick {
            None => true,
            Some(last) => tick < last || tick > last + SETTLE_GAP,
        };
        self.last_tick = Some(tick);
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
            for k in 0..self.cur.len() {
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
            for k in 0..self.prev.len() {
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
        self.prev_plants.clear();
        self.prev_plants.extend(self.plants[1..].iter().map(|p| (p.stand, *p)));
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
        for (x, y, z) in stands.cells() {
            if !matches!(stands.at(i64::from(x), i64::from(y), z), Part::Trunk(_) | Part::Crown { .. }) {
                continue;
            }
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
            self.cur.push(Cell { index, xyz: [x, y, z], plant, stand, weight: 0.0, rank: 0.0 });
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

    /// Write the tick's list: every plant cell with its texel from the filled `master`, and
    /// every leaving cell. `full`: the fill packed in full, which reset the palette, so the
    /// leaving cells take their styles' slots again.
    pub(super) fn build(&mut self, master: &[VoxelTexel], palette: &mut Palette, full: bool, list: &mut PlantList) {
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
            let t = master[c.index as usize];
            list.cells.push(pack_cell(c.xyz, c.plant, c.weight, false, time, u32::from_le_bytes(t.0)));
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
    }

    /// Cells arriving or leaving now.
    #[cfg(test)]
    pub(super) fn changing(&self) -> usize {
        self.changes.len()
    }
}

fn release_key(palette: &mut Palette, key: &StyleKey) {
    if let Some(slot) = palette.find(key) {
        palette.release(slot);
    }
}
