//! Multipart rigs: one animal drawn as several bounded sprites through **one** root-owned
//! pixel query.
//!
//! A [`Sprite`] stamp is bounded by the nine-pixel per-stamp footprint, and that bound is not
//! negotiable per material. A body longer than that — the Lanternjaw is 18 px from tail fan
//! to jaw and its strike reaches 12 px ahead of its anchor — is drawn as **parts**: each part
//! is an ordinary in-budget sprite placed at a fixed body-local offset from the animal's root
//! anchor. What is *not* done is stamping each part from its own anchor: two independent
//! stamps near a top vertex can pick different chart images for the same pixel and tear a
//! joint or double its coverage, and carrying an attachment offset with `travel` reflects it
//! at the open rim, folding a claw back onto the body. Instead (Astra,
//! `design/7_Research/astra-lanternjaw-production-plan-2026-09-13.md`) the rig makes **one**
//! `unfold_pixels` query from the root with a conservative radius — the surface's own
//! `MAX_LOCAL_RADIUS` of 32 is the legal bound for a query; nine is the per-sprite material
//! budget — and samples every part in that one chart image. Every physical pixel then has one
//! owner and one body coordinate, joints share it, the open rim simply has no pixels past it,
//! and a top vertex shows the same single localized cut any stamp shows there.
//!
//! Two kinds of overlap are distinguished:
//!
//! * pieces of **one material** (a hull cut into column ranges so each piece fits its budget)
//!   share a `layer` and are **summed**: a material rasterized on one lattice and cut into
//!   aligned pieces is reconstructed exactly — bilinear sampling is linear, so the sum of the
//!   pieces' samples is the sample of the uncut material — with no translucent seam where two
//!   separately filtered halves would otherwise source-over and no bright ridge where they
//!   would double;
//! * genuinely separate depths (a far forelimb under the hull, a near one over it) use
//!   different `layer`s and are composited **source-over in ascending layer order**, the
//!   study's painting order made explicit.
//!
//! A state cross-fade composes each state's complete depth-ordered body sample first and
//! mixes those premultiplied samples, then source-overs once — never two partially opaque
//! whole-body redraws.

use cubarium_surface::{MAX_LOCAL_RADIUS, PixelImage, SurfacePoint, Vec2, unfold_pixels};

use crate::{Canvas, Sprite};

/// One piece of a rig for one frame.
#[derive(Clone, Copy, Debug)]
pub struct RigPart<'a> {
    /// The piece's pixels; its pivot is the point placed at `offset`.
    pub sprite: &'a Sprite,
    /// Body-local position of the sprite's pivot: body `+x` is the heading, body `+y` the
    /// heading turned one quarter turn clockwise on screen (image-down when the heading is
    /// image-right), exactly the frame of [`crate::stamp_body`] and of every sprite stamp.
    pub offset: Vec2,
    /// Depth. Pieces with equal `layer` are one material and are summed; layers composite
    /// source-over in ascending order.
    pub layer: u8,
}

/// Extra query radius beyond the parts' own extents, in pixels: the radial
/// `Sprite::extent` support is a hair short of a texel's true bilinear corner (`√2` versus
/// `1/√2 + 1/2`), and this margin more than covers the difference so a generous reference
/// query can never paint a pixel the rig's own query did not enumerate.
pub const RIG_MARGIN: f64 = 0.5;

/// How far a box-filter sample can sit from its destination pixel's centre, **radially**, in
/// chart pixels: the corner of the pixel's square, `√2 · (0.5 − 0.5 / n) < 1/√2`. Added to a
/// minified rig's query radius so the outermost sample of the outermost pixel is inside the
/// query (a per-axis half pixel was not enough: at scale 0.2 a corner sample sits 0.566 px
/// out, and Astra's fixture — a 1×1 sprite at Front (31.902, 31.902) — lost a real, if
/// tiny, bilinear tail at pixel (32, 32)).
pub const SUPERSAMPLE_REACH: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// The smallest whole-rig scale the box filter supports: `1 / MAX_GRID`. Below it a
/// destination pixel would need a grid past [`MAX_GRID`], and the work grows as the square
/// of `1 / scale` without bound, so a smaller scale is a **configuration error** that panics
/// in every build before any sampling — never silently enlarged to fit, never point-sampled
/// behind the caller's back. The bound this buys: at most `7² + 8² = 113` complete body
/// samples per pixel where two grids blend (`8² = 64` at an integer `1 / scale`); the
/// Lanternjaw's admitted minimum (0.2) needs a grid of 5, at most `4² + 5² = 41` samples.
pub const MIN_RIG_SCALE: f64 = 1.0 / MAX_GRID as f64;
/// The largest box-filter grid per axis.
pub const MAX_GRID: usize = 8;

