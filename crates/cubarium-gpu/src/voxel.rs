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

use anyhow::{Result, bail};

/// `voxel.frag`'s SMOOTH_SUN: the sun's share of the light when it is smooth.
const SMOOTH_SUN: f32 = 1.0 / 3.0;
/// How many plant styles one frame may paint with.
///
/// A style is one stand's wood/crown/heart in linear light, already carrying its crown
/// fill and its wilt, so the count is the number of *distinct* colour triples a frame
/// needs — seed banks collapse to one per species, and identical stands collapse
/// together. The CPU presenter's own cap is 65 535 stands; a frame that really needs more
/// than this many distinct styles reuses style 0 for the rest and the sink says so.
pub const MAX_STYLES: usize = 256;

/// The most entries one frame's **emitter table** holds ([`VoxelRenderer::set_emit_cells`],
/// GLOW-EASE): an open-addressed hash of cells, at most half full, so up to half this many
/// emitting cells draw their own colour each frame. A cell past that draws its style's.
pub const EMIT_TABLE: usize = 1 << 15;

/// The longest probe the shader makes in the emitter table; a cell the host cannot place
/// within it is left out (it draws its style's emission).
pub const EMIT_PROBE: usize = 32;

/// One emitting cell's colour this frame (GLOW-EASE): what its emitting texels draw
/// instead of their style's emissive colour (or, for a latticevine tile, the tile's own),
/// in linear light, and its **on** share: the texel draws `rgb + (1 − on) × its lit
/// pigment`, so one fading out ends on its pigment. `cell` is its texture index,
/// `(z · height + y) · width + x`.
///
/// The same table carries a latticevine cell's **old tile** while it cross-fades into a new
/// one (GLOW-EASE round 2, `vine`): a cell may be in it for either or both; `emits` false
/// leaves its emission to its tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmitCell {
    pub cell: u32,
    pub rgb: [f32; 3],
    pub on: f32,
    /// Whether `rgb` and `on` apply (the cell is an emitter the host eases).
    pub emits: bool,
    /// A vine cell's old tile, `(b, a)` as its texel carried it, and the share of it still
    /// shown (`0..=1`): the shader draws both tiles, the old one this share of coverage and
    /// colour.
    pub vine: Option<((u8, u8), f32)>,
    /// CELL-SHRINK: a plant cell's grow-in size (`0..=1`) while it arrives or leaves through
    /// the plant pass, which weights the light it casts on the rest of the world: its AO
    /// crease and its sun shadow (`world.glsl`'s `plantSize`). `None` casts it whole.
    pub plant: Option<f32>,
    /// CELL-SHRINK: on a crown cell beside a cell of its crown that is arriving or leaving,
    /// the share (`0..=1`) of its outline edge on that side (left, right) drawn: one less
    /// the neighbour's size (`world.glsl`'s `crownEdge`). `None` draws the edge as the
    /// cell's glyph has it. Not carried beside a vine cell's old tile.
    pub edge: [Option<f32>; 2],
}

impl EmitCell {
    /// An emitter's colour and on share alone.
    pub fn emitter(cell: u32, rgb: [f32; 3], on: f32) -> EmitCell {
        EmitCell {
            cell,
            rgb,
            on,
            emits: true,
            vine: None,
            plant: None,
            edge: [None; 2],
        }
    }

    /// A cell with nothing to carry yet: no emission, no old tile, a whole plant cell.
    pub fn none(cell: u32) -> EmitCell {
        EmitCell {
            cell,
            rgb: [0.0; 3],
            on: 0.0,
            emits: false,
            vine: None,
            plant: None,
            edge: [None; 2],
        }
    }
}

/// A plant size's code in an emitter table entry's fourth word (bits 24..31): 0 for none (a
/// whole cell), else `1 + size × PLANT_STEPS`.
pub const PLANT_STEPS: f32 = 125.0;

/// A crown edge share's code in an emitter table entry's fourth word (bits 0..8 left, 8..16
/// right, where a vine cell keeps its old tile): 0 for none, else `1 + share × EDGE_STEPS`.
pub const EDGE_STEPS: f32 = 254.0;

/// An emitter table entry's fourth word: bit 31 set when the entry carries no emission;
/// a vine cell's old tile in bits 0..16 (`b`, then `a`) and its share, `1..=255` of 255, in
/// bits 16..24 (0: no old tile); a plant cell's size in bits 24..31 ([`PLANT_STEPS`]).
/// `world.glsl`'s `cellEmit`, `cellVineFrom` and `plantSize` read it.
pub fn emit_word(c: &EmitCell) -> u32 {
    let mut w = if c.emits { 0 } else { 1 << 31 };
    if let Some(((b, a), share)) = c.vine {
        let s = (share.clamp(0.0, 1.0) * 255.0).round() as u32;
        if s > 0 {
            w |= u32::from(b) | u32::from(a) << 8 | s << 16;
        }
    }
    if w & 0xff_0000 == 0 {
        for (k, e) in c.edge.iter().enumerate() {
            if let Some(e) = e {
                w |= (1 + (e.clamp(0.0, 1.0) * EDGE_STEPS).round() as u32) << (8 * k);
            }
        }
    }
    if let Some(size) = c.plant {
        w |= (1 + (size.clamp(0.0, 1.0) * PLANT_STEPS).round() as u32) << 24;
    }
    w
}

/// The share of a crown cell's edge (`side` 0 left, 1 right) [`emit_word`] carries, as the
/// shader reads it back (1: the glyph's own edge).
pub fn edge_share_of(word: u32, side: usize) -> f32 {
    if word & 0xff_0000 != 0 {
        return 1.0;
    }
    match (word >> (8 * side)) & 255 {
        0 => 1.0,
        e => (e - 1) as f32 / EDGE_STEPS,
    }
}

/// The size [`emit_word`] gives a plant cell, as the shader reads it back (1: whole).
pub fn plant_size_of(word: u32) -> f32 {
    match (word >> 24) & 127 {
        0 => 1.0,
        p => (p - 1) as f32 / PLANT_STEPS,
    }
}

/// The bit of `view.w` (the emitter table's size) saying some entry carries a vine cell's
/// old tile, so the shader looks vine cells up only then.
pub const EMIT_VINES: u32 = 1 << 20;

/// The bit of `view.w` saying some entry carries a plant cell's size (CELL-SHRINK), so the
/// shader looks plant cells up only then.
pub const EMIT_PLANTS: u32 = 1 << 21;

/// The table capacity and payload flags used by both the shader and the walk variant.
/// An empty table selects the whole-cell walk even if the supplied cells carry sizes.
pub(crate) fn emit_view(cap: u32, cells: &[EmitCell]) -> u32 {
    if cap == 0 {
        return 0;
    }
    let vines = cells.iter().any(|c| emit_word(c) & 0xff_0000 != 0);
    let plants = cells.iter().any(|c| c.plant.is_some());
    cap | if vines { EMIT_VINES } else { 0 } | if plants { EMIT_PLANTS } else { 0 }
}

/// The emitter table's hash of `key` (a texture index plus one) into `cap` entries, a power
/// of two: `world.glsl`'s `emitHash`.
#[inline]
pub fn emit_hash(key: u32, cap: u32) -> u32 {
    key.wrapping_mul(0x9E37_79B1) >> (32 - cap.trailing_zeros()).min(31) & (cap - 1)
}

/// Lay `cells` out as the shader reads them: `cap` entries (the smallest power of two at
/// least twice their number, at least 64, at most [`EMIT_TABLE`]), each `(key, rg, b·on, 0)`
/// with `key` the texture index plus one (0: empty) and the colour as half floats. A cell
/// that finds no free entry within [`EMIT_PROBE`] is left out. Returns `cap` (0: none).
pub fn emit_table(cells: &[EmitCell], out: &mut Vec<[u32; 4]>) -> u32 {
    out.clear();
    if cells.is_empty() {
        return 0;
    }
    let cap = (cells.len() * 2).next_power_of_two().clamp(64, EMIT_TABLE);
    out.resize(cap, [0; 4]);
    let half = |a: f32, b: f32| u32::from(half_bits(a)) | u32::from(half_bits(b)) << 16;
    for c in cells {
        let key = c.cell.wrapping_add(1);
        let mut h = emit_hash(key, cap as u32) as usize;
        for _ in 0..EMIT_PROBE {
            if out[h][0] == 0 || out[h][0] == key {
                out[h] = [
                    key,
                    half(c.rgb[0], c.rgb[1]),
                    half(c.rgb[2], c.on),
                    emit_word(c),
                ];
                break;
            }
            h = (h + 1) & (cap - 1);
        }
    }
    cap as u32
}

/// An f32 as IEEE half bits (round to nearest, clamped to the half range; NaN to 0), as
/// GLSL's `unpackHalf2x16` reads them.
fn half_bits(v: f32) -> u16 {
    let v = if v.is_nan() {
        0.0
    } else {
        v.clamp(-65504.0, 65504.0)
    };
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32 - 127 + 15;
    let man = b & 0x7f_ffff;
    if exp <= 0 {
        if exp < -10 {
            return sign;
        }
        let m = (man | 0x80_0000) >> (1 - exp);
        return sign | ((m + 0x1000) >> 13) as u16;
    }
    let r = ((exp as u32) << 10 | man >> 13) + ((man >> 12) & 1);
    sign | r.min(0x7bff) as u16
}
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

