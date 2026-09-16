//! Q1/Q2/Q3 on the Vulkan side, with raw `ash`.
//!
//!   vk_scanout bench  [w] [h] [frames]   render to a VkImage, copy to a host
//!                                        buffer, read it back (Q1)
//!   vk_scanout flip   [frames] [conn]    render into a dma-buf-exported image,
//!                                        AddFB2 + page flip it (Q2)
//!   vk_scanout dumb   [frames] [conn]    render, read back, memcpy into a KMS
//!                                        dumb buffer, flip (Q3)
//!
//! `flip`/`dumb` take DRM master, so cube-screen-shim must be stopped first.
use std::ffi::CStr;
use std::os::unix::io::{FromRawFd, OwnedFd};
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use ash::vk;
use gpu_scanout::{kms, stat};

const VERT: &[u8] = include_bytes!("../../shaders/quad.vert.spv");
const FRAG: &[u8] = include_bytes!("../../shaders/quad.frag.spv");
/// DRM XRGB8888 is B,G,R,X in memory, i.e. VK_FORMAT_B8G8R8A8_UNORM.
const FORMAT: vk::Format = vk::Format::B8G8R8A8_UNORM;

fn main() -> Result<()> {
    let mut a = std::env::args().skip(1);
    let mode = a.next().unwrap_or_else(|| "bench".into());
    match mode.as_str() {
        "bench" => {
            let w: u32 = a.next().unwrap_or_else(|| "1920".into()).parse()?;
            let h: u32 = a.next().unwrap_or_else(|| "1080".into()).parse()?;
            let n: usize = a.next().unwrap_or_else(|| "60".into()).parse()?;
            bench(w, h, n)
        }
        "flip" => {
            let n: usize = a.next().unwrap_or_else(|| "180".into()).parse()?;
            let c = a.next().unwrap_or_else(|| "DP-1".into());
            flip(n, &c)
        }
        "gpucopy" => {
            let n: usize = a.next().unwrap_or_else(|| "180".into()).parse()?;
            let c = a.next().unwrap_or_else(|| "DP-1".into());
            gpucopy(n, &c)
        }
        "fliplinear" => {
            let n: usize = a.next().unwrap_or_else(|| "180".into()).parse()?;
            let c = a.next().unwrap_or_else(|| "DP-1".into());
            fliplinear(n, &c)
        }
        "dumb" => {
            let n: usize = a.next().unwrap_or_else(|| "180".into()).parse()?;
            let c = a.next().unwrap_or_else(|| "DP-1".into());
            dumb(n, &c)
        }
        other => bail!("unknown mode {other}"),
    }
}

// ---------------------------------------------------------------- device ---

struct Gpu {
    _entry: ash::Entry,
    instance: ash::Instance,
    pdev: vk::PhysicalDevice,
    device: ash::Device,
    queue: vk::Queue,
    qfam: u32,
    mem: vk::PhysicalDeviceMemoryProperties,
    timestamp_period: f32,
    has_modifier_ext: bool,
}

