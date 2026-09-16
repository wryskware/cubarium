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
use crate::scene::{LAYERS, Layer, RingLayout, Scene, SpriteInstance};
use crate::vk::{Gpu, HostBuffer, barrier};

const FULLSCREEN_VERT: &[u8] = include_bytes!("../shaders/fullscreen.vert.spv");
const BACKGROUND_FRAG: &[u8] = include_bytes!("../shaders/background.frag.spv");
const WATER_FRAG: &[u8] = include_bytes!("../shaders/water.frag.spv");
const SPRITE_VERT: &[u8] = include_bytes!("../shaders/sprite.vert.spv");
const SPRITE_FRAG: &[u8] = include_bytes!("../shaders/sprite.frag.spv");
const PRESENT_FRAG: &[u8] = include_bytes!("../shaders/present.frag.spv");

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
    pub fn fit(raster: (u32, u32), target: (u32, u32), quarter_turns: u32, encode_srgb: bool) -> Option<PresentTransform> {
        let (rw, rh) = if quarter_turns % 2 == 0 { raster } else { (raster.1, raster.0) };
        let factor = (target.0 / rw).min(target.1 / rh);
        (factor >= 1).then_some(PresentTransform { factor, quarter_turns, encode_srgb })
    }

    /// The push constants: two columns of the panel→raster matrix, then its offset.
    fn push(&self, target: (u32, u32)) -> [f32; 8] {
        let k = 1.0 / self.factor as f32;
        let (pw, ph) = (target.0 as f32, target.1 as f32);
        // col0 = (m00, m10), col1 = (m01, m11): `M · p = col0·p.x + col1·p.y`.
        let (col0, col1, off) = match self.quarter_turns % 4 {
            0 => ([k, 0.0], [0.0, k], [0.0, 0.0]),
            1 => ([0.0, -k], [k, 0.0], [0.0, (pw - 1.0) * k]),
            2 => ([-k, 0.0], [0.0, -k], [(pw - 1.0) * k, (ph - 1.0) * k]),
            _ => ([0.0, k], [-k, 0.0], [(ph - 1.0) * k, 0.0]),
        };
        [col0[0], col0[1], col1[0], col1[1], off[0], off[1], if self.encode_srgb { 1.0 } else { 0.0 }, 0.0]
    }
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
pub struct TargetImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub framebuffer: vk::Framebuffer,
}

pub struct Renderer {
    pub layout: RingLayout,
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
    // --- the present pass, one per target format ---
    present_set_layout: vk::DescriptorSetLayout,
    present_set: vk::DescriptorSet,
    present_pipeline_layout: vk::PipelineLayout,
    present: HashMap<(i32, i32), (vk::RenderPass, vk::Pipeline)>,
    descriptor_pool: vk::DescriptorPool,
    // --- submission ---
    pub command_pool: vk::CommandPool,
    queries: vk::QueryPool,
}

