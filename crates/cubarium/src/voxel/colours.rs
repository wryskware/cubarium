//! Organism colours for the baked voxel models: the colour pass after package V.
//!
//! Wrysk, 2026-09-23: "do a colour pass. feel free to get creative with it. we dont need
//! a strict 8/16bit color palette." The first cut of package V painted every producer in
//! the dossiers' one blue ramp, so bloomcrown, umbrellafrond, vaulttree and the reeds
//! merged with each other and with the blue-violet terrain. This module is the whole
//! look of the model path in one place, so a later pass edits numbers here and nowhere
//! else.
//!
//! The rules it follows (art direction §05: deep indigo and violet masses holding up
//! electric colour; readable at a glance):
//! - **One hue family per species**, chosen away from the terrain's violet and the
//!   water's cyan, so a stand reads as its species at a glance.
//! - **A ramp from base to tip** by each cell's height in its model: dark and saturated
//!   at the ground, bright at the tips, so a crown has form rather than a flat fill.
//! - **Hue-shifted shadows:** a style's `wood` slot (the colour a crown's front face leans
//!   toward) is the body pulled toward deep indigo, not toward black.
//! - **Small things glow:** glowcap caps, vaulttree drapes, ripe blooms and lanterns
//!   carry a hot `heart` for the face's centre stripe.
//! - **Warm creatures in a cool garden:** the two founders are coral and gold, so moving
//!   inhabitants separate from every producer. Orange stays the ripe accent; the
//!   cropping flash is a pale yellow.
//!
//! Every colour here is a look placeholder for Wrysk to judge (design/backlog.md §2).

use cubarium_voxel_fauna::Founder;
use cubarium_voxel_flora::Species;

use super::model::{self, Tag};

/// sRGB `0xRRGGBB`.
pub type Rgb = u32;

/// The three colours one voxel paints with, in sRGB: `shadow` is the style's `wood` slot
/// (a trunk is drawn in it, a crown's front face leans toward it), `body` its `crown`
/// slot, `glint` its `heart` slot (the centre stripe of a heart face).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swatch {
    pub shadow: Rgb,
    pub body: Rgb,
    pub glint: Rgb,
}

/// How many height bands a model's ramp is cut into. Quantised so a meadow shares a
/// handful of styles: the GPU path holds 256 per frame.
pub const BANDS: u8 = 4;

/// The colour shadows lean toward: the scene's own deep indigo.
const INK: Rgb = 0x14_0A3C;
/// How far a shadow leans toward [`INK`].
const SHADOW_DEPTH: f32 = 0.55;
/// The colour a default glint leans toward.
const GLOW: Rgb = 0xF4_F0FF;
/// How far a default glint leans toward [`GLOW`].
const GLINT_LIFT: f32 = 0.45;

/// The height band of a cell `dy` voxels above its model's anchor, in a model whose
/// highest cell is `top`. A model one cell tall paints its tip colour.
pub fn band(dy: i32, top: i32) -> u8 {
    if top <= 0 {
        return BANDS - 1;
    }
    let t = (dy.clamp(0, top) as f32) / (top as f32);
    (t * f32::from(BANDS - 1)).round() as u8
}

