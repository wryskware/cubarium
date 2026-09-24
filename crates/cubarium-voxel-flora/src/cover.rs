//! **The latticevine face cover** (D15 Revision 2,
//! `design/art-direction/species-dossier-D15-latticevine-2026-09-23.md`; brief
//! `design/handoffs/latticevine-cover-sim-2026-09-23.md`).
//!
//! **API sketch only.** The test author's pass: the public surface the behaviour tests in
//! `tests/latticevine.rs` are written against, with `todo!()` bodies. The implementer fills
//! them in and may add to this surface; a change to what the tests call is stated in the
//! implementer's report with its reason.
//!
//! # The model, as the sketch reads it
//!
//! - A **face** is a solid terrain voxel plus an outward direction: the four horizontal
//!   sides and down (an overhang's underside). Up is the ordinary stand world and is never a
//!   face here. A face is **eligible** when its voxel is solid terrain and the voxel beyond
//!   it is **inside the world** and **air** — not solid, and not holding water
//!   ([`face_eligible`]).
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

use serde::{Deserialize, Serialize};

use cubarium_voxel::{VoxelView, World};

use crate::{Flora, Site, Taken};

/// A vine's identity: unique for the life of the layer, never reused.
pub type VineId = u64;

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
        let _ = view;
        todo!("Face::front")
    }
}

/// Whether `face` can carry cover: its voxel is solid terrain and the voxel in front of it
/// is inside the world, air, and not holding water. Any solid material qualifies.
pub fn face_eligible(view: &VoxelView<'_>, face: Face) -> bool {
    let _ = (view, face);
    todo!("face_eligible")
}

/// **The one light rule the step uses** for a face: `sky` — the sky visibility of the air
/// voxel in front of the face — times the orientation factor, 1 for the four sides and
/// [`VineConfig::underside_light`] for [`FaceDir::Down`]. Shading by stand crowns is not in
/// it (v1).
pub fn face_light(config: &VineConfig, dir: FaceDir, sky: f64) -> f64 {
    let _ = (config, dir, sky);
    todo!("face_light")
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
    /// Water held for transpiration, cubic metres.
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

/// The vines and their covered faces. Storage is the implementer's (dense arrays or
/// FxHash keyed by face); the sketch keeps two sorted vectors.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cover {
    /// Sorted by id.
    vines: Vec<Vine>,
    /// Sorted by face.
    faces: Vec<CoveredFace>,
}

impl Cover {
    /// Every living vine, sorted by id.
    pub fn vines(&self) -> &[Vine] {
        todo!("Cover::vines")
    }

    pub fn vine(&self, id: VineId) -> Option<&Vine> {
        let _ = id;
        todo!("Cover::vine")
    }

    /// The covered face at `face`, if any vine owns it.
    pub fn face(&self, face: Face) -> Option<&CoveredFace> {
        let _ = face;
        todo!("Cover::face")
    }

    /// Every covered face, sorted by face.
    pub fn faces(&self) -> Vec<CoveredFace> {
        todo!("Cover::faces")
    }

    /// The faces `id` owns, sorted by face.
    pub fn faces_of(&self, id: VineId) -> Vec<CoveredFace> {
        let _ = id;
        todo!("Cover::faces_of")
    }

    /// One [`FaceDraw`] per covered face, sorted by face: the presenter's whole read.
    pub fn draw(&self) -> Vec<FaceDraw> {
        todo!("Cover::draw")
    }
}

/// The latticevine's numbers: [`crate::FloraConfig::latticevine`]. Every default is a
/// placeholder for the implementer to choose and state (`design/backlog.md` §1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VineConfig {
    /// Root box, as a stand's: voxels down from the support, and sideways.
    pub rooting_depth: u32,
    pub rooting_radius: u32,
    /// Water demand per unit of total leafiness, cubic metres per second.
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
    /// How long the leaf fall takes once dormancy starts, seconds.
    pub leaf_fall_s: f64,
    /// Organic income per second of a face at light 1, leafiness 1, moisture 1.
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
    /// Reserve paid to cover one new face (its runner plus construction).
    pub spread_cost: f64,
    /// How often a vine tries to spread, seconds.
    pub spread_check_s: f64,
    /// Reserve the parent pays to root a sister.
    pub sister_cost: f64,
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
}

impl Default for VineConfig {
    fn default() -> VineConfig {
        VineConfig {
            rooting_depth: 2,
            rooting_radius: 1,
            transpiration_m3_per_s: 1e-7,
            wilt_water: 0.1,
            full_water: 0.5,
            dry_water: 0.15,
            wet_water: 0.4,
            dry_spell_s: 600.0,
            wet_spell_s: 300.0,
            leaf_fall_s: 60.0,
            assimilation: 0.01,
            underside_light: 0.15,
            leaf_mass: 0.05,
            runner_mass: 0.02,
            leaf_upkeep: 0.0005,
            wood_upkeep: 0.00005,
            regrow_rate: 0.01,
            spread_cost: 0.05,
            spread_check_s: 5.0,
            sister_cost: 0.2,
            contest_threshold: 0.2,
            contest_check_s: 30.0,
            spur_reserve_min: 0.05,
            spur_window_s: 120.0,
            spur_jitter_s: 60.0,
            bud_s: 60.0,
            flower_s: 120.0,
            fruit_s: 240.0,
            spent_s: 60.0,
            nectar: 0.005,
            fruit: 0.01,
            energy_density: 1.0,
        }
    }
}

impl Flora {
    /// Root a founder vine on `seed.root` covering `seed.first` at leafiness 1, rooted,
    /// with `seed.reserve`; its organic matter booked as `seeded_*_in`. Refused (`None`)
    /// when the root is not a support face or already holds a vine root, or `first` is not
    /// eligible, is owned, or is not next to the root (the module doc's root relation). A
    /// stand on the same site is no obstacle.
    pub fn seed_vine(&mut self, world: &World, seed: VineSeed) -> Option<VineId> {
        let _ = (world, seed, &self.cover);
        todo!("Flora::seed_vine")
    }

    /// Hand-built cover for founders and fixtures: extend `vine` onto `face` at
    /// `leafiness`, its runner and leaf booked as `seeded_*_in`. Refused when `face` is not
    /// eligible, is owned, or is not cover-adjacent to a face `vine` already owns.
    pub fn cover_face(&mut self, world: &World, vine: VineId, face: Face, leafiness: f64) -> bool {
        let _ = (world, vine, face, leafiness);
        todo!("Flora::cover_face")
    }

    /// A grazer's bite: thin `face`'s leafiness by up to `want` organic units of leaf and
    /// return what was taken, booked as `consumed_*_out`. The runner stays and the face
    /// stays owned. `None` when the face is not covered or holds no leaf.
    pub fn crop(&mut self, face: Face, want: f64) -> Option<Taken> {
        let _ = (face, want);
        todo!("Flora::crop")
    }

    /// Drink up to `want` of `face`'s nectar. `None` unless the face is in flower with
    /// nectar left. Booked as `consumed_*_out`.
    pub fn take_nectar(&mut self, face: Face, want: f64) -> Option<Taken> {
        let _ = (face, want);
        todo!("Flora::take_nectar")
    }

    /// Eat up to `want` of `face`'s beads. `None` unless the face is in fruit with beads
    /// left. Booked as `consumed_*_out`.
    pub fn take_fruit(&mut self, face: Face, want: f64) -> Option<Taken> {
        let _ = (face, want);
        todo!("Flora::take_fruit")
    }
}
