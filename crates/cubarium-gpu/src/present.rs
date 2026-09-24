//! The last pass, shared by every renderer in this crate, and the contract a target
//! draws through.
//!
//! There are two renderers now — the ring's [`Renderer`](crate::render::Renderer) over
//! its sprite [`Scene`](crate::scene::Scene), and the voxel strip's
//! [`VoxelRenderer`](crate::voxel::VoxelRenderer) over one uploaded voxel texture — and
//! exactly one way to put either one's world raster on a target: a nearest ×k upscale
//! plus a quarter turn, `present.frag`. [`PresentPass`] is that pass as a value a
//! renderer owns, and [`FrameSource`] is what a target needs from a renderer so that
//! `Headless`, the shim and the desktop window are written once rather than once per
//! renderer.
//!
//! The trait's associated [`FrameSource::Frame`] is what one frame *is* for that
//! renderer: `&Scene` for the ring, `()` for the voxel strip, whose frame arrived with
//! the tick's upload instead of with a draw call.

use anyhow::Result;
use ash::vk;
use std::collections::HashMap;

use crate::render::{PresentTransform, TargetImage};
use crate::vk::Gpu;

const FULLSCREEN_VERT: &[u8] = include_bytes!("../shaders/fullscreen.vert.spv");
const PRESENT_FRAG: &[u8] = include_bytes!("../shaders/present.frag.spv");

/// Where the present pass draws: the attachment, its extent and format, the layout it
/// must be left in, and the transform from that surface back to the world raster.
pub type TargetSlot<'a> = (
    &'a TargetImage,
    (u32, u32),
    vk::Format,
    vk::ImageLayout,
    PresentTransform,
);

/// The present pass: one descriptor set holding the world raster, one pipeline layout,
/// and a render pass and pipeline per target attachment format.
///
/// A target's framebuffers must be built against the render pass [`PresentPass::pass`]
/// hands back for their own format and final layout, which is why the map is keyed on
/// both and built on first use: the shim's dma-buf views are `B8G8R8A8_SRGB` left in
/// `GENERAL`, and a swapchain's are something else again.
pub struct PresentPass {
    set_layout: vk::DescriptorSetLayout,
    set: vk::DescriptorSet,
    pool: vk::DescriptorPool,
    pipeline_layout: vk::PipelineLayout,
    passes: HashMap<(i32, i32), (vk::RenderPass, vk::Pipeline)>,
}

