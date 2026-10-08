//! **The latticevine tile layer**: which cell and which tile a covered rock face draws
//! as (D15 Revision 2, `design/handoffs/latticevine-visual-2026-09-24.md`).
//!
//! A covered face ([`FaceDraw`]) draws in the **air voxel in front of it**, and that
//! cell carries everything the GPU needs in bytes an air voxel otherwise leaves unused
//! ([`VineCell::texel_bytes`]). How each direction reaches the picture from this
//! elevated camera, which looks along `+z` and draws only front (`−z`) and top faces:
//!
//! - **`−z`, facing the camera** ([`CellKind::Flush`]): the tile is drawn **on the rock
//!   face itself**, flush, where the slab walk meets it; the cell in front only carries
//!   the data. The rock shows through the tile's holes.
//! - **underside** ([`CellKind::Curtain`]): an underside is never seen from above, so
//!   the tile hangs as a curtain on the front face of the air cell below the overhang.
//! - **`±x`** ([`CellKind::SliverLeft`], [`CellKind::SliverRight`]): a side face is seen
//!   edge-on, so only the quarter of the air cell's front face against the wall shows the
//!   tile: the cover wrapping round a pillar's corner.
//! - **`+z`** faces look away from the camera ([`CellKind::Behind`]): a plain voxel
//!   cell like any other, but never a tile.
//!
//! One air voxel can front several covered faces (an inside corner): the most visible
//! kind wins, in the order above. An organism or a ground mark in the same air voxel
//! wins over any vine (the packer only writes a vine into a cell no part claimed).
//!
//! The CPU presenter draws the same cells as flat colour, with no tiles and no accents.

use std::collections::HashMap;

use cubarium_voxel::Config as WorldConfig;
use cubarium_voxel_flora::{Face, FaceDir, FaceDraw, Site, SpurPhase, VineId};
use rustc_hash::FxHashSet;

/// Leafiness at or above which a face draws the full tile.
pub const FULL_AT: f64 = 0.66;
/// Leafiness at or above which a face draws the thinned tile; below it, bare runner.
pub const THIN_AT: f64 = 0.2;

/// Which of the three tile densities a face draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Density {
    Bare = 0,
    Thin = 1,
    Full = 2,
}

impl Density {
    pub const ALL: [Density; 3] = [Density::Bare, Density::Thin, Density::Full];

    pub fn name(self) -> &'static str {
        match self {
            Density::Bare => "bare",
            Density::Thin => "thin",
            Density::Full => "full",
        }
    }
}

/// The density a face draws: a dormant vine is bare runner everywhere.
pub fn density(leafiness: f64, dormant: bool) -> Density {
    if dormant || !(leafiness >= THIN_AT) {
        Density::Bare
    } else if leafiness >= FULL_AT {
        Density::Full
    } else {
        Density::Thin
    }
}

/// Which accent overlays a face: only while its spur is in that phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Accent {
    None = 0,
    Bud = 1,
    Flower = 2,
    Fruit = 3,
}

impl Accent {
    /// The three overlays, in the atlas's order.
    pub const DRAWN: [Accent; 3] = [Accent::Bud, Accent::Flower, Accent::Fruit];

    pub fn name(self) -> &'static str {
        match self {
            Accent::None => "none",
            Accent::Bud => "bud",
            Accent::Flower => "flower",
            Accent::Fruit => "fruit",
        }
    }
}

pub fn accent(spur: SpurPhase) -> Accent {
    match spur {
        SpurPhase::Bud => Accent::Bud,
        SpurPhase::Flower => Accent::Flower,
        SpurPhase::Fruit => Accent::Fruit,
        SpurPhase::Bare | SpurPhase::Spent => Accent::None,
    }
}

