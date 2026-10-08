//! **The renderer hosted in an app** (H1 of `design/handoffs/hybrid-renderer-2026-10-07.md`,
//! D-0005): `cubarium voxel`'s GPU sink drawing a [`cubarium_session::Session`]'s world into
//! frames another program shows — the Godot app's world view.
//!
//! # How a frame flows
//!
//! - **The session's loop** steps the world and, through a [`LoopHook`], copies what the
//!   pack reads into a [`PackSnapshot`] once a loop period when the world moved, and hands
//!   it to the pack thread ([`PackLane`]). That copy is all the pack costs the loop (H3).
//! - **The pack thread** packs and stages each snapshot into the sink while the loop steps
//!   the next tick. After a pack it publishes a [`PickMap`] (which animal each drawn body
//!   cell belongs to, and which voxels are solid) and moves the frame clock to the tick it
//!   packed, so the frames between ticks run on from the world they show.
//! - **The presenter thread** draws frames at the display rate: the frame clock (the tick
//!   plus the fraction elapsed toward the next), the view's turn round the ring and the
//!   highlight, then the renderer's passes and a copy of the raster into a
//!   [`ReadbackRing`] slot. It never waits on the GPU: it polls the ring, and a finished
//!   slot's bytes go to the [`Mailbox`], where the newest frame replaces an untaken one. A
//!   frame is drawn only when the picture would change (the renderer's content version).
//! - **The app** takes the newest frame when it likes ([`Hosted::take_frame`]) and shows it
//!   with the same fit and nearest upscale as the desktop window.
//!
//! One lock, the sink's, is shared by the pack thread and the presenter (to record and to
//! retire). The bytes are copied out under it. The loop never takes it after a world is
//! installed.
//!
//! A static world (a candidate in the new-world picker) is staged once from a
//! [`CandidateSource`] on a thread of its own and drawn the same way; its clock stands
//! still, and it redraws only when the view turns or the highlight moves. The stager holds
//! the candidate's world by its own handle, never the session's candidate list (which the
//! picker reads every frame), and still worlds stage one at a time across the process, in
//! the order they were asked for ([`StageTurns`]): each one's sky plane takes every core
//! but one, so the first thumbnail is drawn in one plane's time rather than all of them
//! together in four's.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cubarium_gpu::target::readback::ReadbackRing;
use cubarium_gpu::voxel::Highlight;
use cubarium_session::{CandidateSource, HookClock, LoopHook};
use cubarium_voxel::World;
use cubarium_voxel_fauna::Fauna;
use cubarium_voxel_flora::Flora;
use rustc_hash::FxHashMap;

use super::snapshot::{PackSnapshot, SnapshotCopier};
use super::{Offscreen, VoxelGpuSink};
use crate::voxel::figure::FigurePick;
use crate::voxel::project::Projection;
use crate::voxel::{PX_AUTO, VoxelConfig, cover_px_per_voxel, load_config, load_models};

/// Read-back slots: one being copied out while the GPU draws the next.
const RING: usize = 2;
/// How long a static world waits for the lit tier's sky plane before it is drawn anyway.
const STATIC_LIGHT_WAIT: Duration = Duration::from_secs(20);
/// How many still worlds stage at once in this process ([`StageTurns`]).
const STATIC_STAGERS: usize = 1;
/// How often a waiting stager looks at its quit flag (a shutdown joins it, on the app's
/// thread).
const STAGE_SLICE: Duration = Duration::from_millis(10);
/// How often a still world waiting only for the light shafts' first bake packs again (the
/// bake lands only through a pack); the pack thread's wait for the lit tier's sky plane too.
const RESTAGE_EVERY: Duration = Duration::from_millis(50);
/// How soon the pack thread tries a snapshot again that the renderer refused (every staging
/// buffer was still being read).
const REFUSED_RETRY: Duration = Duration::from_millis(2);

/// The process's still-world stagers take turns ([`StageTurns`]).
static STAGE_TURNS: StageTurns = StageTurns::new(STATIC_STAGERS);

// --- turns ----------------------------------------------------------------------------------

/// **Still worlds stage a few at a time, first asked first served.** A stager's work is
/// opening a renderer, packing the world and waiting for its sky plane, which is computed
/// on every core but one; several at once only share the cores, so all of them finish late
/// together. A stager that is told to quit while it waits leaves the queue at once.
pub struct StageTurns {
    state: Mutex<TurnQueue>,
    moved: Condvar,
}

struct TurnQueue {
    limit: usize,
    held: usize,
    next: u64,
    waiting: VecDeque<u64>,
}

/// A turn taken from [`StageTurns::take`]; dropping it gives the turn back.
pub struct StageTurn<'a>(&'a StageTurns);

impl StageTurns {
    /// At most `limit` turns held at once (at least one).
    pub const fn new(limit: usize) -> StageTurns {
        StageTurns {
            state: Mutex::new(TurnQueue {
                limit: if limit == 0 { 1 } else { limit },
                held: 0,
                next: 0,
                waiting: VecDeque::new(),
            }),
            moved: Condvar::new(),
        }
    }

    /// Wait for a turn, behind everyone who asked before; `None` once `give_up` says so
    /// (looked at every [`STAGE_SLICE`] at least).
    pub fn take(&self, give_up: impl Fn() -> bool) -> Option<StageTurn<'_>> {
        let mut q = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let ticket = q.next;
        q.next += 1;
        q.waiting.push_back(ticket);
        loop {
            if give_up() {
                q.waiting.retain(|&t| t != ticket);
                drop(q);
                self.moved.notify_all();
                return None;
            }
            if q.waiting.front() == Some(&ticket) && q.held < q.limit {
                q.waiting.pop_front();
                q.held += 1;
                drop(q);
                // The next in line may fit too.
                self.moved.notify_all();
                return Some(StageTurn(self));
            }
            q = self.moved.wait_timeout(q, STAGE_SLICE).unwrap_or_else(|e| e.into_inner()).0;
        }
    }

    /// Have every waiting stager look at its `give_up` now.
    pub fn wake(&self) {
        self.moved.notify_all();
    }

    /// Stagers waiting for a turn.
    pub fn waiting(&self) -> usize {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).waiting.len()
    }
}

impl Drop for StageTurn<'_> {
    fn drop(&mut self) {
        let mut q = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        q.held -= 1;
        drop(q);
        self.0.moved.notify_all();
    }
}

// --- the mailbox ------------------------------------------------------------------------

/// **The newest value wins.** A writer posts and never waits on a reader: posting over an
/// untaken value replaces it (and hands the old one back, to reuse its buffer); a reader
/// takes whatever is newest, or nothing. The lock is held only for a swap, never while a
/// value is read or written.
pub struct Mailbox<T> {
    slot: Mutex<Option<T>>,
    posted: AtomicU64,
    displaced: AtomicU64,
}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Mailbox { slot: Mutex::new(None), posted: AtomicU64::new(0), displaced: AtomicU64::new(0) }
    }
}

impl<T> Mailbox<T> {
    /// Leave `value` for the reader; returns the untaken one it replaced.
    pub fn post(&self, value: T) -> Option<T> {
        self.posted.fetch_add(1, Ordering::Relaxed);
        let old = self.slot.lock().map(|mut s| s.replace(value)).unwrap_or(None);
        if old.is_some() {
            self.displaced.fetch_add(1, Ordering::Relaxed);
        }
        old
    }

    /// The newest value, if one was posted since the last take.
    pub fn take(&self) -> Option<T> {
        self.slot.lock().ok()?.take()
    }

    /// Values posted, and how many of them were replaced before a reader took them.
    pub fn counts(&self) -> (u64, u64) {
        (self.posted.load(Ordering::Relaxed), self.displaced.load(Ordering::Relaxed))
    }
}

