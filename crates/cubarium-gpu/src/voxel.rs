//! The voxel strip drawn per pixel by a slab walk in a fragment shader.
//!
//! # What this renderer is
//!
//! `crates/cubarium/src/voxel/present.rs` is the **definition of the picture** — the
//! elevated-orthographic projection, the autotiled block faces, the translucent water,
//! the plants as voxels — and it draws on the CPU, one rectangle at a time, far to near.
//! This is a second renderer of that same picture, and it works the other way round:
//! for every raster pixel, `voxel.frag` walks the depth slabs **near to far** and asks
//! which voxel and which face owns the pixel in each slab. The first opaque face ends the
//! walk; water faces met on the way blend front to back.
//!
//! That inversion is why the CPU's ownership bookkeeping is absent here. `nearer_owns`
//! and the `front_hidden`/`top_hidden` culls exist because the CPU paints with no depth
//! test and must not paint a translucent face twice; walking front to back gives the same
//! ownership by construction. The **rules** are ported — rim, bevel, chamfer, riser,
//! contour suppression, roof shadow, haze, water fill and its one-surface-per-pixel
//! clip, pore darkening, trunk cylinder, crown edge and skirt, sprout — and the
//! bookkeeping is not, except the two places the rules genuinely are local functions of
//! a cell and its nearer neighbour: the water body's `stop` row and the water top's
//! `nearer_owns`, both of which the shader evaluates the same way the CPU does.
//!
//! # What crosses the bus, per tick
//!
//! | what | format | size at 128×48×24 |
//! |---|---|---|
//! | one texel per voxel: material + plant part, free water, pore water, plant style | `R8G8B8A8_UINT` 3D | 576 KiB |
//! | the roof-gap table (`build_roof`), voxels to the nearest solid above | `R8_UINT` 3D | 144 KiB |
//! | the frame's plant styles: wood, crown, heart in linear light | `R32G32B32A32_SFLOAT` 3×256 | 12 KiB |
//!
//! The roof table is a texture rather than a column walk in the shader because the walk
//! costs up to `height` fetches per shaded face and the table costs one; both are
//! implemented ([`VoxelParams::roof_from_texture`]) and the measurement is in the report.
//! Nothing here is per **frame**: a frame is one draw of one full-screen triangle, so the
//! render rate is free of the tick rate and the raster can be any `px_per_voxel` over the
//! same upload.
//!
//! # The packing
//!
//! [`VoxelTexel`] is the one place the layout is written down, and `voxel.frag` unpacks
//! it the same way. Free water is quantised to 8 bits with a **floor**: any water at all
//! is at least 1, because the CPU presenter draws at least one row of water for any
//! `free` above its epsilon and a plain `round` would lose a film. The quantisation is
//! the one knowingly lossy step in the path (±1/510 in the fraction, which moves a
//! drawn water row only where `free · s` sits within that of a half-pixel).

use anyhow::{Context, Result, bail};
use ash::vk;

use crate::present::{FrameSource, PresentPass, TargetSlot};
use crate::render::{RASTER_FORMAT, framebuffer};
use crate::vk::{Gpu, HostBuffer, barrier};

const FULLSCREEN_VERT: &[u8] = include_bytes!("../shaders/fullscreen.vert.spv");
const VOXEL_FRAG: &[u8] = include_bytes!("../shaders/voxel.frag.spv");

/// Timestamp slots: top of pipe, after the uploads, after the world raster, bottom.
const QUERY_SLOTS: u32 = 4;

/// How many plant styles one frame may paint with.
///
/// A style is one stand's wood/crown/heart in linear light, already carrying its crown
/// fill and its wilt, so the count is the number of *distinct* colour triples a frame
/// needs — seed banks collapse to one per species, and identical stands collapse
/// together. The CPU presenter's own cap is 65 535 stands; a frame that really needs more
/// than this many distinct styles reuses style 0 for the rest and the sink says so.
pub const MAX_STYLES: usize = 256;
/// Slots in the generic organism face-glyph atlas. Glyph zero is the empty/default tile.
pub const MAX_GLYPHS: usize = 64;

/// Plant part classes, as the texel's `part` field carries them.
pub const PART_NONE: u8 = 0;
pub const PART_TRUNK: u8 = 1;
pub const PART_CROWN: u8 = 2;
pub const PART_CROWN_HEART: u8 = 3;
pub const PART_SPROUT: u8 = 4;

/// The **fauna** range of the same 3-bit field: `5..=7`, of which one is spoken for.
///
/// `PART_ANIMAL_INTERIM` is the interim glyph of `cubarium::voxel::animal` — a flat block
/// in the style's `wood`, with the rim and the cap a solid plant cell gets and nothing
/// else — so an animal rides the part/style path a plant already rides and this renderer
/// needs no new pass and no new texture. The two ids above it are deliberately unspoken
/// for: when the art direction says what a consumer looks like, its parts number from
/// here, and `part` has room for them without a format change.
pub const PART_ANIMAL_INTERIM: u8 = 5;
/// The first part id of the fauna range, for a reader that wants to ask "is this an
/// animal" rather than "which animal part is this".
pub const PART_FAUNA_FIRST: u8 = PART_ANIMAL_INTERIM;
/// A fallen log: horizontal dead wood cylinder.
pub const PART_LOG: u8 = 6;
/// Floor marks: leaf litter or carrion remnants.
pub const PART_FLOOR_MARK: u8 = 7;

// --- face textures -------------------------------------------------------------------

/// Which face of a voxel a texture is drawn on. The elevated camera never shows an
/// underside, so there is no third face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TexFace {
    /// A front face: `s × s` pixels on screen, row 0 at the top.
    Side,
    /// A top face: `s × rise` pixels on screen, row 0 at the **back** edge and row
    /// `rise − 1` against the front face's top row.
    Top,
}

/// The textured faces, in the atlas's slot order, as `(stem, face)`. `voxel.frag` numbers
/// them the same way (`TEX_*`), and a face file is `<stem>-<variant>.png`.
///
/// Terrain is `2 · (material − 1)` for the side and one more for the top, so the shader
/// finds a material's slots by arithmetic. `turf-side` is soil's side face where the top
/// is open to the sky: the same soil with a fringe hanging over its top rows. The plant
/// slots are drawn on baked model cells only, chosen by the style's role
/// ([`VoxelStyle::role`]).
pub const TEXTURE_SLOTS: [(&str, TexFace); 13] = [
    ("bedrock-side", TexFace::Side),
    ("bedrock-top", TexFace::Top),
    ("rock-side", TexFace::Side),
    ("rock-top", TexFace::Top),
    ("soil-side", TexFace::Side),
    ("soil-top", TexFace::Top),
    ("turf-side", TexFace::Side),
    ("bark-side", TexFace::Side),
    ("bark-top", TexFace::Top),
    ("leaf-side", TexFace::Side),
    ("leaf-top", TexFace::Top),
    ("drape-side", TexFace::Side),
    ("drape-top", TexFace::Top),
];

/// Variants per textured face. A voxel's variant is a hash of its position, so a wall
/// does not show one tile repeating.
pub const TEX_VARIANTS: u32 = 4;

/// A style's texture role, carried in [`VoxelStyle::wood`]'s alpha: which plant slots a
/// cell painted with it samples. Zero draws exactly as before textures existed — every
/// glyph, animal, log and floor mark.
pub const ROLE_NONE: u8 = 0;
/// A baked model's trunk: bark, opaque.
pub const ROLE_BARK: u8 = 1;
/// A baked model's foliage: the leaf cutout.
pub const ROLE_LEAF: u8 = 2;
/// A baked model's drape: the drape cutout.
pub const ROLE_DRAPE: u8 = 3;
/// A baked model's accent (bloom, fruit): opaque and untextured.
pub const ROLE_ACCENT: u8 = 4;

/// Every face texture at one level of detail, as one `R8G8B8A8_UNORM` atlas uploaded once.
///
/// Slot `k`, variant `v` is the `s × s` cell at `(v · s, k · s)`; a top face uses its
/// first `rise` rows. The texels are **sRGB-encoded**: a terrain texel is the face's
/// colour and replaces the material's strata colour; a generic plant texel's red channel
/// is a multiplier on the style colour, `r / 128` (so 128 leaves it alone), and its alpha
/// below 128 is a hole in a cutout.
///
/// The [`TEXTURE_SLOTS`] come first. **Named slots** follow them, one per face a caller
/// adds with [`VoxelTextures::add_slot`] (the species sets, `species/<species>/<role>-<face>`):
/// those are **direct colour** and are reached only through a style's face slots
/// ([`VoxelStyle::with_faces`]), never by arithmetic.
#[derive(Clone, Debug, PartialEq)]
pub struct VoxelTextures {
    pub s: u32,
    pub rise: u32,
    pub rgba: Vec<u8>,
    /// Per slot, a bit per variant already put.
    filled: Vec<u8>,
    /// The named slots after the fixed ones, in slot order: `(stem, face)`.
    named: Vec<(String, TexFace)>,
    /// The latticevine tiles at this level, [`VoxelTextures::vine_size`]: row
    /// `3 · set + density` (plain, climbing root, hanging root; bare, thin, full), column
    /// `16 · mask + exits`; row 9 holds the accents (bud, flower, fruit) in columns 0–2.
    /// **Direct colour**, sRGB-encoded, alpha a cutout.
    pub vine_rgba: Vec<u8>,
    /// Whether any vine tile was put: the shader draws the tile layer only then.
    pub vine_on: bool,
}

/// Columns of the vine atlas: sixteen per neighbour mask, one per exits pattern.
pub const VINE_COLS: u32 = 256;
/// Rows of the vine atlas: three densities of plain, climbing and hanging tiles, then the
/// accents.
pub const VINE_ROWS: u32 = 10;

impl VoxelTextures {
    /// An atlas with nothing in it: every face draws as it did before textures.
    pub fn empty(s: u32, rise: u32) -> VoxelTextures {
        let (w, h) = Self::size(s);
        VoxelTextures {
            s,
            rise,
            rgba: vec![0; (w * h * 4) as usize],
            filled: vec![0; TEXTURE_SLOTS.len()],
            named: Vec::new(),
            vine_rgba: vec![0; (VINE_COLS * VINE_ROWS * s * s * 4) as usize],
            vine_on: false,
        }
    }

    /// The vine atlas's size in texels for `s` px per voxel.
    pub fn vine_size(s: u32) -> (u32, u32) {
        (VINE_COLS * s, VINE_ROWS * s)
    }

    /// Copy one `s × s` vine tile (RGBA8) into row `row`, column `col`.
    pub fn put_vine(&mut self, row: u32, col: u32, rgba: &[u8]) -> Result<()> {
        let s = self.s;
        if row >= VINE_ROWS || col >= VINE_COLS || rgba.len() != (s * s * 4) as usize {
            bail!("no vine tile at row {row} column {col} of {} bytes", rgba.len());
        }
        let aw = VINE_COLS * s;
        for r in 0..s {
            let src = (r * s * 4) as usize;
            let dst = (((row * s + r) * aw + col * s) * 4) as usize;
            self.vine_rgba[dst..dst + (s * 4) as usize]
                .copy_from_slice(&rgba[src..src + (s * 4) as usize]);
        }
        self.vine_on = true;
        Ok(())
    }

