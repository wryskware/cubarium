//! Drawing a voxel strip as pixel art: strata palette, autotiled block faces,
//! translucent free water, and an atmospheric tint that fades with depth.
//!
//! # The draw order, and what owns a pixel
//!
//! **Slabs run far to near (`z` descending) and, inside a slab, bottom to top
//! (`y` ascending); each voxel contributes at most one front rectangle and one top
//! rectangle, and a rectangle is drawn only where the slab one step nearer does not
//! already own it.** There is no depth buffer: `z` descending is what makes a nearer wall
//! paint over a farther one, and `y` ascending is what makes free water — which is
//! blended, not written — land on top of the lit floor it stands on rather than under it.
//! Water and terrain are interleaved in that one traversal, per voxel, so a nearer wall
//! or roof occludes water exactly as it occludes rock.
//!
//! Face ownership is geometric, not a guess. Writing `R = front_row(y, z)`, `s` for the
//! voxel side and `rise` for the depth step, voxel `(x, y, z)` owns the rows
//! `R .. R + s` (front) and `R − rise .. R` (top), both `s` pixels wide, and:
//!
//! - the **front** rectangle of `(x, y, z)` is covered exactly when `(x, y, z − 1)` is
//!   solid — that voxel's own front covers `R + rise .. R + rise + s` and whichever of
//!   its top face or the front of `(x, y + 1, z − 1)` sits above it covers `R .. R + rise`;
//! - the **top** rectangle is covered exactly when `(x, y + 1, z − 1)` is solid, whose
//!   front and cap together span `R − s .. R + rise`. It is *not* covered merely because
//!   `(x, y, z − 1)` is: that case is a ledge, and the far block's top face is what you
//!   see receding above the nearer block's cap;
//! - a top face is drawn only where the voxel above is air, so a cap and the front face
//!   above it are never both written; where two faces of one physical surface meet, the
//!   lower and nearer one owns the shared row. Every physical face is therefore written
//!   once per pixel, which is what keeps a translucent blend from compounding.
//!
//! `x` wraps rather than clips (the ring has no edge), `y` clips: rows off the top of a
//! cropped raster are dropped and the `y` loop stops once a slab's rows are all above it,
//! so `base` stays pinned to the bottom of the raster and a taller or deeper config loses
//! sky, never registration.
//!
//! # Blocks
//!
//! Each block is *autotiled* from its neighbours' occupancy rather than from a tile atlas:
//!
//! - the front face's top row is a rim light where the voxel above is air;
//! - an exposed left or right side gets a darkened bevel column, so single blocks read,
//!   and so a cliff — which this projection gives zero width, its normal being `±x` —
//!   still has an edge;
//! - where the top row meets an exposed side, that corner pixel takes the **top-face**
//!   colour, which chamfers a stair-step and makes a staircase of blocks read as a slope;
//! - a *top* face is bevelled on a side only at a real drop — nothing beside it at this
//!   level **or one below**. One step down is a slope, not an edge, and bevelling it
//!   paints a dark column every `s` pixels across every slope: corduroy, not relief;
//! - a **riser** — the exposed front of a surface exactly one voxel above the surface one
//!   step nearer — is ground carrying on into depth, not a cut wall. It is shaded from the
//!   top-face light at its head down toward the front colour at its foot by [`RISER_LEAN`],
//!   with its haze interpolated across the same rows from slab `z` to slab `z − 1`, and
//!   with **no rim row**: the rim is the contour line, and a plateau that recedes in `z`
//!   by one voxel a slab would otherwise read as ruled shelving — a bright line and a dark
//!   band every `rise` rows — instead of as a slope;
//! - for the same reason a top face draws its [`TOP_BACK`] contour row only where the
//!   surface *ends* going back. Where it carries on at this level the row is suppressed,
//!   and the cap's haze is interpolated per row so consecutive slabs of one plateau form
//!   a single smooth ramp rather than a stack of shelves;
//! - a top face with solid material above it in its own column is sheltered and keeps only
//!   [`ROOF_LIGHT`] of the light directly under the roof, fading back to full light over
//!   [`ROOF_FALLOFF_VOXELS`]. The shadow is an occlusion blend that falls off downward from
//!   the thing casting it, so a lip shades the floor beneath it and not the whole column to
//!   the bottom of the frame.
//!
//! # Water
//!
//! Free water is translucent: a voxel holding `free` of its void volume fills the bottom
//! `round(free · s)` rows of its front rectangle, blended over whatever is behind it. A
//! **top** is drawn only at a water/air boundary, at `y + free` — a full cell with more
//! water above it is not an extra translucent surface — and those tops tile upward across
//! slabs into the pool's receding surface. A water body is clipped to the rows the nearer
//! slab's water or rock has not already claimed, so a pool many slabs deep blends one
//! water face per pixel instead of multiplying its alpha into a striped wall. Pore water
//! darkens soil toward the water colour instead of filling it.
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
/// How far a top face's back row shades back toward the front colour — the contour that
/// says "the ground ends here going back". Drawn only where the surface really does end
/// there, and only when the depth step is 3 px or more: at 2 px it is a 1-pixel hatch.
pub const TOP_BACK: f32 = 0.30;
/// How much of the top-face light reaches a face directly beneath a roof — the floor of a
/// cave, the ground under an overhang. Without this a covered passage is lit exactly like
/// open ground and reads as an artefact rather than as a hole.
pub const ROOF_LIGHT: f32 = 0.22;
/// Voxels of separation over which a roof's shadow fades back to full light. The shadow
/// belongs to the sheltered surface under the lip, not to every face below it: a canyon
/// floor twenty voxels under the same overhang is lit, not blacked out to the frame edge.
pub const ROOF_FALLOFF_VOXELS: f32 = 4.0;
/// How far the front face's top row leans toward the top face: the rim light.
pub const RIM: f32 = 0.38;
/// An exposed side column is the front colour times this.
pub const EDGE_DARK: f32 = 0.72;
/// How far an exposed side of a top face shades toward the front colour.
pub const TOP_EDGE: f32 = 0.18;
/// How far a riser — a one-voxel step of ground into depth — leans from the top-face
/// light at its head toward the front colour at its foot. Enough tilt to read as a slope,
/// not enough to read as a wall.
pub const RISER_LEAN: f32 = 0.45;
/// How far fully saturated soil darkens toward [`WATER_DEEP_SRGB`].
pub const WET: f32 = 0.55;
/// A water surface row blends this much harder than the body.
pub const SKIN_ALPHA_GAIN: f32 = 1.7;
/// How hard a water *top* face blends relative to the surface row it caps.
pub const WATER_TOP_ALPHA: f32 = 0.8;
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