// --- the pack lane ---------------------------------------------------------------------

/// **One snapshot waits; one is packed.** The loop fills a snapshot only when the slot is
/// free ([`PackLane::claim`]), so a snapshot is never displaced unpacked (its pore-water
/// window is never lost) and the copy is paid only for a snapshot that will be packed. The
/// pack thread takes the posted snapshot, packs it, and gives its buffer back to be filled
/// again. A snapshot is owned by one side at a time: the pack never reads one being filled.
pub struct PackLane<S> {
    state: Mutex<Lane<S>>,
    moved: Condvar,
}

struct Lane<S> {
    posted: Option<S>,
    spare: Option<S>,
    quit: bool,
}

impl<S> Default for PackLane<S> {
    fn default() -> Self {
        PackLane { state: Mutex::new(Lane { posted: None, spare: None, quit: false }), moved: Condvar::new() }
    }
}

impl<S> PackLane<S> {
    fn lock(&self) -> std::sync::MutexGuard<'_, Lane<S>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The copier's turn: while the slot is free, `Some` of a spent buffer to fill again
    /// (or `None` inside: make one); `None` while a posted snapshot still waits, or after
    /// [`PackLane::quit`].
    pub fn claim(&self) -> Option<Option<S>> {
        let mut l = self.lock();
        (!l.quit && l.posted.is_none()).then(|| l.spare.take())
    }

    /// Hand a filled snapshot to the pack thread. False after [`PackLane::quit`].
    pub fn post(&self, s: S) -> bool {
        let mut l = self.lock();
        if l.quit {
            return false;
        }
        l.posted = Some(s);
        drop(l);
        self.moved.notify_all();
        true
    }

    /// The pack thread's wait: until a snapshot is posted (it replaces `held`, whose buffer
    /// goes back to the copier), until `retry` has passed with one `held`, or until the
    /// lane quits (false). Looks at its quit flag every [`STAGE_SLICE`] at least.
    pub fn wait(&self, held: &mut Option<S>, retry: Option<Duration>) -> bool {
        let deadline = retry.filter(|_| held.is_some()).map(|d| Instant::now() + d);
        let mut l = self.lock();
        loop {
            if l.quit {
                return false;
            }
            if let Some(s) = l.posted.take() {
                if let Some(old) = held.replace(s) {
                    l.spare = Some(old);
                }
                return true;
            }
            let now = Instant::now();
            if deadline.is_some_and(|d| now >= d) {
                return true;
            }
            let slice = deadline.map_or(STAGE_SLICE, |d| (d - now).min(STAGE_SLICE));
            l = self.moved.wait_timeout(l, slice).unwrap_or_else(|e| e.into_inner()).0;
        }
    }

    /// A snapshot packed (or of a world no longer shown): its buffer back for the copier.
    pub fn done(&self, s: S) {
        self.lock().spare = Some(s);
    }

    /// Stop: the pack thread's wait returns false, and nothing more is posted.
    pub fn quit(&self) {
        self.lock().quit = true;
        self.moved.notify_all();
    }
}

/// What became of a snapshot the pack thread tried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Packed {
    /// Staged, and the renderer is ready to show it.
    Done,
    /// The renderer refused it (every staging buffer still being read): try again soon.
    Refused,
    /// Staged, but the lit tier's sky plane has not arrived: pack again until it has.
    AwaitingLight,
    /// Of a world no longer installed (or none is): drop it.
    Stale,
}

// --- picking ------------------------------------------------------------------------------

/// What picking reads, published after each pack: the projection, every drawn body cell
/// and its animal, and which voxels are solid.
#[derive(Clone, Debug)]
pub struct PickMap {
    pub s: u32,
    pub rise: u32,
    pub base: i32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    /// `(z · width + x) · height + y` → animal id.
    cells: FxHashMap<u32, u64>,
    by_animal: FxHashMap<u64, Vec<(u32, u32, u32)>>,
    /// One bit per voxel in the world's own order, `(y · depth + z) · width + x`.
    solid: Arc<Vec<u64>>,
    /// Bumped by every new map, so a highlight knows when to follow.
    pub version: u64,
}

impl PickMap {
    /// A map for a world of `proj`'s geometry, its solid voxels given by `solid` (the
    /// world's order), with these drawn body cells.
    pub fn new(proj: &Projection, solid: Arc<Vec<u64>>, cells: &[((u32, u32, u32), u64)], version: u64) -> PickMap {
        let mut map = FxHashMap::default();
        let mut by_animal: FxHashMap<u64, Vec<(u32, u32, u32)>> = FxHashMap::default();
        for &((x, y, z), id) in cells {
            map.insert((z * proj.width + x) * proj.height + y, id);
            by_animal.entry(id).or_default().push((x, y, z));
        }
        PickMap {
            s: proj.s,
            rise: proj.rise,
            base: proj.base,
            width: proj.width,
            height: proj.height,
            depth: proj.depth,
            cells: map,
            by_animal,
            solid,
            version,
        }
    }

    /// The solid bits of a world, in its own order.
    pub fn solid_bits(world: &World) -> Vec<u64> {
        Self::solid_bits_of(world.view().material)
    }

    /// The solid bits of a world's material, in its own order.
    pub fn solid_bits_of(m: &[cubarium_voxel::Material]) -> Vec<u64> {
        let mut bits = vec![0u64; m.len().div_ceil(64)];
        for (i, v) in m.iter().enumerate() {
            if v.is_solid() {
                bits[i / 64] |= 1 << (i % 64);
            }
        }
        bits
    }

    fn raster_w(&self) -> i32 {
        (self.width * self.s) as i32
    }

    fn is_solid(&self, x: u32, y: u32, z: u32) -> bool {
        let i = ((y * self.depth + z) * self.width + x) as usize;
        self.solid.get(i / 64).is_some_and(|w| w >> (i % 64) & 1 == 1)
    }

    fn animal(&self, x: u32, y: u32, z: u32) -> Option<u64> {
        self.cells.get(&((z * self.width + x) * self.height + y)).copied()
    }

    /// The animal under world raster pixel `(px, py)` (`px` wraps round the ring): the
    /// renderer's own walk inverted — slabs near to far, a voxel's front face before the cap
    /// of the voxel under it — ending at the first solid voxel. Failing that, the animal with
    /// a drawn cell nearest the pixel within `radius` raster pixels, the nearer slab on a tie.
    pub fn pick(&self, px: i32, py: i32, radius: f32) -> Option<u64> {
        if self.width == 0 || self.s == 0 {
            return None;
        }
        let px = px.rem_euclid(self.raster_w());
        let x = (px / self.s as i32) as u32;
        let s = self.s as i32;
        for z in 0..self.depth {
            let q = self.base - (z * self.rise) as i32 - py - 1;
            if q < 0 {
                break;
            }
            let level = q / s;
            let r = q - level * s;
            if level < self.height as i32 {
                let y = level as u32;
                if self.is_solid(x, y, z) {
                    break;
                }
                if let Some(id) = self.animal(x, y, z) {
                    return Some(id);
                }
            }
            let below = level - 1;
            if r < self.rise as i32 && below >= 0 && below < self.height as i32 {
                let y = below as u32;
                if self.is_solid(x, y, z) {
                    break;
                }
                if let Some(id) = self.animal(x, y, z) {
                    return Some(id);
                }
            }
        }
        self.nearest(px, py, radius)
    }

