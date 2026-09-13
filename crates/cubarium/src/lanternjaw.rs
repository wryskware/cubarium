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
        cubarium_render::RigPart {
            sprite: &self.sprite,
            offset: self.offset,
            layer: self.name.layer(),
        }
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

/// A semantic channel read as a fraction of one: NaN is 0 and the range is clamped, so no
/// adapter mistake can make the body vanish or paint outside its footprint.
fn unit(v: f64) -> f64 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
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
    let u = if reach < 0.0 {
        1.0 + reach / 0.35
    } else {
        reach
    };
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
        Rect {
            x0,
            y0,
            w,
            h,
            offset: Vec2::new(ox, oy),
        }
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
fn line(buf: &mut [Rgba], rect: &Rect, a: [f64; 2], b: [f64; 2], colour: Rgba, extend_start: bool) {
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
                    cells.push(Cell {
                        col,
                        y: ROW_TOP + row as f64,
                        ch,
                    });
                }
            }
        }
        Lanternjaw {
            cells,
            palette: Palette::decode_once(),
        }
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
        self.rasterize(&Channels::study(seconds, mode), out);
    }

    /// Rasterize the eight parts from explicit channels: the whole of what
    /// [`Lanternjaw::parts`] did after computing its per-mode quantities, unchanged —
    /// **`parts(seconds, mode)` is `rasterize(&Channels::study(seconds, mode), out)`, bit for
    /// bit**. The gut breath adds `GUT_BREATH_PX · gut · sin(2π t / GUT_BREATH_SECONDS)` to
    /// `dy` of template columns 4..=8 (and to the legs and cocoon on those columns, which ride
    /// the same columns); a gut of 0 adds exactly 0. The cocoon, when `Some(reveal)`, is the
    /// study's six cells with every alpha multiplied by `reveal` (its breath on `t`); `None`
    /// paints none.
    pub fn rasterize(&self, ch: &Channels, out: &mut Vec<Part>) {
        out.clear();
        let t = if ch.t.is_finite() { ch.t } else { 0.0 };
        let p = &self.palette;

        // --- motion parameters, now explicit data rather than a mode ----------------
        let pulse_gain = ch.pulse_gain;
        let compress = ch.compress;
        let lunge = ch.lunge;
        let reach = ch.reach;
        let tail_lift = ch.tail_lift;
        let accent = ch.accent;
        // The ambient blinks, summed, plus the attack's one post-recoil blink.
        let mut closure = 0.0;
        for (weight, period) in ch.blinks {
            // A zero weight contributes exactly 0, so the study's single blink is bit for
            // bit what it always was.
            if weight == 0.0 || !period.is_finite() || period == 0.0 {
                continue;
            }
            closure += weight * blink_closure(t, period);
        }
        closure = (closure + ch.blink_extra).clamp(0.0, 1.0);
        let cocoon_reveal = ch.cocoon.map(unit);
        let gut = unit(ch.gut);

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
        // The far limb lags [`FAR_LAG_SECONDS`] and rides a pixel higher, at 55 % alpha; the
        // lag itself is the producer's business (the study's modulo, the episode's clamp).
        let far_reach = ch.far_reach;
        let far_limb = with_alpha(
            decode(mix3(limb_dark, limb_lit, far_reach.clamp(0.0, 1.0))),
            0.55,
        );

        // --- per-column body offsets: a travelling sinuous wave, looser at the tail,
        //     plus the coil (shorten) and the head lunge. Both unrounded. ------------
        let mut col_dx = [0.0f64; COL_COUNT];
        let mut col_dy = [0.0f64; COL_COUNT];
        // The gut breath: a slow swell of the abdomen columns while a meal remains. A gut of
        // 0 is skipped, so it adds exactly 0 and the gallery is untouched.
        let gut_breath = if gut > 0.0 {
            let swell = GUT_BREATH_PX * gut * (TAU * t / GUT_BREATH_SECONDS).sin();
            if swell.is_finite() { swell } else { 0.0 }
        } else {
            0.0
        };
        for i in 0..COL_COUNT {
            let rel = i as f64 / (COL_COUNT - 1) as f64;
            let taper = 0.25 + 0.75 * (1.0 - rel);
            let mut dy = 0.0;
            for (amp, period) in ch.waves {
                // A zero-amplitude wave is skipped rather than added, so one wave alone is
                // bit for bit the study's single wave.
                if amp == 0.0 {
                    continue;
                }
                let arg = TAU * t / period + i as f64 * 0.4;
                if !arg.is_finite() {
                    continue;
                }
                dy += amp * taper * arg.sin();
            }
            if gut_breath != 0.0 && (4..=8).contains(&i) {
                dy += gut_breath;
            }
            if i < 3 {
                dy -= tail_lift;
            }
            col_dy[i] = dy;
            col_dx[i] = compress * (1.0 - rel) + lunge * ((i as f64 - 9.0) / 8.0).clamp(0.0, 1.0);
        }

        // Two buffers per part: the study's own painting order inside one part is an
        // additive material (`back`) with at most one source-over group over it (`front`) —
        // the claw over its limb, the cocoon over the legs.
        let mut back: Vec<Vec<Rgba>> = PART_RECTS
            .iter()
            .map(|r| vec![[0.0f32; 4]; r.w * r.h])
            .collect();
        let mut front: Vec<Vec<Rgba>> = PART_RECTS
            .iter()
            .map(|r| vec![[0.0f32; 4]; r.w * r.h])
            .collect();

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
            let amount = if ch.gait_amount.is_finite() {
                ch.gait_amount
            } else {
                0.0
            };
            let (swing, lift) = if amount != 0.0 && ch.gait.is_finite() && ch.gait != 0.0 {
                let u = frac(t / ch.gait + k as f64 * 0.34);
                // The study's binary lift is a continuous bump here.
                let lift = if u < 0.38 { (PI * u / 0.38).sin() } else { 0.0 };
                (amount * (1.3 * (TAU * u).sin()), amount * lift)
            } else {
                // An amount of 0 plants the legs exactly, whatever the period is.
                (0.0, 0.0)
            };
            splat(&mut back[UNDERSIDE], rect, base + dx, 2.0 + dy, p.leg);
            splat(
                &mut back[UNDERSIDE],
                rect,
                base + dx + swing,
                3.0 + dy - lift,
                p.leg,
            );
        }

        // --- cocoon, over the legs as the study paints it ---------------------------
        // `Some(reveal)` paints the study's six cells with every alpha scaled by the reveal
        // (a premultiplied colour times a scalar is the same colour at that coverage), so a
        // reveal of 1 is the study's cocoon bit for bit and `None` paints nothing at all.
        if let Some(reveal) = cocoon_reveal.filter(|r| *r > 0.0) {
            let breath = 0.5 + 0.5 * (TAU * t / COCOON_BREATH_SECONDS).sin();
            let revealed = |c: Rgba| -> Rgba {
                let r = reveal as f32;
                [c[0] * r, c[1] * r, c[2] * r, c[3] * r]
            };
            let shell = revealed(with_alpha(p.cyan_s, 0.34 + 0.16 * breath));
            let rim = revealed(with_alpha(p.cyan, 0.5 + 0.24 * breath));
            let core = revealed(lerp4(
                with_alpha(p.cyan, 0.62),
                [p.pink[0], p.pink[1], p.pink[2], 1.0],
                0.18 + 0.3 * breath,
            ));
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
            let mut crest = 0.0;
            for (weight, period) in ch.pulses {
                // A zero-weight crest is skipped: the study's single crest is unchanged.
                if weight == 0.0 {
                    continue;
                }
                let u = frac(t / period + (COL_COUNT - 1 - i) as f64 * 0.06);
                if !u.is_finite() {
                    continue;
                }
                let d = u.min(1.0 - u);
                crest += weight * smoothstep(1.0 - d / 0.26);
            }
            (pulse_gain * crest).clamp(0.0, 1.25)
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
            out.push(Part {
                name,
                sprite,
                offset: rect.offset,
            });
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

// ---------------------------------------------------------------------------------------
// The living body: semantic pose inputs, real attack phases and juvenile scale.
//
// `Mode` and `parts(seconds, mode)` are the study's gallery: a six-second loop that strikes
// every cycle and a `Bud` that always wears a cocoon. Ecology has none of that. The living
// entry points below take independent channels — ambient time for the rhythms, real
// movement, a real attack episode with its own elapsed time, real gut and funded escrow —
// and the study modes are re-expressed as one producer of the same explicit `Channels`, so
// the gallery, its palette and its silhouette tests stay exactly what they were.
// (Contract: `design/7_Research/lanternjaw-ecology-animation-contract-2026-09-13.md`.)
// ---------------------------------------------------------------------------------------

/// Smallest whole-rig scale the body is drawn at. The adult is 1. The mapping from core
/// structure to this scale is the core profile's (`body_scale = max(body_scale_min, (S /
/// S_adult)^body_scale_exponent)`, trial minimum 0.2 and exponent 0.5, so a valid world with
/// `child_structure_fraction = 0.1` yields `sqrt(0.1) = 0.316…` and the default 0.4 yields
/// 0.632…); the renderer admits exactly the core's minimum and scales *everything* by the
/// value it is handed (see [`Lanternjaw::draw_living`]), so the drawn claws and the core's
/// contact geometry can never separate. At 0.2 the body is under four pixels long and reads
/// as a violet-and-cyan smudge with a brighter head — coherent with the adult's proportions,
/// not legible as the adult's structure; that is the admitted range, not a rendering choice.
pub const SCALE_MIN: f64 = 0.2;
/// Largest admitted whole-rig scale: the adult.
pub const SCALE_MAX: f64 = 1.0;
/// Seconds the far forelimb lags the near one, in **attack** time (the study's 45 ms), never a
/// frame count and never a modulo of presentation time.
pub const FAR_LAG_SECONDS: f64 = 0.045;
/// Seconds of a real `Strike` spent on the cubic extension, at its **end**: the study's
/// 120 ms snap, placed so full contact is reached exactly at the phase's settlement boundary.
pub const EXTEND_SECONDS: f64 = 0.12;
/// Seconds of recoil from the displayed reach at the start of `Recovering` / `Handling`.
pub const RECOIL_SECONDS: f64 = 0.2;
/// Seconds after the recoil ends before the one post-strike blink starts.
pub const BLINK_AFTER_RECOIL: f64 = 0.3;
/// Seconds over which the hush of an attack releases into the ambient body during
/// `Handling` (the meal is shown by the gut breath, not by the arms).
pub const HANDLING_RELEASE_SECONDS: f64 = 1.0;
/// Time constant, seconds, of the lantern chain's charge bleeding out after a release.
pub const CHARGE_DECAY_SECONDS: f64 = 0.9;
/// Fraction of a real gestation over which the cocoon is revealed (its alpha rises with
/// `smoothstep(progress / COCOON_REVEAL)`); after that it breathes at full alpha.
pub const COCOON_REVEAL: f64 = 0.2;
/// The cocoon's breath period, seconds of ambient time (the study's 2.6 s).
pub const COCOON_BREATH_SECONDS: f64 = 2.6;
/// The gut breath: a slow vertical swell of the abdomen columns while gut material remains,
/// `GUT_BREATH_PX · gut · sin(2π t / GUT_BREATH_SECONDS)` added to the wave of template
/// columns 4..=8, in body pixels at gut fraction 1.
pub const GUT_BREATH_PX: f64 = 0.3;
pub const GUT_BREATH_SECONDS: f64 = 2.2;
/// How far an attack hushes the ambient wave: amplitude × `(1 − WAVE_HUSH · hush)` (the study's
/// hunt wave is 0.22 of rest's 0.55).
pub const WAVE_HUSH: f64 = 0.6;

/// Where the body touches the world, in **adult** body pixels at the named pose, for the core
/// adapter's contact geometry (`lanternjaw-ecology-animation-contract-2026-09-13.md`). A
/// study coordinate names a pixel, so these are painted texel *centres* (the `+0.5` of the
/// body lattice included), without the decorative wave. Multiply by the rig scale for a
/// juvenile ([`effectors`]). Not a claim about ecology: the core owns any contact tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effectors {
    /// The ingestion mouth: the outer jaw column's centre. `(8.5, 0)` folded, `(9.6, 0)` at
    /// full lunge.
    pub mouth: Vec2,
    /// The near claw's centre at full extension: `(12.3 + head_dx + 0.5, 0.6 + 0.5)` with
    /// `head_dx = −0.3 · (4/17) + 1.1 · 0.5`, i.e. `(13.2794, 1.1)`.
    pub near_claw: Vec2,
    /// The far claw's centre once its 45 ms lag is over — **after** the settlement boundary,
    /// not at it (at the boundary the far claw is still 62 % through its own extension): the
    /// same as the near claw, one pixel higher. A contact check at settlement should use
    /// `near_claw`.
    pub far_claw: Vec2,
}

