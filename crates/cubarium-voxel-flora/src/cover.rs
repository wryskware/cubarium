//! **The latticevine face cover** (D15 Revision 2,
//! `design/art-direction/species-dossier-D15-latticevine-2026-09-23.md`; brief
//! `design/handoffs/latticevine-cover-sim-2026-09-23.md`).
//!
//! # The model
//!
//! - A **face** is a solid terrain voxel plus an outward direction: the four horizontal
//!   sides and down (an overhang's underside). Up is the ordinary stand world and is never a
//!   face here. A face is **eligible** when its voxel is solid terrain (any material) and the
//!   voxel beyond it is **inside the world**, **air**, and not flooded — its free water under
//!   [`STANDING_SUPPORT_FILL`], so a shower's film running past a wall foot does not strip
//!   the root face while a pool rising over it does ([`face_eligible`]).
//! - A **vine** ([`Vine`]) is rooted on a support face ([`Site`]) and owns **covered faces**
//!   ([`CoveredFace`]). Sisters share a `lineage`; nothing else.
//! - **Cover adjacency** (what "spread onto an adjacent face" and "path distance along
//!   owned cover" walk over), three kinds:
//!   1. *planar*: the same direction on a face-adjacent voxel in the face's own plane (up,
//!      down or sideways along a wall; sideways along an underside);
//!   2. *convex edge*: the same voxel, two perpendicular allowed directions (round a
//!      corner of a pillar, or off a wall's bottom edge onto the underside beneath it);
//!   3. *concave corner*: two faces with perpendicular directions whose **front voxels
//!      coincide** (a wall meeting the underside of the ledge above it, or an inside
//!      corner of two walls).
//! - **Root relation** (where a founder or a sister may root, and which face it starts on):
//!   a root on support face `s` is next to (a) any face whose front voxel is the voxel
//!   directly above `s` — the **foot** of a wall — and (b) any side face of `s`'s own
//!   voxel — the **lip** of a ledge, from which the vine hangs down.
//!
//! # One tick
//!
//! [`prep`] runs before the drink: faces that stopped being eligible and vines whose root
//! support went are booked out as `removed_*`, each face's light is read (once, through the
//! layer's sky cache, and again only when the terrain version moves), and each vine's leaf
//! and light totals are summed. The drink (`step.rs`) then adds each vine's demand to the
//! same per-voxel split the stands use. [`grow`] runs after the stands grow: dormancy
//! timers, income and upkeep per vine, then one pass over the faces for dieback, leaf fall,
//! regrowth (a face greens only once the face nearer its root was green at the start of the
//! tick, so regreening runs out from the root), the spur cycle and the contest wheel; then
//! the contests, the starving vines' outermost runners, and the spread and sister checks of
//! the vines whose slot on the spread wheel is due.
//!
//! # Mass
//!
//! A vine's organic matter is its reserve plus, per face, `leafiness · leaf_mass`, the
//! runner's `runner_mass`, and any nectar and fruit; its energy is `energy_density` times
//! that, and it holds no mineral. Income is `fixed_in`, upkeep and construction are
//! `respired_out`, leaf fall, spent fruit, dropped runners and a dead vine's reserve are
//! litter on the ground below, grazing and feeding are `consumed_*_out`, founders and
//! hand-built cover are `seeded_*_in`, and terrain edits are `removed_*_out`. Regrowth,
//! spreading onto a face, nectar and fruit set, and a sister's purse are transfers inside
//! the vines.

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use cubarium_voxel::world::STANDING_SUPPORT_FILL;
use cubarium_voxel::{Config as WorldConfig, DT, Material, VoxelView, World};

use crate::step::{self, Rng};
use crate::{Flora, FloraConfig, FloraLedger, Ground, Site, Taken};

/// A vine's identity: unique for the life of the layer, never reused.
pub type VineId = u64;

/// Stream key for the spur start jitter (`step.rs` keeps the others).
const DOMAIN_SPUR: u64 = 10;
/// Stream key for the founders' draw.
const DOMAIN_VINE_FOUNDERS: u64 = 11;
/// Slack on timers and spells counted in `DT` steps: twenty additions of 0.05 are not 1.0.
const EPS: f64 = 1e-9;
/// No path from the root: a face cut off from its root by a lost or contested face.
const FAR: u32 = u32::MAX;
/// A flat voxel index not yet resolved.
const UNRESOLVED: u32 = u32::MAX;
/// The fewest voxels between two founders' roots, measured along the strip.
const FOUNDER_SPACING: i64 = 8;

/// Which way a covered face looks. Up is not here: the top of a solid is a stand's
/// support face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FaceDir {
    /// Faces `+x`: the voxel beyond is `x + 1` (wrapped).
    PosX,
    /// Faces `−x`.
    NegX,
    /// Faces `+z`.
    PosZ,
    /// Faces `−z`.
    NegZ,
    /// An underside: the voxel beyond is `y − 1`.
    Down,
}

impl FaceDir {
    pub const ALL: [FaceDir; 5] = [
        FaceDir::PosX,
        FaceDir::NegX,
        FaceDir::PosZ,
        FaceDir::NegZ,
        FaceDir::Down,
    ];
    /// The four sides.
    pub const SIDES: [FaceDir; 4] = [FaceDir::PosX, FaceDir::NegX, FaceDir::PosZ, FaceDir::NegZ];

    /// `(dx, dy, dz)` from the face's voxel to the voxel beyond it.
    pub fn offset(self) -> (i64, i64, i64) {
        match self {
            FaceDir::PosX => (1, 0, 0),
            FaceDir::NegX => (-1, 0, 0),
            FaceDir::PosZ => (0, 0, 1),
            FaceDir::NegZ => (0, 0, -1),
            FaceDir::Down => (0, -1, 0),
        }
    }

    pub fn is_underside(self) -> bool {
        self == FaceDir::Down
    }

    fn index(self) -> u64 {
        match self {
            FaceDir::PosX => 0,
            FaceDir::NegX => 1,
            FaceDir::PosZ => 2,
            FaceDir::NegZ => 3,
            FaceDir::Down => 4,
        }
    }

    /// The allowed directions perpendicular to this one.
    fn perpendicular(self) -> &'static [FaceDir] {
        match self {
            FaceDir::PosX | FaceDir::NegX => &[FaceDir::PosZ, FaceDir::NegZ, FaceDir::Down],
            FaceDir::PosZ | FaceDir::NegZ => &[FaceDir::PosX, FaceDir::NegX, FaceDir::Down],
            FaceDir::Down => &FaceDir::SIDES,
        }
    }

    /// The four voxel steps that stay in this face's plane.
    fn plane_steps(self) -> [(i64, i64, i64); 4] {
        match self {
            FaceDir::PosX | FaceDir::NegX => [(0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)],
            FaceDir::PosZ | FaceDir::NegZ => [(0, 1, 0), (0, -1, 0), (1, 0, 0), (-1, 0, 0)],
            FaceDir::Down => [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)],
        }
    }
}

/// A face address: the solid voxel `(x, y, z)` (x stored wrapped into `0..width`) and the
/// direction it looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Face {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub dir: FaceDir,
}

impl Face {
    pub fn new(x: u32, y: u32, z: u32, dir: FaceDir) -> Face {
        Face { x, y, z, dir }
    }

    /// The voxel in front of the face, `x` wrapped; `None` when it lies outside the world
    /// (`z` outside `0..depth`, `y` below 0).
    pub fn front(&self, view: &VoxelView<'_>) -> Option<(u32, u32, u32)> {
        front_in(view.config, *self)
    }

    fn key(self) -> (u64, u64, u64) {
        (
            u64::from(self.x),
            (u64::from(self.y) << 32) | u64::from(self.z),
            self.dir.index(),
        )
    }

    /// A well-mixed hash of the address: the contest wheel's phase.
    fn hash(self) -> u64 {
        let (a, b, c) = self.key();
        step::mix(step::mix(a ^ 0x5bd1_e995) ^ b ^ (c << 61))
    }
}

/// One voxel step from `(x, y, z)`, `x` wrapped; `None` off the top, bottom or walls.
fn step_voxel(
    c: &WorldConfig,
    x: u32,
    y: u32,
    z: u32,
    d: (i64, i64, i64),
) -> Option<(u32, u32, u32)> {
    let ny = y as i64 + d.1;
    let nz = z as i64 + d.2;
    if ny < 0 || ny >= c.height as i64 || nz < 0 || nz >= c.depth as i64 {
        return None;
    }
    let nx = (x as i64 + d.0).rem_euclid(c.width as i64);
    Some((nx as u32, ny as u32, nz as u32))
}

fn front_in(c: &WorldConfig, f: Face) -> Option<(u32, u32, u32)> {
    if f.y >= c.height || f.z >= c.depth {
        return None;
    }
    step_voxel(c, f.x, f.y, f.z, f.dir.offset())
}

