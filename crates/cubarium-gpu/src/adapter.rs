//! From one CPU stamp to one GPU instance: the mapping Stage B's driver is written in.
//!
//! # Where the seam is, and why it is here
//!
//! The brief put "the adapter" in this crate. It is here — but only the half that can
//! be: **what a stamp becomes**, not **which stamps a world wants**. The second half is
//! `art_present`, which lives in `crates/cubarium` and which this crate cannot depend on
//! without a dependency cycle (`cubarium` depends on `cubarium-gpu` for the sink).
//!
//! The alternative — transcribing `art_present` into this crate — was considered and
//! rejected. FW-5 landed a complete, public, ring-aware `ArtGeometry`, so a second copy
//! would be ~2,900 lines that (a) diverge the first time FW-5 touches the original,
//! (b) make the fidelity test measure the transcription rather than the renderer, and
//! (c) would have to be re-verified against the CPU picture anyway. The driver in
//! `crates/cubarium/src/sink/gpu.rs` therefore asks the *real* presenter every question
//! — which band, which species, which stage pair, which bend, which opacity — and turns
//! each answer into a [`Stamp`] here.
//!
//! What that buys, concretely: the two pictures cannot disagree about *what to draw*.
//! Every difference the fidelity test finds is a rasterisation difference, which is
//! exactly the thing worth measuring.
//!
//! # The mapping
//!
//! | CPU (`art_present`) | here |
//! |---|---|
//! | `stamp_layers*`'s `anchor: SurfacePoint` | [`Stamp::anchor`] — on a ring `(u, v)` **are** raster pixels, so this is a cast |
//! | `heading: Vec2` | [`Stamp::heading`] — the tile's `+x`; `+y` is `(−h.y, h.x)` both sides |
//! | `&[(Pose, f32)]` | [`Stamp::layers`], up to four frames after [`SpriteInstance::push_pose`] flattens each pose |
//! | `Bend { amplitude, base, root, length }` | [`Stamp::bend`], same units (source texels) |
//! | `Mask::{None, Axial, Strip, Radial}` | [`StampMask`] |
//! | `Tone { colour, shade, mix }` | [`StampTone`] |
//! | `opacity: f32` | [`Stamp::opacity`] |
//! | `scale: f64` | the renderer's `S`; a stamp never carries its own |

use crate::atlas::Atlas;
use crate::scene::{
    MASK_RADIAL, NO_MASK_FLOOR, NO_MASK_REVEAL, SOURCE_ATLAS, SOURCE_SCRATCH, SpriteInstance,
};

/// One pose of one clip at a weight: `cubarium_render`'s `(Pose, f32)`, with the two
/// bracketing frames named by their index in [`Atlas::frames`].
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PoseRef {
    /// The pose's first frame.
    pub a: u32,
    /// Its second; equal to `a` for a still pose.
    pub b: u32,
    /// `Pose::mix`: 0 is `a` exactly, 1 is `b` exactly.
    pub mix: f32,
    /// This pose's weight in the composite.
    pub weight: f32,
}

/// `cubarium_render::Mask`, without the dependency.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum StampMask {
    #[default]
    None,
    /// Reveal from the tile's bottom edge upward, in source rows.
    Axial { reveal: f32 },
    /// The rows from `floor` to `reveal` above the bottom edge, for a tall column's
    /// trunk segments, each of which must paint its own rows exactly once.
    Strip { floor: f32, reveal: f32 },
    /// Reveal from the pivot outward, for a radial (canopy) plant.
    Radial { reveal: f32 },
}

/// `cubarium_render::Tone` with its `Shade`, without the dependency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StampTone {
    pub colour: [f32; 3],
    pub shade_floor: f32,
    pub shade_reference: f32,
    pub mix: f32,
}

/// One frame already uploaded into the renderer's per-frame scratch page: where a
/// procedurally rasterised rig part lives, since it has no place in a baked atlas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScratchFrame {
    /// Where `Renderer::scratch_push` put it.
    pub origin: [u16; 2],
    /// Its size in texels.
    pub size: [u16; 2],
    /// Its pivot in texels, from the sprite's own `pivot()` — **not** assumed central,
    /// because a rig part's pivot is wherever the rig hangs it from.
    pub pivot: [u16; 2],
}

