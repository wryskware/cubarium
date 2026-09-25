//! The light shafts' sun-visibility volume, baked on the CPU off the loop thread (package V,
//! `design/handoffs/presentation-plan-2026-09-24.md`).
//!
//! # What the volume is
//!
//! One byte per voxel: whether the sun reaches that cell's air. Its occluders are exactly
//! `voxel.frag`'s `sunReaches`: terrain, trunks, logs and animals stop a ray, and a crown
//! cell stops it or lets it through **as a whole cell**, by the same hash under the same
//! pass chance ([`crown_lets`]), so the shafts in the air line up with the cast shadows on
//! the faces. Each cell marches the same cell-by-cell walk from its own centre toward the
//! sun, with the same 128-cell cap ([`bake`]); its own cell does not count, so the ground
//! and a wall read the light of the air just in front of them and the trilinear sample of
//! the air next to a lit surface is not darkened by the surface itself.
//!
//! **Why a march per cell and not a sweep.** A sweep along the sun (a cell's pass times the
//! visibility upstream) is O(cells), but the sun is not along an axis, so "upstream" falls
//! between cells and is interpolated, and the interpolation diffuses: a shaft from a
//! one-voxel slot spreads several voxels wide over the terrarium's depth and stops lining up
//! with the hard shadows. The exact march stops at the first occluder, above the highest
//! occluder in the world at once ([`Grid::top`]), and is split over threads.
//!
//! # When it is rebaked
//!
//! [`Rebake`]: at most once every `[light] volumetric_rebake_s` on the frame clock, and
//! only when the world (its occluders, compared whole) or the sun (by more than
//! [`MIN_ANGLE`]) has changed since the last bake. A bake aims at where the sun will be
//! when its fade has ended ([`SunTrack`]), so the shafts follow a moving sun rather than
//! trailing it. **Nothing pops**: the renderer holds the last two bakes and fades from one
//! to the other over the interval on the every-frame clock
//! ([`cubarium_gpu::sunvis::SunVisVolume`]), and a bake that lands while a fade is still
//! running waits for it to finish. The first bake is drawn whole, before the founding
//! frame ends.

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

use cubarium_gpu::VoxelRenderer;
use cubarium_gpu::voxel::VoxelStaging;
use cubarium_voxel::VoxelView;

use super::light::Canopy;
use crate::voxel::animal::Animals;
use crate::voxel::appearance;
use crate::voxel::stand::{Part, Stands};

/// A cell nothing stops the sun in.
pub(crate) const OPEN: u8 = 0;
/// A cell that stops the sun: terrain, a trunk, a log, an animal.
pub(crate) const OPAQUE: u8 = 255;
/// A crown cell whose pass chance is `nibble`: `CROWN + nibble`.
pub(crate) const CROWN: u8 = 1;

/// `voxel.frag`'s `SUN_MARCH`: cells a ray crosses before it counts as lit.
pub(crate) const SUN_MARCH: usize = 128;

/// The sun moving less than this since the last bake is no reason to bake again (radians,
/// a quarter of a degree).
pub(crate) const MIN_ANGLE: f64 = 0.25 * std::f64::consts::PI / 180.0;

/// `voxel.frag`'s `cellHash`, bit for bit.
#[inline]
pub(crate) fn cell_hash(x: i32, y: i32, z: i32, salt: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA77)
        ^ (z as u32).wrapping_mul(0xC2B2_AE3D)
        ^ (salt as u32).wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h
}

/// `voxel.frag`'s `crownLets`: whether a ray gets through crown cell `(x, y, z)` (x already
/// wrapped) whose pass chance is `pass` fifteenths. `h / 65536 < pass / 15`, in integers.
#[inline]
pub(crate) fn crown_lets(pass: u8, x: i32, y: i32, z: i32) -> bool {
    let h = cell_hash(x, y, z, 32) & 0xFFFF;
    u64::from(h) * 15 < u64::from(pass) * 65536
}

/// The world's sun occluders, one byte a cell ([`OPEN`], [`OPAQUE`], or [`CROWN`] plus a
/// pass nibble) in the voxels' texture order `(z · height + y) · width + x`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Grid {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) depth: u32,
    pub(crate) occ: Vec<u8>,
}

