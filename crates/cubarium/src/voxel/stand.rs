//! Turning a [`FloraView`]'s stands into voxels the presenter can draw, and the six
//! species' palettes — five of which are decided and one of which, the glowcap's, is
//! explicitly **interim** (see `GLOWCAP_INTERIM_CAP_SRGB`).
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
//! A site whose [`cubarium_voxel_flora::Ground`] holds a **seed cohort** is a single
//! **sprout** cell just above its support: dormant propagule material, not yet a plant.
//! Round 3 moved the glyph there from the deleted `Stage::Establishing`, so the picture
//! still shows waiting propagules. A site can hold a bank *and* a living stand, and the
//! stand's own cells outrank the sprout, so the mark shows only where the gap is open.
//!
//! Whole voxels are the picture's own quantisation. The model's crown top is a float and
//! its disc a float radius; the drawing rounds both, so a stand's drawn top can sit half
//! a voxel from the height the shade model used. Pixel art has no finer answer at 4 px
//! per voxel.

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FloraView, Species, Stand};

use crate::present::{mix, srgb_linear};

// --- The five palettes ---------------------------------------------------------------
//
// All five live in the Outrun family of `design/appearance.md`, as the strata do — deep
// indigo and violet, electric blue-cyan, magenta, one warm accent — and the two originals
// are separated the way `art/PLANTS.md` separates the same two names: bloomcrown warm —
// violet wood under a magenta canopy with a warm bloom heart — and umbrellafrond cool —
// deep teal wood under a turquoise frond with a mint heart. Warm against cool is the
// one distinction that survives a 4-pixel-wide trunk.
//
// The three round-4 producers take the three remaining directions of that family, so that
// every crown is its own hue and no two of the five sit next to each other:
//
// - **springturf** electric blue — the producer cyan of the appearance doc, on an indigo
//   wood. A one-cell turf, so what has to read at a glance is the single bright pixel.
// - **stonecushion** pale stone-lilac — the only *low-chroma* palette of the five, which
//   is how a mineral crust reads beside four saturated plants. Its distinction is chroma
//   and value rather than hue, and that survives being one cell wide.
// - **velvetpad** deep violet — saturated indigo-violet with a periwinkle heart, the
//   understory floor under somebody else's canopy.
//
// `the_five_palettes_are_distinct_and_the_three_new_crowns_stamp_a_cell` in this module's
// tests keeps every pair apart in linear light.

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

/// Springturf wood: indigo, barely seen — a turf's crown sits straight on the ground, so
/// this shows only as the colour a shed crown falls back toward.
pub const TURF_WOOD_SRGB: u32 = 0x002E_4FB5;
/// Springturf canopy: the appearance doc's producer cyan, `#42C5F8`.
pub const TURF_CROWN_SRGB: u32 = 0x0042_C5F8;
/// Springturf's centre: pale ice.
pub const TURF_HEART_SRGB: u32 = 0x00BF_EBFF;

/// Stonecushion wood: grey-violet, the mineral end of the family.
pub const CUSHION_WOOD_SRGB: u32 = 0x0057_506E;
/// Stonecushion canopy: pale stone-lilac — the one low-chroma crown of the five.
pub const CUSHION_CROWN_SRGB: u32 = 0x00B9_A8D6;
/// Stonecushion's centre: near-white lilac.
pub const CUSHION_HEART_SRGB: u32 = 0x00EF_E6FF;

/// Velvetpad wood: deep indigo-violet.
pub const PAD_WOOD_SRGB: u32 = 0x002B_1B6B;
/// Velvetpad canopy: electric violet.
pub const PAD_CROWN_SRGB: u32 = 0x007B_5CF0;
/// Velvetpad's centre: periwinkle.
pub const PAD_HEART_SRGB: u32 = 0x00C3_B4FF;

// --- The glowcap's interim glyph -----------------------------------------------------
//
// **A placeholder, and named one.** The art direction of the voxel world is Wrysk's own
// thread (`design/handoffs/voxel-art-direction-handoff-2026-09-17.md`), which will produce
// `design/voxel-art-direction.md`; the agent-made consumer study at 68a8215 is **paused and
// not canon** and is deliberately not implemented here. Until that doc lands, a glowcap is
// one cell in one placeholder colour on the face above its support, and a small follow-up
// package replaces this with whatever the art direction specifies.
//
// The colour is chosen for one reason only — to be unmistakably *not* one of the five, so
// that a fungus in a screenshot is legible as a sixth thing. Acid yellow-green is the one
// direction of the Outrun family none of the five producers uses, and it is the farthest of
// the candidates measured from all five crowns in linear light (0.70 to umbrellafrond's
// turquoise, the nearest of them, against the five's own closest pair at 0.45):
// `the_interim_glowcap_glyph_is_one_cell_in_its_own_placeholder_colour` in this module's
// tests pins the distance and the single cell.

/// Glowcap mycelium: a dim olive. A one-cell stand has no trunk, so this shows only as the
/// colour a spent cap falls back toward. **Interim.**
pub const GLOWCAP_INTERIM_WOOD_SRGB: u32 = 0x004A_5A2E;
/// The glowcap's cap: acid yellow-green, the one hue none of the five producers holds.
/// **Interim** — a placeholder glyph colour, not an art-direction decision.
pub const GLOWCAP_INTERIM_CAP_SRGB: u32 = 0x00C8_F03C;
/// The cap's centre, which for a one-cell stand is the whole of it: pale bioluminescent
/// green. **Interim.**
pub const GLOWCAP_INTERIM_HEART_SRGB: u32 = 0x00EF_FFC0;

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

