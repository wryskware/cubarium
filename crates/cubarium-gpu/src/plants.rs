//! **The plant pass** (`[figures] plants = "models"`, the last bullet of H2 in
//! `design/handoffs/hybrid-renderer-2026-10-07.md`): plants drawn as models so they can sway
//! in a breeze and grow in voxel by voxel, in the same pass as the animals.
//!
//! # Why a layer and not the figures' atlas
//!
//! A world holds thousands of plants (6 619 stands on the wide 640 × 128 × 96 save, 5 488 of
//! them with block cells) but few cells (31 592 there; 7 645 on the 256 world). The figures'
//! atlas has 2 048 tile slots and re-rasterises every tile every frame; plants do not move
//! between frames except by sway, and never translate. So every plant cell is one instance
//! drawn straight into one raster-aligned layer: no slots, no packing, the same pass for a
//! turf tuft and a vaulttree, and a cell at rest lands exactly on the pixels the walk drew
//! it on.
//!
//! # The three steps
//!
//! - **The walk** (`voxel.frag`, `PLANT_MODELS`) lets its ray through every dry block plant
//!   cell. The cells stay in the volume, so the sun march, the sky plane, the canopy and
//!   the AO see them: plants cast and receive light and shadow exactly as before.
//! - **Visibility** (`plant.vert`, `voxel.frag` with `PLANT_VIS`): each cell's front face
//!   and cap, moved and scaled, into an R32 id layer at `supersample` samples a raster pixel
//!   axis, depth tested. An id names the cell, the face and the stamped face's texel.
//! - **Shade** (`voxel.frag` with `PLANT_SHADE`), a full-screen pass before the figures:
//!   each raster pixel lights the distinct texels its samples show with the walk's own
//!   `glyphFrontAt` / `glyphCapAt` at the cell's resting place, blends them by coverage
//!   over the raster where they are in front of the walk's end depth, and writes the
//!   **occluder**: the walk's depth, or the plants' nearest where they are in front. The
//!   figures test their texels against it ([`crate::figures::FigurePass::set_occluder`]),
//!   so an animal meets a plant texel by texel, as it met the stamped plant in the walk.
//!
//! # Motion
//!
//! - **Sway**: a cell moves sideways by its plant's tip ([`Breeze`], evaluated here once a
//!   plant a frame) times its weight, `(height / plant height)^p`: the root never moves.
//! - **Grow-in**: a cell arriving or leaving is drawn scaled about its own centre, from its
//!   own moment over [`PlantMotion::grow_s`]; the host staggers the moments along the plant.
//!
//! Every `unsafe` block below holds one `ash` call and nothing else (WORKING_POLICY,
//! "No unsafe code").

use anyhow::{Context, Result};
use ash::vk;

use crate::figures::{Plane, over, pipeline, plain};
use crate::vk::{Gpu, HostBuffer};

const PLANT_VERT: &[u8] = include_bytes!("../shaders/plant.vert.spv");
const PLANT_VIS_FRAG: &[u8] = include_bytes!("../shaders/plant_vis.frag.spv");
const PLANT_SHADE_FRAG: &[u8] = include_bytes!("../shaders/plant_shade.frag.spv");
const FULLSCREEN_VERT: &[u8] = include_bytes!("../shaders/fullscreen.vert.spv");

/// The most plant cells (settled, arriving and leaving) a frame draws.
pub const MAX_PLANT_CELLS: usize = 1 << 18;
/// The most plants a cell list names (a cell's plant field has 19 bits).
pub const MAX_PLANTS: usize = 1 << 16;
/// The most layer samples a raster pixel axis (`plant_main.glsl`'s `PLANT_SAMPLES`).
pub const MAX_SUPERSAMPLE: u32 = 4;
/// The largest voxel a layer id can name a texel of (six bits each for column and row).
pub const MAX_PX_PER_VOXEL: u32 = 64;

const IDS: vk::Format = vk::Format::R32_UINT;

/// The breeze plants sway in: one gust packet a period travelling round the ring, a
/// flutter inside it, and a slow change in how strong each packet peaks. Presentation
/// only. Modelled on the old renderer's shared breeze (`art_present::wind`): the packet
/// eases in with zero slope, holds, eases out, and then is **exactly** calm for the rest of
/// the period, so a plant rests and then stirs.
///
/// **The ring**: a point's place round it is `ring = x / width` in `[0, 1)`. The packet
/// reaches it `sweeps · period_s · ring` seconds late and the flutter runs `flutter_waves`
/// whole waves round it, both whole numbers, so the breeze at `ring = 1` is the breeze at
/// `ring = 0` and nothing jumps at the seam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breeze {
    /// Seconds from one packet's start to the next.
    pub period_s: f64,
    /// Seconds a packet eases in over, holds, and eases out over; the rest is calm.
    pub rise_s: f64,
    pub hold_s: f64,
    pub fall_s: f64,
    /// How deep the flutter dips a packet (its factor runs over `[1 − flutter, 1]`), and
    /// its period in seconds.
    pub flutter: f64,
    pub flutter_s: f64,
    /// How far a packet's peak may fall below 1 (`[1 − peak_vary, 1]`), on this period.
    pub peak_vary: f64,
    pub peak_s: f64,
    /// Packet periods the gust front takes to go once round the ring (0: everywhere at
    /// once); its sign is the way it travels, +x for positive.
    pub sweeps: i32,
    /// Flutter waves round the ring.
    pub flutter_waves: i32,
    /// What a shower and a downpour add to the strength (each times how much of it is
    /// falling, eased with the weather).
    pub shower_gain: f64,
    pub storm_gain: f64,
}