fn open_gpu() -> Result<Gpu> {
    let entry = unsafe { ash::Entry::load() }.context("load libvulkan.so.1")?;
    let app = vk::ApplicationInfo::default()
        .application_name(c"gpu-scanout")
        .api_version(vk::make_api_version(0, 1, 1, 0));
    let inst_exts = [
        ash::khr::get_physical_device_properties2::NAME.as_ptr(),
        ash::khr::external_memory_capabilities::NAME.as_ptr(),
    ];
    let instance = unsafe {
        entry.create_instance(
            &vk::InstanceCreateInfo::default().application_info(&app).enabled_extension_names(&inst_exts),
            None,
        )
    }
    .context("vkCreateInstance")?;

    let pdev = *unsafe { instance.enumerate_physical_devices() }?
        .first()
        .ok_or_else(|| anyhow!("no Vulkan physical device"))?;
    let props = unsafe { instance.get_physical_device_properties(pdev) };
    println!(
        "device: {} (api {}.{}.{})",
        unsafe { CStr::from_ptr(props.device_name.as_ptr()) }.to_string_lossy(),
        vk::api_version_major(props.api_version),
        vk::api_version_minor(props.api_version),
        vk::api_version_patch(props.api_version)
    );

    let avail: Vec<String> = unsafe { instance.enumerate_device_extension_properties(pdev) }?
        .iter()
        .map(|e| unsafe { CStr::from_ptr(e.extension_name.as_ptr()) }.to_string_lossy().into_owned())
        .collect();
    let wanted = [
        ash::khr::external_memory::NAME,
        ash::khr::external_memory_fd::NAME,
        ash::ext::external_memory_dma_buf::NAME,
        ash::ext::image_drm_format_modifier::NAME,
        ash::ext::queue_family_foreign::NAME,
        ash::khr::bind_memory2::NAME,
        ash::khr::image_format_list::NAME,
        ash::khr::sampler_ycbcr_conversion::NAME,
        ash::khr::maintenance1::NAME,
        ash::khr::get_memory_requirements2::NAME,
        ash::khr::dedicated_allocation::NAME,
    ];
    let enabled: Vec<&CStr> = wanted.into_iter().filter(|n| avail.iter().any(|a| a.as_str() == n.to_str().unwrap())).collect();
    let has_modifier_ext = enabled.contains(&ash::ext::image_drm_format_modifier::NAME);
    let ptrs: Vec<*const std::ffi::c_char> = enabled.iter().map(|n| n.as_ptr()).collect();

    let qfams = unsafe { instance.get_physical_device_queue_family_properties(pdev) };
    let qfam = qfams
        .iter()
        .position(|q| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .ok_or_else(|| anyhow!("no graphics queue family"))? as u32;
    let prio = [1.0f32];
    let qci = [vk::DeviceQueueCreateInfo::default().queue_family_index(qfam).queue_priorities(&prio)];
    let device = unsafe {
        instance.create_device(
            pdev,
            &vk::DeviceCreateInfo::default().queue_create_infos(&qci).enabled_extension_names(&ptrs),
            None,
        )
    }
    .context("vkCreateDevice")?;
    let queue = unsafe { device.get_device_queue(qfam, 0) };
    Ok(Gpu {
        _entry: entry,
        mem: unsafe { instance.get_physical_device_memory_properties(pdev) },
        timestamp_period: props.limits.timestamp_period,
        instance,
        pdev,
        device,
        queue,
        qfam,
        has_modifier_ext,
    })
}

impl Gpu {
    fn mem_type(&self, bits: u32, want: vk::MemoryPropertyFlags) -> Result<u32> {
        (0..self.mem.memory_type_count)
            .find(|i| bits & (1 << i) != 0 && self.mem.memory_types[*i as usize].property_flags.contains(want))
            .ok_or_else(|| anyhow!("no memory type for bits={bits:#x} want={want:?}"))
    }
}

// --------------------------------------------------------------- renderer ---

/// Everything needed to draw "clear + one textured quad" into a given image.
struct Renderer {
    rp: vk::RenderPass,
    pipeline: vk::Pipeline,
    pl: vk::PipelineLayout,
    dsl: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    tex: vk::Image,
    tex_mem: vk::DeviceMemory,
    tex_view: vk::ImageView,
    sampler: vk::Sampler,
    cmd_pool: vk::CommandPool,
    queries: vk::QueryPool,
}

impl Renderer {
    fn new(g: &Gpu, final_layout: vk::ImageLayout) -> Result<Renderer> {
        let d = &g.device;
        let cmd_pool = unsafe {
            d.create_command_pool(
                &vk::CommandPoolCreateInfo::default().queue_family_index(g.qfam).flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }?;
        let queries = unsafe {
            d.create_query_pool(&vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::TIMESTAMP).query_count(2), None)
        }?;

        let attach = [vk::AttachmentDescription::default()
            .format(FORMAT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(final_layout)];
        let refs = [vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let sub = [vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&refs)];
        let rp = unsafe { d.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attach).subpasses(&sub), None) }?;

        // 256x256 checker, uploaded through a staging buffer.
        let tw = 256u32;
        let mut texels = vec![0u8; (tw * tw * 4) as usize];
        for y in 0..tw {
            for x in 0..tw {
                let i = ((y * tw + x) * 4) as usize;
                let c = if (x / 16 + y / 16) % 2 == 0 { 255u8 } else { 40 };
                texels[i] = (y * 255 / tw) as u8; // B
                texels[i + 1] = (x * 255 / tw) as u8; // G
                texels[i + 2] = c; // R
                texels[i + 3] = 255;
            }
        }
        let (tex, tex_mem) = create_image(
            g,
            tw,
            tw,
            vk::Format::B8G8R8A8_UNORM,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        let (stage, stage_mem, ptr) = create_host_buffer(g, texels.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC)?;
        unsafe { std::ptr::copy_nonoverlapping(texels.as_ptr(), ptr as *mut u8, texels.len()) };
        one_shot(g, cmd_pool, |cb| unsafe {
            barrier(d, cb, tex, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
            d.cmd_copy_buffer_to_image(
                cb,
                stage,
                tex,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[vk::BufferImageCopy::default()
                    .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                    .image_extent(vk::Extent3D { width: tw, height: tw, depth: 1 })],
            );
            barrier(d, cb, tex, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        })?;
        unsafe {
            d.destroy_buffer(stage, None);
            d.free_memory(stage_mem, None);
        }
        let tex_view = unsafe {
            d.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(tex)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::B8G8R8A8_UNORM)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                None,
            )
        }?;
        let sampler = unsafe {
            d.create_sampler(
                &vk::SamplerCreateInfo::default().mag_filter(vk::Filter::LINEAR).min_filter(vk::Filter::LINEAR),
                None,
            )
        }?;

        let bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        let dsl = unsafe { d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None) }?;
        let sizes = [vk::DescriptorPoolSize::default().ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1)];
        let pool = unsafe { d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None) }?;
        let dsls = [dsl];
        let set = unsafe { d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&dsls)) }?[0];
        let ii = [vk::DescriptorImageInfo::default().sampler(sampler).image_view(tex_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        unsafe {
            d.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&ii)],
                &[],
            )
        };

        let pl = unsafe { d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&dsls), None) }?;
        let vs = shader(d, VERT)?;
        let fs = shader(d, FRAG)?;
        let stages = [
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
            vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main"),
        ];
        let vi = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_STRIP);
        let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let rs = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let cba = [vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)];
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&cba);
        let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
        let pipeline = unsafe {
            d.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[vk::GraphicsPipelineCreateInfo::default()
                    .stages(&stages)
                    .vertex_input_state(&vi)
                    .input_assembly_state(&ia)
                    .viewport_state(&vp)
                    .rasterization_state(&rs)
                    .multisample_state(&ms)
                    .color_blend_state(&cb)
                    .dynamic_state(&dy)
                    .layout(pl)
                    .render_pass(rp)
                    .subpass(0)],
                None,
            )
        }
        .map_err(|(_, e)| anyhow!("vkCreateGraphicsPipelines: {e}"))?[0];
        unsafe {
            d.destroy_shader_module(vs, None);
            d.destroy_shader_module(fs, None);
        }
        Ok(Renderer { rp, pipeline, pl, dsl, pool, set, tex, tex_mem, tex_view, sampler, cmd_pool, queries })
    }

    /// Record clear + quad into `fb`, with timestamps around the render pass.
    unsafe fn record(&self, g: &Gpu, cb: vk::CommandBuffer, fb: vk::Framebuffer, w: u32, h: u32, tint: f32) {
        let d = &g.device;
        d.cmd_reset_query_pool(cb, self.queries, 0, 2);
        d.cmd_write_timestamp(cb, vk::PipelineStageFlags::TOP_OF_PIPE, self.queries, 0);
        let clear = [vk::ClearValue { color: vk::ClearColorValue { float32: [0.05 + tint * 0.5, 0.02, 0.12 + tint * 0.3, 1.0] } }];
        d.cmd_begin_render_pass(
            cb,
            &vk::RenderPassBeginInfo::default()
                .render_pass(self.rp)
                .framebuffer(fb)
                .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: w, height: h } })
                .clear_values(&clear),
            vk::SubpassContents::INLINE,
        );
        d.cmd_set_viewport(cb, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 }]);
        d.cmd_set_scissor(cb, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: w, height: h } }]);
        d.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
        d.cmd_bind_descriptor_sets(cb, vk::PipelineBindPoint::GRAPHICS, self.pl, 0, &[self.set], &[]);
        d.cmd_draw(cb, 4, 1, 0, 0);
        d.cmd_end_render_pass(cb);
        d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, 1);
    }

    fn gpu_ms(&self, g: &Gpu) -> f64 {
        let mut ts = [0u64; 2];
        let ok = unsafe { g.device.get_query_pool_results(self.queries, 0, &mut ts, vk::QueryResultFlags::TYPE_64) };
        if ok.is_err() {
            return f64::NAN;
        }
        (ts[1].wrapping_sub(ts[0])) as f64 * g.timestamp_period as f64 / 1.0e6
    }

    fn destroy(&self, g: &Gpu) {
        let d = &g.device;
        unsafe {
            d.destroy_pipeline(self.pipeline, None);
            d.destroy_pipeline_layout(self.pl, None);
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.dsl, None);
            d.destroy_sampler(self.sampler, None);
            d.destroy_image_view(self.tex_view, None);
            d.destroy_image(self.tex, None);
            d.free_memory(self.tex_mem, None);
            d.destroy_render_pass(self.rp, None);
            d.destroy_query_pool(self.queries, None);
            d.destroy_command_pool(self.cmd_pool, None);
        }
    }
}

