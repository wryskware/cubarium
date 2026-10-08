//! **The view feed**: what a presentation (Godot) draws from, built on the loop's thread
//! after the ticks of each clock period and handed off without a lock the loop could wait
//! on.
//!
//! **Terrain and water** come as chunks of [`CHUNK`]³ voxels. A chunk is
//! `3 · CHUNK³` bytes, three planes one after the other — material, free water, pore
//! water — each in `x`-fastest, then `y` (up), then `z` (into the depth) order:
//! `local = (lz · CHUNK + ly) · CHUNK + lx`. Cells past the world's edge (a size that is
//! not a multiple of [`CHUNK`]) read as air and dry. Material is
//! [`cubarium_voxel::Material`]'s discriminant (air 0, bedrock 1, rock 2, soil 3); free
//! water is the void fraction full, 0–255, and never 0 for a cell that holds any
//! (`> WATER_EPSILON`); pore water is the pore fraction full, 0–255.
//!
//! Only chunks whose bytes changed are sent: material changes are found by the world's
//! `terrain_version`, free water by its wet set (the cells wet now and the cells wet at
//! the last update), pore water by a sweep over the whole grid spread across
//! [`FeedConfig::pore_sweep`] updates. The first update after a world is installed sends
//! every chunk.
//!
//! **Organisms** are plain records, species as ids ([`crate::species_table`]): animals by
//! id, lineage, place, heading, behaviour state, mobility mode, life stage, flags and size;
//! plants by id, species, site and growth.
//!
//! **Sky**: the weather, the light and the day clock.

use std::sync::Arc;

use cubarium_observatory::query;
use cubarium_voxel::World;
use cubarium_voxel_fauna::Fauna;
use cubarium_voxel_flora::Flora;
use serde::{Deserialize, Serialize};

/// Voxels along a chunk's edge.
pub const CHUNK: u32 = 16;
/// Voxels in a chunk.
pub const CHUNK_CELLS: usize = (CHUNK * CHUNK * CHUNK) as usize;
/// Bytes in a chunk: the material, free-water and pore-water planes.
pub const CHUNK_BYTES: usize = 3 * CHUNK_CELLS;
/// Free water at or below this void fraction is drawn dry. A presentation threshold, not
/// a number of the simulation.
pub const WATER_EPSILON: f64 = 1e-4;

/// How the feed spends its time. Execution and presentation only: no rule, no number of
/// the simulation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeedConfig {
    /// The pore-water sweep covers the grid once every this many feed updates (one
    /// update per clock period, 20 a second): pore water creeps, and a full sweep every
    /// update is most of the feed's cost on a large world.
    pub pore_sweep: u32,
}

impl Default for FeedConfig {
    fn default() -> FeedConfig {
        FeedConfig { pore_sweep: 4 }
    }
}

/// One chunk's bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct ChunkData {
    /// The chunk's coordinates in chunks: `[x, y, z]`.
    pub at: [u32; 3],
    /// [`CHUNK_BYTES`]: the material, free-water and pore-water planes.
    pub bytes: Box<[u8]>,
}

/// The animals, one entry per body in each column (structure of arrays).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Animals {
    pub ids: Vec<i64>,
    /// The lineage ([`cubarium_voxel_fauna::Founder::index`]), or −1 for a body without one.
    pub species: Vec<i32>,
    /// Metres: `x` along the ring (wrapped), `y` up (the body's base), `z` into the depth;
    /// three per body.
    pub pos: Vec<f32>,
    /// Radians from +z toward +x.
    pub heading: Vec<f32>,
    /// The behaviour state ([`crate::species_table`]'s `states`).
    pub state: Vec<u8>,
    /// How it moves: ground, wall, climb, fly, glide, perched.
    pub mobility: Vec<u8>,
    /// Life stage ([`crate::species_table`]'s `stages`).
    pub stage: Vec<u8>,
    /// Bits ([`crate::species_table`]'s `flags`): striking, riding, hunkered, yielding,
    /// alert, wounded.
    pub flags: Vec<u8>,
    /// Body over its lineage's breeding size: below 1 is a juvenile's share of it.
    pub size: Vec<f32>,
}

/// The plants (stands), one entry per stand in each column.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plants {
    pub ids: Vec<i64>,
    /// [`cubarium_voxel_flora::Species::index`].
    pub species: Vec<i32>,
    /// The support face it stands on, voxels `[x, y, z]`; three per stand.
    pub site: Vec<i32>,
    /// Wood over the species' grown wood, 0–1: how grown it is.
    pub growth: Vec<f32>,
    /// Foliage, the species' own units (0 is bare).
    pub foliage: Vec<f32>,
}

