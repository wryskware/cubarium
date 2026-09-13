//! Ground cover, water, and rain rendering.

use super::*;

// --- Ground cover --------------------------------------------------------------------
//
// Between the plants the ground is not bare glow: each band has a tileable 8×8 texture
// (`art/PLANTS.md` "Ground cover") laid on an 8-pixel lattice, fading in with the same
// density that grows the band's plants. Review-tunable from a viewing session.

/// Opacity of a ground tile where its band's density is 1. Review-tunable; drop it first
/// (toward 0.18) if the lattices read as a grid at 1×.
pub const GROUND_OPACITY: f32 = 0.30;
/// Lattice pitch of the ground cover in pixels; the tiles are 8×8 with pivot (4, 4), so
/// pitch 8 tiles the face without gaps.
pub const GROUND_LATTICE: u8 = 8;
/// Seeds the per-lattice-point phase hash.
const GROUND_SEED: u64 = 0x6772_6F75_6E64_0001;

/// The opacity of a band's ground cover at density `t`.
///
/// **Normative**: `GROUND_OPACITY · clamp((t − t₀) / (1 − t₀), 0, 1)` with `t₀` the band's
/// first stage threshold, so the texture appears exactly where the band's plants begin
/// to grow and a quiet world (every cell under its first threshold) shows none — which
/// keeps the decided M2 image intact above the horizon in a quiet world. `NaN` is 0.
pub fn ground_opacity(t: f64, band: Band) -> f32 {
    let t0 = stage_thresholds(band)[0];
    if t.is_nan() || t0 >= 1.0 {
        return 0.0;
    }
    (((t - t0) / (1.0 - t0)).clamp(0.0, 1.0) as f32) * GROUND_OPACITY
}

/// The pixel centers of the ground lattice on one face: `x ≡ 4 (mod 8)`, `y ≡ 4 (mod 8)`,
/// 64 points.
pub fn ground_points(face: Face) -> impl Iterator<Item = (Face, u8, u8)> {
    let half = GROUND_LATTICE / 2;
    (0..FACE_SIZE as u8 / GROUND_LATTICE).flat_map(move |j| {
        (0..FACE_SIZE as u8 / GROUND_LATTICE)
            .map(move |i| (face, i * GROUND_LATTICE + half, j * GROUND_LATTICE + half))
    })
}

/// How much of a lattice point belongs to the band whose tile it draws: the soil weight
/// for the soil tile, its complement for the foliage and canopy tiles, so the texture
/// cross-fades through the horizon with the ground under it.
pub fn ground_weight(face: Face, x: u8, y: u8, band: Band) -> f32 {
    let w = soil_weight(face, x, y);
    match band {
        Band::Soil => w,
        _ => 1.0 - w,
    }
}

/// A stable per-lattice-point offset into the tile's breath, in `[0, seconds)`, so a face
/// of texture does not blink in lockstep.
pub fn ground_phase_of(face: Face, x: u8, y: u8, seconds: f64) -> f64 {
    if !(seconds.is_finite() && seconds > 0.0) {
        return 0.0;
    }
    let key = (face.index() as u64) << 16 | u64::from(x) << 8 | u64::from(y);
    let mut hash = SplitMix64::new(GROUND_SEED ^ key);
    hash.next_f64() * seconds
}

/// The pose of a ground tile at a presentation time ([`present_seconds`]).
///
/// **Normative**: exactly [`Clip::sample`]'s looping rule applied to the tile's frames —
/// `u = (seconds mod length) / length · n`, the samples `floor(u)` and `(floor(u) + 1) mod
/// n`, blended by `fract(u)`, so the breath wraps last → first without a step. A tile with
/// one frame, a nonsense length or a non-finite time is its first frame held still.
pub fn ground_pose(tile: &GroundTile, seconds: f64) -> Pose<'_> {
    let n = tile.frames.len();
    if n < 2 || !seconds.is_finite() || !(tile.seconds.is_finite() && tile.seconds > 0.0) {
        return Pose::still(&tile.frames[0]);
    }
    let u = seconds.rem_euclid(tile.seconds) / tile.seconds * n as f64;
    let i = (u.floor() as usize).min(n - 1);
    Pose {
        first: &tile.frames[i],
        second: &tile.frames[(i + 1) % n],
        mix: (u - i as f64) as f32,
    }
}

// --- Water ---------------------------------------------------------------------------
//
// The pools, streams and rain of `design/water.md`, drawn from the world's per-cell depth
// and this tick's rain. Water is drawn source-over the ground and under the plants, so
// reeds stand in it and bodies wade over it. Every constant is review-tunable.