    /// Where the walk's ray through world raster pixel `(px, py)` first meets solid terrain,
    /// z in voxels (a front face at its slab, a cap across its rows), as `voxel.frag`'s
    /// `depthOut` has it for terrain; `f32::MAX` where it meets none.
    pub fn terrain_z(&self, px: i32, py: i32) -> f32 {
        if self.width == 0 || self.s == 0 {
            return f32::MAX;
        }
        let px = px.rem_euclid(self.raster_w());
        let x = (px / self.s as i32) as u32;
        let s = self.s as i32;
        for z in 0..self.depth {
            let q = self.base - (z * self.rise) as i32 - py - 1;
            if q < 0 {
                break;
            }
            let level = q / s;
            let r = q - level * s;
            if level < self.height as i32 && self.is_solid(x, level as u32, z) {
                return z as f32;
            }
            let below = level - 1;
            if r < self.rise as i32 && below >= 0 && below < self.height as i32 && self.is_solid(x, below as u32, z) {
                return z as f32 + (r as f32 + 0.5) / self.rise as f32;
            }
        }
        f32::MAX
    }

    /// The animal with a drawn cell nearest `(px, py)` within `radius` raster pixels.
    fn nearest(&self, px: i32, py: i32, radius: f32) -> Option<u64> {
        if radius <= 0.0 {
            return None;
        }
        let rw = self.raster_w();
        let mut best: Option<(f32, u32, u64)> = None;
        for (&key, &id) in &self.cells {
            let y = key % self.height;
            let col = key / self.height;
            let (x, z) = (col % self.width, col / self.width);
            let (x0, x1, y0, y1) = self.footprint(x, y, z);
            let dx = {
                // Round the ring, the short way.
                let c = (x0 + x1) as f32 / 2.0;
                let d = (px as f32 - c).rem_euclid(rw as f32);
                let d = d.min(rw as f32 - d);
                (d - (x1 - x0) as f32 / 2.0).max(0.0)
            };
            let dy = if py < y0 { (y0 - py) as f32 } else if py > y1 { (py - y1) as f32 } else { 0.0 };
            let d = (dx * dx + dy * dy).sqrt();
            if d <= radius && best.is_none_or(|(bd, bz, _)| d < bd || (d == bd && z < bz)) {
                best = Some((d, z, id));
            }
        }
        best.map(|(_, _, id)| id)
    }

    /// A voxel's drawn pixels: columns `x0..=x1`, rows `y0..=y1` (its top face and its front
    /// face), in world raster pixels.
    fn footprint(&self, x: u32, y: u32, z: u32) -> (i32, i32, i32, i32) {
        let x0 = (x * self.s) as i32;
        let front = self.base - ((y + 1) * self.s) as i32 - (z * self.rise) as i32;
        (x0, x0 + self.s as i32 - 1, front - self.rise as i32, front + self.s as i32 - 1)
    }

    /// Where animal `id` is drawn: the middle of its cells' pixels, in world raster pixels
    /// (`x` in `0..raster_w`), or `None` when it is not drawn.
    pub fn animal_at(&self, id: u64) -> Option<(f32, f32)> {
        let cells = self.by_animal.get(&id)?;
        let rw = self.raster_w() as f32;
        let first = cells.first()?;
        let ref_x = (first.0 * self.s) as f32;
        let (mut sx, mut sy) = (0.0f32, 0.0f32);
        for &(x, y, z) in cells {
            let (x0, x1, y0, y1) = self.footprint(x, y, z);
            let c = (x0 + x1) as f32 / 2.0;
            // Each cell the short way round from the first, so a body across the seam
            // averages to its middle.
            let d = (c - ref_x + rw / 2.0).rem_euclid(rw) - rw / 2.0;
            sx += ref_x + d;
            sy += (y0 + y1) as f32 / 2.0;
        }
        let n = cells.len() as f32;
        Some(((sx / n).rem_euclid(rw), sy / n))
    }

    /// The cells animal `id` is drawn in.
    pub fn cells_of(&self, id: u64) -> &[(u32, u32, u32)] {
        self.by_animal.get(&id).map_or(&[], |v| v.as_slice())
    }
}

// --- the shared state ---------------------------------------------------------------------

/// How the app wants the world shown.
#[derive(Clone, Debug)]
pub struct HostedOptions {
    /// The presentation config (`[light]`, haze, `px_per_voxel`, …): a `cubarium voxel`
    /// config file. Its world table is ignored; the world is whatever the session runs.
    pub config: PathBuf,
    /// What the config's relative asset paths (`models_dir`, `textures_dir`) are relative
    /// to: the repository root.
    pub root: PathBuf,
    /// The screen the world is drawn for, pixels: `px_per_voxel = "auto"` covers it as the
    /// desktop window does ([`cover_px_per_voxel`]).
    pub screen: (u32, u32),
    /// Frames a second the presenter draws at most.
    pub fps: f64,
    /// How far a highlighted animal leans toward white (the mesh view's overlay: 0.4).
    pub highlight_mix: f32,
}

/// One frame for the app.
#[derive(Clone, Debug, Default)]
pub struct HostedFrame {
    /// `width · height · 4` sRGB RGBA bytes.
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// The raster columns this frame is turned round the ring ([`Hosted::set_view_x`]).
    pub view_x: i32,
    /// The world's tick it shows, and which world (counts installs).
    pub tick: u64,
    pub generation: u64,
}

/// What the presenter has spent, cumulatively.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostedStats {
    /// Frames drawn and handed to the mailbox.
    pub frames: u64,
    /// Milliseconds: recording and submitting a frame (CPU), the GPU's three stages
    /// (uploads, the walk, the present pass; the last retired frame's), copying a frame out
    /// of its read-back slot, and packing a tick (on the loop).
    pub record_ms: f64,
    pub gpu_ms: f64,
    pub copy_ms: f64,
    pub pack_ms: f64,
    pub packs: u64,
    /// Milliseconds the loop spent copying what the pack reads (H3: the pack's whole cost
    /// to the loop), over this many snapshots.
    pub snapshot_ms: f64,
    pub snapshots: u64,
    /// Ticks whose pack was refused because the GPU held every staging buffer.
    pub packs_refused: u64,
    /// Frames posted over an untaken one.
    pub displaced: u64,
    /// A still world was drawn before its lit tier's sky plane arrived (it waited
    /// [`STATIC_LIGHT_WAIT`] and gave up), so its picture lacks the sky's shading.
    pub unlit: bool,
}

struct Clock {
    tick: u64,
    at: Instant,
    /// Ticks a second at the run's speed.
    tps: f64,
    /// Ticks the last step moved: how far the clock may run ahead of the packed tick.
    ahead: f64,
    paused: bool,
    /// The last time drawn, so a pause holds it and the clock never runs backwards.
    drawn: f64,
}

impl Clock {
    fn still(tick: u64) -> Clock {
        Clock { tick, at: Instant::now(), tps: 0.0, ahead: 0.0, paused: true, drawn: tick as f64 }
    }

    /// The frame's time in ticks, now.
    fn now(&mut self) -> f64 {
        if !self.paused {
            let run = (self.at.elapsed().as_secs_f64() * self.tps).min(self.ahead.max(1.0) - 1e-6);
            self.drawn = self.drawn.max(self.tick as f64 + run.max(0.0));
        }
        self.drawn
    }
}

struct Controls {
    view_x: i32,
    highlight: Option<u64>,
}

/// The renderer for one world, and its read-back ring. The ring is declared first so it
/// is dropped first: it waits for its frames, then the sink frees the device.
struct SinkState {
    ring: ReadbackRing,
    sink: VoxelGpuSink<Offscreen>,
    generation: u64,
    /// The lit tier's sky plane has arrived (or the flat tier, which has none): frames may
    /// be shown.
    ready: bool,
    /// The content version last submitted.
    drawn_version: Option<u64>,
    /// `(tag, view_x, tick)` of the frames in flight.
    flight: VecDeque<(u64, i32, u64)>,
    /// The highlight drawn: the animal and the pick map's version it was taken from.
    highlight_drawn: Option<(u64, u64)>,
    /// The projection it draws with, for the pick map.
    proj: Projection,
}

