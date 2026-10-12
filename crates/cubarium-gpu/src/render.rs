//! Flat renderer and CPU presentation/profiling data.
pub use crate::wgpu_backend::TargetSlot as TargetImage;
pub use crate::wgpu_backend::flat::{RASTER_FORMAT, Renderer};
pub const SCRATCH_SIDE: u32 = 256;
/// How the world raster lands on a target: an integer upscale and a quarter-turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentTransform {
    /// The integer upscale factor `k`. Every pixel of the raster becomes a `k × k`
    /// block, which is what keeps the pixel grid visible on the panel.
    pub factor: u32,
    /// Quarter turns clockwise. The DP-1 panel is portrait (1080 × 1920) and the ring
    /// is landscape, so the board uses 1 (or 3, if the panel is mounted the other way).
    pub quarter_turns: u32,
    /// True when the target attachment is a UNORM format and the shader must encode
    /// sRGB itself.
    pub encode_srgb: bool,
}

impl PresentTransform {
    /// The largest integer upscale of `raster` that fits `target` at this rotation,
    /// or `None` if even 1× does not fit.
    pub fn fit(
        raster: (u32, u32),
        target: (u32, u32),
        quarter_turns: u32,
        encode_srgb: bool,
    ) -> Option<PresentTransform> {
        let (rw, rh) = if quarter_turns % 2 == 0 {
            raster
        } else {
            (raster.1, raster.0)
        };
        let factor = (target.0 / rw).min(target.1 / rh);
        (factor >= 1).then_some(PresentTransform {
            factor,
            quarter_turns,
            encode_srgb,
        })
    }

    /// The push constants: two columns of the panel→raster matrix, then its offset.
    pub(crate) fn push(&self, target: (u32, u32)) -> [f32; 8] {
        let k = 1.0 / self.factor as f32;
        let (pw, ph) = (target.0 as f32, target.1 as f32);
        // col0 = (m00, m10), col1 = (m01, m11): `M · p = col0·p.x + col1·p.y`.
        let (col0, col1, off) = match self.quarter_turns % 4 {
            0 => ([k, 0.0], [0.0, k], [0.0, 0.0]),
            1 => ([0.0, -k], [k, 0.0], [0.0, (pw - 1.0) * k]),
            2 => ([-k, 0.0], [0.0, -k], [(pw - 1.0) * k, (ph - 1.0) * k]),
            _ => ([0.0, k], [-k, 0.0], [(ph - 1.0) * k, 0.0]),
        };
        [
            col0[0],
            col0[1],
            col1[0],
            col1[1],
            off[0],
            off[1],
            if self.encode_srgb { 1.0 } else { 0.0 },
            0.0,
        ]
    }
}

/// Where a frame's sprite fill goes: what the quads cover against what the art in them
/// can actually paint. All three are **raster pixels summed over every instance**, so
/// they count overdraw rather than area of the image.
///
/// Measured only when [`Renderer::fill_profile`] is set (`--gpu-fill-profile`): it is a
/// walk over the frame's instances with a hash lookup each, which is the adapter's own
/// scarce resource and has no business on the shipped path.
#[derive(Clone, Copy, Debug, Default)]
pub struct FillProfile {
    /// What the quads this renderer builds rasterise.
    pub quad_px: f64,
    /// What a quad around the **whole tile** would have rasterised — the fill before
    /// the opaque box, and the ratio worth quoting.
    pub tile_px: f64,
    /// What the instances' **painted** source texels cover: `α > 0` texels only, at the
    /// same scale. The floor no quad can go below.
    pub opaque_px: f64,
    /// Instances counted.
    pub instances: usize,
    /// [`FillProfile::quad_px`] split by [`crate::scene::Layer`], in `LAYERS` order: where the fill
    /// actually goes, which is the question a budget is answered with.
    pub per_layer: [f64; crate::scene::LAYER_COUNT],
}

