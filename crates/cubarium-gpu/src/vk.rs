//! The Vulkan device, opened the way `design/7_Research/gpu-scanout-spike-2026-09-16.md`
//! found works on this board, plus the handful of allocation helpers every pass needs.
//!
//! Three things the spike learned are load-bearing here and are **not** obvious:
//!
//! * There is no ICD manifest on the Tachyon; Qualcomm's loader finds
//!   `libvulkan_adreno.so` itself and `ash::Entry::load()` just works.
//! * `VK_EXT_image_drm_format_modifier` is deliberately **not** enabled. The blob
//!   advertises exactly one modifier for `B8G8R8A8_UNORM` — UBWC — and the 5.4
//!   downstream KMS driver rejects it, so the only scanout path is a modifier-free
//!   `AddFB2` over a `VK_IMAGE_TILING_LINEAR` image.
//! * The device reports API 1.1.128, so nothing above Vulkan 1.1 may be used: no
//!   dynamic rendering, no timeline semaphores, no `VK_KHR_synchronization2`.

use std::ffi::{CStr, c_char};

use anyhow::{Context, Result, anyhow};
use ash::vk;

/// An opened physical device and its one graphics queue.
pub struct Gpu {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub pdev: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue: vk::Queue,
    pub queue_family: u32,
    pub memory: vk::PhysicalDeviceMemoryProperties,
    /// Nanoseconds per timestamp tick; how GPU times are measured.
    pub timestamp_period: f32,
    /// The device's reported name, for the report.
    pub name: String,
    /// Whether `VK_EXT_external_memory_dma_buf` and friends came up, i.e. whether
    /// [`crate::target::Scanout`] can work at all.
    pub has_dma_buf: bool,
}

impl Gpu {
    /// Open the first physical device, with the instance extensions the dma-buf export
    /// needs and any of `extra_instance_extensions` the loader has (the window target
    /// adds the surface extensions through that).
    pub fn open(extra_instance_extensions: &[&CStr]) -> Result<Gpu> {
        let entry = unsafe { ash::Entry::load() }.context("load libvulkan.so.1")?;
        let app = vk::ApplicationInfo::default()
            .application_name(c"cubarium-gpu")
            .api_version(vk::make_api_version(0, 1, 1, 0));
        let mut inst_exts: Vec<*const c_char> = vec![
            ash::khr::get_physical_device_properties2::NAME.as_ptr(),
            ash::khr::external_memory_capabilities::NAME.as_ptr(),
        ];
        inst_exts.extend(extra_instance_extensions.iter().map(|e| e.as_ptr()));
        let instance = unsafe {
            entry.create_instance(
                &vk::InstanceCreateInfo::default()
                    .application_info(&app)
                    .enabled_extension_names(&inst_exts),
                None,
            )
        }
        .context("vkCreateInstance")?;

        let pdev = *unsafe { instance.enumerate_physical_devices() }?
            .first()
            .ok_or_else(|| anyhow!("no Vulkan physical device"))?;
        let props = unsafe { instance.get_physical_device_properties(pdev) };
        let name = unsafe { CStr::from_ptr(props.device_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();

        let available: Vec<String> = unsafe { instance.enumerate_device_extension_properties(pdev) }?
            .iter()
            .map(|e| unsafe { CStr::from_ptr(e.extension_name.as_ptr()) }.to_string_lossy().into_owned())
            .collect();
        // No `image_drm_format_modifier`: see the module comment.
        let wanted = [
            ash::khr::external_memory::NAME,
            ash::khr::external_memory_fd::NAME,
            ash::ext::external_memory_dma_buf::NAME,
            ash::khr::bind_memory2::NAME,
            ash::khr::get_memory_requirements2::NAME,
            ash::khr::dedicated_allocation::NAME,
            ash::khr::maintenance1::NAME,
            ash::khr::swapchain::NAME,
        ];
        let enabled: Vec<&CStr> = wanted
            .into_iter()
            .filter(|n| available.iter().any(|a| a.as_str() == n.to_str().unwrap_or("")))
            .collect();
        let has_dma_buf = enabled.contains(&ash::ext::external_memory_dma_buf::NAME)
            && enabled.contains(&ash::khr::external_memory_fd::NAME);
        let ptrs: Vec<*const c_char> = enabled.iter().map(|n| n.as_ptr()).collect();

        let families = unsafe { instance.get_physical_device_queue_family_properties(pdev) };
        let queue_family = families
            .iter()
            .position(|q| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .ok_or_else(|| anyhow!("no graphics queue family"))? as u32;
        let priorities = [1.0f32];
        let queues = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&priorities)];
        let device = unsafe {
            instance.create_device(
                pdev,
                &vk::DeviceCreateInfo::default()
                    .queue_create_infos(&queues)
                    .enabled_extension_names(&ptrs),
                None,
            )
        }
        .context("vkCreateDevice")?;
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        Ok(Gpu {
            memory: unsafe { instance.get_physical_device_memory_properties(pdev) },
            timestamp_period: props.limits.timestamp_period,
            entry,
            instance,
            pdev,
            device,
            queue,
            queue_family,
            name,
            has_dma_buf,
        })
    }

    /// The first memory type in `bits` with all of `want`.
    pub fn memory_type(&self, bits: u32, want: vk::MemoryPropertyFlags) -> Result<u32> {
        (0..self.memory.memory_type_count)
            .find(|i| {
                bits & (1 << i) != 0
                    && self.memory.memory_types[*i as usize].property_flags.contains(want)
            })
            .ok_or_else(|| anyhow!("no memory type for bits={bits:#x} want={want:?}"))
    }

    /// A device-local image with no external memory.
    pub fn image(
        &self,
        width: u32,
        height: u32,
        format: vk::Format,
        tiling: vk::ImageTiling,
        usage: vk::ImageUsageFlags,
    ) -> Result<(vk::Image, vk::DeviceMemory)> {
        let d = &self.device;
        let image = unsafe {
            d.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(format)
                    .extent(vk::Extent3D { width, height, depth: 1 })
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
        let req = unsafe { d.get_image_memory_requirements(image) };
        let memory = unsafe {
            d.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(req.size)
                    .memory_type_index(
                        self.memory_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)?,
                    ),
                None,
            )
        }?;
        unsafe { d.bind_image_memory(image, memory, 0) }?;
        Ok((image, memory))
    }