// The state is genuinely `Send`: the loop's pack and the presenter's record take it in
// turn under one `Mutex`, and nothing in it is tied to a thread (the GPU's mapped buffers
// hold their pointers in `AtomicPtr`s, `cubarium_gpu::vk::HostBuffer`).

struct Shared {
    options: HostedOptions,
    cfg: VoxelConfig,
    sink: Mutex<Option<SinkState>>,
    mailbox: Mailbox<HostedFrame>,
    /// Spent frame buffers, for the next copy.
    spare: Mutex<Vec<Vec<u8>>>,
    clock: Mutex<Clock>,
    controls: Mutex<Controls>,
    pick: Mutex<Option<Arc<PickMap>>>,
    /// The figures of the newest frame drawn (the model pass, H2): what picking reads when
    /// animals are drawn as models.
    figures: Mutex<Option<Arc<FigurePick>>>,
    stats: Mutex<HostedStats>,
    error: Mutex<Option<String>>,
    generation: AtomicU64,
    quit: AtomicBool,
    /// [`HostedStats::unlit`].
    unlit: AtomicBool,
    /// The presenter, the pack thread, the still-world stagers and renderers being
    /// released: joined by [`Shared::shutdown`].
    threads: Mutex<Vec<JoinHandle<()>>>,
    /// The live world's snapshots, from the loop to the pack thread (H3).
    lane: PackLane<Box<PackSnapshot>>,
    /// The pack thread has been started.
    packing: AtomicBool,
}

impl Shared {
    fn fail(&self, e: anyhow::Error) {
        let text = format!("{e:#}");
        eprintln!("cubarium hosted renderer: {text}");
        if let Ok(mut s) = self.error.lock() {
            *s = Some(text);
        }
    }

    /// The renderer for a world of `world`'s shape.
    fn open(&self, world: &cubarium_voxel::Config) -> Result<(VoxelGpuSink<Offscreen>, Projection)> {
        let mut cfg = self.cfg.clone();
        cfg.sim.world = world.clone();
        if cfg.px_per_voxel == PX_AUTO {
            cfg.px_per_voxel = cover_px_per_voxel(cfg.tilt_degrees, cfg.raster_height, world, self.options.screen);
        }
        let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, world)?;
        super::check(proj)?;
        let sink = VoxelGpuSink::offscreen(&cfg, proj, load_models(&cfg))?;
        Ok((sink, proj))
    }

    /// Install a renderer for a world just staged into it. Returns its generation.
    fn install(&self, sink: VoxelGpuSink<Offscreen>, proj: Projection, ready: bool) -> Result<u64> {
        let ring = ReadbackRing::new(sink.gpu.clone(), &sink.renderer, sink.renderer.raster_image(), RING)?;
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let state = SinkState {
            ring,
            sink,
            generation,
            ready,
            drawn_version: None,
            flight: VecDeque::new(),
            highlight_drawn: None,
            proj,
        };
        // The old renderer goes outside the lock: dropping it waits for its GPU. A renderer
        // shut down ([`Shared::shutdown`]) takes no new one: checked under the lock its
        // release takes, so none is left behind.
        let old = self.sink.lock().map(|mut s| {
            if self.quit.load(Ordering::SeqCst) {
                Some(state)
            } else {
                s.replace(state)
            }
        });
        drop(old);
        Ok(generation)
    }

    /// Stop every thread of this renderer and free it: the presenter and any still-world
    /// stager are told to quit, woken and joined, then the GPU renderer (with its sky-plane
    /// and sun-visibility workers, which join their own threads) is dropped. Idempotent;
    /// called from [`Hosted`]'s drop and from [`HostedStop::shutdown`]. Never from one of
    /// those threads.
    fn shutdown(&self) {
        self.quit.store(true, Ordering::SeqCst);
        // A stager waiting for its turn looks at its quit flag now, not at its next slice;
        // so does the pack thread waiting for a snapshot (one mid-pack finishes it first).
        STAGE_TURNS.wake();
        self.lane.quit();
        let threads = std::mem::take(&mut *self.threads.lock().unwrap_or_else(|e| e.into_inner()));
        for t in &threads {
            t.thread().unpark();
        }
        for t in threads {
            let _ = t.join();
        }
        let old = self.sink.lock().ok().and_then(|mut s| s.take());
        drop(old);
    }

    /// Publish the pick map of the pack just staged.
    fn publish_pick(&self, sink: &VoxelGpuSink<Offscreen>, proj: &Projection, solid: Arc<Vec<u64>>) {
        let version = self.pick.lock().ok().and_then(|p| p.as_ref().map(|m| m.version)).unwrap_or(0) + 1;
        let map = PickMap::new(proj, solid, &sink.packer.animals.owned_cells(), version);
        if let Ok(mut p) = self.pick.lock() {
            *p = Some(Arc::new(map));
        }
    }

    /// Pack one snapshot of the live world into the installed renderer (the pack thread),
    /// publish its pick map, and move the frame clock to its tick. `solid` caches the solid
    /// bits by world generation and terrain version.
    fn pack_snapshot(&self, snap: &mut PackSnapshot, solid: &mut Option<(u64, u64, Arc<Vec<u64>>)>) -> Packed {
        let at = Instant::now();
        let Ok(mut guard) = self.sink.lock() else { return Packed::Stale };
        let Some(st) = guard.as_mut() else { return Packed::Stale };
        if st.generation != snap.generation {
            return Packed::Stale;
        }
        let staged = st.sink.stage_snapshot(snap);
        let mut packed = Packed::Refused;
        if staged {
            if !st.ready && !st.sink.light_pending() {
                st.ready = true;
            }
            let key = (snap.generation, snap.terrain_version());
            let bits = match solid {
                Some((g, v, bits)) if (*g, *v) == key => bits.clone(),
                _ => {
                    let bits = Arc::new(PickMap::solid_bits_of(snap.material()));
                    *solid = Some((key.0, key.1, bits.clone()));
                    bits
                }
            };
            self.publish_pick(&st.sink, &st.proj, bits);
            packed = if st.ready { Packed::Done } else { Packed::AwaitingLight };
        }
        drop(guard);
        if staged && let Ok(mut k) = self.clock.lock() {
            // The frames run on from the tick just packed, as far as the ticks it moved.
            let tick = snap.tick();
            if tick != k.tick {
                k.ahead = tick.saturating_sub(k.tick).max(1) as f64;
                k.tick = tick;
                k.at = Instant::now();
                if k.drawn < tick as f64 {
                    k.drawn = tick as f64;
                }
            }
        }
        if let Ok(mut s) = self.stats.lock() {
            s.pack_ms += at.elapsed().as_secs_f64() * 1e3;
            s.packs += u64::from(staged);
        }
        packed
    }

    /// One pass of the presenter: hand back finished frames, and draw a new one if one is
    /// due and the picture would change. Returns whether frames are in flight.
    fn present(&self, due: bool) -> Result<bool> {
        let mut guard = self.sink.lock().map_err(|_| anyhow::anyhow!("the renderer's lock is poisoned"))?;
        let Some(st) = guard.as_mut() else { return Ok(false) };
        // Finished frames, oldest first.
        let mut out = Vec::new();
        let mut copy_ms = 0.0;
        let gpu = st.sink.gpu.clone();
        let (w, h) = st.ring.size();
        {
            let flight = &mut st.flight;
            let generation = st.generation;
            st.ring.poll(&mut st.sink.renderer, |tag, bytes| {
                let at = Instant::now();
                let mut buf = self.spare.lock().ok().and_then(|mut s| s.pop()).unwrap_or_default();
                buf.clear();
                buf.extend_from_slice(bytes);
                copy_ms += at.elapsed().as_secs_f64() * 1e3;
                while let Some(&(t, view_x, tick)) = flight.front() {
                    flight.pop_front();
                    if t == tag {
                        out.push(HostedFrame { rgba: std::mem::take(&mut buf), width: w, height: h, view_x, tick, generation });
                        break;
                    }
                }
                if !buf.is_empty()
                    && let Ok(mut s) = self.spare.lock()
                {
                    s.push(buf);
                }
            })?;
        }
        let gpu_ms = (!out.is_empty()).then(|| st.sink.renderer.gpu_split(&gpu)).flatten();
        if due && st.ready && st.ring.can_submit(&st.sink.renderer) {
            let t = self.clock.lock().map(|mut c| c.now()).unwrap_or(0.0);
            let (view_x, highlight) =
                self.controls.lock().map(|c| (c.view_x, c.highlight)).unwrap_or((0, None));
            let figures = st.sink.figures_on();
            if figures {
                st.sink.set_figure_highlight(highlight.map(|id| (id, self.options.highlight_mix)));
            }
            st.sink.set_clock(t.floor() as u64, t.fract());
            if figures && let Ok(mut f) = self.figures.lock() {
                *f = Some(st.sink.figure_pick());
            }
            st.sink.renderer.set_view_x(view_x);
            let pick = self.pick.lock().ok().and_then(|p| p.clone());
            let want = highlight.zip(pick.as_ref().map(|p| p.version));
            if !figures && want != st.highlight_drawn {
                st.highlight_drawn = want;
                let h = highlight.zip(pick).map(|(id, p)| Highlight {
                    cells: p.cells_of(id).to_vec(),
                    mix: self.options.highlight_mix,
                });
                st.sink.renderer.set_highlight(h.filter(|h| !h.cells.is_empty()));
            }
            let version = st.sink.renderer.content_version();
            if st.drawn_version != Some(version) {
                let at = Instant::now();
                if let Some(sv) = &mut st.sink.sunvis {
                    sv.frame(st.sink.clock_s, st.sink.renderer.params().sun, &mut st.sink.renderer);
                }
                let tag = st.sink.frames;
                if st.ring.submit(&mut st.sink.renderer, (), tag)? {
                    st.sink.frames += 1;
                    st.flight.push_back((tag, st.sink.renderer.view_x(), t.floor() as u64));
                    st.drawn_version = Some(st.sink.renderer.content_version());
                    if let Ok(mut s) = self.stats.lock() {
                        s.record_ms += at.elapsed().as_secs_f64() * 1e3;
                    }
                }
            }
        }
        let busy = st.ring.in_flight() > 0;
        drop(guard);
        if let Ok(mut s) = self.stats.lock() {
            s.copy_ms += copy_ms;
            if let Some(g) = gpu_ms {
                s.gpu_ms += g.iter().sum::<f64>();
            }
            s.frames += out.len() as u64;
        }
        for f in out {
            if let Some(old) = self.mailbox.post(f)
                && let Ok(mut s) = self.spare.lock()
                && s.len() < 4
            {
                s.push(old.rgba);
            }
        }
        Ok(busy)
    }
}

