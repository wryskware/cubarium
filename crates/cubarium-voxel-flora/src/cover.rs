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
//!   the world's [`cubarium_voxel::Config::standing_support_fill`], so a shower's film
//!   running past a wall foot does not strip the root face while a pool rising over it
//!   does ([`face_eligible`]).
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
//!      corner of two walls);
//!   4. *stair*: two side faces with the same direction, one voxel up and one voxel back,
//!      so the upper one's front voxel sits on the lower one's voxel — a runner crossing a
//!      one-voxel tread up a stepped slope ([`stair_up`]). A deeper tread is not crossed.
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
//! that, and its mineral is its own store ([`Vine::mineral`], drawn from its root's soluble
//! pool, the minimal gut's repair 3). Income is `fixed_in`, upkeep and construction are
//! `respired_out`, leaf fall, spent fruit, dropped runners and a dead vine's reserve are
//! litter on the ground below, grazing and feeding are `consumed_*_out`, founders and
//! hand-built cover are `seeded_*_in`, and terrain edits are `removed_*_out`. Regrowth,
//! spreading onto a face, nectar and fruit set, and a sister's purse are transfers inside
//! the vines.

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use cubarium_voxel::{Config as WorldConfig, Material, VoxelView, World};

use cubarium_voxel::organism::{Budget, build_from};

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
/// is inside the world, air, and not flooded (free water under the world's
/// [`cubarium_voxel::Config::standing_support_fill`]).
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
    view.material[i] == Material::Air && view.free[i] < c.standing_support_fill
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

/// **An open wall's lit area**: the eligible faces joined to `start` in its own plane
/// (the same direction; up, down and sideways along a wall, [`FaceDir::plane_steps`]),
/// and the sum of their [`face_light`], read as the step reads it (the sky of the front
/// voxel's floor, through `sky`, a site-sorted cache). The founders' rank. Light is read
/// on at most 64 faces spread through the wall and scaled up to its size.
pub(crate) fn wall_lit_area(
    view: &VoxelView<'_>,
    vc: &VineConfig,
    start: Face,
    sky: &mut Vec<(Site, f64)>,
) -> (Vec<Face>, f64) {
    let c = view.config;
    let mut faces = vec![start];
    let mut seen: FxHashSet<Face> = FxHashSet::default();
    seen.insert(start);
    let mut i = 0;
    while i < faces.len() {
        let f = faces[i];
        i += 1;
        for s in f.dir.plane_steps() {
            if let Some((x, y, z)) = step_voxel(c, f.x, f.y, f.z, s) {
                let n = Face::new(x, y, z, f.dir);
                if !seen.contains(&n) && face_eligible(view, n) {
                    seen.insert(n);
                    faces.push(n);
                }
            }
        }
    }
    const SAMPLES: usize = 64;
    let stride = faces.len().div_ceil(SAMPLES).max(1);
    let (mut sum, mut n) = (0.0, 0usize);
    for f in faces.iter().step_by(stride) {
        let sv = front_in(c, *f)
            .and_then(|(x, y, z)| (y >= 1).then_some(Site { x, y: y - 1, z }))
            .map_or(0.0, |k| step::sky_at(sky, view, k));
        sum += face_light(vc, f.dir, sv);
        n += 1;
    }
    let area = if n > 0 { sum / n as f64 * faces.len() as f64 } else { 0.0 };
    (faces, area)
}

/// The side face one step up a staircase from `f`, the same direction: the riser whose
/// front voxel sits on top of `f`'s own voxel, across a one-voxel tread. `None` for an
/// underside or off the world. Not screened.
pub(crate) fn stair_up(c: &WorldConfig, f: Face) -> Option<Face> {
    if f.dir.is_underside() {
        return None;
    }
    let o = f.dir.offset();
    step_voxel(c, f.x, f.y, f.z, (-o.0, 1, -o.2)).map(|(x, y, z)| Face::new(x, y, z, f.dir))
}

/// The side face one step down a staircase from `f`: [`stair_up`] the other way.
pub(crate) fn stair_down(c: &WorldConfig, f: Face) -> Option<Face> {
    if f.dir.is_underside() {
        return None;
    }
    let o = f.dir.offset();
    step_voxel(c, f.x, f.y, f.z, (o.0, -1, o.2)).map(|(x, y, z)| Face::new(x, y, z, f.dir))
}

