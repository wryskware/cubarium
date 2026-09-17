//! Turning a [`FloraView`]'s stands into voxels the presenter can draw, and the two
//! species' palettes.
//!
//! # Why an occupancy grid and not a sprite list
//!
//! The presenter has no depth buffer: it paints slabs far to near and, inside a slab,
//! rows bottom to top, and that traversal *is* the depth test (see
//! [`crate::voxel::present`]). A stand drawn as one sprite at one moment of that
//! traversal would therefore be either wholly in front of or wholly behind everything in
//! the slabs it spans — and a crown is a **horizontal disc**, so it spans several of
//! them. The only placement that occludes correctly is the one the terrain already uses:
//! decompose the stand into voxels and let each voxel be drawn at the point where a solid
//! block at *its own* `(x, y, z)` would be drawn.
//!
//! So this module builds, once per frame, a sparse-in-spirit dense grid of
//! [`Part`] over the world's `(z, x, y)` — the same indexing the roof map uses — and the
//! presenter reads it per voxel inside its one traversal. Nearer terrain then hides a
//! trunk because the nearer slab paints later; the trunk hides farther terrain because
//! its own slab paints later than that one; and water in the trunk's own cell blends over
//! it because, within a cell, the plant is stamped before the water.
//!
//! # The geometry
//!
//! A stand on support `(x, y, z)` rises `H = round(crown_height(W))` voxels above the
//! support's top face: a **trunk** in cells `y + 1 ..= y + H − 1` of the support's own
//! column, and a **crown** disc of half-width `crown_radius(W)` voxels, in `x` and `z`,
//! centred on `y + H` — the cell the trunk's top would have been, because a crown grows
//! at the top of a stem and not above it. That disc is the same set of columns
//! `cubarium_voxel_flora`'s shade model calls crown cover, so what shades is what is
//! drawn. `x` wraps with the ring; `z` and `y` clip at the world's faces; a cell inside
//! solid terrain is dropped, so a stand whose support has been buried does not paint
//! inside rock.
//!
//! An [`Stage::Establishing`] stand is a single **sprout** cell just above its support:
//! propagule material, not yet a plant.
//!
//! Whole voxels are the picture's own quantisation. The model's crown top is a float and
//! its disc a float radius; the drawing rounds both, so a stand's drawn top can sit half
//! a voxel from the height the shade model used. Pixel art has no finer answer at 4 px
//! per voxel.

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FloraView, Species, Stage, Stand};

use crate::present::{mix, srgb_linear};

// --- The two palettes ----------------------------------------------------------------
//
// Both live in the Outrun family of `design/appearance.md`, as the strata do, and are
// separated the way `art/PLANTS.md` separates the same two names: bloomcrown warm —
// violet wood under a magenta canopy with a warm bloom heart — and umbrellafrond cool —
// deep teal wood under a turquoise frond with a mint heart. Warm against cool is the
// one distinction that survives a 4-pixel-wide trunk.

/// Bloomcrown wood: a warm plum, clear of the soil violet it stands on.
pub const BLOOM_WOOD_SRGB: u32 = 0x0075_2A58;
/// Bloomcrown canopy at full foliage: the appearance doc's magenta.
pub const BLOOM_CROWN_SRGB: u32 = 0x00E0_4A96;
/// The bloom heart: the one warm accent, as the cube's art keeps it.
pub const BLOOM_HEART_SRGB: u32 = 0x00FF_A85C;

/// Umbrellafrond wood: deep teal, a hollow-dweller's stem.
pub const FROND_WOOD_SRGB: u32 = 0x0021_5A70;
/// Umbrellafrond canopy at full foliage: turquoise.
pub const FROND_CROWN_SRGB: u32 = 0x0033_D2AE;
/// The frond's centre: mint.
pub const FROND_HEART_SRGB: u32 = 0x00A6_F5DC;

/// How far a crown with no foliage left falls back toward its own wood colour. Crown
/// fill is `P / (α·W)`, so a stand that has shed its canopy reads as bare structure
/// rather than as a full crown that happens to be dark.
pub const CROWN_BARE: f32 = 0.85;
/// The most a wilting stand desaturates toward its own luminance, at `μ = 0`. Bounded
/// well below 1 on purpose: a dry stand must still read as its species' warm or cool,
/// because the species is the thing the picture is for.
pub const WILT_DESAT: f32 = 0.5;
/// How far a wilting stand also darkens, at `μ = 0`.
pub const WILT_DARK: f32 = 0.25;
/// How far the crown's heart pixels lean toward the heart colour at full foliage.
pub const HEART_TINT: f32 = 0.75;

