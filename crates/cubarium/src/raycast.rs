//! The preview's software cube: an orbit camera and a per-pixel ray cast against the
//! five face planes of the unit cube `[-1, 1]^3`.
//!
//! There is no second geometry table here. A face's plane, its square, and the `(u, v)`
//! inversion all come from `cubarium_surface::face_frame`, the same embedding
//! `SurfacePoint::embed` uses, so the preview cannot drift from the surface crate.
//! The `-Y` plane is simply absent: the cube's bottom is open, and a ray that would only
//! hit it shows background.

use cube_proto::{FACE_SIZE, Face};
use cubarium_surface::{FACE_EXTENT, face_frame};

type V3 = [f64; 3];

#[inline]
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

#[inline]
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 0.0 { scale(a, 1.0 / l) } else { a }
}

/// Tolerance, in unit-cube units, for accepting a hit exactly on a face's border.
const EDGE_EPS: f64 = 1e-9;

/// A surface hit: which chart, and where in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub face: Face,
    /// Chart coordinates in pixel units, `[0, 64]`.
    pub u: f64,
    pub v: f64,
    /// Ray parameter of the hit.
    pub t: f64,
}

impl Hit {
    /// Nearest-pixel sample coordinates.
    pub fn pixel(&self) -> (u8, u8) {
        let px = |c: f64| c.floor().clamp(0.0, (FACE_SIZE - 1) as f64) as u8;
        (px(self.u), px(self.v))
    }
}

/// Invert the `face_frame` embedding for a point known to lie on `face`'s plane: returns
/// the chart coordinates when the point is inside the face square, `None` otherwise.
pub fn invert_face(face: Face, p: V3) -> Option<(f64, f64)> {
    let f = face_frame(face);
    let rel = sub(p, f.center);
    let a = dot(rel, f.tangent_u);
    let b = dot(rel, f.tangent_v);
    if a.abs() > 1.0 + EDGE_EPS || b.abs() > 1.0 + EDGE_EPS {
        return None;
    }
    // `SurfacePoint::embed` uses `a = u / 32 - 1`, `b = v / 32 - 1`.
    let half = FACE_EXTENT / 2.0;
    Some((((a + 1.0) * half).clamp(0.0, FACE_EXTENT), ((b + 1.0) * half).clamp(0.0, FACE_EXTENT)))
}

/// The nearest of the five face planes hit by the ray `origin + t·dir`, `t > 0`.
/// The open bottom has no plane, so a ray through it returns `None`.
pub fn cast(origin: V3, dir: V3) -> Option<Hit> {
    let mut best: Option<Hit> = None;
    for face in Face::ALL {
        let f = face_frame(face);
        // Every face center satisfies `center · normal = 1`, so the plane is `p · n = 1`.
        let nd = dot(dir, f.normal);
        if nd.abs() < 1e-12 {
            continue;
        }
        let t = (1.0 - dot(origin, f.normal)) / nd;
        if t.is_nan() || t <= 1e-9 {
            continue;
        }
        if best.is_some_and(|b| t >= b.t) {
            continue;
        }
        let p = add(origin, scale(dir, t));
        if let Some((u, v)) = invert_face(face, p) {
            best = Some(Hit { face, u, v, t });
        }
    }
    best
}

/// A yaw/pitch orbit camera looking at the cube's center.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// Radians, about `+Y`. Zero looks along `-Z` at the Front face.
    pub yaw: f64,
    /// Radians above the equator, clamped to ±85°.
    pub pitch: f64,
    /// Distance from the origin, in unit-cube units.
    pub distance: f64,
    /// Vertical field of view in radians.
    pub fov: f64,
}

/// Pitch is clamped to this many radians either side of the equator.
pub const MAX_PITCH: f64 = 85.0 * std::f64::consts::PI / 180.0;

impl Default for Camera {
    /// Slightly above the Front/Right corner, as the contract asks.
    fn default() -> Camera {
        Camera {
            yaw: std::f64::consts::FRAC_PI_4,
            pitch: 22.0 * std::f64::consts::PI / 180.0,
            distance: 5.0,
            fov: 38.0 * std::f64::consts::PI / 180.0,
        }
    }
}