/// One call to `stamp_layers_bent_toned`, as data.
#[derive(Clone, Debug)]
pub struct Stamp {
    /// The pivot's position in raster pixels.
    pub anchor: [f32; 2],
    /// The tile's `+x` in raster space.
    pub heading: [f32; 2],
    /// Up to [`MAX_POSES`] poses; more than four *frames* between them is what
    /// [`Stamp::instance`] reports as dropped. A fixed array rather than a `Vec`: a
    /// ring frame builds a few thousand stamps and an allocation each would be the
    /// adapter's largest single cost.
    pub layers: [PoseRef; MAX_POSES],
    /// `(amplitude, base, root, length)`.
    pub bend: [f32; 4],
    pub mask: StampMask,
    pub tone: Option<StampTone>,
    pub opacity: f32,
    /// A per-stamp multiplier on the world scale, as `stamp_layers`'s own `scale`
    /// argument. 1 everywhere but a juvenile body's 0.7 and a hunter rig's own scale.
    pub scale: f32,
    /// When set, the stamp draws this one scratch frame and [`Stamp::layers`] is ignored.
    pub scratch: Option<ScratchFrame>,
}

impl PoseRef {
    /// A slot that carries nothing.
    pub const NONE: PoseRef = PoseRef { a: 0, b: 0, mix: 0.0, weight: 0.0 };
}

/// The most poses one stamp can carry: `art_present` never composites more than three
/// (`stage_layers`'s fruit blend is two, an authored growth step names three, a body's
/// cross-fade at most three).
pub const MAX_POSES: usize = 3;

impl Default for Stamp {
    fn default() -> Stamp {
        Stamp {
            anchor: [0.0, 0.0],
            heading: [1.0, 0.0],
            layers: [PoseRef::NONE; MAX_POSES],
            bend: [0.0; 4],
            mask: StampMask::None,
            tone: None,
            opacity: 1.0,
            scale: 1.0,
            scratch: None,
        }
    }
}

