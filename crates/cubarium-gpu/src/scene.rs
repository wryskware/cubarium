//! `Scene`: everything the GPU renderer draws for one frame of a ring world, and
//! the **only** interface Stage B's `RenderView` adapter has to fill.
//!
//! The split is deliberate. Per `design/7_Research/presenter-budget-2026-09-16.md`
//! §4(4), the scalar fields change at the **tick** rate (20 Hz) and the instances at
//! the **frame** rate (60 Hz), and the two travel to the GPU by different routes: the
//! fields are five small cell textures re-uploaded when [`Fields::revision`] changes,
//! the instances are one vertex buffer rewritten every frame. `Scene` keeps that
//! shape visible so the adapter cannot accidentally make a 20 Hz thing cost 60 Hz.
//!
//! # Coordinates
//!
//! * **Raster pixels** `(x, y)`: `x` right and wrapping, `y` down, origin at the top
//!   left of the `w × h` world raster. This is the image the CPU presenter would
//!   have rasterized; the panel's integer upscale and 90° rotation happen after it,
//!   in the present pass, and nothing in a `Scene` knows about them.
//! * **Cells**: the field grid, `w / (4·S)` by `h / (4·S)` — 80 × 45 at every `S` in
//!   `design/flat-world-plan-2026-09-16.md` §6's ladder.
//! * **Source texels**: a sprite tile's own pixels, `+x` right and `+y` **down**,
//!   row 0 the top of the tile. One source texel covers an `S × S` block of raster
//!   pixels when the heading is axis-aligned.
//! * **Height** `h = 1 − 2·v/H` where `v` is the raster row: `+1` at the top row,
//!   `−1` at the bottom, exactly `Topology::height` in
//!   `design/flat-world-plan-2026-09-16.md` §5a. The horizon fade reads this and
//!   nothing else, so the ring needs no chart and no seam-aware pixel→cell map.

use bytemuck::{Pod, Zeroable};

/// The ring's raster and its field grid.
///
/// **Invariant**: `w` and `h` are both divisible by `4·scale`, which is what makes
/// `cells_x × cells_y` exactly 80 × 45 at every rung of §6's resolution ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RingLayout {
    /// Raster width in pixels, and the wrap period: column `w` is column `0`.
    pub w: u32,
    /// Raster height in pixels. There is no wrap in `y`; the top and bottom rows are
    /// the ring's rims.
    pub h: u32,
    /// The world scale `S` of §6: one field cell is `4·S` raster pixels and
    /// one field cell is `4·S` of them — `cubarium_surface::Scale::cell_pixels`, which
    /// FW-1 froze at `4·S` with `CELL_PIXELS = 4.0`.
    ///
    /// **Integer only, unlike `Scale`.** FW-1's `Scale` carries an `f64`, so the
    /// contract admits `S = 1.5`. This renderer does not: the pixel-art rule is that
    /// one authored source texel covers an exact `S × S` block of raster pixels, and a
    /// half-integer factor has no such block. §6's ladder rungs 1, 2, 3 and 6 are all
    /// integers; the 480×270 rung at `S = 1.5` would have to be drawn by the CPU
    /// presenter or re-baked, and this renderer refuses it rather than blurring it.
    pub scale: u32,
}

impl RingLayout {
    /// The 320×180, `S = 1` rung: the configuration FW-0 measured and §6 recommends
    /// as the fallback.
    pub const RING_320: RingLayout = RingLayout { w: 320, h: 180, scale: 1 };
    /// The 640×360, `S = 2` rung: §6's recommended candidate.
    pub const RING_640: RingLayout = RingLayout { w: 640, h: 360, scale: 2 };

    /// Whether the raster divides into whole cells at this scale.
    pub fn is_valid(&self) -> bool {
        let cell = 4 * self.scale;
        self.scale > 0 && self.w > 0 && self.h > 0 && self.w % cell == 0 && self.h % cell == 0
    }

    /// Field cells across the ring.
    pub fn cells_x(&self) -> u32 {
        self.w / (4 * self.scale)
    }

    /// Field cell rows.
    pub fn cells_y(&self) -> u32 {
        self.h / (4 * self.scale)
    }

    /// Cells in the field textures.
    pub fn cell_count(&self) -> usize {
        (self.cells_x() * self.cells_y()) as usize
    }
}