    /// Bit `k` set: fixed slot `k` ([`TEXTURE_SLOTS`]) holds every variant, and the
    /// shader samples it. Named slots are not in the mask: a style names them.
    pub fn mask(&self) -> u32 {
        let all = (1u8 << TEX_VARIANTS) - 1;
        self.filled[..TEXTURE_SLOTS.len()]
            .iter()
            .enumerate()
            .filter(|(_, f)| **f == all)
            .fold(0, |m, (k, _)| m | 1 << k)
    }

    /// The fixed atlas's size in texels for `s` px per voxel: the [`TEXTURE_SLOTS`] alone.
    pub fn size(s: u32) -> (u32, u32) {
        (TEX_VARIANTS * s, TEXTURE_SLOTS.len() as u32 * s)
    }

    /// This atlas's size in texels, named slots and all: what the renderer uploads.
    pub fn atlas_size(&self) -> (u32, u32) {
        (TEX_VARIANTS * self.s, self.slots() as u32 * self.s)
    }

    /// Fixed and named slots.
    pub fn slots(&self) -> usize {
        TEXTURE_SLOTS.len() + self.named.len()
    }

    /// Add an empty named slot after every other and return its index: its texels are
    /// zero (a hole) until its variants are put.
    pub fn add_slot(&mut self, stem: &str, face: TexFace) -> usize {
        let slot = self.slots();
        self.named.push((stem.to_string(), face));
        self.filled.push(0);
        let (aw, ah) = self.atlas_size();
        self.rgba.resize((aw * ah * 4) as usize, 0);
        slot
    }

    /// The slot of a named face, if one was added.
    pub fn slot_named(&self, stem: &str) -> Option<usize> {
        self.named
            .iter()
            .position(|(n, _)| n == stem)
            .map(|k| TEXTURE_SLOTS.len() + k)
    }

    /// Every named slot, as `(slot, stem)`.
    pub fn named(&self) -> impl Iterator<Item = (usize, &str)> {
        self.named
            .iter()
            .enumerate()
            .map(|(k, (n, _))| (TEXTURE_SLOTS.len() + k, n.as_str()))
    }

    fn slot_face(&self, slot: usize) -> (&str, TexFace) {
        match TEXTURE_SLOTS.get(slot) {
            Some(&(stem, face)) => (stem, face),
            None => {
                let (stem, face) = &self.named[slot - TEXTURE_SLOTS.len()];
                (stem.as_str(), *face)
            }
        }
    }

    /// The size of one face image of `slot` at this level.
    pub fn face_size(&self, slot: usize) -> (u32, u32) {
        match self.slot_face(slot).1 {
            TexFace::Side => (self.s, self.s),
            TexFace::Top => (self.s, self.rise),
        }
    }

    /// Copy one face image (RGBA8, [`VoxelTextures::face_size`]) into its cell. The slot
    /// is switched on once all its variants have been put.
    pub fn put(&mut self, slot: usize, variant: u32, rgba: &[u8]) -> Result<()> {
        if slot >= self.slots() || variant >= TEX_VARIANTS {
            bail!("no texture slot {slot} variant {variant}");
        }
        let (fw, fh) = self.face_size(slot);
        if rgba.len() != (fw * fh * 4) as usize {
            bail!(
                "{} variant {variant} is {} bytes, not the {fw}x{fh} face this level draws",
                self.slot_face(slot).0,
                rgba.len()
            );
        }
        let (aw, _) = self.atlas_size();
        let (x0, y0) = (variant * self.s, slot as u32 * self.s);
        for row in 0..fh {
            let src = (row * fw * 4) as usize;
            let dst = (((y0 + row) * aw + x0) * 4) as usize;
            self.rgba[dst..dst + (fw * 4) as usize].copy_from_slice(&rgba[src..src + (fw * 4) as usize]);
        }
        self.filled[slot] |= 1 << variant;
        Ok(())
    }
}

/// One voxel, as the shader reads it: `R8G8B8A8_UINT`.
///
/// * `r` — material id in bits 0–1, part class in bits 2–4, generic glyph id in bits 5–7;
/// * `g` — free water as a fraction of the void volume, `0` for dry and `1..=255`
///   for any water at all (see the module header on the floor);
/// * `b` — pore water as a fraction of the pore capacity;
/// * `a` — the plant style index, meaningful only when `part != PART_NONE`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct VoxelTexel(pub [u8; 4]);

impl VoxelTexel {
    /// Pack one voxel. `material` is `Material as u8` (0 air … 3 soil), `part` one of the
    /// `PART_*` classes, `free` and `pore` fractions in `0..=1`, and `dry` says the cell
    /// holds no water at all — which is the presenter's `free <= WATER_EPSILON`, decided
    /// by the caller because the epsilon is the presenter's constant and not this
    /// format's.
    pub fn pack(material: u8, part: u8, free: f32, dry: bool, pore: f32, style: u8) -> VoxelTexel {
        Self::pack_glyph(material, part, 0, free, dry, pore, style)
    }

    #[inline]
    pub fn pack_glyph(
        material: u8,
        part: u8,
        glyph: u8,
        free: f32,
        dry: bool,
        pore: f32,
        style: u8,
    ) -> VoxelTexel {
        let g = if dry { 0 } else { quantise(free).max(1) };
        VoxelTexel([
            (material & 0x03) | ((part & 0x07) << 2) | ((glyph & 0x07) << 5),
            g,
            quantise(pore),
            style,
        ])
    }

    pub fn material(self) -> u8 {
        self.0[0] & 0x03
    }

    pub fn part(self) -> u8 {
        (self.0[0] >> 2) & 0x07
    }

    pub fn glyph(self) -> u8 {
        self.0[0] >> 5
    }

    /// The free-water fraction the shader sees, which is the quantised one.
    pub fn free(self) -> f32 {
        f32::from(self.0[1]) / 255.0
    }

    /// Whether the cell holds no water at all.
    pub fn dry(self) -> bool {
        self.0[1] == 0
    }

    pub fn pore(self) -> f32 {
        f32::from(self.0[2]) / 255.0
    }

    pub fn style(self) -> u8 {
        self.0[3]
    }

    /// An **air** voxel with no part, carrying a latticevine cell in the fields such a
    /// voxel leaves unused: the glyph id (nonzero marks the vine), `b` (pore water, which
    /// air has none of) and `a` (the style, read only under a part). The layout is
    /// `cubarium::voxel::vine::VineCell::texel_bytes`'s, and `voxel.frag` reads it back.
    pub fn with_vine(self, glyph: u8, b: u8, a: u8) -> VoxelTexel {
        let mut t = self;
        t.0[0] = (t.0[0] & 0x1f) | (glyph & 7) << 5;
        t.0[2] = b;
        t.0[3] = a;
        t
    }
}

#[inline]
fn quantise(v: f32) -> u8 {
    if !v.is_finite() || v <= 0.0 {
        return 0;
    }
    (v.min(1.0) * 255.0 + 0.5) as u8
}

/// One plant style as the style texture carries it: wood, crown, heart, in linear light.
/// `wood`'s alpha is the style's texture role (`ROLE_*`); `crown`'s and `heart`'s alphas
/// are its **face slots**, the named atlas slot its side and top faces draw with, plus
/// one (`0` is none: the role's generic slot, tinted by the style colour).
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct VoxelStyle {
    pub wood: [f32; 4],
    pub crown: [f32; 4],
    pub heart: [f32; 4],
}

impl VoxelStyle {
    pub fn new(wood: [f32; 3], crown: [f32; 3], heart: [f32; 3]) -> VoxelStyle {
        let v = |c: [f32; 3]| [c[0], c[1], c[2], 0.0];
        VoxelStyle {
            wood: [wood[0], wood[1], wood[2], f32::from(ROLE_NONE)],
            crown: v(crown),
            heart: v(heart),
        }
    }

    /// The same style, its side and top faces drawn with these named atlas slots
    /// ([`VoxelTextures::add_slot`]) in direct colour. `None` keeps the role's generic slot.
    pub fn with_faces(mut self, faces: [Option<u16>; 2]) -> VoxelStyle {
        self.crown[3] = faces[0].map_or(0.0, |s| f32::from(s) + 1.0);
        self.heart[3] = faces[1].map_or(0.0, |s| f32::from(s) + 1.0);
        self
    }

    /// The named slots the side and top faces draw with.
    pub fn faces(&self) -> [Option<u16>; 2] {
        let f = |a: f32| (a >= 0.5).then(|| a as u16 - 1);
        [f(self.crown[3]), f(self.heart[3])]
    }

    /// The same colours, drawn with the textures of `role` (`ROLE_*`).
    pub fn with_role(mut self, role: u8) -> VoxelStyle {
        self.wood[3] = f32::from(role);
        self
    }

    /// The texture role this style draws with.
    pub fn role(&self) -> u8 {
        self.wood[3] as u8
    }
}

/// Everything the picture is a function of besides the voxels: the projection, the
/// strata palette and every shading constant the CPU presenter names.
///
/// There are no defaults on purpose. The values live in
/// `crates/cubarium/src/voxel/present.rs`, which is the definition of the picture; a
/// second copy here with its own numbers is exactly how two renderers drift apart, so
/// the caller fills every field from that module's public constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoxelParams {
    /// Pixels per voxel edge.
    pub s: u32,
    /// Pixels a voxel of depth lifts the image.
    pub rise: u32,
    /// Row one past the bottom of the `y = 0, z = 0` front face.
    pub base: i32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub raster_w: u32,
    pub raster_h: u32,
    pub haze: f32,
    pub water_alpha: f32,
    /// Read the roof gap from the uploaded table (`true`) or walk the column in the
    /// shader (`false`). The picture is identical; only the cost differs.
    pub roof_from_texture: bool,
    pub sky: [f32; 3],
    pub sky_horizon: [f32; 3],
    pub atmosphere: f32,
    pub rain_tick: f32,
    pub dither: f32,
    pub sky_gradient: bool,
    pub bedrock: [f32; 3],
    pub rock: [f32; 3],
    pub soil: [f32; 3],
    pub water_deep: [f32; 3],
    pub water_surface: [f32; 3],
    pub light: [f32; 3],
    pub haze_colour: [f32; 3],
    pub top_gain: f32,
    pub top_tint: f32,
    pub top_back: f32,
    pub rim: f32,
    pub edge_dark: f32,
    pub top_edge: f32,
    pub riser_lean: f32,
    pub wet: f32,
    pub roof_light: f32,
    pub roof_falloff: f32,
    pub skin_alpha_gain: f32,
    pub water_top_alpha: f32,
    pub plant_top_gain: f32,
    pub plant_top_tint: f32,
    pub plant_rim: f32,
    pub crown_edge: f32,
    pub crown_under: f32,
    pub trunk_shade: [f32; 2],
    pub trunk_light_at: f32,
    /// `lighting = "lit"`: the pipeline is built with `voxel.frag`'s `LIT` specialisation
    /// constant on, and the sky and canopy planes are allocated and uploaded. Off (the
    /// `flat` tier) the pipeline runs the flat shader code and neither plane exists.
    /// Fixed for a renderer's life.
    pub lit: bool,
    /// The lit tier's ambient: what full light is worth (`ambient_gain`), the darkest
    /// rung as a fraction of it (`ambient_floor`), how many rungs the ladder has
    /// (`light_levels`, at least 2), and how far a fully occluded corner darkens
    /// (`ao_strength`).
    pub ambient_gain: f32,
    pub ambient_floor: f32,
    pub light_levels: u32,
    pub ao_strength: f32,
    /// The ambient light's colour, in linear light: the sky's hue at unit luminance.
    pub ambient_colour: [f32; 3],
    /// The lit tier's sun: the unit direction **toward** it (x right, y up, z into the
    /// scene), or zero for no sun. A texel the sun reaches keeps its ambient rung; one it
    /// does not (a cast shadow, or a face turned away) is one rung darker.
    pub sun: [f32; 3],
    /// How far a sunlit texel leans toward `light`, times N·L. `0` is purely
    /// multiplicative.
    pub sun_tint: f32,
}