impl Grid {
    pub(crate) fn open(width: u32, height: u32, depth: u32) -> Grid {
        Grid {
            width,
            height,
            depth,
            occ: vec![OPEN; width as usize * height as usize * depth as usize],
        }
    }

    #[inline]
    pub(crate) fn index(&self, x: u32, y: u32, z: u32) -> usize {
        VoxelStaging::index(self.width, self.height, x, y, z)
    }

    pub(crate) fn set(&mut self, x: u32, y: u32, z: u32, v: u8) {
        let i = self.index(x, y, z);
        self.occ[i] = v;
    }

    /// One past the highest cell holding anything that stops the sun: a ray above it that
    /// climbs is lit.
    pub(crate) fn top(&self) -> u32 {
        let (w, h) = (self.width as usize, self.height as usize);
        (0..h)
            .rev()
            .find(|&y| {
                self.occ
                    .chunks_exact(w * h)
                    .any(|slab| slab[y * w..(y + 1) * w].iter().any(|&v| v != OPEN))
            })
            .map_or(0, |y| y as u32 + 1)
    }
}

/// What the loop thread hands the baking thread: the terrain as it is (a copy, in the
/// world's own order) and the cells a stand or an animal occupies, with their bytes.
pub(crate) struct Snapshot {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) depth: u32,
    /// `Material as u8`, world order `(y · depth + z) · width + x`.
    pub(crate) material: Vec<u8>,
    /// `(x, y, z, byte)` for every plant or animal cell that is not [`OPEN`].
    pub(crate) parts: Vec<(u32, u32, u32, u8)>,
}

impl Snapshot {
    /// The world's occluders as the packer draws them (`Packer::fill`): a cell inside
    /// terrain is terrain; an animal's cell stops the sun; a stand's trunk and log cells
    /// stop it, its crown and heart cells carry the pass nibble the packer gives them
    /// (`Canopy::plant_byte`'s high nibble); sprouts and ground marks do not count.
    pub(crate) fn of(
        view: &VoxelView<'_>,
        stands: &Stands,
        animals: &Animals,
        canopy: Option<&Canopy>,
    ) -> Snapshot {
        let c = view.config;
        let (w, h, d) = (c.width, c.height, c.depth);
        let material: Vec<u8> = view.material.iter().map(|&m| m as u8).collect();
        let solid = |x: u32, y: u32, z: u32| {
            material[(y as usize * d as usize + z as usize) * w as usize + x as usize] != 0
        };
        let mut parts = Vec::new();
        for (x, y, z) in stands.cells() {
            if solid(x, y, z) {
                continue;
            }
            let xi = i64::from(x);
            let p = stands.at(xi, i64::from(y), z);
            let byte = match appearance::plant_class(p) {
                1 | 6 => OPAQUE,
                2 | 3 => {
                    let crown = matches!(p, Part::Crown { .. });
                    let pass = canopy.map_or(15, |cn| {
                        cn.plant_byte(stands.owner(xi, y, z), crown, x, y, z) >> 4
                    });
                    CROWN + pass
                }
                _ => continue,
            };
            parts.push((x, y, z, byte));
        }
        for (x, y, z) in animals.cells() {
            if solid(x, y, z) {
                continue;
            }
            if animals.style(animals.at(i64::from(x), i64::from(y), z)).is_some() {
                parts.push((x, y, z, OPAQUE));
            }
        }
        Snapshot {
            width: w,
            height: h,
            depth: d,
            material,
            parts,
        }
    }

    /// The occluder grid: terrain, then the parts over it (an animal after a plant in the
    /// same cell, as the packer stamps them).
    pub(crate) fn grid(&self) -> Grid {
        let mut g = Grid::open(self.width, self.height, self.depth);
        let (w, d) = (self.width as usize, self.depth as usize);
        for y in 0..self.height as usize {
            for z in 0..d {
                let src = &self.material[(y * d + z) * w..(y * d + z + 1) * w];
                let dst = g.index(0, y as u32, z as u32);
                for (o, &m) in g.occ[dst..dst + w].iter_mut().zip(src) {
                    *o = if m != 0 { OPAQUE } else { OPEN };
                }
            }
        }
        for &(x, y, z, v) in &self.parts {
            g.set(x, y, z, v);
        }
        g
    }
}