impl Default for Breeze {
    fn default() -> Breeze {
        Breeze {
            period_s: 30.0,
            rise_s: 5.0,
            hold_s: 8.0,
            fall_s: 5.0,
            flutter: 0.3,
            flutter_s: 2.3,
            peak_vary: 0.15,
            peak_s: 97.0,
            sweeps: 1,
            flutter_waves: 8,
            shower_gain: 0.25,
            storm_gain: 0.75,
        }
    }
}

/// The clamped Hermite smoothstep `t²(3 − 2t)`.
fn hermite(t: f64) -> f64 {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    t * t * (3.0 - 2.0 * t)
}

impl Breeze {
    /// The packet's envelope at `when`: 0..1, exactly 0 in the calm.
    pub fn envelope(&self, when: f64) -> f64 {
        if !(when.is_finite() && self.period_s > 0.0) {
            return 0.0;
        }
        let u = when.rem_euclid(self.period_s);
        let (rise, hold, fall) = (self.rise_s.max(0.0), self.hold_s.max(0.0), self.fall_s.max(0.0));
        if u < rise {
            hermite(u / rise)
        } else if u < rise + hold {
            1.0
        } else if u < rise + hold + fall {
            1.0 - hermite((u - rise - hold) / fall)
        } else {
            0.0
        }
    }

    /// How hard the breeze blows at `ring` (`x / width`, any real: it is taken round the
    /// ring) at `seconds`, with `rain` (drizzle, shower, downpour) falling: 0 in the calm,
    /// 1 at a dry packet's highest peak.
    pub fn strength(&self, ring: f64, seconds: f64, rain: [f32; 3]) -> f64 {
        if !(ring.is_finite() && seconds.is_finite()) {
            return 0.0;
        }
        let ring = ring.rem_euclid(1.0);
        let envelope = self.envelope(seconds - f64::from(self.sweeps) * self.period_s * ring);
        if envelope <= 0.0 {
            return 0.0;
        }
        let tau = std::f64::consts::TAU;
        let wave = |phase: f64, depth: f64| 1.0 - depth + depth * 0.5 * (1.0 + (tau * phase).sin());
        let flutter = if self.flutter_s > 0.0 {
            wave(seconds / self.flutter_s - f64::from(self.flutter_waves) * ring, self.flutter.clamp(0.0, 1.0))
        } else {
            1.0
        };
        let peak = if self.peak_s > 0.0 {
            wave(seconds / self.peak_s, self.peak_vary.clamp(0.0, 1.0))
        } else {
            1.0
        };
        let rain = 1.0 + self.shower_gain * f64::from(rain[1]) + self.storm_gain * f64::from(rain[2]);
        let s = envelope * flutter * peak * rain.max(0.0);
        if s.is_finite() { s.max(0.0) } else { 0.0 }
    }
}

/// The plant pass's settings, fixed for a renderer's life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlantMotion {
    /// Layer samples per raster pixel axis, `1..=`[`MAX_SUPERSAMPLE`].
    pub supersample: u32,
    /// Seconds one voxel takes to grow in from nothing, or shrink out.
    pub grow_s: f32,
    pub breeze: Breeze,
}

impl Default for PlantMotion {
    fn default() -> PlantMotion {
        PlantMotion { supersample: 2, grow_s: 0.35, breeze: Breeze::default() }
    }
}

/// One plant of a cell list: where it stands round the ring (its root's column, voxels),
/// how far its top moves at full breeze (voxels, signed: + is +x) and how many seconds it
/// answers the breeze late. Plant 0 of every list is the still plant (no amplitude): the
/// cells no stand owns.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlantRec {
    pub x: f32,
    pub amp: f32,
    pub lag: f32,
}

/// A tick's plant cells, as the pack writes them ([`crate::voxel::VoxelStaging::plants`]):
/// each one [`pack_cell`]'s words, the plants they name, and the clock second their times
/// count from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlantList {
    pub cells: Vec<[u32; 4]>,
    pub plants: Vec<PlantRec>,
    /// Seconds of the frame clock (sim seconds) that a cell's time 0 is.
    pub epoch: f64,
    /// Frame-clock seconds at which every cell of the list has finished arriving or
    /// leaving: the picture moves on its own until then.
    pub settled_at: f64,
}