fn presenter_main(shared: Arc<Shared>) {
    let period = Duration::from_secs_f64(1.0 / shared.options.fps.clamp(1.0, 1000.0));
    let mut next = Instant::now();
    while !shared.quit.load(Ordering::Relaxed) {
        let now = Instant::now();
        let due = now >= next;
        if due {
            next += period;
            if next < now {
                next = now + period;
            }
        }
        let busy = match shared.present(due) {
            Ok(b) => b,
            Err(e) => {
                shared.fail(e);
                false
            }
        };
        // Poll finished frames often; otherwise sleep to the next frame. Parked, not slept,
        // so a shutdown's unpark ends the wait at once.
        let until = next.saturating_duration_since(Instant::now());
        std::thread::park_timeout(if busy { until.min(Duration::from_micros(500)) } else { until.min(period) });
    }
}

/// The pack thread: packs each snapshot the loop posts, trying a refused one again soon and
/// one still waiting for its sky plane every [`RESTAGE_EVERY`], until the lane quits.
fn pack_main(shared: &Arc<Shared>) {
    let mut held: Option<Box<PackSnapshot>> = None;
    let mut retry = None;
    let mut solid = None;
    while shared.lane.wait(&mut held, retry) {
        let Some(snap) = held.as_mut() else { continue };
        retry = match shared.pack_snapshot(snap, &mut solid) {
            Packed::Done | Packed::Stale => {
                if let Some(s) = held.take() {
                    shared.lane.done(s);
                }
                None
            }
            Packed::Refused => Some(REFUSED_RETRY),
            Packed::AwaitingLight => Some(RESTAGE_EVERY),
        };
    }
}

// --- the host -------------------------------------------------------------------------------

/// **The world's renderer for an app**: a presenter thread and the frames it draws. Draw a
/// live session through [`Hosted::hook`], or a candidate through [`Hosted::show_static`].
///
/// Dropping it stops and joins every thread it started ([`HostedStop`] does the same from
/// elsewhere): an app that unloads this code (the Godot extension, at exit) must not leave
/// a thread of it running.
pub struct Hosted {
    shared: Arc<Shared>,
}

/// A handle that stops a [`Hosted`] renderer's threads without owning it: for an owner
/// whose drop may come too late (the Godot extension stops every renderer still alive when
/// its library is being unloaded). Holds the renderer only weakly.
#[derive(Clone)]
pub struct HostedStop(Weak<Shared>);

impl HostedStop {
    /// Stop and join the renderer's threads and free it, as dropping it would. Returns
    /// whether it was still alive. The `Hosted` itself stays usable but draws nothing.
    pub fn shutdown(&self) -> bool {
        match self.0.upgrade() {
            Some(s) => {
                let live = !s.quit.load(Ordering::SeqCst);
                s.shutdown();
                live
            }
            None => false,
        }
    }

    /// Whether anything of the renderer is still alive (the `Hosted` or a hook of it).
    pub fn alive(&self) -> bool {
        self.0.strong_count() > 0
    }
}

