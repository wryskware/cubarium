//! The lit tier's bloom (package L5, `design/handoffs/presentation-plan-2026-09-24.md`):
//! a restrained, blocky halo around the emitters, added onto the world raster.
//!
//! `voxel.frag` (lit) writes each pixel's emitted light to a second attachment, the
//! **emission** image: an emitter's colour where the walk ended on an emitting texel,
//! zero everywhere else. `bloom.frag` then runs three small passes:
//!
//! 1. **gather** into a cell image, one texel per voxel cell (an `s × s` block of raster
//!    pixels, its rows aligned with the voxel bands): the brightest emitted colour in the
//!    cell, per channel;
//! 2. **spread** into a second cell image: each cell takes the gathered colour of every
//!    cell within `bloom_radius` cells, times a falloff quantised to three steps
//!    (1, 2/3, 1/3), the largest per channel;
//! 3. **add** onto the raster, nearest, additive: every pixel gets its cell's halo times
//!    `bloom`, except an emitting pixel, which stays exactly as drawn.
//!
//! So the halo is made of whole voxel cells at three strengths, and the emitters stay
//! crisp. It runs only when the raster is redrawn, so a skipped frame never adds twice.

use anyhow::{Context, Result};
use ash::vk;

use crate::render::{RASTER_FORMAT, colour_pass, colour_pass_n, framebuffer};
use crate::vk::Gpu;
use crate::voxel::VoxelParams;

const FULLSCREEN_VERT: &[u8] = include_bytes!("../shaders/fullscreen.vert.spv");
const BLOOM_FRAG: &[u8] = include_bytes!("../shaders/bloom.frag.spv");

/// The emission attachment's format: sRGB-encoded like the raster, so a dim emitter keeps
/// its precision.
pub(crate) const EMIT_FORMAT: vk::Format = RASTER_FORMAT;
/// The cell images' format.
const CELL_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

const GATHER: i32 = 0;
const SPREAD: i32 = 1;
const ADD: i32 = 2;

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

pub(crate) struct Bloom {
    emit_image: vk::Image,
    emit_memory: vk::DeviceMemory,
    pub(crate) emit_view: vk::ImageView,
    cells: [(vk::Image, vk::DeviceMemory, vk::ImageView, vk::Framebuffer); 2],
    cell_pass: vk::RenderPass,
    add_pass: vk::RenderPass,
    add_framebuffer: vk::Framebuffer,
    set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    /// Gather (emission), spread (cells 0), add (cells 1); binding 1 is the emission.
    sets: [vk::DescriptorSet; 3],
    layout: vk::PipelineLayout,
    cell_pipeline: vk::Pipeline,
    add_pipeline: vk::Pipeline,
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
        let cell = |_: usize| -> Result<_> {
            let (image, memory) = gpu.image(
                grid.0,
                grid.1,
                CELL_FORMAT,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            )?;
            let view = gpu.view(image, CELL_FORMAT)?;
            let fb = framebuffer(d, cell_pass, view, grid.0, grid.1)?;
            Ok((image, memory, view, fb))
        };
        let cells = [cell(0)?, cell(1)?];
        let add_pass = colour_pass_n(
            d,
            &[RASTER_FORMAT],
            vk::AttachmentLoadOp::LOAD,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;
        let add_framebuffer = framebuffer(d, add_pass, raster_view, params.raster_w, params.raster_h)?;

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
        let sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(6)];
        let pool = unsafe {
            d.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(3)
                    .pool_sizes(&sizes),
                None,
            )
        }?;
        let layouts = [set_layout; 3];
        let allocated = unsafe {
            d.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            )
        }?;
        let sets = [allocated[0], allocated[1], allocated[2]];
        let info = |view: vk::ImageView| {
            [vk::DescriptorImageInfo::default()
                .sampler(nearest)
                .image_view(view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]
        };
        let (ie, ic0, ic1) = (info(emit_view), info(cells[0].2), info(cells[1].2));
        fn write(
            set: vk::DescriptorSet,
            b: u32,
            i: &[vk::DescriptorImageInfo],
        ) -> vk::WriteDescriptorSet<'_> {
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(b)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(i)
        }
        unsafe {
            d.update_descriptor_sets(
                &[
                    write(sets[0], 0, &ie),
                    write(sets[0], 1, &ie),
                    write(sets[1], 0, &ic0),
                    write(sets[1], 1, &ie),
                    write(sets[2], 0, &ic1),
                    write(sets[2], 1, &ie),
                ],
                &[],
            )
        };
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
        let fs = gpu.shader(BLOOM_FRAG).context("the bloom fragment shader")?;
        let cell_pipeline =
            crate::render::fullscreen_pipeline(d, cell_pass, layout, vs, fs, false)?;
        // Premultiplied source-over with the shader's alpha at 0 is `raster + halo`.
        let add_pipeline = crate::render::fullscreen_pipeline(d, add_pass, layout, vs, fs, true)?;
        unsafe {
            d.destroy_shader_module(vs, None);
            d.destroy_shader_module(fs, None);
        }
        Ok(Bloom {
            emit_image,
            emit_memory,
            emit_view,
            cells,
            cell_pass,
            add_pass,
            add_framebuffer,
            set_layout,
            pool,
            sets,
            layout,
            cell_pipeline,
            add_pipeline,
            grid,
        })
    }

    /// Record the three passes after the slab walk has drawn the raster and the emission.
    ///
    /// # Safety
    /// `cb` is recording, outside a render pass, after the slab-walk pass.
    pub(crate) unsafe fn record(&self, d: &ash::Device, cb: vk::CommandBuffer, params: &VoxelParams) {
        let (cw, ch, off) = self.grid;
        let push = |mode: i32| -> [u32; 12] {
            [
                mode as u32,
                params.s,
                params.bloom_radius,
                0,
                cw,
                ch,
                off,
                params.raster_h,
                params.bloom.max(0.0).to_bits(),
                0,
                0,
                0,
            ]
        };
        unsafe {
            let passes = [
                (GATHER, self.cell_pass, self.cells[0].3, cw, ch, self.cell_pipeline, self.sets[0]),
                (SPREAD, self.cell_pass, self.cells[1].3, cw, ch, self.cell_pipeline, self.sets[1]),
                (
                    ADD,
                    self.add_pass,
                    self.add_framebuffer,
                    params.raster_w,
                    params.raster_h,
                    self.add_pipeline,
                    self.sets[2],
                ),
            ];
            for (mode, pass, fb, w, h, pipeline, set) in passes {
                colour_to_read(d, cb);
                crate::render::begin(d, cb, pass, fb, w, h);
                d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline);
                d.cmd_bind_descriptor_sets(
                    cb,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layout,
                    0,
                    &[set],
                    &[],
                );
                d.cmd_push_constants(
                    cb,
                    self.layout,
                    vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(&push(mode)),
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
            for (image, memory, view, fb) in self.cells {
                d.destroy_framebuffer(fb, None);
                d.destroy_image_view(view, None);
                d.destroy_image(image, None);
                d.free_memory(memory, None);
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
}
