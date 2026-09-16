//! Substrate rendering from a scalar field.

use crate::{Canvas, PixelCells};
use cube_proto::Face;
use cubarium_surface::{Edge, ScalarField, SurfacePoint, cell_of, pixel_neighbor};

/// Add `color · min(value / scale, 1)` for each pixel from its cell (nearest-cell sample).
/// With `filter`, apply a seam-aware one-pixel box filter first: each pixel's value is the
/// mean of its own cell value (weight 4) and the cell values of its four pixel neighbors
/// found through `cubarium_surface::pixel_neighbor` (weight 1 each), normalized over the
/// neighbors actually present so the rim is not darkened. The filter changes presentation
/// only; it never changes the field.
///
/// The pixel→cell map is recomputed per pixel here. A presenter that draws a field every
/// frame should hold a [`PixelCells`] and call [`draw_field_with`], which is the same
/// image bit for bit.
pub fn draw_field(canvas: &mut Canvas, field: &ScalarField, scale: f64, color: [f32; 3], filter: bool) {
    if scale.is_nan() || scale <= 0.0 {
        return;
    }
    let topo = canvas.topology();
    let world = canvas.scale();
    let at = |face, x, y| {
        field.get(cell_of(topo, world, &SurfacePoint::pixel_center(topo, face, x, y)))
    };
    draw(canvas, scale, color, |face, x, y| {
        let own = at(face, x, y);
        if !filter {
            return own;
        }
        // Weight 4 for the pixel's own cell, 1 for each pixel neighbor's cell, normalized
        // over the neighbors that exist so the open rim is not dark.
        let mut sum = own * 4.0;
        let mut weight = 4.0;
        for edge in Edge::ALL {
            if let Some((nf, nx, ny)) = pixel_neighbor(topo, face, x, y, edge) {
                sum += at(nf, nx, ny);
                weight += 1.0;
            }
        }
        sum / weight
    });
}

/// [`draw_field`] reading the pixel→cell map from a table built once (FW-P's W1).
///
/// **Normative**: bit-identical to [`draw_field`] — [`PixelCells`] tabulates exactly the
/// answers `cell_of` and `pixel_neighbor` give and sums the neighbours in the same order.
/// Panics if the table was built for a different topology or scale than the canvas.
pub fn draw_field_with(
    canvas: &mut Canvas,
    cells: &PixelCells,
    field: &ScalarField,
    scale: f64,
    color: [f32; 3],
    filter: bool,
) {
    if scale.is_nan() || scale <= 0.0 {
        return;
    }
    assert!(
        cells.fits(canvas.topology(), canvas.scale()),
        "the pixel-cell table is for {:?} at S = {}, the canvas for {:?} at S = {}",
        cells.topology(),
        cells.scale().world(),
        canvas.topology(),
        canvas.scale().world(),
    );
    draw(canvas, scale, color, |face, x, y| {
        if filter {
            cells.filtered(field, face, x, y)
        } else {
            cells.value(field, face, x, y)
        }
    });
}

