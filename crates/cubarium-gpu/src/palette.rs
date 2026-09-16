//! The decided Outrun palette and the background passes' constants, in one uniform
//! block.
//!
//! Every number here is transcribed from the CPU presenter — `crates/cubarium/src/
//! present.rs` (floor, producer ramp, detritus), `art_present/habitat.rs` (horizon,
//! soil) and `art_present/environment.rs` (water, algae, rain) — and every colour is
//! the sRGB hex `design/appearance.md` "Palette" was reviewed in, decoded to linear
//! light by the same transfer function `cubarium_render::srgb_decode` uses. The
//! shaders read this block and carry no constants of their own, so a review-tuning
//! session changes one Rust file and both renderers move together.

use bytemuck::{Pod, Zeroable};

/// `0xRRGGBB` to linear light. Bit-identical to `cubarium_render::srgb_decode` on
/// every 8-bit code.
pub fn srgb_linear(hex: u32) -> [f32; 3] {
    let channel = |code: u8| -> f32 {
        let c = f64::from(code) / 255.0;
        let l = if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        l as f32
    };
    [
        channel(((hex >> 16) & 0xFF) as u8),
        channel(((hex >> 8) & 0xFF) as u8),
        channel((hex & 0xFF) as u8),
    ]
}

// --- present.rs ---------------------------------------------------------------------
pub const FLOOR_SRGB: u32 = 0x0012_093A;
pub const FLOOR_BRIGHTNESS: f32 = 0.12;
pub const PRODUCER_LOW_SRGB: u32 = 0x001E_2798;
pub const PRODUCER_HIGH_SRGB: u32 = 0x0042_C5F8;
pub const PRODUCER_SATURATION: f32 = 0.6;
pub const RAMP_MIN_BRIGHTNESS: f32 = 0.06;
pub const RAMP_MAX_BRIGHTNESS: f32 = 0.55;
pub const DETRITUS_SRGB: u32 = 0x0051_0B6D;
pub const DETRITUS_THRESHOLD: f32 = 0.05;
pub const DETRITUS_SCALE: f32 = 1.5;
pub const HUE_MAGENTA_SRGB: u32 = 0x00FF_2AFC;
pub const HUE_CYAN_SRGB: u32 = 0x0042_C6FF;
pub const FEEDING_SRGB: u32 = 0x00FF_9B50;

// --- art_present/habitat.rs ---------------------------------------------------------
/// The horizon: `h = 1 − 2v/H` below this is soil.
pub const SOIL_TOP: f32 = -0.33;
/// Half-width of the soft horizon, in `h`.
pub const HORIZON: f32 = 0.06;
pub const SOIL_SCALE: f32 = 1.5;
pub const SOIL_LOW_SRGB: u32 = 0x0024_1033;
pub const SOIL_HIGH_SRGB: u32 = 0x007A_3B8F;
pub const SOIL_MIN_BRIGHTNESS: f32 = 0.10;
pub const SOIL_MAX_BRIGHTNESS: f32 = 0.32;

// --- art_present/environment.rs -----------------------------------------------------
pub const WATER_LOW_SRGB: u32 = 0x001E_9BF2;
pub const WATER_HIGH_SRGB: u32 = 0x0042_C5F8;
pub const WATER_FILM: f32 = 0.6;
pub const WATER_BRIGHT: f32 = 0.55;
pub const WATER_SHIMMER: f32 = 0.08;
pub const WATER_SHIMMER_SECONDS: f32 = 2.5;
pub const ALGAE_SRGB: u32 = 0x007B_EBC9;
pub const ALGAE_TINT: f32 = 0.6;
pub const RAIN_SRGB: u32 = 0x00B8_F0FF;