/// Whether the sun (unit direction `l`, climbing) reaches the air of cell `(x, y, z)`:
/// `voxel.frag`'s `sunReaches` from the cell's centre, its own cell not counted.
fn reaches(g: &Grid, l: [f32; 3], top: i32, x: u32, y: u32, z: u32) -> bool {
    let (w, h, d) = (g.width as i32, g.height as i32, g.depth as i32);
    let dir = l.map(|k| if k > 0.0 { 1 } else if k < 0.0 { -1 } else { 0 });
    let inv = l.map(|k| if k.abs() > 1e-6 { 1.0 / k.abs() } else { 1e30 });
    let mut c = [x as i32, y as i32, z as i32];
    // The centre is half a cell from every boundary.
    let mut next = [0.5 * inv[0], 0.5 * inv[1], 0.5 * inv[2]];
    for i in 0..SUN_MARCH {
        if c[1] >= h || c[2] < 0 || c[2] >= d || c[1] >= top {
            return true;
        }
        if c[1] < 0 {
            return false;
        }
        let axis = if next[0] < next[1] && next[0] < next[2] {
            0
        } else if next[1] < next[2] {
            1
        } else {
            2
        };
        if i > 0 {
            let xw = c[0].rem_euclid(w);
            let v = g.occ[g.index(xw as u32, c[1] as u32, c[2] as u32)];
            if v == OPAQUE {
                return false;
            }
            if v != OPEN && !crown_lets(v - CROWN, xw, c[1], c[2]) {
                return false;
            }
        }
        c[axis] += dir[axis];
        next[axis] += inv[axis];
    }
    true
}

/// Bake the volume: 255 where the sun (the direction **toward** it, any length) reaches a
/// cell's air, 0 where it does not, in `g`'s order, over `threads` threads. A sun at or
/// below the horizon reaches nothing, as it lights no face.
pub(crate) fn bake(g: &Grid, sun: [f32; 3], threads: usize, out: &mut [u8]) {
    let len = (sun[0] * sun[0] + sun[1] * sun[1] + sun[2] * sun[2]).sqrt();
    if !len.is_finite() || len <= 0.0 || sun[1] <= 0.0 {
        out.fill(0);
        return;
    }
    let l = sun.map(|k| k / len);
    let top = g.top() as i32;
    let slab = g.width as usize * g.height as usize;
    let per = (g.depth as usize).div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        for (k, chunk) in out.chunks_mut(slab * per).enumerate() {
            s.spawn(move || {
                for (j, row) in chunk.chunks_mut(g.width as usize).enumerate() {
                    let z = (k * per + j / g.height as usize) as u32;
                    let y = (j % g.height as usize) as u32;
                    for (x, o) in row.iter_mut().enumerate() {
                        *o = if reaches(g, l, top, x as u32, y, z) { 255 } else { 0 };
                    }
                }
            });
        }
    });
}

/// The angle between two directions, in radians (zero for a zero vector).
fn angle(a: [f32; 3], b: [f32; 3]) -> f64 {
    let n = |v: [f32; 3]| v.map(f64::from);
    let (a, b) = (n(a), n(b));
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let la = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    let lb = (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
    if la <= 0.0 || lb <= 0.0 {
        return if la == lb { 0.0 } else { std::f64::consts::PI };
    }
    (dot / (la * lb)).clamp(-1.0, 1.0).acos()
}

/// The sun's motion, from the directions the sink hands the renderer each frame: an axis
/// and a rate, so a bake can aim at where the sun will be.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SunTrack {
    mark: Option<(f64, [f32; 3])>,
    axis: [f64; 3],
    /// Radians a second.
    rate: f64,
}

impl SunTrack {
    /// How far apart two samples are taken, in seconds.
    const SPAN: f64 = 0.5;

    pub(crate) fn update(&mut self, now: f64, sun: [f32; 3]) {
        let Some((t, d)) = self.mark else {
            self.mark = Some((now, sun));
            return;
        };
        if now < t {
            *self = SunTrack {
                mark: Some((now, sun)),
                ..SunTrack::default()
            };
            return;
        }
        if now - t < Self::SPAN {
            return;
        }
        let (a, b) = (d.map(f64::from), sun.map(f64::from));
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let n = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        if n > 1e-12 {
            self.axis = cross.map(|k| k / n);
            self.rate = angle(d, sun) / (now - t);
        } else {
            self.rate = 0.0;
        }
        self.mark = Some((now, sun));
    }

