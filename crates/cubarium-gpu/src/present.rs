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

/// How the world raster lands in a **desktop window**: as large as fits, aspect kept,
/// centred, with black bars on the other axis only — at any scale, whole or not. The
/// present pass draws each window pixel as the area average of the raster it covers (a
/// box filter in linear light, `present.frag`): a whole upscale is exactly the nearest
/// one, any other upscale blends one pixel at each texel edge, and a downscale is
/// supersampled. It is a development window, so it has no quarter-turn and no rule about
/// what a person may resize it to.
///
/// The present pass maps a window pixel to a raster pixel through its push constants, so
/// this is a second way of building them next to [`PresentTransform::push`]. The two
/// differ in one respect the panel must never see: `offset.w` is 1 here, which tells
/// `present.frag` to filter and to paint a pixel black when its footprint misses the
/// raster. The panel's transform leaves it 0 and its picture is exactly what it was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowFit {
    /// Raster pixels per window pixel: below 1 for an upscale, above 1 for a downscale.
    /// The same on both axes; the limiting axis fills the window exactly.
    pub step: f32,
    /// Where the raster's top-left corner lands, in window pixels (fractional on the
    /// barred axis, so the picture is centred exactly).
    pub corner: (f32, f32),
    /// The window pixel the picture starts on, and its size in window pixels: the
    /// pixels any of whose footprint is raster. Everything else is the black bars.
    pub origin: (u32, u32),
    pub size: (u32, u32),
    /// True when the swapchain is a UNORM format and the shader must encode sRGB itself.
    pub encode_srgb: bool,
}

impl WindowFit {
    /// The fit of a `raster` into a `window`, both in pixels. A window of zero area fits
    /// nothing and the caller does not present to it.
    pub fn fit(raster: (u32, u32), window: (u32, u32), encode_srgb: bool) -> WindowFit {
        let (rw, rh) = (f64::from(raster.0.max(1)), f64::from(raster.1.max(1)));
        let (ww, wh) = (f64::from(window.0.max(1)), f64::from(window.1.max(1)));
        // The smallest step that fits both axes: the long side fills the window.
        let s = (rw / ww).max(rh / wh);
        let corner = |r: f64, w: f64| 0.5 * (w - r / s);
        let (cx, cy) = (corner(rw, ww), corner(rh, wh));
        // Pixels whose footprint overlaps the raster at all.
        let run = |c: f64, r: f64, w: f64| {
            let first = c.floor().max(0.0);
            let end = (c + r / s).ceil().min(w);
            (first as u32, (end - first).max(1.0) as u32)
        };
        let (ox, sx) = run(cx, rw, ww);
        let (oy, sy) = run(cy, rh, wh);
        WindowFit {
            step: s as f32,
            corner: (cx as f32, cy as f32),
            origin: (ox, oy),
            size: (sx, sy),
            encode_srgb,
        }
    }