/// Shallow water: electric blue. Review-tunable within the Outrun family.
pub const WATER_LOW_SRGB: u32 = 0x001E_9BF2;
/// Deep water (depth ≥ 1): electric cyan. Review-tunable within the Outrun family.
pub const WATER_HIGH_SRGB: u32 = 0x0042_C5F8;
/// Depth (d) at which the water film covers `1 − 1/e` of the ground: a 0.05 film is
/// faint, a pool of 1 is solid. Review-tunable.
pub const WATER_FILM: f64 = 0.6;
/// Brightness of the water color at coverage 1. Review-tunable.
pub const WATER_BRIGHT: f32 = 0.55;
/// Shimmer amplitude, as a fraction of [`WATER_BRIGHT`]. Review-tunable.
pub const WATER_SHIMMER: f32 = 0.08;
/// Shimmer period in simulated seconds. Review-tunable.
pub const WATER_SHIMMER_SECONDS: f64 = 2.5;
/// Seeds the per-pixel shimmer phase.
const WATER_SEED: u64 = 0x7761_7465_7200_0001;
/// Algae in a pool: the water color leans toward this mint where the wet cell holds
/// producers (`design/water.md` "Algae"). Review-tunable within the flora family.
pub const ALGAE_SRGB: u32 = 0x007B_EBC9;
/// How far a fully grown mat pulls the water color toward [`ALGAE_SRGB`]. Review-tunable.
pub const ALGAE_TINT: f32 = 0.6;

/// The water color over a cell whose producer density (fraction of the ramp saturation)
/// is `p_t`: `mix(water_color(w), algae, min(p_t, 1) · ALGAE_TINT)`. The soil ground itself
/// still draws no lawn; the mat shows only as a tint on the pool.
pub fn algae_water_color(w: f64, p_t: f64) -> [f32; 3] {
    let base = water_color(w);
    let t = if p_t.is_nan() {
        0.0
    } else {
        p_t.clamp(0.0, 1.0) as f32 * ALGAE_TINT
    };
    present::mix(base, *ALGAE_COLOR, t)
}

static ALGAE_COLOR: LazyLock<[f32; 3]> = LazyLock::new(|| present::srgb_linear(ALGAE_SRGB));

/// The coverage of the water layer at depth `w`: `1 − exp(−w / WATER_FILM)`, 0 for a dry
/// or nonsense depth.
pub fn water_coverage(w: f64) -> f32 {
    if !(w.is_finite() && w > 0.0) {
        return 0.0;
    }
    (1.0 - (-w / WATER_FILM).exp()) as f32
}

/// The water color at depth `w`: blue at the surface film, cyan at depth 1 and beyond.
pub fn water_color(w: f64) -> [f32; 3] {
    let (low, high) = *WATER_RAMP;
    let t = if w.is_nan() {
        0.0
    } else {
        w.clamp(0.0, 1.0) as f32
    };
    present::mix(low, high, t)
}

static WATER_RAMP: LazyLock<([f32; 3], [f32; 3])> = LazyLock::new(|| {
    (
        present::srgb_linear(WATER_LOW_SRGB),
        present::srgb_linear(WATER_HIGH_SRGB),
    )
});

/// The shimmer phase of a pixel, in `[0, 2π)`, from a hash of its position: a fixed
/// pattern that the time term slides through.
pub fn water_phase(face: Face, x: u8, y: u8) -> f64 {
    WATER_PHASE[weight_index(face, x, y)]
}

static WATER_PHASE: LazyLock<Box<[f64]>> = LazyLock::new(|| {
    let mut phases = vec![0.0f64; 5 * FACE_PIXELS];
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let key = (face.index() as u64) << 16 | u64::from(x) << 8 | u64::from(y);
                phases[weight_index(face, x, y)] =
                    SplitMix64::new(WATER_SEED ^ key).next_f64() * std::f64::consts::TAU;
            }
        }
    }
    phases.into_boxed_slice()
});

/// The water brightness at a presentation time ([`present_seconds`]) for a pixel of
/// shimmer phase `phase`: `WATER_BRIGHT · (1 + WATER_SHIMMER · sin(2π · seconds /
/// WATER_SHIMMER_SECONDS + phase))`. Simulated time only: a paused world holds its glints,
/// the glints slide continuously between ticks, and the pattern repeats exactly every
/// `WATER_SHIMMER_SECONDS`.
pub fn water_brightness(seconds: f64, phase: f64) -> f32 {
    let cycle = (seconds / WATER_SHIMMER_SECONDS).fract();
    WATER_BRIGHT * (1.0 + WATER_SHIMMER * (std::f64::consts::TAU * cycle + phase).sin() as f32)
}