impl Stamp {
    /// The instance this stamp draws as, and **how many frames had to be dropped** to
    /// fit four slots.
    ///
    /// Dropping is by weight, lightest first, and the survivors are renormalised so the
    /// composite still sums to 1 — a stamp that lost a layer is dimmer nowhere and
    /// merely a little less faded. It can only happen to a body whose state changed
    /// twice inside `BODY_FADE_SECONDS`; the driver counts the occurrences so the
    /// report can say how often rather than guess.
    pub fn instance(&self, atlas: &Atlas) -> (SpriteInstance, usize) {
        if let Some(frame) = self.scratch {
            let mut instance = SpriteInstance {
                anchor: self.anchor,
                heading: self.heading,
                frames: [frame.origin, [0, 0], [0, 0], [0, 0]],
                size: frame.size,
                pivot: frame.pivot,
                weights: [1.0, 0.0, 0.0, 0.0],
                bend: self.bend,
                opacity: self.opacity,
                scale: self.scale,
                source: SOURCE_SCRATCH,
                // A rig part is rasterised fresh every frame into the scratch page, so
                // there is no measured box for it: the quad is the whole tile, exactly
                // as it was before the box existed.
                bbox: [0.0, 0.0, f32::from(frame.size[0]), f32::from(frame.size[1])],
                ..SpriteInstance::empty()
            };
            self.apply(&mut instance);
            return (instance, 0);
        }
        // Flatten every pose into (frame, weight), then keep the four heaviest. A pose
        // sitting exactly on a frame contributes one entry, not two, which is why an
        // idle plant and a fruiting one both fit.
        let mut flat = [(0u32, 0f32); MAX_POSES * 2];
        let mut n = 0usize;
        let mut push = |frame: u32, weight: f32| {
            flat[n] = (frame, weight);
            n += 1;
        };
        for pose in &self.layers {
            if !(pose.weight.is_finite() && pose.weight > 0.0) {
                continue;
            }
            let mix = if pose.mix.is_finite() { pose.mix.clamp(0.0, 1.0) } else { 0.0 };
            if mix <= 0.0 {
                push(pose.a, pose.weight);
            } else if mix >= 1.0 {
                push(pose.b, pose.weight);
            } else {
                push(pose.a, pose.weight * (1.0 - mix));
                push(pose.b, pose.weight * mix);
            }
        }
        drop(push);
        let flat = &mut flat[..n];
        // Merge repeats first: `stage_layers` hands the same pose twice when a plant is
        // not in fruit, and a fade between two clips of one state is one frame.
        flat.sort_by_key(|(frame, _)| *frame);
        let mut unique = 0usize;
        for i in 0..flat.len() {
            if unique > 0 && flat[unique - 1].0 == flat[i].0 {
                flat[unique - 1].1 += flat[i].1;
            } else {
                flat[unique] = flat[i];
                unique += 1;
            }
        }
        let flat = &mut flat[..unique];
        let total: f32 = flat.iter().map(|(_, w)| *w).sum();
        flat.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let dropped = flat.len().saturating_sub(4);
        let flat = &flat[..flat.len().min(4)];
        let kept: f32 = flat.iter().map(|(_, w)| *w).sum();
        let renormalise = if kept > 0.0 && total > 0.0 { total / kept } else { 1.0 };

        let mut instance = SpriteInstance {
            anchor: self.anchor,
            heading: self.heading,
            bend: self.bend,
            opacity: self.opacity,
            scale: self.scale,
            source: SOURCE_ATLAS,
            ..SpriteInstance::empty()
        };
        for (slot, (frame, weight)) in flat.iter().enumerate() {
            let rect = atlas.rect(*frame);
            if slot == 0 {
                instance.size = [rect.w, rect.h];
                instance.pivot = [rect.w / 2, rect.h / 2];
            }
            instance.frames[slot] = [rect.x, rect.y];
            instance.weights[slot] = weight * renormalise;
            // The quad is built around the union of the *kept* frames' opaque boxes. A
            // dropped frame is a frame with no weight, so it cannot paint and does not
            // widen the quad.
            instance.cover(rect.bbox);
        }
        self.apply(&mut instance);
        (instance, dropped)
    }

    /// The mask and tone, which do not depend on where the frames came from.
    fn apply(&self, instance: &mut SpriteInstance) {
        match self.mask {
            StampMask::None => {}
            StampMask::Axial { reveal } => {
                instance.mask_floor = NO_MASK_FLOOR;
                instance.mask_reveal = reveal;
            }
            StampMask::Strip { floor, reveal } => {
                instance.mask_floor = floor;
                instance.mask_reveal = reveal;
            }
            StampMask::Radial { reveal } => {
                instance.mask_flags = MASK_RADIAL;
                instance.mask_reveal = reveal;
            }
        }
        if let Some(tone) = self.tone {
            instance.tone_colour = tone.colour;
            instance.tone_mix = tone.mix;
            instance.shade_floor = tone.shade_floor;
            instance.shade_reference = tone.shade_reference;
        }
    }
}

