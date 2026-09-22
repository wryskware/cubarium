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
//! **top** is drawn only at a water/air boundary, at `y + free` — a cell drawn full with
//! more water above it is not an extra translucent surface, while a cell whose water stops
//! short of its own brim always is one, because then the air gap is inside that cell
//! whatever stands over it — and those tops tile upward across slabs into the pool's
//! receding surface. [`water_open_up`] is that test, in drawn pixels and not in the raw
//! fraction, so a settled body whose interior equalizes to a hair under one stays one body
//! rather than hatching a bright skin across every slab of it. A
//! water body is clipped to the rows the nearer slab's water or rock has not already
//! claimed, and a water top is clipped **row by row** by [`VoxelPresenter::nearer_owns`]:
//! its rows are an interval, a nearer roof or skin can own part of that interval and not
//! the rest, and keeping or dropping the whole band would either blend a row twice or lose
//! a row of visible water. Either way a pool many slabs deep blends one water face per
//! pixel instead of multiplying its alpha into a striped wall. Pore water darkens soil
//! toward the water colour instead of filling it.
//!
//! # Stands
//!
//! A plant is not a sprite hung on the picture: it is **voxels**, decomposed by
//! [`super::stand`] into a trunk column and a horizontal crown disc, and stamped inside
//! this same traversal at each cell's own `(x, y, z)`. That is the whole of its depth
//! sorting. A stand is hidden by nearer terrain because the nearer slab paints later; it
//! hides farther terrain and farther water because its own slab paints later than
//! theirs; and within one cell the plant is stamped **before** the water, so a trunk
//! standing in a pool is submerged under the water's blend instead of painted over it.
//!
//! # Animals
//!
//! The same, one layer later: an animal is voxels too ([`super::animal`]), stamped in this
//! traversal at each cell's own `(x, y, z)`, after the plant of its cell and before that
//! cell's water. Until the art direction lands it is an **interim** glyph — a 2x1x2 block
//! in a placeholder colour — and the presenter draws it with the plainest stamp in this
//! file on purpose.
//!
//! Terrain's own face culling never consults the plants, and it does not need to: a
//! plant is opaque and always draws after whatever it covers, so the only thing a cull
//! would save is work. The one test a plant cell does make is whether the cell above it
//! is occupied, plant or terrain — because a rim row is *not* fixed by overpainting, and
//! a trunk with a rim on every voxel is a banded pole rather than a stem.
//!
//! Depth tint: every colour in slice `z` is mixed toward the haze colour by
//! `haze · z / (depth − 1)`, so the back wall recedes and the front reads as the front.

use std::sync::LazyLock;

use cubarium_render::Canvas;
use cubarium_voxel::{Material, VoxelView};
use cubarium_voxel_fauna::FaunaView;
use cubarium_voxel_flora::FloraView;
use cube_proto::Face;

use crate::present::{mix, srgb_linear};

use super::VoxelConfig;
use super::animal::{AnimalPart, Animals};
use super::appearance::{self, FaceTexel, Pigment};
use super::project::Projection;
use super::stand::{Part, Stands};

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
pub const TOP_GAIN: f32 = 1.8;
/// How far a top face leans toward [`LIGHT_SRGB`].
pub const TOP_TINT: f32 = 0.15;
/// How far a top face's back row shades back toward the front colour — the contour that
/// says "the ground ends here going back". Drawn only where the surface really does end
/// there, and only when the depth step is 3 px or more: at 2 px it is a 1-pixel hatch.
pub const TOP_BACK: f32 = 0.24;
/// How much of the top-face light reaches a face directly beneath a roof — the floor of a
/// cave, the ground under an overhang. Without this a covered passage is lit exactly like
/// open ground and reads as an artefact rather than as a hole.
pub const ROOF_LIGHT: f32 = 0.22;
/// Voxels of separation over which a roof's shadow fades back to full light. The shadow
/// belongs to the sheltered surface under the lip, not to every face below it: a canyon
/// floor twenty voxels under the same overhang is lit, not blacked out to the frame edge.
pub const ROOF_FALLOFF_VOXELS: f32 = 4.0;
/// How far the front face's top row leans toward the top face: the rim light.
pub const RIM: f32 = 0.28;
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
/// How much brighter a plant's top face is than the front face below it. Gentler than
/// [`TOP_GAIN`] on purpose: canopy colours start bright, and the terrain's gain would
/// clip a magenta crown to white.
pub const PLANT_TOP_GAIN: f32 = 1.5;
/// How far a plant's top face leans toward [`LIGHT_SRGB`].
pub const PLANT_TOP_TINT: f32 = 0.14;
/// How far a plant's front top row leans toward its own top face: the rim that reads as
/// a lit upper edge. Drawn only where the cell above is empty.
pub const PLANT_RIM: f32 = 0.42;
/// The trunk's cylinder, darkest and brightest column multipliers. One voxel is four
/// pixels across, so a stem is round only if those four pixels say so.
pub const TRUNK_SHADE: [f32; 2] = [0.62, 1.22];
/// Where across the trunk the light falls, as a fraction of its width.
pub const TRUNK_LIGHT_AT: f32 = 0.35;
/// How far a crown's silhouette column darkens: the canopy's own outline, drawn only
/// where the disc ends and never between two cells of one crown.
pub const CROWN_EDGE: f32 = 0.70;
/// How far a crown's front face — the canopy's skirt, seen from under the leaves —
/// shades toward the stand's own wood. Without it a canopy several voxels across reads
/// as a solid slab of colour rather than as a lit surface with a shaded underside.
pub const CROWN_UNDER: f32 = 0.22;

/// A water surface row blends this much harder than the body.
pub const SKIN_ALPHA_GAIN: f32 = 1.7;
/// How hard a water *top* face blends relative to the surface row it caps.
pub const WATER_TOP_ALPHA: f32 = 0.8;
/// Below this fraction a voxel is dry and draws nothing.
///
/// `pub(crate)` for the GPU packer, which must call a cell dry at exactly the same
/// fraction this presenter does (`crate::sink::gpu::voxel`): the voxel texture carries a
/// quantised fraction and a zero there *means* dry, so the threshold has to be this one
/// and not a second copy of it.
pub(crate) const WATER_EPSILON: f32 = 1e-4;

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

/// The sky zenith colour, in linear light. What the presenter clears to at the top.
pub fn sky() -> [f32; 3] {
    STRATA.sky
}

/// The dusky horizon sky colour.
pub const SKY_HORIZON_SRGB: u32 = 0x002E_1B4D;

/// The sky horizon colour, in linear light.
pub fn sky_horizon() -> [f32; 3] {
    mul(srgb_linear(SKY_HORIZON_SRGB), 0.35)
}

