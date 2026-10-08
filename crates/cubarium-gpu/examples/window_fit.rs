//! The desktop window's present pass, offscreen: a world raster PNG drawn into a
//! window-sized image through [`WindowFit`] and `present.frag`, exactly as the swapchain
//! window draws it, and written out. With `--repeat N` it also times the pass.
//!
//! ```text
//! window_fit RASTER.png WxH OUT.png [--repeat N]
//! ```
//!
//! For captures of how a raster fills a window of a given size without opening one, and
//! for the pass's cost at that size.

use std::time::Instant;

use anyhow::{Context, Result, bail};
use ash::vk;
use cubarium_gpu::present::WindowFit;
use cubarium_gpu::render::framebuffer;
use cubarium_gpu::vk::{Gpu, barrier};
use cubarium_gpu::{PresentPass, TargetImage};

const FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;

fn read_png(path: &str) -> Result<(u32, u32, Vec<u8>)> {
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(path).with_context(|| format!("opening {path}"))?,
    ));
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().context("png size")?];
    let info = reader.next_frame(&mut buf)?;
    let (w, h) = (info.width, info.height);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..(w * h * 4) as usize].to_vec(),
        png::ColorType::Rgb => buf[..(w * h * 3) as usize]
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        t => bail!("{path}: unsupported colour type {t:?}"),
    };
    Ok((w, h, rgba))
}

fn write_png(path: &str, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        bail!("usage: window_fit RASTER.png WxH OUT.png [--repeat N]");
    }
    let (rw, rh, pixels) = read_png(&args[1])?;
    let (ww, wh) = args[2]
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))
        .context("window size is WxH")?;
    let repeat: u32 = match args.get(4).map(String::as_str) {
        Some("--repeat") => args.get(5).context("--repeat N")?.parse()?,
        _ => 0,
    };

    let gpu = Gpu::open(&[])?;
    let d = &gpu.device;
    let pool = unsafe {
        d.create_command_pool(
            &vk::CommandPoolCreateInfo::default().queue_family_index(gpu.queue_family),
            None,
        )
    }?;
    let (raster, raster_mem) = gpu.image(
        rw,
        rh,
        FORMAT,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
    )?;
    let raster_view = gpu.view(raster, FORMAT)?;
    let upload = gpu.host_buffer(pixels.len() as u64, vk::BufferUsageFlags::TRANSFER_SRC)?;
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
    })?;
    let sampler = unsafe {
        d.create_sampler(
            &vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::NEAREST)
                .min_filter(vk::Filter::NEAREST),
            None,
        )
    }?;
    let mut pass = PresentPass::new(&gpu, raster_view, sampler)?;
    let (render_pass, pipeline) =
        pass.entry(&gpu, FORMAT, vk::ImageLayout::TRANSFER_SRC_OPTIMAL)?;
    let (target, target_mem) = gpu.image(
        ww,
        wh,
        FORMAT,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
    )?;
    let target_view = gpu.view(target, FORMAT)?;
    let fb = framebuffer(d, render_pass, target_view, ww, wh)?;
    let slot = TargetImage {
        image: target,
        view: target_view,
        framebuffer: fb,
    };
    let fit = WindowFit::fit((rw, rh), (ww, wh), false);
    eprintln!("window_fit: {rw}x{rh} into {ww}x{wh}: {fit:?}");
    let readback = gpu.readback_buffer(u64::from(ww * wh * 4))?;
    gpu.one_shot(pool, |cb| unsafe {
        pass.record_push(d, cb, &slot, (ww, wh), fit.push(), render_pass, pipeline);
        d.cmd_copy_image_to_buffer(
            cb,
            target,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            readback.buffer,
            &[region(ww, wh)],
        );
    })?;
    let out = unsafe { readback.bytes() }[..(ww * wh * 4) as usize].to_vec();
    write_png(&args[3], ww, wh, &out)?;

    if repeat > 0 {
        // One warm-up, then `repeat` passes in one command buffer: the per-pass cost with
        // the submit and the wait spread over all of them.
        let run = |n: u32| -> Result<f64> {
            let t = Instant::now();
            gpu.one_shot(pool, |cb| unsafe {
                for _ in 0..n {
                    pass.record_push(d, cb, &slot, (ww, wh), fit.push(), render_pass, pipeline);
                }
            })?;
            Ok(t.elapsed().as_secs_f64() * 1e3)
        };
        run(repeat)?;
        let ms = run(repeat)? / f64::from(repeat);
        println!("present pass {rw}x{rh} -> {ww}x{wh}: {ms:.4} ms");
    }

    unsafe {
        d.device_wait_idle()?;
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
    pass.destroy(&gpu);
    upload.destroy(&gpu);
    readback.destroy(&gpu);
    Ok(())
}