/// A plant cell's colours: its species, what the cell is, the palette material the bake
/// gave it, and its height band. `ripe` says whether a warm accent shows ripe (the
/// caller decides from the parcel); an unripe accent paints the species' closed colour.
pub fn plant(species: Species, tag: Tag, material: u8, band: u8, ripe: bool) -> Swatch {
    let t = f32::from(band.min(BANDS - 1)) / f32::from(BANDS - 1);
    let r = recipe(species);
    match tag {
        Tag::Trunk => {
            let (base, top) = r.part(material).unwrap_or(r.bark);
            let bark = lerp(base, top, t);
            Swatch {
                shadow: bark,
                body: bark,
                glint: lerp(bark, GLOW, 0.3),
            }
        }
        Tag::Foliage(_) => {
            // The bake's shades of one part: `p0` is a part's dark base, `p2` its lit
            // tip. Folded into the ramp rather than into a separate table.
            let bias = if material == model::named_index("p0") {
                -0.3
            } else if material == model::named_index("p2") {
                0.25
            } else {
                0.0
            };
            let body = match r.part(material) {
                Some((base, tip)) => lerp(base, tip, t),
                None => lerp(r.leaf.0, r.leaf.1, (t + bias).clamp(0.0, 1.0)),
            };
            let layer = match tag {
                Tag::Foliage(i) => i,
                _ => 0,
            };
            let body = match r.core {
                Some((l, m, c)) if l == layer && m == material => c,
                _ => body,
            };
            shaded(body, r.leaf_glint)
        }
        Tag::Drape(_) => {
            let (top, bottom) = r.part(material).or(r.drape).unwrap_or(r.leaf);
            // A drape hangs: its lowest cells are the tips.
            shaded(lerp(bottom, top, t), r.leaf_glint)
        }
        Tag::Accent => {
            if material == model::WARM {
                if ripe {
                    Swatch {
                        shadow: lerp(r.ripe.0, INK, 0.35),
                        body: r.ripe.0,
                        glint: r.ripe.1,
                    }
                } else {
                    shaded(r.unripe, None)
                }
            } else if material == model::MAGENTA {
                Swatch {
                    shadow: lerp(SENSE, INK, 0.3),
                    body: SENSE,
                    glint: 0xFF_D8F8,
                }
            } else {
                shaded(r.unripe, None)
            }
        }
    }
}

/// The inherited sense patch on any body, and a plant's magenta accent.
const SENSE: Rgb = 0xFF_2AFC;

/// A founder cell's colours. `band` is the cell's height in the body (the back is
/// lit, the belly dark); `accent` is a sense patch or eye.
pub fn animal(founder: Founder, material: u8, band: u8, accent: bool, cropping: bool) -> Swatch {
    let t = f32::from(band.min(BANDS - 1)) / f32::from(BANDS - 1);
    let (belly, back, eye) = match founder {
        // Frondgrazer: a coral shell over a maroon belly, cyan eyes.
        Founder::Browser => (0xA2_2A4A, 0xFF_8A6E, 0x5F_F0FF),
        // Littershredder: a gold segmented back, magenta feelers (dossier D4's pixel).
        Founder::Blind => (0xB0_6A18, 0xFF_D860, SENSE),
        // Lanternjaw (interim, stage 1: its body plan is open): a dark teal carapace, an
        // ember jaw. No model is baked for it, so this serves a future bake only.
        Founder::Lanternjaw => (0x14_2A2E, 0x30_5A58, 0xFF_5A3C),
    };
    if accent {
        let c = if cropping { CROPPING_FLASH } else { eye };
        return Swatch {
            shadow: c,
            body: c,
            glint: 0xFF_FFFF,
        };
    }
    // The bake's pale cutting edge (`lilac`) stays pale, and its dark legs (`violet`)
    // stay dark, on any body.
    let body = if material == model::named_index("lilac") {
        0xFF_E0CC
    } else if material == model::named_index("violet") {
        lerp(belly, INK, 0.45)
    } else {
        lerp(belly, back, t)
    };
    // A model body is drawn with the plain block glyph, which paints every face in the
    // `shadow` slot: the body colour goes there too.
    Swatch {
        shadow: body,
        body,
        glint: lerp(body, GLOW, GLINT_LIFT),
    }
}

/// A browsing mouth's flash (the glyph path's orange, moved to pale yellow now that
/// orange is the ripe accent and the grazer's shell is coral).
pub const CROPPING_FLASH: Rgb = 0xFF_F27A;

/// The starving tint of a sense patch (dossier D3/D4).
pub const STARVED_ACCENT: Rgb = 0x7A_2A78;

/// Bloomcrown's core (D9's rounded mass between the vanes): the pick's blush pod, sampled
/// 2026-10-01 (`design/art-direction/species-colours-2026-09-24.md`).
/// Its unripe bloom paints the same colour. It glows (package L5).
pub const BLOOMCROWN_CORE: Rgb = 0xFE_D9F2;
/// The bake's foliage layer that holds bloomcrown's core (material `p1`).
const BLOOMCROWN_CORE_LAYER: u8 = 1;
/// Bloomcrown's ripe bloom: D9's "ripe: the core turns the warm accent", the pick's
/// orange pod tip (sampled 2026-10-01).
pub const BLOOMCROWN_BLOOM: Rgb = 0xFE_AC42;