    /// The present pass's push constants: two columns of the window-to-raster matrix,
    /// the offset, the sRGB flag, and the window flag.
    ///
    /// The shader floors `gl_FragCoord` to the pixel's corner; the offset puts the sample
    /// at the pixel's **centre**, `(n + 0.5 - corner) · step` raster pixels in, and the
    /// shader averages the raster over `step` around it.
    pub fn push(&self) -> [f32; 8] {
        let s = self.step;
        let (cx, cy) = self.corner;
        [
            s,
            0.0,
            0.0,
            s,
            (0.5 - cx) * s,
            (0.5 - cy) * s,
            if self.encode_srgb { 1.0 } else { 0.0 },
            1.0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `present.frag`'s own arithmetic along one axis: the raster span window pixel `p`'s
    /// footprint covers, before the shader clamps it to the raster.
    fn footprint(push: &[f32; 8], axis: usize, p: u32) -> (f64, f64) {
        let (step, offset) = (f64::from(push[axis * 3]), f64::from(push[4 + axis]));
        let centre = step * f64::from(p) + offset;
        (centre - 0.5 * step, centre + 0.5 * step)
    }

    /// Whether the shader draws pixel `p` (its footprint overlaps the raster) or a bar.
    fn shows(push: &[f32; 8], axis: usize, p: u32, raster: u32) -> bool {
        let (lo, hi) = footprint(push, axis, p);
        hi > 0.0 && lo < f64::from(raster)
    }

    /// Fits up and down, whole and not: the limiting axis fills the window edge to edge,
    /// the other is centred with equal bars, the footprints tile the raster without gap or
    /// overlap, and the picture's reported run is exactly the pixels that show.
    #[test]
    fn the_raster_fills_the_window_on_its_limiting_axis_with_its_aspect_kept() {
        let cases = [
            ((2304, 1392), (2560, 1600)),
            ((3584, 2176), (2560, 1600)),
            ((3584, 2176), (3840, 2160)),
            ((2048, 1264), (1920, 1200)),
            ((320, 180), (1000, 600)),
            ((3328, 2048), (640, 400)),
        ];
        for (raster, window) in cases {
            let fit = WindowFit::fit(raster, window, false);
            let push = fit.push();
            let r = [raster.0, raster.1];
            let w = [window.0, window.1];
            let fills = |a: usize| {
                let first = footprint(&push, a, 0).0;
                let last = footprint(&push, a, w[a] - 1).1;
                first.abs() < 1e-3 && (last - f64::from(r[a])).abs() < 1e-2
            };
            assert!(fills(0) || fills(1), "{raster:?} in {window:?}: no axis fills");
            for a in 0..2 {
                let shown: Vec<u32> = (0..w[a]).filter(|&p| shows(&push, a, p, r[a])).collect();
                let (o, n) = ([fit.origin.0, fit.origin.1][a], [fit.size.0, fit.size.1][a]);
                assert_eq!(shown.first().copied(), Some(o), "{raster:?} in {window:?} axis {a}");
                assert_eq!(shown.len() as u32, n, "{raster:?} in {window:?} axis {a}");
                let (before, after) = (o, w[a] - o - n);
                assert!(before.abs_diff(after) <= 1, "axis {a}: bars {before} and {after}");
                for p in 1..w[a] {
                    let gap = footprint(&push, a, p).0 - footprint(&push, a, p - 1).1;
                    assert!(gap.abs() < 1e-3, "axis {a}: footprints {p} and {} part", p - 1);
                }
            }
            // The same step on both axes: the aspect is the raster's.
            assert_eq!(push[0], push[3]);
        }
    }

    /// A window a whole multiple `k` of the raster: every pixel's footprint lies inside
    /// one texel, so the area average is exactly the nearest upscale, `k` pixels a texel.
    #[test]
    fn a_whole_upscale_reads_one_texel_a_pixel() {
        for (raster, k) in [((256u32, 128u32), 3u32), ((320, 180), 1), ((4, 2), 7)] {
            let window = (raster.0 * k, raster.1 * k);
            let fit = WindowFit::fit(raster, window, false);
            let push = fit.push();
            for p in 0..window.0 {
                let (lo, hi) = footprint(&push, 0, p);
                let texel = (p / k) as f64;
                assert!(lo >= texel - 1e-4 && hi <= texel + 1.0 + 1e-4, "k = {k}, p = {p}");
            }
        }
    }

    /// The filter and the bars are the window's alone: the panel's push constants leave
    /// the flag at 0, so `present.frag` runs for it exactly as before.
    #[test]
    fn only_the_window_asks_for_the_window_filter() {
        let panel = PresentTransform {
            factor: 6,
            quarter_turns: 1,
            encode_srgb: false,
        };
        assert_eq!(panel.push((1080, 1920))[7], 0.0);
        assert_eq!(WindowFit::fit((320, 180), (640, 360), false).push()[7], 1.0);
    }
}