/// What a voxel holds, if anything: which part of which drawn stand.
///
/// The index is into [`Stands::styles`], not into the flora's stands: two stands of one
/// species with the same foliage and moisture share a style only by coincidence, but the
/// presenter never needs to know which stand a cell came from — only how to paint it and
/// whether the cell beside it belongs to the same crown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// No plant here.
    None,
    /// A trunk cell: solid wood, a cylinder one voxel across.
    Trunk(u16),
    /// A crown cell. `heart` is the disc's centre column, over the trunk's top.
    Crown { style: u16, heart: bool },
    /// An establishing stand: a mark, not a block.
    Sprout(u16),
}

impl Part {
    /// Does this part fill its voxel's faces? A sprout does not — it is a few pixels in
    /// the middle of an otherwise empty cell, so the cell above it is still open sky and
    /// the cell in front of it still shows what is behind.
    pub fn is_block(self) -> bool {
        matches!(self, Part::Trunk(_) | Part::Crown { .. })
    }

    /// The style this part paints with, if it paints at all.
    pub fn style(self) -> Option<u16> {
        match self {
            Part::None => None,
            Part::Trunk(s) | Part::Sprout(s) => Some(s),
            Part::Crown { style, .. } => Some(style),
        }
    }

    /// Two crown cells of the same stand are one canopy, so the seam between them is not
    /// a silhouette edge. Anything else beside a crown cell is.
    fn same_crown(self, other: Part) -> bool {
        match (self, other) {
            (Part::Crown { style: a, .. }, Part::Crown { style: b, .. }) => a == b,
            _ => false,
        }
    }
}

/// The colours one stand paints with this frame, in linear light, already carrying its
/// crown fill and its wilt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// Wood: the trunk's mid tone.
    pub wood: [f32; 3],
    /// Canopy.
    pub crown: [f32; 3],
    /// The crown's centre.
    pub heart: [f32; 3],
}

/// Every stand of a frame, as voxels plus the styles they paint with.
///
/// Indexed `(z · width + x) · height + y`, the roof map's layout, so the presenter's
/// inner loop touches this grid the same way it touches that one.
pub struct Stands {
    width: u32,
    height: u32,
    depth: u32,
    grid: Vec<Part>,
    styles: Vec<Style>,
}

impl Stands {
    /// An empty grid for a world of this shape.
    pub fn empty(width: u32, height: u32, depth: u32) -> Stands {
        Stands {
            width,
            height,
            depth,
            grid: vec![Part::None; width as usize * height as usize * depth as usize],
            styles: Vec::new(),
        }
    }

    /// Rebuild from a flora view. Reuses the allocation: the presenter calls this every
    /// frame.
    pub fn rebuild(&mut self, view: &VoxelView<'_>, flora: FloraView<'_>) {
        let c = view.config;
        if (self.width, self.height, self.depth) != (c.width, c.height, c.depth) {
            *self = Stands::empty(c.width, c.height, c.depth);
        } else {
            self.grid.fill(Part::None);
            self.styles.clear();
        }
        // Stands arrive in site order, which is the order the styles are pushed in, so
        // the grid is a pure function of the view and not of any iteration accident.
        for stand in flora.stands {
            let style = self.styles.len().min(u16::MAX as usize) as u16;
            if usize::from(style) != self.styles.len() {
                break; // more than 65 535 stands in one strip: refuse to alias styles.
            }
            self.styles.push(style_of(flora, stand));
            for (cell, part) in parts_of(flora, stand, style) {
                self.place(view, cell, part);
            }
        }
    }

    /// Write one part, letting the stronger claim keep the cell. Wood beats canopy —
    /// a trunk through another stand's crown is a trunk — and a crown beats a sprout.
    fn place(&mut self, view: &VoxelView<'_>, cell: Cell, part: Part) {
        if cell.y >= self.height || cell.z >= self.depth {
            return;
        }
        if view.material_at(cell.x, cell.y, cell.z).is_solid() {
            return;
        }
        let i = self.index(cell.x, cell.y, cell.z);
        let rank = |p: Part| match p {
            Part::None => 0u8,
            Part::Sprout(_) => 1,
            Part::Crown { .. } => 2,
            Part::Trunk(_) => 3,
        };
        if rank(part) >= rank(self.grid[i]) {
            self.grid[i] = part;
        }
    }