impl Renderer {
    /// Build every pipeline and upload the atlas. One call per process.
    pub fn new(gpu: &Gpu, atlas: &Atlas, layout: RingLayout) -> Result<Renderer> {
        if !layout.is_valid() {
            bail!("{layout:?} does not divide into whole {}-pixel cells", 4 * layout.scale);
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
                    .query_count(2),
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
        let staging = gpu.host_buffer(atlas.rgba.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC)?;
        staging.write(&atlas.rgba);
        gpu.one_shot(command_pool, |cb| unsafe {
            barrier(d, cb, atlas_image, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
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
                    .image_extent(vk::Extent3D { width: atlas.width, height: atlas.height, depth: 1 })],
            );
            barrier(d, cb, atlas_image, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        })?;
        staging.destroy(gpu);
        let atlas_view = gpu.view(atlas_image, vk::Format::R8G8B8A8_SRGB)?;

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
                barrier(d, cb, image, vk::ImageLayout::UNDEFINED, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
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
        ];
        let scene_set_layout = unsafe {
            d.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&scene_bindings),
                None,
            )
        }?;
        let present_bindings = [sampler_binding(0)];
        let present_set_layout = unsafe {
            d.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&present_bindings),
                None,
            )
        }?;
        let sizes = [
            vk::DescriptorPoolSize::default().ty(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(1),
            vk::DescriptorPoolSize::default().ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(4),
        ];
        let descriptor_pool = unsafe {
            d.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default().max_sets(2).pool_sizes(&sizes),
                None,
            )
        }?;
        let layouts = [scene_set_layout, present_set_layout];
        let sets = unsafe {
            d.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&layouts),
            )
        }?;
        let (scene_set, present_set) = (sets[0], sets[1]);

        let buffer_info = [vk::DescriptorBufferInfo::default()
            .buffer(uniforms.buffer)
            .range(std::mem::size_of::<SceneUniforms>() as u64)];
        let image_info = |view: vk::ImageView| {
            [vk::DescriptorImageInfo::default()
                .sampler(nearest)
                .image_view(view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]
        };
        let (i1, i2, i3, i4) = (
            image_info(field_views[0]),
            image_info(field_views[1]),
            image_info(atlas_view),
            image_info(raster_view),
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
                    sampler_write(present_set, 0, &i4),
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
        let present_layouts = [present_set_layout];
        let ranges = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .size(32)];
        let present_pipeline_layout = unsafe {
            d.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&present_layouts)
                    .push_constant_ranges(&ranges),
                None,
            )
        }?;

        let fullscreen = gpu.shader(FULLSCREEN_VERT)?;
        let background_fs = gpu.shader(BACKGROUND_FRAG)?;
        let water_fs = gpu.shader(WATER_FRAG)?;
        let sprite_vs = gpu.shader(SPRITE_VERT)?;
        let sprite_fs = gpu.shader(SPRITE_FRAG)?;
        let background_pipeline = fullscreen_pipeline(d, scene_pass, scene_pipeline_layout, fullscreen, background_fs, false)?;
        let water_pipeline = fullscreen_pipeline(d, scene_pass, scene_pipeline_layout, fullscreen, water_fs, true)?;
        let sprite_pipeline = sprite_pipeline(d, scene_pass, scene_pipeline_layout, sprite_vs, sprite_fs)?;
        unsafe {
            d.destroy_shader_module(fullscreen, None);
            d.destroy_shader_module(background_fs, None);
            d.destroy_shader_module(water_fs, None);
            d.destroy_shader_module(sprite_vs, None);
            d.destroy_shader_module(sprite_fs, None);
        }

        Ok(Renderer {
            layout,
            raster_image,
            raster_memory,
            raster_view,
            raster_framebuffer,
            scene_pass,
            atlas_image,
            atlas_memory,
            atlas_view,
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
            present_set_layout,
            present_set,
            present_pipeline_layout,
            present: HashMap::new(),
            descriptor_pool,
            command_pool,
            queries,
        })
    }

    /// The raster image, for a readback or for a target that wants to blit it.
    pub fn raster_image(&self) -> vk::Image {
        self.raster_image
    }

    /// The render pass a target's framebuffers must be built against, creating it for
    /// this attachment format and final layout on first use.
    pub fn present_pass(
        &mut self,
        gpu: &Gpu,
        format: vk::Format,
        final_layout: vk::ImageLayout,
    ) -> Result<vk::RenderPass> {
        Ok(self.present_entry(gpu, format, final_layout)?.0)
    }

    fn present_entry(
        &mut self,
        gpu: &Gpu,
        format: vk::Format,
        final_layout: vk::ImageLayout,
    ) -> Result<(vk::RenderPass, vk::Pipeline)> {
        let key = (format.as_raw(), final_layout.as_raw());
        if let Some(entry) = self.present.get(&key) {
            return Ok(*entry);
        }
        let d = &gpu.device;
        let pass = colour_pass(d, format, vk::AttachmentLoadOp::DONT_CARE, final_layout)?;
        let vs = gpu.shader(FULLSCREEN_VERT)?;
        let fs = gpu.shader(PRESENT_FRAG)?;
        let pipeline = fullscreen_pipeline(d, pass, self.present_pipeline_layout, vs, fs, false)?;
        unsafe {
            d.destroy_shader_module(vs, None);
            d.destroy_shader_module(fs, None);
        }
        self.present.insert(key, (pass, pipeline));
        Ok((pass, pipeline))
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
        target: Option<(&TargetImage, (u32, u32), vk::Format, vk::ImageLayout, PresentTransform)>,
    ) -> Result<usize> {
        if scene.layout != self.layout {
            bail!("scene layout {:?} is not the renderer's {:?}", scene.layout, self.layout);
        }
        let d = &gpu.device;
        self.uniforms.write(&[SceneUniforms::new(
            self.layout,
            scene.fields.producer_max,
            scene.seconds,
            scene.f,
        )]);

        // Instances, concatenated in draw order; `first_instance` then selects a layer.
        let mut offsets = [(0u32, 0u32); crate::scene::LAYER_COUNT];
        let mut flat: Vec<SpriteInstance> = Vec::with_capacity(scene.instance_count());
        for layer in LAYERS {
            let list = &scene.layers[layer as usize];
            offsets[layer as usize] = (flat.len() as u32, list.len() as u32);
            flat.extend_from_slice(list);
        }
        if flat.len() > self.instance_capacity {
            bail!(
                "{} instances exceeds the renderer's capacity of {}",
                flat.len(),
                self.instance_capacity
            );
        }
        self.instances.write(&flat);

        let target = target
            .map(|(image, extent, format, layout, xform)| {
                self.present_entry(gpu, format, layout)
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
            d.cmd_reset_query_pool(cb, self.queries, 0, 2);
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::TOP_OF_PIPE, self.queries, 0);

            if upload {
                let (cx, cy) = (self.layout.cells_x(), self.layout.cells_y());
                let plane = u64::from(cx) * u64::from(cy) * 8;
                for i in 0..2 {
                    barrier(d, cb, self.field_images[i], vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
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
                            .image_extent(vk::Extent3D { width: cx, height: cy, depth: 1 })],
                    );
                    barrier(d, cb, self.field_images[i], vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                }
            }

            // --- the world raster ---
            begin(d, cb, self.scene_pass, self.raster_framebuffer, self.layout.w, self.layout.h);
            d.cmd_bind_descriptor_sets(
                cb,
                vk::PipelineBindPoint::GRAPHICS,
                self.scene_pipeline_layout,
                0,
                &[self.scene_set],
                &[],
            );
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.background_pipeline);
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

            // --- the panel ---
            if let Some((image, extent, xform, pass, pipeline)) = target {
                begin(d, cb, pass, image.framebuffer, extent.0, extent.1);
                d.cmd_bind_descriptor_sets(
                    cb,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.present_pipeline_layout,
                    0,
                    &[self.present_set],
                    &[],
                );
                d.cmd_push_constants(
                    cb,
                    self.present_pipeline_layout,
                    vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(&xform.push(extent)),
                );
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
                d.cmd_draw(cb, 3, 1, 0, 0);
                d.cmd_end_render_pass(cb);
            }

            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, 1);
            d.end_command_buffer(cb)?;
        }
        Ok(flat.len())
    }

    /// Milliseconds between this frame's two timestamps, or `NaN` if the queries are
    /// not ready.
    pub fn gpu_ms(&self, gpu: &Gpu) -> f64 {
        let mut ts = [0u64; 2];
        if unsafe {
            gpu.device
                .get_query_pool_results(self.queries, 0, &mut ts, vk::QueryResultFlags::TYPE_64)
        }
        .is_err()
        {
            return f64::NAN;
        }
        ts[1].wrapping_sub(ts[0]) as f64 * f64::from(gpu.timestamp_period) / 1.0e6
    }

    /// Copy the world raster into a host buffer, `w · 4` bytes per row. For the golden
    /// test and for desktop screenshots; never on the frame path.
    pub fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>> {
        let size = u64::from(self.layout.w) * u64::from(self.layout.h) * 4;
        let host = gpu.host_buffer(size, vk::BufferUsageFlags::TRANSFER_DST)?;
        let d = &gpu.device;
        gpu.one_shot(self.command_pool, |cb| unsafe {
            barrier(d, cb, self.raster_image, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
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
                    .image_extent(vk::Extent3D { width: self.layout.w, height: self.layout.h, depth: 1 })],
            );
            barrier(d, cb, self.raster_image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
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
        let d = &gpu.device;
        unsafe {
            for (pass, pipeline) in self.present.values() {
                d.destroy_pipeline(*pipeline, None);
                d.destroy_render_pass(*pass, None);
            }
            d.destroy_pipeline(self.background_pipeline, None);
            d.destroy_pipeline(self.water_pipeline, None);
            d.destroy_pipeline(self.sprite_pipeline, None);
            d.destroy_pipeline_layout(self.scene_pipeline_layout, None);
            d.destroy_pipeline_layout(self.present_pipeline_layout, None);
            d.destroy_descriptor_pool(self.descriptor_pool, None);
            d.destroy_descriptor_set_layout(self.scene_set_layout, None);
            d.destroy_descriptor_set_layout(self.present_set_layout, None);
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
    let round = u16::from((mantissa >> 12) & 1 == 1 && (mantissa & 0x0FFF != 0 || (mantissa >> 13) & 1 == 1));
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

fn colour_pass(
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
            &vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpasses),
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
unsafe fn begin(
    d: &ash::Device,
    cb: vk::CommandBuffer,
    pass: vk::RenderPass,
    fb: vk::Framebuffer,
    w: u32,
    h: u32,
) {
    unsafe {
        let extent = vk::Extent2D { width: w, height: h };
        d.cmd_begin_render_pass(
            cb,
            &vk::RenderPassBeginInfo::default()
                .render_pass(pass)
                .framebuffer(fb)
                .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }),
            vk::SubpassContents::INLINE,
        );
        d.cmd_set_viewport(
            cb,
            0,
            &[vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 }],
        );
        d.cmd_set_scissor(cb, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent }]);
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

fn fullscreen_pipeline(
    d: &ash::Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    vs: vk::ShaderModule,
    fs: vk::ShaderModule,
    blend: bool,
) -> Result<vk::Pipeline> {
    let vi = vk::PipelineVertexInputStateCreateInfo::default();
    build_pipeline(d, pass, layout, vs, fs, vi, vk::PrimitiveTopology::TRIANGLE_LIST, blend)
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
        attribute(0, F::R32G32_SFLOAT, 0),              // anchor
        attribute(1, F::R32G32_SFLOAT, 8),              // heading
        attribute(2, F::R16G16B16A16_UINT, 16),         // frame0 x, y, w, h
        attribute(3, F::R16G16B16A16_UINT, 24),         // frame1 x, y + pivot x, y
        attribute(4, F::R32G32_SFLOAT, 32),             // mix, opacity
        attribute(5, F::R32G32B32A32_SFLOAT, 40),       // bend
        attribute(6, F::R32G32_SFLOAT, 56),             // mask floor, reveal
        attribute(7, F::R32G32B32_SFLOAT, 64),          // tone colour
        attribute(8, F::R32G32B32_SFLOAT, 76),          // shade floor, reference, tone mix
    ];
    let vi = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&bindings)
        .vertex_attribute_descriptions(&attributes);
    build_pipeline(d, pass, layout, vs, fs, vi, vk::PrimitiveTopology::TRIANGLE_STRIP, true)
}

fn build_pipeline(
    d: &ash::Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    vs: vk::ShaderModule,
    fs: vk::ShaderModule,
    vi: vk::PipelineVertexInputStateCreateInfo,
    topology: vk::PrimitiveTopology,
    blend: bool,
) -> Result<vk::Pipeline> {
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vs)
            .name(c"main"),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fs)
            .name(c"main"),
    ];
    let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(topology);
    let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
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
        vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)
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
        let x = PresentTransform { factor: 6, quarter_turns: 1, encode_srgb: false };
        let p = x.push((1080, 1920));
        let map = |px: f32, py: f32| {
            (
                (p[0] * px + p[2] * py + p[4]).floor(),
                (p[1] * px + p[3] * py + p[5]).floor(),
            )
        };
        assert_eq!(map(0.0, 0.0), (0.0, 179.0), "panel top-left is the raster's bottom-left");
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