// ----------------------------------------------------------------- helpers ---

fn shader(d: &ash::Device, spv: &[u8]) -> Result<vk::ShaderModule> {
    let words: Vec<u32> = spv.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    Ok(unsafe { d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None) }?)
}

unsafe fn barrier(d: &ash::Device, cb: vk::CommandBuffer, image: vk::Image, from: vk::ImageLayout, to: vk::ImageLayout) {
    d.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::ALL_COMMANDS,
        vk::PipelineStageFlags::ALL_COMMANDS,
        vk::DependencyFlags::empty(),
        &[],
        &[],
        &[vk::ImageMemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::MEMORY_WRITE)
            .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
            .old_layout(from)
            .new_layout(to)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1))],
    );
}

fn one_shot(g: &Gpu, pool: vk::CommandPool, f: impl FnOnce(vk::CommandBuffer)) -> Result<()> {
    let d = &g.device;
    let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(pool).command_buffer_count(1)) }?[0];
    unsafe {
        d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
        f(cb);
        d.end_command_buffer(cb)?;
        let cbs = [cb];
        d.queue_submit(g.queue, &[vk::SubmitInfo::default().command_buffers(&cbs)], vk::Fence::null())?;
        d.queue_wait_idle(g.queue)?;
        d.free_command_buffers(pool, &cbs);
    }
    Ok(())
}

fn create_image(
    g: &Gpu,
    w: u32,
    h: u32,
    format: vk::Format,
    tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags,
    props: vk::MemoryPropertyFlags,
) -> Result<(vk::Image, vk::DeviceMemory)> {
    let d = &g.device;
    let img = unsafe {
        d.create_image(
            &vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D { width: w, height: h, depth: 1 })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(tiling)
                .usage(usage)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED),
            None,
        )
    }?;
    let req = unsafe { d.get_image_memory_requirements(img) };
    let mem = unsafe {
        d.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(g.mem_type(req.memory_type_bits, props)?), None)
    }?;
    unsafe { d.bind_image_memory(img, mem, 0) }?;
    Ok((img, mem))
}

fn create_host_buffer(g: &Gpu, size: u64, usage: vk::BufferUsageFlags) -> Result<(vk::Buffer, vk::DeviceMemory, *mut std::ffi::c_void)> {
    let d = &g.device;
    let buf = unsafe { d.create_buffer(&vk::BufferCreateInfo::default().size(size).usage(usage).sharing_mode(vk::SharingMode::EXCLUSIVE), None) }?;
    let req = unsafe { d.get_buffer_memory_requirements(buf) };
    let idx = g
        .mem_type(req.memory_type_bits, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_CACHED)
        .or_else(|_| g.mem_type(req.memory_type_bits, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT))?;
    let mem = unsafe { d.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(idx), None) }?;
    unsafe { d.bind_buffer_memory(buf, mem, 0) }?;
    let ptr = unsafe { d.map_memory(mem, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty()) }?;
    Ok((buf, mem, ptr))
}

fn framebuffer_for(g: &Gpu, rp: vk::RenderPass, image: vk::Image, w: u32, h: u32) -> Result<(vk::ImageView, vk::Framebuffer)> {
    let d = &g.device;
    let view = unsafe {
        d.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(FORMAT)
                .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
            None,
        )
    }?;
    let views = [view];
    let fb = unsafe { d.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(rp).attachments(&views).width(w).height(h).layers(1), None) }?;
    Ok((view, fb))
}

// ------------------------------------------------------------- Q1: bench ----