/// Every face cover-adjacent to `f` (planar, convex edge, concave corner, stair), in a
/// fixed order, into `out`. Not screened for eligibility.
pub(crate) fn neighbours(c: &WorldConfig, f: Face, out: &mut Vec<Face>) {
    out.clear();
    for s in f.dir.plane_steps() {
        if let Some((x, y, z)) = step_voxel(c, f.x, f.y, f.z, s) {
            out.push(Face::new(x, y, z, f.dir));
        }
    }
    out.extend(stair_up(c, f));
    out.extend(stair_down(c, f));
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
    /// The root box's mean available water last tick (the available-water scale: 0 at the wilting
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
    /// **Mineral in the vine's tissue** (the minimal gut's repair 3 item 2): drawn from
    /// the soluble pool of its root's site up to [`VineConfig::mineral_fraction`] of its
    /// tissue, and carried out pro rata by its leaf, nectar and fruit when they are eaten.
    /// A founder arrives with none.
    #[serde(default)]
    pub mineral: f64,
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
    /// Seconds left in the current spur phase, or, bare, of its random rest before the next
    /// start (at most 0 until one is drawn: below 0, the spent phase's overshoot, which
    /// the rest drawn next is shortened by).
    pub spur_timer_s: f64,
    /// Organic matter in the flower. Positive only in [`SpurPhase::Flower`].
    pub nectar: f64,
    /// Organic matter in the bead bunch. Positive only in [`SpurPhase::Fruit`].
    pub fruit: f64,
    /// Pollinator visits to this face's flower (FLOWER-FRUIT, [`Flora::visit_face`]),
    /// counted while it is in flower and cleared when the next bud opens.
    #[serde(default)]
    pub visits: u16,
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
    /// Wall-face geometry by x column, shared by immutable queries. Invalidated only
    /// when a face is added or removed; ownership and stocks do not change geometry.
    wall_columns: std::sync::OnceLock<FxHashMap<u32, Vec<Face>>>,
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

    /// **The flowers** (the beacon export, S3 of the senses pass): every covered face in
    /// flower holding nectar, in no particular order — what a flyer's light sense and its
    /// floral smell read, without the copy and sort of [`Cover::faces`].
    pub fn flowers(&self) -> impl Iterator<Item = &CoveredFace> + '_ {
        self.faces.iter().filter(|c| c.spur == SpurPhase::Flower && c.nectar > 0.0)
    }

    /// How many faces are covered.
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Every covered face, in storage order (no copy, no sort).
    pub fn iter(&self) -> impl Iterator<Item = &CoveredFace> + '_ {
        self.faces.iter()
    }

    /// Covered wall faces in the x columns accepted by `keep`, in no particular order.
    /// The geometry index is transient and built lazily, including after a load.
    /// Queries still check their current distance, height and any nesting conditions.
    pub fn wall_faces_in_columns<'a>(
        &'a self,
        keep: impl Fn(u32) -> bool + 'a,
    ) -> impl Iterator<Item = Face> + 'a {
        let columns = self.rt.wall_columns.get_or_init(|| {
            let mut columns: FxHashMap<u32, Vec<Face>> = FxHashMap::default();
            for cf in &self.faces {
                if !cf.face.dir.is_underside() {
                    columns.entry(cf.face.x).or_default().push(cf.face);
                }
            }
            columns
        });
        columns
            .iter()
            .filter(move |(x, _)| keep(**x))
            .flat_map(|(_, faces)| faces.iter().copied())
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

    /// [`Cover::draw`] without the allocation or the sort: every covered face's
    /// [`FaceDraw`], in no particular order. A read-only view for a presenter that only
    /// needs to know whether what it draws moved.
    pub fn face_draws(&self) -> impl Iterator<Item = FaceDraw> + '_ {
        self.faces.iter().map(|c| FaceDraw {
            face: c.face,
            owner: c.owner,
            leafiness: c.leafiness,
            rooted: c.rooted,
            dormant: self.vine(c.owner).is_some_and(|v| v.dormant),
            spur: c.spur,
        })
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
        self.rt.wall_columns.take();
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
        self.rt.wall_columns.take();
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
    /// Income moisture ramp on the available-water scale: 0 at `wilt_water`,
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
    /// The flowering share of the reserve: spreading and sisters spend only what is above
    /// `spread_hold · spur_reserve_min` per covered face, so a growing vine still has the
    /// reserve to keep flowering. Leaf regrowth is not held back. 0 spends it all
    /// on spreading (the vine never flowers while it grows). At 2 the hold is about a
    /// minute of a lit face's income at the 3× pace, so the spread barely slows.
    pub spread_hold: f64,
    /// **The share of covered faces carrying a flower or a bead bunch at once**, over the
    /// long run (plants-flowering-vines 2026-10-03). Each face cycles on its own: a rest
    /// drawn at random (exponential, keyed by the face and the tick), then bud, flower,
    /// fruit and spent. The rest's mean is what makes this share:
    /// `(flower_s + fruit_s) / spur_share` is a face's mean cycle ([`VineConfig::spur_cycle_s`]),
    /// never shorter than the four phases back to back (1 or more: no rest at all). 0
    /// never flowers.
    pub spur_share: f64,
    /// Phase lengths, seconds.
    pub bud_s: f64,
    pub flower_s: f64,
    pub fruit_s: f64,
    pub spent_s: f64,
    /// **Nectar a covered face offers per second over the long run**, organic units per
    /// reference face: production per unit of vine area. A flower opens holding this rate
    /// times the mean cycle, so the long-run supply of a colony of `N` faces is
    /// `N · nectar_per_face_s` whatever [`VineConfig::spur_share`] is.
    pub nectar_per_face_s: f64,
    /// **Bead fruit a covered face sets per second over the long run**, organic units per
    /// reference face, the same way: a bunch sets holding this rate times the mean cycle —
    /// when its flower was pollinated; an unvisited flower sets `pollination_floor` of it.
    pub fruit_per_face_s: f64,
    /// **What a unit of bead fruit costs the vine**, units of reserve (FLOWER-FRUIT, Wrysk
    /// 2026-10-09, about 2–3): the unit beyond the fruit is respired, its energy as heat.
    /// 1: fruit at par.
    pub fruit_cost: f64,
    /// **The pollination floor**: the share of a full bunch a flower no pollinator visited
    /// sets ([`Flora::visit_face`]). 1: visits change nothing.
    pub pollination_floor: f64,
    /// **The visit-to-fruit rule**: visits to a flower that set a full bunch; `v` visits set
    /// `floor + (1 − floor) · min(1, v / pollination_visits)` of it.
    pub pollination_visits: f64,
    /// Energy per unit of organic matter in the vine's tissue.
    pub energy_density: f64,
    /// **Mineral per unit of the vine's tissue** (the minimal gut's repair 3 item 2): the
    /// vine draws soluble mineral from its root's site up to this fraction of its tissue
    /// (reserve, leaf and runners), and its leaf, nectar and fruit carry this fraction of
    /// their organic matter out of that store, never more than it holds. 0.01, the
    /// bloomcrown's nectar fraction.
    #[serde(default = "default_vine_mineral_fraction")]
    pub mineral_fraction: f64,
    /// Founder vines [`Flora::seed_vine_founders`] roots on a fresh world: the **floor**
    /// under the area count `founders_per_m2` ([`VineConfig::founder_count`]).
    pub founders: u32,
    /// Founder vines per square metre of world floor (`width · depth · voxel_m²`); the
    /// count is `max(founders, round(founders_per_m2 · area))`. 0.0625 gives the 192 m²
    /// desktop terrarium 12.
    pub founders_per_m2: f64,
    /// Each founder's starting reserve.
    pub founder_reserve: f64,
    /// The fewest voxels between two founders' roots (straight-line distance, the `x`
    /// axis wrapped). A count of voxels, **not** rescaled with the voxel size.
    pub founder_spacing: u32,
    /// Metres of floor (the horizontal distance, `x` wrapped, `z`; height ignored) the
    /// founders keep between their roots while the walls allow it, so the tallest walls
    /// of **different** structures get one each rather than one tall structure taking
    /// them all. When too few spread-apart walls remain, the rest root at
    /// `founder_spacing`, so the count holds. 0 turns it off. 3 m gives the 32 m-wide
    /// desktop terrarium room for about ten.
    pub founder_spread_m: f64,
    /// **A banked vine seed's dormancy** (roster mechanics fix 3), seconds: latticevine
    /// seed voided from a gut ([`crate::Flora::deposit_vine_seeds`]) older than this at
    /// its site's bank check falls to the litter. A stand seed's `seed_max_age_s`; 8 h.
    pub seed_max_age_s: f64,
}

