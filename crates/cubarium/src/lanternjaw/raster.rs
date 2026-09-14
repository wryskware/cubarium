use std::f64::consts::{PI, TAU};

use cubarium_render::{Canvas, RigPart, Sprite, stamp_rig};
use cubarium_surface::{PixelImage, SurfacePoint, Vec2};

use super::envelopes::{
    BOUND_ABOVE, BOUND_BACK, BOUND_BELOW, BOUND_FRONT, COL_COUNT, COL_LEFT, PART_EXTENT_MAX,
    QUERY_RADIUS_MAX, ROW_TOP, ROWS, blink_closure, frac, mix3, smoothstep, unit,
};
use super::living::{COCOON_BREATH_SECONDS, Channels, GUT_BREATH_PX, GUT_BREATH_SECONDS};
use super::model::{Mode, Part, PartName};
use super::palette::{
    BG, CYAN, INDIGO, ORANGE, PINK, Palette, Rgba, VIOLET, decode, lerp4, opaque, with_alpha,
};

// ---------------------------------------------------------------------------------------
// Poses, in body-local pixels, exactly as `fable.js` has them.
// ---------------------------------------------------------------------------------------

const LIMB_SHOULDER: [f64; 2] = [4.0, 1.0];
const LIMB_FOLD: ([f64; 2], [f64; 2]) = ([6.4, 2.2], [4.3, 2.5]);
const LIMB_COCK: ([f64; 2], [f64; 2]) = ([5.8, 1.7], [3.6, 0.7]);
pub(super) const LIMB_STRIKE: ([f64; 2], [f64; 2]) = ([8.4, 1.9], [12.3, 0.6]);
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
pub(super) const CELL_CENTRE: f64 = 0.5;

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
pub(super) const HEAD: usize = 5;
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
