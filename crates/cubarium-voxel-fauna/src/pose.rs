//! **Continuous pose** for a phase-one founder body: an `(x, z)` position in metres on the
//! support layer plus a heading in radians.
//!
//! An [`Animal`](crate::Animal) still stores the support [`Site`] it stands on — that is how
//! the existing heuristic browser moves, one face per step, and it stays. A pose is the
//! finer-grained state the phase-one founders need: P1-B resolves forward effort and turn
//! effort into a movement of `x`/`z`/`heading`, and P1-C samples receptors at sub-voxel
//! positions. In P1-A nothing moves the pose: a placed body keeps it while it pays
//! maintenance and ages.
//!
//! `x` is wrapped by the world width and `z` runs `0..depth`; both are in metres from the
//! origin of the voxel grid, with voxel centres at `(i + 0.5) · voxel_m`. Heading is
//! measured from **+z** (into the habitat, away from the camera) and rotates toward +x, so
//! the forward unit vector is `(sin h, cos h)`.

use cubarium_voxel_flora::Site;
use serde::{Deserialize, Serialize};

/// A body's continuous place on its support layer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    /// Metres along the wrapped `x` axis.
    pub x: f64,
    /// Metres along `z`, `0..depth · voxel_m`.
    pub z: f64,
    /// Heading in radians from +z toward +x.
    pub heading_rad: f64,
}

impl Pose {
    /// The origin, heading along +z.
    pub const ZERO: Pose = Pose {
        x: 0.0,
        z: 0.0,
        heading_rad: 0.0,
    };

    /// The centre of `site`'s top face, heading along +z.
    pub fn at_site(site: Site, voxel_m: f64) -> Pose {
        Pose {
            x: (f64::from(site.x) + 0.5) * voxel_m,
            z: (f64::from(site.z) + 0.5) * voxel_m,
            heading_rad: 0.0,
        }
    }

    /// The column this pose's `(x, z)` falls in: floored to voxel indices. `x` is returned
    /// unwrapped because the strip wraps it; nothing here aliases an out-of-bounds position
    /// onto a valid row. `None` when a component is not finite, `voxel_m` is not positive,
    /// or `z` falls outside `0..depth` — a pose off the strip's front or back is rejected,
    /// never clamped onto a ground row.
    pub fn column(&self, voxel_m: f64, depth: u32) -> Option<(i64, u32)> {
        if !self.is_finite() || !(voxel_m > 0.0) {
            return None;
        }
        let z = (self.z / voxel_m).floor();
        if z < 0.0 || z >= depth as f64 {
            return None;
        }
        Some(((self.x / voxel_m).floor() as i64, z as u32))
    }

    /// The forward unit vector `(sin h, cos h)`.
    pub fn forward(&self) -> (f64, f64) {
        (self.heading_rad.sin(), self.heading_rad.cos())
    }

    /// Whether every component is finite. A pose that is not finite is refused at the
    /// boundary rather than carried into a tick.
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.z.is_finite() && self.heading_rad.is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_site_s_pose_is_its_face_centre_and_reads_back_to_its_column() {
        let site = Site { x: 4, y: 2, z: 7 };
        let p = Pose::at_site(site, 0.25);
        assert_eq!(p.x, 1.125);
        assert_eq!(p.z, 1.875);
        assert_eq!(p.column(0.25, 10), Some((4, 7)));
        assert!(p.is_finite());
        assert_eq!(Pose::ZERO.forward(), (0.0, 1.0));
    }

    /// The column conversion is checked, not clamping: a non-finite pose or a `z` off the
    /// strip is `None`, never silently aliased onto a valid row.
    #[test]
    fn an_out_of_bounds_pose_is_rejected_not_clamped() {
        let mut nan = Pose::at_site(Site { x: 1, y: 1, z: 1 }, 0.25);
        nan.x = f64::NAN;
        assert_eq!(nan.column(0.25, 8), None);

        let mut behind = Pose::ZERO;
        behind.z = -0.01;
        assert_eq!(behind.column(0.25, 8), None);

        let mut beyond = Pose::ZERO;
        beyond.z = 8.0 * 0.25;
        assert_eq!(beyond.column(0.25, 8), None);

        assert_eq!(Pose::ZERO.column(0.0, 8), None);
    }
}