/// One species' colours.
struct Recipe {
    /// Stem or bark, `(base, top)`.
    bark: (Rgb, Rgb),
    /// Foliage, `(base, tip)`.
    leaf: (Rgb, Rgb),
    /// A special foliage colour for one bake material in one foliage layer, `(layer,
    /// material, colour)`: bloomcrown's core, the reeds' heads.
    core: Option<(u8, u8, Rgb)>,
    /// Foliage glint, if not the default lift toward white.
    leaf_glint: Option<Rgb>,
    /// Per-part ramps sampled from the artbook pick (model pass, 2026-10-01): a trunk,
    /// foliage or drape cell the bake drew in this palette material paints this
    /// `(base, tip)` ramp instead of `bark`, `leaf` or `drape`.
    parts: &'static [(&'static str, Rgb, Rgb)],
    /// Drape, `(top, bottom tip)`; `None` uses the leaf ramp.
    drape: Option<(Rgb, Rgb)>,
    /// A ripe warm accent, `(body, glint)`.
    ripe: (Rgb, Rgb),
    /// An accent that is not ripe (a closed bell, a bloom not yet open).
    unripe: Rgb,
}

impl Recipe {
    /// The pick's ramp for a bake material, if this species has one.
    fn part(&self, material: u8) -> Option<(Rgb, Rgb)> {
        let name = model::PALETTE.get(usize::from(material))?.0;
        self.parts
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|&(_, a, b)| (a, b))
    }
}

