//! Pixel ownership for seam-aware rasterization.

use crate::{ChartPath, Face, SurfacePoint, Vec2};

/// One destination pixel and its owning unfolding relative to an anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelImage {
    pub face: Face,
    pub x: u8,
    pub y: u8,
    /// The pixel center in the anchor's chart coordinates under the owning unfolding.
    /// Renderers evaluate body masks at `local - anchor.chart()` rotated into the body frame.
    pub local: Vec2,
    /// Surface distance from the anchor to the pixel center.
    pub distance: f64,
    pub path: ChartPath,
}

/// Every surface pixel whose center lies within surface distance `radius` of `anchor`,
/// each exactly once, with its shortest valid unfolding (ties by `ChartPath` order).
/// Output is sorted by `(face index, y, x)`; `out` is cleared first and its capacity
/// reused. Panics if `radius > MAX_LOCAL_RADIUS` or the anchor is not canonical.
///
/// Normative: the result equals the brute-force set `{ p : unfold(anchor, p, radius) }`
/// over all 20,480 pixel centers. Implementations may take the single-chart fast path
/// when the anchor is at least `radius` from every chart edge (then every pixel is a
/// direct image), and otherwise iterate the pixels of each chart image's preimage
/// bounding box with chord pre-rejection; the fast path and the general path must agree
/// exactly. A pixel visible through two images (a creature near a top vertex, or a
/// footprint reaching around a corner) keeps only its shortest one, so a stamp gets one
/// contribution per pixel and a shape may show a localized discontinuity at a vertex
/// rather than doubled brightness.
pub fn unfold_pixels(anchor: SurfacePoint, radius: f64, out: &mut Vec<PixelImage>) {
    let _ = (anchor, radius, out);
    todo!("unfold_pixels")
}
