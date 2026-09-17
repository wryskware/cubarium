//! The voxel strip drawn per pixel by a slab walk in a fragment shader.
//!
//! # What this renderer is
//!
//! `crates/cubarium/src/voxel/present.rs` is the **definition of the picture** — the
//! elevated-orthographic projection, the autotiled block faces, the translucent water,
//! the plants as voxels — and it draws on the CPU, one rectangle at a time, far to near.
//! This is a second renderer of that same picture, and it works the other way round:
//! for every raster pixel, `voxel.frag` walks the depth slabs **near to far** and asks
//! which voxel and which face owns the pixel in each slab. The first opaque face ends the
//! walk; water faces met on the way blend front to back.
//!
//! That inversion is why the CPU's ownership bookkeeping is absent here. `nearer_owns`
//! and the `front_hidden`/`top_hidden` culls exist because the CPU paints with no depth
//! test and must not paint a translucent face twice; walking front to back gives the same
//! ownership by construction. The **rules** are ported — rim, bevel, chamfer, riser,
//! contour suppression, roof shadow, haze, water fill and its one-surface-per-pixel
//! clip, pore darkening, trunk cylinder, crown edge and skirt, sprout — and the
//! bookkeeping is not, except the two places the rules genuinely are local functions of
//! a cell and its nearer neighbour: the water body's `stop` row and the water top's
//! `nearer_owns`, both of which the shader evaluates the same way the CPU does.
//!
//! # What crosses the bus, per tick
//!
//! | what | format | size at 128×48×24 |
//! |---|---|---|
//! | one texel per voxel: material + plant part, free water, pore water, plant style | `R8G8B8A8_UINT` 3D | 576 KiB |
//! | the roof-gap table (`build_roof`), voxels to the nearest solid above | `R8_UINT` 3D | 144 KiB |
//! | the frame's plant styles: wood, crown, heart in linear light | `R32G32B32A32_SFLOAT` 3×256 | 12 KiB |
//!
//! The roof table is a texture rather than a column walk in the shader because the walk
//! costs up to `height` fetches per shaded face and the table costs one; both are
//! implemented ([`VoxelParams::roof_from_texture`]) and the measurement is in the report.
//! Nothing here is per **frame**: a frame is one draw of one full-screen triangle, so the
//! render rate is free of the tick rate and the raster can be any `px_per_voxel` over the
//! same upload.
//!
//! # The packing
//!
//! [`VoxelTexel`] is the one place the layout is written down, and `voxel.frag` unpacks
//! it the same way. Free water is quantised to 8 bits with a **floor**: any water at all
//! is at least 1, because the CPU presenter draws at least one row of water for any
//! `free` above its epsilon and a plain `round` would lose a film. The quantisation is
//! the one knowingly lossy step in the path (±1/510 in the fraction, which moves a
//! drawn water row only where `free · s` sits within that of a half-pixel).

use anyhow::{Context, Result, bail};
use ash::vk;

use crate::present::{FrameSource, PresentPass, TargetSlot};
use crate::render::{RASTER_FORMAT, framebuffer};
use crate::vk::{Gpu, HostBuffer, barrier};

const FULLSCREEN_VERT: &[u8] = include_bytes!("../shaders/fullscreen.vert.spv");
const VOXEL_FRAG: &[u8] = include_bytes!("../shaders/voxel.frag.spv");

/// Timestamp slots: top of pipe, after the uploads, after the world raster, bottom.
const QUERY_SLOTS: u32 = 4;

/// How many plant styles one frame may paint with.
///
/// A style is one stand's wood/crown/heart in linear light, already carrying its crown
/// fill and its wilt, so the count is the number of *distinct* colour triples a frame
/// needs — seed banks collapse to one per species, and identical stands collapse
/// together. The CPU presenter's own cap is 65 535 stands; a frame that really needs more
/// than this many distinct styles reuses style 0 for the rest and the sink says so.
pub const MAX_STYLES: usize = 256;

/// Plant part classes, as the texel's `part` field carries them.
pub const PART_NONE: u8 = 0;
pub const PART_TRUNK: u8 = 1;
pub const PART_CROWN: u8 = 2;
pub const PART_CROWN_HEART: u8 = 3;
pub const PART_SPROUT: u8 = 4;