/// How much of the top-face light a surface `gap` voxels below its roof keeps. `0` is an
/// open column, `1` is directly under the roof.
fn roof_shade(gap: u16) -> f32 {
    if gap == 0 {
        return 1.0;
    }
    let t = 1.0 - (-(f32::from(gap - 1) / ROOF_FALLOFF_VOXELS)).exp();
    ROOF_LIGHT + (1.0 - ROOF_LIGHT) * t
}

// --- The presenter -------------------------------------------------------------------

/// Drawn state for one voxel: the faces it owns and how they are shaded. Assembled from
/// the voxel's neighbours before anything is written, so the write loops stay flat.
struct Facing {
    open_up: bool,
    open_left: bool,
    open_right: bool,
    drop_left: bool,
    drop_right: bool,
    /// The slab one step nearer covers the front rectangle.
    front_hidden: bool,
    /// The slab one step nearer covers the top rectangle.
    top_hidden: bool,
    /// The exposed front is a one-voxel step of ground into depth, not a cut wall.
    riser: bool,
    /// The surface carries on at this level into the next slab back, so the top face's
    /// contour row would be a ruled line across a continuous plane.
    back_continues: bool,
}

/// Draws a [`VoxelView`] into a ring [`Canvas`]. Holds no world state: the picture is a
/// pure function of the view, the projection and the config.
pub struct VoxelPresenter {
    cfg: VoxelConfig,
    proj: Projection,
    /// The crop is reported once, not once per frame.
    said_cropped: bool,
    /// Per `(z, x, y)`, voxels from this one up to the nearest solid above it in its own
    /// column, or `0` where the column is open to the sky. Rebuilt once per frame rather
    /// than rescanned per voxel; it is what the roof shadow falls off over.
    roof: Vec<u16>,
}

