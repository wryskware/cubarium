//! Lanternjaw — the selected megafauna body, drawn code-natively as a multipart rig.
//!
//! **Presentation only.** This module draws a picture: it is not a founder, a species, a
//! diet, a capability or a hunting outcome, and nothing in `cubarium-core` knows it exists.
//! It is not in the atelier pack and no rig index selects it, so no saved genome, no
//! founder and no `rig_of` fallback changes because it is here. It exists so that a later,
//! separately validated predator experiment has an actual reusable body to draw with —
//! the production form of the art study in `art/studies/megafauna/fable.js` (Wrysk's
//! preferred candidate, 2026-09-12), whose look this module preserves and whose
//! whole-pixel stepped motion it deliberately does **not** carry over.
//!
//! # What is preserved from the study
//!
//! The 18 × 5 hull template (`ROWS`), the nine-plate segmentation, the cyan lantern chain
//! with its travelling crest, the folded raptorial forelimbs that carry light only as they
//! extend, three walking-leg pairs, the tail fan, the bud cocoon, the four modes and every
//! rhythm, envelope and colour of `fable.md` "Footprint and timing" and "Palette". The warm
//! accent still appears only at the peak of the hunt's 240 ms strike envelope.
//!
//! # What changes for production
//!
//! * **Fractional motion.** The study rounded every column offset, every limb joint and the
//!   anchor to whole pixels. Here the sinuous wave `dy_i`, the coil/lunge `dx_i`, the limb
//!   joints and the leg swing are used **unrounded**: every template cell is splatted
//!   bilinearly at its fractional position, the limbs are anti-aliased polylines, and the
//!   anchor is whatever fractional [`SurfacePoint`] the caller passes, so motion reads as
//!   fractional brightness at 60 fps the way the world's bodies already do. The one binary
//!   quantity in the study, the leg lift (`u < 0.38 ? 1 : 0`), becomes the continuous bump
//!   `sin(π · u / 0.38)` for `u < 0.38`, 0 otherwise.
//! * **The study's colours, decoded once; alpha where the study meant translucency.** Every
//!   colour mix is computed in sRGB exactly as `fable.js` computes it (including its mixes
//!   toward the `#0B0525` background for the opaque structure — rim, plates, seams, head
//!   plates, eye, jaw, lantern sockets, legs, near limb, claw — which Astra asked to keep
//!   as deliberately dark opaque shell), then decoded with [`cubarium_render::srgb_decode`]
//!   into premultiplied linear. Only the tail fan, the lantern halo and glow, the cocoon
//!   and the far limb carry real alpha (the study's mix fraction over the pure colour), so
//!   those compose over the cube's real ground instead of painting a background-coloured
//!   block. Over bright water the dark carapace therefore reads as a dark silhouette; see
//!   `captures/lanternjaw/grounds.png`.
//! * **Parts through one query.** The body is eight bounded sprites (see [`PartName`]) each
//!   rasterized fresh for the frame in body coordinates and drawn by
//!   [`cubarium_render::stamp_rig`] through **one** root-owned pixel query: the hull's four
//!   pieces are one material (one layer, summed — they are cut from one lattice, so the sum
//!   is the uncut hull), and the far limb, the underside, the glow and the near limb are
//!   separate depths composited in that order. No part is stamped from its own anchor.
//!
//! # Footprint
//!
//! **Normative.** In every mode at every time, every painted texel centre of every part, in
//! body-local pixels from the anchor (body `+x` forward, `+y` down when facing image-right),
//! lies within `x ∈ [−BOUND_BACK, BOUND_FRONT]`, `y ∈ [−BOUND_ABOVE, BOUND_BELOW]`; every
//! part's sprite, from its own pivot, has an extent of at most [`PART_EXTENT_MAX`]; and the
//! rig's query radius ([`cubarium_render::rig_radius`]) is at most [`QUERY_RADIUS_MAX`]. A
//! part that could not be built within its budget is a bug, not a skipped frame:
//! [`Lanternjaw::parts`] always returns exactly the eight parts of [`PartName::ALL`].
//!
//! The hull pieces, the underside and the glow share the **body lattice**: their `offset`s
//! and their sprites' pivots have integer coordinates, so every texel centre of those parts
//! sits at a half-integer body coordinate exactly as a template cell does. This is what makes
//! the hull's four pieces sum to the uncut hull under bilinear sampling.

use std::f64::consts::{PI, TAU};

use cubarium_render::{Canvas, RigPart, Sprite, srgb_decode, stamp_rig};
use cubarium_surface::{PixelImage, SurfacePoint, Vec2};

/// The four presentation modes of the study. Names describe choreography, not ecology: a
/// `Hunt` loop is one articulated strike in a six-second cycle and says nothing about prey,
/// a `Bud` shows a cocoon under the tail and says nothing about gestation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Rest,
    Move,
    Hunt,
    Bud,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Rest, Mode::Move, Mode::Hunt, Mode::Bud];
}

/// The parts of the rig in **painting order** (back to front), with their depth layer:
///
/// | part | layer | content |
/// | --- | --- | --- |
/// | `FarLimb` | 0 | the far raptorial forelimb, 45 ms behind, one pixel higher, 55 % alpha |
/// | `Underside` | 1 | the three walking-leg pairs and, in `Bud`, the cocoon |
/// | `Tail` | 2 | hull template columns 0..=3 (`dx` −9..=−6): the fan and the first rim |
/// | `Abdomen` | 2 | columns 4..=8 (`dx` −5..=−1): three lanterns and their seams |
/// | `Thorax` | 2 | columns 9..=12 (`dx` 0..=3): two lanterns and their seams |
/// | `Head` | 2 | columns 13..=17 (`dx` 4..=8): brow lamp, crown, plates, eye, jaw |
/// | `Glow` | 3 | the lantern halos (one row up) and glows (two rows up) of every lantern |
/// | `NearLimb` | 4 | the near forelimb, its claw in the claw colour |
///
/// A hull template cell is rasterized into the piece its **column** belongs to whatever its
/// displacement (the pieces have padding), so a coil that compresses the hull is still one
/// summed material. Each hull piece's `offset` is the integer body-local point `(left column's
/// dx + 2, 0)` (for the five-column pieces; `(−7, 0)` for the tail) and its pivot is at
/// integer sprite coordinates, so all four pieces share the body lattice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PartName {
    FarLimb,
    Underside,
    Tail,
    Abdomen,
    Thorax,
    Head,
    Glow,
    NearLimb,
}

impl PartName {
    /// Painting order.
    pub const ALL: [PartName; 8] = [
        PartName::FarLimb,
        PartName::Underside,
        PartName::Tail,
        PartName::Abdomen,
        PartName::Thorax,
        PartName::Head,
        PartName::Glow,
        PartName::NearLimb,
    ];