fn bench(w: u32, h: u32, frames: usize) -> Result<()> {
    let g = open_gpu()?;
    let r = Renderer::new(&g, vk::ImageLayout::TRANSFER_SRC_OPTIMAL)?;
    let d = &g.device;
    let (img, img_mem) = create_image(
        &g,
        w,
        h,
        FORMAT,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    let (view, fb) = framebuffer_for(&g, r.rp, img, w, h)?;
    let size = (w as u64) * (h as u64) * 4;
    let (buf, buf_mem, ptr) = create_host_buffer(&g, size, vk::BufferUsageFlags::TRANSFER_DST)?;

    let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(r.cmd_pool).command_buffer_count(1)) }?[0];
    let fence = unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?;

    let (mut submit_ms, mut gpu_ms, mut copy_ms) = (vec![], vec![], vec![]);
    let mut out = vec![0u8; size as usize];
    for f in 0..frames {
        let t0 = Instant::now();
        unsafe {
            d.reset_command_buffer(cb, vk::CommandBufferResetFlags::empty())?;
            d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            r.record(&g, cb, fb, w, h, 0.0);
            d.cmd_copy_image_to_buffer(
                cb,
                img,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                buf,
                &[vk::BufferImageCopy::default()
                    .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                    .image_extent(vk::Extent3D { width: w, height: h, depth: 1 })],
            );
            d.end_command_buffer(cb)?;
            d.reset_fences(&[fence])?;
            let cbs = [cb];
            d.queue_submit(g.queue, &[vk::SubmitInfo::default().command_buffers(&cbs)], fence)?;
            d.wait_for_fences(&[fence], true, u64::MAX)?;
        }
        let t1 = Instant::now();
        unsafe { std::ptr::copy_nonoverlapping(ptr as *const u8, out.as_mut_ptr(), out.len()) };
        let t2 = Instant::now();
        if f >= 5 {
            submit_ms.push((t1 - t0).as_secs_f64() * 1e3);
            copy_ms.push((t2 - t1).as_secs_f64() * 1e3);
            gpu_ms.push(r.gpu_ms(&g));
        }
    }
    let px = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        (out[i], out[i + 1], out[i + 2], out[i + 3])
    };
    println!("{w}x{h} BGRA corner(4,4)={:?} centre={:?}", px(4, 4), px(w / 2, h / 2));
    stat("GPU render (timestamps, render pass only)", &gpu_ms);
    stat("submit..fence (render + image->buffer copy)", &submit_ms);
    stat("host memcpy out of the mapped buffer", &copy_ms);

    unsafe {
        d.destroy_fence(fence, None);
        d.unmap_memory(buf_mem);
        d.destroy_buffer(buf, None);
        d.free_memory(buf_mem, None);
        d.destroy_framebuffer(fb, None);
        d.destroy_image_view(view, None);
        d.destroy_image(img, None);
        d.free_memory(img_mem, None);
    }
    r.destroy(&g);
    Ok(())
}

// -------------------------------------------------------- Q2: dma-buf flip ---

struct DmabufImage {
    image: vk::Image,
    mem: vk::DeviceMemory,
    modifier: u64,
    pitch: u32,
    offset: u32,
    fd: OwnedFd,
}

/// Which DRM format modifiers can this device render into for `FORMAT`?
fn usable_modifiers(g: &Gpu) -> Result<Vec<vk::DrmFormatModifierPropertiesEXT>> {
    let mut list = vk::DrmFormatModifierPropertiesListEXT::default();
    let mut p2 = vk::FormatProperties2::default().push_next(&mut list);
    unsafe { g.instance.get_physical_device_format_properties2(g.pdev, FORMAT, &mut p2) };
    let n = list.drm_format_modifier_count as usize;
    if n == 0 {
        return Ok(vec![]);
    }
    let mut props = vec![vk::DrmFormatModifierPropertiesEXT::default(); n];
    let mut list = vk::DrmFormatModifierPropertiesListEXT::default().drm_format_modifier_properties(&mut props);
    let mut p2 = vk::FormatProperties2::default().push_next(&mut list);
    unsafe { g.instance.get_physical_device_format_properties2(g.pdev, FORMAT, &mut p2) };
    Ok(props)
}

fn create_dmabuf_image(g: &Gpu, w: u32, h: u32, mods: &[u64]) -> Result<DmabufImage> {
    let d = &g.device;
    let usage = vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC;
    let mut ext_ci = vk::ExternalMemoryImageCreateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let mut mod_ci = vk::ImageDrmFormatModifierListCreateInfoEXT::default().drm_format_modifiers(mods);
    let ci = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(FORMAT)
        .extent(vk::Extent3D { width: w, height: h, depth: 1 })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .push_next(&mut ext_ci)
        .push_next(&mut mod_ci);
    let image = unsafe { d.create_image(&ci, None) }.context("vkCreateImage(DRM_FORMAT_MODIFIER_EXT)")?;

    let req = unsafe { d.get_image_memory_requirements(image) };
    let mut ded = vk::MemoryDedicatedAllocateInfo::default().image(image);
    let mut exp = vk::ExportMemoryAllocateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let ai = vk::MemoryAllocateInfo::default()
        .allocation_size(req.size)
        .memory_type_index(g.mem_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)?)
        .push_next(&mut ded)
        .push_next(&mut exp);
    let mem = unsafe { d.allocate_memory(&ai, None) }.context("vkAllocateMemory(export dma_buf)")?;
    unsafe { d.bind_image_memory(image, mem, 0) }?;

    let mod_dev = ash::ext::image_drm_format_modifier::Device::new(&g.instance, d);
    let mut mp = vk::ImageDrmFormatModifierPropertiesEXT::default();
    unsafe { mod_dev.get_image_drm_format_modifier_properties(image, &mut mp) }.context("vkGetImageDrmFormatModifierPropertiesEXT")?;

    let layout = unsafe {
        d.get_image_subresource_layout(
            image,
            vk::ImageSubresource::default().aspect_mask(vk::ImageAspectFlags::MEMORY_PLANE_0_EXT).mip_level(0).array_layer(0),
        )
    };

    let fd_dev = ash::khr::external_memory_fd::Device::new(&g.instance, d);
    let raw = unsafe {
        fd_dev.get_memory_fd(&vk::MemoryGetFdInfoKHR::default().memory(mem).handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT))
    }
    .context("vkGetMemoryFdKHR(DMA_BUF)")?;
    Ok(DmabufImage {
        image,
        mem,
        modifier: mp.drm_format_modifier,
        pitch: layout.row_pitch as u32,
        offset: layout.offset as u32,
        fd: unsafe { OwnedFd::from_raw_fd(raw) },
    })
}

