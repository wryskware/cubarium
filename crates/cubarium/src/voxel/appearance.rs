//! Backend-neutral voxel-face art.
//!
//! Organism anatomy is resolved here into small face glyphs. Renderers only sample the
//! resulting texels and apply projection, visibility, light and haze; neither backend
//! knows where an eye, marking or outline belongs.
//!
//! # Emission
//!
//! An emitting texel carries the reserved tone [`TONE_EMIT`]. The GPU renderer's lit tier
//! draws it in its cell's style's emissive colour, unlit and at full value; the cell then
//! lights the glow volume around it and blooms. Two things must hold for a texel to glow:
//! its glyph marks it with [`TONE_EMIT`], **and** its style has an emissive colour. The
//! flat tier and the CPU presenter draw the tone as a plain texel.
//!
//! - **Plants:** a crown or heart cell whose style emits draws its glyph's emissive
//!   variant ([`emissive_glyph`]): a crown's lip rows, a heart's stripe. A style can also
//!   emit **whole** (`VoxelStyle::emit_whole`: bloomcrown's core), whatever its tones.
//! - **Animals** (package L5): an animal part emits when
//!   `colours::animal_emission` gives its model material an emissive colour (the style
//!   side, stored by `Animals::set_emission`) **and** the packer draws its cell with
//!   [`animal_emissive_glyph`] (the glyph side), whose texels carry [`TONE_EMIT`]. Today
//!   that glyph is [`GLYPH_ANIMAL_EMIT`], the whole block. A body whose glow is a pattern
//!   (the chorister's lit parts, a spot, a stripe) adds its own glyph in the animal
//!   class's one free slot (7) marking only those texels, and names it in
//!   [`animal_emissive_glyph`]. No current founder part emits, and sense patches never do
//!   (Wrysk, 2026-09-24).

use super::animal::AnimalPart;
use super::stand::Part;

/// Eight variants for each of the texel format's eight part classes.
pub const GLYPHS_PER_PART: u8 = 8;
pub const ATLAS_GLYPHS: usize = 64;
pub const ANIMAL_PART_CLASS: u8 = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct GlyphId(pub u8);

pub const GLYPH_NONE: GlyphId = GlyphId(0);
pub const GLYPH_INTERIM: GlyphId = GlyphId(1);
pub const GLYPH_BODY: GlyphId = GlyphId(2);
pub const GLYPH_FRONT_BODY: GlyphId = GlyphId(3);
pub const GLYPH_HEAD_LEFT: GlyphId = GlyphId(4);
pub const GLYPH_HEAD_RIGHT: GlyphId = GlyphId(5);
/// An emitting animal cell (package L5): the interim block with every front and cap texel
/// [`TONE_EMIT`]. Only the lit tier's packer draws it, for a part whose style emits.
pub const GLYPH_ANIMAL_EMIT: GlyphId = GlyphId(6);

/// Which of an organism style's three authored colours a face texel uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Pigment {
    Primary = 0,
    Secondary = 1,
    Accent = 2,
}

/// Generic material treatments. Values 16..=31 encode a quantised surface gain.
pub const TONE_PLAIN: u8 = 0;
pub const TONE_OUTLINE: u8 = 1;
pub const TONE_OPEN_RIM: u8 = 2;
pub const TONE_LIT: u8 = 3;
pub const TONE_UNDER: u8 = 4;
pub const TONE_UNDER_EDGE: u8 = 5;
/// An **emitting** texel (package L step 4): in the lit tier it draws its style's emissive
/// colour (`VoxelStyle::with_emit`) unlit and at full value; a style with none, and the
/// flat tier, draw it as [`TONE_PLAIN`]. Only the emissive glyph variants
/// ([`emissive_glyph`]) carry it.
pub const TONE_EMIT: u8 = 6;
pub const TONE_CAP_GAIN_BASE: u8 = 32;
pub const TONE_TRANSPARENT: u8 = 63;