/// The sky colour at raster row `py` of height `h`.
pub fn sky_at(py: i32, h: i32) -> [f32; 3] {
    let t = (py as f32 / h.max(1) as f32).clamp(0.0, 1.0);
    mix(sky(), sky_horizon(), t)
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

/// Micro-dithering grain across solid voxel faces to break up flat surfaces
#[inline]
fn face_grain(x: i64, y: u32, z: u32, dx: i32, dy: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(73856093)
        ^ y.wrapping_mul(19349663)
        ^ z.wrapping_mul(83492791)
        ^ (dx as u32).wrapping_mul(2654435761)
        ^ (dy as u32).wrapping_mul(38291);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h & 15) as i32 - 7) as f32 * 0.005
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
    /// The frame's stands, as voxels. Rebuilt per frame beside the roof map.
    stands: Stands,
    /// The frame's animals, as voxels, on the same grid and in the same traversal. Empty
    /// for a run with no animal layer, which is every run before voxel round 5c.
    animals: Animals,
}

impl VoxelPresenter {
    pub fn new(cfg: VoxelConfig, proj: Projection) -> VoxelPresenter {
        VoxelPresenter {
            cfg,
            proj,
            said_cropped: false,
            roof: Vec::new(),
            stands: Stands::empty(0, 0, 0),
            animals: Animals::empty(0, 0, 0),
        }
    }

    pub fn projection(&self) -> Projection {
        self.proj
    }

    /// Paint one frame. The canvas must be the single-chart ring the projection sized.
    ///
    /// The flora view is the plant layer's stands, drawn into the same traversal as the
    /// terrain and the water — see the module header. A run with no plants passes an
    /// empty view and pays one branch per voxel for it.
    pub fn draw(&mut self, view: &VoxelView<'_>, flora: FloraView<'_>, canvas: &mut Canvas) {
        self.draw_with_fauna(view, flora, None, canvas);
    }

