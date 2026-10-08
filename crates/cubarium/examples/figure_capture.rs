//! `figure_capture`: one saved world drawn by several variants of the GPU sink at once,
//! frame for frame on the same clock — H2's evidence (the stamped animals against the model
//! pass, its turn modes), `design/handoffs/hybrid-renderer-2026-10-07.md`.
//!
//! ```text
//! cargo run --release -p cubarium --example figure_capture -- --save DIR --out DIR \
//!     [--config config/desktop/terrarium.toml] [--set LINE]... [--px N] \
//!     [--variant NAME=SETLINE[;SETLINE]]... [--seconds 0] [--fps 60] [--warm 20] \
//!     [--crop NAME=X0,Y0,X1,Y1]... [--every 1] [--mp4]
//! ```
//!
//! `--mp4` pipes each variant's frames (and each crop's) to `ffmpeg` instead of PNGs:
//! `NAME.mp4` and `NAME-CROP.mp4`, x264 yuv444p crf 12 at `--fps`, real time.
//!
//! The save (a session save directory, read only) is loaded once and stepped by one sim;
//! every tick is staged into every variant's sink, and every frame (`--fps` a second of sim
//! time, `--seconds` long, after `--warm` ticks) is drawn by each at the same clock and
//! written as `NAME/fKKKKK.png` (whole raster) and `NAME-CROP/fKKKKK.png` per crop. With
//! `--seconds 0` it writes one still per variant, `NAME.png`. Each variant's costs go to
//! `costs.txt`: GPU ms per frame (all, the model pass), CPU ms to draw a frame, pack ms per
//! tick. No window is ever opened.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};

use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::VoxelConfig;
use cubarium::voxel::project::Projection;
use cubarium_voxel_sim::SimConfig;

fn configured(path: &PathBuf, sets: &[String]) -> Result<VoxelConfig> {
    let mut table: toml::Table = std::fs::read_to_string(path)?.parse()?;
    for line in sets {
        let s: toml::Table = line.parse().with_context(|| format!("--set {line}"))?;
        for (k, v) in s {
            match (table.get_mut(&k), v) {
                (Some(toml::Value::Table(t)), toml::Value::Table(s)) => t.extend(s),
                (_, v) => {
                    table.insert(k, v);
                }
            }
        }
    }
    Ok(toml::Value::Table(table).try_into()?)
}

/// An ffmpeg process taking raw RGBA frames.
struct Encoder {
    child: std::process::Child,
    w: u32,
    h: u32,
}

impl Encoder {
    fn new(path: &std::path::Path, w: u32, h: u32, fps: f64) -> Result<Encoder> {
        let child = std::process::Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
            .arg(format!("{w}x{h}"))
            .args(["-r"])
            .arg(format!("{fps}"))
            .args(["-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv444p", "-crf", "12", "-preset", "fast"])
            .arg(path)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .context("ffmpeg")?;
        Ok(Encoder { child, w, h })
    }

    fn put(&mut self, rgba: &[u8]) -> Result<()> {
        debug_assert_eq!(rgba.len(), (self.w * self.h * 4) as usize);
        self.child.stdin.as_mut().context("ffmpeg's stdin")?.write_all(rgba)?;
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        drop(self.child.stdin.take());
        let status = self.child.wait()?;
        if !status.success() {
            bail!("ffmpeg failed: {status}");
        }
        Ok(())
    }
}

struct Variant {
    name: String,
    sink: VoxelGpuSink,
    video: Option<Encoder>,
    crops: Vec<Encoder>,
    draw_ms: f64,
    clock_ms: f64,
    frames: u64,
    gpu: (u64, [f64; 3]),
}