/// Foliage steps the canopy plane holds per column ([`VoxelStaging::canopy`]).
pub const CANOPY_STEPS: usize = 4;

/// Where each plane starts in a staging buffer ([`VoxelRenderer::offsets`]).
const AT_VOXELS: usize = 0;
const AT_ROOF: usize = 1;
const AT_STYLES: usize = 2;
const AT_GLYPHS: usize = 3;
const AT_SKY: usize = 4;
const AT_CANOPY: usize = 5;
const AT_END: usize = 6;

impl VoxelParams {
    fn validate(&self) -> Result<()> {
        if self.s == 0 || self.rise == 0 || self.rise > self.s {
            bail!(
                "px_per_voxel {} and rise {} are not a projection",
                self.s,
                self.rise
            );
        }
        if self.width == 0 || self.height == 0 || self.depth == 0 {
            bail!(
                "an empty world: {}x{}x{}",
                self.width,
                self.height,
                self.depth
            );
        }
        if self.raster_w != self.width * self.s {
            bail!(
                "the raster is {} px wide but {} voxels at {} px each is {}",
                self.raster_w,
                self.width,
                self.s,
                self.width * self.s
            );
        }
        if self.raster_h == 0 {
            bail!("a zero-height raster");
        }
        Ok(())
    }

    /// Voxels in the world: one texel each.
    pub fn voxel_count(&self) -> usize {
        self.width as usize * self.height as usize * self.depth as usize
    }

    /// Bytes one tick uploads: world planes, styles and the small generic glyph atlas,
    /// and in the lit tier the canopy plane (the sky plane goes up only when the terrain
    /// moves, as the roof does).
    pub fn upload_bytes(&self) -> usize {
        self.voxel_count() * 5
            + MAX_STYLES * std::mem::size_of::<VoxelStyle>()
            + self.glyph_bytes()
            + self.canopy_bytes()
    }

    pub fn glyph_bytes(&self) -> usize {
        self.s as usize * (self.s + self.rise) as usize * MAX_GLYPHS
    }

    /// The sky plane's bytes: one per voxel in the lit tier, none in the flat one.
    pub fn sky_bytes(&self) -> usize {
        if self.lit { self.voxel_count() } else { 0 }
    }

    /// The canopy plane's bytes: [`CANOPY_STEPS`] `(cell, transmission)` pairs per column
    /// in the lit tier, none in the flat one.
    pub fn canopy_bytes(&self) -> usize {
        if self.lit {
            self.width as usize * self.depth as usize * CANOPY_STEPS * 2
        } else {
            0
        }
    }

    /// The canopy image's extent: two RGBA8 texels per column, `(x, 2z)` and `(x, 2z+1)`.
    fn canopy_extent(&self) -> (u32, u32) {
        if self.lit {
            (self.width, self.depth * (CANOPY_STEPS as u32 / 2))
        } else {
            (1, 1)
        }
    }

    /// The uniform block, in the layout `voxel.frag` declares, for an atlas holding the
    /// slots in `tex_mask`.
    fn uniforms(&self, tex_mask: u32, vine_on: bool) -> VoxelUniforms {
        let v = |c: [f32; 3]| [c[0], c[1], c[2], 0.0];
        VoxelUniforms {
            geom: [
                self.s as i32,
                self.rise as i32,
                self.base,
                i32::from(self.roof_from_texture),
            ],
            extent: [
                self.width as i32,
                self.height as i32,
                self.depth as i32,
                self.raster_h as i32,
            ],
            knobs: [self.haze, self.water_alpha, self.atmosphere, self.rain_tick],
            sky: v(self.sky),
            sky_horizon: v(self.sky_horizon),
            bedrock: v(self.bedrock),
            rock: v(self.rock),
            soil: v(self.soil),
            water_deep: v(self.water_deep),
            water_surface: v(self.water_surface),
            light: v(self.light),
            haze_colour: v(self.haze_colour),
            shade_a: [self.top_gain, self.top_tint, self.top_back, self.rim],
            shade_b: [self.edge_dark, self.top_edge, self.riser_lean, self.wet],
            roof: [
                self.roof_light,
                self.roof_falloff,
                self.dither,
                if self.sky_gradient { 1.0 } else { 0.0 },
            ],
            water: [self.skin_alpha_gain, self.water_top_alpha, 0.0, 0.0],
            plant_a: [
                self.plant_top_gain,
                self.plant_top_tint,
                self.plant_rim,
                self.crown_edge,
            ],
            plant_b: [
                self.crown_under,
                self.trunk_shade[0],
                self.trunk_shade[1],
                self.trunk_light_at,
            ],
            tex: [tex_mask as i32, i32::from(vine_on), 0, 0],
            light_k: [
                self.ambient_gain,
                self.ambient_floor,
                self.light_levels.max(2) as f32,
                self.ao_strength,
            ],
            ambient: v(self.ambient_colour),
            sun: [self.sun[0], self.sun[1], self.sun[2], self.sun_tint],
        }
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct VoxelUniforms {
    geom: [i32; 4],
    extent: [i32; 4],
    knobs: [f32; 4],
    sky: [f32; 4],
    sky_horizon: [f32; 4],
    bedrock: [f32; 4],
    rock: [f32; 4],
    soil: [f32; 4],
    water_deep: [f32; 4],
    water_surface: [f32; 4],
    light: [f32; 4],
    haze_colour: [f32; 4],
    shade_a: [f32; 4],
    shade_b: [f32; 4],
    roof: [f32; 4],
    water: [f32; 4],
    plant_a: [f32; 4],
    plant_b: [f32; 4],
    /// The face textures present (`VoxelTextures::mask`), whether the vine tiles are, then
    /// padding.
    tex: [i32; 4],
    /// The lit tier: ambient gain, ambient floor, ladder rungs, AO strength.
    light_k: [f32; 4],
    /// The lit tier's ambient colour.
    ambient: [f32; 4],
    /// The lit tier's sun: the unit direction toward it, and the sunlit tint.
    sun: [f32; 4],
}

/// Where one tick's world is written, straight into mapped memory.
///
/// The slices are the staging buffer itself, so the packer on the CPU writes the
/// voxels once instead of filling a `Vec` and copying it. Voxels and the roof table are
/// indexed `(z · height + y) · width + x` — texture upload order, not the core's
/// `Config::index`.
pub struct VoxelStaging<'a> {
    pub voxels: &'a mut [VoxelTexel],
    pub roof: &'a mut [u8],
    pub styles: &'a mut [VoxelStyle],
    /// `s × (s + rise)` face texels per glyph: front rows followed by cap rows.
    pub glyphs: &'a mut [u8],
    /// The lit tier's sky plane, in the voxels' order: at each open cell, the core's
    /// `sky_visibility` of the cell under it (the fan from the foot of the open cell), at
    /// 255 for open sky. Empty in the flat tier.
    pub sky: &'a mut [u8],
    /// The lit tier's canopy plane: per column `(x, z)`, [`CANOPY_STEPS`] pairs of
    /// `(cell, cumulative transmission)`, highest cell first, at byte
    /// `((z · 2 + k / 2) · width + x) · 4 + (k % 2) · 2`. A face of a voxel at height `y`
    /// takes the transmission of the last step whose cell is above `y`; a cell of 0 ends
    /// the list. Written every pack. Empty in the flat tier.
    pub canopy: &'a mut [u8],
    /// Whether this pack must write [`VoxelStaging::roof`], [`VoxelStaging::glyphs`] and
    /// [`VoxelStaging::sky`] (indexed [`PLANE_ROOF`], [`PLANE_GLYPHS`], [`PLANE_SKY`]). A
    /// plane marked false already holds the content its key names and must be left
    /// alone. See [`SlowPlanes`].
    pub write: [bool; SLOW_PLANES],
}

impl VoxelStaging<'_> {
    /// The index of `(x, y, z)` in [`VoxelStaging::voxels`] and [`VoxelStaging::roof`].
    #[inline]
    pub fn index(width: u32, height: u32, x: u32, y: u32, z: u32) -> usize {
        (z as usize * height as usize + y as usize) * width as usize + x as usize
    }
}

/// Frames the CPU may have in hand at once, and so the width of every per-frame
/// resource: the staging buffers, the uniform blocks and the timestamp sets.
///
/// **Two.** One frame is on the GPU while the next is packed and recorded; a third would
/// only help if a third frame could be outstanding, and the presenter never lets one be —
/// it replaces the frame waiting to go up rather than queueing another behind it.
pub const STAGING_RING: usize = 2;

/// One recorded frame's claim on the ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recorded {
    /// The staging buffer it uploads from, or `None` when it re-presents a world the GPU
    /// already holds.
    pub staging: Option<usize>,
    /// The slot it reads its uniforms from and writes its timestamps into. Everything the
    /// CPU writes for one frame is indexed by this, so a frame the GPU is reading and the
    /// frame being recorded never share a byte.
    pub slot: usize,
}

/// Which per-frame resources the next frame may write, and which ones recorded frames
/// still read.
///
/// The whole hazard of packing off the present thread lives here: a
/// `vkCmdCopyBufferToImage` reads the staging buffer when the GPU runs it, not when it is
/// recorded, so the CPU may not write anything a recorded-but-unfinished frame names. The
/// rules are three:
///
/// * a pack overwrites the buffer of an earlier pack that has not been recorded yet —
///   nothing has read it, and the newer world is the one that should go up;
/// * a buffer a recorded frame names is untouchable until that frame retires;
/// * a frame that is *displaced* before it is ever submitted gives its buffer back and
///   the pack it carried is owed again, or the world would never reach the GPU.
#[derive(Debug)]
pub struct StagingRing {
    n: usize,
    /// The buffer holding a pack the GPU has not been shown yet.
    packed: Option<usize>,
    /// One entry per recorded frame, oldest first.
    recorded: std::collections::VecDeque<Recorded>,
}

impl StagingRing {
    pub fn new(n: usize) -> StagingRing {
        StagingRing {
            n: n.max(1),
            packed: None,
            recorded: std::collections::VecDeque::new(),
        }
    }

    /// The buffer the next pack may write, or `None` when every one of them is spoken for
    /// by a frame that has not retired — then the pack is skipped and the world goes up
    /// next tick, one tick stale, which is the cheap half of the trade.
    pub fn for_pack(&self) -> Option<usize> {
        if let Some(i) = self.packed {
            return Some(i);
        }
        (0..self.n).find(|i| !self.recorded.iter().any(|f| f.staging == Some(*i)))
    }

