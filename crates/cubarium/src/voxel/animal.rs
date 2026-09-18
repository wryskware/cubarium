//! Turning a [`FaunaView`]'s animals into voxels the presenter can draw — and, until the
//! art direction says otherwise, the **interim glyph** it draws them as.
//!
//! # The interim glyph, and why it is interim
//!
//! A frondgrazer is a 2×1×2 block of voxels in one placeholder colour, standing on the
//! support face the model says it stands on. That is the round's brief
//! (`design/handoffs/voxel-round5bc-consumers-briefs-2026-09-17.md`): **the look of the
//! voxel world is Wrysk's own art-direction thread**, the agent-made consumer study at
//! 68a8215 is paused and not canon, and nothing here proposes a shape or a palette for a
//! consumer. When `design/voxel-art-direction.md` lands, a follow-up package replaces this
//! module's geometry and its one colour with what that document specifies; everything else
//! — the grid, the part/style path, the presenter hook, the GPU part id — stays.
//!
//! So this file deliberately holds no articulation, no facing, no legs, no animation and
//! no sub-tick motion. An animal is on one face per tick
//! (`cubarium_voxel_fauna::step`), and how a body moves between faces is the art thread's
//! question, with `pace-in-body-lengths` applying to it then.
//!
//! # Why an occupancy grid, again
//!
//! The same reason [`super::stand`] builds one: the presenter has no depth buffer, it
//! paints slabs far to near, and that traversal *is* the depth test. A body drawn as one
//! sprite would be wholly in front of or wholly behind everything in the two slabs it
//! spans, and a 2-deep block spans two. Decomposed into voxels, each cell is painted where
//! a solid block at its own `(x, y, z)` would be, so nearer terrain hides the animal and
//! the animal hides farther terrain, for free.
//!
//! The grid is built and read exactly like the plants': one [`AnimalPart`] per voxel over
//! `(z, x, y)`, one [`Style`] per animal, and the presenter reads it per voxel inside its
//! one traversal. An animal is stamped **after** the plant in its cell and before the
//! water, so a grazer standing in a turf covers the turf and a grazer standing in a pool
//! is submerged under the water's blend.

use cubarium_voxel::VoxelView;
use cubarium_voxel_fauna::{Animal, FaunaView, Species};

use crate::present::srgb_linear;

use super::stand::{Cell, Style};

/// The interim body colour: a placeholder, and chosen to look like one.
///
/// The five plants own magenta, turquoise, cyan, stone-lilac and violet
/// (`super::stand`), and the strata own indigo and deep violet, so a chartreuse green is
/// the one thing in the picture that can be **nothing but** the placeholder it is. It is
/// not a proposal: see this module's header.
pub const INTERIM_ANIMAL_SRGB: u32 = 0x00A8_FF3C;

/// How far, in voxels, the interim block reaches from the animal's own column: one, in `x`
/// and in `z`, for the 2×1×2 body the brief asks for. The cell it stands in is the void
/// directly above its support face.
pub const INTERIM_SPAN: i64 = 1;

/// What a voxel holds of an animal, if anything.
///
/// One variant beside `None`, on purpose: the interim glyph has no parts, and a body with
/// parts is the art thread's to define. The index is into [`Animals::styles`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimalPart {
    /// No animal here.
    None,
    /// One cell of an interim body block.
    Interim(u16),
}

impl AnimalPart {
    /// Does this part fill its voxel's faces? The interim block does: it is a block.
    pub fn is_block(self) -> bool {
        matches!(self, AnimalPart::Interim(_))
    }

    /// The style this part paints with, if it paints at all.
    pub fn style(self) -> Option<u16> {
        match self {
            AnimalPart::None => None,
            AnimalPart::Interim(s) => Some(s),
        }
    }
}

/// One animal's colours this frame. The interim glyph is one flat colour, so all three
/// channels of the plants' [`Style`] carry it: the presenter paints the body from `wood`
/// and the shared type is what lets an animal ride the same part/style path a plant does.
pub fn interim_style(species: Species) -> Style {
    let c = srgb_linear(match species {
        Species::Frondgrazer => INTERIM_ANIMAL_SRGB,
    });
    Style {
        wood: c,
        crown: c,
        heart: c,
    }
}