/// Whether `face` can carry cover: its voxel is solid terrain and the voxel in front of it
/// is inside the world, air, and not flooded (free water under [`STANDING_SUPPORT_FILL`]).
/// Any solid material qualifies.
pub fn face_eligible(view: &VoxelView<'_>, face: Face) -> bool {
    let c = view.config;
    if face.x >= c.width || face.y >= c.height || face.z >= c.depth {
        return false;
    }
    if !view.material_at(face.x as i64, face.y, face.z).is_solid() {
        return false;
    }
    let Some((fx, fy, fz)) = front_in(c, face) else {
        return false;
    };
    let i = c.index(fx as i64, fy, fz);
    view.material[i] == Material::Air && view.free[i] < STANDING_SUPPORT_FILL
}

/// **The one light rule the step uses** for a face: `sky` — the sky visibility of the air
/// voxel in front of the face — times the orientation factor, 1 for the four sides and
/// [`VineConfig::underside_light`] for [`FaceDir::Down`]. Shading by stand crowns is not in
/// it (v1).
pub fn face_light(config: &VineConfig, dir: FaceDir, sky: f64) -> f64 {
    if dir.is_underside() {
        sky * config.underside_light
    } else {
        sky
    }
}

/// Every face cover-adjacent to `f` (planar, convex edge, concave corner), in a fixed
/// order, into `out`. Not screened for eligibility.
pub(crate) fn neighbours(c: &WorldConfig, f: Face, out: &mut Vec<Face>) {
    out.clear();
    for s in f.dir.plane_steps() {
        if let Some((x, y, z)) = step_voxel(c, f.x, f.y, f.z, s) {
            out.push(Face::new(x, y, z, f.dir));
        }
    }
    for &d in f.dir.perpendicular() {
        out.push(Face::new(f.x, f.y, f.z, d));
    }
    if let Some((fx, fy, fz)) = front_in(c, f) {
        for &d in f.dir.perpendicular() {
            let o = d.offset();
            if let Some((x, y, z)) = step_voxel(c, fx, fy, fz, (-o.0, -o.1, -o.2)) {
                out.push(Face::new(x, y, z, d));
            }
        }
    }
}

/// Whether a root on `s` is next to `f` (the module doc's root relation).
fn root_adjacent(c: &WorldConfig, s: Site, f: Face) -> bool {
    (f.dir != FaceDir::Down && (f.x, f.y, f.z) == (s.x, s.y, s.z))
        || front_in(c, f) == Some((s.x, s.y + 1, s.z))
}

/// The support faces a root next to `f` could sit on: under its front voxel (a foot), and
/// its own voxel for a side face (a lip). Not screened.
fn root_sites(c: &WorldConfig, f: Face) -> [Option<Site>; 2] {
    let foot = front_in(c, f).and_then(|(x, y, z)| (y >= 1).then_some(Site { x, y: y - 1, z }));
    let lip = (f.dir != FaceDir::Down).then_some(Site {
        x: f.x,
        y: f.y,
        z: f.z,
    });
    [foot, lip]
}

/// A spur's place in its cycle: bare → bud → flower → fruit → spent → bare.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpurPhase {
    #[default]
    Bare,
    Bud,
    /// Holds [`CoveredFace::nectar`].
    Flower,
    /// Holds [`CoveredFace::fruit`], the bead mass.
    Fruit,
    Spent,
}

/// One latticevine plant.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vine {
    pub id: VineId,
    /// Shared by a founder and every sister rooted from it, transitively.
    pub lineage: u64,
    /// The support face the root drinks under, like a stand's site.
    pub root: Site,
    /// The face next to the root that carries the root arch; path distances along the
    /// cover are measured from it.
    pub root_face: Face,
    /// The root box's mean available water last tick (package F's scale: 0 at the wilting
    /// point, 1 at field capacity): what income's moisture ramp and the dormancy spells
    /// read.
    pub water: f64,
    /// Pore water the root actually got last tick, cubic metres: its share of the drink
    /// path's split. For inspection and tests, like [`crate::Stand::water_m3`].
    pub drank_m3: f64,
    /// Reserve, organic material units: pays upkeep, regrowth, spread, sisters and spurs.
    pub reserve: f64,
    /// Leafless and holding: spurs stop, upkeep is woody only.
    pub dormant: bool,
    /// Seconds the root's moisture has stayed below [`VineConfig::dry_water`].
    pub dry_s: f64,
    /// Seconds the root's moisture has stayed above [`VineConfig::wet_water`].
    pub wet_s: f64,
    /// Flora tick at which it rooted.
    pub born_tick: u64,
}

/// One covered rock face.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoveredFace {
    pub face: Face,
    pub owner: VineId,
    /// `0..=1`. The face holds `leafiness · VineConfig::leaf_mass` of leaf; at 0 it is a bare
    /// runner, still owned.
    pub leafiness: f64,
    /// Whether the owner's root arch is on this face (the face next to its root).
    pub rooted: bool,
    pub spur: SpurPhase,
    /// Seconds left in the current spur phase (or of the jittered delay before a start).
    pub spur_timer_s: f64,
    /// Organic matter in the flower. Positive only in [`SpurPhase::Flower`].
    pub nectar: f64,
    /// Organic matter in the bead bunch. Positive only in [`SpurPhase::Fruit`].
    pub fruit: f64,
}

impl CoveredFace {
    fn organic(&self, vc: &VineConfig) -> f64 {
        self.leafiness * vc.leaf_mass + vc.runner_mass + self.nectar + self.fruit
    }
}

/// What the presenter draws a covered face from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceDraw {
    pub face: Face,
    pub owner: VineId,
    pub leafiness: f64,
    pub rooted: bool,
    /// The owner's dormancy.
    pub dormant: bool,
    pub spur: SpurPhase,
}

/// A founder: where it roots, the face it starts on, its reserve, and optionally the
/// lineage it joins (a test or a seeder building sisters by hand; `None` is a new lineage).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VineSeed {
    pub root: Site,
    pub first: Face,
    pub reserve: f64,
    pub lineage: Option<u64>,
}

/// Per-face working state, derived from the saved records and rebuilt when missing.
#[derive(Clone, Copy, Debug)]
struct FaceExtra {
    /// Path distance from the owner's root face along its cover; [`FAR`] when cut off.
    dist: u32,
    /// The owned face one step nearer the root.
    parent: Option<Face>,
    /// [`face_light`] of the face; NaN until read, and again after a terrain change.
    light: f64,
    /// Where this face's litter lands; `None` until resolved.
    litter: Option<Site>,
    /// Leafy at the start of this tick: the regreening gate.
    green0: bool,
    /// Flat indices of the face's voxel and its front voxel ([`UNRESOLVED`] until read).
    voxel: u32,
    front: u32,
    /// BFS visit stamp.
    seen: u32,
}

impl FaceExtra {
    fn new(dist: u32, parent: Option<Face>) -> FaceExtra {
        FaceExtra {
            dist,
            parent,
            light: f64::NAN,
            litter: None,
            green0: false,
            voxel: UNRESOLVED,
            front: UNRESOLVED,
            seen: 0,
        }
    }
}

/// Per-vine working state, aligned with `Cover::vines`.
#[derive(Clone, Copy, Debug, Default)]
struct VineAux {
    /// Covered faces owned.
    faces: u32,
    /// Σ leafiness and Σ light · leafiness at the start of the tick.
    leaf: f64,
    light_leaf: f64,
    /// The drink's read: root box mean available water, the moisture ramp, the demand
    /// asked and what the split gave.
    mu: f64,
    demand: f64,
    drank: f64,
    /// Dieback this tick: the fraction of every face's leaf that survives.
    leaf_scale: f64,
    /// Upkeep unpaid even by the leaves: the outermost runner goes.
    starving: bool,
}

/// The rebuilt-on-demand indexes (never saved).
#[derive(Clone, Debug, Default)]
struct Runtime {
    built: bool,
    /// Face → slot in `Cover::faces`.
    index: FxHashMap<Face, u32>,
    extra: Vec<FaceExtra>,
    aux: Vec<VineAux>,
    /// Root site → the vine rooted there.
    roots: FxHashMap<Site, VineId>,
    /// The terrain version the lights and litter sites were read against.
    light_version: Option<u64>,
    /// Vines whose path distances need a fresh walk.
    dirty: Vec<VineId>,
    stamp: u32,
    /// Scratch.
    nbuf: Vec<Face>,
}

/// The vines and their covered faces: dense records with an FxHash index keyed by face,
/// the working indexes rebuilt from the records after a load.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cover {
    /// Sorted by id.
    vines: Vec<Vine>,
    /// Slot order (a removal swaps the last record in).
    faces: Vec<CoveredFace>,
    next_vine: VineId,
    next_lineage: u64,
    /// The tick the last shower ended on: spurs may start within `spur_window_s` of it.
    shower_end_tick: Option<u64>,
    /// Faces to contest at the next step: crossed below the threshold between ticks.
    contest_queue: Vec<Face>,
    #[serde(skip)]
    rt: Runtime,
}