/// One breath of the pose table: its phase at the staged tick (0..1, so the frame clock's
/// wrap never shows), its period in ticks and its dip in voxels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Breath {
    pub base: f32,
    pub period: f32,
    pub amp: f32,
}

impl Breath {
    pub const ZERO: Breath = Breath {
        base: 0.0,
        period: 0.0,
        amp: 0.0,
    };

    /// The dip `ticks` after the staged tick at an entry's own phase offset: a cosine, full
    /// at a breath's bottom.
    fn dip(self, phase: f32, ticks: f32) -> f32 {
        let period = if self.period > 0.0 { self.period } else { 1.0 };
        let t = self.base + ticks / period + phase;
        self.amp * (0.5 + 0.5 * (std::f32::consts::TAU * t).cos())
    }
}

/// One head move of an idle body (`cubarium::voxel::idle`): a dip out and back once, eased
/// in and out, starting `start` ticks after the staged tick (negative when under way) and
/// lasting `ticks`, bringing the head cells `dip` voxels down at its furthest. A zero dip is
/// no move (a body entry).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadMove {
    pub start: f32,
    pub ticks: f32,
    pub dip: f32,
}

impl HeadMove {
    pub const NONE: HeadMove = HeadMove {
        start: 0.0,
        ticks: 0.0,
        dip: 0.0,
    };

    /// How far out the move is `ticks` after the staged tick, 0 to 1 and back to 0:
    /// `sin²(π u)` over the move's own time `u`, so it leaves and returns with no speed.
    pub fn out(self, ticks: f32) -> f32 {
        if self.ticks <= 0.0 {
            return 0.0;
        }
        let u = (ticks - self.start) / self.ticks;
        if u <= 0.0 || u >= 1.0 {
            return 0.0;
        }
        let s = (std::f32::consts::PI * u).sin();
        s * s
    }
}

/// One entry of the pose table (WX4): a posing body, or its head while it moves.
///
/// - `sleep` is the sleep weight at the staged tick, `sleep_rate` its change per tick,
///   `drop` the rows the body settles when fully asleep, `sleep_phase` its breath's offset.
/// - `idle` is the idle weight (an awake body standing still) at the staged tick,
///   `idle_rate` its change per tick, `idle_phase` its breath's offset.
/// - `head` is a head move under way or due: the head cells of a body in a move take an
///   entry of their own with it, the rest of the body one with [`HeadMove::NONE`].
/// - `hunker` is the hunker weight (a frondgrazer clamped under its shield, HUNKER) at the
///   staged tick and `hunker_rate` its change per tick: the body settles by `drop` at it,
///   as at the sleep weight (the larger of the two settles it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoseEntry {
    pub sleep: f32,
    pub sleep_rate: f32,
    pub drop: f32,
    pub sleep_phase: f32,
    pub idle: f32,
    pub idle_rate: f32,
    pub idle_phase: f32,
    pub head: HeadMove,
    pub hunker: f32,
    pub hunker_rate: f32,
}

impl PoseEntry {
    pub const ZERO: PoseEntry = PoseEntry {
        sleep: 0.0,
        sleep_rate: 0.0,
        drop: 0.0,
        sleep_phase: 0.0,
        idle: 0.0,
        idle_rate: 0.0,
        idle_phase: 0.0,
        head: HeadMove::NONE,
        hunker: 0.0,
        hunker_rate: 0.0,
    };
}

/// A pack's pose table (`poseK` in `voxel.frag`, WX4): the staged tick (wrapped as
/// [`FrameClock::time`] wraps it), the sleep and idle breaths, and the entries. Entry 0 is
/// never used by a cell (a posing animal cell's `b` byte is its class in the top two bits
/// and its entry, 1 to 63, below; `b = 0` is a body not posing, drawn as it always was).
/// An animal texel of glyph 0 is faceless: an empty voxel a posing body can move into,
/// which only sends the walk looking for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoseTable {
    pub tick: f32,
    pub sleep: Breath,
    pub idle: Breath,
    pub entries: [PoseEntry; POSE_ENTRIES],
}

impl PoseTable {
    pub const EMPTY: PoseTable = PoseTable {
        tick: 0.0,
        sleep: Breath::ZERO,
        idle: Breath::ZERO,
        entries: [PoseEntry::ZERO; POSE_ENTRIES],
    };

    /// Whether any body poses in this pack.
    pub fn posing(&self) -> bool {
        self.entries[1..].iter().any(|e| *e != PoseEntry::ZERO)
    }
}

/// Where one entry's cells are drawn `ticks` after the staged tick, in voxels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoseAt {
    /// Down: the sleep's settle, plus a head's dip.
    pub settle: f32,
    /// The breath's dip of the top of the back: the sleep's and the idle's.
    pub breath: f32,
}

/// One entry, `ticks` after the staged tick: each weight runs on from the staged one at its
/// rate (clamped to `0..=1`) and is eased by a smoothstep; the settle is the drop at the
/// sleep weight (or the hunker weight, whichever is further down); each breath dips at its
/// own weight; a head move goes out and back at the idle weight.
pub fn pose_at(table: &PoseTable, entry: &PoseEntry, ticks: f32) -> PoseAt {
    pose_with(table.sleep, table.idle, entry, ticks)
}

/// [`pose_at`] with the table's two breaths given alone: what the model pass evaluates a
/// figure's own entries with (H2).
pub fn pose_with(sleep: Breath, idle: Breath, entry: &PoseEntry, ticks: f32) -> PoseAt {
    let ticks = ticks.clamp(0.0, 8.0);
    let smooth = |w: f32, r: f32| {
        let w = (w + r * ticks).clamp(0.0, 1.0);
        w * w * (3.0 - 2.0 * w)
    };
    let ws = smooth(entry.sleep, entry.sleep_rate);
    let wi = smooth(entry.idle, entry.idle_rate);
    let wh = smooth(entry.hunker, entry.hunker_rate);
    let head = entry.head.out(ticks) * wi;
    PoseAt {
        settle: entry.drop * ws.max(wh) + entry.head.dip * head,
        breath: ws * sleep.dip(entry.sleep_phase, ticks) + wi * idle.dip(entry.idle_phase, ticks),
    }
}

/// The pose table as the walk reads it this frame, at frame clock `time` (ticks, as
/// [`FrameClock::time`]) and `s` pixels a voxel: entry `k`'s settle and breath dip in whole
/// pixel rows, the settle at most two voxels (as far as the walk looks). Unused entries stay
/// zero.
pub fn pose_rows(table: &PoseTable, time: f32, s: u32) -> [[f32; 4]; POSE_ENTRIES] {
    let mut out = [[0.0; 4]; POSE_ENTRIES];
    let s = s as f32;
    let px = |v: f32| (v * s + 0.5).floor();
    for (o, e) in out.iter_mut().zip(&table.entries).skip(1) {
        if *e != PoseEntry::ZERO {
            let p = pose_at(table, e, time - table.tick);
            *o = [px(p.settle).min(2.0 * s), px(p.breath), 0.0, 0.0];
        }
    }
    out
}

/// Entries in the frame's pose table ([`PoseTable`]): the texel's six entry bits.
pub const POSE_ENTRIES: usize = 64;

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
            bail!(
                "no vine tile at row {row} column {col} of {} bytes",
                rgba.len()
            );
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
            self.rgba[dst..dst + (fw * 4) as usize]
                .copy_from_slice(&rgba[src..src + (fw * 4) as usize]);
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
/// one (`0` is none: the role's generic slot, tinted by the style colour). `emit` is the
/// lit tier's emissive colour in linear light, zero for none: a texel of the glyph atlas's
/// emissive tone draws it unlit at full value (package L step 4); `emit`'s alpha 1 makes
/// every texel of the style's cells emit ([`VoxelStyle::emit_whole`], package L5).
/// The fractional part of a whole-cell emitter's pigment alphas ([`VoxelStyle::emit_whole`]).
const WHOLE_EMITTER: f32 = 0.25;

#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct VoxelStyle {
    pub wood: [f32; 4],
    pub crown: [f32; 4],
    pub heart: [f32; 4],
    pub emit: [f32; 4],
}

impl VoxelStyle {
    pub fn new(wood: [f32; 3], crown: [f32; 3], heart: [f32; 3]) -> VoxelStyle {
        let v = |c: [f32; 3]| [c[0], c[1], c[2], 0.0];
        VoxelStyle {
            wood: [wood[0], wood[1], wood[2], f32::from(ROLE_NONE)],
            crown: v(crown),
            heart: v(heart),
            emit: [0.0; 4],
        }
    }

    /// The same style, its emissive-tone texels drawn in `emit` (linear light) in the lit
    /// tier. `None` is no emission.
    pub fn with_emit(mut self, emit: Option<[f32; 3]>) -> VoxelStyle {
        self.emit = emit.map_or([0.0; 4], |c| [c[0], c[1], c[2], 0.0]);
        self
    }

    /// The same style emitting on **every** visible texel of its cells, not only the
    /// emissive-tone texels its glyph marks (a bloomcrown core glows whole). No effect
    /// without an emissive colour. Call it after [`VoxelStyle::with_role`] and
    /// [`VoxelStyle::with_faces`]: the flag is a quarter in the fractional part of the three
    /// pigment columns' alphas (whose whole parts are the role and the face slots), so the
    /// shader's pigment fetch, which every texel makes, carries it.
    pub fn emit_whole(mut self, whole: bool) -> VoxelStyle {
        let frac = if whole { WHOLE_EMITTER } else { 0.0 };
        for a in [&mut self.wood[3], &mut self.crown[3], &mut self.heart[3]] {
            *a = a.floor() + frac;
        }
        self.emit[3] = if whole { 1.0 } else { 0.0 };
        self
    }