impl Default for VineConfig {
    /// The reference defaults, on the 0.25 m voxel (a face of 0.0625 m²). Placeholders
    /// (`design/backlog.md` §1), chosen for the dossier's pace — "slow to cover, like
    /// stonecushion" — with the pace set by what a face earns, not by a timer:
    ///
    /// - **Earning back a face.** A new face costs `spread_cost` 0.05 (its runner 0.02, the
    ///   rest construction) plus its leaf, 0.05 at full leaf: 0.1 in all. A lit side face
    ///   (sky about 0.7) nets about `0.7 · 2.7e-4 − 1e-5 − 2e-6 ≈ 1.8e-4` a second at full
    ///   leaf, so it pays for a new face in about ten minutes, and a colony that
    ///   reinvests everything (a new face also regrows from 0.2 leaf at `regrow_rate`
    ///   0.002/s, about seven minutes to full) doubles in roughly ten to fifteen minutes:
    ///   about three times the first pace (9e-5, a doubling in half an hour), raised
    ///   2026-10-01 because a live world's vines could not be seen
    ///   (`design/handoffs/plants-glowcap-vine-2026-10-01.md` A3). Wrysk 2026-10-02 kept
    ///   3× once the vine also holds back its flowering share (`spread_hold`). One
    ///   founder covers a
    ///   lit, watered wall of a few hundred faces in a few hours (`tests/latticevine.rs`
    ///   t16–t17).
    /// - **Water per leaf area like the ground plants.** Per square metre of leaf,
    ///   springturf transpires about `1e-6 · 0.12 / 0.196 ≈ 6e-7` m³/s at full size and
    ///   stonecushion about `1e-6 · 0.2 / 0.077 ≈ 2.6e-6`; the vine's 6e-8 m³/s per full
    ///   face is `9.6e-7` per square metre, between the two. A colony of a hundred faces
    ///   drinks 6e-6 m³/s, about half a litre an hour per ten faces, from one root box:
    ///   a big cover on a dry foot runs its root out of water and goes dormant.
    fn default() -> VineConfig {
        VineConfig {
            rooting_depth: 2,
            rooting_radius: 1,
            transpiration_m3_per_s: 6e-8,
            wilt_water: 0.1,
            full_water: 0.5,
            dry_water: 0.15,
            wet_water: 0.4,
            dry_spell_s: 600.0,
            wet_spell_s: 300.0,
            leaf_fall_s: 60.0,
            assimilation: 2.7e-4,
            underside_light: 0.15,
            leaf_mass: 0.05,
            runner_mass: 0.02,
            leaf_upkeep: 1e-5,
            wood_upkeep: 2e-6,
            regrow_rate: 0.002,
            spread_cost: 0.05,
            spread_check_s: 5.0,
            sister_cost: 0.1,
            sister_min_dist: 4,
            contest_threshold: 0.2,
            contest_check_s: 30.0,
            spur_reserve_min: 0.005,
            spread_hold: 2.0,
            // A tenth of a colony's faces in flower or fruit at once (a third of them in
            // flower): with the phases below a face's mean cycle is an hour, so a face
            // flowers about twice a world day (7,200 s), each time at a random moment.
            spur_share: 0.1,
            bud_s: 60.0,
            flower_s: 120.0,
            fruit_s: 240.0,
            spent_s: 60.0,
            // Per reference face per second; over the hour's cycle a flower opens with
            // 0.002 and a bunch sets with 0.005, the per-spur amounts the shower-timed
            // cycle had. Per world day (7,200 s) a reference face offers 0.004 nectar and
            // 0.01 fruit; a 0.125 m face a quarter of that.
            nectar_per_face_s: 0.002 / 3600.0,
            fruit_per_face_s: 0.005 / 3600.0,
            // FLOWER-FRUIT (Wrysk 2026-10-09), plausible starts, untuned: the lanternberry's
            // cost and floor; a vine flower is one flower, so one visit pollinates it.
            fruit_cost: 2.5,
            pollination_floor: 0.1,
            pollination_visits: 1.0,
            energy_density: 1.0,
            mineral_fraction: 0.01,
            founders: 6,
            founders_per_m2: 0.0625,
            founder_reserve: 0.1,
            founder_spacing: 8,
            founder_spread_m: 3.0,
            seed_max_age_s: 28_800.0,
        }
    }
}

impl VineConfig {
    /// **A face's mean spur cycle**, seconds: `(flower_s + fruit_s) / spur_share`, never
    /// shorter than the four phases back to back. Infinite when `spur_share` is not
    /// positive (no face ever flowers).
    pub fn spur_cycle_s(&self) -> f64 {
        let phases = self.bud_s + self.flower_s + self.fruit_s + self.spent_s;
        if !(self.spur_share > 0.0) {
            return f64::INFINITY;
        }
        ((self.flower_s + self.fruit_s) / self.spur_share).max(phases)
    }

    /// The mean rest between one face's cycles, seconds: the mean cycle less its phases.
    pub fn spur_rest_s(&self) -> f64 {
        let phases = self.bud_s + self.flower_s + self.fruit_s + self.spent_s;
        (self.spur_cycle_s() - phases).max(0.0)
    }

    /// The nectar a flower opens with: [`VineConfig::nectar_per_face_s`] times the mean
    /// cycle (0 when no face flowers).
    pub fn nectar_per_flower(&self) -> f64 {
        let t = self.spur_cycle_s();
        if t.is_finite() { (self.nectar_per_face_s * t).max(0.0) } else { 0.0 }
    }

    /// The fruit a bead bunch sets with: [`VineConfig::fruit_per_face_s`] times the mean
    /// cycle (0 when no face flowers).
    pub fn fruit_per_bunch(&self) -> f64 {
        let t = self.spur_cycle_s();
        if t.is_finite() { (self.fruit_per_face_s * t).max(0.0) } else { 0.0 }
    }

    /// How many founders a world of `area_m2` of floor gets:
    /// `max(founders, round(founders_per_m2 · area_m2))`.
    pub fn founder_count(&self, area_m2: f64) -> usize {
        let by_area = (self.founders_per_m2 * area_m2).round();
        let by_area = if by_area.is_finite() && by_area > 0.0 { by_area as usize } else { 0 };
        by_area.max(self.founders as usize)
    }
}


