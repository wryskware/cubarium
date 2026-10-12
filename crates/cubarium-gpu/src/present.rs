//! Shared safe last pass and source contract; CPU window fitting is unchanged.
#[cfg(test)]
use crate::render::PresentTransform;
pub use crate::wgpu_backend::{FrameSource, PresentPass, TargetSlot};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowFit {
    /// Raster pixels per window pixel: below 1 for an upscale, above 1 for a downscale.
    /// The same on both axes; the limiting axis fills the window exactly.
    pub step: f32,
    /// Where the raster's top-left corner lands, in window pixels (fractional on the
    /// barred axis, so the picture is centred exactly).
    pub corner: (f32, f32),
    /// The window pixel the picture starts on, and its size in window pixels: the
    /// pixels any of whose footprint is raster. Everything else is the black bars.
    pub origin: (u32, u32),
    pub size: (u32, u32),
    /// True when the swapchain is a UNORM format and the shader must encode sRGB itself.
    pub encode_srgb: bool,
}

impl WindowFit {
    /// The fit of a `raster` into a `window`, both in pixels. A window of zero area fits
    /// nothing and the caller does not present to it.
    pub fn fit(raster: (u32, u32), window: (u32, u32), encode_srgb: bool) -> WindowFit {
        let (rw, rh) = (f64::from(raster.0.max(1)), f64::from(raster.1.max(1)));
        let (ww, wh) = (f64::from(window.0.max(1)), f64::from(window.1.max(1)));
        // The smallest step that fits both axes: the long side fills the window.
        let s = (rw / ww).max(rh / wh);
        let corner = |r: f64, w: f64| 0.5 * (w - r / s);
        let (cx, cy) = (corner(rw, ww), corner(rh, wh));
        // Pixels whose footprint overlaps the raster at all.
        let run = |c: f64, r: f64, w: f64| {
            let first = c.floor().max(0.0);
            let end = (c + r / s).ceil().min(w);
            (first as u32, (end - first).max(1.0) as u32)
        };
        let (ox, sx) = run(cx, rw, ww);
        let (oy, sy) = run(cy, rh, wh);
        WindowFit {
            step: s as f32,
            corner: (cx as f32, cy as f32),
            origin: (ox, oy),
            size: (sx, sy),
            encode_srgb,
        }
    }

    /// The present pass's push constants: two columns of the window-to-raster matrix,
    /// the offset, the sRGB flag, and the window flag.
    ///
    /// The shader floors `gl_FragCoord` to the pixel's corner; the offset puts the sample
    /// at the pixel's **centre**, `(n + 0.5 - corner) · step` raster pixels in, and the
    /// shader averages the raster over `step` around it.
    pub fn push(&self) -> [f32; 8] {
        let s = self.step;
        let (cx, cy) = self.corner;
        [
            s,
            0.0,
            0.0,
            s,
            (0.5 - cx) * s,
            (0.5 - cy) * s,
            if self.encode_srgb { 1.0 } else { 0.0 },
            1.0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `present.frag`'s own arithmetic along one axis: the raster span window pixel `p`'s
    /// footprint covers, before the shader clamps it to the raster.
    fn footprint(push: &[f32; 8], axis: usize, p: u32) -> (f64, f64) {
        let (step, offset) = (f64::from(push[axis * 3]), f64::from(push[4 + axis]));
        let centre = step * f64::from(p) + offset;
        (centre - 0.5 * step, centre + 0.5 * step)
    }

    /// Whether the shader draws pixel `p` (its footprint overlaps the raster) or a bar.
    fn shows(push: &[f32; 8], axis: usize, p: u32, raster: u32) -> bool {
        let (lo, hi) = footprint(push, axis, p);
        hi > 0.0 && lo < f64::from(raster)
    }

    /// Fits up and down, whole and not: the limiting axis fills the window edge to edge,
    /// the other is centred with equal bars, the footprints tile the raster without gap or
    /// overlap, and the picture's reported run is exactly the pixels that show.
    #[test]
    fn the_raster_fills_the_window_on_its_limiting_axis_with_its_aspect_kept() {
        let cases = [
            ((2304, 1392), (2560, 1600)),
            ((3584, 2176), (2560, 1600)),
            ((3584, 2176), (3840, 2160)),
            ((2048, 1264), (1920, 1200)),
            ((320, 180), (1000, 600)),
            ((3328, 2048), (640, 400)),
        ];
        for (raster, window) in cases {
            let fit = WindowFit::fit(raster, window, false);
            let push = fit.push();
            let r = [raster.0, raster.1];
            let w = [window.0, window.1];
            let fills = |a: usize| {
                let first = footprint(&push, a, 0).0;
                let last = footprint(&push, a, w[a] - 1).1;
                first.abs() < 1e-3 && (last - f64::from(r[a])).abs() < 1e-2
            };
            assert!(
                fills(0) || fills(1),
                "{raster:?} in {window:?}: no axis fills"
            );
            for a in 0..2 {
                let shown: Vec<u32> = (0..w[a]).filter(|&p| shows(&push, a, p, r[a])).collect();
                let (o, n) = ([fit.origin.0, fit.origin.1][a], [fit.size.0, fit.size.1][a]);
                assert_eq!(
                    shown.first().copied(),
                    Some(o),
                    "{raster:?} in {window:?} axis {a}"
                );
                assert_eq!(shown.len() as u32, n, "{raster:?} in {window:?} axis {a}");
                let (before, after) = (o, w[a] - o - n);
                assert!(
                    before.abs_diff(after) <= 1,
                    "axis {a}: bars {before} and {after}"
                );
                for p in 1..w[a] {
                    let gap = footprint(&push, a, p).0 - footprint(&push, a, p - 1).1;
                    assert!(
                        gap.abs() < 1e-3,
                        "axis {a}: footprints {p} and {} part",
                        p - 1
                    );
                }
            }
            // The same step on both axes: the aspect is the raster's.
            assert_eq!(push[0], push[3]);
        }
    }

    /// A window a whole multiple `k` of the raster: every pixel's footprint lies inside
    /// one texel, so the area average is exactly the nearest upscale, `k` pixels a texel.
    #[test]
    fn a_whole_upscale_reads_one_texel_a_pixel() {
        for (raster, k) in [((256u32, 128u32), 3u32), ((320, 180), 1), ((4, 2), 7)] {
            let window = (raster.0 * k, raster.1 * k);
            let fit = WindowFit::fit(raster, window, false);
            let push = fit.push();
            for p in 0..window.0 {
                let (lo, hi) = footprint(&push, 0, p);
                let texel = (p / k) as f64;
                assert!(
                    lo >= texel - 1e-4 && hi <= texel + 1.0 + 1e-4,
                    "k = {k}, p = {p}"
                );
            }
        }
    }

    /// The filter and the bars are the window's alone: the panel's push constants leave
    /// the flag at 0, so `present.frag` runs for it exactly as before.
    #[test]
    fn only_the_window_asks_for_the_window_filter() {
        let panel = PresentTransform {
            factor: 6,
            quarter_turns: 1,
            encode_srgb: false,
        };
        assert_eq!(panel.push((1080, 1920))[7], 0.0);
        assert_eq!(WindowFit::fit((320, 180), (640, 360), false).push()[7], 1.0);
    }
}