/// One voxel, as the shader reads it: `R8G8B8A8_UINT`.
///
/// * `r` — material id in bits 0–1, plant part class in bits 2–4;
/// * `g` — free water as a fraction of the void volume, `0` for dry and `1..=255`
///   for any water at all (see the module header on the floor);
/// * `b` — pore water as a fraction of the pore capacity;
/// * `a` — the plant style index, meaningful only when `part != PART_NONE`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct VoxelTexel(pub [u8; 4]);

impl VoxelTexel {
    /// Pack one voxel. `material` is `Material as u8` (0 air … 3 soil), `part` one of the
    /// `PART_*` classes, `free` and `pore` fractions in `0..=1`, and `dry` says the cell
    /// holds no water at all — which is the presenter's `free <= WATER_EPSILON`, decided
    /// by the caller because the epsilon is the presenter's constant and not this
    /// format's.
    pub fn pack(material: u8, part: u8, free: f32, dry: bool, pore: f32, style: u8) -> VoxelTexel {
        let g = if dry { 0 } else { quantise(free).max(1) };
        VoxelTexel([(material & 0x03) | ((part & 0x07) << 2), g, quantise(pore), style])
    }

    pub fn material(self) -> u8 {
        self.0[0] & 0x03
    }

    pub fn part(self) -> u8 {
        (self.0[0] >> 2) & 0x07
    }

    /// The free-water fraction the shader sees, which is the quantised one.
    pub fn free(self) -> f32 {
        f32::from(self.0[1]) / 255.0
    }

    /// Whether the cell holds no water at all.
    pub fn dry(self) -> bool {
        self.0[1] == 0
    }

    pub fn pore(self) -> f32 {
        f32::from(self.0[2]) / 255.0
    }

    pub fn style(self) -> u8 {
        self.0[3]
    }
}

fn quantise(v: f32) -> u8 {
    if !v.is_finite() || v <= 0.0 {
        return 0;
    }
    (v.min(1.0) * 255.0 + 0.5) as u8
}

/// One plant style as the style texture carries it: wood, crown, heart, in linear light.
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct VoxelStyle {
    pub wood: [f32; 4],
    pub crown: [f32; 4],
    pub heart: [f32; 4],
}

impl VoxelStyle {
    pub fn new(wood: [f32; 3], crown: [f32; 3], heart: [f32; 3]) -> VoxelStyle {
        let v = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
        VoxelStyle { wood: v(wood), crown: v(crown), heart: v(heart) }
    }
}

/// Everything the picture is a function of besides the voxels: the projection, the
/// strata palette and every shading constant the CPU presenter names.
///
/// There are no defaults on purpose. The values live in
/// `crates/cubarium/src/voxel/present.rs`, which is the definition of the picture; a
/// second copy here with its own numbers is exactly how two renderers drift apart, so
/// the caller fills every field from that module's public constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoxelParams {
    /// Pixels per voxel edge.
    pub s: u32,
    /// Pixels a voxel of depth lifts the image.
    pub rise: u32,
    /// Row one past the bottom of the `y = 0, z = 0` front face.
    pub base: i32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub raster_w: u32,
    pub raster_h: u32,
    pub haze: f32,
    pub water_alpha: f32,
    /// Read the roof gap from the uploaded table (`true`) or walk the column in the
    /// shader (`false`). The picture is identical; only the cost differs.
    pub roof_from_texture: bool,
    pub sky: [f32; 3],
    pub bedrock: [f32; 3],
    pub rock: [f32; 3],
    pub soil: [f32; 3],
    pub water_deep: [f32; 3],
    pub water_surface: [f32; 3],
    pub light: [f32; 3],
    pub haze_colour: [f32; 3],
    pub top_gain: f32,
    pub top_tint: f32,
    pub top_back: f32,
    pub rim: f32,
    pub edge_dark: f32,
    pub top_edge: f32,
    pub riser_lean: f32,
    pub wet: f32,
    pub roof_light: f32,
    pub roof_falloff: f32,
    pub skin_alpha_gain: f32,
    pub water_top_alpha: f32,
    pub plant_top_gain: f32,
    pub plant_top_tint: f32,
    pub plant_rim: f32,
    pub crown_edge: f32,
    pub crown_under: f32,
    pub trunk_shade: [f32; 2],
    pub trunk_light_at: f32,
}