impl Hosted {
    /// Read the presentation config and start the presenter (it draws nothing until a world
    /// is given).
    pub fn new(options: HostedOptions) -> Result<Hosted> {
        let mut cfg = load_config(&options.config)
            .with_context(|| format!("the presentation config {}", options.config.display()))?;
        cfg.figures.apply_env_overrides()?;
        let rooted = |p: &Path| if p.is_relative() { options.root.join(p) } else { p.to_path_buf() };
        cfg.models_dir = rooted(&cfg.models_dir);
        cfg.textures_dir = rooted(&cfg.textures_dir);
        let shared = Arc::new(Shared {
            options,
            cfg,
            sink: Mutex::new(None),
            mailbox: Mailbox::default(),
            spare: Mutex::new(Vec::new()),
            clock: Mutex::new(Clock::still(0)),
            controls: Mutex::new(Controls { view_x: 0, highlight: None }),
            pick: Mutex::new(None),
            figures: Mutex::new(None),
            stats: Mutex::new(HostedStats::default()),
            error: Mutex::new(None),
            generation: AtomicU64::new(0),
            quit: AtomicBool::new(false),
            unlit: AtomicBool::new(false),
            threads: Mutex::new(Vec::new()),
            lane: PackLane::default(),
            packing: AtomicBool::new(false),
        });
        let presenter = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("cubarium-hosted-presenter".into())
                .spawn(move || presenter_main(shared))
                .context("spawning the presenter")?
        };
        shared.threads.lock().unwrap_or_else(|e| e.into_inner()).push(presenter);
        Ok(Hosted { shared })
    }

    /// A handle that stops this renderer from elsewhere ([`HostedStop`]).
    pub fn stopper(&self) -> HostedStop {
        HostedStop(Arc::downgrade(&self.shared))
    }

    /// The hook a session's loop runs this renderer from ([`cubarium_session::Session::set_hook`]).
    /// Starts the pack thread the hook's snapshots go to (once).
    pub fn hook(&self) -> Box<dyn LoopHook> {
        if !self.shared.packing.swap(true, Ordering::SeqCst) {
            self.spawn("cubarium-hosted-pack", pack_main);
        }
        Box::new(LiveHook {
            shared: self.shared.clone(),
            proj: None,
            owed: false,
            copier: SnapshotCopier::new(self.shared.cfg.pack.pore_sweep),
            generation: 0,
        })
    }

    /// Draw one still world: candidate `index` of `source`, staged on a thread of its own
    /// (opening the renderer and the lit tier's sky plane take a moment). Its clock stands
    /// at its tick. A `queued` one (a thumbnail) waits its turn behind the other queued
    /// stagers of the process ([`StageTurns`]); one that is not (a candidate opened large,
    /// asked for by hand) starts at once.
    pub fn show_static(&self, source: CandidateSource, index: usize, queued: bool) {
        self.spawn("cubarium-hosted-static", move |shared| {
            if let Err(e) = stage_static(shared, &source, index, queued) {
                shared.fail(e);
            }
        });
    }

    /// Run `work` on a thread of this renderer's own, joined by [`Shared::shutdown`]. Nothing
    /// runs after a shutdown, or when the thread can't be started (the failure is reported,
    /// and `work` is dropped here).
    fn spawn(&self, name: &str, work: impl FnOnce(&Arc<Shared>) + Send + 'static) {
        // Under the thread list's lock, so a shutdown either joins this thread or has
        // already refused it.
        let mut threads = self.shared.threads.lock().unwrap_or_else(|e| e.into_inner());
        if self.shared.quit.load(Ordering::SeqCst) {
            return;
        }
        // Threads that have finished are joined here, so the list holds only live ones.
        let (done, live): (Vec<_>, Vec<_>) = std::mem::take(&mut *threads).into_iter().partition(|t| t.is_finished());
        *threads = live;
        for t in done {
            let _ = t.join();
        }
        let shared = self.shared.clone();
        match std::thread::Builder::new().name(name.into()).spawn(move || work(&shared)) {
            Ok(t) => threads.push(t),
            Err(e) => self.shared.fail(e.into()),
        }
    }

    /// The newest frame not yet taken.
    pub fn take_frame(&self) -> Option<HostedFrame> {
        self.shared.mailbox.take()
    }

    /// Give a taken frame's buffer back for reuse.
    pub fn recycle(&self, frame: HostedFrame) {
        if let Ok(mut s) = self.shared.spare.lock()
            && s.len() < 4
        {
            s.push(frame.rgba);
        }
    }

    /// Turn the picture round the ring by `x` raster columns (the camera's pan, whole
    /// pixels; the app shifts the shown frame by the fraction).
    pub fn set_view_x(&self, x: i32) {
        if let Ok(mut c) = self.shared.controls.lock() {
            c.view_x = x;
        }
    }

    /// Draw animal `id` highlighted, or none.
    pub fn set_highlight(&self, id: Option<u64>) {
        if let Ok(mut c) = self.shared.controls.lock() {
            c.highlight = id;
        }
    }

    /// The animal under world raster pixel `(px, py)`, or near it: with the model pass, the
    /// figure whose cell the pixel's ray meets first in front of the terrain's own hit
    /// ([`FigurePick::hit`], [`PickMap::terrain_z`]), else the one with a cell nearest within
    /// `radius`; with stamped animals, [`PickMap::pick`].
    pub fn pick(&self, px: i32, py: i32, radius: f32) -> Option<u64> {
        let map = self.pick_map();
        if let Some(f) = self.figure_pick() {
            let ground = map.as_ref().map_or(f32::MAX, |m| m.terrain_z(px, py));
            if let Some((z, id)) = f.hit(px, py)
                && z <= ground + cubarium_gpu::figures::DEPTH_BIAS
            {
                return Some(id);
            }
            return map.and_then(|m| m.pick(px, py, 0.0)).or_else(|| f.nearest(px, py, radius));
        }
        map?.pick(px, py, radius)
    }

    /// Where animal `id` is drawn, world raster pixels ([`FigurePick::animal_at`] with the
    /// model pass, [`PickMap::animal_at`] without).
    pub fn animal_at(&self, id: u64) -> Option<(f32, f32)> {
        if let Some(f) = self.figure_pick() {
            return f.animal_at(id);
        }
        self.pick_map()?.animal_at(id)
    }

    fn figure_pick(&self) -> Option<Arc<FigurePick>> {
        self.shared.figures.lock().ok()?.clone()
    }

    fn pick_map(&self) -> Option<Arc<PickMap>> {
        self.shared.pick.lock().ok()?.clone()
    }

    /// What the presenter has spent so far.
    pub fn stats(&self) -> HostedStats {
        let mut s = self.shared.stats.lock().map(|s| *s).unwrap_or_default();
        s.displaced = self.shared.mailbox.counts().1;
        s.unlit = self.shared.unlit.load(Ordering::SeqCst);
        if let Ok(g) = self.shared.sink.lock()
            && let Some(st) = g.as_ref()
        {
            s.packs_refused = st.sink.packs_skipped;
        }
        s
    }

    /// The last thing that went wrong, if anything.
    pub fn error(&self) -> Option<String> {
        self.shared.error.lock().ok()?.clone()
    }

    /// Drop the renderer (its GPU memory), keeping the last frame taken. A thumbnail that
    /// will not move again does this. Returns at once: the renderer is freed on a thread of
    /// this renderer's own (freeing it waits for its GPU, tens of milliseconds, which the
    /// app's frame should not), joined like the others at shutdown.
    pub fn release(&self) {
        let Some(old) = self.shared.sink.lock().ok().and_then(|mut s| s.take()) else { return };
        self.spawn("cubarium-hosted-release", move |_| drop(old));
    }
}

impl Drop for Hosted {
    fn drop(&mut self) {
        self.shared.shutdown();
    }
}

/// Stage candidate `index` of `source` as a still world: wait for a turn
/// ([`STAGE_TURNS`]) when `queued`, open a renderer for it, pack it, wait for the lit tier's sky plane
/// (up to [`STATIC_LIGHT_WAIT`]) and install it. The candidate's world is held by its own
/// handle throughout, never under the session's candidate list.
fn stage_static(shared: &Arc<Shared>, source: &CandidateSource, index: usize, queued: bool) -> Result<()> {
    let quit = || shared.quit.load(Ordering::SeqCst);
    let turn = if queued { STAGE_TURNS.take(quit) } else { None };
    if queued && turn.is_none() {
        return Ok(());
    }
    let p = source.get(index).context("the candidate is not ready")?;
    let (world, flora, fauna) = (&p.world, &p.flora, &p.fauna);
    let (mut sink, proj) = shared.open(world.config())?;
    sink.set_clock(world.tick(), 0.0);
    sink.stage_world(world, flora, fauna);
    let (tick, solid) = (world.tick(), Arc::new(PickMap::solid_bits(world)));
    let started = Instant::now();
    while sink.light_pending() && started.elapsed() < STATIC_LIGHT_WAIT && !quit() {
        // Wait for the sky plane without packing again; a pack takes it in once it is done.
        // (The light shafts' first bake lands only through packs: with only that pending,
        // this packs every slice.)
        if sink.wait_sky(STAGE_SLICE) && !quit() {
            sink.stage_world(world, flora, fauna);
            if sink.light_pending() {
                // Parked, not slept: a shutdown unparks it and it gives up at once.
                std::thread::park_timeout(RESTAGE_EVERY);
            }
        }
    }
    if quit() {
        // Dropping the renderer here cancels its sky plane and joins that worker.
        return Ok(());
    }
    shared.unlit.store(sink.light_pending(), Ordering::SeqCst);
    shared.publish_pick(&sink, &proj, solid);
    if let Ok(mut c) = shared.clock.lock() {
        *c = Clock::still(tick);
    }
    shared.install(sink, proj, true).map(|_| ())
}