/// The adult effectors at full extension (`lunge = 1.1`, `compress = −0.3`, `reach = 1`),
/// scaled by `scale`. **Normative**: `mouth = scale · (8 + 1.1 + 0.5, 0)`, `near_claw = scale ·
/// (12.3 + head_dx + 0.5, 1.1)`, `far_claw = near_claw − scale · (0, 1)`, with
/// `head_dx = −0.3 · (1 − 13/17) + 1.1 · clamp((13 − 9) / 8, 0, 1)`.
pub fn effectors(scale: f64) -> Effectors {
    let head_dx = -0.3 * (1.0 - 13.0 / 17.0) + 1.1 * ((13.0 - 9.0) / 8.0f64).clamp(0.0, 1.0);
    let near = Vec2::new(
        (LIMB_STRIKE.1[0] + head_dx + CELL_CENTRE) * scale,
        (LIMB_STRIKE.1[1] + CELL_CENTRE) * scale,
    );
    Effectors {
        mouth: Vec2::new((8.0 + 1.1 + CELL_CENTRE) * scale, 0.0),
        near_claw: near,
        far_claw: Vec2::new(near.x, near.y - scale),
    }
}

/// The displayed extension state of the forelimbs and hull at one instant, carried from one
/// attack phase into the next so an interrupted motion continues from where it was drawn
/// rather than snapping to a study keyframe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    /// Near forelimb: `< 0` cocked, 0 folded, 1 fully extended.
    pub near: f64,
    /// Far forelimb, the same scale (it lags the near one by [`FAR_LAG_SECONDS`]).
    pub far: f64,
    /// Hull compression (the study's `compress`, 1.7 fully coiled, −0.3 at full lunge).
    pub compress: f64,
    /// Head lunge (the study's `lunge`, 1.1 at full extension).
    pub lunge: f64,
    /// Lantern chain charge, 0..1.
    pub charge: f64,
}

impl Reach {
    /// Arms folded, hull at rest, chain uncharged: the state of every phase-free body.
    pub const FOLDED: Reach = Reach {
        near: 0.0,
        far: 0.0,
        compress: 0.0,
        lunge: 0.0,
        charge: 0.0,
    };
    /// Full extension at settlement, the study's snap arrival.
    pub const EXTENDED: Reach = Reach {
        near: 1.0,
        far: 1.0,
        compress: -0.3,
        lunge: 1.1,
        charge: 1.0,
    };
}

