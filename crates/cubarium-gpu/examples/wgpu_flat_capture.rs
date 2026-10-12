//! Headless flat-scene capture with the synthetic example's inputs and clocks.
#![forbid(unsafe_code)]

use anyhow::{Context, Result, ensure};
use cubarium_gpu::atlas::Atlas;
use cubarium_gpu::scene::{RingLayout, Scene};
use cubarium_gpu::synthetic::SyntheticWorld;
use cubarium_gpu::wgpu_backend::flat::Renderer;
use cubarium_gpu::wgpu_backend::{FrameSource, Gpu, ReadbackRing};

fn main() -> Result<()> {
    let mut layout = RingLayout::RING_320;
    let mut art = std::path::PathBuf::from("assets/atelier");
    let mut png = None;
    let mut seed = 1u64;
    let mut at = 0.0f64;
    let mut frames = 1usize;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .with_context(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--art" => art = value.into(),
            "--png" => png = Some(std::path::PathBuf::from(value)),
            "--seed" => seed = value.parse()?,
            "--at" => at = value.parse()?,
            "--frames" => frames = value.parse()?,
            "--scale" => layout.scale = value.parse()?,
            "--target" => ensure!(value == "headless", "this capture is headless"),
            "--ring" => {
                let (w, h) = value.split_once('x').context("--ring WxH")?;
                layout.w = w.parse()?;
                layout.h = h.parse()?;
            }
            _ => anyhow::bail!("unknown argument {flag}"),
        }
    }
    ensure!(frames > 0, "capture needs at least one frame");
    let atlas = Atlas::load(&art)?;
    let gpu = Gpu::open()?;
    let mut renderer = Renderer::new(&gpu, &atlas, layout)?;
    renderer.art_scale = layout.scale as f32;
    let mut world = SyntheticWorld::new(layout, seed);
    let mut readback = ReadbackRing::new(&gpu, (layout.w, layout.h), 1)?;
    let mut pixels = Vec::new();
    for _ in 0..frames {
        let tick = (at * 20.0).floor() as u64 + 1;
        let fraction = (at * 20.0).fract();
        world.tick(tick, (tick as f64 - 1.0) / 20.0);
        world.frame(
            &atlas,
            (tick as f64 - 1.0 + fraction) / 20.0,
            fraction as f32,
        );
        ensure!(
            readback.submit_source(&gpu, &mut renderer, world.scene(), tick)?,
            "capture backpressure"
        );
        // A rejected attempt must not retire or discard the submitted capture.
        let pending = renderer.frames_in_flight();
        let mut invalid_layout = layout;
        invalid_layout.w += 1;
        let invalid = Scene::new(invalid_layout);
        let mut rejected = gpu.encoder("invalid-layout regression");
        ensure!(
            renderer
                .record_frame(&gpu, &mut rejected, &invalid, None)
                .is_err(),
            "invalid layout accepted"
        );
        drop(rejected);
        ensure!(
            renderer.frames_in_flight() == pending,
            "invalid attempt removed an older frame"
        );
        readback.drain_source(&gpu, &mut renderer, |_, bytes| {
            pixels.clear();
            pixels.extend_from_slice(bytes);
        })?;
    }
    if let Some(path) = png {
        cubarium_gpu::target::write_png(&path, layout.w, layout.h, &pixels)?;
    }
    println!(
        "safe wgpu flat scene: {}x{}, S={}, {} instances, {} RGBA bytes",
        layout.w,
        layout.h,
        layout.scale,
        world.scene().instance_count(),
        pixels.len()
    );
    Ok(())
}