fn flip(frames: usize, conn: &str) -> Result<()> {
    use drm::buffer::{DrmFourcc, DrmModifier};
    let g = open_gpu()?;
    if !g.has_modifier_ext {
        bail!("VK_EXT_image_drm_format_modifier not available");
    }
    let out = kms::Output::open(Path::new("/dev/dri/card0"), conn)?;
    println!("connector {conn}: {}x{}@{} crtc={:?}", out.width, out.height, out.mode.vrefresh(), out.crtc);
    let (w, h) = (out.width, out.height);

    let props = usable_modifiers(&g)?;
    println!("device advertises {} DRM format modifiers for B8G8R8A8_UNORM:", props.len());
    for p in &props {
        println!("  0x{:016x} planes={} features={:?}", p.drm_format_modifier, p.drm_format_modifier_plane_count, p.drm_format_modifier_tiling_features);
    }
    let candidates: Vec<u64> = props
        .iter()
        .filter(|p| p.drm_format_modifier_plane_count == 1 && p.drm_format_modifier_tiling_features.contains(vk::FormatFeatureFlags::COLOR_ATTACHMENT))
        .map(|p| p.drm_format_modifier)
        .collect();
    if candidates.is_empty() {
        bail!("no single-plane colour-attachment modifier for B8G8R8A8_UNORM");
    }

    let r = Renderer::new(&g, vk::ImageLayout::GENERAL)?;
    let d = &g.device;
    let mut imgs = Vec::new();
    for _ in 0..2 {
        let di = create_dmabuf_image(&g, w, h, &candidates)?;
        println!("dma-buf image: modifier=0x{:016x} pitch={} offset={} fd={}", di.modifier, di.pitch, di.offset, {
            use std::os::unix::io::AsRawFd;
            di.fd.as_raw_fd()
        });
        let (view, fb) = framebuffer_for(&g, r.rp, di.image, w, h)?;
        imgs.push((di, view, fb));
    }

    // Import each into DRM and make a framebuffer out of it.
    let mut fbs = Vec::new();
    for (di, _, _) in &imgs {
        let dup = di.fd.try_clone()?;
        let (buf, fbh) = out.import_dmabuf(dup, w, h, DrmFourcc::Xrgb8888, DrmModifier::from(di.modifier), di.pitch, di.offset)?;
        println!("imported: drm handle={:?} fb={:?}", buf.handle, fbh);
        fbs.push((buf, fbh));
    }

    let cbs = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(r.cmd_pool).command_buffer_count(2)) }?;
    let fence = unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?;

    out.set_crtc(fbs[0].1)?;
    println!("mode set on {conn}");

    let (mut gpu_ms, mut submit_ms, mut flip_ms) = (vec![], vec![], vec![]);
    let mut last = Instant::now();
    for f in 0..frames {
        let i = f % 2;
        let t0 = Instant::now();
        unsafe {
            d.reset_command_buffer(cbs[i], vk::CommandBufferResetFlags::empty())?;
            d.begin_command_buffer(cbs[i], &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            r.record(&g, cbs[i], imgs[i].2, w, h, (f as f32 / 30.0).sin().abs());
            // Release to the display engine.
            d.cmd_pipeline_barrier(
                cbs[i],
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .dst_access_mask(vk::AccessFlags::empty())
                    .old_layout(vk::ImageLayout::GENERAL)
                    .new_layout(vk::ImageLayout::GENERAL)
                    .src_queue_family_index(g.qfam)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_FOREIGN_EXT)
                    .image(imgs[i].0.image)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1))],
            );
            d.end_command_buffer(cbs[i])?;
            d.reset_fences(&[fence])?;
            let one = [cbs[i]];
            d.queue_submit(g.queue, &[vk::SubmitInfo::default().command_buffers(&one)], fence)?;
            d.wait_for_fences(&[fence], true, u64::MAX)?;
        }
        let t1 = Instant::now();
        out.flip(fbs[i].1)?;
        let t2 = Instant::now();
        if f >= 5 {
            gpu_ms.push(r.gpu_ms(&g));
            submit_ms.push((t1 - t0).as_secs_f64() * 1e3);
            flip_ms.push((t2 - t1).as_secs_f64() * 1e3);
        }
        if f >= 5 {
            let _ = last;
        }
        last = t2;
    }
    let _ = last;
    stat("GPU render into the dma-buf (timestamps)", &gpu_ms);
    stat("submit..fence", &submit_ms);
    stat("page flip queue..flip-complete", &flip_ms);
    println!("held the last frame; sleeping 3 s so it can be seen/photographed");
    std::thread::sleep(std::time::Duration::from_secs(3));

    unsafe { d.device_wait_idle()? };
    unsafe {
        d.destroy_fence(fence, None);
        for (di, view, fb) in &imgs {
            d.destroy_framebuffer(*fb, None);
            d.destroy_image_view(*view, None);
            d.destroy_image(di.image, None);
            d.free_memory(di.mem, None);
        }
    }
    r.destroy(&g);
    Ok(())
}

// ---- Q2c: can Vulkan render straight into a LINEAR dma-buf the KMS driver
// ---- understands without modifiers?