impl PartialEq for Cover {
    fn eq(&self, other: &Cover) -> bool {
        self.vines == other.vines
            && self.faces() == other.faces()
            && self.next_vine == other.next_vine
            && self.next_lineage == other.next_lineage
            && self.shower_end_tick == other.shower_end_tick
    }
}

fn vine_at(vines: &[Vine], id: VineId) -> Option<usize> {
    vines.binary_search_by_key(&id, |v| v.id).ok()
}

impl Cover {
    /// Every living vine, sorted by id.
    pub fn vines(&self) -> &[Vine] {
        &self.vines
    }

    pub fn vine(&self, id: VineId) -> Option<&Vine> {
        vine_at(&self.vines, id).map(|i| &self.vines[i])
    }

    /// The covered face at `face`, if any vine owns it.
    pub fn face(&self, face: Face) -> Option<&CoveredFace> {
        if self.rt.built {
            self.rt.index.get(&face).map(|&s| &self.faces[s as usize])
        } else {
            self.faces.iter().find(|c| c.face == face)
        }
    }

    /// How many faces are covered.
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Every covered face, sorted by face.
    pub fn faces(&self) -> Vec<CoveredFace> {
        let mut out = self.faces.clone();
        out.sort_unstable_by_key(|c| c.face);
        out
    }

    /// The faces `id` owns, sorted by face.
    pub fn faces_of(&self, id: VineId) -> Vec<CoveredFace> {
        let mut out: Vec<CoveredFace> = self
            .faces
            .iter()
            .filter(|c| c.owner == id)
            .copied()
            .collect();
        out.sort_unstable_by_key(|c| c.face);
        out
    }

    /// One [`FaceDraw`] per covered face, sorted by face: the presenter's whole read.
    pub fn draw(&self) -> Vec<FaceDraw> {
        let mut out: Vec<FaceDraw> = self
            .faces
            .iter()
            .map(|c| FaceDraw {
                face: c.face,
                owner: c.owner,
                leafiness: c.leafiness,
                rooted: c.rooted,
                dormant: self.vine(c.owner).is_some_and(|v| v.dormant),
                spur: c.spur,
            })
            .collect();
        out.sort_unstable_by_key(|d| d.face);
        out
    }

    /// Organic matter in every vine: reserves, leaves, runners, nectar and fruit.
    pub(crate) fn organic(&self, vc: &VineConfig) -> f64 {
        self.vines.iter().map(|v| v.reserve).sum::<f64>()
            + self.faces.iter().map(|c| c.organic(vc)).sum::<f64>()
    }

    // ------------------------------------------------------------ indexes

    fn ensure_built(&mut self) {
        if self.rt.built {
            return;
        }
        let rt = &mut self.rt;
        rt.index.clear();
        rt.index.reserve(self.faces.len());
        rt.extra.clear();
        rt.aux = vec![VineAux::default(); self.vines.len()];
        rt.roots.clear();
        for (s, c) in self.faces.iter().enumerate() {
            rt.index.insert(c.face, s as u32);
            rt.extra.push(FaceExtra::new(FAR, None));
            if let Some(vi) = vine_at(&self.vines, c.owner) {
                rt.aux[vi].faces += 1;
            }
        }
        for v in &self.vines {
            rt.roots.insert(v.root, v.id);
        }
        rt.light_version = None;
        rt.dirty = self.vines.iter().map(|v| v.id).collect();
        rt.built = true;
    }

    fn mark_dirty(&mut self, id: VineId) {
        self.rt.dirty.push(id);
    }

    fn add_face(&mut self, cf: CoveredFace, dist: u32, parent: Option<Face>) {
        let slot = self.faces.len() as u32;
        self.rt.index.insert(cf.face, slot);
        self.faces.push(cf);
        self.rt.extra.push(FaceExtra::new(dist, parent));
        if let Some(vi) = vine_at(&self.vines, cf.owner) {
            self.rt.aux[vi].faces += 1;
        }
    }

    /// Remove the record in `slot`, swapping the last one in; returns it.
    fn remove_slot(&mut self, slot: usize) -> CoveredFace {
        let cf = self.faces.swap_remove(slot);
        self.rt.extra.swap_remove(slot);
        self.rt.index.remove(&cf.face);
        if slot < self.faces.len() {
            self.rt.index.insert(self.faces[slot].face, slot as u32);
        }
        if let Some(vi) = vine_at(&self.vines, cf.owner) {
            self.rt.aux[vi].faces = self.rt.aux[vi].faces.saturating_sub(1);
        }
        self.rt.dirty.push(cf.owner);
        cf
    }

    fn add_vine(&mut self, v: Vine) {
        debug_assert!(self.vines.last().is_none_or(|l| l.id < v.id));
        self.rt.roots.insert(v.root, v.id);
        self.vines.push(v);
        self.rt.aux.push(VineAux::default());
    }

    fn remove_vine(&mut self, vi: usize) -> Vine {
        let v = self.vines.remove(vi);
        self.rt.aux.remove(vi);
        if self.rt.roots.get(&v.root) == Some(&v.id) {
            self.rt.roots.remove(&v.root);
        }
        v
    }

    fn slot_of(&self, f: Face) -> Option<usize> {
        self.rt.index.get(&f).map(|&s| s as usize)
    }

    /// Breadth-first walk over the faces `owner` owns, from `start`, in visit order:
    /// `(slot, distance, parent)`. Marks visits with a fresh stamp.
    fn walk(
        &mut self,
        c: &WorldConfig,
        start: usize,
        owner: VineId,
        out: &mut Vec<(u32, u32, Option<Face>)>,
    ) {
        out.clear();
        self.rt.stamp = self.rt.stamp.wrapping_add(1);
        if self.rt.stamp == 0 {
            for e in &mut self.rt.extra {
                e.seen = 0;
            }
            self.rt.stamp = 1;
        }
        let stamp = self.rt.stamp;
        let mut nbuf = std::mem::take(&mut self.rt.nbuf);
        self.rt.extra[start].seen = stamp;
        out.push((start as u32, 0, None));
        let mut head = 0;
        while head < out.len() {
            let (slot, d, _) = out[head];
            head += 1;
            let here = self.faces[slot as usize].face;
            neighbours(c, here, &mut nbuf);
            for &n in &nbuf {
                if let Some(&ns) = self.rt.index.get(&n) {
                    let ns = ns as usize;
                    if self.faces[ns].owner == owner && self.rt.extra[ns].seen != stamp {
                        self.rt.extra[ns].seen = stamp;
                        out.push((ns as u32, d + 1, Some(here)));
                    }
                }
            }
        }
        self.rt.nbuf = nbuf;
    }

    /// Fresh path distances for vine `vi` from its root face: every face reached gets its
    /// distance and parent; the rest are left to the caller. Returns the walk.
    fn walk_vine(&mut self, c: &WorldConfig, vi: usize, out: &mut Vec<(u32, u32, Option<Face>)>) {
        out.clear();
        let v = self.vines[vi];
        let Some(start) = self.slot_of(v.root_face) else {
            return;
        };
        if self.faces[start].owner != v.id {
            return;
        }
        self.walk(c, start, v.id, out);
        for &(s, d, p) in out.iter() {
            let e = &mut self.rt.extra[s as usize];
            e.dist = d;
            e.parent = p;
        }
    }

    /// Every slot `id` owns (a scan: events only).
    fn slots_of(&self, id: VineId) -> Vec<usize> {
        (0..self.faces.len())
            .filter(|&s| self.faces[s].owner == id)
            .collect()
    }

    /// Reserve per covered face: the contest's score.
    fn score(&self, vi: usize) -> f64 {
        self.vines[vi].reserve / f64::from(self.rt.aux[vi].faces.max(1))
    }
}

