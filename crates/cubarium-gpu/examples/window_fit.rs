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

#![forbid(unsafe_code)]
use anyhow::{Context, Result, bail};
use cubarium_gpu::{
    present::WindowFit,
    wgpu_backend::{Gpu, PresentPass, headless::read_texture},
};
use std::time::Instant;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

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

    let gpu = Gpu::open()?;
    let size = |w, h| wgpu::Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let raster = gpu.texture(
        "window-fit raster",
        size(rw, rh),
        wgpu::TextureDimension::D2,
        FORMAT,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    let view = raster.create_view(&Default::default());
    let target = gpu.texture(
        "window-fit output",
        size(ww, wh),
        wgpu::TextureDimension::D2,
        FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let target_view = target.create_view(&Default::default());
    let pass = PresentPass::new(&gpu, FORMAT)?;
    let fit = WindowFit::fit((rw, rh), (ww, wh), false);
    let mut upload = gpu.encoder("window-fit upload");
    gpu.upload_texture(
        &mut upload,
        &raster,
        wgpu::Origin3d::ZERO,
        size(rw, rh),
        4,
        &pixels,
    )?;
    gpu.queue.submit([upload.finish()]);
    let run = |count: u32| -> Result<f64> {
        let start = Instant::now();
        let mut encoder = gpu.encoder("window-fit present");
        for _ in 0..count {
            pass.record(&gpu, &mut encoder, &view, &target_view, fit.push());
        }
        let submission = gpu.queue.submit([encoder.finish()]);
        gpu.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(30)),
        })?;
        Ok(start.elapsed().as_secs_f64() * 1e3)
    };
    run(1)?;
    let pixels = read_texture(&gpu, &target)?;
    write_png(&args[3], ww, wh, &pixels)?;
    if repeat > 0 {
        run(repeat)?;
        let ms = run(repeat)? / f64::from(repeat);
        println!("present pass {rw}x{rh} -> {ww}x{wh}: {ms:.4} ms");
    }
    Ok(())
}