    /// Where `sun` will be `ahead` seconds from now at the current rate (Rodrigues).
    pub(crate) fn predict(&self, sun: [f32; 3], ahead: f64) -> [f32; 3] {
        let th = self.rate * ahead.max(0.0);
        if th.abs() < 1e-9 {
            return sun;
        }
        let v = sun.map(f64::from);
        let k = self.axis;
        let (s, c) = th.sin_cos();
        let kv = [
            k[1] * v[2] - k[2] * v[1],
            k[2] * v[0] - k[0] * v[2],
            k[0] * v[1] - k[1] * v[0],
        ];
        let kd = k[0] * v[0] + k[1] * v[1] + k[2] * v[2];
        std::array::from_fn(|i| (v[i] * c + kv[i] * s + k[i] * kd * (1.0 - c)) as f32)
    }
}

/// When a bake may start ([`crate::sink::gpu::sunvis`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rebake {
    /// Seconds on the frame clock between two bakes' starts, at least.
    pub(crate) interval: f64,
    last_start: Option<f64>,
    in_flight: bool,
    /// The sun the last bake aimed at.
    baked_sun: Option<[f32; 3]>,
    /// Whether the baking thread holds a world to rebake under a new sun.
    has_world: bool,
}

impl Rebake {
    pub(crate) fn new(interval: f64) -> Rebake {
        Rebake {
            interval: interval.max(0.0),
            last_start: None,
            in_flight: false,
            baked_sun: None,
            has_world: false,
        }
    }

    /// Whether the interval since the last start has passed (or the clock went back).
    fn waited(&self, now: f64) -> bool {
        self.last_start
            .is_none_or(|t| now < t || now - t >= self.interval)
    }

    /// Whether a new world snapshot should go to the baking thread now. The thread bakes it
    /// only if its occluders or the sun changed; it is sent at most once an interval.
    pub(crate) fn world_due(&self, now: f64) -> bool {
        !self.in_flight && self.waited(now)
    }

    /// Whether the sun alone has moved enough, since the last bake aimed, to bake the
    /// world the thread holds again.
    pub(crate) fn sun_due(&self, now: f64, target: [f32; 3]) -> bool {
        !self.in_flight
            && self.has_world
            && self.last_start.is_some()
            && self.waited(now)
            && self.baked_sun.is_none_or(|b| angle(b, target) > MIN_ANGLE)
    }

    pub(crate) fn started(&mut self, now: f64, target: [f32; 3], world: bool) {
        self.last_start = Some(now);
        self.in_flight = true;
        self.baked_sun = Some(target);
        self.has_world |= world;
    }

    pub(crate) fn landed(&mut self) {
        self.in_flight = false;
    }
}

enum Job {
    World(Snapshot, [f32; 3]),
    Sun([f32; 3]),
}

/// A finished job: the new volume, or `None` when nothing it depends on had changed; and
/// its wall time in milliseconds.
type Done = (Option<Vec<u8>>, f64);

/// The baking thread: holds the last grid and sun it baked, and bakes only on a change.
struct Worker {
    jobs: Sender<Job>,
    done: Receiver<Done>,
}

impl Worker {
    fn spawn(threads: usize) -> Worker {
        let (jobs, inbox) = mpsc::channel::<Job>();
        let (outbox, done) = mpsc::channel::<Done>();
        std::thread::Builder::new()
            .name("cubarium-sunvis".into())
            .spawn(move || {
                let mut grid: Option<Grid> = None;
                let mut sun: Option<[f32; 3]> = None;
                while let Ok(job) = inbox.recv() {
                    let started = Instant::now();
                    let (changed, aim) = match job {
                        Job::World(snap, aim) => {
                            let g = snap.grid();
                            let same = grid.as_ref() == Some(&g);
                            grid = Some(g);
                            (!same, aim)
                        }
                        Job::Sun(aim) => (false, aim),
                    };
                    let moved = sun.is_none_or(|s| angle(s, aim) > MIN_ANGLE);
                    let out = match &grid {
                        Some(g) if changed || moved => {
                            let mut v = vec![0u8; g.occ.len()];
                            bake(g, aim, threads, &mut v);
                            sun = Some(aim);
                            Some(v)
                        }
                        _ => None,
                    };
                    let ms = started.elapsed().as_secs_f64() * 1e3;
                    if outbox.send((out, ms)).is_err() {
                        break;
                    }
                }
            })
            .expect("spawning the sun-visibility thread");
        Worker { jobs, done }
    }
}