/// The site a face's litter lands on: the first solid under its front voxel, which is a
/// support face by construction; `fallback` when there is none.
fn default_vine_mineral_fraction() -> f64 {
    VineConfig::default().mineral_fraction
}

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
        ledger.removed_mineral_out += v.mineral;
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
            && view.free[x.front as usize] < view.config.standing_support_fill;
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
    dt: f64,
) {
    wants.clear();
    let Flora { config, cover, .. } = flora;
    let vc = &config.latticevine;
    for vi in 0..cover.vines.len() {
        // A stand's root box and read. The vine has no aeration rule, so no cell counts
        // as saturated.
        let (water, band, _) = step::read_root_box(
            view,
            cover.vines[vi].root,
            vc.rooting_depth,
            vc.rooting_radius,
            f64::INFINITY,
            box_,
            drinkable,
        );
        let avail = if band > 0.0 { water / band } else { 0.0 };
        let mu = step::ramp(avail, vc.wilt_water, vc.full_water);
        let a = &mut cover.rt.aux[vi];
        let demand = (vc.transpiration_m3_per_s * a.leaf * mu * dt).max(0.0);
        a.mu = mu;
        a.demand = demand;
        a.drank = 0.0;
        cover.vines[vi].water = avail;
        step::share_over_box(demand, box_, drinkable, |v, share| {
            wants.push((v as u32, vi as u32, share));
        });
    }
}

/// The drink's credit: a vine's share of what each of its cells gave up.
pub(crate) fn drink_credit(flora: &mut Flora, vi: usize, got: f64) {
    flora.cover.rt.aux[vi].drank += got;
}

/// Every vine's id, in vine order: who a planned drink's vine indices name.
pub(crate) fn vine_ids(flora: &Flora) -> impl Iterator<Item = VineId> + '_ {
    flora.cover.vines.iter().map(|v| v.id)
}