/// The latticevine's numbers: [`crate::FloraConfig::latticevine`]. Every default is a
/// placeholder (`design/backlog.md` §1), authored on the 0.25 m reference voxel; only the
/// root box is rescaled on a finer grid ([`crate::FloraConfig::for_voxel_size`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VineConfig {
    /// Root box, as a stand's: voxels down from the support, and sideways.
    pub rooting_depth: u32,
    pub rooting_radius: u32,
    /// Water demand per unit of total leafiness, cubic metres per second, times the
    /// moisture ramp. Not capped: a thirsty vine can empty its box, and shares it with any
    /// stand there through the one split.
    pub transpiration_m3_per_s: f64,
    /// Income moisture ramp on the available-water scale (package F): 0 at `wilt_water`,
    /// 1 at `full_water`.
    pub wilt_water: f64,
    pub full_water: f64,
    /// Dormancy: the root's available water below `dry_water` for `dry_spell_s` means
    /// dormancy; above `wet_water` (> `dry_water`) for `wet_spell_s` means regreening.
    pub dry_water: f64,
    pub wet_water: f64,
    pub dry_spell_s: f64,
    pub wet_spell_s: f64,
    /// How long the leaf fall takes once dormancy starts, seconds (a full face; a thinner
    /// one is bare sooner).
    pub leaf_fall_s: f64,
    /// Organic income per second of a face at light 1, leafiness 1, moisture 1, water met.
    pub assimilation: f64,
    /// Orientation factor of an underside ([`face_light`]); the sides are 1.
    pub underside_light: f64,
    /// Organic matter a face's leaves hold at leafiness 1.
    pub leaf_mass: f64,
    /// Organic matter in one face's runner (wood).
    pub runner_mass: f64,
    /// Leaf upkeep per second of a face at leafiness 1, organic units.
    pub leaf_upkeep: f64,
    /// Woody upkeep per second per covered face, paid dormant or not.
    pub wood_upkeep: f64,
    /// Leafiness per second a paid face regains.
    pub regrow_rate: f64,
    /// Reserve paid to cover one new face: its runner, the rest respired as construction.
    /// The new face also buys leaf up to `contest_threshold`, so it is born not contestable.
    pub spread_cost: f64,
    /// How often a vine tries to spread (and to root a sister), seconds.
    pub spread_check_s: f64,
    /// Reserve the parent hands a sister: the sister's starting reserve.
    pub sister_cost: f64,
    /// A sister roots only from a face at least this many faces from the parent's root
    /// along the cover, so a colony along a wall foot does not root at every face.
    pub sister_min_dist: u32,
    /// Leafiness below which a face is contestable.
    pub contest_threshold: f64,
    /// The slow check wheel's period for contestable faces, seconds.
    pub contest_check_s: f64,
    /// Reserve **per covered face** a vine needs before its spurs start a cycle.
    pub spur_reserve_min: f64,
    /// How long after a shower ends a cycle may still start, seconds.
    pub spur_window_s: f64,
    /// Spread of the per-face start delay, seconds.
    pub spur_jitter_s: f64,
    /// Phase lengths, seconds.
    pub bud_s: f64,
    pub flower_s: f64,
    pub fruit_s: f64,
    pub spent_s: f64,
    /// Organic matter a flower holds when it opens, and a bead bunch when it sets.
    pub nectar: f64,
    pub fruit: f64,
    /// Energy per unit of organic matter in the vine's tissue.
    pub energy_density: f64,
    /// Founder vines [`Flora::seed_vine_founders`] roots on a fresh world.
    pub founders: u32,
    /// Each founder's starting reserve.
    pub founder_reserve: f64,
}

impl Default for VineConfig {
    fn default() -> VineConfig {
        VineConfig {
            rooting_depth: 2,
            rooting_radius: 1,
            transpiration_m3_per_s: 5e-8,
            wilt_water: 0.1,
            full_water: 0.5,
            dry_water: 0.15,
            wet_water: 0.4,
            dry_spell_s: 600.0,
            wet_spell_s: 300.0,
            leaf_fall_s: 60.0,
            assimilation: 0.001,
            underside_light: 0.15,
            leaf_mass: 0.05,
            runner_mass: 0.02,
            leaf_upkeep: 0.0001,
            wood_upkeep: 0.00001,
            regrow_rate: 0.01,
            spread_cost: 0.03,
            spread_check_s: 5.0,
            sister_cost: 0.2,
            sister_min_dist: 4,
            contest_threshold: 0.2,
            contest_check_s: 30.0,
            spur_reserve_min: 0.02,
            spur_window_s: 120.0,
            spur_jitter_s: 60.0,
            bud_s: 60.0,
            flower_s: 120.0,
            fruit_s: 240.0,
            spent_s: 60.0,
            nectar: 0.002,
            fruit: 0.005,
            energy_density: 1.0,
            founders: 6,
            founder_reserve: 0.3,
        }
    }
}

impl VineConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (label, v) in [
            ("transpiration_m3_per_s", self.transpiration_m3_per_s),
            ("wilt_water", self.wilt_water),
            ("full_water", self.full_water),
            ("dry_water", self.dry_water),
            ("wet_water", self.wet_water),
            ("dry_spell_s", self.dry_spell_s),
            ("wet_spell_s", self.wet_spell_s),
            ("assimilation", self.assimilation),
            ("underside_light", self.underside_light),
            ("leaf_mass", self.leaf_mass),
            ("runner_mass", self.runner_mass),
            ("leaf_upkeep", self.leaf_upkeep),
            ("wood_upkeep", self.wood_upkeep),
            ("regrow_rate", self.regrow_rate),
            ("spread_cost", self.spread_cost),
            ("sister_cost", self.sister_cost),
            ("contest_threshold", self.contest_threshold),
            ("spur_reserve_min", self.spur_reserve_min),
            ("spur_window_s", self.spur_window_s),
            ("spur_jitter_s", self.spur_jitter_s),
            ("bud_s", self.bud_s),
            ("flower_s", self.flower_s),
            ("fruit_s", self.fruit_s),
            ("spent_s", self.spent_s),
            ("nectar", self.nectar),
            ("fruit", self.fruit),
            ("energy_density", self.energy_density),
            ("founder_reserve", self.founder_reserve),
        ] {
            if !v.is_finite() || v < 0.0 {
                return Err(format!(
                    "latticevine: {label} is {v}, not finite and nonnegative"
                ));
            }
        }
        for (label, v) in [
            ("leaf_fall_s", self.leaf_fall_s),
            ("spread_check_s", self.spread_check_s),
            ("contest_check_s", self.contest_check_s),
            ("leaf_mass", self.leaf_mass),
        ] {
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("latticevine: {label} is {v}, not positive"));
            }
        }
        if self.wet_water <= self.dry_water {
            return Err(format!(
                "latticevine: wet_water {} must be above dry_water {}",
                self.wet_water, self.dry_water
            ));
        }
        if self.spread_cost < self.runner_mass {
            return Err(format!(
                "latticevine: spread_cost {} cannot pay the runner_mass {} it builds",
                self.spread_cost, self.runner_mass
            ));
        }
        if self.contest_threshold > 1.0 {
            return Err("latticevine: contest_threshold is above full leaf".into());
        }
        Ok(())
    }
}

/// `period` seconds as whole ticks, at least one.
fn period_ticks(seconds: f64) -> u64 {
    ((seconds / DT).round() as u64).max(1)
}

/// The site a face's litter lands on: the first solid under its front voxel, which is a
/// support face by construction; `fallback` when there is none.
fn litter_site(view: &VoxelView<'_>, f: Face, fallback: Site) -> Site {
    if let Some((x, y, z)) = front_in(view.config, f) {
        for yy in (0..y).rev() {
            if view.material_at(x as i64, yy, z).is_solid() {
                return Site { x, y: yy, z };
            }
        }
    }
    fallback
}

/// Book `organic` of vine tissue into the litter at `site`, through the one litter rule.
fn deposit_litter(
    config: &FloraConfig,
    ground: &mut Vec<Ground>,
    ledger: &mut FloraLedger,
    site: Site,
    organic: f64,
) {
    if !(organic > 0.0) {
        return;
    }
    let e = config.latticevine.energy_density;
    let gi = step::provisioned_slot(config, ground, site, ledger);
    step::add_litter_cap(
        config.litter_energy_cap,
        &mut ground[gi],
        organic,
        0.0,
        e * organic,
        ledger,
    );
    ledger.vine_litter += organic;
}

/// Deposit a batch of `(site, organic)` litter, summed per site, in site order.
fn flush_litter(
    config: &FloraConfig,
    ground: &mut Vec<Ground>,
    ledger: &mut FloraLedger,
    litter: &mut Vec<(Site, f64)>,
) {
    if litter.is_empty() {
        return;
    }
    litter.sort_by_key(|e| e.0);
    let mut i = 0;
    while i < litter.len() {
        let site = litter[i].0;
        let mut sum = 0.0;
        while i < litter.len() && litter[i].0 == site {
            sum += litter[i].1;
            i += 1;
        }
        deposit_litter(config, ground, ledger, site, sum);
    }
    litter.clear();
}

fn book_removed(ledger: &mut FloraLedger, e: f64, organic: f64) {
    ledger.removed_organic_out += organic;
    ledger.removed_energy_out += e * organic;
}

fn book_respired(ledger: &mut FloraLedger, e: f64, organic: f64) {
    ledger.respired_out += organic;
    ledger.heat_out += e * organic;
}

// ------------------------------------------------------------------ the tick