fn fliplinear(frames: usize, conn: &str) -> Result<()> {
    use drm::buffer::{DrmFourcc, DrmModifier};
    use std::os::unix::io::FromRawFd;
    let g = open_gpu()?;
    let d = &g.device;

    // Is LINEAR tiling + COLOR_ATTACHMENT + dma_buf export supported at all?
    let mut ext_info = vk::PhysicalDeviceExternalImageFormatInfo::default().handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(FORMAT)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(vk::ImageTiling::LINEAR)
        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
        .push_next(&mut ext_info);
    let mut ext_props = vk::ExternalImageFormatProperties::default();
    let mut props2 = vk::ImageFormatProperties2::default().push_next(&mut ext_props);
    match unsafe { g.instance.get_physical_device_image_format_properties2(g.pdev, &info, &mut props2) } {
        Ok(()) => println!(
            "LINEAR + COLOR_ATTACHMENT + dma_buf: max={:?} exportable={:?} compatible={:?}",
            props2.image_format_properties.max_extent,
            ext_props.external_memory_properties.external_memory_features,
            ext_props.external_memory_properties.compatible_handle_types
        ),
        Err(e) => bail!("vkGetPhysicalDeviceImageFormatProperties2(LINEAR, COLOR_ATTACHMENT, DMA_BUF) failed: {e}"),
    }

    let out = kms::Output::open(Path::new("/dev/dri/card0"), conn)?;
    let (w, h) = (out.width, out.height);
    let r = Renderer::new(&g, vk::ImageLayout::GENERAL)?;

    let mut imgs = Vec::new();
    for _ in 0..2 {
        let mut ext_ci = vk::ExternalMemoryImageCreateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let image = unsafe {
            d.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(FORMAT)
                    .extent(vk::Extent3D { width: w, height: h, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::LINEAR)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .push_next(&mut ext_ci),
                None,
            )
        }
        .context("vkCreateImage(LINEAR, external)")?;
        let req = unsafe { d.get_image_memory_requirements(image) };
        let mut ded = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let mut exp = vk::ExportMemoryAllocateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let mem = unsafe {
            d.allocate_memory(
                &vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(g.mem_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)?).push_next(&mut ded).push_next(&mut exp),
                None,
            )
        }
        .context("vkAllocateMemory(export, linear)")?;
        unsafe { d.bind_image_memory(image, mem, 0) }?;
        let layout = unsafe {
            d.get_image_subresource_layout(image, vk::ImageSubresource::default().aspect_mask(vk::ImageAspectFlags::COLOR).mip_level(0).array_layer(0))
        };
        let fd_dev = ash::khr::external_memory_fd::Device::new(&g.instance, d);
        let raw = unsafe { fd_dev.get_memory_fd(&vk::MemoryGetFdInfoKHR::default().memory(mem).handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)) }
            .context("vkGetMemoryFdKHR(linear image)")?;
        println!("linear dma-buf: rowPitch={} offset={} size={}", layout.row_pitch, layout.offset, req.size);
        let (view, fb) = framebuffer_for(&g, r.rp, image, w, h)?;
        imgs.push((image, mem, layout.row_pitch as u32, layout.offset as u32, unsafe { OwnedFd::from_raw_fd(raw) }, view, fb));
    }

    let mut fbs = Vec::new();
    for (_, _, pitch, offset, fd, _, _) in &imgs {
        let dup = fd.try_clone()?;
        let (_buf, fbh) = out.import_dmabuf(dup, w, h, DrmFourcc::Xrgb8888, DrmModifier::Invalid, *pitch, *offset)?;
        fbs.push((_buf, fbh));
    }
    println!("AddFB2 (implicit linear) accepted both linear dma-bufs");

    let cbs = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(r.cmd_pool).command_buffer_count(2)) }?;
    let fence = unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?;
    out.set_crtc(fbs[0].1)?;

    let (mut gpu_ms, mut submit_ms, mut flip_ms) = (vec![], vec![], vec![]);
    let cpu0 = cpu_time();
    let wall0 = Instant::now();
    for f in 0..frames {
        let i = f % 2;
        let t0 = Instant::now();
        unsafe {
            d.reset_command_buffer(cbs[i], vk::CommandBufferResetFlags::empty())?;
            d.begin_command_buffer(cbs[i], &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            r.record(&g, cbs[i], imgs[i].6, w, h, (f as f32 / 30.0).sin().abs());
            d.end_command_buffer(cbs[i])?;
            d.reset_fences(&[fence])?;
            let one = [cbs[i]];
            d.queue_submit(g.queue, &[vk::SubmitInfo::default().command_buffers(&one)], fence)?;
            d.wait_for_fences(&[fence], true, u64::MAX)?;
        }
        let t1 = Instant::now();
        out.flip(fbs[i].1)?;
        let t2 = Instant::now();
        if f >= 5 {
            gpu_ms.push(r.gpu_ms(&g));
            submit_ms.push((t1 - t0).as_secs_f64() * 1e3);
            flip_ms.push((t2 - t1).as_secs_f64() * 1e3);
        }
    }
    let cpu = cpu_time() - cpu0;
    let wall = wall0.elapsed().as_secs_f64();
    // Proof: pull the last-scanned-out image back through the GPU and look at it.
    {
        let i = (frames - 1) % 2;
        let size = (imgs[i].2 as u64) * (h as u64);
        let (hb, hm, ptr) = create_host_buffer(&g, size, vk::BufferUsageFlags::TRANSFER_DST)?;
        one_shot(&g, r.cmd_pool, |cb| unsafe {
            barrier(d, cb, imgs[i].0, vk::ImageLayout::GENERAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
            d.cmd_copy_image_to_buffer(cb, imgs[i].0, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, hb,
                &[vk::BufferImageCopy::default().buffer_row_length(imgs[i].2 / 4).buffer_image_height(h)
                    .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                    .image_extent(vk::Extent3D { width: w, height: h, depth: 1 })]);
        })?;
        let b = unsafe { std::slice::from_raw_parts(ptr as *const u8, size as usize) };
        let pitch = imgs[i].2 as usize;
        let px = |x: usize, y: usize| (b[y * pitch + x * 4], b[y * pitch + x * 4 + 1], b[y * pitch + x * 4 + 2], b[y * pitch + x * 4 + 3]);
        println!("scanout dma-buf BGRA corner(4,4)={:?} centre={:?} nonzero={}/{}", px(4, 4), px(w as usize / 2, h as usize / 2), b.iter().filter(|v| **v != 0).count(), b.len());
        unsafe { d.unmap_memory(hm); d.destroy_buffer(hb, None); d.free_memory(hm, None); }
    }
    stat("GPU render straight into the scanout dma-buf", &gpu_ms);
    stat("submit..fence", &submit_ms);
    stat("page flip queue..flip-complete", &flip_ms);
    println!("wall={wall:.2}s cpu={cpu:.2}s -> {:.3} CPU core-seconds per second, {:.1} fps", cpu / wall, frames as f64 / wall);
    std::thread::sleep(std::time::Duration::from_secs(3));
    unsafe { d.device_wait_idle()? };
    Ok(())
}

// ------ Q2b: DRM owns the buffer, the GPU writes into it (no CPU copy) -------

/// Import a dma-buf fd as a VkBuffer so the GPU can blit into scanout memory.
fn import_dmabuf_buffer(g: &Gpu, fd: OwnedFd, size: u64) -> Result<(vk::Buffer, vk::DeviceMemory)> {
    use std::os::unix::io::IntoRawFd;
    let d = &g.device;
    let mut ext = vk::ExternalMemoryBufferCreateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let buf = unsafe {
        d.create_buffer(
            &vk::BufferCreateInfo::default().size(size).usage(vk::BufferUsageFlags::TRANSFER_DST).sharing_mode(vk::SharingMode::EXCLUSIVE).push_next(&mut ext),
            None,
        )
    }
    .context("vkCreateBuffer(external dma_buf)")?;
    let req = unsafe { d.get_buffer_memory_requirements(buf) };

    let fd_dev = ash::khr::external_memory_fd::Device::new(&g.instance, d);
    let raw = fd.into_raw_fd();
    let mut fdprops = vk::MemoryFdPropertiesKHR::default();
    unsafe { fd_dev.get_memory_fd_properties(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT, raw, &mut fdprops) }
        .context("vkGetMemoryFdPropertiesKHR(DMA_BUF)")?;
    let bits = req.memory_type_bits & fdprops.memory_type_bits;
    println!("import: req.size={} req.bits={:#x} fd.bits={:#x} -> {:#x}", req.size, req.memory_type_bits, fdprops.memory_type_bits, bits);
    let idx = (0..32).find(|i| bits & (1 << i) != 0).ok_or_else(|| anyhow!("no memory type accepts this dma-buf"))? as u32;

    let mut imp = vk::ImportMemoryFdInfoKHR::default().handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT).fd(raw);
    let mut ded = vk::MemoryDedicatedAllocateInfo::default().buffer(buf);
    let mem = unsafe {
        d.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size.max(size)).memory_type_index(idx).push_next(&mut imp).push_next(&mut ded), None)
    }
    .context("vkAllocateMemory(import dma_buf)")?;
    unsafe { d.bind_buffer_memory(buf, mem, 0) }.context("vkBindBufferMemory(imported)")?;
    Ok((buf, mem))
}

