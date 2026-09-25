//! The renderer: four pipelines over one world raster, and a fifth that puts the
//! raster on a target.
//!
//! # The passes, against `presenter-budget-2026-09-16.md` §2
//!
//! | # | CPU pass | here |
//! |---|---|---|
//! | 1–4 | floor, producer ramp + horizon, detritus flecks, soil ground | **one** full-screen `background.frag` over the cell textures |
//! | 5 | ground cover (the 8-px lattice) | instanced quads, [`Layer::GroundCover`] |
//! | 6 | water with shimmer | full-screen `water.frag`, source-over |
//! | 7, 8 | plants and the soil snag | instanced quads, [`Layer::Plants`] |
//! | 9 | tall columns | instanced quads, [`Layer::Tall`] |
//! | 10 | rain | instanced quads over one white texel, [`Layer::Rain`] |
//! | 11, 12 | bodies, hunters | instanced quads, [`Layer::Bodies`] |
//! | — | `srgb_encode` (W7, 1.37 ms on the board) | the `_SRGB` attachment, i.e. free |
//!
//! **One deviation from the brief, and why.** The brief asked for *one* full-screen
//! background pass including the water. The CPU presenter draws the ground-cover
//! lattice **between** the soil and the water (`art_present/mod.rs`: ground cover at
//! 1069–1101, water at 1103–08), so folding the water into the background would put
//! the lattice on top of the pools instead of under them. Water is therefore its own
//! full-screen pass after the lattice. It costs one extra pass over the *world
//! raster* — 57,600 pixels at S = 1, 230,400 at S = 2 — and never runs at panel
//! resolution, where only `present.frag` does.

use anyhow::{Context, Result, anyhow, bail};
use ash::vk;
use std::collections::HashMap;

use crate::atlas::Atlas;
use crate::palette::SceneUniforms;
use crate::present::{FrameSource, PresentPass, TargetSlot};
use crate::scene::{LAYERS, Layer, RingLayout, Scene, SpriteInstance};
use crate::vk::{Gpu, HostBuffer, barrier};

const FULLSCREEN_VERT: &[u8] = include_bytes!("../shaders/fullscreen.vert.spv");
const BACKGROUND_FRAG: &[u8] = include_bytes!("../shaders/background.frag.spv");
const WATER_FRAG: &[u8] = include_bytes!("../shaders/water.frag.spv");
const SPRITE_VERT: &[u8] = include_bytes!("../shaders/sprite.vert.spv");
const SPRITE_FRAG: &[u8] = include_bytes!("../shaders/sprite.frag.spv");

/// The side of the per-frame scratch page, in texels.
pub const SCRATCH_SIDE: u32 = 256;

/// Timestamp slots per frame: top of pipe, after the uploads, after the world raster
/// pass, bottom of pipe. See [`Renderer::gpu_split`].
const QUERY_SLOTS: u32 = 4;

/// The world raster's format. `_SRGB` is the whole of the encode: the attachment
/// converts on write and blending happens in linear light, which is what the CPU
/// canvas does by hand and then pays 1.37 ms of `powf` for.
pub const RASTER_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;

/// How the world raster lands on a target: an integer upscale and a quarter-turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentTransform {
    /// The integer upscale factor `k`. Every pixel of the raster becomes a `k × k`
    /// block, which is what keeps the pixel grid visible on the panel.
    pub factor: u32,
    /// Quarter turns clockwise. The DP-1 panel is portrait (1080 × 1920) and the ring
    /// is landscape, so the board uses 1 (or 3, if the panel is mounted the other way).
    pub quarter_turns: u32,
    /// True when the target attachment is a UNORM format and the shader must encode
    /// sRGB itself.
    pub encode_srgb: bool,
}

impl PresentTransform {
    /// The largest integer upscale of `raster` that fits `target` at this rotation,
    /// or `None` if even 1× does not fit.
    pub fn fit(
        raster: (u32, u32),
        target: (u32, u32),
        quarter_turns: u32,
        encode_srgb: bool,
    ) -> Option<PresentTransform> {
        let (rw, rh) = if quarter_turns % 2 == 0 {
            raster
        } else {
            (raster.1, raster.0)
        };
        let factor = (target.0 / rw).min(target.1 / rh);
        (factor >= 1).then_some(PresentTransform {
            factor,
            quarter_turns,
            encode_srgb,
        })
    }

    /// The push constants: two columns of the panel→raster matrix, then its offset.
    pub(crate) fn push(&self, target: (u32, u32)) -> [f32; 8] {
        let k = 1.0 / self.factor as f32;
        let (pw, ph) = (target.0 as f32, target.1 as f32);
        // col0 = (m00, m10), col1 = (m01, m11): `M · p = col0·p.x + col1·p.y`.
        let (col0, col1, off) = match self.quarter_turns % 4 {
            0 => ([k, 0.0], [0.0, k], [0.0, 0.0]),
            1 => ([0.0, -k], [k, 0.0], [0.0, (pw - 1.0) * k]),
            2 => ([-k, 0.0], [0.0, -k], [(pw - 1.0) * k, (ph - 1.0) * k]),
            _ => ([0.0, k], [-k, 0.0], [(ph - 1.0) * k, 0.0]),
        };
        [
            col0[0],
            col0[1],
            col1[0],
            col1[1],
            off[0],
            off[1],
            if self.encode_srgb { 1.0 } else { 0.0 },
            0.0,
        ]
    }
}