    /// Whether every texel of this style's cells emits ([`VoxelStyle::emit_whole`]).
    pub fn emits_whole(&self) -> bool {
        self.crown[3].fract() > WHOLE_EMITTER / 2.0
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
    /// (`light_levels`: 0 or 1 for smooth, continuous light; 2 or more for a ladder), and
    /// how far an AO crease line darkens the light (`ao_strength`).
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
    /// The lit tier's water (package W): absorption per voxel of water path, in units of
    /// the deep colour's optical depth (after `1 / water_absorb` voxels what is left of the
    /// light behind is the palette's deep colour), which also sets the surface-to-deep
    /// in-scatter ramp.
    pub water_absorb: f32,
    /// What the surface's Fresnel reflectance (about 5-7 % at this camera) is multiplied
    /// by, clamped to 1.
    pub reflect_gain: f32,
    /// How far a ripple tilts the quantised surface normal (its horizontal part).
    pub ripple: f32,
    /// Cells a reflected ray is marched before it counts as sky.
    pub reflect_cells: u32,
    /// The lit tier's water surface details (W checkpoint 2), each a strength, `0` for
    /// none: foam along shores and where a fall meets a pool (`foam`), a highlight in the
    /// palette's light on sunlit ripple lines tilted toward the sun (`glint`), and rings
    /// on open-sky water while it rains (`rain_rings`).
    pub foam: f32,
    pub glint: f32,
    pub rain_rings: f32,
    /// How far glints and rain rings lean from their palette colour toward white (`0`:
    /// the palette's light and surface colours, `1`: white).
    pub highlight: f32,
    /// The lit tier's bloom ([`crate::bloom`]): how much of an emitter's colour its halo
    /// adds (smooth: times the blurred emission; blocky: at full step), `0` for none (the
    /// passes are skipped).
    pub bloom: f32,
    /// How far the halo reaches, in voxel cells (smooth: twice the Gaussian's sigma;
    /// blocky: whole cells, rounded).
    pub bloom_radius: f32,
    /// Smooth (a blurred glow) or blocky (L5's whole-cell halo).
    pub bloom_style: crate::bloom::BloomStyle,
    /// Capture-only: draw the water's derived flow field instead of the water (never
    /// set by the live display).
    pub debug_flow: bool,
    /// The lit tier's effect switches and the light shafts' numbers ([`crate::sunvis`]).
    /// The switches are fixed for a renderer's life; the numbers are not.
    pub effects: crate::sunvis::Effects,
    /// The lit tier's daytime sky (`[light] day_sky`, [`crate::weather::DaySky`]).
    pub day_sky: crate::weather::DaySky,
    /// How much of the brightest stars' light feeds the bloom (`[light] star_bloom`; 0
    /// skips it).
    pub star_bloom: f32,
    /// The day sky and its clouds by day, times this (`[light] day_sky_brightness`).
    pub day_sky_brightness: f32,
    /// How the ground fog moves (`[light] fog_motion`, a trial).
    pub fog_motion: crate::weather::FogMotion,
    /// **Plants as models** (`[figures] plants = "models"`, [`crate::plants`]): the walk
    /// lets its ray through the dry block plant cells and the plant pass draws them,
    /// swaying and growing in. `None` (the default) draws them in the walk as ever. Fixed
    /// for a renderer's life.
    pub plants: Option<crate::plants::PlantMotion>,
    /// **Drape cords** (`[drape]`, [`crate::cord`]): a baked model's drape cells drawn as a
    /// thin cord of this many screen pixels instead of a cube; [`DrapeCords::OFF`] draws
    /// the cubes. Read every frame from the uniform.
    ///
    /// [`DrapeCords::OFF`]: crate::cord::DrapeCords::OFF
    pub drape_cords: crate::cord::DrapeCords,
}

/// Texels per style in the style texture: wood, crown, heart, emit.

/// Voxels per glow-volume cell along each axis: the lit tier's local light from emitters
/// is built at a quarter of the world's resolution ([`VoxelStaging::glow`]).
pub const GLOW_CELL: u32 = 4;

/// Foliage steps the canopy plane holds per column ([`VoxelStaging::canopy`]).
pub const CANOPY_STEPS: usize = 4;

/// Where each plane starts in a staging buffer ([`VoxelRenderer::offsets`]).

impl VoxelParams {
    pub(crate) fn validate(&self) -> Result<()> {
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
        if self.raster_w == 0 {
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

    /// The glow volume's extent: one texel per [`GLOW_CELL`]³ voxels, rounded up, in the
    /// lit tier; one texel in the flat one.
    pub fn glow_extent(&self) -> (u32, u32, u32) {
        if self.lit {
            (
                self.width.div_ceil(GLOW_CELL),
                self.height.div_ceil(GLOW_CELL),
                self.depth.div_ceil(GLOW_CELL),
            )
        } else {
            (1, 1, 1)
        }
    }

    /// The glow volume's bytes: RGBA8 per texel in the lit tier, none in the flat one.
    pub fn glow_bytes(&self) -> usize {
        if self.lit {
            let (w, h, d) = self.glow_extent();
            w as usize * h as usize * d as usize * 4
        } else {
            0
        }
    }

    /// The canopy image's extent: two RGBA8 texels per column, `(x, 2z)` and `(x, 2z+1)`.
    pub(crate) fn canopy_extent(&self) -> (u32, u32) {
        if self.lit {
            (self.width, self.depth * (CANOPY_STEPS as u32 / 2))
        } else {
            (1, 1)
        }
    }

    /// The uniform block, in the layout `voxel.frag` declares, for an atlas holding the
    /// slots in `tex_mask`.
    pub(crate) fn uniforms(
        &self,
        tex_mask: u32,
        vine_on: bool,
        clock: FrameClock,
        look: &crate::weather::Look,
    ) -> VoxelUniforms {
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
            sky: v(look.sky),
            sky_horizon: v(look.sky_horizon),
            bedrock: v(self.bedrock),
            rock: v(self.rock),
            soil: v(self.soil),
            water_deep: v(self.water_deep),
            water_surface: v(self.water_surface),
            light: v(self.light),
            // The flat tier keeps the palette's haze and tints the whole picture instead.
            haze_colour: v(if self.lit {
                look.haze
            } else {
                self.haze_colour
            }),
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
            tex: {
                let [width, shape] = self.drape_cords.uniform();
                [tex_mask as i32, i32::from(vine_on), width, shape]
            },
            light_k: [
                self.ambient_gain,
                self.ambient_floor,
                if self.light_levels < 2 {
                    0.0
                } else {
                    self.light_levels as f32
                },
                self.ao_strength,
            ],
            // A lightning flash is fill light: added to the ambient, it lifts every lit texel.
            ambient: {
                let a = look.ambient.map(|c| c * look.ambient_level);
                [
                    a[0] + look.flash_colour[0],
                    a[1] + look.flash_colour[1],
                    a[2] + look.flash_colour[2],
                    0.0,
                ]
            },
            sun: [look.sun[0], look.sun[1], look.sun[2], look.sun_tint],
            water_l: [
                self.water_absorb.max(0.0),
                self.reflect_gain.max(0.0),
                self.ripple,
                self.reflect_cells as f32,
            ],
            clock: [
                clock.time,
                clock.step,
                if self.debug_flow { 1.0 } else { 0.0 },
                0.0,
            ],
            water_m: [
                self.foam.max(0.0),
                self.glint.max(0.0),
                self.rain_rings.max(0.0),
                self.highlight.clamp(0.0, 1.0),
            ],
            vol: self.effects.uniform(1.0),
            // The sun's share of a rung over the ambient level (1 and the smooth share at a
            // clear noon: exactly the walk's old constants).
            day_k: [
                look.sun_strength / look.ambient_level.max(1e-3),
                SMOOTH_SUN * look.sun_strength / look.ambient_level.max(1e-3),
                look.ambient_level.max(1e-3),
                look.band,
            ],
            day_l: [look.unlit, look.sun_strength, 0.0, 0.0],
            sun_lean: [
                look.sun_lean[0],
                look.sun_lean[1],
                look.sun_lean[2],
                look.sun_side,
            ],
            cloud_k: [look.cloud_cover, look.drift, look.storm, look.seconds],
            cloud_body: [
                look.cloud_body[0],
                look.cloud_body[1],
                look.cloud_body[2],
                look.stars,
            ],
            cloud_lit: [
                look.cloud_lit[0],
                look.cloud_lit[1],
                look.cloud_lit[2],
                look.cloud_edge,
            ],
            band_a: v(look.band_colours[0]),
            band_b: v(look.band_colours[1]),
            band_c: v(look.band_colours[2]),
            tint: v(look.tint),
            day_a: [
                look.day_sky[0][0],
                look.day_sky[0][1],
                look.day_sky[0][2],
                look.day,
            ],
            day_b: [
                look.day_sky[1][0],
                look.day_sky[1][1],
                look.day_sky[1][2],
                look.star_bloom * look.stars,
            ],
            day_c: v(look.day_sky[2]),
            cloud_day: v(look.cloud_day),
            cloud_shade: v(look.cloud_shade),
            rain_k: [look.rain[0], look.rain[1], look.rain[2], look.rings],
            rain_c: [
                look.rain_colour[0],
                look.rain_colour[1],
                look.rain_colour[2],
                look.fog_sun,
            ],
            // The fog's floor is the renderer's (`VoxelRenderer::set_fog_floor`).
            fog_k: [look.fog, 0.0, FOG_DEPTH, FOG_FALLOFF],
            fog_c: [
                look.fog_colour[0],
                look.fog_colour[1],
                look.fog_colour[2],
                look.fog_glow,
            ],
            flash_c: [
                look.flash_colour[0],
                look.flash_colour[1],
                look.flash_colour[2],
                look.flash,
            ],
            strike_k: match look.strike {
                Some(s) => [s.x as f32 + 0.5, s.y as f32, s.z as f32, look.bolt],
                None => [0.0; 4],
            },
            strike_l: match look.strike {
                Some(s) => [
                    (s.seed & 0xFFFF) as f32,
                    ((s.seed >> 16) & 0xFFFF) as f32,
                    0.0,
                    0.0,
                ],
                None => [0.0; 4],
            },
            poses: [[0.0; 4]; POSE_ENTRIES],
            emit_k: [[1.0; 4]; MAX_STYLES / 4],
            view: [0; 4],
            hi_min: [0; 4],
            hi_max: [0; 4],
            hi_k: [0.0; 4],
            hi_cells: [[0; 4]; HIGHLIGHT_CELLS / 4],
            fig_sun: [0.0; 4],
            fig_map: [0.0; 4],
            fog_m: self.fog_motion.uniform(),
            rain_h: [look.rain_haze[0], look.rain_haze[1], look.rain_haze[2], 0.0],
            camera: [0, self.raster_h as i32, 0, 0],
        }
    }
}

/// The fog's full-density band above its floor, voxels, and how many voxels over it the
/// fog thins by a factor of e (interim).
pub const FOG_DEPTH: f32 = 4.0;
pub const FOG_FALLOFF: f32 = 5.0;

/// The frame's clock, as the lit tier's water reads it: sim time in ticks (the tick plus
/// the fraction of it elapsed), and the water's animation step, both wrapped where the
/// shader's patterns repeat so a long run keeps its precision. The step is whole (the
/// water moves in steps) unless the clock is smooth, when it runs on between them and
/// the water moves every frame at the same speed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameClock {
    /// Ticks, wrapped at [`FrameClock::TICK_WRAP`].
    pub time: f32,
    /// Animation steps, wrapped at [`FrameClock::STEP_WRAP`]: whole, or fractional when
    /// smooth.
    pub step: f32,
    /// Sim seconds, unwrapped: what the weather eases on (`crate::weather::WeatherEase`).
    pub seconds: f64,
}

impl FrameClock {
    /// Every period `voxel.frag`'s water patterns have in steps divides this.
    pub const STEP_WRAP: u64 = 2048;
    /// Sim time wraps here (about 14.5 h at 20 Hz), well inside an f32's whole ticks.
    pub const TICK_WRAP: u64 = 1 << 20;

