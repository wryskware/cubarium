//! One `VK_IMAGE_TILING_LINEAR` colour attachment exported as a dma-buf, which is what
//! both board targets — the direct KMS flip and GS-2's shim socket — hand to the display.
//!
//! Variant (c) of `gpu-scanout-spike-2026-09-16.md` Q2: the present pass renders
//! *straight into* the memory the panel scans out. No readback, no blit, the CPU never
//! touches a pixel. `VK_EXT_image_drm_format_modifier` is deliberately unused — the blob
//! advertises only UBWC for this format and this 5.4 kernel rejects it; a UBWC buffer
//! passed through a plain `AddFB2` is *accepted* and flips at 60 Hz while showing
//! garbage, which is the nastiest trap on the board.

use std::os::unix::io::{FromRawFd, OwnedFd};

use anyhow::{Context, Result};
use ash::vk;

use crate::vk::Gpu;

/// DRM's `XR24` is B, G, R, X in memory, i.e. `VK_FORMAT_B8G8R8A8_UNORM`.
pub const FORMAT: vk::Format = vk::Format::B8G8R8A8_UNORM;

/// A scanout image and everything the importing side needs to describe it.
pub struct LinearImage {
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    /// The view's format: `_SRGB` when the hardware does the encode, `FORMAT` when the
    /// present shader must.
    pub view_format: vk::Format,
    pub pitch: u32,
    pub offset: u32,
    /// The exported dma-buf. Taken once, by whoever imports it.
    pub fd: Option<OwnedFd>,
}

/// Whether `B8G8R8A8_UNORM` linear + colour attachment + dma-buf export works at all.
///
/// Asked every run rather than assumed from the spike: a driver update that withdrew it
/// would otherwise show up as a black panel with no error.
pub fn linear_export_supported(gpu: &Gpu) -> Result<()> {
    let mut external = vk::PhysicalDeviceExternalImageFormatInfo::default()
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(FORMAT)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(vk::ImageTiling::LINEAR)
        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
        .push_next(&mut external);
    let mut props = vk::ImageFormatProperties2::default();
    unsafe {
        gpu.instance
            .get_physical_device_image_format_properties2(gpu.pdev, &info, &mut props)
    }
    .context("LINEAR + COLOR_ATTACHMENT + dma_buf is not supported on this device")
}

/// Whether the same image can carry a `B8G8R8A8_SRGB` view, which is what lets the
/// hardware do the sRGB encode into a DRM `XR24` buffer (`presenter-budget` W7 becoming
/// free rather than merely cheaper). When it cannot, `present.frag` encodes instead.
pub fn srgb_view_supported(gpu: &Gpu) -> bool {
    let mut external = vk::PhysicalDeviceExternalImageFormatInfo::default()
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(FORMAT)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(vk::ImageTiling::LINEAR)
        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
        .flags(vk::ImageCreateFlags::MUTABLE_FORMAT)
        .push_next(&mut external);
    let mut props = vk::ImageFormatProperties2::default();
    unsafe {
        gpu.instance
            .get_physical_device_image_format_properties2(gpu.pdev, &info, &mut props)
            .is_ok()
    }
}

/// Allocate one exportable linear scanout image.
///
/// `shader_encode` selects the fallback: `true` makes a plain UNORM image whose view is
/// UNORM too (the present shader encodes), `false` makes a mutable-format image with an
/// `_SRGB` view.
pub fn export_linear(
    gpu: &Gpu,
    width: u32,
    height: u32,
    shader_encode: bool,
) -> Result<LinearImage> {
    let d = &gpu.device;
    let mut external = vk::ExternalMemoryImageCreateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let mut create = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(FORMAT)
        .extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::LINEAR)
        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    if !shader_encode {
        create = create.flags(vk::ImageCreateFlags::MUTABLE_FORMAT);
    }
    let image = unsafe { d.create_image(&create.push_next(&mut external), None) }
        .context("vkCreateImage(LINEAR, external)")?;
    let requirements = unsafe { d.get_image_memory_requirements(image) };
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
    let mut export = vk::ExportMemoryAllocateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let memory = unsafe {
        d.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(gpu.memory_type(
                    requirements.memory_type_bits,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )?)
                .push_next(&mut dedicated)
                .push_next(&mut export),
            None,
        )
    }
    .context("vkAllocateMemory(export, linear)")?;
    unsafe { d.bind_image_memory(image, memory, 0) }?;
    let layout = unsafe {
        d.get_image_subresource_layout(
            image,
            vk::ImageSubresource::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(0)
                .array_layer(0),
        )
    };
    let view_format = if shader_encode {
        FORMAT
    } else {
        vk::Format::B8G8R8A8_SRGB
    };
    let view = gpu.view(image, view_format)?;
    let fd_device = ash::khr::external_memory_fd::Device::new(&gpu.instance, d);
    let raw = unsafe {
        fd_device.get_memory_fd(
            &vk::MemoryGetFdInfoKHR::default()
                .memory(memory)
                .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT),
        )
    }
    .context("vkGetMemoryFdKHR(linear image)")?;
    Ok(LinearImage {
        image,
        memory,
        view,
        view_format,
        pitch: layout.row_pitch as u32,
        offset: layout.offset as u32,
        fd: Some(unsafe { OwnedFd::from_raw_fd(raw) }),
    })
}

/// The image's rows as RGBA8, with the row padding dropped and B and R swapped: the
/// bytes a PNG wants, read back out of the buffer the panel is actually scanning out.
pub fn read_back(
    gpu: &Gpu,
    pool: vk::CommandPool,
    image: &LinearImage,
    width: u32,
    height: u32,
    layout: vk::ImageLayout,
) -> Result<Vec<u8>> {
    let pitch = image.pitch as usize;
    let host = gpu.host_buffer(
        u64::from(image.pitch) * u64::from(height),
        vk::BufferUsageFlags::TRANSFER_DST,
    )?;
    let d = &gpu.device;
    gpu.one_shot(pool, |cb| unsafe {
        crate::vk::barrier(
            d,
            cb,
            image.image,
            layout,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        );
        d.cmd_copy_image_to_buffer(
            cb,
            image.image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            host.buffer,
            &[vk::BufferImageCopy::default()
                .buffer_row_length(image.pitch / 4)
                .buffer_image_height(height)
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .layer_count(1),
                )
                .image_extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })],
        );
        crate::vk::barrier(
            d,
            cb,
            image.image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            layout,
        );
    })?;
    let bytes = unsafe { host.bytes() };
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let s = y * pitch + x * 4;
            let t = (y * width as usize + x) * 4;
            rgba[t] = bytes[s + 2];
            rgba[t + 1] = bytes[s + 1];
            rgba[t + 2] = bytes[s];
            rgba[t + 3] = 255;
        }
    }
    host.destroy(gpu);
    Ok(rgba)
}