impl PresentPass {
    /// Build the pass for one renderer's world raster. `sampler` is the renderer's own
    /// nearest sampler; the upscale is `texelFetch`, so nothing but the address modes
    /// could ever come from it.
    pub fn new(gpu: &Gpu, raster_view: vk::ImageView, sampler: vk::Sampler) -> Result<PresentPass> {
        let d = &gpu.device;
        let bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        let set_layout = unsafe {
            d.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }?;
        let sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)];
        let pool = unsafe {
            d.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&sizes),
                None,
            )
        }?;
        let layouts = [set_layout];
        let set = unsafe {
            d.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            )
        }?[0];
        let info = [vk::DescriptorImageInfo::default()
            .sampler(sampler)
            .image_view(raster_view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        unsafe {
            d.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&info)],
                &[],
            )
        };
        let ranges = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .size(32)];
        let pipeline_layout = unsafe {
            d.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&layouts)
                    .push_constant_ranges(&ranges),
                None,
            )
        }?;
        Ok(PresentPass {
            set_layout,
            set,
            pool,
            pipeline_layout,
            passes: HashMap::new(),
        })
    }

    /// The render pass a target's framebuffers must be built against.
    pub fn pass(
        &mut self,
        gpu: &Gpu,
        format: vk::Format,
        final_layout: vk::ImageLayout,
    ) -> Result<vk::RenderPass> {
        Ok(self.entry(gpu, format, final_layout)?.0)
    }

    /// The render pass and pipeline for this attachment, created on first use.
    pub fn entry(
        &mut self,
        gpu: &Gpu,
        format: vk::Format,
        final_layout: vk::ImageLayout,
    ) -> Result<(vk::RenderPass, vk::Pipeline)> {
        let key = (format.as_raw(), final_layout.as_raw());
        if let Some(entry) = self.passes.get(&key) {
            return Ok(*entry);
        }
        let d = &gpu.device;
        let pass =
            crate::render::colour_pass(d, format, vk::AttachmentLoadOp::DONT_CARE, final_layout)?;
        let vs = gpu.shader(FULLSCREEN_VERT)?;
        let fs = gpu.shader(PRESENT_FRAG)?;
        let pipeline =
            crate::render::fullscreen_pipeline(d, pass, self.pipeline_layout, vs, fs, false)?;
        unsafe {
            d.destroy_shader_module(vs, None);
            d.destroy_shader_module(fs, None);
        }
        self.passes.insert(key, (pass, pipeline));
        Ok((pass, pipeline))
    }

    /// Record the pass into `cb`: one full-screen triangle over the world raster.
    ///
    /// # Safety
    /// `cb` must be recording and outside a render pass.
    pub unsafe fn record(
        &self,
        d: &ash::Device,
        cb: vk::CommandBuffer,
        image: &TargetImage,
        extent: (u32, u32),
        xform: PresentTransform,
        pass: vk::RenderPass,
        pipeline: vk::Pipeline,
    ) {
        unsafe { self.record_push(d, cb, image, extent, xform.push(extent), pass, pipeline) }
    }

    /// [`PresentPass::record`] with the push constants given outright: how a desktop
    /// window hands over its [`WindowFit`], which is not a quarter-turn and a factor.
    ///
    /// # Safety
    /// `cb` must be recording and outside a render pass.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn record_push(
        &self,
        d: &ash::Device,
        cb: vk::CommandBuffer,
        image: &TargetImage,
        extent: (u32, u32),
        push: [f32; 8],
        pass: vk::RenderPass,
        pipeline: vk::Pipeline,
    ) {
        unsafe {
            crate::render::begin(d, cb, pass, image.framebuffer, extent.0, extent.1);
            d.cmd_bind_descriptor_sets(
                cb,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.set],
                &[],
            );
            d.cmd_push_constants(
                cb,
                self.pipeline_layout,
                vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::cast_slice(&push),
            );
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
            d.cmd_draw(cb, 3, 1, 0, 0);
            d.cmd_end_render_pass(cb);
        }
    }

    /// Release everything. The device must be idle.
    pub fn destroy(&self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            for (pass, pipeline) in self.passes.values() {
                d.destroy_pipeline(*pipeline, None);
                d.destroy_render_pass(*pass, None);
            }
            d.destroy_pipeline_layout(self.pipeline_layout, None);
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
        }
    }
}

/// What a target needs from a renderer: a raster of a known size, a command pool to
/// allocate its own buffers from, the present render pass for its attachment format, and
/// one call that records a whole frame.
///
/// This is the seam that lets [`Headless`](crate::target::Headless), the shim client and
/// the desktop window serve both renderers. A target names the renderer only through this
/// trait, so nothing in `target/**` knows whether it is showing a ring of sprites or a
/// voxel strip.
pub trait FrameSource {
    /// What one frame is for this renderer.
    type Frame<'a>;

    /// The world raster's size in pixels.
    fn raster_size(&self) -> (u32, u32);

    /// The pool a target allocates its command buffers from.
    fn command_pool(&self) -> vk::CommandPool;

    /// The present render pass for an attachment format and final layout.
    fn present_pass(
        &mut self,
        gpu: &Gpu,
        format: vk::Format,
        final_layout: vk::ImageLayout,
    ) -> Result<vk::RenderPass>;

    /// Record the whole frame — the renderer's own passes into the world raster, then,
    /// if a target is given, the present pass onto it. The command buffer must not be in
    /// flight.
    fn record_frame(
        &mut self,
        gpu: &Gpu,
        cb: vk::CommandBuffer,
        frame: Self::Frame<'_>,
        target: Option<TargetSlot<'_>>,
    ) -> Result<()>;

    /// A recorded frame has completed on the GPU — the oldest outstanding one, since
    /// frames retire in the order they were submitted. Anything the recording named, such
    /// as a staging buffer it uploads from, may be reused. A target that waits for its own
    /// fence calls this itself; a target that presents on another thread calls it when
    /// that thread reports the frame done.
    fn frame_retired(&mut self) {}

    /// The newest recorded frame will never be submitted: a fresher one displaced it
    /// before the presenter took it. Whatever the recording claimed is owed again.
    fn frame_discarded(&mut self) {}

    /// Recorded frames that have not retired: what the recorder has handed the GPU and
    /// not been told about yet. A target that keeps frames in flight refuses to record
    /// another once this reaches the renderer's ring width, since a further frame would
    /// have no per-frame resources of its own to write.
    fn frames_in_flight(&self) -> usize {
        0
    }