// --- Substrate & Dead organic matter palettes ----------------------------------------
pub const LOG_RIM_SRGB: u32 = 0x007A_4A6E;
pub const LOG_FRONT_SRGB: u32 = 0x004E_2846;
pub const LOG_DARK_SRGB: u32 = 0x003A_1C36;
pub const LOG_TOP_SRGB: u32 = 0x006E_3E62;

pub const LITTER_SRGB: u32 = 0x003D_1E38;
pub const LITTER_FLECKS_SRGB: u32 = 0x00A0_5580;

pub const CARRION_SRGB: u32 = 0x002A_1628;
pub const CARRION_BONE_SRGB: u32 = 0x008A_7C9A;

/// A fallen log's style: dark bark wood, flat wood top, cut-end belly.
pub fn log_style() -> Style {
    Style {
        wood: srgb_linear(LOG_FRONT_SRGB),
        crown: srgb_linear(LOG_TOP_SRGB),
        heart: srgb_linear(LOG_DARK_SRGB),
    }
}

/// Leaf litter detritus style.
pub fn litter_style() -> Style {
    Style {
        wood: srgb_linear(LITTER_SRGB),
        crown: srgb_linear(LITTER_FLECKS_SRGB),
        heart: srgb_linear(LITTER_SRGB),
    }
}

/// Carrion / skeletal remnants style.
pub fn carrion_style() -> Style {
    Style {
        wood: srgb_linear(CARRION_SRGB),
        crown: srgb_linear(CARRION_BONE_SRGB),
        heart: srgb_linear(CARRION_BONE_SRGB),
    }
}

/// What a voxel holds, if anything: which part of which drawn stand or substrate feature.
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
    /// A site holding a seed cohort: a mark, not a block.
    Sprout(u16),
    /// A fallen log: horizontal dead wood cylinder on the support face.
    Log(u16),
    /// Leaf litter and organic detritus: a subtle textured mark on the floor.
    Litter(u16),
    /// Carrion / skeletal remnants on the floor.
    Carrion(u16),
}

impl Part {
    /// Does this part fill its voxel's faces? Sprouts, litter and carrion do not — they
    /// are marks on the floor of an otherwise empty cell.
    pub fn is_block(self) -> bool {
        matches!(self, Part::Trunk(_) | Part::Crown { .. } | Part::Log(_))
    }