/// Where a frame's sprite fill goes: what the quads cover against what the art in them
/// can actually paint. All three are **raster pixels summed over every instance**, so
/// they count overdraw rather than area of the image.
///
/// Measured only when [`Renderer::fill_profile`] is set (`--gpu-fill-profile`): it is a
/// walk over the frame's instances with a hash lookup each, which is the adapter's own
/// scarce resource and has no business on the shipped path.
#[derive(Clone, Copy, Debug, Default)]
pub struct FillProfile {
    /// What the quads this renderer builds rasterise.
    pub quad_px: f64,
    /// What a quad around the **whole tile** would have rasterised — the fill before
    /// the opaque box, and the ratio worth quoting.
    pub tile_px: f64,
    /// What the instances' **painted** source texels cover: `α > 0` texels only, at the
    /// same scale. The floor no quad can go below.
    pub opaque_px: f64,
    /// Instances counted.
    pub instances: usize,
    /// [`FillProfile::quad_px`] split by [`Layer`], in `LAYERS` order: where the fill
    /// actually goes, which is the question a budget is answered with.
    pub per_layer: [f64; crate::scene::LAYER_COUNT],
}

/// How long one frame took, as the GPU and the CPU each saw it.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTiming {
    /// Timestamp-query milliseconds across the whole command buffer.
    pub gpu_ms: f64,
    /// Wall milliseconds from `begin_command_buffer` to the fence signalling.
    pub submit_ms: f64,
    /// Instances drawn.
    pub instances: usize,
}

/// One target attachment the present pass can draw into.
#[derive(Clone, Copy, Debug)]
pub struct TargetImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub framebuffer: vk::Framebuffer,
}

pub struct Renderer {
    pub layout: RingLayout,
    /// Let the wind's displacement land between source texels at `S ≥ 2`
    /// (`--gpu-bend-substep`). Off by default: a whole-texel bend is what keeps every
    /// sprite texel on an exact `S × S` block, and the two are identical at `S = 1`.
    pub bend_substep: bool,
    /// Sample sprites the way the **CPU presenter** does — `Sprite::sample`'s four
    /// bilinear taps, at the un-snapped sub-pixel anchor — instead of nearest on a whole
    /// texel.
    ///
    /// This is not a quality setting. It exists so the two renderers can be compared with
    /// only the sampler between them: in this mode the GPU picture is the CPU picture, and
    /// any remaining difference is a bug in the adapter rather than a consequence of the
    /// pixel-art rule. Off by default, because the default *is* the pixel-art rule.
    pub filter_bilinear: bool,
    /// How many raster pixels one authored **source texel** covers.
    ///
    /// **Default 1, which is what the CPU presenter does.** `art_present` passes
    /// `scale = 1.0` to every plant, ground and body stamp whatever the world's `S` is,
    /// so a 16 × 16 tile covers 16 × 16 raster pixels at `S = 1` and at `S = 2` alike:
    /// `S` scales the *cell grid* and everything measured in cells, and leaves the art at
    /// the size it was authored.
    ///
    /// `design/flat-world-plan-2026-09-16.md` §6 says otherwise — "one factor that
    /// multiplies every length in the world — sprite tile, field cell, body extent" — and
    /// the two have not been reconciled. This is the knob that shows both: 1 is the
    /// presenter's rule and the reference the fidelity test compares against; `S` is the
    /// plan's, and is what the synthetic scene used.
    pub art_scale: f32,
    /// Measure [`FillProfile`] each frame (`--gpu-fill-profile`). Off by default.
    pub fill_profile: bool,
    /// The last recorded frame's profile; all zeroes while [`Renderer::fill_profile`]
    /// is off.
    fill: FillProfile,
    /// Frame origin in atlas texels → how many of that frame's texels paint. Built once
    /// from the atlas, so the profile costs a lookup rather than a second pass over the
    /// pack's pixels.
    opaque_by_origin: HashMap<[u16; 2], u32>,
    // --- the world raster ---
    raster_image: vk::Image,
    raster_memory: vk::DeviceMemory,
    raster_view: vk::ImageView,
    raster_framebuffer: vk::Framebuffer,
    scene_pass: vk::RenderPass,
    // --- resources the scene passes bind ---
    atlas_image: vk::Image,
    atlas_memory: vk::DeviceMemory,
    atlas_view: vk::ImageView,
    /// The per-frame page a procedurally rasterised rig is uploaded into.
    scratch_image: vk::Image,
    scratch_memory: vk::DeviceMemory,
    scratch_view: vk::ImageView,
    scratch_staging: HostBuffer,
    /// `(x, y)` of the next free texel and the current shelf's height, reset each frame.
    scratch_cursor: (u32, u32, u32),
    /// Regions written into the staging buffer this frame, to be copied before the pass.
    scratch_regions: Vec<(u32, u32, u32, u32, u64)>,
    field_images: [vk::Image; 2],
    field_memory: [vk::DeviceMemory; 2],
    field_views: [vk::ImageView; 2],
    field_staging: HostBuffer,
    field_revision: Option<u64>,
    nearest: vk::Sampler,
    uniforms: HostBuffer,
    instances: HostBuffer,
    instance_capacity: usize,
    scene_set_layout: vk::DescriptorSetLayout,
    scene_set: vk::DescriptorSet,
    // --- pipelines ---
    scene_pipeline_layout: vk::PipelineLayout,
    background_pipeline: vk::Pipeline,
    water_pipeline: vk::Pipeline,
    sprite_pipeline: vk::Pipeline,
    /// The last pass, shared with the voxel renderer: see [`PresentPass`].
    present: PresentPass,
    descriptor_pool: vk::DescriptorPool,
    // --- submission ---
    pub command_pool: vk::CommandPool,
    queries: vk::QueryPool,
}