/// A cell's time while it is neither arriving nor leaving: long grown.
pub const SETTLED: f32 = -1.0e30;

/// One plant cell as `plant.glsl` reads it: voxel `(x, y, z)` (x round the ring), its
/// plant, how much of its plant's tip it moves by (`weight`, 0..1), whether it is leaving,
/// the moment it starts arriving or leaving (seconds from the list's epoch; [`SETTLED`]
/// for neither), and, leaving, the texel it was stamped with (`r | g << 8 | b << 16 | a << 24`:
/// it is no longer in the volume).
pub fn pack_cell(cell: [u32; 3], plant: u32, weight: f32, dying: bool, time: f32, texel: u32) -> [u32; 4] {
    let w = (weight.clamp(0.0, 1.0) * 4095.0).round() as u32;
    [
        (cell[0] & 4095) | (cell[1] & 1023) << 12 | (cell[2] & 1023) << 22,
        (plant & 0x7FFFF) | u32::from(dying) << 19 | w << 20,
        time.to_bits(),
        texel,
    ]
}

/// A settled or arriving cell's fourth word (in place of a leaving cell's texel): the
/// weights of its top and bottom faces, 12 bits each, and the top bit set to say so. The
/// plant pass moves a drape cord's joins by these ([`crate::cord`]), so two cells meeting at
/// a face, given the same weight for it, put the join at the same screen point. Bit 30 says
/// the cell is a drape cell, whose front quad `plant.vert` widens for the cord's long wave
/// ([`crate::cord::DrapeCords::reach`]).
pub fn pack_faces(top: f32, bottom: f32, drape: bool) -> u32 {
    let q = |w: f32| (w.clamp(0.0, 1.0) * 4095.0).round() as u32;
    1 << 31 | u32::from(drape) << 30 | q(top) << 12 | q(bottom)
}

/// The top and bottom face weights of [`pack_faces`], or `None` for a word without them.
pub fn unpack_faces(w: u32) -> Option<(f32, f32)> {
    (w >> 31 != 0).then(|| (((w >> 12) & 4095) as f32 / 4095.0, (w & 4095) as f32 / 4095.0))
}

/// The voxel, plant, weight, leaving flag and time of a packed cell ([`pack_cell`]).
pub fn unpack_cell(c: [u32; 4]) -> ([u32; 3], u32, f32, bool, f32) {
    (
        [c[0] & 4095, (c[0] >> 12) & 1023, c[0] >> 22],
        c[1] & 0x7FFFF,
        (c[1] >> 20) as f32 / 4095.0,
        c[1] & 0x80000 != 0,
        f32::from_bits(c[2]),
    )
}

/// How big a cell is drawn at `t` (seconds from the list's epoch), 0..1 about its centre:
/// `plant.glsl`'s `plantScale`. Arriving from `time`, it grows over `grow_s`; leaving from
/// `time`, it shrinks; both smoothstepped.
pub fn cell_scale(t: f32, time: f32, dying: bool, grow_s: f32) -> f32 {
    let since = t - time;
    let r = if grow_s > 0.0 {
        (since / grow_s).clamp(0.0, 1.0)
    } else if since >= 0.0 {
        1.0
    } else {
        0.0
    };
    let s = r * r * (3.0 - 2.0 * r);
    if dying { 1.0 - s } else { s }
}

/// The ramp position `r` in 0..1 whose smoothstep is `s`: what a reversed change starts
/// from so the scale goes on from where it was.
pub fn unsmooth(s: f32) -> f32 {
    let s = s.clamp(0.0, 1.0);
    0.5 - ((1.0 - 2.0 * s).asin() / 3.0).sin()
}

/// Each plant's tip this instant: `amp` times the breeze at its column, `lag` seconds late.
pub fn tips_into(plants: &[PlantRec], breeze: &Breeze, width: u32, seconds: f64, rain: [f32; 3], out: &mut Vec<f32>) {
    out.clear();
    let w = f64::from(width.max(1));
    out.extend(plants.iter().map(|p| {
        if p.amp == 0.0 {
            0.0
        } else {
            (f64::from(p.amp) * breeze.strength(f64::from(p.x) / w, seconds - f64::from(p.lag), rain)) as f32
        }
    }));
}

/// The push block (`plant.glsl`'s `PlantPush`).
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct PlantPush {
    geom: [i32; 4],
    k: [i32; 4],
    t: [f32; 4],
    layer: [f32; 4],
}

const CELL_BYTES: u64 = (MAX_PLANT_CELLS * 16) as u64;
const SLOT_BYTES: u64 = CELL_BYTES + (MAX_PLANTS * 4) as u64;

