//! The synthetic ring scene, on whichever target this build has.
//!
//! ```text
//! synthetic --art assets/atelier --ring 320x180 --scale 1 \
//!           [--target headless|window|scanout] [--frames 3600] [--png out.png]
//! ```
//!
//! `headless` renders `--frames` frames and writes the last one out; it is what the
//! desktop screenshots and the golden image come from. `scanout` takes DRM master and
//! page-flips at DP-1 (stop `cube-screen-shim` first). `window` opens a `winit` window.

use anyhow::{Context, Result, bail};
use cubarium_gpu::atlas::Atlas;
use cubarium_gpu::render::Renderer;
use cubarium_gpu::scene::RingLayout;
use cubarium_gpu::synthetic::SyntheticWorld;
use cubarium_gpu::target::Headless;
use cubarium_gpu::vk::Gpu;

/// The world's tick rate, `design/architecture.md`'s 20 Hz.
const TICK_HZ: f64 = 20.0;

struct Args {
    art: std::path::PathBuf,
    layout: RingLayout,
    target: String,
    frames: usize,
    fps: f64,
    png: Option<std::path::PathBuf>,
    at: Option<f64>,
    connector: String,
    quarter_turns: u32,
    seed: u64,
}

fn parse() -> Result<Args> {
    let mut a = Args {
        art: "assets/atelier".into(),
        layout: RingLayout::RING_320,
        target: "headless".into(),
        frames: 600,
        fps: 60.0,
        png: None,
        at: None,
        connector: "DP-1".into(),
        quarter_turns: 1,
        seed: 1,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| anyhow::anyhow!("{flag} wants a value"));
        match flag.as_str() {
            "--art" => a.art = value()?.into(),
            "--ring" => {
                let v = value()?;
                let (w, h) = v.split_once('x').ok_or_else(|| anyhow::anyhow!("--ring WxH"))?;
                a.layout.w = w.parse()?;
                a.layout.h = h.parse()?;
            }
            "--scale" => a.layout.scale = value()?.parse()?,
            "--target" => a.target = value()?,
            "--frames" => a.frames = value()?.parse()?,
            "--fps" => a.fps = value()?.parse()?,
            "--png" => a.png = Some(value()?.into()),
            "--at" => a.at = Some(value()?.parse()?),
            "--connector" => a.connector = value()?,
            "--quarter-turns" => a.quarter_turns = value()?.parse()?,
            "--seed" => a.seed = value()?.parse()?,
            other => bail!("unknown flag {other}"),
        }
    }
    Ok(a)
}

fn main() -> Result<()> {
    let args = parse()?;
    let atlas = Atlas::load(&args.art).with_context(|| format!("load {}", args.art.display()))?;
    println!(
        "pack {}: {} frames, atlas {}x{}",
        args.art.display(),
        atlas.frames.len(),
        atlas.width,
        atlas.height
    );
    match args.target.as_str() {
        "headless" => headless(&args, &atlas),
        #[cfg(feature = "scanout")]
        "scanout" => scanout(&args, &atlas),
        #[cfg(feature = "window")]
        "window" => window(&args, &atlas),
        other => bail!("target {other} is not built into this binary"),
    }
}

/// Presentation seconds for a frame, exactly `art_present::present_seconds`:
/// `(tick − 1 + f) · DT`.
fn present_seconds(tick: u64, f: f64) -> f64 {
    (tick as f64 - 1.0 + f) / TICK_HZ
}

fn headless(args: &Args, atlas: &Atlas) -> Result<()> {
    let gpu = Gpu::open(&[])?;
    println!("device: {} (dma-buf: {})", gpu.name, gpu.has_dma_buf);
    let mut renderer = Renderer::new(&gpu, atlas, args.layout)?;
    let mut world = SyntheticWorld::new(args.layout, args.seed);
    let mut target = Headless::new(&gpu, &renderer)?;

    let mut gpu_ms = Vec::new();
    let mut instances = 0;
    let wall = std::time::Instant::now();
    for frame in 0..args.frames {
        let seconds = args.at.unwrap_or(frame as f64 / args.fps);
        let tick = (seconds * TICK_HZ).floor() as u64 + 1;
        let f = (seconds * TICK_HZ).fract();
        world.tick(tick, present_seconds(tick, 0.0));
        let scene = world.frame(atlas, present_seconds(tick, f), f as f32);
        instances = scene.instance_count();
        let ms = target.draw(&gpu, &mut renderer, scene)?;
        if frame >= 5 {
            gpu_ms.push(ms);
        }
    }
    let elapsed = wall.elapsed().as_secs_f64();
    stat("GPU per frame", &gpu_ms);
    println!(
        "{} frames, {instances} instances, {:.1} fps end to end (CPU-bound: this waits on every frame)",
        args.frames,
        args.frames as f64 / elapsed
    );
    if let Some(path) = &args.png {
        let rgba = target.read(&gpu, &renderer)?;
        cubarium_gpu::target::write_png(path, args.layout.w, args.layout.h, &rgba)?;
        println!("wrote {}", path.display());
    }
    target.destroy(&gpu);
    renderer.destroy(&gpu);
    Ok(())
}