    /// A pack has been written into `i`.
    pub fn packed(&mut self, i: usize) {
        self.packed = Some(i);
    }

    /// A frame is being recorded: it uploads whatever was packed and takes the first slot
    /// no outstanding frame is using, and both are its own until it retires.
    pub fn record(&mut self) -> Recorded {
        let slot = (0..self.n)
            .find(|s| !self.recorded.iter().any(|f| f.slot == *s))
            // Only reachable if the caller records more frames at once than the ring is
            // wide, which is what `in_flight` is there to stop. Sharing the oldest slot
            // is the least bad answer: a stale uniform, never a wild pointer.
            .unwrap_or(0);
        let frame = Recorded {
            staging: self.packed.take(),
            slot,
        };
        self.recorded.push_back(frame);
        frame
    }

    /// The oldest recorded frame has completed on the GPU. Frames retire in the order
    /// they were submitted, so the oldest is always the one that finished.
    pub fn retire(&mut self) -> Option<Recorded> {
        self.recorded.pop_front()
    }

    /// The newest recorded frame will never be submitted — a fresher one displaced it.
    /// Its buffer is free again and the upload it carried is owed again.
    pub fn discard(&mut self) -> Option<Recorded> {
        let frame = self.recorded.pop_back()?;
        if frame.staging.is_some() {
            self.packed = frame.staging;
        }
        Some(frame)
    }

    /// How many frames are recorded and not yet retired.
    pub fn in_flight(&self) -> usize {
        self.recorded.len()
    }

    /// The buffers recorded frames are reading, for the tests.
    #[cfg(test)]
    fn reading(&self) -> Vec<usize> {
        self.recorded.iter().filter_map(|f| f.staging).collect()
    }
}

/// The roof table's place in [`SlowPlanes`] and in a pack's keys.
pub const PLANE_ROOF: usize = 0;
/// The glyph atlas's place in [`SlowPlanes`] and in a pack's keys.
pub const PLANE_GLYPHS: usize = 1;
/// The lit tier's sky plane's place in [`SlowPlanes`] and in a pack's keys.
pub const PLANE_SKY: usize = 2;
/// How many planes are written and uploaded only when they change.
pub const SLOW_PLANES: usize = 3;

/// What the **slow planes** — the roof table, the glyph atlas and the sky plane — hold,
/// per staging buffer and in the images, so a pack writes them and a frame uploads them
/// only when they have changed.
///
/// The roof and the sky are functions of the terrain and the atlas of the projection, so
/// most ticks none of them moves. The caller names each plane's content by a key (equal key, equal
/// bytes); this remembers which key each buffer holds and which key the images will hold
/// once every recorded frame has run. Two buffers alternate, so a changed roof is written
/// into each of them once and uploaded once.
///
/// A displaced frame never ran, so after a discard the images' contents are unknown and
/// the next upload copies both planes again. That is the safe answer, and a discard is
/// rare enough that its cost does not matter.
#[derive(Debug)]
pub struct SlowPlanes {
    buffers: Vec<[Option<u64>; SLOW_PLANES]>,
    images: [Option<u64>; SLOW_PLANES],
}

impl SlowPlanes {
    pub fn new(n: usize) -> SlowPlanes {
        SlowPlanes {
            buffers: vec![[None; SLOW_PLANES]; n.max(1)],
            images: [None; SLOW_PLANES],
        }
    }

    /// A pack into `buffer` will hold `keys`: which planes it must write. The buffer is
    /// taken to hold them from here on, so the pack must write every plane named.
    pub fn pack(&mut self, buffer: usize, keys: [u64; SLOW_PLANES]) -> [bool; SLOW_PLANES] {
        let held = &mut self.buffers[buffer];
        let write = std::array::from_fn(|p| held[p] != Some(keys[p]));
        *held = keys.map(Some);
        write
    }

    /// A frame uploads from `buffer`: which planes it must copy into their images.
    pub fn upload(&mut self, buffer: usize) -> [bool; SLOW_PLANES] {
        let held = self.buffers[buffer];
        let copy = std::array::from_fn(|p| held[p].is_none() || held[p] != self.images[p]);
        self.images = held;
        copy
    }

    /// A frame that carried an upload was displaced before it ran.
    pub fn forget_images(&mut self) {
        self.images = [None; SLOW_PLANES];
    }
}

/// The voxel strip's renderer: one texture uploaded per tick, one full-screen draw per
/// frame, and the shared [`PresentPass`] onto a target.
pub struct VoxelRenderer {
    params: VoxelParams,
    // --- the world raster ---
    raster_image: vk::Image,
    raster_memory: vk::DeviceMemory,
    raster_view: vk::ImageView,
    raster_framebuffer: vk::Framebuffer,
    raster_pass: vk::RenderPass,
    // --- the uploaded world ---
    voxel_image: vk::Image,
    voxel_memory: vk::DeviceMemory,
    voxel_view: vk::ImageView,
    roof_image: vk::Image,
    roof_memory: vk::DeviceMemory,
    roof_view: vk::ImageView,
    style_image: vk::Image,
    style_memory: vk::DeviceMemory,
    style_view: vk::ImageView,
    glyph_image: vk::Image,
    glyph_memory: vk::DeviceMemory,
    glyph_view: vk::ImageView,
    /// The face-texture atlas at this `s`, uploaded once at construction.
    tex_image: vk::Image,
    tex_memory: vk::DeviceMemory,
    tex_view: vk::ImageView,
    /// Which of its slots the shader samples.
    tex_mask: u32,
    /// The vine tile atlas at this `s`, uploaded once beside it.
    vine_image: vk::Image,
    vine_memory: vk::DeviceMemory,
    vine_view: vk::ImageView,
    vine_on: bool,
    /// The lit tier's sky plane (`R8_UINT`, one texel per voxel) and canopy plane
    /// (`R8G8B8A8_UINT`, two texels per column); 1-texel stand-ins in the flat tier,
    /// which never reads them.
    sky_image: vk::Image,
    sky_memory: vk::DeviceMemory,
    sky_view: vk::ImageView,
    canopy_image: vk::Image,
    canopy_memory: vk::DeviceMemory,
    canopy_view: vk::ImageView,
    staging: Vec<HostBuffer>,
    /// Which staging buffer the next pack may use and which the GPU is still reading.
    ring: StagingRing,
    /// Which roof table, glyph atlas and sky plane each staging buffer and the images
    /// hold.
    slow: SlowPlanes,
    /// Byte offsets into each of [`VoxelRenderer::staging`] of the six planes, indexed
    /// by the `AT_*` constants; the last is the buffer's size.
    offsets: [u64; 7],
    /// Whether a staged world is waiting to be uploaded.
    dirty: bool,
    /// Whether the world raster already holds the picture the next frame would draw.
    ///
    /// **A frame between two ticks is the same picture.** This renderer uploads one
    /// texture per tick and interpolates nothing across it, so at 60 fps over a 20 Hz
    /// world two frames in three would redraw, texel for texel, what is already in the
    /// raster. They do not: a frame whose world has not moved records only the present
    /// pass onto its slot, and the slab walk and the upload are skipped. Anything that
    /// changes the picture — a pack, a parameter, the weather, the founding pulse —
    /// clears this.
    raster_current: bool,
    /// What a frame recorded now would show, as a number that moves whenever the picture
    /// would: a pack, a parameter, the weather. Two recordings at the same version are
    /// the same frame, so the second one is not worth making.
    version: u64,
    /// Whether the last recorded frame redrew the raster or only re-presented it. The
    /// presenter reports the two costs apart, which is what says whether the panel's
    /// ceiling is the world pass or the upscale onto it.
    redrew: bool,
    /// Whether anything has ever been staged: a frame before the first upload would
    /// sample undefined texels, so it is refused rather than drawn.
    staged: bool,
    /// One uniform block per ring slot, bound at a dynamic offset: the frame the GPU is
    /// reading and the frame being recorded never share one.
    uniforms: HostBuffer,
    /// Bytes between those blocks — `minUniformBufferOffsetAlignment`, measured.
    uniform_stride: u64,
    /// The slot of the last frame that retired, i.e. the one whose timestamps are the
    /// newest complete set. `None` before the first frame finishes.
    last_done: Option<usize>,
    nearest: vk::Sampler,
    set_layout: vk::DescriptorSetLayout,
    set: vk::DescriptorSet,
    pool: vk::DescriptorPool,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    present: PresentPass,
    pub command_pool: vk::CommandPool,
    queries: vk::QueryPool,
}

