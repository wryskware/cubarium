//! The desktop window's present pass on the GPU: `present.frag` with a [`WindowFit`]'s
//! push constants, drawn into an offscreen image and read back.
//!
//! The fit's arithmetic is tested on the CPU in `present.rs`; this is the one check that
//! the shader's letterbox branch agrees with it — bars black, every raster pixel a whole
//! `k × k` block, and a window smaller than the raster reading exactly the texels the fit
//! says. `None` from a machine with no Vulkan device, which is not a failure of the pass.

use ash::vk;
use cubarium_gpu::render::framebuffer;
use cubarium_gpu::vk::{Gpu, barrier};
use cubarium_gpu::present::WindowFit;
use cubarium_gpu::{PresentPass, TargetImage};

const FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

/// A 4 × 2 raster whose every texel is a different colour.
fn raster_pixel(x: u32, y: u32) -> [u8; 4] {
    [40 + 50 * x as u8, 30 + 100 * y as u8, 7 * (x + 4 * y) as u8 + 1, 255]
}

/// Draw the 4 × 2 raster into a `window`-sized image through the present pass at `fit`,
/// and read it back as RGBA rows.
fn present(gpu: &Gpu, window: (u32, u32), fit: WindowFit) -> Vec<u8> {
    let d = &gpu.device;
    let (rw, rh) = (4u32, 2u32);
    let pool = unsafe {
        d.create_command_pool(
            &vk::CommandPoolCreateInfo::default().queue_family_index(gpu.queue_family),
            None,
        )
    }
    .unwrap();

    // The raster: uploaded, then left where every frame leaves it.
    let (raster, raster_mem) = gpu
        .image(
            rw,
            rh,
            FORMAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        )
        .unwrap();
    let raster_view = gpu.view(raster, FORMAT).unwrap();
    let pixels: Vec<u8> = (0..rh)
        .flat_map(|y| (0..rw).flat_map(move |x| raster_pixel(x, y)))
        .collect();
    let upload = gpu
        .host_buffer(pixels.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC)
        .unwrap();
    upload.write(&pixels);
    let region = |w: u32, h: u32| {
        vk::BufferImageCopy::default()
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
    };
    gpu.one_shot(pool, |cb| unsafe {
        barrier(
            d,
            cb,
            raster,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );
        d.cmd_copy_buffer_to_image(
            cb,
            upload.buffer,
            raster,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region(rw, rh)],
        );
        barrier(
            d,
            cb,
            raster,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
    })
    .unwrap();

    let sampler = unsafe {
        d.create_sampler(
            &vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::NEAREST)
                .min_filter(vk::Filter::NEAREST),
            None,
        )
    }
    .unwrap();
    let mut pass = PresentPass::new(gpu, raster_view, sampler).unwrap();
    let (render_pass, pipeline) = pass
        .entry(gpu, FORMAT, vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
        .unwrap();

    // The "swapchain image".
    let (target, target_mem) = gpu
        .image(
            window.0,
            window.1,
            FORMAT,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
        )
        .unwrap();
    let target_view = gpu.view(target, FORMAT).unwrap();
    let fb = framebuffer(d, render_pass, target_view, window.0, window.1).unwrap();
    let slot = TargetImage {
        image: target,
        view: target_view,
        framebuffer: fb,
    };
    let readback = gpu
        .readback_buffer(u64::from(window.0 * window.1 * 4))
        .unwrap();
    gpu.one_shot(pool, |cb| unsafe {
        pass.record_push(d, cb, &slot, window, fit.push(), render_pass, pipeline);
        d.cmd_copy_image_to_buffer(
            cb,
            target,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            readback.buffer,
            &[region(window.0, window.1)],
        );
    })
    .unwrap();
    let out = unsafe { readback.bytes() }[..(window.0 * window.1 * 4) as usize].to_vec();

    unsafe {
        d.destroy_framebuffer(fb, None);
        d.destroy_image_view(target_view, None);
        d.destroy_image(target, None);
        d.free_memory(target_mem, None);
        d.destroy_image_view(raster_view, None);
        d.destroy_image(raster, None);
        d.free_memory(raster_mem, None);
        d.destroy_sampler(sampler, None);
        d.destroy_command_pool(pool, None);
    }
    pass.destroy(gpu);
    upload.destroy(gpu);
    readback.destroy(gpu);
    out
}

fn open() -> Option<Gpu> {
    match Gpu::open(&[]) {
        Ok(gpu) => Some(gpu),
        Err(e) => {
            eprintln!("no Vulkan device here ({e}); the window's present pass was not checked");
            None
        }
    }
}

fn at(image: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    image[i..i + 4].try_into().unwrap()
}

/// 4 × 2 into 11 × 8: a whole 2× upscale, 8 × 4 centred at (1, 2), black all round.
#[test]
fn the_window_pass_upscales_whole_and_paints_the_bars_black() {
    let Some(gpu) = open() else { return };
    let window = (11, 8);
    let fit = WindowFit::fit((4, 2), window, false);
    assert_eq!((fit.factor, fit.size, fit.origin), (2, (8, 4), (1, 2)));
    let image = present(&gpu, window, fit);
    for y in 0..window.1 {
        for x in 0..window.0 {
            let inside = (1..9).contains(&x) && (2..6).contains(&y);
            let want = if inside {
                raster_pixel((x - 1) / 2, (y - 2) / 2)
            } else {
                [0, 0, 0, 255]
            };
            assert_eq!(at(&image, window.0, x, y), want, "pixel ({x}, {y})");
        }
    }
}

/// 4 × 2 into 3 × 3, smaller than the raster: a nearest downscale that reads exactly the
/// texels the fit's arithmetic names, with the leftover rows black.
#[test]
fn the_window_pass_downscales_nearest_where_the_fit_says() {
    let Some(gpu) = open() else { return };
    let window = (3, 3);
    let fit = WindowFit::fit((4, 2), window, false);
    assert_eq!(fit.factor, 0);
    let push = fit.push();
    let image = present(&gpu, window, fit);
    for y in 0..window.1 {
        for x in 0..window.0 {
            let sx = (push[0] * x as f32 + push[4]).floor();
            let sy = (push[3] * y as f32 + push[5]).floor();
            let want = if (0.0..4.0).contains(&sx) && (0.0..2.0).contains(&sy) {
                raster_pixel(sx as u32, sy as u32)
            } else {
                [0, 0, 0, 255]
            };
            assert_eq!(at(&image, window.0, x, y), want, "pixel ({x}, {y})");
        }
    }
}