    /// The clock at `tick` plus `fraction` of the next, for water animated at `hz` steps a
    /// second over a sim running `tick_hz` ticks a second; `smooth` keeps the step's
    /// fraction.
    pub fn at(tick: u64, fraction: f64, hz: f64, tick_hz: f64, smooth: bool) -> FrameClock {
        let t = tick as f64 + fraction.clamp(0.0, 1.0);
        let step = if hz > 0.0 && tick_hz > 0.0 {
            let s = t * hz / tick_hz;
            if smooth { s } else { s.floor() }
        } else {
            0.0
        };
        FrameClock {
            time: ((tick % Self::TICK_WRAP) as f64 + fraction.clamp(0.0, 1.0)) as f32,
            step: step.rem_euclid(Self::STEP_WRAP as f64) as f32,
            seconds: if tick_hz > 0.0 { t / tick_hz } else { 0.0 },
        }
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(crate) struct VoxelUniforms {
    pub(crate) geom: [i32; 4],
    pub(crate) extent: [i32; 4],
    pub(crate) knobs: [f32; 4],
    pub(crate) sky: [f32; 4],
    pub(crate) sky_horizon: [f32; 4],
    pub(crate) bedrock: [f32; 4],
    pub(crate) rock: [f32; 4],
    pub(crate) soil: [f32; 4],
    pub(crate) water_deep: [f32; 4],
    pub(crate) water_surface: [f32; 4],
    pub(crate) light: [f32; 4],
    pub(crate) haze_colour: [f32; 4],
    pub(crate) shade_a: [f32; 4],
    pub(crate) shade_b: [f32; 4],
    pub(crate) roof: [f32; 4],
    pub(crate) water: [f32; 4],
    pub(crate) plant_a: [f32; 4],
    pub(crate) plant_b: [f32; 4],
    /// The face textures present (`VoxelTextures::mask`), whether the vine tiles are, and
    /// the drape cords ([`crate::cord::DrapeCords::uniform`]).
    pub(crate) tex: [i32; 4],
    /// The lit tier: ambient gain, ambient floor, ladder rungs, AO strength.
    pub(crate) light_k: [f32; 4],
    /// The lit tier's ambient colour.
    pub(crate) ambient: [f32; 4],
    /// The lit tier's sun: the unit direction toward it, and the sunlit tint.
    pub(crate) sun: [f32; 4],
    /// The lit tier's water: absorption, reflection gain, ripple tilt, reflection cells.
    pub(crate) water_l: [f32; 4],
    /// Sim time in ticks, the water's animation step, the flow overlay flag, padding.
    pub(crate) clock: [f32; 4],
    /// The lit tier's water details: foam, glint, rain rings, highlight.
    pub(crate) water_m: [f32; 4],
    /// The light shafts: density, glow weight, falloff, the volume's fade.
    pub(crate) vol: [f32; 4],
    /// The weather and the time of day (`crate::weather::Look`): ambient level, sun
    /// strength, stars, twilight band.
    pub(crate) day_k: [f32; 4],
    /// The unlit water's level, then padding.
    pub(crate) day_l: [f32; 4],
    /// The colour a sunlit texel leans toward; which side the sun is on.
    pub(crate) sun_lean: [f32; 4],
    /// Cloud cover, drift (voxels), storm, twinkle clock (seconds).
    pub(crate) cloud_k: [f32; 4],
    pub(crate) cloud_body: [f32; 4],
    /// The clouds' lit edge colour, and its strength.
    pub(crate) cloud_lit: [f32; 4],
    /// The twilight band's colours, top to bottom.
    pub(crate) band_a: [f32; 4],
    pub(crate) band_b: [f32; 4],
    pub(crate) band_c: [f32; 4],
    /// The flat tier's whole-picture tint.
    pub(crate) tint: [f32; 4],
    /// The lit sky's day gradient (zenith, middle, horizon) and how much of it shows; the
    /// stars' bloom share rides in `day_b.w`.
    pub(crate) day_a: [f32; 4],
    pub(crate) day_b: [f32; 4],
    pub(crate) day_c: [f32; 4],
    /// The clouds by day: body and shadowed underside.
    pub(crate) cloud_day: [f32; 4],
    pub(crate) cloud_shade: [f32; 4],
    /// The rain (checkpoint 2): drizzle, shower, downpour, and the rings' density; the
    /// streaks' colour.
    pub(crate) rain_k: [f32; 4],
    pub(crate) rain_c: [f32; 4],
    /// The fog: density at its floor, the floor (voxels), the full band, the falloff; its
    /// light and its glow weight.
    pub(crate) fog_k: [f32; 4],
    pub(crate) fog_c: [f32; 4],
    /// Lightning: the fill flash's light and level; the bolt's column (x centre, top y, z)
    /// and brightness; its seed in two 16-bit halves.
    pub(crate) flash_c: [f32; 4],
    pub(crate) strike_k: [f32; 4],
    pub(crate) strike_l: [f32; 4],
    /// The pose table of the pack this frame draws, evaluated on its clock ([`pose_rows`]).
    pub(crate) poses: [[f32; 4]; POSE_ENTRIES],
    /// Each style slot's emission gain this frame: its curve's
    /// ([`VoxelStaging::emit_curves`], [`VoxelRenderer::set_emit_gains`]), four slots a
    /// row; `1` draws the style's emissive colour as it is.
    pub(crate) emit_k: [[f32; 4]; MAX_STYLES / 4],
    /// The view ([`VoxelRenderer::set_view_x`]): raster columns turned round the ring, the
    /// highlight's cell count (0 none, -1 its box alone), padding.
    pub(crate) view: [i32; 4],
    /// The highlighted animal's box: first x, lowest y, nearest z; x span, highest y,
    /// farthest z ([`VoxelRenderer::set_highlight`]).
    pub(crate) hi_min: [i32; 4],
    pub(crate) hi_max: [i32; 4],
    /// How far its cells lean toward white.
    pub(crate) hi_k: [f32; 4],
    /// Its cells, [`Highlight::key`], four a row.
    pub(crate) hi_cells: [[u32; 4]; HIGHLIGHT_CELLS / 4],
    /// The figures' sun map ([`crate::figures::sun_map`]): `figSun`, `figMap`.
    pub(crate) fig_sun: [f32; 4],
    pub(crate) fig_map: [f32; 4],
    /// How the fog moves ([`crate::weather::FogMotion::uniform`]): `fogM`.
    pub(crate) fog_m: [f32; 4],
    /// How much of each rain mode's haze hangs in the air (`rainH`,
    /// [`crate::weather::Look::rain_haze`]).
    pub(crate) rain_h: [f32; 4],
    pub(crate) camera: [i32; 4],
}

/// The most cells a highlight names one by one; a larger body is highlighted by its box.
pub const HIGHLIGHT_CELLS: usize = 256;

/// One animal drawn highlighted ([`VoxelRenderer::set_highlight`]): its cells, and how far
/// they lean toward white over whatever lit colour the walk gives them.
#[derive(Clone, Debug, PartialEq)]
pub struct Highlight {
    /// `(x, y, z)` voxels, `x` wrapped into the world.
    pub cells: Vec<(u32, u32, u32)>,
    /// 0 none .. 1 white.
    pub mix: f32,
}

impl Highlight {
    /// A cell as the shader's list holds it.
    pub fn key(x: u32, y: u32, z: u32) -> u32 {
        (x & 0xFFF) | (y & 0x3FF) << 12 | (z & 0x3FF) << 22
    }

    /// The uniform block's view of it, for a world `width` voxels round: the box (`x` from
    /// its first column round the ring, so a body across the seam has a small box), the
    /// cell count (-1 when there are too many to list), and the list.
    pub(crate) fn block(
        &self,
        width: u32,
    ) -> ([i32; 4], [i32; 4], i32, [[u32; 4]; HIGHLIGHT_CELLS / 4]) {
        let mut cells = [[0u32; 4]; HIGHLIGHT_CELLS / 4];
        if self.cells.is_empty() || width == 0 {
            return ([0; 4], [0; 4], 0, cells);
        }
        // The box's x: the start of the widest gap between occupied columns, round the ring.
        let mut cols: Vec<u32> = self.cells.iter().map(|c| c.0 % width).collect();
        cols.sort_unstable();
        cols.dedup();
        let (mut start, mut gap) = (cols[0], 0u32);
        for (i, &c) in cols.iter().enumerate() {
            let next = cols[(i + 1) % cols.len()];
            let g = (next + width - c) % width;
            let g = if g == 0 { width } else { g };
            if g > gap {
                gap = g;
                start = next;
            }
        }
        let span = width - gap;
        let (mut y0, mut y1, mut z0, mut z1) = (u32::MAX, 0, u32::MAX, 0);
        for &(_, y, z) in &self.cells {
            y0 = y0.min(y);
            y1 = y1.max(y);
            z0 = z0.min(z);
            z1 = z1.max(z);
        }
        let count = if self.cells.len() <= HIGHLIGHT_CELLS {
            for (i, &(x, y, z)) in self.cells.iter().enumerate() {
                cells[i / 4][i % 4] = Highlight::key(x % width, y, z);
            }
            self.cells.len() as i32
        } else {
            -1
        };
        (
            [start as i32, y0 as i32, z0 as i32, 0],
            [span as i32, y1 as i32, z1 as i32, 0],
            count,
            cells,
        )
    }
}

/// Where one tick's world is written, straight into mapped memory.
///
/// The slices are the staging buffer itself, so the packer on the CPU writes the
/// voxels once instead of filling a `Vec` and copying it. The roof table is indexed
/// `(z · height + y) · width + x` — texture upload order, not the core's `Config::index`.
pub struct VoxelStaging<'a> {
    /// The plant pass's cell list for this buffer ([`crate::plants::PlantList`]), when
    /// plants are models: the pack writes the whole list every time.
    pub plants: Option<&'a mut crate::plants::PlantList>,
    /// The voxels this buffer carries up ([`VoxelStaging::carried`]): the whole volume in
    /// texture order, or the owed bricks one after another ([`write_upload`]).
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
    /// the list. A slow plane ([`PLANE_CANOPY`]). Empty in the flat tier.
    pub canopy: &'a mut [u8],
    /// The lit tier's glow volume: the local light of the frame's emitters, RGBA8 (alpha
    /// unused) at [`GLOW_CELL`]³ voxels a texel, `((z · h + y) · w + x) · 4` over
    /// [`VoxelParams::glow_extent`], in units of full ambient light. A slow plane
    /// ([`PLANE_GLOW`]): written only when its key moves. Empty in the flat tier.
    pub glow: &'a mut [u8],
    /// The pose table ([`PoseTable`]), [`PoseTable::EMPTY`] when handed over: a pack with no
    /// animal posing leaves it so.
    pub poses: &'a mut PoseTable,
    /// Each style slot's emission curve: `0` (as handed over) draws its emissive colour as
    /// it is, `k` times curve `k`'s gain this frame ([`VoxelRenderer::set_emit_gains`]).
    pub emit_curves: &'a mut [u8; MAX_STYLES],
    /// Whether this pack must write [`VoxelStaging::roof`], [`VoxelStaging::glyphs`],
    /// [`VoxelStaging::sky`], [`VoxelStaging::glow`], [`VoxelStaging::styles`] and
    /// [`VoxelStaging::canopy`] (indexed [`PLANE_ROOF`], [`PLANE_GLYPHS`], [`PLANE_SKY`],
    /// [`PLANE_GLOW`], [`PLANE_STYLES`], [`PLANE_CANOPY`]). A plane marked false already
    /// holds the content its key names and must be left alone. See [`SlowPlanes`].
    pub write: [bool; SLOW_PLANES],
    /// The voxel bricks the images do not hold yet ([`BrickOwed`]): the pack marks the
    /// bricks it changed, then writes every owed brick into [`VoxelStaging::voxels`]
    /// ([`write_upload`]).
    pub owed: &'a mut BrickOwed,
    /// What this buffer's [`VoxelStaging::voxels`] carries up: the whole volume, or the
    /// listed bricks packed one after another. Written by [`write_upload`].
    pub carried: &'a mut Carried,
    /// The style table's key ([`PLANE_STYLES`]), known only once the pack has written it:
    /// the pack writes the whole table into every buffer and names its version here.
    pub styles_key: &'a mut u64,
}

/// The edge of a voxel brick, the unit a change-only pack uploads: `BRICK`³ texels.
pub const BRICK: u32 = 8;
/// Texels in one brick's slot of the staging buffer.
pub const BRICK_TEXELS: usize = (BRICK * BRICK * BRICK) as usize;

/// Bricks along each axis of a `width × height × depth` volume (the last one in an axis
/// may be partial).
pub fn brick_dims(width: u32, height: u32, depth: u32) -> (u32, u32, u32) {
    (
        width.div_ceil(BRICK),
        height.div_ceil(BRICK),
        depth.div_ceil(BRICK),
    )
}

/// The brick holding voxel `(x, y, z)`: `(bz · bricks_h + by) · bricks_w + bx`.
#[inline]
pub fn brick_of(width: u32, height: u32, x: u32, y: u32, z: u32) -> u32 {
    let (bw, bh) = (width.div_ceil(BRICK), height.div_ceil(BRICK));
    ((z / BRICK) * bh + y / BRICK) * bw + x / BRICK
}

/// Which voxel bricks the images are owed: everything a pack changed that no recorded
/// frame has carried up yet, or the whole volume.
///
/// A pack marks the bricks it changed ([`BrickOwed::mark`]) and writes every owed brick
/// into its staging buffer. Recording that buffer's frame takes what it carried off the
/// list ([`BrickOwed::taken`]): frames run in the order they are recorded, so a later pack
/// owes only what changed after it. A frame displaced before it ran gives its bricks back
/// ([`BrickOwed::returned`]).
#[derive(Clone, Debug)]
pub struct BrickOwed {
    owed: Vec<bool>,
    list: Vec<u32>,
    full: bool,
}

impl BrickOwed {
    /// Nothing held yet: the whole volume is owed.
    pub fn new(width: u32, height: u32, depth: u32) -> BrickOwed {
        let (bw, bh, bd) = brick_dims(width, height, depth);
        BrickOwed {
            owed: vec![false; (bw * bh * bd) as usize],
            list: Vec::new(),
            full: true,
        }
    }

