//! Authored RGBA sprites, composited through the existing surface atlas.

use cubarium_surface::{PixelImage, SurfacePoint, Vec2, unfold_pixels};

use crate::{Canvas, srgb_decode};

/// Linear premultiplied pixels. The pivot is measured from the image's upper-left
/// boundary; body +x is forward. Transparent padding never enlarges the footprint.
#[derive(Clone, Debug)]
pub struct Sprite {
    width: usize,
    height: usize,
    pivot: Vec2,
    pixels: Vec<[f32; 4]>,
    extent: f64,
}

/// The hard per-stamp footprint bound, in face pixels: no stamp may need surface pixels
/// further than this from its anchor. Not review-tunable — it is the radius the shared
/// unfolding (`cubarium_surface::unfold_pixels`) is proven correct for, and every sprite,
/// pose blend and [`Bend`] is budgeted against it rather than the other way round.
const FOOTPRINT_RADIUS: f64 = 9.0;

/// Half a pixel diagonal plus half a pixel of bilinear support: the distance beyond a
/// painted texel's center that a stamp of that texel can still touch.
const TEXEL_SUPPORT: f64 = std::f64::consts::FRAC_1_SQRT_2 + 0.5;

impl Sprite {
    pub fn from_rgba(
        width: usize,
        height: usize,
        pivot: Vec2,
        bytes: &[u8],
    ) -> Result<Self, String> {
        if width == 0
            || height == 0
            || width > 64
            || height > 64
            || bytes.len() != width * height * 4
            || !pivot.is_finite()
        {
            return Err("invalid sprite dimensions, pivot or RGBA length".into());
        }
        let mut extent = 0.0f64;
        let mut pixels = Vec::with_capacity(width * height);
        for (i, rgba) in bytes.as_chunks::<4>().0.iter().enumerate() {
            let a = f32::from(rgba[3]) / 255.0;
            pixels.push([
                srgb_decode(rgba[0]) * a,
                srgb_decode(rgba[1]) * a,
                srgb_decode(rgba[2]) * a,
                a,
            ]);
            if a > 0.0 {
                let x = (i % width) as f64 + 0.5 - pivot.x;
                let y = (i / width) as f64 + 0.5 - pivot.y;
                // Half a pixel diagonal plus half a pixel of bilinear support.
                extent = extent.max(x.hypot(y) + TEXEL_SUPPORT);
            }
        }
        if extent > FOOTPRINT_RADIUS {
            return Err(format!(
                "sprite extent {extent:.2} exceeds the 9-pixel surface budget"
            ));
        }
        Ok(Self {
            width,
            height,
            pivot,
            pixels,
            extent,
        })
    }

    pub fn extent(&self) -> f64 {
        self.extent
    }