/// The four in-plane steps of a face as its viewer sees it: up, right, down, left. "Right"
/// is the viewer's right standing in front of the face; the camera looks along `+z` with
/// `+x` to its right, and the other sides turn with it. An underside is read as the camera
/// would see it from below: up is `+z`, away from the camera.
pub fn plane_steps(dir: FaceDir) -> [(i64, i64, i64); 4] {
    let (right, up) = match dir {
        FaceDir::NegZ => ((1, 0, 0), (0, 1, 0)),
        FaceDir::PosZ => ((-1, 0, 0), (0, 1, 0)),
        FaceDir::PosX => ((0, 0, 1), (0, 1, 0)),
        FaceDir::NegX => ((0, 0, -1), (0, 1, 0)),
        FaceDir::Down => ((1, 0, 0), (0, 0, 1)),
    };
    let neg = |(a, b, c): (i64, i64, i64)| (-a, -b, -c);
    [up, right, neg(up), neg(right)]
}

/// The neighbour mask of `face`: bit 0 up, bit 1 right, bit 2 down, bit 3 left, set where
/// the face one voxel along that step, looking the same way, is covered — or, up and down
/// a side face, the face one step up or down a staircase ([`stair_step`]). `x` wraps with
/// the ring; a step off the world's top, bottom or walls is uncovered.
pub fn mask(face: Face, world: &WorldConfig, covered: impl Fn(Face) -> bool) -> u8 {
    let mut m = 0;
    for (bit, step) in plane_steps(face.dir).into_iter().enumerate() {
        let stair = match bit {
            0 => stair_step(face, 1, world),
            2 => stair_step(face, -1, world),
            _ => None,
        };
        if step_face(face, step, world).is_some_and(&covered) || stair.is_some_and(&covered) {
            m |= 1 << bit;
        }
    }
    m
}

/// The face one voxel along `(dx, dy, dz)` from `face`, looking the same way; `None` off
/// the world's top, bottom or walls.
fn step_face(face: Face, (dx, dy, dz): (i64, i64, i64), world: &WorldConfig) -> Option<Face> {
    let y = i64::from(face.y) + dy;
    let z = i64::from(face.z) + dz;
    if y < 0 || y >= i64::from(world.height) || z < 0 || z >= i64::from(world.depth) {
        return None;
    }
    let x = (i64::from(face.x) + dx).rem_euclid(i64::from(world.width));
    Some(Face::new(x as u32, y as u32, z as u32, face.dir))
}

/// The side face one step up (`dy = 1`) or down (`dy = −1`) a staircase from `face`: one
/// voxel up and one back from the face, the flora layer's stair adjacency, which carries a
/// runner across a one-voxel tread. `None` for an underside.
pub fn stair_step(face: Face, dy: i64, world: &WorldConfig) -> Option<Face> {
    if face.dir.is_underside() {
        return None;
    }
    let (ox, _, oz) = face.dir.offset();
    step_face(face, (-ox * dy, dy, -oz * dy), world)
}

/// The mask of a tile flipped top to bottom: up and down swap. A hanging root with no
/// `-hang` file of its own draws its climbing tile upside down, with this mask.
pub fn flip_mask(m: u8) -> u8 {
    (m & 0b1010) | ((m & 1) << 2) | ((m >> 2) & 1)
}

/// Whether a rooted face hangs from its root (a vine rooted on a ledge's lip, the root arch
/// at the tile's top) rather than climbing from it (rooted at a wall's foot, the arch at
/// the bottom). The lip is a side of the root's own voxel; the foot is one voxel up.
pub fn hanging(face: Face, root: Site) -> bool {
    face.y <= root.y
}

/// Which set of tiles a face draws from: plain cover, a climbing root (arch at the
/// bottom) or a hanging root (arch at the top).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileSet {
    Plain = 0,
    Climbing = 1,
    Hanging = 2,
}

impl TileSet {
    pub const ALL: [TileSet; 3] = [TileSet::Plain, TileSet::Climbing, TileSet::Hanging];
}

/// The vine atlas's rows: three tile sets of three densities, then the accents.
pub const ACCENT_ROW: u32 = 9;

/// The atlas row of `(set, density)`.
pub fn tile_row(set: TileSet, density: Density) -> u32 {
    3 * set as u32 + density as u32
}