impl VoxelPresenter {
    pub fn new(cfg: VoxelConfig, proj: Projection) -> VoxelPresenter {
        VoxelPresenter { cfg, proj, said_cropped: false, roof: Vec::new() }
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
        let water_alpha = self.cfg.water_alpha.clamp(0.0, 1.0);

        self.build_roof(view);

        for z in (0..p.depth).rev() {
            let col = z as usize * p.width as usize;
            for y in 0..p.height {
                // Everything this row of voxels can write lies in `front_row − rise ..
                // front_row + s`, and `front_row` only decreases as `y` climbs: once the
                // band is entirely above the raster, so is every row above it.
                if p.front_row(y, z) + p.s as i32 <= 0 {
                    break;
                }
                for x in 0..i64::from(p.width) {
                    let m = view.material_at(x, y, z);
                    let free = view.free_at(x, y, z) as f32;
                    if m.is_solid() {
                        let f = self.facing(view, x, y, z);
                        if f.front_hidden && (f.top_hidden || !f.open_up) {
                            continue;
                        }
                        let gap = self.roof[(col + x as usize) * p.height as usize + y as usize];
                        self.block(view, canvas, surf, x, y, z, m, &f, roof_shade(gap));
                    } else if free > WATER_EPSILON {
                        self.water(view, canvas, surf, x, y, z, free, water_alpha);
                    }
                }
            }
        }
    }

    /// Per `(z, x, y)`: voxels up to the nearest solid above, `0` where the sky is open.
    fn build_roof(&mut self, view: &VoxelView<'_>) {
        let p = self.proj;
        let (w, h, d) = (p.width as usize, p.height as usize, p.depth as usize);
        self.roof.clear();
        self.roof.resize(w * h * d, 0);
        for z in 0..p.depth {
            for x in 0..p.width {
                let col = (z as usize * w + x as usize) * h;
                let mut nearest: Option<u32> = None;
                for y in (0..p.height).rev() {
                    self.roof[col + y as usize] =
                        nearest.map_or(0, |r: u32| (r - y).min(u32::from(u16::MAX)) as u16);
                    if view.material_at(i64::from(x), y, z).is_solid() {
                        nearest = Some(y);
                    }
                }
            }
        }
    }

    /// Haze at a fractional depth. Fractional because a top face or a riser spans the gap
    /// between two slabs: interpolating it per row is what turns a receding plateau into
    /// one smooth ramp instead of a stack of same-coloured shelves.
    #[inline]
    fn haze_at(&self, zf: f32) -> f32 {
        if self.proj.depth <= 1 {
            return 0.0;
        }
        let t = (zf / (self.proj.depth - 1) as f32).clamp(0.0, 1.0);
        self.cfg.haze.clamp(0.0, 1.0) * t
    }