/// The per-**tick** scalar fields, one value per cell in row-major order.
///
/// All six are `cells_x · cells_y` long; a short or missing vector is uploaded as
/// zeros, so a scene that has no water or no rain simply leaves those empty. The
/// whole set is `2 × 80 × 45 × 8` bytes = **57.6 KB**, which is the budget
/// `presenter-budget-2026-09-16.md` §4(4) allowed ("≈ 60 KB at ring scale").
#[derive(Clone, Debug, Default)]
pub struct Fields {
    /// Producer density, in the world's own units. The ramp saturates at
    /// [`Fields::producer_max`] × `PRODUCER_SATURATION`.
    pub producer: Vec<f32>,
    /// Water depth `d`, metres of film over the cell.
    pub water: Vec<f32>,
    /// Litter plus remains, metres per cell, already scaled the way
    /// `art_present/mod.rs` scales it before the fleck threshold.
    pub detritus: Vec<f32>,
    /// The cell's plant growth stage as a continuous number: `< 0` bare, else the
    /// interpolated stage. Uploaded for the background shader's use and as the field
    /// Stage B's plant instances are derived from; the background pass itself does
    /// not read it today.
    pub growth: Vec<f32>,
    /// Tall column height in trunk segments at this cell, `0` where no column stands.
    pub tall: Vec<f32>,
    /// Rain rate, in the same units `art_present::environment::rain_streaks` takes.
    pub rain: Vec<f32>,
    /// `P_max`: the producer ramp's reference, so the shader can normalize.
    pub producer_max: f32,
    /// Bumped by the producer whenever any of the above changed. The renderer
    /// re-uploads only when it differs from the revision it last saw, so a 60 Hz
    /// frame loop over a 20 Hz world pays the upload once every three frames.
    pub revision: u64,
}

/// One sprite stamp: the GPU's equivalent of one `cubarium_render::stamp_layers_bent_toned`.
///
/// Every field is in the units the CPU presenter uses, so the adapter is a
/// transcription rather than a translation. This is the layout the vertex buffer
/// takes verbatim — `#[repr(C)]` and `Pod`, 120 bytes, ten vertex attributes.
///
/// # Four frame slots, and why exactly four
///
/// `art_present` composites `Σ wᵢ · poseᵢ.sample(..)` and **then** one source-over, and
/// each pose is itself a two-frame blend. A weighted sum of poses whose members are
/// themselves lerps is just a weighted sum of the underlying frames — pose `i` at weight
/// `wᵢ` with blend `mᵢ` contributes `wᵢ(1 − mᵢ)` of frame A and `wᵢmᵢ` of frame B — so
/// `[(frame, weight); 4]` expresses **two whole poses** exactly.
///
/// Two is what the presenter needs almost everywhere. An idle plant is one pose; a
/// fruiting one is two (`tall.rs::stage_layers`); and an authored growth step is three
/// *named* layers but never three *live* ones, because `growth_weights` has
/// `GROW_BLEND = 0.12 < ½`, so `w_from` and `w_to` are never both positive
/// (`growth.rs`'s own normative note). The one case that can exceed it is a body whose
/// state changed twice inside `BODY_FADE_SECONDS = 0.3 s`, where `BodyMemory::layers_at`
/// returns three: the driver then drops the lightest and renormalises, and counts how
/// often it did (`sink/gpu.rs`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct SpriteInstance {
    /// Where the tile's pivot sits, in raster pixels. Snapped to a whole pixel by
    /// the vertex shader: the pixel-art rule is that nothing lands off the grid.
    pub anchor: [f32; 2],
    /// The direction the tile's `+x` axis points, in raster space. `(1, 0)` draws the
    /// tile upright and unrotated; the tile's `+y` is `(−heading.y, heading.x)`,
    /// exactly `sprite.rs`'s `side`.
    pub heading: [f32; 2],
    /// Up to four atlas frames, by their origin in atlas texels. All four share
    /// [`SpriteInstance::size`] and [`SpriteInstance::pivot`], which every frame of one
    /// pack does.
    pub frames: [[u16; 2]; 4],
    /// The frames' `w, h` in atlas texels.
    pub size: [u16; 2],
    /// The tile's pivot in source texels — `(8, 8)` for every 16×16 tile in
    /// `assets/atelier`, `(4, 4)` for a ground tile.
    pub pivot: [u16; 2],
    /// Each frame's weight in the composite. They should sum to 1 for an exact lerp; a
    /// zero weight costs nothing but a compare.
    pub weights: [f32; 4],
    /// `cubarium_render::Bend` as `(amplitude, base, root, length)`, in source texels
    /// along the tile's `+x`.
    pub bend: [f32; 4],
    /// `Mask` floor, in source rows above the tile's bottom edge. [`NO_MASK_FLOOR`]
    /// for `Mask::Axial` and `Mask::None`; ignored when [`Self::mask_flags`] is radial.
    pub mask_floor: f32,
    /// `Mask` reveal — rows above the bottom edge for an axial or strip mask, distance
    /// from the pivot for a radial one. [`NO_MASK_REVEAL`] is `Mask::None`.
    pub mask_reveal: f32,
    /// [`MASK_AXIAL`] or [`MASK_RADIAL`]. A ring **does** have radial plants: FW-5's
    /// `ArtGeometry::is_radial` calls a cell radial when its band is `Canopy`, and a
    /// ring's canopy is its top `1 − CANOPY_TOP` of rows, not a face the ring lacks.
    pub mask_flags: f32,
    /// Multiplies the premultiplied sample, exactly the CPU `opacity`.
    pub opacity: f32,
    /// `Tone::colour`, linear light.
    pub tone_colour: [f32; 3],
    /// `Tone::mix`; 0 is the untoned stamp, bit for bit.
    pub tone_mix: f32,
    /// `Shade::floor`.
    pub shade_floor: f32,
    /// `Shade::reference`.
    pub shade_reference: f32,
    /// A per-stamp multiplier on the world scale `S`, as `stamp_layers`'s own `scale`.
    ///
    /// 1 for everything except a juvenile body, which the CPU presenter draws at
    /// `JUVENILE_SCALE = 0.7` (`present.rs`). That is the one stamp in the image whose
    /// texels do **not** land on an `S × S` block, on either renderer: 0.7 is not a
    /// whole number of pixels and no choice of sampler makes it one.
    pub scale: f32,
    /// Which texture the frames index: [`SOURCE_ATLAS`] or [`SOURCE_SCRATCH`].
    pub source: f32,
}