    /// The same frame with an animal layer in it: the bodies are stamped into the same
    /// traversal as the terrain, the water and the plants, from their own occupancy grid
    /// ([`super::animal`]), each cell where a solid block at its own `(x, y, z)` would be.
    /// `None` is a run with no fauna and is what [`VoxelPresenter::draw`] passes, so a
    /// caller that has no animals draws exactly the picture it drew before.
    pub fn draw_with_fauna(
        &mut self,
        view: &VoxelView<'_>,
        flora: FloraView<'_>,
        fauna: Option<FaunaView<'_>>,
        canvas: &mut Canvas,
    ) {
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

        if self.cfg.sky_gradient {
            let cw = canvas.width() as i32;
            let ch = canvas.height() as i32;
            for r in 0..ch {
                let row_sky = sky_at(r, ch);
                for c in 0..cw {
                    canvas.pixels_mut()[r as usize * cw as usize + c as usize] = row_sky;
                }
            }

            // If atmospheric moisture exists aloft, draw soft drifting mist wisps across the upper sky
            if view.atmosphere_m3 > 0.0 {
                let moisture_factor = (view.atmosphere_m3 as f32 / 1.5).clamp(0.1, 1.0);
                let drift_x = (view.tick / 2) as i32;
                let mist_rows = (canvas.height() / 3).min(56) as i32;
                let cloud_c = srgb_linear(0x0036_2858);
                let cloud_edge = srgb_linear(0x005E_72A0);
                let w = canvas.width() as i32;
                for r in 0..mist_rows {
                    let vertical_t = 1.0 - (r as f32 / mist_rows as f32);
                    let row_factor = vertical_t * vertical_t * moisture_factor;
                    for c in 0..w {
                        let wave1 = ((c + drift_x + r * 4) as f32 * 0.045).sin() * 0.5 + 0.5;
                        let wave2 = ((c * 2 - drift_x + 37) as f32 * 0.025).cos() * 0.5 + 0.5;
                        let density = wave1 * wave2 * row_factor;
                        if density > 0.10 {
                            let alpha = ((density - 0.10) * 1.8).min(0.70);
                            let col = mix(cloud_c, cloud_edge, wave1 * 0.5);
                            let idx = r as usize * canvas.width() as usize + c as usize;
                            canvas.pixels_mut()[idx] = mix(canvas.pixels()[idx], col, alpha);
                        }
                    }
                }
            }
        } else {
            canvas.pixels_mut().fill(STRATA.sky);
        }

        let surf = Surface {
            w: i32::from(canvas.width()),
            h: i32::from(canvas.height()),
        };
        let p = self.proj;
        let water_alpha = self.cfg.water_alpha.clamp(0.0, 1.0);

        self.build_roof(view);
        self.stands.rebuild(view, flora);
        let plants = !self.stands.is_empty();
        self.animals.rebuild(view, fauna);
        let beasts = !self.animals.is_empty();

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
                    let shade = || {
                        roof_shade(self.roof[(col + x as usize) * p.height as usize + y as usize])
                    };
                    if m.is_solid() {
                        let f = self.facing(view, x, y, z);
                        if f.front_hidden && (f.top_hidden || !f.open_up) {
                            continue;
                        }
                        self.block(view, canvas, surf, x, y, z, m, &f, shade());
                        continue;
                    }
                    // A plant stands in the void, and the water of its own cell blends
                    // over it: a trunk in a pool is submerged, not painted across the
                    // surface it stands in.
                    let part = if plants {
                        self.stands.at(x, i64::from(y), z)
                    } else {
                        Part::None
                    };
                    if part != Part::None {
                        self.plant(view, canvas, surf, x, y, z, part, shade());
                    }
                    // The animal after the plant in its own cell — a body standing in a
                    // turf covers the turf — and before the water, so one standing in a
                    // pool is submerged under the water's blend like everything else.
                    let beast = if beasts {
                        self.animals.at(x, i64::from(y), z)
                    } else {
                        AnimalPart::None
                    };
                    if beast != AnimalPart::None {
                        self.animal(view, canvas, surf, x, y, z, beast, shade());
                    }
                    let free = view.free_at(x, y, z) as f32;
                    if free > WATER_EPSILON {
                        self.water(view, canvas, surf, x, y, z, free, water_alpha);
                    }
                }
            }
        }

        // If rain is active this tick, draw animated falling rain streaks across the scene
        if self.cfg.sky_gradient && view.is_raining() {
            let rain_c = srgb_linear(WATER_SURFACE_SRGB);
            let w = canvas.width() as i32;
            let h = canvas.height() as i32;
            let tick = view.tick as i32;
            let speed = 4;
            for c in 0..w {
                let hash = ((c.wrapping_mul(1664525).wrapping_add(1013904223)) as u32 >> 16) as i32;
                if hash % 5 != 0 {
                    continue;
                }
                let y_offset = (tick * speed + hash).rem_euclid(32);
                let mut r = y_offset;
                while r < h {
                    let streak_len = 3 + (hash & 1);
                    for len in 0..streak_len {
                        let py = r + len;
                        let px = c + (len / 2);
                        if py < h {
                            let wrapped_px = px.rem_euclid(w);
                            let idx = py as usize * w as usize + wrapped_px as usize;
                            let alpha = if len == streak_len - 1 { 0.40 } else { 0.20 };
                            canvas.pixels_mut()[idx] = mix(canvas.pixels()[idx], rain_c, alpha);
                        }
                    }
                    r += 32 + (hash % 16);
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
            riser: open_up && nearer && !solid(view, x, yi, z - 1) && solid(view, x, yi - 1, z - 1),
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
        let wet = if m.pore_capacity() > 0.0 {
            view.pore_at(x, y, z).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let body = mix(strata_of(m), STRATA.water_deep, wet * WET);
        let lit_fn = |b: [f32; 3]| {
            let mut l = mix(mul(b, TOP_GAIN), STRATA.light, TOP_TINT);
            if shade < 1.0 {
                l = mix(b, l, shade);
            }
            l
        };

        let (c0, r0, w, h) = p.front_rect(x, y, z);
        let cols = w as i32;
        let dither = self.cfg.dither;
        let dither_scale = dither / 0.04;
        if !f.front_hidden {
            if f.riser {
                // Ground stepping one voxel into depth: shade it as a slope and let its
                // haze cross from this slab to the nearer one, with no rim contour.
                for dy in 0..h as i32 {
                    let t = if h > 1 {
                        dy as f32 / (h - 1) as f32
                    } else {
                        0.0
                    };
                    for dx in 0..cols {
                        let b = if dither > 0.0 {
                            mul(body, 1.0 + face_grain(x, y, z, dx, dy) * dither_scale)
                        } else {
                            body
                        };
                        let l = lit_fn(b);
                        let slope = mix(l, b, RISER_LEAN * t);
                        let c = hazed(slope, self.haze_at(z as f32 - t));
                        let e = hazed(mul(slope, EDGE_DARK), self.haze_at(z as f32 - t));
                        let on_side = (dx == 0 && f.open_left) || (dx + 1 == cols && f.open_right);
                        surf.put(canvas, c0 + dx, r0 + dy, if on_side { e } else { c });
                    }
                }
            } else {
                for dy in 0..h as i32 {
                    for dx in 0..cols {
                        let b = if dither > 0.0 {
                            mul(body, 1.0 + face_grain(x, y, z, dx, dy) * dither_scale)
                        } else {
                            body
                        };
                        let l = lit_fn(b);
                        let front = hazed(b, haze);
                        let rim = hazed(mix(b, l, RIM), haze);
                        let edge = hazed(mul(b, EDGE_DARK), haze);
                        let top = hazed(l, haze);
                        let on_cap = dy == 0 && f.open_up;
                        let on_side = (dx == 0 && f.open_left) || (dx + 1 == cols && f.open_right);
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
                for dx in 0..cols {
                    let b = if dither > 0.0 {
                        mul(body, 1.0 + face_grain(x, y, z, dx, dy + 100) * dither_scale)
                    } else {
                        body
                    };
                    let l = lit_fn(b);
                    let plane = if dy == 0 && rise > 2 && !f.back_continues {
                        mix(l, b, TOP_BACK)
                    } else {
                        l
                    };
                    let face = hazed(plane, hz);
                    let bevel = hazed(mix(plane, b, TOP_EDGE), hz);
                    let on_drop = (dx == 0 && f.drop_left) || (dx + 1 == cols && f.drop_right);
                    surf.put(
                        canvas,
                        c0 + dx,
                        r0 - rise + dy,
                        if on_drop { bevel } else { face },
                    );
                }
            }
        }
    }

    /// Draw one plant voxel: an opaque pixel-art stack in the cell's own front and top
    /// rectangles, at the moment of the traversal a solid block there would have.
    ///
    /// Hand-drawn, not stamped from the baked atlas. `assets/atelier`'s `bloomcrown` and
    /// `umbrellafrond` are 16×16 *top-down* canopy tiles for the cube's faces — radial
    /// discs with no stem and no side silhouette — inside a 9-pixel extent budget, and
    /// nothing in them can carry a crown fill or a wilt. At 4 px per voxel in this
    /// elevated-orthographic view a stand is a 4-pixel-wide stem under a canopy a few
    /// voxels across, so the tiles would have to be reprojected and recoloured per frame
    /// to say what the model says. Placeholder geometry it is; real texture comes later.
    #[allow(clippy::too_many_arguments)]
    fn plant(
        &self,
        view: &VoxelView<'_>,
        canvas: &mut Canvas,
        surf: Surface,
        x: i64,
        y: u32,
        z: u32,
        part: Part,
        shade: f32,
    ) {
        let Some(style) = self.stands.style(part) else {
            return;
        };
        let p = self.proj;
        let (s, rise) = (p.s as i32, p.rise as i32);
        let yi = i64::from(y);
        let haze = self.haze_at(z as f32);
        let (c0, r0, w, _) = p.front_rect(x, y, z);
        let cols = w as i32;

        // Whatever stands in the cell above closes this one's cap, plant or terrain. The
        // one test a plant makes: overpainting fixes a covered face, but it does not fix
        // a rim row, and a rim on every voxel of a stem is a banded pole.
        let covered_up = solid(view, x, yi + 1, z) || self.stands.at(x, yi + 1, z).is_block();

        let open_left = !self.stands.crown_continues(part, x - 1, y, z);
        let open_right = !self.stands.crown_continues(part, x + 1, y, z);
        let class = appearance::plant_class(part);
        let glyph = appearance::plant_glyph(part, open_left, open_right);

        for dy in 0..s {
            for dx in 0..cols {
                let texel = appearance::front_texel(class, glyph, dx as u32, dy as u32, p.s);
                if let Some(c) = glyph_front_colour(style, texel, shade, covered_up) {
                    surf.put(canvas, c0 + dx, r0 + dy, hazed(c, haze));
                }
            }
        }

        if part.is_block() && !covered_up {
            for dy in 0..rise {
                let zf = z as f32 + (rise - 1 - dy) as f32 / rise as f32;
                let hz = self.haze_at(zf);
                for dx in 0..cols {
                    let texel = appearance::cap_texel(class, glyph, dx as u32, dy as u32, p.s);
                    let cap = glyph_cap_colour(style, texel, shade);
                    surf.put(canvas, c0 + dx, r0 - rise + dy, hazed(cap, hz));
                }
            }
        }
    }

    /// One cell of an animal's glyph: an articulated body/head for founders,
    /// or a flat block in the placeholder colour for the interim fallback.
    #[allow(clippy::too_many_arguments)]
    fn animal(
        &self,
        view: &VoxelView<'_>,
        canvas: &mut Canvas,
        surf: Surface,
        x: i64,
        y: u32,
        z: u32,
        part: AnimalPart,
        shade: f32,
    ) {
        let Some(style) = self.animals.style(part) else {
            return;
        };
        let p = self.proj;
        let (s, rise) = (p.s as i32, p.rise as i32);
        let yi = i64::from(y);
        let haze = self.haze_at(z as f32);
        let (c0, r0, w, _) = p.front_rect(x, y, z);
        let cols = w as i32;

        // Whatever stands in the cell above closes this one's cap — terrain, a plant's
        // block, or the rest of a taller animal, when there is ever one.
        let covered_up = solid(view, x, yi + 1, z)
            || self.stands.at(x, yi + 1, z).is_block()
            || self.animals.at(x, yi + 1, z).is_block();

        let glyph = part.glyph();

        for dy in 0..s {
            for dx in 0..cols {
                let texel = appearance::front_texel(
                    appearance::ANIMAL_PART_CLASS,
                    glyph,
                    dx as u32,
                    dy as u32,
                    p.s,
                );
                if let Some(c) = glyph_front_colour(style, texel, shade, covered_up) {
                    surf.put(canvas, c0 + dx, r0 + dy, hazed(c, haze));
                }
            }
        }
        if !covered_up {
            for dy in 0..rise {
                let zf = z as f32 + (rise - 1 - dy) as f32 / rise as f32;
                let hz = self.haze_at(zf);
                for dx in 0..cols {
                    let texel = appearance::cap_texel(
                        appearance::ANIMAL_PART_CLASS,
                        glyph,
                        dx as u32,
                        dy as u32,
                        p.s,
                    );
                    surf.put(
                        canvas,
                        c0 + dx,
                        r0 - rise + dy,
                        hazed(glyph_cap_colour(style, texel, shade), hz),
                    );
                }
            }
        }
    }

    /// Does the slab one step nearer already own screen `row`, in the pixel columns of
    /// voxel `(x, y, z)`?
    ///
    /// Face ownership is an interval, not a flag: a water top's rows overlap the bands of
    /// several nearer voxels, and a nearer face may own some of those rows and not others.
    /// Writing `R = front_row(y, z)` and `F = front_row(y', z − 1) = R + rise − (y' − y)·s`,
    /// voxel `(x, y', z − 1)` claims
    ///
    /// - `F − rise .. F + s` when it is **solid**: its own front, plus whichever of its cap
    ///   or the front of `(x, y' + 1, z − 1)` sits over that front — the two rules the
    ///   module header states for `front_hidden` and `top_hidden`, read as one band;
    /// - `skin − rise .. F + s` when it holds **water**, `skin` being its own surface row:
    ///   its body, and its top where it has one.
    ///
    /// Anything a face of `(x, y, z)` can write lies in `R − rise .. R + s`, and
    /// `F + s > R − rise` needs `y' − y < 2·rise/s + 1 ≤ 3` while `F − rise < R + s` needs
    /// `y' ≥ y`: the three voxels `y' = y, y + 1, y + 2` are the whole of it.
    ///
    /// The bands are the *unclipped* ones on purpose. Where the nearer slab's own face is
    /// itself culled, whatever culls it is nearer still, so the row is owned either way —
    /// which is what makes one step of this test stand for the whole stack in front.
    fn nearer_owns(&self, view: &VoxelView<'_>, x: i64, y: u32, z: u32, row: i32) -> bool {
        if z == 0 {
            return false;
        }
        let (s, rise) = (self.proj.s as i32, self.proj.rise as i32);
        let r = self.proj.front_row(y, z);
        (0..3i64).any(|dy| {
            let yn = i64::from(y) + dy;
            let f = r + rise - dy as i32 * s;
            let top = if solid(view, x, yn, z - 1) {
                f - rise
            } else {
                let fill = fill_px_at(view, x, yn, z - 1, self.proj.s);
                if fill == 0 {
                    return false;
                }
                let skin = f + s - fill;
                if water_open_up(view, x, yn, z - 1, self.proj.s) {
                    skin - rise
                } else {
                    skin
                }
            };
            (top..f + s).contains(&row)
        })
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

        // Only a water/air boundary is a surface, and a partial cell always has one.
        let open_up = water_open_up(view, x, yi, z, p.s);

        // What the slab one step nearer already owns in these pixel columns.
        let near_solid = z > 0 && solid(view, x, yi, z - 1);
        let near_fill = if z > 0 {
            fill_px_at(view, x, yi, z - 1, p.s)
        } else {
            0
        };
        let near_skin = r0 + rise + s - near_fill;
        let near_open_up = z > 0 && water_open_up(view, x, yi, z - 1, p.s);

        // The surface's own top face, receding into depth exactly as a block's does — but
        // clipped row by row rather than kept or dropped whole. Its rows are an interval,
        // and a nearer face (a roof, a nearer pool's skin) can own part of that interval
        // without owning all of it: cull the band and a visible row of water is lost, keep
        // it and an owned row is blended twice. Each surviving row is blended exactly once.
        if open_up {
            for dy in 0..rise {
                let row = skin_row - rise + dy;
                if self.nearer_owns(view, x, y, z, row) {
                    continue;
                }
                let zf = z as f32 + (rise - 1 - dy) as f32 / rise as f32;
                let c = hazed(STRATA.water_surface, self.haze_at(zf));
                for dx in 0..cols {
                    surf.blend(canvas, c0 + dx, row, c, skin_alpha * WATER_TOP_ALPHA);
                }
            }
        }

        // The body, clipped to the rows no nearer slab has claimed: one water face per
        // pixel, so depth recedes instead of multiplying the alpha into a striped wall.
        let stop = if near_solid {
            skin_row
        } else if near_fill > 0 {
            bottom.min(if near_open_up {
                near_skin - rise
            } else {
                near_skin
            })
        } else {
            bottom
        };
        for row in skin_row..stop {
            let is_skin = row == skin_row && open_up;
            let (rgb, a) = if is_skin {
                (skin, skin_alpha)
            } else {
                (body, alpha)
            };
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

/// A plant colour as its own top face: brighter, leaning to the scene light, and shaded
/// by the same roof occlusion the terrain's caps take.
#[inline]
fn plant_lit(c: [f32; 3], shade: f32) -> [f32; 3] {
    let lit = mix(mul(c, PLANT_TOP_GAIN), STRATA.light, PLANT_TOP_TINT);
    if shade < 1.0 { mix(c, lit, shade) } else { lit }
}

fn glyph_pigment(style: super::stand::Style, texel: FaceTexel) -> [f32; 3] {
    match texel.pigment() {
        Pigment::Primary => style.wood,
        Pigment::Secondary => style.crown,
        Pigment::Accent => style.heart,
    }
}

fn glyph_gain(tone: u8) -> f32 {
    0.5 + f32::from(tone & 15) / 16.0
}

fn glyph_front_colour(
    style: super::stand::Style,
    texel: FaceTexel,
    shade: f32,
    covered_up: bool,
) -> Option<[f32; 3]> {
    let base = glyph_pigment(style, texel);
    Some(match texel.tone() {
        appearance::TONE_TRANSPARENT => return None,
        appearance::TONE_OUTLINE => mul(base, EDGE_DARK),
        appearance::TONE_OPEN_RIM if !covered_up => mix(base, plant_lit(base, shade), PLANT_RIM),
        appearance::TONE_LIT => plant_lit(base, shade),
        appearance::TONE_UNDER => mix(base, style.wood, CROWN_UNDER),
        appearance::TONE_UNDER_EDGE => mul(mix(base, style.wood, CROWN_UNDER), CROWN_EDGE),
        tone @ 16..=31 => mul(base, glyph_gain(tone)),
        _ => base,
    })
}

fn glyph_cap_colour(style: super::stand::Style, texel: FaceTexel, shade: f32) -> [f32; 3] {
    let base = glyph_pigment(style, texel);
    match texel.tone() {
        appearance::TONE_OUTLINE => mul(plant_lit(base, shade), CROWN_EDGE),
        tone @ 32..=47 => mix(
            plant_lit(base, shade),
            mul(base, glyph_gain(tone)),
            TOP_EDGE,
        ),
        _ => plant_lit(base, shade),
    }
}

/// Rows of a voxel's front rectangle that a `free` fraction fills: at least one, so any
/// water at all is visible, and never more than the whole face.
#[inline]
fn fill_px(free: f32, s: u32) -> i32 {
    (free.clamp(0.0, 1.0) * s as f32)
        .round()
        .clamp(1.0, s as f32) as i32
}

/// Does the water in `(x, y, z)` have air above it — that is, is there a surface to draw?
///
/// A cell whose water stops short of the top of its own front rectangle always has one:
/// the gap is inside that cell, above its own water line, so the surface is exposed
/// whatever stands in the voxel above. Only a cell drawn to its brim depends on its
/// neighbour, and then only open air counts — water above makes the two one body, and a
/// solid above makes the surface an underside nobody sees.
///
/// The test is the *drawn* fill in pixels rather than the raw fraction, because they are
/// not the same question. Equalization leaves a settled pool's interior a hair under one,
/// which still draws a full face; reading that as a partial cell would hatch a bright skin
/// across every slab of the body.
#[inline]
fn water_open_up(view: &VoxelView<'_>, x: i64, y: i64, z: u32, s: u32) -> bool {
    fill_px_at(view, x, y, z, s) < s as i32
        || (!solid(view, x, y + 1, z) && free_at(view, x, y + 1, z) <= WATER_EPSILON)
}

/// [`fill_px`] of a neighbour, or `0` where it holds no water.
#[inline]
fn fill_px_at(view: &VoxelView<'_>, x: i64, y: i64, z: u32, s: u32) -> i32 {
    let free = free_at(view, x, y, z);
    if free <= WATER_EPSILON {
        0
    } else {
        fill_px(free, s)
    }
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

    /// **A falling column has to read at 4 px.** A cell mid-fall holds a small fraction of
    /// its void, and the drawn height is `round(free · s)`, which is zero at four pixels
    /// for anything under an eighth full. Both paths already floor it at one row —
    /// `present::fill_px` clamps to 1, and the GPU packs the free byte as
    /// `quantise(free).max(1)` for any cell the CPU calls wet — so a thread of water is a
    /// pixel wide rather than nothing. This pins that agreement, because the two clamps
    /// live in different crates and one could be *fixed* without the other.
    #[test]
    fn a_thin_falling_column_draws_one_row_on_both_paths() {
        let s = 4u32;
        for free in [1e-3f32, 0.01, 0.05, 0.1, 0.2, 0.49, 0.5, 0.9, 1.0] {
            let cpu = fill_px(free, s);
            assert_eq!(cpu, cpu.clamp(1, s as i32), "cpu rows are 1..={s}: {free}");

            // The GPU sees the quantised byte and repeats the same arithmetic in
            // `voxel.frag`'s `fillPxQ`.
            let texel = cubarium_gpu::voxel::VoxelTexel::pack_glyph(
                0,
                0,
                0,
                free,
                free <= WATER_EPSILON,
                0.0,
                0,
            );
            let q = (texel.free() * 255.0).round() as i32;
            assert!(q >= 1, "any wet cell packs to at least one: {free}");
            // `voxel.frag`'s `fillPxQ`: floor(q/255 · S + 0.5), clamped to 1..=S.
            let gpu = (q as f32 / 255.0 * s as f32 + 0.5).floor() as i32;
            let gpu = gpu.clamp(1, s as i32);
            assert_eq!(
                gpu, cpu,
                "the two paths draw the same rows for free = {free}"
            );
        }
        // And a dry cell draws nothing on either.
        assert_eq!(fill_px_at_free(0.0, s), 0);
    }

    /// `fill_px_at` without a world behind it, for the row-count comparison above.
    fn fill_px_at_free(free: f32, s: u32) -> i32 {
        if free <= WATER_EPSILON {
            0
        } else {
            fill_px(free, s)
        }
    }

    use cubarium_render::Canvas;
    use cubarium_surface::{Scale, Topology};
    use cubarium_voxel::{Command, Config, World};
    use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species};

    fn config() -> Config {
        Config {
            width: 32,
            height: 12,
            depth: 4,
            ..Config::default()
        }
    }

    /// Geometry tests need a uniform clear and deterministic flat faces. Atmosphere and
    /// grain have their own picture-level checks; inheriting the live visual defaults
    /// turns classification assertions into tests of decoration instead.
    fn test_voxel_config() -> VoxelConfig {
        VoxelConfig {
            dither: 0.0,
            sky_gradient: false,
            ..VoxelConfig::default()
        }
    }

    /// No plants: the terrain-and-water picture every test written before the flora
    /// existed is about.
    fn present(world: &World) -> (Canvas, Projection) {
        present_with(test_voxel_config(), world)
    }

    fn present_with(cfg: VoxelConfig, world: &World) -> (Canvas, Projection) {
        present_flora(cfg, world, &Flora::new(FloraConfig::default()))
    }

    fn present_flora(cfg: VoxelConfig, world: &World, flora: &Flora) -> (Canvas, Projection) {
        let proj = Projection::new(
            cfg.tilt_degrees,
            cfg.px_per_voxel,
            cfg.raster_height,
            world.config(),
        )
        .unwrap();
        let mut canvas = Canvas::new(
            Topology::Ring {
                w: proj.raster_w,
                h: proj.raster_h,
            },
            Scale::ONE,
        );
        VoxelPresenter::new(cfg, proj).draw(&world.view(), flora.view(), &mut canvas);
        (canvas, proj)
    }

    /// A world with a flat soil floor at `top`, every slab: the ground a stand needs
    /// under it before anything about the stand can be tested.
    fn floor(c: &Config, top: u32) -> World {
        let mut world = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                for y in 0..=top {
                    set(&mut world, x, y, z, Material::Soil);
                }
            }
        }
        world
    }

    /// One full-grown founder of a species on the highest support of a column.
    fn seeded(world: &World, x: i64, z: u32, species: Species) -> Flora {
        let mut flora = Flora::new(FloraConfig::default());
        let wood = flora.config().species(species).wood_max;
        assert!(
            flora.apply(
                world,
                FloraCommand::Seed {
                    x,
                    z,
                    species,
                    wood
                }
            ),
            "the fixture must really seed a stand at ({x}, {z})"
        );
        flora
    }

    fn set(world: &mut World, x: i64, y: u32, z: u32, m: Material) {
        world.apply(Command::SetMaterial {
            x,
            y,
            z,
            material: m,
        });
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
            world.apply(Command::AddWater {
                x: 4,
                y: 6,
                z: 1,
                volume_m3: c.voxel_volume(),
            });
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
                let u = (x - shift).rem_euclid(w) as f64 / w as f64 * std::f64::consts::TAU;
                let top = (4.0 + 3.0 * u.cos()).round() as u32;
                for z in 0..c.depth {
                    for y in 1..=top {
                        let m = if y == top {
                            Material::Soil
                        } else {
                            Material::Rock
                        };
                        world.apply(Command::SetMaterial {
                            x,
                            y,
                            z,
                            material: m,
                        });
                    }
                    // Translucent water standing in every trough, so the seam is judged
                    // with a blended face crossing it and not only with opaque rock.
                    if top <= 2 {
                        for y in top + 1..=3 {
                            world.apply(Command::AddWater {
                                x,
                                y,
                                z,
                                volume_m3: vol,
                            });
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
        let c = Config {
            depth: 1,
            ..config()
        };
        let mut world = World::empty(c.clone());
        for y in 1..4 {
            for x in 2..8 {
                set(&mut world, x, y, 0, Material::Rock);
            }
        }
        let vol = c.voxel_volume();
        for x in 2..8 {
            world.apply(Command::AddWater {
                x,
                y: 4,
                z: 0,
                volume_m3: vol * 0.5,
            });
        }
        let (canvas, proj) = present(&world);
        let col = proj.col(4) + 1;
        let (_, r0, _, s) = proj.front_rect(4, 4, 0);
        let at = |row: i32| pixel(&canvas, &proj, col, row);

        // The bottom two of the four rows are water; the surface row is the top of those.
        let bottom = r0 + s as i32 - 1;
        let skin_row = r0 + s as i32 - 2;
        assert!(bluer_than_sky(at(bottom)), "the bottom row must be water");
        assert!(
            bluer_than_sky(at(skin_row)),
            "the surface row must be water"
        );
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
        let c = Config {
            depth: 1,
            height: 16,
            ..config()
        };
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
                world.apply(Command::AddWater {
                    x,
                    y,
                    z: 0,
                    volume_m3: vol,
                });
            }
            world.apply(Command::AddWater {
                x,
                y: 7,
                z: 0,
                volume_m3: vol * 0.5,
            });
        }

        let (canvas, proj) = present(&world);
        let col = proj.col(4) + 1;
        let floor = proj.front_row(2, 0); // the first row below the water column
        let surface_top =
            proj.front_row(7, 0) + proj.s as i32 - fill_px(0.5, proj.s) - proj.rise as i32;

        // Classify every row of the column between the sky and the rock floor.
        let body = pixel(&canvas, &proj, col, floor - 1);
        let surface = pixel(&canvas, &proj, col, surface_top);
        assert!(
            bluer_than_sky(body) && bluer_than_sky(surface),
            "both must be water"
        );
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

    /// Two partial pools one slab apart: their top bands overlap by a row, and the nearer
    /// one owns it. The shared row must be blended once, not twice.
    ///
    /// At `s = 4`, `rise = 2`, with `R = front_row(y, 1)`: half-full water at `(x, y, 1)`
    /// puts its skin at `R + 2` and its top band at `R .. R + 2`; three-quarter-full water
    /// at `(x, y, 0)` puts its skin at `R + 3` and its top band at `R + 1 .. R + 3`. Row
    /// `R + 1` is in both. The far cell's own render is the control for row `R`, which it
    /// keeps, and the nearer cell's own render is the control for row `R + 1`: one blend
    /// over sky. A second blend on top of that could not match it.
    #[test]
    fn overlapping_partial_water_tops_blend_each_shared_row_once() {
        let (x, y) = (5i64, 6u32);
        let c = config();
        let vol = c.voxel_volume();
        // `fars` and `nears` select which of the two pools exists, so each is its own
        // control for the rows it alone would own.
        let build = |fars: bool, nears: bool| {
            let mut world = World::empty(c.clone());
            if fars {
                world.apply(Command::AddWater {
                    x,
                    y,
                    z: 1,
                    volume_m3: vol * 0.5,
                });
            }
            if nears {
                world.apply(Command::AddWater {
                    x,
                    y,
                    z: 0,
                    volume_m3: vol * 0.75,
                });
            }
            world
        };

        let (both, proj) = present(&build(true, true));
        assert_eq!(
            (proj.s, proj.rise),
            (4, 2),
            "the fixture is stated at s = 4, rise = 2"
        );
        let (far_only, _) = present(&build(true, false));
        let (near_only, _) = present(&build(false, true));
        let r = proj.front_row(y, 1);
        let col = proj.col(x) + 1;
        let at = |canvas: &Canvas, row: i32| pixel(canvas, &proj, col, row);

        // Both tops really do want row R + 1: alone, each of them blends it.
        assert!(
            bluer_than_sky(at(&far_only, r + 1)),
            "the far top alone covers row R + 1"
        );
        assert!(
            bluer_than_sky(at(&near_only, r + 1)),
            "the near top alone covers row R + 1"
        );

        // Row R is the far top's alone, and survives: clipping is by row, not by band.
        assert_eq!(
            at(&both, r),
            at(&far_only, r),
            "row R is the far top's, and it keeps it"
        );
        assert!(bluer_than_sky(at(&both, r)), "row R must still be water");

        // Row R + 1 is the nearer top's, blended over sky exactly once. Had the far top
        // also blended it, the nearer blend would land on water instead of on sky.
        assert_eq!(
            at(&both, r + 1),
            at(&near_only, r + 1),
            "row R + 1 must be one blend of the nearer top, not two: {:?} vs {:?}",
            at(&both, r + 1),
            at(&near_only, r + 1),
        );
    }

    /// A roof that covers only part of a water top clips it to the rows it really owns.
    ///
    /// At `s = 4`, `rise = 2`, with `R = front_row(y, 1)`: quarter-full water at
    /// `(x, y, 1)` has its skin at `R + 3` and its top band at `R + 1 .. R + 3`. A lone
    /// rock at `(x, y + 1, 0)` covers `R − 4 .. R + 2` — row `R + 1` but not row `R + 2`.
    /// Dropping the whole band loses a row of visible water.
    ///
    /// The partial cell's surface is also exposed *because it is partial*: the air gap is
    /// inside the cell, above its own water line, so the voxel above cannot close it.
    #[test]
    fn a_partial_roof_clips_a_water_top_instead_of_culling_it() {
        let (x, y) = (5i64, 6u32);
        let c = config();
        let build = |water: bool, roof: bool| {
            let mut world = World::empty(c.clone());
            if water {
                world.apply(Command::AddWater {
                    x,
                    y,
                    z: 1,
                    volume_m3: c.voxel_volume() * 0.25,
                });
            }
            if roof {
                set(&mut world, x, y + 1, 0, Material::Rock);
            }
            world
        };

        let (both, proj) = present(&build(true, true));
        assert_eq!(
            (proj.s, proj.rise),
            (4, 2),
            "the fixture is stated at s = 4, rise = 2"
        );
        let (water_only, _) = present(&build(true, false));
        let (roof_only, _) = present(&build(false, true));
        let r = proj.front_row(y, 1);
        let col = proj.col(x) + 1;
        let at = |canvas: &Canvas, row: i32| pixel(canvas, &proj, col, row);

        // The roof reaches row R + 1 and no further: alone, it leaves R + 2 as sky.
        assert_ne!(
            at(&roof_only, r + 1),
            sky(),
            "the roof's front covers row R + 1"
        );
        assert_eq!(
            at(&roof_only, r + 2),
            sky(),
            "the roof's front stops before row R + 2"
        );
        // The water top alone blends both rows of its band, over sky.
        assert!(
            bluer_than_sky(at(&water_only, r + 1)),
            "the band alone covers row R + 1"
        );
        assert!(
            bluer_than_sky(at(&water_only, r + 2)),
            "the band alone covers row R + 2"
        );

        // With the roof in front, the row the roof does not own is still that one blend —
        // culling the whole band on the strength of a solid at `(x, y + 1, z − 1)` would
        // have left it sky.
        assert_eq!(
            at(&both, r + 2),
            at(&water_only, r + 2),
            "row R + 2 must stay visible water: {:?} vs {:?}",
            at(&both, r + 2),
            at(&water_only, r + 2),
        );
        // The row the roof does own is the roof's own opaque front.
        assert_eq!(
            at(&both, r + 1),
            at(&roof_only, r + 1),
            "row R + 1 is the roof's"
        );
        // And the skin row beneath the band, which neither the roof nor the clip touches.
        assert_eq!(
            at(&both, r + 3),
            at(&water_only, r + 3),
            "the skin row is unchanged"
        );
        assert!(bluer_than_sky(at(&both, r + 3)), "the skin row is water");
    }

    /// A plateau that recedes in `z` reads as one plane, not as ruled shelving: the
    /// interior slabs' front faces are culled, the surviving top faces tile row by row, and
    /// the brightness down that band is monotone — no rim line and no contour row anywhere
    /// inside it.
    #[test]
    fn a_plateau_receding_in_z_has_no_ruled_bands() {
        let c = Config {
            width: 16,
            height: 12,
            depth: 8,
            ..Config::default()
        };
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
        let rows: Vec<i32> = (1..=(proj.depth * proj.rise) as i32)
            .map(|k| near_top - k)
            .collect();
        assert!(
            rows.iter().all(|&r| r >= 0),
            "the band must be inside the raster"
        );

        let lums: Vec<f32> = rows
            .iter()
            .map(|&r| lum(pixel(&canvas, &proj, col, r)))
            .collect();
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
        let c = Config {
            width: 48,
            height: 24,
            depth: 1,
            ..Config::default()
        };
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
        assert!(
            under < deep,
            "a floor right under the roof must be darker: {under} vs {deep}"
        );
        assert!(
            deep < open,
            "a floor far below the roof must still be lit: {deep} vs {open}"
        );
        assert!(
            deep > under + (open - under) * 0.5,
            "the falloff must be most of the way back to the light by 16 voxels: \
             {under} / {deep} / {open}"
        );
    }

    /// A stand stands **on** its support face: its pixels are all above the support
    /// voxel's own front face, its trunk is a warm column in the support's own pixel
    /// columns, and nothing below the support changes.
    ///
    /// `z = 0` so the whole stand is in the nearest slab and nothing can be in front of
    /// it: this test is about where a stand is, not about what hides it.
    #[test]
    fn a_seeded_stands_trunk_stands_on_its_support_face() {
        let c = config();
        let world = floor(&c, 5);
        let bare = Flora::new(FloraConfig::default());
        let flora = seeded(&world, 8, 0, Species::Bloomcrown);
        let cfg = test_voxel_config();
        let (with, proj) = present_flora(cfg.clone(), &world, &flora);
        let (without, _) = present_flora(cfg, &world, &bare);
        assert_ne!(
            with.pixels(),
            without.pixels(),
            "the stand must reach the raster"
        );

        // Every pixel the stand changed is strictly above the support's front face: the
        // support's own top row is the lowest row the trunk's front rectangle reaches,
        // and it reaches it from above.
        let w = usize::from(proj.raster_w);
        let support_top = proj.front_row(5, 0);
        for (i, (a, b)) in with.pixels().iter().zip(without.pixels()).enumerate() {
            let row = (i / w) as i32;
            assert!(
                a == b || row < support_top,
                "row {row} is at or below the support face (row {support_top})"
            );
        }

        // The trunk itself: in the support's own pixel columns, one voxel above it, and
        // warm — bloomcrown's plum, the one thing in the picture that is.
        let col = proj.col(8) + 1;
        let row = proj.front_row(6, 0) + 1;
        let px = pixel(&with, &proj, col, row);
        assert_ne!(px, sky(), "the trunk must be drawn");
        assert!(px[0] > px[2], "bloomcrown's wood is warm: {px:?}");
        assert_ne!(
            px,
            pixel(&without, &proj, col, row),
            "and it must differ from what the bare world leaves there"
        );

        // Umbrellafrond, in the same place, is the cool one: the two species are told
        // apart by hue and not only by size.
        let cool = seeded(&world, 8, 0, Species::Umbrellafrond);
        let (other, _) = present_flora(test_voxel_config(), &world, &cool);
        let cx = pixel(&other, &proj, col, row);
        assert!(cx[2] > cx[0], "umbrellafrond's wood is cool: {cx:?}");
    }

    /// A nearer taller column hides a stand exactly as it hides terrain: the picture
    /// cannot tell the stand is there. The same world without the wall is the control.
    #[test]
    fn a_nearer_taller_column_hides_a_stand_entirely() {
        let c = config();
        // The floor sits low on purpose. Depth *lifts* the image, so a stand three slabs
        // back is drawn `3 · rise` rows higher than the same stand in front: a wall can
        // only hide it if the world has the headroom for the wall to out-climb that
        // lift. On this 12-voxel world a stand on a floor at `y = 5` reaches above a
        // wall built to the ceiling — which is the projection telling the truth, not a
        // bug, so the fixture gives the wall the room instead.
        let build = |wall: bool| {
            let mut world = floor(&c, 2);
            if wall {
                // A wall in the nearest slab, from just above the floor to the top of
                // the world: whatever grows behind it is behind it.
                for x in 0..i64::from(c.width) {
                    for y in 3..c.height {
                        set(&mut world, x, y, 0, Material::Rock);
                    }
                }
            }
            world
        };
        let cfg = test_voxel_config();
        let bare = Flora::new(FloraConfig::default());

        // Control: with nothing in front of it, the stand plainly reaches the raster.
        let open = build(false);
        let flora = seeded(&open, 8, 1, Species::Umbrellafrond);
        assert_ne!(
            present_flora(cfg.clone(), &open, &flora).0.pixels(),
            present_flora(cfg.clone(), &open, &bare).0.pixels(),
            "the stand must be visible with nothing in front of it"
        );

        // Walled: the stand's cells are drawn in slabs 1 and 2, the wall in slab 0 after
        // them, so not one pixel of it survives.
        let walled = build(true);
        let hidden = seeded(&walled, 8, 1, Species::Umbrellafrond);
        assert_eq!(
            present_flora(cfg.clone(), &walled, &hidden).0.pixels(),
            present_flora(cfg, &walled, &bare).0.pixels(),
            "a nearer taller column must hide the whole stand"
        );
    }

    /// A farther stand's crown is not drawn over a nearer water surface: the water is
    /// drawn after it, so the crown shows *through* the blend rather than across it.
    ///
    /// The rows are found rather than stated: a row where the crown alone and the water
    /// alone both change the bare picture is a row both want, and every one of them must
    /// end up the water's.
    #[test]
    fn a_farther_crown_is_not_drawn_over_a_nearer_water_surface() {
        let c = config();
        let vol = c.voxel_volume();
        let x = 8i64;
        let build = |water: bool| {
            let mut world = floor(&c, 3);
            if water {
                // A pool in the nearest slab, at the height the far crown reaches.
                // Nothing is stepped here, so it stays where it is put.
                for dx in -1..=1 {
                    world.apply(Command::AddWater {
                        x: x + dx,
                        y: 9,
                        z: 0,
                        volume_m3: vol * 0.75,
                    });
                }
            }
            world
        };
        let cfg = test_voxel_config();
        let bare_flora = Flora::new(FloraConfig::default());
        let dry = build(false);
        let flora = seeded(&dry, x, 3, Species::Umbrellafrond);

        let (bare, proj) = present_flora(cfg.clone(), &dry, &bare_flora);
        let (crown_only, _) = present_flora(cfg.clone(), &dry, &flora);
        let wet = build(true);
        let (water_only, _) = present_flora(cfg.clone(), &wet, &bare_flora);
        let (both, _) = present_flora(cfg, &wet, &flora);

        let col = proj.col(x) + 1;
        let at = |canvas: &Canvas, row: i32| pixel(canvas, &proj, col, row);
        let rows: Vec<i32> = (0..i32::from(proj.raster_h))
            .filter(|&r| at(&crown_only, r) != at(&bare, r) && at(&water_only, r) != at(&bare, r))
            .collect();
        assert!(
            !rows.is_empty(),
            "the fixture must put the crown and the water in one row"
        );

        // What "the water owns the row" means exactly. If the water is drawn over the
        // crown with the same colour and the same alpha it uses over the bare picture,
        // then `seen = (1−a)·crown + a·W` and `water = (1−a)·bare + a·W`, so
        //
        //     seen − water = (1 − a) · (crown − bare)
        //
        // channel for channel — one ratio, the same in all three, strictly inside
        // `0..1`. A crown painted *over* the water would give `seen = crown` and no such
        // ratio; a second blend would change the alpha and break it too. So this one
        // number is the whole ordering claim, and it is also a claim that the water is
        // blended exactly once.
        for &r in &rows {
            let (crown, water, seen) = (at(&crown_only, r), at(&water_only, r), at(&both, r));
            assert_ne!(seen, crown, "row {r} is the water's, not the crown's");
            assert!(
                bluer_than_sky(seen),
                "row {r} must still read as water: {seen:?}"
            );
            let bare_px = at(&bare, r);
            let mut ratios = Vec::new();
            for k in 0..3 {
                let behind = crown[k] - bare_px[k];
                if behind.abs() > 0.02 {
                    ratios.push((seen[k] - water[k]) / behind);
                }
            }
            assert!(
                !ratios.is_empty(),
                "row {r}: the crown must differ from the ground"
            );
            for &q in &ratios {
                assert!(
                    (q - ratios[0]).abs() < 1e-3,
                    "row {r}: one blend has one alpha, got {ratios:?}"
                );
                assert!(
                    (0.0..1.0).contains(&q),
                    "row {r}: the water must cover the crown, not the other way: {q}"
                );
            }
        }
    }

    /// Green-blue dominant and brighter than the sky: the only thing in the palette that
    /// is, so it identifies water without pinning a colour constant.
    fn bluer_than_sky(px: [f32; 3]) -> bool {
        px[2] > sky()[2] * 2.0 && px[2] > px[0]
    }

    /// The authored fixture really does carry the four features it is for.
    #[test]
    fn the_authored_scene_has_a_ridge_a_hollow_with_water_and_an_overhang() {
        let c = Config {
            width: 64,
            height: 24,
            depth: 8,
            ..Config::default()
        };
        let world = super::super::scene::authored(c.clone());
        let view = world.view();

        let tops: Vec<u32> = (0..i64::from(c.width))
            .map(|x| view.surface_y(x, 0).unwrap_or(0))
            .collect();
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
        let c = Config {
            width: 16,
            height: 20,
            depth: 4,
            ..Config::default()
        };
        let world = super::super::scene::authored(c.clone());
        let cfg = test_voxel_config();
        let (full, fp) = present_with(cfg.clone(), &world);
        let short_h = fp.raster_h / 2;
        let (short, sp) = present_with(
            VoxelConfig {
                raster_height: short_h,
                ..cfg
            },
            &world,
        );
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