impl VoxelParams {
    fn validate(&self) -> Result<()> {
        if self.s == 0 || self.rise == 0 || self.rise > self.s {
            bail!("px_per_voxel {} and rise {} are not a projection", self.s, self.rise);
        }
        if self.width == 0 || self.height == 0 || self.depth == 0 {
            bail!("an empty world: {}x{}x{}", self.width, self.height, self.depth);
        }
        if self.raster_w != self.width * self.s {
            bail!(
                "the raster is {} px wide but {} voxels at {} px each is {}",
                self.raster_w,
                self.width,
                self.s,
                self.width * self.s
            );
        }
        if self.raster_h == 0 {
            bail!("a zero-height raster");
        }
        Ok(())
    }

    /// Voxels in the world: one texel each.
    pub fn voxel_count(&self) -> usize {
        self.width as usize * self.height as usize * self.depth as usize
    }

    /// Bytes one tick uploads: the voxel texture, the roof table and the style table.
    pub fn upload_bytes(&self) -> usize {
        self.voxel_count() * 5 + MAX_STYLES * std::mem::size_of::<VoxelStyle>()
    }

    /// The uniform block, in the layout `voxel.frag` declares.
    fn uniforms(&self) -> VoxelUniforms {
        let v = |c: [f32; 3]| [c[0], c[1], c[2], 0.0];
        VoxelUniforms {
            geom: [self.s as i32, self.rise as i32, self.base, i32::from(self.roof_from_texture)],
            extent: [self.width as i32, self.height as i32, self.depth as i32, 0],
            knobs: [self.haze, self.water_alpha, 0.0, 0.0],
            sky: v(self.sky),
            bedrock: v(self.bedrock),
            rock: v(self.rock),
            soil: v(self.soil),
            water_deep: v(self.water_deep),
            water_surface: v(self.water_surface),
            light: v(self.light),
            haze_colour: v(self.haze_colour),
            shade_a: [self.top_gain, self.top_tint, self.top_back, self.rim],
            shade_b: [self.edge_dark, self.top_edge, self.riser_lean, self.wet],
            roof: [self.roof_light, self.roof_falloff, 0.0, 0.0],
            water: [self.skin_alpha_gain, self.water_top_alpha, 0.0, 0.0],
            plant_a: [self.plant_top_gain, self.plant_top_tint, self.plant_rim, self.crown_edge],
            plant_b: [
                self.crown_under,
                self.trunk_shade[0],
                self.trunk_shade[1],
                self.trunk_light_at,
            ],
        }
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct VoxelUniforms {
    geom: [i32; 4],
    extent: [i32; 4],
    knobs: [f32; 4],
    sky: [f32; 4],
    bedrock: [f32; 4],
    rock: [f32; 4],
    soil: [f32; 4],
    water_deep: [f32; 4],
    water_surface: [f32; 4],
    light: [f32; 4],
    haze_colour: [f32; 4],
    shade_a: [f32; 4],
    shade_b: [f32; 4],
    roof: [f32; 4],
    water: [f32; 4],
    plant_a: [f32; 4],
    plant_b: [f32; 4],
}

/// Where one tick's world is written, straight into mapped memory.
///
/// The three slices are the staging buffer itself, so the packer on the CPU writes the
/// voxels once instead of filling a `Vec` and copying it. Voxels and the roof table are
/// indexed `(z · height + y) · width + x` — texture upload order, not the core's
/// `Config::index`.
pub struct VoxelStaging<'a> {
    pub voxels: &'a mut [VoxelTexel],
    pub roof: &'a mut [u8],
    pub styles: &'a mut [VoxelStyle],
}

impl VoxelStaging<'_> {
    /// The index of `(x, y, z)` in [`VoxelStaging::voxels`] and [`VoxelStaging::roof`].
    #[inline]
    pub fn index(width: u32, height: u32, x: u32, y: u32, z: u32) -> usize {
        (z as usize * height as usize + y as usize) * width as usize + x as usize
    }
}

/// The voxel strip's renderer: one texture uploaded per tick, one full-screen draw per
/// frame, and the shared [`PresentPass`] onto a target.
pub struct VoxelRenderer {
    params: VoxelParams,
    // --- the world raster ---
    raster_image: vk::Image,
    raster_memory: vk::DeviceMemory,
    raster_view: vk::ImageView,
    raster_framebuffer: vk::Framebuffer,
    raster_pass: vk::RenderPass,
    // --- the uploaded world ---
    voxel_image: vk::Image,
    voxel_memory: vk::DeviceMemory,
    voxel_view: vk::ImageView,
    roof_image: vk::Image,
    roof_memory: vk::DeviceMemory,
    roof_view: vk::ImageView,
    style_image: vk::Image,
    style_memory: vk::DeviceMemory,
    style_view: vk::ImageView,
    staging: HostBuffer,
    /// Byte offsets into [`VoxelRenderer::staging`] of the three planes.
    offsets: (u64, u64, u64),
    /// Whether the staging buffer holds a world the GPU has not seen yet.
    dirty: bool,
    /// Whether anything has ever been staged: a frame before the first upload would
    /// sample undefined texels, so it is refused rather than drawn.
    staged: bool,
    uniforms: HostBuffer,
    nearest: vk::Sampler,
    set_layout: vk::DescriptorSetLayout,
    set: vk::DescriptorSet,
    pool: vk::DescriptorPool,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    present: PresentPass,
    pub command_pool: vk::CommandPool,
    queries: vk::QueryPool,
}

