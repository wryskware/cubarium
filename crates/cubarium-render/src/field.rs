//! Substrate rendering from a scalar field.

use crate::Canvas;
use cubarium_surface::ScalarField;

/// Add `color · min(value / scale, 1)` for each pixel from its cell (nearest-cell sample).
/// With `filter`, apply a seam-aware one-pixel box filter first: each pixel's value is the
/// mean of its own cell value (weight 4) and the cell values of its four pixel neighbors
/// found through `cubarium_surface::pixel_neighbor` (weight 1 each), normalized over the
/// neighbors actually present so the rim is not darkened. The filter changes presentation
/// only; it never changes the field.
pub fn draw_field(canvas: &mut Canvas, field: &ScalarField, scale: f64, color: [f32; 3], filter: bool) {
    let _ = (canvas, field, scale, color, filter);
    todo!("draw_field")
}