impl Renderer {
    /// Build every pipeline and upload the atlas. One call per process.
    pub fn new(gpu: &Gpu, atlas: &Atlas, layout: RingLayout) -> Result<Renderer> {
        if !layout.is_valid() {
            bail!(
                "{layout:?} does not divide into whole {}-pixel cells",
                4 * layout.scale
            );
        }
        let d = &gpu.device;
        let command_pool = unsafe {
            d.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(gpu.queue_family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }?;
        let queries = unsafe {
            d.create_query_pool(
                &vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::TIMESTAMP)
                    .query_count(QUERY_SLOTS),
                None,
            )
        }?;

        // --- the world raster and its render pass ---
        let scene_pass = colour_pass(
            d,
            RASTER_FORMAT,
            vk::AttachmentLoadOp::DONT_CARE,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;
        let (raster_image, raster_memory) = gpu.image(
            layout.w,
            layout.h,
            RASTER_FORMAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_SRC,
        )?;
        let raster_view = gpu.view(raster_image, RASTER_FORMAT)?;
        let raster_framebuffer = framebuffer(d, scene_pass, raster_view, layout.w, layout.h)?;

        // --- the atlas, uploaded once ---
        let (atlas_image, atlas_memory) = gpu.image(
            atlas.width,
            atlas.height,
            vk::Format::R8G8B8A8_SRGB,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        )?;
        let staging =
            gpu.host_buffer(atlas.rgba.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC)?;
        staging.write(&atlas.rgba);
        gpu.one_shot(command_pool, |cb| unsafe {
            barrier(
                d,
                cb,
                atlas_image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            );
            d.cmd_copy_buffer_to_image(
                cb,
                staging.buffer,
                atlas_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy::default()
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D {
                        width: atlas.width,
                        height: atlas.height,
                        depth: 1,
                    })],
            );
            barrier(
                d,
                cb,
                atlas_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        })?;
        staging.destroy(gpu);
        let atlas_view = gpu.view(atlas_image, vk::Format::R8G8B8A8_SRGB)?;

        // --- the scratch page ---
        // 256 × 256 RGBA16F = 256 KB, which holds the Lanternjaw's eight parts (none of
        // them larger than the rig's query radius) for a handful of hunters at once. It
        // is premultiplied *linear*, not sRGB: `lanternjaw::Part`'s sprite is already in
        // that form and a round trip through 8-bit sRGB would be the only lossy step in
        // the whole path.
        let (scratch_image, scratch_memory) = gpu.image(
            SCRATCH_SIDE,
            SCRATCH_SIDE,
            vk::Format::R16G16B16A16_SFLOAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        )?;
        gpu.one_shot(command_pool, |cb| unsafe {
            barrier(
                d,
                cb,
                scratch_image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        })?;
        let scratch_view = gpu.view(scratch_image, vk::Format::R16G16B16A16_SFLOAT)?;
        let scratch_staging = gpu.host_buffer(
            u64::from(SCRATCH_SIDE) * u64::from(SCRATCH_SIDE) * 8,
            vk::BufferUsageFlags::TRANSFER_SRC,
        )?;

        // --- the two cell textures ---
        let (cx, cy) = (layout.cells_x(), layout.cells_y());
        let mut field_images = [vk::Image::null(); 2];
        let mut field_memory = [vk::DeviceMemory::null(); 2];
        let mut field_views = [vk::ImageView::null(); 2];
        for i in 0..2 {
            let (image, memory) = gpu.image(
                cx,
                cy,
                vk::Format::R16G16B16A16_SFLOAT,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            )?;
            gpu.one_shot(command_pool, |cb| unsafe {
                barrier(
                    d,
                    cb,
                    image,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );
            })?;
            field_images[i] = image;
            field_memory[i] = memory;
            field_views[i] = gpu.view(image, vk::Format::R16G16B16A16_SFLOAT)?;
        }
        // 2 textures × 4 channels × 2 bytes per cell = 57.6 KB at 80×45, which is the
        // per-tick upload budget `presenter-budget` §4(4) allowed.
        let field_staging = gpu.host_buffer(
            u64::from(cx) * u64::from(cy) * 8 * 2,
            vk::BufferUsageFlags::TRANSFER_SRC,
        )?;

        let nearest = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    // The ring wraps in x and does not in y. Nothing samples outside
                    // today (`cellValue` wraps by hand), but the address modes say what
                    // the topology is where a future pass forgets to.
                    .address_mode_u(vk::SamplerAddressMode::REPEAT)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )
        }?;

        let uniforms = gpu.host_buffer(
            std::mem::size_of::<SceneUniforms>() as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
        )?;
        let instance_capacity = 8192;
        let instances = gpu.host_buffer(
            (instance_capacity * std::mem::size_of::<SpriteInstance>()) as u64,
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )?;