/// Before the drink: terrain removals, lights, and each vine's leaf and light totals.
pub(crate) fn prep(flora: &mut Flora, world: &World) {
    let Flora {
        config,
        cover,
        ledger,
        sky,
        ..
    } = flora;
    cover.ensure_built();
    if cover.faces.is_empty() && cover.vines.is_empty() {
        return;
    }
    let view = world.view();
    let c = view.config;
    let vc = &config.latticevine;
    let e = vc.energy_density;

    if cover.rt.light_version != Some(view.terrain_version) {
        for x in &mut cover.rt.extra {
            x.light = f64::NAN;
            x.litter = None;
        }
        cover.rt.light_version = Some(view.terrain_version);
    }

    // A vine whose root support is gone dies whole: its tissue is booked out with the
    // terrain that took it, as a stand's is.
    let mut vi = 0;
    while vi < cover.vines.len() {
        let v = cover.vines[vi];
        if view.is_support(v.root.x as i64, v.root.y, v.root.z) {
            vi += 1;
            continue;
        }
        for s in cover.slots_of(v.id).into_iter().rev() {
            let cf = cover.remove_slot(s);
            book_removed(ledger, e, cf.organic(vc));
            ledger.vine_faces_lost += 1;
        }
        book_removed(ledger, e, v.reserve);
        cover.remove_vine(vi);
        ledger.vine_deaths += 1;
    }

    // Faces that stopped being eligible go the same way; then the totals.
    cover.rt.dirty.sort_unstable();
    cover.rt.dirty.dedup();
    let mut need_light: Vec<u32> = Vec::new();
    let mut s = 0;
    while s < cover.faces.len() {
        let f = cover.faces[s].face;
        let x = &mut cover.rt.extra[s];
        if x.voxel == UNRESOLVED {
            x.voxel = if f.x < c.width && f.y < c.height && f.z < c.depth {
                c.index(f.x as i64, f.y, f.z) as u32
            } else {
                UNRESOLVED - 1
            };
            x.front =
                front_in(c, f).map_or(UNRESOLVED - 1, |(a, b, d)| c.index(a as i64, b, d) as u32);
        }
        let ok = x.voxel < UNRESOLVED - 1
            && x.front < UNRESOLVED - 1
            && view.material[x.voxel as usize].is_solid()
            && view.material[x.front as usize] == Material::Air
            && view.free[x.front as usize] < STANDING_SUPPORT_FILL;
        if !ok {
            let cf = cover.remove_slot(s);
            book_removed(ledger, e, cf.organic(vc));
            ledger.vine_faces_lost += 1;
            continue;
        }
        if x.light.is_nan() {
            need_light.push(s as u32);
        }
        s += 1;
    }
    cover.rt.dirty.sort_unstable();
    cover.rt.dirty.dedup();

    if !need_light.is_empty() {
        // The layer's sky cache, keyed by the support face under the front voxel: the sky
        // of the front voxel's floor. Missing keys are read and merged in one sort.
        let key = |f: Face| {
            front_in(c, f).and_then(|(x, y, z)| (y >= 1).then_some(Site { x, y: y - 1, z }))
        };
        let mut missing: Vec<Site> = need_light
            .iter()
            .filter_map(|&s| key(cover.faces[s as usize].face))
            .filter(|k| sky.binary_search_by_key(k, |e| e.0).is_err())
            .collect();
        missing.sort_unstable();
        missing.dedup();
        if !missing.is_empty() {
            #[cfg(feature = "profile")]
            cubarium_voxel::profile::add(
                cubarium_voxel::profile::Count::SkyRays,
                missing.len() as u64,
            );
            sky.extend(
                missing
                    .iter()
                    .map(|&k| (k, view.sky_visibility(k.x as i64, k.y, k.z))),
            );
            sky.sort_by_key(|e| e.0);
        }
        for &s in &need_light {
            let f = cover.faces[s as usize].face;
            let sv = key(f).map_or(0.0, |k| step::sky_at(sky, &view, k));
            cover.rt.extra[s as usize].light = face_light(vc, f.dir, sv);
        }
    }

    for a in &mut cover.rt.aux {
        a.leaf = 0.0;
        a.light_leaf = 0.0;
    }
    let any_dirty = !cover.rt.dirty.is_empty();
    for s in 0..cover.faces.len() {
        let cf = &cover.faces[s];
        let x = &mut cover.rt.extra[s];
        x.green0 = cf.leafiness > 0.0;
        if any_dirty && cover.rt.dirty.binary_search(&cf.owner).is_ok() {
            x.dist = FAR;
            x.parent = None;
        }
        if let Some(vi) = vine_at(&cover.vines, cf.owner) {
            let a = &mut cover.rt.aux[vi];
            a.leaf += cf.leafiness;
            a.light_leaf += x.light * cf.leafiness;
        }
    }
    if any_dirty {
        let dirty = std::mem::take(&mut cover.rt.dirty);
        let mut walk = Vec::new();
        for id in dirty {
            if let Some(vi) = vine_at(&cover.vines, id) {
                cover.walk_vine(c, vi, &mut walk);
            }
        }
    }
}

/// The drink's read for the vines: each root box's mean available water and moisture
/// ramp, the vine's demand, and that demand shared over the box's cells as
/// `(voxel, vine index, want)` in vine order.
pub(crate) fn drink_wants(
    flora: &mut Flora,
    view: &VoxelView<'_>,
    box_: &mut Vec<usize>,
    drinkable: &mut Vec<f64>,
    wants: &mut Vec<(u32, u32, f64)>,
) {
    wants.clear();
    let Flora { config, cover, .. } = flora;
    let vc = &config.latticevine;
    let volume = view.config.voxel_volume();
    for vi in 0..cover.vines.len() {
        let root = cover.vines[vi].root;
        step::root_box_dims_into(view, root, vc.rooting_depth, vc.rooting_radius, box_);
        drinkable.clear();
        let (mut water, mut band) = (0.0, 0.0);
        for &i in box_.iter() {
            let m = view.material[i];
            let pore = view.pore[i];
            let w = (m.field_capacity() - m.wilting_point()).max(0.0) * m.pore_capacity();
            water += step::available_water(m, pore) * w;
            band += w;
            drinkable.push((pore - m.wilting_point()).max(0.0) * m.pore_capacity() * volume);
        }
        let avail = if band > 0.0 { water / band } else { 0.0 };
        let mu = step::ramp(avail, vc.wilt_water, vc.full_water);
        let a = &mut cover.rt.aux[vi];
        let demand = (vc.transpiration_m3_per_s * a.leaf * mu * DT).max(0.0);
        a.mu = mu;
        a.demand = demand;
        a.drank = 0.0;
        cover.vines[vi].water = avail;
        if demand <= 0.0 || box_.is_empty() {
            continue;
        }
        let stock: f64 = drinkable.iter().sum();
        if stock <= 0.0 {
            continue;
        }
        for (&v, &d) in box_.iter().zip(drinkable.iter()) {
            let share = demand * d / stock;
            if share > 0.0 {
                wants.push((v as u32, vi as u32, share));
            }
        }
    }
}

/// The drink's credit: a vine's share of what each of its cells gave up.
pub(crate) fn drink_credit(flora: &mut Flora, vi: usize, got: f64) {
    flora.cover.rt.aux[vi].drank += got;
}

/// After the drink's split: the vines' `drank_m3`.
pub(crate) fn drink_done(flora: &mut Flora) {
    let cover = &mut flora.cover;
    for (v, a) in cover.vines.iter_mut().zip(&cover.rt.aux) {
        v.drank_m3 = a.drank;
    }
}