    /// Width in pixels.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> usize {
        self.height
    }

    /// A copy of this sprite with every pixel in `rows` (from the top) that `other`
    /// also paints with exactly the same premultiplied RGBA made transparent.
    ///
    /// This is how a tall plant's crown becomes a *cap*: the crown tile repeats the trunk
    /// pattern in the rows where it overlaps the top trunk segment so the two join
    /// seamlessly at whole-cell positions. Drawn at a fractional height while the column
    /// grows, those duplicated rows would re-stamp a sub-pixel-shifted, blurred copy of
    /// the trunk; subtracting them leaves the crown's own art (dome, bulbs, halo) over a
    /// crisp trunk that shows through. Pixels `other` does not paint, and pixels outside
    /// `rows`, are kept as they are. The extent shrinks to what remains (never grows).
    /// `other` must have the same dimensions; otherwise `self` is returned unchanged.
    pub fn subtract(&self, other: &Sprite, rows: std::ops::Range<usize>) -> Sprite {
        if other.width != self.width || other.height != self.height {
            return self.clone();
        }
        let mut pixels = self.pixels.clone();
        for y in rows.start.min(self.height)..rows.end.min(self.height) {
            for x in 0..self.width {
                let i = y * self.width + x;
                if other.pixels[i][3] > 0.0 && other.pixels[i] == pixels[i] {
                    pixels[i] = [0.0; 4];
                }
            }
        }
        let mut extent = 0.0f64;
        for (i, p) in pixels.iter().enumerate() {
            if p[3] > 0.0 {
                let x = (i % self.width) as f64 + 0.5 - self.pivot.x;
                let y = (i / self.width) as f64 + 0.5 - self.pivot.y;
                extent = extent.max(x.hypot(y) + TEXEL_SUPPORT);
            }
        }
        Sprite { width: self.width, height: self.height, pivot: self.pivot, pixels, extent }
    }

    /// The largest `|`[`Bend::amplitude`]`|` this sprite may be stamped with, in tile
    /// pixels, before a painted texel's bilinear support could reach past the nine-pixel
    /// footprint bound.
    ///
    /// **Normative.** For every painted texel (alpha > 0) of this sprite, with `x` and `y`
    /// its center's offset from the pivot and
    ///
    /// ```text
    /// s = Bend::profile(height − row + 0.5 + base)
    /// ```
    ///
    /// the largest fraction of the amplitude any destination sample that reads this texel
    /// can carry (`row` counted from the top), the texel must satisfy
    ///
    /// ```text
    /// hypot(|x| + |amplitude| · s + 1, |y| + 1) ≤ 9
    /// ```
    ///
    /// which bounds `|amplitude| ≤ (sqrt(max(0, 81 − (|y| + 1)²)) − |x| − 1) / s`. The result
    /// is the smallest such bound over the sprite, never negative; texels with `s = 0` (a
    /// whole pixel above the root, or a non-positive `length`) impose no bound, and a sprite
    /// with no bendable painted texel returns [`f64::INFINITY`]. It is a pure function of the
    /// sprite and the three bend shape parameters — not of the wind — so a caller measures it
    /// once per asset and never per frame.
    ///
    /// Two things in that criterion are the whole point of it.
    ///
    /// * `s` is read **one pixel above** the texel's own center, because a destination sample
    ///   one row above the texel still takes a share of it through the bilinear filter while
    ///   carrying that higher row's own, larger, displacement. Bounding only the texel
    ///   center's own `D` admits amplitudes whose filter tail then falls outside the
    ///   footprint and is silently clipped.
    /// * The bound is the whole **bilinear support**, `+1` on each axis from the displaced
    ///   texel center, not the radial `FRAC_1_SQRT_2 + 0.5` of [`Sprite::extent`]. A texel is
    ///   read by every destination sample within one pixel of it on each axis, and it is
    ///   those samples that must lie inside the stamp's footprint. Because the footprint
    ///   grows by exactly `|amplitude|` while this criterion holds, the wind can never lose a
    ///   sample the unbent stamp would have painted.
    ///
    /// `|x|` is used rather than the signed offset so that the same budget holds for wind
    /// blowing either way, and the worst case over every texel is taken so that no texel of
    /// this sprite is ever clipped by the footprint. This is a *sufficient* bound, not the
    /// sprite's extent plus the amplitude: a tall sprite whose widest texels sit at its
    /// still root keeps a usable budget.
    pub fn bend_headroom(&self, root: f64, length: f64, base: f64) -> f64 {
        let probe = Bend { amplitude: 1.0, base, root, length };
        let mut bound = f64::INFINITY;
        for (i, p) in self.pixels.iter().enumerate() {
            if p[3] <= 0.0 {
                continue;
            }
            let row = (i / self.width) as f64 + 0.5;
            // One pixel above the texel center: the highest row whose bilinear support
            // still reaches this texel, and therefore the largest `D` that can read it.
            let s = probe.profile(self.height as f64 - (row - 1.0) + base);
            if s <= 0.0 {
                continue;
            }
            let x = ((i % self.width) as f64 + 0.5 - self.pivot.x).abs();
            let y = (row - self.pivot.y).abs() + 1.0;
            let across = (FOOTPRINT_RADIUS * FOOTPRINT_RADIUS - y * y).max(0.0).sqrt();
            bound = bound.min(((across - x - 1.0) / s).max(0.0));
        }
        bound
    }

    /// Whether, in `rows` (from the top), every texel `other` paints (alpha > 0) this
    /// sprite paints with exactly the same premultiplied RGBA. Texels `other` leaves
    /// transparent are not compared. Mismatched dimensions are `false`.
    pub fn paints_like(&self, other: &Sprite, rows: std::ops::Range<usize>) -> bool {
        if other.width != self.width || other.height != self.height {
            return false;
        }
        for y in rows.start.min(self.height)..rows.end.min(self.height) {
            for x in 0..self.width {
                let i = y * self.width + x;
                if other.pixels[i][3] > 0.0 && other.pixels[i] != self.pixels[i] {
                    return false;
                }
            }
        }
        true
    }

    fn pixel(&self, x: i32, y: i32) -> [f32; 4] {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            [0.0; 4]
        } else {
            self.pixels[y as usize * self.width + x as usize]
        }
    }

    fn sample(&self, point: Vec2) -> [f32; 4] {
        let p = point + self.pivot - Vec2::new(0.5, 0.5);
        let x = p.x.floor() as i32;
        let y = p.y.floor() as i32;
        let fx = (p.x - f64::from(x)) as f32;
        let fy = (p.y - f64::from(y)) as f32;
        let mut result = [0.0; 4];
        for (dx, dy, weight) in [
            (0, 0, (1.0 - fx) * (1.0 - fy)),
            (1, 0, fx * (1.0 - fy)),
            (0, 1, (1.0 - fx) * fy),
            (1, 1, fx * fy),
        ] {
            let pixel = self.pixel(x + dx, y + dy);
            for channel in 0..4 {
                result[channel] += pixel[channel] * weight;
            }
        }
        result
    }
}

/// Source-over blend, not additive light: dark authored outlines remain visible.
/// Shared `unfold_pixels` supplies seam orientation and unique vertex ownership.
///
/// Exactly [`stamp_pose`] with a single sprite and no mask.
pub fn stamp_sprite(
    canvas: &mut Canvas,
    anchor: SurfacePoint,
    heading: Vec2,
    sprite: &Sprite,
    scale: f64,
    opacity: f32,
    scratch: &mut Vec<PixelImage>,
) {
    stamp_pose(canvas, anchor, heading, Pose::still(sprite), scale, opacity, Mask::None, scratch);
}

/// A pose between two samples of one clip: `first` blended toward `second` by `mix`.
///
/// `mix = 0` is `first` exactly (and takes a single-sprite fast path); `mix = 1` is
/// `second` exactly. Both sprites should come from the same clip (same size and pivot);
/// the blend is a per-pixel linear mix of the two *premultiplied* linear RGBA samples,
/// composited with one source-over, so a pixel that is opaque in both stays opaque at
/// every mix — two half-opacity stamps would instead let a quarter of the background
/// through at the midpoint.
#[derive(Clone, Copy, Debug)]
pub struct Pose<'a> {
    pub first: &'a Sprite,
    pub second: &'a Sprite,
    /// In `[0, 1]`; a non-finite value reads as 0.
    pub mix: f32,
}