        // --- descriptors ---
        let scene_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
            sampler_binding(1),
            sampler_binding(2),
            sampler_binding(3),
            sampler_binding(4),
        ];
        let scene_set_layout = unsafe {
            d.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&scene_bindings),
                None,
            )
        }?;
        let sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(4),
        ];
        let descriptor_pool = unsafe {
            d.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&sizes),
                None,
            )
        }?;
        let layouts = [scene_set_layout];
        let scene_set = unsafe {
            d.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&layouts),
            )
        }?[0];

        let buffer_info = [vk::DescriptorBufferInfo::default()
            .buffer(uniforms.buffer)
            .range(std::mem::size_of::<SceneUniforms>() as u64)];
        let image_info = |view: vk::ImageView| {
            [vk::DescriptorImageInfo::default()
                .sampler(nearest)
                .image_view(view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]
        };
        let (i1, i2, i3, i5) = (
            image_info(field_views[0]),
            image_info(field_views[1]),
            image_info(atlas_view),
            image_info(scratch_view),
        );
        unsafe {
            d.update_descriptor_sets(
                &[
                    vk::WriteDescriptorSet::default()
                        .dst_set(scene_set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(&buffer_info),
                    sampler_write(scene_set, 1, &i1),
                    sampler_write(scene_set, 2, &i2),
                    sampler_write(scene_set, 3, &i3),
                    sampler_write(scene_set, 4, &i5),
                ],
                &[],
            )
        };

        // --- pipelines ---
        let set_layouts = [scene_set_layout];
        let scene_pipeline_layout = unsafe {
            d.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts),
                None,
            )
        }?;
        let fullscreen = gpu.shader(FULLSCREEN_VERT)?;
        let background_fs = gpu.shader(BACKGROUND_FRAG)?;
        let water_fs = gpu.shader(WATER_FRAG)?;
        let sprite_vs = gpu.shader(SPRITE_VERT)?;
        let sprite_fs = gpu.shader(SPRITE_FRAG)?;
        let background_pipeline = fullscreen_pipeline(
            d,
            scene_pass,
            scene_pipeline_layout,
            fullscreen,
            background_fs,
            false,
        )?;
        let water_pipeline = fullscreen_pipeline(
            d,
            scene_pass,
            scene_pipeline_layout,
            fullscreen,
            water_fs,
            true,
        )?;
        let sprite_pipeline =
            sprite_pipeline(d, scene_pass, scene_pipeline_layout, sprite_vs, sprite_fs)?;
        unsafe {
            d.destroy_shader_module(fullscreen, None);
            d.destroy_shader_module(background_fs, None);
            d.destroy_shader_module(water_fs, None);
            d.destroy_shader_module(sprite_vs, None);
            d.destroy_shader_module(sprite_fs, None);
        }

        Ok(Renderer {
            layout,
            bend_substep: false,
            filter_bilinear: false,
            art_scale: 1.0,
            fill_profile: false,
            fill: FillProfile::default(),
            opaque_by_origin: atlas
                .frames
                .iter()
                .map(|f| ([f.x, f.y], f.opaque))
                .collect(),
            raster_image,
            raster_memory,
            raster_view,
            raster_framebuffer,
            scene_pass,
            atlas_image,
            atlas_memory,
            atlas_view,
            scratch_image,
            scratch_memory,
            scratch_view,
            scratch_staging,
            scratch_cursor: (0, 0, 0),
            scratch_regions: Vec::new(),
            field_images,
            field_memory,
            field_views,
            field_staging,
            field_revision: None,
            nearest,
            uniforms,
            instances,
            instance_capacity,
            scene_set_layout,
            scene_set,
            scene_pipeline_layout,
            background_pipeline,
            water_pipeline,
            sprite_pipeline,
            present: PresentPass::new(gpu, raster_view, nearest)?,
            descriptor_pool,
            command_pool,
            queries,
        })
    }

    /// The raster image, for a readback or for a target that wants to blit it.
    pub fn raster_image(&self) -> vk::Image {
        self.raster_image
    }

    /// Forget the previous frame's scratch allocations. Called once per frame, before
    /// any [`Renderer::scratch_push`], and cheap: the page is overwritten, not cleared.
    pub fn scratch_begin(&mut self) {
        self.scratch_cursor = (0, 0, 0);
        self.scratch_regions.clear();
    }

    /// Copy one procedurally rasterised sprite into this frame's scratch page.
    ///
    /// `pixels` is premultiplied linear RGBA in row-major order, `w · h` long — exactly
    /// what `cubarium_render::Sprite::texel` hands back. Returns where it landed, or
    /// `None` when the page is full, which the caller should treat as "skip this part"
    /// rather than as an error: a missing claw is better than a dropped frame.
    pub fn scratch_push(&mut self, w: u32, h: u32, pixels: &[[f32; 4]]) -> Option<[u16; 2]> {
        if w == 0 || h == 0 || w > SCRATCH_SIDE || pixels.len() < (w * h) as usize {
            return None;
        }
        let (mut x, mut y, mut shelf) = self.scratch_cursor;
        if x + w > SCRATCH_SIDE {
            x = 0;
            y += shelf;
            shelf = 0;
        }
        if y + h > SCRATCH_SIDE {
            return None;
        }
        // Each region is staged at its own byte offset and copied as its own rect, so
        // the staging buffer is written densely and the page never needs a full upload.
        let offset = self
            .scratch_regions
            .iter()
            .map(|r| u64::from(r.2) * u64::from(r.3) * 8)
            .sum();
        let mut halves = vec![0u16; (w * h * 4) as usize];
        for (i, p) in pixels[..(w * h) as usize].iter().enumerate() {
            for c in 0..4 {
                halves[i * 4 + c] = f16(p[c]);
            }
        }
        let bytes: &[u8] = bytemuck::cast_slice(&halves);
        if offset + bytes.len() as u64 > self.scratch_staging.size {
            return None;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.scratch_staging.ptr.add(offset as usize),
                bytes.len(),
            )
        };
        self.scratch_regions.push((x, y, w, h, offset));
        self.scratch_cursor = (x + w, y, shelf.max(h));
        Some([x as u16, y as u16])
    }

    /// The render pass a target's framebuffers must be built against, creating it for
    /// this attachment format and final layout on first use.
    pub fn present_pass(
        &mut self,
        gpu: &Gpu,
        format: vk::Format,
        final_layout: vk::ImageLayout,
    ) -> Result<vk::RenderPass> {
        self.present.pass(gpu, format, final_layout)
    }

    /// Upload what changed and record the whole frame: the four scene passes into the
    /// world raster, then — if a target is given — the present pass onto it.
    ///
    /// The command buffer must not be in flight. Returns the instance count drawn.
    pub fn record(
        &mut self,
        gpu: &Gpu,
        cb: vk::CommandBuffer,
        scene: &Scene,
        target: Option<TargetSlot<'_>>,
    ) -> Result<usize> {
        if scene.layout != self.layout {
            bail!(
                "scene layout {:?} is not the renderer's {:?}",
                scene.layout,
                self.layout
            );
        }
        let d = &gpu.device;
        self.uniforms.write(&[SceneUniforms::new(
            self.layout,
            scene.fields.producer_max,
            scene.seconds,
            scene.f,
            self.bend_substep,
            self.filter_bilinear,
            self.art_scale,
        )]);

        // Instances, concatenated in draw order; `first_instance` then selects a layer.
        // Written layer by layer straight into the mapped buffer: a scratch `Vec` here
        // was an allocation and a second 530 KB copy on every frame, and GS-1b's report
        // already names per-frame allocation as the adapter's largest single cost.
        let mut offsets = [(0u32, 0u32); crate::scene::LAYER_COUNT];
        let mut total = 0usize;
        for layer in LAYERS {
            let list = &scene.layers[layer as usize];
            offsets[layer as usize] = (total as u32, list.len() as u32);
            total += list.len();
        }
        if total > self.instance_capacity {
            bail!(
                "{total} instances exceeds the renderer's capacity of {}",
                self.instance_capacity
            );
        }
        for layer in LAYERS {
            self.instances.write_at(
                offsets[layer as usize].0 as usize,
                &scene.layers[layer as usize],
            );
        }
        if self.fill_profile {
            self.fill = self.profile(scene);
        }

        let target = target
            .map(|(image, extent, format, layout, xform)| {
                self.present
                    .entry(gpu, format, layout)
                    .map(|(pass, pipeline)| (image, extent, xform, pass, pipeline))
            })
            .transpose()?;

        let upload = self.field_revision != Some(scene.fields.revision);
        if upload {
            self.stage_fields(&scene.fields);
            self.field_revision = Some(scene.fields.revision);
        }

        unsafe {
            d.begin_command_buffer(
                cb,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            d.cmd_reset_query_pool(cb, self.queries, 0, QUERY_SLOTS);
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::TOP_OF_PIPE, self.queries, 0);

            if upload {
                let (cx, cy) = (self.layout.cells_x(), self.layout.cells_y());
                let plane = u64::from(cx) * u64::from(cy) * 8;
                for i in 0..2 {
                    barrier(
                        d,
                        cb,
                        self.field_images[i],
                        vk::ImageLayout::UNDEFINED,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    );
                    d.cmd_copy_buffer_to_image(
                        cb,
                        self.field_staging.buffer,
                        self.field_images[i],
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[vk::BufferImageCopy::default()
                            .buffer_offset(plane * i as u64)
                            .image_subresource(
                                vk::ImageSubresourceLayers::default()
                                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                                    .layer_count(1),
                            )
                            .image_extent(vk::Extent3D {
                                width: cx,
                                height: cy,
                                depth: 1,
                            })],
                    );
                    barrier(
                        d,
                        cb,
                        self.field_images[i],
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    );
                }
            }

            if !self.scratch_regions.is_empty() {
                let copies: Vec<vk::BufferImageCopy> = self
                    .scratch_regions
                    .iter()
                    .map(|&(x, y, w, h, offset)| {
                        vk::BufferImageCopy::default()
                            .buffer_offset(offset)
                            .buffer_row_length(w)
                            .buffer_image_height(h)
                            .image_offset(vk::Offset3D {
                                x: x as i32,
                                y: y as i32,
                                z: 0,
                            })
                            .image_subresource(
                                vk::ImageSubresourceLayers::default()
                                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                                    .layer_count(1),
                            )
                            .image_extent(vk::Extent3D {
                                width: w,
                                height: h,
                                depth: 1,
                            })
                    })
                    .collect();
                barrier(
                    d,
                    cb,
                    self.scratch_image,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                );
                d.cmd_copy_buffer_to_image(
                    cb,
                    self.scratch_staging.buffer,
                    self.scratch_image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &copies,
                );
                barrier(
                    d,
                    cb,
                    self.scratch_image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );
            }

            // --- the world raster ---
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, 1);
            begin(
                d,
                cb,
                self.scene_pass,
                self.raster_framebuffer,
                self.layout.w,
                self.layout.h,
            );
            d.cmd_bind_descriptor_sets(
                cb,
                vk::PipelineBindPoint::GRAPHICS,
                self.scene_pipeline_layout,
                0,
                &[self.scene_set],
                &[],
            );
            d.cmd_bind_pipeline(
                cb,
                vk::PipelineBindPoint::GRAPHICS,
                self.background_pipeline,
            );
            d.cmd_draw(cb, 3, 1, 0, 0);

            d.cmd_bind_vertex_buffers(cb, 0, &[self.instances.buffer], &[0]);
            let sprites = |layer: Layer| {
                let (first, count) = offsets[layer as usize];
                if count > 0 {
                    d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.sprite_pipeline);
                    d.cmd_draw(cb, 4, count, 0, first);
                }
            };
            sprites(Layer::GroundCover);
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.water_pipeline);
            d.cmd_draw(cb, 3, 1, 0, 0);
            sprites(Layer::Plants);
            sprites(Layer::Tall);
            sprites(Layer::Rain);
            sprites(Layer::Bodies);
            d.cmd_end_render_pass(cb);
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, 2);

            // --- the panel ---
            if let Some((image, extent, xform, pass, pipeline)) = target {
                self.present
                    .record(d, cb, image, extent, xform, pass, pipeline);
            }

            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, 3);
            d.end_command_buffer(cb)?;
        }
        Ok(total)
    }

    /// The last recorded frame's fill profile; zeroes unless [`Renderer::fill_profile`]
    /// is on.
    pub fn fill(&self) -> FillProfile {
        self.fill
    }

    /// [`FillProfile`] of one frame's instances.
    ///
    /// The three numbers are the same sum with a different area each: the quad
    /// `sprite.vert` builds, the quad it would have built around the whole tile, and
    /// the instance's painted texels. A stamp's own `scale` (a juvenile's 0.7, a rig
    /// part's) multiplies the renderer's `S · art_scale`, exactly as the shader does,
    /// so the areas are raster pixels and comparable across rungs.
    fn profile(&self, scene: &Scene) -> FillProfile {
        let world = self.layout.scale as f32 * self.art_scale;
        let mut p = FillProfile {
            instances: scene.instance_count(),
            ..FillProfile::default()
        };
        for layer in LAYERS {
            for i in &scene.layers[layer as usize] {
                let k = f64::from((world * i.scale.max(1e-3)).powi(2));
                let (qw, qh) = i.quad_texels();
                let quad = f64::from(qw * qh) * k;
                p.quad_px += quad;
                p.per_layer[layer as usize] += quad;
                // The quad before the opaque box: the whole tile, with GS-1b's one texel
                // of pad on each axis plus the bend's reach.
                let pad = f64::from(i.bend[0].abs() + 1.0);
                p.tile_px += (f64::from(i.size[0]) + 2.0 * pad) * (f64::from(i.size[1]) + 2.0) * k;
                let opaque: u32 = i
                    .frames
                    .iter()
                    .zip(i.weights)
                    .filter(|(_, w)| *w > 0.0)
                    .filter_map(|(o, _)| self.opaque_by_origin.get(o).copied())
                    .max()
                    .unwrap_or(0);
                p.opaque_px += f64::from(opaque) * k;
            }
        }
        p
    }

    /// Milliseconds between this frame's first and last timestamps, or `NaN` if the
    /// queries are not ready.
    pub fn gpu_ms(&self, gpu: &Gpu) -> f64 {
        self.gpu_split(gpu).map_or(f64::NAN, |s| s[0] + s[1] + s[2])
    }

    /// The frame's three GPU stages in milliseconds — **uploads**, the **world raster**
    /// pass and the **present** pass — or `None` if the queries are not ready.
    ///
    /// The split is between render passes, not inside one. This is a tiler: everything
    /// recorded inside a render pass is deferred to that pass's binning and resolve, so
    /// a timestamp between two draws inside the raster pass would measure nothing.
    /// Between passes it is exact, and it is the split that matters here — the present
    /// pass writes the panel's whole 1080x1920 and is fixed, while the raster pass is
    /// where the sprite fill lives and is what `--gpu-art-scale` multiplies.
    pub fn gpu_split(&self, gpu: &Gpu) -> Option<[f64; 3]> {
        let mut ts = [0u64; QUERY_SLOTS as usize];
        if unsafe {
            gpu.device.get_query_pool_results(
                self.queries,
                0,
                &mut ts,
                vk::QueryResultFlags::TYPE_64,
            )
        }
        .is_err()
        {
            return None;
        }
        let ns = f64::from(gpu.timestamp_period) / 1.0e6;
        Some([
            ts[1].wrapping_sub(ts[0]) as f64 * ns,
            ts[2].wrapping_sub(ts[1]) as f64 * ns,
            ts[3].wrapping_sub(ts[2]) as f64 * ns,
        ])
    }

    /// Copy the world raster into a host buffer, `w · 4` bytes per row. For the golden
    /// test and for desktop screenshots; never on the frame path.
    pub fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>> {
        let size = u64::from(self.layout.w) * u64::from(self.layout.h) * 4;
        let host = gpu.host_buffer(size, vk::BufferUsageFlags::TRANSFER_DST)?;
        let d = &gpu.device;
        gpu.one_shot(self.command_pool, |cb| unsafe {
            barrier(
                d,
                cb,
                self.raster_image,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            );
            d.cmd_copy_image_to_buffer(
                cb,
                self.raster_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                host.buffer,
                &[vk::BufferImageCopy::default()
                    .buffer_row_length(self.layout.w)
                    .buffer_image_height(self.layout.h)
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D {
                        width: self.layout.w,
                        height: self.layout.h,
                        depth: 1,
                    })],
            );
            barrier(
                d,
                cb,
                self.raster_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        })?;
        let bytes = unsafe { host.bytes() }[..size as usize].to_vec();
        host.destroy(gpu);
        Ok(bytes)
    }

    /// Pack the six fields into the staging buffer as two RGBA half-float planes.
    fn stage_fields(&self, fields: &crate::scene::Fields) {
        let n = self.layout.cell_count();
        let mut halves = vec![0u16; n * 8];
        let at = |v: &Vec<f32>, i: usize| v.get(i).copied().unwrap_or(0.0);
        for i in 0..n {
            halves[i * 4] = f16(at(&fields.producer, i));
            halves[i * 4 + 1] = f16(at(&fields.water, i));
            halves[i * 4 + 2] = f16(at(&fields.detritus, i));
            halves[i * 4 + 3] = f16(at(&fields.rain, i));
            let j = n * 4 + i * 4;
            halves[j] = f16(at(&fields.growth, i));
            halves[j + 1] = f16(at(&fields.tall, i));
        }
        self.field_staging.write(&halves);
    }

    /// Release everything. The device must be idle.
    pub fn destroy(&mut self, gpu: &Gpu) {
        self.present.destroy(gpu);
        let d = &gpu.device;
        unsafe {
            d.destroy_pipeline(self.background_pipeline, None);
            d.destroy_pipeline(self.water_pipeline, None);
            d.destroy_pipeline(self.sprite_pipeline, None);
            d.destroy_pipeline_layout(self.scene_pipeline_layout, None);
            d.destroy_descriptor_pool(self.descriptor_pool, None);
            d.destroy_descriptor_set_layout(self.scene_set_layout, None);
            self.uniforms.destroy(gpu);
            self.instances.destroy(gpu);
            self.field_staging.destroy(gpu);
            d.destroy_sampler(self.nearest, None);
            for i in 0..2 {
                d.destroy_image_view(self.field_views[i], None);
                d.destroy_image(self.field_images[i], None);
                d.free_memory(self.field_memory[i], None);
            }
            d.destroy_image_view(self.atlas_view, None);
            d.destroy_image(self.atlas_image, None);
            d.free_memory(self.atlas_memory, None);
            self.scratch_staging.destroy(gpu);
            d.destroy_image_view(self.scratch_view, None);
            d.destroy_image(self.scratch_image, None);
            d.free_memory(self.scratch_memory, None);
            d.destroy_framebuffer(self.raster_framebuffer, None);
            d.destroy_image_view(self.raster_view, None);
            d.destroy_image(self.raster_image, None);
            d.free_memory(self.raster_memory, None);
            d.destroy_render_pass(self.scene_pass, None);
            d.destroy_query_pool(self.queries, None);
            d.destroy_command_pool(self.command_pool, None);
        }
    }
}