/// After the stands grow: dormancy, the economy, the per-face pass, contests, starvation,
/// spread and sisters.
pub(crate) fn grow(flora: &mut Flora, world: &World) {
    let tick = flora.tick;
    let view = world.view();
    let raining = view.is_raining();
    let shower_end = flora.was_raining && !raining;
    let Flora {
        config,
        cover,
        ground,
        ledger,
        ..
    } = flora;
    cover.ensure_built();
    if shower_end {
        cover.shower_end_tick = Some(tick);
    }
    if cover.vines.is_empty() {
        cover.contest_queue.clear();
        return;
    }
    let c = view.config;
    let vc = &config.latticevine;
    let e = vc.energy_density;
    let thr = vc.contest_threshold;

    // ---- 1. dormancy and the economy, per vine
    for vi in 0..cover.vines.len() {
        let a = cover.rt.aux[vi];
        let v = &mut cover.vines[vi];
        if v.water < vc.dry_water {
            v.dry_s += DT;
        } else {
            v.dry_s = 0.0;
        }
        if v.water > vc.wet_water {
            v.wet_s += DT;
        } else {
            v.wet_s = 0.0;
        }
        if !v.dormant && v.dry_s >= vc.dry_spell_s - EPS {
            v.dormant = true;
            ledger.vine_dormancies += 1;
        } else if v.dormant && v.wet_s >= vc.wet_spell_s - EPS {
            v.dormant = false;
        }

        let water = if a.demand > 0.0 {
            (a.drank / a.demand).min(1.0)
        } else {
            1.0
        };
        let income = if v.dormant {
            0.0
        } else {
            (vc.assimilation * a.mu * water * a.light_leaf * DT).max(0.0)
        };
        v.reserve += income;
        ledger.fixed_in += income;
        ledger.light_in += e * income;

        let upkeep = (vc.leaf_upkeep * a.leaf + vc.wood_upkeep * f64::from(a.faces)) * DT;
        let paid = upkeep.min(v.reserve).max(0.0);
        v.reserve -= paid;
        book_respired(ledger, e, paid);
        let short = upkeep - paid;
        let aux = &mut cover.rt.aux[vi];
        aux.leaf_scale = 1.0;
        aux.starving = false;
        if short > 1e-15 {
            // Dieback: the leaves are burned for the rest, evenly; what the leaves cannot
            // pay, a runner can: the outermost goes.
            let leaf = a.leaf * vc.leaf_mass;
            if !v.dormant && leaf > short {
                aux.leaf_scale = 1.0 - short / leaf;
            } else {
                if !v.dormant && leaf > 0.0 {
                    aux.leaf_scale = 0.0;
                }
                aux.starving = true;
            }
        }
    }

    // ---- 2. one pass over the faces
    let window = period_ticks(vc.spur_window_s);
    let in_window = cover
        .shower_end_tick
        .is_some_and(|t| tick.saturating_sub(t) <= window);
    let wheel = period_ticks(vc.contest_check_s);
    let wheel_slot = tick % wheel;
    let fall = DT / vc.leaf_fall_s;
    let mut litter: Vec<(Site, f64)> = Vec::new();
    let world_seed = c.seed;
    {
        let Cover {
            vines,
            faces,
            contest_queue,
            rt,
            ..
        } = cover;
        for s in 0..faces.len() {
            let owner = faces[s].owner;
            let Some(vi) = vine_at(vines, owner) else {
                continue;
            };
            // The regreening gate, read before this face is borrowed: the face one step
            // nearer the root was leafy when the tick began (or there is no such face).
            let x = rt.extra[s];
            let open = faces[s].rooted
                || x.dist == 0
                || match x.parent {
                    None => true,
                    Some(p) => match rt.index.get(&p) {
                        Some(&ps) if faces[ps as usize].owner == owner => {
                            rt.extra[ps as usize].green0
                        }
                        _ => true,
                    },
                };
            let a = rt.aux[vi];
            let v = &mut vines[vi];
            let f = &mut faces[s];
            let (face, root) = (f.face, v.root);
            let before = f.leafiness;

            if a.leaf_scale < 1.0 && f.leafiness > 0.0 {
                let kept = f.leafiness * a.leaf_scale;
                let burned = (f.leafiness - kept) * vc.leaf_mass;
                f.leafiness = kept;
                book_respired(ledger, e, burned);
            }
            if v.dormant {
                if f.leafiness > 0.0 {
                    let dl = fall.min(f.leafiness);
                    let lost = if f.leafiness - dl <= 1e-12 {
                        std::mem::replace(&mut f.leafiness, 0.0)
                    } else {
                        f.leafiness -= dl;
                        dl
                    };
                    drop_at(rt, s, &view, face, root, &mut litter, lost * vc.leaf_mass);
                }
            } else if open && f.leafiness < 1.0 && vc.regrow_rate > 0.0 && v.reserve > 0.0 {
                let mut dl = (vc.regrow_rate * DT).min(1.0 - f.leafiness);
                let mut cost = dl * vc.leaf_mass;
                if cost > v.reserve {
                    cost = v.reserve;
                    dl = cost / vc.leaf_mass;
                }
                v.reserve -= cost;
                f.leafiness += dl;
                if 1.0 - f.leafiness < 1e-12 {
                    f.leafiness = 1.0;
                }
            }

            // Spurs.
            match f.spur {
                SpurPhase::Bare => {
                    if v.dormant {
                        f.spur_timer_s = 0.0;
                    } else if shower_end {
                        let (ka, kb, kc) = f.face.key();
                        let mut rng =
                            Rng::keyed(DOMAIN_SPUR, ka ^ world_seed.rotate_left(17), kb ^ tick, kc);
                        f.spur_timer_s = DT + rng.unit() * vc.spur_jitter_s;
                    } else if f.spur_timer_s > 0.0 {
                        f.spur_timer_s -= DT;
                        if f.spur_timer_s <= EPS {
                            f.spur_timer_s = 0.0;
                            let per_face = v.reserve / f64::from(a.faces.max(1));
                            if in_window && per_face >= vc.spur_reserve_min {
                                f.spur = SpurPhase::Bud;
                                f.spur_timer_s = vc.bud_s;
                            }
                        }
                    }
                }
                SpurPhase::Bud => {
                    if v.dormant {
                        f.spur = SpurPhase::Bare;
                        f.spur_timer_s = 0.0;
                    } else {
                        f.spur_timer_s -= DT;
                        if f.spur_timer_s <= EPS {
                            let n = vc.nectar.min(v.reserve).max(0.0);
                            v.reserve -= n;
                            f.nectar = n;
                            f.spur = SpurPhase::Flower;
                            f.spur_timer_s = vc.flower_s;
                        }
                    }
                }
                SpurPhase::Flower => {
                    if v.dormant {
                        v.reserve += f.nectar;
                        f.nectar = 0.0;
                        f.spur = SpurPhase::Bare;
                        f.spur_timer_s = 0.0;
                    } else {
                        f.spur_timer_s -= DT;
                        if f.spur_timer_s <= EPS {
                            // What the visitors left is taken back; the beads are paid.
                            v.reserve += f.nectar;
                            f.nectar = 0.0;
                            let b = vc.fruit.min(v.reserve).max(0.0);
                            v.reserve -= b;
                            f.fruit = b;
                            f.spur = SpurPhase::Fruit;
                            f.spur_timer_s = vc.fruit_s;
                        }
                    }
                }
                SpurPhase::Fruit => {
                    // Beads already set stay through a dormancy until eaten or spent.
                    f.spur_timer_s -= DT;
                    if f.spur_timer_s <= EPS {
                        let b = f.fruit;
                        f.fruit = 0.0;
                        if b > 0.0 {
                            drop_at(rt, s, &view, face, root, &mut litter, b);
                        }
                        f.spur = SpurPhase::Spent;
                        f.spur_timer_s = vc.spent_s;
                    }
                }
                SpurPhase::Spent => {
                    f.spur_timer_s -= DT;
                    if f.spur_timer_s <= EPS {
                        f.spur = SpurPhase::Bare;
                        f.spur_timer_s = 0.0;
                    }
                }
            }

            // A face that crossed below the threshold this tick, or a weak one whose slot on
            // the contest wheel is due.
            if f.leafiness < thr
                && !f.rooted
                && (before >= thr || f.face.hash() % wheel == wheel_slot)
            {
                contest_queue.push(f.face);
            }
        }
    }
    flush_litter(config, ground, ledger, &mut litter);

    // ---- 3. contests
    let mut queue = std::mem::take(&mut cover.contest_queue);
    queue.sort_unstable();
    queue.dedup();
    for f in queue {
        contest(cover, c, f, thr, ledger);
    }

    // ---- 4. starving vines lose their outermost runner
    let starving: Vec<VineId> = cover
        .vines
        .iter()
        .zip(&cover.rt.aux)
        .filter(|(_, a)| a.starving)
        .map(|(v, _)| v.id)
        .collect();
    for id in starving {
        drop_outermost(cover, c, &view, id, vc, &mut litter, ledger);
    }
    flush_litter(config, ground, ledger, &mut litter);

    // ---- 5. spread and sisters, for the vines due on the spread wheel
    let spread_period = period_ticks(vc.spread_check_s);
    let due: Vec<VineId> = cover
        .vines
        .iter()
        .filter(|v| {
            !v.dormant
                && step::mix(v.id)
                    .wrapping_add(tick)
                    .is_multiple_of(spread_period)
        })
        .map(|v| v.id)
        .collect();
    for id in due {
        spread(cover, &view, id, vc, tick, ledger);
    }

    // ---- 6. a vine with no faces left is dead: its reserve falls at its root
    let mut vi = 0;
    while vi < cover.vines.len() {
        if cover.rt.aux[vi].faces > 0 {
            vi += 1;
            continue;
        }
        let v = cover.remove_vine(vi);
        deposit_litter(config, ground, ledger, v.root, v.reserve);
        ledger.vine_deaths += 1;
    }
}

/// Queue `organic` of face `s`'s tissue for the litter under it.
fn drop_at(
    rt: &mut Runtime,
    s: usize,
    view: &VoxelView<'_>,
    face: Face,
    root: Site,
    litter: &mut Vec<(Site, f64)>,
    organic: f64,
) {
    let site = *rt.extra[s]
        .litter
        .get_or_insert_with(|| litter_site(view, face, root));
    litter.push((site, organic));
}