/// The uniform block both background passes and the sprite pass bind at set 0,
/// binding 0. `std140`: every `vec3` is padded to 16 bytes, which is why each colour
/// carries its scalar companion in `w`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SceneUniforms {
    /// `(w, h, 1/w, 1/h)` of the raster.
    pub raster: [f32; 4],
    /// `(cells_x, cells_y, scale S, producer_max)`.
    pub grid: [f32; 4],
    /// `(seconds, tick phase f, filter, art scale)`. `seconds` is wrapped into a 3,600 s window
    /// before it reaches the shader so `f32` keeps sub-millisecond phase precision
    /// after days of simulated time.
    pub time: [f32; 4],
    /// Floor colour already multiplied by its brightness; `w` unused.
    pub floor: [f32; 4],
    /// Producer ramp low; `w` = `RAMP_MIN_BRIGHTNESS`.
    pub producer_low: [f32; 4],
    /// Producer ramp high; `w` = `RAMP_MAX_BRIGHTNESS`.
    pub producer_high: [f32; 4],
    /// Detritus fleck colour; `w` = `DETRITUS_SCALE`.
    pub detritus: [f32; 4],
    /// Soil wash low; `w` = `SOIL_MIN_BRIGHTNESS`.
    pub soil_low: [f32; 4],
    /// Soil wash high; `w` = `SOIL_MAX_BRIGHTNESS`.
    pub soil_high: [f32; 4],
    /// Shallow water; `w` = `WATER_FILM`.
    pub water_low: [f32; 4],
    /// Deep water; `w` = `WATER_BRIGHT`.
    pub water_high: [f32; 4],
    /// Algae mint; `w` = `ALGAE_TINT`.
    pub algae: [f32; 4],
    /// `(SOIL_TOP, HORIZON, DETRITUS_THRESHOLD, SOIL_SCALE)`.
    pub bands: [f32; 4],
    /// `(PRODUCER_SATURATION, WATER_SHIMMER, WATER_SHIMMER_SECONDS, bend_substep)`.
    ///
    /// `bend_substep` is 1 when the wind's displacement may land between source texels
    /// (`--gpu-bend-substep`) and 0 when it is rounded to a whole one, which is the
    /// default and the only setting that keeps every texel on an exact `S × S` block.
    pub knobs: [f32; 4],
}

impl SceneUniforms {
    /// The block for one frame of a ring.
    pub fn new(
        layout: crate::RingLayout,
        producer_max: f32,
        seconds: f64,
        f: f32,
        bend_substep: bool,
        filter_bilinear: bool,
        art_scale: f32,
    ) -> SceneUniforms {
        let rgb = |hex: u32, w: f32| {
            let c = srgb_linear(hex);
            [c[0], c[1], c[2], w]
        };
        let floor = srgb_linear(FLOOR_SRGB);
        SceneUniforms {
            raster: [
                layout.w as f32,
                layout.h as f32,
                1.0 / layout.w as f32,
                1.0 / layout.h as f32,
            ],
            grid: [
                layout.cells_x() as f32,
                layout.cells_y() as f32,
                layout.scale as f32,
                producer_max,
            ],
            // 3,600 simulated seconds is 20 whole periods of the 2.5 s shimmer and
            // 9,000 of the 0.4 s rain fall, so the wrap is invisible in every clip.
            time: [
                seconds.rem_euclid(3600.0) as f32,
                f,
                if filter_bilinear { 1.0 } else { 0.0 },
                art_scale,
            ],
            floor: [
                floor[0] * FLOOR_BRIGHTNESS,
                floor[1] * FLOOR_BRIGHTNESS,
                floor[2] * FLOOR_BRIGHTNESS,
                0.0,
            ],
            producer_low: rgb(PRODUCER_LOW_SRGB, RAMP_MIN_BRIGHTNESS),
            producer_high: rgb(PRODUCER_HIGH_SRGB, RAMP_MAX_BRIGHTNESS),
            detritus: rgb(DETRITUS_SRGB, DETRITUS_SCALE),
            soil_low: rgb(SOIL_LOW_SRGB, SOIL_MIN_BRIGHTNESS),
            soil_high: rgb(SOIL_HIGH_SRGB, SOIL_MAX_BRIGHTNESS),
            water_low: rgb(WATER_LOW_SRGB, WATER_FILM),
            water_high: rgb(WATER_HIGH_SRGB, WATER_BRIGHT),
            algae: rgb(ALGAE_SRGB, ALGAE_TINT),
            bands: [SOIL_TOP, HORIZON, DETRITUS_THRESHOLD, SOIL_SCALE],
            knobs: [
                PRODUCER_SATURATION,
                WATER_SHIMMER,
                WATER_SHIMMER_SECONDS,
                if bend_substep { 1.0 } else { 0.0 },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_decode_matches_the_renderer_on_the_whole_eight_bit_lattice() {
        for code in 0..=255u32 {
            let ours = super::srgb_linear(code << 16)[0];
            let theirs = cubarium_srgb_decode(code as u8);
            assert!((ours - theirs).abs() < 1e-7, "code {code}: {ours} vs {theirs}");
        }
    }

    /// `cubarium_render::srgb_decode`, copied so the test does not add a dependency.
    fn cubarium_srgb_decode(encoded: u8) -> f32 {
        let c = f64::from(encoded) / 255.0;
        let l = if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        l as f32
    }
}