/// The weather, the light and the day clock.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sky {
    pub tick: u64,
    /// The world's day (from 1), where in it (`0..1`, 0.5 noon) and the 24-hour dial.
    pub day: i64,
    pub day_phase: f32,
    pub hour: f32,
    pub daylight: f32,
    pub sun_elevation: f32,
    pub cloud_cover: f32,
    pub fog: f32,
    /// `clear`, `drizzle`, `shower` or `downpour`, and the rate falling, m/s.
    pub rain: String,
    pub rain_m_per_s: f32,
    pub temperature_c: f32,
    pub rh: f32,
    /// The latest lightning strike: `[tick, x, z, seed]`.
    pub strike: Option<[u64; 4]>,
}

/// Everything but the terrain, as of one tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub tick: u64,
    pub animals: Animals,
    pub plants: Plants,
    pub sky: Sky,
}

impl Frame {
    /// The frame of the world in hand.
    pub fn of(world: &World, flora: &Flora, fauna: &Fauna) -> Frame {
        let config = fauna.config();
        let view = fauna.view();
        let n = view.animals.len();
        let mut a = Animals {
            ids: Vec::with_capacity(n),
            species: Vec::with_capacity(n),
            pos: Vec::with_capacity(3 * n),
            heading: Vec::with_capacity(n),
            state: Vec::with_capacity(n),
            mobility: Vec::with_capacity(n),
            stage: Vec::with_capacity(n),
            flags: Vec::with_capacity(n),
            size: Vec::with_capacity(n),
        };
        for b in view.animals {
            a.ids.push(b.id as i64);
            a.species.push(b.founder.map_or(-1, |f| f.index() as i32));
            a.pos.extend([b.pose.x as f32, b.pose.y_m as f32, b.pose.z as f32]);
            a.heading.push(b.pose.heading_rad as f32);
            a.state.push(b.state as u8);
            a.mobility.push(b.mobility.mode as u8);
            a.stage.push(stage_code(query::stage_of(config, b)));
            let mut flags = 0u8;
            if b.strike.is_some() {
                flags |= FLAG_STRIKING;
            }
            if b.attached.is_some() {
                flags |= FLAG_RIDING;
            }
            if b.hunkered {
                flags |= FLAG_HUNKERED;
            }
            if b.yielding {
                flags |= FLAG_YIELDING;
            }
            if b.alert_ticks > 0 {
                flags |= FLAG_ALERT;
            }
            if b.wound > 0.0 {
                flags |= FLAG_WOUNDED;
            }
            a.flags.push(flags);
            let breed = match b.founder {
                Some(f) => config.founder(f).core.birth_body,
                None => config.species(b.species).birth_body,
            };
            a.size.push(if breed > 0.0 { (b.body / breed) as f32 } else { 1.0 });
        }
        let fv = flora.view();
        let fc = flora.config();
        let m = fv.stands.len();
        let mut p = Plants {
            ids: Vec::with_capacity(m),
            species: Vec::with_capacity(m),
            site: Vec::with_capacity(3 * m),
            growth: Vec::with_capacity(m),
            foliage: Vec::with_capacity(m),
        };
        for s in fv.stands {
            p.ids.push(s.id as i64);
            p.species.push(s.species.index() as i32);
            p.site.extend([s.site.x as i32, s.site.y as i32, s.site.z as i32]);
            let max = fc.species(s.species).wood_max;
            p.growth.push(if max > 0.0 { (s.wood / max).clamp(0.0, 1.0) as f32 } else { 1.0 });
            p.foliage.push(s.foliage as f32);
        }
        Frame { tick: world.tick(), animals: a, plants: p, sky: sky_of(world) }
    }
}

/// [`Animals::flags`] bits.
pub const FLAG_STRIKING: u8 = 1;
pub const FLAG_RIDING: u8 = 2;
pub const FLAG_HUNKERED: u8 = 4;
pub const FLAG_YIELDING: u8 = 8;
pub const FLAG_ALERT: u8 = 16;
pub const FLAG_WOUNDED: u8 = 32;

/// [`Animals::stage`]'s codes, in this order.
pub const STAGES: [query::LifeStage; 4] = [
    query::LifeStage::Rider,
    query::LifeStage::Pup,
    query::LifeStage::Juvenile,
    query::LifeStage::Adult,
];

fn stage_code(s: query::LifeStage) -> u8 {
    STAGES.iter().position(|&x| x == s).unwrap_or(0) as u8
}