    /// Which faces a voxel owns, from its neighbours' occupancy alone.
    fn facing(&self, view: &VoxelView<'_>, x: i64, y: u32, z: u32) -> Facing {
        let p = self.proj;
        let yi = i64::from(y);
        let open_up = !solid(view, x, yi + 1, z);
        let open_left = !solid(view, x - 1, yi, z);
        let open_right = !solid(view, x + 1, yi, z);
        let nearer = z > 0;
        Facing {
            open_up,
            open_left,
            open_right,
            // A *top* face is bevelled only at a real drop — nothing beside it at this
            // level or one below. One step down is a slope, not an edge.
            drop_left: open_left && !solid(view, x - 1, yi - 1, z),
            drop_right: open_right && !solid(view, x + 1, yi - 1, z),
            front_hidden: nearer && solid(view, x, yi, z - 1),
            top_hidden: nearer && solid(view, x, yi + 1, z - 1),
            riser: open_up
                && nearer
                && !solid(view, x, yi, z - 1)
                && solid(view, x, yi - 1, z - 1),
            back_continues: z + 1 < p.depth
                && solid(view, x, yi, z + 1)
                && !solid(view, x, yi + 1, z + 1),
        }
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
        f: &Facing,
        shade: f32,
    ) {
        let p = self.proj;
        let haze = self.haze_at(z as f32);
        let wet =
            if m.pore_capacity() > 0.0 { view.pore_at(x, y, z).clamp(0.0, 1.0) as f32 } else { 0.0 };
        let body = mix(strata_of(m), STRATA.water_deep, wet * WET);
        let mut lit = mix(mul(body, TOP_GAIN), STRATA.light, TOP_TINT);
        if shade < 1.0 {
            lit = mix(body, lit, shade);
        }

        let (c0, r0, w, h) = p.front_rect(x, y, z);
        let cols = w as i32;
        if !f.front_hidden {
            if f.riser {
                // Ground stepping one voxel into depth: shade it as a slope and let its
                // haze cross from this slab to the nearer one, with no rim contour.
                for dy in 0..h as i32 {
                    let t = if h > 1 { dy as f32 / (h - 1) as f32 } else { 0.0 };
                    let slope = mix(lit, body, RISER_LEAN * t);
                    let c = hazed(slope, self.haze_at(z as f32 - t));
                    let e = hazed(mul(slope, EDGE_DARK), self.haze_at(z as f32 - t));
                    for dx in 0..cols {
                        let on_side =
                            (dx == 0 && f.open_left) || (dx + 1 == cols && f.open_right);
                        surf.put(canvas, c0 + dx, r0 + dy, if on_side { e } else { c });
                    }
                }
            } else {
                let front = hazed(body, haze);
                let rim = hazed(mix(body, lit, RIM), haze);
                let edge = hazed(mul(body, EDGE_DARK), haze);
                let top = hazed(lit, haze);
                for dy in 0..h as i32 {
                    for dx in 0..cols {
                        let on_cap = dy == 0 && f.open_up;
                        let on_side =
                            (dx == 0 && f.open_left) || (dx + 1 == cols && f.open_right);
                        let rgb = match (on_cap, on_side) {
                            // The chamfer: a lit corner pixel turns a stair-step into a
                            // slope.
                            (true, true) => top,
                            (true, false) => rim,
                            (false, true) => edge,
                            (false, false) => front,
                        };
                        surf.put(canvas, c0 + dx, r0 + dy, rgb);
                    }
                }
            }
        }

        if f.open_up && !f.top_hidden {
            let rise = p.rise as i32;
            for dy in 0..rise {
                let zf = z as f32 + (rise - 1 - dy) as f32 / rise as f32;
                let hz = self.haze_at(zf);
                // The contour row only where the ground really ends going back.
                let plane = if dy == 0 && rise > 2 && !f.back_continues {
                    mix(lit, body, TOP_BACK)
                } else {
                    lit
                };
                let face = hazed(plane, hz);
                let bevel = hazed(mix(plane, body, TOP_EDGE), hz);
                for dx in 0..cols {
                    let on_drop =
                        (dx == 0 && f.drop_left) || (dx + 1 == cols && f.drop_right);
                    surf.put(canvas, c0 + dx, r0 - rise + dy, if on_drop { bevel } else { face });
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
        alpha: f32,
    ) {
        let p = self.proj;
        let (s, rise) = (p.s as i32, p.rise as i32);
        let yi = i64::from(y);
        let fill = fill_px(free, p.s);
        let (c0, r0, w, _) = p.front_rect(x, y, z);
        let cols = w as i32;
        let bottom = r0 + s;
        let skin_row = bottom - fill;

        let haze = self.haze_at(z as f32);
        let body = hazed(STRATA.water_deep, haze);
        let skin = hazed(STRATA.water_surface, haze);
        let skin_alpha = (alpha * SKIN_ALPHA_GAIN).min(0.95);

        // Only a water/air boundary is a surface. A full cell with more water above it is
        // the inside of the body, not another translucent skin.
        let open_up =
            !solid(view, x, yi + 1, z) && free_at(view, x, yi + 1, z) <= WATER_EPSILON;

        // What the slab one step nearer already owns in these pixel columns.
        let near_solid = z > 0 && solid(view, x, yi, z - 1);
        let near_fill = if z > 0 { fill_px_at(view, x, yi, z - 1, p.s) } else { 0 };
        let near_skin = r0 + rise + s - near_fill;
        let near_open_up = z > 0
            && !solid(view, x, yi + 1, z - 1)
            && free_at(view, x, yi + 1, z - 1) <= WATER_EPSILON;
        // A nearer water top covers this one only when it sits at or above it, which needs
        // the nearer cell to be the deeper of the two: settled water never does, a
        // draining front briefly can.
        let top_hidden = (z > 0 && solid(view, x, yi + 1, z - 1))
            || (near_fill > 0 && near_open_up && near_skin <= skin_row);

        // The surface's own top face, receding into depth exactly as a block's does. Tops
        // of consecutive slabs are adjacent row bands, never overlapping ones, so this is
        // the pool's receding surface and not a stack of skins.
        if open_up && !top_hidden {
            for dy in 0..rise {
                let zf = z as f32 + (rise - 1 - dy) as f32 / rise as f32;
                let c = hazed(STRATA.water_surface, self.haze_at(zf));
                for dx in 0..cols {
                    surf.blend(
                        canvas,
                        c0 + dx,
                        skin_row - rise + dy,
                        c,
                        skin_alpha * WATER_TOP_ALPHA,
                    );
                }
            }
        }

        // The body, clipped to the rows no nearer slab has claimed: one water face per
        // pixel, so depth recedes instead of multiplying the alpha into a striped wall.
        let stop = if near_solid {
            skin_row
        } else if near_fill > 0 {
            bottom.min(if near_open_up { near_skin - rise } else { near_skin })
        } else {
            bottom
        };
        for row in skin_row..stop {
            let is_skin = row == skin_row && open_up;
            let (rgb, a) = if is_skin { (skin, skin_alpha) } else { (body, alpha) };
            for dx in 0..cols {
                surf.blend(canvas, c0 + dx, row, rgb, a);
            }
        }
    }
}

/// Mix a colour toward the haze by `t`.
#[inline]
fn hazed(c: [f32; 3], t: f32) -> [f32; 3] {
    mix(c, STRATA.haze, t)
}

/// Rows of a voxel's front rectangle that a `free` fraction fills: at least one, so any
/// water at all is visible, and never more than the whole face.
#[inline]
fn fill_px(free: f32, s: u32) -> i32 {
    (free.clamp(0.0, 1.0) * s as f32).round().clamp(1.0, s as f32) as i32
}

/// [`fill_px`] of a neighbour, or `0` where it holds no water.
#[inline]
fn fill_px_at(view: &VoxelView<'_>, x: i64, y: i64, z: u32, s: u32) -> i32 {
    let free = free_at(view, x, y, z);
    if free <= WATER_EPSILON { 0 } else { fill_px(free, s) }
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
    use cubarium_render::Canvas;
    use cubarium_surface::{Scale, Topology};
    use cubarium_voxel::{Command, Config, World};

    fn config() -> Config {
        Config { width: 32, height: 12, depth: 4, ..Config::default() }
    }

    fn present(world: &World) -> (Canvas, Projection) {
        present_with(VoxelConfig::default(), world)
    }

    fn present_with(cfg: VoxelConfig, world: &World) -> (Canvas, Projection) {
        let proj =
            Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, world.config())
                .unwrap();
        let mut canvas =
            Canvas::new(Topology::Ring { w: proj.raster_w, h: proj.raster_h }, Scale::ONE);
        VoxelPresenter::new(cfg, proj).draw(&world.view(), &mut canvas);
        (canvas, proj)
    }

    fn set(world: &mut World, x: i64, y: u32, z: u32, m: Material) {
        world.apply(Command::SetMaterial { x, y, z, material: m });
    }

    /// Relative luminance, enough to order two shades of the same face.
    fn lum(px: [f32; 3]) -> f32 {
        0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2]
    }

    fn pixel(canvas: &Canvas, proj: &Projection, col: i32, row: i32) -> [f32; 3] {
        let w = usize::from(proj.raster_w);
        canvas.pixels()[row as usize * w + col.rem_euclid(i32::from(proj.raster_w)) as usize]
    }

    /// Face ownership under tilt: a voxel whose neighbour one step nearer covers it
    /// contributes nothing at all, so its *material* — which changes every colour it could
    /// paint — cannot reach the raster.
    ///
    /// The same fixture with the nearer column removed is the control: then the far
    /// voxel's material plainly does reach it.
    #[test]
    fn a_nearer_voxel_hides_a_farther_one() {
        let build = |m: Material, nearer: bool| {
            let mut world = World::empty(config());
            set(&mut world, 4, 6, 1, m);
            if nearer {
                // Two blocks at `z = 0`: the lower one's front and cap cover the far
                // voxel's front, the upper one's front covers its top face.
                set(&mut world, 4, 6, 0, Material::Rock);
                set(&mut world, 4, 7, 0, Material::Rock);
            }
            world
        };

        // Control: with nothing in front of it, the far voxel's material is visible.
        assert_ne!(
            present(&build(Material::Rock, false)).0.pixels(),
            present(&build(Material::Soil, false)).0.pixels(),
            "the far voxel must be visible with nothing in front of it"
        );

        // Covered: the picture cannot tell what the far voxel is made of.
        assert_eq!(
            present(&build(Material::Rock, true)).0.pixels(),
            present(&build(Material::Soil, true)).0.pixels(),
            "a nearer column must hide the far voxel entirely"
        );

        // Water is interleaved with terrain in the same traversal, so a nearer wall
        // occludes it too: neither its body nor its top reaches the raster.
        let pool = |nearer: bool| {
            let c = config();
            let mut world = World::empty(c.clone());
            if nearer {
                set(&mut world, 4, 6, 0, Material::Rock);
                set(&mut world, 4, 7, 0, Material::Rock);
            }
            world.apply(Command::AddWater { x: 4, y: 6, z: 1, volume_m3: c.voxel_volume() });
            world
        };
        assert_ne!(
            present(&pool(false)).0.pixels(),
            present(&World::empty(config())).0.pixels(),
            "the water must be visible with nothing in front of it"
        );
        let mut wall = World::empty(config());
        set(&mut wall, 4, 6, 0, Material::Rock);
        set(&mut wall, 4, 7, 0, Material::Rock);
        assert_eq!(
            present(&pool(true)).0.pixels(),
            present(&wall).0.pixels(),
            "a nearer wall must occlude the water behind it"
        );

        // And a *ledge* is not a cover: one nearer block leaves the far block's top face
        // showing above its cap, which is what reads as depth.
        let mut ledge = World::empty(config());
        set(&mut ledge, 4, 6, 1, Material::Rock);
        set(&mut ledge, 4, 6, 0, Material::Rock);
        let mut bare = World::empty(config());
        set(&mut bare, 4, 6, 0, Material::Rock);
        assert_ne!(
            present(&ledge).0.pixels(),
            present(&bare).0.pixels(),
            "the far block's top face must survive a one-block ledge"
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

    /// A world periodic in `x` renders to a raster periodic in pixel columns, water
    /// included: the render of the strip is a *crop* of the infinite tiling, so shifting
    /// every feature by half the strip rotates the picture by exactly half the raster, and
    /// the first and last pixel columns are adjacent samples of the same field.
    ///
    /// Adjacent, not equal — they are a voxel apart, like any other pair across a voxel
    /// boundary — so the seam is tested as a step no larger than the largest interior
    /// step, not as an equality.
    #[test]
    fn a_cropped_periodic_tiling_matches_the_wrapped_render_across_the_seam() {
        fn ridge(c: Config, shift: i64) -> World {
            let mut world = World::empty(c.clone());
            let w = i64::from(c.width);
            let vol = c.voxel_volume();
            for x in 0..w {
                let u =
                    (x - shift).rem_euclid(w) as f64 / w as f64 * std::f64::consts::TAU;
                let top = (4.0 + 3.0 * u.cos()).round() as u32;
                for z in 0..c.depth {
                    for y in 1..=top {
                        let m = if y == top { Material::Soil } else { Material::Rock };
                        world.apply(Command::SetMaterial { x, y, z, material: m });
                    }
                    // Translucent water standing in every trough, so the seam is judged
                    // with a blended face crossing it and not only with opaque rock.
                    if top <= 2 {
                        for y in top + 1..=3 {
                            world.apply(Command::AddWater { x, y, z, volume_m3: vol });
                        }
                    }
                }
            }
            world
        }

        let c = config();
        let half = i64::from(c.width) / 2;
        let (a, proj) = present(&ridge(c.clone(), 0));
        let (b, _) = present(&ridge(c.clone(), half));

        // B is A rotated by half the raster's columns, pixel for pixel: the tiling is
        // periodic and the render is a crop of it.
        let w = usize::from(proj.raster_w);
        let rows = usize::from(proj.raster_h);
        let shift = half as usize * proj.s as usize;
        for row in 0..rows {
            for x in 0..w {
                assert_eq!(
                    b.pixels()[row * w + (x + shift) % w],
                    a.pixels()[row * w + x],
                    "row {row}, column {x}"
                );
            }
        }

        // And the seam is a plain adjacency: the change from the last pixel column to the
        // first is no bigger than the largest change between any interior pair.
        let diff = |i: usize, j: usize| -> f32 {
            (0..rows)
                .map(|row| {
                    let (p, q) = (a.pixels()[row * w + i], a.pixels()[row * w + j]);
                    (0..3).map(|k| (p[k] - q[k]).abs()).sum::<f32>()
                })
                .sum()
        };
        let interior = (1..w).map(|x| diff(x - 1, x)).fold(0.0f32, f32::max);
        let seam = diff(w - 1, 0);
        assert!(
            seam <= interior + 1e-6,
            "the seam step {seam} exceeds the largest interior step {interior}"
        );
        assert!(interior > 0.0, "the fixture must actually vary across x");
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
        let col = proj.col(4) + 1;
        let (_, r0, _, s) = proj.front_rect(4, 4, 0);
        let at = |row: i32| pixel(&canvas, &proj, col, row);

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

    /// A stacked water column draws exactly one surface: the free-water top belongs to the
    /// water/air boundary at `y + free`, and the full cells beneath it are body, not a pile
    /// of translucent skins each blending again.
    ///
    /// Read as run structure down one pixel column: sky, then one unbroken run of surface
    /// rows (the top face and the skin row it caps), then body all the way to the floor.
    #[test]
    fn a_stacked_water_column_draws_exactly_one_surface() {
        let c = Config { depth: 1, height: 16, ..config() };
        let mut world = World::empty(c.clone());
        for x in 2..8 {
            for y in 0..3 {
                set(&mut world, x, y, 0, Material::Rock);
            }
        }
        let vol = c.voxel_volume();
        for x in 2..8 {
            // Four full cells and a half one on top: one boundary, at y = 7.5.
            for y in 3..7 {
                world.apply(Command::AddWater { x, y, z: 0, volume_m3: vol });
            }
            world.apply(Command::AddWater { x, y: 7, z: 0, volume_m3: vol * 0.5 });
        }

        let (canvas, proj) = present(&world);
        let col = proj.col(4) + 1;
        let floor = proj.front_row(2, 0); // the first row below the water column
        let surface_top = proj.front_row(7, 0) + proj.s as i32 - fill_px(0.5, proj.s) - proj.rise as i32;

        // Classify every row of the column between the sky and the rock floor.
        let body = pixel(&canvas, &proj, col, floor - 1);
        let surface = pixel(&canvas, &proj, col, surface_top);
        assert!(bluer_than_sky(body) && bluer_than_sky(surface), "both must be water");
        assert!(
            surface[1] > body[1] * 1.5,
            "the surface must be distinguishable from the body: {surface:?} vs {body:?}"
        );
        let classify = |row: i32| -> &'static str {
            let px = pixel(&canvas, &proj, col, row);
            if px == sky() {
                "sky"
            } else if (px[1] - surface[1]).abs() < (px[1] - body[1]).abs() {
                "surface"
            } else {
                "body"
            }
        };
        let runs: Vec<(&str, usize)> = (0..floor).map(classify).fold(Vec::new(), |mut v, k| {
            match v.last_mut() {
                Some((last, n)) if *last == k => *n += 1,
                _ => v.push((k, 1)),
            }
            v
        });
        let surfaces = runs.iter().filter(|(k, _)| *k == "surface").count();
        assert_eq!(surfaces, 1, "exactly one surface, got runs {runs:?}");
        assert_eq!(
            runs.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            vec!["sky", "surface", "body"],
            "sky, one surface, then body to the floor: {runs:?}"
        );
        // The surface run is the top face plus the skin row it caps.
        assert_eq!(runs[1].1, proj.rise as usize + 1);
    }

    /// A plateau that recedes in `z` reads as one plane, not as ruled shelving: the
    /// interior slabs' front faces are culled, the surviving top faces tile row by row, and
    /// the brightness down that band is monotone — no rim line and no contour row anywhere
    /// inside it.
    #[test]
    fn a_plateau_receding_in_z_has_no_ruled_bands() {
        let c = Config { width: 16, height: 12, depth: 8, ..Config::default() };
        let mut world = World::empty(c.clone());
        for x in 0..i64::from(c.width) {
            for z in 0..c.depth {
                for y in 0..=5 {
                    set(&mut world, x, y, z, Material::Rock);
                }
            }
        }
        let (canvas, proj) = present(&world);
        let col = proj.col(8) + 2;
        // The plateau's surface band: `depth · rise` rows above the near cut wall.
        let near_top = proj.front_row(5, 0);
        let rows: Vec<i32> = (1..=(proj.depth * proj.rise) as i32).map(|k| near_top - k).collect();
        assert!(rows.iter().all(|&r| r >= 0), "the band must be inside the raster");

        let lums: Vec<f32> =
            rows.iter().map(|&r| lum(pixel(&canvas, &proj, col, r))).collect();
        // Going up is going back, which is hazier, so the band only ever dims. A ruled
        // shelf is a row that breaks that: darker than the row below *and* than the one
        // above it.
        for w in lums.windows(2) {
            assert!(
                w[1] <= w[0] + 1e-6,
                "the receding plateau must not brighten going back: {lums:?}"
            );
        }
        for i in 1..lums.len() - 1 {
            assert!(
                !(lums[i] < lums[i - 1] - 1e-6 && lums[i] < lums[i + 1] - 1e-6),
                "row {i} of the plateau is a contour line: {lums:?}"
            );
        }
        // And it really is the top face all the way, not a stack of fronts: the whole band
        // is brighter than the cut wall below it.
        let wall = lum(pixel(&canvas, &proj, col, near_top + 2));
        assert!(
            lums.iter().all(|&l| l > wall),
            "the band must be top faces, not front faces: {lums:?} vs wall {wall}"
        );
    }

    /// The roof shadow is an occlusion blend that falls off downward from the thing casting
    /// it: a floor directly under an overhang is dark, a floor far below the same overhang
    /// is not, and an open floor is fully lit.
    #[test]
    fn a_roof_shadow_falls_off_with_depth_below_the_roof() {
        let c = Config { width: 48, height: 24, depth: 1, ..Config::default() };
        let mut world = World::empty(c.clone());
        // One roof at y = 18 over x = 8..28, with sheltered floors at y = 16 (gap 2) and
        // y = 2 (gap 16) under it, and an open floor at y = 2 outside it.
        for x in 8..28 {
            set(&mut world, x, 18, 0, Material::Rock);
        }
        for x in 8..16 {
            set(&mut world, x, 16, 0, Material::Rock);
        }
        for x in 16..28 {
            set(&mut world, x, 2, 0, Material::Rock);
        }
        for x in 32..40 {
            set(&mut world, x, 2, 0, Material::Rock);
        }

        let (canvas, proj) = present(&world);
        let cap = |x: i64, y: u32| -> f32 {
            let (c0, r0, _, _) = proj.front_rect(x, y, 0);
            lum(pixel(&canvas, &proj, c0 + 1, r0 - 1))
        };
        let (under, deep, open) = (cap(12, 16), cap(20, 2), cap(36, 2));
        assert!(under < deep, "a floor right under the roof must be darker: {under} vs {deep}");
        assert!(deep < open, "a floor far below the roof must still be lit: {deep} vs {open}");
        assert!(
            deep > under + (open - under) * 0.5,
            "the falloff must be most of the way back to the light by 16 voxels: \
             {under} / {deep} / {open}"
        );
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

    /// A raster shorter than the projection keeps its bottom registration: the floor line
    /// and everything near it are pixel for pixel what the full raster has, and only rows
    /// off the top are lost.
    #[test]
    fn a_cropped_raster_keeps_the_bottom_registration() {
        let c = Config { width: 16, height: 20, depth: 4, ..Config::default() };
        let world = super::super::scene::authored(c.clone());
        let cfg = VoxelConfig::default();
        let (full, fp) = present_with(cfg.clone(), &world);
        let short_h = fp.raster_h / 2;
        let (short, sp) =
            present_with(VoxelConfig { raster_height: short_h, ..cfg }, &world);
        assert!(sp.cropped && sp.raster_h == short_h);

        let w = usize::from(fp.raster_w);
        let lost = usize::from(fp.raster_h - short_h);
        for row in 0..usize::from(short_h) {
            assert_eq!(
                &short.pixels()[row * w..(row + 1) * w],
                &full.pixels()[(row + lost) * w..(row + lost + 1) * w],
                "cropped row {row} must be full row {}",
                row + lost
            );
        }
    }
}