/// The real hunting phases that move the body (the core's `Perched` and `Stalking` are
/// phase-free: no episode, arms folded, the ambient body driven by real movement).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttackPhase {
    /// Fold → cock over the whole real windup; no extension, no contact.
    Windup,
    /// Hold the cocked entry pose, then one cubic extension in the final
    /// [`EXTEND_SECONDS`], reaching full contact **at** the phase's settlement boundary and
    /// holding it past it.
    Strike,
    /// Recoil from the entry reach over [`RECOIL_SECONDS`], then folded stillness for the
    /// rest of the recovery. No repeat strike.
    Recovering,
    /// The successful-capture recoil, after which the hush releases and the ambient body
    /// resumes; the meal itself is the gut breath of [`LivingPose::gut`], never a flourish
    /// implied by the phase name.
    Handling,
}

/// One real attack episode as the adapter sees it at a presentation instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackEpisode {
    pub phase: AttackPhase,
    /// Seconds of **attack time** since this phase was entered (from the phase-entry tick and
    /// the presenter's fractional clock), never a modulo of anything. Negative reads as 0.
    pub elapsed: f64,
    /// The phase's real duration in seconds (the core profile's windup / strike length;
    /// for `Recovering` and `Handling` any positive value — only their recoil and release
    /// constants matter). Non-positive or non-finite reads as an instantaneous phase.
    pub duration: f64,
    /// The reach displayed at the instant this phase was entered — the previous episode's
    /// [`AttackChannels::reach`] at its final elapsed time, or [`Reach::FOLDED`] for an attack
    /// that starts from rest — so every phase continues from the picture the last one left.
    pub from: Reach,
}

/// What an attack episode contributes to the body at one instant.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackChannels {
    pub near_reach: f64,
    pub far_reach: f64,
    pub compress: f64,
    pub lunge: f64,
    pub charge: f64,
    /// The strike accent on jaw and claw, 0..1, bounded by [`envelope`].
    pub accent: f64,
    /// The one post-recoil blink, 0..1, added to the ambient closure.
    pub blink: f64,
    /// How far the ambient rhythms are hushed, 0..1: the wave amplitude scales by
    /// `1 − WAVE_HUSH · hush` and the lantern gain by `1 − hush · (0.62 − 0.92 · charge)`.
    pub hush: f64,
}

impl AttackChannels {
    /// The displayed state, to hand to the next episode as its `from`.
    pub fn reach(&self) -> Reach {
        Reach {
            near: self.near_reach,
            far: self.far_reach,
            compress: self.compress,
            lunge: self.lunge,
            charge: self.charge,
        }
    }
}

/// The attack channels of an episode at its `elapsed` time; `None` is a phase-free body
/// (all zero, [`Reach::FOLDED`]).
///
/// **Normative**, with `t = max(elapsed, 0)`, `D = duration` (non-positive or non-finite ⇒ 0),
/// `f = from`, `smoothstep` the Hermite polynomial on a clamped argument and `e(u) = 1 −
/// (1 − u)³`:
///
/// * `Windup`: `u = smoothstep(t / D)` (1 when `D = 0`); `near = f.near + (−0.35 − f.near)·u`,
///   `compress = f.compress + (1.7 − f.compress)·u`, `lunge = f.lunge·(1 − u)`, `charge =
///   f.charge + (1 − f.charge)·u`, `accent = blink = 0`, `hush = 1`.
/// * `Strike`: `E = min(EXTEND_SECONDS, D)`, `hold = D − E`. For `t < hold` the entry pose is
///   held: `near = f.near`, `compress = f.compress`, `lunge = f.lunge`, `charge = f.charge`.
///   For `t ≥ hold`: `u = clamp((t − hold) / E, 0, 1)` (1 when `E = 0`), `near = f.near + (1 −
///   f.near)·e(u)`, `compress = f.compress + (−0.3 − f.compress)·e(u)`, `lunge = f.lunge + (1.1
///   − f.lunge)·e(u)`, `charge = f.charge · exp(−(t − hold) / CHARGE_DECAY_SECONDS)`. In
///   **both** branches `accent = envelope((t − (hold − 0.02)) / ACCENT_SECONDS)`: the study's
///   20 ms pre-roll opens inside the hold, rising from exactly 0 (it is 0 for every `t ≤ hold −
///   0.02`, so a paid approach never accents) rather than entering with a step. Past `D` the
///   full pose is **held** (settlement), the accent finishing on its own. `blink = 0`, `hush =
///   1`.
/// * `Recovering`: `u = smoothstep(t / RECOIL_SECONDS)`; `near = f.near·(1 − u)`, `compress =
///   f.compress·(1 − u)`, `lunge = f.lunge·(1 − u)`, `charge = f.charge · exp(−t /
///   CHARGE_DECAY_SECONDS)`, `accent = 0`, `blink = envelope((t − (RECOIL_SECONDS +
///   BLINK_AFTER_RECOIL)) / BLINK_SECONDS)`, `hush = 1` throughout.
/// * `Handling`: as `Recovering`, except `hush = 1 − smoothstep((t − RECOIL_SECONDS) /
///   HANDLING_RELEASE_SECONDS)`.
/// * The far forelimb lags: `far_reach` is the `near` of the same rules evaluated at `t −
///   FAR_LAG_SECONDS` when that is ≥ 0, and `f.far` before that (the previous episode's
///   displayed far reach holds through the lag, so the far claw never jumps at a phase
///   boundary).
///
/// The study's `hunt_state` is this schedule with `Windup { D = T_SNAP − T_COIL, from:
/// FOLDED }`, `Strike { D = E = T_OPEN − T_SNAP, from: the windup's end }` and `Recovering
/// { from: the strike's end }`, so at the study's keyframes reach, compress, lunge, charge
/// and blink agree to 1e-9. The accent agrees only within the phase that owns it: cutting the
/// study's gesture into phases makes its 20 ms pre-roll unreachable where it falls before a
/// phase boundary (the study's own 120 ms windup ends at `T_SNAP`, so over `(T_SNAP − 0.02,
/// T_SNAP)` this schedule has no accent where `hunt_state` has up to 0.103), and `Recovering`
/// carries none where the study's plateau still shows at `T_OPEN`. A real windup's pre-roll
/// lives inside its own strike, so a real attack never loses it.
pub fn attack_channels(episode: Option<&AttackEpisode>) -> AttackChannels {
    let Some(episode) = episode else {
        return AttackChannels::default();
    };
    let t = attack_time(episode.elapsed);
    let mut channels = phase_channels(episode, t);
    // The far forelimb reads the same schedule 45 ms of attack time earlier; before the
    // episode began there is nothing to read, so the previous episode's displayed far reach
    // holds through the lag and the far claw never jumps at a phase boundary.
    let lagged = t - FAR_LAG_SECONDS;
    channels.far_reach = if lagged >= 0.0 {
        phase_channels(episode, lagged).near_reach
    } else {
        episode.from.far
    };
    channels
}

/// `max(elapsed, 0)` with a NaN read as 0 (an infinite elapsed is a held phase, which every
/// rule below saturates correctly).
fn attack_time(elapsed: f64) -> f64 {
    if elapsed.is_nan() {
        0.0
    } else {
        elapsed.max(0.0)
    }
}