/// A planned drink's shortfall off vine `id`'s `drank_m3`, if the vine still stands (the
/// overlapped tick's settle; `step.rs`). Vines are sorted by id.
pub(crate) fn drink_debit(flora: &mut Flora, id: VineId, debit: f64) {
    let vines = &mut flora.cover.vines;
    if let Ok(i) = vines.binary_search_by_key(&id, |v| v.id) {
        vines[i].drank_m3 = (vines[i].drank_m3 - debit).max(0.0);
    }
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
pub(crate) fn grow(flora: &mut Flora, world: &World, climate: step::Climate, span: u64) {
    let tick = flora.tick;
    let dt = climate.dt;
    let view = world.view();
    let Flora {
        config,
        cover,
        ground,
        ledger,
        ..
    } = flora;
    cover.ensure_built();
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
            v.dry_s += dt;
        } else {
            v.dry_s = 0.0;
        }
        if v.water > vc.wet_water {
            v.wet_s += dt;
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
        // A stand's income function, over the vine's lit leaves: `light_leaf` is
        // Σ light · leafiness, and the limit is the share of its water demand met.
        let income = if v.dormant {
            0.0
        } else {
            step::photosynthesis(vc.assimilation, a.light_leaf, climate.daylight, a.mu, water, dt)
        };
        ledger.fixed_in += income;
        ledger.light_in += e * income;

        // The organism budget: upkeep from income, then reserve, and the rest of the income
        // to the reserve, which has no cap. Building (regrowth, spread, spurs) is paid out
        // of the reserve below.
        let upkeep = (vc.leaf_upkeep * a.leaf + vc.wood_upkeep * f64::from(a.faces))
            * climate.upkeep
            * climate.night
            * dt;
        let mut budget = Budget::new(income, f64::INFINITY);
        let short = budget.upkeep(&mut v.reserve, upkeep).unpaid;
        v.reserve += budget.store(f64::INFINITY);
        book_respired(ledger, e, budget.close());
        // The mineral store (the minimal gut's repair 3 item 2): toward
        // `mineral_fraction` of its tissue, drawn from (or, above it, returned to) the
        // soluble pool of its root's site. A transfer inside the layer.
        let tissue = v.reserve + a.leaf * vc.leaf_mass + f64::from(a.faces) * vc.runner_mass;
        let target = vc.mineral_fraction * tissue;
        if (v.mineral - target).abs() > 1e-18 {
            let gi = step::provisioned_slot(config, ground, v.root, ledger);
            let g = &mut ground[gi];
            if v.mineral < target {
                let draw = (target - v.mineral).min(g.mineral.max(0.0));
                g.mineral -= draw;
                v.mineral += draw;
            } else {
                let back = v.mineral - target;
                v.mineral -= back;
                g.mineral += back;
            }
        }
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
    // The spur clock (plants-flowering-vines 2026-10-03): every face on its own, a random
    // rest between cycles whose mean sets the share in flower or fruit, and the amounts
    // that make the long-run production per face the knob's.
    let rest_mean = vc.spur_rest_s();
    let flowering = vc.spur_cycle_s().is_finite();
    let (nectar_amount, fruit_amount) = (vc.nectar_per_flower(), vc.fruit_per_bunch());
    let wheel = cubarium_voxel::ticks_at_least_one(vc.contest_check_s);
    let fall = dt / vc.leaf_fall_s;
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
                // Leaf out of the reserve at par: a vine's leaf has no construction cost.
                let want = (vc.regrow_rate * dt).min(1.0 - f.leafiness) * vc.leaf_mass;
                f.leafiness += build_from(&mut v.reserve, want, 1.0) / vc.leaf_mass;
                if 1.0 - f.leafiness < 1e-12 {
                    f.leafiness = 1.0;
                }
            }

            // Spurs.
            match f.spur {
                SpurPhase::Bare => {
                    // A bare spur's timer is its rest; at or below 0 none is drawn yet,
                    // and below 0 it is the time the spent phase already ran past its
                    // end. Dormancy clears it, so a vine waking draws fresh rests face by
                    // face.
                    if v.dormant || !flowering {
                        f.spur_timer_s = 0.0;
                    } else {
                        if f.spur_timer_s <= 0.0 {
                            // Exponential, keyed by the face and the tick: memoryless, so
                            // the starts of a colony are scattered in time and place from
                            // the first, and no event lines them up. The rest runs from
                            // where the spent phase ended, so the overshoot is carried and
                            // this step is the rest's first: a cycle is its phases and its
                            // rest on any plant clock (`tests/plant_clock_rates.rs`).
                            let (ka, kb, kc) = f.face.key();
                            let mut rng =
                                Rng::keyed(DOMAIN_SPUR, ka ^ world_seed.rotate_left(17), kb ^ tick, kc);
                            let u = 1.0 - rng.unit();
                            f.spur_timer_s += (-u.ln() * rest_mean).max(0.0);
                        }
                        f.spur_timer_s -= dt;
                        if f.spur_timer_s <= EPS {
                            // Without the reserve this start is skipped and a new rest is
                            // drawn next tick: a short vine never queues its spurs up to
                            // open together once it can pay.
                            let per_face = v.reserve / f64::from(a.faces.max(1));
                            if per_face >= vc.spur_reserve_min {
                                f.spur = SpurPhase::Bud;
                                f.spur_timer_s += vc.bud_s;
                            } else {
                                f.spur_timer_s = 0.0;
                            }
                        }
                    }
                }
                SpurPhase::Bud => {
                    if v.dormant {
                        f.spur = SpurPhase::Bare;
                        f.spur_timer_s = 0.0;
                    } else {
                        f.spur_timer_s -= dt;
                        if f.spur_timer_s <= EPS {
                            let n = nectar_amount.min(v.reserve).max(0.0);
                            v.reserve -= n;
                            f.nectar = n;
                            f.visits = 0;
                            f.spur = SpurPhase::Flower;
                            f.spur_timer_s += vc.flower_s;
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
                        f.spur_timer_s -= dt;
                        if f.spur_timer_s <= EPS {
                            // What the visitors left is taken back; the beads set by
                            // the visits (FLOWER-FRUIT) and are paid at `fruit_cost`, the
                            // overhead respired.
                            v.reserve += f.nectar;
                            f.nectar = 0.0;
                            let share = crate::pollinated_share(vc.pollination_floor, vc.pollination_visits, f64::from(f.visits));
                            let cost = vc.fruit_cost.max(1.0);
                            let b = (share * fruit_amount).min(v.reserve / cost).max(0.0);
                            v.reserve -= cost * b;
                            let overhead = (cost - 1.0) * b;
                            if overhead > 0.0 {
                                book_respired(ledger, e, overhead);
                                ledger.fruit_overhead += overhead;
                            }
                            let visited = f.visits > 0;
                            ledger.crops_set[usize::from(visited)] += 1;
                            ledger.fruit_set[usize::from(visited)] += b;
                            f.fruit = b;
                            f.spur = SpurPhase::Fruit;
                            f.spur_timer_s += vc.fruit_s;
                        }
                    }
                }
                SpurPhase::Fruit => {
                    // Beads already set stay through a dormancy until eaten or spent.
                    f.spur_timer_s -= dt;
                    if f.spur_timer_s <= EPS {
                        let b = f.fruit;
                        f.fruit = 0.0;
                        if b > 0.0 {
                            drop_at(rt, s, &view, face, root, &mut litter, b);
                        }
                        f.spur = SpurPhase::Spent;
                        f.spur_timer_s += vc.spent_s;
                    }
                }
                SpurPhase::Spent => {
                    f.spur_timer_s -= dt;
                    if f.spur_timer_s <= EPS {
                        // The overshoot, kept for the rest drawn next (never above 0, so
                        // the bare spur does draw one).
                        f.spur = SpurPhase::Bare;
                        f.spur_timer_s = f.spur_timer_s.min(0.0);
                    }
                }
            }

            // A face that crossed below the threshold this tick, or a weak one whose slot on
            // the contest wheel is due.
            if f.leafiness < thr
                && !f.rooted
                && (before >= thr
                    || step::due_in_window(wheel - f.face.hash() % wheel, tick, span, wheel))
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
    let spread_period = cubarium_voxel::ticks_at_least_one(vc.spread_check_s);
    let due: Vec<VineId> = cover
        .vines
        .iter()
        .filter(|v| {
            !v.dormant && step::due_in_window(step::mix(v.id), tick, span, spread_period)
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
        // Its mineral goes back to its root's soluble pool (inside the layer).
        if v.mineral > 0.0 {
            let gi = step::provisioned_slot(config, ground, v.root, ledger);
            ground[gi].mineral += v.mineral;
        }
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
/// from the root first, the root face last — goes: the runner's wood is drawn back into
/// the reserve (the dossier's "the plant's reserve sits in the runners"), so the next
/// runner goes only once that is spent too and a drought shrinks the colony over hours
/// rather than ticks; what leaf, nectar and fruit it still held fall as litter; the face is
/// bare rock.
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
    cover.vines[vi].reserve += vc.runner_mass;
    litter.push((site, cf.organic(vc) - vc.runner_mass));
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
    // The flowering share stays: only the reserve above it spreads.
    let hold = vc.spread_hold * vc.spur_reserve_min * f64::from(cover.rt.aux[vi].faces.max(1));
    let spare = |cover: &Cover| cover.vines[vi].reserve - hold;
    let can_spread = spare(cover) >= new_cost;
    let can_sister = spare(cover) >= vc.sister_cost;
    if !can_spread && !can_sister {
        return;
    }
    let mut walk = Vec::new();
    cover.walk_vine(c, vi, &mut walk);
    if walk.is_empty() {
        return;
    }

    // A sister first, where the cover reaches free soil far enough from the root
    // (plants-flowering-vines 2026-10-03): before, the spread spent the spare reserve
    // first and a sister was looked for with what was left, so a vine with faces still
    // to take — a big wall, never fully covered — never rooted one and the whole wall
    // drank and starved through one root box. A sister takes this check; the spread
    // goes on at the next.
    if can_sister {
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
        if let Some((site, root_slot)) = found {
            root_sister(cover, c, vi, site, root_slot, vc, tick, ledger);
            return;
        }
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
            if spare(cover) < new_cost {
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
                    visits: 0,
                    nectar: 0.0,
                    fruit: 0.0,
                },
                d + 1,
                Some(from),
            );
            ledger.vine_faces_spread += 1;
        }
    }
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
        mineral: 0.0,
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

/// **One latticevine seed** (roster mechanics fix 3): the organic matter a seed needs to
/// found a vine on one face — a runner and leaf up to `contest_threshold`, what a spread
/// pays to cover a new face — the vine's counterpart of a stand seed's package
/// (`alive_min / w_frac`).
pub fn vine_seed_package(vc: &VineConfig) -> f64 {
    vc.runner_mass + vc.contest_threshold * vc.leaf_mass
}

impl Flora {
    /// **A banked vine seed comes up** (roster mechanics fix 3), at its site's bank check:
    /// with at least one whole seed ([`vine_seed_package`]) banked on `site`, a soil
    /// support face that holds no vine root, and a face beside it a vine can root on (a
    /// wall foot, then a ledge lip: the founders' candidates, the first eligible and
    /// unowned in a fixed order), the whole lot founds a new vine there — its runner, leaf
    /// up to leafiness 1 and the rest as reserve. A transfer inside the plant layer, so
    /// nothing is booked in; the lot's mineral goes to the site's pool (a vine holds
    /// none). `false` leaves the lot banked.
    pub(crate) fn germinate_vine_seed(&mut self, world: &World, site: Site) -> bool {
        let view = world.view();
        let c = view.config;
        let Ok(gi) = self.ground.binary_search_by_key(&site, |g| g.site) else {
            return false;
        };
        let Some(lot) = self.ground[gi].vine_seeds else {
            return false;
        };
        let vc = self.config.latticevine.clone();
        if lot.organic < vine_seed_package(&vc) - 1e-15
            || site.x >= c.width
            || !view.is_support(i64::from(site.x), site.y, site.z)
            || view.material_at(i64::from(site.x), site.y, site.z) != Material::Soil
        {
            return false;
        }
        self.cover.ensure_built();
        if self.cover.rt.roots.contains_key(&site) {
            return false;
        }
        let ok = |cover: &Cover, f: Face| face_eligible(&view, f) && !cover.rt.index.contains_key(&f) && root_adjacent(c, site, f);
        let mut first = None;
        for d in FaceDir::SIDES {
            let o = d.offset();
            if let Some((wx, wy, wz)) = step_voxel(c, site.x, site.y + 1, site.z, (-o.0, -o.1, -o.2)) {
                let f = Face::new(wx, wy, wz, d);
                if ok(&self.cover, f) {
                    first = Some(f);
                    break;
                }
            }
        }
        if first.is_none() {
            first = FaceDir::SIDES
                .into_iter()
                .map(|d| Face::new(site.x, site.y, site.z, d))
                .find(|&f| ok(&self.cover, f));
        }
        let Some(first) = first else {
            return false;
        };
        let leafiness = ((lot.organic - vc.runner_mass) / vc.leaf_mass).clamp(0.0, 1.0);
        let reserve = (lot.organic - vc.runner_mass - leafiness * vc.leaf_mass).max(0.0);
        let cover = &mut self.cover;
        let id = cover.next_vine;
        cover.next_vine += 1;
        let lineage = cover.next_lineage;
        cover.next_lineage += 1;
        cover.add_vine(Vine {
            id,
            lineage,
            root: site,
            root_face: first,
            water: 0.0,
            drank_m3: 0.0,
            reserve,
            dormant: false,
            dry_s: 0.0,
            wet_s: 0.0,
            born_tick: self.tick,
            mineral: 0.0,
        });
        cover.add_face(
            CoveredFace {
                face: first,
                owner: id,
                leafiness,
                rooted: true,
                spur: SpurPhase::Bare,
                spur_timer_s: 0.0,
                visits: 0,
                nectar: 0.0,
                fruit: 0.0,
            },
            0,
            None,
        );
        let g = &mut self.ground[gi];
        g.mineral += lot.mineral;
        g.vine_seeds = None;
        self.ledger.vine_births += 1;
        true
    }

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
            mineral: 0.0,
        });
        let cf = CoveredFace {
            face: seed.first,
            owner: id,
            leafiness: 1.0,
            rooted: true,
            spur: SpurPhase::Bare,
            spur_timer_s: 0.0,
            visits: 0,
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
            visits: 0,
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
        let owner = f.owner;
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
        let fraction = vc.mineral_fraction;
        let energy = vc.energy_density * took;
        let mineral = self.vine_mineral_out(owner, fraction * took);
        let taken = Taken {
            organic: took,
            mineral,
            energy,
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

    /// **A sip across a patch** (VINE-C, Wrysk 2026-10-09: option C): up to `want` of the
    /// nectar of every face in `faces` that is in flower, drawn from each in proportion to
    /// what it holds, so a mouth reaching several small faces drinks as from one face of their
    /// total. `None` when none of them holds nectar.
    pub fn take_nectar_patch(&mut self, faces: &[Face], want: f64) -> Option<Taken> {
        self.take_spur_patch(faces, want, SpurPhase::Flower)
    }

    /// **A bite across a patch** (VINE-C): up to `want` of the beads of every face in
    /// `faces` that is in fruit, drawn from each in proportion to what it holds. `None` when
    /// none of them holds fruit.
    pub fn take_fruit_patch(&mut self, faces: &[Face], want: f64) -> Option<Taken> {
        self.take_spur_patch(faces, want, SpurPhase::Fruit)
    }

    /// What `face` holds of `phase`'s product (nectar in flower, beads in fruit); 0 in any
    /// other phase or off the cover.
    pub fn spur_stock(&self, face: Face, phase: SpurPhase) -> f64 {
        self.cover.face(face).map_or(0.0, |f| match (f.spur, phase) {
            (SpurPhase::Flower, SpurPhase::Flower) => f.nectar,
            (SpurPhase::Fruit, SpurPhase::Fruit) => f.fruit,
            _ => 0.0,
        })
    }

    /// **A fixture's spur** (tests and studies, VINE-C): put covered `face` at the start of
    /// `phase` (flower or fruit) holding `stock` of its product, nectar or beads — the face's
    /// old product taken out and the new one booked as seeded (`seeded_*_in`), so the ledger
    /// still closes. `false`, changing nothing, off the cover or for another phase.
    #[doc(hidden)]
    pub fn set_spur(&mut self, face: Face, phase: SpurPhase, stock: f64) -> bool {
        if !matches!(phase, SpurPhase::Flower | SpurPhase::Fruit) || !(stock >= 0.0) || !stock.is_finite() {
            return false;
        }
        let e = self.config.latticevine.energy_density;
        let length = if phase == SpurPhase::Flower { self.config.latticevine.flower_s } else { self.config.latticevine.fruit_s };
        let cover = &mut self.cover;
        cover.ensure_built();
        let Some(s) = cover.slot_of(face) else {
            return false;
        };
        let f = &mut cover.faces[s];
        let old = f.nectar + f.fruit;
        f.spur = phase;
        // The phase's whole length to run.
        f.spur_timer_s = length;
        f.visits = 0;
        f.nectar = if phase == SpurPhase::Flower { stock } else { 0.0 };
        f.fruit = if phase == SpurPhase::Fruit { stock } else { 0.0 };
        let net = stock - old;
        self.ledger.seeded_organic_in += net;
        self.ledger.seeded_energy_in += e * net;
        true
    }

    fn take_spur_patch(&mut self, faces: &[Face], want: f64, phase: SpurPhase) -> Option<Taken> {
        if !(want > 0.0) || !want.is_finite() {
            return None;
        }
        self.cover.ensure_built();
        let mut seen: Vec<Face> = Vec::with_capacity(faces.len());
        let mut held: Vec<(Face, f64)> = Vec::with_capacity(faces.len());
        for &f in faces {
            if seen.contains(&f) {
                continue;
            }
            seen.push(f);
            let s = self.spur_stock(f, phase);
            if s > 0.0 {
                held.push((f, s));
            }
        }
        let total: f64 = held.iter().map(|h| h.1).sum();
        if !(total > 0.0) {
            return None;
        }
        let share = (want / total).min(1.0);
        let mut sum = Taken::default();
        for (f, s) in held {
            // All of it at a share of 1, so a full draw empties every face exactly.
            let ask = if share >= 1.0 { s } else { s * share };
            if let Some(t) = self.take_spur(f, ask, phase) {
                sum.organic += t.organic;
                sum.mineral += t.mineral;
                sum.energy += t.energy;
            }
        }
        (sum.organic > 0.0).then_some(sum)
    }

    /// **A pollinator's visit** to `face`'s flower (FLOWER-FRUIT): counted on the face while
    /// it is in flower, toward the bunch it sets ([`VineConfig::pollination_visits`]), and
    /// in `FloraLedger::flower_visits`. `false`, booking nothing, unless the face is in
    /// flower.
    pub fn visit_face(&mut self, face: Face) -> bool {
        let cover = &mut self.cover;
        cover.ensure_built();
        let Some(s) = cover.slot_of(face) else {
            return false;
        };
        let f = &mut cover.faces[s];
        if f.spur != SpurPhase::Flower {
            return false;
        }
        f.visits = f.visits.saturating_add(1);
        self.ledger.flower_visits[1] += 1;
        true
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
        let owner = f.owner;
        let mineral = self.vine_mineral_out(owner, self.config.latticevine.mineral_fraction * took);
        let taken = Taken {
            organic: took,
            mineral,
            energy: e * took,
        };
        Some(self.book_vine_consumed(taken))
    }

    /// Up to `want` of mineral out of vine `owner`'s store (all it holds when that is
    /// less): what an eaten product carries.
    fn vine_mineral_out(&mut self, owner: VineId, want: f64) -> f64 {
        let Some(vi) = vine_at(&self.cover.vines, owner) else {
            return 0.0;
        };
        let v = &mut self.cover.vines[vi];
        let m = want.max(0.0).min(v.mineral);
        v.mineral -= m;
        m
    }

    fn book_vine_consumed(&mut self, taken: Taken) -> Taken {
        self.ledger.consumed_organic_out += taken.organic;
        self.ledger.consumed_mineral_out += taken.mineral;
        self.ledger.consumed_energy_out += taken.energy;
        taken
    }

    /// Root [`VineConfig::founder_count`] founder vines on a fresh world (the floor's area
    /// times [`VineConfig::founders_per_m2`], at least [`VineConfig::founders`]), each with
    /// [`VineConfig::founder_reserve`]: on soil support faces at the **foot** of a wall at
    /// least three faces tall, or on the **lip** of a soil ledge with at least two faces of
    /// drop below it (a stepped slope's risers count, joined by [`stair_up`]). The
    /// **biggest lit open walls first** (plants-flowering-vines 2026-10-03): each
    /// candidate carries the lit area of the open wall its first face is part of — the
    /// eligible faces joined to it in its own plane, each weighted by its [`face_light`]
    /// ([`wall_lit_area`]) — and the world seed's shuffle only breaks ties. (Before, the
    /// rank was a wall's height counted up stair risers too, so a long stepped slope beat
    /// every sheer face: on the desktop terrarium no founder took any of the big open
    /// walls.) Kept
    /// [`VineConfig::founder_spacing`] voxels apart, and [`VineConfig::founder_spread_m`]
    /// of floor apart while the walls allow it (a second pass fills the rest at the
    /// spacing alone). Returns how many rooted.
    pub fn seed_vine_founders(&mut self, world: &World) -> usize {
        let view = world.view();
        let c = view.config;
        let area = f64::from(c.width) * f64::from(c.depth) * c.voxel_m * c.voxel_m;
        let want = self.config.latticevine.founder_count(area);
        if want == 0 {
            return 0;
        }
        let ok = |f: Face| face_eligible(&view, f);
        // The next eligible face up from `f`, straight up the wall or up a stair.
        let up = |f: Face| {
            step_voxel(c, f.x, f.y, f.z, (0, 1, 0))
                .map(|(x, y, z)| Face::new(x, y, z, f.dir))
                .filter(|&n| ok(n))
                .or_else(|| stair_up(c, f).filter(|&n| ok(n)))
        };
        let mut candidates: Vec<(Site, Face, f64)> = Vec::new();
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
                        // At least three faces tall, counting a stepped slope's risers.
                        if let Some((wx, wy, wz)) = step_voxel(c, x, y + 1, z, (-o.0, -o.1, -o.2))
                            && ok(Face::new(wx, wy, wz, d))
                            && up(Face::new(wx, wy, wz, d)).and_then(up).is_some()
                        {
                            let f = Face::new(wx, wy, wz, d);
                            candidates.push((s, f, 0.0));
                        }
                        // Lip: the ledge's own side, with a drop below it, straight or
                        // down a stair.
                        let lip = Face::new(x, y, z, d);
                        if y >= 1
                            && ok(lip)
                            && (ok(Face::new(x, y - 1, z, d))
                                || stair_down(c, lip).is_some_and(|n| ok(n)))
                        {
                            candidates.push((s, lip, 0.0));
                        }
                    }
                }
            }
        }
        if candidates.is_empty() {
            return 0;
        }
        // Each candidate's wall: its lit open area, one walk per wall.
        {
            let vc = &self.config.latticevine;
            let mut sky: Vec<(Site, f64)> = Vec::new();
            let mut wall_of: FxHashMap<Face, usize> = FxHashMap::default();
            let mut areas: Vec<f64> = Vec::new();
            for cand in &mut candidates {
                let w = match wall_of.get(&cand.1) {
                    Some(&w) => w,
                    None => {
                        let (faces, area) = wall_lit_area(&view, vc, cand.1, &mut sky);
                        let w = areas.len();
                        areas.push(area);
                        for f in faces {
                            wall_of.insert(f, w);
                        }
                        w
                    }
                };
                cand.2 = areas[w];
            }
        }
        // A keyed shuffle, then the biggest lit wall first — the sort is stable, so the
        // shuffle breaks the ties — then the first that keep their distance.
        let mut rng = Rng::keyed(DOMAIN_VINE_FOUNDERS, c.seed, candidates.len() as u64, 0);
        for i in (1..candidates.len()).rev() {
            let j = rng.below(i + 1);
            candidates.swap(i, j);
        }
        candidates.sort_by(|a, b| b.2.total_cmp(&a.2));
        let width = c.width as i64;
        let spacing = i64::from(self.config.latticevine.founder_spacing);
        let apart = |a: Site, b: Site| {
            let dx = (a.x as i64 - b.x as i64).rem_euclid(width);
            let dx = dx.min(width - dx);
            let dy = a.y as i64 - b.y as i64;
            let dz = a.z as i64 - b.z as i64;
            dx * dx + dy * dy + dz * dz >= spacing * spacing
        };
        // Floor distance in voxels: `x` wrapped and `z`, height ignored.
        let spread = self.config.latticevine.founder_spread_m / c.voxel_m;
        let spread_apart = |a: Site, b: Site| {
            let dx = (a.x as i64 - b.x as i64).rem_euclid(width);
            let dx = dx.min(width - dx) as f64;
            let dz = (a.z as i64 - b.z as i64) as f64;
            dx * dx + dz * dz >= spread * spread
        };
        let reserve = self.config.latticevine.founder_reserve;
        let mut rooted: Vec<Site> = Vec::new();
        // First spread apart, tallest first; then, if the walls ran out, the rest at the
        // spacing alone.
        let passes: &[bool] = if spread > 0.0 { &[true, false] } else { &[false] };
        for &wide in passes {
            for &(site, first, _) in &candidates {
                if rooted.len() >= want {
                    break;
                }
                if !rooted
                    .iter()
                    .all(|&r| apart(r, site) && (!wide || spread_apart(r, site)))
                {
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
        }
        rooted.len()
    }
}