/// The `mask_floor` of an unmasked or purely axial stamp.
pub const NO_MASK_FLOOR: f32 = -1.0e9;
/// The `mask_reveal` of `Mask::None`.
pub const NO_MASK_REVEAL: f32 = 1.0e9;
/// [`SpriteInstance::mask_flags`] for `Mask::None`, `Mask::Axial` and `Mask::Strip`.
pub const MASK_AXIAL: f32 = 0.0;
/// [`SpriteInstance::mask_flags`] for `Mask::Radial`: the reveal is measured from the
/// pivot outward instead of from the tile's bottom edge.
pub const MASK_RADIAL: f32 = 1.0;
/// [`SpriteInstance::source`]: the baked pack's atlas, straight sRGB + alpha.
pub const SOURCE_ATLAS: f32 = 0.0;
/// [`SpriteInstance::source`]: the per-frame scratch page, premultiplied linear RGBA16F.
///
/// The Lanternjaw is not a baked pack: `lanternjaw::Lanternjaw::parts_living` rasterises
/// eight fresh `Sprite`s every frame from the rig's channels, so its texels cannot live
/// in an atlas built at load. They are uploaded into a small page instead, in the
/// premultiplied linear form the rasteriser already produced — no sRGB round trip, so a
/// rig part is bit-exact rather than re-encoded.
pub const SOURCE_SCRATCH: f32 = 1.0;