/// One phase's channels at attack time `t`, everything but the far forelimb's lag.
fn phase_channels(episode: &AttackEpisode, t: f64) -> AttackChannels {
    let f = episode.from;
    let d = if episode.duration.is_finite() && episode.duration > 0.0 {
        episode.duration
    } else {
        0.0
    };
    let mut c = AttackChannels {
        far_reach: f.far,
        ..AttackChannels::default()
    };
    match episode.phase {
        AttackPhase::Windup => {
            // The study's 120 ms coil stretched over the whole real windup: no extension and
            // no contact, whatever the windup's length.
            let u = if d > 0.0 { smoothstep(t / d) } else { 1.0 };
            c.near_reach = f.near + (-0.35 - f.near) * u;
            c.compress = f.compress + (1.7 - f.compress) * u;
            c.lunge = f.lunge * (1.0 - u);
            c.charge = f.charge + (1.0 - f.charge) * u;
            c.hush = 1.0;
        }
        AttackPhase::Strike => {
            // Hold the entry pose through the paid approach, then one cubic extension in the
            // final `EXTEND_SECONDS`, arriving at full contact exactly at the settlement
            // boundary and holding it past that.
            let e_span = EXTEND_SECONDS.min(d);
            let hold = d - e_span;
            // The accent envelope opens its documented 20 ms *before* the extension, which
            // for a strike with a paid approach falls inside the hold — the study's own
            // pre-roll before `T_SNAP`. It is evaluated in both branches so it rises from 0
            // instead of entering at `envelope(0.02 / ACCENT_SECONDS) ≈ 0.103`; it is 0 for
            // every `t ≤ hold − 0.02`, so nothing accents during the approach proper.
            c.accent = envelope((t - (hold - 0.02)) / ACCENT_SECONDS);
            if t < hold {
                c.near_reach = f.near;
                c.compress = f.compress;
                c.lunge = f.lunge;
                c.charge = f.charge;
            } else {
                let u = if e_span > 0.0 {
                    ((t - hold) / e_span).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let e = 1.0 - (1.0 - u) * (1.0 - u) * (1.0 - u);
                c.near_reach = f.near + (1.0 - f.near) * e;
                c.compress = f.compress + (-0.3 - f.compress) * e;
                c.lunge = f.lunge + (1.1 - f.lunge) * e;
                c.charge = f.charge * (-(t - hold) / CHARGE_DECAY_SECONDS).exp();
            }
            c.hush = 1.0;
        }
        AttackPhase::Recovering | AttackPhase::Handling => {
            // Recoil from the reach actually displayed when the phase was entered, then
            // folded stillness: no repeat strike while the recovery runs out.
            let u = smoothstep(t / RECOIL_SECONDS);
            c.near_reach = f.near * (1.0 - u);
            c.compress = f.compress * (1.0 - u);
            c.lunge = f.lunge * (1.0 - u);
            c.charge = f.charge * (-t / CHARGE_DECAY_SECONDS).exp();
            c.blink = envelope((t - (RECOIL_SECONDS + BLINK_AFTER_RECOIL)) / BLINK_SECONDS);
            c.hush = if episode.phase == AttackPhase::Handling {
                // The meal is the gut breath, not the arms: the hush releases into the
                // ambient body once the capture recoil is over.
                1.0 - smoothstep((t - RECOIL_SECONDS) / HANDLING_RELEASE_SECONDS)
            } else {
                1.0
            };
        }
    }
    c
}

/// The semantic inputs of one frame of the living body. Every field is independent; nothing
/// here is a mode and nothing loops on its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LivingPose {
    /// Presentation seconds ([`crate::art_present::present_seconds`]) for the **ambient**
    /// rhythms only: the body wave, the lantern crest, the ambient blink, the gait phase, the
    /// cocoon and gut breaths. An attack never reads it.
    pub ambient: f64,
    /// 0 = perched (the rest wave, planted legs, the rest blink), 1 = full locomotion (the
    /// move wave, the gait, the move blink); in between a blend. The adapter derives it from
    /// real root movement (for example speed over the profile's maximum), so a valid target
    /// does not by itself move the legs.
    pub movement: f64,
    /// The real attack episode, if any.
    pub attack: Option<AttackEpisode>,
    /// Actual gut fill, 0..1 (`gut_material / gut_capacity`): the abdomen breathes by
    /// [`GUT_BREATH_PX`]` · gut`. 0 is no meal, whatever the phase says.
    pub gut: f64,
    /// Normalized gestation of a **funded** escrow, `Some(0..1)`; `None` means no cocoon at
    /// all, whatever the phase, satiety or reserve.
    pub cocoon: Option<f64>,
}

/// The explicit inputs of the rasterizer — everything [`Lanternjaw::parts`] used to compute
/// from `(seconds, mode)` before splatting, made data. Two producers build it:
/// [`Channels::study`] (the gallery modes, bit for bit what they always drew) and
/// [`Channels::living`] (the semantic pose). Public so tests can probe either producer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channels {
    /// Ambient seconds for every rhythm below.
    pub t: f64,
    /// Two body waves `(amplitude px, period s)`, summed: template column `i` is lifted by
    /// `Σ amp · taper_i · sin(2π t / period + 0.4 i)`; an amplitude of 0 contributes 0 exactly.
    pub waves: [(f64, f64); 2],
    /// Two lantern crests `(weight, period s)`, summed: column `i`'s brightness is
    /// `clamp(pulse_gain · Σ weight · smoothstep(1 − d / 0.26), 0, 1.25)` with `u = fract(t /
    /// period + (17 − i)·0.06)`, `d = min(u, 1 − u)`; a weight of 0 contributes 0 exactly.
    pub pulses: [(f64, f64); 2],
    pub pulse_gain: f64,
    /// Two ambient blinks `(weight, period s)`, summed into the eye closure, plus
    /// `blink_extra`; the sum is clamped to `[0, 1]`.
    pub blinks: [(f64, f64); 2],
    pub blink_extra: f64,
    /// The leg cycle period in seconds (the study's 1.2); `gait_amount` scales the swing
    /// (`1.3 · sin(2π u)`) and the lift bump (`sin(π u / 0.38)` for `u < 0.38`). A `gait_amount`
    /// of 0 plants the legs whatever the period.
    pub gait: f64,
    pub gait_amount: f64,
    /// The forelimbs and hull, as [`AttackChannels`] carries them.
    pub reach: f64,
    pub far_reach: f64,
    pub compress: f64,
    pub lunge: f64,
    pub accent: f64,
    /// Tail columns (`i < 3`) lifted by this many pixels.
    pub tail_lift: f64,
    /// `Some(reveal)`: the cocoon's six cells at their study colours times `reveal`, breathing
    /// on `t` at [`COCOON_BREATH_SECONDS`]; `None` draws no cocoon.
    pub cocoon: Option<f64>,
    /// The gut breath amplitude factor, 0..1 (see [`GUT_BREATH_PX`]).
    pub gut: f64,
}

