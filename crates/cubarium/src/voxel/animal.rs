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

use cubarium_gpu::voxel::{HeadMove, PoseEntry};
use cubarium_voxel::VoxelView;
use cubarium_voxel_fauna::{Animal, DT, FaunaConfig, FaunaView, Founder, Species, State, StrikePhase};
use rustc_hash::FxHashMap;

use crate::present::srgb_linear;

use super::appearance::{self, GlyphId};
use super::colours;
use super::model::{self, ModelCell, ModelLibrary, Tag};
use super::hunker;
use super::idle;
use super::sleep;
use super::stand::{Cell, Style, wilted};

/// How many rows a rider is drawn above its own site: its host's drawn height in cells
/// (a founder host's body height rounded up, the interim block's one row otherwise).
/// `None` when the host is not in the view.
pub(super) fn rider_lift(fauna: &FaunaView<'_>, host: u64, voxel_m: f64) -> Option<u32> {
    let host = fauna.animal(host)?;
    let Some(founder) = host.founder else {
        return Some(1);
    };
    if !(voxel_m > 0.0) {
        return None;
    }
    let height = fauna.config.founder(founder).body_at(host.body).height_m;
    Some(((height / voxel_m).ceil() as u32).max(1))
}

/// The cells of `animal`'s founder model for its mode ([`ModelLibrary::animal_in_mode`]) at
/// its body's length, if the library has one.
pub(super) fn model_cells<'l>(
    lib: &'l ModelLibrary,
    config: &FaunaConfig,
    animal: &Animal,
) -> Option<&'l [ModelCell]> {
    let founder = animal.founder?;
    let length = config.founder(founder).body_at(animal.body).length_m;
    // The model for the body's mode (F3: the seedporter's poses), else its one model.
    lib.animal_in_mode(founder, animal.mobility.mode)?.select(length)
}

/// The cells of `animal`'s baked hunker ([`model::hunker_model`]) in the bin of its body's
/// length (the same bin as its standing model's: the hunker is baked in the founder's
/// bins), if the library has one.
pub(super) fn hunker_cells<'l>(
    lib: &'l ModelLibrary,
    config: &FaunaConfig,
    animal: &Animal,
) -> Option<&'l [ModelCell]> {
    let founder = animal.founder?;
    let length = config.founder(founder).body_at(animal.body).length_m;
    lib.specimen(model::hunker_model(founder)?)?.select(length)
}

/// What a model body is posed by this rebuild: its sleep, idle and hunker weights (each
/// with its rate), and its baked hunker while it hunkers.
#[derive(Clone, Copy, Default)]
struct Poses<'l> {
    asleep: Option<(f32, f32)>,
    idling: Option<(f32, f32)>,
    hunkering: Option<(f32, f32)>,
    toward: Option<&'l [ModelCell]>,
}

/// The unit `(dx, dz)` from a climbing body's column into the face it is on: the wall
/// ahead on the way up, the cliff it hangs from (behind the way it walked) on the way
/// down.
pub use cubarium_session::view_state::wall_toward;

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

/// Interaction accents. The cropping flash is the colour an eating animal's accent leans
/// toward, by at most `VoxelConfig::meal_flash` ([`MEAL_FLASH_DEFAULT`]).
pub const ANIMAL_CROPPING_FLASH_SRGB: u32 = 0x00FF_9B50;

/// `VoxelConfig::meal_flash` by default: how far an eating animal's accent leans toward
/// its flash colour at the height of the flash, `0..=1`. Subtle: a warm tint on the eye
/// or sense patch, not a light.
pub const MEAL_FLASH_DEFAULT: f32 = 0.2;

/// Seconds the meal flash takes to ease in once an animal starts cropping, and to ease out
/// once it stops. Presentation only.
pub const MEAL_FLASH_FADE_S: f64 = 0.6;

/// Steps of the flash's eased weight that model styles are keyed by: fine enough that one
/// step at full strength moves the accent by `meal_flash / FLASH_LEVELS` of the way.
const FLASH_LEVELS: u8 = 8;
pub const ANIMAL_STARVING_BODY_SRGB: u32 = 0x0028_2834;

/// Lanternjaw (ambush carnivore) interim palette — stage-1 looks, no taste call; its body
/// plan is open (`design/predation-and-flight-2026-09-28.md` §5): a dark teal box, an ember
/// jaw, and the lit lure's one warm emissive voxel at the tail tip.
pub const LANTERNJAW_BODY_SRGB: u32 = 0x0014_2A2E;
pub const LANTERNJAW_BACK_SRGB: u32 = 0x0030_5A58;
pub const LANTERNJAW_JAW_SRGB: u32 = 0x00FF_5A3C;
pub const LANTERNJAW_LURE_SRGB: u32 = 0x00FF_E27A;
/// A carcass is its body's colour at this brightness, lying flat.
pub const CARCASS_DARKEN: f32 = 0.35;
/// How many cells a carcass's footprint follows the ground up or down from its site's
/// support: each cell lies on its own column's ground within this band.
pub const CARCASS_DRAPE: u32 = 3;
/// Remains (below 30 % of the body, §2.5): one small pale glyph.
pub const REMAINS_SRGB: u32 = 0x00D8_D0C0;
/// A wounded body takes the starving tint at half strength (the brief's item 11).
pub const WOUNDED_TINT: f32 = 0.5 * STARVING_TINT;

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
    /// The meal flash's eased weight in [`FLASH_LEVELS`] steps, 0 for none.
    flash: u8,
    wounded: bool,
}

impl AnimalKey {
    const MATERIALS: usize = model::PALETTE.len();
    const FLASHES: usize = FLASH_LEVELS as usize + 1;
    const COUNT: usize =
        Founder::COUNT * Self::MATERIALS * colours::BANDS as usize * 8 * Self::FLASHES;

    fn index(self) -> usize {
        let founder = self.founder.index();
        let material = usize::from(self.material).min(Self::MATERIALS - 1);
        let mut i = founder * Self::MATERIALS + material;
        i = i * colours::BANDS as usize + usize::from(self.band.min(colours::BANDS - 1));
        let flags = usize::from(self.wounded) * 4
            + usize::from(self.accent) * 2
            + usize::from(self.starving);
        (i * 8 + flags) * Self::FLASHES + usize::from(self.flash.min(FLASH_LEVELS))
    }
}

/// Whether an animal shows as starving: reserve empty and body under 0.015 (D3's
/// starving flag).
pub use cubarium_session::view_state::is_starving;

/// Whether an animal shows as wounded (predation §2.4): any wound it has not healed.
pub fn is_wounded(animal: &Animal) -> bool {
    animal.wound > 0.0
}

/// One animal's style this frame: distinct palettes for founders, cropping flash,
/// starvation desaturation, or interim fallback. A wounded body that is not starving takes
/// the starving tint at half strength. `flash` is how far the accent leans toward
/// [`ANIMAL_CROPPING_FLASH_SRGB`] this frame (the meal flash, eased), `0..=1`.
pub fn animal_style(animal: &Animal, flash: f32) -> Style {
    let style = animal_style_unwounded(animal, flash);
    if is_wounded(animal) && !is_starving(animal) {
        Style {
            wood: wilted(style.wood, WOUNDED_TINT),
            crown: wilted(style.crown, WOUNDED_TINT),
            heart: style.heart,
        }
    } else {
        style
    }
}

fn animal_style_unwounded(animal: &Animal, flash: f32) -> Style {
    let Some(founder) = animal.founder else {
        return interim_style(animal.species);
    };

    let is_starving = is_starving(animal);
    // The accent, leaning toward the flash colour by `flash`.
    let lean = |accent: u32| {
        let (a, f) = (srgb_linear(accent), srgb_linear(ANIMAL_CROPPING_FLASH_SRGB));
        let t = flash.clamp(0.0, 1.0);
        [0, 1, 2].map(|i| a[i] + (f[i] - a[i]) * t)
    };

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
            let accent = if is_starving {
                0x004E_5868
            } else {
                LITTERSHREDDER_FEELER_SRGB
            };
            Style {
                wood: srgb_linear(body),
                crown: srgb_linear(rim),
                heart: lean(accent),
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
            let accent = if is_starving {
                0x0068_7280
            } else {
                FRONDGRAZER_EYE_SRGB
            };
            Style {
                wood: srgb_linear(body),
                crown: srgb_linear(head),
                heart: lean(accent),
            }
        }
        // The lanternjaw's, and the interim glyph colours of a lineage without its own.
        _ => {
            let body = if is_starving {
                ANIMAL_STARVING_BODY_SRGB
            } else {
                LANTERNJAW_BODY_SRGB
            };
            let back = if is_starving {
                0x0038_3C4A
            } else {
                LANTERNJAW_BACK_SRGB
            };
            Style {
                wood: srgb_linear(body),
                crown: srgb_linear(back),
                heart: lean(LANTERNJAW_JAW_SRGB),
            }
        }
    }
}