impl<'a> Pose<'a> {
    /// One sprite, no blend.
    pub fn still(sprite: &'a Sprite) -> Pose<'a> {
        Pose { first: sprite, second: sprite, mix: 0.0 }
    }

    /// The blend weight, sanitized into `[0, 1]`.
    pub fn weight(&self) -> f32 {
        if self.mix.is_finite() { self.mix.clamp(0.0, 1.0) } else { 0.0 }
    }

    /// The extent the stamp unfolds for this pose: `first`'s at weight 0, `second`'s at
    /// weight 1, the larger of the two in between — exactly the sprites [`stamp_pose`]
    /// samples, so a caller sizing a scale from it never shrinks for a sprite that is not
    /// drawn.
    pub fn extent(&self) -> f64 {
        let w = self.weight();
        if w <= 0.0 {
            self.first.extent
        } else if w >= 1.0 {
            self.second.extent
        } else {
            self.first.extent.max(self.second.extent)
        }
    }

    /// The premultiplied sample of this pose at sprite-local `point` (see
    /// [`Sprite::sample`]): `first` blended toward `second` by the weight. Endpoint
    /// weights sample one sprite only.
    fn sample(&self, point: Vec2) -> [f32; 4] {
        let w = self.weight();
        if w <= 0.0 {
            return self.first.sample(point);
        }
        if w >= 1.0 {
            return self.second.sample(point);
        }
        let mut rgba = self.first.sample(point);
        let other = self.second.sample(point);
        for c in 0..4 {
            rgba[c] += (other[c] - rgba[c]) * w;
        }
        rgba
    }
}

/// A rooted horizontal displacement applied in the sprite's own (unscaled) tile
/// coordinates before sampling. Forward map `x' = x + D(H)`, `y' = y`; the renderer samples
/// the exact inverse `x = x' − D(H)`. Rows are preserved, so [`Mask`] coverage is evaluated
/// at the same (material) row and strip ownership is unchanged.
///
/// This is how wind moves a plant: one number per plant (or per column) per frame, and a
/// smooth profile that is still at the root, so a stem leans and its roots do not skate.
/// There is no iterative solve, no fold and no vertical rescaling, and no per-pixel
/// trigonometry — [`Bend::displacement`] is two multiplies and a clamp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bend {
    /// Displacement at and beyond `root + length`, in tile pixels along tile `+x`.
    pub amplitude: f64,
    /// Height of the tile's bottom edge above the plant's root line, in tile pixels
    /// (0 for a small plant; `4i − 8` for column tile `i`).
    pub base: f64,
    /// Heights `H ≤ root` are fixed (the painted root rows stay still).
    pub root: f64,
    /// Fixed mature bend length (not the current growth height): the height above the root
    /// at which the full amplitude is reached. Authored per family, so a column's lower
    /// trunk pixels do not slide sideways whenever a new segment grows.
    pub length: f64,
}

impl Bend {
    /// No bend at all: the identity, which [`stamp_layers_bent`] takes on exactly the code
    /// path [`stamp_layers`] has always taken, bit for bit and at the same cost.
    pub const NONE: Bend = Bend { amplitude: 0.0, base: 0.0, root: 0.0, length: 0.0 };

    /// Whether this bend displaces nothing, so the stamp is the unbent one.
    ///
    /// **Normative**: true when `amplitude` is zero or non-finite, when `length` is
    /// non-positive or non-finite, or when `base` or `root` is non-finite. Nonsense wind
    /// therefore draws the still image rather than a wrong one or nothing at all.
    pub fn is_identity(self) -> bool {
        !(self.amplitude.is_finite() && self.amplitude != 0.0)
            || !(self.length.is_finite() && self.length > 0.0)
            || !(self.base.is_finite() && self.root.is_finite())
    }

    /// The fraction of [`Bend::amplitude`] a sample at height `H` above the plant's root
    /// line receives, in `[0, 1]`.
    ///
    /// **Normative**: `smoothstep(clamp((H − root) / length, 0, 1))` with the Hermite
    /// polynomial `t²(3 − 2t)`, so both the value and the slope are 0 at `H = root` (the
    /// root is fixed and the bend grows out of it without a kink) and the value is 1, with
    /// slope 0, at and beyond `root + length`. A non-positive or non-finite `length`, and a
    /// `NaN` height, yield 0.
    pub fn profile(self, h: f64) -> f64 {
        if !(self.length.is_finite() && self.length > 0.0) {
            return 0.0;
        }
        let t = (h - self.root) / self.length;
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        t * t * (3.0 - 2.0 * t)
    }

    /// The displacement `D` of a sample at tile row coordinate `p_y` (pixels down from the
    /// tile's top edge, unscaled) in a tile `tile_height` rows tall.
    ///
    /// **Normative**: `H = (tile_height − p_y) + base` and `D = amplitude ·
    /// `[`Bend::profile`]`(H)`. The identity ([`Bend::is_identity`]) is 0 everywhere.
    pub fn displacement(self, tile_height: f64, p_y: f64) -> f64 {
        if !self.amplitude.is_finite() {
            return 0.0;
        }
        self.amplitude * self.profile(tile_height - p_y + self.base)
    }
}

/// A coverage mask applied in the sprite's own tile coordinates, before compositing.
/// Units are sprite pixels, measured on the *unscaled* tile; `h` below is a sample's
/// height above the tile's bottom edge (a pixel center at row `r` from the top of an
/// `H`-row tile has `h = H − r − 0.5`), `r` its distance from the pivot.
///
/// Every variant is a one-pixel linear ramp times a *start envelope*, so the image is a
/// continuous function of the reveal all the way from nothing: a reveal advancing by a
/// fraction of a pixel changes the image by a fraction of a pixel, no row pops in whole,
/// and a reveal tending to zero tends to an empty stamp (the envelope is what keeps a
/// sample sitting exactly on the pivot, or a bilinear tail sampled below the bottom
/// edge, from appearing at half coverage the instant the reveal is positive). Coverage
/// multiplies all four premultiplied channels, so a masked pixel darkens outline and
/// fill together and never leaves a halo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mask {
    /// No mask: coverage 1 everywhere.
    None,
    /// Reveal from the tile's bottom edge upward, for a plant authored standing along the
    /// tile's `−y`. **Normative**: coverage `clamp(reveal − h + 0.5, 0, 1) · clamp(reveal,
    /// 0, 1)`; `reveal ≤ 0` draws nothing, `reveal ≥ H + 0.5` is [`Mask::None`], and the
    /// row centered at `h = reveal − 0.5` is the one fading in.
    Axial { reveal: f64 },
    /// Reveal from the pivot outward, for a radial (top-down) plant. **Normative**:
    /// coverage `clamp(reveal − r + 0.5, 0, 1) · clamp(reveal, 0, 1)`; `reveal ≤ 0` draws
    /// nothing, `reveal ≥ extent + 0.5` is [`Mask::None`].
    Radial { reveal: f64 },
    /// A horizontal strip of the tile, from `floor` up to `reveal` (both heights above the
    /// bottom edge), for a tile that must paint only the rows it *owns* — a tall plant's
    /// trunk segments overlap by twelve rows and each row must be composited exactly once.
    /// **Normative**: coverage `clamp(reveal − h + 0.5, 0, 1) · clamp(h − floor + 0.5, 0,
    /// 1) · clamp(reveal − floor, 0, 1)`. Rows whose centers lie in `(floor, reveal)` are
    /// whole: with integer `floor` and `reveal` the strip is exactly rows `floor ..
    /// reveal` counted from the bottom, `reveal ≤ floor` draws nothing, and a strip
    /// growing from `floor` fades its first row in.
    Strip { floor: f64, reveal: f64 },
}