    /// How many frames this renderer has per-frame resources for — staging buffers,
    /// uniform blocks, timestamp sets. A target must not record past it.
    fn frame_capacity(&self) -> usize {
        usize::MAX
    }

    /// What this renderer would put in a frame recorded now, as a number that changes
    /// whenever the picture would. A frame already recorded and still waiting at this
    /// version would come out the same, so it is left alone rather than recorded again —
    /// which matters on a main thread that also has a world to step. `None` from a
    /// renderer that cannot say, and then nothing is ever held.
    fn content_version(&self) -> Option<u64> {
        None
    }

    /// Whether the frame just recorded drew the world again, or only put a raster it
    /// already held onto a new target. A target that reports what a frame cost keeps the
    /// two apart, because they cost very different amounts.
    fn redrew_last(&self) -> bool {
        true
    }

    /// The world raster's view, for a target that runs the present pass itself — on
    /// another thread, onto an image it acquires there — rather than asking
    /// [`FrameSource::record_frame`] to. The raster is left in `SHADER_READ_ONLY_OPTIMAL`
    /// by every frame. `None` from a renderer that does not lend it; such a target then
    /// cannot be used with it.
    fn raster_view(&self) -> Option<vk::ImageView> {
        None
    }

    /// Milliseconds the last frame's timestamps saw, or `NaN` if they are not ready.
    fn gpu_ms(&self, gpu: &Gpu) -> f64;

    /// The world raster as `w · h · 4` bytes, already sRGB-encoded.
    fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>>;
}

/// How the world raster lands in a **desktop window**: the largest whole upscale that
/// fits, centred, with black bars — or, in a window smaller than the raster, a nearest
/// downscale that fits, centred the same way. It is a development window, so it has no
/// quarter-turn and no rule about what a person may resize it to.
///
/// The present pass maps a window pixel to a raster pixel through its push constants, so
/// this is a second way of building them next to [`PresentTransform::push`]. The two
/// differ in one respect the panel must never see: `offset.w` is 1 here, which tells
/// `present.frag` to paint a pixel black when it maps outside the raster. The panel's
/// transform leaves it 0 and its picture is exactly what it was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowFit {
    /// The whole upscale `k`, or 0 when the window is too small for even 1x and the
    /// raster is downscaled.
    pub factor: u32,
    /// Raster pixels per window pixel: `1 / k` for an upscale, more than 1 for a
    /// downscale.
    pub step: f32,
    /// The window pixel the raster's top-left corner lands on.
    pub origin: (u32, u32),
    /// The picture's size in window pixels; everything else is the black bars.
    pub size: (u32, u32),
    /// True when the swapchain is a UNORM format and the shader must encode sRGB itself.
    pub encode_srgb: bool,
}

impl WindowFit {
    /// The fit of a `raster` into a `window`, both in pixels. A window of zero area fits
    /// nothing and the caller does not present to it.
    pub fn fit(raster: (u32, u32), window: (u32, u32), encode_srgb: bool) -> WindowFit {
        let (rw, rh) = (raster.0.max(1), raster.1.max(1));
        let (ww, wh) = (window.0.max(1), window.1.max(1));
        let k = (ww / rw).min(wh / rh);
        let (step, size) = if k >= 1 {
            (1.0 / k as f32, (rw * k, rh * k))
        } else {
            // The smallest step that fits both axes, so the aspect is kept and the long
            // side fills the window.
            let s = (f64::from(rw) / f64::from(ww)).max(f64::from(rh) / f64::from(wh));
            let side = |r: u32, w: u32| ((f64::from(r) / s).round() as u32).clamp(1, w);
            (s as f32, (side(rw, ww), side(rh, wh)))
        };
        WindowFit {
            factor: k,
            step,
            origin: ((ww - size.0) / 2, (wh - size.1) / 2),
            size,
            encode_srgb,
        }
    }