#[cfg(feature = "scanout")]
fn scanout(args: &Args, atlas: &Atlas) -> Result<()> {
    use cubarium_gpu::target::Scanout;
    let gpu = Gpu::open(&[])?;
    println!("device: {} (dma-buf: {})", gpu.name, gpu.has_dma_buf);
    let mut renderer = Renderer::new(&gpu, atlas, args.layout)?;
    let mut world = SyntheticWorld::new(args.layout, args.seed);
    let mut target = Scanout::open(&gpu, &mut renderer, &args.connector, args.quarter_turns)?;
    println!(
        "panel {}x{} at {:.2} Hz, upscale x{}, {} quarter turn(s)",
        target.width(),
        target.height(),
        target.refresh_hz(),
        target.transform().factor,
        target.transform().quarter_turns
    );

    let (mut gpu_ms, mut submit_ms, mut flip_ms) = (Vec::new(), Vec::new(), Vec::new());
    let cpu0 = cpu_seconds();
    let wall = std::time::Instant::now();
    for frame in 0..args.frames {
        let seconds = frame as f64 / args.fps;
        let tick = (seconds * TICK_HZ).floor() as u64 + 1;
        let f = (seconds * TICK_HZ).fract();
        world.tick(tick, present_seconds(tick, 0.0));
        let scene = world.frame(atlas, present_seconds(tick, f), f as f32);
        let timing = target.draw(&gpu, &mut renderer, scene)?;
        if frame >= 5 {
            gpu_ms.push(timing.0);
            submit_ms.push(timing.1);
            flip_ms.push(timing.2);
        }
    }
    let elapsed = wall.elapsed().as_secs_f64();
    let cpu = cpu_seconds() - cpu0;
    stat("GPU render into the scanout dma-buf", &gpu_ms);
    stat("submit..fence", &submit_ms);
    stat("page flip queue..complete", &flip_ms);
    println!(
        "wall={elapsed:.2}s cpu={cpu:.2}s -> {:.3} CPU core-seconds per second, {:.1} fps, {} instances",
        cpu / elapsed,
        args.frames as f64 / elapsed,
        world.scene().instance_count()
    );
    if let Some(path) = &args.png {
        let rgba = renderer.read_raster(&gpu)?;
        cubarium_gpu::target::write_png(path, args.layout.w, args.layout.h, &rgba)?;
        println!("wrote {}", path.display());
    }
    target.destroy(&gpu);
    renderer.destroy(&gpu);
    Ok(())
}

#[cfg(feature = "window")]
fn window(args: &Args, atlas: &Atlas) -> Result<()> {
    cubarium_gpu::target::Window::run(atlas, args.layout, args.seed, args.fps)
}

fn stat(name: &str, values: &[f64]) {
    if values.is_empty() {
        println!("{name}: no samples");
        return;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "{name}: n={} min={:.2} p50={:.2} p95={:.2} max={:.2} mean={:.2} ms",
        v.len(),
        v[0],
        v[v.len() / 2],
        v[(v.len() * 95 / 100).min(v.len() - 1)],
        v[v.len() - 1],
        mean
    );
}

#[cfg(feature = "scanout")]
fn cpu_seconds() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // utime and stime are fields 14 and 15, after the (possibly parenthesised) comm.
    let tail = stat.rsplit_once(')').map(|(_, t)| t).unwrap_or("");
    let fields: Vec<&str> = tail.split_whitespace().collect();
    let hz = 100.0;
    let get = |i: usize| fields.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    (get(11) + get(12)) / hz
}
