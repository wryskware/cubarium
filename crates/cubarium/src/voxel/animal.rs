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
use cubarium_voxel_fauna::{Animal, FaunaConfig, FaunaView, Founder, Species, State};

use crate::present::srgb_linear;

use super::appearance::{self, GlyphId};
use super::colours;
use super::model::{self, ModelCell, ModelLibrary, Tag};
use super::stand::{Cell, Style, wilted};

/// The cells of `animal`'s founder model at its body's length, if the library has one.
fn model_cells<'l>(
    lib: &'l ModelLibrary,
    config: &FaunaConfig,
    animal: &Animal,
) -> Option<&'l [ModelCell]> {
    let founder = animal.founder?;
    let length = config.founder(founder).body_at(animal.body).length_m;
    lib.animal(founder)?.select(length)
}

/// The unit `(dx, dz)` from a climbing body's column into the face it is on: the wall
/// ahead on the way up, the cliff it hangs from (behind the way it walked) on the way
/// down.
fn wall_toward(climb: &cubarium_voxel_fauna::WallClimb) -> (i32, i32) {
    let (dx, dz) = (i32::from(climb.dir.0), i32::from(climb.dir.1));
    if climb.ascending {
        (dx, dz)
    } else {
        (-dx, -dz)
    }
}

/// The interim body colour: a placeholder, and chosen to look like one.
///
/// The five plants own magenta, turquoise, cyan, stone-lilac and violet
/// (`super::stand`), and the strata own indigo and deep violet, so a chartreuse green is
/// the one thing in the picture that can be **nothing but** the placeholder it is. It is
/// not a proposal: see this module's header.
pub const INTERIM_ANIMAL_SRGB: u32 = 0x00A8_FF3C;

/// Littershredder (detritivore crawler) palette:
pub const LITTERSHREDDER_BODY_SRGB: u32 = 0x0025_2B58;
pub const LITTERSHREDDER_RIM_SRGB: u32 = 0x003E_4E7A;
pub const LITTERSHREDDER_FEELER_SRGB: u32 = 0x0042_C5F8;

/// Frondgrazer (browser) palette:
pub const FRONDGRAZER_BODY_SRGB: u32 = 0x001E_2248;
pub const FRONDGRAZER_HEAD_SRGB: u32 = 0x0042_4E88;
pub const FRONDGRAZER_EYE_SRGB: u32 = 0x00D0_F4FF;

/// Interaction accents:
pub const ANIMAL_CROPPING_FLASH_SRGB: u32 = 0x00FF_9B50;
pub const ANIMAL_STARVING_BODY_SRGB: u32 = 0x0028_2834;

/// How far, in voxels, the interim block reaches from the animal's own column: one, in `x`
/// and in `z`, for the 2×1×2 body the brief asks for. The cell it stands in is the void
/// directly above its support face.
pub const INTERIM_SPAN: i64 = 1;

/// What a voxel holds of an animal, if anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimalPart {
    /// No animal here.
    None,
    /// One cell of an interim body block.
    Interim(u16),
    /// An articulated body cell.
    Body {
        style: u16,
        head: bool,
        facing_right: bool,
    },
    /// An articulated head cell.
    Head { style: u16, facing_right: bool },
    /// One cell of a baked voxel model ([`crate::voxel::model`]): a plain lit block in
    /// its palette material.
    Model(u16),
}

impl AnimalPart {
    /// Does this part fill its voxel's faces?
    pub fn is_block(self) -> bool {
        matches!(
            self,
            AnimalPart::Interim(_)
                | AnimalPart::Body { .. }
                | AnimalPart::Head { .. }
                | AnimalPart::Model(_)
        )
    }

    /// The style this part paints with, if it paints at all.
    pub fn style(self) -> Option<u16> {
        match self {
            AnimalPart::None => None,
            AnimalPart::Interim(s)
            | AnimalPart::Body { style: s, .. }
            | AnimalPart::Head { style: s, .. }
            | AnimalPart::Model(s) => Some(s),
        }
    }