    /// Brick `b` changed.
    #[inline]
    pub fn mark(&mut self, b: u32) {
        if self.full {
            return;
        }
        let o = &mut self.owed[b as usize];
        if !*o {
            *o = true;
            self.list.push(b);
        }
    }

    /// Everything changed: the next pack writes the whole volume.
    pub fn mark_full(&mut self) {
        self.full = true;
        for &b in &self.list {
            self.owed[b as usize] = false;
        }
        self.list.clear();
    }

    /// Whether the whole volume is owed.
    pub fn full(&self) -> bool {
        self.full
    }

    /// The owed bricks, in the order they were first marked.
    pub fn list(&self) -> &[u32] {
        &self.list
    }

    /// How many bricks the volume has.
    pub fn bricks(&self) -> usize {
        self.owed.len()
    }

    /// A recorded frame carries `carried` up.
    pub fn taken(&mut self, carried: &Carried) {
        if carried.full {
            // Everything owed went up with it: changes are only marked by a pack, and
            // this was the last pack before the frame.
            self.full = false;
            for &b in &self.list {
                self.owed[b as usize] = false;
            }
            self.list.clear();
            return;
        }
        if self.full {
            return;
        }
        for &b in &carried.bricks {
            self.owed[b as usize] = false;
        }
        let owed = &self.owed;
        self.list.retain(|&b| owed[b as usize]);
    }