    /// The present pass's push constants: two columns of the window-to-raster matrix,
    /// the offset, the sRGB flag, and the letterbox flag.
    ///
    /// The shader floors `gl_FragCoord` to the pixel's corner; the offset puts the sample
    /// back at the pixel's **centre** before it is scaled. At a whole upscale `k` that
    /// centre is `(n + 0.5) / k` raster pixels in, which is never a whole number, so a
    /// rounding error in `1 / k` can never move a pixel onto the neighbouring texel.
    pub fn push(&self) -> [f32; 8] {
        let s = self.step;
        let (ox, oy) = (self.origin.0 as f32, self.origin.1 as f32);
        [
            s,
            0.0,
            0.0,
            s,
            (0.5 - ox) * s,
            (0.5 - oy) * s,
            if self.encode_srgb { 1.0 } else { 0.0 },
            1.0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `present.frag`'s own arithmetic along one axis: the raster pixel window pixel `p`
    /// reads, or `None` where the letterbox paints black.
    fn read(push: &[f32; 8], axis: usize, p: u32, raster: u32) -> Option<u32> {
        let (scale, offset) = (push[axis * 3], push[4 + axis]);
        let src = (scale * p as f32 + offset).floor();
        (src >= 0.0 && src < raster as f32).then_some(src as u32)
    }

    /// The window pixels that show the raster along one axis, and what each one reads.
    fn axis(fit: &WindowFit, axis: usize, window: u32, raster: u32) -> Vec<(u32, u32)> {
        let push = fit.push();
        (0..window)
            .filter_map(|p| read(&push, axis, p, raster).map(|s| (p, s)))
            .collect()
    }

    /// A raster that fits is drawn at the largest whole upscale, centred: every raster
    /// pixel becomes exactly `k` window pixels in each direction, in order, and the bars
    /// either side read nothing.
    #[test]
    fn a_raster_that_fits_is_upscaled_by_the_largest_whole_factor_and_centred() {
        let fit = WindowFit::fit((320, 180), (1000, 600), false);
        assert_eq!(fit.factor, 3);
        assert_eq!(fit.size, (960, 540));
        assert_eq!(fit.origin, (20, 30));
        for (a, window, raster, origin) in [(0, 1000, 320, 20), (1, 600, 180, 30)] {
            let shown = axis(&fit, a, window, raster);
            assert_eq!(shown.len() as u32, raster * 3, "axis {a}");
            for (i, (p, s)) in shown.iter().enumerate() {
                assert_eq!(*p, origin + i as u32, "axis {a}: one unbroken run");
                assert_eq!(*s, i as u32 / 3, "axis {a}: pixel {p} reads the wrong texel");
            }
        }
    }

    /// The same at the factor the desktop really uses, 1x and at a large `k`, where a
    /// rounding error in `1 / k` would first show as a doubled or a missing column.
    #[test]
    fn every_whole_factor_gives_every_texel_exactly_k_pixels() {
        for (raster, window) in [((3328, 2048), (3328, 2048)), ((256, 128), (3840, 2160))] {
            let fit = WindowFit::fit(raster, window, false);
            let k = fit.factor;
            assert!(k >= 1);
            let shown = axis(&fit, 0, window.0, raster.0);
            assert_eq!(shown.len() as u32, raster.0 * k);
            for (i, (_, s)) in shown.iter().enumerate() {
                assert_eq!(*s, i as u32 / k, "k = {k}");
            }
        }
    }

    /// A window smaller than the raster downscales nearest, keeps the aspect, and fills
    /// the long side; the picture's reported size is exactly the run of pixels that read
    /// the raster, and every read is inside it.
    #[test]
    fn a_window_smaller_than_the_raster_downscales_nearest_and_keeps_the_aspect() {
        let (raster, window) = ((3328, 2048), (640, 400));
        let fit = WindowFit::fit(raster, window, true);
        assert_eq!(fit.factor, 0);
        assert!((fit.step - 5.2).abs() < 1e-6, "{}", fit.step);
        assert_eq!(fit.size.0, 640, "the long side fills the window");
        for (a, w, r) in [(0, window.0, raster.0), (1, window.1, raster.1)] {
            let shown = axis(&fit, a, w, r);
            let first = shown.first().unwrap().0;
            assert_eq!(first, [fit.origin.0, fit.origin.1][a], "axis {a}");
            assert_eq!(shown.len() as u32, [fit.size.0, fit.size.1][a], "axis {a}");
            assert!(shown.windows(2).all(|p| p[1].0 == p[0].0 + 1 && p[1].1 > p[0].1));
        }
        assert_eq!(fit.push()[6], 1.0, "a UNORM swapchain encodes in the shader");
    }

    /// The letterbox is the window's alone: the panel's push constants leave the flag at
    /// 0, so `present.frag` runs for it exactly as before.
    #[test]
    fn only_the_window_asks_for_the_letterbox() {
        let panel = PresentTransform {
            factor: 6,
            quarter_turns: 1,
            encode_srgb: false,
        };
        assert_eq!(panel.push((1080, 1920))[7], 0.0);
        assert_eq!(WindowFit::fit((320, 180), (640, 360), false).push()[7], 1.0);
    }
}