/// One resolved texel in a voxel face glyph, packed for an `R8_UINT` atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct FaceTexel(pub u8);

impl FaceTexel {
    pub const fn new(pigment: Pigment, tone: u8) -> FaceTexel {
        FaceTexel(pigment as u8 | ((tone & 0x3f) << 2))
    }

    pub const fn pigment(self) -> Pigment {
        match self.0 & 0x03 {
            1 => Pigment::Secondary,
            2 => Pigment::Accent,
            _ => Pigment::Primary,
        }
    }

    pub const fn tone(self) -> u8 {
        self.0 >> 2
    }
}

/// Resolve anatomy to a visual glyph before either renderer sees it.
pub fn animal_glyph(part: AnimalPart) -> GlyphId {
    match part {
        AnimalPart::None => GLYPH_NONE,
        AnimalPart::Interim(_) | AnimalPart::Model(_) => GLYPH_INTERIM,
        AnimalPart::Body { head: false, .. } => GLYPH_BODY,
        AnimalPart::Body { head: true, .. } => GLYPH_FRONT_BODY,
        AnimalPart::Head {
            facing_right: false,
            ..
        } => GLYPH_HEAD_LEFT,
        AnimalPart::Head {
            facing_right: true, ..
        } => GLYPH_HEAD_RIGHT,
    }
}

/// The bit of a crown or heart glyph id that selects its **emissive** variant: the same
/// face with the texels an emitting cell lights marked [`TONE_EMIT`]. A crown cell (a
/// glowcap's lip tissue) emits along its front face's lowest rows, the lower-facing edge
/// the dossier puts the lip on (D1: a 1-px strip at 6 px a voxel); a heart cell (a
/// latticevine bell, a lanternberry lantern) emits its accent stripe, front and cap.
pub const GLYPH_EMIT_BIT: u8 = 4;

/// The emissive variant of a crown or heart glyph.
pub fn emissive_glyph(glyph: GlyphId) -> GlyphId {
    GlyphId(glyph.0 | GLYPH_EMIT_BIT)
}

/// The glyph an emitting animal cell draws with, in place of `glyph`: see the module's
/// "Emission" section. Every animal glyph maps to the whole emitting block today.
pub fn animal_emissive_glyph(glyph: GlyphId) -> GlyphId {
    let _ = glyph;
    GLYPH_ANIMAL_EMIT
}

fn emits(glyph: GlyphId) -> bool {
    glyph.0 & GLYPH_EMIT_BIT != 0
}

/// Rows of a crown's front face its lip emits along: one at 6 px a voxel, in proportion
/// above that.
fn lip_rows(s: u32) -> u32 {
    (s / 6).max(1)
}

/// Resolve one flora cell to an atlas variant. Crown bits name exposed left/right edges;
/// the face recipe itself remains in this module.
pub fn plant_glyph(part: Part, open_left: bool, open_right: bool) -> GlyphId {
    match part {
        Part::Crown { .. } => GlyphId(u8::from(open_left) | (u8::from(open_right) << 1)),
        _ => GLYPH_NONE,
    }
}

pub fn plant_class(part: Part) -> u8 {
    match part {
        Part::None => 0,
        Part::Trunk(_) => 1,
        Part::Crown { heart: false, .. } => 2,
        Part::Crown { heart: true, .. } => 3,
        Part::Sprout(_) => 4,
        Part::Log(_) => 6,
        Part::Litter(_) | Part::Carrion(_) => 7,
    }
}

fn edge_variant(glyph: GlyphId, x: u32, s: u32) -> bool {
    (x == 0 && glyph.0 & 1 != 0) || (x + 1 == s && glyph.0 & 2 != 0)
}

fn gain_tone(gain: f32, cap: bool) -> u8 {
    let q = ((gain.clamp(0.5, 1.4375) - 0.5) * 16.0 + 0.5) as u8;
    (if cap { TONE_CAP_GAIN_BASE } else { 16 }) + q.min(15)
}