impl VoxelRenderer {
    /// Build the pass and allocate every texture for a world of this shape. One call per
    /// process; the world's *contents* arrive through [`VoxelRenderer::stage`].
    pub fn new(gpu: &Gpu, params: VoxelParams) -> Result<VoxelRenderer> {
        params.validate()?;
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

        let raster_pass = crate::render::colour_pass(
            d,
            RASTER_FORMAT,
            vk::AttachmentLoadOp::DONT_CARE,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;
        let (raster_image, raster_memory) = gpu.image(
            params.raster_w,
            params.raster_h,
            RASTER_FORMAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_SRC,
        )?;
        let raster_view = gpu.view(raster_image, RASTER_FORMAT)?;
        let raster_framebuffer =
            framebuffer(d, raster_pass, raster_view, params.raster_w, params.raster_h)?;

        let limit = unsafe { gpu.instance.get_physical_device_properties(gpu.pdev) }
            .limits
            .max_image_dimension3_d;
        let side = params.width.max(params.height).max(params.depth);
        if side > limit {
            bail!(
                "a {}x{}x{} world needs a 3D image of side {side}; this device allows {limit}",
                params.width,
                params.height,
                params.depth
            );
        }
        let (voxel_image, voxel_memory) = image_3d(
            gpu,
            params.width,
            params.height,
            params.depth,
            vk::Format::R8G8B8A8_UINT,
        )?;
        let (roof_image, roof_memory) =
            image_3d(gpu, params.width, params.height, params.depth, vk::Format::R8_UINT)?;
        let (style_image, style_memory) = gpu.image(
            3,
            MAX_STYLES as u32,
            vk::Format::R32G32B32A32_SFLOAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        )?;
        gpu.one_shot(command_pool, |cb| unsafe {
            for image in [voxel_image, roof_image, style_image] {
                barrier(
                    d,
                    cb,
                    image,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );
            }
        })?;
        let voxel_view = view_3d(gpu, voxel_image, vk::Format::R8G8B8A8_UINT)?;
        let roof_view = view_3d(gpu, roof_image, vk::Format::R8_UINT)?;
        let style_view = gpu.view(style_image, vk::Format::R32G32B32A32_SFLOAT)?;

        let n = params.voxel_count();
        let voxel_bytes = (n * std::mem::size_of::<VoxelTexel>()) as u64;
        let roof_bytes = n as u64;
        let style_bytes = (MAX_STYLES * std::mem::size_of::<VoxelStyle>()) as u64;
        // Each plane starts on a 16-byte boundary: `vkCmdCopyBufferToImage` wants the
        // offset to be a multiple of the texel size, and the style texels are 16 bytes.
        let offsets = (0, align16(voxel_bytes), align16(voxel_bytes) + align16(roof_bytes));
        let staging = gpu.host_buffer(offsets.2 + style_bytes, vk::BufferUsageFlags::TRANSFER_SRC)?;
        let uniforms = gpu.host_buffer(
            std::mem::size_of::<VoxelUniforms>() as u64,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
        )?;
        uniforms.write(&[params.uniforms()]);

        let nearest = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    // The strip wraps in x and has real faces in y and z; every fetch is
                    // a `texelFetch` with the wrap done by hand, so these say what the
                    // topology is rather than doing anything.
                    .address_mode_u(vk::SamplerAddressMode::REPEAT)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )
        }?;

        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            sampled(1),
            sampled(2),
            sampled(3),
        ];
        let set_layout = unsafe {
            d.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }?;
        let sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(3),
        ];
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
        let buffer_info = [vk::DescriptorBufferInfo::default()
            .buffer(uniforms.buffer)
            .range(std::mem::size_of::<VoxelUniforms>() as u64)];
        let image_info = |view: vk::ImageView| {
            [vk::DescriptorImageInfo::default()
                .sampler(nearest)
                .image_view(view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]
        };
        let (iv, ir, is) = (image_info(voxel_view), image_info(roof_view), image_info(style_view));
        unsafe {
            d.update_descriptor_sets(
                &[
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(&buffer_info),
                    sampled_write(set, 1, &iv),
                    sampled_write(set, 2, &ir),
                    sampled_write(set, 3, &is),
                ],
                &[],
            )
        };

        let pipeline_layout = unsafe {
            d.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts),
                None,
            )
        }?;
        let vs = gpu.shader(FULLSCREEN_VERT)?;
        let fs = gpu.shader(VOXEL_FRAG).context("the voxel fragment shader")?;
        let pipeline =
            crate::render::fullscreen_pipeline(d, raster_pass, pipeline_layout, vs, fs, false)?;
        unsafe {
            d.destroy_shader_module(vs, None);
            d.destroy_shader_module(fs, None);
        }

        Ok(VoxelRenderer {
            params,
            raster_image,
            raster_memory,
            raster_view,
            raster_framebuffer,
            raster_pass,
            voxel_image,
            voxel_memory,
            voxel_view,
            roof_image,
            roof_memory,
            roof_view,
            style_image,
            style_memory,
            style_view,
            staging,
            offsets,
            dirty: false,
            staged: false,
            uniforms,
            nearest,
            set_layout,
            set,
            pool,
            pipeline_layout,
            pipeline,
            present: PresentPass::new(gpu, raster_view, nearest)?,
            command_pool,
            queries,
        })
    }

    pub fn params(&self) -> VoxelParams {
        self.params
    }

    /// Change the shading knobs an operator can move — the haze, the water opacity and
    /// which way the roof gap is read — without rebuilding anything. The projection and
    /// the world's extent are fixed at construction and are refused here.
    pub fn set_params(&mut self, params: VoxelParams) -> Result<()> {
        let fixed = |p: &VoxelParams| {
            (p.s, p.rise, p.base, p.width, p.height, p.depth, p.raster_w, p.raster_h)
        };
        if fixed(&params) != fixed(&self.params) {
            bail!("the projection and the world's extent are fixed for a VoxelRenderer");
        }
        self.params = params;
        self.uniforms.write(&[params.uniforms()]);
        Ok(())
    }

    /// Write one tick's world straight into the staging buffer.
    ///
    /// The styles slice is zeroed first, so a frame with fewer stands than the last one
    /// cannot paint with a stale colour. The voxels and the roof table are not: every
    /// texel is written every tick, and clearing 720 KiB to then overwrite it is 720 KiB
    /// of memory traffic nobody reads.
    pub fn stage(&mut self, fill: impl FnOnce(VoxelStaging<'_>)) {
        let n = self.params.voxel_count();
        let (voxels, roof, styles) = unsafe {
            (
                std::slice::from_raw_parts_mut(
                    self.staging.ptr.add(self.offsets.0 as usize) as *mut VoxelTexel,
                    n,
                ),
                std::slice::from_raw_parts_mut(self.staging.ptr.add(self.offsets.1 as usize), n),
                std::slice::from_raw_parts_mut(
                    self.staging.ptr.add(self.offsets.2 as usize) as *mut VoxelStyle,
                    MAX_STYLES,
                ),
            )
        };
        styles.fill(VoxelStyle::default());
        fill(VoxelStaging { voxels, roof, styles });
        self.dirty = true;
        self.staged = true;
    }

    /// The world raster, for a readback or a blit.
    pub fn raster_image(&self) -> vk::Image {
        self.raster_image
    }

    /// Record the frame: the three uploads if the world moved, the slab-walk pass into
    /// the world raster, then — if a target is given — the present pass onto it.
    pub fn record(
        &mut self,
        gpu: &Gpu,
        cb: vk::CommandBuffer,
        target: Option<TargetSlot<'_>>,
    ) -> Result<()> {
        if !self.staged {
            bail!("VoxelRenderer::record before the first stage: there is no world to draw");
        }
        let d = &gpu.device;
        let target = target
            .map(|(image, extent, format, layout, xform)| {
                self.present
                    .entry(gpu, format, layout)
                    .map(|(pass, pipeline)| (image, extent, xform, pass, pipeline))
            })
            .transpose()?;
        let (w, h, dd) = (self.params.width, self.params.height, self.params.depth);
        let upload = self.dirty;
        self.dirty = false;

        unsafe {
            d.begin_command_buffer(
                cb,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            d.cmd_reset_query_pool(cb, self.queries, 0, QUERY_SLOTS);
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::TOP_OF_PIPE, self.queries, 0);

            if upload {
                let planes = [
                    (self.voxel_image, self.offsets.0, w, h, dd),
                    (self.roof_image, self.offsets.1, w, h, dd),
                    (self.style_image, self.offsets.2, 3, MAX_STYLES as u32, 1),
                ];
                for (image, offset, pw, ph, pd) in planes {
                    barrier(
                        d,
                        cb,
                        image,
                        vk::ImageLayout::UNDEFINED,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    );
                    d.cmd_copy_buffer_to_image(
                        cb,
                        self.staging.buffer,
                        image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[vk::BufferImageCopy::default()
                            .buffer_offset(offset)
                            .image_subresource(
                                vk::ImageSubresourceLayers::default()
                                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                                    .layer_count(1),
                            )
                            .image_extent(vk::Extent3D { width: pw, height: ph, depth: pd })],
                    );
                    barrier(
                        d,
                        cb,
                        image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    );
                }
            }
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, 1);

            crate::render::begin(
                d,
                cb,
                self.raster_pass,
                self.raster_framebuffer,
                self.params.raster_w,
                self.params.raster_h,
            );
            d.cmd_bind_descriptor_sets(
                cb,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.set],
                &[],
            );
            d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            d.cmd_draw(cb, 3, 1, 0, 0);
            d.cmd_end_render_pass(cb);
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, 2);

            if let Some((image, extent, xform, pass, pipeline)) = target {
                self.present.record(d, cb, image, extent, xform, pass, pipeline);
            }
            d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, 3);
            d.end_command_buffer(cb)?;
        }
        Ok(())
    }

    /// The frame's three GPU stages in milliseconds — uploads, the slab-walk pass, the
    /// present pass — or `None` if the queries are not ready.
    pub fn gpu_split(&self, gpu: &Gpu) -> Option<[f64; 3]> {
        let mut ts = [0u64; QUERY_SLOTS as usize];
        if unsafe {
            gpu.device
                .get_query_pool_results(self.queries, 0, &mut ts, vk::QueryResultFlags::TYPE_64)
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

    /// Copy the world raster into a host buffer, `w · 4` bytes per row.
    pub fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>> {
        let (w, h) = (self.params.raster_w, self.params.raster_h);
        let size = u64::from(w) * u64::from(h) * 4;
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
                    .buffer_row_length(w)
                    .buffer_image_height(h)
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D { width: w, height: h, depth: 1 })],
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

    /// Release everything. The device must be idle.
    pub fn destroy(&mut self, gpu: &Gpu) {
        self.present.destroy(gpu);
        let d = &gpu.device;
        unsafe {
            d.destroy_pipeline(self.pipeline, None);
            d.destroy_pipeline_layout(self.pipeline_layout, None);
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_sampler(self.nearest, None);
            self.staging.destroy(gpu);
            self.uniforms.destroy(gpu);
            for (view, image, memory) in [
                (self.voxel_view, self.voxel_image, self.voxel_memory),
                (self.roof_view, self.roof_image, self.roof_memory),
                (self.style_view, self.style_image, self.style_memory),
            ] {
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(memory, None);
            }
            d.destroy_framebuffer(self.raster_framebuffer, None);
            d.destroy_image_view(self.raster_view, None);
            d.destroy_image(self.raster_image, None);
            d.free_memory(self.raster_memory, None);
            d.destroy_render_pass(self.raster_pass, None);
            d.destroy_query_pool(self.queries, None);
            d.destroy_command_pool(self.command_pool, None);
        }
    }
}