/// One model cell's colours in linear light (its style: shadow, body, glint) and its
/// emissive colour in sRGB if its material glows: the founder's coat at the cell's material
/// and height band, a sense patch's accent leaning toward the meal flash by `flash`, a
/// starving body wilted (its accent the starved one), a wounded one half as much.
pub(crate) fn model_colours(
    founder: Founder,
    material: u8,
    band: u8,
    accent: bool,
    starving: bool,
    wounded: bool,
    flash: f32,
) -> (Style, Option<u32>) {
    let sw = colours::animal(founder, material, band, accent, flash);
    let paint = |rgb| {
        let c = srgb_linear(rgb);
        if starving {
            wilted(c, STARVING_TINT)
        } else if wounded {
            wilted(c, WOUNDED_TINT)
        } else {
            c
        }
    };
    let (wood, crown, heart) = if starving && accent {
        let c = srgb_linear(colours::STARVED_ACCENT);
        (c, c, c)
    } else {
        (paint(sw.shadow), paint(sw.body), paint(sw.glint))
    };
    (Style { wood, crown, heart }, colours::animal_emission(founder, material, accent))
}

/// A lineage's body colour in linear light, for a carcass: the three glyph lineages'
/// own, and any other lineage's coat body (its first slot's low end).
fn animal_colour(founder: Founder) -> [f32; 3] {
    srgb_linear(match founder.name() {
        "littershredder" => LITTERSHREDDER_BODY_SRGB,
        "frondgrazer" => FRONDGRAZER_BODY_SRGB,
        "lanternjaw" => LANTERNJAW_BODY_SRGB,
        _ => colours::founder_coat(founder).first().map_or(LANTERNJAW_BODY_SRGB, |c| c.1),
    })
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
    cells_of_local(animal, config, style, voxel_m)
        .into_iter()
        .map(|(cell, part, _)| (cell, part))
        .collect()
}