/// The box-filter schedule for a scale: the two grids whose blend a pixel takes and the
/// weight of the finer one. **Normative**: `scale ≥ 1` ⇒ `(1, 1, 0)`, the point sample;
/// otherwise with `q = 1 / scale`, `(⌊q⌋, ⌈q⌉, q − ⌊q⌋)`, so an integer `q` is one grid alone
/// and the blend weight is continuous in the scale, reaching 0 exactly at every integer `q`
/// (and so at scale 1, where the finer grid is the point sample too).
pub fn grid_schedule(scale: f64) -> (usize, usize, f64) {
    if !(scale < 1.0) {
        return (1, 1, 0.0);
    }
    let q = 1.0 / scale;
    let lo = q.floor();
    (lo as usize, q.ceil() as usize, q - lo)
}

/// The query radius a rig needs: `max` over every part with a positive weight of
/// `|offset| + sprite.extent()`, plus [`RIG_MARGIN`]; 0 for no parts. **Normative**, and a
/// pure function of the parts, so a caller can check a rig's bound in a test rather than at
/// draw time. A non-finite offset of a participating part is **not** ignored: the radius is
/// then non-finite, which [`stamp_rig`] rejects rather than drawing part of the rig.
pub fn rig_radius(states: &[(&[RigPart<'_>], f32)]) -> f64 {
    let mut radius = 0.0f64;
    let mut any = false;
    for (parts, weight) in states {
        if state_weight(*weight) <= 0.0 {
            continue;
        }
        for part in *parts {
            any = true;
            let reach = part.offset.length() + part.sprite.extent();
            // `f64::max` would *ignore* a NaN operand and hide a broken rig; a nonsense
            // offset must propagate into the radius so the draw call rejects it.
            if reach.is_nan() || reach > radius {
                radius = reach;
            }
        }
    }
    if any { radius + RIG_MARGIN } else { 0.0 }
}

/// A state weight sanitized into "sampled" (`> 0`) or "not sampled" (`0`), exactly as
/// [`crate::stamp_layers`] sanitizes its layer weights.
#[inline]
fn state_weight(w: f32) -> f32 {
    if w.is_finite() && w > 0.0 { w } else { 0.0 }
}

/// Stamp a rig at `root` facing `heading` (chart tangent, any length): a weighted mixture of
/// **states**, each a list of parts, through one pixel query and one source-over per pixel.
///
/// **Normative.**
///
/// * `h = heading.normalized()`; none draws nothing. Body frame as [`RigPart::offset`]:
///   `side = (−h.y, h.x)`, and a destination pixel at chart `local` has body coordinate
///   `b = (h · d, side · d)` with `d = local − root.chart()`.
/// * State weights are sanitized as [`crate::stamp_layers`] sanitizes layer weights
///   (non-finite or negative reads 0); a state at weight 0 is not sampled and does not
///   enlarge the query. Nothing is drawn when every weight is 0, when `opacity` is not finite
///   or not positive, or when the radius is 0.
/// * The query is `unfold_pixels(root, `[`rig_radius`]`, scratch)`. A [`rig_radius`] that is
///   not finite (a participating part with a non-finite offset) or exceeds
///   `MAX_LOCAL_RADIUS` is a rig **configuration error** and **panics in every build**,
///   exactly as `unfold_pixels` rejects an illegal radius: silently clamping it would drop
///   visible material instead of reporting a broken rig. A rig's own tests bound its radius,
///   and a state at weight 0 neither enlarges the query nor can fail this validation.
/// * For each pixel, for each state with weight `w > 0`: the state's sample is built by
///   walking its parts grouped by ascending `layer`; within a layer `m = Σ sprite.sample_at(b
///   − offset)` over the layer's parts, then `m` is clamped to a valid premultiplied colour
///   (`a ≤ 1`, each of `r, g, b ≤ a`); the state's running sample `s` becomes `m + s · (1 −
///   m.a)` (the layer over what lies beneath). A part whose sprite cannot reach `b` (outside
///   its image plus one pixel on each axis) is skipped without sampling. The pixel's body
///   sample is `Σ w · s` over the states; with weights summing to 1 an opaque pixel stays
///   opaque through a fade.
/// * The body sample `c` is composited once: `canvas = c · opacity + canvas · (1 − c.a ·
///   opacity)`, with `opacity` clamped to 1; a pixel with `c.a == 0` is left untouched.
///
/// Properties this guarantees, which the independent tests check: a rig of one state with
/// one part at offset 0 draws what [`crate::stamp_layers_bent_with_radius`] draws with that
/// sprite at the rig's radius (bit for bit but for float reassociation — one configuration
/// in the sweep differs by a single f32 ulp), and differs from [`crate::stamp_sprite`] only
/// on the bilinear filter tail the sprite's own radial extent clipped (Astra's fixture: a
/// 16×16 sprite with texel (12, 12) painted, root Front (32.05, 32.05), paints Front (37, 37)
/// at 0.0025 where the plain stamp painted nothing — a pixel's owner does not depend on the
/// radius, so nothing else changes); a material cut into lattice-aligned pieces in one
/// layer draws what the uncut sprite draws, at integer and fractional roots and at rotated
/// headings, to floating-point rounding; two layers source-over in order; pixels past the open
/// rim do not exist, so a part hanging over it is cut, never reflected; a body straddling a
/// seam is one continuous body, and at a top vertex every pixel has one owner.
pub fn stamp_rig(
    canvas: &mut Canvas,
    root: SurfacePoint,
    heading: Vec2,
    states: &[(&[RigPart<'_>], f32)],
    opacity: f32,
    scratch: &mut Vec<PixelImage>,
) {
    stamp_rig_query(canvas, root, heading, states, opacity, 1.0, None, scratch);
}

/// [`stamp_rig`] with the **whole rig** scaled about the root: a juvenile is the same rig,
/// every part at the same relative place, `scale` times smaller.
///
/// **Normative.** `scale` must be finite and positive — anything else is a rig
/// configuration error that panics in every build, exactly like an illegal radius. Three
/// things change against [`stamp_rig`] and nothing else:
///
/// * every sample's body coordinate is divided by `scale` before the parts are sampled
///   (`b / scale`), so a part at offset `o` appears at `scale · o` from the root and its
///   texels `scale` pixels apart — offsets, pivots, the lattice, the lunge and every
///   attachment shrink together, and nothing detaches;
/// * below scale 1 a destination pixel is **box-filtered** over its own chart-aligned
///   square: an `n × n` grid of samples at chart offsets `((i + 0.5) / n − 0.5, (j + 0.5) / n
///   − 0.5)` **along the chart axes** (a pixel is a chart square whatever the body's heading;
///   cube seams are right-angle chart permutations, so the square stays axis-aligned on the
///   far side too), each sample depth-composited exactly as one pixel is, then averaged. The
///   grid follows [`grid_schedule`]: with `q = 1 / scale` the value is the mean of the `⌊q⌋`
///   grid blended toward the `⌈q⌉` grid by `q − ⌊q⌋`, so the filter is **continuous in the
///   scale** — an integer `q` is one grid, and just below scale 1 the image is almost the
///   point sample rather than a sudden full-width box (Astra's 43.75 % centre jump at
///   `1 − 1e-9` is gone). This is an approximate, smoothly varying reconstruction filter —
///   a finite quadrature, not an exact box integral even at an integer `q` (a rotated
///   bilinear field is not integrated exactly by any grid); it is a continuous
///   reconstruction choice, stated as such. A texel smaller than a pixel thereby contributes
///   about its share of the pixel's area instead of being hit or missed by one point sample:
///   the pixel over a 0.2-scale claw carries about `0.2²` of the claw's light — present and
///   geometrically honest, though far too faint to see on an LED;
/// * the query radius is [`rig_radius`]` · scale`, plus [`SUPERSAMPLE_REACH`] (the radial
///   corner reach of a box sample, `1/√2`) when box-filtered, validated against
///   `MAX_LOCAL_RADIUS` after scaling;
/// * the work is bounded: `scale` must be at least [`MIN_RIG_SCALE`] (`1 / MAX_GRID`, so at
///   most `MAX_GRID²` samples per pixel); a smaller scale is a configuration error that panics
///   before any sampling.
///
/// `scale = 1` is [`stamp_rig`] bit for bit (`n = 1`, offset exactly zero). Above 1 the body
/// is magnified by the plain bilinear sample and the caller's own radius bound must still
/// hold. The root, heading, ownership and depth rules are those of [`stamp_rig`]. Every
/// painted pixel centre lies within `scale · (r + √2) + SUPERSAMPLE_REACH` of the root when
/// `r` bounds the painted texel centres (the texel's bilinear corner is `√2` texels out, a box
/// sample up to `1/√2` px from its pixel centre; the reach term only below scale 1) — the
/// saved centred-texel case at scale 0.2 paints a pixel 0.846 px out, inside this 0.990
/// bound, and past the `0.7` an earlier per-axis statement would have allowed.
pub fn stamp_rig_scaled(
    canvas: &mut Canvas,
    root: SurfacePoint,
    heading: Vec2,
    states: &[(&[RigPart<'_>], f32)],
    scale: f64,
    opacity: f32,
    scratch: &mut Vec<PixelImage>,
) {
    stamp_rig_query(canvas, root, heading, states, opacity, scale, None, scratch);
}

/// [`stamp_rig`] with an explicit query radius in place of [`rig_radius`]. Test support only:
/// it lets an acceptance sweep draw the same rig at its own radius and at a deliberately
/// larger legal one and assert the images are identical — that is, that the rig's query lost
/// no support. Panics past `MAX_LOCAL_RADIUS` exactly as `unfold_pixels` does.
#[doc(hidden)]
pub fn stamp_rig_with_radius(
    canvas: &mut Canvas,
    root: SurfacePoint,
    heading: Vec2,
    states: &[(&[RigPart<'_>], f32)],
    opacity: f32,
    radius: f64,
    scratch: &mut Vec<PixelImage>,
) {
    stamp_rig_query(
        canvas,
        root,
        heading,
        states,
        opacity,
        1.0,
        Some(radius),
        scratch,
    );
}

#[allow(clippy::too_many_arguments)]
fn stamp_rig_query(
    canvas: &mut Canvas,
    root: SurfacePoint,
    heading: Vec2,
    states: &[(&[RigPart<'_>], f32)],
    opacity: f32,
    scale: f64,
    override_radius: Option<f64>,
    scratch: &mut Vec<PixelImage>,
) {
    assert!(
        scale.is_finite() && scale >= MIN_RIG_SCALE,
        "rig scale {scale} is not finite and at least MIN_RIG_SCALE ({MIN_RIG_SCALE}): a rig \
         configuration error, not a drawable frame"
    );
    let Some(h) = heading.normalized() else {
        return;
    };
    if !opacity.is_finite() || opacity <= 0.0 {
        return;
    }
    let opacity = opacity.min(1.0);
    let radius = match override_radius {
        // Test support: `unfold_pixels` itself rejects an illegal radius, as documented.
        Some(radius) => radius,
        None => {
            let radius = rig_radius(states) * scale;
            assert!(
                radius.is_finite() && radius <= MAX_LOCAL_RADIUS,
                "rig radius {radius} is not finite and at most MAX_LOCAL_RADIUS \
                 ({MAX_LOCAL_RADIUS}): a rig configuration error, not a drawable frame"
            );
            radius
        }
    };
    if !(radius > 0.0) {
        return;
    }
    let side = Vec2::new(-h.y, h.x);
    let origin = root.chart();
    // Minification: below scale 1 a destination pixel covers `q = 1 / scale` art texels per
    // axis. Its value is the box average of an `n × n` grid of samples over the pixel's own
    // (chart-aligned) square, and because `q` is rarely an integer the two neighbouring
    // grids `⌊q⌋` and `⌈q⌉` are blended by `q − ⌊q⌋`, so the filter changes continuously
    // with the scale — at scale 1 the blend is the point grid alone, bit for bit, and just
    // below 1 it is almost that point sample rather than a sudden full-width box.
    let (n_lo, n_hi, blend) = grid_schedule(scale);
    let minified = n_hi > 1;
    let radius = if minified {
        radius + SUPERSAMPLE_REACH
    } else {
        radius
    };
    assert!(
        radius <= MAX_LOCAL_RADIUS,
        "rig radius {radius} with its minification reach exceeds MAX_LOCAL_RADIUS"
    );
    unfold_pixels(root, radius, scratch);
    for pixel in scratch.iter() {
        let d = pixel.local - origin;
        // The mean over one `n × n` grid of the complete depth-composited body sample, each
        // sample at a chart offset inside the destination pixel (`(0, 0)` exactly for `n = 1`).
        let grid = |n: usize| -> [f32; 4] {
            let mut sum = [0.0f32; 4];
            for i in 0..n {
                for j in 0..n {
                    let d = if n == 1 {
                        d
                    } else {
                        let ox = (i as f64 + 0.5) / n as f64 - 0.5;
                        let oy = (j as f64 + 0.5) / n as f64 - 0.5;
                        d + Vec2::new(ox, oy)
                    };
                    // The body coordinate of this sample, shared by every part, in the
                    // rig's own (adult) units: a scaled rig reads its art `1 / scale`
                    // further out.
                    let b = Vec2::new(h.dot(d) / scale, side.dot(d) / scale);
                    for (parts, weight) in states {
                        let weight = state_weight(*weight);
                        if weight <= 0.0 {
                            continue;
                        }
                        let sample = state_sample(parts, b);
                        for c in 0..4 {
                            sum[c] += sample[c] * weight;
                        }
                    }
                }
            }
            if n > 1 {
                let inv = 1.0 / (n * n) as f32;
                for c in &mut sum {
                    *c *= inv;
                }
            }
            sum
        };
        let body = if !minified {
            grid(1)
        } else if blend <= 0.0 || n_lo == n_hi {
            grid(n_hi)
        } else {
            let (lo, hi) = (grid(n_lo), grid(n_hi));
            let w = blend as f32;
            std::array::from_fn(|c| lo[c] + (hi[c] - lo[c]) * w)
        };
        let a = body[3] * opacity;
        if a <= 0.0 {
            continue;
        }
        let background = canvas.get(pixel.face, pixel.x, pixel.y);
        canvas.set(
            pixel.face,
            pixel.x,
            pixel.y,
            std::array::from_fn(|c| body[c] * opacity + background[c] * (1.0 - a)),
        );
    }
}

/// One state's complete depth-ordered sample at body coordinate `b`: each `layer` is one
/// material (its parts **summed**, then clamped to a valid premultiplied colour), and the
/// layers are composited source-over in ascending order.
///
/// Layers are visited by repeatedly taking the smallest layer above the last one, so the
/// result does not depend on the order the caller happened to list its parts in, and the
/// whole walk allocates nothing.
fn state_sample(parts: &[RigPart<'_>], b: Vec2) -> [f32; 4] {
    let mut sample = [0.0f32; 4];
    let mut next = parts.iter().map(|p| p.layer).min();
    while let Some(layer) = next {
        let mut material = [0.0f32; 4];
        let mut above: Option<u8> = None;
        for part in parts {
            if part.layer == layer {
                let point = b - part.offset;
                if !part.sprite.can_reach(point) {
                    continue;
                }
                let texel = part.sprite.sample_at(point);
                for c in 0..4 {
                    material[c] += texel[c];
                }
            } else if part.layer > layer {
                above = Some(above.map_or(part.layer, |a: u8| a.min(part.layer)));
            }
        }
        next = above;
        // A material's own pieces can overlap where the body compresses; coverage never
        // exceeds one and light never exceeds coverage.
        let a = material[3].min(1.0);
        material[3] = a;
        for c in 0..3 {
            material[c] = material[c].min(a);
        }
        if a > 0.0 {
            let k = 1.0 - a;
            for c in 0..4 {
                sample[c] = material[c] + sample[c] * k;
            }
        }
    }
    sample
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Sprite, stamp_sprite};
    use cubarium_surface::Face as SurfaceFace;
    use cube_proto::Face;

    fn total_light(canvas: &Canvas) -> f64 {
        SurfaceFace::ALL
            .into_iter()
            .flat_map(|f| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (f, x, y))))
            .map(|(f, x, y)| canvas.get(f, x, y).into_iter().map(f64::from).sum::<f64>())
            .sum()
    }

    /// One opaque texel at image `(12, 12)` of a 16×16 sprite pivoted at `(8, 8)`: its own
    /// extent is `hypot(4.5, 4.5) + 1/√2 + 1/2 ≈ 7.571`, well inside the nine-pixel material
    /// budget, but its bilinear support reaches further than that radial approximation.
    fn one_texel() -> Sprite {
        let mut pixels = vec![[0.0f32; 4]; 16 * 16];
        pixels[12 * 16 + 12] = [1.0, 1.0, 1.0, 1.0];
        Sprite::from_premultiplied(16, 16, Vec2::new(8.0, 8.0), pixels).expect("in budget")
    }

    /// The one documented difference from a legacy single stamp, and the reason the rig's
    /// query carries [`RIG_MARGIN`]: `Sprite::extent`'s radial support is a hair short of a
    /// texel's true bilinear corner, so `stamp_sprite` drops the far tail of a diagonal
    /// texel's filter while the rig — whose query is large enough to enumerate it — paints
    /// it. The rig is right and the tail is real light; nothing here is clipped back.
    #[test]
    fn the_rig_paints_a_filter_tail_the_legacy_extent_radius_clipped() {
        let sprite = one_texel();
        let parts = [RigPart {
            sprite: &sprite,
            offset: Vec2::ZERO,
            layer: 0,
        }];
        let root = cubarium_surface::SurfacePoint::new(Face::Front, 32.05, 32.05);
        let heading = Vec2::new(1.0, 0.0);
        let mut rig = Canvas::new();
        let mut legacy = Canvas::new();
        stamp_rig(
            &mut rig,
            root,
            heading,
            &[(&parts, 1.0)],
            1.0,
            &mut Vec::new(),
        );
        stamp_sprite(
            &mut legacy,
            root,
            heading,
            &sprite,
            1.0,
            1.0,
            &mut Vec::new(),
        );
        // Astra's fixture: the pixel's body coordinate is (5.45, 5.45), its surface distance
        // 7.70746 — past the legacy extent 7.57107, inside the rig's 8.07107 — and the
        // bilinear weight there is 0.05 · 0.05.
        assert_eq!(
            legacy.get(Face::Front, 37, 37),
            [0.0; 3],
            "the legacy radius clipped it"
        );
        let tail = rig.get(Face::Front, 37, 37);
        for c in 0..3 {
            assert!(
                (f64::from(tail[c]) - 0.0025).abs() < 1e-7,
                "the tail should be the bilinear weight of the one texel, got {tail:?}"
            );
        }
        assert!(sprite.extent() < 7.58 && rig_radius(&[(&parts, 1.0)]) > 8.07);
        // And it is the *only* difference: every other pixel agrees bit for bit.
        for face in SurfaceFace::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    if (face, x, y) == (Face::Front, 37, 37) {
                        continue;
                    }
                    assert_eq!(
                        rig.get(face, x, y),
                        legacy.get(face, x, y),
                        "{face:?} ({x}, {y})"
                    );
                }
            }
        }
    }

    /// Seam light conservation for a **rotated** rig is a statement about the seam, so its
    /// control has to sit at the same sub-pixel phase: a rotated material resampled at
    /// another phase legitimately carries slightly different light whether or not a seam is
    /// involved. At the same phase, crossing a side/side seam changes the total light by
    /// far less than resampling at a different phase mid-face does.
    #[test]
    fn a_rotated_rig_conserves_its_light_across_a_seam_at_the_same_phase() {
        let sprite = one_texel();
        let parts = [RigPart {
            sprite: &sprite,
            offset: Vec2::new(3.0, 0.0),
            layer: 0,
        }];
        let heading = Vec2::new(1.0, 1.0);
        let light_at = |u: f64, v: f64| {
            let mut canvas = Canvas::new();
            stamp_rig(
                &mut canvas,
                cubarium_surface::SurfacePoint::new(Face::Front, u, v),
                heading,
                &[(&parts, 1.0)],
                1.0,
                &mut Vec::new(),
            );
            total_light(&canvas)
        };
        let centre = light_at(32.5, 32.5);
        let seam = light_at(63.5, 32.5);
        let other_phase = light_at(32.0, 32.0);
        assert!(centre > 0.5, "the fixture must carry real light");
        let seam_error = (seam - centre).abs() / centre;
        let phase_error = (other_phase - centre).abs() / centre;
        assert!(
            seam_error < 1e-6,
            "the seam lost light: {seam_error} ({seam} vs {centre})"
        );
        assert!(
            phase_error > seam_error,
            "a different sub-pixel phase should move more light than a seam does: \
             phase {phase_error}, seam {seam_error}"
        );
    }

    /// A nonsense offset is a rig configuration error in every build, not a silently
    /// clamped query.
    #[test]
    #[should_panic(expected = "rig radius")]
    fn a_non_finite_offset_panics_instead_of_drawing_part_of_the_rig() {
        let sprite = one_texel();
        let parts = [RigPart {
            sprite: &sprite,
            offset: Vec2::new(f64::NAN, 0.0),
            layer: 0,
        }];
        assert!(rig_radius(&[(&parts, 1.0)]).is_nan());
        stamp_rig(
            &mut Canvas::new(),
            cubarium_surface::SurfacePoint::new(Face::Front, 32.5, 32.5),
            Vec2::new(1.0, 0.0),
            &[(&parts, 1.0)],
            1.0,
            &mut Vec::new(),
        );
    }
}