/// The voxel renderer as a target's frame source. One frame carries nothing: the world
/// arrived with the tick's [`VoxelRenderer::stage`], and a frame is one draw over it.
impl FrameSource for VoxelRenderer {
    type Frame<'a> = ();

    fn raster_size(&self) -> (u32, u32) {
        (self.params.raster_w, self.params.raster_h)
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
        self.present.pass(gpu, format, final_layout)
    }

    fn record_frame(
        &mut self,
        gpu: &Gpu,
        cb: vk::CommandBuffer,
        _frame: (),
        target: Option<TargetSlot<'_>>,
    ) -> Result<()> {
        VoxelRenderer::record(self, gpu, cb, target)
    }

    fn gpu_ms(&self, gpu: &Gpu) -> f64 {
        self.gpu_split(gpu).map_or(f64::NAN, |s| s[0] + s[1] + s[2])
    }

    fn read_raster(&self, gpu: &Gpu) -> Result<Vec<u8>> {
        VoxelRenderer::read_raster(self, gpu)
    }
}

// --- the shader's arithmetic, in Rust -------------------------------------------------

/// Which voxel level and which row inside it a raster pixel falls on, in one slab.
///
/// This is the shader's own first three lines, and the reason it is here in Rust is that
/// it is the whole of the projection inversion: everything else in `voxel.frag` is a
/// colour rule over neighbours. Writing `A = base − z·rise − sy` and `q = A − 1`, the
/// pixel sits in the band of voxel `level = ⌊q / s⌋` at `r = q − level·s`, where `r`
/// counts rows **up** from the bottom of that band. Two faces can own it:
///
/// - the **front** face of `(x, level, z)`, at row `s − 1 − r` of that face;
/// - when `r < rise`, the **top** face of `(x, level − 1, z)`, at cap row `rise − 1 − r`
///   — the cap of the voxel below occupies the bottom `rise` rows of the band above it,
///   which is exactly why the CPU presenter draws a cap only where the voxel above is
///   air.
///
/// `None` means the pixel is below this slab's `y = 0` front face, and since `q` falls by
/// `rise` per slab, every deeper slab is below it too: the walk stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlabHit {
    /// The voxel whose front face owns the pixel; may be `>= height`, meaning the pixel
    /// is above this slab's column and only a cap can own it.
    pub level: i32,
    /// Rows up from the bottom of that band, `0 .. s`.
    pub r: u32,
}

