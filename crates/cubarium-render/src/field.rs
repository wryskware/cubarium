//! Substrate rendering from a scalar field.

use crate::Canvas;
use cube_proto::{FACE_SIZE, Face};
use cubarium_surface::{Edge, ScalarField, SurfacePoint, cell_of, pixel_neighbor};

/// Add `color · min(value / scale, 1)` for each pixel from its cell (nearest-cell sample).
/// With `filter`, apply a seam-aware one-pixel box filter first: each pixel's value is the
/// mean of its own cell value (weight 4) and the cell values of its four pixel neighbors
/// found through `cubarium_surface::pixel_neighbor` (weight 1 each), normalized over the
/// neighbors actually present so the rim is not darkened. The filter changes presentation
/// only; it never changes the field.
pub fn draw_field(canvas: &mut Canvas, field: &ScalarField, scale: f64, color: [f32; 3], filter: bool) {
    if scale.is_nan() || scale <= 0.0 {
        return;
    }
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let own = field.get(cell_of(&SurfacePoint::pixel_center(face, x, y)));
                let value = if filter {
                    // Weight 4 for the pixel's own cell, 1 for each pixel neighbor's cell,
                    // normalized over the neighbors that exist so the open rim is not dark.
                    let mut sum = own * 4.0;
                    let mut weight = 4.0;
                    for edge in Edge::ALL {
                        if let Some((nf, nx, ny)) = pixel_neighbor(face, x, y, edge) {
                            sum += field.get(cell_of(&SurfacePoint::pixel_center(nf, nx, ny)));
                            weight += 1.0;
                        }
                    }
                    sum / weight
                } else {
                    own
                };
                let t = (value / scale).min(1.0);
                if t != 0.0 {
                    let t = t as f32;
                    canvas.add(face, x, y, [color[0] * t, color[1] * t, color[2] * t]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_surface::{CellId, FieldGraph, diffuse};

    fn sample(canvas: &Canvas) -> Vec<f32> {
        let mut v = Vec::with_capacity(5 * 64 * 64);
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    v.push(canvas.get(face, x, y)[0]);
                }
            }
        }
        v
    }

    #[test]
    fn the_filter_preserves_a_constant_field_everywhere() {
        let field = ScalarField::constant(3.0);
        let mut filtered = Canvas::new();
        draw_field(&mut filtered, &field, 6.0, [1.0, 1.0, 1.0], true);
        let mut plain = Canvas::new();
        draw_field(&mut plain, &field, 6.0, [1.0, 1.0, 1.0], false);
        let (a, b) = (sample(&filtered), sample(&plain));
        assert_eq!(a.len(), 20_480);
        for (i, (&f, &p)) in a.iter().zip(b.iter()).enumerate() {
            assert!((f - 0.5).abs() < 1e-6, "pixel {i} filtered to {f}, expected 0.5");
            assert!((f - p).abs() < 1e-6, "pixel {i}: filter changed a constant field");
        }
    }

    #[test]
    fn values_clamp_at_the_scale_and_zero_stays_black() {
        let mut field = ScalarField::zeros();
        field.set(CellId::new(Face::Front, 4, 4), 100.0);
        let mut canvas = Canvas::new();
        draw_field(&mut canvas, &field, 6.0, [0.12, 0.5, 0.2], false);
        // The 4x4 pixels of that cell are saturated; everything else is black.
        let mut lit = 0;
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let p = canvas.get(face, x, y);
                    if p != [0.0; 3] {
                        lit += 1;
                        assert!((p[0] - 0.12).abs() < 1e-6 && (p[1] - 0.5).abs() < 1e-6);
                        assert_eq!(face, Face::Front);
                        assert!((16..20).contains(&x) && (16..20).contains(&y));
                    }
                }
            }
        }
        assert_eq!(lit, 16);
    }

    #[test]
    fn the_filter_softens_a_cell_edge_without_touching_the_field() {
        let mut field = ScalarField::zeros();
        field.set(CellId::new(Face::Front, 4, 4), 6.0);
        let before = field.clone();
        let mut canvas = Canvas::new();
        draw_field(&mut canvas, &field, 6.0, [1.0, 1.0, 1.0], true);
        assert_eq!(field.total(), before.total());
        // A pixel just outside the cell now picks up a share.
        assert!(canvas.get(Face::Front, 15, 17)[0] > 0.0);
        // A pixel whose whole neighborhood is inside the cell keeps the full value.
        assert!((canvas.get(Face::Front, 17, 17)[0] - 1.0).abs() < 1e-6);
        // One on the cell's border is softened: (6*4 + 6 + 6 + 6 + 0) / 8 / 6 = 0.875.
        let border = canvas.get(Face::Front, 16, 17)[0];
        assert!((border - 0.875).abs() < 1e-6, "border {border}");
    }

    #[test]
    fn a_nonpositive_scale_draws_nothing() {
        let field = ScalarField::constant(3.0);
        let mut canvas = Canvas::new();
        draw_field(&mut canvas, &field, 0.0, [1.0; 3], true);
        assert!(sample(&canvas).iter().all(|&v| v == 0.0));
    }

    #[test]
    fn a_diffused_deposit_reaches_more_than_one_face() {
        let graph = FieldGraph::new();
        let mut field = ScalarField::zeros();
        let mut scratch = ScalarField::zeros();
        cubarium_surface::deposit(
            &mut field,
            SurfacePoint::new(Face::Front, 62.0, 32.0),
            10.0,
            40.0,
        );
        for _ in 0..20 {
            diffuse(&mut field, &mut scratch, &graph, 0.15);
        }
        let mut canvas = Canvas::new();
        draw_field(&mut canvas, &field, 6.0, [0.12, 0.5, 0.2], true);
        let mut faces = std::collections::HashSet::new();
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    if canvas.get(face, x, y) != [0.0; 3] {
                        faces.insert(face);
                    }
                }
            }
        }
        assert!(faces.contains(&Face::Front) && faces.contains(&Face::Right), "{faces:?}");
    }
}