impl VoxelRenderer {
    /// Build the pass and allocate every texture for a world of this shape. One call per
    /// process; the world's *contents* arrive through [`VoxelRenderer::stage`].
    ///
    /// `textures` is the face-texture atlas at this projection's level, uploaded here and
    /// never again; [`VoxelTextures::empty`] draws every face as before textures.
    pub fn new(gpu: &Gpu, params: VoxelParams, textures: &VoxelTextures) -> Result<VoxelRenderer> {
        params.validate()?;
        if (textures.s, textures.rise) != (params.s, params.rise)
            || textures.rgba.len() != {
                let (w, h) = textures.atlas_size();
                (w * h * 4) as usize
            }
        {
            bail!(
                "the face textures are for {} px per voxel and rise {}, the projection is {} and {}",
                textures.s,
                textures.rise,
                params.s,
                params.rise
            );
        }
        let d = &gpu.device;
        let command_pool = unsafe {
            d.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(gpu.queue_family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }?;
        let queries = unsafe {
            d.create_query_pool(
                &vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::TIMESTAMP)
                    // One set of timestamps per ring slot: two frames may be in the
                    // command stream at once and each writes its own.
                    .query_count(QUERY_SLOTS * STAGING_RING as u32),
                None,
            )
        }?;

        let raster_pass = crate::render::colour_pass(
            d,
            RASTER_FORMAT,
            vk::AttachmentLoadOp::DONT_CARE,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;
        let (raster_image, raster_memory) = gpu.image(
            params.raster_w,
            params.raster_h,
            RASTER_FORMAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_SRC,
        )?;
        let raster_view = gpu.view(raster_image, RASTER_FORMAT)?;
        let raster_framebuffer = framebuffer(
            d,
            raster_pass,
            raster_view,
            params.raster_w,
            params.raster_h,
        )?;

        let limit = unsafe { gpu.instance.get_physical_device_properties(gpu.pdev) }
            .limits
            .max_image_dimension3_d;
        let side = params.width.max(params.height).max(params.depth);
        if side > limit {
            bail!(
                "a {}x{}x{} world needs a 3D image of side {side}; this device allows {limit}",
                params.width,
                params.height,
                params.depth
            );
        }
        let (voxel_image, voxel_memory) = image_3d(
            gpu,
            params.width,
            params.height,
            params.depth,
            vk::Format::R8G8B8A8_UINT,
        )?;
        let (roof_image, roof_memory) = image_3d(
            gpu,
            params.width,
            params.height,
            params.depth,
            vk::Format::R8_UINT,
        )?;
        let (style_image, style_memory) = gpu.image(
            3,
            MAX_STYLES as u32,
            vk::Format::R32G32B32A32_SFLOAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        )?;
        let (glyph_image, glyph_memory) = gpu.image(
            params.s,
            (params.s + params.rise) * MAX_GLYPHS as u32,
            vk::Format::R8_UINT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        )?;
        let (tex_w, tex_h) = textures.atlas_size();
        let (tex_image, tex_memory) = gpu.image(
            tex_w,
            tex_h,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        )?;
        // The lit tier's planes. The flat tier never reads them, so it gets one texel of
        // each: its memory and its uploads stay what they were before the lit tier.
        let (sky_image, sky_memory) = if params.lit {
            image_3d(
                gpu,
                params.width,
                params.height,
                params.depth,
                vk::Format::R8_UINT,
            )?
        } else {
            image_3d(gpu, 1, 1, 1, vk::Format::R8_UINT)?
        };
        let (canopy_w, canopy_h) = params.canopy_extent();
        let (canopy_image, canopy_memory) = gpu.image(
            canopy_w,
            canopy_h,
            vk::Format::R8G8B8A8_UINT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        )?;
        let (vine_w, vine_h) = VoxelTextures::vine_size(params.s);
        let (vine_image, vine_memory) = gpu.image(
            vine_w,
            vine_h,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        )?;
        // Both atlases go up once, through a buffer that lives only for this copy.
        let vine_at = align16(textures.rgba.len() as u64);
        let tex_staging = gpu.host_buffer(
            vine_at + textures.vine_rgba.len() as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
        )?;
        tex_staging.write(&textures.rgba);
        tex_staging.write_bytes_at(vine_at, &textures.vine_rgba);
        gpu.one_shot(command_pool, |cb| unsafe {
            for image in [
                voxel_image,
                roof_image,
                style_image,
                glyph_image,
                sky_image,
                canopy_image,
            ] {
                barrier(
                    d,
                    cb,
                    image,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );
            }
            barrier(
                d,
                cb,
                tex_image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );
            d.cmd_copy_buffer_to_image(
                cb,
                tex_staging.buffer,
                tex_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy::default()
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D {
                        width: tex_w,
                        height: tex_h,
                        depth: 1,
                    })],
            );
            barrier(
                d,
                cb,
                tex_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
            barrier(
                d,
                cb,
                vine_image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );
            d.cmd_copy_buffer_to_image(
                cb,
                tex_staging.buffer,
                vine_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy::default()
                    .buffer_offset(vine_at)
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D {
                        width: vine_w,
                        height: vine_h,
                        depth: 1,
                    })],
            );
            barrier(
                d,
                cb,
                vine_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        })?;
        tex_staging.destroy(gpu);
        let tex_view = gpu.view(tex_image, vk::Format::R8G8B8A8_UNORM)?;
        let tex_mask = textures.mask();
        let vine_view = gpu.view(vine_image, vk::Format::R8G8B8A8_UNORM)?;
        let vine_on = textures.vine_on;
        let voxel_view = view_3d(gpu, voxel_image, vk::Format::R8G8B8A8_UINT)?;
        let roof_view = view_3d(gpu, roof_image, vk::Format::R8_UINT)?;
        let style_view = gpu.view(style_image, vk::Format::R32G32B32A32_SFLOAT)?;
        let glyph_view = gpu.view(glyph_image, vk::Format::R8_UINT)?;
        let sky_view = view_3d(gpu, sky_image, vk::Format::R8_UINT)?;
        let canopy_view = gpu.view(canopy_image, vk::Format::R8G8B8A8_UINT)?;

        let n = params.voxel_count();
        let sizes = [
            (n * std::mem::size_of::<VoxelTexel>()) as u64,
            n as u64,
            (MAX_STYLES * std::mem::size_of::<VoxelStyle>()) as u64,
            params.glyph_bytes() as u64,
            params.sky_bytes() as u64,
            params.canopy_bytes() as u64,
        ];
        // Each plane starts on a 16-byte boundary: `vkCmdCopyBufferToImage` wants the
        // offset to be a multiple of the texel size, and the style texels are 16 bytes.
        let mut offsets = [0u64; 7];
        for (i, size) in sizes.iter().enumerate() {
            offsets[i + 1] = offsets[i] + align16(*size);
        }
        let staging = (0..STAGING_RING)
            .map(|_| gpu.host_buffer(offsets[AT_END], vk::BufferUsageFlags::TRANSFER_SRC))
            .collect::<Result<Vec<_>>>()?;
        // One block per ring slot. The stride is `minUniformBufferOffsetAlignment`,
        // which is 256 bytes on some devices and 64 on others, so it is measured rather
        // than assumed; the frame binds its own slot's offset.
        let uniform_stride = align_to(
            std::mem::size_of::<VoxelUniforms>() as u64,
            unsafe { gpu.instance.get_physical_device_properties(gpu.pdev) }
                .limits
                .min_uniform_buffer_offset_alignment
                .max(1),
        );
        let uniforms = gpu.host_buffer(
            uniform_stride * STAGING_RING as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
        )?;
        for slot in 0..STAGING_RING as u64 {
            uniforms.write_bytes_at(uniform_stride * slot, &[params.uniforms(tex_mask, vine_on)]);
        }

