//! The lit tier's bloom (`design/handoffs/presentation-plan-2026-09-24.md`, "## L5" and
//! "## L5 and W fixes, round 2"): a halo around the emitters, added onto the world raster.
//!
//! `voxel.frag` (lit) writes each pixel's emitted light to a second attachment, the
//! **emission** image: an emitter's colour where the walk ended on an emitting texel,
//! zero everywhere else. Two styles ([`BloomStyle`]) turn it into a halo:
//!
//! - **smooth** (the default, `shaders/bloom_smooth.frag`): the emission goes down a mip
//!   chain (dual-Kawase, bilinear), is blurred by a separable Gaussian at the last level,
//!   comes back up the chain (a bilinear tent) and is added onto the raster, bilinear, times
//!   `bloom x SMOOTH_GAIN`. The Gaussian's sigma is half of `bloom_radius` voxel cells, so the halo
//!   falls to about an eighth of its peak `bloom_radius` cells out, the same in cells at
//!   every scale. The chain goes as deep as keeps that sigma at 2-4 texels.
//! - **blocky** (L5's, `shaders/bloom.frag`): gathered at voxel-cell resolution (the
//!   brightest emitted colour in each `s x s` cell, its rows on the voxel bands), spread
//!   `bloom_radius` cells with a falloff quantised to 1, 2/3, 1/3 (the largest per
//!   channel), and added nearest: whole voxel cells at three strengths.
//!
//! Either way an emitting pixel is left exactly as drawn, so the emitters stay crisp, and
//! the passes run only when the raster is redrawn, so a skipped frame never adds twice.

use anyhow::{Context, Result};
use ash::vk;

use crate::render::{RASTER_FORMAT, colour_pass, colour_pass_n, framebuffer};
use crate::vk::Gpu;
use crate::voxel::VoxelParams;

const FULLSCREEN_VERT: &[u8] = include_bytes!("../shaders/fullscreen.vert.spv");
const BLOOM_FRAG: &[u8] = include_bytes!("../shaders/bloom.frag.spv");
const BLOOM_SMOOTH_FRAG: &[u8] = include_bytes!("../shaders/bloom_smooth.frag.spv");

/// The emission attachment's format: sRGB-encoded like the raster, so a dim emitter keeps
/// its precision.
pub(crate) const EMIT_FORMAT: vk::Format = RASTER_FORMAT;
/// The cell and mip images' format.
const CELL_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

/// What the smooth style multiplies `bloom` by: the blur spreads a small emitter's light
/// over many pixels, so at the same `bloom` the smooth halo reads about as strong as the
/// blocky one (whose cells take the emitter's full colour).
pub const SMOOTH_GAIN: f32 = 20.0;

/// How the halo is made (`[light] bloom_style`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BloomStyle {
    /// Blurred down a mip chain and added bilinear: a soft glow.
    #[default]
    Smooth,
    /// L5's: whole voxel cells at three strengths, added nearest.
    Blocky,
}

// Blocky passes.
const GATHER: i32 = 0;
const SPREAD: i32 = 1;
const ADD: i32 = 2;
// Smooth passes.
const DOWN: i32 = 0;
const BLUR_H: i32 = 1;
const BLUR_V: i32 = 2;
const UP: i32 = 3;
const ADD_SMOOTH: i32 = 4;

/// The voxel cell grid over a raster: how many cells across and down, and how many rows
/// the grid is shifted by so a cell's rows are one voxel band's (`base` is a band edge).
pub fn cell_grid(params: &VoxelParams) -> (u32, u32, u32) {
    let s = params.s.max(1);
    let off = (-params.base).rem_euclid(s as i32) as u32;
    (
        params.raster_w.div_ceil(s),
        (params.raster_h + off).div_ceil(s),
        off,
    )
}

/// The smooth bloom's Gaussian sigma in raster pixels: half of `bloom_radius` voxel cells.
pub fn smooth_sigma_px(params: &VoxelParams) -> f32 {
    params.bloom_radius.max(0.0) * params.s.max(1) as f32 * 0.5
}

/// How many times the smooth bloom halves the emission before it blurs: as many as keep
/// the Gaussian at least two texels wide there, one at least, six at most.
pub fn smooth_levels(params: &VoxelParams) -> u32 {
    let sigma = smooth_sigma_px(params);
    if sigma < 4.0 {
        return 1;
    }
    ((sigma / 2.0).log2().floor() as u32).clamp(1, 6)
}

