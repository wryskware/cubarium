//! Continuous surface points, the cube face frames, and pixel-level seam neighbors.

use crate::{Edge, Face, Topology, Vec2};

/// A continuous position on the surface: one chart plus local coordinates in pixel units.
///
/// Canonical points satisfy `0 <= u < w` and `0 <= v < h` for the chart's extent
/// ([`Topology::extent`]; 64×64 on the cube). Transient points produced during a sweep
/// may sit exactly on a boundary (`u == w` or `v == h`).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurfacePoint {
    pub face: Face,
    pub u: f64,
    pub v: f64,
}

impl SurfacePoint {
    #[inline]
    pub const fn new(face: Face, u: f64, v: f64) -> SurfacePoint {
        SurfacePoint { face, u, v }
    }

    /// The center `(x + 0.5, y + 0.5)` of pixel `(x, y)` on `face`. Panics if the pixel
    /// is outside the chart's extent.
    pub fn pixel_center(topo: Topology, face: Face, x: u16, y: u16) -> SurfacePoint {
        let (w, h) = topo.extent(face);
        assert!(
            f64::from(x) < w && f64::from(y) < h,
            "pixel ({x}, {y}) out of range for {topo:?}"
        );
        SurfacePoint::new(face, f64::from(x) + 0.5, f64::from(y) + 0.5)
    }

    /// Finite and inside `[0, w) × [0, h)`.
    #[inline]
    pub fn is_canonical(&self, topo: Topology) -> bool {
        let (w, h) = topo.extent(self.face);
        self.u.is_finite()
            && self.v.is_finite()
            && (0.0..w).contains(&self.u)
            && (0.0..h).contains(&self.v)
    }

    /// Map a coordinate equal to the chart extent onto the largest double below it so the
    /// point is canonical. Coordinates outside `[0, extent]` are a caller bug and are
    /// clamped in release builds (debug builds panic).
    pub fn canonicalize(self, topo: Topology) -> SurfacePoint {
        let (w, h) = topo.extent(self.face);
        debug_assert!(
            (0.0..=w).contains(&self.u) && (0.0..=h).contains(&self.v),
            "canonicalize called on {self:?} in {topo:?}"
        );
        let fix = |c: f64, extent: f64| {
            if c >= extent {
                extent.next_down()
            } else if c < 0.0 || c.is_nan() {
                0.0
            } else {
                c
            }
        };
        SurfacePoint::new(self.face, fix(self.u, w), fix(self.v, h))
    }

    /// The pixel containing this point: floor of each coordinate, with a transient
    /// boundary coordinate clamped to the last pixel.
    #[inline]
    pub fn pixel(&self, topo: Topology) -> (u16, u16) {
        let (w, h) = topo.extent(self.face);
        let px = |c: f64, extent: f64| c.floor().clamp(0.0, extent - 1.0) as u16;
        (px(self.u, w), px(self.v, h))
    }

    /// Local coordinates as a vector.
    #[inline]
    pub fn chart(&self) -> Vec2 {
        Vec2::new(self.u, self.v)
    }
}

/// The rigid frame of one face chart on the unit cube (see [`Topology::embed`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceFrame {
    /// Chart center `(u, v) = (32, 32)`.
    pub center: [f64; 3],
    /// Unit 3D direction of `+u`.
    pub tangent_u: [f64; 3],
    /// Unit 3D direction of `+v`.
    pub tangent_v: [f64; 3],
    /// Outward unit normal.
    pub normal: [f64; 3],
}