    #[inline]
    fn index(&self, x: i64, y: u32, z: u32) -> usize {
        let xw = x.rem_euclid(i64::from(self.width)) as usize;
        (z as usize * self.width as usize + xw) * self.height as usize + y as usize
    }

    /// What stands in a voxel. `x` wraps; a `y` or `z` outside the world is empty.
    #[inline]
    pub fn at(&self, x: i64, y: i64, z: u32) -> Part {
        if y < 0 || y >= i64::from(self.height) || z >= self.depth || self.grid.is_empty() {
            return Part::None;
        }
        self.grid[self.index(x, y as u32, z)]
    }

    /// The colours a part paints with.
    #[inline]
    pub fn style(&self, part: Part) -> Option<Style> {
        part.style().map(|s| self.styles[usize::from(s)])
    }

    /// Is the cell beside `(x, y, z)` part of the same canopy? Used for the crown's
    /// silhouette: a disc wants an edge where it ends and no edge inside itself.
    #[inline]
    pub fn crown_continues(&self, part: Part, x: i64, y: u32, z: u32) -> bool {
        part.same_crown(self.at(x, i64::from(y), z))
    }

    /// Whether anything at all is drawn. The presenter skips its per-voxel lookup
    /// entirely on an empty world, which is every frame of a run with no flora.
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }
}

/// One cell a stand occupies. `x` is **unwrapped** — the crown of a stand near the seam
/// reaches past either end of the strip, and [`Stands`] is what wraps it, exactly as the
/// projection wraps a voxel column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub x: i64,
    pub y: u32,
    pub z: u32,
}

/// The cells one stand occupies, with the part each holds.
///
/// Public so a test can state the geometry without a grid or a world in the way. `y` and
/// `z` beyond the world are returned as they are and dropped on placement: clipping at
/// the faces is the grid's job, and a caller reading this wants the stand's shape.
pub fn parts_of(flora: FloraView<'_>, stand: &Stand, style: u16) -> Vec<(Cell, Part)> {
    let sc = flora.config.species(stand.species);
    let site = stand.site;
    let sx = i64::from(site.x);
    let mut out = Vec::new();
    if stand.stage == Stage::Establishing {
        out.push((Cell { x: sx, y: site.y + 1, z: site.z }, Part::Sprout(style)));
        return out;
    }

    // The trunk: the stand stands `H` cells above its support's top face, and the top
    // one of those is the crown's own centre, so the wood runs to `H − 1`. A stand whose
    // crown height rounds to one is a crown sitting straight on the ground, with no stem
    // — which is what a one-voxel-tall plant is.
    let h = crown_height_voxels(sc.crown_height(stand.wood));
    for k in 1..h {
        out.push((Cell { x: sx, y: site.y + k, z: site.z }, Part::Trunk(style)));
    }

    // The crown: the horizontal disc the shade model covers, at the trunk's top. `z` has
    // real faces and clips; `x` runs on and wraps.
    let top = site.y + h;
    let r = sc.crown_radius(stand.wood).max(0.0);
    let r2 = r * r;
    let span = r.floor() as i64;
    for dz in -span..=span {
        for dx in -span..=span {
            if (dx * dx + dz * dz) as f64 > r2 {
                continue;
            }
            let z = i64::from(site.z) + dz;
            if z < 0 {
                continue;
            }
            out.push((
                Cell { x: sx + dx, y: top, z: z as u32 },
                Part::Crown { style, heart: dx == 0 && dz == 0 },
            ));
        }
    }
    out
}

/// Trunk cells for a model crown height in voxels: rounded, and never zero.
pub fn crown_height_voxels(height: f64) -> u32 {
    if !height.is_finite() {
        return 1;
    }
    (height.round().max(1.0) as u32).min(u32::from(u16::MAX))
}