/// Each mip level's size: half the one above, rounded up.
pub fn level_sizes(params: &VoxelParams, levels: u32) -> Vec<(u32, u32)> {
    let mut size = (params.raster_w, params.raster_h);
    (0..levels)
        .map(|_| {
            size = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
            size
        })
        .collect()
}

/// A render target the passes write and then read.
#[derive(Clone, Copy)]
struct Target {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    fb: vk::Framebuffer,
    w: u32,
    h: u32,
}

/// One recorded pass: what it draws into and with what.
struct Pass {
    mode: i32,
    fb: vk::Framebuffer,
    w: u32,
    h: u32,
    add: bool,
    set: vk::DescriptorSet,
    /// The source's size (smooth passes).
    src: (u32, u32),
}

pub(crate) struct Bloom {
    style: BloomStyle,
    emit_image: vk::Image,
    emit_memory: vk::DeviceMemory,
    pub(crate) emit_view: vk::ImageView,
    /// Blocky: the gathered and spread cells. Smooth: the mip levels, then the blur's
    /// scratch at the last level's size.
    targets: Vec<Target>,
    cell_pass: vk::RenderPass,
    add_pass: vk::RenderPass,
    add_framebuffer: vk::Framebuffer,
    set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    passes: Vec<Pass>,
    layout: vk::PipelineLayout,
    cell_pipeline: vk::Pipeline,
    add_pipeline: vk::Pipeline,
    /// The smooth style's bilinear sampler (x repeats, y is black past the edges).
    linear: Option<vk::Sampler>,
    grid: (u32, u32, u32),
}