    /// The depth layer of [`cubarium_render::RigPart::layer`], per the table above.
    pub fn layer(self) -> u8 {
        match self {
            PartName::FarLimb => 0,
            PartName::Underside => 1,
            PartName::Tail | PartName::Abdomen | PartName::Thorax | PartName::Head => 2,
            PartName::Glow => 3,
            PartName::NearLimb => 4,
        }
    }
}

/// One rasterized part for one frame: the sprite and the body-local position of its pivot
/// (see [`cubarium_render::RigPart`] for the body frame).
#[derive(Clone, Debug)]
pub struct Part {
    pub name: PartName,
    pub sprite: Sprite,
    pub offset: Vec2,
}

impl Part {
    /// This part as the renderer sees it, at its name's layer.
    pub fn rig_part(&self) -> cubarium_render::RigPart<'_> {
        cubarium_render::RigPart { sprite: &self.sprite, offset: self.offset, layer: self.name.layer() }
    }
}

/// The hull template, one character per cube pixel, exactly as `fable.js` has it: row 0 is
/// `dy = −4`, column 0 is `dx = −9`; `o` rim, `c` plate, `d` seam, `v` head plate, `f` fan,
/// `e` eye, `j` jaw, `L` lantern (the brow lamp is the `L` at column 13).
pub const ROWS: [&str; 8] = [
    "                  ",
    "             oooo ",
    "f   oooooooooLvvvo",
    "ff oLdLdLdLdLccecj",
    "fffocdcdcdcdccccjj",
    "ff  ooooooooooddj ",
    "f                 ",
    "                  ",
];
/// `ROWS[0]` sits at this body-local `y`.
pub const ROW_TOP: f64 = -4.0;
/// Column 0 sits at this body-local `x`.
pub const COL_LEFT: f64 = -9.0;
/// Template columns.
pub const COL_COUNT: usize = 18;

/// The hunt cycle, seconds; every sub-rhythm divides it, so `Hunt` at `t` and `t + 6` are
/// the same picture.
pub const HUNT_PERIOD: f64 = 6.0;
/// Coil begins.
pub const T_COIL: f64 = 3.1;
/// Forelimbs release.
pub const T_SNAP: f64 = 3.22;
/// Full extension.
pub const T_OPEN: f64 = 3.34;
/// Folded again: the articulated strike lasts `T_END − T_COIL` = 0.44 s.
pub const T_END: f64 = 3.54;
/// A blink, closed and open again, eased at both ends.
pub const BLINK_SECONDS: f64 = 0.28;
/// The strike accent on jaw and claw.
pub const ACCENT_SECONDS: f64 = 0.24;
/// Blink periods per mode (rest, move, bud); the hunt blinks once after its recoil.
pub const BLINK_PERIOD_REST: f64 = 4.7;
pub const BLINK_PERIOD_MOVE: f64 = 5.9;
pub const BLINK_PERIOD_BUD: f64 = 5.3;

/// Body-local footprint bound behind the anchor (the fan at `dx = −9` plus half a pixel).
pub const BOUND_BACK: f64 = 10.0;
/// Ahead of the anchor: the claw at 12.3 plus the head's lunge.
pub const BOUND_FRONT: f64 = 13.5;
/// Above the mid-line: the lantern glow two rows over a plate lifted by the wave.
pub const BOUND_ABOVE: f64 = 5.5;
/// Below: a leg's lower pixel on a plate pushed down by the wave.
pub const BOUND_BELOW: f64 = 5.5;
/// The largest extent any part may have from its own pivot, in sprite pixels.
pub const PART_EXTENT_MAX: f64 = 8.0;
/// The largest query radius the rig may need ([`cubarium_render::rig_radius`]): the claw at
/// `BOUND_FRONT` plus its part's support and the rig margin, rounded up.
pub const QUERY_RADIUS_MAX: f64 = 16.0;

/// The raised-cosine accent envelope of the study: a cosine ramp up over the first 40 % of
/// `u ∈ (0, 1)`, a plateau to 60 %, a cosine ramp down; 0 at and outside both ends, with
/// zero slope there. **Normative** — the blink, the strike accent and nothing else ride it.
pub fn envelope(u: f64) -> f64 {
    if !(u > 0.0 && u < 1.0) {
        return 0.0;
    }
    if u < 0.4 {
        0.5 - 0.5 * (std::f64::consts::PI * u / 0.4).cos()
    } else if u < 0.6 {
        1.0
    } else {
        0.5 - 0.5 * (std::f64::consts::PI * (1.0 - u) / 0.4).cos()
    }
}

/// Everything the hunt cycle drives at cycle phase `t_h ∈ [0, HUNT_PERIOD)`, exactly
/// `huntState` of the study: `reach` (< 0 cocked, 0 folded, 1 extended), `compress`,
/// `lunge`, `charge`, `accent` and `blink`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HuntState {
    pub reach: f64,
    pub compress: f64,
    pub lunge: f64,
    pub charge: f64,
    pub accent: f64,
    pub blink: f64,
}

/// **Normative**: the piecewise schedule of `fable.js` `huntState`, unrounded. Coil
/// (`T_COIL..T_SNAP`): `u = smoothstep`, `reach = −0.35u`, `compress = 1.7u`, `charge = u`.
/// Snap (`T_SNAP..T_OPEN`): `e = 1 − (1 − u)³`, `reach = −0.35 + 1.35e`, `compress = 1.7 −
/// 2.0e`, `lunge = 1.1e`. Recoil (`T_OPEN..T_END`): `u = smoothstep`, `reach = 1 − u`,
/// `compress = −0.3 + 0.3u`, `lunge = 1.1(1 − u)`. From `T_SNAP` on, `charge = exp(−(t_h −
/// T_SNAP) / 0.9) · smoothstep((HUNT_PERIOD − t_h) / 0.6)`. `accent = envelope((t_h −
/// (T_SNAP − 0.02)) / ACCENT_SECONDS)`, `blink = envelope((t_h − (T_END + 0.3)) /
/// BLINK_SECONDS)`. A non-finite `t_h` is the default (all zero).
pub fn hunt_state(t_h: f64) -> HuntState {
    if !t_h.is_finite() {
        return HuntState::default();
    }
    let mut s = HuntState::default();
    if t_h >= T_COIL && t_h < T_SNAP {
        let u = smoothstep((t_h - T_COIL) / (T_SNAP - T_COIL));
        s.reach = -0.35 * u;
        s.compress = 1.7 * u;
        s.charge = u;
    } else if t_h >= T_SNAP && t_h < T_OPEN {
        let u = (t_h - T_SNAP) / (T_OPEN - T_SNAP);
        // Fast release, soft arrival.
        let e = 1.0 - (1.0 - u) * (1.0 - u) * (1.0 - u);
        s.reach = -0.35 + 1.35 * e;
        s.compress = 1.7 - 2.0 * e;
        s.lunge = 1.1 * e;
    } else if t_h >= T_OPEN && t_h < T_END {
        let u = smoothstep((t_h - T_OPEN) / (T_END - T_OPEN));
        s.reach = 1.0 - u;
        s.compress = -0.3 + 0.3 * u;
        s.lunge = 1.1 * (1.0 - u);
    }
    if t_h >= T_SNAP {
        // The charge bleeds out of the chain and is forced to zero before the cycle wraps,
        // so the loop has no step.
        s.charge = (-(t_h - T_SNAP) / 0.9).exp() * smoothstep((HUNT_PERIOD - t_h) / 0.6);
    }
    s.accent = envelope((t_h - (T_SNAP - 0.02)) / ACCENT_SECONDS);
    s.blink = envelope((t_h - (T_END + 0.3)) / BLINK_SECONDS);
    s
}