impl Channels {
    /// The study's four modes as channels: **exactly** the quantities [`Lanternjaw::parts`]
    /// computed for `(seconds, mode)` (a non-finite `seconds` reads as 0), so that
    /// [`Lanternjaw::rasterize`] of these is texel-identical to the gallery.
    ///
    /// **Normative.** `Rest`: waves `[(0.55, 5.5), (0, 1)]`, pulses `[(1, 3.0), (0, 1)]`, gain
    /// 1, blinks `[(1, BLINK_PERIOD_REST), (0, 1)]`, gait `(1.2, 0)`, reach/far/compress/lunge/
    /// accent 0, tail 0, cocoon `None`, gut 0. `Move`: waves `[(1.15, 2.4), (0, 1)]`, pulses
    /// `[(1, 2.3), (0, 1)]`, blinks `[(1, BLINK_PERIOD_MOVE), (0, 1)]`, gait `(1.2, 1)`. `Hunt`,
    /// with `h = hunt_state(t mod 6)` and `h′ = hunt_state((t − 0.045) mod 6)`: waves `[(0.22,
    /// 6.0), (0, 1)]`, pulses `[(1, 3.0), (0, 1)]`, gain `0.38 + 0.92 h.charge`, blinks `[(0, 1),
    /// (0, 1)]`, `blink_extra = h.blink`, reach `h.reach`, far `h′.reach`, compress `h.compress`,
    /// lunge `h.lunge`, accent `h.accent`. `Bud`: waves `[(0.4, 6.5), (0, 1)]`, pulses `[(1, 3.6),
    /// (0, 1)]`, gain 0.8, blinks `[(1, BLINK_PERIOD_BUD), (0, 1)]`, tail 1, cocoon `Some(1)`.
    pub fn study(seconds: f64, mode: Mode) -> Channels {
        let t = if seconds.is_finite() { seconds } else { 0.0 };
        // `Rest`, and the shared skeleton of the other three. The second wave, crest and
        // blink are at weight 0 in every mode, and a zero weight contributes exactly 0, so
        // the gallery is the single-wave, single-crest, single-blink body it always was.
        let rest = Channels {
            t,
            waves: [(0.55, 5.5), (0.0, 1.0)],
            pulses: [(1.0, 3.0), (0.0, 1.0)],
            pulse_gain: 1.0,
            blinks: [(1.0, BLINK_PERIOD_REST), (0.0, 1.0)],
            blink_extra: 0.0,
            gait: 1.2,
            gait_amount: 0.0,
            reach: 0.0,
            far_reach: 0.0,
            compress: 0.0,
            lunge: 0.0,
            accent: 0.0,
            tail_lift: 0.0,
            cocoon: None,
            gut: 0.0,
        };
        match mode {
            Mode::Rest => rest,
            Mode::Move => Channels {
                waves: [(1.15, 2.4), (0.0, 1.0)],
                pulses: [(1.0, 2.3), (0.0, 1.0)],
                blinks: [(1.0, BLINK_PERIOD_MOVE), (0.0, 1.0)],
                gait_amount: 1.0,
                ..rest
            },
            Mode::Hunt => {
                let h = hunt_state(t.rem_euclid(HUNT_PERIOD));
                // The study's far limb reads the cycle 45 ms earlier, modulo and all: that
                // synthetic loop is exactly what the gallery mode is, and why the living
                // body reads a real episode instead.
                let far = hunt_state((t - FAR_LAG_SECONDS).rem_euclid(HUNT_PERIOD));
                Channels {
                    waves: [(0.22, HUNT_PERIOD), (0.0, 1.0)],
                    pulses: [(1.0, 3.0), (0.0, 1.0)],
                    pulse_gain: 0.38 + 0.92 * h.charge,
                    blinks: [(0.0, 1.0), (0.0, 1.0)],
                    blink_extra: h.blink,
                    reach: h.reach,
                    far_reach: far.reach,
                    compress: h.compress,
                    lunge: h.lunge,
                    accent: h.accent,
                    ..rest
                }
            }
            Mode::Bud => Channels {
                waves: [(0.4, 6.5), (0.0, 1.0)],
                pulses: [(1.0, 3.6), (0.0, 1.0)],
                pulse_gain: 0.8,
                blinks: [(1.0, BLINK_PERIOD_BUD), (0.0, 1.0)],
                tail_lift: 1.0,
                cocoon: Some(1.0),
                ..rest
            },
        }
    }

    /// The semantic pose as channels.
    ///
    /// **Normative**, with `m = clamp(movement, 0, 1)` (NaN ⇒ 0), `a = attack_channels(pose.attack)`,
    /// `k = 1 − WAVE_HUSH · a.hush`: waves `[(0.55 (1 − m) k, 5.5), (1.15 m k, 2.4)]`, pulses
    /// `[(1 − m, 3.0), (m, 2.3)]`, gain `1 − a.hush · (0.62 − 0.92 a.charge)`, blinks `[(1 − m,
    /// BLINK_PERIOD_REST), (m, BLINK_PERIOD_MOVE)]`, `blink_extra = a.blink`, gait `(1.2, m)`,
    /// reach/far/compress/lunge/accent from `a`, cocoon `Some(smoothstep(p / COCOON_REVEAL))`
    /// for `Some(p)` (NaN ⇒ 0) and `None` otherwise, `tail_lift` = that reveal (0 without a
    /// cocoon), `gut = clamp(gut, 0, 1)` (NaN ⇒ 0), `t = ambient` (non-finite ⇒ 0).
    ///
    /// Consequences the tests hold it to: `movement = 0` with no attack, no gut and no cocoon
    /// is texel-identical to `Mode::Rest`; `movement = 1` likewise to `Mode::Move`; a held
    /// phase never strikes on its own; no escrow, no cocoon; no gut, no breath.
    pub fn living(pose: &LivingPose) -> Channels {
        let m = unit(pose.movement);
        let a = attack_channels(pose.attack.as_ref());
        // An attack hushes the ambient wave and re-gains the chain; at full hush the two
        // reproduce the study's hunt figures (0.55 · 0.4 = 0.22, and 1 − (0.62 − 0.92 c) =
        // 0.38 + 0.92 c).
        let k = 1.0 - WAVE_HUSH * a.hush;
        let reveal = pose.cocoon.map(|p| smoothstep(p / COCOON_REVEAL));
        Channels {
            t: if pose.ambient.is_finite() {
                pose.ambient
            } else {
                0.0
            },
            waves: [(0.55 * (1.0 - m) * k, 5.5), (1.15 * m * k, 2.4)],
            pulses: [(1.0 - m, 3.0), (m, 2.3)],
            pulse_gain: 1.0 - a.hush * (0.62 - 0.92 * a.charge),
            blinks: [(1.0 - m, BLINK_PERIOD_REST), (m, BLINK_PERIOD_MOVE)],
            blink_extra: a.blink,
            gait: 1.2,
            gait_amount: m,
            reach: a.near_reach,
            far_reach: a.far_reach,
            compress: a.compress,
            lunge: a.lunge,
            accent: a.accent,
            // The tail lifts to clear the cocoon exactly as far as the cocoon is revealed.
            tail_lift: reveal.unwrap_or(0.0),
            cocoon: reveal,
            gut: unit(pose.gut),
        }
    }
}

impl Lanternjaw {
    /// The living body's parts: `rasterize(&Channels::living(pose), out)`.
    pub fn parts_living(&self, pose: &LivingPose, out: &mut Vec<Part>) {
        self.rasterize(&Channels::living(pose), out);
    }