/// [`cells_of`] with each cell's place in the body's own frame ([`sleep::Local`]), for a
/// body that can take the sleep pose: the articulated bodies on the ground. A body on a
/// wall and the interim block carry `None`.
pub fn cells_of_local(
    animal: &Animal,
    config: &FaunaConfig,
    style: u16,
    voxel_m: f64,
) -> Vec<(Cell, AnimalPart, Option<sleep::Local>)> {
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
                    None,
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
                out.push((cell, part, None));
            }
        }
        Founder::Blind => {
            // Low, tapered, segmented crawler.  Its head is the first shell cell in the
            // real forward mouth direction, never a decorative cell beyond the probe.
            let mut local = Vec::new();
            for along in 0..length {
                for across in 0..width {
                    let lateral = across - width / 2;
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
                    local.push((
                        sleep::Local {
                            a: along as i32,
                            c: lateral as i32,
                            row: 0,
                            bump: along + 1 != length,
                        },
                        part,
                    ));
                }
            }
            place_local(&mut out, &local, |a, c, row| Cell {
                x: anchor_x + i64::from(a) * step_x + i64::from(c) * side_x,
                y: site.y + 1 + row as u32,
                z: (anchor_z + i64::from(a) * step_z + i64::from(c) * side_z) as u32,
            });
        }
        Founder::Browser => {
            // Broad browser: a raised two-layer torso with only end legs below it leaves
            // a real projected gap under the belly.  The head remains on the mouth's own
            // reachable crown layer rather than climbing above its model contact.
            let mut local = Vec::new();
            for along in 0..length {
                for across in 0..width {
                    let lateral = across - width / 2;
                    let a = (along - length / 2) as i32;
                    let c = lateral as i32;
                    let front = along + 1 == length;
                    local.push((
                        sleep::Local {
                            a,
                            c,
                            row: height as i32 - 1,
                            bump: !front,
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
                            local.push((
                                sleep::Local {
                                    a,
                                    c,
                                    row: leg_y as i32 - 1,
                                    bump: false,
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
            place_local(&mut out, &local, |a, c, row| Cell {
                x: anchor_x + i64::from(a) * step_x + i64::from(c) * side_x,
                y: site.y + 1 + row as u32,
                z: (anchor_z + i64::from(a) * step_z + i64::from(c) * side_z) as u32,
            });
        }
        // The lanternjaw's, and the glyph of any lineage without its own.
        _ => {
            // Interim (stage 1, its body plan open): a box of its ladder size, the front
            // column the jaw. The strike is a pose offset: a crouch (one row lower) in the
            // windup, a stretch (one cell longer) in the lunge.
            let (length, height) = match animal.strike.map(|s| s.phase) {
                Some(StrikePhase::Windup) => (length, height.saturating_sub(1).max(1)),
                Some(StrikePhase::Lunge) => (length + 1, height),
                _ => (length, height),
            };
            let mut local = Vec::new();
            for along in 0..length {
                for across in 0..width {
                    let a = (along - length / 2) as i32;
                    let c = (across - width / 2) as i32;
                    let front = along + 1 == length;
                    for row in 0..height {
                        let top = row + 1 == height;
                        local.push((
                            sleep::Local {
                                a,
                                c,
                                row: row as i32,
                                bump: top && !front,
                            },
                            if front && top {
                                AnimalPart::Head {
                                    style,
                                    facing_right,
                                }
                            } else {
                                AnimalPart::Body {
                                    style,
                                    head: front,
                                    facing_right,
                                }
                            },
                        ));
                    }
                }
            }
            place_local(&mut out, &local, |a, c, row| Cell {
                x: anchor_x + i64::from(a) * step_x + i64::from(c) * side_x,
                y: site.y + 1 + row as u32,
                z: (anchor_z + i64::from(a) * step_z + i64::from(c) * side_z) as u32,
            });
        }
    }
    out
}

/// Put a glyph body's local cells in the world through `cell_at`.
fn place_local(
    out: &mut Vec<(Cell, AnimalPart, Option<sleep::Local>)>,
    local: &[(sleep::Local, AnimalPart)],
    cell_at: impl Fn(i32, i32, i32) -> Cell,
) {
    for &(l, part) in local {
        out.push((cell_at(l.a, l.c, l.row), part, Some(l)));
    }
}

/// One unsimulated animal placed for drawing ([`Animals::stamp_specimens`]): its name in
/// the model library's `specimens`, the anchor cell (just above its support face, as a
/// founder's), and its heading.
#[derive(Clone, Debug, PartialEq)]
pub struct Specimen {
    pub name: String,
    pub anchor: Cell,
    pub heading_rad: f64,
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
    /// Whether model styles carry emissive colours ([`Animals::set_emission`]): the GPU
    /// renderer's lit tier with emission on.
    emission: bool,
    /// Each style's emissive colour in linear light, beside `styles`; shorter than it, or
    /// `None`, for a style that does not emit ([`colours::animal_emission`]).
    emits: Vec<Option<[f32; 3]>>,
    /// Beside `grid`, per voxel: an animal cell's **pose byte** (its [`sleep::Class`] and
    /// pose entry, 0 for a body not posing), or, in an empty voxel under a settling torso,
    /// the byte of the faceless leg cell the sink stamps there ([`Animals::pose_at`]).
    pose: Vec<u8>,
    /// This rebuild's pose entries, index 0 unused (a cell's entry 0 is no pose); and each
    /// entry's index by its bits, so animals in the same pose share one.
    entries: Vec<PoseEntry>,
    entry_of: FxHashMap<[u32; 12], u8>,
    /// Each animal's sleep weight (0 awake to 1 asleep) and whether it was asleep, by id, at
    /// the last rebuild: presentation memory, so a body that falls asleep or wakes takes
    /// [`sleep::BLEND_S`] over it. Every founder in the last rebuild has an entry, so an
    /// animal not in it is new.
    sleep_blend: FxHashMap<u64, (f32, bool)>,
    /// Each animal's idle memory by id at the last rebuild ([`Idle`]), kept the same way.
    idle_blend: FxHashMap<u64, Idle>,
    /// Each animal's hunker weight (0 standing to 1 clamped down) and whether it was
    /// hunkered, by id, at the last rebuild ([`hunker`]), kept the same way; and the seconds
    /// a hunker takes ([`Animals::set_hunker_s`]).
    hunker_blend: FxHashMap<u64, (f32, bool)>,
    hunker_s: f64,
    /// Each eating (or just-eaten) animal's meal-flash weight by id, 0 to 1, at the last
    /// rebuild: it rises over [`MEAL_FLASH_FADE_S`] while the animal crops and falls back
    /// after, so the flash eases in and out. Kept the same way as the sleep blend.
    flash_blend: FxHashMap<u64, f32>,
    /// `VoxelConfig::meal_flash`: how far the accent leans toward the flash at its height.
    meal_flash: f32,
    /// The tick of the last rebuild, which is what the blend's elapsed time is read from.
    last_tick: Option<u64>,
    /// Every body cell this rebuild wrote and the animal that wrote it, in writing order
    /// (a later write of the same cell wins): picking and the highlight read it
    /// ([`Animals::owner_at`], [`Animals::cells_of`]).
    owners: Vec<(u32, u64)>,
    /// The animal being stamped, or [`NO_OWNER`].
    owner: u64,
}

/// No animal: what carcasses, specimens and marks are stamped under.
const NO_OWNER: u64 = u64::MAX;


/// One animal's idle memory ([`idle`]): its idle weight (0 moving to 1 idling) and whether
/// it was idling, at the last rebuild; the ticks it has stood still; where it stood.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Idle {
    weight: f32,
    idle: bool,
    still: u64,
    at: [f64; 3],
}

/// One posing body's pose: each cell's pose byte (beside its cells), its torso row, the rows
/// its belly is marked (0 unless it sleeps), its entry, and its head's entry and move while
/// the head moves.
struct BodyPose {
    bytes: Vec<u8>,
    torso: i32,
    drop: i32,
    entry: u8,
    head: Option<(u8, HeadMove)>,
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
            emission: false,
            emits: Vec::new(),
            pose: vec![0; width as usize * height as usize * depth as usize],
            entries: Vec::new(),
            entry_of: FxHashMap::default(),
            sleep_blend: FxHashMap::default(),
            idle_blend: FxHashMap::default(),
            hunker_blend: FxHashMap::default(),
            hunker_s: hunker::DEFAULT_S,
            flash_blend: FxHashMap::default(),
            meal_flash: MEAL_FLASH_DEFAULT,
            last_tick: None,
            owners: Vec::new(),
            owner: NO_OWNER,
        }
    }

    /// The seconds a hunker takes to go down or come up (`VoxelConfig::hunker_s`).
    pub fn set_hunker_s(&mut self, seconds: f64) {
        self.hunker_s = seconds.max(0.0);
    }

    /// How far an eating animal's accent leans toward the flash colour at the height of
    /// the flash (`VoxelConfig::meal_flash`), `0..=1`; 0 turns the flash off.
    pub fn set_meal_flash(&mut self, strength: f32) {
        self.meal_flash = if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 0.0 };
    }

    /// One animal's meal flash this rebuild in [`FLASH_LEVELS`] steps: its weight eased by
    /// a smoothstep, so it starts and stops gently. 0 when it is not flashing or the
    /// flash is off.
    pub(crate) fn flash_level(&self, animal: &Animal) -> u8 {
        if self.meal_flash <= 0.0 {
            return 0;
        }
        let w = self.flash_blend.get(&animal.id).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let eased = w * w * (3.0 - 2.0 * w);
        (eased * f32::from(FLASH_LEVELS)).round() as u8
    }

    /// How far a flash level leans the accent toward the flash colour.
    pub(crate) fn flash_amount(&self, level: u8) -> f32 {
        self.meal_flash * f32::from(level) / f32::from(FLASH_LEVELS)
    }

    /// Every animal's sleep and idle weights at this rebuild, moved on from the last one by
    /// the time between them **at the state it had then**, so the weight at this tick is
    /// exactly where the last tick's pose entry extrapolated it to and a frame never jumps.
    /// A body seen for the first time is drawn as it is: one that is asleep when the world
    /// loads is not lowered by a blend nobody watched (and one standing still starts to
    /// idle after [`idle::HOLD_S`], as any other).
    fn advance_poses(&mut self, tick: u64, animals: &[Animal]) {
        let dt_ticks = self.last_tick.map_or(0, |t| tick.saturating_sub(t));
        let dt = dt_ticks as f64 * DT;
        self.last_tick = Some(tick);
        let mut seen: FxHashMap<u64, (f32, bool)> = FxHashMap::default();
        let mut idle: FxHashMap<u64, Idle> = FxHashMap::default();
        let mut hunkers: FxHashMap<u64, (f32, bool)> = FxHashMap::default();
        let mut flashes: FxHashMap<u64, f32> = FxHashMap::default();
        let flash_step = if MEAL_FLASH_FADE_S > 0.0 {
            (dt / MEAL_FLASH_FADE_S) as f32
        } else {
            1.0
        };
        let hold = (idle::HOLD_S / DT).round() as u64;
        for a in animals {
            if a.founder.is_none() {
                continue;
            }
            let asleep = a.state == State::Asleep;
            let blend = match self.sleep_blend.get(&a.id) {
                Some(&(b, was)) => sleep::advance_blend(b, was, dt),
                None if asleep => 1.0,
                None => 0.0,
            };
            seen.insert(a.id, (blend, asleep));
            // HUNKER: down while the body says so (not on a wall, which does not pose).
            let down = a.hunkered && a.mobility.climb.is_none();
            let hunkered = match self.hunker_blend.get(&a.id) {
                Some(&(b, was)) => hunker::advance_blend(b, was, dt, self.hunker_s),
                None if down => 1.0,
                None => 0.0,
            };
            hunkers.insert(a.id, (hunkered, down));
            // The meal flash: up while the body crops, down after; a body first seen
            // cropping starts from nothing and eases in like any other.
            let cropping = a.state == State::Cropping;
            let last = self.flash_blend.get(&a.id).copied().unwrap_or(0.0);
            let flash = if cropping { last + flash_step } else { last - flash_step };
            if flash > 0.0 || cropping {
                flashes.insert(a.id, flash.clamp(0.0, 1.0));
            }
            // Idle: awake, standing (not walking, feeding or climbing), where it stood at
            // the last rebuild, fully woken, and so for the hold.
            let at = [a.pose.x, a.pose.z, a.pose.heading_rad];
            let last = self.idle_blend.get(&a.id);
            // A hunkered body is clamped still under its shield: it does not idle.
            let quiet = a.state == State::Resting
                && a.mobility.climb.is_none()
                && blend == 0.0
                && !down
                && hunkered == 0.0
                && last.is_some_and(|m| m.at == at);
            let still = if quiet { last.map_or(0, |m| m.still) + dt_ticks } else { 0 };
            idle.insert(
                a.id,
                Idle {
                    weight: last.map_or(0.0, |m| idle::advance_blend(m.weight, m.idle, dt)),
                    idle: still >= hold,
                    still,
                    at,
                },
            );
        }
        self.sleep_blend = seen;
        self.idle_blend = idle;
        self.hunker_blend = hunkers;
        self.flash_blend = flashes;
    }

    /// One animal's hunker weight at this rebuild and its change per tick from here, or
    /// `None` for a body standing and staying so (or on a wall, which does not pose).
    pub(crate) fn hunker_weight(&self, animal: &Animal) -> Option<(f32, f32)> {
        if animal.mobility.climb.is_some() {
            return None;
        }
        let &(w, down) = self.hunker_blend.get(&animal.id)?;
        let r = hunker::rate_per_tick(DT, self.hunker_s);
        let rate = if down && w < 1.0 {
            r
        } else if !down && w > 0.0 {
            -r
        } else {
            0.0
        };
        (w > 0.0 || rate != 0.0).then_some((w, rate))
    }

    /// One animal's idle weight at this rebuild and its change per tick from here, or
    /// `None` for a body that is not idling and not fading out of it.
    pub(crate) fn idle_weight(&self, animal: &Animal) -> Option<(f32, f32)> {
        if animal.mobility.climb.is_some() {
            return None;
        }
        let m = self.idle_blend.get(&animal.id)?;
        let r = idle::rate_per_tick(DT);
        let rate = if m.idle && m.weight < 1.0 {
            r
        } else if !m.idle && m.weight > 0.0 {
            -r
        } else {
            0.0
        };
        (m.weight > 0.0 || rate != 0.0).then_some((m.weight, rate))
    }

    /// One animal's sleep weight at this rebuild and its change per tick from here, or
    /// `None` for a body that is awake and staying so (or on a wall, which does not pose).
    pub(crate) fn sleep_weight(&self, animal: &Animal) -> Option<(f32, f32)> {
        if animal.mobility.climb.is_some() {
            return None;
        }
        let &(w, asleep) = self.sleep_blend.get(&animal.id)?;
        let r = sleep::rate_per_tick(DT);
        let rate = if asleep && w < 1.0 {
            r
        } else if !asleep && w > 0.0 {
            -r
        } else {
            0.0
        };
        (w > 0.0 || rate != 0.0).then_some((w, rate))
    }

    /// The pose entry index for `e`: an existing one with the same bits, a new one, or, with
    /// the table full, the nearest (a body a hair off its own pose rather than no pose).
    fn intern(&mut self, e: PoseEntry) -> u8 {
        let key = [
            e.sleep,
            e.sleep_rate,
            e.drop,
            e.sleep_phase,
            e.idle,
            e.idle_rate,
            e.idle_phase,
            e.head.start,
            e.head.ticks,
            e.head.dip,
            e.hunker,
            e.hunker_rate,
        ]
        .map(f32::to_bits);
        if let Some(&i) = self.entry_of.get(&key) {
            return i;
        }
        if self.entries.is_empty() {
            self.entries.push(PoseEntry::ZERO);
        }
        if self.entries.len() < sleep::POSE_ENTRIES {
            let i = self.entries.len() as u8;
            self.entries.push(e);
            self.entry_of.insert(key, i);
            return i;
        }
        let cyc = |a: f32, b: f32| {
            let d = (a - b).abs();
            d.min(1.0 - d)
        };
        let far = |b: &PoseEntry| {
            (e.sleep - b.sleep).abs()
                + 20.0 * (e.sleep_rate - b.sleep_rate).abs()
                + 4.0 * (e.drop - b.drop).abs()
                + cyc(e.sleep_phase, b.sleep_phase)
                + (e.idle - b.idle).abs()
                + 20.0 * (e.idle_rate - b.idle_rate).abs()
                + cyc(e.idle_phase, b.idle_phase)
                + 4.0 * (e.head.dip - b.head.dip).abs()
                + 0.1 * (e.head.start - b.head.start).abs()
                + (e.hunker - b.hunker).abs()
                + 20.0 * (e.hunker_rate - b.hunker_rate).abs()
        };
        (1..self.entries.len())
            .min_by(|&i, &j| far(&self.entries[i]).total_cmp(&far(&self.entries[j])))
            .unwrap_or(1) as u8
    }

    /// This rebuild's pose entries ([`PoseEntry`]); index 0 is unused and holds zeros.
    /// Empty when no animal poses.
    pub fn pose_entries(&self) -> &[PoseEntry] {
        &self.entries
    }

    /// One posing body's pose ([`BodyPose`]) from its cells in its own frame and which of
    /// them are its head: its classes ([`sleep::classify`]), one entry for the body and,
    /// while its head moves ([`idle::move_near`], for a founder that moves its head,
    /// [`idle::moves_head`]), one for the head. A body only idling has its legs where they
    /// stand (its settle is 0), so the classes are the sleep's. A hunkering body
    /// (`hunkering`, its weight and rate) settles as a sleeper does, and with its baked
    /// hunker (`toward`) takes [`hunker::classify`]'s classes.
    #[allow(clippy::too_many_arguments)]
    fn pose_body(
        &mut self,
        animal: &Animal,
        tick: u64,
        local: &[sleep::Local],
        head: &[bool],
        asleep: Option<(f32, f32)>,
        idling: Option<(f32, f32)>,
        hunkering: Option<(f32, f32)>,
        toward: Option<&[ModelCell]>,
    ) -> BodyPose {
        let id = animal.id;
        let k = match (hunkering, toward) {
            (Some(_), Some(target)) => hunker::classify(local, target),
            _ => sleep::classify(local),
        };
        let (sw, sr) = asleep.unwrap_or((0.0, 0.0));
        let (iw, ir) = idling.unwrap_or((0.0, 0.0));
        let (hw, hr) = hunkering.unwrap_or((0.0, 0.0));
        let settles = asleep.is_some() || hunkering.is_some();
        let body = PoseEntry {
            sleep: sw,
            sleep_rate: sr,
            drop: if settles { k.drop as f32 } else { 0.0 },
            sleep_phase: if asleep.is_some() { sleep::phase(id) } else { 0.0 },
            idle: iw,
            idle_rate: ir,
            idle_phase: if idling.is_some() { idle::phase(id) } else { 0.0 },
            head: HeadMove::NONE,
            hunker: hw,
            hunker_rate: hr,
        };
        let entry = self.intern(body);
        let head_move = idling
            .filter(|_| animal.founder.is_some_and(idle::moves_head))
            .and_then(|_| idle::move_near(id, tick, DT))
            .map(|m| m.relative_to(tick));
        let head_entry = head_move.map(|m| (self.intern(PoseEntry { head: m, ..body }), m));
        let bytes = k
            .class
            .iter()
            .zip(head)
            .map(|(class, &is_head)| match (head_entry, is_head) {
                // A head folding away under a hunker keeps folding while it moves.
                (Some((e, _)), true) if *class == Some(sleep::Class::Leg1) => {
                    sleep::Class::Leg1.byte(e)
                }
                (Some((e, _)), true) => sleep::Class::Torso.byte(e),
                _ => class.map_or(0, |c| c.byte(entry)),
            })
            .collect();
        BodyPose {
            bytes,
            torso: k.torso,
            drop: if settles { k.drop } else { 0 },
            entry,
            head: head_entry,
        }
    }

    /// The empty voxels a posing body moves into: under a settling torso (the sleep's
    /// belly, [`Animals::mark`]), and under a dipping head. A head's are faceless
    /// torso-class cells (soft marks): the sink draws a plant or a vine over them, and the
    /// walk counts them as air for everything but finding the head.
    fn mark_pose(&mut self, view: &VoxelView<'_>, torso: &[Cell], head: &[Cell], p: &BodyPose) {
        self.mark(view, torso, p.drop, p.entry);
        let Some((e, m)) = p.head else {
            return;
        };
        let soft = sleep::Class::Torso.byte(e);
        if m.dip <= 0.0 {
            return;
        }
        for c in head {
            // Down by the dip and the settle, two rows at most (as far as the walk looks).
            for dy in 1..=(1 + p.drop).min(2) {
                self.put_mark(view, c.x, i64::from(c.y) - i64::from(dy), c.z, soft);
            }
        }
    }

    /// The pose byte at a voxel: an animal cell's class and entry, or, in an empty voxel
    /// under a settling torso, a faceless leg cell's. `x` wraps; outside the world is 0.
    #[inline]
    pub fn pose_at(&self, x: i64, y: i64, z: u32) -> u8 {
        if y < 0 || y >= i64::from(self.height) || z >= self.depth || self.pose.is_empty() {
            return 0;
        }
        self.pose[self.index(x, y as u32, z)]
    }

    /// Give model styles their emissive colours ([`colours::animal_emission`]) from the
    /// next rebuild on. This is the animal half of the emission path; see
    /// `appearance::animal_emissive_glyph` for how an emitting cell is drawn.
    pub fn set_emission(&mut self, on: bool) {
        self.emission = on;
    }

    /// The emissive colour an animal part's cell glows in, in linear light, or `None`.
    #[inline]
    pub fn emission(&self, part: AnimalPart) -> Option<[f32; 3]> {
        let s = usize::from(part.style()?);
        self.emits.get(s).copied().flatten()
    }

    /// Rebuild from an animal view with the **dev-mode glyphs** ([`cells_of`]), or clear
    /// when a run has no animal layer. Reuses the allocation: the presenter calls this
    /// every frame.
    pub fn rebuild(&mut self, view: &VoxelView<'_>, fauna: Option<FaunaView<'_>>) {
        self.rebuild_inner(view, fauna, None, true);
    }

    /// The model pass's rebuild (H2): the pose and flash memory moved on to this tick for
    /// every animal, and nothing of a living animal stamped (carcasses and specimens are
    /// stamped after, as ever). The figures read the memory through
    /// [`crate::voxel::figure::specs`].
    pub fn rebuild_without_living(&mut self, view: &VoxelView<'_>, fauna: FaunaView<'_>) {
        self.rebuild_inner(view, Some(fauna), None, false);
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
        self.rebuild_inner(view, fauna, lib, true);
    }

    fn rebuild_inner(
        &mut self,
        view: &VoxelView<'_>,
        fauna: Option<FaunaView<'_>>,
        lib: Option<&ModelLibrary>,
        living: bool,
    ) {
        let c = view.config;
        if (self.width, self.height, self.depth) != (c.width, c.height, c.depth) {
            let (emission, hunker_s) = (self.emission, self.hunker_s);
            *self = Animals::empty(c.width, c.height, c.depth);
            self.emission = emission;
            self.hunker_s = hunker_s;
        } else {
            // Only what the last rebuild stamped: the grid is otherwise all empty.
            for i in self.stamped.drain(..) {
                self.grid[i as usize] = AnimalPart::None;
                self.pose[i as usize] = 0;
            }
            self.styles.clear();
            self.emits.clear();
        }
        self.entries.clear();
        self.entry_of.clear();
        self.model_styles.fill(u16::MAX);
        self.owners.clear();
        self.owner = NO_OWNER;
        let Some(fauna) = fauna else {
            self.sleep_blend.clear();
            self.idle_blend.clear();
            self.hunker_blend.clear();
            return;
        };
        self.advance_poses(view.tick, fauna.animals);
        if !living {
            return;
        }
        // Animals arrive in id order, which is the order the styles are pushed in, so the
        // grid is a pure function of the view and not of any iteration accident. Two
        // animals on one face paint the same cells and the later id wins; which of two
        // identical interim blocks won is not a visible fact.
        for animal in fauna.animals {
            self.owner = animal.id;
            // A parasite hatchling riding its host (interim, stage 1: plain voxels, no
            // final look): its own small box, lifted onto the host's back.
            if let Some(lift) = animal
                .attached
                .and_then(|hold| rider_lift(&fauna, hold.host, c.voxel_m))
            {
                let style = self.styles.len().min(u16::MAX as usize) as u16;
                if usize::from(style) != self.styles.len() {
                    break;
                }
                let flash = self.flash_amount(self.flash_level(animal));
                self.styles.push(animal_style(animal, flash));
                for (cell, part, _) in cells_of_local(animal, fauna.config, style, c.voxel_m) {
                    let on_back = Cell {
                        y: cell.y + lift,
                        ..cell
                    };
                    self.place(view, on_back, part);
                }
                continue;
            }
            let weight = self.sleep_weight(animal);
            let idling = self.idle_weight(animal);
            let hunkering = self.hunker_weight(animal);
            if let Some(cells) = lib.and_then(|lib| model_cells(lib, fauna.config, animal)) {
                let toward = hunkering
                    .and(lib)
                    .and_then(|lib| hunker_cells(lib, fauna.config, animal));
                let poses = Poses {
                    asleep: weight,
                    idling,
                    hunkering,
                    toward,
                };
                if !self.stamp_model(view, animal, c.voxel_m, cells, poses) {
                    break;
                }
            } else {
                let style = self.styles.len().min(u16::MAX as usize) as u16;
                if usize::from(style) != self.styles.len() {
                    break; // more than 65 535 animals in one strip: refuse to alias styles.
                }
                let flash = self.flash_amount(self.flash_level(animal));
                self.styles.push(animal_style(animal, flash));
                let cells = cells_of_local(animal, fauna.config, style, c.voxel_m);
                if animal.founder == Some(Founder::Lanternjaw) {
                    self.stamp_lure(view, animal, fauna.config, c.voxel_m);
                }
                let posing = weight.is_some() || idling.is_some() || hunkering.is_some();
                let local: Option<Vec<sleep::Local>> =
                    posing.then(|| cells.iter().map(|(_, _, l)| *l).collect()).flatten();
                match local {
                    Some(local) => {
                        // The glyph's head is its head cells, at the facing end.
                        let head: Vec<bool> = cells
                            .iter()
                            .map(|(_, part, _)| matches!(part, AnimalPart::Head { .. }))
                            .collect();
                        let pose = self.pose_body(
                            animal, view.tick, &local, &head, weight, idling, hunkering, None,
                        );
                        let (mut torso, mut heads) = (Vec::new(), Vec::new());
                        for (((cell, part, l), byte), is_head) in
                            cells.into_iter().zip(&pose.bytes).zip(head)
                        {
                            if self.place_with(view, cell, part, *byte) {
                                if l.is_some_and(|l| l.row == pose.torso) {
                                    torso.push(cell);
                                }
                                if is_head {
                                    heads.push(cell);
                                }
                            }
                        }
                        self.mark_pose(view, &torso, &heads, &pose);
                    }
                    None => {
                        for (cell, part, _) in cells {
                            self.place(view, cell, part);
                        }
                    }
                }
            }
        }
        self.owner = NO_OWNER;
    }

    /// The animal whose body is drawn in voxel `(x, y, z)` (`x` wraps), if any: the last one
    /// written there.
    pub fn owner_at(&self, x: i64, y: u32, z: u32) -> Option<u64> {
        if y >= self.height || z >= self.depth {
            return None;
        }
        let i = self.index(x, y, z) as u32;
        self.owners.iter().rev().find(|(c, _)| *c == i).map(|&(_, id)| id)
    }

    /// Every voxel animal `id`'s body is drawn in, `(x, y, z)` with `x` wrapped; a cell a
    /// later body painted over is left out.
    pub fn cells_of(&self, id: u64) -> Vec<(u32, u32, u32)> {
        let (w, h) = (self.width as usize, self.height as usize);
        self.owners
            .iter()
            .filter(|&&(i, who)| who == id && self.owner_at_index(i) == Some(id))
            .map(|&(i, _)| {
                let i = i as usize;
                let (col, y) = (i / h, i % h);
                ((col % w) as u32, y as u32, (col / w) as u32)
            })
            .collect()
    }

    /// Every drawn body cell and its animal, latest write winning, as
    /// `((x, y, z), id)` with `x` wrapped.
    pub fn owned_cells(&self) -> Vec<((u32, u32, u32), u64)> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut seen = rustc_hash::FxHashSet::default();
        let mut out = Vec::new();
        for &(i, id) in self.owners.iter().rev() {
            if seen.insert(i) {
                let iu = i as usize;
                let (col, y) = (iu / h, iu % h);
                out.push((((col % w) as u32, y as u32, (col / w) as u32), id));
            }
        }
        out
    }

    fn owner_at_index(&self, i: u32) -> Option<u64> {
        self.owners.iter().rev().find(|(c, _)| *c == i).map(|&(_, id)| id)
    }

    /// The empty voxels a settling torso comes down into, `1..=drop` rows under each of its
    /// torso-row cells: faceless leg cells of entry `e`, which the sink stamps over whatever
    /// else is there (a sprout under the belly is hidden while the body sleeps on it).
    fn mark(&mut self, view: &VoxelView<'_>, torso: &[Cell], drop: i32, e: u8) {
        for &cell in torso {
            for dy in 1..=drop {
                if let Some(class) = sleep::Classified::under(dy) {
                    self.put_mark(view, cell.x, i64::from(cell.y) - i64::from(dy), cell.z, class.byte(e));
                }
            }
        }
    }

    fn put_mark(&mut self, view: &VoxelView<'_>, x: i64, y: i64, z: u32, byte: u8) {
        if y < 0 || y >= i64::from(self.height) || z >= self.depth {
            return;
        }
        if view.material_at(x, y as u32, z).is_solid() {
            return;
        }
        let i = self.index(x, y as u32, z);
        if self.grid[i] != AnimalPart::None {
            return;
        }
        if self.pose[i] == 0 {
            self.stamped.push(i as u32);
            self.pose[i] = byte;
        }
    }

    /// Stamp one animal's model cells, posed as `poses` says (asleep, idling or hunkering,
    /// or blending). False when the styles would alias.
    fn stamp_model(
        &mut self,
        view: &VoxelView<'_>,
        animal: &Animal,
        voxel_m: f64,
        cells: &[ModelCell],
        poses: Poses<'_>,
    ) -> bool {
        let Poses {
            asleep: weight,
            idling,
            hunkering,
            toward,
        } = poses;
        if !(voxel_m > 0.0) || !animal.pose.is_finite() {
            return true;
        }
        // Asleep, idling (or blending): each cell's pose byte in the model's own frame, by
        // offset, and which are its head. A body neither stamps with no pose.
        let mut class_of: FxHashMap<[i32; 3], u8> = FxHashMap::default();
        let mut head_of: FxHashMap<[i32; 3], ()> = FxHashMap::default();
        let mut pose = None;
        let mut torso: Vec<Cell> = Vec::new();
        let mut heads: Vec<Cell> = Vec::new();
        if (weight.is_some() || idling.is_some() || hunkering.is_some())
            && animal.mobility.climb.is_none()
        {
            let local: Vec<sleep::Local> = cells
                .iter()
                .map(|m| sleep::Local {
                    a: m.offset[0],
                    c: m.offset[2],
                    row: m.offset[1],
                    bump: m.tag != Tag::Accent,
                })
                .collect();
            let torso_row = sleep::classify(&local).torso;
            let head = idle::head_of_model(&local, torso_row);
            let p = self.pose_body(
                animal, view.tick, &local, &head, weight, idling, hunkering, toward,
            );
            for ((m, &byte), &is_head) in cells.iter().zip(&p.bytes).zip(&head) {
                if byte != 0 {
                    class_of.insert(m.offset, byte);
                }
                if is_head && p.head.is_some() {
                    head_of.insert(m.offset, ());
                }
            }
            pose = Some(p);
        }
        let torso_row = pose.as_ref().map_or(i32::MIN, |p| p.torso);
        // A body on a wall is drawn at the layer its feet have reached (package mobility).
        let anchor = Cell {
            x: (animal.pose.x / voxel_m).floor() as i64,
            y: animal.sense_layer(voxel_m) + 1,
            z: (animal.pose.z / voxel_m).floor().max(0.0) as u32,
        };
        let starving = is_starving(animal);
        let flash = self.flash_level(animal);
        let wounded = is_wounded(animal) && !starving;
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
                flash,
                wounded,
            };
            let Some(style) = self.model_style(key) else {
                ok = false;
                return;
            };
            let byte = class_of.get(&m.offset).copied().unwrap_or(0);
            if self.place_with(view, cell, AnimalPart::Model(style), byte) && byte != 0 {
                if m.offset[1] == torso_row {
                    torso.push(cell);
                }
                if head_of.contains_key(&m.offset) {
                    heads.push(cell);
                }
            }
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
        if ok && let Some(p) = pose {
            self.mark_pose(view, &torso, &heads, &p);
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
        let (style_colours, emit) = model_colours(
            key.founder,
            key.material,
            key.band,
            key.accent,
            key.starving,
            key.wounded,
            self.flash_amount(key.flash),
        );
        self.styles.push(style_colours);
        if self.emission
            && let Some(rgb) = emit
        {
            let s = usize::from(style);
            self.emits.resize(s + 1, None);
            self.emits[s] = Some(srgb_linear(rgb));
        }
        self.model_styles[index] = style;
        Some(style)
    }

    /// Whether model styles carry emissive colours ([`Animals::set_emission`]).
    pub fn emission_on(&self) -> bool {
        self.emission
    }

    /// A style of its own for one flat colour, with its emission when the emissive tier is
    /// on and `emits`. `None` when the styles would alias.
    fn flat_style(&mut self, srgb: [f32; 3], emits: bool) -> Option<u16> {
        let style = u16::try_from(self.styles.len())
            .ok()
            .filter(|&s| s != u16::MAX)?;
        self.styles.push(Style {
            wood: srgb,
            crown: srgb,
            heart: srgb,
        });
        if emits && self.emission {
            let s = usize::from(style);
            self.emits.resize(s + 1, None);
            self.emits[s] = Some(srgb);
        }
        Some(style)
    }

    /// **The lit lure** (predation §2.2, interim): one emissive voxel at a lanternjaw's
    /// tail tip, a row over its back, while its lure is held and it is awake.
    fn stamp_lure(&mut self, view: &VoxelView<'_>, animal: &Animal, config: &FaunaConfig, voxel_m: f64) {
        if !(animal.founder_state.lure > 0.0) || animal.state == State::Asleep {
            return;
        }
        let Some(founder) = animal.founder else { return };
        if !(voxel_m > 0.0) || !animal.pose.is_finite() {
            return;
        }
        let body = config.founder(founder).body_at(animal.body);
        let (fx, fz) = animal.pose.forward();
        let back = 0.5 * body.length_m;
        let x = ((animal.pose.x - back * fx) / voxel_m).floor() as i64;
        let z = ((animal.pose.z - back * fz) / voxel_m).floor();
        if z < 0.0 {
            return;
        }
        let rows = ((body.height_m / voxel_m).ceil() as u32).max(1);
        let Some(style) = self.flat_style(srgb_linear(LANTERNJAW_LURE_SRGB), true) else {
            return;
        };
        let cell = Cell {
            x,
            y: animal.sense_layer(voxel_m) + 1 + rows,
            z: z as u32,
        };
        self.place(view, cell, AnimalPart::Model(style));
    }

    /// **Carcasses** (predation §2.5, interim): every carcass record on the ground drawn
    /// as its dead animal's box lying flat and darkened, or — below 30 % of the body it
    /// died with — as one small pale glyph. Call after [`Animals::rebuild`] or
    /// [`Animals::rebuild_with`]; a living body already on a cell keeps it.
    pub fn stamp_carcasses(
        &mut self,
        view: &VoxelView<'_>,
        flora: &cubarium_voxel_flora::FloraView<'_>,
        config: &FaunaConfig,
    ) {
        let v = view.config.voxel_m;
        if !(v > 0.0) {
            return;
        }
        for g in flora.ground.iter() {
            let Some(carcass) = g.carcass else { continue };
            let Some(&founder) = Founder::ALL.get(usize::from(carcass.lineage)) else {
                continue;
            };
            let phys = config.founder(founder);
            let y = g.site.y + 1;
            let (cx, cz) = (i64::from(g.site.x), i64::from(g.site.z));
            if carcass.remains(g.carrion, flora.config) {
                let Some(style) = self.flat_style(srgb_linear(REMAINS_SRGB), false) else {
                    return;
                };
                self.place_if_empty(view, Cell { x: cx, y, z: g.site.z }, AnimalPart::Model(style));
                continue;
            }
            // The body it died with, sized from its organic matter at the lineage's reserve
            // cap: structure + reserve ≈ body · (1 + reserve_cap).
            let body_organic = (carcass.organic / (1.0 + phys.core.reserve_cap))
                .clamp(phys.core.body_min, phys.core.body_max);
            let body = phys.body_at(body_organic);
            let length = ((body.length_m / v).ceil() as i64).max(1);
            let width = ((body.width_m / v).ceil() as i64).max(1);
            let (fx, fz) = (carcass.heading_rad.sin(), carcass.heading_rad.cos());
            let (step_x, step_z) = if fx.abs() >= fz.abs() {
                (if fx >= 0.0 { 1 } else { -1 }, 0)
            } else {
                (0, if fz >= 0.0 { 1 } else { -1 })
            };
            let (side_x, side_z) = (-step_z, step_x);
            let base = animal_colour(founder);
            let dark = [
                base[0] * CARCASS_DARKEN,
                base[1] * CARCASS_DARKEN,
                base[2] * CARCASS_DARKEN,
            ];
            let Some(style) = self.flat_style(dark, false) else {
                return;
            };
            for along in 0..length {
                for across in 0..width {
                    let a = along - length / 2;
                    let c = across - width / 2;
                    let z = cz + a * step_z + c * side_z;
                    if z < 0 {
                        continue;
                    }
                    // Each cell lies on its own column's ground near the body's site, so
                    // the body follows a slope instead of a flat slab standing out over
                    // it (or sunk into it); a cell over a drop deeper than that is left out.
                    let x = cx + a * step_x + c * side_x;
                    let (lo, hi) = (g.site.y.saturating_sub(CARCASS_DRAPE), g.site.y + CARCASS_DRAPE);
                    let Some(ground) = (lo..=hi).rev().find(|&yy| view.is_support(x, yy, z as u32)) else {
                        continue;
                    };
                    let cell = Cell { x, y: ground + 1, z: z as u32 };
                    self.place_if_empty(view, cell, AnimalPart::Model(style));
                }
            }
        }
    }

    /// **Specimens** (model pass MP-A): unsimulated animals drawn from the library's
    /// `specimens` at fixed placements, for `voxel_specimens`'s strip. Each stamps its adult
    /// model like a founder's (turned to the nearest quarter heading), in
    /// [`colours::specimen`]'s coat. Call after [`Animals::rebuild_with`]; a name the
    /// library lacks draws nothing.
    pub fn stamp_specimens(&mut self, view: &VoxelView<'_>, lib: &ModelLibrary, specimens: &[Specimen]) {
        if !lib.serves(view.config.voxel_m) {
            return;
        }
        for s in specimens {
            let Some(cells) = lib.specimen(&s.name).and_then(|m| m.select(f64::INFINITY)) else {
                continue;
            };
            let top = cells.iter().map(|m| m.offset[1]).max().unwrap_or(0);
            let mut placed = Vec::with_capacity(cells.len());
            model::each_animal_cell(cells, s.anchor, s.heading_rad, view, |c, m| placed.push((c, m)));
            let mut style_of: FxHashMap<(u8, u8), u16> = FxHashMap::default();
            for (cell, m) in placed {
                let band = colours::band(m.offset[1], top);
                let style = match style_of.get(&(m.material, band)) {
                    Some(&style) => style,
                    None => {
                        let Some(sw) = colours::specimen(&s.name, m.material, band) else {
                            break;
                        };
                        let Some(style) = u16::try_from(self.styles.len()).ok().filter(|&v| v != u16::MAX)
                        else {
                            return;
                        };
                        self.styles.push(Style {
                            wood: srgb_linear(sw.shadow),
                            crown: srgb_linear(sw.body),
                            heart: srgb_linear(sw.glint),
                        });
                        style_of.insert((m.material, band), style);
                        style
                    }
                };
                self.place(view, cell, AnimalPart::Model(style));
            }
        }
    }

    /// [`Animals::place`] on a cell no body holds.
    fn place_if_empty(&mut self, view: &VoxelView<'_>, cell: Cell, part: AnimalPart) {
        if self.at(cell.x, i64::from(cell.y), cell.z) == AnimalPart::None {
            self.place(view, cell, part);
        }
    }

    /// Write one cell, unless it is outside the world or inside solid terrain: a body does
    /// not paint inside rock, the same rule a stand's cells obey.
    fn place(&mut self, view: &VoxelView<'_>, cell: Cell, part: AnimalPart) {
        self.place_with(view, cell, part, 0);
    }

    /// [`Animals::place`] with the cell's pose byte. True when the cell was written.
    fn place_with(&mut self, view: &VoxelView<'_>, cell: Cell, part: AnimalPart, pose: u8) -> bool {
        if cell.y >= self.height || cell.z >= self.depth {
            return false;
        }
        if view.material_at(cell.x, cell.y, cell.z).is_solid() {
            return false;
        }
        let i = self.index(cell.x, cell.y, cell.z);
        if self.grid[i] == AnimalPart::None && self.pose[i] == 0 {
            self.stamped.push(i as u32);
        }
        self.grid[i] = part;
        self.pose[i] = pose;
        if self.owner != NO_OWNER {
            self.owners.push((i as u32, self.owner));
        }
        true
    }

    /// Every cell an animal paints, and every empty voxel under a settling torso
    /// ([`Animals::pose_at`]), as `(x, y, z)` with `x` wrapped, in no particular order.
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
#[path = "hunker_tests.rs"]
mod hunker_tests;

#[cfg(test)]
mod tests {
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
    use cubarium_voxel_fauna::{Command, Fauna, FaunaConfig, Founder, StartingStores};

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
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
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

    /// WX4: an awake browser is stamped exactly as [`cells_of`] with no pose at all; asleep,
    /// the same cells carry their classes and one pose entry, and the belly is marked.
    #[test]
    fn an_awake_browser_carries_no_pose_and_an_asleep_one_carries_its_settle() {
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
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
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
        let fv = fauna.view();
        let mut a = fv.animals[0];
        let body = |g: &Animals| {
            let mut v: Vec<_> = g.cells().filter(|&(x, y, z)| g.at(x.into(), y.into(), z).is_block()).collect();
            v.sort();
            v
        };
        let mut expect: Vec<_> = cells_of(&a, fv.config, 0, c.voxel_m)
            .into_iter()
            .map(|(c, _)| (c.x as u32, c.y, c.z))
            .collect();
        expect.sort();
        let mut awake = Animals::empty(c.width, c.height, c.depth);
        awake.rebuild(&w.view(), Some(fv));
        assert_eq!(body(&awake), expect);
        assert!(awake.pose_entries().is_empty());
        assert!(awake.cells().all(|(x, y, z)| awake.pose_at(x.into(), y.into(), z) == 0));
        assert_eq!(awake.cells().count(), expect.len(), "no marks");

        a.state = State::Asleep;
        let asleep_view = FaunaView {
            animals: std::slice::from_ref(&a),
            ..fv
        };
        let mut asleep = Animals::empty(c.width, c.height, c.depth);
        asleep.rebuild(&w.view(), Some(asleep_view));
        assert_eq!(body(&asleep), expect, "the body is stamped where it stands");
        assert_eq!(asleep.pose_entries().len(), 2);
        assert_eq!(
            asleep.pose_entries()[1],
            PoseEntry {
                sleep: 1.0,
                drop: 2.0,
                sleep_phase: sleep::phase(a.id),
                ..PoseEntry::ZERO
            }
        );
        for &(x, y, z) in &expect {
            assert_ne!(asleep.pose_at(x.into(), y.into(), z), 0);
        }
        let belly = asleep.cells().filter(|&(x, y, z)| {
            !asleep.at(x.into(), y.into(), z).is_block() && asleep.pose_at(x.into(), y.into(), z) != 0
        });
        assert!(belly.count() > 0, "the air under the belly carries faceless leg cells");
    }

    /// The weight the GPU draws every frame (the staged weight plus its rate times the
    /// frame's fraction of a tick) is continuous at 60 frames a second through falling
    /// asleep and waking: no frame moves it more than a frame's share of the blend. A body
    /// first seen asleep is not blended, and a gone body is forgotten.
    #[test]
    fn the_sleep_weight_is_continuous_across_frames() {
        let c = VoxelConfig::default();
        let mut soil = World::empty(c.clone());
        for x in 0..i64::from(c.width) {
            soil.apply(WorldCommand::SetMaterial {
                x,
                y: 0,
                z: 0,
                material: Material::Soil,
            });
        }
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        assert!(fauna.apply(
            &soil,
            Command::IntroduceFounder {
                x: 3,
                z: 0,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            }
        ));
        let mut a = fauna.view().animals[0];
        let mut grid = Animals::empty(0, 0, 0);
        let mut frames = Vec::new();
        for tick in 100..180u64 {
            a.state = if (110..140).contains(&tick) { State::Asleep } else { State::Resting };
            grid.advance_poses(tick, &[a]);
            let (w, rate) = grid.sleep_weight(&a).unwrap_or((0.0, 0.0));
            for k in 0..3 {
                frames.push((w + rate * k as f32 / 3.0).clamp(0.0, 1.0));
            }
        }
        let frame = (1.0 / 60.0 / sleep::BLEND_S) as f32;
        let biggest = frames.windows(2).map(|p| (p[1] - p[0]).abs()).fold(0.0, f32::max);
        assert!(biggest <= frame + 1e-5, "a frame moved the weight {biggest}");
        assert_eq!(frames.iter().copied().fold(0.0, f32::max), 1.0, "it fell fully asleep");
        assert_eq!(*frames.last().unwrap(), 0.0, "and woke");

        let mut fresh = Animals::empty(0, 0, 0);
        a.state = State::Asleep;
        fresh.advance_poses(500, &[a]);
        assert_eq!(fresh.sleep_weight(&a), Some((1.0, 0.0)));
        fresh.advance_poses(501, &[]);
        assert!(fresh.sleep_blend.is_empty());
    }

    /// A browser on soil, and a world to stamp it in.
    fn standing_browser() -> (World, Fauna) {
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
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        assert!(fauna.apply(
            &w,
            Command::IntroduceFounder {
                x: 6,
                z: 4,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            }
        ));
        (w, fauna)
    }

    /// Stamp `a` at `tick` in `grid`.
    fn stamp_at(grid: &mut Animals, w: &World, fauna: &Fauna, a: &Animal, tick: u64) {
        let fv = fauna.view();
        let mut view = w.view();
        view.tick = tick;
        grid.rebuild(
            &view,
            Some(FaunaView {
                animals: std::slice::from_ref(a),
                ..fv
            }),
        );
    }

    /// Idle (WX4): a body that walks, or crops, never idles, however long it goes on; one
    /// that stands still starts to after the hold, with the breath and its head's own entry
    /// when a head move is due, and its head cells carrying that entry.
    #[test]
    fn a_moving_browser_has_no_idle_pose_and_a_standing_one_idles() {
        let (w, fauna) = standing_browser();
        let mut a = fauna.view().animals[0];
        let c = w.config().clone();
        let mut grid = Animals::empty(c.width, c.height, c.depth);
        for tick in 100..160u64 {
            a.state = if tick % 5 == 0 { State::Cropping } else { State::Resting };
            if tick >= 130 {
                a.state = State::Walking;
                a.pose.x += 0.01;
            }
            stamp_at(&mut grid, &w, &fauna, &a, tick);
            assert!(grid.pose_entries().is_empty(), "tick {tick}: no pose while it moves");
            assert!(grid.idle_weight(&a).is_none());
        }
        a.state = State::Resting;
        let hold = (idle::HOLD_S / DT) as u64;
        let mut idled = None;
        for tick in 160..400u64 {
            stamp_at(&mut grid, &w, &fauna, &a, tick);
            if !grid.pose_entries().is_empty() && idled.is_none() {
                idled = Some(tick);
            }
            // A head move: the head cells carry the head's entry, the rest the body's.
            if let Some(e) = grid.pose_entries().iter().position(|e| e.head != HeadMove::NONE) {
                let heads: Vec<_> = grid
                    .cells()
                    .filter(|&(x, y, z)| matches!(grid.at(x.into(), y.into(), z), AnimalPart::Head { .. }))
                    .collect();
                assert!(!heads.is_empty());
                for (x, y, z) in heads {
                    assert_eq!(usize::from(grid.pose_at(x.into(), y.into(), z) & 63), e);
                }
            }
        }
        // It last moved at tick 159.
        assert_eq!(idled, Some(159 + hold), "it idles once it has stood still for the hold");
        let e = grid.pose_entries()[1];
        assert_eq!((e.idle, e.sleep, e.drop), (1.0, 0.0, 0.0), "idle, not asleep: no settle");
        assert_eq!(e.idle_phase, idle::phase(a.id));
    }

    /// A littershredder idles (it breathes) but never moves its head.
    #[test]
    fn an_idle_littershredder_breathes_and_moves_no_head() {
        let (w, _) = standing_browser();
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        assert!(fauna.apply(
            &w,
            Command::IntroduceFounder {
                x: 6,
                z: 4,
                founder: Founder::Blind,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            }
        ));
        let mut a = fauna.view().animals[0];
        a.state = State::Resting;
        let c = w.config().clone();
        let mut grid = Animals::empty(c.width, c.height, c.depth);
        // Longer than two head-move slots, so a browser would have moved its head.
        for tick in 100..(100 + (3.0 * idle::MOVE_EVERY_S / DT) as u64) {
            stamp_at(&mut grid, &w, &fauna, &a, tick);
            assert!(grid.pose_entries().iter().all(|e| e.head == HeadMove::NONE));
        }
        assert_eq!(grid.pose_entries()[1].idle, 1.0, "it idles");
    }

    /// The idle weight the GPU draws every frame is continuous at 60 frames a second as a
    /// body stops and walks off, and never more than a frame's share of the blend.
    #[test]
    fn the_idle_weight_is_continuous_across_frames() {
        let (w, fauna) = standing_browser();
        let mut a = fauna.view().animals[0];
        let mut grid = Animals::empty(0, 0, 0);
        let mut frames = Vec::new();
        let _ = &w;
        for tick in 100..200u64 {
            let walking = !(110..160).contains(&tick);
            a.state = if walking { State::Walking } else { State::Resting };
            if walking {
                a.pose.x += 0.01;
            }
            grid.advance_poses(tick, &[a]);
            let (wt, rate) = grid.idle_weight(&a).unwrap_or((0.0, 0.0));
            for k in 0..3 {
                frames.push((wt + rate * k as f32 / 3.0).clamp(0.0, 1.0));
            }
        }
        let frame = (1.0 / 60.0 / idle::BLEND_S) as f32;
        let biggest = frames.windows(2).map(|p| (p[1] - p[0]).abs()).fold(0.0, f32::max);
        assert!(biggest <= frame + 1e-5, "a frame moved the weight {biggest}");
        assert_eq!(frames.iter().copied().fold(0.0, f32::max), 1.0, "it idled fully");
        assert_eq!(*frames.last().unwrap(), 0.0, "and stopped when it walked off");
    }

    /// An asleep body takes the sleep pose, not the idle one: the resting body that falls
    /// asleep fades its idle out while it settles, and idles again only once awake.
    #[test]
    fn an_asleep_browser_sleeps_and_does_not_idle() {
        let (w, fauna) = standing_browser();
        let mut a = fauna.view().animals[0];
        let c = w.config().clone();
        let mut grid = Animals::empty(c.width, c.height, c.depth);
        a.state = State::Resting;
        for tick in 100..140u64 {
            stamp_at(&mut grid, &w, &fauna, &a, tick);
        }
        assert_eq!(grid.pose_entries()[1].idle, 1.0);
        a.state = State::Asleep;
        for tick in 140..200u64 {
            stamp_at(&mut grid, &w, &fauna, &a, tick);
        }
        let entries = grid.pose_entries();
        assert!(entries.len() >= 2);
        for e in &entries[1..] {
            assert_eq!((e.sleep, e.idle, e.head), (1.0, 0.0, HeadMove::NONE));
            assert_eq!(e.drop, 2.0);
        }
        assert_eq!(grid.idle_weight(&a), None);
    }

    /// The meal flash eases: while a browser crops, its accent leans toward the flash
    /// colour a little more each tick, never further than the configured strength and
    /// never by a jump; once it stops, it leans back to its own colour within the fade.
    /// A strength of 0 is no flash at all.
    #[test]
    fn the_meal_flash_eases_in_and_out_within_its_strength() {
        let (w, fauna) = standing_browser();
        let mut a = fauna.view().animals[0];
        let eye = srgb_linear(FRONDGRAZER_EYE_SRGB);
        let flash = srgb_linear(ANIMAL_CROPPING_FLASH_SRGB);
        // How far the accent leans, along the channel where the two differ most.
        let ch = (0..3)
            .max_by(|&i, &j| (flash[i] - eye[i]).abs().total_cmp(&(flash[j] - eye[j]).abs()))
            .unwrap();
        let lean = |s: &Style| (s.heart[ch] - eye[ch]) / (flash[ch] - eye[ch]);
        let fade_ticks = (MEAL_FLASH_FADE_S / DT).ceil() as u64;
        for strength in [0.0f32, 0.2, 0.7] {
            let mut grid = Animals::empty(w.config().width, w.config().height, w.config().depth);
            grid.set_meal_flash(strength);
            // The most one tick may move it: the smoothstep's steepest slope over one
            // tick's share of the fade, plus one quantisation step.
            let step = strength
                * (1.5 * (DT / MEAL_FLASH_FADE_S) as f32 + 1.0 / f32::from(FLASH_LEVELS))
                + 1e-4;
            let mut last = 0.0f32;
            let mut tick = 1;
            a.state = State::Cropping;
            for _ in 0..fade_ticks + 2 {
                stamp_at(&mut grid, &w, &fauna, &a, tick);
                let l = lean(&grid.styles[0]);
                assert!(l >= last - 1e-4, "{strength}: the flash fell while eating");
                assert!(l - last <= step, "{strength}: tick {tick} jumped {}", l - last);
                assert!(l <= strength + 1e-3, "{strength}: leaned {l}");
                last = l;
                tick += 1;
            }
            assert!((last - strength).abs() < 1e-3, "{strength}: peaked at {last}");
            a.state = State::Resting;
            for _ in 0..fade_ticks + 2 {
                stamp_at(&mut grid, &w, &fauna, &a, tick);
                let l = lean(&grid.styles[0]);
                assert!(l <= last + 1e-4, "{strength}: the flash rose after eating");
                assert!(last - l <= step, "{strength}: tick {tick} dropped {}", last - l);
                last = l;
                tick += 1;
            }
            assert!(last.abs() < 1e-4, "{strength}: still leaning {last} after the fade");
        }
    }

    /// A carcass lies on the ground under each of its cells: across a step down its cells
    /// beyond the edge drop to the lower ground instead of standing out flat over it
    /// (Wrysk's "teal water artifact", 2026-10-08: a loftstrider's body as a floating slab).
    #[test]
    fn a_carcass_follows_the_ground_under_each_cell() {
        use cubarium_voxel_flora::{Carcass, Deposit, DepositKind, Flora, FloraConfig, Site};
        let c = VoxelConfig { width: 32, height: 12, depth: 16, voxel_m: 0.125, ..VoxelConfig::default() };
        let mut w = World::empty(c.clone());
        // Ground tops at y 5 left of x 16, y 3 from x 16 on: a two-cell step down.
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                let top = if x < 16 { 5 } else { 3 };
                for y in 0..=top {
                    w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
                }
            }
        }
        let cfg = FaunaConfig::placeholder();
        let mut flora = Flora::new(FloraConfig::default());
        let site = Site { x: 15, y: 5, z: 8 };
        assert!(flora.deposit(site, Deposit { kind: DepositKind::Carrion, organic: 50.0, mineral: 0.0, energy: 0.0 }));
        // Lying along +x across the step, as long as a grown body of its lineage.
        let carcass = Carcass { lineage: Founder::Browser.index() as u8, heading_rad: std::f64::consts::FRAC_PI_2, organic: 50.0 };
        assert!(flora.lay_carcass(site, carcass));
        let view = w.view();
        let mut grid = Animals::empty(0, 0, 0);
        grid.rebuild(&view, None);
        grid.stamp_carcasses(&view, &flora.view(), &cfg);
        let mut cells = Vec::new();
        for x in 0..i64::from(c.width) {
            for y in 0..i64::from(c.height) {
                if grid.at(x, y, 8) != AnimalPart::None {
                    cells.push((x, y));
                }
            }
        }
        assert!(cells.iter().any(|&(x, _)| x < 16) && cells.iter().any(|&(x, _)| x >= 16), "{cells:?}");
        for &(x, y) in &cells {
            assert!(view.is_solid(x, y as u32 - 1, 8), "cell ({x}, {y}) lies on the ground: {cells:?}");
            assert!(!view.is_solid(x, y as u32, 8), "cell ({x}, {y}) is not in the ground: {cells:?}");
        }
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
