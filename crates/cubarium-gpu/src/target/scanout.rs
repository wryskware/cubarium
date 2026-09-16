//! The board: render straight into memory the panel scans out, and page-flip it.
//!
//! This is variant (c) of `gpu-scanout-spike-2026-09-16.md` Q2, the only one that both
//! works and shows the rendered image: a small ring of `VK_IMAGE_TILING_LINEAR`
//! `B8G8R8A8_UNORM` images with `VkExportMemoryAllocateInfo(DMA_BUF)`, each imported
//! once at startup with `drmPrimeFDToHandle` + a modifier-free `AddFB2` at the pitch
//! `vkGetImageSubresourceLayout` reports, then legacy `page_flip` on DP-1. The CPU never
//! touches a pixel and there is no blit: the present pass writes into the scanned-out
//! memory itself.
//!
//! **Why not the obvious alternatives.** `VK_EXT_image_drm_format_modifier` offers only
//! UBWC here and this kernel rejects it; passing a UBWC buffer through a plain `AddFB2`
//! is *accepted* and flips at 60 Hz while displaying garbage, which is the nastiest trap
//! on the board. A readback into a KMS dumb buffer costs 17.9 ms of memcpy and cannot
//! reach 60 fps at all.
//!
//! **sRGB.** The scanout image must be `B8G8R8A8_UNORM` for DRM's `XR24`, so the present
//! pass cannot lean on an `_SRGB` attachment for the encode. Two routes are tried in
//! order: a mutable-format image with an `_SRGB` view (the hardware encodes, free), and
//! failing that the shader's own encode (`present.frag`, five instructions at panel
//! resolution). Which one this board took is printed at startup and recorded in the
//! report.

use std::os::unix::io::{FromRawFd, OwnedFd};
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use ash::vk;
use drm::buffer::DrmFourcc;

use super::kms::Output;
use crate::render::{PresentTransform, Renderer, TargetImage, framebuffer};
use crate::scene::Scene;
use crate::vk::{Gpu, barrier};

/// DRM's `XR24` is B, G, R, X in memory, i.e. `VK_FORMAT_B8G8R8A8_UNORM`.
const FORMAT: vk::Format = vk::Format::B8G8R8A8_UNORM;
/// Two images is enough: the renderer waits for its fence before flipping, so at most
/// one is in scanout and one being drawn.
const RING: usize = 2;

struct Frame {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    target: TargetImage,
    fb: drm::control::framebuffer::Handle,
    command_buffer: vk::CommandBuffer,
    fence: vk::Fence,
}

pub struct Scanout {
    output: Output,
    frames: Vec<Frame>,
    transform: PresentTransform,
    index: usize,
    /// Whether the sRGB encode is the attachment's (false) or the shader's (true).
    shader_encode: bool,
}