    /// Draw the living body at whole-rig `scale` ([`SCALE_MIN`]`..=`[`SCALE_MAX`]; anything else,
    /// or a non-finite scale, is a configuration error that panics in every build — the
    /// adapter clamps its mapping to the admitted range): [`Lanternjaw::parts_living`] then one
    /// [`cubarium_render::stamp_rig_scaled`]`(canvas, anchor, heading, &[(rig_parts, 1.0)],
    /// scale, opacity, scratch)`. Everything scales together: offsets, pivots, the lattice, the
    /// lunge, the limbs and the query radius, so `effectors(scale)` names where the scaled
    /// claws and jaw are drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_living(
        &self,
        canvas: &mut Canvas,
        anchor: SurfacePoint,
        heading: Vec2,
        pose: &LivingPose,
        scale: f64,
        opacity: f32,
        parts: &mut Vec<Part>,
        scratch: &mut Vec<PixelImage>,
    ) {
        assert!(
            scale.is_finite() && (SCALE_MIN..=SCALE_MAX).contains(&scale),
            "Lanternjaw scale {scale} is outside the admitted {SCALE_MIN}..={SCALE_MAX}"
        );
        self.parts_living(pose, parts);
        let rig: Vec<RigPart<'_>> = parts.iter().map(Part::rig_part).collect();
        debug_assert!(
            cubarium_render::rig_radius(&[(&rig, 1.0)]) <= QUERY_RADIUS_MAX,
            "the rig needs a query radius past QUERY_RADIUS_MAX"
        );
        cubarium_render::stamp_rig_scaled(
            canvas,
            anchor,
            heading,
            &[(&rig, 1.0)],
            scale,
            opacity,
            scratch,
        );
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

    /// Twenty instants that straddle every keyframe of the study's strike as well as
    /// ordinary fractions: the refactor's own fixture.
    const INSTANTS: [f64; 20] = [
        0.0, 0.333, 1.25, 3.16, 3.22, 3.28, 3.34, 3.44, 3.54, 5.9, 7.7, 11.99, 0.0417, 0.7183,
        2.0891, 2.9737, 4.4429, 5.0613, 6.6271, 9.3847,
    ];

    /// The refactor's identity: the gallery is the channel rasterizer driven by the study
    /// producer, texel for texel, not merely "close".
    #[test]
    fn parts_is_rasterize_of_the_study_channels() {
        let rig = Lanternjaw::new();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        let mut texels = 0usize;
        for mode in Mode::ALL {
            for t in INSTANTS
                .into_iter()
                .chain((0..120).map(|f| f64::from(f) / 10.0))
            {
                rig.parts(t, mode, &mut a);
                rig.rasterize(&Channels::study(t, mode), &mut b);
                same(&a, &b, "parts is rasterize of the study channels");
                texels += a
                    .iter()
                    .map(|p| p.sprite.width() * p.sprite.height())
                    .sum::<usize>();
            }
        }
        assert!(texels > 500_000, "only {texels} texels compared");
    }

    /// `Channels::living` with no attack, no meal and no escrow *is* the gallery's `Rest` at
    /// movement 0 and its `Move` at movement 1 — the same texels, so every silhouette and
    /// palette property of the study still describes the living body.
    #[test]
    fn the_living_body_at_rest_and_in_motion_is_the_gallery() {
        let rig = Lanternjaw::new();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for t in INSTANTS {
            for (movement, mode) in [(0.0, Mode::Rest), (1.0, Mode::Move)] {
                let pose = LivingPose {
                    ambient: t,
                    movement,
                    attack: None,
                    gut: 0.0,
                    cocoon: None,
                };
                rig.parts_living(&pose, &mut a);
                rig.parts(t, mode, &mut b);
                same(&a, &b, "the living body is the gallery mode");
            }
        }
    }

    /// No escrow, no cocoon; no gut, no breath — whatever else the pose says. Both are
    /// *exact*: the cocoon is skipped and the gut breath adds a literal zero.
    #[test]
    fn without_a_meal_or_an_escrow_nothing_is_added_to_the_body() {
        let rig = Lanternjaw::new();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        let quiet = LivingPose {
            ambient: 2.5,
            movement: 0.3,
            attack: None,
            gut: 0.0,
            cocoon: None,
        };
        rig.parts_living(&quiet, &mut a);
        // A handling phase whose gut is empty, and satiety without an escrow: same picture
        // as the same body with no phase at all once the hush has released.
        rig.parts_living(
            &LivingPose {
                gut: 0.0,
                cocoon: None,
                ..quiet
            },
            &mut b,
        );
        same(&a, &b, "no gut and no escrow");
        // A real meal and a real escrow both change it, so the test is not vacuous.
        rig.parts_living(&LivingPose { gut: 1.0, ..quiet }, &mut b);
        assert!(differs(&a, &b), "a real gut should breathe");
        rig.parts_living(
            &LivingPose {
                cocoon: Some(1.0),
                ..quiet
            },
            &mut b,
        );
        assert!(differs(&a, &b), "a funded escrow should show a cocoon");
        // And `Some(0)` paints no cocoon at all: the reveal starts from nothing.
        rig.parts_living(
            &LivingPose {
                cocoon: Some(0.0),
                ..quiet
            },
            &mut b,
        );
        let unlifted = Channels::living(&LivingPose {
            cocoon: Some(0.0),
            ..quiet
        });
        assert_eq!(
            unlifted.tail_lift, 0.0,
            "an unrevealed cocoon does not lift the tail"
        );
        same(&a, &b, "an escrow at progress 0");
    }

    fn differs(a: &[Part], b: &[Part]) -> bool {
        a.iter().zip(b).any(|(p, q)| {
            (0..p.sprite.height() as i32).any(|y| {
                (0..p.sprite.width() as i32).any(|x| p.sprite.texel(x, y) != q.sprite.texel(x, y))
            })
        })
    }

    /// The study's `hunt_state` is the real schedule with `Windup { D = T_SNAP − T_COIL }`,
    /// `Strike { D = E = T_OPEN − T_SNAP }` and `Recovering` chained by their displayed
    /// reaches — so the two agree over the whole gesture, not only at its ends.
    ///
    /// One documented exception: `hunt_state` carries its accent envelope on past `T_OPEN`
    /// (it is still at plateau there and only finishes at `T_SNAP − 0.02 + ACCENT_SECONDS =
    /// 3.44`), whereas the real schedule finishes that envelope during the **Strike's
    /// settlement** — which the study, recoiling the instant it arrives, has no room for —
    /// and `Recovering` is documented to carry no accent at all. The accent is therefore
    /// compared over the windup's and the strike's own spans.
    #[test]
    fn the_phase_schedule_is_the_studys_hunt_state() {
        let windup = |elapsed: f64| AttackEpisode {
            phase: AttackPhase::Windup,
            elapsed,
            duration: T_SNAP - T_COIL,
            from: Reach::FOLDED,
        };
        let strike_from = attack_channels(Some(&windup(T_SNAP - T_COIL))).reach();
        let strike = |elapsed: f64| AttackEpisode {
            phase: AttackPhase::Strike,
            elapsed,
            duration: T_OPEN - T_SNAP,
            from: strike_from,
        };
        let recover_from = attack_channels(Some(&strike(T_OPEN - T_SNAP))).reach();
        let recovering = |elapsed: f64| AttackEpisode {
            phase: AttackPhase::Recovering,
            elapsed,
            duration: T_END - T_OPEN,
            from: recover_from,
        };
        let close = |a: f64, b: f64, what: &str| {
            assert!((a - b).abs() <= 1e-9, "{what}: {a} vs {b}");
        };
        let mut checked = 0usize;
        // 1 ms steps through the whole articulated gesture, plus its four keyframes exactly.
        for step in 0..=440 {
            let t_h = T_COIL + f64::from(step) / 1000.0;
            let (episode, accent_too) = if t_h < T_SNAP {
                (windup(t_h - T_COIL), t_h <= T_SNAP - 0.02)
            } else if t_h <= T_OPEN {
                (strike(t_h - T_SNAP), true)
            } else {
                (recovering(t_h - T_OPEN), false)
            };
            let h = hunt_state(t_h);
            let c = attack_channels(Some(&episode));
            close(c.near_reach, h.reach, &format!("reach at {t_h}"));
            close(c.compress, h.compress, &format!("compress at {t_h}"));
            close(c.lunge, h.lunge, &format!("lunge at {t_h}"));
            close(c.charge, h.charge, &format!("charge at {t_h}"));
            if accent_too {
                close(c.accent, h.accent, &format!("accent at {t_h}"));
            }
            checked += 1;
        }
        assert_eq!(checked, 441);
        // The keyframes themselves, with the strike still owning its settlement boundary.
        for (t_h, episode) in [
            (T_COIL, windup(0.0)),
            (T_SNAP, strike(0.0)),
            (T_OPEN, strike(T_OPEN - T_SNAP)),
            (T_END, recovering(T_END - T_OPEN)),
        ] {
            let h = hunt_state(t_h);
            let c = attack_channels(Some(&episode));
            close(c.near_reach, h.reach, &format!("keyframe reach at {t_h}"));
            close(
                c.compress,
                h.compress,
                &format!("keyframe compress at {t_h}"),
            );
            close(c.lunge, h.lunge, &format!("keyframe lunge at {t_h}"));
            close(c.accent, h.accent, &format!("keyframe accent at {t_h}"));
        }
        // The one post-strike blink lands where the study's does.
        close(
            attack_channels(Some(&recovering(T_END + 0.3 + 0.14 - T_OPEN))).blink,
            hunt_state(T_END + 0.3 + 0.14).blink,
            "the post-recoil blink",
        );
        // And no phase is a phase at all.
        assert_eq!(attack_channels(None), AttackChannels::default());
        assert_eq!(attack_channels(None).reach(), Reach::FOLDED);
    }

    /// A real one-second strike: the cocked entry pose is held through the paid approach, the
    /// extension is the final [`EXTEND_SECONDS`], and full contact is reached **at** the
    /// settlement boundary and held past it. No recoil ever comes from the strike itself.
    #[test]
    fn a_real_strike_holds_its_entry_pose_then_settles_at_full_extension() {
        let from = Reach {
            near: -0.35,
            far: -0.35,
            compress: 1.7,
            lunge: 0.0,
            charge: 1.0,
        };
        let strike = |elapsed: f64| {
            attack_channels(Some(&AttackEpisode {
                phase: AttackPhase::Strike,
                elapsed,
                duration: 1.0,
                from,
            }))
        };
        for t in [0.0, 0.4, 0.8, 0.87, 0.88] {
            let c = strike(t);
            assert_eq!(
                c.near_reach, from.near,
                "the entry pose should be held at {t}"
            );
            assert_eq!(c.compress, from.compress, "compress at {t}");
            assert_eq!(c.lunge, from.lunge, "lunge at {t}");
        }
        // The accent is one continuous envelope that opens its 20 ms *before* the extension —
        // inside the hold, exactly where the study opens it before `T_SNAP` — so it rises
        // from 0 rather than entering at `envelope(0.02 / ACCENT_SECONDS) ≈ 0.103`, and it is
        // still exactly 0 through the approach proper.
        let hold = 1.0 - EXTEND_SECONDS;
        for t in [0.0, 0.4, 0.8, hold - 0.02] {
            assert_eq!(
                strike(t).accent,
                0.0,
                "no accent during the paid approach, at {t}"
            );
        }
        for t in [0.87, 0.88, 0.94, 1.0, 1.1] {
            let want = envelope((t - (hold - 0.02)) / ACCENT_SECONDS);
            assert!(
                (strike(t).accent - want).abs() < 1e-12,
                "the accent at {t} is {}, not the envelope's {want}",
                strike(t).accent
            );
        }
        assert!(strike(0.87).accent > 0.0, "the envelope has opened by 0.87");
        assert_eq!(
            strike(1.5).accent,
            0.0,
            "the accent finishes during the settlement"
        );
        // Mid-extension it is genuinely on the way, not a jump.
        let mid = strike(0.94);
        assert!(
            mid.near_reach > from.near && mid.near_reach < 1.0,
            "mid {mid:?}"
        );
        for t in [1.0, 1.5, 8.0] {
            let c = strike(t);
            let e = Reach::EXTENDED;
            assert!(
                (c.near_reach - e.near).abs() < 1e-12,
                "near at {t}: {}",
                c.near_reach
            );
            assert!(
                (c.compress - e.compress).abs() < 1e-12,
                "compress at {t}: {}",
                c.compress
            );
            assert!(
                (c.lunge - e.lunge).abs() < 1e-12,
                "lunge at {t}: {}",
                c.lunge
            );
            assert_eq!(c.hush, 1.0, "a strike hushes the ambient body at {t}");
        }
        // Eight seconds in — longer than the study's whole cycle — it has still not struck
        // twice and has still not recoiled.
        assert!(strike(8.0).near_reach >= 1.0 - 1e-12);
    }

    /// A windup held far past its own duration cocks and stays cocked: no extension, no
    /// contact, no synthetic strike.
    #[test]
    fn a_held_windup_never_extends() {
        for t in [0.6, 1.0, 6.5, 8.0, 60.0] {
            let c = attack_channels(Some(&AttackEpisode {
                phase: AttackPhase::Windup,
                elapsed: t,
                duration: 0.6,
                from: Reach::FOLDED,
            }));
            assert!(
                (c.near_reach - -0.35).abs() < 1e-12,
                "at {t}: near {}",
                c.near_reach
            );
            assert!(c.far_reach <= 0.0, "at {t}: far {}", c.far_reach);
            assert_eq!(c.lunge, 0.0, "at {t}");
            assert_eq!(c.accent, 0.0, "at {t}");
            assert!(
                (c.compress - 1.7).abs() < 1e-12,
                "at {t}: compress {}",
                c.compress
            );
        }
    }

    /// An interrupted motion recoils from the reach that was actually on screen.
    #[test]
    fn recovering_starts_from_the_reach_it_was_handed() {
        let from = Reach {
            near: 0.4,
            far: 0.4,
            compress: -0.1,
            lunge: 0.44,
            charge: 0.5,
        };
        let episode = |elapsed: f64| AttackEpisode {
            phase: AttackPhase::Recovering,
            elapsed,
            duration: 5.0,
            from,
        };
        let start = attack_channels(Some(&episode(0.0)));
        assert_eq!(
            start.near_reach, 0.4,
            "the recoil starts from the displayed reach"
        );
        assert_eq!(start.far_reach, 0.4, "and so does the far limb");
        assert_eq!(start.compress, -0.1);
        assert_eq!(start.lunge, 0.44);
        // Negative elapsed is the same instant, never a rewind into the previous phase.
        assert_eq!(attack_channels(Some(&episode(-1.0))), start);
        let done = attack_channels(Some(&episode(RECOIL_SECONDS)));
        assert!(
            done.near_reach.abs() < 1e-12 && done.lunge.abs() < 1e-12,
            "{done:?}"
        );
        // Folded stillness for the rest of the recovery, and the hush stays on.
        for t in [0.5, 2.0, 4.9, 9.0] {
            let c = attack_channels(Some(&episode(t)));
            assert!(
                c.near_reach.abs() < 1e-12 && c.lunge.abs() < 1e-12,
                "at {t}: {c:?}"
            );
            assert_eq!(c.hush, 1.0, "at {t}");
        }
        // `Handling` recoils identically but releases the hush over the documented second.
        let handling = |elapsed: f64| AttackEpisode {
            phase: AttackPhase::Handling,
            elapsed,
            ..episode(elapsed)
        };
        assert_eq!(attack_channels(Some(&handling(0.0))).near_reach, 0.4);
        assert_eq!(attack_channels(Some(&handling(RECOIL_SECONDS))).hush, 1.0);
        assert_eq!(
            attack_channels(Some(&handling(RECOIL_SECONDS + HANDLING_RELEASE_SECONDS))).hush,
            0.0
        );
        let half = attack_channels(Some(&handling(RECOIL_SECONDS + 0.5))).hush;
        assert!(
            (half - 0.5).abs() < 1e-12,
            "the release is a smoothstep: {half}"
        );
    }

    /// The far forelimb is the near one 45 ms of **attack** time ago, and before the episode
    /// began it is whatever the previous episode last displayed — never a modulo of the
    /// presentation clock and never a replayed earlier attack.
    #[test]
    fn the_far_forelimb_lags_the_near_one_by_far_lag_seconds() {
        let from = Reach {
            near: -0.2,
            far: 0.31,
            compress: 0.5,
            lunge: 0.2,
            charge: 0.4,
        };
        for phase in [
            AttackPhase::Windup,
            AttackPhase::Strike,
            AttackPhase::Recovering,
            AttackPhase::Handling,
        ] {
            let at = |elapsed: f64| {
                attack_channels(Some(&AttackEpisode {
                    phase,
                    elapsed,
                    duration: 1.0,
                    from,
                }))
            };
            // Before the lag has elapsed, the far limb holds the reach it was handed.
            for t in [0.0, 0.02, 0.0449] {
                assert_eq!(
                    at(t).far_reach,
                    from.far,
                    "{phase:?} at {t} should hold from.far"
                );
            }
            for t in [FAR_LAG_SECONDS, 0.3, 0.9, 1.0, 2.0] {
                let far = at(t).far_reach;
                let near_before = at(t - FAR_LAG_SECONDS).near_reach;
                assert!(
                    (far - near_before).abs() < 1e-15,
                    "{phase:?} at {t}: far {far} should be the near reach at {}: {near_before}",
                    t - FAR_LAG_SECONDS
                );
            }
        }
    }

    /// The gut breath is a *breath*: a sub-pixel swell of template columns 4..=8 and of the
    /// parts that ride them (the abdomen hull piece, its lanterns' bloom in `Glow`, and the
    /// one walking leg based on column 8), and of nothing else. The tail, the thorax, the
    /// head and both forelimbs — which hang off the head's column — never move with a meal.
    #[test]
    fn the_gut_breath_swells_the_abdomen_columns_and_nothing_else() {
        let rig = Lanternjaw::new();
        let (mut empty, mut full) = (Vec::new(), Vec::new());
        // A quarter of the breath period past zero is the swell's peak.
        let pose = LivingPose {
            ambient: GUT_BREATH_SECONDS / 4.0,
            movement: 0.0,
            attack: None,
            gut: 0.0,
            cocoon: None,
        };
        rig.parts_living(&pose, &mut empty);
        rig.parts_living(&LivingPose { gut: 0.8, ..pose }, &mut full);
        let mut worst = [0.0f32; 8];
        for (i, (a, b)) in empty.iter().zip(&full).enumerate() {
            for y in 0..a.sprite.height() as i32 {
                for x in 0..a.sprite.width() as i32 {
                    let (p, q) = (a.sprite.texel(x, y), b.sprite.texel(x, y));
                    for c in 0..4 {
                        worst[i] = worst[i].max((p[c] - q[c]).abs());
                    }
                }
            }
        }
        eprintln!(
            "gut breath 0.8 at its peak ({} px): worst texel change per part {:?}",
            GUT_BREATH_PX * 0.8,
            PartName::ALL.iter().zip(worst).collect::<Vec<_>>()
        );
        for (name, w) in PartName::ALL.into_iter().zip(worst) {
            let rides = matches!(
                name,
                PartName::Abdomen | PartName::Glow | PartName::Underside
            );
            if rides {
                assert!(
                    w > 0.01,
                    "{name:?} rides the abdomen columns but moved by {w}"
                );
            } else {
                assert_eq!(w, 0.0, "{name:?} must not move with a meal");
            }
        }
    }

    /// `Channels::living` reproduces the study's hunt gains at full hush, which is what makes
    /// one hushed body and the gallery's hunt the same kind of picture.
    #[test]
    fn a_full_hush_reproduces_the_studys_hunt_wave_and_chain_gain() {
        let pose = LivingPose {
            ambient: 1.0,
            movement: 0.0,
            attack: Some(AttackEpisode {
                phase: AttackPhase::Windup,
                elapsed: 0.3,
                duration: 0.6,
                from: Reach::FOLDED,
            }),
            gut: 0.0,
            cocoon: None,
        };
        let ch = Channels::living(&pose);
        let a = attack_channels(pose.attack.as_ref());
        assert_eq!(a.hush, 1.0);
        assert!(
            (ch.waves[0].0 - 0.22).abs() < 1e-15,
            "hushed wave {:?}",
            ch.waves
        );
        assert!(
            (ch.pulse_gain - (0.38 + 0.92 * a.charge)).abs() < 1e-15,
            "chain gain {}",
            ch.pulse_gain
        );
        // Released, the ambient body is back: no attack at all is the rest wave and gain 1.
        let quiet = Channels::living(&LivingPose {
            attack: None,
            ..pose
        });
        assert_eq!(quiet.waves[0].0, 0.55);
        assert_eq!(quiet.pulse_gain, 1.0);
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
                assert!(
                    radius <= QUERY_RADIUS_MAX,
                    "{mode:?} frame {frame}: radius {radius}"
                );
            }
        }
        // Non-vacuity: the budgets are actually approached, not trivially satisfied.
        assert!(
            worst_extent > 5.0 && worst_radius > 12.0,
            "{worst_extent} / {worst_radius}"
        );
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
        assert_eq!(
            offsets,
            vec![-7.0, -3.0, 2.0, 6.0],
            "left column dx + 2 per piece"
        );
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
        assert!(
            hunt > 0.2,
            "the hunt's accent should reach the warm colour, got {hunt}"
        );
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
        assert!(
            largest > 0.0 && largest <= 0.31,
            "largest accent step {largest}"
        );
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
            let chosen: Vec<RigPart<'_>> = parts
                .iter()
                .filter(|p| keep(p.name))
                .map(Part::rig_part)
                .collect();
            let mut canvas = Canvas::new();
            stamp_rig(
                &mut canvas,
                anchor,
                heading,
                &[(&chosen, 1.0)],
                1.0,
                &mut Vec::new(),
            );
            canvas
        };
        let whole = subset(&|_| true);
        let without_near = subset(&|n| n != PartName::NearLimb);
        let hull = subset(&|n| n.layer() == 2);
        let near = subset(&|n| n == PartName::NearLimb);
        let light = |c: &Canvas, x: u8, y: u8| {
            c.get(Face::Front, x, y)
                .into_iter()
                .map(f64::from)
                .sum::<f64>()
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
        assert!(
            over_hull >= 3,
            "the near limb changed only {over_hull} hull pixels"
        );

        // The brightest near-limb pixel is the claw, ahead of the hull's own front.
        let (bx, by) = (0..64u8)
            .flat_map(|x| (0..64u8).map(move |y| (x, y)))
            .max_by(|&a, &b| {
                light(&near, a.0, a.1)
                    .partial_cmp(&light(&near, b.0, b.1))
                    .unwrap()
            })
            .unwrap();
        assert!(
            bx >= 43,
            "the claw should reach past the jaw, but the brightest pixel is at {bx}"
        );
        assert_eq!(
            light(&hull, bx, by),
            0.0,
            "the claw at ({bx}, {by}) is past the hull, so no hull pixel is there"
        );
    }
}