/// One contested face: its owner keeps it unless an owner of an adjacent face has strictly
/// more reserve per covered face.
fn contest(cover: &mut Cover, c: &WorldConfig, f: Face, thr: f64, ledger: &mut FloraLedger) {
    let Some(slot) = cover.slot_of(f) else {
        return;
    };
    let cf = cover.faces[slot];
    if cf.leafiness >= thr || cf.rooted {
        return;
    }
    let Some(owner_vi) = vine_at(&cover.vines, cf.owner) else {
        return;
    };
    let mut best = cover.score(owner_vi);
    let mut winner: Option<(usize, Face, u32)> = None;
    let mut nbuf = std::mem::take(&mut cover.rt.nbuf);
    neighbours(c, f, &mut nbuf);
    for &n in &nbuf {
        let Some(ns) = cover.slot_of(n) else {
            continue;
        };
        let o = cover.faces[ns].owner;
        if o == cf.owner {
            continue;
        }
        let Some(vi) = vine_at(&cover.vines, o) else {
            continue;
        };
        let score = cover.score(vi);
        let d = cover.rt.extra[ns].dist;
        // Strictly ahead: the owner keeps a tie, and the first challenger keeps one too.
        if score > best {
            best = score;
            winner = Some((vi, n, d));
        }
    }
    cover.rt.nbuf = nbuf;
    let Some((vi, via, d)) = winner else {
        return;
    };
    let to = cover.vines[vi].id;
    cover.faces[slot].owner = to;
    cover.rt.aux[owner_vi].faces -= 1;
    cover.rt.aux[vi].faces += 1;
    let x = &mut cover.rt.extra[slot];
    x.dist = d.saturating_add(1);
    x.parent = Some(via);
    cover.mark_dirty(cf.owner);
    cover.mark_dirty(to);
    ledger.vine_faces_contested += 1;
}

/// A starving vine's outermost face — greatest path distance from its root, a face cut off
/// from the root first, the root face last — goes: its tissue to the litter below it, the
/// face to bare rock.
fn drop_outermost(
    cover: &mut Cover,
    c: &WorldConfig,
    view: &VoxelView<'_>,
    id: VineId,
    vc: &VineConfig,
    litter: &mut Vec<(Site, f64)>,
    ledger: &mut FloraLedger,
) {
    let Some(vi) = vine_at(&cover.vines, id) else {
        return;
    };
    let slots = cover.slots_of(id);
    for &s in &slots {
        cover.rt.extra[s].dist = FAR;
        cover.rt.extra[s].parent = None;
    }
    let mut walk = Vec::new();
    cover.walk_vine(c, vi, &mut walk);
    let Some(&s) = slots.iter().max_by_key(|&&s| {
        let cf = &cover.faces[s];
        (!cf.rooted, cover.rt.extra[s].dist, cf.face)
    }) else {
        return;
    };
    let root = cover.vines[vi].root;
    let site = litter_site(view, cover.faces[s].face, root);
    let cf = cover.remove_slot(s);
    litter.push((site, cf.organic(vc)));
    ledger.vine_faces_lost += 1;
}

/// A vine's spread check: every eligible unowned face adjacent to its cover that the
/// reserve can pay for, nearest the root first; then at most one sister.
fn spread(
    cover: &mut Cover,
    view: &VoxelView<'_>,
    id: VineId,
    vc: &VineConfig,
    tick: u64,
    ledger: &mut FloraLedger,
) {
    let c = view.config;
    let e = vc.energy_density;
    let Some(vi) = vine_at(&cover.vines, id) else {
        return;
    };
    let new_cost = vc.spread_cost + vc.contest_threshold * vc.leaf_mass;
    let can_spread = cover.vines[vi].reserve >= new_cost;
    let can_sister = cover.vines[vi].reserve >= vc.sister_cost;
    if !can_spread && !can_sister {
        return;
    }
    let mut walk = Vec::new();
    cover.walk_vine(c, vi, &mut walk);
    if walk.is_empty() {
        return;
    }

    if can_spread {
        let mut nbuf = std::mem::take(&mut cover.rt.nbuf);
        let mut seen: FxHashSet<Face> = FxHashSet::default();
        let mut targets: Vec<(Face, u32, Face)> = Vec::new();
        for &(s, d, _) in &walk {
            let here = cover.faces[s as usize].face;
            neighbours(c, here, &mut nbuf);
            for &n in &nbuf {
                if !cover.rt.index.contains_key(&n) && seen.insert(n) && face_eligible(view, n) {
                    targets.push((n, d, here));
                }
            }
        }
        cover.rt.nbuf = nbuf;
        let root_face = cover.vines[vi].root_face;
        for (n, d, from) in targets {
            if cover.vines[vi].reserve < new_cost {
                break;
            }
            cover.vines[vi].reserve -= new_cost;
            book_respired(ledger, e, vc.spread_cost - vc.runner_mass);
            cover.add_face(
                CoveredFace {
                    face: n,
                    owner: id,
                    leafiness: vc.contest_threshold,
                    rooted: n == root_face,
                    spur: SpurPhase::Bare,
                    spur_timer_s: 0.0,
                    nectar: 0.0,
                    fruit: 0.0,
                },
                d + 1,
                Some(from),
            );
            ledger.vine_faces_spread += 1;
        }
    }

    // A sister, where the cover reaches free soil far enough from the root.
    if cover.vines[vi].reserve < vc.sister_cost {
        return;
    }
    let own_root = cover.vines[vi].root;
    let mut found: Option<(Site, usize)> = None;
    'members: for &(s, d, _) in &walk {
        if d < vc.sister_min_dist {
            continue;
        }
        let f = cover.faces[s as usize].face;
        for site in root_sites(c, f).into_iter().flatten() {
            if site != own_root
                && !cover.rt.roots.contains_key(&site)
                && view.material_at(site.x as i64, site.y, site.z) == Material::Soil
                && view.is_support(site.x as i64, site.y, site.z)
            {
                found = Some((site, s as usize));
                break 'members;
            }
        }
    }
    let Some((site, root_slot)) = found else {
        return;
    };
    root_sister(cover, c, vi, site, root_slot, vc, tick, ledger);
}

/// Root a sister of vine `vi` on `site`, its root arch on the face in `root_slot`. It takes
/// the faces strictly nearer its root than the parent's along the parent's cover.
#[allow(clippy::too_many_arguments)]
fn root_sister(
    cover: &mut Cover,
    c: &WorldConfig,
    vi: usize,
    site: Site,
    root_slot: usize,
    vc: &VineConfig,
    tick: u64,
    ledger: &mut FloraLedger,
) {
    let parent = cover.vines[vi];
    let id = cover.next_vine;
    cover.next_vine += 1;
    cover.vines[vi].reserve -= vc.sister_cost;
    let root_face = cover.faces[root_slot].face;
    cover.add_vine(Vine {
        id,
        lineage: parent.lineage,
        root: site,
        root_face,
        water: 0.0,
        drank_m3: 0.0,
        reserve: vc.sister_cost,
        dormant: false,
        dry_s: 0.0,
        wet_s: 0.0,
        born_tick: tick,
    });
    let sister_vi = cover.vines.len() - 1;
    // The parent's distances are fresh from this check's walk.
    let mut walk = Vec::new();
    cover.walk(c, root_slot, parent.id, &mut walk);
    for (s, d, p) in walk {
        let s = s as usize;
        if d < cover.rt.extra[s].dist {
            cover.faces[s].owner = id;
            cover.rt.aux[vi].faces -= 1;
            cover.rt.aux[sister_vi].faces += 1;
            let x = &mut cover.rt.extra[s];
            x.dist = d;
            x.parent = p;
        }
    }
    cover.faces[root_slot].rooted = true;
    cover.mark_dirty(parent.id);
    cover.mark_dirty(id);
    ledger.vine_sisters += 1;
}

// ------------------------------------------------------------------ the public verbs

impl Flora {
    /// Root a founder vine on `seed.root` covering `seed.first` at leafiness 1, rooted,
    /// with `seed.reserve`; its organic matter booked as `seeded_*_in`. Refused (`None`)
    /// when the root is not a support face or already holds a vine root, or `first` is not
    /// eligible, is owned, or is not next to the root (the module doc's root relation). A
    /// stand on the same site is no obstacle.
    pub fn seed_vine(&mut self, world: &World, seed: VineSeed) -> Option<VineId> {
        let view = world.view();
        let c = view.config;
        let cover = &mut self.cover;
        cover.ensure_built();
        let vc = &self.config.latticevine;
        let root = seed.root;
        if root.x >= c.width
            || !view.is_support(root.x as i64, root.y, root.z)
            || cover.rt.roots.contains_key(&root)
            || !face_eligible(&view, seed.first)
            || cover.rt.index.contains_key(&seed.first)
            || !root_adjacent(c, root, seed.first)
            || !(seed.reserve.is_finite() && seed.reserve >= 0.0)
        {
            return None;
        }
        let id = cover.next_vine;
        cover.next_vine += 1;
        let lineage = match seed.lineage {
            Some(l) => {
                cover.next_lineage = cover.next_lineage.max(l + 1);
                l
            }
            None => {
                let l = cover.next_lineage;
                cover.next_lineage += 1;
                l
            }
        };
        cover.add_vine(Vine {
            id,
            lineage,
            root,
            root_face: seed.first,
            water: 0.0,
            drank_m3: 0.0,
            reserve: seed.reserve,
            dormant: false,
            dry_s: 0.0,
            wet_s: 0.0,
            born_tick: self.tick,
        });
        let cf = CoveredFace {
            face: seed.first,
            owner: id,
            leafiness: 1.0,
            rooted: true,
            spur: SpurPhase::Bare,
            spur_timer_s: 0.0,
            nectar: 0.0,
            fruit: 0.0,
        };
        let organic = seed.reserve + cf.organic(vc);
        cover.add_face(cf, 0, None);
        self.ledger.seeded_organic_in += organic;
        self.ledger.seeded_energy_in += vc.energy_density * organic;
        self.ledger.vine_births += 1;
        Some(id)
    }