impl Scanout {
    /// Take DRM master, allocate the scanout ring and set the mode.
    pub fn open(
        gpu: &Gpu,
        renderer: &mut Renderer,
        connector: &str,
        quarter_turns: u32,
    ) -> Result<Scanout> {
        if !gpu.has_dma_buf {
            bail!("this device has no VK_EXT_external_memory_dma_buf; scanout is impossible");
        }
        // Is LINEAR + COLOR_ATTACHMENT + dma-buf export supported at all? The spike's
        // first question, asked again because a driver update could change the answer
        // and the failure would otherwise be a silent black panel.
        let mut external = vk::PhysicalDeviceExternalImageFormatInfo::default()
            .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
        let info = vk::PhysicalDeviceImageFormatInfo2::default()
            .format(FORMAT)
            .ty(vk::ImageType::TYPE_2D)
            .tiling(vk::ImageTiling::LINEAR)
            .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
            .push_next(&mut external);
        let mut external_props = vk::ExternalImageFormatProperties::default();
        let mut props = vk::ImageFormatProperties2::default().push_next(&mut external_props);
        unsafe { gpu.instance.get_physical_device_image_format_properties2(gpu.pdev, &info, &mut props) }
            .context("LINEAR + COLOR_ATTACHMENT + dma_buf is not supported on this device")?;

        let output = Output::open(Path::new("/dev/dri/card0"), connector)?;
        let (w, h) = (output.width, output.height);
        let transform =
            PresentTransform::fit((renderer.layout.w, renderer.layout.h), (w, h), quarter_turns, true)
                .ok_or_else(|| {
                    anyhow!(
                        "a {}x{} raster does not fit {w}x{h} at {quarter_turns} quarter turn(s)",
                        renderer.layout.w,
                        renderer.layout.h
                    )
                })?;

        // Route 1: one image whose memory the present pass writes through an _SRGB view,
        // so the hardware does the encode. Route 2: a plain UNORM image and the shader's
        // own encode. Both scan out the same bytes.
        let (view_format, shader_encode) = match srgb_view_is_possible(gpu) {
            true => (vk::Format::B8G8R8A8_SRGB, false),
            false => (FORMAT, true),
        };
        let transform = PresentTransform { encode_srgb: shader_encode, ..transform };
        let pass = renderer.present_pass(gpu, view_format, vk::ImageLayout::GENERAL)?;

        let d = &gpu.device;
        let command_buffers = unsafe {
            d.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(renderer.command_pool)
                    .command_buffer_count(RING as u32),
            )
        }?;
        let mut output = output;
        let mut frames = Vec::with_capacity(RING);
        for i in 0..RING {
            let (image, memory, pitch, offset, fd) = export_linear(gpu, w, h, shader_encode)?;
            let view = unsafe {
                d.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(view_format)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .level_count(1)
                                .layer_count(1),
                        ),
                    None,
                )
            }?;
            let fb = output.import_dmabuf(fd, w, h, DrmFourcc::Xrgb8888, pitch, offset)?;
            frames.push(Frame {
                image,
                memory,
                view,
                target: TargetImage {
                    image,
                    view,
                    framebuffer: framebuffer(d, pass, view, w, h)?,
                },
                fb,
                command_buffer: command_buffers[i],
                fence: unsafe { d.create_fence(&vk::FenceCreateInfo::default(), None) }?,
            });
        }
        output.set_crtc(frames[0].fb)?;
        println!(
            "scanout: {RING} linear dma-bufs, AddFB2 accepted, sRGB encode by {}",
            if shader_encode { "the present shader" } else { "the _SRGB attachment" }
        );
        Ok(Scanout { output, frames, transform, index: 0, shader_encode })
    }

    pub fn width(&self) -> u32 {
        self.output.width
    }

    pub fn height(&self) -> u32 {
        self.output.height
    }

    pub fn refresh_hz(&self) -> f64 {
        self.output.refresh_hz()
    }

    pub fn transform(&self) -> PresentTransform {
        self.transform
    }

    /// Whether the sRGB encode is the present shader's rather than the attachment's.
    pub fn shader_encode(&self) -> bool {
        self.shader_encode
    }

    /// Render one frame into the next image of the ring and flip it.
    ///
    /// Returns `(GPU ms, submit..fence ms, flip queue..complete ms)`.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        renderer: &mut Renderer,
        scene: &Scene,
    ) -> Result<(f64, f64, f64)> {
        let d = &gpu.device;
        let i = self.index;
        self.index = (self.index + 1) % RING;
        let frame = &self.frames[i];
        let format = if self.shader_encode { FORMAT } else { vk::Format::B8G8R8A8_SRGB };

        let start = Instant::now();
        unsafe { d.reset_command_buffer(frame.command_buffer, vk::CommandBufferResetFlags::empty()) }?;
        renderer.record(
            gpu,
            frame.command_buffer,
            scene,
            Some((
                &frame.target,
                (self.output.width, self.output.height),
                format,
                vk::ImageLayout::GENERAL,
                self.transform,
            )),
        )?;
        let one = [frame.command_buffer];
        unsafe {
            d.reset_fences(&[frame.fence])?;
            d.queue_submit(gpu.queue, &[vk::SubmitInfo::default().command_buffers(&one)], frame.fence)?;
            d.wait_for_fences(&[frame.fence], true, u64::MAX)?;
        }
        let submitted = Instant::now();
        self.output.flip(frame.fb)?;
        let flipped = Instant::now();
        Ok((
            renderer.gpu_ms(gpu),
            (submitted - start).as_secs_f64() * 1e3,
            (flipped - submitted).as_secs_f64() * 1e3,
        ))
    }

    /// Read the *scanned-out* image back through the GPU: the proof that what the panel
    /// is reading is the frame that was drawn, not a plausible-looking other buffer.
    pub fn read_scanout(&self, gpu: &Gpu, renderer: &Renderer) -> Result<(u32, u32, Vec<u8>)> {
        let (w, h) = (self.output.width, self.output.height);
        let i = (self.index + RING - 1) % RING;
        let frame = &self.frames[i];
        let layout = unsafe {
            gpu.device.get_image_subresource_layout(
                frame.image,
                vk::ImageSubresource::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .mip_level(0)
                    .array_layer(0),
            )
        };
        let pitch = layout.row_pitch as usize;
        let host = gpu.host_buffer(layout.row_pitch * u64::from(h), vk::BufferUsageFlags::TRANSFER_DST)?;
        let d = &gpu.device;
        gpu.one_shot(renderer.command_pool, |cb| unsafe {
            barrier(d, cb, frame.image, vk::ImageLayout::GENERAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL);
            d.cmd_copy_image_to_buffer(
                cb,
                frame.image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                host.buffer,
                &[vk::BufferImageCopy::default()
                    .buffer_row_length(layout.row_pitch as u32 / 4)
                    .buffer_image_height(h)
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1),
                    )
                    .image_extent(vk::Extent3D { width: w, height: h, depth: 1 })],
            );
            barrier(d, cb, frame.image, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::GENERAL);
        })?;
        // BGRA in memory to RGBA, dropping the row padding.
        let bytes = unsafe { host.bytes() };
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h as usize {
            for x in 0..w as usize {
                let s = y * pitch + x * 4;
                let t = (y * w as usize + x) * 4;
                rgba[t] = bytes[s + 2];
                rgba[t + 1] = bytes[s + 1];
                rgba[t + 2] = bytes[s];
                rgba[t + 3] = 255;
            }
        }
        host.destroy(gpu);
        Ok((w, h, rgba))
    }

    pub fn destroy(&mut self, gpu: &Gpu) {
        let d = &gpu.device;
        unsafe {
            let _ = d.device_wait_idle();
            for frame in &self.frames {
                d.destroy_fence(frame.fence, None);
                d.destroy_framebuffer(frame.target.framebuffer, None);
                d.destroy_image_view(frame.view, None);
                d.destroy_image(frame.image, None);
                d.free_memory(frame.memory, None);
            }
        }
    }
}