/// The loop thread's half: schedules bakes, takes them in, and fades between them.
pub(crate) struct SunVisDriver {
    worker: Worker,
    rebake: Rebake,
    track: SunTrack,
    /// The bake the renderer is fading in (or shows): the next upload's red channel.
    shown: Option<Vec<u8>>,
    /// A landed bake waiting for the current fade to end.
    waiting: Option<Vec<u8>>,
    /// The current fade: its start on the frame clock and its length.
    fade: Option<(f64, f64)>,
    /// The last bake's wall time, seconds: how far ahead of the fade's end a bake aims.
    bake_s: f64,
    /// Threads a bake runs on.
    pub(crate) threads: usize,
    /// Bakes done and their total wall time (ms), and bakes skipped as unchanged.
    pub(crate) baked: u64,
    pub(crate) bake_ms: f64,
    pub(crate) unchanged: u64,
}

impl SunVisDriver {
    /// A driver rebaking at most every `interval` seconds of the frame clock.
    pub(crate) fn new(interval: f64) -> SunVisDriver {
        // Every core the process may use but one, as the sky plane's thread takes: a bake
        // is a burst of a few tens of milliseconds at most once a minute.
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .saturating_sub(1)
            .max(1);
        SunVisDriver {
            worker: Worker::spawn(threads),
            rebake: Rebake::new(interval),
            track: SunTrack::default(),
            shown: None,
            waiting: None,
            fade: None,
            bake_s: 0.0,
            threads,
            baked: 0,
            bake_ms: 0.0,
            unchanged: 0,
        }
    }

    /// Whether the first bake has yet to reach the renderer: the founding frame waits for
    /// it, and a capture stages until it has.
    pub(crate) fn pending(&self) -> bool {
        self.shown.is_none()
    }

    /// Where a bake started now should aim: the sun when its fade will have ended.
    fn aim(&self, sun: [f32; 3]) -> [f32; 3] {
        self.track.predict(sun, self.rebake.interval + self.bake_s)
    }

    /// After a pack: hand the thread a new world when one is due, and take in a finished
    /// bake.
    pub(crate) fn after_pack(
        &mut self,
        now: f64,
        sun: [f32; 3],
        snapshot: impl FnOnce() -> Snapshot,
        renderer: &mut VoxelRenderer,
    ) {
        self.track.update(now, sun);
        self.land(now, renderer);
        if self.rebake.world_due(now) {
            let aim = self.aim(sun);
            if self.worker.jobs.send(Job::World(snapshot(), aim)).is_ok() {
                self.rebake.started(now, aim, true);
            }
        }
    }

    /// Every frame: take in a finished bake, move the fade, and rebake under a sun that
    /// has moved (a world that has not been packed since, as in a capture, keeps its grid).
    pub(crate) fn frame(&mut self, now: f64, sun: [f32; 3], renderer: &mut VoxelRenderer) {
        self.track.update(now, sun);
        self.land(now, renderer);
        if let Some((start, len)) = self.fade {
            let f = if len > 0.0 { (now - start) / len } else { 1.0 };
            renderer.set_sunvis_fade(f.clamp(0.0, 1.0) as f32);
        }
        let aim = self.aim(sun);
        if self.rebake.sun_due(now, aim) && self.worker.jobs.send(Job::Sun(aim)).is_ok() {
            self.rebake.started(now, aim, false);
        }
    }

    /// Take in whatever the thread has finished, and upload the newest bake once the fade
    /// before it has ended and the renderer has room.
    fn land(&mut self, now: f64, renderer: &mut VoxelRenderer) {
        while let Ok((bake, ms)) = self.worker.done.try_recv() {
            self.rebake.landed();
            self.bake_s = ms / 1e3;
            match bake {
                Some(v) => {
                    self.baked += 1;
                    self.bake_ms += ms;
                    self.waiting = Some(v);
                }
                None => self.unchanged += 1,
            }
        }
        let faded = self
            .fade
            .is_none_or(|(start, len)| now < start || now - start >= len);
        if self.waiting.is_none() || !faded || renderer.sunvis_accepts().is_none() {
            return;
        }
        let new = self.waiting.take().expect("checked");
        // The first bake is drawn whole; every later one fades in from the one before.
        let (rg, fade) = match &self.shown {
            Some(old) => (interleave(old, &new), 0.0),
            None => (interleave(&new, &new), 1.0),
        };
        if renderer.put_sunvis(&rg, fade) {
            if self.shown.is_some() {
                self.fade = Some((now, self.rebake.interval));
            }
            self.shown = Some(new);
        } else {
            self.waiting = Some(new);
        }
    }
}

