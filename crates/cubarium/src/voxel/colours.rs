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
            let bark = lerp(r.bark.0, r.bark.1, t);
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
            let body = lerp(r.leaf.0, r.leaf.1, (t + bias).clamp(0.0, 1.0));
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
            let (top, bottom) = r.drape.unwrap_or(r.leaf);
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
    /// Drape, `(top, bottom tip)`; `None` uses the leaf ramp.
    drape: Option<(Rgb, Rgb)>,
    /// A ripe warm accent, `(body, glint)`.
    ripe: (Rgb, Rgb),
    /// An accent that is not ripe (a closed bell, a bloom not yet open).
    unripe: Rgb,
}

fn recipe(species: Species) -> Recipe {
    let plain = Recipe {
        bark: (0x3A_1A5A, 0x5A_2E7A),
        leaf: (0x1E_2798, 0x42_C5F8),
        core: None,
        leaf_glint: None,
        drape: None,
        ripe: (0xFF_9B50, 0xFF_E8A8),
        unripe: 0x4A_2A6A,
    };
    match species {
        // Hot pink vanes over a berry rosette, a pale blush core, an orange bloom.
        Species::Bloomcrown => Recipe {
            bark: (0x4A_1250, 0x6E_1E6A),
            leaf: (0x9A_1080, 0xFF_52D0),
            core: Some((1, model::named_index("p1"), 0xFF_D2F2)),
            leaf_glint: Some(0xFF_E0F6),
            ripe: (0xFF_8A3A, 0xFF_E890),
            unripe: 0xFF_D2F2,
            ..plain
        },
        // Sea-glass tiers: deep teal at the lowest tier to aquamarine at the top.
        Species::Umbrellafrond => Recipe {
            bark: (0x24_1650, 0x3E_2A7E),
            leaf: (0x0A_4A5E, 0x46_F2D2),
            leaf_glint: Some(0xC8_FFF2),
            ..plain
        },
        // Mint tufts.
        Species::Springturf => Recipe {
            leaf: (0x10_6A5A, 0x6A_F7BE),
            leaf_glint: Some(0xD8_FFEC),
            ..plain
        },
        // Pale lilac-white domes over a deep violet root mat.
        Species::Stonecushion => Recipe {
            bark: (0x51_0B6D, 0x51_0B6D),
            leaf: (0x7A_5AC8, 0xEE_E4FF),
            ..plain
        },
        // Cobalt velvet with cyan glints.
        Species::Velvetpad => Recipe {
            leaf: (0x1C_46B0, 0x2E_6CE0),
            leaf_glint: Some(0x7C_E8FF),
            ..plain
        },
        // Luminous aqua caps on a lilac stalk.
        Species::Glowcap => Recipe {
            bark: (0x8A_5AC0, 0xB9_9BE6),
            leaf: (0x2A_E0B0, 0x7C_FFDA),
            leaf_glint: Some(0xF0_FFF8),
            ..plain
        },
        // The old giant: emerald lobes over plum bark, curtains of glowing cyan drape.
        Species::Vaulttree => Recipe {
            bark: (0x33_1242, 0x5A_2670),
            leaf: (0x0B_4A3C, 0x5C_E68E),
            leaf_glint: Some(0xD6_FFE2),
            drape: Some((0x1E_8AB8, 0x8C_F6FF)),
            ..plain
        },
        // Acid-lime leaves on plum stems, glowing orange lanterns.
        Species::Lanternberry => Recipe {
            bark: (0x3A_1A5A, 0x52_2A6E),
            leaf: (0x4E_7E16, 0xCC_FF52),
            leaf_glint: Some(0xF2_FFC0),
            ripe: (0xFF_7A2A, 0xFF_E6A0),
            unripe: 0x6A_2E72,
            ..plain
        },
        // Pale celadon stalks rising out of the water to electric magenta heads.
        Species::Siphonreed => Recipe {
            leaf: (0x24_4E66, 0xC4_EEDC),
            core: Some((0, model::named_index("p2"), 0xFF_40D2)),
            leaf_glint: Some(0xFF_C8F4),
            ..plain
        },
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