        let nearest = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    // The strip wraps in x and has real faces in y and z; every fetch is
                    // a `texelFetch` with the wrap done by hand, so these say what the
                    // topology is rather than doing anything.
                    .address_mode_u(vk::SamplerAddressMode::REPEAT)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )
        }?;

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                // Dynamic: one descriptor over the whole ring, the frame's slot chosen at
                // bind time. A per-slot descriptor set would have to be rewritten while a
                // frame was reading it.
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            sampled(1),
            sampled(2),
            sampled(3),
            sampled(4),
            sampled(5),
            sampled(6),
            sampled(7),
            sampled(8),
        ];
        let set_layout = unsafe {
            d.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }?;
        let sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                .descriptor_count(1),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(8),
        ];
        let pool = unsafe {
            d.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&sizes),
                None,
            )
        }?;
        let layouts = [set_layout];
        let set = unsafe {
            d.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            )
        }?[0];
        let buffer_info = [vk::DescriptorBufferInfo::default()
            .buffer(uniforms.buffer)
            .range(std::mem::size_of::<VoxelUniforms>() as u64)];
        let image_info = |view: vk::ImageView| {
            [vk::DescriptorImageInfo::default()
                .sampler(nearest)
                .image_view(view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]
        };
        let (iv, ir, is, ig, it, ivn) = (
            image_info(voxel_view),
            image_info(roof_view),
            image_info(style_view),
            image_info(glyph_view),
            image_info(tex_view),
            image_info(vine_view),
        );
        let (isk, ica) = (image_info(sky_view), image_info(canopy_view));
        unsafe {
            d.update_descriptor_sets(
                &[
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
                        .buffer_info(&buffer_info),
                    sampled_write(set, 1, &iv),
                    sampled_write(set, 2, &ir),
                    sampled_write(set, 3, &is),
                    sampled_write(set, 4, &ig),
                    sampled_write(set, 5, &it),
                    sampled_write(set, 6, &ivn),
                    sampled_write(set, 7, &isk),
                    sampled_write(set, 8, &ica),
                ],
                &[],
            )
        };

        let pipeline_layout = unsafe {
            d.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts),
                None,
            )
        }?;
        let vs = gpu.shader(FULLSCREEN_VERT)?;
        let fs = gpu
            .shader(VOXEL_FRAG)
            .context("the voxel fragment shader")?;
        // `voxel.frag`'s `LIT` (constant_id 0, a 32-bit bool): the flat tier's pipeline is
        // specialised with it off, so the lit branches are dead code there and the panel's
        // GPU runs the flat shader code alone.
        let lit = u32::from(params.lit);
        let entries = [vk::SpecializationMapEntry::default()
            .constant_id(0)
            .offset(0)
            .size(4)];
        let spec = vk::SpecializationInfo::default()
            .map_entries(&entries)
            .data(bytemuck::bytes_of(&lit));
        let pipeline = crate::render::fullscreen_pipeline_specialised(
            d,
            raster_pass,
            pipeline_layout,
            vs,
            fs,
            false,
            Some(&spec),
        )?;
        unsafe {
            d.destroy_shader_module(vs, None);
            d.destroy_shader_module(fs, None);
        }

        Ok(VoxelRenderer {
            params,
            raster_image,
            raster_memory,
            raster_view,
            raster_framebuffer,
            raster_pass,
            voxel_image,
            voxel_memory,
            voxel_view,
            roof_image,
            roof_memory,
            roof_view,
            style_image,
            style_memory,
            style_view,
            glyph_image,
            glyph_memory,
            glyph_view,
            tex_image,
            tex_memory,
            tex_view,
            tex_mask,
            vine_image,
            vine_memory,
            vine_view,
            vine_on,
            sky_image,
            sky_memory,
            sky_view,
            canopy_image,
            canopy_memory,
            canopy_view,
            staging,
            uniform_stride,
            last_done: None,
            ring: StagingRing::new(STAGING_RING),
            slow: SlowPlanes::new(STAGING_RING),
            offsets,
            dirty: false,
            raster_current: false,
            redrew: true,
            version: 0,
            staged: false,
            uniforms,
            nearest,
            set_layout,
            set,
            pool,
            pipeline_layout,
            pipeline,
            present: PresentPass::new(gpu, raster_view, nearest)?,
            command_pool,
            queries,
        })
    }

    pub fn params(&self) -> VoxelParams {
        self.params
    }

    /// Change the shading knobs an operator can move — the haze, the water opacity and
    /// which way the roof gap is read — without rebuilding anything. The projection and
    /// the world's extent are fixed at construction and are refused here.
    pub fn set_params(&mut self, params: VoxelParams) -> Result<()> {
        let fixed = |p: &VoxelParams| {
            (
                p.s, p.rise, p.base, p.width, p.height, p.depth, p.raster_w, p.raster_h, p.lit,
            )
        };
        if fixed(&params) != fixed(&self.params) {
            bail!(
                "the projection, the world's extent and the lighting tier are fixed for a \
                 VoxelRenderer"
            );
        }
        self.params = params;
        self.raster_current = false;
        self.version += 1;
        Ok(())
    }

    /// Update dynamic atmosphere moisture and rain animation tick uniforms.
    /// **Nothing is written to the GPU here.** The uniform block a frame reads is written
    /// when that frame is recorded, into that frame's own slot; writing it now would write
    /// under whatever frame the GPU is reading.
    pub fn update_weather(&mut self, atmosphere: f32, rain_tick: f32) {
        self.params.atmosphere = atmosphere;
        self.params.rain_tick = rain_tick;
        self.raster_current = false;
        self.version += 1;
    }

    /// Whether a pack has a staging buffer to go into. False while every one of them is
    /// being read by a frame that has not retired — the caller keeps the world owed and
    /// packs it next tick rather than doing the work and throwing it away.
    pub fn can_stage(&self) -> bool {
        self.ring.for_pack().is_some()
    }

    /// Write one tick's world straight into the staging buffer.
    ///
    /// The styles slice is zeroed first, so a frame with fewer stands than the last one
    /// cannot paint with a stale colour. The voxels are not: every texel is written every
    /// tick, and clearing them to then overwrite them is memory traffic nobody reads.
    ///
    /// `slow` names the roof table's and the glyph atlas's contents (equal key, equal
    /// bytes). `fill` is told through [`VoxelStaging::write`] which of them this buffer
    /// does not hold yet; the rest it must leave alone, and a frame uploads only the ones
    /// the images do not hold. A tick that did not move the terrain writes and uploads no
    /// roof.
    ///
    /// Returns `false` when every staging buffer is still being read by a frame that has
    /// not retired: the pack is **skipped**, not queued, and the next tick packs again.
    /// A tick-stale picture is the price; writing under the GPU is not an option.
    pub fn stage(&mut self, slow: [u64; SLOW_PLANES], fill: impl FnOnce(VoxelStaging<'_>)) -> bool {
        let Some(into) = self.ring.for_pack() else {
            return false;
        };
        let write = self.slow.pack(into, slow);
        let buffer = &self.staging[into];
        let n = self.params.voxel_count();
        let at = |plane: usize| self.offsets[plane] as usize;
        // Disjoint by construction: each plane is its own aligned range of the buffer.
        let (voxels, roof, styles, glyphs, sky, canopy) = unsafe {
            (
                std::slice::from_raw_parts_mut(
                    buffer.ptr.add(at(AT_VOXELS)) as *mut VoxelTexel,
                    n,
                ),
                std::slice::from_raw_parts_mut(buffer.ptr.add(at(AT_ROOF)), n),
                std::slice::from_raw_parts_mut(
                    buffer.ptr.add(at(AT_STYLES)) as *mut VoxelStyle,
                    MAX_STYLES,
                ),
                std::slice::from_raw_parts_mut(
                    buffer.ptr.add(at(AT_GLYPHS)),
                    self.params.glyph_bytes(),
                ),
                std::slice::from_raw_parts_mut(buffer.ptr.add(at(AT_SKY)), self.params.sky_bytes()),
                std::slice::from_raw_parts_mut(
                    buffer.ptr.add(at(AT_CANOPY)),
                    self.params.canopy_bytes(),
                ),
            )
        };
        styles.fill(VoxelStyle::default());
        fill(VoxelStaging {
            voxels,
            roof,
            styles,
            glyphs,
            sky,
            canopy,
            write,
        });
        self.ring.packed(into);
        self.dirty = true;
        self.staged = true;
        self.raster_current = false;
        self.version += 1;
        true
    }

    /// The oldest recorded frame has completed: its staging buffer may be packed again,
    /// and its timestamps are now the newest complete set.
    pub fn retire_frame(&mut self) {
        if let Some(frame) = self.ring.retire() {
            self.last_done = Some(frame.slot);
        }
    }

    /// The newest recorded frame was displaced before it was ever submitted. Its buffer
    /// comes back and its upload is owed again — without this the world it carried would
    /// never reach the GPU, because recording cleared the dirty flag.
    pub fn discard_frame(&mut self) {
        if let Some(frame) = self.ring.discard()
            && frame.staging.is_some()
        {
            self.dirty = true;
            // Its slow planes never reached the images either.
            self.slow.forget_images();
            // Its upload never ran, so the raster is not what that frame would have made
            // it: the next frame must draw the world again.
            self.raster_current = false;
        }
    }

    /// Whether the last recorded frame redrew the world raster.
    pub fn redrew_last(&self) -> bool {
        self.redrew
    }

    /// What a frame recorded now would show. See [`VoxelRenderer::version`].
    pub fn content_version(&self) -> u64 {
        self.version
    }

    /// Frames recorded and not yet retired.
    pub fn frames_in_flight(&self) -> usize {
        self.ring.in_flight()
    }

    /// The world raster, for a readback or a blit.
    pub fn raster_image(&self) -> vk::Image {
        self.raster_image
    }

    /// Record the frame: the three uploads if the world moved, the slab-walk pass into
    /// the world raster, then — if a target is given — the present pass onto it.
    pub fn record(
        &mut self,
        gpu: &Gpu,
        cb: vk::CommandBuffer,
        target: Option<TargetSlot<'_>>,
    ) -> Result<()> {
        if !self.staged {
            bail!("VoxelRenderer::record before the first stage: there is no world to draw");
        }
        let d = &gpu.device;
        let target = target
            .map(|(image, extent, format, layout, xform)| {
                self.present
                    .entry(gpu, format, layout)
                    .map(|(pass, pipeline)| (image, extent, xform, pass, pipeline))
            })
            .transpose()?;
        let (w, h, dd) = (self.params.width, self.params.height, self.params.depth);
        // Whatever was packed is this frame's to upload, and its buffer is this frame's
        // until it retires. A frame that uploads nothing still takes a place in the
        // ring's order, so retirements and frames stay one to one.
        let frame = self.ring.record();
        let upload = self.dirty && frame.staging.is_some();
        self.dirty = false;
        // Which of the roof and the atlas this upload carries: only the ones the images
        // do not already hold.
        let slow = match frame.staging {
            Some(i) if upload => self.slow.upload(i),
            _ => [false; SLOW_PLANES],
        };
        // The raster already holds this picture unless something changed it, and the
        // upload only ever comes with a change.
        let redraw = !self.raster_current;
        self.raster_current = true;
        self.redrew = redraw;
        // This frame's own uniform block, written now and read by the GPU when it runs:
        // `set_params` and `update_weather` only moved `self.params`.
        self.uniforms.write_bytes_at(
            self.uniform_stride * frame.slot as u64,
            &[self.params.uniforms(self.tex_mask, self.vine_on)],
        );
        // and its own four timestamps.
        let q = frame.slot as u32 * QUERY_SLOTS;

        unsafe {
            d.begin_command_buffer(
                cb,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            d.cmd_reset_query_pool(cb, self.queries, q, QUERY_SLOTS);
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::TOP_OF_PIPE, self.queries, q);

            if upload && redraw {
                let lit = self.params.lit;
                let (cw, ch) = self.params.canopy_extent();
                let planes = [
                    (true, self.voxel_image, self.offsets[AT_VOXELS], w, h, dd),
                    (slow[PLANE_ROOF], self.roof_image, self.offsets[AT_ROOF], w, h, dd),
                    (
                        true,
                        self.style_image,
                        self.offsets[AT_STYLES],
                        3,
                        MAX_STYLES as u32,
                        1,
                    ),
                    (
                        slow[PLANE_GLYPHS],
                        self.glyph_image,
                        self.offsets[AT_GLYPHS],
                        self.params.s,
                        (self.params.s + self.params.rise) * MAX_GLYPHS as u32,
                        1,
                    ),
                    (
                        lit && slow[PLANE_SKY],
                        self.sky_image,
                        self.offsets[AT_SKY],
                        w,
                        h,
                        dd,
                    ),
                    (lit, self.canopy_image, self.offsets[AT_CANOPY], cw, ch, 1),
                ];
                let source =
                    self.staging[frame.staging.expect("an upload has a staged buffer")].buffer;
                // A plane left out keeps its image, in `SHADER_READ_ONLY_OPTIMAL`, holding
                // what an earlier frame uploaded — no barrier, since the `UNDEFINED`
                // transition is what would throw its contents away.
                for (_, image, offset, pw, ph, pd) in planes.into_iter().filter(|p| p.0) {
                    barrier(
                        d,
                        cb,
                        image,
                        vk::ImageLayout::UNDEFINED,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    );
                    d.cmd_copy_buffer_to_image(
                        cb,
                        source,
                        image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[vk::BufferImageCopy::default()
                            .buffer_offset(offset)
                            .image_subresource(
                                vk::ImageSubresourceLayers::default()
                                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                                    .layer_count(1),
                            )
                            .image_extent(vk::Extent3D {
                                width: pw,
                                height: ph,
                                depth: pd,
                            })],
                    );
                    barrier(
                        d,
                        cb,
                        image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    );
                }
            }
            d.cmd_write_timestamp(
                cb,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                self.queries,
                q + 1,
            );

            if redraw {
                // The pass leaves the raster in `SHADER_READ_ONLY_OPTIMAL`, which is
                // where the present pass wants it — so a frame that skips this one finds
                // the image in the right layout, holding the right picture, and only one
                // frame is ever in flight, so nothing is still writing it.
                crate::render::begin(
                    d,
                    cb,
                    self.raster_pass,
                    self.raster_framebuffer,
                    self.params.raster_w,
                    self.params.raster_h,
                );
                d.cmd_bind_descriptor_sets(
                    cb,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout,
                    0,
                    &[self.set],
                    &[(self.uniform_stride * frame.slot as u64) as u32],
                );
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
                d.cmd_draw(cb, 3, 1, 0, 0);
                d.cmd_end_render_pass(cb);
            }
            d.cmd_write_timestamp(
                cb,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                self.queries,
                q + 2,
            );

            if let Some((image, extent, xform, pass, pipeline)) = target {
                self.present
                    .record(d, cb, image, extent, xform, pass, pipeline);
            }
            d.cmd_write_timestamp(
                cb,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                self.queries,
                q + 3,
            );
            d.end_command_buffer(cb)?;
        }
        Ok(())
    }

    /// The frame's three GPU stages in milliseconds — uploads, the slab-walk pass, the
    /// present pass — or `None` if the queries are not ready.
    /// The timestamps read are the **last retired** frame's. With a frame in flight and
    /// another being recorded, the newest set in the pool is not finished, and reading it
    /// would subtract a stamp that had not been written.
    pub fn gpu_split(&self, gpu: &Gpu) -> Option<[f64; 3]> {
        let base = self.last_done? as u32 * QUERY_SLOTS;
        let mut ts = [0u64; QUERY_SLOTS as usize];
        if unsafe {
            gpu.device.get_query_pool_results(
                self.queries,
                base,
                &mut ts,
                vk::QueryResultFlags::TYPE_64,
            )
        }
        .is_err()
        {
            return None;
        }
        let ns = f64::from(gpu.timestamp_period) / 1.0e6;
        Some([
            ts[1].wrapping_sub(ts[0]) as f64 * ns,
            ts[2].wrapping_sub(ts[1]) as f64 * ns,
            ts[3].wrapping_sub(ts[2]) as f64 * ns,
        ])
    }

    /// Copy the world raster into a host buffer, `w · 4` bytes per row.
    pub fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>> {
        let (w, h) = (self.params.raster_w, self.params.raster_h);
        let size = u64::from(w) * u64::from(h) * 4;
        let host = gpu.readback_buffer(size)?;
        let d = &gpu.device;
        gpu.one_shot(self.command_pool, |cb| unsafe {
            barrier(
                d,
                cb,
                self.raster_image,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            );
            d.cmd_copy_image_to_buffer(
                cb,
                self.raster_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                host.buffer,
                &[vk::BufferImageCopy::default()
                    .buffer_row_length(w)
                    .buffer_image_height(h)
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D {
                        width: w,
                        height: h,
                        depth: 1,
                    })],
            );
            barrier(
                d,
                cb,
                self.raster_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        })?;
        let bytes = unsafe { host.bytes() }[..size as usize].to_vec();
        host.destroy(gpu);
        Ok(bytes)
    }

    /// Release everything. The device must be idle.
    pub fn destroy(&mut self, gpu: &Gpu) {
        self.present.destroy(gpu);
        let d = &gpu.device;
        unsafe {
            d.destroy_pipeline(self.pipeline, None);
            d.destroy_pipeline_layout(self.pipeline_layout, None);
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_sampler(self.nearest, None);
            for buffer in &self.staging {
                buffer.destroy(gpu);
            }
            self.uniforms.destroy(gpu);
            for (view, image, memory) in [
                (self.voxel_view, self.voxel_image, self.voxel_memory),
                (self.roof_view, self.roof_image, self.roof_memory),
                (self.style_view, self.style_image, self.style_memory),
                (self.glyph_view, self.glyph_image, self.glyph_memory),
                (self.tex_view, self.tex_image, self.tex_memory),
                (self.vine_view, self.vine_image, self.vine_memory),
                (self.sky_view, self.sky_image, self.sky_memory),
                (self.canopy_view, self.canopy_image, self.canopy_memory),
            ] {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(memory, None);
            }
            d.destroy_framebuffer(self.raster_framebuffer, None);
            d.destroy_image_view(self.raster_view, None);
            d.destroy_image(self.raster_image, None);
            d.free_memory(self.raster_memory, None);
            d.destroy_render_pass(self.raster_pass, None);
            d.destroy_query_pool(self.queries, None);
            d.destroy_command_pool(self.command_pool, None);
        }
    }
}