    /// A 2D colour view of a whole image.
    pub fn view(&self, image: vk::Image, format: vk::Format) -> Result<vk::ImageView> {
        Ok(unsafe {
            self.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
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

    /// A host-visible, persistently mapped buffer: how every per-frame upload travels.
    pub fn host_buffer(&self, size: u64, usage: vk::BufferUsageFlags) -> Result<HostBuffer> {
        let d = &self.device;
        let buffer = unsafe {
            d.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(size.max(4))
                    .usage(usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
        }?;
        let req = unsafe { d.get_buffer_memory_requirements(buffer) };
        let want = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
        let index = self.memory_type(req.memory_type_bits, want)?;
        let memory = unsafe {
            d.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(req.size)
                    .memory_type_index(index),
                None,
            )
        }?;
        unsafe { d.bind_buffer_memory(buffer, memory, 0) }?;
        let ptr = unsafe { d.map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty()) }?;
        Ok(HostBuffer { buffer, memory, ptr: ptr as *mut u8, size: req.size })
    }

    /// Run a one-shot command buffer to completion.
    pub fn one_shot(&self, pool: vk::CommandPool, f: impl FnOnce(vk::CommandBuffer)) -> Result<()> {
        let d = &self.device;
        let cb = unsafe {
            d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default().command_pool(pool).command_buffer_count(1),
            )
        }?[0];
        unsafe {
            d.begin_command_buffer(
                cb,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            f(cb);
            d.end_command_buffer(cb)?;
            let one = [cb];
            d.queue_submit(self.queue, &[vk::SubmitInfo::default().command_buffers(&one)], vk::Fence::null())?;
            d.queue_wait_idle(self.queue)?;
            d.free_command_buffers(pool, &one);
        }
        Ok(())
    }

    /// A shader module from SPIR-V bytes.
    pub fn shader(&self, spv: &[u8]) -> Result<vk::ShaderModule> {
        let words: Vec<u32> = spv
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Ok(unsafe {
            self.device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
        }?)
    }
}

/// A mapped host buffer, freed on drop by its owner (not by `Drop`: the device must
/// outlive it and Rust cannot express that here without a lifetime everywhere).
pub struct HostBuffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub ptr: *mut u8,
    pub size: u64,
}

impl HostBuffer {
    /// Copy a `Pod` slice into the buffer, truncated at its size.
    pub fn write<T: bytemuck::Pod>(&self, data: &[T]) {
        self.write_at(0, data);
    }

    /// Copy a `Pod` slice in at element `first`, truncated at the buffer's size.
    ///
    /// This is how the instance buffer is filled: one call per layer, straight from the
    /// scene's own vectors, so a frame's four thousand instances are copied **once**
    /// rather than concatenated into a scratch `Vec` and copied again.
    pub fn write_at<T: bytemuck::Pod>(&self, first: usize, data: &[T]) {
        let offset = first * std::mem::size_of::<T>();
        let bytes = bytemuck::cast_slice(data);
        let n = bytes.len().min((self.size as usize).saturating_sub(offset));
        if n > 0 {
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.ptr.add(offset), n) };
        }
    }

    /// Read the buffer back as bytes.
    ///
    /// # Safety
    /// The caller must have waited for every GPU write into this buffer.
    pub unsafe fn bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.size as usize) }
    }

    pub fn destroy(&self, gpu: &Gpu) {
        unsafe {
            gpu.device.unmap_memory(self.memory);
            gpu.device.destroy_buffer(self.buffer, None);
            gpu.device.free_memory(self.memory, None);
        }
    }
}

/// A blunt whole-pipeline image barrier. Vulkan 1.1 without `synchronization2`, and at
/// two or three barriers a frame the precision is not worth the risk of getting the
/// masks wrong on a driver from 2023.
///
/// # Safety
/// `cb` must be recording.
pub unsafe fn barrier(
    device: &ash::Device,
    cb: vk::CommandBuffer,
    image: vk::Image,
    from: vk::ImageLayout,
    to: vk::ImageLayout,
) {
    unsafe {
        device.cmd_pipeline_barrier(
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
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1),
                )],
        )
    };
}