fn gpucopy(frames: usize, conn: &str) -> Result<()> {
    use drm::buffer::{Buffer, DrmFourcc};
    use drm::control::Device as _;
    let g = open_gpu()?;
    let out = kms::Output::open(Path::new("/dev/dri/card0"), conn)?;
    let (w, h) = (out.width, out.height);
    println!("connector {conn}: {w}x{h}@{}", out.mode.vrefresh());

    let r = Renderer::new(&g, vk::ImageLayout::TRANSFER_SRC_OPTIMAL)?;
    let d = &g.device;
    let (img, img_mem) = create_image(
        &g, w, h, FORMAT, vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    let (view, vfb) = framebuffer_for(&g, r.rp, img, w, h)?;

    // Two dumb buffers, each exported to the GPU as a dma-buf.
    let mut targets = Vec::new();
    for _ in 0..2 {
        let db = out.card.create_dumb_buffer((w, h), DrmFourcc::Xrgb8888, 32)?;
        let fb = out.card.add_framebuffer(&db, 24, 32)?;
        let pitch = db.pitch();
        let size = (pitch as u64) * (h as u64);
        let fd = out.export_prime(db.handle())?;
        let (vbuf, vmem) = import_dmabuf_buffer(&g, fd, size)?;
        println!("dumb buffer pitch={pitch} size={size} imported into Vulkan");
        targets.push((db, fb, vbuf, vmem, pitch));
    }

    let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(r.cmd_pool).command_buffer_count(1)) }?[0];
    let fence = unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?;
    out.set_crtc(targets[0].1)?;
    println!("mode set on {conn}");

    let (mut gpu_ms, mut submit_ms, mut flip_ms) = (vec![], vec![], vec![]);
    let cpu0 = cpu_time();
    let wall0 = Instant::now();
    for f in 0..frames {
        let i = f % 2;
        let t0 = Instant::now();
        unsafe {
            d.reset_command_buffer(cb, vk::CommandBufferResetFlags::empty())?;
            d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            r.record(&g, cb, vfb, w, h, (f as f32 / 30.0).sin().abs());
            d.cmd_copy_image_to_buffer(
                cb, img, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, targets[i].2,
                &[vk::BufferImageCopy::default()
                    .buffer_row_length(targets[i].4 / 4)
                    .buffer_image_height(h)
                    .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                    .image_extent(vk::Extent3D { width: w, height: h, depth: 1 })],
            );
            d.end_command_buffer(cb)?;
            d.reset_fences(&[fence])?;
            let one = [cb];
            d.queue_submit(g.queue, &[vk::SubmitInfo::default().command_buffers(&one)], fence)?;
            d.wait_for_fences(&[fence], true, u64::MAX)?;
        }
        let t1 = Instant::now();
        out.flip(targets[i].1)?;
        let t2 = Instant::now();
        if f >= 5 {
            gpu_ms.push(r.gpu_ms(&g));
            submit_ms.push((t1 - t0).as_secs_f64() * 1e3);
            flip_ms.push((t2 - t1).as_secs_f64() * 1e3);
        }
    }
    let cpu = cpu_time() - cpu0;
    let wall = wall0.elapsed().as_secs_f64();
    // Proof the scanned-out bytes really are the rendered frame: map the dumb
    // buffer the display is showing and look at it.
    {
        let i = (frames - 1) % 2;
        let pitch = targets[i].4 as usize;
        let mut map = out.card.map_dumb_buffer(&mut targets[i].0)?;
        let b = map.as_mut();
        let px = |x: usize, y: usize| (b[y * pitch + x * 4], b[y * pitch + x * 4 + 1], b[y * pitch + x * 4 + 2], b[y * pitch + x * 4 + 3]);
        let nonzero = b.iter().filter(|v| **v != 0).count();
        println!("scanout buffer BGRA corner(4,4)={:?} centre={:?} nonzero_bytes={}/{}", px(4, 4), px(w as usize / 2, h as usize / 2), nonzero, b.len());
    }
    stat("GPU render (timestamps)", &gpu_ms);
    stat("submit..fence (render + GPU blit into the scanout dma-buf)", &submit_ms);
    stat("page flip queue..flip-complete", &flip_ms);
    println!("wall={wall:.2}s cpu={cpu:.2}s -> {:.3} CPU core-seconds per second, {:.1} fps", cpu / wall, frames as f64 / wall);
    std::thread::sleep(std::time::Duration::from_secs(3));

    unsafe {
        d.device_wait_idle()?;
        d.destroy_fence(fence, None);
        for (_, _, vbuf, vmem, _) in &targets {
            d.destroy_buffer(*vbuf, None);
            d.free_memory(*vmem, None);
        }
        d.destroy_framebuffer(vfb, None);
        d.destroy_image_view(view, None);
        d.destroy_image(img, None);
        d.free_memory(img_mem, None);
    }
    r.destroy(&g);
    Ok(())
}