/// The stem of a tile's master file: `vine-<density>-<mask>-<exits>`,
/// `vine-root-<density>-<mask>-<exits>`, or `vine-root-<density>-<mask>-<exits>-hang`,
/// each a hex digit. `exits: None` is the older name with no exits digit, which the
/// loader falls back to while the per-exit set has not landed.
pub fn tile_name(set: TileSet, density: Density, mask: u8, exits: Option<u8>) -> String {
    let m = mask & 15;
    let e = exits.map_or(String::new(), |e| format!("-{:x}", e & m));
    match set {
        TileSet::Plain => format!("vine-{}-{m:x}{e}", density.name()),
        TileSet::Climbing => format!("vine-root-{}-{m:x}{e}", density.name()),
        TileSet::Hanging => format!("vine-root-{}-{m:x}{e}-hang", density.name()),
    }
}

/// The vine atlas's column for a tile: sixteen per mask, one per exits pattern (only the
/// patterns inside the mask are ever drawn).
pub fn tile_column(mask: u8, exits: u8) -> u32 {
    u32::from(mask & 15) * 16 + u32::from(exits & mask & 15)
}

/// Where a covered face's runners cross its covered edges: bit `k` (the mask's order)
/// set means the two-thirds position along that edge, clear the one-third. Each shared
/// edge's choice is a hash of **the edge itself** — its two voxels, unordered, with the
/// coordinate along the faces' normal dropped, and the faces' direction — so the faces on
/// either side of it pick the same crossing and their runners meet, a stair's included. Bits outside `mask` are zero.
pub fn exits(face: Face, mask: u8, world: &WorldConfig) -> u8 {
    let mut e = 0;
    for (bit, (dx, dy, dz)) in plane_steps(face.dir).into_iter().enumerate() {
        if mask & (1 << bit) == 0 {
            continue;
        }
        // The coordinate along the face's normal is left out, so a runner up a stair (one
        // voxel back across the tread) crosses where the upper face expects it.
        let flat = |(x, y, z): (u32, u32, u32)| match face.dir {
            FaceDir::PosX | FaceDir::NegX => (0, y, z),
            FaceDir::PosZ | FaceDir::NegZ => (x, y, 0),
            FaceDir::Down => (x, 0, z),
        };
        let a = flat((face.x, face.y, face.z));
        let b = flat((
            (i64::from(face.x) + dx).rem_euclid(i64::from(world.width)) as u32,
            (i64::from(face.y) + dy) as u32,
            (i64::from(face.z) + dz) as u32,
        ));
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let mut h = 0x9E37_79B9_7F4A_7C15u64 ^ face.dir as u64;
        for v in [lo.0, lo.1, lo.2, hi.0, hi.1, hi.2] {
            h = (h ^ u64::from(v)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            h ^= h >> 31;
        }
        if h & 1 != 0 {
            e |= 1 << bit;
        }
    }
    e
}

/// The stem of an accent overlay's master file.
pub fn accent_name(a: Accent) -> String {
    format!("vine-accent-{}", a.name())
}

/// How a cell shows its face (see the module header), most visible first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CellKind {
    Flush = 0,
    Curtain = 1,
    SliverLeft = 2,
    SliverRight = 3,
    Behind = 4,
}

impl CellKind {
    /// The kind a face of `dir` draws as.
    pub fn of(dir: FaceDir) -> CellKind {
        match dir {
            FaceDir::NegZ => CellKind::Flush,
            FaceDir::Down => CellKind::Curtain,
            // A `+x` face has its rock on the cell's left.
            FaceDir::PosX => CellKind::SliverLeft,
            FaceDir::NegX => CellKind::SliverRight,
            FaceDir::PosZ => CellKind::Behind,
        }
    }

    /// Whether the tile layer draws this kind at all.
    pub fn tiled(self) -> bool {
        self != CellKind::Behind
    }
}

/// One air voxel carrying one covered face's tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VineCell {
    /// The air voxel in front of the face, `x` wrapped.
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub kind: CellKind,
    pub rooted: bool,
    pub hanging: bool,
    pub density: Density,
    /// The face's neighbour mask ([`mask`]), as the face is seen.
    pub mask: u8,
    /// Where its runners cross the covered edges ([`exits`]).
    pub exits: u8,
    pub accent: Accent,
}