/// IEEE-754 binary32 to binary16, round-to-nearest-even, with overflow to infinity and
/// subnormals handled. The fields carry densities in `[0, ~10]`, where half-float gives
/// three decimal digits — far more than a 20 Hz ecology's values mean.
fn f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xFF) as i32;
    let mantissa = bits & 0x007F_FFFF;
    if exponent == 0xFF {
        // Inf or NaN.
        return sign | 0x7C00 | if mantissa != 0 { 0x0200 } else { 0 };
    }
    let unbiased = exponent - 127 + 15;
    if unbiased >= 0x1F {
        return sign | 0x7C00;
    }
    if unbiased <= 0 {
        if unbiased < -10 {
            return sign;
        }
        let m = mantissa | 0x0080_0000;
        let shift = (14 - unbiased) as u32;
        let half = (m >> shift) as u16;
        let round = (m >> (shift - 1)) & 1;
        return sign | (half + round as u16);
    }
    let half = ((unbiased as u32) << 10) as u16 | (mantissa >> 13) as u16;
    let round = u16::from(
        (mantissa >> 12) & 1 == 1 && (mantissa & 0x0FFF != 0 || (mantissa >> 13) & 1 == 1),
    );
    sign | (half + round)
}

fn sampler_binding(binding: u32) -> vk::DescriptorSetLayoutBinding<'static> {
    vk::DescriptorSetLayoutBinding::default()
        .binding(binding)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
}