impl Bloom {
    pub(crate) fn new(
        gpu: &Gpu,
        params: &VoxelParams,
        raster_view: vk::ImageView,
        nearest: vk::Sampler,
    ) -> Result<Bloom> {
        let d = &gpu.device;
        let style = params.bloom_style;
        let (emit_image, emit_memory) = gpu.image(
            params.raster_w,
            params.raster_h,
            EMIT_FORMAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        )?;
        let emit_view = gpu.view(emit_image, EMIT_FORMAT)?;
        let grid = cell_grid(params);
        let cell_pass = colour_pass(
            d,
            CELL_FORMAT,
            vk::AttachmentLoadOp::DONT_CARE,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;
        let target = |w: u32, h: u32| -> Result<Target> {
            let (image, memory) = gpu.image(
                w,
                h,
                CELL_FORMAT,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            )?;
            let view = gpu.view(image, CELL_FORMAT)?;
            let fb = framebuffer(d, cell_pass, view, w, h)?;
            Ok(Target { image, memory, view, fb, w, h })
        };
        let targets: Vec<Target> = match style {
            BloomStyle::Blocky => vec![target(grid.0, grid.1)?, target(grid.0, grid.1)?],
            BloomStyle::Smooth => {
                let sizes = level_sizes(params, smooth_levels(params));
                let last = *sizes.last().unwrap();
                let mut t = sizes
                    .iter()
                    .map(|&(w, h)| target(w, h))
                    .collect::<Result<Vec<_>>>()?;
                t.push(target(last.0, last.1)?);
                t
            }
        };
        let add_pass = colour_pass_n(
            d,
            &[RASTER_FORMAT],
            vk::AttachmentLoadOp::LOAD,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;
        let add_framebuffer = framebuffer(d, add_pass, raster_view, params.raster_w, params.raster_h)?;
        let linear = match style {
            BloomStyle::Blocky => None,
            BloomStyle::Smooth => Some(unsafe {
                d.create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(vk::Filter::LINEAR)
                        .min_filter(vk::Filter::LINEAR)
                        .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                        .address_mode_u(vk::SamplerAddressMode::REPEAT)
                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .border_color(vk::BorderColor::FLOAT_TRANSPARENT_BLACK),
                    None,
                )
            }?),
        };

        // Every pass: (mode, source view, target or the raster, source size).
        let raster = (params.raster_w, params.raster_h);
        let plan: Vec<(i32, vk::ImageView, Option<Target>, (u32, u32))> = match style {
            BloomStyle::Blocky => vec![
                (GATHER, emit_view, Some(targets[0]), raster),
                (SPREAD, targets[0].view, Some(targets[1]), (grid.0, grid.1)),
                (ADD, targets[1].view, None, (grid.0, grid.1)),
            ],
            BloomStyle::Smooth => {
                let n = targets.len() - 1;
                let mut plan = Vec::new();
                let mut from = (emit_view, raster);
                for t in &targets[..n] {
                    plan.push((DOWN, from.0, Some(*t), from.1));
                    from = (t.view, (t.w, t.h));
                }
                let (last, scratch) = (targets[n - 1], targets[n]);
                plan.push((BLUR_H, last.view, Some(scratch), (last.w, last.h)));
                plan.push((BLUR_V, scratch.view, Some(last), (last.w, last.h)));
                for i in (1..n).rev() {
                    let (lo, hi) = (targets[i], targets[i - 1]);
                    plan.push((UP, lo.view, Some(hi), (lo.w, lo.h)));
                }
                plan.push((ADD_SMOOTH, targets[0].view, None, (targets[0].w, targets[0].h)));
                plan
            }
        };

        let binding = |b: u32| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(b)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        };
        let bindings = [binding(0), binding(1)];
        let set_layout = unsafe {
            d.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }?;
        let count = plan.len() as u32;
        let sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(2 * count)];
        let pool = unsafe {
            d.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(count)
                    .pool_sizes(&sizes),
                None,
            )
        }?;
        let layouts = vec![set_layout; plan.len()];
        let sets = unsafe {
            d.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            )
        }?;
        let source_sampler = linear.unwrap_or(nearest);
        let infos: Vec<[vk::DescriptorImageInfo; 2]> = plan
            .iter()
            .map(|&(_, view, _, _)| {
                let info = |sampler: vk::Sampler, view: vk::ImageView| {
                    vk::DescriptorImageInfo::default()
                        .sampler(sampler)
                        .image_view(view)
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                };
                [info(source_sampler, view), info(nearest, emit_view)]
            })
            .collect();
        let writes: Vec<vk::WriteDescriptorSet> = sets
            .iter()
            .zip(&infos)
            .flat_map(|(&set, i)| {
                (0..2).map(move |b| {
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(b as u32)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&i[b..b + 1])
                })
            })
            .collect();
        unsafe { d.update_descriptor_sets(&writes, &[]) };
        let passes = plan
            .iter()
            .zip(&sets)
            .map(|(&(mode, _, target, src), &set)| match target {
                Some(t) => Pass { mode, fb: t.fb, w: t.w, h: t.h, add: false, set, src },
                None => Pass {
                    mode,
                    fb: add_framebuffer,
                    w: params.raster_w,
                    h: params.raster_h,
                    add: true,
                    set,
                    src,
                },
            })
            .collect();

        let ranges = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .size(48)];
        let set_layouts = [set_layout];
        let layout = unsafe {
            d.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&set_layouts)
                    .push_constant_ranges(&ranges),
                None,
            )
        }?;
        let vs = gpu.shader(FULLSCREEN_VERT)?;
        let fs = gpu
            .shader(match style {
                BloomStyle::Blocky => BLOOM_FRAG,
                BloomStyle::Smooth => BLOOM_SMOOTH_FRAG,
            })
            .context("the bloom fragment shader")?;
        let cell_pipeline =
            crate::render::fullscreen_pipeline(d, cell_pass, layout, vs, fs, false)?;
        // Premultiplied source-over with the shader's alpha at 0 is `raster + halo`.
        let add_pipeline = crate::render::fullscreen_pipeline(d, add_pass, layout, vs, fs, true)?;
        unsafe {
            d.destroy_shader_module(vs, None);
            d.destroy_shader_module(fs, None);
        }
        Ok(Bloom {
            style,
            emit_image,
            emit_memory,
            emit_view,
            targets,
            cell_pass,
            add_pass,
            add_framebuffer,
            set_layout,
            pool,
            passes,
            layout,
            cell_pipeline,
            add_pipeline,
            linear,
            grid,
        })
    }

    /// Record the passes after the slab walk has drawn the raster and the emission.
    ///
    /// # Safety
    /// `cb` is recording, outside a render pass, after the slab-walk pass.
    pub(crate) unsafe fn record(&self, d: &ash::Device, cb: vk::CommandBuffer, params: &VoxelParams) {
        let (cw, ch, off) = self.grid;
        let sigma = {
            // In the last level's texels.
            let last = self.targets.len().saturating_sub(2);
            let scale = params.raster_w as f32 / self.targets[last].w.max(1) as f32;
            smooth_sigma_px(params) / scale
        };
        let taps = (3.0 * sigma).ceil().clamp(1.0, 32.0) as u32;
        let push = |p: &Pass| -> [u32; 12] {
            match self.style {
                BloomStyle::Blocky => [
                    p.mode as u32,
                    params.s,
                    params.bloom_radius.max(0.0).round() as u32,
                    0,
                    cw,
                    ch,
                    off,
                    params.raster_h,
                    params.bloom.max(0.0).to_bits(),
                    0,
                    0,
                    0,
                ],
                BloomStyle::Smooth => [
                    p.mode as u32,
                    p.src.0,
                    p.src.1,
                    taps,
                    p.w,
                    p.h,
                    0,
                    0,
                    (params.bloom.max(0.0) * SMOOTH_GAIN).to_bits(),
                    sigma.to_bits(),
                    0,
                    0,
                ],
            }
        };
        unsafe {
            for p in &self.passes {
                colour_to_read(d, cb);
                let (pass, pipeline) = if p.add {
                    (self.add_pass, self.add_pipeline)
                } else {
                    (self.cell_pass, self.cell_pipeline)
                };
                crate::render::begin(d, cb, pass, p.fb, p.w, p.h);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
                d.cmd_bind_descriptor_sets(
                    cb,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layout,
                    0,
                    &[p.set],
                    &[],
                );
                d.cmd_push_constants(
                    cb,
                    self.layout,
                    vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(&push(p)),
                );
                d.cmd_draw(cb, 3, 1, 0, 0);
                d.cmd_end_render_pass(cb);
            }
            colour_to_read(d, cb);
        }
    }

    pub(crate) fn destroy(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            d.destroy_pipeline(self.cell_pipeline, None);
            d.destroy_pipeline(self.add_pipeline, None);
            d.destroy_pipeline_layout(self.layout, None);
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_framebuffer(self.add_framebuffer, None);
            for t in &self.targets {
                d.destroy_framebuffer(t.fb, None);
                d.destroy_image_view(t.view, None);
                d.destroy_image(t.image, None);
                d.free_memory(t.memory, None);
            }
            if let Some(s) = self.linear {
                d.destroy_sampler(s, None);
            }
            d.destroy_image_view(self.emit_view, None);
            d.destroy_image(self.emit_image, None);
            d.free_memory(self.emit_memory, None);
            d.destroy_render_pass(self.cell_pass, None);
            d.destroy_render_pass(self.add_pass, None);
        }
    }
}