// ------------------------------------------------------- Q3: dumb readback ---

fn dumb(frames: usize, conn: &str) -> Result<()> {
    use drm::buffer::{Buffer, DrmFourcc};
    use drm::control::Device as _;
    let g = open_gpu()?;
    let out = kms::Output::open(Path::new("/dev/dri/card0"), conn)?;
    let (w, h) = (out.width, out.height);
    println!("connector {conn}: {w}x{h}@{}", out.mode.vrefresh());

    let r = Renderer::new(&g, vk::ImageLayout::TRANSFER_SRC_OPTIMAL)?;
    let d = &g.device;
    let (img, img_mem) = create_image(
        &g,
        w,
        h,
        FORMAT,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;
    let (view, vfb) = framebuffer_for(&g, r.rp, img, w, h)?;
    let size = (w as u64) * (h as u64) * 4;
    let (buf, buf_mem, ptr) = create_host_buffer(&g, size, vk::BufferUsageFlags::TRANSFER_DST)?;

    let mut dbs = Vec::new();
    for _ in 0..2 {
        let db = out.card.create_dumb_buffer((w, h), DrmFourcc::Xrgb8888, 32)?;
        let fb = out.card.add_framebuffer(&db, 24, 32)?;
        dbs.push((db, fb));
    }
    println!("dumb buffers: pitch={} len={}", dbs[0].0.pitch(), dbs[0].0.size().0);

    let cb = unsafe { d.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(r.cmd_pool).command_buffer_count(1)) }?[0];
    let fence = unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?;
    out.set_crtc(dbs[0].1)?;

    let (mut gpu_ms, mut submit_ms, mut copy_ms, mut flip_ms) = (vec![], vec![], vec![], vec![]);
    let cpu0 = cpu_time();
    let wall0 = Instant::now();
    for f in 0..frames {
        let i = f % 2;
        let t0 = Instant::now();
        unsafe {
            d.reset_command_buffer(cb, vk::CommandBufferResetFlags::empty())?;
            d.begin_command_buffer(cb, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;
            r.record(&g, cb, vfb, w, h, (f as f32 / 30.0).sin().abs());
            d.cmd_copy_image_to_buffer(
                cb,
                img,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                buf,
                &[vk::BufferImageCopy::default()
                    .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                    .image_extent(vk::Extent3D { width: w, height: h, depth: 1 })],
            );
            d.end_command_buffer(cb)?;
            d.reset_fences(&[fence])?;
            let one = [cb];
            d.queue_submit(g.queue, &[vk::SubmitInfo::default().command_buffers(&one)], fence)?;
            d.wait_for_fences(&[fence], true, u64::MAX)?;
        }
        let t1 = Instant::now();
        {
            let pitch = dbs[i].0.pitch() as usize;
            let mut map = out.card.map_dumb_buffer(&mut dbs[i].0)?;
            let dst = map.as_mut();
            let row = (w * 4) as usize;
            for y in 0..h as usize {
                unsafe { std::ptr::copy_nonoverlapping((ptr as *const u8).add(y * row), dst.as_mut_ptr().add(y * pitch), row) };
            }
        }
        let t2 = Instant::now();
        out.flip(dbs[i].1)?;
        let t3 = Instant::now();
        if f >= 5 {
            gpu_ms.push(r.gpu_ms(&g));
            submit_ms.push((t1 - t0).as_secs_f64() * 1e3);
            copy_ms.push((t2 - t1).as_secs_f64() * 1e3);
            flip_ms.push((t3 - t2).as_secs_f64() * 1e3);
        }
    }
    let cpu = cpu_time() - cpu0;
    let wall = wall0.elapsed().as_secs_f64();
    stat("GPU render (timestamps)", &gpu_ms);
    stat("submit..fence (render + image->buffer copy)", &submit_ms);
    stat("memcpy into the dumb buffer", &copy_ms);
    stat("page flip queue..flip-complete", &flip_ms);
    println!("wall={wall:.2}s cpu={cpu:.2}s -> {:.3} CPU core-seconds per second, {:.1} fps", cpu / wall, frames as f64 / wall);
    std::thread::sleep(std::time::Duration::from_secs(2));

    unsafe {
        d.device_wait_idle()?;
        d.destroy_fence(fence, None);
        d.unmap_memory(buf_mem);
        d.destroy_buffer(buf, None);
        d.free_memory(buf_mem, None);
        d.destroy_framebuffer(vfb, None);
        d.destroy_image_view(view, None);
        d.destroy_image(img, None);
        d.free_memory(img_mem, None);
    }
    r.destroy(&g);
    Ok(())
}

/// Process CPU time (user + system), seconds.
fn cpu_time() -> f64 {
    let mut u = std::mem::MaybeUninit::<libc_rusage>::uninit();
    unsafe {
        getrusage(0, u.as_mut_ptr());
        let u = u.assume_init();
        u.ru_utime.tv_sec as f64 + u.ru_utime.tv_usec as f64 / 1e6 + u.ru_stime.tv_sec as f64 + u.ru_stime.tv_usec as f64 / 1e6
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Timeval {
    tv_sec: i64,
    tv_usec: i64,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct libc_rusage {
    ru_utime: Timeval,
    ru_stime: Timeval,
    rest: [i64; 14],
}
extern "C" {
    fn getrusage(who: i32, usage: *mut libc_rusage) -> i32;
}