/// The cells one animal occupies, with the part each holds.
///
/// Public so a test can state the geometry without a grid or a world in the way. `x` is
/// **unwrapped** — a body near the seam reaches past the end of the strip and [`Animals`]
/// is what wraps it — and `z` beyond the back wall is returned as it is and dropped on
/// placement, exactly as [`super::stand::parts_of`] does.
pub fn cells_of(animal: &Animal, style: u16) -> Vec<(Cell, AnimalPart)> {
    let site = animal.site;
    let mut out = Vec::with_capacity(4);
    for dz in 0..=INTERIM_SPAN {
        for dx in 0..=INTERIM_SPAN {
            out.push((
                Cell {
                    x: i64::from(site.x) + dx,
                    // The body stands on the face, so it fills the void above it.
                    y: site.y + 1,
                    z: (i64::from(site.z) + dz) as u32,
                },
                AnimalPart::Interim(style),
            ));
        }
    }
    out
}

/// Every animal of a frame, as voxels plus the styles they paint with.
///
/// Indexed `(z · width + x) · height + y`, the roof map's and [`super::stand::Stands`]'s
/// layout, so the presenter's inner loop touches this grid the same way it touches those.
pub struct Animals {
    width: u32,
    height: u32,
    depth: u32,
    grid: Vec<AnimalPart>,
    styles: Vec<Style>,
}

impl Animals {
    /// An empty grid for a world of this shape.
    pub fn empty(width: u32, height: u32, depth: u32) -> Animals {
        Animals {
            width,
            height,
            depth,
            grid: vec![AnimalPart::None; width as usize * height as usize * depth as usize],
            styles: Vec::new(),
        }
    }

    /// Rebuild from an animal view, or clear when a run has no animal layer. Reuses the
    /// allocation: the presenter calls this every frame.
    pub fn rebuild(&mut self, view: &VoxelView<'_>, fauna: Option<FaunaView<'_>>) {
        let c = view.config;
        if (self.width, self.height, self.depth) != (c.width, c.height, c.depth) {
            *self = Animals::empty(c.width, c.height, c.depth);
        } else {
            self.grid.fill(AnimalPart::None);
            self.styles.clear();
        }
        let Some(fauna) = fauna else { return };
        // Animals arrive in id order, which is the order the styles are pushed in, so the
        // grid is a pure function of the view and not of any iteration accident. Two
        // animals on one face paint the same cells and the later id wins; which of two
        // identical interim blocks won is not a visible fact.
        for animal in fauna.animals {
            let style = self.styles.len().min(u16::MAX as usize) as u16;
            if usize::from(style) != self.styles.len() {
                break; // more than 65 535 animals in one strip: refuse to alias styles.
            }
            self.styles.push(interim_style(animal.species));
            for (cell, part) in cells_of(animal, style) {
                self.place(view, cell, part);
            }
        }
    }

    /// Write one cell, unless it is outside the world or inside solid terrain: a body does
    /// not paint inside rock, the same rule a stand's cells obey.
    fn place(&mut self, view: &VoxelView<'_>, cell: Cell, part: AnimalPart) {
        if cell.y >= self.height || cell.z >= self.depth {
            return;
        }
        if view.material_at(cell.x, cell.y, cell.z).is_solid() {
            return;
        }
        let i = self.index(cell.x, cell.y, cell.z);
        self.grid[i] = part;
    }

    #[inline]
    fn index(&self, x: i64, y: u32, z: u32) -> usize {
        let xw = x.rem_euclid(i64::from(self.width)) as usize;
        (z as usize * self.width as usize + xw) * self.height as usize + y as usize
    }

    /// What stands in a voxel. `x` wraps; a `y` or `z` outside the world is empty.
    #[inline]
    pub fn at(&self, x: i64, y: i64, z: u32) -> AnimalPart {
        if y < 0 || y >= i64::from(self.height) || z >= self.depth || self.grid.is_empty() {
            return AnimalPart::None;
        }
        self.grid[self.index(x, y as u32, z)]
    }