/// What the last pass wrote as a colour attachment is visible to the next pass's
/// fragment reads and attachment loads.
unsafe fn colour_to_read(d: &ash::Device, cb: vk::CommandBuffer) {
    unsafe {
        d.cmd_pipeline_barrier(
            cb,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::DependencyFlags::empty(),
            &[vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(
                    vk::AccessFlags::SHADER_READ
                        | vk::AccessFlags::COLOR_ATTACHMENT_READ
                        | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                )],
            &[],
            &[],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cell_grid_covers_the_raster_with_rows_on_the_voxel_bands() {
        let mut p = crate::voxel::tests::params();
        p.s = 6;
        p.raster_w = 600;
        p.raster_h = 100;
        p.base = 97;
        let (w, h, off) = cell_grid(&p);
        assert_eq!(w, 100);
        // Row `base - s` (the top of the y = 0, z = 0 front face) starts a cell.
        assert_eq!((91 + off) % 6, 0);
        assert!(h * 6 >= 100 + off && (h - 1) * 6 < 100 + off);
    }

    #[test]
    fn the_smooth_chain_keeps_the_blur_a_few_texels_wide_at_every_scale() {
        let mut p = crate::voxel::tests::params();
        p.bloom_radius = 2.0;
        for s in [4, 6, 9, 13, 20] {
            p.s = s;
            p.raster_w = 256 * s;
            p.raster_h = 1024;
            let n = smooth_levels(&p);
            let sizes = level_sizes(&p, n);
            assert_eq!(sizes.len() as u32, n);
            assert_eq!(sizes[0], (128 * s, 512));
            let sigma = smooth_sigma_px(&p) / (1u32 << n) as f32;
            assert!((1.5..=4.0).contains(&sigma), "{s} px: sigma {sigma} texels at level {n}");
        }
    }
}
