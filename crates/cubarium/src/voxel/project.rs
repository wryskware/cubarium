//! The elevated orthographic projection of the voxel strip into ring pixels.
//!
//! Voxel `(x, y, z)` — `x` along the strip and wrapping, `y` up, `z` into the habitat —
//! maps to pixels by a *vertical* shear: depth lifts a voxel up the screen and never
//! sideways, which is what keeps the strip's own width equal to the raster's and makes
//! the seam in `x` a pure modulo. Writing `s` for `px_per_voxel` and
//! `rise = round(s · tan(tilt))`:
//!
//! ```text
//! sx = x · s                                (mod raster width)
//! sy = base − y · s − z · rise              (sy grows downward; base is the floor)
//! ```
//!
//! so voxel `(x, y, z)` owns two axis-aligned rectangles, both `s` pixels wide:
//!
//! - its **front face**, rows `base − (y+1)·s − z·rise` .. `+ s`;
//! - its **top face**, the `rise` rows directly above that.
//!
//! The two tile exactly: the front face of the voxel one step *nearer* sits `rise` rows
//! lower, and that nearer voxel's own top face covers the strip left over. A whole
//! opaque slab of world therefore paints with no gaps, which is what lets
//! [`crate::voxel::present`] cull interior voxels by neighbour occupancy alone.
//!
//! `rise` is rounded to a whole number of pixels rather than kept as a float: this is
//! pixel art, and a fractional depth step would smear every block edge along the strip.
//! Tilt therefore reaches the picture only through `rise`, and tilts that round to the
//! same `rise` give the same image.

use anyhow::{Result, ensure};

use cubarium_voxel::Config as WorldConfig;

/// One fixed mapping from voxel coordinates to ring pixels, plus the raster it needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Projection {
    /// Pixels per voxel edge: the side of a front face.
    pub s: u32,
    /// Pixels a voxel of depth lifts the image, `1..=s`.
    pub rise: u32,
    /// World extent in voxels.
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    /// The ring raster this projection draws into.
    pub raster_w: u16,
    pub raster_h: u16,
    /// Screen row one past the bottom of the `y = 0, z = 0` front face: the floor line.
    pub base: i32,
    /// True when the projected strip is taller than the raster, so rows are lost off the
    /// top. The presenter reports this once.
    pub cropped: bool,
}

impl Projection {
    /// Build the projection for a world at a tilt and pixel scale.
    ///
    /// `raster_height` of zero derives the raster height from the world, so the whole
    /// strip fits exactly; a smaller value crops from the top.
    pub fn new(
        tilt_degrees: f64,
        px_per_voxel: u32,
        raster_height: u16,
        world: &WorldConfig,
    ) -> Result<Projection> {
        ensure!(
            world.width > 0 && world.height > 0 && world.depth > 0,
            "an empty world"
        );
        let s = px_per_voxel.max(1);
        ensure!(
            tilt_degrees.is_finite() && tilt_degrees > 0.0 && tilt_degrees < 90.0,
            "tilt_degrees must be in (0, 90), got {tilt_degrees}"
        );
        let rise = (f64::from(s) * tilt_degrees.to_radians().tan())
            .round()
            .clamp(1.0, f64::from(s)) as u32;

        let raster_w = u64::from(world.width) * u64::from(s);
        ensure!(
            raster_w <= u64::from(u16::MAX),
            "the strip is {raster_w} pixels wide at {s} px per voxel; \
             lower px_per_voxel or the world width"
        );
        let full_h =
            u64::from(world.height) * u64::from(s) + u64::from(world.depth) * u64::from(rise);
        ensure!(
            full_h <= u64::from(u16::MAX),
            "the projected strip is {full_h} pixels tall; lower px_per_voxel or the tilt"
        );
        let raster_h = if raster_height == 0 {
            full_h as u16
        } else {
            raster_height
        };

        Ok(Projection {
            s,
            rise,
            width: world.width,
            height: world.height,
            depth: world.depth,
            raster_w: raster_w as u16,
            raster_h,
            base: i32::from(raster_h),
            cropped: full_h > u64::from(raster_h),
        })
    }

    /// The pixel column a voxel column starts at. `x` wraps, so the strip has no edge.
    #[inline]
    pub fn col(&self, x: i64) -> i32 {
        (x.rem_euclid(i64::from(self.width)) as i32) * self.s as i32
    }

    /// The top row of the front face of `(_, y, z)`. May be negative when cropped.
    #[inline]
    pub fn front_row(&self, y: u32, z: u32) -> i32 {
        self.base - ((y + 1) * self.s) as i32 - (z * self.rise) as i32
    }

    /// Front face of a voxel as `(col, row, w, h)`; `h` is always `s`.
    #[inline]
    pub fn front_rect(&self, x: i64, y: u32, z: u32) -> (i32, i32, u32, u32) {
        (self.col(x), self.front_row(y, z), self.s, self.s)
    }

    /// Top face of a voxel as `(col, row, w, h)`; `h` is always `rise`, and it sits
    /// directly above the front face.
    #[inline]
    pub fn top_rect(&self, x: i64, y: u32, z: u32) -> (i32, i32, u32, u32) {
        (
            self.col(x),
            self.front_row(y, z) - self.rise as i32,
            self.s,
            self.rise,
        )
    }