fn sampler_write<'a>(
    set: vk::DescriptorSet,
    binding: u32,
    info: &'a [vk::DescriptorImageInfo; 1],
) -> vk::WriteDescriptorSet<'a> {
    vk::WriteDescriptorSet::default()
        .dst_set(set)
        .dst_binding(binding)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(info)
}

pub(crate) fn colour_pass(
    d: &ash::Device,
    format: vk::Format,
    load: vk::AttachmentLoadOp,
    final_layout: vk::ImageLayout,
) -> Result<vk::RenderPass> {
    let attachments = [vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(load)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(final_layout)];
    let refs = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&refs)];
    Ok(unsafe {
        d.create_render_pass(
            &vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(&subpasses),
            None,
        )
    }?)
}

/// A single-attachment framebuffer for a render pass. Public because a target lives
/// outside this crate (the desktop window) and must build its own.
pub fn framebuffer(
    d: &ash::Device,
    pass: vk::RenderPass,
    view: vk::ImageView,
    width: u32,
    height: u32,
) -> Result<vk::Framebuffer> {
    let views = [view];
    Ok(unsafe {
        d.create_framebuffer(
            &vk::FramebufferCreateInfo::default()
                .render_pass(pass)
                .attachments(&views)
                .width(width)
                .height(height)
                .layers(1),
            None,
        )
    }?)
}