/// The unmasked sentinels, for a caller building an instance by hand.
pub const NO_MASK: (f32, f32, f32) = (NO_MASK_FLOOR, NO_MASK_REVEAL, crate::scene::MASK_AXIAL);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::MASK_AXIAL;
    use std::path::Path;

    fn atlas() -> Atlas {
        Atlas::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")).unwrap()
    }

    fn poses(layers: &[(u32, u32, f32, f32)]) -> Stamp {
        let mut stamp = Stamp::default();
        for (slot, &(a, b, mix, weight)) in stamp.layers.iter_mut().zip(layers) {
            *slot = PoseRef { a, b, mix, weight };
        }
        stamp
    }

    #[test]
    fn a_pose_on_a_frame_costs_one_slot_and_a_blend_costs_two() {
        let atlas = atlas();
        let (still, dropped) = poses(&[(4, 5, 0.0, 1.0)]).instance(&atlas);
        assert_eq!((still.used(), dropped), (1, 0));
        let (blend, dropped) = poses(&[(4, 5, 0.25, 1.0)]).instance(&atlas);
        assert_eq!((blend.used(), dropped), (2, 0));
        assert!((blend.weights[0] - 0.75).abs() < 1e-6, "{:?}", blend.weights);
        assert!((blend.weights[1] - 0.25).abs() < 1e-6, "{:?}", blend.weights);
    }

    #[test]
    fn two_blended_poses_fill_four_slots_exactly_and_drop_nothing() {
        // The fruiting plant and the middle of an authored growth step: the two cases
        // the four slots exist for.
        let atlas = atlas();
        let (instance, dropped) = poses(&[(4, 5, 0.3, 0.6), (40, 41, 0.7, 0.4)]).instance(&atlas);
        assert_eq!((instance.used(), dropped), (4, 0));
        let sum: f32 = instance.weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "{sum}");
    }

    #[test]
    fn a_third_blended_pose_drops_the_lightest_frames_and_renormalises() {
        let atlas = atlas();
        let (instance, dropped) =
            poses(&[(4, 5, 0.5, 0.5), (40, 41, 0.5, 0.4), (80, 81, 0.5, 0.1)]).instance(&atlas);
        assert_eq!(dropped, 2, "the third pose's two frames are the lightest");
        assert_eq!(instance.used(), 4);
        let sum: f32 = instance.weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "a dropped layer must not dim the stamp: {sum}");
    }

    #[test]
    fn the_same_frame_twice_is_merged_rather_than_spending_two_slots() {
        // `stage_layers` hands the same pose twice when a plant carries no fruit.
        let atlas = atlas();
        let (instance, dropped) = poses(&[(4, 4, 0.0, 0.6), (4, 4, 0.0, 0.4)]).instance(&atlas);
        assert_eq!((instance.used(), dropped), (1, 0));
        assert!((instance.weights[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_scratch_stamp_names_its_own_page_and_keeps_its_pivot() {
        let atlas = atlas();
        let (instance, dropped) = Stamp {
            scratch: Some(ScratchFrame { origin: [17, 3], size: [11, 9], pivot: [2, 7] }),
            scale: 0.75,
            ..Default::default()
        }
        .instance(&atlas);
        assert_eq!(dropped, 0);
        assert_eq!(instance.source, SOURCE_SCRATCH);
        assert_eq!((instance.frames[0], instance.size, instance.pivot), ([17, 3], [11, 9], [2, 7]));
        assert_eq!(instance.weights, [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(instance.scale, 0.75);
    }

    #[test]
    fn each_mask_lands_in_the_fields_the_shader_reads() {
        let atlas = atlas();
        let with = |mask| Stamp { mask, ..poses(&[(4, 4, 0.0, 1.0)]) }.instance(&atlas).0;
        let none = with(StampMask::None);
        assert_eq!((none.mask_floor, none.mask_reveal, none.mask_flags), NO_MASK);
        assert_eq!(none.source, SOURCE_ATLAS);
        let axial = with(StampMask::Axial { reveal: 6.0 });
        assert_eq!((axial.mask_floor, axial.mask_reveal, axial.mask_flags), (NO_MASK_FLOOR, 6.0, MASK_AXIAL));
        let strip = with(StampMask::Strip { floor: 11.0, reveal: 15.0 });
        assert_eq!((strip.mask_floor, strip.mask_reveal), (11.0, 15.0));
        let radial = with(StampMask::Radial { reveal: 4.5 });
        assert_eq!((radial.mask_reveal, radial.mask_flags), (4.5, MASK_RADIAL));
    }

    #[test]
    fn the_measured_extent_is_inside_the_tile_and_bigger_than_a_bare_sprout() {
        let atlas = atlas();
        let stage0 = atlas.plant("lanternstalk", crate::PlantClip::Stage(0)).unwrap();
        let stage2 = atlas.plant("lanternstalk", crate::PlantClip::Stage(2)).unwrap();
        let e0 = atlas.rect(stage0.first).extent;
        let e2 = atlas.rect(stage2.first).extent;
        assert!(e0 > 0.0 && e2 > e0, "a grown plant reaches further than a sprout: {e0} vs {e2}");
        // 16 x 16 with the pivot at the centre: the furthest corner is 8·√2 + √2/2.
        assert!(e2 < 12.0, "extent {e2} is outside the tile");
    }
}
