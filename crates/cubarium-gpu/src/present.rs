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
                &vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes),
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
        Ok(PresentPass { set_layout, set, pool, pipeline_layout, passes: HashMap::new() })
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
        let pass = crate::render::colour_pass(
            d,
            format,
            vk::AttachmentLoadOp::DONT_CARE,
            final_layout,
        )?;
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
                bytemuck::cast_slice(&xform.push(extent)),
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

    /// Milliseconds the last frame's timestamps saw, or `NaN` if they are not ready.
    fn gpu_ms(&self, gpu: &Gpu) -> f64;

    /// The world raster as `w · h · 4` bytes, already sRGB-encoded.
    fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>>;
}