/// `cmd_begin_render_pass` with the viewport and scissor set.
pub(crate) unsafe fn begin(
    d: &ash::Device,
    cb: vk::CommandBuffer,
    pass: vk::RenderPass,
    fb: vk::Framebuffer,
    w: u32,
    h: u32,
) {
    unsafe {
        let extent = vk::Extent2D {
            width: w,
            height: h,
        };
        d.cmd_begin_render_pass(
            cb,
            &vk::RenderPassBeginInfo::default()
                .render_pass(pass)
                .framebuffer(fb)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent,
                }),
            vk::SubpassContents::INLINE,
        );
        d.cmd_set_viewport(
            cb,
            0,
            &[vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: w as f32,
                height: h as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            }],
        );
        d.cmd_set_scissor(
            cb,
            0,
            &[vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            }],
        );
    }
}

fn premultiplied_blend() -> vk::PipelineColorBlendAttachmentState {
    // Source-over on premultiplied colour: exactly `rgba · opacity + background · (1 − a)`,
    // which is what `stamp_unfolded` writes.
    vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(vk::ColorComponentFlags::RGBA)
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::ONE)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .alpha_blend_op(vk::BlendOp::ADD)
}

pub(crate) fn fullscreen_pipeline(
    d: &ash::Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    vs: vk::ShaderModule,
    fs: vk::ShaderModule,
    blend: bool,
) -> Result<vk::Pipeline> {
    fullscreen_pipeline_specialised(d, pass, layout, vs, fs, blend, None)
}

/// [`fullscreen_pipeline`] with the fragment stage's specialisation constants set.
pub(crate) fn fullscreen_pipeline_specialised(
    d: &ash::Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    vs: vk::ShaderModule,
    fs: vk::ShaderModule,
    blend: bool,
    fragment: Option<&vk::SpecializationInfo>,
) -> Result<vk::Pipeline> {
    let vi = vk::PipelineVertexInputStateCreateInfo::default();
    build_pipeline(
        d,
        pass,
        layout,
        vs,
        fs,
        vi,
        vk::PrimitiveTopology::TRIANGLE_LIST,
        blend,
        fragment,
    )
}