impl Default for SpriteInstance {
    fn default() -> Self {
        SpriteInstance {
            anchor: [0.0, 0.0],
            heading: [1.0, 0.0],
            frames: [[0, 0]; 4],
            size: [0, 0],
            pivot: [0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
            bend: [0.0; 4],
            mask_floor: NO_MASK_FLOOR,
            mask_reveal: NO_MASK_REVEAL,
            mask_flags: MASK_AXIAL,
            opacity: 1.0,
            tone_colour: [0.0, 0.0, 0.0],
            tone_mix: 0.0,
            shade_floor: 1.0,
            shade_reference: 1.0,
            scale: 1.0,
            source: SOURCE_ATLAS,
        }
    }
}

impl SpriteInstance {
    /// Add one pose — two frames and the blend between them — at `weight`, as
    /// `cubarium_render::stamp_layers` takes `(Pose, f32)`.
    ///
    /// Returns `false` and changes nothing when all four slots are already spoken for,
    /// which is the caller's cue to drop its lightest layer and renormalise rather than
    /// to silently lose one.
    #[must_use]
    pub fn push_pose(&mut self, a: u32, b: u32, mix: f32, weight: f32, atlas: &crate::Atlas) -> bool {
        if !(weight.is_finite() && weight > 0.0) {
            return true;
        }
        let mix = if mix.is_finite() { mix.clamp(0.0, 1.0) } else { 0.0 };
        // A pose at the exact start or end of its blend is one frame, not two: taking
        // the fast path here is what keeps an idle plant inside two slots.
        let pairs: &[(u32, f32)] = &if mix <= 0.0 {
            [(a, weight), (b, 0.0)]
        } else if mix >= 1.0 {
            [(b, weight), (a, 0.0)]
        } else {
            [(a, weight * (1.0 - mix)), (b, weight * mix)]
        };
        let wanted = pairs.iter().filter(|(_, w)| *w > 0.0).count();
        let used = self.used();
        if used + wanted > self.frames.len() {
            return false;
        }
        let mut slot = used;
        for (frame, w) in pairs.iter().copied().filter(|(_, w)| *w > 0.0) {
            let rect = atlas.rect(frame);
            if slot == 0 {
                self.size = [rect.w, rect.h];
                self.pivot = [rect.w / 2, rect.h / 2];
            }
            self.frames[slot] = [rect.x, rect.y];
            self.weights[slot] = w;
            slot += 1;
        }
        true
    }

    /// How many frame slots carry weight.
    pub fn used(&self) -> usize {
        self.weights.iter().filter(|w| **w > 0.0).count()
    }

    /// An instance with no frames at all: what a builder starts from.
    pub fn empty() -> SpriteInstance {
        SpriteInstance { weights: [0.0; 4], ..Default::default() }
    }
}

/// The draw order. One `Vec<SpriteInstance>` per layer, drawn in this order, with
/// the water pass slipped in after [`Layer::GroundCover`] — which is exactly the CPU
/// presenter's order in `art_present/mod.rs::draw_with_fruit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// The 8-px ground lattice, under the water.
    GroundCover = 0,
    /// Small plants and the soil snag, over the water.
    Plants = 1,
    /// Tall columns: base, trunk strips, crown.
    Tall = 2,
    /// Rain streaks. Drawn as instanced quads over [`Atlas::solid`](crate::Atlas::solid).
    Rain = 3,
    /// Organisms, then hunters, over everything.
    Bodies = 4,
}

/// How many layers a [`Scene`] carries.
pub const LAYER_COUNT: usize = 5;

/// Every layer in draw order.
pub const LAYERS: [Layer; LAYER_COUNT] = [
    Layer::GroundCover,
    Layer::Plants,
    Layer::Tall,
    Layer::Rain,
    Layer::Bodies,
];

/// One frame of a ring world.
#[derive(Clone, Debug)]
pub struct Scene {
    /// The raster and the cell grid.
    pub layout: RingLayout,
    /// The tick-rate fields.
    pub fields: Fields,
    /// The frame-rate instances, in [`LAYERS`] order.
    pub layers: [Vec<SpriteInstance>; LAYER_COUNT],
    /// Presentation seconds — `(tick − 1 + f) · DT` as `art_present::present_seconds`
    /// computes it. Every clip phase, the water shimmer and the rain fall read this
    /// and nothing else, so a paused world holds its pose and `--speed` stays honest.
    pub seconds: f64,
    /// The completed tick this frame interpolates out of.
    pub tick: u64,
    /// The tick phase in `[0, 1)`; the adapter has already applied it to the
    /// instances, and the renderer keeps it only so a shader can be told about it.
    pub f: f32,
}