/// The cube embedding table as explicit vectors: with `a = u/32 - 1` and `b = v/32 - 1`,
///
/// | Face | Position | Tangent +u | Tangent +v | Normal |
/// | --- | --- | --- | --- | --- |
/// | Front | `(a, -b, 1)` | `+X` | `-Y` | `+Z` |
/// | Right | `(1, -b, -a)` | `-Z` | `-Y` | `+X` |
/// | Back | `(-a, -b, -1)` | `-X` | `-Y` | `-Z` |
/// | Left | `(-1, -b, a)` | `+Z` | `-Y` | `-X` |
/// | Top | `(a, 1, b)` | `+X` | `+Z` | `+Y` |
///
/// A ring has no face frames: its chart embeds as a cylinder, not as a plane of the unit
/// cube (see [`Topology::embed`]).
pub const fn face_frame(face: Face) -> FaceFrame {
    match face {
        Face::Front => FaceFrame {
            center: [0.0, 0.0, 1.0],
            tangent_u: [1.0, 0.0, 0.0],
            tangent_v: [0.0, -1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        },
        Face::Right => FaceFrame {
            center: [1.0, 0.0, 0.0],
            tangent_u: [0.0, 0.0, -1.0],
            tangent_v: [0.0, -1.0, 0.0],
            normal: [1.0, 0.0, 0.0],
        },
        Face::Back => FaceFrame {
            center: [0.0, 0.0, -1.0],
            tangent_u: [-1.0, 0.0, 0.0],
            tangent_v: [0.0, -1.0, 0.0],
            normal: [0.0, 0.0, -1.0],
        },
        Face::Left => FaceFrame {
            center: [-1.0, 0.0, 0.0],
            tangent_u: [0.0, 0.0, 1.0],
            tangent_v: [0.0, -1.0, 0.0],
            normal: [-1.0, 0.0, 0.0],
        },
        Face::Top => FaceFrame {
            center: [0.0, 1.0, 0.0],
            tangent_u: [1.0, 0.0, 0.0],
            tangent_v: [0.0, 0.0, 1.0],
            normal: [0.0, 1.0, 0.0],
        },
    }
}

/// The pixel adjacent to `(face, x, y)` across `edge` at pixel resolution, crossing a
/// seam when the pixel lies on that edge of the chart. `None` only across an open rim:
/// the cube's four bottom edges, the ring's top and bottom rows.
///
/// Rules: if the pixel is not on `edge`, the answer is the neighboring pixel inside the
/// same chart. If it is on `edge`, the cube asks `cross_seam(face, edge, t)` with `t` the
/// pixel's along-edge index, and the ring wraps `x` to the other end of the same row.
/// Used by seam-aware pixel filters; never a distance metric.
pub fn pixel_neighbor(
    topo: Topology,
    face: Face,
    x: u16,
    y: u16,
    edge: Edge,
) -> Option<(Face, u16, u16)> {
    let (w, h) = topo.extent(face);
    assert!(
        f64::from(x) < w && f64::from(y) < h,
        "pixel ({x}, {y}) out of range for {topo:?}"
    );
    let (last_x, last_y) = ((w as u16) - 1, (h as u16) - 1);
    let on_edge = match edge {
        Edge::Top => y == 0,
        Edge::Right => x == last_x,
        Edge::Bottom => y == last_y,
        Edge::Left => x == 0,
    };
    if !on_edge {
        return Some(match edge {
            Edge::Top => (face, x, y - 1),
            Edge::Right => (face, x + 1, y),
            Edge::Bottom => (face, x, y + 1),
            Edge::Left => (face, x - 1, y),
        });
    }
    match topo {
        Topology::Cube => {
            let t = match edge {
                Edge::Top | Edge::Bottom => x as u8,
                Edge::Right | Edge::Left => y as u8,
            };
            let (f, nx, ny, _) = crate::cross_seam(face, edge, t)?;
            Some((f, u16::from(nx), u16::from(ny)))
        }
        Topology::Ring { .. } => match edge {
            // The wrap: the same row, the other end. The rims have no neighbour.
            Edge::Right => Some((face, 0, y)),
            Edge::Left => Some((face, last_x, y)),
            Edge::Top | Edge::Bottom => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Scale, SurfacePoint, cross_seam};

    const CUBE: Topology = Topology::Cube;
    const RING: Topology = Topology::Ring { w: 320, h: 180 };

    #[test]
    fn interior_neighbors_stay_in_chart() {
        assert_eq!(
            pixel_neighbor(CUBE, Face::Front, 10, 10, Edge::Top),
            Some((Face::Front, 10, 9))
        );
        assert_eq!(
            pixel_neighbor(CUBE, Face::Front, 10, 10, Edge::Right),
            Some((Face::Front, 11, 10))
        );
        assert_eq!(
            pixel_neighbor(CUBE, Face::Front, 10, 10, Edge::Bottom),
            Some((Face::Front, 10, 11))
        );
        assert_eq!(
            pixel_neighbor(CUBE, Face::Front, 10, 10, Edge::Left),
            Some((Face::Front, 9, 10))
        );
        // On an edge, but asked for a direction that does not leave the chart.
        assert_eq!(
            pixel_neighbor(CUBE, Face::Front, 0, 0, Edge::Bottom),
            Some((Face::Front, 0, 1))
        );
        assert_eq!(
            pixel_neighbor(CUBE, Face::Front, 0, 0, Edge::Right),
            Some((Face::Front, 1, 0))
        );
    }

    #[test]
    fn edge_neighbors_agree_with_cross_seam() {
        for face in Face::ALL {
            for edge in Edge::ALL {
                for t in 0..64u8 {
                    let (x, y) = edge.pixel(t);
                    let got = pixel_neighbor(CUBE, face, u16::from(x), u16::from(y), edge);
                    let want = cross_seam(face, edge, t)
                        .map(|(f, nx, ny, _)| (f, u16::from(nx), u16::from(ny)));
                    assert_eq!(got, want, "{face:?} {edge:?} t={t}");
                }
            }
        }
    }

    #[test]
    fn open_rim_has_no_neighbor() {
        for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
            for x in 0..64u16 {
                assert_eq!(pixel_neighbor(CUBE, face, x, 63, Edge::Bottom), None);
            }
        }
        // Top has no open edge.
        for edge in Edge::ALL {
            for t in 0..64u8 {
                let (x, y) = edge.pixel(t);
                assert!(
                    pixel_neighbor(CUBE, Face::Top, u16::from(x), u16::from(y), edge).is_some()
                );
            }
        }
    }

    #[test]
    fn embedding_matches_pixel_direction() {
        for face in Face::ALL {
            for y in 0..64u16 {
                for x in 0..64u16 {
                    let p = SurfacePoint::pixel_center(CUBE, face, x, y);
                    let want = cube_proto::geometry::pixel_direction(face, x as u8, y as u8);
                    let got = CUBE.embed(Scale::ONE, &p);
                    for k in 0..3 {
                        assert!(
                            (got[k] - want[k]).abs() < 1e-12,
                            "{face:?} ({x},{y}) axis {k}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_ring_wraps_sideways_and_stops_at_the_rims() {
        for y in [0u16, 1, 90, 179] {
            assert_eq!(
                pixel_neighbor(RING, Face::Front, 319, y, Edge::Right),
                Some((Face::Front, 0, y))
            );
            assert_eq!(
                pixel_neighbor(RING, Face::Front, 0, y, Edge::Left),
                Some((Face::Front, 319, y))
            );
            assert_eq!(
                pixel_neighbor(RING, Face::Front, 5, y, Edge::Right),
                Some((Face::Front, 6, y))
            );
        }
        for x in [0u16, 1, 160, 319] {
            assert_eq!(pixel_neighbor(RING, Face::Front, x, 0, Edge::Top), None);
            assert_eq!(
                pixel_neighbor(RING, Face::Front, x, 179, Edge::Bottom),
                None
            );
            assert_eq!(
                pixel_neighbor(RING, Face::Front, x, 1, Edge::Top),
                Some((Face::Front, x, 0))
            );
        }
        // The wrap is reciprocal at every row.
        for y in 0..180u16 {
            let right = pixel_neighbor(RING, Face::Front, 319, y, Edge::Right).expect("wrap");
            assert_eq!(
                pixel_neighbor(RING, right.0, right.1, right.2, Edge::Left),
                Some((Face::Front, 319, y))
            );
        }
    }

    #[test]
    fn ring_points_canonicalize_against_the_ring_extent() {
        let p = SurfacePoint::new(Face::Front, 320.0, 180.0).canonicalize(RING);
        assert_eq!(p.pixel(RING), (319, 179));
        assert!(p.is_canonical(RING));
        assert!(!SurfacePoint::new(Face::Front, 320.0, 10.0).is_canonical(RING));
        assert!(SurfacePoint::new(Face::Front, 300.0, 10.0).is_canonical(RING));
        // 300 is off the cube but on the ring: the extent is the topology's, not a constant.
        assert!(!SurfacePoint::new(Face::Front, 300.0, 10.0).is_canonical(CUBE));
        assert_eq!(
            SurfacePoint::pixel_center(RING, Face::Front, 319, 179),
            SurfacePoint::new(Face::Front, 319.5, 179.5)
        );
    }
}