    /// Hand-built cover for founders and fixtures: extend `vine` onto `face` at
    /// `leafiness`, its runner and leaf booked as `seeded_*_in`. Refused when `face` is not
    /// eligible, is owned, or is not cover-adjacent to a face `vine` already owns.
    pub fn cover_face(&mut self, world: &World, vine: VineId, face: Face, leafiness: f64) -> bool {
        let view = world.view();
        let c = view.config;
        let cover = &mut self.cover;
        cover.ensure_built();
        let vc = &self.config.latticevine;
        let Some(vi) = vine_at(&cover.vines, vine) else {
            return false;
        };
        if !leafiness.is_finite()
            || !face_eligible(&view, face)
            || cover.rt.index.contains_key(&face)
        {
            return false;
        }
        let mut nbuf = Vec::new();
        neighbours(c, face, &mut nbuf);
        let mut via: Option<(u32, Face)> = None;
        for &n in &nbuf {
            if let Some(s) = cover.slot_of(n)
                && cover.faces[s].owner == vine
            {
                let d = cover.rt.extra[s].dist;
                if via.is_none_or(|(vd, _)| d < vd) {
                    via = Some((d, n));
                }
            }
        }
        let Some((d, from)) = via else {
            return false;
        };
        let cf = CoveredFace {
            face,
            owner: vine,
            leafiness: leafiness.clamp(0.0, 1.0),
            rooted: face == cover.vines[vi].root_face,
            spur: SpurPhase::Bare,
            spur_timer_s: 0.0,
            nectar: 0.0,
            fruit: 0.0,
        };
        let organic = cf.organic(vc);
        cover.add_face(cf, d.saturating_add(1), Some(from));
        self.ledger.seeded_organic_in += organic;
        self.ledger.seeded_energy_in += vc.energy_density * organic;
        true
    }

    /// A grazer's bite: thin `face`'s leafiness by up to `want` organic units of leaf and
    /// return what was taken, booked as `consumed_*_out`. The runner stays and the face
    /// stays owned. `None` when the face is not covered or holds no leaf.
    pub fn crop(&mut self, face: Face, want: f64) -> Option<Taken> {
        if !(want > 0.0) || !want.is_finite() {
            return None;
        }
        let vc = &self.config.latticevine;
        let cover = &mut self.cover;
        cover.ensure_built();
        let s = cover.slot_of(face)?;
        let f = &mut cover.faces[s];
        let leaf = f.leafiness * vc.leaf_mass;
        if !(leaf > 0.0) {
            return None;
        }
        let before = f.leafiness;
        let took = if want >= leaf {
            f.leafiness = 0.0;
            leaf
        } else {
            f.leafiness = (leaf - want) / vc.leaf_mass;
            want
        };
        if before >= vc.contest_threshold && f.leafiness < vc.contest_threshold && !f.rooted {
            cover.contest_queue.push(face);
        }
        let taken = Taken {
            organic: took,
            mineral: 0.0,
            energy: vc.energy_density * took,
        };
        Some(self.book_vine_consumed(taken))
    }

    /// Drink up to `want` of `face`'s nectar. `None` unless the face is in flower with
    /// nectar left. Booked as `consumed_*_out`.
    pub fn take_nectar(&mut self, face: Face, want: f64) -> Option<Taken> {
        self.take_spur(face, want, SpurPhase::Flower)
    }

    /// Eat up to `want` of `face`'s beads. `None` unless the face is in fruit with beads
    /// left. Booked as `consumed_*_out`.
    pub fn take_fruit(&mut self, face: Face, want: f64) -> Option<Taken> {
        self.take_spur(face, want, SpurPhase::Fruit)
    }

    fn take_spur(&mut self, face: Face, want: f64, phase: SpurPhase) -> Option<Taken> {
        if !(want > 0.0) || !want.is_finite() {
            return None;
        }
        let e = self.config.latticevine.energy_density;
        let cover = &mut self.cover;
        cover.ensure_built();
        let s = cover.slot_of(face)?;
        let f = &mut cover.faces[s];
        if f.spur != phase {
            return None;
        }
        let stock = match phase {
            SpurPhase::Flower => &mut f.nectar,
            _ => &mut f.fruit,
        };
        if !(*stock > 0.0) {
            return None;
        }
        let took = if want >= *stock {
            std::mem::replace(stock, 0.0)
        } else {
            *stock -= want;
            want
        };
        let taken = Taken {
            organic: took,
            mineral: 0.0,
            energy: e * took,
        };
        Some(self.book_vine_consumed(taken))
    }

    fn book_vine_consumed(&mut self, taken: Taken) -> Taken {
        self.ledger.consumed_organic_out += taken.organic;
        self.ledger.consumed_mineral_out += taken.mineral;
        self.ledger.consumed_energy_out += taken.energy;
        taken
    }

    /// Root [`VineConfig::founders`] founder vines on a fresh world, each with
    /// [`VineConfig::founder_reserve`]: on soil support faces at the **foot** of a wall at
    /// least three faces tall, or on the **lip** of a soil ledge with at least two faces of
    /// drop below it, drawn by the world seed and kept [`FOUNDER_SPACING`] voxels apart.
    /// Returns how many rooted.
    pub fn seed_vine_founders(&mut self, world: &World) -> usize {
        let want = self.config.latticevine.founders as usize;
        if want == 0 {
            return 0;
        }
        let view = world.view();
        let c = view.config;
        let ok = |f: Face| face_eligible(&view, f);
        let mut candidates: Vec<(Site, Face)> = Vec::new();
        for z in 0..c.depth {
            for x in 0..c.width {
                for y in view.supports_in_column(x as i64, z) {
                    if view.material_at(x as i64, y, z) != Material::Soil {
                        continue;
                    }
                    let s = Site { x, y, z };
                    for d in FaceDir::SIDES {
                        let o = d.offset();
                        // Foot: the wall voxel beside the air over `s`, looking back at it.
                        if let Some((wx, wy, wz)) = step_voxel(c, x, y + 1, z, (-o.0, -o.1, -o.2))
                            && ok(Face::new(wx, wy, wz, d))
                            && wy + 2 < c.height
                            && ok(Face::new(wx, wy + 1, wz, d))
                            && ok(Face::new(wx, wy + 2, wz, d))
                        {
                            candidates.push((s, Face::new(wx, wy, wz, d)));
                        }
                        // Lip: the ledge's own side, with a drop below it.
                        if y >= 1 && ok(Face::new(x, y, z, d)) && ok(Face::new(x, y - 1, z, d)) {
                            candidates.push((s, Face::new(x, y, z, d)));
                        }
                    }
                }
            }
        }
        if candidates.is_empty() {
            return 0;
        }
        // A keyed shuffle, then the first that keep their distance.
        let mut rng = Rng::keyed(DOMAIN_VINE_FOUNDERS, c.seed, candidates.len() as u64, 0);
        for i in (1..candidates.len()).rev() {
            let j = rng.below(i + 1);
            candidates.swap(i, j);
        }
        let width = c.width as i64;
        let apart = |a: Site, b: Site| {
            let dx = (a.x as i64 - b.x as i64).rem_euclid(width);
            let dx = dx.min(width - dx);
            let dy = a.y as i64 - b.y as i64;
            let dz = a.z as i64 - b.z as i64;
            dx * dx + dy * dy + dz * dz >= FOUNDER_SPACING * FOUNDER_SPACING
        };
        let reserve = self.config.latticevine.founder_reserve;
        let mut rooted: Vec<Site> = Vec::new();
        for (site, first) in candidates {
            if rooted.len() >= want {
                break;
            }
            if !rooted.iter().all(|&r| apart(r, site)) {
                continue;
            }
            if self
                .seed_vine(
                    world,
                    VineSeed {
                        root: site,
                        first,
                        reserve,
                        lineage: None,
                    },
                )
                .is_some()
            {
                rooted.push(site);
            }
        }
        rooted.len()
    }
}
