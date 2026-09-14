use cubarium_render::srgb_decode;

use super::envelopes::mix3;

// Palette. The study's own sRGB values, mixed the way the study mixes them (`css` rounds to
// eight bits, so a decoded colour here is exactly the colour the browser showed), decoded
// once into linear light. Only the pixels the study faked with a mix toward its background
// and that read as translucent carry real alpha (see `Lanternjaw::parts`).
// ---------------------------------------------------------------------------------------

/// Premultiplied linear RGBA, the form [`Sprite::from_premultiplied`] takes.
pub(super) type Rgba = [f32; 4];

/// `#0B0525`, the study background: used only as a mix endpoint, never painted.
pub(super) const BG: [f64; 3] = [11.0, 5.0, 37.0];
pub(super) const INDIGO: [f64; 3] = [30.0, 39.0, 152.0];
pub(super) const VIOLET: [f64; 3] = [81.0, 11.0, 109.0];
pub(super) const PINK: [f64; 3] = [255.0, 42.0, 252.0];
pub(super) const CYAN: [f64; 3] = [66.0, 198.0, 255.0];
pub(super) const CYAN_S: [f64; 3] = [66.0, 197.0, 248.0];
/// Brief flash only: it tints the pink at the peak of the strike envelope.
pub(super) const ORANGE: [f64; 3] = [255.0, 155.0, 80.0];

/// An opaque study colour: rounded to eight bits exactly as the study's `css` does, then
/// decoded into linear light. Premultiplied by an alpha of 1 is the colour itself.
pub(super) fn opaque(c: [f64; 3]) -> Rgba {
    let d = decode(c);
    [d[0], d[1], d[2], 1.0]
}

/// A study colour decoded into linear light, without alpha.
pub(super) fn decode(c: [f64; 3]) -> [f32; 3] {
    let byte = |v: f64| srgb_decode(v.round().clamp(0.0, 255.0) as u8);
    [byte(c[0]), byte(c[1]), byte(c[2])]
}

/// A linear colour at coverage `a`, premultiplied.
pub(super) fn with_alpha(c: [f32; 3], a: f64) -> Rgba {
    let a = (a.clamp(0.0, 1.0)) as f32;
    [c[0] * a, c[1] * a, c[2] * a, a]
}

/// A premultiplied lerp, for the cocoon's core (a translucent cyan mixed toward opaque pink).
pub(super) fn lerp4(a: Rgba, b: Rgba, t: f64) -> Rgba {
    let u = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) } as f32;
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * u)
}

/// The study's static shades, decoded once.
#[derive(Clone, Debug)]
pub(super) struct Palette {
    /// Carapace rim and belly line (`o`).
    pub(super) edge: Rgba,
    /// Carapace plate (`c`).
    pub(super) shell: Rgba,
    /// Segment division (`d`).
    pub(super) seam: Rgba,
    /// Head plates (`v`).
    pub(super) plate: Rgba,
    /// The trailing tail fan, translucent: `CYAN_S` at 0.40, its tip at 0.22.
    pub(super) fan: Rgba,
    pub(super) fan_tip: Rgba,
    /// Walking legs.
    pub(super) leg: Rgba,
    /// Linear `CYAN`, for the halo and glow alphas and the cocoon rim.
    pub(super) cyan: [f32; 3],
    /// Linear `CYAN_S`, for the cocoon shell.
    pub(super) cyan_s: [f32; 3],
    /// Linear `PINK`, for the cocoon core.
    pub(super) pink: [f32; 3],
}

impl Palette {
    pub(super) fn decode_once() -> Palette {
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