impl SlabHit {
    /// Row of the front face of [`SlabHit::level`] this pixel is, `0` at the top.
    pub fn front_row(self, s: u32) -> u32 {
        s - 1 - self.r
    }

    /// Row of the top face of `level − 1` this pixel is, `0` at the top, or `None` when
    /// no cap reaches it.
    pub fn cap_row(self, rise: u32) -> Option<u32> {
        (self.r < rise).then(|| rise - 1 - self.r)
    }
}

/// The slab lookup: which voxel level and row owns raster pixel `sy` in slab `z`.
pub fn slab_hit(params: &VoxelParams, sy: i32, z: u32) -> Option<SlabHit> {
    let q = params.base - (z * params.rise) as i32 - sy - 1;
    if q < 0 {
        return None;
    }
    let s = params.s as i32;
    Some(SlabHit { level: q / s, r: (q % s) as u32 })
}

// --- helpers --------------------------------------------------------------------------

fn align16(n: u64) -> u64 {
    (n + 15) & !15
}

fn image_3d(
    gpu: &Gpu,
    width: u32,
    height: u32,
    depth: u32,
    format: vk::Format,
) -> Result<(vk::Image, vk::DeviceMemory)> {
    let d = &gpu.device;
    let image = unsafe {
        d.create_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_3D)
                .format(format)
                .extent(vk::Extent3D { width, height, depth })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED),
            None,
        )
    }?;
    let req = unsafe { d.get_image_memory_requirements(image) };
    let memory = unsafe {
        d.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(req.size)
                .memory_type_index(
                    gpu.memory_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)?,
                ),
            None,
        )
    }?;
    unsafe { d.bind_image_memory(image, memory, 0) }?;
    Ok((image, memory))
}

