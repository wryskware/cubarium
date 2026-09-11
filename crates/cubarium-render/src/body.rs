//! Seam-aware body stamps.

use crate::Canvas;
use cubarium_surface::{PixelImage, SurfacePoint, Vec2, unfold_pixels};

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
        let mut best = 0.0f64;
        for l in &self.lobes {
            let d = (p - l.offset).length();
            let c = if d <= l.radius - 0.5 {
                1.0
            } else if d >= l.radius + 0.5 {
                0.0
            } else {
                l.radius + 0.5 - d
            };
            if c > best {
                best = c;
            }
        }
        best.clamp(0.0, 1.0)
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
    let Some(h) = heading.normalized() else {
        scratch.clear();
        return;
    };
    // `R(heading)` sends body `+x` to `h` and body `+y` to `h` turned one quarter turn
    // clockwise on screen. Counter-clockwise on screen is `(x, y) -> (y, -x)`
    // (`rotate_heading`), so clockwise is its inverse `(x, y) -> (-y, x)`: with `h`
    // image-right `(1, 0)` the body's `+y` is image-down `(0, 1)`, as documented.
    let perp = Vec2::new(-h.y, h.x);
    unfold_pixels(anchor, shape.extent() + 0.5, scratch);
    let a = anchor.chart();
    for img in scratch.iter() {
        // `R` is orthonormal, so its inverse is its transpose: project onto the axes.
        let d = img.local - a;
        let body = Vec2::new(h.dot(d), perp.dot(d));
        let cov = shape.coverage(body);
        if cov > 0.0 {
            let c = cov as f32;
            canvas.add(img.face, img.x, img.y, [color[0] * c, color[1] * c, color[2] * c]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_proto::Face;
    use std::collections::HashSet;

    fn one_lobe(radius: f64) -> BodyShape {
        BodyShape { lobes: vec![Lobe { offset: Vec2::ZERO, radius }] }
    }

    /// The asymmetric fixture body from the host `body` scene.
    fn asymmetric() -> BodyShape {
        BodyShape {
            lobes: vec![
                Lobe { offset: Vec2::new(0.0, 0.0), radius: 1.6 },
                Lobe { offset: Vec2::new(2.4, 0.0), radius: 1.0 },
                Lobe { offset: Vec2::new(-1.4, 1.3), radius: 0.9 },
            ],
        }
    }

    fn lit(canvas: &Canvas) -> Vec<(Face, u8, u8, f32)> {
        let mut v = Vec::new();
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let p = canvas.get(face, x, y);
                    if p[0] != 0.0 || p[1] != 0.0 || p[2] != 0.0 {
                        v.push((face, x, y, p[0]));
                    }
                }
            }
        }
        v
    }

    fn total(canvas: &Canvas) -> f64 {
        let mut t = 0.0f64;
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    t += f64::from(canvas.get(face, x, y)[0]);
                }
            }
        }
        t
    }

    #[test]
    fn coverage_is_soft_and_takes_the_max_not_the_sum() {
        let s = one_lobe(2.0);
        assert_eq!(s.coverage(Vec2::ZERO), 1.0);
        assert_eq!(s.coverage(Vec2::new(1.5, 0.0)), 1.0);
        assert!((s.coverage(Vec2::new(2.0, 0.0)) - 0.5).abs() < 1e-12);
        assert_eq!(s.coverage(Vec2::new(2.5, 0.0)), 0.0);
        assert_eq!(s.coverage(Vec2::new(3.0, 0.0)), 0.0);
        // Two coincident lobes never sum past 1.
        let two = BodyShape {
            lobes: vec![
                Lobe { offset: Vec2::ZERO, radius: 2.0 },
                Lobe { offset: Vec2::new(0.2, 0.0), radius: 2.0 },
            ],
        };
        for k in 0..40 {
            let p = Vec2::new(k as f64 * 0.15, 0.0);
            assert!(two.coverage(p) <= 1.0);
        }
        assert_eq!(s.extent(), 2.0);
        assert!((asymmetric().extent() - 3.4).abs() < 1e-12);
    }

    /// The body frame: `+x` is the heading, `+y` is the heading turned one quarter turn
    /// clockwise on screen (image-down when the heading is image-right).
    #[test]
    fn body_frame_plus_y_is_clockwise_on_screen() {
        // A tiny lobe offset one pixel along body +y, nothing at the origin.
        let shape = BodyShape { lobes: vec![Lobe { offset: Vec2::new(0.0, 1.0), radius: 0.3 }] };
        let mut scratch = Vec::new();

        // Heading image-right: the lobe must land one pixel image-down.
        let mut canvas = Canvas::new();
        let anchor = SurfacePoint::pixel_center(Face::Front, 32, 32);
        stamp_body(&mut canvas, anchor, Vec2::new(1.0, 0.0), &shape, [1.0; 3], &mut scratch);
        let l = lit(&canvas);
        assert_eq!(l.len(), 1, "expected exactly one lit pixel, got {l:?}");
        assert_eq!((l[0].0, l[0].1, l[0].2), (Face::Front, 32, 33), "chart offset (0, +1)");

        // Heading image-up `(0, -1)`: the lobe must land one pixel image-right.
        let mut canvas = Canvas::new();
        stamp_body(&mut canvas, anchor, Vec2::new(0.0, -1.0), &shape, [1.0; 3], &mut scratch);
        let l = lit(&canvas);
        assert_eq!(l.len(), 1, "expected exactly one lit pixel, got {l:?}");
        assert_eq!((l[0].0, l[0].1, l[0].2), (Face::Front, 33, 32), "chart offset (+1, 0)");
    }

    #[test]
    fn centered_lobe_lights_the_expected_pixels_each_once() {
        let shape = one_lobe(1.6);
        let mut scratch = Vec::new();
        let mut canvas = Canvas::new();
        let anchor = SurfacePoint::pixel_center(Face::Front, 32, 32);
        stamp_body(&mut canvas, anchor, Vec2::new(1.0, 0.0), &shape, [1.0; 3], &mut scratch);

        // Every pixel offered by `unfold_pixels` is offered exactly once.
        let mut seen = HashSet::new();
        for img in &scratch {
            assert!(seen.insert((img.face, img.x, img.y)), "pixel {img:?} offered twice");
        }
        assert_eq!(seen.len(), scratch.len());

        // Coverage > 0 exactly where the pixel-center distance is below radius + 0.5 = 2.1:
        // the 13 offsets with |d| in {0, 1, sqrt 2, 2}.
        let mut want: Vec<(u8, u8)> = Vec::new();
        for dy in -3i32..=3 {
            for dx in -3i32..=3 {
                if ((dx * dx + dy * dy) as f64).sqrt() < 2.1 {
                    want.push(((32 + dx) as u8, (32 + dy) as u8));
                }
            }
        }
        assert_eq!(want.len(), 13);
        let got = lit(&canvas);
        assert_eq!(got.len(), 13, "expected 13 lit pixels, got {}", got.len());
        for (x, y) in want {
            assert!(got.iter().any(|g| g.0 == Face::Front && g.1 == x && g.2 == y), "({x},{y})");
        }
        // The center pixel is fully covered.
        assert_eq!(canvas.get(Face::Front, 32, 32)[0], 1.0);
    }

    #[test]
    fn seam_straddling_stamp_matches_a_mid_face_stamp_in_total_coverage() {
        let shape = asymmetric();
        let heading = Vec2::new(1.0, 0.0);
        let mut scratch = Vec::new();

        let mut mid = Canvas::new();
        stamp_body(
            &mut mid,
            SurfacePoint::new(Face::Front, 31.9, 32.5),
            heading,
            &shape,
            [1.0; 3],
            &mut scratch,
        );
        let mid_faces: HashSet<Face> = lit(&mid).into_iter().map(|p| p.0).collect();
        assert_eq!(mid_faces.len(), 1);

        let mut seam = Canvas::new();
        stamp_body(
            &mut seam,
            SurfacePoint::new(Face::Front, 63.9, 32.5),
            heading,
            &shape,
            [1.0; 3],
            &mut scratch,
        );
        let seam_lit = lit(&seam);
        let faces: HashSet<Face> = seam_lit.iter().map(|p| p.0).collect();
        assert!(faces.contains(&Face::Front), "lit faces {faces:?}");
        assert!(faces.contains(&Face::Right), "lit faces {faces:?}");

        let (a, b) = (total(&mid), total(&seam));
        assert!((a - b).abs() <= 1e-6, "mid {a} vs seam {b}");
        assert!(a > 5.0, "the fixture body should cover several pixels, got {a}");
    }

    #[test]
    fn a_top_vertex_stamp_never_lights_a_pixel_twice() {
        let shape = asymmetric();
        let mut scratch = Vec::new();
        let mut canvas = Canvas::new();
        // The Top/Front/Right corner region: three charts meet.
        stamp_body(
            &mut canvas,
            SurfacePoint::new(Face::Top, 63.5, 63.5),
            Vec2::new(0.6, 0.8),
            &shape,
            [1.0; 3],
            &mut scratch,
        );
        let mut seen = HashSet::new();
        for img in &scratch {
            assert!(seen.insert((img.face, img.x, img.y)), "pixel offered twice at a vertex");
        }
        for (_, _, _, v) in lit(&canvas) {
            assert!(v <= 1.0 + 1e-6, "pixel brighter than full coverage: {v}");
        }
        let faces: HashSet<Face> = lit(&canvas).into_iter().map(|p| p.0).collect();
        assert!(faces.len() >= 2, "a vertex stamp should reach more than one chart: {faces:?}");
    }

    #[test]
    fn a_zero_heading_draws_nothing() {
        let mut canvas = Canvas::new();
        let mut scratch = Vec::new();
        stamp_body(
            &mut canvas,
            SurfacePoint::pixel_center(Face::Front, 10, 10),
            Vec2::ZERO,
            &one_lobe(2.0),
            [1.0; 3],
            &mut scratch,
        );
        assert!(lit(&canvas).is_empty());
    }
}