/// The study's `smoothstep`: the Hermite polynomial on a clamped argument.
fn smoothstep(u: f64) -> f64 {
    let c = if u.is_nan() { 0.0 } else { u.clamp(0.0, 1.0) };
    c * c * (3.0 - 2.0 * c)
}

/// The study's `frac`.
fn frac(v: f64) -> f64 {
    v - v.floor()
}

/// The study's `mix` on sRGB triples: a clamped linear interpolation.
fn mix3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let u = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    [
        a[0] + (b[0] - a[0]) * u,
        a[1] + (b[1] - a[1]) * u,
        a[2] + (b[2] - a[2]) * u,
    ]
}

/// Seconds since the most recent blink start, folded into [`envelope`].
fn blink_closure(t: f64, period: f64) -> f64 {
    envelope((frac(t / period) * period) / BLINK_SECONDS)
}

// ---------------------------------------------------------------------------------------
// Palette. The study's own sRGB values, mixed the way the study mixes them (`css` rounds to
// eight bits, so a decoded colour here is exactly the colour the browser showed), decoded
// once into linear light. Only the pixels the study faked with a mix toward its background
// and that read as translucent carry real alpha (see `Lanternjaw::parts`).
// ---------------------------------------------------------------------------------------

/// Premultiplied linear RGBA, the form [`Sprite::from_premultiplied`] takes.
type Rgba = [f32; 4];

/// `#0B0525`, the study background: used only as a mix endpoint, never painted.
const BG: [f64; 3] = [11.0, 5.0, 37.0];
const INDIGO: [f64; 3] = [30.0, 39.0, 152.0];
const VIOLET: [f64; 3] = [81.0, 11.0, 109.0];
const PINK: [f64; 3] = [255.0, 42.0, 252.0];
const CYAN: [f64; 3] = [66.0, 198.0, 255.0];
const CYAN_S: [f64; 3] = [66.0, 197.0, 248.0];
/// Brief flash only: it tints the pink at the peak of the strike envelope.
const ORANGE: [f64; 3] = [255.0, 155.0, 80.0];

/// An opaque study colour: rounded to eight bits exactly as the study's `css` does, then
/// decoded into linear light. Premultiplied by an alpha of 1 is the colour itself.
fn opaque(c: [f64; 3]) -> Rgba {
    let d = decode(c);
    [d[0], d[1], d[2], 1.0]
}

/// A study colour decoded into linear light, without alpha.
fn decode(c: [f64; 3]) -> [f32; 3] {
    let byte = |v: f64| srgb_decode(v.round().clamp(0.0, 255.0) as u8);
    [byte(c[0]), byte(c[1]), byte(c[2])]
}

/// A linear colour at coverage `a`, premultiplied.
fn with_alpha(c: [f32; 3], a: f64) -> Rgba {
    let a = (a.clamp(0.0, 1.0)) as f32;
    [c[0] * a, c[1] * a, c[2] * a, a]
}

/// A premultiplied lerp, for the cocoon's core (a translucent cyan mixed toward opaque pink).
fn lerp4(a: Rgba, b: Rgba, t: f64) -> Rgba {
    let u = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) } as f32;
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * u)
}

/// The study's static shades, decoded once.
#[derive(Clone, Debug)]
struct Palette {
    /// Carapace rim and belly line (`o`).
    edge: Rgba,
    /// Carapace plate (`c`).
    shell: Rgba,
    /// Segment division (`d`).
    seam: Rgba,
    /// Head plates (`v`).
    plate: Rgba,
    /// The trailing tail fan, translucent: `CYAN_S` at 0.40, its tip at 0.22.
    fan: Rgba,
    fan_tip: Rgba,
    /// Walking legs.
    leg: Rgba,
    /// Linear `CYAN`, for the halo and glow alphas and the cocoon rim.
    cyan: [f32; 3],
    /// Linear `CYAN_S`, for the cocoon shell.
    cyan_s: [f32; 3],
    /// Linear `PINK`, for the cocoon core.
    pink: [f32; 3],
}