    /// The projected height of the whole strip, before any crop.
    pub fn full_height(&self) -> u32 {
        self.height * self.s + self.depth * self.rise
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(width: u32, height: u32, depth: u32) -> WorldConfig {
        WorldConfig {
            width,
            height,
            depth,
            ..WorldConfig::default()
        }
    }

    /// The documented defaults, on the documented world, land on documented pixels.
    #[test]
    fn a_known_voxel_lands_on_the_expected_pixel() {
        let p = Projection::new(30.0, 4, 0, &world(128, 48, 16)).unwrap();
        // round(4 · tan 30°) = round(2.309) = 2.
        assert_eq!((p.s, p.rise), (4, 2));
        // The strip fits the raster exactly in both directions.
        assert_eq!((p.raster_w, p.raster_h), (512, 48 * 4 + 16 * 2));
        assert!(!p.cropped);
        assert_eq!(p.base, 224);

        // The near-bottom-left voxel's front face is the bottom-left 4x4 block.
        assert_eq!(p.front_rect(0, 0, 0), (0, 220, 4, 4));
        // Its top face is the two rows above it.
        assert_eq!(p.top_rect(0, 0, 0), (0, 218, 4, 2));
        // The far-top voxel's top face is the topmost row pair: the strip is exactly as
        // tall as the raster.
        assert_eq!(p.top_rect(0, 47, 15), (0, 0, 4, 2));
        // One voxel along is one block along; one voxel of depth is `rise` rows up.
        assert_eq!(p.front_rect(7, 0, 0), (28, 220, 4, 4));
        assert_eq!(p.front_rect(0, 0, 1), (0, 218, 4, 4));
        assert_eq!(p.front_rect(0, 1, 0), (0, 216, 4, 4));
    }

    /// The front face of the voxel one step nearer sits exactly `rise` rows lower, and
    /// that nearer voxel's top face covers the strip left over: no seam between depth
    /// slices.
    #[test]
    fn depth_slices_tile_without_a_gap() {
        let p = Projection::new(35.0, 6, 0, &world(16, 8, 4)).unwrap();
        for z in 1..p.depth {
            let (_, far_top, _, _) = p.top_rect(3, 2, z);
            let (_, near_top, _, near_h) = p.front_rect(3, 2, z - 1);
            // The far voxel's covered band is `far_top .. front_row(2, z) + s`; the
            // nearer front face starts `rise` rows below the far front face.
            assert_eq!(near_top, p.front_row(2, z) + p.rise as i32);
            // And the nearer voxel's own top face fills exactly the rows between.
            let (_, near_cap, _, cap_h) = p.top_rect(3, 2, z - 1);
            assert_eq!(near_cap + cap_h as i32, near_top);
            assert!(far_top < near_cap + cap_h as i32);
            assert_eq!(near_h, p.s);
        }
    }

    /// `x` wraps: the strip has no left or right edge, and a column beyond either end is
    /// the wrapped column, pixel for pixel.
    #[test]
    fn wrapped_columns_land_on_the_same_pixel_column() {
        let p = Projection::new(30.0, 4, 0, &world(32, 8, 4)).unwrap();
        for x in -40i64..40 {
            assert_eq!(p.col(x), p.col(x + i64::from(p.width)), "x = {x}");
            assert_eq!(p.col(x), p.col(x - i64::from(p.width)), "x = {x}");
        }
        assert_eq!(p.col(0), 0);
        assert_eq!(p.col(-1), p.col(31));
        assert_eq!(p.col(-1), 31 * 4);
        // The last block ends exactly at the raster's right edge, so the strip tiles it.
        assert_eq!(p.col(31) + p.s as i32, i32::from(p.raster_w));
    }

    /// A raster shorter than the projection crops from the top and says so; the floor
    /// line stays at the bottom either way.
    #[test]
    fn a_short_raster_crops_from_the_top() {
        let full = Projection::new(30.0, 4, 0, &world(32, 40, 8)).unwrap();
        assert_eq!(full.raster_h, 40 * 4 + 8 * 2);
        let short = Projection::new(30.0, 4, 100, &world(32, 40, 8)).unwrap();
        assert!(short.cropped);
        assert_eq!(short.raster_h, 100);
        assert_eq!(short.base, 100, "the floor is still the bottom row");
        assert_eq!(short.front_rect(0, 0, 0), (0, 96, 4, 4));
        // The tall voxels are off the top, which is what `cropped` reports.
        assert!(short.front_row(39, 7) < 0);
    }

    /// Tilt reaches the picture only through a whole-pixel `rise`, clamped into `1..=s`.
    #[test]
    fn the_depth_step_is_a_whole_number_of_pixels_in_range() {
        for &(tilt, s, want) in &[
            (30.0, 4, 2u32),
            (35.0, 4, 3),
            (25.0, 4, 2),
            (30.0, 3, 2),
            (5.0, 4, 1),
            (80.0, 4, 4),
        ] {
            let p = Projection::new(tilt, s, 0, &world(16, 8, 4)).unwrap();
            assert_eq!(p.rise, want, "tilt {tilt} at {s} px");
            assert!((1..=p.s).contains(&p.rise));
        }
        assert!(Projection::new(0.0, 4, 0, &world(16, 8, 4)).is_err());
        assert!(Projection::new(90.0, 4, 0, &world(16, 8, 4)).is_err());
    }
}