    /// The colours a part paints with.
    #[inline]
    pub fn style(&self, part: AnimalPart) -> Option<Style> {
        part.style().map(|s| self.styles[usize::from(s)])
    }

    /// Whether anything at all is drawn. The presenter skips its per-voxel lookup entirely
    /// on a world with no animals, which is every frame of a run with no fauna.
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
    use cubarium_voxel_fauna::{Command, Fauna, FaunaConfig, Species};

    use super::*;

    fn world() -> World {
        let c = VoxelConfig {
            width: 8,
            height: 6,
            depth: 3,
            voxel_m: 1.0,
            ..VoxelConfig::default()
        };
        let mut w = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                for y in 1..=2 {
                    w.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        w
    }

    /// The interim glyph is four cells — 2 in `x`, 1 in `y`, 2 in `z` — in the void above
    /// the face the model puts the animal on, and it paints in one placeholder colour.
    #[test]
    fn the_interim_glyph_is_a_two_by_one_by_two_block_over_its_support_face() {
        let w = world();
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(fauna.apply(
            &w,
            Command::Introduce {
                x: 3,
                z: 1,
                species: Species::Frondgrazer,
                body: 0.02
            }
        ));
        let mut grid = Animals::empty(0, 0, 0);
        grid.rebuild(&w.view(), Some(fauna.view()));
        assert!(!grid.is_empty());

        // The support face is the top soil row, `y = 2`, so the body fills `y = 3`.
        for (x, z, held) in [
            (3, 1, true),
            (4, 1, true),
            (3, 2, true),
            (4, 2, true),
            (2, 1, false),
            (5, 1, false),
            (3, 0, false),
        ] {
            assert_eq!(
                grid.at(x, 3, z).is_block(),
                held,
                "({x}, 3, {z}) should {} be part of the body",
                if held { "" } else { "not" }
            );
        }
        // Nothing in the face itself, nothing above the block.
        assert_eq!(grid.at(3, 2, 1), AnimalPart::None);
        assert_eq!(grid.at(3, 4, 1), AnimalPart::None);

        let style = grid.style(grid.at(3, 3, 1)).expect("the interim style");
        assert_eq!(style, interim_style(Species::Frondgrazer));
        assert_eq!(
            style.wood, style.crown,
            "the interim glyph is one flat colour"
        );
        // And it is none of the five plant palettes.
        for plant in cubarium_voxel_flora::Species::ALL {
            assert_ne!(style.wood, super::super::stand::seed_style(plant).crown);
        }
    }

    /// A body at the seam wraps in `x`, and one against the back wall is clipped in `z`
    /// rather than wrapping into the front.
    #[test]
    fn the_glyph_wraps_in_x_and_clips_at_the_back_wall() {
        let w = world();
        let mut fauna = Fauna::new(FaunaConfig::default());
        // The last column, so the block's second half is column 0.
        assert!(fauna.apply(
            &w,
            Command::Introduce {
                x: 7,
                z: 2,
                species: Species::Frondgrazer,
                body: 0.02
            }
        ));
        let mut grid = Animals::empty(0, 0, 0);
        grid.rebuild(&w.view(), Some(fauna.view()));
        assert!(grid.at(7, 3, 2).is_block());
        assert!(grid.at(0, 3, 2).is_block(), "the body crosses the seam");
        assert!(
            grid.at(8, 3, 2).is_block(),
            "which is the same cell, unwrapped"
        );
        // `z = 3` is outside a 3-deep world, and nothing of it landed at `z = 0`.
        assert_eq!(grid.at(7, 3, 0), AnimalPart::None);
    }

    /// No animal layer is an empty grid and no lookup: the presenter's existing behaviour
    /// for a run without fauna, unchanged.
    #[test]
    fn no_fauna_is_an_empty_grid() {
        let w = world();
        let mut grid = Animals::empty(0, 0, 0);
        grid.rebuild(&w.view(), None);
        assert!(grid.is_empty());
        assert_eq!(grid.at(3, 3, 1), AnimalPart::None);
    }
}