/// What the plant pass drew in a frame, for captures and diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlantStats {
    pub cells: usize,
    pub plants: usize,
    /// Cells arriving or leaving at the list's epoch.
    pub changing: usize,
    /// Cells a list had beyond [`MAX_PLANT_CELLS`] (not drawn).
    pub dropped: usize,
}

/// The plant pass's GPU side: the id and depth layer, the two pipelines, one cell buffer a
/// ring slot, and the cell lists the pack hands over.
pub struct PlantPass {
    motion: PlantMotion,
    aa: u32,
    layer: (u32, u32),
    raster: (u32, u32),
    world: (u32, u32, u32),
    ids: Plane,
    depth: Plane,
    vis_pass: vk::RenderPass,
    vis_fb: vk::Framebuffer,
    vis_pipeline: vk::Pipeline,
    occluder: Plane,
    shade_pass: vk::RenderPass,
    shade_fb: vk::Framebuffer,
    shade_pipeline: vk::Pipeline,
    frames: Vec<HostBuffer>,
    /// The drawn list's version each ring slot's buffer holds.
    written: Vec<u64>,
    set_layouts: [vk::DescriptorSetLayout; 2],
    pool: vk::DescriptorPool,
    data_sets: Vec<vk::DescriptorSet>,
    image_set: vk::DescriptorSet,
    sampler: vk::Sampler,
    layout: vk::PipelineLayout,
    /// The pack's list for each staging buffer, and the list the frames draw (the one the
    /// last uploading frame took), with its version.
    staged: Vec<PlantList>,
    drawn: PlantList,
    version: u64,
    /// This frame's tips, the clock they were taken at, and the tips last drawn.
    tips: Vec<f32>,
    seconds: f64,
    rain: [f32; 3],
    drawn_tips: Vec<f32>,
    stats: PlantStats,
}