/// Whether a `B8G8R8A8_UNORM` linear exportable image can carry a `B8G8R8A8_SRGB` view,
/// which is what lets the hardware do the sRGB encode on a DRM `XR24` buffer.
fn srgb_view_is_possible(gpu: &Gpu) -> bool {
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

/// One linear `B8G8R8A8_UNORM` image, exported as a dma-buf fd, with the row pitch and
/// offset `AddFB2` needs.
fn export_linear(
    gpu: &Gpu,
    width: u32,
    height: u32,
    plain: bool,
) -> Result<(vk::Image, vk::DeviceMemory, u32, u32, OwnedFd)> {
    let d = &gpu.device;
    let mut external =
        vk::ExternalMemoryImageCreateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let mut create = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(FORMAT)
        .extent(vk::Extent3D { width, height, depth: 1 })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::LINEAR)
        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    if !plain {
        create = create.flags(vk::ImageCreateFlags::MUTABLE_FORMAT);
    }
    let image = unsafe { d.create_image(&create.push_next(&mut external), None) }
        .context("vkCreateImage(LINEAR, external)")?;
    let requirements = unsafe { d.get_image_memory_requirements(image) };
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
    let mut export =
        vk::ExportMemoryAllocateInfo::default().handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let memory = unsafe {
        d.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(
                    gpu.memory_type(requirements.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)?,
                )
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
    let fd_device = ash::khr::external_memory_fd::Device::new(&gpu.instance, d);
    let raw = unsafe {
        fd_device.get_memory_fd(
            &vk::MemoryGetFdInfoKHR::default()
                .memory(memory)
                .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT),
        )
    }
    .context("vkGetMemoryFdKHR(linear image)")?;
    Ok((
        image,
        memory,
        layout.row_pitch as u32,
        layout.offset as u32,
        unsafe { OwnedFd::from_raw_fd(raw) },
    ))
}
