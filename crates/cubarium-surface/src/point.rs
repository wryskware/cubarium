//! Continuous surface points, the cube embedding, and pixel-level seam neighbors.

use crate::{Edge, Face, FACE_EXTENT, Vec2};

/// A continuous position on the surface: one chart plus local coordinates in pixel units.
///
/// Canonical points satisfy `0 <= u < 64` and `0 <= v < 64` (finite). Transient points
/// produced during a sweep may sit exactly on a boundary (`u == 64` or `v == 64`).
#[derive(Clone, Copy, Debug, PartialEq)]
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

    /// The center `(x + 0.5, y + 0.5)` of pixel `(x, y)` on `face`. Panics if `x` or `y`
    /// is 64 or more.
    pub fn pixel_center(face: Face, x: u8, y: u8) -> SurfacePoint {
        assert!(x < 64 && y < 64, "pixel ({x}, {y}) out of range");
        SurfacePoint::new(face, f64::from(x) + 0.5, f64::from(y) + 0.5)
    }

    /// Finite and inside `[0, 64)` on both axes.
    #[inline]
    pub fn is_canonical(&self) -> bool {
        self.u.is_finite()
            && self.v.is_finite()
            && (0.0..FACE_EXTENT).contains(&self.u)
            && (0.0..FACE_EXTENT).contains(&self.v)
    }

    /// Map a coordinate equal to 64 onto the largest double below 64 so the point is
    /// canonical. Coordinates outside `[0, 64]` are a caller bug and are clamped in
    /// release builds (debug builds panic).
    pub fn canonicalize(self) -> SurfacePoint {
        debug_assert!(
            (0.0..=FACE_EXTENT).contains(&self.u) && (0.0..=FACE_EXTENT).contains(&self.v),
            "canonicalize called on {self:?}"
        );
        let fix = |c: f64| {
            if c >= FACE_EXTENT {
                FACE_EXTENT.next_down()
            } else if c < 0.0 || c.is_nan() {
                0.0
            } else {
                c
            }
        };
        SurfacePoint::new(self.face, fix(self.u), fix(self.v))
    }

    /// The pixel containing this point: floor of each coordinate, with 64 clamped to 63
    /// for transient boundary points.
    #[inline]
    pub fn pixel(&self) -> (u8, u8) {
        let px = |c: f64| c.floor().clamp(0.0, 63.0) as u8;
        (px(self.u), px(self.v))
    }

    /// Local coordinates as a vector.
    #[inline]
    pub fn chart(&self) -> Vec2 {
        Vec2::new(self.u, self.v)
    }

    /// Position on the unit cube `[-1, 1]^3`, matching `cube_proto::geometry::pixel_direction`
    /// at pixel centers: with `a = u/32 - 1` and `b = v/32 - 1`,
    ///
    /// | Face | Position | Tangent +u | Tangent +v | Normal |
    /// | --- | --- | --- | --- | --- |
    /// | Front | `(a, -b, 1)` | `+X` | `-Y` | `+Z` |
    /// | Right | `(1, -b, -a)` | `-Z` | `-Y` | `+X` |
    /// | Back | `(-a, -b, -1)` | `-X` | `-Y` | `-Z` |
    /// | Left | `(-1, -b, a)` | `+Z` | `-Y` | `-X` |
    /// | Top | `(a, 1, b)` | `+X` | `+Z` | `+Y` |
    ///
    /// Multiply by 32 to convert unit-cube lengths into pixels.
    pub fn embed(&self) -> [f64; 3] {
        let f = face_frame(self.face);
        let a = self.u / 32.0 - 1.0;
        let b = self.v / 32.0 - 1.0;
        [
            f.center[0] + a * f.tangent_u[0] + b * f.tangent_v[0],
            f.center[1] + a * f.tangent_u[1] + b * f.tangent_v[1],
            f.center[2] + a * f.tangent_u[2] + b * f.tangent_v[2],
        ]
    }

    /// A chart tangent vector expressed in unit-cube 3D coordinates (pixel units are
    /// preserved: the result has the same length as `t` scaled by 1/32).
    pub fn embed_tangent(&self, t: Vec2) -> [f64; 3] {
        let f = face_frame(self.face);
        let (a, b) = (t.x / 32.0, t.y / 32.0);
        [
            a * f.tangent_u[0] + b * f.tangent_v[0],
            a * f.tangent_u[1] + b * f.tangent_v[1],
            a * f.tangent_u[2] + b * f.tangent_v[2],
        ]
    }

    /// Squared straight-line 3D chord between the two points, in pixel units squared.
    /// This is a lower bound on squared surface distance, never a metric for
    /// interactions on its own: a chord may pass through the cube.
    pub fn chord_sq(&self, other: &SurfacePoint) -> f64 {
        let a = self.embed();
        let b = other.embed();
        let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) * 1024.0
    }
}

/// The rigid frame of one face chart on the unit cube (see [`SurfacePoint::embed`]).
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

/// The embedding table above as explicit vectors.
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
/// seam through `cross_seam` when the pixel lies on that edge of the chart. `None` only
/// across the open bottom rim. Used by seam-aware pixel filters; never a distance metric.
///
/// Rules: if the pixel is not on `edge`, the answer is the neighboring pixel inside the
/// same chart. If it is on `edge`, the answer is `cross_seam(face, edge, t)` with `t` the
/// pixel's along-edge index, or `None` for a side face's `Edge::Bottom`.
pub fn pixel_neighbor(face: Face, x: u8, y: u8, edge: Edge) -> Option<(Face, u8, u8)> {
    let _ = (face, x, y, edge);
    todo!("pixel_neighbor")
}