fn trunk_gain(x: u32, s: u32) -> f32 {
    let u = (x as f32 + 0.5) / s.max(1) as f32;
    let t = (1.0 - (u - 0.35).abs() / 0.65).clamp(0.0, 1.0);
    0.62 + (1.22 - 0.62) * t
}

/// The front face texel at local pixel `(x, y)`, with `y = 0` at the top.
pub fn front_texel(part: u8, glyph: GlyphId, x: u32, y: u32, s: u32) -> FaceTexel {
    let bottom = y + 1 == s;
    let top = y == 0;
    let outline_or_rim = if bottom {
        TONE_OUTLINE
    } else if top {
        TONE_OPEN_RIM
    } else {
        TONE_PLAIN
    };

    if part == 1 {
        return FaceTexel::new(Pigment::Primary, gain_tone(trunk_gain(x, s), false));
    }
    if part == 2 || part == 3 {
        let heart = part == 3 && x * 2 >= s.saturating_sub(2) && x * 2 < s + 2;
        let lip = part == 2 && y + lip_rows(s) >= s;
        let tone = if emits(glyph) && (heart || lip) {
            TONE_EMIT
        } else if edge_variant(glyph, x, s) {
            TONE_UNDER_EDGE
        } else {
            TONE_UNDER
        };
        return FaceTexel::new(
            if heart {
                Pigment::Accent
            } else {
                Pigment::Secondary
            },
            tone,
        );
    }
    if part == 4 || part == 7 {
        let (mark, rows) = if part == 4 {
            ((s / 2).max(1), (s / 2).max(1))
        } else {
            ((s * 3 / 4).max(1), (s / 3).max(1))
        };
        let x0 = (s - mark) / 2;
        let y0 = s - rows;
        if x < x0 || x >= x0 + mark || y < y0 {
            return FaceTexel::new(Pigment::Primary, TONE_TRANSPARENT);
        }
        let pigment = if y == y0 {
            Pigment::Secondary
        } else {
            Pigment::Primary
        };
        return FaceTexel::new(pigment, if y == y0 { TONE_LIT } else { TONE_PLAIN });
    }
    if part == 6 {
        return FaceTexel::new(
            Pigment::Primary,
            if bottom {
                TONE_OUTLINE
            } else if top {
                TONE_OPEN_RIM
            } else {
                TONE_PLAIN
            },
        );
    }

    if part == ANIMAL_PART_CLASS && glyph == GLYPH_ANIMAL_EMIT {
        return FaceTexel::new(Pigment::Primary, TONE_EMIT);
    }
    match glyph {
        GLYPH_INTERIM => FaceTexel::new(
            Pigment::Primary,
            if top { TONE_OPEN_RIM } else { TONE_PLAIN },
        ),
        GLYPH_BODY => FaceTexel::new(Pigment::Primary, outline_or_rim),
        GLYPH_FRONT_BODY => {
            // A quiet secondary shoulder distinguishes the anterior without repeating a
            // bright stripe on every body voxel.
            let shoulder = s >= 4 && y == 1 && (x == 0 || x + 1 == s);
            FaceTexel::new(
                if shoulder {
                    Pigment::Secondary
                } else {
                    Pigment::Primary
                },
                outline_or_rim,
            )
        }
        GLYPH_HEAD_LEFT | GLYPH_HEAD_RIGHT => {
            let eye_x = if glyph == GLYPH_HEAD_RIGHT {
                s.saturating_sub(2)
            } else {
                1.min(s.saturating_sub(1))
            };
            let eye = x == eye_x && (y == 1 || (s > 4 && y == 2));
            FaceTexel::new(
                if eye {
                    Pigment::Accent
                } else {
                    Pigment::Secondary
                },
                outline_or_rim,
            )
        }
        _ => FaceTexel::new(Pigment::Primary, TONE_PLAIN),
    }
}