/// The live hook: on the session's loop, it copies what the pack reads and hands it to the
/// pack thread. Only a new world is packed here (its renderer is opened with it).
struct LiveHook {
    shared: Arc<Shared>,
    proj: Option<Projection>,
    /// The world moved while a snapshot still waited: copy one at the next period.
    owed: bool,
    copier: SnapshotCopier,
    /// The installed renderer's generation, carried by every snapshot.
    generation: u64,
}

impl LiveHook {
    fn clock(&self, c: HookClock) {
        // The tick the frames run from moves when a pack lands (`Shared::pack_snapshot`).
        if let Ok(mut k) = self.shared.clock.lock() {
            k.tps = f64::from(cubarium_voxel::TICK_HZ) * c.speed;
            k.paused = c.paused;
        }
    }

    /// Copy the world into a snapshot for the pack thread. False when the last one is still
    /// waiting to be packed (the world stays owed).
    fn post(&mut self, world: &World, flora: &Flora, fauna: &Fauna) -> bool {
        if self.proj.is_none() {
            return true;
        }
        let Some(spare) = self.shared.lane.claim() else { return false };
        let mut snap = spare.unwrap_or_default();
        self.copier.fill(&world.view(), flora.view(), fauna.view(), &mut snap);
        snap.generation = self.generation;
        let ms = snap.copy_ms();
        self.shared.lane.post(snap);
        if let Ok(mut s) = self.shared.stats.lock() {
            s.snapshot_ms += ms;
            s.snapshots += 1;
        }
        true
    }
}

impl LoopHook for LiveHook {
    fn installed(&mut self, world: &World, flora: &Flora, fauna: &Fauna, clock: HookClock) {
        if self.shared.quit.load(Ordering::SeqCst) {
            return;
        }
        self.copier.reset();
        if let Ok(mut c) = self.shared.clock.lock() {
            *c = Clock::still(clock.tick);
        }
        self.clock(clock);
        match self.shared.open(world.config()) {
            Ok((mut sink, proj)) => {
                sink.set_clock(world.tick(), 0.0);
                let staged = sink.stage_world(world, flora, fauna);
                let ready = staged && !sink.light_pending();
                self.proj = Some(proj);
                let solid = Arc::new(PickMap::solid_bits(world));
                self.shared.publish_pick(&sink, &proj, solid);
                match self.shared.install(sink, proj, ready) {
                    Ok(g) => self.generation = g,
                    Err(e) => self.shared.fail(e),
                }
                // Not ready yet (the sky plane is coming): the pack thread packs the next
                // snapshot until it is.
                self.owed = !ready;
            }
            Err(e) => {
                self.proj = None;
                self.shared.fail(e);
            }
        }
    }

    fn period(&mut self, world: &World, flora: &Flora, fauna: &Fauna, moved: bool, clock: HookClock) {
        self.clock(clock);
        if moved || self.owed {
            self.owed = !self.post(world, flora, fauna);
        }
    }