impl Camera {
    pub fn position(&self) -> V3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        // yaw 0 puts the camera on `+Z`, facing the Front face; +yaw swings toward `+X`,
        // so yaw = 45° looks at the Front/Right corner.
        scale([cp * sy, sp, cp * cy], self.distance)
    }

    pub fn orbit(&mut self, dyaw: f64, dpitch: f64) {
        self.yaw += dyaw;
        self.pitch = (self.pitch + dpitch).clamp(-MAX_PITCH, MAX_PITCH);
    }

    /// The camera basis: forward (toward the origin), right, and up.
    fn basis(&self) -> (V3, V3, V3) {
        let pos = self.position();
        let forward = norm(scale(pos, -1.0));
        let world_up: V3 = [0.0, 1.0, 0.0];
        let mut right = cross(forward, world_up);
        if dot(right, right) < 1e-12 {
            right = [1.0, 0.0, 0.0];
        }
        let right = norm(right);
        let up = norm(cross(right, forward));
        (forward, right, up)
    }

    /// A perspective ray through normalized viewport coordinates: `sx` and `sy` in
    /// `[-1, 1]`, with `sy` measured image-down.
    pub fn ray(&self, sx: f64, sy: f64) -> (V3, V3) {
        let (forward, right, up) = self.basis();
        let h = (self.fov * 0.5).tan();
        let d = add(forward, add(scale(right, sx * h), scale(up, -sy * h)));
        (self.position(), norm(d))
    }

    /// Fill `out` with one entry per viewport pixel of a `size`×`size` square viewport:
    /// the face and pixel each ray lands on, or `None` for background.
    pub fn trace_viewport(&self, size: usize, out: &mut Vec<Option<(Face, u8, u8)>>) {
        out.clear();
        out.reserve(size * size);
        let inv = 2.0 / size as f64;
        for py in 0..size {
            let sy = (py as f64 + 0.5) * inv - 1.0;
            for px in 0..size {
                let sx = (px as f64 + 0.5) * inv - 1.0;
                let (o, d) = self.ray(sx, sy);
                out.push(cast(o, d).map(|h| {
                    let (x, y) = h.pixel();
                    (h.face, x, y)
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_surface::SurfacePoint;

    /// The contract's required test: the ray-caster's face and `(u, v)` inversion must
    /// round-trip the `face_frame` embedding at the center of every one of the 20,480
    /// surface pixels.
    #[test]
    fn the_ray_caster_inverts_face_frame_for_every_pixel_center() {
        let mut checked = 0u32;
        for face in Face::ALL {
            let n = face_frame(face).normal;
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let p = SurfacePoint::pixel_center(face, x, y);
                    let target = p.embed();

                    // Straight inversion of the embedding.
                    let (u, v) = invert_face(face, target).expect("pixel center is on its face");
                    assert!((u - (f64::from(x) + 0.5)).abs() < 1e-9, "{face:?} ({x},{y}) u={u}");
                    assert!((v - (f64::from(y) + 0.5)).abs() < 1e-9, "{face:?} ({x},{y}) v={v}");

                    // The full cast from outside along the face normal must pick this
                    // face (not a farther plane) and land on this pixel.
                    let origin = add(target, scale(n, 3.0));
                    let hit = cast(origin, scale(n, -1.0)).expect("a ray down the normal hits");
                    assert_eq!(hit.face, face, "{face:?} ({x},{y}) hit {:?}", hit.face);
                    assert_eq!(hit.pixel(), (x, y), "{face:?} ({x},{y}) -> {:?}", hit.pixel());
                    assert!((hit.u - (f64::from(x) + 0.5)).abs() < 1e-9);
                    assert!((hit.v - (f64::from(y) + 0.5)).abs() < 1e-9);
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 20_480);
    }

    #[test]
    fn the_bottom_is_open() {
        // There is no -Y plane, so a ray aimed up the axis passes straight through the
        // open bottom and lands on the *inside* of the Top face instead of a sixth face.
        assert_eq!(cast([0.0, -5.0, 0.0], [0.0, 1.0, 0.0]).map(|h| h.face), Some(Face::Top));
        assert_eq!(cast([0.3, -5.0, -0.4], [0.0, 1.0, 0.0]).map(|h| h.face), Some(Face::Top));
        // Outside the footprint, the ray misses the cube entirely.
        assert!(cast([3.0, -5.0, 0.0], [0.0, 1.0, 0.0]).is_none());
        // Grazing just below the cube: every plane hit falls outside its face square,
        // which is exactly the "hits only where the bottom would be" case.
        assert!(cast([0.0, -1.5, 5.0], [0.0, 0.0, -1.0]).is_none());
        // From above, the Top face is hit.
        let h = cast([0.0, 5.0, 0.0], [0.0, -1.0, 0.0]).expect("top");
        assert_eq!(h.face, Face::Top);
        assert_eq!(h.pixel(), (32, 32));
    }

    #[test]
    fn the_nearest_face_wins_not_the_far_side() {
        // Looking at the Front face from far out on +Z: the Back plane is also on the
        // ray but much farther away.
        let h = cast([0.0, 0.0, 9.0], [0.0, 0.0, -1.0]).expect("front");
        assert_eq!(h.face, Face::Front);
        assert!((h.t - 8.0).abs() < 1e-9);
    }

    #[test]
    fn the_default_camera_looks_at_the_front_right_corner_from_above() {
        let c = Camera::default();
        let p = c.position();
        assert!(p[0] > 0.5, "camera must be on the Right side: {p:?}");
        assert!(p[2] > 0.5, "camera must be on the Front side: {p:?}");
        assert!(p[1] > 0.0, "camera must be above the equator: {p:?}");
        // The center ray hits the cube, on Front or Right.
        let (o, d) = c.ray(0.0, 0.0);
        let h = cast(o, d).expect("the center of the viewport shows the cube");
        assert!(matches!(h.face, Face::Front | Face::Right), "{:?}", h.face);
    }

    #[test]
    fn orbiting_clamps_the_pitch_and_keeps_the_cube_in_view() {
        let mut c = Camera::default();
        c.orbit(0.0, 10.0);
        assert!((c.pitch - MAX_PITCH).abs() < 1e-12);
        c.orbit(0.0, -20.0);
        assert!((c.pitch + MAX_PITCH).abs() < 1e-12);
        // From every yaw the viewport center still shows some face (from below, the
        // open bottom may show through, so only check the upper hemisphere).
        for k in 0..16 {
            let c = Camera { yaw: f64::from(k) * std::f64::consts::TAU / 16.0, ..Camera::default() };
            let (o, d) = c.ray(0.0, 0.0);
            assert!(cast(o, d).is_some(), "yaw index {k}");
        }
    }

    #[test]
    fn a_traced_viewport_has_one_entry_per_pixel_and_some_background() {
        let c = Camera::default();
        let mut hits = Vec::new();
        c.trace_viewport(64, &mut hits);
        assert_eq!(hits.len(), 64 * 64);
        assert!(hits.iter().any(|h| h.is_none()), "the corners must be background");
        let faces: std::collections::HashSet<Face> =
            hits.iter().flatten().map(|&(f, _, _)| f).collect();
        assert!(faces.contains(&Face::Front) && faces.contains(&Face::Right));
        assert!(faces.contains(&Face::Top), "the default view is from above");
    }
}