/// How long one frame took, as the GPU and the CPU each saw it.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTiming {
    /// Timestamp-query milliseconds across the whole command buffer.
    pub gpu_ms: f64,
    /// Wall milliseconds from `begin_command_buffer` to the fence signalling.
    pub submit_ms: f64,
    /// Instances drawn.
    pub instances: usize,
}

pub(crate) fn f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xFF) as i32;
    let mantissa = bits & 0x007F_FFFF;
    if exponent == 0xFF {
        // Inf or NaN.
        return sign | 0x7C00 | if mantissa != 0 { 0x0200 } else { 0 };
    }
    let unbiased = exponent - 127 + 15;
    if unbiased >= 0x1F {
        return sign | 0x7C00;
    }
    if unbiased <= 0 {
        if unbiased < -10 {
            return sign;
        }
        let m = mantissa | 0x0080_0000;
        let shift = (14 - unbiased) as u32;
        let half = (m >> shift) as u16;
        let round = (m >> (shift - 1)) & 1;
        return sign | (half + round as u16);
    }
    let half = ((unbiased as u32) << 10) as u16 | (mantissa >> 13) as u16;
    let round = u16::from(
        (mantissa >> 12) & 1 == 1 && (mantissa & 0x0FFF != 0 || (mantissa >> 13) & 1 == 1),
    );
    sign | (half + round)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_half_float_conversion_round_trips_the_values_a_field_carries() {
        for v in [0.0f32, 1.0, 0.5, 0.1, 1.5, 12.75, 1e-4, 65504.0, -3.25] {
            let back = half_to_f32(f16(v));
            let tolerance = v.abs().max(1.0) * 1e-3;
            assert!((back - v).abs() <= tolerance, "{v} -> {back}");
        }
        assert_eq!(f16(f32::INFINITY), 0x7C00);
        assert_eq!(f16(0.0), 0);
    }

    fn half_to_f32(h: u16) -> f32 {
        let sign = u32::from(h >> 15) << 31;
        let exponent = i32::from((h >> 10) & 0x1F);
        let mantissa = u32::from(h & 0x3FF);
        if exponent == 0 {
            if mantissa == 0 {
                return f32::from_bits(sign);
            }
            return f32::from_bits(sign) + (mantissa as f32) * 2.0f32.powi(-24);
        }
        if exponent == 0x1F {
            return f32::from_bits(sign | 0x7F80_0000 | (mantissa << 13));
        }
        f32::from_bits(sign | (((exponent - 15 + 127) as u32) << 23) | (mantissa << 13))
    }

    #[test]
    fn the_quarter_turn_maps_the_rings_corners_onto_the_portrait_panel() {
        // 320x180 at k = 6 is 1920x1080; the panel is 1080x1920, so one quarter turn.
        let x = PresentTransform {
            factor: 6,
            quarter_turns: 1,
            encode_srgb: false,
        };
        let p = x.push((1080, 1920));
        let map = |px: f32, py: f32| {
            (
                (p[0] * px + p[2] * py + p[4]).floor(),
                (p[1] * px + p[3] * py + p[5]).floor(),
            )
        };
        assert_eq!(
            map(0.0, 0.0),
            (0.0, 179.0),
            "panel top-left is the raster's bottom-left"
        );
        assert_eq!(map(1079.0, 0.0), (0.0, 0.0));
        assert_eq!(map(0.0, 1919.0), (319.0, 179.0));
        assert_eq!(map(1079.0, 1919.0), (319.0, 0.0));
    }

    #[test]
    fn fit_takes_the_largest_integer_upscale_that_the_panel_holds() {
        let x = PresentTransform::fit((320, 180), (1080, 1920), 1, false).unwrap();
        assert_eq!(x.factor, 6);
        let x = PresentTransform::fit((640, 360), (1080, 1920), 1, false).unwrap();
        assert_eq!(x.factor, 3);
        assert!(PresentTransform::fit((2000, 2000), (1080, 1920), 0, false).is_none());
    }
}