/// The shared per-pixel loop: whatever the value comes from, the light is the same.
fn draw(canvas: &mut Canvas, scale: f64, color: [f32; 3], value: impl Fn(Face, u16, u16) -> f64) {
    let width = canvas.width();
    for &face in canvas.charts() {
        for y in canvas.rows_of(face) {
            for x in 0..width {
                let t = (value(face, x, y) / scale).min(1.0);
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
    use cube_proto::Face;
    use cubarium_surface::{CellId, FieldGraph, Scale, Topology, diffuse};

    fn sample(canvas: &Canvas) -> Vec<f32> {
        let mut v = Vec::with_capacity(5 * 64 * 64);
        for face in Face::ALL {
            for y in 0..64u16 {
                for x in 0..64u16 {
                    v.push(canvas.get(face, x, y)[0]);
                }
            }
        }
        v
    }

    #[test]
    fn the_filter_preserves_a_constant_field_everywhere() {
        let field = ScalarField::constant(Topology::Cube, Scale::ONE, 3.0);
        let mut filtered = Canvas::cube();
        draw_field(&mut filtered, &field, 6.0, [1.0, 1.0, 1.0], true);
        let mut plain = Canvas::cube();
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
        let mut field = ScalarField::zeros(Topology::Cube, Scale::ONE);
        field.set(CellId::new(Topology::Cube, Scale::ONE, Face::Front, 4, 4), 100.0);
        let mut canvas = Canvas::cube();
        draw_field(&mut canvas, &field, 6.0, [0.12, 0.5, 0.2], false);
        // The 4x4 pixels of that cell are saturated; everything else is black.
        let mut lit = 0;
        for face in Face::ALL {
            for y in 0..64u16 {
                for x in 0..64u16 {
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
        let mut field = ScalarField::zeros(Topology::Cube, Scale::ONE);
        field.set(CellId::new(Topology::Cube, Scale::ONE, Face::Front, 4, 4), 6.0);
        let before = field.clone();
        let mut canvas = Canvas::cube();
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

    /// The tabulated map draws exactly what the recomputed one draws, filtered and not,
    /// on both topologies.
    #[test]
    fn the_table_draws_the_same_image_bit_for_bit() {
        for (topo, world) in [
            (Topology::Cube, Scale::ONE),
            (Topology::Ring { w: 96, h: 48 }, Scale::ONE),
            (Topology::Ring { w: 96, h: 48 }, Scale::new(2.0)),
        ] {
            let mut f = ScalarField::zeros(topo, world);
            for (i, v) in f.values.iter_mut().enumerate() {
                *v = (i as f64 * 0.7).sin().abs() * 9.0 + (i % 5) as f64 * 1e-9;
            }
            let cells = PixelCells::new(topo, world);
            for filter in [false, true] {
                let mut plain = Canvas::new(topo, world);
                draw_field(&mut plain, &f, 6.0, [0.12, 0.5, 0.2], filter);
                let mut tabled = Canvas::new(topo, world);
                draw_field_with(&mut tabled, &cells, &f, 6.0, [0.12, 0.5, 0.2], filter);
                assert_eq!(plain.pixels(), tabled.pixels(), "{topo:?} filter={filter}");
                assert!(plain.pixels().iter().any(|p| *p != [0.0; 3]), "the pass drew");
            }
        }
    }

    #[test]
    fn a_nonpositive_scale_draws_nothing() {
        let field = ScalarField::constant(Topology::Cube, Scale::ONE, 3.0);
        let mut canvas = Canvas::cube();
        draw_field(&mut canvas, &field, 0.0, [1.0; 3], true);
        assert!(sample(&canvas).iter().all(|&v| v == 0.0));
    }

    #[test]
    fn a_diffused_deposit_reaches_more_than_one_face() {
        let graph = FieldGraph::new(Topology::Cube, Scale::ONE);
        let mut field = ScalarField::zeros(Topology::Cube, Scale::ONE);
        let mut scratch = ScalarField::zeros(Topology::Cube, Scale::ONE);
        cubarium_surface::deposit(Topology::Cube, Scale::ONE, 
            &mut field,
            SurfacePoint::new(Face::Front, 62.0, 32.0),
            10.0,
            40.0,
        );
        for _ in 0..20 {
            diffuse(&mut field, &mut scratch, &graph, 0.15);
        }
        let mut canvas = Canvas::cube();
        draw_field(&mut canvas, &field, 6.0, [0.12, 0.5, 0.2], true);
        let mut faces = std::collections::HashSet::new();
        for face in Face::ALL {
            for y in 0..64u16 {
                for x in 0..64u16 {
                    if canvas.get(face, x, y) != [0.0; 3] {
                        faces.insert(face);
                    }
                }
            }
        }
        assert!(faces.contains(&Face::Front) && faces.contains(&Face::Right), "{faces:?}");
    }
}
