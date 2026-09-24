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
/// the face one voxel along that step, looking the same way, is covered. `x` wraps with
/// the ring; a step off the world's top, bottom or walls is uncovered.
pub fn mask(face: Face, world: &WorldConfig, covered: impl Fn(Face) -> bool) -> u8 {
    let mut m = 0;
    for (bit, (dx, dy, dz)) in plane_steps(face.dir).into_iter().enumerate() {
        let y = i64::from(face.y) + dy;
        let z = i64::from(face.z) + dz;
        if y < 0 || y >= i64::from(world.height) || z < 0 || z >= i64::from(world.depth) {
            continue;
        }
        let x = (i64::from(face.x) + dx).rem_euclid(i64::from(world.width));
        if covered(Face::new(x as u32, y as u32, z as u32, face.dir)) {
            m |= 1 << bit;
        }
    }
    m
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
/// edge's choice is a hash of **the edge itself** — its two voxels, unordered, and the
/// faces' direction — so the faces on either side of it pick the same crossing and their
/// runners meet. Bits outside `mask` are zero.
pub fn exits(face: Face, mask: u8, world: &WorldConfig) -> u8 {
    let mut e = 0;
    for (bit, (dx, dy, dz)) in plane_steps(face.dir).into_iter().enumerate() {
        if mask & (1 << bit) == 0 {
            continue;
        }
        let a = (face.x, face.y, face.z);
        let b = (
            (i64::from(face.x) + dx).rem_euclid(i64::from(world.width)) as u32,
            (i64::from(face.y) + dy) as u32,
            (i64::from(face.z) + dz) as u32,
        );
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
pub fn cells(
    draws: &[FaceDraw],
    world: &WorldConfig,
    root: impl Fn(VineId) -> Option<Site>,
) -> Vec<VineCell> {
    let covered: FxHashSet<Face> = draws.iter().map(|d| d.face).collect();
    let mut by_voxel: HashMap<(u32, u32, u32), VineCell> = HashMap::new();
    for d in draws {
        let kind = CellKind::of(d.face.dir);
        let f = d.face;
        let (ox, oy, oz) = f.dir.offset();
        let y = i64::from(f.y) + oy;
        let z = i64::from(f.z) + oz;
        if y < 0 || y >= i64::from(world.height) || z < 0 || z >= i64::from(world.depth) {
            continue;
        }
        let x = (i64::from(f.x) + ox).rem_euclid(i64::from(world.width)) as u32;
        let cell = VineCell {
            x,
            y: y as u32,
            z: z as u32,
            kind,
            rooted: d.rooted,
            hanging: d.rooted && root(d.owner).is_some_and(|r| hanging(f, r)),
            density: density(d.leafiness, d.dormant),
            mask: 0,
            exits: 0,
            accent: accent(d.spur),
        };
        let m = mask(f, world, |n| covered.contains(&n));
        let cell = VineCell {
            mask: m,
            exits: exits(f, m, world),
            ..cell
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> WorldConfig {
        WorldConfig {
            width: 16,
            height: 12,
            depth: 6,
            ..WorldConfig::default()
        }
    }

    fn draw(face: Face, leafiness: f64) -> FaceDraw {
        FaceDraw {
            face,
            owner: 1,
            leafiness,
            rooted: false,
            dormant: false,
            spur: SpurPhase::Bare,
        }
    }

    /// A face facing the camera: up is `+y` and right is `+x`, so a vertical run's middle
    /// face has up and down set and a row's end has only its inner side.
    #[test]
    fn the_mask_reads_up_right_down_left_in_the_faces_plane() {
        let w = world();
        let at = |x, y| Face::new(x, y, 3, FaceDir::NegZ);
        let covered = [at(5, 4), at(5, 5), at(5, 6), at(6, 5)];
        let is = |f: Face| covered.contains(&f);
        assert_eq!(mask(at(5, 5), &w, is), 0b0111, "up, right, down");
        assert_eq!(mask(at(5, 4), &w, is), 0b0001, "up only");
        assert_eq!(mask(at(6, 5), &w, is), 0b1000, "left only");
        // A face looking another way is not a neighbour.
        let other = [Face::new(5, 6, 3, FaceDir::PosX)];
        assert_eq!(mask(at(5, 5), &w, |f| other.contains(&f)), 0);
        // `+z` faces turn: the camera's right is their left.
        let back = |x| Face::new(x, 5, 3, FaceDir::PosZ);
        assert_eq!(mask(back(5), &w, |f| f == back(6)), 0b1000);
        // A `+x` face's right is `+z`.
        let side = |z| Face::new(5, 5, z, FaceDir::PosX);
        assert_eq!(mask(side(3), &w, |f| f == side(4)), 0b0010);
    }

    #[test]
    fn the_mask_wraps_in_x_and_stops_at_the_worlds_edges() {
        let w = world();
        let f = Face::new(15, 11, 0, FaceDir::NegZ);
        let right = Face::new(0, 11, 0, FaceDir::NegZ);
        assert_eq!(mask(f, &w, |n| n == right), 0b0010, "x wraps");
        assert_eq!(mask(f, &w, |_| true), 0b1110, "nothing above the top row");
    }

    #[test]
    fn density_follows_leafiness_and_a_dormant_vine_is_bare() {
        assert_eq!(density(1.0, false), Density::Full);
        assert_eq!(density(0.66, false), Density::Full);
        assert_eq!(density(0.5, false), Density::Thin);
        assert_eq!(density(0.2, false), Density::Thin);
        assert_eq!(density(0.19, false), Density::Bare);
        assert_eq!(density(f64::NAN, false), Density::Bare);
        assert_eq!(density(1.0, true), Density::Bare);
    }

    #[test]
    fn an_accent_shows_only_in_its_spur_phase() {
        assert_eq!(accent(SpurPhase::Bud), Accent::Bud);
        assert_eq!(accent(SpurPhase::Flower), Accent::Flower);
        assert_eq!(accent(SpurPhase::Fruit), Accent::Fruit);
        assert_eq!(accent(SpurPhase::Bare), Accent::None);
        assert_eq!(accent(SpurPhase::Spent), Accent::None);
    }

    #[test]
    fn tile_names_follow_the_file_interface() {
        assert_eq!(tile_name(TileSet::Plain, Density::Full, 0xb, None), "vine-full-b");
        assert_eq!(tile_name(TileSet::Plain, Density::Full, 0xb, Some(0xf)), "vine-full-b-b");
        assert_eq!(tile_name(TileSet::Climbing, Density::Bare, 0, Some(0)), "vine-root-bare-0-0");
        assert_eq!(
            tile_name(TileSet::Hanging, Density::Thin, 12, Some(4)),
            "vine-root-thin-c-4-hang"
        );
        assert_eq!(tile_column(0xb, 0xf), 0xb * 16 + 0xb);
        assert_eq!(accent_name(Accent::Flower), "vine-accent-flower");
    }

    /// Each face lands in the air voxel in front of it, and where two faces front one voxel
    /// the one facing the camera wins.
    #[test]
    fn cells_sit_in_front_and_the_most_visible_face_wins_a_shared_voxel() {
        let w = world();
        let front = Face::new(4, 2, 3, FaceDir::NegZ); // cell (4, 2, 2)
        let side = Face::new(3, 2, 2, FaceDir::PosX); // cell (4, 2, 2) too
        let under = Face::new(8, 5, 1, FaceDir::Down); // cell (8, 4, 1)
        let back = Face::new(10, 2, 1, FaceDir::PosZ);
        let mut d = vec![draw(side, 1.0), draw(front, 0.4), draw(under, 0.1), draw(back, 1.0)];
        d[1].spur = SpurPhase::Fruit;
        let cells = cells(&d, &w, |_| None);
        assert_eq!(cells.len(), 3);
        let b = cells.iter().find(|c| (c.x, c.y, c.z) == (10, 2, 2)).unwrap();
        assert!(!b.kind.tiled(), "a `+z` face is a plain cell only");
        let c = cells.iter().find(|c| (c.x, c.y, c.z) == (4, 2, 2)).unwrap();
        assert_eq!(c.kind, CellKind::Flush);
        assert_eq!((c.density, c.accent), (Density::Thin, Accent::Fruit));
        let u = cells.iter().find(|c| (c.x, c.y, c.z) == (8, 4, 1)).unwrap();
        assert_eq!((u.kind, u.density), (CellKind::Curtain, Density::Bare));
    }

    /// A rooted face on a ledge's lip hangs and draws the `-hang` tile; one at a wall's foot
    /// climbs.
    #[test]
    fn a_hanging_root_draws_the_hanging_tile() {
        let w = world();
        let root = Site { x: 4, y: 6, z: 3 };
        let lip = Face::new(4, 6, 3, FaceDir::NegZ);
        let below = Face::new(4, 5, 3, FaceDir::NegZ);
        let mut d = vec![draw(lip, 1.0), draw(below, 1.0)];
        d[0].rooted = true;
        let c = cells(&d, &w, |_| Some(root));
        let top = c.iter().find(|c| c.y == 6).unwrap();
        assert!(top.hanging);
        assert_eq!(top.mask, 0b0100, "covered below");
        assert_eq!(top.tile().0, TileSet::Hanging);
        assert_eq!(top.tile().2, 0b0100);
        let under = c.iter().find(|c| c.y == 5).unwrap();
        assert_eq!(under.tile().0, TileSet::Plain);
        let foot = Site { x: 4, y: 5, z: 3 };
        assert!(!hanging(Face::new(4, 6, 3, FaceDir::NegZ), foot), "the foot climbs");
        assert_eq!(flip_mask(0b1011), 0b1110);
    }

    #[test]
    fn the_texel_bytes_carry_every_field() {
        let c = VineCell {
            x: 0,
            y: 0,
            z: 0,
            kind: CellKind::SliverRight,
            rooted: true,
            hanging: true,
            density: Density::Thin,
            mask: 0b1001,
            exits: 0b1111,
            accent: Accent::Flower,
        };
        // Rooted and hanging, thin: row 3 · 2 + 1; exits only where the mask is set.
        assert_eq!(c.texel_bytes(), (4, 0b1001 | 0b1001 << 4, 7 | 2 << 4));
    }

    /// The default look: each covered face is a plain crown cell in the air in front of
    /// it, in the vine's colours; a face in flower is a heart cell tinted lilac with the
    /// cyan mouth as its glint; and a stand in the same cell keeps it.
    #[test]
    fn covered_faces_are_plain_voxel_cells_and_an_organism_keeps_its_cell() {
        use crate::voxel::stand::{Part, Stands};
        use cubarium_voxel::{Command, Material, World};
        use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species, VineSeed};
        let mut world = World::empty(world());
        for z in 0..6u32 {
            for x in 0..16i64 {
                for y in 0..3u32 {
                    let material = if y == 0 { Material::Bedrock } else { Material::Soil };
                    world.apply(Command::SetMaterial { x, y, z, material });
                }
                if (4..=10).contains(&x) && z >= 3 {
                    for y in 3..9u32 {
                        world.apply(Command::SetMaterial { x, y, z, material: Material::Rock });
                    }
                }
            }
        }
        let mut flora = Flora::new(FloraConfig::default());
        let face = |x, y| Face::new(x, y, 3, FaceDir::NegZ);
        let vine = flora
            .seed_vine(
                &world,
                VineSeed { root: Site { x: 6, y: 2, z: 2 }, first: face(6, 3), reserve: 1.0, lineage: None },
            )
            .unwrap();
        assert!(flora.cover_face(&world, vine, face(6, 4), 1.0));
        assert!(flora.cover_face(&world, vine, face(7, 4), 0.1));
        let view = world.view();
        let mut stands = Stands::empty(16, 12, 6);
        stands.rebuild(&view, flora.view());
        let full = crate::present::srgb_linear(crate::voxel::colours::vine(Density::Full, Accent::None).body);
        let part = stands.at(6, 4, 2);
        assert!(matches!(part, Part::Crown { heart: false, .. }), "{part:?}");
        assert_eq!(stands.style(part).unwrap().crown, full);
        let bare = crate::present::srgb_linear(crate::voxel::colours::vine(Density::Bare, Accent::None).body);
        assert_eq!(stands.style(stands.at(7, 4, 2)).unwrap().crown, bare);

        let mut draws = flora.view().cover.draw();
        for d in &mut draws {
            if d.face == face(6, 4) {
                d.spur = SpurPhase::Flower;
            }
        }
        stands.set_cover_draws(Some(draws));
        stands.rebuild(&view, flora.view());
        let part = stands.at(6, 4, 2);
        assert!(matches!(part, Part::Crown { heart: true, .. }), "{part:?}");
        let lilac = crate::present::srgb_linear(0xB9_9BE6);
        assert_eq!(stands.style(part).unwrap().crown, lilac);

        let wood = FloraConfig::default().species(Species::Bloomcrown).wood_max * 0.4;
        assert!(flora.apply(&world, FloraCommand::Seed { x: 6, z: 2, species: Species::Bloomcrown, wood }));
        stands.set_cover_draws(None);
        stands.rebuild(&view, flora.view());
        assert!(matches!(stands.at(6, 3, 2), Part::Trunk(_)), "the stand keeps its cell");
    }

    /// The faces on either side of a shared edge pick the same crossing, for every edge
    /// and every direction; and both crossings turn up across a wall.
    #[test]
    fn neighbours_agree_on_where_a_runner_crosses_their_shared_edge() {
        let w = world();
        let mut seen = [false; 2];
        for dir in FaceDir::ALL {
            for y in 1..10u32 {
                for x in 0..16u32 {
                    let f = Face::new(x, y, 2, dir);
                    let steps = plane_steps(dir);
                    for (bit, (dx, dy, dz)) in steps.into_iter().enumerate() {
                        let n = Face::new(
                            (i64::from(x) + dx).rem_euclid(16) as u32,
                            (i64::from(y) + dy) as u32,
                            (2 + dz) as u32,
                            dir,
                        );
                        let back = (bit + 2) % 4;
                        let mine = exits(f, 1 << bit, &w) >> bit & 1;
                        let theirs = exits(n, 1 << back, &w) >> back & 1;
                        assert_eq!(mine, theirs, "{f:?} and {n:?}");
                        seen[usize::from(mine)] = true;
                    }
                }
            }
        }
        assert_eq!(seen, [true, true]);
        assert_eq!(exits(Face::new(3, 3, 2, FaceDir::NegZ), 0, &w), 0, "no edge, no exit");
    }
}