/// The seam-aware one-pixel box filter of a cell field at a pixel: the own cell's value
/// weighted 4, each existing pixel neighbor's cell weighted 1, normalized over what is
/// present (so the open rim does not darken). Exactly `cubarium_render::draw_field`'s
/// filter.
pub(super) fn filtered_at(field: &ScalarField, face: Face, x: u8, y: u8) -> f64 {
    let mut sum = field.get(cell_of(&SurfacePoint::pixel_center(face, x, y))) * 4.0;
    let mut divisor = 4.0;
    for edge in Edge::ALL {
        if let Some((nf, nx, ny)) = pixel_neighbor(face, x, y, edge) {
            sum += field.get(cell_of(&SurfacePoint::pixel_center(nf, nx, ny)));
            divisor += 1.0;
        }
    }
    sum / divisor
}

/// The water layer: for every pixel with filtered depth `w > 0`, `px = color(w) · b · a +
/// px · (1 − a)` with `a = `[`water_coverage`]`(w)` and `b = `[`water_brightness`]; the
/// color is [`algae_water_color`] with the pixel's own cell's producer density over
/// `saturation`, so a pool with a mat reads mint rather than pure blue.
pub(super) fn draw_water(
    canvas: &mut Canvas,
    water: &ScalarField,
    producer: &ScalarField,
    saturation: f64,
    seconds: f64,
) {
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let w = filtered_at(water, face, x, y);
                let a = water_coverage(w);
                if a <= 0.0 {
                    continue;
                }
                let p_t = if saturation.is_finite() && saturation > 0.0 {
                    producer.get(cell_of(&SurfacePoint::pixel_center(face, x, y))) / saturation
                } else {
                    0.0
                };
                let c = algae_water_color(w, p_t);
                let b = water_brightness(seconds, water_phase(face, x, y));
                let under = canvas.get(face, x, y);
                canvas.set(
                    face,
                    x,
                    y,
                    std::array::from_fn(|i| c[i] * b * a + under[i] * (1.0 - a)),
                );
            }
        }
    }
}

// --- Rain ----------------------------------------------------------------------------

/// Rain streak color: near-white cyan. Review-tunable.
pub const RAIN_SRGB: u32 = 0x00B8_F0FF;
/// A streak's opacity at rain rate ≥ 1 d/s. Review-tunable.
pub const RAIN_OPACITY: f32 = 0.5;
/// Streaks per unit of rain rate in a cell (`ceil(rain · RAIN_DENSITY)`). Review-tunable.
pub const RAIN_DENSITY: f64 = 3.0;
/// Most streaks one cell may show at once.
pub const RAIN_MAX_STREAKS: usize = 6;
/// How fast a streak falls down a side face, in pixels per simulated second.
pub const RAIN_SPEED: f64 = 20.0;
/// A streak's fall wraps within its cell every this many simulated seconds.
pub const RAIN_PERIOD: f64 = 0.4;
/// On the level top face a streak is a sparkle, on for this long of each period.
pub const RAIN_BLINK: f64 = 0.15;
/// Seeds the per-streak origin hash.
const RAIN_SEED: u64 = 0x7261_696E_0000_0001;

/// How many streaks a cell shows at rain rate `rain`: `ceil(rain · RAIN_DENSITY)`, capped
/// at [`RAIN_MAX_STREAKS`], 0 for no rain.
pub fn rain_streaks(rain: f32) -> usize {
    if !(rain.is_finite() && rain > 0.0) {
        return 0;
    }
    ((f64::from(rain) * RAIN_DENSITY).ceil() as usize).clamp(1, RAIN_MAX_STREAKS)
}

/// The stable sub-cell origin `(dx, dy) ∈ [0, 4)²` of streak `k` of a cell.
pub fn rain_origin(cell: CellId, k: usize) -> (u8, u8) {
    let mut hash = SplitMix64::new(RAIN_SEED ^ (cell.index() as u64) << 8 ^ k as u64);
    ((hash.next_u64() % 4) as u8, (hash.next_u64() % 4) as u8)
}

/// How far a streak has fallen at simulated time `seconds`: `RAIN_SPEED · (seconds mod
/// RAIN_PERIOD)` pixels.
pub fn rain_fall(seconds: f64) -> f64 {
    if !seconds.is_finite() {
        return 0.0;
    }
    RAIN_SPEED * seconds.rem_euclid(RAIN_PERIOD)
}