/// The receding cap texel at local pixel `(x, y)`.
pub fn cap_texel(part: u8, glyph: GlyphId, x: u32, _y: u32, s: u32) -> FaceTexel {
    if part == 1 {
        return FaceTexel::new(Pigment::Primary, gain_tone(trunk_gain(x, s), true));
    }
    if part == 2 || part == 3 {
        let heart = part == 3 && x * 2 >= s.saturating_sub(2) && x * 2 < s + 2;
        return FaceTexel::new(
            if heart {
                Pigment::Accent
            } else {
                Pigment::Secondary
            },
            if heart && emits(glyph) {
                TONE_EMIT
            } else if edge_variant(glyph, x, s) {
                TONE_OUTLINE
            } else {
                TONE_PLAIN
            },
        );
    }
    if part == 6 {
        return FaceTexel::new(Pigment::Secondary, TONE_PLAIN);
    }
    if part == ANIMAL_PART_CLASS && glyph == GLYPH_ANIMAL_EMIT {
        return FaceTexel::new(Pigment::Primary, TONE_EMIT);
    }
    match glyph {
        GLYPH_INTERIM => FaceTexel::new(Pigment::Primary, TONE_PLAIN),
        GLYPH_BODY | GLYPH_FRONT_BODY => FaceTexel::new(Pigment::Secondary, TONE_PLAIN),
        GLYPH_HEAD_LEFT | GLYPH_HEAD_RIGHT => {
            let eye_x = if glyph == GLYPH_HEAD_RIGHT {
                s.saturating_sub(2)
            } else {
                1.min(s.saturating_sub(1))
            };
            FaceTexel::new(
                if x == eye_x {
                    Pigment::Accent
                } else {
                    Pigment::Secondary
                },
                TONE_PLAIN,
            )
        }
        _ => FaceTexel::new(Pigment::Primary, TONE_PLAIN),
    }
}

/// Atlas layout shared with the GPU: one `s × (s + rise)` strip per glyph, front first.
pub fn atlas_len(s: u32, rise: u32) -> usize {
    s as usize * (s + rise) as usize * ATLAS_GLYPHS
}