impl Mask {
    /// Whether the mask can let anything through at all.
    fn is_empty(self) -> bool {
        match self {
            Mask::None => false,
            Mask::Axial { reveal } | Mask::Radial { reveal } => !(reveal > 0.0),
            Mask::Strip { floor, reveal } => !(reveal > floor),
        }
    }

    /// Coverage of a sample at tile-local `p` (pixels from the tile's upper-left corner)
    /// for a tile `height` rows tall with pivot `pivot`.
    fn coverage(self, p: Vec2, height: usize, pivot: Vec2) -> f32 {
        let unit = |v: f64| if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
        let c = match self {
            Mask::None => return 1.0,
            Mask::Axial { reveal } => {
                let h = height as f64 - p.y;
                unit(reveal - h + 0.5) * unit(reveal)
            }
            Mask::Radial { reveal } => {
                let r = (p - pivot).length();
                unit(reveal - r + 0.5) * unit(reveal)
            }
            Mask::Strip { floor, reveal } => {
                let h = height as f64 - p.y;
                unit(reveal - h + 0.5) * unit(h - floor + 0.5) * unit(reveal - floor)
            }
        };
        c as f32
    }
}

/// Stamp a blended pose through one unfold and one source-over per pixel, with an
/// optional reveal mask.
///
/// The footprint is [`Pose::extent`] times `scale`, and the same nine-pixel budget applies
/// to it. The heading and scale work exactly as in [`stamp_sprite`]; with `mix = 0` and
/// [`Mask::None`] this *is* `stamp_sprite`, bit for bit. A `mix` of 1 draws `second` alone
/// (no blend arithmetic), so a clip that lands exactly on a frame costs what a single
/// stamp costs. Exactly [`stamp_layers`] with this one pose at weight 1.
pub fn stamp_pose(
    canvas: &mut Canvas,
    anchor: SurfacePoint,
    heading: Vec2,
    pose: Pose,
    scale: f64,
    opacity: f32,
    mask: Mask,
    scratch: &mut Vec<PixelImage>,
) {
    stamp_layers(canvas, anchor, heading, &[(pose, 1.0)], scale, opacity, mask, scratch);
}

/// Stamp a weighted mix of poses through one unfold and one source-over per pixel.
///
/// This is the cross-clip fade: a body changing state, or a plant coming into fruit, is a
/// mix of two (rarely three) clips, each of which keeps its *own* temporal blend between
/// bracketing samples, so a fade neither drops the sway back to held frames nor snaps at
/// its ends. The sample composited is `Σ wᵢ · sampleᵢ` in premultiplied linear RGBA over
/// the layers with `wᵢ > 0`; callers pass weights summing to 1 for an exact lerp (a
/// pixel opaque in every layer then stays opaque, where the same fade as separate
/// partially opaque stamps would let background through). Non-finite or negative
/// weights read as 0; a layer at weight 0 is not sampled and does not enlarge the
/// footprint. The footprint is the largest participating [`Pose::extent`] times `scale`,
/// under the nine-pixel budget. The mask applies to the mixed sample, in the tile
/// coordinates of the first participating layer's first sprite (all layers of one stamp
/// share a tile size and pivot: they come from one rig).
///
/// Exactly [`stamp_layers_bent`] with [`Bend::NONE`], bit for bit.
pub fn stamp_layers(
    canvas: &mut Canvas,
    anchor: SurfacePoint,
    heading: Vec2,
    layers: &[(Pose, f32)],
    scale: f64,
    opacity: f32,
    mask: Mask,
    scratch: &mut Vec<PixelImage>,
) {
    stamp_layers_bent(canvas, anchor, heading, layers, scale, opacity, mask, Bend::NONE, scratch);
}

/// [`stamp_layers`] with a rooted horizontal [`Bend`] applied in the shared tile
/// coordinates of its layers: the one deformation the wind uses.
///
/// **Normative.** Everything [`stamp_layers`] documents still holds; the bend changes
/// exactly two things.
///
/// * **Where the layers are read.** For a destination pixel at tile-local `p` (pixels from
///   the tile's upper-left corner, on the *unscaled* tile, `p = local + pivot`), the mixed
///   sample of every layer is taken at `(p.x − D, p.y)` in that layer's own sprite, with
///   `D = `[`Bend::displacement`]`(tile_height, p.y)` of the first participating layer's
///   tile. This is the exact inverse of the forward map `x' = x + D(H)`, `y' = y`, so no
///   solve, fold or vertical resampling is involved and a row of the sprite lands on the
///   same row of the tile. The mask is evaluated at `p` itself — the *material* row and
///   radius — so a growth reveal stays attached to the plant and [`Mask::Strip`] row
///   ownership is exactly what it was.
/// * **How far the stamp unfolds.** The footprint radius becomes `min(9, (extent +
///   |amplitude|) · scale)` with `extent` the largest participating [`Pose::extent`], so a
///   texel carried sideways by its own `D` still has its bilinear support inside the stamp.
///   The nine-pixel bound is never enlarged; the caller guarantees, through
///   [`Sprite::bend_headroom`], that every painted texel displaced by its own `D` stays
///   inside it, so the `min` never clips anything. A stamp is still rejected only when the
///   *unbent* `extent · scale` exceeds nine pixels — an in-budget pose is never skipped,
///   scaled down or silently clipped because the wind is blowing.
///
/// An identity [`Bend`] ([`Bend::is_identity`] — zero or non-finite amplitude, non-positive
/// length, non-finite base or root) takes exactly the path [`stamp_layers`] has always
/// taken: the same radius, the same sample coordinates, the same arithmetic, so the
/// zero-wind image is bit for bit the image without this feature and costs the same.
pub fn stamp_layers_bent(
    canvas: &mut Canvas,
    anchor: SurfacePoint,
    heading: Vec2,
    layers: &[(Pose, f32)],
    scale: f64,
    opacity: f32,
    mask: Mask,
    bend: Bend,
    scratch: &mut Vec<PixelImage>,
) {
    stamp_bent(canvas, anchor, heading, layers, scale, opacity, mask, bend, None, scratch);
}