fn recipe(species: Species) -> Recipe {
    let plain = Recipe {
        bark: (0x3A_1A5A, 0x5A_2E7A),
        leaf: (0x1E_2798, 0x42_C5F8),
        core: None,
        leaf_glint: None,
        parts: &[],
        drape: None,
        ripe: (0xFF_9B50, 0xFF_E8A8),
        unripe: 0x4A_2A6A,
    };
    match species {
        // Every plant below is sampled 2026-10-01 from its artbook pick
        // (art/gen/artbook/README.md): one base-to-tip ramp per part the builder draws.
        //
        // Bloomcrown (bloomcrown-pink-preferred.png): violet stem, periwinkle basal straps,
        // hot-pink vanes round a blush pod (the core), an orange cap (the bloom).
        Species::Bloomcrown => Recipe {
            bark: (0x2E_035F, 0x5C_1EAF),
            leaf: (0x8B_0251, 0xFD_48D1),
            core: Some((BLOOMCROWN_CORE_LAYER, model::named_index("p1"), BLOOMCROWN_CORE)),
            leaf_glint: Some(0xFD_C6F2),
            parts: &[
                ("violet", 0x2E_035F, 0x5C_1EAF),
                ("p0", 0x5D_55C2, 0x85_82F3),
                ("p2", 0x8B_0251, 0xFD_48D1),
            ],
            ripe: (BLOOMCROWN_BLOOM, 0xFE_E6EB),
            unripe: BLOOMCROWN_CORE,
            ..plain
        },
        // Umbrellafrond (umbrellafrond.png): a violet stem with darker collars, teal
        // fronds with pale mint ribs.
        Species::Umbrellafrond => Recipe {
            bark: (0x4D_1378, 0x78_23B4),
            leaf: (0x00_696F, 0x02_9894),
            leaf_glint: Some(0xDC_FDEE),
            parts: &[
                ("violet", 0x4D_1378, 0x78_23B4),
                ("plum", 0x51_176D, 0x7E_2C9D),
                ("p1", 0x00_696F, 0x02_9894),
                ("p2", 0x4D_E0C4, 0xDC_FDEE),
            ],
            ..plain
        },
        // Springturf (springturf.png): tall mint-teal straps, a short dark-teal rank,
        // violet deep between them.
        Species::Springturf => Recipe {
            leaf: (0x03_7B7E, 0x48_F5BE),
            leaf_glint: Some(0xA4_FCDB),
            parts: &[
                ("p2", 0x03_7B7E, 0x48_F5BE),
                ("p0", 0x02_3A47, 0x03_7B7E),
                ("violet", 0x20_054E, 0x32_166B),
            ],
            ..plain
        },
        // Stonecushion (stonecushion.png): pale-lilac lobes, cyan pores, a dark
        // magenta-violet root mat.
        Species::Stonecushion => Recipe {
            bark: (0x39_0542, 0x56_1470),
            leaf: (0x7A_4ED0, 0xDA_BBFD),
            leaf_glint: Some(0xF4_E6FD),
            parts: &[
                ("lilac", 0x7A_4ED0, 0xDA_BBFD),
                ("p2", 0x01_B8EC, 0x01_B8EC),
                ("detritus", 0x39_0542, 0x56_1470),
            ],
            ..plain
        },
        // Velvetpad (velvetpad-provisional.png, provisional): a dark violet pad with a
        // purple lip, cobalt moss, cyan channels and blue-to-cyan buds.
        Species::Velvetpad => Recipe {
            leaf: (0x1B_296C, 0x38_66DA),
            leaf_glint: Some(0x75_F1F8),
            parts: &[
                ("violet", 0x33_1D47, 0x48_295E),
                ("plum", 0x5F_3D8D, 0x8F_60B8),
                ("p1", 0x1B_296C, 0x38_66DA),
                ("p2", 0x45_94F0, 0x75_F1F8),
                ("teal", 0x75_F1F8, 0x75_F1F8),
            ],
            ..plain
        },
        // Glowcap (glowcap-on-vaulttree.png, the mushroom only): a dark violet stalk, a
        // purple cap with flat dark spots, paler lilac spots and magenta speckles, a cyan
        // brim and underside (the lip that glows).
        Species::Glowcap => Recipe {
            bark: (0x55_4199, 0x35_256D),
            leaf: (0x44_26A1, 0x76_4BD7),
            leaf_glint: Some(0xB0_78EF),
            parts: &[
                ("detritus", 0x55_4199, 0x35_256D),
                ("violet", 0x44_26A1, 0x76_4BD7),
                ("p0", 0x29_1654, 0x29_1654),
                ("lilac", 0xB0_78EF, 0xB0_78EF),
                ("magenta", 0xF6_07FC, 0xF6_07FC),
                ("p2", 0x2F_B0E2, 0x3F_F9FC),
            ],
            ..plain
        },
        // Vaulttree (vaulttree.png): lit and mid violet trunk plates and one dark-teal
        // plate, violet limbs; dark-teal shelves with mint rims; cyan tendrils,
        // brightest at their tips.
        Species::Vaulttree => Recipe {
            bark: (0x5F_36A9, 0x4A_2D8E),
            leaf: (0x01_4C4D, 0x01_696E),
            leaf_glint: Some(0x96_F9DC),
            drape: Some((0x01_97B1, 0x00_CEEC)),
            parts: &[
                ("plum", 0x7B_46CA, 0x6C_3EB9),
                ("violet", 0x5F_36A9, 0x4A_2D8E),
                ("detritus", 0x01_261F, 0x01_261F),
                ("p1", 0x01_4C4D, 0x01_696E),
                ("p2", 0x50_C5A9, 0x96_F9DC),
            ],
            ..plain
        },
        // Lanternberry (lanternberry-blue.png): violet stems, blue leaflets, deep-plum
        // lanterns glowing orange inside when ripe.
        Species::Lanternberry => Recipe {
            bark: (0x2E_0944, 0x4E_178C),
            leaf: (0x01_3793, 0x18_ADFC),
            leaf_glint: Some(0x53_F1FD),
            parts: &[
                ("violet", 0x2E_0944, 0x4E_178C),
                ("p1", 0x01_3793, 0x18_ADFC),
            ],
            ripe: (LANTERN_RIPE, 0xFD_EC9D),
            unripe: 0x37_0A49,
            ..plain
        },
        // Siphonreed (siphonreed.png): slim teal stems, pale celadon swollen shafts,
        // rounded pink fan heads.
        Species::Siphonreed => Recipe {
            leaf: (0x01_4B74, 0x23_A6BC),
            leaf_glint: Some(0xEE_FCE5),
            parts: &[
                ("p1", 0x01_4B74, 0x23_A6BC),
                ("lilac", 0x57_C5C9, 0xA5_EED3),
                ("p2", 0xEB_2189, 0xFD_4FB6),
            ],
            ..plain
        },
    }
}