    /// A frame carrying `carried` was displaced before it ran.
    pub fn returned(&mut self, carried: &Carried) {
        if carried.full {
            self.mark_full();
            return;
        }
        for &b in &carried.bricks {
            self.mark(b);
        }
    }
}

/// What one staging buffer carries up ([`VoxelStaging::carried`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Carried {
    /// The whole volume, in texture order.
    pub full: bool,
    /// Otherwise these bricks, brick `k` of the list in texels `k · BRICK_TEXELS ..`, `x`
    /// fastest then `y` then `z` within the brick at a row length and image height of
    /// [`BRICK`] (a partial brick leaves its slot's tail unused).
    pub bricks: Vec<u32>,
}

pub(crate) fn image_upload_bytes(
    p: &VoxelParams,
    carried: &Carried,
    slow: [bool; SLOW_PLANES],
) -> usize {
    let n = p.voxel_count();
    let voxels = if carried.full {
        n
    } else {
        carried.bricks.len() * BRICK_TEXELS
    } * 4;
    let sizes = [
        n,
        p.glyph_bytes(),
        p.sky_bytes(),
        p.glow_bytes(),
        MAX_STYLES * std::mem::size_of::<VoxelStyle>(),
        p.canopy_bytes(),
    ];
    voxels
        + slow
            .iter()
            .enumerate()
            .filter(|&(k, &up)| {
                up && (p.lit || !matches!(k, PLANE_SKY | PLANE_GLOW | PLANE_CANOPY))
            })
            .map(|(k, _)| sizes[k])
            .sum::<usize>()
}

#[cfg(test)]
mod upload_stats_tests {
    use super::*;