/// [`stamp_layers_bent`] unfolding an explicitly given radius instead of the footprint it
/// would compute for itself.
///
/// Test support only, and not part of the drawn contract: it exists so an acceptance sweep
/// can stamp the same pose and bend at the nine-pixel footprint and again at a deliberately
/// larger radius and assert the two images are identical — that is, that nothing the stamp
/// would have painted was outside the footprint. Passing a radius above
/// `cubarium_surface::MAX_LOCAL_RADIUS` panics, exactly as `unfold_pixels` does.
#[doc(hidden)]
#[allow(clippy::too_many_arguments)]
pub fn stamp_layers_bent_with_radius(
    canvas: &mut Canvas,
    anchor: SurfacePoint,
    heading: Vec2,
    layers: &[(Pose, f32)],
    scale: f64,
    opacity: f32,
    mask: Mask,
    bend: Bend,
    radius: f64,
    scratch: &mut Vec<PixelImage>,
) {
    stamp_bent(canvas, anchor, heading, layers, scale, opacity, mask, bend, Some(radius), scratch);
}

#[allow(clippy::too_many_arguments)]
fn stamp_bent(
    canvas: &mut Canvas,
    anchor: SurfacePoint,
    heading: Vec2,
    layers: &[(Pose, f32)],
    scale: f64,
    opacity: f32,
    mask: Mask,
    bend: Bend,
    override_radius: Option<f64>,
    scratch: &mut Vec<PixelImage>,
) {
    let mut extent = 0.0f64;
    let mut reference: Option<&Sprite> = None;
    for (pose, w) in layers {
        if layer_weight(*w) > 0.0 {
            extent = extent.max(pose.extent());
            reference.get_or_insert(pose.first);
        }
    }
    let Some(reference) = reference else { return };
    if !scale.is_finite()
        || scale <= 0.0
        || extent * scale > FOOTPRINT_RADIUS
        || !opacity.is_finite()
        || opacity <= 0.0
        || extent == 0.0
        || mask.is_empty()
    {
        return;
    }
    let Some(h) = heading.normalized() else {
        return;
    };
    let opacity = opacity.min(1.0);
    if bend.is_identity() {
        let radius = override_radius.unwrap_or(extent * scale);
        stamp_unfolded::<false>(
            canvas,
            anchor,
            h,
            layers,
            scale,
            opacity,
            mask,
            reference,
            Bend::NONE,
            radius,
            scratch,
        );
    } else {
        let radius = override_radius
            .unwrap_or_else(|| ((extent + bend.amplitude.abs()) * scale).min(FOOTPRINT_RADIUS));
        stamp_unfolded::<true>(
            canvas, anchor, h, layers, scale, opacity, mask, reference, bend, radius, scratch,
        );
    }
}

/// A layer weight sanitized into "sampled" (`> 0`) or "not sampled" (`0`).
#[inline]
fn layer_weight(w: f32) -> f32 {
    if w.is_finite() && w > 0.0 { w } else { 0.0 }
}