impl Scene {
    /// An empty scene at a layout: every field zero, every layer empty.
    pub fn new(layout: RingLayout) -> Scene {
        Scene {
            layout,
            fields: Fields::default(),
            layers: Default::default(),
            seconds: 0.0,
            tick: 0,
            f: 0.0,
        }
    }

    /// Push an instance into a layer, wrapping it across the ring seam where its
    /// footprint reaches past a rim.
    ///
    /// **This is the whole of the ring's seam handling.** A stamp whose footprint
    /// crosses `x = 0` or `x = w` is pushed a second time displaced by one ring
    /// circumference, so the two halves are drawn by two ordinary quads with no wrap
    /// logic anywhere in the shader. On the cube the same job needed
    /// `unfold_pixels_general` for 44 % of plant slots at 4× the cost
    /// (`presenter-budget-2026-09-16.md` W2a); here it is one comparison and, for a
    /// few dozen instances a frame, one extra quad.
    pub fn push(&mut self, layer: Layer, instance: SpriteInstance) {
        let w = self.layout.w as f32;
        // Half the tile's diagonal plus the bend, in raster pixels: a bound on how
        // far from the anchor the stamp can paint.
        // How far from the anchor a stamp can paint: half the tile's longest side plus
        // the bend, at the art scale. `art_scale` is the renderer's, so this uses the
        // largest it can be (`S`) rather than reaching for it — a seam duplicate that is
        // not needed costs one quad, and one that is missing costs a visible cut.
        let tile = f32::from(instance.size[0].max(instance.size[1]));
        let radius = (0.5 * tile * instance.scale.max(1.0) + instance.bend[0].abs() + 1.0)
            * self.layout.scale as f32;
        let list = &mut self.layers[layer as usize];
        list.push(instance);
        if instance.anchor[0] - radius < 0.0 {
            list.push(SpriteInstance {
                anchor: [instance.anchor[0] + w, instance.anchor[1]],
                ..instance
            });
        } else if instance.anchor[0] + radius > w {
            list.push(SpriteInstance {
                anchor: [instance.anchor[0] - w, instance.anchor[1]],
                ..instance
            });
        }
    }

    /// Drop every instance, keeping the fields and the layout: what a frame loop
    /// calls before refilling the layers.
    pub fn clear_instances(&mut self) {
        for layer in &mut self.layers {
            layer.clear();
        }
    }

    /// Total instances this frame, over all layers.
    pub fn instance_count(&self) -> usize {
        self.layers.iter().map(Vec::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_resolution_ladder_divides_into_eighty_by_forty_five_cells() {
        for layout in [
            RingLayout::RING_320,
            RingLayout::RING_640,
            RingLayout { w: 960, h: 540, scale: 3 },
            RingLayout { w: 1920, h: 1080, scale: 6 },
        ] {
            assert!(layout.is_valid(), "{layout:?}");
            assert_eq!((layout.cells_x(), layout.cells_y()), (80, 45), "{layout:?}");
        }
        assert!(!RingLayout { w: 321, h: 180, scale: 1 }.is_valid());
    }

    #[test]
    fn a_stamp_reaching_past_a_rim_is_pushed_twice_and_one_in_the_middle_once() {
        let mut scene = Scene::new(RingLayout::RING_320);
        let tile = SpriteInstance { size: [16, 16], ..Default::default() };
        scene.push(Layer::Plants, SpriteInstance { anchor: [160.0, 90.0], ..tile });
        assert_eq!(scene.layers[Layer::Plants as usize].len(), 1);
        scene.push(Layer::Plants, SpriteInstance { anchor: [2.0, 90.0], ..tile });
        assert_eq!(scene.layers[Layer::Plants as usize].len(), 3);
        assert_eq!(scene.layers[Layer::Plants as usize][2].anchor, [322.0, 90.0]);
        scene.push(Layer::Plants, SpriteInstance { anchor: [318.0, 90.0], ..tile });
        assert_eq!(scene.layers[Layer::Plants as usize][4].anchor, [-2.0, 90.0]);
    }

    #[test]
    fn the_instance_is_a_plain_flat_record_the_vertex_buffer_can_take() {
        assert_eq!(std::mem::size_of::<SpriteInstance>(), 120);
        let _: &[u8] = bytemuck::bytes_of(&SpriteInstance::default());
    }
}