/// A latticevine cell's colours (D15's dossier hexes): the shingled blue ramp when full,
/// its dark base when thinned, the deep-plum runner when bare or dormant. A cell whose
/// spur is in bud, flower or fruit is tinted whole by that accent — a pale-lilac bell, a
/// magenta bead bunch, a violet bud — and its `glint` is the heart stripe down its middle:
/// the bell's cyan mouth, the beads' cyan seeds, the bud's lilac tip.
pub fn vine(density: super::vine::Density, accent: super::vine::Accent) -> Swatch {
    use super::vine::{Accent, Density};
    match accent {
        Accent::Flower => return shaded(0xB9_9BE6, Some(0x42_C5F8)),
        Accent::Fruit => return shaded(0xFF_2AFC, Some(0x42_C5F8)),
        Accent::Bud => return shaded(0x3A_1A7A, Some(0xB9_9BE6)),
        Accent::None => {}
    }
    match density {
        Density::Full => shaded(0x2B_6AD0, Some(0x42_C5F8)),
        Density::Thin => shaded(0x1E_2798, Some(0x2B_6AD0)),
        Density::Bare => shaded(0x2A_0E4A, Some(0x3A_1A7A)),
    }
}

// --- emission (package L step 4) ----------------------------------------------------------
//
// Which parts glow, and in which colour, is data: only parts the species dossiers name as
// luminous **and** that today's models or appearance data draw, each in the dossier's own
// hex for that part (design/art-direction/species-dossiers-2026-09-21.md,
// species-dossier-D15-latticevine-2026-09-23.md). Not the colour pass's placeholders
// above: these are the dossiers' numbers. The renderer's lit tier draws an emitting texel
// in this colour, unlit, lets it light the open cells around it, and blooms around it.
//
// Package L5 (Wrysk, 2026-09-24) adds bloomcrown's core, which glows whole in its own
// colour (the colour file's blush, orange once ripe), and the animal path
// ([`animal_emission`]). Sense patches never glow.

/// A glowcap's lip, by the colony state the dossier draws it in (D1): absent on a button
/// or a spent colony, half intensity while fruiting, full when ripe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lip {
    None = 0,
    Fruiting = 1,
    Ripe = 2,
}

/// D1 glowcap `[lip]`: "the luminous tissue ... electric cyan", full when ripe. Sampled
/// 2026-10-01 from the pick's brim.
pub const LIP_RIPE: Rgb = 0x3F_F9FC;
/// D1 glowcap, fruiting: "lip at half intensity" (the pick's underside cyan).
pub const LIP_FRUITING: Rgb = 0x2F_B0E2;
/// D12 lanternberry, ripe: the open lantern's "warm interior seen through the opening"
/// (the roster names the fruit luminous: organism-scale-and-roster-2026-09-21.md),
/// sampled 2026-10-01 from the pick's lantern interior.
pub const LANTERN_RIPE: Rgb = 0xFD_A243;
/// D15 latticevine flower: "its lip flaring into a glowing `#42C5F8` mouth".
pub const BELL_MOUTH: Rgb = 0x42_C5F8;

/// How one emitting part glows: its colour, and whether its **whole** cell emits (a
/// bloomcrown core) or only the texels its glyph marks with the emissive tone (a glowcap's
/// lip rows, a lantern's or a bell's heart stripe).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glow {
    pub rgb: Rgb,
    pub whole: bool,
}

impl Glow {
    const fn marked(rgb: Rgb) -> Glow {
        Glow { rgb, whole: false }
    }
}