/// The renderer's two channels: `from` in red, `to` in green. Runs on the loop thread when
/// a bake lands, so it is written to vectorise.
fn interleave(from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut rg = vec![0u8; from.len() * 2];
    for (o, (&a, &b)) in rg.chunks_exact_mut(2).zip(from.iter().zip(to)) {
        o[0] = a;
        o[1] = b;
    }
    rg
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUN: [f32; 3] = [-1.0, 2.0, -1.0];

    /// A cell's visibility in a baked volume.
    fn vis(g: &Grid, out: &[u8], x: u32, y: u32, z: u32) -> u8 {
        out[g.index(x, y, z)]
    }

    fn baked(g: &Grid, sun: [f32; 3]) -> Vec<u8> {
        let mut out = vec![7u8; g.occ.len()];
        bake(g, sun, 3, &mut out);
        out
    }

    #[test]
    fn an_occluder_shades_the_cells_downstream_along_the_sun_and_nothing_upstream() {
        // Straight up: the shadow of a block is the column under it.
        let mut g = Grid::open(8, 12, 6);
        g.set(3, 8, 2, OPAQUE);
        let out = baked(&g, [0.0, 1.0, 0.0]);
        for y in 0..8 {
            assert_eq!(vis(&g, &out, 3, y, 2), 0, "under the block at y {y}");
        }
        for y in 9..12 {
            assert_eq!(vis(&g, &out, 3, y, 2), 255, "above the block at y {y}");
        }
        assert_eq!(vis(&g, &out, 2, 4, 2), 255);
        assert_eq!(vis(&g, &out, 3, 4, 3), 255);
        // The block's own cell reads the air in front of it: lit.
        assert_eq!(vis(&g, &out, 3, 8, 2), 255);

        // The default sun (up-left, toward the camera): the shadow runs down, right and
        // back, one x and one z for every two y.
        let mut g = Grid::open(16, 16, 12);
        g.set(4, 10, 3, OPAQUE);
        let out = baked(&g, SUN);
        assert_eq!(vis(&g, &out, 5, 8, 4), 0, "two below, one right, one back");
        assert_eq!(vis(&g, &out, 6, 6, 5), 0, "four below, two right, two back");
        assert_eq!(vis(&g, &out, 3, 12, 2), 255, "upstream");
        assert_eq!(vis(&g, &out, 5, 8, 2), 255, "beside the shadow");
    }

    #[test]
    fn a_sun_below_the_horizon_reaches_nothing_and_ground_reads_its_air() {
        let mut g = Grid::open(4, 6, 4);
        for x in 0..4 {
            for z in 0..4 {
                g.set(x, 0, z, OPAQUE);
            }
        }
        assert!(baked(&g, [0.0, -1.0, -1.0]).iter().all(|&v| v == 0));
        assert!(baked(&g, [0.0; 3]).iter().all(|&v| v == 0));
        let out = baked(&g, [0.0, 1.0, 0.0]);
        assert_eq!(vis(&g, &out, 1, 0, 1), 255, "the floor's own cell reads the open sky");
        assert_eq!(g.top(), 1);
    }

    #[test]
    fn a_crown_cell_passes_or_blocks_the_whole_cell_by_the_shaders_hash() {
        // The shader's hash, reproduced: fixed values from `cellHash`'s constants.
        assert_eq!(cell_hash(0, 0, 0, 0), 0);
        let h = cell_hash(3, 5, 7, 32);
        assert!(crown_lets(15, 3, 5, 7), "pass 15 lets every ray through");
        assert!(!crown_lets(0, 3, 5, 7), "pass 0 blocks every ray");
        assert_eq!(crown_lets(8, 3, 5, 7), (h & 0xFFFF) as f64 / 65536.0 < 8.0 / 15.0);

        // A column of crown cells above a floor: a cell under it is lit exactly when every
        // crown cell its (vertical) ray crosses lets it through.
        let (w, h, d) = (32, 6, 16);
        let mut g = Grid::open(w, h, d);
        for x in 0..w {
            for z in 0..d {
                g.set(x, 4, z, CROWN + 8);
            }
        }
        let out = baked(&g, [0.0, 1.0, 0.0]);
        let mut lit = 0;
        for x in 0..w {
            for z in 0..d {
                let expect = crown_lets(8, x as i32, 4, z as i32);
                assert_eq!(vis(&g, &out, x, 2, z) == 255, expect, "cell ({x}, 2, {z})");
                lit += usize::from(expect);
            }
        }
        // The mean is the pass chance.
        let mean = lit as f64 / (w * d) as f64;
        assert!((mean - 8.0 / 15.0).abs() < 0.12, "mean pass {mean}");
    }

    #[test]
    fn x_wraps_like_the_strip() {
        // The default sun is up-left, so a block's shadow falls to its right: from the
        // last column it wraps onto the first.
        let mut g = Grid::open(8, 10, 8);
        g.set(7, 6, 2, OPAQUE);
        let out = baked(&g, SUN);
        assert_eq!(vis(&g, &out, 0, 4, 3), 0);
    }

    #[test]
    fn the_rebake_throttle_respects_the_interval() {
        let mut r = Rebake::new(60.0);
        assert!(r.world_due(0.0), "the first bake is due at once");
        r.started(0.0, SUN, true);
        assert!(!r.world_due(1.0), "in flight");
        r.landed();
        assert!(!r.world_due(59.9));
        assert!(!r.sun_due(59.9, [1.0, 2.0, -1.0]), "a moved sun waits for the interval too");
        assert!(r.world_due(60.0));
        assert!(r.sun_due(60.0, [1.0, 2.0, -1.0]));
        assert!(!r.sun_due(60.0, SUN), "an unmoved sun is no reason");
        r.started(60.0, [1.0, 2.0, -1.0], false);
        r.landed();
        assert!(!r.world_due(119.0) && r.world_due(120.0));
        // A clock that goes back (a capture rewinding) does not wait out the interval.
        assert!(r.world_due(10.0));
    }

    #[test]
    fn a_sun_only_rebake_needs_a_world_on_the_thread() {
        let mut r = Rebake::new(1.0);
        assert!(!r.sun_due(5.0, SUN));
        r.started(0.0, SUN, true);
        r.landed();
        assert!(r.sun_due(5.0, [0.0, 1.0, -1.0]));
    }

    #[test]
    fn the_track_aims_where_a_turning_sun_will_be() {
        let mut t = SunTrack::default();
        let at = |deg: f64| {
            let r = deg.to_radians();
            [r.sin() as f32, 1.0, -(r.cos() as f32)]
        };
        t.update(0.0, at(0.0));
        t.update(1.0, at(10.0)); // turning about y (the vectors are not unit, the axis is)
        let ahead = t.predict(at(10.0), 2.0);
        // Not exactly at(30): the samples are not unit length. Near it.
        let a = angle(ahead, at(30.0)).to_degrees();
        assert!(a < 3.0, "aimed {a:.2} degrees off");
        let mut still = SunTrack::default();
        still.update(0.0, SUN);
        still.update(1.0, SUN);
        assert_eq!(still.predict(SUN, 60.0), SUN);
    }

    #[test]
    fn the_snapshot_grid_puts_terrain_under_parts() {
        let s = Snapshot {
            width: 3,
            height: 2,
            depth: 2,
            // world order (y · depth + z) · width + x: soil at (1, 0, 1).
            material: vec![0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0],
            parts: vec![(2, 1, 0, CROWN + 4), (0, 1, 1, OPAQUE)],
        };
        let g = s.grid();
        assert_eq!(g.occ[g.index(1, 0, 1)], OPAQUE);
        assert_eq!(g.occ[g.index(2, 1, 0)], CROWN + 4);
        assert_eq!(g.occ[g.index(0, 1, 1)], OPAQUE);
        assert_eq!(g.occ.iter().filter(|&&v| v != OPEN).count(), 3);
    }
}