    /// The style this part paints with, if it paints at all.
    pub fn style(self) -> Option<u16> {
        match self {
            Part::None => None,
            Part::Trunk(s)
            | Part::Sprout(s)
            | Part::Log(s)
            | Part::Litter(s)
            | Part::Carrion(s) => Some(s),
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
    /// Every grid index this rebuild wrote, once each: what the next rebuild clears
    /// instead of the whole grid, and what a packer walks instead of every voxel.
    stamped: Vec<u32>,
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
            stamped: Vec::new(),
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
            // Only what the last rebuild stamped: the grid is otherwise all empty.
            for i in self.stamped.drain(..) {
                self.grid[i as usize] = Part::None;
            }
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
        // Dead wood (fallen logs): rendered where dead_wood is above threshold.
        for g in flora.ground {
            if g.dead_wood >= 0.05 {
                let style = self.styles.len().min(u16::MAX as usize) as u16;
                if usize::from(style) != self.styles.len() {
                    break;
                }
                self.styles.push(log_style());
                self.place(
                    view,
                    Cell {
                        x: i64::from(g.site.x),
                        y: g.site.y + 1,
                        z: g.site.z,
                    },
                    Part::Log(style),
                );
            }
        }
        // Leaf litter and organic detritus on the soil.
        for g in flora.ground {
            if g.litter >= 0.05 {
                let style = self.styles.len().min(u16::MAX as usize) as u16;
                if usize::from(style) != self.styles.len() {
                    break;
                }
                self.styles.push(litter_style());
                self.place(
                    view,
                    Cell {
                        x: i64::from(g.site.x),
                        y: g.site.y + 1,
                        z: g.site.z,
                    },
                    Part::Litter(style),
                );
            }
        }
        // Carrion / remains.
        for g in flora.ground {
            if g.carrion >= 0.05 {
                let style = self.styles.len().min(u16::MAX as usize) as u16;
                if usize::from(style) != self.styles.len() {
                    break;
                }
                self.styles.push(carrion_style());
                self.place(
                    view,
                    Cell {
                        x: i64::from(g.site.x),
                        y: g.site.y + 1,
                        z: g.site.z,
                    },
                    Part::Carrion(style),
                );
            }
        }
        // Then the seed banks, also in site order: one sprout mark per site that holds a
        // cohort, in the colours of whichever species' cohorts hold the most there.
        for g in flora.ground {
            let Some(species) = g.seed_species() else {
                continue;
            };
            let style = self.styles.len().min(u16::MAX as usize) as u16;
            if usize::from(style) != self.styles.len() {
                break;
            }
            self.styles.push(seed_style(species));
            self.place(
                view,
                Cell {
                    x: i64::from(g.site.x),
                    y: g.site.y + 1,
                    z: g.site.z,
                },
                Part::Sprout(style),
            );
        }
    }

    /// Write one part, letting the stronger claim keep the cell. Wood beats canopy —
    /// a trunk through another stand's crown is a trunk — and a crown beats a sprout/log.
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
            Part::Litter(_) | Part::Carrion(_) => 1,
            Part::Sprout(_) => 2,
            Part::Log(_) => 3,
            Part::Crown { .. } => 4,
            Part::Trunk(_) => 5,
        };
        if rank(part) >= rank(self.grid[i]) {
            if self.grid[i] == Part::None {
                self.stamped.push(i as u32);
            }
            self.grid[i] = part;
        }
    }

    /// Every cell something stands in, as `(x, y, z)` with `x` wrapped, in no particular
    /// order. A cell may hold [`Part::None`] only if a caller placed one.
    pub fn cells(&self) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
        let (w, h) = (self.width as usize, self.height as usize);
        self.stamped.iter().map(move |&i| {
            let i = i as usize;
            let (col, y) = (i / h, i % h);
            ((col % w) as u32, y as u32, (col / w) as u32)
        })
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

    // A glowcap on dead wood perches on the log (+1 higher base)
    let on_log = stand.species == Species::Glowcap
        && flora
            .ground
            .iter()
            .find(|g| g.site == site)
            .map_or(false, |g| g.dead_wood >= 0.05);
    let base_y = site.y + if on_log { 1 } else { 0 };

    // The interim glowcap glyph is its own shape and not a profile: one stem cell and
    // one cap, on the face or on the log. Unchanged.
    if stand.species == Species::Glowcap {
        let h = crown_height_voxels(sc.crown_height(stand.wood, flora.config.voxel_m));
        let top = if on_log { 2 } else { h };
        for k in 1..top {
            out.push((
                Cell {
                    x: sx,
                    y: base_y + k,
                    z: site.z,
                },
                Part::Trunk(style),
            ));
        }
        if on_log {
            out.push((
                Cell {
                    x: sx,
                    y: base_y + 1,
                    z: site.z,
                },
                Part::Trunk(style),
            ));
        }
        out.push((
            Cell {
                x: sx,
                y: base_y + top,
                z: site.z,
            },
            Part::Crown { style, heart: true },
        ));
        return out;
    }

    // Since the layers package the picture is the **profile**: the trunk cells, then
    // one disc per foliage-bearing layer at that layer's own cell and its own radius,
    // bottom-up. The per-species flourish each glyph already had — bloomcrown's cup,
    // the frond's droop, turf's ragged skyline, the pad's scallop, the cushion's
    // lobes — belongs to the **crown**, so it is applied to the topmost foliage layer
    // and the layers under it are plain discs. On a single-layer species that is the
    // glyph it always drew, cell for cell.
    //
    // Interim glyphs throughout, as `WORKING_POLICY.md`'s art-direction correction
    // requires: nothing here is an art decision.
    let layers = flora.profile_layers(stand);
    let foliage: Vec<&cubarium_voxel_flora::StandLayer> =
        layers.iter().filter(|l| l.kind.bears_foliage()).collect();
    let top_cell = foliage
        .iter()
        .map(|l| l.cell)
        .max()
        .unwrap_or(i64::from(site.y) + 1);
    let stem_to = (top_cell - i64::from(site.y)).max(0) as u32;

    // The stem: every cell under the highest disc, which is what `1..h` always drew,
    // plus whatever a `Trunk` layer claims above or beside it.
    for k in 1..stem_to {
        out.push((
            Cell {
                x: sx,
                y: base_y + k,
                z: site.z,
            },
            Part::Trunk(style),
        ));
    }
    for layer in layers.iter().filter(|l| !l.kind.bears_foliage()) {
        for cell_y in layer.cells.0..=layer.cells.1 {
            let k = (cell_y - i64::from(site.y)).max(0) as u32;
            out.push((
                Cell {
                    x: sx,
                    y: base_y + k,
                    z: site.z,
                },
                Part::Trunk(style),
            ));
        }
    }

    let h = crown_height_voxels(sc.crown_height(stand.wood, flora.config.voxel_m));
    for (i, layer) in foliage.iter().enumerate() {
        let crown = i + 1 == foliage.len();
        let top = base_y + (layer.cell - i64::from(site.y)).max(0) as u32;
        let r = layer.radius_v.max(0.0);
        let r2 = r * r;
        let span = r.floor() as i64;
        let lobe_shift = if stand.id & 1 == 0 { 0.5 } else { -0.5 };
        for dz in -span..=span {
            for dx in -span..=span {
                let dist2 = (dx * dx + dz * dz) as f64;
                if dist2 > r2 {
                    continue;
                }
                let z = i64::from(site.z) + dz;
                if z < 0 {
                    continue;
                }
                let mut crown_y = top;
                if crown {
                    match stand.species {
                        // A broad rim around a lowered centre reads as a cup from the
                        // side-on view. It never rises beyond the model crown height or
                        // spreads past its cover; small crowns remain a single cap.
                        Species::Bloomcrown => {
                            let inner = (r - 0.85).max(0.45);
                            let notch = h > 1
                                && r >= 1.0
                                && dist2 <= inner * inner
                                && (dx + dz + stand.id as i64).rem_euclid(5) != 0;
                            crown_y = top - u32::from(notch);
                        }
                        // A narrow stalk opens into a broad parasol. Only sparse rim
                        // cells droop, so it keeps a continuous, readable canopy.
                        Species::Umbrellafrond => {
                            let rim = dist2 > (r - 0.75).max(0.5).powi(2);
                            let droops = rim
                                && (dx - dz + stand.id as i64).rem_euclid(3) == 0
                                && top > base_y + 1;
                            crown_y = top - u32::from(droops);
                        }
                        // Low tufts have a ragged skyline, deterministically varied by
                        // the stand identity so growth does not shimmer between frames.
                        Species::Springturf => {
                            let short = dist2 > 0.5
                                && (dx * 3 + dz * 5 + stand.id as i64).rem_euclid(3) == 0;
                            crown_y = top.saturating_sub(u32::from(short)).max(base_y + 1);
                        }
                        // Thin mats keep a broad scalloped outline: omit alternating rim
                        // cells, never interior cells, and retain the model's bounds.
                        Species::Velvetpad => {
                            let rim = dist2 > (r - 0.8).max(0.0).powi(2);
                            if rim && (dx - dz + stand.id as i64).rem_euclid(3) == 0 {
                                continue;
                            }
                        }
                        // Two shallow lobes, stepped at their centres, a compact dome.
                        Species::Stonecushion => {
                            let left = (dx as f64 + lobe_shift).powi(2) + (dz as f64).powi(2);
                            let right = (dx as f64 - lobe_shift).powi(2) + (dz as f64).powi(2);
                            let core = left.min(right);
                            let raised = core < (r * 0.5).max(0.45).powi(2) && h > 1;
                            crown_y = top + u32::from(raised);
                        }
                        Species::Glowcap => {}
                    }
                }
                out.push((
                    Cell {
                        x: sx + dx,
                        y: crown_y,
                        z: z as u32,
                    },
                    Part::Crown {
                        style,
                        // One heart per stand, in the crown: a lower layer's centre is
                        // its own tissue, not the plant's accent.
                        heart: crown && dx == 0 && dz == 0,
                    },
                ));
            }
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

/// The three sRGB hex colours of a species, before fill or wilt move them.
fn palette(species: Species) -> (u32, u32, u32) {
    match species {
        Species::Bloomcrown => (BLOOM_WOOD_SRGB, BLOOM_CROWN_SRGB, BLOOM_HEART_SRGB),
        Species::Umbrellafrond => (FROND_WOOD_SRGB, FROND_CROWN_SRGB, FROND_HEART_SRGB),
        Species::Springturf => (TURF_WOOD_SRGB, TURF_CROWN_SRGB, TURF_HEART_SRGB),
        Species::Stonecushion => (CUSHION_WOOD_SRGB, CUSHION_CROWN_SRGB, CUSHION_HEART_SRGB),
        Species::Velvetpad => (PAD_WOOD_SRGB, PAD_CROWN_SRGB, PAD_HEART_SRGB),
        // Interim, and named so: see the block above the constants.
        Species::Glowcap => (
            GLOWCAP_INTERIM_WOOD_SRGB,
            GLOWCAP_INTERIM_CAP_SRGB,
            GLOWCAP_INTERIM_HEART_SRGB,
        ),
    }
}

/// A seed cohort's colours: its species' palette, unmoved. A dormant cohort has no
/// foliage to fill a crown with and no `μ` to wilt by — the sprout mark is drawn in
/// `crown`, so what it says is only which species is waiting there.
pub fn seed_style(species: Species) -> Style {
    let (wood, crown, heart) = palette(species);
    Style {
        wood: srgb_linear(wood),
        crown: srgb_linear(crown),
        heart: srgb_linear(heart),
    }
}

/// One stand's colours: species palette, then crown fill, then wilt.
pub fn style_of(flora: FloraView<'_>, stand: &Stand) -> Style {
    let sc = flora.config.species(stand.species);
    let (wood, crown, heart) = palette(stand.species);
    let (wood, crown, heart) = (srgb_linear(wood), srgb_linear(crown), srgb_linear(heart));

    // Crown fill: `P / P_cap`. A stand that has shed its foliage keeps its structure,
    // so an empty crown leans to the wood colour rather than to black.
    let cap = sc.alpha * stand.wood;
    let fill = if cap > 0.0 {
        (stand.foliage / cap).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
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
    use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Stage};

    fn world() -> World {
        let mut world = World::empty(Config {
            width: 32,
            height: 16,
            depth: 4,
            ..Config::default()
        });
        for z in 0..4 {
            for x in 0..32 {
                for y in 0..=3 {
                    world.apply(VoxelCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
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
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 10,
                z: 2,
                species: sp,
                wood
            }
        ));

        let view = world.view();
        let mut stands = Stands::empty(32, 16, 4);
        stands.rebuild(&view, flora.view());

        let sc = flora.config().species(sp);
        let h = crown_height_voxels(sc.crown_height(wood, flora.config().voxel_m));
        // Package L's ladder: a full-grown umbrellafrond is 2 m, eight 0.25 m voxels.
        assert_eq!(h, 8, "umbrellafrond's full crown height is 8 voxels");
        // The support is the soil skyline at y = 3, so the trunk runs y = 4..=11.
        for y in 4..3 + h {
            assert!(
                matches!(stands.at(10, i64::from(y), 2), Part::Trunk(_)),
                "y = {y} must be trunk"
            );
        }
        // The crown's centre replaces the trunk's top cell, and the disc is one cell
        // thick: nothing above it, nothing below it but trunk.
        assert!(
            matches!(
                stands.at(10, i64::from(3 + h), 2),
                Part::Crown { heart: true, .. }
            ),
            "the crown centres on the trunk top"
        );
        assert_eq!(
            stands.at(10, i64::from(4 + h), 2),
            Part::None,
            "the crown is one cell thick"
        );

        // Since the layers package the picture is the **profile**: an adult
        // umbrellafrond is three tiers whose lowest is the widest
        // (`design/organism-anatomy-2026-09-21.md` §3), so the wide disc is no longer
        // at the crown top — the top tier is `r 0.5` of the crown radius and the
        // lowest is the whole of it. That is the shade the ecology is after and the
        // thing the lollipop could not say.
        let r = sc.crown_radius(wood, flora.config().voxel_m);
        assert!(r > 2.0, "the fixture wants a disc wider than one cell: {r}");
        let tiers: Vec<(i64, f64)> = flora
            .view()
            .layers(
                flora
                    .view()
                    .stand_at(Site { x: 10, y: 3, z: 2 })
                    .expect("the frond"),
            )
            .map(|l| (l.cell, l.radius_v))
            .collect();
        assert_eq!(tiers.len(), 3, "three tiers: {tiers:?}");
        assert!(
            tiers[0].1 > tiers[1].1 && tiers[1].1 > tiers[2].1,
            "the lowest tier is the widest: {tiers:?}"
        );
        let (wide_y, wide_r) = tiers[0];
        assert!(wide_r > 2.0, "and it is the whole crown radius: {wide_r}");
        assert!(matches!(
            stands.at(12, wide_y, 2),
            Part::Crown { heart: false, .. }
        ));
        assert!(matches!(
            stands.at(10, wide_y, 0),
            Part::Crown { heart: false, .. }
        ));
        // And it stops: `dx² + dz² > r²` is outside (the widest tier's radius is the
        // ladder's 0.75 m, three cells).
        assert_eq!(
            stands.at(14, wide_y, 2),
            Part::None,
            "the widest tier has an edge"
        );
        // The top tier is narrow, so two cells out at the crown top is empty air.
        assert_eq!(
            stands.at(12, i64::from(3 + h), 2),
            Part::None,
            "the top tier is half the crown radius"
        );

        // `x` wraps with the ring, and the cells above the world's ceiling are dropped
        // rather than folded back in.
        assert_eq!(stands.at(10 + 32, i64::from(4), 2), stands.at(10, 4, 2));
        assert_eq!(stands.at(10, 99, 2), Part::None);
    }

    /// A site holding a seed cohort draws a sprout: one cell just above its support, and
    /// not a block, so the sky above it stays open and what is behind it still shows.
    /// Round 3 moved this glyph off the deleted `Stage::Establishing` and onto the bank.
    ///
    /// The cohort is made the way the model makes them — a donor's paid package — so the
    /// test reads the presenter against the layer's own state and not a hand-built one.
    /// Voxel round 3b: a donor **saves** for one recipient and sends it a whole package,
    /// so at the placeholder `propagule_rate` the first one leaves after 300 s and not on
    /// the first tick. This fixture raises that rate to 3.0 /s so the package is away in
    /// one tick, which is what the presenter is being asked about.
    #[test]
    fn a_site_holding_a_seed_cohort_is_a_single_sprout_cell() {
        let mut world = world();
        let mut config = FloraConfig::default();
        config.bloomcrown.propagule_rate = 3.0;
        let mut flora = Flora::new(config);
        let sp = Species::Bloomcrown;
        let wood = flora.config().species(sp).wood_max;
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 6,
                z: 1,
                species: sp,
                wood
            }
        ));
        flora.step(&mut world);

        let banks: Vec<Site> = flora
            .view()
            .ground
            .iter()
            .filter(|g| g.seed_species().is_some())
            .map(|g| g.site)
            .collect();
        assert!(
            !banks.is_empty(),
            "one tick of a full-grown donor should have seeded a bank"
        );
        assert!(
            !banks.contains(&Site { x: 6, y: 3, z: 1 }),
            "a donor does not seed itself"
        );

        let view = world.view();
        let mut stands = Stands::empty(32, 16, 4);
        stands.rebuild(&view, flora.view());
        for site in banks {
            let part = stands.at(i64::from(site.x), i64::from(site.y) + 1, site.z);
            assert!(
                matches!(part, Part::Sprout(_)),
                "no sprout at {site:?}: {part:?}"
            );
            assert!(!part.is_block(), "a sprout is a mark, not a block");
            // Two cells up is empty: a bank is one cell and never a stem.
            assert_eq!(
                stands.at(i64::from(site.x), i64::from(site.y) + 2, site.z),
                Part::None
            );
            let style = stands.style(part).expect("a sprout paints");
            assert_eq!(
                style,
                seed_style(Species::Bloomcrown),
                "the bank's own species"
            );
        }
    }

    /// Two species' cohorts can share one site now, and the picture has one glyph per
    /// site: the species holding the most organic matter there is the one drawn.
    #[test]
    fn a_shared_bank_draws_the_species_that_holds_the_most() {
        use cubarium_voxel_flora::{Ground, SeedCohort};
        let mut g = Ground::new(Site { x: 1, y: 2, z: 0 }, 0.0);
        assert_eq!(g.seed_species(), None, "an empty bank draws nothing");
        g.seeds.push(SeedCohort {
            species: Species::Bloomcrown,
            organic: 0.01,
            mineral: 0.0002,
            bin_start_tick: 0,
        });
        g.seeds.push(SeedCohort {
            species: Species::Umbrellafrond,
            organic: 0.03,
            mineral: 0.0006,
            bin_start_tick: 0,
        });
        assert_eq!(
            g.seed_species(),
            Some(Species::Umbrellafrond),
            "the larger bank"
        );
        g.seeds[0].organic = 0.05;
        assert_eq!(
            g.seed_species(),
            Some(Species::Bloomcrown),
            "and now the other one"
        );
    }

    /// A cell that terrain has taken is not painted: a stand whose support was buried
    /// does not draw inside rock.
    #[test]
    fn a_buried_cell_holds_no_part() {
        let mut world = world();
        let mut flora = Flora::new(FloraConfig::default());
        let sp = Species::Bloomcrown;
        let wood = flora.config().species(sp).wood_max;
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 6,
                z: 1,
                species: sp,
                wood
            }
        ));
        world.apply(VoxelCommand::SetMaterial {
            x: 6,
            y: 4,
            z: 1,
            material: Material::Rock,
        });

        let view = world.view();
        let mut stands = Stands::empty(32, 16, 4);
        stands.rebuild(&view, flora.view());
        assert_eq!(
            stands.at(6, 4, 1),
            Part::None,
            "the buried trunk cell is not drawn"
        );
    }

    /// The two species are warm and cool, a bare crown falls back toward its own wood,
    /// and a wilting stand dulls without losing which species it is.
    #[test]
    fn the_palettes_are_warm_and_cool_and_fill_and_wilt_move_them() {
        let flora = Flora::new(FloraConfig::default());
        let make = |sp: Species, foliage: f64, moisture: f64| -> Style {
            let sc = flora.config().species(sp);
            style_of(flora.view(), &{
                let mut stand = Stand {
                    id: 0,
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
                    aeration_stress: 0.0,
                    parcel: 0.0,
                    layer_stock: [0.0; cubarium_voxel_flora::MAX_FOLIAGE_LAYERS],
                    profile_stage: 0,
                };
                // A hand-built stand bins its own foliage, as every stand the model
                // makes does.
                stand.bin_layers(flora.config());
                stand
            })
        };
        let full = |sp: Species| {
            let sc = flora.config().species(sp);
            make(sp, sc.alpha * sc.wood_max, 1.0)
        };

        let bloom = full(Species::Bloomcrown);
        let frond = full(Species::Umbrellafrond);
        assert!(
            bloom.crown[0] > bloom.crown[2],
            "bloomcrown is warm: {:?}",
            bloom.crown
        );
        assert!(
            frond.crown[2] > frond.crown[0],
            "umbrellafrond is cool: {:?}",
            frond.crown
        );
        assert!(
            bloom.heart[0] > bloom.crown[0],
            "the bloom heart is the warmer pixel"
        );

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
            let (lo, hi) = c
                .iter()
                .fold((f32::MAX, 0.0f32), |(l, h), &v| (l.min(v), h.max(v)));
            hi - lo
        };
        assert!(sat(dry.crown) < sat(frond.crown), "a dry stand is duller");
        assert!(
            sat(dry.crown) > sat(frond.crown) * 0.3,
            "but not grey: {:?}",
            dry.crown
        );
        assert!(
            dry.crown[2] > dry.crown[0],
            "and still cool: {:?}",
            dry.crown
        );
    }

    fn frond_cap(flora: &Flora) -> f64 {
        let sc = flora.config().species(Species::Umbrellafrond);
        sc.alpha * sc.wood_max
    }

    /// Round 4's three producers: each one stamps **at least one cell** at both ends of its
    /// own size range, its sprout mark is its own, and every one of the five crowns is
    /// distinct from the other four in linear light.
    ///
    /// The three new crowns are one cell tall by design — on package L's ladder they are
    /// 0.125–0.1875 m, which `crown_height_voxels()` rounds to 1 at 0.25 m — so they have **no
    /// trunk at all**: the crown disc sits straight on the ground, which is what a turf, a
    /// cushion and a pad are. That is the case the geometry has to get right, because the
    /// trunk loop is `1..h` and an `h` of 1 runs it zero times.
    #[test]
    fn the_five_palettes_are_distinct_and_the_three_new_crowns_stamp_a_cell() {
        let world = world();
        let flora = Flora::new(FloraConfig::default());

        // Geometry, at `alive_min` and at `wood_max`: one cell or more, always a heart, and
        // never a trunk cell for the three ground-level species.
        for species in [
            Species::Springturf,
            Species::Stonecushion,
            Species::Velvetpad,
        ] {
            let sc = flora.config().species(species);
            for wood in [sc.alive_min, sc.wood_max] {
                let mut stand = Stand {
                    id: 0,
                    site: Site { x: 10, y: 3, z: 2 },
                    species,
                    stage: Stage::Alive,
                    wood,
                    foliage: sc.alpha * wood,
                    reserve: 0.0,
                    light: 1.0,
                    moisture: 1.0,
                    water_m3: 0.0,
                    mineral: 0.0,
                    aeration_stress: 0.0,
                    parcel: 0.0,
                    layer_stock: [0.0; cubarium_voxel_flora::MAX_FOLIAGE_LAYERS],
                    profile_stage: 0,
                };
                stand.bin_layers(flora.config());
                let parts = parts_of(flora.view(), &stand, 0);
                assert_eq!(
                    crown_height_voxels(sc.crown_height(wood, flora.config().voxel_m)),
                    1,
                    "{} at wood {wood} is not one cell tall",
                    species.name()
                );
                let trunks = parts
                    .iter()
                    .filter(|(_, p)| matches!(p, Part::Trunk(_)))
                    .count();
                let crowns: Vec<&Cell> = parts
                    .iter()
                    .filter(|(_, p)| matches!(p, Part::Crown { .. }))
                    .map(|(c, _)| c)
                    .collect();
                assert_eq!(
                    trunks,
                    0,
                    "{}: a stemless plant grew a stem",
                    species.name()
                );
                assert!(
                    !crowns.is_empty(),
                    "{} at wood {wood} stamped no cell at all",
                    species.name()
                );
                assert!(
                    crowns.iter().all(|c| c.y == 4),
                    "{}: the crown left the ground: {crowns:?}",
                    species.name()
                );
                assert_eq!(
                    parts
                        .iter()
                        .filter(|(_, p)| matches!(p, Part::Crown { heart: true, .. }))
                        .count(),
                    1,
                    "{}: one heart and no more",
                    species.name()
                );
            }
        }
        // And the shapes at full size, on package L's ladder at 0.25 m: springturf's
        // 0.25 m radius draws the five-cell plus, stonecushion's 0.156 m stays one cell,
        // and the pad's 0.375 m is the broad one — the role's own "low and broad".
        let cells = |species: Species| -> usize {
            let sc = flora.config().species(species);
            let mut stand = Stand {
                id: 0,
                site: Site { x: 10, y: 3, z: 2 },
                species,
                stage: Stage::Alive,
                wood: sc.wood_max,
                foliage: sc.alpha * sc.wood_max,
                reserve: 0.0,
                light: 1.0,
                moisture: 1.0,
                water_m3: 0.0,
                mineral: 0.0,
                aeration_stress: 0.0,
                parcel: 0.0,
                layer_stock: [0.0; cubarium_voxel_flora::MAX_FOLIAGE_LAYERS],
                profile_stage: 0,
            };
            stand.bin_layers(flora.config());
            // **Distinct cells**, not parts: since the layers package two layers of one
            // profile may claim the same cell — stonecushion's dome is a wide skirt and
            // a narrow cap, and at this size both round to the first cell over the face
            // — and what this test is about is the silhouette the grid ends up with.
            let parts = parts_of(flora.view(), &stand, 0);
            let mut cells: Vec<(i64, u32, u32)> =
                parts.iter().map(|(c, _)| (c.x, c.y, c.z)).collect();
            cells.sort_unstable();
            cells.dedup();
            cells.len()
        };
        let (turf, cushion, pad) = (
            cells(Species::Springturf),
            cells(Species::Stonecushion),
            cells(Species::Velvetpad),
        );
        assert!(
            turf >= 1 && cushion >= 1 && pad >= 1,
            "turf {turf}, cushion {cushion}, pad {pad}"
        );
        assert!(
            turf > cushion,
            "the turf is wider than the cushion on the ladder: {turf} against {cushion}"
        );
        assert!(
            pad > turf,
            "the pad must be the broad one: {pad} against {turf}"
        );

        // A stand of each of the five actually reaches the grid, on its own support face.
        let mut flora = Flora::new(FloraConfig::default());
        for (x, species) in Species::ALL.into_iter().enumerate() {
            let sc = flora.config().species(species);
            let wood = sc.wood_max;
            assert!(flora.apply(
                &world,
                Command::Seed {
                    x: x as i64 * 4,
                    z: 1,
                    species,
                    wood
                }
            ));
        }
        let view = world.view();
        let mut stands = Stands::empty(32, 16, 4);
        stands.rebuild(&view, flora.view());
        for (x, species) in Species::ALL.into_iter().enumerate() {
            let part = stands.at(x as i64 * 4, 4, 1);
            assert!(
                part.is_block(),
                "{} stamped nothing at all: {part:?}",
                species.name()
            );
            let style = stands.style(part).expect("it paints");
            assert_eq!(
                style,
                style_of(
                    flora.view(),
                    flora
                        .view()
                        .stand_at(Site {
                            x: x as u32 * 4,
                            y: 3,
                            z: 1
                        })
                        .expect("seeded")
                )
            );
        }

        // The palettes: every pair of the five crowns apart in linear light, and the same
        // for the sprout marks, which are the unmoved palette.
        let crowns: Vec<(Species, [f32; 3])> = Species::ALL
            .into_iter()
            .map(|s| (s, seed_style(s).crown))
            .collect();
        let dist = |a: [f32; 3], b: [f32; 3]| {
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        };
        let mut closest = f32::MAX;
        for (i, (sa, a)) in crowns.iter().enumerate() {
            for (sb, b) in crowns.iter().skip(i + 1) {
                let d = dist(*a, *b);
                assert!(
                    d > 0.25,
                    "{} and {} are {d} apart in linear light",
                    sa.name(),
                    sb.name()
                );
                closest = closest.min(d);
            }
        }
        // Measured: the closest pair of the five is stonecushion's stone-lilac against
        // velvetpad's violet, at 0.44. Pinned as a floor, not as a value, so a repaint has
        // room to move but not to collapse two species into one colour.
        assert!(
            closest > 0.4,
            "the closest pair of the five is {closest} apart"
        );

        // And the three new ones land where the module doc says: springturf blue-dominant,
        // velvetpad blue-dominant but far darker in green, stonecushion the low-chroma one.
        let chroma = |c: [f32; 3]| {
            let (lo, hi) = c
                .iter()
                .fold((f32::MAX, 0.0f32), |(l, h), &v| (l.min(v), h.max(v)));
            hi - lo
        };
        let turf = seed_style(Species::Springturf).crown;
        let cushion = seed_style(Species::Stonecushion).crown;
        let pad = seed_style(Species::Velvetpad).crown;
        assert!(
            turf[2] > turf[0] && turf[2] > turf[1],
            "springturf is blue: {turf:?}"
        );
        assert!(
            pad[2] > pad[0] && pad[2] > pad[1],
            "velvetpad is violet-blue: {pad:?}"
        );
        assert!(
            turf[1] > pad[1] * 2.0,
            "the cyan and the violet must part in green"
        );
        for other in [
            turf,
            pad,
            seed_style(Species::Bloomcrown).crown,
            seed_style(Species::Umbrellafrond).crown,
        ] {
            assert!(
                chroma(cushion) < chroma(other),
                "stonecushion must be the low-chroma one: {} against {}",
                chroma(cushion),
                chroma(other)
            );
        }
    }

    /// **The glowcap's interim glyph.** One cell at every size — no trunk, one crown cell,
    /// and that cell is the disc's heart, so what a fungus is in the picture is a single
    /// pixel cluster on the face above its support. And one placeholder colour that is
    /// unmistakably not one of the five producers': the nearest of them in linear light is
    /// umbrellafrond's turquoise at **0.70**, against the five's own closest pair
    /// (stonecushion and velvetpad) at 0.45.
    ///
    /// Named interim in the code and pinned here as interim: the art direction of the voxel
    /// world is its own thread, the paused study at 68a8215 is not implemented, and a
    /// follow-up package replaces this glyph with whatever `design/voxel-art-direction.md`
    /// specifies. What this test protects until then is only that a glowcap is *legible as a
    /// sixth thing* and occupies exactly one cell.
    #[test]
    fn the_interim_glowcap_glyph_is_one_cell_in_its_own_placeholder_colour() {
        let world = world();
        let mut flora = Flora::new(FloraConfig::default());
        let sc = flora.config().species(Species::Glowcap).clone();

        // The geometry, at both ends of its size range.
        for wood in [sc.alive_min, sc.wood_max] {
            let mut stand = Stand {
                id: 0,
                site: Site { x: 10, y: 3, z: 2 },
                species: Species::Glowcap,
                stage: Stage::Alive,
                wood,
                foliage: sc.alpha * wood,
                reserve: 0.0,
                light: 0.0,
                moisture: 1.0,
                water_m3: 0.0,
                mineral: 0.0,
                aeration_stress: 0.0,
                parcel: 0.0,
                layer_stock: [0.0; cubarium_voxel_flora::MAX_FOLIAGE_LAYERS],
                profile_stage: 0,
            };
            stand.bin_layers(flora.config());
            let parts = parts_of(flora.view(), &stand, 0);
            assert_eq!(
                parts.len(),
                1,
                "a glowcap at wood {wood} is not one cell: {parts:?}"
            );
            assert_eq!(
                parts[0].0,
                Cell { x: 10, y: 4, z: 2 },
                "not on its support's own face"
            );
            assert_eq!(
                parts[0].1,
                Part::Crown {
                    style: 0,
                    heart: true
                },
                "{parts:?}"
            );
            assert_eq!(crown_height_voxels(sc.crown_height(wood, flora.config().voxel_m)), 1);
        }

        // And on the grid, through the whole presenter path.
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 7,
                z: 1,
                species: Species::Glowcap,
                wood: sc.wood_max
            }
        ));
        let view = world.view();
        let mut stands = Stands::empty(32, 16, 4);
        stands.rebuild(&view, flora.view());
        let part = stands.at(7, 4, 1);
        assert!(part.is_block(), "the cap stamped nothing: {part:?}");
        assert_eq!(
            stands.at(7, 5, 1),
            Part::None,
            "a cap is one cell and never a stem"
        );
        let style = stands.style(part).expect("it paints");
        let standing = flora
            .view()
            .stand_at(Site { x: 7, y: 3, z: 1 })
            .expect("seeded");
        assert_eq!(
            style,
            style_of(flora.view(), standing),
            "the cell paints its own stand"
        );
        // A founder is planted with `moisture` 0 until its first tick, so the *drawn*
        // colour is the interim cap wilted; the palette entry itself is the constant, which
        // is what `seed_style` reads and what the distance below is measured on.
        assert_eq!(
            seed_style(Species::Glowcap).crown,
            srgb_linear(GLOWCAP_INTERIM_CAP_SRGB)
        );

        // Distinct from all five producers' crowns in linear light, with room to spare.
        let dist = |a: [f32; 3], b: [f32; 3]| {
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        };
        let cap = seed_style(Species::Glowcap).crown;
        let mut nearest = f32::MAX;
        for species in Species::ALL {
            if species == Species::Glowcap {
                continue;
            }
            let d = dist(cap, seed_style(species).crown);
            assert!(d > 0.4, "the interim cap is {d} from {}", species.name());
            nearest = nearest.min(d);
        }
        assert!(
            nearest > 0.6,
            "the nearest producer crown is {nearest} away"
        );
    }
}