/// The emission of a baked model cell, or `None`: a glowcap's lip tissue (the bake's `p2`
/// foliage, the cyan it builds the lip from) in its colony state, a lanternberry's
/// lantern (its warm accent) while ripe fruit hangs, and bloomcrown's core (its `p1`
/// core foliage and its bloom accent) whole, blush, or orange once the bloom is ripe.
/// `ripe` is the cell's accent state as [`plant`] takes it.
pub fn plant_emission(species: Species, tag: Tag, material: u8, ripe: bool, lip: Lip) -> Option<Glow> {
    match (species, tag) {
        (Species::Glowcap, Tag::Foliage(_)) if material == model::named_index("p2") => match lip {
            Lip::None => None,
            Lip::Fruiting => Some(Glow::marked(LIP_FRUITING)),
            Lip::Ripe => Some(Glow::marked(LIP_RIPE)),
        },
        (Species::Lanternberry, Tag::Accent) if material == model::WARM && ripe => {
            Some(Glow::marked(LANTERN_RIPE))
        }
        (Species::Bloomcrown, Tag::Foliage(BLOOMCROWN_CORE_LAYER))
            if material == model::named_index("p1") =>
        {
            Some(Glow {
                rgb: BLOOMCROWN_CORE,
                whole: true,
            })
        }
        (Species::Bloomcrown, Tag::Accent) if material == model::WARM => Some(Glow {
            rgb: if ripe { BLOOMCROWN_BLOOM } else { BLOOMCROWN_CORE },
            whole: true,
        }),
        _ => None,
    }
}

/// The emission of an animal's model cell, or `None`. **No current founder has a
/// luminous part**: the frondgrazer's cyan eyes and the littershredder's magenta feelers
/// are pigment, and sense patches never glow (Wrysk, 2026-09-24). An animal whose design
/// calls a part luminous (the chorister, when it reaches the engine) names that part here
/// by its bake material; the renderer then draws the cell with the emissive animal glyph
/// (`appearance::animal_emissive_glyph`) and it lights, glows and blooms like a plant's.
pub fn animal_emission(founder: Founder, material: u8, accent: bool) -> Option<Rgb> {
    let _ = (founder, material, accent);
    None
}

/// The emissive colour of a latticevine accent: the flower's bell mouth.
pub fn vine_emission(accent: super::vine::Accent) -> Option<Rgb> {
    match accent {
        super::vine::Accent::Flower => Some(BELL_MOUTH),
        _ => None,
    }
}

/// A body colour with its hue-shifted shadow and a glint.
fn shaded(body: Rgb, glint: Option<Rgb>) -> Swatch {
    Swatch {
        shadow: lerp(body, INK, SHADOW_DEPTH),
        body,
        glint: glint.unwrap_or_else(|| lerp(body, GLOW, GLINT_LIFT)),
    }
}

/// `a` to `b` by `t`, per sRGB channel.
pub fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let ch = |c: Rgb, s: u32| ((c >> s) & 0xFF) as f32;
    let mixed = |s: u32| ((ch(a, s) + (ch(b, s) - ch(a, s)) * t).round() as u32).min(255) << s;
    mixed(16) | mixed(8) | mixed(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_run_from_the_base_to_the_tip() {
        assert_eq!(band(0, 10), 0);
        assert_eq!(band(10, 10), BANDS - 1);
        assert_eq!(band(0, 0), BANDS - 1, "a one-cell model paints its tip");
        assert_eq!(band(-3, 5), 0);
    }

    #[test]
    fn every_species_has_its_own_leaf_tip() {
        let tips: Vec<Rgb> = Species::ALL.iter().map(|&s| recipe(s).leaf.1).collect();
        for (i, a) in tips.iter().enumerate() {
            for b in &tips[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn an_unripe_accent_is_not_the_ripe_colour() {
        for s in [Species::Bloomcrown, Species::Lanternberry] {
            let ripe = plant(s, Tag::Accent, model::WARM, 0, true);
            let closed = plant(s, Tag::Accent, model::WARM, 0, false);
            assert_ne!(ripe.body, closed.body, "{s:?}");
        }
    }
}