    #[test]
    fn upload_totals_count_drawn_batches_and_leave_unchanged_ticks_empty() {
        let gpu = Gpu::open().expect("headless GPU device");
        let p = tests::params();
        let mut r = VoxelRenderer::new(&gpu, p, &VoxelTextures::empty(p.s, p.rise)).unwrap();
        let mut target = crate::target::Headless::new(&gpu, &r).unwrap();
        let master = vec![VoxelTexel::default(); p.voxel_count()];
        let stage = |r: &mut VoxelRenderer, changed: bool| {
            assert!(r.stage([1; SLOW_PLANES], |st| {
                if changed {
                    st.owed.mark(0);
                }
                write_upload(
                    &master, p.width, p.height, p.depth, st.owed, st.voxels, st.carried,
                );
                st.roof.fill(0);
                st.glyphs.fill(0);
                st.sky.fill(255);
                st.canopy.fill(0);
                st.glow.fill(0);
            }));
        };
        stage(&mut r, false);
        assert_eq!(r.upload_totals(), (0, 0), "a stage has not uploaded");
        target.draw(&gpu, &mut r, ()).unwrap();
        let first = r.upload_totals();
        assert!(first.0 > 0);
        stage(&mut r, false);
        assert_eq!(r.last_upload_bytes(), 0, "an unchanged tick owes no images");
        target.draw(&gpu, &mut r, ()).unwrap();
        let empty = r.upload_totals();
        assert_eq!(empty.0, first.0, "a tick does not upload the full capacity");
        target.draw(&gpu, &mut r, ()).unwrap();
        assert_eq!(r.upload_totals(), empty, "a re-present uploads nothing");
        stage(&mut r, true);
        stage(&mut r, true);
        let bytes = r.last_upload_bytes() as u64;
        assert!(bytes > 0);
        assert_eq!(
            r.upload_totals(),
            empty,
            "overwritten stages are not uploads"
        );
        target.draw(&gpu, &mut r, ()).unwrap();
        assert_eq!(r.upload_totals(), (empty.0 + bytes, empty.1 + 1));
        target.destroy(&gpu);
        r.destroy(&gpu);
    }
}

/// Write what `owed` says the images lack from `master` (texture order) into a staging
/// buffer's `voxels`, and say what it carries in `carried`. The whole volume when it is
/// owed, when the owed bricks are over a quarter of them, or when they do not fit.
pub fn write_upload(
    master: &[VoxelTexel],
    width: u32,
    height: u32,
    depth: u32,
    owed: &BrickOwed,
    voxels: &mut [VoxelTexel],
    carried: &mut Carried,
) {
    let n = master.len();
    let whole = owed.full()
        || owed.list().len() * 4 > owed.bricks()
        || owed.list().len() * BRICK_TEXELS > voxels.len();
    if whole {
        voxels[..n].copy_from_slice(master);
        carried.full = true;
        carried.bricks.clear();
        return;
    }
    carried.full = false;
    carried.bricks.clear();
    carried.bricks.extend_from_slice(owed.list());
    let (bw, bh, _) = brick_dims(width, height, depth);
    let (w, h) = (width as usize, height as usize);
    for (k, &b) in owed.list().iter().enumerate() {
        let (bx, by, bz) = (b % bw, (b / bw) % bh, b / (bw * bh));
        let (x0, y0, z0) = (bx * BRICK, by * BRICK, bz * BRICK);
        let ex = (width - x0).min(BRICK) as usize;
        let slot = &mut voxels[k * BRICK_TEXELS..(k + 1) * BRICK_TEXELS];
        for dz in 0..(depth - z0).min(BRICK) {
            for dy in 0..(height - y0).min(BRICK) {
                let src = ((z0 + dz) as usize * h + (y0 + dy) as usize) * w + x0 as usize;
                let dst = ((dz * BRICK + dy) * BRICK) as usize;
                slot[dst..dst + ex].copy_from_slice(&master[src..src + ex]);
            }
        }
    }
}

/// The copy regions of a brick upload: one per carried brick, from its slot of the
/// staging buffer (`base` the voxel plane's offset) into its place in the image.

/// The upload of `carried` from a staging buffer's `voxels` into `image` (texture order),
/// on the CPU: what the frame's copy does on the GPU. For tests and the pack oracle.
pub fn apply_upload(
    voxels: &[VoxelTexel],
    carried: &Carried,
    width: u32,
    height: u32,
    depth: u32,
    image: &mut [VoxelTexel],
) {
    if carried.full {
        image.copy_from_slice(&voxels[..image.len()]);
        return;
    }
    let (bw, bh, _) = brick_dims(width, height, depth);
    let (w, h) = (width as usize, height as usize);
    for (k, &b) in carried.bricks.iter().enumerate() {
        let (bx, by, bz) = (b % bw, (b / bw) % bh, b / (bw * bh));
        let (x0, y0, z0) = (bx * BRICK, by * BRICK, bz * BRICK);
        let ex = (width - x0).min(BRICK) as usize;
        let slot = &voxels[k * BRICK_TEXELS..(k + 1) * BRICK_TEXELS];
        for dz in 0..(depth - z0).min(BRICK) {
            for dy in 0..(height - y0).min(BRICK) {
                let dst = ((z0 + dz) as usize * h + (y0 + dy) as usize) * w + x0 as usize;
                let src = ((dz * BRICK + dy) * BRICK) as usize;
                image[dst..dst + ex].copy_from_slice(&slot[src..src + ex]);
            }
        }
    }
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

    /// Whether a pack waits for a frame to take it.
    pub fn for_pack_waiting(&self) -> bool {
        self.packed.is_some()
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
    /// Its buffer is free again and the upload it carried is owed again: it goes up as
    /// the waiting pack, unless a newer pack already waits. That one stays, and the
    /// displaced frame's bricks, given back to [`BrickOwed`], go up with the pack after.
    pub fn discard(&mut self) -> Option<Recorded> {
        let frame = self.recorded.pop_back()?;
        if frame.staging.is_some() && self.packed.is_none() {
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
/// The lit tier's glow volume's place in [`SlowPlanes`] and in a pack's keys.
pub const PLANE_GLOW: usize = 3;
/// The style table's place (with its emission curves) in [`SlowPlanes`] and in a pack's keys.
pub const PLANE_STYLES: usize = 4;
/// The lit tier's canopy plane's place in [`SlowPlanes`] and in a pack's keys.
pub const PLANE_CANOPY: usize = 5;
/// How many planes are written and uploaded only when they change.
pub const SLOW_PLANES: usize = 6;

/// What the **slow planes** — the roof table, the glyph atlas, the sky plane, the glow
/// volume, the style table and the canopy plane — hold,
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

    /// `buffer` holds `key` for `plane` after all: a plane whose content is known only
    /// once the pack has run (the style table).
    pub fn restamp(&mut self, buffer: usize, plane: usize, key: u64) {
        self.buffers[buffer][plane] = Some(key);
    }

    /// Which planes a frame uploading from `buffer` now would copy, without taking it.
    pub fn would_upload(&self, buffer: usize) -> [bool; SLOW_PLANES] {
        let held = self.buffers[buffer];
        std::array::from_fn(|p| held[p].is_none() || held[p] != self.images[p])
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

#[cfg(test)]
use crate::wgpu_backend::Gpu;
pub use crate::wgpu_backend::voxel::{Pipelines, VoxelRenderer};
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
pub(crate) fn weather_drawn(look: &crate::weather::Look, params: &VoxelParams) -> bool {
    look.rain.iter().any(|&r| r > 0.0)
        || look.rain_haze.iter().any(|&h| h > 0.0)
        || look.bolt > 0.0
        || (look.fog > 0.0 && !params.effects.volume_on(params.lit))
}
#[cfg(test)]
#[path = "fog_occlude_tests.rs"]
mod fog_occlude_tests;

#[cfg(test)]
#[path = "still_pipelines_tests.rs"]
mod still_pipelines_tests;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn params() -> VoxelParams {
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
            water_absorb: 0.1,
            reflect_gain: 6.0,
            ripple: 0.2,
            reflect_cells: 64,
            foam: 0.0,
            glint: 0.0,
            rain_rings: 0.0,
            highlight: 0.0,
            bloom: 0.0,
            bloom_radius: 2.0,
            bloom_style: crate::bloom::BloomStyle::Smooth,
            debug_flow: false,
            effects: crate::sunvis::Effects::default(),
            day_sky: crate::weather::DaySky::Warm,
            star_bloom: 0.0,
            day_sky_brightness: 1.0,
            fog_motion: crate::weather::FogMotion::default(),
            plants: None,
            drape_cords: crate::cord::DrapeCords::OFF,
        }
    }

    /// The water's clock steps at its own rate over sim time (12 steps a second over 20
    /// ticks a second: a step every 5/3 ticks, fractions included), and wraps its step
    /// and its time where the shader's patterns repeat.
    #[test]
    fn a_highlight_boxes_its_cells_the_short_way_round_the_ring() {
        let h = Highlight {
            cells: vec![(9, 2, 1), (0, 3, 1), (9, 3, 2)],
            mix: 0.4,
        };
        let (lo, hi, count, cells) = h.block(10);
        // Columns 9 and 0 are neighbours across the seam: the box starts at 9, spans 1.
        assert_eq!((lo[0], hi[0]), (9, 1));
        assert_eq!((lo[1], hi[1], lo[2], hi[2]), (2, 3, 1, 2));
        assert_eq!(count, 3);
        assert_eq!(cells[0][1], Highlight::key(0, 3, 1));
        // Too many cells to list: the box alone.
        let big = Highlight {
            cells: vec![(1, 1, 1); HIGHLIGHT_CELLS + 1],
            mix: 0.4,
        };
        assert_eq!(big.block(10).2, -1);
        assert_eq!(
            Highlight {
                cells: vec![],
                mix: 0.4
            }
            .block(10)
            .2,
            0
        );
    }

    #[test]
    fn the_frame_clock_steps_at_the_water_rate_and_wraps() {
        let at = |tick: u64, f: f64| FrameClock::at(tick, f, 12.0, 20.0, false);
        assert_eq!(at(0, 0.0).step, 0.0);
        assert_eq!(at(1, 0.6).step, 0.0);
        assert_eq!(at(1, 0.7).step, 1.0);
        assert_eq!(at(5, 0.0).step, 3.0);
        assert_eq!(at(100, 0.25).time, 100.25);
        // 3415 ticks are 2049 steps: one past the wrap.
        assert_eq!(at(3415, 0.0).step, 1.0);
        assert_eq!(at(FrameClock::TICK_WRAP + 3, 0.5).time, 3.5);
        // Smooth, the step runs on between whole steps, and wraps the same.
        let smooth = |tick: u64, f: f64| FrameClock::at(tick, f, 12.0, 20.0, true).step;
        assert!((smooth(1, 0.5) - 0.9).abs() < 1e-6);
        assert!((smooth(3415, 0.5) - 1.3).abs() < 1e-3);
    }

    /// The water's surface details reach the shader's `waterM` in order (foam, glint, rain
    /// rings, highlight), a negative strength as none and the highlight clamped to 0..1.
    #[test]
    fn the_water_details_reach_their_uniform() {
        let p = VoxelParams {
            foam: 0.6,
            glint: -1.0,
            rain_rings: 0.5,
            highlight: 1.5,
            ..params()
        };
        let u = p.uniforms(0, false, FrameClock::default(), &crate::weather::noon(&p));
        assert_eq!(u.water_m, [0.6, 0.0, 0.5, 1.0]);
    }

    /// Any water at all packs to a non-zero fraction, so the presenter's "at least one
    /// row of water is visible" survives the quantisation; a dry cell packs to zero.
    #[test]
    fn a_whole_emitter_keeps_its_role_and_face_slots() {
        let s = VoxelStyle::new([0.1; 3], [0.2; 3], [0.3; 3])
            .with_role(ROLE_LEAF)
            .with_faces([Some(3), None])
            .with_emit(Some([1.0, 0.5, 0.2]))
            .emit_whole(true);
        assert!(s.emits_whole());
        assert_eq!((s.role(), s.faces()), (ROLE_LEAF, [Some(3), None]));
        // The shader's reads: `int(a + 0.5)` for the role and the slots, `fract(a)` for the flag.
        assert_eq!((s.wood[3] + 0.5) as i32, i32::from(ROLE_LEAF));
        assert_eq!((s.crown[3] + 0.5) as i32 - 1, 3);
        assert_eq!((s.heart[3] + 0.5) as i32 - 1, -1);
        for a in [s.wood[3], s.crown[3], s.heart[3]] {
            assert!(a.fract() > 0.125);
        }
        let off = s.emit_whole(false);
        assert!(!off.emits_whole());
        assert_eq!((off.role(), off.faces()), (ROLE_LEAF, [Some(3), None]));
    }

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

    /// A frame displaced after a newer pack was made keeps that newer pack waiting: the
    /// displaced frame's buffer is free, and its bricks stay owed for the pack after.
    #[test]
    fn a_displaced_frame_leaves_a_newer_pack_waiting_and_its_bricks_owed() {
        let mut ring = StagingRing::new(STAGING_RING);
        let a = ring.for_pack().expect("free");
        ring.packed(a);
        let frame = ring.record();
        assert_eq!(frame.staging, Some(a));
        let b = ring.for_pack().expect("the other buffer");
        assert_ne!(a, b);
        ring.packed(b);
        assert_eq!(ring.discard(), Some(frame));
        assert_eq!(
            ring.for_pack(),
            Some(b),
            "the newer pack is still the one waiting"
        );
        assert_eq!(ring.record().staging, Some(b), "and it is what goes up");
    }

    /// The bricks through record, pack, discard, record: nothing a pack changed is lost,
    /// whether the displaced pack carried bricks or the whole volume.
    #[test]
    fn bricks_survive_a_displaced_frame_and_a_newer_pack() {
        let (w, h, d) = (32, 16, 8);
        for first_full in [false, true] {
            let mut owed = BrickOwed::new(w, h, d);
            let mut carried = [Carried::default(), Carried::default()];
            let master = vec![VoxelTexel::default(); (w * h * d) as usize];
            let mut buf = vec![VoxelTexel::default(); master.len()];
            if !first_full {
                // A first whole upload, run.
                write_upload(&master, w, h, d, &owed, &mut buf, &mut carried[0]);
                owed.taken(&carried[0]);
                owed.mark(1);
                owed.mark(2);
            }
            // Pack A into buffer 0, recorded.
            write_upload(&master, w, h, d, &owed, &mut buf, &mut carried[0]);
            assert_eq!(carried[0].full, first_full);
            owed.taken(&carried[0]);
            // Pack B into buffer 1: bricks 2 and 3 changed since.
            owed.mark(2);
            owed.mark(3);
            write_upload(&master, w, h, d, &owed, &mut buf, &mut carried[1]);
            // A is displaced; B goes up.
            owed.returned(&carried[0]);
            owed.taken(&carried[1]);
            if first_full {
                assert!(owed.full(), "a displaced whole upload is owed whole");
            } else {
                assert_eq!(
                    owed.list(),
                    &[1],
                    "A's brick 1 is still owed; 2 and 3 went up with B"
                );
            }
        }
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
        assert_eq!(
            slow.pack(0, [7, 1, 7, 0, 0, 0]),
            all,
            "a new buffer holds nothing"
        );
        assert_eq!(slow.upload(0), all, "and the images nothing");
        assert_eq!(
            slow.pack(1, [7, 1, 7, 0, 0, 0]),
            all,
            "the other buffer is new too"
        );
        assert_eq!(slow.upload(1), none, "but the images already hold it");
        assert_eq!(slow.pack(0, [7, 1, 7, 0, 0, 0]), none, "nothing moved");
        assert_eq!(slow.upload(0), none);

        // The terrain moves: the roof goes into each buffer once, and up once.
        assert_eq!(
            slow.pack(1, [8, 1, 7, 0, 0, 0]),
            [true, false, false, false, false, false]
        );
        assert_eq!(slow.upload(1), [true, false, false, false, false, false]);
        assert_eq!(
            slow.pack(0, [8, 1, 7, 0, 0, 0]),
            [true, false, false, false, false, false],
            "buffer 0 still held 7"
        );
        assert_eq!(slow.upload(0), none, "the images hold 8 already");
        assert_eq!(slow.pack(1, [8, 1, 7, 0, 0, 0]), none);

        // The sky for it arrives later, from its own thread: it alone moves.
        assert_eq!(
            slow.pack(0, [8, 1, 8, 0, 0, 0]),
            [false, false, true, false, false, false]
        );
        assert_eq!(slow.upload(0), [false, false, true, false, false, false]);
    }

    /// A displaced frame's upload never ran: whatever it carried is owed again, so the
    /// images are not trusted to hold it.
    #[test]
    fn a_displaced_upload_leaves_the_slow_planes_owed() {
        let mut slow = SlowPlanes::new(2);
        slow.pack(0, [3, 1, 3, 0, 0, 0]);
        slow.upload(0);
        slow.pack(1, [4, 1, 4, 0, 0, 0]);
        slow.upload(1);
        slow.forget_images();
        assert_eq!(
            slow.pack(1, [4, 1, 4, 0, 0, 0]),
            [false; SLOW_PLANES],
            "the buffer still holds it"
        );
        assert_eq!(slow.upload(1), [true; SLOW_PLANES], "the images might not");
    }

    #[test]
    fn cropped_rasters_are_valid_but_empty_rasters_are_refused() {
        let mut p = params();
        p.raster_w = 100;
        assert!(p.validate().is_ok());
        p.raster_w = 0;
        assert!(p.validate().is_err());
        let mut p = params();
        p.rise = 5;
        assert!(p.validate().is_err());
        assert_eq!(params().voxel_count(), 128 * 48 * 24);
        assert_eq!(
            params().upload_bytes(),
            128 * 48 * 24 * 5 + 256 * 64 + 4 * (4 + 2) * MAX_GLYPHS
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
        let top = TEXTURE_SLOTS
            .iter()
            .position(|f| f.0 == "rock-top")
            .unwrap();
        assert_eq!(t.face_size(top), (6, 3));
        assert!(
            t.put(top, 0, &[0; 6 * 6 * 4]).is_err(),
            "a side-sized top is refused"
        );
        for v in 0..TEX_VARIANTS {
            assert_eq!(t.mask(), 0, "{v} of 4 variants is not a slot");
            t.put(top, v, &[v as u8 + 1; 6 * 3 * 4]).unwrap();
        }
        assert_eq!(t.mask(), 1 << top);
        let at = |x: u32, y: u32| t.rgba[((y * aw + x) * 4) as usize];
        let row = top as u32 * 6;
        assert_eq!(at(2 * 6, row), 3, "variant 2's first texel");
        assert_eq!(at(2 * 6 + 5, row + 2), 3, "and its last");
        assert_eq!(
            at(2 * 6, row + 3),
            0,
            "a top face leaves its cell's lower rows"
        );
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
        assert_eq!(
            (&f.crown[..3], &f.heart[..3]),
            (&[0.2f32; 3][..], &[0.3f32; 3][..])
        );
    }

    /// A named slot goes after the fixed ones, grows the atlas by one row of cells, is
    /// found by name, and stays out of the fixed mask.
    #[test]
    fn a_named_slot_grows_the_atlas_after_the_fixed_slots() {
        let (s, rise) = (6, 3);
        let mut t = VoxelTextures::empty(s, rise);
        let slot = t.add_slot("species/siphonreed/leaf-top", TexFace::Top);
        assert_eq!(slot, TEXTURE_SLOTS.len());
        assert_eq!(
            t.atlas_size(),
            (4 * 6, (TEXTURE_SLOTS.len() as u32 + 1) * 6)
        );
        assert_eq!(
            t.rgba.len(),
            (4 * 6 * (TEXTURE_SLOTS.len() + 1) * 6 * 4) as usize
        );
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

    /// Whole-cell and weighted walks follow the table carried by this frame, independently
    /// of emitter and vine payloads, and return to the whole-cell walk after the change.
    #[test]
    fn plant_light_variant_follows_the_current_table_payload() {
        let mut cells = vec![EmitCell::emitter(7, [0.2, 0.4, 0.6], 0.5)];
        let mut table = Vec::new();
        let cap = emit_table(&cells, &mut table);
        assert_eq!(
            emit_view(cap, &cells) & EMIT_PLANTS,
            0,
            "emission alone uses the whole-cell walk"
        );

        cells[0].vine = Some(((3, 2), 0.5));
        assert_ne!(emit_view(cap, &cells) & EMIT_VINES, 0);
        assert_eq!(
            emit_view(cap, &cells) & EMIT_PLANTS,
            0,
            "vine changes alone use the whole-cell walk"
        );

        cells.push(EmitCell {
            plant: Some(0.25),
            edge: [Some(0.75), None],
            ..EmitCell::none(9)
        });
        let cap = emit_table(&cells, &mut table);
        let view = emit_view(cap, &cells);
        assert_ne!(
            view & EMIT_PLANTS,
            0,
            "a changing plant selects the weighted walk"
        );
        assert_ne!(view & EMIT_VINES, 0, "the vine payload remains enabled");
        assert_eq!(
            view & (EMIT_VINES - 1),
            cap,
            "payload flags leave the hash capacity intact"
        );
        assert_eq!(
            emit_view(0, &cells),
            0,
            "no uploaded table uses the whole-cell walk"
        );

        cells.pop();
        let cap = emit_table(&cells, &mut table);
        assert_eq!(
            emit_view(cap, &cells) & EMIT_PLANTS,
            0,
            "the next frame returns to the whole-cell walk"
        );
        cells.clear();
        assert_eq!(emit_view(emit_table(&cells, &mut table), &cells), 0);
    }

    /// The emitter table as `world.glsl`'s `cellEmit` reads it: every cell given is found
    /// by its own probe with its colour and on share (to half precision), a cell not given
    /// is not, and an empty list is no table.
    #[test]
    fn the_emitter_table_finds_each_cell_the_shader_way() {
        let cells: Vec<EmitCell> = (0..500u32)
            .map(|k| EmitCell {
                cell: k * 7919 % 100_003,
                rgb: [k as f32 / 500.0, 0.25, 1.0 - k as f32 / 1000.0],
                on: (k % 11) as f32 / 10.0,
                emits: k % 3 != 0,
                vine: (k % 2 == 0).then(|| ((k as u8, (k >> 3) as u8), (k % 7) as f32 / 6.0)),
                plant: (k % 5 != 0).then(|| (k % 13) as f32 / 12.0),
                edge: [
                    (k % 3 == 1).then(|| (k % 9) as f32 / 8.0),
                    (k % 4 == 1).then(|| (k % 5) as f32 / 4.0),
                ],
            })
            .collect();
        let mut out = Vec::new();
        let cap = emit_table(&cells, &mut out);
        assert!(cap.is_power_of_two() && cap as usize >= 2 * cells.len());
        let unhalf = |h: u32| {
            let (sign, exp, man) = ((h >> 15) & 1, (h >> 10) & 31, h & 1023);
            let v = if exp == 0 {
                man as f32 / 1024.0 * 2f32.powi(-14)
            } else {
                (1.0 + man as f32 / 1024.0) * 2f32.powi(exp as i32 - 15)
            };
            if sign == 1 { -v } else { v }
        };
        let find = |cell: u32| -> Option<([f32; 4], u32)> {
            let key = cell + 1;
            let mut h = emit_hash(key, cap);
            for _ in 0..EMIT_PROBE {
                let e = out[h as usize];
                if e[0] == key {
                    return Some((
                        [
                            unhalf(e[1] & 0xffff),
                            unhalf(e[1] >> 16),
                            unhalf(e[2] & 0xffff),
                            unhalf(e[2] >> 16),
                        ],
                        e[3],
                    ));
                }
                if e[0] == 0 {
                    return None;
                }
                h = (h + 1) & (cap - 1);
            }
            None
        };
        for c in &cells {
            let (got, word) = find(c.cell).expect("every cell is found");
            let want = [c.rgb[0], c.rgb[1], c.rgb[2], c.on];
            for k in 0..4 {
                assert!(
                    (got[k] - want[k]).abs() <= want[k].abs() / 1000.0 + 1e-6,
                    "{got:?} for {want:?}"
                );
            }
            // The fourth word as `cellEmit` and `cellVineFrom` read it.
            assert_eq!(word >> 31 == 0, c.emits, "the emission bit");
            let share = ((word >> 16) & 255) as f32 / 255.0;
            match c.vine {
                Some(((b, a), s)) if s > 0.5 / 255.0 => {
                    assert_eq!(
                        ((word & 255) as u8, ((word >> 8) & 255) as u8),
                        (b, a),
                        "the old tile"
                    );
                    assert!(
                        (share - s).abs() <= 0.5 / 255.0 + 1e-6,
                        "its share {share} for {s}"
                    );
                }
                _ => assert_eq!(share, 0.0, "no old tile"),
            }
            // CELL-SHRINK: the plant size as `plantSize` reads it, whole when none was given;
            // the emission bit and the old tile are left as they were.
            let size = plant_size_of(word);
            match c.plant {
                Some(p) => assert!(
                    (size - p).abs() <= 0.5 / PLANT_STEPS + 1e-6,
                    "size {size} for {p}"
                ),
                None => assert_eq!(size, 1.0, "a cell given no size is whole"),
            }
            // Its crown edges, unless an old vine tile holds those bits.
            let tile = matches!(c.vine, Some((_, s)) if s > 0.5 / 255.0);
            for side in 0..2 {
                let got = edge_share_of(word, side);
                match c.edge[side] {
                    Some(e) if !tile => assert!(
                        (got - e).abs() <= 0.5 / EDGE_STEPS + 1e-6,
                        "edge {got} for {e}"
                    ),
                    _ => assert_eq!(
                        got, 1.0,
                        "no edge given (or a vine tile's) draws the glyph's"
                    ),
                }
            }
        }
        assert!(find(100_004).is_none());
        assert_eq!(emit_table(&[], &mut out), 0);
    }
}