impl Palette {
    fn decode_once() -> Palette {
        Palette {
            edge: opaque(mix3(BG, INDIGO, 0.72)),
            shell: opaque(VIOLET),
            seam: opaque(mix3(BG, VIOLET, 0.45)),
            plate: opaque(mix3(VIOLET, PINK, 0.20)),
            fan: with_alpha(decode(CYAN_S), 0.40),
            fan_tip: with_alpha(decode(CYAN_S), 0.22),
            leg: opaque(mix3(BG, INDIGO, 0.82)),
            cyan: decode(CYAN),
            cyan_s: decode(CYAN_S),
            pink: decode(PINK),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Poses, in body-local pixels, exactly as `fable.js` has them.
// ---------------------------------------------------------------------------------------

const LIMB_SHOULDER: [f64; 2] = [4.0, 1.0];
const LIMB_FOLD: ([f64; 2], [f64; 2]) = ([6.4, 2.2], [4.3, 2.5]);
const LIMB_COCK: ([f64; 2], [f64; 2]) = ([5.8, 1.7], [3.6, 0.7]);
const LIMB_STRIKE: ([f64; 2], [f64; 2]) = ([8.4, 1.9], [12.3, 0.6]);
/// Walking legs, body-local `x`.
const LEG_BASE: [f64; 3] = [-1.0, 1.0, 3.0];

/// Elbow and claw at `reach`: below 0 cocked back, 0 folded, 1 fully extended.
fn limb_pose(reach: f64) -> ([f64; 2], [f64; 2]) {
    let (from, to) = if reach < 0.0 {
        (LIMB_COCK, LIMB_FOLD)
    } else {
        (LIMB_FOLD, LIMB_STRIKE)
    };
    let u = if reach < 0.0 { 1.0 + reach / 0.35 } else { reach };
    let lerp = |a: [f64; 2], b: [f64; 2]| [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u];
    (lerp(from.0, to.0), lerp(from.1, to.1))
}

// ---------------------------------------------------------------------------------------
// The body lattice.
// ---------------------------------------------------------------------------------------

/// A study coordinate names a *pixel*, not a point: `fable.js` paints `fillRect(ax + sx,
/// ay + sy, 1, 1)`, so the painted pixel's centre lies half a pixel further along each axis
/// than the coordinate it is named by. Every position in this module is therefore a study
/// coordinate and every splat lands at a **half-integer** body coordinate — the body lattice
/// that the integer part offsets and integer sprite pivots share (see the module doc), which
/// is what lets the hull's four pieces sum to the uncut hull.
const CELL_CENTRE: f64 = 0.5;

/// One part's image in body coordinates: the texel at image index `(ix, iy)` has its centre
/// at body `(x0 + ix + 0.5, y0 + iy + 0.5)`, and the part's pivot — its `offset` — is the
/// integer sprite coordinate `(offset.x − x0, offset.y − y0)`.
#[derive(Clone, Copy, Debug)]
struct Rect {
    x0: f64,
    y0: f64,
    w: usize,
    h: usize,
    offset: Vec2,
}

impl Rect {
    const fn new(x0: f64, y0: f64, w: usize, h: usize, ox: f64, oy: f64) -> Rect {
        Rect { x0, y0, w, h, offset: Vec2::new(ox, oy) }
    }

    /// The pivot in sprite pixels: integer by construction, because `x0`, `y0` and the
    /// offset all are.
    fn pivot(&self) -> Vec2 {
        Vec2::new(self.offset.x - self.x0, self.offset.y - self.y0)
    }
}

/// Every part's image and body-local pivot, in [`PartName::ALL`] order. Each rectangle has
/// at least one texel of margin around everything the part can paint in any mode at any
/// time, so a bilinear splat never falls off its own image and loses light; the margin is
/// transparent, so it costs neither extent nor query radius.
const PART_RECTS: [Rect; 8] = [
    // FarLimb: the same image as the near limb, one pixel higher in content.
    Rect::new(1.0, -3.0, 15, 9, 8.0, 1.0),
    // Underside: three leg pairs and, in `Bud`, the cocoon under the tail.
    Rect::new(-8.0, -1.0, 16, 8, 0.0, 3.0),
    // Tail: template columns 0..=3.
    Rect::new(-12.0, -6.0, 10, 13, -7.0, 0.0),
    // Abdomen: columns 4..=8.
    Rect::new(-8.0, -5.0, 11, 10, -3.0, 0.0),
    // Thorax: columns 9..=12.
    Rect::new(-3.0, -5.0, 10, 10, 2.0, 0.0),
    // Head: columns 13..=17.
    Rect::new(1.0, -6.0, 12, 11, 6.0, 0.0),
    // Glow: every lantern's halo and glow.
    Rect::new(-8.0, -7.0, 16, 9, 0.0, -3.0),
    // NearLimb.
    Rect::new(1.0, -3.0, 15, 9, 8.0, 1.0),
];

/// Index into [`PartName::ALL`] / [`PART_RECTS`].
const FAR_LIMB: usize = 0;
const UNDERSIDE: usize = 1;
const TAIL: usize = 2;
const ABDOMEN: usize = 3;
const THORAX: usize = 4;
const HEAD: usize = 5;
const GLOW: usize = 6;
const NEAR_LIMB: usize = 7;

/// The hull piece a template column belongs to, whatever its displacement.
fn hull_piece(col: usize) -> usize {
    match col {
        0..=3 => TAIL,
        4..=8 => ABDOMEN,
        9..=12 => THORAX,
        _ => HEAD,
    }
}

/// Add `colour · weight` into `buf` at study coordinate `(x, y)`, split bilinearly over the
/// four texels whose centres surround it: a resample, so two half-covered rows of one column
/// sum to the whole colour.
fn splat_weighted(buf: &mut [Rgba], rect: &Rect, x: f64, y: f64, colour: Rgba, weight: f64) {
    if !(weight > 0.0) || !x.is_finite() || !y.is_finite() {
        return;
    }
    // Texel-index coordinates: an integer value sits exactly on a texel centre.
    let px = x + CELL_CENTRE - rect.x0 - 0.5;
    let py = y + CELL_CENTRE - rect.y0 - 0.5;
    let ix = px.floor();
    let iy = py.floor();
    let fx = px - ix;
    let fy = py - iy;
    debug_assert!(
        ix >= 0.0 && iy >= 0.0 && ix + 1.0 < rect.w as f64 && iy + 1.0 < rect.h as f64,
        "splat at study ({x}, {y}) falls outside its part's image {rect:?}"
    );
    let ix = ix as isize;
    let iy = iy as isize;
    for (ox, oy, w) in [
        (0, 0, (1.0 - fx) * (1.0 - fy)),
        (1, 0, fx * (1.0 - fy)),
        (0, 1, (1.0 - fx) * fy),
        (1, 1, fx * fy),
    ] {
        let w = (w * weight) as f32;
        if w <= 0.0 {
            continue;
        }
        let (tx, ty) = (ix + ox, iy + oy);
        if tx < 0 || ty < 0 || tx >= rect.w as isize || ty >= rect.h as isize {
            continue;
        }
        let texel = &mut buf[ty as usize * rect.w + tx as usize];
        for c in 0..4 {
            texel[c] += colour[c] * w;
        }
    }
}

/// One whole cell of colour at a study coordinate.
fn splat(buf: &mut [Rgba], rect: &Rect, x: f64, y: f64, colour: Rgba) {
    splat_weighted(buf, rect, x, y, colour, 1.0);
}

/// An anti-aliased one-pixel-wide segment: the mass of a unit-wide bar of this length,
/// sampled finely along the segment and splatted, so the total light is the segment's own
/// area and a diagonal is as bright as an axis-aligned one. `extend_start` lengthens the
/// segment half a pixel backwards so its first pixel is whole, the way the study's `line`
/// painted both endpoints; the far end is left short because the claw texel is painted over
/// it and because the strike's reach is the body's frontmost bound.
fn line(
    buf: &mut [Rgba],
    rect: &Rect,
    a: [f64; 2],
    b: [f64; 2],
    colour: Rgba,
    extend_start: bool,
) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy);
    if !(length > 0.0) {
        splat(buf, rect, a[0], a[1], colour);
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let start = if extend_start { -0.5 } else { 0.0 };
    let span = length - start;
    let steps = (span / 0.1).ceil().max(1.0);
    let mass = span / steps;
    let mut i = 0.0;
    while i < steps {
        let s = start + span * (i + 0.5) / steps;
        splat_weighted(buf, rect, a[0] + ux * s, a[1] + uy * s, colour, mass);
        i += 1.0;
    }
}

/// Coverage never exceeds one and light never exceeds coverage.
fn clamp_buffer(buf: &mut [Rgba]) {
    for texel in buf.iter_mut() {
        let a = texel[3].clamp(0.0, 1.0);
        texel[3] = a;
        for c in 0..3 {
            texel[c] = texel[c].clamp(0.0, a);
        }
    }
}

/// `front` source-over `back`, in place, both premultiplied.
fn composite(back: &mut [Rgba], front: &[Rgba]) {
    for (b, f) in back.iter_mut().zip(front) {
        if f[3] <= 0.0 {
            continue;
        }
        let k = 1.0 - f[3];
        for c in 0..4 {
            b[c] = f[c] + b[c] * k;
        }
    }
}

/// One template cell, parsed once.
#[derive(Clone, Copy, Debug)]
struct Cell {
    col: usize,
    /// The study's `y` for this row (`ROW_TOP + row`).
    y: f64,
    ch: u8,
}

/// The Lanternjaw rig: the template parsed once, the palette decoded once.
#[derive(Clone, Debug)]
pub struct Lanternjaw {
    cells: Vec<Cell>,
    palette: Palette,
}

impl Default for Lanternjaw {
    fn default() -> Self {
        Lanternjaw::new()
    }
}

impl Lanternjaw {
    pub fn new() -> Lanternjaw {
        let mut cells = Vec::with_capacity(80);
        for (row, text) in ROWS.iter().enumerate() {
            for (col, ch) in text.bytes().enumerate().take(COL_COUNT) {
                if ch != b' ' {
                    cells.push(Cell { col, y: ROW_TOP + row as f64, ch });
                }
            }
        }
        Lanternjaw { cells, palette: Palette::decode_once() }
    }

    /// The eight parts of the body at presentation `seconds` in `mode`, in painting order, into
    /// `out` (cleared first, capacity reused).
    ///
    /// **Normative.** A pure function of `(seconds, mode)`: the same arguments give the same
    /// sprites texel for texel, whatever was asked before, so a repeated draw is the same
    /// image, pausing holds the pose, and nothing here restarts on a frame. A non-finite
    /// `seconds` reads as 0. **Colours** are computed exactly as `fable.js` computes them, in
    /// sRGB (every `mix` of the study, including the time-varying lantern, blink, accent and
    /// limb mixes, and the static mixes toward its `#0B0525` background for the *opaque*
    /// structure: rim `o`, plates `c`, seams `d`, head plates `v`, eye, jaw, the lantern
    /// sockets, the legs, the near limb and the claw), then decoded once with
    /// [`cubarium_render::srgb_decode`] into premultiplied linear. The **translucent** pixels
    /// carry real alpha instead of a mix toward the background: the tail fan (`CYAN_S` at
    /// alpha 0.40, its tip 0.22), the lantern halo (`CYAN` at `0.5 · smoothstep((b − 0.35) /
    /// 0.45)`) and glow (`CYAN` at `0.22 · smoothstep((b − 0.72) / 0.28)`), the cocoon (its
    /// shell/rim/core mix fractions as alpha over their pure colours), and the far limb (its
    /// colour at alpha 0.55). The motion parameters per mode are the study's (`fable.md`
    /// "Footprint and timing"): rest — wave ±0.55 px on 5.5 s tapered to the tail, pulse
    /// 3.0 s, blink every 4.7 s; move — wave ±1.15 px on 2.4 s, gait 1.2 s with three leg
    /// pairs at 0.34 phase steps, pulse 2.3 s, blink every 5.9 s; hunt — [`hunt_state`] of
    /// `seconds mod HUNT_PERIOD`, wave ±0.22 on 6 s, pulse 3.0 s at gain `0.38 + 0.92 ·
    /// charge`; bud — wave ±0.4 on 6.5 s, pulse 3.6 s at gain 0.8, tail lifted 1 px, cocoon
    /// breathing on 2.6 s, blink every 5.3 s. Per template column `i` (`rel = i / 17`,
    /// `taper = 0.25 + 0.75(1 − rel)`): `dy_i = amp · taper · sin(2π t / period + 0.4 i)` (minus
    /// the tail lift for `i < 3`) and `dx_i = compress(1 − rel) + lunge · clamp((i − 9) / 8, 0,
    /// 1)`, **both unrounded**. The lantern of column `i` is `b = clamp(smoothstep(1 − d /
    /// 0.26) · gain, 0, 1.25)` with `u = fract(t / pulse + (17 − i) · 0.06)`, `d = min(u, 1 −
    /// u)`; its colour is `LANTERN_DIM → CYAN` by `b`; a halo one row up is `CYAN` at alpha
    /// `0.5 · smoothstep((b − 0.35) / 0.45)` over whatever the hull painted there, and a glow
    /// two rows up is `CYAN` at alpha `0.22 · smoothstep((b − 0.72) / 0.28)`. The far limb
    /// reads the hunt state 45 ms earlier and rides one pixel higher, at 55 % alpha. Every
    /// other colour, offset and pose is the study's, read from `fable.js`.
    ///
    /// **Rasterization.** Each part is a fresh [`Sprite::from_premultiplied`] whose pivot is
    /// the part's body-local pivot (`Part::offset`) expressed in its own image; a template
    /// cell at fractional body position `(x, y)` is split bilinearly over the four texels
    /// whose centres surround it and **added** into the part's premultiplied buffer (a
    /// resample, so two half-covered rows of one column sum to the whole colour), and the
    /// buffer's alpha is clamped to 1 with the colour clamped to the alpha. Hull cells go to
    /// the hull piece of their column; legs and the cocoon to `Underside`; halos and glows to
    /// `Glow`; the limbs are anti-aliased one-pixel-wide polylines shoulder → elbow → claw
    /// (sample each segment at sub-pixel steps and splat, or an equivalent exact coverage)
    /// with the claw texel in the claw colour. Depth between parts is the renderer's business
    /// ([`PartName::layer`]); [`BOUND_BACK`], [`BOUND_FRONT`], [`BOUND_ABOVE`], [`BOUND_BELOW`],
    /// [`PART_EXTENT_MAX`] and [`QUERY_RADIUS_MAX`] hold for every frame.
    pub fn parts(&self, seconds: f64, mode: Mode, out: &mut Vec<Part>) {
        out.clear();
        let t = if seconds.is_finite() { seconds } else { 0.0 };
        let p = &self.palette;

        // --- motion parameters, exactly the study's `draw` --------------------------
        let mut wave_amp = 0.55;
        let mut wave_period = 5.5;
        let mut pulse_period = 3.0;
        let mut pulse_gain = 1.0;
        // 0 = legs planted.
        let mut gait = 0.0;
        let mut compress = 0.0;
        let mut lunge = 0.0;
        let mut reach = 0.0;
        let mut tail_lift = 0.0;
        let mut closure = blink_closure(t, BLINK_PERIOD_REST);
        let mut accent = 0.0;
        let mut bud = false;
        match mode {
            Mode::Rest => {}
            Mode::Move => {
                wave_amp = 1.15;
                wave_period = 2.4;
                pulse_period = 2.3;
                gait = 1.2;
                closure = blink_closure(t, BLINK_PERIOD_MOVE);
            }
            Mode::Hunt => {
                let h = hunt_state(t.rem_euclid(HUNT_PERIOD));
                wave_amp = 0.22;
                wave_period = HUNT_PERIOD;
                pulse_period = 3.0;
                pulse_gain = 0.38 + 0.92 * h.charge;
                compress = h.compress;
                lunge = h.lunge;
                reach = h.reach;
                accent = h.accent;
                closure = h.blink;
            }
            Mode::Bud => {
                wave_amp = 0.4;
                wave_period = 6.5;
                pulse_period = 3.6;
                pulse_gain = 0.8;
                tail_lift = 1.0;
                bud = true;
                closure = blink_closure(t, BLINK_PERIOD_BUD);
            }
        }

        // --- this frame's colours, interpolated by the study's envelopes ------------
        let lit = reach.clamp(0.0, 1.0);
        let jaw_hot = mix3(PINK, ORANGE, 0.45);
        let jaw = opaque(mix3(mix3(VIOLET, PINK, 0.26), jaw_hot, accent));
        let claw = opaque(mix3(mix3(VIOLET, PINK, 0.32), jaw_hot, accent));
        // The raptorial limbs are a colder violet-blue than the head and dark while folded:
        // the resting silhouette is one clean hull and the strike is the only moment the arms
        // carry light. Both limbs are the same continuous function of the reach envelope.
        let limb_dark = mix3(BG, INDIGO, 0.80);
        let limb_lit = mix3(INDIGO, PINK, 0.45);
        let limb = opaque(mix3(limb_dark, limb_lit, lit));
        let eye = opaque(mix3(PINK, mix3(BG, VIOLET, 0.55), closure));
        let lantern_dim = mix3(BG, CYAN, 0.44);
        // The far limb lags 45 ms of simulated time and rides a pixel higher, at 55 % alpha.
        let far_reach = if mode == Mode::Hunt {
            hunt_state((t - 0.045).rem_euclid(HUNT_PERIOD)).reach
        } else {
            reach
        };
        let far_limb = with_alpha(
            decode(mix3(limb_dark, limb_lit, far_reach.clamp(0.0, 1.0))),
            0.55,
        );

        // --- per-column body offsets: a travelling sinuous wave, looser at the tail,
        //     plus the coil (shorten) and the head lunge. Both unrounded. ------------
        let mut col_dx = [0.0f64; COL_COUNT];
        let mut col_dy = [0.0f64; COL_COUNT];
        for i in 0..COL_COUNT {
            let rel = i as f64 / (COL_COUNT - 1) as f64;
            let taper = 0.25 + 0.75 * (1.0 - rel);
            let mut dy = wave_amp * taper * (TAU * t / wave_period + i as f64 * 0.4).sin();
            if i < 3 {
                dy -= tail_lift;
            }
            col_dy[i] = dy;
            col_dx[i] = compress * (1.0 - rel) + lunge * ((i as f64 - 9.0) / 8.0).clamp(0.0, 1.0);
        }

        // Two buffers per part: the study's own painting order inside one part is an
        // additive material (`back`) with at most one source-over group over it (`front`) —
        // the claw over its limb, the cocoon over the legs.
        let mut back: Vec<Vec<Rgba>> =
            PART_RECTS.iter().map(|r| vec![[0.0f32; 4]; r.w * r.h]).collect();
        let mut front: Vec<Vec<Rgba>> =
            PART_RECTS.iter().map(|r| vec![[0.0f32; 4]; r.w * r.h]).collect();

        // --- forelimbs -------------------------------------------------------------
        let head_dx = col_dx[13];
        let head_dy = col_dy[13];
        for (idx, reach, dy, body, tip) in [
            (FAR_LIMB, far_reach, head_dy - 1.0, far_limb, far_limb),
            (NEAR_LIMB, reach, head_dy, limb, claw),
        ] {
            let rect = &PART_RECTS[idx];
            let (elbow, claw_at) = limb_pose(reach);
            let shoulder = [LIMB_SHOULDER[0] + head_dx, LIMB_SHOULDER[1] + dy];
            let elbow = [elbow[0] + head_dx, elbow[1] + dy];
            let claw_at = [claw_at[0] + head_dx, claw_at[1] + dy];
            line(&mut back[idx], rect, shoulder, elbow, body, true);
            line(&mut back[idx], rect, elbow, claw_at, body, true);
            splat(&mut front[idx], rect, claw_at[0], claw_at[1], tip);
        }

        // --- legs: one visible pair per segment, on their own body columns ----------
        let rect = &PART_RECTS[UNDERSIDE];
        for (k, base) in LEG_BASE.iter().enumerate() {
            let ci = (base - COL_LEFT) as usize;
            let (dx, dy) = (col_dx[ci], col_dy[ci]);
            let (swing, lift) = if gait > 0.0 {
                let u = frac(t / gait + k as f64 * 0.34);
                // The study's binary lift is a continuous bump here.
                let lift = if u < 0.38 { (PI * u / 0.38).sin() } else { 0.0 };
                (1.3 * (TAU * u).sin(), lift)
            } else {
                (0.0, 0.0)
            };
            splat(&mut back[UNDERSIDE], rect, base + dx, 2.0 + dy, p.leg);
            splat(&mut back[UNDERSIDE], rect, base + dx + swing, 3.0 + dy - lift, p.leg);
        }

        // --- cocoon (bud), over the legs as the study paints it ---------------------
        if bud {
            let breath = 0.5 + 0.5 * (TAU * t / 2.6).sin();
            let shell = with_alpha(p.cyan_s, 0.34 + 0.16 * breath);
            let rim = with_alpha(p.cyan, 0.5 + 0.24 * breath);
            let core = lerp4(
                with_alpha(p.cyan, 0.62),
                [p.pink[0], p.pink[1], p.pink[2], 1.0],
                0.18 + 0.3 * breath,
            );
            let dy = col_dy[4];
            for (x, y, colour) in [
                (-5.0, 2.0, shell),
                (-4.0, 2.0, rim),
                (-3.0, 2.0, shell),
                (-5.0, 3.0, shell),
                (-4.0, 3.0, core),
                (-3.0, 3.0, shell),
            ] {
                splat(&mut front[UNDERSIDE], rect, x, y + dy, colour);
            }
        }

        // --- hull, and the lantern chain's bloom -------------------------------------
        // One crest travelling head to tail.
        let lantern = |i: usize| {
            let u = frac(t / pulse_period + (COL_COUNT - 1 - i) as f64 * 0.06);
            let d = u.min(1.0 - u);
            (smoothstep(1.0 - d / 0.26) * pulse_gain).clamp(0.0, 1.25)
        };
        for cell in &self.cells {
            let i = cell.col;
            let sx = COL_LEFT + i as f64 + col_dx[i];
            let sy = cell.y + col_dy[i];
            let piece = hull_piece(i);
            let rect = &PART_RECTS[piece];
            let colour = match cell.ch {
                b'o' => p.edge,
                b'c' => p.shell,
                b'd' => p.seam,
                b'v' => p.plate,
                b'f' => {
                    if i == 0 {
                        p.fan_tip
                    } else {
                        p.fan
                    }
                }
                b'e' => eye,
                b'j' => jaw,
                b'L' => {
                    let b = lantern(i);
                    let glow_rect = &PART_RECTS[GLOW];
                    // The bloom fades up out of whatever lies under it, so no halo pixel
                    // ever switches on.
                    let halo = smoothstep((b - 0.35) / 0.45);
                    if halo > 0.0 {
                        let c = with_alpha(p.cyan, 0.5 * halo);
                        splat(&mut back[GLOW], glow_rect, sx, sy - 1.0, c);
                    }
                    let glow = smoothstep((b - 0.72) / 0.28);
                    if glow > 0.0 {
                        let c = with_alpha(p.cyan, 0.22 * glow);
                        splat(&mut back[GLOW], glow_rect, sx, sy - 2.0, c);
                    }
                    opaque(mix3(lantern_dim, CYAN, b))
                }
                _ => continue,
            };
            splat(&mut back[piece], rect, sx, sy, colour);
        }

        // --- assemble ---------------------------------------------------------------
        for (idx, name) in PartName::ALL.into_iter().enumerate() {
            let rect = PART_RECTS[idx];
            let mut pixels = std::mem::take(&mut back[idx]);
            let mut over = std::mem::take(&mut front[idx]);
            clamp_buffer(&mut pixels);
            clamp_buffer(&mut over);
            composite(&mut pixels, &over);
            // Source-over of two valid premultiplied colours is valid; the clamp only
            // absorbs floating-point rounding, which the constructor rejects outright.
            clamp_buffer(&mut pixels);
            debug_assert_in_bounds(&pixels, &rect, name);
            let sprite = Sprite::from_premultiplied(rect.w, rect.h, rect.pivot(), pixels)
                .unwrap_or_else(|e| panic!("Lanternjaw part {name:?} could not be built: {e}"));
            debug_assert!(
                sprite.extent() <= PART_EXTENT_MAX,
                "{name:?} extent {} exceeds PART_EXTENT_MAX",
                sprite.extent()
            );
            out.push(Part { name, sprite, offset: rect.offset });
        }
    }

    /// Draw the body at `anchor` facing `heading` (chart tangent, any length) at presentation
    /// `seconds` in `mode`, at `opacity`, reusing `parts` and `scratch`.
    ///
    /// **Normative**: [`Lanternjaw::parts`] into `parts`, then one
    /// [`cubarium_render::stamp_rig`]`(canvas, anchor, heading, &[(rig_parts, 1.0)], opacity,
    /// scratch)` with `rig_parts` the parts' [`Part::rig_part`]s in order. Nothing else: the
    /// single root query, the rim, the material sums and the depth order are `stamp_rig`'s,
    /// and a non-normalizable heading draws nothing. The caller is responsible for `anchor`
    /// being canonical. A later presenter cross-fading two modes passes both modes' parts as
    /// two weighted states to the same `stamp_rig` call.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        canvas: &mut Canvas,
        anchor: SurfacePoint,
        heading: Vec2,
        seconds: f64,
        mode: Mode,
        opacity: f32,
        parts: &mut Vec<Part>,
        scratch: &mut Vec<PixelImage>,
    ) {
        self.parts(seconds, mode, parts);
        let rig: Vec<RigPart<'_>> = parts.iter().map(Part::rig_part).collect();
        debug_assert!(
            cubarium_render::rig_radius(&[(&rig, 1.0)]) <= QUERY_RADIUS_MAX,
            "the rig needs a query radius past QUERY_RADIUS_MAX"
        );
        stamp_rig(canvas, anchor, heading, &[(&rig, 1.0)], opacity, scratch);
    }
}

/// Every painted texel centre of one part, in body-local pixels, inside the module's
/// normative footprint. Debug builds only: a violation is a rasterization bug.
fn debug_assert_in_bounds(pixels: &[Rgba], rect: &Rect, name: PartName) {
    if !cfg!(debug_assertions) {
        return;
    }
    for (i, texel) in pixels.iter().enumerate() {
        if texel[3] <= 0.0 {
            continue;
        }
        let x = rect.x0 + (i % rect.w) as f64 + 0.5;
        let y = rect.y0 + (i / rect.w) as f64 + 0.5;
        assert!(
            x >= -BOUND_BACK && x <= BOUND_FRONT && y >= -BOUND_ABOVE && y <= BOUND_BELOW,
            "{name:?} paints a texel at body ({x}, {y}), outside the footprint"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_render::rig_radius;

    fn rig_parts(parts: &[Part]) -> Vec<RigPart<'_>> {
        parts.iter().map(Part::rig_part).collect()
    }

    /// Every mode, every 60 fps instant of twelve seconds: eight parts in painting order,
    /// every part inside its own budget and the whole rig inside its query bound. The
    /// footprint itself is asserted inside `parts` (debug builds), which this exercises.
    #[test]
    fn every_frame_of_every_mode_builds_eight_bounded_parts() {
        let rig = Lanternjaw::new();
        let mut parts = Vec::new();
        let mut worst_extent = 0.0f64;
        let mut worst_radius = 0.0f64;
        for mode in Mode::ALL {
            for frame in 0..(12 * 60) {
                rig.parts(f64::from(frame) / 60.0, mode, &mut parts);
                assert_eq!(parts.len(), 8, "{mode:?} frame {frame}");
                for (part, name) in parts.iter().zip(PartName::ALL) {
                    assert_eq!(part.name, name, "{mode:?}: parts are in painting order");
                    worst_extent = worst_extent.max(part.sprite.extent());
                    assert!(
                        part.sprite.extent() <= PART_EXTENT_MAX,
                        "{mode:?} {name:?}: extent {}",
                        part.sprite.extent()
                    );
                }
                let radius = rig_radius(&[(&rig_parts(&parts), 1.0)]);
                worst_radius = worst_radius.max(radius);
                assert!(radius <= QUERY_RADIUS_MAX, "{mode:?} frame {frame}: radius {radius}");
            }
        }
        // Non-vacuity: the budgets are actually approached, not trivially satisfied.
        assert!(worst_extent > 5.0 && worst_radius > 12.0, "{worst_extent} / {worst_radius}");
    }

    /// The hull pieces, the underside and the glow share the body lattice: integer offsets
    /// and integer sprite pivots, so their texel centres coincide in body coordinates.
    #[test]
    fn the_summed_material_parts_share_the_body_lattice() {
        let rig = Lanternjaw::new();
        let mut parts = Vec::new();
        rig.parts(1.25, Mode::Hunt, &mut parts);
        for part in &parts {
            let lattice = matches!(
                part.name,
                PartName::Tail
                    | PartName::Abdomen
                    | PartName::Thorax
                    | PartName::Head
                    | PartName::Underside
                    | PartName::Glow
            );
            if !lattice {
                continue;
            }
            let (offset, pivot) = (part.offset, part.sprite.pivot());
            assert_eq!(offset.x.fract(), 0.0, "{:?} offset {offset:?}", part.name);
            assert_eq!(offset.y.fract(), 0.0, "{:?} offset {offset:?}", part.name);
            assert_eq!(pivot.x.fract(), 0.0, "{:?} pivot {pivot:?}", part.name);
            assert_eq!(pivot.y.fract(), 0.0, "{:?} pivot {pivot:?}", part.name);
        }
        // And the four hull pieces are one material in `PartName` column order.
        let offsets: Vec<f64> = parts
            .iter()
            .filter(|p| p.name.layer() == 2)
            .map(|p| p.offset.x)
            .collect();
        assert_eq!(offsets, vec![-7.0, -3.0, 2.0, 6.0], "left column dx + 2 per piece");
    }

    /// A pure function of `(seconds, mode)`: a repeated call is the same sprite texel for
    /// texel, and a non-finite instant reads as 0.
    #[test]
    fn parts_are_a_pure_function_of_the_instant_and_the_mode() {
        let rig = Lanternjaw::new();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for (seconds, mode) in [(0.0, Mode::Rest), (3.3, Mode::Hunt), (7.77, Mode::Move)] {
            rig.parts(seconds, mode, &mut a);
            rig.parts(seconds, mode, &mut b);
            same(&a, &b, "a repeated call");
        }
        rig.parts(f64::NAN, Mode::Bud, &mut a);
        rig.parts(0.0, Mode::Bud, &mut b);
        same(&a, &b, "a non-finite instant reads as 0");
        // The hunt cycle is exactly periodic, so a loop has no step.
        rig.parts(1.75, Mode::Hunt, &mut a);
        rig.parts(1.75 + HUNT_PERIOD, Mode::Hunt, &mut b);
        same(&a, &b, "the hunt cycle at t and t + HUNT_PERIOD");
    }

    fn same(a: &[Part], b: &[Part], what: &str) {
        assert_eq!(a.len(), b.len());
        for (p, q) in a.iter().zip(b) {
            assert_eq!(p.name, q.name);
            assert_eq!(p.offset, q.offset, "{what}: {:?}", p.name);
            for y in 0..p.sprite.height() as i32 {
                for x in 0..p.sprite.width() as i32 {
                    assert_eq!(
                        p.sprite.texel(x, y),
                        q.sprite.texel(x, y),
                        "{what}: {:?} texel ({x}, {y})",
                        p.name
                    );
                }
            }
        }
    }

    /// The strike's warm accent belongs to the hunt alone, and it is an envelope rather than
    /// a flash: the jaw walks to it and back through intermediate colours.
    #[test]
    fn the_strike_accent_is_a_hunt_only_envelope() {
        let rig = Lanternjaw::new();
        let mut parts = Vec::new();
        // The jaw's own colour is the accent's carrier; sample the Head piece's peak red.
        let mut hottest = |mode: Mode| {
            let mut peak = 0.0f32;
            for frame in 0..(12 * 60) {
                rig.parts(f64::from(frame) / 60.0, mode, &mut parts);
                let head = &parts[HEAD].sprite;
                for y in 0..head.height() as i32 {
                    for x in 0..head.width() as i32 {
                        let t = head.texel(x, y);
                        // The accent is the only colour whose red far exceeds its blue.
                        if t[3] > 0.5 && t[0] > t[2] * 1.5 {
                            peak = peak.max(t[0]);
                        }
                    }
                }
            }
            peak
        };
        let hunt = hottest(Mode::Hunt);
        assert!(hunt > 0.2, "the hunt's accent should reach the warm colour, got {hunt}");
        for quiet in [Mode::Rest, Mode::Move, Mode::Bud] {
            assert_eq!(hottest(quiet), 0.0, "{quiet:?} showed the strike accent");
        }
        // Intermediate values, with no single-frame jump: the envelope's largest step at
        // 60 fps is far below its own swing.
        let mut previous = hunt_state(T_SNAP - 0.2).accent;
        let mut largest = 0.0f64;
        for frame in 0..60 {
            let accent = hunt_state(T_SNAP - 0.2 + f64::from(frame) / 60.0).accent;
            largest = largest.max((accent - previous).abs());
            previous = accent;
        }
        assert!(largest > 0.0 && largest <= 0.31, "largest accent step {largest}");
    }

    /// At full extension the near limb is *in front of* the hull where the two overlap, and
    /// its claw — its brightest texel, in the claw colour — is deliberately **past** the hull
    /// altogether: `fable.md` has the tip four pixels beyond the jaw at `dx + 8`, which is
    /// what [`BOUND_FRONT`] (12.3 plus the lunge) is sized for. Both halves matter: the depth
    /// is observable on the shoulder, and the claw is not on the carapace at all.
    #[test]
    fn at_the_strike_the_near_limb_is_over_the_hull_and_its_claw_reaches_past_it() {
        use cubarium_render::stamp_rig;
        use cubarium_surface::Face;

        let rig = Lanternjaw::new();
        let mut parts = Vec::new();
        rig.parts(T_OPEN, Mode::Hunt, &mut parts);
        let anchor = SurfacePoint::pixel_center(Face::Front, 32, 32);
        let heading = Vec2::new(1.0, 0.0);
        let subset = |keep: &dyn Fn(PartName) -> bool| {
            let chosen: Vec<RigPart<'_>> =
                parts.iter().filter(|p| keep(p.name)).map(Part::rig_part).collect();
            let mut canvas = Canvas::new();
            stamp_rig(&mut canvas, anchor, heading, &[(&chosen, 1.0)], 1.0, &mut Vec::new());
            canvas
        };
        let whole = subset(&|_| true);
        let without_near = subset(&|n| n != PartName::NearLimb);
        let hull = subset(&|n| n.layer() == 2);
        let near = subset(&|n| n == PartName::NearLimb);
        let light = |c: &Canvas, x: u8, y: u8| {
            c.get(Face::Front, x, y).into_iter().map(f64::from).sum::<f64>()
        };

        let mut over_hull = 0usize;
        for x in 0..64u8 {
            for y in 0..64u8 {
                if light(&hull, x, y) <= 0.0 || light(&near, x, y) <= 0.0 {
                    continue;
                }
                let moved = (0..3)
                    .map(|c| {
                        (whole.get(Face::Front, x, y)[c] - without_near.get(Face::Front, x, y)[c])
                            .abs()
                    })
                    .fold(0.0f32, f32::max);
                if moved > 0.005 {
                    over_hull += 1;
                }
            }
        }
        assert!(over_hull >= 3, "the near limb changed only {over_hull} hull pixels");

        // The brightest near-limb pixel is the claw, ahead of the hull's own front.
        let (bx, by) = (0..64u8)
            .flat_map(|x| (0..64u8).map(move |y| (x, y)))
            .max_by(|&a, &b| {
                light(&near, a.0, a.1).partial_cmp(&light(&near, b.0, b.1)).unwrap()
            })
            .unwrap();
        assert!(bx >= 43, "the claw should reach past the jaw, but the brightest pixel is at {bx}");
        assert_eq!(
            light(&hull, bx, by),
            0.0,
            "the claw at ({bx}, {by}) is past the hull, so no hull pixel is there"
        );
    }
}