fn sprite_pipeline(
    d: &ash::Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    vs: vk::ShaderModule,
    fs: vk::ShaderModule,
) -> Result<vk::Pipeline> {
    use vk::Format as F;
    // `SpriteInstance`, field by field. Keep this table and `scene.rs` together.
    let bindings = [vk::VertexInputBindingDescription::default()
        .binding(0)
        .stride(std::mem::size_of::<SpriteInstance>() as u32)
        .input_rate(vk::VertexInputRate::INSTANCE)];
    let attribute = |location: u32, format: F, offset: u32| {
        vk::VertexInputAttributeDescription::default()
            .location(location)
            .binding(0)
            .format(format)
            .offset(offset)
    };
    let attributes = [
        attribute(0, F::R32G32_SFLOAT, 0),          // anchor
        attribute(1, F::R32G32_SFLOAT, 8),          // heading
        attribute(2, F::R16G16B16A16_UINT, 16),     // frames 0 and 1, by origin
        attribute(3, F::R16G16B16A16_UINT, 24),     // frames 2 and 3
        attribute(4, F::R16G16B16A16_UINT, 32),     // size w, h + pivot x, y
        attribute(5, F::R32G32B32A32_SFLOAT, 40),   // the four frame weights
        attribute(6, F::R32G32B32A32_SFLOAT, 56),   // bend amplitude, base, root, length
        attribute(7, F::R32G32B32A32_SFLOAT, 72),   // mask floor, reveal, flags, opacity
        attribute(8, F::R32G32B32A32_SFLOAT, 88),   // tone colour rgb + tone mix
        attribute(9, F::R32G32B32A32_SFLOAT, 104),  // shade floor, reference, scale, source
        attribute(10, F::R32G32B32A32_SFLOAT, 120), // the opaque box, x0 y0 x1 y1
    ];
    let vi = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&bindings)
        .vertex_attribute_descriptions(&attributes);
    build_pipeline(
        d,
        pass,
        layout,
        vs,
        fs,
        vi,
        vk::PrimitiveTopology::TRIANGLE_STRIP,
        true,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_pipeline(
    d: &ash::Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    vs: vk::ShaderModule,
    fs: vk::ShaderModule,
    vi: vk::PipelineVertexInputStateCreateInfo,
    topology: vk::PrimitiveTopology,
    blend: bool,
    fragment: Option<&vk::SpecializationInfo>,
) -> Result<vk::Pipeline> {
    let mut fs_stage = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::FRAGMENT)
        .module(fs)
        .name(c"main");
    if let Some(spec) = fragment {
        fs_stage = fs_stage.specialization_info(spec);
    }
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vs)
            .name(c"main"),
        fs_stage,
    ];
    let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(topology);
    let vp = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let rs = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
        .line_width(1.0);
    let ms = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let attachments = [if blend {
        premultiplied_blend()
    } else {
        vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
    }];
    let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);
    let dynamic = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
    let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic);
    let info = [vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vi)
        .input_assembly_state(&ia)
        .viewport_state(&vp)
        .rasterization_state(&rs)
        .multisample_state(&ms)
        .color_blend_state(&cb)
        .dynamic_state(&dy)
        .layout(layout)
        .render_pass(pass)
        .subpass(0)];
    unsafe { d.create_graphics_pipelines(vk::PipelineCache::null(), &info, None) }
        .map_err(|(_, e)| anyhow!("vkCreateGraphicsPipelines: {e}"))
        .map(|p| p[0])
        .context("build a graphics pipeline")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_half_float_conversion_round_trips_the_values_a_field_carries() {
        for v in [0.0f32, 1.0, 0.5, 0.1, 1.5, 12.75, 1e-4, 65504.0, -3.25] {
            let back = half_to_f32(f16(v));
            let tolerance = v.abs().max(1.0) * 1e-3;
            assert!((back - v).abs() <= tolerance, "{v} -> {back}");
        }
        assert_eq!(f16(f32::INFINITY), 0x7C00);
        assert_eq!(f16(0.0), 0);
    }

    fn half_to_f32(h: u16) -> f32 {
        let sign = u32::from(h >> 15) << 31;
        let exponent = i32::from((h >> 10) & 0x1F);
        let mantissa = u32::from(h & 0x3FF);
        if exponent == 0 {
            if mantissa == 0 {
                return f32::from_bits(sign);
            }
            return f32::from_bits(sign) + (mantissa as f32) * 2.0f32.powi(-24);
        }
        if exponent == 0x1F {
            return f32::from_bits(sign | 0x7F80_0000 | (mantissa << 13));
        }
        f32::from_bits(sign | (((exponent - 15 + 127) as u32) << 23) | (mantissa << 13))
    }

    #[test]
    fn the_quarter_turn_maps_the_rings_corners_onto_the_portrait_panel() {
        // 320x180 at k = 6 is 1920x1080; the panel is 1080x1920, so one quarter turn.
        let x = PresentTransform {
            factor: 6,
            quarter_turns: 1,
            encode_srgb: false,
        };
        let p = x.push((1080, 1920));
        let map = |px: f32, py: f32| {
            (
                (p[0] * px + p[2] * py + p[4]).floor(),
                (p[1] * px + p[3] * py + p[5]).floor(),
            )
        };
        assert_eq!(
            map(0.0, 0.0),
            (0.0, 179.0),
            "panel top-left is the raster's bottom-left"
        );
        assert_eq!(map(1079.0, 0.0), (0.0, 0.0));
        assert_eq!(map(0.0, 1919.0), (319.0, 179.0));
        assert_eq!(map(1079.0, 1919.0), (319.0, 0.0));
    }

    #[test]
    fn fit_takes_the_largest_integer_upscale_that_the_panel_holds() {
        let x = PresentTransform::fit((320, 180), (1080, 1920), 1, false).unwrap();
        assert_eq!(x.factor, 6);
        let x = PresentTransform::fit((640, 360), (1080, 1920), 1, false).unwrap();
        assert_eq!(x.factor, 3);
        assert!(PresentTransform::fit((2000, 2000), (1080, 1920), 0, false).is_none());
    }
}

/// The ring renderer as a target's frame source: one frame is one [`Scene`].
impl FrameSource for Renderer {
    type Frame<'a> = &'a Scene;

    fn raster_size(&self) -> (u32, u32) {
        (self.layout.w, self.layout.h)
    }

    fn command_pool(&self) -> vk::CommandPool {
        self.command_pool
    }

    fn present_pass(
        &mut self,
        gpu: &Gpu,
        format: vk::Format,
        final_layout: vk::ImageLayout,
    ) -> Result<vk::RenderPass> {
        Renderer::present_pass(self, gpu, format, final_layout)
    }

    fn record_frame(
        &mut self,
        gpu: &Gpu,
        cb: vk::CommandBuffer,
        frame: &Scene,
        target: Option<TargetSlot<'_>>,
    ) -> Result<()> {
        Renderer::record(self, gpu, cb, frame, target).map(|_| ())
    }

    fn gpu_ms(&self, gpu: &Gpu) -> f64 {
        Renderer::gpu_ms(self, gpu)
    }

    fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>> {
        Renderer::read_raster(self, gpu)
    }
}
