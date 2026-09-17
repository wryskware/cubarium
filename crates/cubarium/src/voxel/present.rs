//! Drawing a voxel strip as pixel art: strata palette, autotiled block faces,
//! translucent free water, and an atmospheric tint that fades with depth.
//!
//! One pass per depth slice, back to front (`z` descending), so a nearer voxel simply
//! paints over a farther one — there is no depth buffer and no sorting beyond the loop
//! order. Inside a slice `y` ascends, which matters only for water: a block's top face
//! is drawn only when the voxel above it is air, so two opaque faces of one slice never
//! overlap.
//!
//! Each block contributes two rectangles from [`crate::voxel::project`] — a front face
//! `s × s` in the strata colour and a top face `s × rise` lit from above — and the block
//! is *autotiled* from its neighbours' occupancy rather than from a tile atlas:
//!
//! - the front face's top row is a rim light where the voxel above is air;
//! - an exposed left or right side gets a darkened bevel column, so single blocks read;
//! - where the top row meets an exposed side, that corner pixel takes the **top-face**
//!   colour, which chamfers a stair-step and makes a staircase of blocks read as a slope;
//! - a *top* face is bevelled on a side only at a real drop — nothing beside it at this
//!   level **or one below**. One step down is a slope, not an edge, and bevelling it
//!   paints a dark column every `s` pixels across every slope: corduroy, not relief;
//! - a top face with solid material higher up its own column is under a roof and keeps
//!   only [`ROOF_LIGHT`] of the light, which is the only shadowing in the picture and the
//!   thing that makes a covered passage read as a hole rather than as an artefact.
//!
//! Free water is translucent: a voxel holding `free` of its void volume fills the bottom
//! `round(free · s)` rows of its front rectangle, blended over whatever is behind it,
//! with a brighter surface row and a receding `rise`-row top face wherever nothing sits
//! on top of it. Pore water darkens soil toward the water colour instead of filling it.
//!
//! Depth tint: every colour in slice `z` is mixed toward the haze colour by
//! `haze · z / (depth − 1)`, so the back wall recedes and the front reads as the front.

use std::sync::LazyLock;

use cube_proto::Face;
use cubarium_render::Canvas;
use cubarium_voxel::{Material, VoxelView};

use crate::present::{mix, srgb_linear};

use super::VoxelConfig;
use super::project::Projection;

// --- Strata palette ------------------------------------------------------------------
//
// The Outrun family of `design/appearance.md`: deep indigo and violet substrate with
// electric blue-cyan light, and water the one bright thing. The sky is the appearance
// doc's floor colour at its floor brightness, so "empty" reads as dark, not off.

/// Sky behind the strip: `design/appearance.md`'s floor colour and brightness.
pub const SKY_SRGB: u32 = 0x0012_093A;
pub const SKY_BRIGHTNESS: f32 = 0.12;
/// The impermeable foundation: the darkest stratum, barely above the sky.
pub const BEDROCK_SRGB: u32 = 0x001A_1038;
/// Hard rock: cool indigo.
pub const ROCK_SRGB: u32 = 0x002C_2070;
/// Soil: the violet of the appearance doc's detritus, lifted enough to read as a layer.
pub const SOIL_SRGB: u32 = 0x0059_206B;
/// Deep free water: the producer ramp's low end.
pub const WATER_DEEP_SRGB: u32 = 0x001E_2798;
/// A free-water surface: the producer ramp's high end, the palette's brightest cyan.
pub const WATER_SURFACE_SRGB: u32 = 0x0042_C5F8;
/// The light top faces are lit by: the body ramp's cyan.
pub const LIGHT_SRGB: u32 = 0x0042_C6FF;
/// What distance fades toward.
pub const HAZE_SRGB: u32 = 0x0024_1657;