pub fn write_atlas(s: u32, rise: u32, out: &mut [u8]) {
    assert_eq!(out.len(), atlas_len(s, rise));
    let glyph_h = s + rise;
    for part in 0..8u8 {
        for glyph in 0..GLYPHS_PER_PART {
            let atlas_glyph = u32::from(part * GLYPHS_PER_PART + glyph);
            for y in 0..s {
                for x in 0..s {
                    let i = ((atlas_glyph * glyph_h + y) * s + x) as usize;
                    out[i] = front_texel(part, GlyphId(glyph), x, y, s).0;
                }
            }
            for y in 0..rise {
                for x in 0..s {
                    let i = ((atlas_glyph * glyph_h + s + y) * s + x) as usize;
                    out[i] = cap_texel(part, GlyphId(glyph), x, y, s).0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_direction_is_resolved_in_the_shared_glyph() {
        let left = front_texel(ANIMAL_PART_CLASS, GLYPH_HEAD_LEFT, 1, 1, 6);
        let right = front_texel(ANIMAL_PART_CLASS, GLYPH_HEAD_RIGHT, 4, 1, 6);
        assert_eq!(left.pigment(), Pigment::Accent);
        assert_eq!(right.pigment(), Pigment::Accent);
        assert_eq!(
            front_texel(ANIMAL_PART_CLASS, GLYPH_HEAD_LEFT, 4, 1, 6).pigment(),
            Pigment::Secondary
        );
    }

    #[test]
    fn body_detail_is_quiet_and_heads_keep_the_accent() {
        for y in 0..6 {
            for x in 0..6 {
                assert_ne!(
                    front_texel(ANIMAL_PART_CLASS, GLYPH_BODY, x, y, 6).pigment(),
                    Pigment::Accent
                );
            }
        }
        assert_eq!(
            front_texel(ANIMAL_PART_CLASS, GLYPH_HEAD_RIGHT, 4, 1, 6).pigment(),
            Pigment::Accent
        );
    }

    #[test]
    fn atlas_is_the_documented_front_then_cap_layout() {
        let (s, rise) = (6, 3);
        let mut atlas = vec![0; atlas_len(s, rise)];
        write_atlas(s, rise, &mut atlas);
        let row =
            u32::from(ANIMAL_PART_CLASS * GLYPHS_PER_PART + GLYPH_HEAD_RIGHT.0) * (s + rise) + 1;
        assert_eq!(
            atlas[(row * s + 4) as usize],
            FaceTexel::new(Pigment::Accent, TONE_PLAIN).0
        );
    }

    #[test]
    fn only_the_emissive_variants_carry_the_emitting_tone() {
        let s = 12;
        let tones = |part: u8, glyph: GlyphId| -> Vec<u8> {
            let mut t = Vec::new();
            for y in 0..s {
                for x in 0..s {
                    t.push(front_texel(part, glyph, x, y, s).tone());
                    t.push(cap_texel(part, glyph, x, y, s).tone());
                }
            }
            t
        };
        for part in 0..8 {
            for g in 0..GLYPHS_PER_PART {
                if (part == 2 || part == 3) && g & GLYPH_EMIT_BIT != 0 {
                    continue;
                }
                if part == ANIMAL_PART_CLASS && GlyphId(g) == GLYPH_ANIMAL_EMIT {
                    continue;
                }
                assert!(!tones(part, GlyphId(g)).contains(&TONE_EMIT), "part {part} glyph {g}");
            }
        }
        // A lip emits along the front's lowest rows only, never on its cap.
        let lip = emissive_glyph(GlyphId(0));
        assert_eq!(front_texel(2, lip, 3, s - 1, s).tone(), TONE_EMIT);
        assert_eq!(front_texel(2, lip, 3, s - 3, s).tone(), TONE_UNDER);
        assert_ne!(cap_texel(2, lip, 3, 0, s).tone(), TONE_EMIT);
        // A heart emits its accent stripe, front and cap, and nothing else.
        let heart = emissive_glyph(GlyphId(0));
        for x in 0..s {
            let stripe = front_texel(3, GlyphId(0), x, 4, s).pigment() == Pigment::Accent;
            assert_eq!(front_texel(3, heart, x, 4, s).tone() == TONE_EMIT, stripe);
            assert_eq!(cap_texel(3, heart, x, 0, s).tone() == TONE_EMIT, stripe);
        }
    }

    #[test]
    fn an_emitting_animal_cell_emits_on_every_texel_and_no_other_animal_glyph_does() {
        let s = 8;
        for part in [
            AnimalPart::Model(0),
            AnimalPart::Interim(0),
            AnimalPart::Head {
                style: 0,
                facing_right: true,
            },
        ] {
            let g = animal_emissive_glyph(animal_glyph(part));
            for y in 0..s {
                for x in 0..s {
                    assert_eq!(front_texel(ANIMAL_PART_CLASS, g, x, y, s).tone(), TONE_EMIT);
                    assert_eq!(cap_texel(ANIMAL_PART_CLASS, g, x, y, s).tone(), TONE_EMIT);
                }
            }
            assert_ne!(
                front_texel(ANIMAL_PART_CLASS, animal_glyph(part), 1, 1, s).tone(),
                TONE_EMIT
            );
        }
    }

    #[test]
    fn flora_faces_are_resolved_in_the_same_atlas() {
        let crown = front_texel(2, GlyphId(1), 0, 2, 6);
        assert_eq!(crown.pigment(), Pigment::Secondary);
        assert_eq!(crown.tone(), TONE_UNDER_EDGE);
        assert_eq!(front_texel(4, GLYPH_NONE, 0, 0, 6).tone(), TONE_TRANSPARENT);
    }
}