    fn closed(&mut self) {
        self.proj = None;
        let old = self.shared.sink.lock().ok().and_then(|mut s| s.take());
        drop(old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_frame_wins() {
        let m = Mailbox::default();
        assert_eq!(m.take(), None::<u32>);
        for i in 1..=5u32 {
            m.post(i);
        }
        assert_eq!(m.take(), Some(5));
        assert_eq!(m.take(), None);
        assert_eq!(m.counts(), (5, 4));
    }

    #[test]
    fn a_slow_reader_never_blocks_the_presenter() {
        // The reader takes a frame and sits on it; the presenter keeps posting and never
        // waits for it, and the reader's next take is the newest.
        let m = Arc::new(Mailbox::default());
        m.post(0u32);
        let (held_tx, held_rx) = std::sync::mpsc::channel();
        let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
        let reader = {
            let m = m.clone();
            std::thread::spawn(move || {
                let held = m.take();
                held_tx.send(held).unwrap();
                go_rx.recv().unwrap(); // a slow reader: busy with its frame until told
                m.take()
            })
        };
        assert_eq!(held_rx.recv().unwrap(), Some(0));
        for i in 1..=1000u32 {
            m.post(i);
        }
        // Every post returned while the reader still held its frame.
        go_tx.send(()).unwrap();
        assert_eq!(reader.join().unwrap(), Some(1000));
    }

    /// Wait until `turns` has `n` stagers queued.
    // ---- the pack lane (H3) ----

    /// The copier fills only a free slot, and gets the packed buffer back to fill again.
    #[test]
    fn the_copier_fills_only_a_free_slot_and_reuses_the_packed_buffer() {
        let lane: PackLane<Vec<u64>> = PackLane::default();
        assert_eq!(lane.claim(), Some(None), "free, no buffer yet");
        assert!(lane.post(vec![1]));
        assert_eq!(lane.claim(), None, "a posted snapshot still waits");
        let mut held = None;
        assert!(lane.wait(&mut held, None));
        assert_eq!(held, Some(vec![1]));
        assert_eq!(lane.claim(), Some(None), "taken: the slot is free again");
        lane.done(held.take().unwrap());
        assert_eq!(lane.claim(), Some(Some(vec![1])), "the packed buffer comes back");
        // A refused snapshot held for a retry is replaced by a newer one, its buffer spare.
        assert!(lane.post(vec![2]));
        assert!(lane.wait(&mut held, None));
        assert!(lane.wait(&mut held, Some(Duration::from_millis(1))), "the retry comes due");
        assert_eq!(held, Some(vec![2]));
        assert!(lane.post(vec![3]));
        assert!(lane.wait(&mut held, Some(Duration::from_secs(60))));
        assert_eq!(held, Some(vec![3]));
        assert_eq!(lane.claim(), Some(Some(vec![2])));
        lane.quit();
        assert_eq!(lane.claim(), None);
        assert!(!lane.post(vec![4]));
        assert!(!lane.wait(&mut held, None));
    }

    /// A quit while the pack thread is in the middle of a pack: the pack finishes, the
    /// thread sees the quit and ends, and the join returns.
    #[test]
    fn a_quit_while_the_pack_thread_is_busy_joins_it() {
        let lane = Arc::new(PackLane::<u64>::default());
        let (busy, finished) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
        let t = {
            let (lane, busy, finished) = (lane.clone(), busy.clone(), finished.clone());
            std::thread::spawn(move || {
                let mut held = None;
                while lane.wait(&mut held, None) {
                    busy.store(true, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(150));
                    finished.store(true, Ordering::SeqCst);
                    lane.done(held.take().unwrap());
                }
            })
        };
        assert!(lane.post(7));
        let at = Instant::now();
        while !busy.load(Ordering::SeqCst) {
            assert!(at.elapsed() < Duration::from_secs(5), "the pack never started");
            std::thread::yield_now();
        }
        lane.quit();
        t.join().expect("the pack thread ends");
        assert!(finished.load(Ordering::SeqCst), "the pack in hand was finished, not torn off");
    }

    /// The pack never reads a snapshot while it is being filled: every snapshot the pack
    /// thread takes is whole (all of one tick) and newer than the last.
    #[test]
    fn the_pack_never_sees_a_torn_snapshot() {
        let lane = Arc::new(PackLane::<Vec<u64>>::default());
        let packer = {
            let lane = lane.clone();
            std::thread::spawn(move || {
                let (mut held, mut last, mut seen) = (None, 0u64, 0u64);
                while lane.wait(&mut held, None) {
                    let s: Vec<u64> = held.take().unwrap();
                    assert!(s.iter().all(|&v| v == s[0]), "a torn snapshot");
                    assert!(s[0] > last, "an old snapshot after a newer one");
                    last = s[0];
                    seen += 1;
                    lane.done(s);
                }
                seen
            })
        };
        let mut tick = 0u64;
        let at = Instant::now();
        while tick < 2000 && at.elapsed() < Duration::from_secs(20) {
            if let Some(spare) = lane.claim() {
                tick += 1;
                let mut s = spare.unwrap_or_else(|| vec![0; 4096]);
                // Filled element by element, as a copy is.
                for v in s.iter_mut() {
                    *v = tick;
                }
                lane.post(s);
            } else {
                std::thread::yield_now();
            }
        }
        // Let the last posted one be taken, then stop.
        while lane.claim().is_none() {
            std::thread::yield_now();
        }
        lane.quit();
        let seen = packer.join().expect("no torn or stale snapshot");
        assert!(seen > 0);
    }

    fn queued(turns: &StageTurns, n: usize) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while turns.waiting() != n {
            assert!(Instant::now() < deadline, "{} queued, not {n}", turns.waiting());
            std::thread::yield_now();
        }
    }

    #[test]
    fn stagers_take_turns_first_asked_first_served() {
        let turns = Arc::new(StageTurns::new(1));
        let first = turns.take(|| false).expect("a free turn is taken at once");
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut threads = Vec::new();
        for k in 0..4 {
            let (t, order) = (turns.clone(), order.clone());
            threads.push(std::thread::spawn(move || {
                let _turn = t.take(|| false).expect("a turn");
                order.lock().unwrap().push(k);
            }));
            // Each asks only once the one before is in the queue.
            queued(&turns, k + 1);
        }
        assert!(order.lock().unwrap().is_empty(), "nobody stages while the turn is held");
        drop(first);
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(*order.lock().unwrap(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn no_more_turns_are_held_than_the_limit() {
        for limit in [1, 2, 3] {
            let turns = Arc::new(StageTurns::new(limit));
            let (now, most) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
            let threads: Vec<_> = (0..8)
                .map(|_| {
                    let (turns, now, most) = (turns.clone(), now.clone(), most.clone());
                    std::thread::spawn(move || {
                        let _turn = turns.take(|| false).expect("a turn");
                        let n = now.fetch_add(1, Ordering::SeqCst) + 1;
                        most.fetch_max(n, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(5));
                        now.fetch_sub(1, Ordering::SeqCst);
                    })
                })
                .collect();
            for t in threads {
                t.join().unwrap();
            }
            let most = most.load(Ordering::SeqCst);
            assert!(most >= 1 && most <= limit as u64, "{most} held at once, limit {limit}");
        }
    }

    #[test]
    fn a_stager_told_to_quit_leaves_the_queue_and_the_next_goes_on() {
        let turns = Arc::new(StageTurns::new(1));
        let held = turns.take(|| false).expect("a free turn");
        let quit = Arc::new(AtomicBool::new(false));
        let quitter = {
            let (turns, quit) = (turns.clone(), quit.clone());
            std::thread::spawn(move || turns.take(|| quit.load(Ordering::SeqCst)).is_some())
        };
        queued(&turns, 1);
        let next = {
            let turns = turns.clone();
            std::thread::spawn(move || turns.take(|| false).is_some())
        };
        queued(&turns, 2);
        quit.store(true, Ordering::SeqCst);
        assert!(!quitter.join().unwrap(), "the quitter gave up without a turn");
        queued(&turns, 1);
        drop(held);
        assert!(next.join().unwrap(), "the one behind it got its turn");
        assert_eq!(turns.waiting(), 0);
    }

    /// A 4×2×3 world at 2 px a voxel, rise 1: raster 8 wide, base 2·2 + 3·1 = 7.
    fn proj() -> Projection {
        let mut c = cubarium_voxel::Config::default();
        c.width = 4;
        c.height = 2;
        c.depth = 3;
        Projection::new(30.0, 2, 0, &c).unwrap()
    }

    fn solid(p: &Projection, cells: &[(u32, u32, u32)]) -> Arc<Vec<u64>> {
        let mut bits = vec![0u64; ((p.width * p.height * p.depth) as usize).div_ceil(64)];
        for &(x, y, z) in cells {
            let i = ((y * p.depth + z) * p.width + x) as usize;
            bits[i / 64] |= 1 << (i % 64);
        }
        Arc::new(bits)
    }

    #[test]
    fn a_screen_point_inverts_to_its_cell() {
        let p = proj();
        assert_eq!((p.s, p.rise, p.base), (2, 1, 7));
        // Animal 9 in cell (1, 0, 1): its front face is columns 2..=3, rows
        // base − (0+1)·2 − 1·1 = 4 ..= 5; its top face row 3.
        let map = PickMap::new(&p, solid(&p, &[]), &[((1, 0, 1), 9)], 1);
        assert_eq!(map.pick(2, 4, 0.0), Some(9));
        assert_eq!(map.pick(3, 5, 0.0), Some(9));
        assert_eq!(map.pick(2, 3, 0.0), Some(9), "its top face");
        assert_eq!(map.pick(4, 4, 0.0), None, "the next column");
        // Round the ring: column 2 + 8 is column 2.
        assert_eq!(map.pick(10, 4, 0.0), Some(9));
        // A rock in front (cell (1, 0, 0), rows 5..=6 front, 4 top) hides it where it
        // covers it.
        let hidden = PickMap::new(&p, solid(&p, &[(1, 0, 0)]), &[((1, 0, 1), 9)], 1);
        assert_eq!(hidden.pick(2, 5, 0.0), None);
        assert_eq!(hidden.pick(2, 4, 0.0), None, "the rock's top covers the body's front row 4");
        assert_eq!(hidden.pick(2, 3, 0.0), Some(9), "above the rock the body shows");
    }

    #[test]
    fn the_nearer_body_wins_and_a_near_miss_finds_the_nearest() {
        let p = proj();
        // Two bodies in one column: (0, 0, 0) in front of (0, 1, 1)? Same pixel rows differ;
        // put them where one walk meets both: (0, 0, 1) front rows 4..=5 and (0, 1, 2)
        // front rows 7 − 4 − 2 = 1 ..= 2 — no overlap, so stack the nearer at z 0 over
        // the farther's top face instead: (0, 0, 0) front rows 5..=6, (0, 0, 1) top row 4.
        let map = PickMap::new(&p, solid(&p, &[]), &[((0, 0, 0), 1), ((0, 0, 1), 2)], 1);
        assert_eq!(map.pick(0, 5, 0.0), Some(1));
        assert_eq!(map.pick(0, 4, 0.0), Some(1), "the nearer body's cap row before the farther's front");
        // Nothing at column 6, row 5; within 3 px the nearest drawn cell is body 1's
        // (columns 0..=1 round the ring from 6: 2 px away).
        assert_eq!(map.pick(6, 5, 0.0), None);
        assert_eq!(map.pick(6, 5, 3.0), Some(1));
        assert_eq!(map.pick(6, 5, 1.0), None);
    }

    #[test]
    fn a_body_across_the_seam_is_drawn_at_its_middle() {
        let p = proj();
        let map = PickMap::new(&p, solid(&p, &[]), &[((3, 0, 0), 5), ((0, 0, 0), 5)], 1);
        let (x, _) = map.animal_at(5).unwrap();
        // Pixels 6..=7 and then 0..=1 round the ring: the middle is 7.5, not 3.5.
        assert!((x - 7.5).abs() < 1e-3, "{x}");
    }
}