impl PlantPass {
    /// The pass for a raster `raster` big over a `world`-sized strip: lit through the walk's
    /// set `walk_layout` with the walk's specialisation `constants`, blended over the raster
    /// `raster_view` (and the lit tier's emission `emit`) after reading the walk's depth
    /// `walk_depth`. `staging` lists, `slots` ring slots.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        gpu: &Gpu,
        motion: PlantMotion,
        raster: (u32, u32),
        world: (u32, u32, u32),
        walk_layout: vk::DescriptorSetLayout,
        constants: &[u32],
        (raster_format, raster_view): (vk::Format, vk::ImageView),
        emit: Option<(vk::Format, vk::ImageView)>,
        walk_depth: vk::ImageView,
        depth_format: vk::Format,
        staging: usize,
        slots: usize,
    ) -> Result<PlantPass> {
        let d = &gpu.device;
        let aa = motion.supersample.clamp(1, MAX_SUPERSAMPLE);
        let layer = (raster.0 * aa, raster.1 * aa);
        let ids = Plane::new(
            gpu,
            layer.0,
            layer.1,
            IDS,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
        )?;
        let depth = Plane::new(
            gpu,
            layer.0,
            layer.1,
            depth_format,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::DEPTH,
        )?;
        let atts = [
            vk::AttachmentDescription::default()
                .format(IDS)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
            vk::AttachmentDescription::default()
                .format(depth_format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
        ];
        let colour_ref = [vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let depth_ref = vk::AttachmentReference::default()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let sub = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&colour_ref)
            .depth_stencil_attachment(&depth_ref)];
        let vis_pass =
            unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&atts).subpasses(&sub), None) }?;
        let views = [ids.view, depth.view];
        let vis_fb = unsafe {
            d.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(vis_pass)
                    .attachments(&views)
                    .width(layer.0)
                    .height(layer.1)
                    .layers(1),
                None,
            )
        }?;

        // The shade: over the raster (and the emission), loaded and left for sampling, and
        // the occluder written whole.
        let occluder = Plane::new(
            gpu,
            raster.0,
            raster.1,
            crate::figures::WALK_DEPTH_FORMAT,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::ImageAspectFlags::COLOR,
        )?;
        let loaded = |format: vk::Format| {
            vk::AttachmentDescription::default()
                .format(format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::LOAD)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        };
        let mut shade_atts = vec![loaded(raster_format)];
        let mut shade_views = vec![raster_view];
        if let Some((f, v)) = emit {
            shade_atts.push(loaded(f));
            shade_views.push(v);
        }
        shade_atts.push(
            vk::AttachmentDescription::default()
                .format(crate::figures::WALK_DEPTH_FORMAT)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
        );
        shade_views.push(occluder.view);
        let at = |i: u32| vk::AttachmentReference::default().attachment(i).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        // `[raster, emit, occluder]`, or `[raster, -, occluder]` in the flat tier, as the
        // shader writes them.
        let shade_refs = if emit.is_some() {
            vec![at(0), at(1), at(2)]
        } else {
            vec![at(0), vk::AttachmentReference::default().attachment(vk::ATTACHMENT_UNUSED), at(1)]
        };
        let shade_sub = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&shade_refs)];
        let shade_pass = unsafe {
            d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&shade_atts).subpasses(&shade_sub), None)
        }?;
        let shade_fb = unsafe {
            d.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(shade_pass)
                    .attachments(&shade_views)
                    .width(raster.0)
                    .height(raster.1)
                    .layers(1),
                None,
            )
        }?;

        let storage = vk::BufferUsageFlags::STORAGE_BUFFER;
        let frames = (0..slots).map(|_| gpu.host_buffer(SLOT_BYTES, storage)).collect::<Result<Vec<_>>>()?;
        let both = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let ssbo = |b: u32| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(b)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(both)
        };
        let img = |b: u32| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(b)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        };
        let data_bindings = [ssbo(0), ssbo(1)];
        let image_bindings = [img(0), img(1), img(2)];
        let data_layout = unsafe {
            d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&data_bindings), None)
        }?;
        let image_layout = unsafe {
            d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&image_bindings), None)
        }?;
        let sizes = [
            vk::DescriptorPoolSize::default().ty(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(2 * slots as u32),
            vk::DescriptorPoolSize::default().ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(3),
        ];
        let pool = unsafe {
            d.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default().max_sets(slots as u32 + 1).pool_sizes(&sizes),
                None,
            )
        }?;
        let mut layouts = vec![data_layout; slots];
        layouts.push(image_layout);
        let mut sets = unsafe {
            d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))
        }?;
        let image_set = sets.pop().context("the plant pass's image set")?;
        let data_sets = sets;
        for (set, frame) in data_sets.iter().zip(&frames) {
            let infos = [
                [vk::DescriptorBufferInfo::default().buffer(frame.buffer).offset(0).range(CELL_BYTES)],
                [vk::DescriptorBufferInfo::default().buffer(frame.buffer).offset(CELL_BYTES).range(SLOT_BYTES - CELL_BYTES)],
            ];
            let writes: Vec<_> = infos
                .iter()
                .enumerate()
                .map(|(b, info)| {
                    vk::WriteDescriptorSet::default()
                        .dst_set(*set)
                        .dst_binding(b as u32)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(info)
                })
                .collect();
            unsafe { d.update_descriptor_sets(&writes, &[]) };
        }
        let sampler = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                None,
            )
        }?;
        let image_infos = [ids.view, depth.view, walk_depth].map(|v| {
            [vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(v)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]
        });
        let writes: Vec<_> = image_infos
            .iter()
            .enumerate()
            .map(|(b, info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(image_set)
                    .dst_binding(b as u32)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(info)
            })
            .collect();
        unsafe { d.update_descriptor_sets(&writes, &[]) };

        let set_layouts = [walk_layout, data_layout, image_layout];
        let push = [vk::PushConstantRange::default()
            .stage_flags(both)
            .offset(0)
            .size(std::mem::size_of::<PlantPush>() as u32)];
        let layout = unsafe {
            d.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts).push_constant_ranges(&push),
                None,
            )
        }?;
        // The walk's switches (`LIT` and the effects), then SKY, WEATHER, WEATHER_DEPTH and
        // POSE off, and PLANT_MODELS on: `plantHidden` names the cells this pass draws.
        let mut flags: Vec<u32> = constants.to_vec();
        flags.extend([0, 0, 0, 0, 1]);
        let entries: Vec<_> = (0..flags.len() as u32)
            .map(|i| vk::SpecializationMapEntry::default().constant_id(i).offset(i * 4).size(4))
            .collect();
        let spec = vk::SpecializationInfo::default().map_entries(&entries).data(bytemuck::cast_slice(&flags));
        let vis_pipeline = {
            let vs = gpu.shader(PLANT_VERT)?;
            let fs = gpu.shader(PLANT_VIS_FRAG).context("the plant pass's visibility shader")?;
            let p = pipeline(d, vis_pass, layout, vs, fs, Some(&spec), vk::PrimitiveTopology::TRIANGLE_LIST, &[plain()], true);
            unsafe { d.destroy_shader_module(vs, None) };
            unsafe { d.destroy_shader_module(fs, None) };
            p?
        };
        let shade_pipeline = {
            let vs = gpu.shader(FULLSCREEN_VERT)?;
            let fs = gpu.shader(PLANT_SHADE_FRAG).context("the plant pass's shading shader")?;
            let blend = [over(), over(), plain()];
            let p = pipeline(d, shade_pass, layout, vs, fs, Some(&spec), vk::PrimitiveTopology::TRIANGLE_LIST, &blend, false);
            unsafe { d.destroy_shader_module(vs, None) };
            unsafe { d.destroy_shader_module(fs, None) };
            p?
        };
        Ok(PlantPass {
            motion: PlantMotion { supersample: aa, ..motion },
            aa,
            layer,
            raster,
            world,
            ids,
            depth,
            vis_pass,
            vis_fb,
            vis_pipeline,
            occluder,
            shade_pass,
            shade_fb,
            shade_pipeline,
            written: vec![u64::MAX; frames.len()],
            frames,
            set_layouts: [data_layout, image_layout],
            pool,
            data_sets,
            image_set,
            sampler,
            layout,
            staged: vec![PlantList::default(); staging],
            drawn: PlantList::default(),
            version: 0,
            tips: Vec::new(),
            seconds: 0.0,
            rain: [0.0; 3],
            drawn_tips: Vec::new(),
            stats: PlantStats::default(),
        })
    }

    pub fn motion(&self) -> PlantMotion {
        self.motion
    }

    /// The list the pack writes for staging buffer `into`.
    pub(crate) fn staging(&mut self, into: usize) -> &mut PlantList {
        &mut self.staged[into]
    }

    /// The frames draw staging buffer `from`'s list from now on (the frame that uploads it).
    pub(crate) fn adopt(&mut self, from: usize) {
        std::mem::swap(&mut self.drawn, &mut self.staged[from]);
        self.version += 1;
        let n = self.drawn.cells.len();
        if n > MAX_PLANT_CELLS {
            eprintln!("plant pass: {n} plant cells, capacity {MAX_PLANT_CELLS}; the rest are not drawn");
        }
        let changing = self.drawn.cells.iter().filter(|c| f32::from_bits(c[2]) > SETTLED).count();
        self.stats = PlantStats {
            cells: n.min(MAX_PLANT_CELLS),
            plants: self.drawn.plants.len(),
            changing,
            dropped: n.saturating_sub(MAX_PLANT_CELLS),
        };
        // The tips index the new list's plants.
        self.take_tips(self.seconds, self.rain);
    }

    pub fn stats(&self) -> PlantStats {
        self.stats
    }

    /// Take the plants' tips at `seconds` (the frame clock, sim seconds) under `rain`. True
    /// when the picture moves: a tip moved since the last frame drawn, or a cell is still
    /// arriving or leaving.
    pub(crate) fn set_clock(&mut self, seconds: f64, rain: [f32; 3]) -> bool {
        self.take_tips(seconds, rain);
        self.tips != self.drawn_tips || seconds < self.drawn.settled_at
    }

    fn take_tips(&mut self, seconds: f64, rain: [f32; 3]) {
        self.seconds = seconds;
        self.rain = rain;
        let plants = &self.drawn.plants[..self.drawn.plants.len().min(MAX_PLANTS)];
        tips_into(plants, &self.motion.breeze, self.world.0, seconds, rain, &mut self.tips);
    }

    /// Write this frame's cells (when ring slot `slot` does not hold the drawn list yet)
    /// and tips into the slot's buffer.
    pub(crate) fn prepare(&mut self, slot: usize) {
        let buf = &self.frames[slot];
        if self.written[slot] != self.version {
            let n = self.drawn.cells.len().min(MAX_PLANT_CELLS);
            buf.write_bytes_at(0, &self.drawn.cells[..n]);
            self.written[slot] = self.version;
        }
        buf.write_bytes_at(CELL_BYTES, &self.tips);
        self.drawn_tips.clone_from(&self.tips);
    }

    fn push(&self, s: u32, rise: u32, base: i32, view_x: i32, bias: f32) -> PlantPush {
        PlantPush {
            geom: [s as i32, rise as i32, base, self.world.0 as i32],
            k: [
                self.drawn.cells.len().min(MAX_PLANT_CELLS) as i32,
                self.aa as i32,
                view_x,
                self.world.2 as i32,
            ],
            t: [(self.seconds - self.drawn.epoch) as f32, self.motion.grow_s, bias, 0.0],
            layer: [self.layer.0 as f32, self.layer.1 as f32, self.raster.0 as f32, self.raster.1 as f32],
        }
    }

    /// Whether there is anything to draw.
    pub fn drawing(&self) -> bool {
        !self.drawn.cells.is_empty()
    }

    /// The occluder the figures test against: the walk's end depth, or the nearest plant in
    /// front of it, per raster pixel. Written every frame the world is drawn.
    pub(crate) fn occluder_view(&self) -> vk::ImageView {
        self.occluder.view
    }

    /// Record the visibility layer for ring slot `slot` (after [`PlantPass::prepare`]):
    /// `walk_set` is the walk's set 0 at its dynamic offset `offset`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_vis(
        &self,
        d: &ash::Device,
        cb: vk::CommandBuffer,
        slot: usize,
        walk_set: vk::DescriptorSet,
        offset: u32,
        (s, rise, base): (u32, u32, i32),
        view_x: i32,
        cord_reach: f32,
    ) {
        // The last frame's shade read the layer this pass clears.
        let m = [vk::MemoryBarrier::default()];
        unsafe {
            d.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                vk::DependencyFlags::empty(),
                &m,
                &[],
                &[],
            )
        };
        let clears = [
            vk::ClearValue { color: vk::ClearColorValue { uint32: [0; 4] } },
            vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } },
        ];
        let extent = vk::Extent2D { width: self.layer.0, height: self.layer.1 };
        let area = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent };
        let begin = vk::RenderPassBeginInfo::default()
            .render_pass(self.vis_pass)
            .framebuffer(self.vis_fb)
            .render_area(area)
            .clear_values(&clears);
        unsafe { d.cmd_begin_render_pass(cb, &begin, vk::SubpassContents::INLINE) };
        let n = self.drawn.cells.len().min(MAX_PLANT_CELLS) as u32;
        if n > 0 {
            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: self.layer.0 as f32,
                height: self.layer.1 as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            unsafe { d.cmd_set_viewport(cb, 0, &[viewport]) };
            unsafe { d.cmd_set_scissor(cb, 0, &[area]) };
            unsafe { d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.vis_pipeline) };
            let sets = [walk_set, self.data_sets[slot]];
            unsafe { d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &sets, &[offset]) };
            let mut push = self.push(s, rise, base, view_x, crate::figures::DEPTH_BIAS);
            push.t[3] = cord_reach;
            unsafe { d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&push)) };
            // Twelve vertices a cell (front and cap), every cell twice: the second range
            // draws the cells crossing the raster's right edge a ring to the left.
            unsafe { d.cmd_draw(cb, 12, 2 * n, 0, 0) };
        }
        unsafe { d.cmd_end_render_pass(cb) };
        // The layer (and the walk's raster and depth, written before it) for the shade.
        let m = [vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_access_mask(
                vk::AccessFlags::SHADER_READ | vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )];
        unsafe {
            d.cmd_pipeline_barrier(
                cb,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::DependencyFlags::empty(),
                &m,
                &[],
                &[],
            )
        };
    }

    /// Record the shade (after [`PlantPass::record_vis`]): the plants over the raster and
    /// the occluder, one full-screen triangle. Every frame that draws the world records it,
    /// so the figures always read a whole occluder.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_shade(
        &self,
        d: &ash::Device,
        cb: vk::CommandBuffer,
        slot: usize,
        walk_set: vk::DescriptorSet,
        offset: u32,
        (s, rise, base): (u32, u32, i32),
        view_x: i32,
    ) {
        let extent = vk::Extent2D { width: self.raster.0, height: self.raster.1 };
        let area = vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent };
        let begin = vk::RenderPassBeginInfo::default()
            .render_pass(self.shade_pass)
            .framebuffer(self.shade_fb)
            .render_area(area);
        unsafe { d.cmd_begin_render_pass(cb, &begin, vk::SubpassContents::INLINE) };
        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.raster.0 as f32,
            height: self.raster.1 as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        unsafe { d.cmd_set_viewport(cb, 0, &[viewport]) };
        unsafe { d.cmd_set_scissor(cb, 0, &[area]) };
        unsafe { d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.shade_pipeline) };
        let sets = [walk_set, self.data_sets[slot], self.image_set];
        unsafe { d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &sets, &[offset]) };
        let push = self.push(s, rise, base, view_x, crate::figures::DEPTH_BIAS);
        unsafe { d.cmd_push_constants(cb, self.layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(&push)) };
        unsafe { d.cmd_draw(cb, 3, 1, 0, 0) };
        unsafe { d.cmd_end_render_pass(cb) };
    }

    pub(crate) fn destroy(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe { d.destroy_pipeline(self.vis_pipeline, None) };
        unsafe { d.destroy_pipeline(self.shade_pipeline, None) };
        unsafe { d.destroy_pipeline_layout(self.layout, None) };
        unsafe { d.destroy_descriptor_pool(self.pool, None) };
        for l in self.set_layouts {
            unsafe { d.destroy_descriptor_set_layout(l, None) };
        }
        unsafe { d.destroy_sampler(self.sampler, None) };
        unsafe { d.destroy_framebuffer(self.vis_fb, None) };
        unsafe { d.destroy_render_pass(self.vis_pass, None) };
        unsafe { d.destroy_framebuffer(self.shade_fb, None) };
        unsafe { d.destroy_render_pass(self.shade_pass, None) };
        self.occluder.destroy(d);
        for f in &self.frames {
            f.destroy(gpu);
        }
        self.ids.destroy(d);
        self.depth.destroy(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DRY: [f32; 3] = [0.0; 3];

    #[test]
    fn the_breeze_is_the_same_on_both_sides_of_the_seam() {
        // Whole sweeps and flutter waves round the ring: ring 1 is ring 0, and the last
        // column before the seam is as close to the first as any two neighbours are.
        let b = Breeze::default();
        let width = 640.0;
        let mut worst_seam: f64 = 0.0;
        let mut worst_step: f64 = 0.0;
        for k in 0..2000 {
            let t = 0.037 * f64::from(k);
            assert_eq!(b.strength(0.0, t, DRY), b.strength(1.0, t, DRY), "ring 1 is ring 0 at t {t}");
            worst_seam = worst_seam.max((b.strength(639.0 / width, t, DRY) - b.strength(0.0, t, DRY)).abs());
            worst_step = worst_step.max((b.strength(320.0 / width, t, DRY) - b.strength(321.0 / width, t, DRY)).abs());
        }
        assert!(worst_seam <= worst_step + 1e-12, "across the seam {worst_seam} vs a column step {worst_step}");
        assert!(worst_step > 0.0, "the breeze does vary along the ring");
    }

    #[test]
    fn a_packet_rests_then_stirs_and_never_jumps() {
        let b = Breeze::default();
        // Exactly calm after the fall, at every place.
        let calm = b.rise_s + b.hold_s + b.fall_s + 0.5;
        assert_eq!(b.strength(0.0, calm, DRY), 0.0);
        assert!(b.strength(0.0, b.rise_s + 1.0, DRY) > 0.0, "a gust in the hold");
        // Continuous on the frame clock: no step bigger than a small slope allows.
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while t < 2.0 * b.period_s {
            let (a, c) = (b.strength(0.3, t, DRY), b.strength(0.3, t + dt, DRY));
            assert!((a - c).abs() < 0.05, "a jump of {} at t {t}", (a - c).abs());
            t += dt;
        }
    }

    #[test]
    fn rain_strengthens_the_breeze_and_only_where_it_blows() {
        let b = Breeze::default();
        let t = b.rise_s + 2.0;
        assert!(b.strength(0.0, t, [0.0, 0.0, 1.0]) > b.strength(0.0, t, DRY));
        let calm = b.rise_s + b.hold_s + b.fall_s + 1.0;
        assert_eq!(b.strength(0.0, calm, [0.0, 0.0, 1.0]), 0.0, "a calm stays calm in the rain");
    }

    #[test]
    fn face_weights_round_trip_and_a_shared_face_packs_alike_from_both_cells() {
        assert_eq!(unpack_faces(0), None, "a word without face weights");
        let (t, b) = unpack_faces(pack_faces(0.75, 0.25, true)).unwrap();
        assert!((t - 0.75).abs() < 1e-3 && (b - 0.25).abs() < 1e-3);
        // The face between a lower and an upper cell: the lower's top, the upper's bottom.
        let face = 0.4321;
        let lower = unpack_faces(pack_faces(face, 0.3, false)).unwrap().0;
        let upper = unpack_faces(pack_faces(0.6, face, true)).unwrap().1;
        assert_eq!(lower, upper);
    }

    #[test]
    fn a_cell_round_trips_its_packing() {
        let c = pack_cell([639, 127, 95], 4321, 0.5, true, 1.25, 0xAABBCCDD);
        let (cell, plant, w, dying, t) = unpack_cell(c);
        assert_eq!((cell, plant, dying, t), ([639, 127, 95], 4321, true, 1.25));
        assert!((w - 0.5).abs() < 1.0 / 4095.0);
        assert_eq!(c[3], 0xAABBCCDD);
    }

    #[test]
    fn a_cell_scales_in_and_out_continuously_and_a_reversal_starts_where_it_was() {
        let g = 0.4;
        assert_eq!(cell_scale(-0.1, 0.0, false, g), 0.0, "not yet arrived");
        assert_eq!(cell_scale(1.0, 0.0, false, g), 1.0, "grown");
        assert_eq!(cell_scale(-0.1, 0.0, true, g), 1.0, "not yet leaving");
        assert_eq!(cell_scale(1.0, 0.0, true, g), 0.0, "gone");
        let mid = cell_scale(0.1, 0.0, false, g);
        assert!(mid > 0.0 && mid < 1.0);
        // Reversed at t = 0.1: leaving from the moment that gives the same scale now.
        let r = unsmooth(1.0 - mid);
        let t_off = 0.1 - r * g;
        assert!((cell_scale(0.1, t_off, true, g) - mid).abs() < 1e-5, "no jump at the reversal");
    }

    #[test]
    fn a_still_plant_has_no_tip_and_a_plant_follows_the_breeze_late() {
        let b = Breeze { sweeps: 0, flutter: 0.0, peak_vary: 0.0, ..Breeze::default() };
        let plants = [
            PlantRec::default(),
            PlantRec { x: 10.0, amp: 2.0, lag: 0.0 },
            PlantRec { x: 10.0, amp: 2.0, lag: 1.0 },
        ];
        let mut tips = Vec::new();
        tips_into(&plants, &b, 64, b.rise_s * 0.5, DRY, &mut tips);
        assert_eq!(tips[0], 0.0);
        assert!(tips[1] > tips[2] && tips[2] > 0.0, "the lagging plant is behind on the rise: {tips:?}");
    }
}