/// How much brighter a top face is than the front face it caps.
pub const TOP_GAIN: f32 = 2.4;
/// How far a top face leans toward [`LIGHT_SRGB`].
pub const TOP_TINT: f32 = 0.20;
/// How far a top face's back row shades back toward the front colour. Only applied when
/// the depth step is 3 px or more: at 2 px it turns every slope into a 1-pixel hatch.
pub const TOP_BACK: f32 = 0.30;
/// How much of the top-face light reaches a face that has solid material above it
/// somewhere in its own column — the floor of a cave, or the ground under an overhang.
/// Without this a covered passage is lit exactly like open ground and reads as an
/// artefact rather than as a hole.
pub const ROOF_LIGHT: f32 = 0.22;
/// How far the front face's top row leans toward the top face: the rim light.
pub const RIM: f32 = 0.38;
/// An exposed side column is the front colour times this.
pub const EDGE_DARK: f32 = 0.72;
/// How far an exposed side of a top face shades toward the front colour.
pub const TOP_EDGE: f32 = 0.18;
/// How far fully saturated soil darkens toward [`WATER_DEEP_SRGB`].
pub const WET: f32 = 0.55;
/// A water surface row blends this much harder than the body.
pub const SKIN_ALPHA_GAIN: f32 = 1.7;
/// Below this fraction a voxel is dry and draws nothing.
const WATER_EPSILON: f32 = 1e-4;

struct Strata {
    sky: [f32; 3],
    bedrock: [f32; 3],
    rock: [f32; 3],
    soil: [f32; 3],
    water_deep: [f32; 3],
    water_surface: [f32; 3],
    light: [f32; 3],
    haze: [f32; 3],
}

static STRATA: LazyLock<Strata> = LazyLock::new(|| Strata {
    sky: mul(srgb_linear(SKY_SRGB), SKY_BRIGHTNESS),
    bedrock: srgb_linear(BEDROCK_SRGB),
    rock: srgb_linear(ROCK_SRGB),
    soil: srgb_linear(SOIL_SRGB),
    water_deep: srgb_linear(WATER_DEEP_SRGB),
    water_surface: srgb_linear(WATER_SURFACE_SRGB),
    light: srgb_linear(LIGHT_SRGB),
    haze: srgb_linear(HAZE_SRGB),
});