fn main() -> Result<()> {
    let mut config = PathBuf::from("config/desktop/terrarium.toml");
    let (mut save, mut out) = (None::<PathBuf>, None::<PathBuf>);
    let mut sets: Vec<String> = Vec::new();
    let mut variants: Vec<(String, Vec<String>)> = Vec::new();
    let mut crops: Vec<(String, [u32; 4])> = Vec::new();
    let (mut seconds, mut fps, mut warm, mut px, mut every) = (0.0f64, 60.0f64, 20u64, None::<u32>, 1u64);
    let mut mp4 = false;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().context("missing value");
        match k.as_str() {
            "--config" => config = v()?.into(),
            "--save" => save = Some(v()?.into()),
            "--out" => out = Some(v()?.into()),
            "--set" => sets.push(v()?),
            "--px" => px = Some(v()?.parse()?),
            "--seconds" => seconds = v()?.parse()?,
            "--fps" => fps = v()?.parse()?,
            "--warm" => warm = v()?.parse()?,
            "--every" => every = v()?.parse()?,
            "--mp4" => mp4 = true,
            "--variant" => {
                let s = v()?;
                let (name, lines) = s.split_once('=').context("--variant NAME=SET[;SET]")?;
                variants.push((name.into(), lines.split(';').filter(|l| !l.is_empty()).map(String::from).collect()));
            }
            "--crop" => {
                let s = v()?;
                let (name, b) = s.split_once('=').context("--crop NAME=X0,Y0,X1,Y1")?;
                let b: Vec<u32> = b.split(',').map(|n| n.parse()).collect::<Result<_, _>>()?;
                if b.len() != 4 {
                    bail!("--crop NAME=X0,Y0,X1,Y1");
                }
                crops.push((name.into(), [b[0], b[1], b[2], b[3]]));
            }
            _ => bail!("unknown argument {k}"),
        }
    }
    let (save, out) = (save.context("--save DIR")?, out.context("--out DIR")?);
    if variants.is_empty() {
        variants.push(("stamped".into(), vec!["figures = { animals = \"stamped\" }".into()]));
        variants.push(("models".into(), vec![]));
    }
    std::fs::create_dir_all(&out)?;
    let base = configured(&config, &sets)?;
    let prepared = cubarium_session::prepare_loaded(&base.sim, &save).map_err(anyhow::Error::msg)?;
    let world_cfg = prepared.world.config().clone();
    let mut sim = cubarium_session::create::ready_sim(
        prepared.world,
        prepared.flora,
        prepared.fauna,
        SimConfig::with_threads(cubarium_voxel::default_threads()),
    );
    let mut sinks = Vec::new();
    for (name, lines) in &variants {
        let mut all = sets.clone();
        all.extend(lines.iter().cloned());
        let mut cfg = configured(&config, &all)?;
        cfg.sim.world = world_cfg.clone();
        if let Some(px) = px {
            cfg.px_per_voxel = px;
        } else if cfg.px_per_voxel == cubarium::voxel::PX_AUTO {
            cfg.px_per_voxel = cubarium::voxel::cover_px_per_voxel(cfg.tilt_degrees, cfg.raster_height, &world_cfg, (1920, 1080));
        }
        let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &world_cfg)?;
        let sink = VoxelGpuSink::new(
            &cfg,
            proj,
            VoxelGpuSinkOptions {
                target: GpuTargetKind::Headless,
                models: cubarium::voxel::load_models(&cfg),
                ..VoxelGpuSinkOptions::default()
            },
        )?;
        sinks.push(Variant {
            name: name.clone(),
            sink,
            video: None,
            crops: Vec::new(),
            draw_ms: 0.0,
            clock_ms: 0.0,
            frames: 0,
            gpu: (0, [0.0; 3]),
        });
    }
    // The lit tier's sky plane first.
    let started = Instant::now();
    for v in &mut sinks {
        loop {
            v.sink.set_clock(sim.world().tick(), 0.0);
            let staged = v.sink.stage_world(sim.world(), sim.flora(), sim.fauna());
            v.sink.render()?;
            if staged && !v.sink.light_pending() {
                break;
            }
            if started.elapsed().as_secs() > 120 {
                bail!("the sky plane never arrived");
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    let stage_all = |sinks: &mut Vec<Variant>, sim: &cubarium_voxel_sim::Sim| {
        for v in sinks.iter_mut() {
            v.sink.set_clock(sim.world().tick(), 0.0);
            while !v.sink.stage_world(sim.world(), sim.flora(), sim.fauna()) {
                let _ = v.sink.render();
            }
        }
    };
    for _ in 0..warm {
        sim.step();
        stage_all(&mut sinks, &sim);
        for v in sinks.iter_mut() {
            v.sink.render()?;
        }
    }
    let p0 = sinks.iter().map(|v| v.sink.draw_split().0).collect::<Vec<_>>();
    let ticks0 = sim.world().tick();
    let frames = if seconds > 0.0 { (seconds * fps).round() as u64 } else { 1 };
    let hz = f64::from(cubarium_voxel::TICK_HZ);
    let mut tick = sim.world().tick();
    for v in &mut sinks {
        if mp4 && seconds > 0.0 {
            let p = v.sink.params();
            v.video = Some(Encoder::new(&out.join(format!("{}.mp4", v.name)), p.raster_w, p.raster_h, fps)?);
            for (c, b) in &crops {
                let (w, h) = (b[2].min(p.raster_w) - b[0], b[3].min(p.raster_h) - b[1]);
                v.crops.push(Encoder::new(&out.join(format!("{}-{c}.mp4", v.name)), w, h, fps)?);
            }
            continue;
        }
        if seconds > 0.0 {
            std::fs::create_dir_all(out.join(&v.name))?;
            for (c, _) in &crops {
                std::fs::create_dir_all(out.join(format!("{}-{c}", v.name)))?;
            }
        }
    }
    for k in 0..frames {
        let t = ticks0 as f64 + k as f64 * hz / fps;
        while (tick as f64) < t.floor() {
            sim.step();
            tick = sim.world().tick();
            stage_all(&mut sinks, &sim);
        }
        for v in sinks.iter_mut() {
            let at = Instant::now();
            v.sink.set_clock(t.floor() as u64, t.fract());
            v.clock_ms += at.elapsed().as_secs_f64() * 1e3;
            v.sink.render()?;
            v.draw_ms += at.elapsed().as_secs_f64() * 1e3;
            v.frames += 1;
            if k % every != 0 {
                continue;
            }
            let p = v.sink.params();
            let rgba = v.sink.read_raster()?;
            if let Some(e) = &mut v.video {
                e.put(&rgba)?;
                for ((_, b), e) in crops.iter().zip(&mut v.crops) {
                    let (x0, y0, x1, y1) = (b[0], b[1], b[2].min(p.raster_w), b[3].min(p.raster_h));
                    let mut px = Vec::with_capacity(((x1 - x0) * (y1 - y0) * 4) as usize);
                    for y in y0..y1 {
                        let row = (y * p.raster_w + x0) as usize * 4;
                        px.extend_from_slice(&rgba[row..row + (x1 - x0) as usize * 4]);
                    }
                    e.put(&px)?;
                }
                continue;
            }
            let path = if seconds > 0.0 {
                out.join(&v.name).join(format!("f{k:05}.png"))
            } else {
                out.join(format!("{}.png", v.name))
            };
            cubarium_gpu::target::write_png(&path, p.raster_w, p.raster_h, &rgba)?;
            for (c, b) in &crops {
                let (x0, y0, x1, y1) = (b[0].min(p.raster_w), b[1].min(p.raster_h), b[2].min(p.raster_w), b[3].min(p.raster_h));
                let (w, h) = (x1 - x0, y1 - y0);
                let mut px = Vec::with_capacity((w * h * 4) as usize);
                for y in y0..y1 {
                    let row = (y * p.raster_w + x0) as usize * 4;
                    px.extend_from_slice(&rgba[row..row + w as usize * 4]);
                }
                let path = if seconds > 0.0 {
                    out.join(format!("{}-{c}", v.name)).join(format!("f{k:05}.png"))
                } else {
                    out.join(format!("{}-{c}.png", v.name))
                };
                cubarium_gpu::target::write_png(&path, w, h, &px)?;
            }
        }
    }
    for v in &mut sinks {
        if let Some(e) = v.video.take() {
            e.finish()?;
        }
        for e in v.crops.drain(..) {
            e.finish()?;
        }
    }
    let mut costs = std::fs::File::create(out.join("costs.txt"))?;
    let ticks = (sim.world().tick() - ticks0).max(1) as f64;
    for (v, p0) in sinks.iter_mut().zip(p0) {
        let (n, stages) = v.sink.gpu_stage_totals();
        let _ = v.gpu;
        let (fig_ms, fig_n) = v.sink.figure_gpu();
        let p = v.sink.params();
        let line = format!(
            "{}: raster {}x{} ({} px/voxel); GPU {:.3} ms/frame (upload {:.3}, walk+models+weather+bloom {:.3}, present {:.3}) over {} frames; \
             model pass {:.3} GPU ms/frame over {} frames; draw (CPU, record+submit+wait) {:.3} ms/frame, of which the clock (the figures' frame) {:.3}; pack {:.3} ms/tick over {} ticks",
            v.name,
            p.raster_w,
            p.raster_h,
            p.s,
            stages.iter().sum::<f64>() / n.max(1) as f64,
            stages[0] / n.max(1) as f64,
            stages[1] / n.max(1) as f64,
            stages[2] / n.max(1) as f64,
            n,
            fig_ms,
            fig_n,
            v.draw_ms / v.frames.max(1) as f64,
            v.clock_ms / v.frames.max(1) as f64,
            (v.sink.draw_split().0 - p0) / ticks,
            ticks as u64,
        );
        println!("{line}");
        writeln!(costs, "{line}")?;
    }
    Ok(())
}