/// One stand's colours: species palette, then crown fill, then wilt.
pub fn style_of(flora: FloraView<'_>, stand: &Stand) -> Style {
    let sc = flora.config.species(stand.species);
    let (wood, crown, heart) = match stand.species {
        Species::Bloomcrown => (BLOOM_WOOD_SRGB, BLOOM_CROWN_SRGB, BLOOM_HEART_SRGB),
        Species::Umbrellafrond => (FROND_WOOD_SRGB, FROND_CROWN_SRGB, FROND_HEART_SRGB),
    };
    let (wood, crown, heart) = (srgb_linear(wood), srgb_linear(crown), srgb_linear(heart));

    // Crown fill: `P / P_cap`. A stand that has shed its foliage keeps its structure,
    // so an empty crown leans to the wood colour rather than to black.
    let cap = sc.alpha * stand.wood;
    let fill = if cap > 0.0 { (stand.foliage / cap).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let crown = mix(mix(crown, wood, CROWN_BARE), crown, fill);
    let heart = mix(crown, heart, HEART_TINT * fill);

    // Wilt: `μ` is the moisture the model last read, so `1 − μ` is how dry the stand is.
    let wilt = 1.0 - stand.moisture.clamp(0.0, 1.0) as f32;
    Style {
        wood: wilted(wood, wilt),
        crown: wilted(crown, wilt),
        heart: wilted(heart, wilt),
    }
}

/// Desaturate toward the colour's own luminance and darken a little: the one change that
/// reads as thirst at four pixels across without inventing a third hue.
fn wilted(c: [f32; 3], wilt: f32) -> [f32; 3] {
    if wilt <= 0.0 {
        return c;
    }
    let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let grey = mix(c, [l, l, l], WILT_DESAT * wilt);
    let k = 1.0 - WILT_DARK * wilt;
    [grey[0] * k, grey[1] * k, grey[2] * k]
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
    use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site};

    fn world() -> World {
        let mut world = World::empty(Config { width: 32, height: 16, depth: 4, ..Config::default() });
        for z in 0..4 {
            for x in 0..32 {
                for y in 0..=3 {
                    world.apply(VoxelCommand::SetMaterial { x, y, z, material: Material::Soil });
                }
            }
        }
        world
    }

    /// A seeded stand becomes a trunk of `round(crown_height)` cells standing on the
    /// support's top face, with its crown disc one cell thick at the trunk's top and
    /// centred on it. The cells are the shade model's crown columns, not a sprite.
    #[test]
    fn a_stand_becomes_a_trunk_of_cells_under_a_horizontal_crown_disc() {
        let world = world();
        let mut flora = Flora::new(FloraConfig::default());
        let sp = Species::Umbrellafrond;
        let wood = flora.config().species(sp).wood_max;
        assert!(flora.apply(&world, Command::Seed { x: 10, z: 2, species: sp, wood }));

        let view = world.view();
        let mut stands = Stands::empty(32, 16, 4);
        stands.rebuild(&view, flora.view());

        let sc = flora.config().species(sp);
        let h = crown_height_voxels(sc.crown_height(wood));
        assert_eq!(h, 5, "umbrellafrond's full crown height is 5 voxels");
        // The support is the soil skyline at y = 3, so the trunk runs y = 4..=8.
        for y in 4..3 + h {
            assert!(
                matches!(stands.at(10, i64::from(y), 2), Part::Trunk(_)),
                "y = {y} must be trunk"
            );
        }
        // The crown's centre replaces the trunk's top cell, and the disc is one cell
        // thick: nothing above it, nothing below it but trunk.
        assert!(
            matches!(stands.at(10, i64::from(3 + h), 2), Part::Crown { heart: true, .. }),
            "the crown centres on the trunk top"
        );
        assert_eq!(stands.at(10, i64::from(4 + h), 2), Part::None, "the crown is one cell thick");

        // The disc spreads in `x` and `z`, which is what the shade model calls cover.
        let r = sc.crown_radius(wood);
        assert!(r > 2.0, "the fixture wants a disc wider than one cell: {r}");
        assert!(matches!(stands.at(12, i64::from(3 + h), 2), Part::Crown { heart: false, .. }));
        assert!(matches!(stands.at(10, i64::from(3 + h), 0), Part::Crown { heart: false, .. }));
        // And it stops: `dx² + dz² > r²` is outside.
        assert_eq!(stands.at(13, i64::from(3 + h), 2), Part::None, "the disc has an edge");

        // `x` wraps with the ring, and the cells above the world's ceiling are dropped
        // rather than folded back in.
        assert_eq!(stands.at(10 + 32, i64::from(4), 2), stands.at(10, 4, 2));
        assert_eq!(stands.at(10, 99, 2), Part::None);
    }

    /// An establishing stand is a sprout: one cell, and not a block, so the sky above it
    /// stays open and what is behind it still shows.
    #[test]
    fn an_establishing_stand_is_a_single_sprout_cell() {
        let flora = Flora::new(FloraConfig::default());
        let stand = Stand {
            site: Site { x: 4, y: 3, z: 1 },
            species: Species::Bloomcrown,
            stage: Stage::Establishing,
            wood: 0.01,
            foliage: 0.0,
            reserve: 0.0,
            light: 0.0,
            moisture: 1.0,
            water_m3: 0.0,
            mineral: 0.0,
        };
        let parts = parts_of(flora.view(), &stand, 0);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0, Cell { x: 4, y: 4, z: 1 });
        assert_eq!(parts[0].1, Part::Sprout(0));
        assert!(!parts[0].1.is_block());
    }

    /// A cell that terrain has taken is not painted: a stand whose support was buried
    /// does not draw inside rock.
    #[test]
    fn a_buried_cell_holds_no_part() {
        let mut world = world();
        let mut flora = Flora::new(FloraConfig::default());
        let sp = Species::Bloomcrown;
        let wood = flora.config().species(sp).wood_max;
        assert!(flora.apply(&world, Command::Seed { x: 6, z: 1, species: sp, wood }));
        world.apply(VoxelCommand::SetMaterial { x: 6, y: 4, z: 1, material: Material::Rock });

        let view = world.view();
        let mut stands = Stands::empty(32, 16, 4);
        stands.rebuild(&view, flora.view());
        assert_eq!(stands.at(6, 4, 1), Part::None, "the buried trunk cell is not drawn");
    }

    /// The two species are warm and cool, a bare crown falls back toward its own wood,
    /// and a wilting stand dulls without losing which species it is.
    #[test]
    fn the_palettes_are_warm_and_cool_and_fill_and_wilt_move_them() {
        let flora = Flora::new(FloraConfig::default());
        let make = |sp: Species, foliage: f64, moisture: f64| -> Style {
            let sc = flora.config().species(sp);
            style_of(
                flora.view(),
                &Stand {
                    site: Site { x: 0, y: 0, z: 0 },
                    species: sp,
                    stage: Stage::Alive,
                    wood: sc.wood_max,
                    foliage,
                    reserve: 0.0,
                    light: 0.0,
                    moisture,
                    water_m3: 0.0,
                    mineral: 0.0,
                },
            )
        };
        let full = |sp: Species| {
            let sc = flora.config().species(sp);
            make(sp, sc.alpha * sc.wood_max, 1.0)
        };

        let bloom = full(Species::Bloomcrown);
        let frond = full(Species::Umbrellafrond);
        assert!(bloom.crown[0] > bloom.crown[2], "bloomcrown is warm: {:?}", bloom.crown);
        assert!(frond.crown[2] > frond.crown[0], "umbrellafrond is cool: {:?}", frond.crown);
        assert!(bloom.heart[0] > bloom.crown[0], "the bloom heart is the warmer pixel");

        // An empty crown leans to the wood: structure, not a dark canopy.
        let bare = make(Species::Bloomcrown, 0.0, 1.0);
        let d = |a: [f32; 3], b: [f32; 3]| (0..3).map(|i| (a[i] - b[i]).abs()).sum::<f32>();
        assert!(
            d(bare.crown, bare.wood) < d(bloom.crown, bloom.wood),
            "a bare crown must sit nearer its own wood"
        );

        // Wilt dulls and darkens, and stops short of grey: the species must survive it.
        let dry = make(Species::Umbrellafrond, frond_cap(&flora), 0.0);
        let sat = |c: [f32; 3]| {
            let (lo, hi) = c.iter().fold((f32::MAX, 0.0f32), |(l, h), &v| (l.min(v), h.max(v)));
            hi - lo
        };
        assert!(sat(dry.crown) < sat(frond.crown), "a dry stand is duller");
        assert!(sat(dry.crown) > sat(frond.crown) * 0.3, "but not grey: {:?}", dry.crown);
        assert!(dry.crown[2] > dry.crown[0], "and still cool: {:?}", dry.crown);
    }

    fn frond_cap(flora: &Flora) -> f64 {
        let sc = flora.config().species(Species::Umbrellafrond);
        sc.alpha * sc.wood_max
    }
}