/// How bright a top-face sparkle is at a presentation time ([`present_seconds`]).
///
/// **Normative**: with `u = seconds mod `[`RAIN_PERIOD`], a raised cosine `0.5 · (1 −
/// cos(2π · u / RAIN_BLINK))` while `u < `[`RAIN_BLINK`], 0 after it; a non-finite time is
/// 0. It peaks at 1 halfway through the blink and is continuous everywhere, so a sparkle
/// swells and dies instead of switching on.
pub fn rain_blink(seconds: f64) -> f32 {
    if !seconds.is_finite() || !(RAIN_BLINK > 0.0) {
        return 0.0;
    }
    let u = seconds.rem_euclid(RAIN_PERIOD);
    if u >= RAIN_BLINK {
        return 0.0;
    }
    (0.5 * (1.0 - (std::f64::consts::TAU * u / RAIN_BLINK).cos())) as f32
}

/// Whether a top-face sparkle is lit at all: `rain_blink(seconds) > 0`.
pub fn rain_blink_on(seconds: f64) -> bool {
    rain_blink(seconds) > 0.0
}

/// The pixels streak `k` of a cell marks at a presentation time ([`present_seconds`]),
/// face-local, each with the weight of its coverage.
///
/// **Normative**: on a side face the streak is a 1×2 mark at the *continuous* downhill
/// position `s = origin + `[`rain_fall`]`(seconds)`. With `φ = fract(rain_fall)` and the
/// head pixel at the wrapped integer position (`(origin + floor(fall)).rem_euclid(4)`
/// within the cell's four pixels), the marks are the head at `1 − φ`, one pixel downhill
/// at 1 and two pixels downhill at `φ`, each dropped if it leaves the face (the two
/// trailing pixels are not wrapped). Their weights sum to 2 whenever all three are on the
/// face, so a streak's light is constant as it falls and the fall reads as smooth rather
/// than as a pixel step. On the top face it is a single pixel at the origin weighted by
/// [`rain_blink`], omitted when that is 0.
pub fn rain_marks(cell: CellId, k: usize, seconds: f64) -> Vec<((u8, u8), f32)> {
    let (dx, dy) = rain_origin(cell, k);
    let x0 = i32::from(cell.cx()) * 4;
    let y0 = i32::from(cell.cy()) * 4;
    let Some(up) = up_of(cell) else {
        let blink = rain_blink(seconds);
        return if blink > 0.0 {
            vec![(
                ((x0 + i32::from(dx)) as u8, (y0 + i32::from(dy)) as u8),
                blink,
            )]
        } else {
            Vec::new()
        };
    };
    let down = Vec2::new(-up.x, -up.y);
    let fall = rain_fall(seconds);
    let phi = (fall - fall.floor()) as f32;
    let steps = fall.floor() as i32;
    let in_face = |v: i32| (0..FACE_SIZE as i32).contains(&v);
    let mut marks = Vec::with_capacity(3);
    let (vertical, sign) = if down.y.abs() >= down.x.abs() {
        (true, if down.y >= 0.0 { 1 } else { -1 })
    } else {
        (false, if down.x >= 0.0 { 1 } else { -1 })
    };
    let along = if vertical {
        i32::from(dy)
    } else {
        i32::from(dx)
    };
    let head = (along + sign * steps).rem_euclid(4);
    for (offset, weight) in [(0, 1.0 - phi), (1, 1.0), (2, phi)] {
        let step = head + sign * offset;
        let (x, y) = if vertical {
            (x0 + i32::from(dx), y0 + step)
        } else {
            (x0 + step, y0 + i32::from(dy))
        };
        // The head is wrapped within its cell; the two trailing pixels are not, so a
        // streak that reaches the edge of the face simply loses its tail.
        if in_face(x) && in_face(y) {
            marks.push(((x as u8, y as u8), weight));
        }
    }
    marks
}

static RAIN_COLOR: LazyLock<[f32; 3]> = LazyLock::new(|| present::srgb_linear(RAIN_SRGB));

/// Rain: for every cell with rain, its streaks source-over the image, each mark at
/// `min(1, RAIN_OPACITY · min(rain, 1) · weight)` — so a streak sliding between two pixels
/// shares its light between them instead of jumping.
pub(super) fn draw_rain(canvas: &mut Canvas, rain: &[f32], seconds: f64) {
    let color = *RAIN_COLOR;
    for (index, cell) in CellId::all().enumerate() {
        let rate = rain.get(index).copied().unwrap_or(0.0);
        let n = rain_streaks(rate);
        if n == 0 {
            continue;
        }
        let scale = RAIN_OPACITY * rate.min(1.0);
        let face = cell.face();
        for k in 0..n {
            for ((x, y), weight) in rain_marks(cell, k, seconds) {
                let alpha = (scale * weight).min(1.0);
                if alpha <= 0.0 {
                    continue;
                }
                let under = canvas.get(face, x, y);
                canvas.set(
                    face,
                    x,
                    y,
                    std::array::from_fn(|i| color[i] * alpha + under[i] * (1.0 - alpha)),
                );
            }
        }
    }
}