fn sky_of(world: &World) -> Sky {
    let view = world.view();
    let w = &view.weather;
    let clock = query::clock(world);
    Sky {
        tick: world.tick(),
        day: clock.day,
        day_phase: w.day_phase,
        hour: clock.hour as f32,
        daylight: w.daylight,
        sun_elevation: w.sun_elevation,
        cloud_cover: w.cloud_cover,
        fog: w.fog,
        rain: query::rain_name(w.mode).to_string(),
        rain_m_per_s: w.rain_m_per_s,
        temperature_c: w.temperature_c,
        rh: w.rh,
        strike: w.last_strike.map(|s| [s.tick, u64::from(s.x), u64::from(s.z), u64::from(s.seed)]),
    }
}

/// The free-water byte of a cell holding `free` of its void.
#[inline]
pub fn water_byte(free: f64) -> u8 {
    if free > WATER_EPSILON {
        ((free.min(1.0) * 255.0).round() as u8).max(1)
    } else {
        0
    }
}

/// The pore-water byte of a cell holding `pore` of its pore space.
#[inline]
pub fn pore_byte(pore: f64) -> u8 {
    (pore.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// **The change tracker**: the bytes last sent, per cell, and which chunks have moved
/// since. Lives on the loop's thread with the world it tracks.
pub struct Tracker {
    w: usize,
    h: usize,
    d: usize,
    /// Chunks along x, y, z.
    cw: usize,
    ch: usize,
    cd: usize,
    material: Vec<u8>,
    free: Vec<u8>,
    pore: Vec<u8>,
    terrain_version: u64,
    wet_before: Vec<usize>,
    wet_now: Vec<usize>,
    dirty_bits: Vec<u64>,
    dirty: Vec<u32>,
    pore_cursor: usize,
    config: FeedConfig,
}

impl Tracker {
    /// A tracker of `world` as it stands, with every chunk owed.
    pub fn new(world: &World, config: FeedConfig) -> Tracker {
        let c = world.config();
        let (w, h, d) = (c.width as usize, c.height as usize, c.depth as usize);
        let k = CHUNK as usize;
        let (cw, ch, cd) = (w.div_ceil(k), h.div_ceil(k), d.div_ceil(k));
        let view = world.view();
        let material: Vec<u8> = view.material.iter().map(|&m| m as u8).collect();
        let free: Vec<u8> = view.free.iter().map(|&f| water_byte(f)).collect();
        let pore: Vec<u8> = view.pore.iter().map(|&p| pore_byte(p)).collect();
        let mut wet_before = Vec::new();
        if !view.wet_cells(&mut wet_before) {
            wet_before = (0..free.len()).filter(|&i| free[i] > 0).collect();
        }
        let chunks = cw * ch * cd;
        let mut t = Tracker {
            w,
            h,
            d,
            cw,
            ch,
            cd,
            material,
            free,
            pore,
            terrain_version: view.terrain_version,
            wet_before,
            wet_now: Vec::new(),
            dirty_bits: vec![0; chunks.div_ceil(64)],
            dirty: Vec::new(),
            pore_cursor: 0,
            config,
        };
        for i in 0..chunks as u32 {
            t.mark_chunk(i);
        }
        t
    }

    /// The world's `[width, height, depth]` and the chunk grid's.
    pub fn dims(&self) -> ([u32; 3], [u32; 3]) {
        (
            [self.w as u32, self.h as u32, self.d as u32],
            [self.cw as u32, self.ch as u32, self.cd as u32],
        )
    }

    #[inline]
    fn chunk_of_cell(&self, i: usize) -> u32 {
        let x = i % self.w;
        let rest = i / self.w;
        let z = rest % self.d;
        let y = rest / self.d;
        let k = CHUNK as usize;
        ((z / k * self.ch + y / k) * self.cw + x / k) as u32
    }

    #[inline]
    fn mark_chunk(&mut self, c: u32) {
        let (word, bit) = ((c / 64) as usize, 1u64 << (c % 64));
        if self.dirty_bits[word] & bit == 0 {
            self.dirty_bits[word] |= bit;
            self.dirty.push(c);
        }
    }

    #[inline]
    fn mark_cell(&mut self, i: usize) {
        let c = self.chunk_of_cell(i);
        self.mark_chunk(c);
    }

    /// Find what moved in `world` since the last update and mark its chunks.
    pub fn update(&mut self, world: &World) {
        let view = world.view();
        // The terrain: only when a material change was committed.
        if view.terrain_version != self.terrain_version {
            self.terrain_version = view.terrain_version;
            for i in 0..self.material.len() {
                let m = view.material[i] as u8;
                if m != self.material[i] {
                    self.material[i] = m;
                    self.mark_cell(i);
                }
            }
        }
        // Free water: the cells wet now and the cells that were.
        let mut now = std::mem::take(&mut self.wet_now);
        if !view.wet_cells(&mut now) {
            // No trusted wet set (a world loaded and not yet stepped): look at every cell.
            now.clear();
            now.extend((0..view.free.len()).filter(|&i| view.free[i] > WATER_EPSILON));
            for i in 0..view.free.len() {
                self.check_free(i, view.free[i]);
            }
        } else {
            for k in 0..now.len() {
                let i = now[k];
                self.check_free(i, view.free[i]);
            }
            let before = std::mem::take(&mut self.wet_before);
            for &i in &before {
                self.check_free(i, view.free[i]);
            }
            self.wet_before = before;
        }
        self.wet_now = std::mem::replace(&mut self.wet_before, now);
        // Pore water: this update's share of the sweep, compared a slice at a time.
        let n = self.pore.len();
        let share = n.div_ceil(self.config.pore_sweep.max(1) as usize).max(1);
        let mut left = share.min(n);
        while left > 0 {
            let start = self.pore_cursor;
            let len = left.min(n - start);
            let mut i = start;
            while i < start + len {
                let b = pore_byte(view.pore[i]);
                if b != self.pore[i] {
                    self.pore[i] = b;
                    self.mark_cell(i);
                }
                i += 1;
            }
            left -= len;
            self.pore_cursor = (start + len) % n;
        }
    }

    #[inline]
    fn check_free(&mut self, i: usize, free: f64) {
        let b = water_byte(free);
        if b != self.free[i] {
            self.free[i] = b;
            self.mark_cell(i);
        }
    }

    /// The chunks marked since the last call, their bytes from what was last seen, and
    /// the marks cleared.
    pub fn take_dirty(&mut self) -> Vec<ChunkData> {
        let dirty = std::mem::take(&mut self.dirty);
        let out = dirty.iter().map(|&c| self.chunk(c)).collect();
        for &c in &dirty {
            self.dirty_bits[(c / 64) as usize] = 0;
        }
        self.dirty = dirty;
        self.dirty.clear();
        out
    }

    /// Every chunk's bytes.
    pub fn all(&self) -> Vec<ChunkData> {
        (0..(self.cw * self.ch * self.cd) as u32).map(|c| self.chunk(c)).collect()
    }

    /// Whether anything is marked.
    pub fn has_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }

    fn chunk(&self, c: u32) -> ChunkData {
        let c = c as usize;
        let cx = c % self.cw;
        let cy = (c / self.cw) % self.ch;
        let cz = c / (self.cw * self.ch);
        let k = CHUNK as usize;
        let mut bytes = vec![0u8; CHUNK_BYTES].into_boxed_slice();
        let (x0, y0, z0) = (cx * k, cy * k, cz * k);
        let xs = k.min(self.w - x0);
        for lz in 0..k.min(self.d - z0) {
            for ly in 0..k.min(self.h - y0) {
                let src = ((y0 + ly) * self.d + z0 + lz) * self.w + x0;
                let dst = (lz * k + ly) * k;
                bytes[dst..dst + xs].copy_from_slice(&self.material[src..src + xs]);
                bytes[CHUNK_CELLS + dst..CHUNK_CELLS + dst + xs].copy_from_slice(&self.free[src..src + xs]);
                bytes[2 * CHUNK_CELLS + dst..2 * CHUNK_CELLS + dst + xs]
                    .copy_from_slice(&self.pore[src..src + xs]);
            }
        }
        ChunkData { at: [cx as u32, cy as u32, cz as u32], bytes }
    }
}

/// What a reader takes: the chunks that changed since its last take (all of them on the
/// first take after a world is installed), and the newest frame.
#[derive(Clone, Debug, Default)]
pub struct View {
    /// Counts installed worlds: a new value means "drop everything, this is another world".
    pub generation: u64,
    /// The world's `[width, height, depth]` in voxels, and the chunk grid's.
    pub dims: [u32; 3],
    pub chunk_dims: [u32; 3],
    pub voxel_m: f64,
    /// Every chunk is here (a first take).
    pub full: bool,
    /// Sorted by chunk index (`(z · cy + y) · cx + x`).
    pub chunks: Vec<ChunkData>,
    /// The newest frame, if one has been built.
    pub frame: Option<Arc<Frame>>,
}

impl View {
    /// A whole view of the world in hand: every chunk and its frame. What a candidate's
    /// thumbnail is drawn from.
    pub fn whole(world: &World, flora: &Flora, fauna: &Fauna, generation: u64) -> View {
        let t = Tracker::new(world, FeedConfig::default());
        let (dims, chunk_dims) = t.dims();
        View {
            generation,
            dims,
            chunk_dims,
            voxel_m: world.config().voxel_m,
            full: true,
            chunks: t.all(),
            frame: Some(Arc::new(Frame::of(world, flora, fauna))),
        }
    }
}