/// The voxel renderer as a target's frame source. One frame carries nothing: the world
/// arrived with the tick's [`VoxelRenderer::stage`], and a frame is one draw over it.
impl FrameSource for VoxelRenderer {
    type Frame<'a> = ();

    fn raster_size(&self) -> (u32, u32) {
        (self.params.raster_w, self.params.raster_h)
    }

    fn command_pool(&self) -> vk::CommandPool {
        self.command_pool
    }

    fn present_pass(
        &mut self,
        gpu: &Gpu,
        format: vk::Format,
        final_layout: vk::ImageLayout,
    ) -> Result<vk::RenderPass> {
        self.present.pass(gpu, format, final_layout)
    }

    fn record_frame(
        &mut self,
        gpu: &Gpu,
        cb: vk::CommandBuffer,
        _frame: (),
        target: Option<TargetSlot<'_>>,
    ) -> Result<()> {
        VoxelRenderer::record(self, gpu, cb, target)
    }

    fn frame_retired(&mut self) {
        VoxelRenderer::retire_frame(self);
    }

    fn frame_discarded(&mut self) {
        VoxelRenderer::discard_frame(self);
    }

    fn frames_in_flight(&self) -> usize {
        VoxelRenderer::frames_in_flight(self)
    }

    fn frame_capacity(&self) -> usize {
        STAGING_RING
    }

    fn redrew_last(&self) -> bool {
        VoxelRenderer::redrew_last(self)
    }

    fn content_version(&self) -> Option<u64> {
        Some(VoxelRenderer::content_version(self))
    }

    fn gpu_ms(&self, gpu: &Gpu) -> f64 {
        self.gpu_split(gpu).map_or(f64::NAN, |s| s[0] + s[1] + s[2])
    }

    fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>> {
        VoxelRenderer::read_raster(self, gpu)
    }
}

// --- the shader's arithmetic, in Rust -------------------------------------------------

/// Which voxel level and which row inside it a raster pixel falls on, in one slab.
///
/// This is the shader's own first three lines, and the reason it is here in Rust is that
/// it is the whole of the projection inversion: everything else in `voxel.frag` is a
/// colour rule over neighbours. Writing `A = base − z·rise − sy` and `q = A − 1`, the
/// pixel sits in the band of voxel `level = ⌊q / s⌋` at `r = q − level·s`, where `r`
/// counts rows **up** from the bottom of that band. Two faces can own it:
///
/// - the **front** face of `(x, level, z)`, at row `s − 1 − r` of that face;
/// - when `r < rise`, the **top** face of `(x, level − 1, z)`, at cap row `rise − 1 − r`
///   — the cap of the voxel below occupies the bottom `rise` rows of the band above it,
///   which is exactly why the CPU presenter draws a cap only where the voxel above is
///   air.
///
/// `None` means the pixel is below this slab's `y = 0` front face, and since `q` falls by
/// `rise` per slab, every deeper slab is below it too: the walk stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlabHit {
    /// The voxel whose front face owns the pixel; may be `>= height`, meaning the pixel
    /// is above this slab's column and only a cap can own it.
    pub level: i32,
    /// Rows up from the bottom of that band, `0 .. s`.
    pub r: u32,
}

impl SlabHit {
    /// Row of the front face of [`SlabHit::level`] this pixel is, `0` at the top.
    pub fn front_row(self, s: u32) -> u32 {
        s - 1 - self.r
    }

    /// Row of the top face of `level − 1` this pixel is, `0` at the top, or `None` when
    /// no cap reaches it.
    pub fn cap_row(self, rise: u32) -> Option<u32> {
        (self.r < rise).then(|| rise - 1 - self.r)
    }
}

/// The slab lookup: which voxel level and row owns raster pixel `sy` in slab `z`.
pub fn slab_hit(params: &VoxelParams, sy: i32, z: u32) -> Option<SlabHit> {
    let q = params.base - (z * params.rise) as i32 - sy - 1;
    if q < 0 {
        return None;
    }
    let s = params.s as i32;
    Some(SlabHit {
        level: q / s,
        r: (q % s) as u32,
    })
}

// --- helpers --------------------------------------------------------------------------

fn align16(n: u64) -> u64 {
    (n + 15) & !15
}

/// `n` rounded up to a multiple of `to` (a power of two, as every Vulkan alignment is).
fn align_to(n: u64, to: u64) -> u64 {
    (n + to - 1) & !(to - 1)
}

fn image_3d(
    gpu: &Gpu,
    width: u32,
    height: u32,
    depth: u32,
    format: vk::Format,
) -> Result<(vk::Image, vk::DeviceMemory)> {
    let d = &gpu.device;
    let image = unsafe {
        d.create_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_3D)
                .format(format)
                .extent(vk::Extent3D {
                    width,
                    height,
                    depth,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED),
            None,
        )
    }?;
    let req = unsafe { d.get_image_memory_requirements(image) };
    let memory = unsafe {
        d.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(req.size)
                .memory_type_index(
                    gpu.memory_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)?,
                ),
            None,
        )
    }?;
    unsafe { d.bind_image_memory(image, memory, 0) }?;
    Ok((image, memory))
}

fn view_3d(gpu: &Gpu, image: vk::Image, format: vk::Format) -> Result<vk::ImageView> {
    Ok(unsafe {
        gpu.device.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_3D)
                .format(format)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1),
                ),
            None,
        )
    }?)
}

fn sampled(binding: u32) -> vk::DescriptorSetLayoutBinding<'static> {
    vk::DescriptorSetLayoutBinding::default()
        .binding(binding)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
}