fn view_3d(gpu: &Gpu, image: vk::Image, format: vk::Format) -> Result<vk::ImageView> {
    Ok(unsafe {
        gpu.device.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_3D)
                .format(format)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1),
                ),
            None,
        )
    }?)
}

fn sampled(binding: u32) -> vk::DescriptorSetLayoutBinding<'static> {
    vk::DescriptorSetLayoutBinding::default()
        .binding(binding)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
}

fn sampled_write<'a>(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> VoxelParams {
        VoxelParams {
            s: 4,
            rise: 2,
            base: 240,
            width: 128,
            height: 48,
            depth: 24,
            raster_w: 512,
            raster_h: 240,
            haze: 0.55,
            water_alpha: 0.5,
            roof_from_texture: true,
            sky: [0.0; 3],
            bedrock: [0.0; 3],
            rock: [0.0; 3],
            soil: [0.0; 3],
            water_deep: [0.0; 3],
            water_surface: [0.0; 3],
            light: [0.0; 3],
            haze_colour: [0.0; 3],
            top_gain: 2.4,
            top_tint: 0.2,
            top_back: 0.3,
            rim: 0.38,
            edge_dark: 0.72,
            top_edge: 0.18,
            riser_lean: 0.45,
            wet: 0.55,
            roof_light: 0.22,
            roof_falloff: 4.0,
            skin_alpha_gain: 1.7,
            water_top_alpha: 0.8,
            plant_top_gain: 1.5,
            plant_top_tint: 0.14,
            plant_rim: 0.42,
            crown_edge: 0.7,
            crown_under: 0.22,
            trunk_shade: [0.62, 1.22],
            trunk_light_at: 0.35,
        }
    }

    /// Any water at all packs to a non-zero fraction, so the presenter's "at least one
    /// row of water is visible" survives the quantisation; a dry cell packs to zero.
    #[test]
    fn the_texel_packing_round_trips_a_voxel() {
        let t = VoxelTexel::pack(3, PART_TRUNK, 0.0, true, 0.75, 9);
        assert_eq!((t.material(), t.part(), t.style()), (3, PART_TRUNK, 9));
        assert!(t.dry() && t.free() == 0.0);
        assert!((t.pore() - 0.75).abs() < 1.0 / 255.0);

        let film = VoxelTexel::pack(0, PART_NONE, 1e-3, false, 0.0, 0);
        assert!(!film.dry(), "a film of water must not pack to dry");
        assert!(film.free() > 0.0);

        for &free in &[0.125f32, 0.375, 0.5, 0.875, 1.0] {
            let t = VoxelTexel::pack(0, PART_NONE, free, false, 0.0, 0);
            // Half a step of the 8-bit channel: the quantisation's whole error.
            assert!((t.free() - free).abs() <= 1.0 / 509.0, "{free} -> {}", t.free());
        }
        // Every part class survives beside a full material and a top style index.
        for part in [PART_NONE, PART_TRUNK, PART_CROWN, PART_CROWN_HEART, PART_SPROUT] {
            let t = VoxelTexel::pack(2, part, 1.0, false, 1.0, 255);
            assert_eq!((t.material(), t.part(), t.style()), (2, part, 255));
            assert_eq!((t.free(), t.pore()), (1.0, 1.0));
        }
    }

    /// The slab lookup against the projection it inverts: the bottom-left voxel's front
    /// face is the bottom `s` rows of column 0, its cap the `rise` rows above, and the
    /// cap of a voxel is the bottom `rise` rows of the band above it.
    #[test]
    fn the_slab_lookup_inverts_the_projection() {
        let p = params();
        // `front_row(0, 0) = 240 - 4 = 236`, so rows 236..240 are voxel 0's front face.
        for (sy, want_row) in [(236, 0), (237, 1), (238, 2), (239, 3)] {
            let hit = slab_hit(&p, sy, 0).expect("inside the slab");
            assert_eq!(hit.level, 0, "sy = {sy}");
            assert_eq!(hit.front_row(p.s), want_row);
        }
        // The two rows above are voxel 0's cap, which is the bottom of voxel 1's band.
        for (sy, want_cap) in [(234, 0), (235, 1)] {
            let hit = slab_hit(&p, sy, 0).unwrap();
            assert_eq!(hit.level, 1, "sy = {sy}");
            assert_eq!(hit.cap_row(p.rise), Some(want_cap));
        }
        // And the two rows above *those* are voxel 1's front face with no cap over them.
        for sy in [232, 233] {
            let hit = slab_hit(&p, sy, 0).unwrap();
            assert_eq!(hit.level, 1);
            assert_eq!(hit.cap_row(p.rise), None);
        }
        // One slab back lifts everything by `rise`.
        assert_eq!(slab_hit(&p, 236 - 2, 1).unwrap(), slab_hit(&p, 236, 0).unwrap());
        // Below the floor line there is nothing, in this slab or any deeper one.
        assert_eq!(slab_hit(&p, 240, 0), None);
    }

    #[test]
    fn a_params_whose_raster_does_not_match_the_world_is_refused() {
        let mut p = params();
        p.raster_w = 500;
        assert!(p.validate().is_err());
        let mut p = params();
        p.rise = 5;
        assert!(p.validate().is_err());
        assert_eq!(params().voxel_count(), 128 * 48 * 24);
        assert_eq!(params().upload_bytes(), 128 * 48 * 24 * 5 + 256 * 48);
    }
}