/// The one unfold-and-composite loop of every stamp. `BENT` is a compile-time constant so
/// the unbent path carries no bend arithmetic and no branch at all.
#[allow(clippy::too_many_arguments)]
fn stamp_unfolded<const BENT: bool>(
    canvas: &mut Canvas,
    anchor: SurfacePoint,
    h: Vec2,
    layers: &[(Pose, f32)],
    scale: f64,
    opacity: f32,
    mask: Mask,
    reference: &Sprite,
    bend: Bend,
    radius: f64,
    scratch: &mut Vec<PixelImage>,
) {
    let side = Vec2::new(-h.y, h.x);
    let tile_height = reference.height as f64;
    unfold_pixels(anchor, radius, scratch);
    for pixel in scratch.iter() {
        let d = pixel.local - anchor.chart();
        let local = Vec2::new(h.dot(d) / scale, side.dot(d) / scale);
        // The material row is never displaced, so the sample only moves along tile +x.
        let at = if BENT {
            Vec2::new(local.x - bend.displacement(tile_height, local.y + reference.pivot.y), local.y)
        } else {
            local
        };
        let mut rgba = [0.0f32; 4];
        for (pose, w) in layers {
            let w = layer_weight(*w);
            if w <= 0.0 {
                continue;
            }
            let sample = pose.sample(at);
            for c in 0..4 {
                rgba[c] += sample[c] * w;
            }
        }
        if mask != Mask::None {
            // In the *material* coordinates of the sprite, so a reveal covers the same
            // material however the wind displaces it. Rows are preserved, so for
            // `Mask::Axial` and `Mask::Strip` — which read only the row — this is exactly
            // the destination coordinate and strip ownership is untouched.
            let coverage = mask.coverage(at + reference.pivot, reference.height, reference.pivot);
            if coverage <= 0.0 {
                continue;
            }
            for v in &mut rgba {
                *v *= coverage;
            }
        }
        let a = rgba[3] * opacity;
        if a <= 0.0 {
            continue;
        }
        let background = canvas.get(pixel.face, pixel.x, pixel.y);
        canvas.set(
            pixel.face,
            pixel.x,
            pixel.y,
            std::array::from_fn(|c| rgba[c] * opacity + background[c] * (1.0 - a)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_proto::Face;

    fn total(canvas: &Canvas) -> f64 {
        Face::ALL
            .into_iter()
            .flat_map(|f| (0..64).flat_map(move |y| (0..64).map(move |x| (f, x, y))))
            .map(|(f, x, y)| canvas.get(f, x, y).into_iter().map(f64::from).sum::<f64>())
            .sum()
    }

    #[test]
    fn alpha_uses_linear_source_over_and_does_not_add_a_halo() {
        let s = Sprite::from_rgba(1, 1, Vec2::new(0.5, 0.5), &[255, 0, 0, 128]).unwrap();
        let mut canvas = Canvas::new();
        canvas.set(Face::Front, 20, 20, [0.0, 0.0, 1.0]);
        stamp_sprite(
            &mut canvas,
            SurfacePoint::pixel_center(Face::Front, 20, 20),
            Vec2::new(1.0, 0.0),
            &s,
            1.0,
            1.0,
            &mut vec![],
        );
        let px = canvas.get(Face::Front, 20, 20);
        let a = 128.0 / 255.0;
        assert!((px[0] - a).abs() < 1e-6 && px[1] == 0.0 && (px[2] - (1.0 - a)).abs() < 1e-6);
    }

    #[test]
    fn an_asymmetric_sprite_crosses_the_seam_without_losing_or_duplicating_light() {
        let pixels = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let s = Sprite::from_rgba(4, 1, Vec2::new(2.0, 0.5), &pixels).unwrap();
        let mut middle = Canvas::new();
        let mut seam = Canvas::new();
        stamp_sprite(
            &mut middle,
            SurfacePoint::new(Face::Front, 32.0, 32.5),
            Vec2::new(1.0, 0.0),
            &s,
            1.0,
            1.0,
            &mut vec![],
        );
        stamp_sprite(
            &mut seam,
            SurfacePoint::new(Face::Front, 63.0, 32.5),
            Vec2::new(1.0, 0.0),
            &s,
            1.0,
            1.0,
            &mut vec![],
        );
        assert!((total(&middle) - total(&seam)).abs() < 1e-6);
        assert_eq!(seam.get(Face::Front, 61, 32), [1.0, 0.0, 0.0]);
        assert_eq!(seam.get(Face::Front, 63, 32), [0.0, 0.0, 1.0]);
        assert_eq!(seam.get(Face::Right, 0, 32), [1.0, 1.0, 1.0]);
    }

    /// A 5×16 tile with an opaque one-pixel stem up its center from row 15 to row 1, the
    /// colour of each row encoding the row, and the pivot at the tile center (8 rows is a
    /// plant tile's pivot row, so this stands like a plant on the anchor's line).
    fn stem() -> Sprite {
        let mut bytes = vec![0u8; 5 * 16 * 4];
        for row in 1..16usize {
            let i = (row * 5 + 2) * 4;
            bytes[i] = (row * 16) as u8;
            bytes[i + 1] = 255;
            bytes[i + 2] = 0;
            bytes[i + 3] = 255;
        }
        Sprite::from_rgba(5, 16, Vec2::new(2.5, 8.0), &bytes).unwrap()
    }

    fn bend_of(amplitude: f64) -> Bend {
        Bend { amplitude, base: 0.0, root: 1.0, length: 13.0 }
    }

    #[test]
    fn the_bend_profile_is_still_at_the_root_and_saturates_at_the_length() {
        let b = bend_of(2.0);
        assert_eq!(b.profile(0.0), 0.0);
        assert_eq!(b.profile(1.0), 0.0, "the root height itself does not move");
        assert_eq!(b.profile(-5.0), 0.0, "below the root is fixed");
        assert_eq!(b.profile(14.0), 1.0);
        assert_eq!(b.profile(99.0), 1.0);
        assert_eq!(b.profile(f64::NAN), 0.0);
        // Zero slope at both ends: the first and last tenth move far less than the middle.
        let step = |h: f64| b.profile(h + 0.1) - b.profile(h);
        assert!(step(1.0) < step(7.5) / 10.0, "no kink at the root");
        assert!(step(13.9) < step(7.5) / 10.0, "no kink at the tip");
        // Monotone and bounded in between.
        let mut last = -1.0;
        for k in 0..=140 {
            let v = b.profile(f64::from(k) / 10.0);
            assert!(v >= last - 1e-12 && (0.0..=1.0).contains(&v));
            last = v;
        }
        // D is the amplitude times the profile, at the documented height.
        assert_eq!(b.displacement(16.0, 0.5), 2.0 * b.profile(15.5));
        assert_eq!(b.displacement(16.0, 15.5), 2.0 * b.profile(0.5));
        // A base lifts the whole tile: tile row 15.5 of a column tile eight px up is at
        // H = 8, not 0.5.
        let lifted = Bend { base: 8.0, ..b };
        assert_eq!(lifted.displacement(16.0, 15.5), 2.0 * b.profile(8.5));
    }

    #[test]
    fn every_identity_bend_draws_the_unbent_image_bit_for_bit() {
        let s = stem();
        let anchors = [
            SurfacePoint::new(Face::Front, 32.5, 32.5),
            SurfacePoint::new(Face::Front, 63.5, 0.5),
            SurfacePoint::new(Face::Top, 0.5, 63.5),
        ];
        let identities = [
            Bend::NONE,
            bend_of(0.0),
            bend_of(f64::NAN),
            bend_of(f64::INFINITY),
            Bend { amplitude: 3.0, base: 0.0, root: 1.0, length: 0.0 },
            Bend { amplitude: 3.0, base: 0.0, root: 1.0, length: -13.0 },
            Bend { amplitude: 3.0, base: f64::NAN, root: 1.0, length: 13.0 },
            Bend { amplitude: 3.0, base: 0.0, root: f64::NAN, length: 13.0 },
        ];
        for anchor in anchors {
            let mut plain = Canvas::new();
            stamp_layers(
                &mut plain,
                anchor,
                Vec2::new(1.0, 0.0),
                &[(Pose::still(&s), 1.0)],
                1.0,
                0.85,
                Mask::None,
                &mut vec![],
            );
            for bend in identities {
                assert!(bend.is_identity(), "{bend:?}");
                let mut bent = Canvas::new();
                stamp_layers_bent(
                    &mut bent,
                    anchor,
                    Vec2::new(1.0, 0.0),
                    &[(Pose::still(&s), 1.0)],
                    1.0,
                    0.85,
                    Mask::None,
                    bend,
                    &mut vec![],
                );
                for face in Face::ALL {
                    for y in 0..64u8 {
                        for x in 0..64u8 {
                            assert_eq!(
                                plain.get(face, x, y),
                                bent.get(face, x, y),
                                "{bend:?} at {anchor:?}, pixel {face:?} ({x},{y})"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_bend_moves_the_tip_monotonically_and_leaves_the_root_row_alone() {
        let s = stem();
        let anchor = SurfacePoint::new(Face::Front, 32.5, 40.5);
        // Heading (1, 0) is `stalk_heading` of "up the face": the tile's +y runs down the
        // face, so the tile's rows are the face's rows (tile row `r` is face `y = 33 + r`)
        // and the bend displaces along the face's +x. The stem's lowest painted row (tile
        // row 15) is 0.5 px above the root line, which the profile holds still; its top row
        // (tile row 1) is past `root + length` and takes the whole amplitude.
        let draw = |amplitude: f64| {
            let mut canvas = Canvas::new();
            stamp_layers_bent(
                &mut canvas,
                anchor,
                Vec2::new(1.0, 0.0),
                &[(Pose::still(&s), 1.0)],
                1.0,
                1.0,
                Mask::None,
                bend_of(amplitude),
                &mut vec![],
            );
            canvas
        };
        let still = draw(0.0);
        for amplitude in [0.5, 1.0, 2.0, 3.0, -3.0] {
            let bent = draw(amplitude);
            assert_eq!(
                still.get(Face::Front, 32, 48),
                bent.get(Face::Front, 32, 48),
                "amplitude {amplitude}: the root row moved"
            );
            for x in 0..64u8 {
                assert_eq!(
                    still.get(Face::Front, x, 48),
                    bent.get(Face::Front, x, 48),
                    "amplitude {amplitude}: the root row changed at x={x}"
                );
            }
            assert_ne!(still_vs(&still, &bent), 0, "amplitude {amplitude} changed nothing at all");
        }
        // Monotone: the displaced tip's light moves further with a larger amplitude.
        let centroid = |canvas: &Canvas| {
            let mut sum = 0.0;
            let mut weight = 0.0;
            for y in 0..36u8 {
                for x in 0..64u8 {
                    let px = canvas.get(Face::Front, x, y);
                    let l = f64::from(px[0] + px[1] + px[2]);
                    if l > 0.0 {
                        // The upper part of the stem only: the tip's neighbourhood.
                        sum += f64::from(x) * l;
                        weight += l;
                    }
                }
            }
            sum / weight
        };
        let mut last = centroid(&still);
        for amplitude in [0.25, 0.5, 1.0, 2.0] {
            let c = centroid(&draw(amplitude));
            assert!(c > last + 1e-6, "amplitude {amplitude}: tip centroid {c} <= {last}");
            last = c;
        }
        // And the other way for a negative amplitude.
        assert!(centroid(&draw(-1.0)) < centroid(&still) - 1e-6);
    }

    /// Pixels where two canvases differ.
    fn still_vs(a: &Canvas, b: &Canvas) -> usize {
        Face::ALL
            .into_iter()
            .flat_map(|f| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (f, x, y))))
            .filter(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y))
            .count()
    }

    #[test]
    fn a_bent_stamp_conserves_its_light_across_a_seam_and_at_a_vertex() {
        let s = stem();
        let mut middle = Canvas::new();
        let mut seam = Canvas::new();
        let mut vertex = Canvas::new();
        let bend = bend_of(2.0);
        for (canvas, anchor) in [
            (&mut middle, SurfacePoint::new(Face::Front, 32.5, 32.5)),
            (&mut seam, SurfacePoint::new(Face::Front, 63.5, 32.5)),
            (&mut vertex, SurfacePoint::new(Face::Front, 63.5, 0.5)),
        ] {
            stamp_layers_bent(
                canvas,
                anchor,
                Vec2::new(1.0, 0.0),
                &[(Pose::still(&s), 1.0)],
                1.0,
                1.0,
                Mask::None,
                bend,
                &mut vec![],
            );
        }
        // Away from the vertex the seam only relabels pixels: the same light, and it is
        // spread over more than one face.
        assert!((total(&middle) - total(&seam)).abs() < 1e-6, "seam lost light");
        assert!(
            Face::ALL.into_iter().filter(|&f| (0..64u8).any(|y| (0..64u8).any(|x| seam.get(f, x, y) != [0.0; 3]))).count() >= 2,
            "the seam stamp did not reach the neighbouring face"
        );
        // At a vertex the 90° angular deficit can only *lose* light (each pixel is
        // composited once), never duplicate it.
        assert!(total(&vertex) <= total(&middle) + 1e-6, "a vertex duplicated light");
        assert!(total(&vertex) > total(&middle) * 0.5, "a vertex swallowed the stamp");
    }

    #[test]
    fn the_headroom_is_the_largest_amplitude_that_keeps_every_texel_inside_the_budget() {
        let s = stem();
        let room = s.bend_headroom(1.0, 13.0, 0.0);
        assert!(room.is_finite() && room > 0.0, "{room}");
        // The criterion, checked by hand at the budget and just past it: the whole bilinear
        // support of every painted texel, displaced by the largest `D` that can read it,
        // inside the nine-pixel footprint.
        let worst = |amplitude: f64| {
            let probe = Bend { amplitude, base: 0.0, root: 1.0, length: 13.0 };
            let mut worst = 0.0f64;
            for row in 0..16usize {
                for col in 0..5usize {
                    if s.pixel(col as i32, row as i32)[3] <= 0.0 {
                        continue;
                    }
                    let y = (row as f64 + 0.5 - s.pivot.y).abs() + 1.0;
                    let x = (col as f64 + 0.5 - s.pivot.x).abs() + 1.0;
                    // One pixel above the texel center, exactly as the criterion says.
                    let d = probe.displacement(16.0, row as f64 - 0.5).abs();
                    worst = worst.max((x + d).hypot(y));
                }
            }
            worst
        };
        assert!(worst(room) <= FOOTPRINT_RADIUS + 1e-9, "at the budget: {}", worst(room));
        assert!(worst(room * 1.2) > FOOTPRINT_RADIUS + 1e-9, "the budget is not tight");
        // A bend nothing can follow (no length) bounds nothing.
        assert_eq!(s.bend_headroom(1.0, 0.0, 0.0), f64::INFINITY);
        // A root above the whole tile freezes every texel, so again no bound.
        assert_eq!(s.bend_headroom(99.0, 13.0, 0.0), f64::INFINITY);
        // Lifting the tile (a column tile high up) can only reduce the budget: more of the
        // tile sits past the root and moves by more of the amplitude.
        assert!(s.bend_headroom(0.0, 48.0, 28.0) <= s.bend_headroom(0.0, 48.0, -8.0));
        // A sprite whose painted texels already sit at the budget gets no room at all.
        let mut wide = vec![0u8; 16 * 16 * 4];
        for (i, b) in wide.iter_mut().enumerate() {
            *b = if i % 4 == 3 { 255 } else { 128 };
        }
        let wide = Sprite::from_rgba(16, 16, Vec2::new(8.0, 8.0), &wide);
        assert!(wide.is_err(), "a full 16x16 tile is past the nine-pixel budget already");
    }

    #[test]
    fn at_every_admitted_amplitude_the_footprint_radius_clips_nothing() {
        let s = stem();
        let room = s.bend_headroom(1.0, 13.0, 0.0);
        // Anchors in the middle of a chart, on a seam, and on a vertex.
        for anchor in [
            SurfacePoint::new(Face::Front, 32.5, 32.5),
            SurfacePoint::new(Face::Front, 63.5, 32.5),
            SurfacePoint::new(Face::Front, 63.5, 0.5),
            SurfacePoint::new(Face::Top, 0.5, 0.5),
        ] {
            for k in 0..=8 {
                let amplitude = room * f64::from(k) / 8.0;
                for amplitude in [amplitude, -amplitude] {
                    let bend = bend_of(amplitude);
                    let mut budgeted = Canvas::new();
                    let mut generous = Canvas::new();
                    stamp_layers_bent(
                        &mut budgeted,
                        anchor,
                        Vec2::new(1.0, 0.0),
                        &[(Pose::still(&s), 1.0)],
                        1.0,
                        1.0,
                        Mask::None,
                        bend,
                        &mut vec![],
                    );
                    stamp_layers_bent_with_radius(
                        &mut generous,
                        anchor,
                        Vec2::new(1.0, 0.0),
                        &[(Pose::still(&s), 1.0)],
                        1.0,
                        1.0,
                        Mask::None,
                        bend,
                        16.0,
                        &mut vec![],
                    );
                    assert_eq!(
                        still_vs(&budgeted, &generous),
                        0,
                        "amplitude {amplitude} at {anchor:?}: a larger radius painted more, so \
                         the nine-pixel footprint clipped the bend"
                    );
                }
            }
        }
    }

    #[test]
    fn a_bend_does_not_move_a_mask_off_its_material_rows() {
        let s = stem();
        let anchor = SurfacePoint::new(Face::Front, 32.5, 32.5);
        let draw = |bend: Bend, mask: Mask| {
            let mut canvas = Canvas::new();
            stamp_layers_bent(
                &mut canvas,
                anchor,
                Vec2::new(1.0, 0.0),
                &[(Pose::still(&s), 1.0)],
                1.0,
                1.0,
                mask,
                bend,
                &mut vec![],
            );
            canvas
        };
        // With the tile's +x along the chart's +u, the tile's rows are the face's rows. A
        // strip owning heights 4..8 above the tile's bottom edge paints the same face rows
        // whether bent or not: coverage is evaluated at the material row.
        let mask = Mask::Strip { floor: 4.0, reveal: 8.0 };
        let plain = draw(Bend::NONE, mask);
        let bent = draw(bend_of(2.0), mask);
        let rows = |canvas: &Canvas| {
            (0..64u8)
                .filter(|&y| (0..64u8).any(|x| canvas.get(Face::Front, x, y) != [0.0; 3]))
                .collect::<Vec<_>>()
        };
        assert_eq!(rows(&plain), rows(&bent), "the bend moved the strip's rows");
        assert!(!rows(&plain).is_empty());
        // An empty mask still draws nothing, bent or not.
        let empty = Mask::Strip { floor: 8.0, reveal: 8.0 };
        assert_eq!(total(&draw(bend_of(2.0), empty)), 0.0);
    }

    #[test]
    fn invalid_assets_and_extent_are_rejected() {
        assert!(Sprite::from_rgba(2, 2, Vec2::ZERO, &[0; 3]).is_err());
        assert!(Sprite::from_rgba(1, 1, Vec2::new(f64::NAN, 0.0), &[0; 4]).is_err());
        assert!(Sprite::from_rgba(32, 32, Vec2::ZERO, &vec![255; 32 * 32 * 4]).is_err());
    }
}