fn sampled_write<'a>(
    set: vk::DescriptorSet,
    binding: u32,
    info: &'a [vk::DescriptorImageInfo; 1],
) -> vk::WriteDescriptorSet<'a> {
    vk::WriteDescriptorSet::default()
        .dst_set(set)
        .dst_binding(binding)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> VoxelParams {
        VoxelParams {
            s: 4,
            rise: 2,
            base: 240,
            width: 128,
            height: 48,
            depth: 24,
            raster_w: 512,
            raster_h: 240,
            haze: 0.55,
            water_alpha: 0.5,
            roof_from_texture: true,
            sky: [0.0; 3],
            sky_horizon: [0.0; 3],
            atmosphere: 0.0,
            rain_tick: 0.0,
            dither: 0.0,
            sky_gradient: false,
            bedrock: [0.0; 3],
            rock: [0.0; 3],
            soil: [0.0; 3],
            water_deep: [0.0; 3],
            water_surface: [0.0; 3],
            light: [0.0; 3],
            haze_colour: [0.0; 3],
            top_gain: 2.4,
            top_tint: 0.2,
            top_back: 0.3,
            rim: 0.38,
            edge_dark: 0.72,
            top_edge: 0.18,
            riser_lean: 0.45,
            wet: 0.55,
            roof_light: 0.22,
            roof_falloff: 4.0,
            skin_alpha_gain: 1.7,
            water_top_alpha: 0.8,
            plant_top_gain: 1.5,
            plant_top_tint: 0.14,
            plant_rim: 0.42,
            crown_edge: 0.7,
            crown_under: 0.22,
            trunk_shade: [0.62, 1.22],
            trunk_light_at: 0.35,
            lit: false,
            ambient_gain: 1.4,
            ambient_floor: 0.2,
            light_levels: 4,
            ao_strength: 0.5,
            ambient_colour: [1.0; 3],
            sun: [0.0; 3],
            sun_tint: 0.0,
        }
    }

    /// Any water at all packs to a non-zero fraction, so the presenter's "at least one
    /// row of water is visible" survives the quantisation; a dry cell packs to zero.
    #[test]
    fn the_texel_packing_round_trips_a_voxel() {
        let t = VoxelTexel::pack(3, PART_TRUNK, 0.0, true, 0.75, 9);
        assert_eq!((t.material(), t.part(), t.style()), (3, PART_TRUNK, 9));
        assert!(t.dry() && t.free() == 0.0);
        assert!((t.pore() - 0.75).abs() < 1.0 / 255.0);

        let animal = VoxelTexel::pack_glyph(0, PART_ANIMAL_INTERIM, 5, 0.0, true, 0.0, 7);
        assert_eq!(
            (animal.part(), animal.glyph(), animal.style()),
            (PART_ANIMAL_INTERIM, 5, 7)
        );

        let film = VoxelTexel::pack(0, PART_NONE, 1e-3, false, 0.0, 0);
        assert!(!film.dry(), "a film of water must not pack to dry");
        assert!(film.free() > 0.0);

        for &free in &[0.125f32, 0.375, 0.5, 0.875, 1.0] {
            let t = VoxelTexel::pack(0, PART_NONE, free, false, 0.0, 0);
            // Half a step of the 8-bit channel: the quantisation's whole error.
            assert!(
                (t.free() - free).abs() <= 1.0 / 509.0,
                "{free} -> {}",
                t.free()
            );
        }
        // Every part class survives beside a full material and a top style index — the
        // fauna range included, and the two ids past it that nothing draws yet, because
        // the field has to hold what the art direction will put there.
        for part in [
            PART_NONE,
            PART_TRUNK,
            PART_CROWN,
            PART_CROWN_HEART,
            PART_SPROUT,
            PART_ANIMAL_INTERIM,
            6,
            7,
        ] {
            let t = VoxelTexel::pack(2, part, 1.0, false, 1.0, 255);
            assert_eq!((t.material(), t.part(), t.style()), (2, part, 255));
            assert_eq!((t.free(), t.pore()), (1.0, 1.0));
        }
    }

    /// The slab lookup against the projection it inverts: the bottom-left voxel's front
    /// face is the bottom `s` rows of column 0, its cap the `rise` rows above, and the
    /// cap of a voxel is the bottom `rise` rows of the band above it.
    #[test]
    fn the_slab_lookup_inverts_the_projection() {
        let p = params();
        // `front_row(0, 0) = 240 - 4 = 236`, so rows 236..240 are voxel 0's front face.
        for (sy, want_row) in [(236, 0), (237, 1), (238, 2), (239, 3)] {
            let hit = slab_hit(&p, sy, 0).expect("inside the slab");
            assert_eq!(hit.level, 0, "sy = {sy}");
            assert_eq!(hit.front_row(p.s), want_row);
        }
        // The two rows above are voxel 0's cap, which is the bottom of voxel 1's band.
        for (sy, want_cap) in [(234, 0), (235, 1)] {
            let hit = slab_hit(&p, sy, 0).unwrap();
            assert_eq!(hit.level, 1, "sy = {sy}");
            assert_eq!(hit.cap_row(p.rise), Some(want_cap));
        }
        // And the two rows above *those* are voxel 1's front face with no cap over them.
        for sy in [232, 233] {
            let hit = slab_hit(&p, sy, 0).unwrap();
            assert_eq!(hit.level, 1);
            assert_eq!(hit.cap_row(p.rise), None);
        }
        // One slab back lifts everything by `rise`.
        assert_eq!(
            slab_hit(&p, 236 - 2, 1).unwrap(),
            slab_hit(&p, 236, 0).unwrap()
        );
        // Below the floor line there is nothing, in this slab or any deeper one.
        assert_eq!(slab_hit(&p, 240, 0), None);
    }

    /// The hazard the ring exists for: `vkCmdCopyBufferToImage` reads the staging buffer
    /// when the GPU *runs* it, not when it was recorded. So the buffer a recorded frame
    /// named must never be the one the next tick is packed into — and when both are
    /// spoken for, the pack is refused rather than written under the GPU.
    #[test]
    fn a_buffer_a_recorded_frame_reads_is_never_handed_to_the_next_pack() {
        let mut ring = StagingRing::new(STAGING_RING);
        let a = ring.for_pack().expect("nothing is in flight");
        ring.packed(a);
        let first = ring.record();
        assert_eq!(first.staging, Some(a));
        assert_eq!(ring.reading(), vec![a], "the GPU has that one");

        let b = ring.for_pack().expect("the other buffer is free");
        assert_ne!(b, a, "packing there would write under the frame in flight");
        ring.packed(b);
        let second = ring.record();
        assert_ne!(
            second.slot, first.slot,
            "and its uniforms and timestamps are its own too"
        );

        assert_eq!(ring.for_pack(), None, "both are spoken for: no pack");
        assert_eq!(ring.retire(), Some(first), "frames retire in order");
        assert_eq!(ring.for_pack(), Some(a), "and its buffer comes back");
    }

    /// A pack nothing has read yet is simply overwritten: the newer world is the one that
    /// should go up, and no buffer is spent holding a world that has been superseded.
    #[test]
    fn a_pack_no_frame_has_taken_is_overwritten_rather_than_queued() {
        let mut ring = StagingRing::new(STAGING_RING);
        let first = ring.for_pack().expect("free");
        ring.packed(first);
        assert_eq!(ring.for_pack(), Some(first), "the same buffer again");
        ring.packed(first);
        assert_eq!(ring.record().staging, Some(first));
        assert_eq!(ring.in_flight(), 1);
    }

    /// A frame the presenter never took is discarded, and the upload it carried is owed
    /// again — without that the world it held would never reach the GPU, because
    /// recording had already cleared the dirty flag.
    #[test]
    fn a_displaced_frame_gives_its_buffer_back_and_owes_its_upload_again() {
        let mut ring = StagingRing::new(STAGING_RING);
        let i = ring.for_pack().expect("free");
        ring.packed(i);
        let frame = ring.record();
        assert_eq!(
            ring.discard(),
            Some(frame),
            "the newest, and only the newest"
        );
        assert_eq!(ring.in_flight(), 0);
        assert_eq!(
            ring.for_pack(),
            Some(i),
            "the pack it carried is owed again"
        );
        assert_eq!(ring.record().staging, Some(i), "and goes up next frame");
    }

    /// A frame that re-presents a world the GPU already holds uploads nothing, but it is
    /// still a frame: it takes a slot of its own and one retirement, or the retirements
    /// and the frames would drift apart and a buffer would be freed too early.
    #[test]
    fn a_frame_that_uploads_nothing_still_takes_its_place_in_the_order() {
        let mut ring = StagingRing::new(STAGING_RING);
        let bare = ring.record();
        assert_eq!(bare.staging, None);
        assert_eq!(ring.in_flight(), 1);

        let i = ring.for_pack().expect("every buffer is free");
        ring.packed(i);
        let loaded = ring.record();
        assert_eq!(loaded.staging, Some(i));
        assert_ne!(loaded.slot, bare.slot, "two live frames, two slots");
        assert_eq!(ring.retire(), Some(bare), "the bare one finished first");
        assert_eq!(
            ring.reading(),
            vec![i],
            "and the loaded one is still reading"
        );
    }

    /// The roof, the atlas and the sky are written into each buffer once per change and
    /// uploaded once per change, with the two buffers alternating under them.
    #[test]
    fn a_slow_plane_is_written_and_uploaded_only_when_its_key_moves() {
        let mut slow = SlowPlanes::new(2);
        let all = [true; SLOW_PLANES];
        let none = [false; SLOW_PLANES];
        assert_eq!(slow.pack(0, [7, 1, 7]), all, "a new buffer holds nothing");
        assert_eq!(slow.upload(0), all, "and the images nothing");
        assert_eq!(slow.pack(1, [7, 1, 7]), all, "the other buffer is new too");
        assert_eq!(slow.upload(1), none, "but the images already hold it");
        assert_eq!(slow.pack(0, [7, 1, 7]), none, "nothing moved");
        assert_eq!(slow.upload(0), none);

        // The terrain moves: the roof goes into each buffer once, and up once.
        assert_eq!(slow.pack(1, [8, 1, 7]), [true, false, false]);
        assert_eq!(slow.upload(1), [true, false, false]);
        assert_eq!(
            slow.pack(0, [8, 1, 7]),
            [true, false, false],
            "buffer 0 still held 7"
        );
        assert_eq!(slow.upload(0), none, "the images hold 8 already");
        assert_eq!(slow.pack(1, [8, 1, 7]), none);

        // The sky for it arrives later, from its own thread: it alone moves.
        assert_eq!(slow.pack(0, [8, 1, 8]), [false, false, true]);
        assert_eq!(slow.upload(0), [false, false, true]);
    }

    /// A displaced frame's upload never ran: whatever it carried is owed again, so the
    /// images are not trusted to hold it.
    #[test]
    fn a_displaced_upload_leaves_the_slow_planes_owed() {
        let mut slow = SlowPlanes::new(2);
        slow.pack(0, [3, 1, 3]);
        slow.upload(0);
        slow.pack(1, [4, 1, 4]);
        slow.upload(1);
        slow.forget_images();
        assert_eq!(
            slow.pack(1, [4, 1, 4]),
            [false; SLOW_PLANES],
            "the buffer still holds it"
        );
        assert_eq!(
            slow.upload(1),
            [true; SLOW_PLANES],
            "the images might not"
        );
    }

    #[test]
    fn a_params_whose_raster_does_not_match_the_world_is_refused() {
        let mut p = params();
        p.raster_w = 500;
        assert!(p.validate().is_err());
        let mut p = params();
        p.rise = 5;
        assert!(p.validate().is_err());
        assert_eq!(params().voxel_count(), 128 * 48 * 24);
        assert_eq!(
            params().upload_bytes(),
            128 * 48 * 24 * 5 + 256 * 48 + 4 * (4 + 2) * MAX_GLYPHS
        );
    }

    /// A face lands in its slot's row and its variant's column, a top face in the first
    /// `rise` rows of its cell; a slot is sampled only once every variant is in.
    #[test]
    fn the_texture_atlas_puts_a_face_in_its_cell_and_switches_a_slot_on_when_whole() {
        let (s, rise) = (6, 3);
        let mut t = VoxelTextures::empty(s, rise);
        let (aw, ah) = VoxelTextures::size(s);
        assert_eq!((aw, ah), (4 * 6, TEXTURE_SLOTS.len() as u32 * 6));
        let top = TEXTURE_SLOTS.iter().position(|f| f.0 == "rock-top").unwrap();
        assert_eq!(t.face_size(top), (6, 3));
        assert!(t.put(top, 0, &[0; 6 * 6 * 4]).is_err(), "a side-sized top is refused");
        for v in 0..TEX_VARIANTS {
            assert_eq!(t.mask(), 0, "{v} of 4 variants is not a slot");
            t.put(top, v, &[v as u8 + 1; 6 * 3 * 4]).unwrap();
        }
        assert_eq!(t.mask(), 1 << top);
        let at = |x: u32, y: u32| t.rgba[((y * aw + x) * 4) as usize];
        let row = top as u32 * 6;
        assert_eq!(at(2 * 6, row), 3, "variant 2's first texel");
        assert_eq!(at(2 * 6 + 5, row + 2), 3, "and its last");
        assert_eq!(at(2 * 6, row + 3), 0, "a top face leaves its cell's lower rows");
    }

    #[test]
    fn a_style_carries_its_texture_role_in_the_wood_alpha() {
        let st = VoxelStyle::new([0.1; 3], [0.2; 3], [0.3; 3]);
        assert_eq!(st.role(), ROLE_NONE);
        assert_eq!(st.with_role(ROLE_LEAF).role(), ROLE_LEAF);
        assert_eq!(st.with_role(ROLE_LEAF).wood[..3], [0.1; 3]);
        assert_eq!(st.faces(), [None, None], "no species faces by default");
        let f = st.with_faces([Some(0), Some(14)]);
        assert_eq!(f.faces(), [Some(0), Some(14)]);
        assert_eq!((&f.crown[..3], &f.heart[..3]), (&[0.2f32; 3][..], &[0.3f32; 3][..]));
    }

    /// A named slot goes after the fixed ones, grows the atlas by one row of cells, is
    /// found by name, and stays out of the fixed mask.
    #[test]
    fn a_named_slot_grows_the_atlas_after_the_fixed_slots() {
        let (s, rise) = (6, 3);
        let mut t = VoxelTextures::empty(s, rise);
        let slot = t.add_slot("species/siphonreed/leaf-top", TexFace::Top);
        assert_eq!(slot, TEXTURE_SLOTS.len());
        assert_eq!(t.atlas_size(), (4 * 6, (TEXTURE_SLOTS.len() as u32 + 1) * 6));
        assert_eq!(t.rgba.len(), (4 * 6 * (TEXTURE_SLOTS.len() + 1) * 6 * 4) as usize);
        assert_eq!(t.face_size(slot), (6, 3));
        for v in 0..TEX_VARIANTS {
            t.put(slot, v, &[9; 6 * 3 * 4]).unwrap();
        }
        assert_eq!(t.slot_named("species/siphonreed/leaf-top"), Some(slot));
        assert_eq!(t.slot_named("species/siphonreed/leaf-side"), None);
        assert_eq!(t.mask(), 0);
        let (aw, _) = t.atlas_size();
        assert_eq!(t.rgba[((slot as u32 * 6 * aw) * 4) as usize], 9);
    }
}