impl VineCell {
    /// The cell as the three fields an air texel with no part leaves unused
    /// (`cubarium_gpu::voxel::VoxelTexel::with_vine`): the glyph id, `b` and `a`.
    ///
    /// - glyph: the kind plus one (zero is no vine);
    /// - `b`: the mask in bits 0–3, the exits in bits 4–7 (together the tile's column,
    ///   [`tile_column`]);
    /// - `a`: the tile's atlas row in bits 0–3, the accent in bits 4–5.
    ///
    /// That is [`VineCell::tile`], already resolved: the shader only looks it up.
    pub fn texel_bytes(&self) -> (u8, u8, u8) {
        let (set, density, mask, exits) = self.tile();
        let glyph = self.kind as u8 + 1;
        let b = mask | (exits & mask) << 4;
        let a = tile_row(set, density) as u8 | (self.accent as u8) << 4;
        (glyph, b, a)
    }

    /// The tile this cell samples, `(set, density, mask, exits)`: **the one place a cell
    /// picks its tile**, so a later per-position variant goes here, in [`tile_row`] and
    /// [`tile_column`].
    pub fn tile(&self) -> (TileSet, Density, u8, u8) {
        let set = match (self.rooted, self.hanging) {
            (false, _) => TileSet::Plain,
            (true, false) => TileSet::Climbing,
            (true, true) => TileSet::Hanging,
        };
        (set, self.density, self.mask & 15, self.exits & self.mask & 15)
    }
}

/// Every drawn cell of a cover, at most one per air voxel (the most visible kind wins,
/// then the lower face address). `root` names each owner's root, for [`hanging`].
pub fn cells(draws: &[FaceDraw], world: &WorldConfig, root: impl Fn(VineId) -> Option<Site>) -> Vec<VineCell> {
    let covered: FxHashSet<Face> = draws.iter().map(|d| d.face).collect();
    let mut by_voxel: HashMap<(u32, u32, u32), VineCell> = HashMap::new();
    for d in draws {
        let Some(cell) = face_cell(d, world, root(d.owner), |n| covered.contains(&n)) else { continue };
        by_voxel
            .entry((cell.x, cell.y, cell.z))
            .and_modify(|c| {
                if cell.kind < c.kind {
                    *c = cell;
                }
            })
            .or_insert(cell);
    }
    let mut out: Vec<VineCell> = by_voxel.into_values().collect();
    out.sort_unstable_by_key(|c| (c.z, c.y, c.x));
    out
}

/// Resolve one covered face using the same rule as the legacy air-cell presenter.
/// The feed retains every face, including faces sharing an air cell.
pub fn face_cell(
    d: &FaceDraw,
    world: &WorldConfig,
    root: Option<Site>,
    covered: impl Fn(Face) -> bool,
) -> Option<VineCell> {
    let kind = CellKind::of(d.face.dir);
    let f = d.face;
    let (ox, oy, oz) = f.dir.offset();
    let y = i64::from(f.y) + oy;
    let z = i64::from(f.z) + oz;
    if y < 0 || y >= i64::from(world.height) || z < 0 || z >= i64::from(world.depth) {
        return None;
    }
    let x = (i64::from(f.x) + ox).rem_euclid(i64::from(world.width)) as u32;
    let cell = VineCell {
        x,
        y: y as u32,
        z: z as u32,
        kind,
        rooted: d.rooted,
        hanging: d.rooted && root.is_some_and(|r| hanging(f, r)),
        density: density(d.leafiness, d.dormant),
        mask: 0,
        exits: 0,
        accent: accent(d.spur),
    };
    let m = mask(f, world, covered);
    let cell = VineCell { mask: m, exits: exits(f, m, world), ..cell };
    Some(cell)
}

impl Accent {
    /// The flower mouth is the only luminous vine accent in the legacy presenter.
    pub fn emissive(self) -> bool {
        self == Self::Flower
    }
}