#[inline]
fn mul(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// The front-face colour of a solid material, before wetness and haze.
fn strata_of(m: Material) -> [f32; 3] {
    match m {
        Material::Air => STRATA.sky,
        Material::Bedrock => STRATA.bedrock,
        Material::Rock => STRATA.rock,
        Material::Soil => STRATA.soil,
    }
}

/// The sky colour, in linear light. What the presenter clears to.
pub fn sky() -> [f32; 3] {
    STRATA.sky
}

// --- The presenter -------------------------------------------------------------------

/// Draws a [`VoxelView`] into a ring [`Canvas`]. Holds no world state: the picture is a
/// pure function of the view, the projection and the config.
pub struct VoxelPresenter {
    cfg: VoxelConfig,
    proj: Projection,
    /// The crop is reported once, not once per frame.
    said_cropped: bool,
    /// Per `(z, x)`, the highest solid voxel: the open sky's reach down each column,
    /// rebuilt once per frame rather than rescanned per voxel. `-1` is an empty column.
    sky_top: Vec<i32>,
}

impl VoxelPresenter {
    pub fn new(cfg: VoxelConfig, proj: Projection) -> VoxelPresenter {
        VoxelPresenter { cfg, proj, said_cropped: false, sky_top: Vec::new() }
    }

    pub fn projection(&self) -> Projection {
        self.proj
    }

    /// Paint one frame. The canvas must be the single-chart ring the projection sized.
    pub fn draw(&mut self, view: &VoxelView<'_>, canvas: &mut Canvas) {
        if self.proj.cropped && !self.said_cropped {
            self.said_cropped = true;
            eprintln!(
                "cubarium: the strip projects to {} rows but the raster is {}; \
                 cropping {} rows off the top",
                self.proj.full_height(),
                self.proj.raster_h,
                self.proj.full_height() - u32::from(self.proj.raster_h),
            );
        }

        canvas.pixels_mut().fill(STRATA.sky);
        let surf = Surface { w: i32::from(canvas.width()), h: i32::from(canvas.height()) };
        let p = self.proj;
        let haze_max = self.cfg.haze.clamp(0.0, 1.0);
        let water_alpha = self.cfg.water_alpha.clamp(0.0, 1.0);

        // The open sky's reach, per column: the highest solid voxel. A face below it has
        // something over its head, which is the only shadowing in the picture.
        self.sky_top.clear();
        self.sky_top.reserve(p.depth as usize * p.width as usize);
        for z in 0..p.depth {
            for x in 0..i64::from(p.width) {
                self.sky_top.push(view.surface_y(x, z).map_or(-1, |y| y as i32));
            }
        }

        for z in (0..p.depth).rev() {
            let t = if p.depth > 1 { z as f32 / (p.depth - 1) as f32 } else { 0.0 };
            let haze = haze_max * t;
            let row = z as usize * p.width as usize;
            for y in 0..p.height {
                for x in 0..i64::from(p.width) {
                    let m = view.material_at(x, y, z);
                    let free = view.free_at(x, y, z) as f32;
                    if m.is_solid() {
                        if self.hidden(view, x, y, z) {
                            continue;
                        }
                        let roofed = (y as i32) < self.sky_top[row + x as usize];
                        self.block(view, canvas, surf, x, y, z, m, haze, roofed);
                    } else if free > WATER_EPSILON {
                        if self.hidden(view, x, y, z) {
                            continue;
                        }
                        self.water(view, canvas, surf, x, y, z, free, haze, water_alpha);
                    }
                }
            }
        }
    }

    /// Whether a nearer slice already covers everything this voxel could paint.
    ///
    /// The voxel's own band is `front_row − rise .. front_row + s`. Front faces at
    /// `z − 1` tile that column in steps of `s` starting `rise` rows lower, so the three
    /// voxels `(x, y..y+2, z − 1)` cover `front_row + rise − 2s .. front_row + rise + s`,
    /// which contains the band for every `rise ≤ s`. Coverage is transitive: a voxel that
    /// is itself hidden is covered by something nearer still.
    fn hidden(&self, view: &VoxelView<'_>, x: i64, y: u32, z: u32) -> bool {
        z > 0 && (0..=2).all(|k| solid(view, x, i64::from(y) + k, z - 1))
    }

    #[allow(clippy::too_many_arguments)]
    fn block(
        &self,
        view: &VoxelView<'_>,
        canvas: &mut Canvas,
        surf: Surface,
        x: i64,
        y: u32,
        z: u32,
        m: Material,
        haze: f32,
        roofed: bool,
    ) {
        let p = self.proj;
        let wet =
            if m.pore_capacity() > 0.0 { view.pore_at(x, y, z).clamp(0.0, 1.0) as f32 } else { 0.0 };
        let body = mix(strata_of(m), STRATA.water_deep, wet * WET);
        let mut lit = mix(mul(body, TOP_GAIN), STRATA.light, TOP_TINT);
        if roofed {
            lit = mix(body, lit, ROOF_LIGHT);
        }

        let front = hazed(body, haze);
        let top = hazed(lit, haze);
        let rim = hazed(mix(body, lit, RIM), haze);
        let edge = hazed(mul(body, EDGE_DARK), haze);
        let top_back = hazed(mix(lit, body, TOP_BACK), haze);
        let top_edge = hazed(mix(lit, body, TOP_EDGE), haze);

        let yi = i64::from(y);
        let open_up = !solid(view, x, yi + 1, z);
        let open_left = !solid(view, x - 1, yi, z);
        let open_right = !solid(view, x + 1, yi, z);
        // A *top* face is bevelled only at a real drop — nothing beside it at this level
        // or one below. One step down is a slope, not an edge: the ground carries on and
        // the step shows as the next block's own front face. Bevelling it anyway paints a
        // dark pixel column every `s` pixels across every slope, which reads as corduroy.
        let drop_left = open_left && !solid(view, x - 1, yi - 1, z);
        let drop_right = open_right && !solid(view, x + 1, yi - 1, z);

        let (c0, r0, w, h) = p.front_rect(x, y, z);
        for dy in 0..h as i32 {
            for dx in 0..w as i32 {
                let on_cap = dy == 0 && open_up;
                let on_side = (dx == 0 && open_left) || (dx + 1 == w as i32 && open_right);
                let rgb = match (on_cap, on_side) {
                    // The chamfer: a lit corner pixel turns a stair-step into a slope.
                    (true, true) => top,
                    (true, false) => rim,
                    (false, true) => edge,
                    (false, false) => front,
                };
                surf.put(canvas, c0 + dx, r0 + dy, rgb);
            }
        }

        if open_up {
            let (tc, tr, tw, th) = p.top_rect(x, y, z);
            for dy in 0..th as i32 {
                for dx in 0..tw as i32 {
                    let mut rgb = if th > 2 && dy == 0 { top_back } else { top };
                    if (dx == 0 && drop_left) || (dx + 1 == tw as i32 && drop_right) {
                        rgb = top_edge;
                    }
                    surf.put(canvas, tc + dx, tr + dy, rgb);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn water(
        &self,
        view: &VoxelView<'_>,
        canvas: &mut Canvas,
        surf: Surface,
        x: i64,
        y: u32,
        z: u32,
        free: f32,
        haze: f32,
        alpha: f32,
    ) {
        let p = self.proj;
        let s = p.s as i32;
        let fill = (free.clamp(0.0, 1.0) * p.s as f32).round().clamp(1.0, p.s as f32) as i32;
        let (c0, r0, w, _) = p.front_rect(x, y, z);
        let bottom = r0 + s;
        let skin_row = bottom - fill;

        let body = hazed(STRATA.water_deep, haze);
        let skin = hazed(STRATA.water_surface, haze);
        let skin_alpha = (alpha * SKIN_ALPHA_GAIN).min(0.95);

        // A voxel with water or rock on top has no surface of its own: it is submerged.
        let yi = i64::from(y);
        let covered = solid(view, x, yi + 1, z) || free_at(view, x, yi + 1, z) > WATER_EPSILON;

        for row in skin_row..bottom {
            let is_skin = row == skin_row && !covered;
            let (rgb, a) = if is_skin { (skin, skin_alpha) } else { (body, alpha) };
            for dx in 0..w as i32 {
                surf.blend(canvas, c0 + dx, row, rgb, a);
            }
        }

        // The surface's own top face, receding into depth exactly as a block's does.
        if !covered {
            for dy in 0..p.rise as i32 {
                for dx in 0..w as i32 {
                    surf.blend(
                        canvas,
                        c0 + dx,
                        skin_row - p.rise as i32 + dy,
                        skin,
                        skin_alpha * 0.8,
                    );
                }
            }
        }
    }
}

/// Mix a colour toward the haze by `t`.
#[inline]
fn hazed(c: [f32; 3], t: f32) -> [f32; 3] {
    mix(c, STRATA.haze, t)
}

/// Solidity with the strip's wrap in `x` and out-of-range `y` reading as air, which is
/// what makes the top of the world an open sky and the autotiling work at `y = 0`.
#[inline]
fn solid(view: &VoxelView<'_>, x: i64, y: i64, z: u32) -> bool {
    y >= 0 && y < i64::from(view.config.height) && view.material_at(x, y as u32, z).is_solid()
}

#[inline]
fn free_at(view: &VoxelView<'_>, x: i64, y: i64, z: u32) -> f32 {
    if y < 0 || y >= i64::from(view.config.height) {
        return 0.0;
    }
    view.free_at(x, y as u32, z) as f32
}

/// The raster's own bounds: wrap in `x`, clip in `y`.
///
/// Wrapping rather than clipping `x` is the ring topology: a block that starts before
/// column 0 or runs past the last column is the same block seen from the other side of
/// the seam, so nothing is ever lost there. `y` has real ends — the sky above and the
/// crop below — so it clips.
#[derive(Clone, Copy)]
struct Surface {
    w: i32,
    h: i32,
}

impl Surface {
    #[inline]
    fn put(self, canvas: &mut Canvas, x: i32, y: i32, rgb: [f32; 3]) {
        if y < 0 || y >= self.h {
            return;
        }
        canvas.set(Face::Front, x.rem_euclid(self.w) as u16, y as u16, rgb);
    }

    #[inline]
    fn blend(self, canvas: &mut Canvas, x: i32, y: i32, rgb: [f32; 3], a: f32) {
        if y < 0 || y >= self.h {
            return;
        }
        let (px, py) = (x.rem_euclid(self.w) as u16, y as u16);
        let under = canvas.get(Face::Front, px, py);
        canvas.set(Face::Front, px, py, mix(under, rgb, a));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel::{Command, Config, World};
    use cubarium_render::Canvas;
    use cubarium_surface::{Scale, Topology};

    fn config() -> Config {
        Config { width: 32, height: 12, depth: 4, ..Config::default() }
    }

    fn present(world: &World) -> (Canvas, Projection) {
        let cfg = VoxelConfig::default();
        let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, 0, world.config()).unwrap();
        let mut canvas = Canvas::new(
            Topology::Ring { w: proj.raster_w, h: proj.raster_h },
            Scale::ONE,
        );
        VoxelPresenter::new(cfg, proj).draw(&world.view(), &mut canvas);
        (canvas, proj)
    }

    fn set(world: &mut World, x: i64, y: u32, z: u32, m: Material) {
        world.apply(Command::SetMaterial { x, y, z, material: m });
    }

    /// Back to front: a voxel with an opaque column in front of it contributes nothing,
    /// so the picture is identical to the one without it.
    ///
    /// The pedestal at `(4, 4, 2)` is in both worlds because adding the far voxel also
    /// puts the block *below* it under a roof, which dims that block's top face — a real
    /// effect, so the fixture keeps it where the nearer column covers it too rather than
    /// pretending it does not happen.
    #[test]
    fn back_to_front_order_hides_a_voxel_behind_a_nearer_one() {
        let build = |far: bool| {
            let mut world = World::empty(config());
            for y in 5..8 {
                set(&mut world, 4, y, 0, Material::Rock);
            }
            set(&mut world, 4, 4, 2, Material::Rock);
            if far {
                set(&mut world, 4, 5, 2, Material::Soil);
            }
            world
        };
        // The far voxel on its own is plainly visible.
        let mut alone = World::empty(config());
        set(&mut alone, 4, 4, 2, Material::Rock);
        let mut alone_far = World::empty(config());
        set(&mut alone_far, 4, 4, 2, Material::Rock);
        set(&mut alone_far, 4, 5, 2, Material::Soil);
        assert_ne!(
            present(&alone).0.pixels(),
            present(&alone_far).0.pixels(),
            "the far voxel must be visible with nothing in front of it"
        );

        // Put the nearer column in front of it and it contributes nothing at all.
        assert_eq!(
            present(&build(true)).0.pixels(),
            present(&build(false)).0.pixels(),
            "a nearer opaque column must hide it entirely"
        );
    }

    /// The strip wraps: authoring the same feature at `x` and at `x + width` gives the
    /// identical picture, because the pixel column is the same one.
    #[test]
    fn a_wrapped_column_draws_identically_at_x_and_x_plus_width() {
        let c = config();
        let w = i64::from(c.width);
        let mut here = World::empty(c.clone());
        let mut wrapped = World::empty(c.clone());
        for (world, x0) in [(&mut here, 3i64), (&mut wrapped, 3 + w)] {
            for y in 1..6 {
                set(world, x0, y, 0, Material::Rock);
                set(world, x0 + 1, y, 1, Material::Soil);
            }
            // And a column that runs off the near end either way.
            set(world, x0 - 5, 1, 0, Material::Soil);
        }
        let (a, _) = present(&here);
        let (b, _) = present(&wrapped);
        assert_eq!(a.pixels(), b.pixels());
    }

    /// A world periodic in `x` renders to a raster periodic in pixel columns: shifting
    /// every feature by half the strip rotates the picture by exactly half the raster,
    /// which is the seam carrying a feature rather than cutting one.
    #[test]
    fn the_seam_carries_a_feature_and_the_pixel_columns_match() {
        fn ridge(c: Config, shift: i64) -> World {
            let mut world = World::empty(c.clone());
            let w = i64::from(c.width);
            for x in 0..w {
                let u = (x - shift).rem_euclid(w) as f64 / w as f64
                    * std::f64::consts::TAU;
                let top = (4.0 + 3.0 * u.cos()).round() as u32;
                for z in 0..c.depth {
                    for y in 1..=top {
                        let m = if y == top { Material::Soil } else { Material::Rock };
                        world.apply(Command::SetMaterial { x, y, z, material: m });
                    }
                }
            }
            world
        }

        let c = config();
        let half = i64::from(c.width) / 2;
        let (a, proj) = present(&ridge(c.clone(), 0));
        let (b, _) = present(&ridge(c.clone(), half));

        // B is A rotated by half the raster's columns, pixel for pixel.
        let w = usize::from(proj.raster_w);
        let shift = half as usize * proj.s as usize;
        for row in 0..usize::from(proj.raster_h) {
            for x in 0..w {
                assert_eq!(
                    b.pixels()[row * w + (x + shift) % w],
                    a.pixels()[row * w + x],
                    "row {row}, column {x}"
                );
            }
        }

        // And the skyline is continuous across column 0: the step from the last pixel
        // column to the first is no larger than the step between any interior pair.
        let skyline: Vec<usize> = (0..w)
            .map(|x| {
                (0..usize::from(proj.raster_h))
                    .find(|&row| a.pixels()[row * w + x] != sky())
                    .unwrap_or(usize::from(proj.raster_h))
            })
            .collect();
        let step = |i: usize, j: usize| skyline[i].abs_diff(skyline[j]);
        let interior = (1..w).map(|x| step(x - 1, x)).max().unwrap();
        assert!(
            step(w - 1, 0) <= interior,
            "the seam step {} exceeds the largest interior step {interior}",
            step(w - 1, 0)
        );
    }

    /// A half-full voxel of free water fills the bottom half of its front rectangle and
    /// nothing above it, and the surface row is the brighter one.
    ///
    /// One depth slice, so the rows above the pool are sky rather than the same pool seen
    /// one step further back — the depth slices stack upward by `rise`.
    #[test]
    fn free_water_fills_from_the_bottom_of_its_voxel() {
        let c = Config { depth: 1, ..config() };
        let mut world = World::empty(c.clone());
        for y in 1..4 {
            for x in 2..8 {
                set(&mut world, x, y, 0, Material::Rock);
            }
        }
        let vol = c.voxel_volume();
        for x in 2..8 {
            world.apply(Command::AddWater { x, y: 4, z: 0, volume_m3: vol * 0.5 });
        }
        let (canvas, proj) = present(&world);
        let w = usize::from(proj.raster_w);
        let col = proj.col(4) as usize + 1;
        let (_, r0, _, s) = proj.front_rect(4, 4, 0);
        let at = |row: i32| canvas.pixels()[row as usize * w + col];

        // The bottom two of the four rows are water; the surface row is the top of those.
        let bottom = r0 + s as i32 - 1;
        let skin_row = r0 + s as i32 - 2;
        assert!(bluer_than_sky(at(bottom)), "the bottom row must be water");
        assert!(bluer_than_sky(at(skin_row)), "the surface row must be water");
        assert!(
            at(skin_row)[2] > at(bottom)[2],
            "the surface row is the brighter one: {:?} vs {:?}",
            at(skin_row),
            at(bottom)
        );
        // The row above the voxel's own water is the water's receding top face, and two
        // rows further up (past `rise`) is sky again.
        assert_eq!(proj.rise, 2);
        assert_eq!(at(skin_row - 3), sky(), "above the water's top face is sky");
    }

    /// Green-blue dominant and brighter than the sky: the only thing in the palette that
    /// is, so it identifies water without pinning a colour constant.
    fn bluer_than_sky(px: [f32; 3]) -> bool {
        px[2] > sky()[2] * 2.0 && px[2] > px[0]
    }

    /// The authored fixture really does carry the four features it is for.
    #[test]
    fn the_authored_scene_has_a_ridge_a_hollow_with_water_and_an_overhang() {
        let c = Config { width: 64, height: 24, depth: 8, ..Config::default() };
        let world = super::super::scene::authored(c.clone());
        let view = world.view();

        let tops: Vec<u32> =
            (0..i64::from(c.width)).map(|x| view.surface_y(x, 0).unwrap_or(0)).collect();
        let (lo, hi) = (*tops.iter().min().unwrap(), *tops.iter().max().unwrap());
        assert!(hi >= lo + 6, "a ridge needs relief: {lo}..{hi}");

        let water: f64 = view.free.iter().sum();
        assert!(water > 0.0, "the hollow must hold standing water");

        // An overhang: solid with air under it somewhere below the column's surface.
        let overhangs = (0..i64::from(c.width))
            .filter(|&x| {
                let top = view.surface_y(x, 0).unwrap_or(0);
                (1..top).any(|y| {
                    view.material_at(x, y, 0) == Material::Air
                        && view.material_at(x, y + 1, 0).is_solid()
                })
            })
            .count();
        assert!(overhangs > 0, "the scene must have a roofed void");

        // And the ridge crosses the seam rather than stopping at it.
        assert!(
            tops[0] > lo + 4 && tops[c.width as usize - 1] > lo + 4,
            "the ridge must straddle x = 0: {} and {}",
            tops[0],
            tops[c.width as usize - 1]
        );
    }
}
