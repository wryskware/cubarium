//! Safe surfaces retain an owned window handle for their entire lifetime.
pub use crate::wgpu_backend::window::{WindowFrame, WindowPresenter};

#[cfg(test)]
use crate::present::WindowFit;

/// How a surface's logical coordinates map to swapchain pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowScale {
    /// No scaling protocol (also the Xlib path).
    Unscaled,
    /// wl_surface buffer scale, a positive whole number.
    Integer(u32),
    /// wp_fractional_scale_v1 preferred_scale, in 120ths.
    Fractional(u32),
}

impl WindowScale {
    /// Rounded halfway up, as required by fractional-scale-v1. Integer arithmetic
    /// preserves rounding on odd logical dimensions without float precision loss.
    pub fn physical_size(self, logical: (u32, u32)) -> (u32, u32) {
        let (numerator, denominator) = match self {
            Self::Unscaled => (1, 1),
            Self::Integer(n) => (n.max(1), 1),
            Self::Fractional(n) => (n.max(1), 120),
        };
        let axis = |n| {
            ((u64::from(n) * u64::from(numerator) + denominator / 2) / denominator)
                .clamp(1, u64::from(u32::MAX)) as u32
        };
        (axis(logical.0), axis(logical.1))
    }

    pub fn factor(self) -> f64 {
        match self {
            Self::Unscaled => 1.0,
            Self::Integer(n) => f64::from(n.max(1)),
            Self::Fractional(n) => f64::from(n.max(1)) / 120.0,
        }
    }
}

#[cfg(test)]
mod scale_tests {
    use super::*;

    #[test]
    fn fractional_size_rounds_each_axis_halfway_up() {
        for scale in [120, 150, 180, 210, 240] {
            for logical in [(1, 3), (17, 29), (1280, 800)] {
                let physical = WindowScale::Fractional(scale).physical_size(logical);
                let expected = |n| (f64::from(n) * f64::from(scale) / 120.0).round() as u32;
                assert_eq!(physical, (expected(logical.0), expected(logical.1)));
            }
        }
    }

    #[test]
    fn integer_scale_and_no_protocol_have_exact_sizes() {
        let logical = (37, 23);
        assert_eq!(WindowScale::Unscaled.physical_size(logical), logical);
        for scale in 1..=4 {
            assert_eq!(
                WindowScale::Integer(scale).physical_size(logical),
                (logical.0 * scale, logical.1 * scale)
            );
        }
    }

    #[test]
    fn window_fit_uses_the_scaled_pixel_extent() {
        let logical = (80, 50);
        let physical = WindowScale::Fractional(180).physical_size(logical);
        let raster = (160, 100);
        let fit = WindowFit::fit(raster, physical, false);
        assert_eq!(fit.size, physical);
        assert_eq!(fit.origin, (0, 0));
        assert!((f64::from(fit.step) - f64::from(raster.0) / f64::from(physical.0)).abs() < 1e-6);
    }
}