impl Flora {
    /// **A latticevine painted on `site`** (WORLD-EDIT, `crate::edit`): a founder vine rooted
    /// on the support face `site`, on the first face beside it a vine can root on (a wall
    /// foot, then a ledge lip: the banked seed's order), with the founders' reserve and that
    /// face's leafiness `leafiness` (clamped to `0..=1`), all booked `seeded_*_in`. The vine's
    /// id, or why not: not a soil support face, a vine already rooted there, or no face
    /// beside it to root on.
    pub fn paint_vine(&mut self, world: &World, site: Site, leafiness: f64) -> Result<VineId, String> {
        let view = world.view();
        let c = view.config;
        if !leafiness.is_finite() {
            return Err("the amount is not finite".into());
        }
        if site.x >= c.width || site.z >= c.depth || !view.is_support(i64::from(site.x), site.y, site.z) {
            return Err("not a support face".into());
        }
        if view.material_at(i64::from(site.x), site.y, site.z) != Material::Soil {
            return Err("a latticevine roots in soil".into());
        }
        self.cover.ensure_built();
        if self.cover.rt.roots.contains_key(&site) {
            return Err("a latticevine is already rooted there".into());
        }
        let ok = |cover: &Cover, f: Face| face_eligible(&view, f) && !cover.rt.index.contains_key(&f) && root_adjacent(c, site, f);
        let mut first = None;
        for d in FaceDir::SIDES {
            let o = d.offset();
            if let Some((wx, wy, wz)) = step_voxel(c, site.x, site.y + 1, site.z, (-o.0, -o.1, -o.2)) {
                let f = Face::new(wx, wy, wz, d);
                if ok(&self.cover, f) {
                    first = Some(f);
                    break;
                }
            }
        }
        if first.is_none() {
            first = FaceDir::SIDES.into_iter().map(|d| Face::new(site.x, site.y, site.z, d)).find(|&f| ok(&self.cover, f));
        }
        let first = first.ok_or("no wall foot or ledge lip beside it for a vine to climb")?;
        let reserve = self.config.latticevine.founder_reserve;
        let id = self
            .seed_vine(world, VineSeed { root: site, first, reserve, lineage: None })
            .ok_or("the cover refused the vine")?;
        let vc = &self.config.latticevine;
        let s = self.cover.slot_of(first).expect("just covered");
        let want = leafiness.clamp(0.0, 1.0);
        let d = (want - self.cover.faces[s].leafiness) * vc.leaf_mass;
        self.cover.faces[s].leafiness = want;
        self.ledger.seeded_organic_in += d;
        self.ledger.seeded_energy_in += vc.energy_density * d;
        Ok(id)
    }

    /// **Every latticevine rooted on `site` taken out** (WORLD-EDIT): its faces, reserve and
    /// mineral booked `removed_*_out`. The vines removed.
    pub fn clear_vines_at(&mut self, site: Site) -> usize {
        self.cover.ensure_built();
        let vc = self.config.latticevine.clone();
        let e = vc.energy_density;
        let mut n = 0;
        let mut vi = 0;
        while vi < self.cover.vines.len() {
            let v = self.cover.vines[vi];
            if v.root != site {
                vi += 1;
                continue;
            }
            for s in self.cover.slots_of(v.id).into_iter().rev() {
                let cf = self.cover.remove_slot(s);
                book_removed(&mut self.ledger, e, cf.organic(&vc));
            }
            book_removed(&mut self.ledger, e, v.reserve);
            self.ledger.removed_mineral_out += v.mineral;
            self.cover.remove_vine(vi);
            n += 1;
        }
        n
    }
}
