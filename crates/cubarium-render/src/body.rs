//! Seam-aware body stamps.

use crate::Canvas;
use cubarium_surface::{PixelImage, SurfacePoint, Vec2};

/// One soft disc in the body frame. The body frame has `+x` along the heading and `+y`
/// to the body's right-hand side as seen on screen when heading is image-right (that is
/// image-down, since chart `+v` is down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lobe {
    pub offset: Vec2,
    pub radius: f64,
}

/// A bounded union of lobes. Extent (max `|offset| + radius`) must stay ≤ 9 pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyShape {
    pub lobes: Vec<Lobe>,
}

impl BodyShape {
    pub fn extent(&self) -> f64 {
        self.lobes.iter().map(|l| l.offset.length() + l.radius).fold(0.0, f64::max)
    }

    /// Coverage of a body-frame point in `[0, 1]`: the maximum over lobes of a soft disc
    /// that is 1 inside `radius - 0.5`, 0 outside `radius + 0.5`, linear between. One
    /// contribution per pixel: lobes take a max, never a sum.
    pub fn coverage(&self, p: Vec2) -> f64 {
        let _ = p;
        todo!("BodyShape::coverage")
    }
}

/// Stamp `shape` at `anchor` with unit `heading` (in the anchor chart) in `color`
/// (linear RGB, multiplied by coverage and added to the canvas).
///
/// Normative: call `unfold_pixels(anchor, shape.extent() + 0.5, scratch)`; for each image
/// compute the body-frame point `R(heading)⁻¹ · (image.local - anchor.chart())` where
/// `R(heading)` maps body `+x` to `heading` and body `+y` to the heading rotated one
/// quarter turn clockwise on screen; add `color · coverage` when coverage > 0. Because
/// each pixel appears once, a body straddling a seam or vertex is never double-lit; the
/// documented localized discontinuity at a top vertex comes from ownership changes.
pub fn stamp_body(canvas: &mut Canvas, anchor: SurfacePoint, heading: Vec2, shape: &BodyShape, color: [f32; 3], scratch: &mut Vec<PixelImage>) {
    let _ = (canvas, anchor, heading, shape, color, scratch);
    todo!("stamp_body")
}