    /// The resolved, backend-neutral face glyph for this occupied voxel.
    pub fn glyph(self) -> GlyphId {
        appearance::animal_glyph(self)
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

/// Everything a model cell's colours depend on, as a dense index for the per-rebuild
/// cache.
#[derive(Clone, Copy)]
struct AnimalKey {
    founder: Founder,
    material: u8,
    band: u8,
    accent: bool,
    starving: bool,
    cropping: bool,
}

impl AnimalKey {
    const MATERIALS: usize = model::PALETTE.len();
    const COUNT: usize = 2 * Self::MATERIALS * colours::BANDS as usize * 8;

    fn index(self) -> usize {
        let founder = match self.founder {
            Founder::Blind => 0,
            Founder::Browser => 1,
        };
        let material = usize::from(self.material).min(Self::MATERIALS - 1);
        let mut i = founder * Self::MATERIALS + material;
        i = i * colours::BANDS as usize + usize::from(self.band.min(colours::BANDS - 1));
        i * 8
            + usize::from(self.accent) * 4
            + usize::from(self.starving) * 2
            + usize::from(self.cropping)
    }
}

/// Whether an animal shows as starving: reserve empty and body under 0.015 (D3's
/// starving flag).
pub fn is_starving(animal: &Animal) -> bool {
    animal.reserve <= 0.001 && animal.body <= 0.015
}

/// One animal's style this frame: distinct palettes for founders, cropping flash,
/// starvation desaturation, or interim fallback.
pub fn animal_style(animal: &Animal) -> Style {
    let Some(founder) = animal.founder else {
        return interim_style(animal.species);
    };

    let is_cropping = animal.state == State::Cropping;
    let is_starving = is_starving(animal);

    match founder {
        Founder::Blind => {
            let body = if is_starving {
                ANIMAL_STARVING_BODY_SRGB
            } else {
                LITTERSHREDDER_BODY_SRGB
            };
            let rim = if is_starving {
                0x0038_3C4A
            } else {
                LITTERSHREDDER_RIM_SRGB
            };
            let accent = if is_cropping {
                ANIMAL_CROPPING_FLASH_SRGB
            } else if is_starving {
                0x004E_5868
            } else {
                LITTERSHREDDER_FEELER_SRGB
            };
            Style {
                wood: srgb_linear(body),
                crown: srgb_linear(rim),
                heart: srgb_linear(accent),
            }
        }
        Founder::Browser => {
            let body = if is_starving {
                ANIMAL_STARVING_BODY_SRGB
            } else {
                FRONDGRAZER_BODY_SRGB
            };
            let head = if is_starving {
                0x003A_3E52
            } else {
                FRONDGRAZER_HEAD_SRGB
            };
            let accent = if is_cropping {
                ANIMAL_CROPPING_FLASH_SRGB
            } else if is_starving {
                0x0068_7280
            } else {
                FRONDGRAZER_EYE_SRGB
            };
            Style {
                wood: srgb_linear(body),
                crown: srgb_linear(head),
                heart: srgb_linear(accent),
            }
        }
    }
}

/// The cells one animal occupies, with the part each holds.
///
/// Public so a test can state the geometry without a grid or a world in the way. `x` is
/// **unwrapped** — a body near the seam reaches past the end of the strip and [`Animals`]
/// is what wraps it — and `z` beyond the back wall is returned as it is and dropped on
/// placement, exactly as [`super::stand::parts_of`] does.
pub fn cells_of(
    animal: &Animal,
    config: &FaunaConfig,
    style: u16,
    voxel_m: f64,
) -> Vec<(Cell, AnimalPart)> {
    let site = animal.site;
    let sx = i64::from(site.x);

    let Some(founder) = animal.founder else {
        let mut out = Vec::with_capacity(4);
        for dz in 0..=INTERIM_SPAN {
            for dx in 0..=INTERIM_SPAN {
                out.push((
                    Cell {
                        x: sx + dx,
                        y: site.y + 1,
                        z: (i64::from(site.z) + dz) as u32,
                    },
                    AnimalPart::Interim(style),
                ));
            }
        }
        return out;
    };

    if !(voxel_m > 0.0) || !animal.pose.is_finite() {
        return Vec::new();
    }
    // **The presenter draws the model body** (decisions §1;
    // `design/handoffs/voxel-body-anchors-2026-09-22.md`): the 2x readability shell is
    // retired, and what is drawn is the animal's own dimensions in metres, rounded up
    // to whole cells. On package L's ladder at 0.125 m the adult browser is 6x3x3 and
    // the adult shredder 3x1x1; at 0.25 m they are 3x2x2 and 2x1x1. A juvenile is drawn
    // smaller, because it is smaller.
    let body = config.founder(founder).body_at(animal.body);
    let length = ((body.length_m / voxel_m).ceil() as i64).max(1);
    let width = ((body.width_m / voxel_m).ceil() as i64).max(1);
    let height = ((body.height_m / voxel_m).ceil() as u32).max(1);
    let (fx, fz) = animal.pose.forward();
    let (step_x, step_z) = if fx.abs() >= fz.abs() {
        (if fx >= 0.0 { 1 } else { -1 }, 0)
    } else {
        (0, if fz >= 0.0 { 1 } else { -1 })
    };
    let (side_x, side_z) = (-step_z, step_x);
    let anchor_x = (animal.pose.x / voxel_m).floor() as i64;
    let anchor_z = (animal.pose.z / voxel_m).floor() as i64;
    let facing_right = fx >= 0.0;
    let mut out = Vec::new();

    match founder {
        Founder::Blind if animal.mobility.climb.is_some() => {
            // On a wall (package mobility, interim): the crawler stood on end in the air
            // column beside the face, head up the face or down it.
            let climb = animal.mobility.climb.expect("a body on a wall");
            let feet = i64::from(animal.sense_layer(voxel_m)) + 1;
            for along in 0..length {
                let y = if climb.ascending {
                    feet + along
                } else {
                    feet - along
                };
                if y < 0 {
                    continue;
                }
                let cell = Cell {
                    x: anchor_x,
                    y: y as u32,
                    z: anchor_z.max(0) as u32,
                };
                let part = if along + 1 == length {
                    AnimalPart::Head {
                        style,
                        facing_right,
                    }
                } else {
                    AnimalPart::Body {
                        style,
                        head: false,
                        facing_right,
                    }
                };
                out.push((cell, part));
            }
        }
        Founder::Blind => {
            // Low, tapered, segmented crawler.  Its head is the first shell cell in the
            // real forward mouth direction, never a decorative cell beyond the probe.
            for along in 0..length {
                for across in 0..width {
                    let lateral = across - width / 2;
                    let cell = Cell {
                        x: anchor_x + along * step_x + lateral * side_x,
                        y: site.y + 1,
                        z: (anchor_z + along * step_z + lateral * side_z) as u32,
                    };
                    let part = if along + 1 == length {
                        AnimalPart::Head {
                            style,
                            facing_right,
                        }
                    } else {
                        AnimalPart::Body {
                            style,
                            head: false,
                            facing_right,
                        }
                    };
                    out.push((cell, part));
                }
            }
        }
        Founder::Browser => {
            // Broad browser: a raised two-layer torso with only end legs below it leaves
            // a real projected gap under the belly.  The head remains on the mouth's own
            // reachable crown layer rather than climbing above its model contact.
            for along in 0..length {
                for across in 0..width {
                    let lateral = across - width / 2;
                    let x = anchor_x + (along - length / 2) * step_x + lateral * side_x;
                    let z = (anchor_z + (along - length / 2) * step_z + lateral * side_z) as u32;
                    let front = along + 1 == length;
                    out.push((
                        Cell {
                            x,
                            y: site.y + height,
                            z,
                        },
                        if front {
                            AnimalPart::Head {
                                style,
                                facing_right,
                            }
                        } else {
                            AnimalPart::Body {
                                style,
                                head: along + 2 == length,
                                facing_right,
                            }
                        },
                    ));
                    // Legs under the two ends, so the belly between them is a real
                    // projected gap at every drawn length. They run from the ground to
                    // the torso: package L's 0.375 m browser is three cells tall at
                    // 0.125 m, and a one-cell leg left its torso floating.
                    if height > 1 && (along == 0 || along + 1 == length) {
                        for leg_y in 1..height {
                            out.push((
                                Cell {
                                    x,
                                    y: site.y + leg_y,
                                    z,
                                },
                                AnimalPart::Body {
                                    style,
                                    head: false,
                                    facing_right,
                                },
                            ));
                        }
                    }
                }
            }
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
    /// Every grid index this rebuild wrote, once each: what the next rebuild clears
    /// instead of the whole grid, and what a packer walks instead of every voxel.
    stamped: Vec<u32>,
    styles: Vec<Style>,
    /// This rebuild's style index for each `(model material, starving)`, or `u16::MAX`:
    /// the model path's styles are shared across animals.
    model_styles: Vec<u16>,
}

/// How far a starving animal's body falls toward grey and dark: the wilt tint at half
/// strength. Its accent turns D3/D4's `#7A2A78` instead.
pub const STARVING_TINT: f32 = 0.5;

impl Animals {
    /// An empty grid for a world of this shape.
    pub fn empty(width: u32, height: u32, depth: u32) -> Animals {
        Animals {
            width,
            height,
            depth,
            grid: vec![AnimalPart::None; width as usize * height as usize * depth as usize],
            stamped: Vec::new(),
            styles: Vec::new(),
            model_styles: vec![u16::MAX; AnimalKey::COUNT],
        }
    }

    /// Rebuild from an animal view with the **dev-mode glyphs** ([`cells_of`]), or clear
    /// when a run has no animal layer. Reuses the allocation: the presenter calls this
    /// every frame.
    pub fn rebuild(&mut self, view: &VoxelView<'_>, fauna: Option<FaunaView<'_>>) {
        self.rebuild_inner(view, fauna, None);
    }

    /// Rebuild with the **baked models** ([`crate::voxel::model`]): a founder with a
    /// model stamps the size bin nearest its body's length, turned to the nearest quarter
    /// heading; one without draws [`cells_of`] exactly as [`Animals::rebuild`] does.
    pub fn rebuild_with(
        &mut self,
        view: &VoxelView<'_>,
        fauna: Option<FaunaView<'_>>,
        lib: &ModelLibrary,
    ) {
        let lib = lib.serves(view.config.voxel_m).then_some(lib);
        self.rebuild_inner(view, fauna, lib);
    }

    fn rebuild_inner(
        &mut self,
        view: &VoxelView<'_>,
        fauna: Option<FaunaView<'_>>,
        lib: Option<&ModelLibrary>,
    ) {
        let c = view.config;
        if (self.width, self.height, self.depth) != (c.width, c.height, c.depth) {
            *self = Animals::empty(c.width, c.height, c.depth);
        } else {
            // Only what the last rebuild stamped: the grid is otherwise all empty.
            for i in self.stamped.drain(..) {
                self.grid[i as usize] = AnimalPart::None;
            }
            self.styles.clear();
        }
        self.model_styles.fill(u16::MAX);
        let Some(fauna) = fauna else { return };
        // Animals arrive in id order, which is the order the styles are pushed in, so the
        // grid is a pure function of the view and not of any iteration accident. Two
        // animals on one face paint the same cells and the later id wins; which of two
        // identical interim blocks won is not a visible fact.
        for animal in fauna.animals {
            if let Some(cells) = lib.and_then(|lib| model_cells(lib, fauna.config, animal)) {
                if !self.stamp_model(view, animal, c.voxel_m, cells) {
                    break;
                }
                continue;
            }
            let style = self.styles.len().min(u16::MAX as usize) as u16;
            if usize::from(style) != self.styles.len() {
                break; // more than 65 535 animals in one strip: refuse to alias styles.
            }
            self.styles.push(animal_style(animal));
            for (cell, part) in cells_of(animal, fauna.config, style, c.voxel_m) {
                self.place(view, cell, part);
            }
        }
    }

    /// Stamp one animal's model cells. False when the styles would alias.
    fn stamp_model(
        &mut self,
        view: &VoxelView<'_>,
        animal: &Animal,
        voxel_m: f64,
        cells: &[ModelCell],
    ) -> bool {
        if !(voxel_m > 0.0) || !animal.pose.is_finite() {
            return true;
        }
        // A body on a wall is drawn at the layer its feet have reached (package mobility).
        let anchor = Cell {
            x: (animal.pose.x / voxel_m).floor() as i64,
            y: animal.sense_layer(voxel_m) + 1,
            z: (animal.pose.z / voxel_m).floor().max(0.0) as u32,
        };
        let starving = is_starving(animal);
        let cropping = animal.state == State::Cropping;
        let Some(founder) = animal.founder else {
            return true;
        };
        // The body's highest cell: the back is lit and the belly dark.
        let top = cells.iter().map(|m| m.offset[1]).max().unwrap_or(0);
        let mut ok = true;
        let mut stamp = |cell: Cell, m: ModelCell| {
            if !ok {
                return;
            }
            let key = AnimalKey {
                founder,
                material: m.material,
                band: colours::band(m.offset[1], top),
                accent: m.tag == Tag::Accent,
                starving,
                cropping,
            };
            let Some(style) = self.model_style(key) else {
                ok = false;
                return;
            };
            self.place(view, cell, AnimalPart::Model(style));
        };
        match animal.mobility.climb {
            // Interim wall pose: pitched flat against the face, head up or down it.
            Some(climb) => {
                let toward = wall_toward(&climb);
                model::each_animal_cell_on_wall(
                    cells,
                    anchor,
                    toward,
                    climb.ascending,
                    view,
                    &mut stamp,
                )
            }
            None => {
                model::each_animal_cell(cells, anchor, animal.pose.heading_rad, view, &mut stamp)
            }
        }
        ok
    }

    /// The style of one model cell's colour key, shared across animals. The colours are
    /// [`colours::animal`]'s; a starving body takes the wilt tint and its sense patch
    /// the starved accent.
    fn model_style(&mut self, key: AnimalKey) -> Option<u16> {
        let index = key.index();
        let cached = self.model_styles[index];
        if cached != u16::MAX {
            return Some(cached);
        }
        let style = u16::try_from(self.styles.len())
            .ok()
            .filter(|&s| s != u16::MAX)?;
        let sw = colours::animal(
            key.founder,
            key.material,
            key.band,
            key.accent,
            key.cropping,
        );
        let paint = |rgb| {
            let c = srgb_linear(rgb);
            if key.starving {
                wilted(c, STARVING_TINT)
            } else {
                c
            }
        };
        let (wood, crown, heart) = if key.starving && key.accent {
            let c = srgb_linear(colours::STARVED_ACCENT);
            (c, c, c)
        } else {
            (paint(sw.shadow), paint(sw.body), paint(sw.glint))
        };
        self.styles.push(Style { wood, crown, heart });
        self.model_styles[index] = style;
        Some(style)
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
        if self.grid[i] == AnimalPart::None {
            self.stamped.push(i as u32);
        }
        self.grid[i] = part;
    }

    /// Every cell an animal paints, as `(x, y, z)` with `x` wrapped, in no particular
    /// order.
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
    use cubarium_voxel_fauna::{Command, Fauna, FaunaConfig, Founder, Species, StartingStores};

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

    #[test]
    fn founder_shells_scale_with_voxel_metres_and_keep_the_browser_head_at_its_front() {
        let c = VoxelConfig {
            voxel_m: 0.125,
            ..VoxelConfig::default()
        };
        let mut w = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y: 0,
                    z,
                    material: Material::Soil,
                });
            }
        }
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(fauna.apply(
            &w,
            Command::IntroduceFounder {
                x: 6,
                z: 4,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: std::f64::consts::FRAC_PI_2,
            }
        ));
        assert!(fauna.apply(
            &w,
            Command::IntroduceFounder {
                x: 2,
                z: 4,
                founder: Founder::Blind,
                stores: StartingStores::FULL,
                heading_rad: std::f64::consts::FRAC_PI_2,
            }
        ));
        let av = fauna.view();
        let animals = av.animals;
        let browser = cells_of(&animals[0], av.config, 0, c.voxel_m);
        let blind = cells_of(&animals[1], av.config, 1, c.voxel_m);

        // The **model body** on package L's ladder: the adult browser is 0.75 × 0.375 ×
        // 0.375 m, so six cells long, three wide and three tall at 0.125 m, and the adult
        // shredder 0.375 × 0.125 × 0.125 m, so three cells long and one wide.
        assert_eq!(
            browser.len(),
            6 * 3 + 2 * 3 * 2,
            "6×3 raised torso plus two-cell legs under both ends"
        );
        assert_eq!(blind.len(), 3, "three low crawler segments at 0.125 m");
        assert!(
            browser
                .iter()
                .any(|(c, p)| { c.x == 8 && c.y == 3 && matches!(p, AnimalPart::Head { .. }) }),
            "the browser head is the front column, on the torso layer"
        );
        assert!(browser.iter().any(|(c, _)| c.y == 1));
        assert!(browser.iter().all(|(c, _)| c.y <= 3));
        for leg_y in 1..=2 {
            assert!(browser.iter().any(|(c, _)| c.x == 3 && c.y == leg_y));
            assert!(browser.iter().any(|(c, _)| c.x == 8 && c.y == leg_y));
        }
        assert!(
            !browser
                .iter()
                .any(|(c, _)| (4..=7).contains(&c.x) && c.y < 3),
            "the torso leaves a visible belly gap between its legs"
        );
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
