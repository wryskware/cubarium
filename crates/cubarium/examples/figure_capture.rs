//! `figure_capture`: one saved world drawn by several variants of the GPU sink at once,
//! frame for frame on the same clock — H2's evidence (the stamped animals against the model
//! pass, its turn modes), `design/handoffs/hybrid-renderer-2026-10-07.md`.
//!
//! ```text
//! cargo run --release -p cubarium --example figure_capture -- --save DIR --out DIR \
//!     [--config config/desktop/terrarium.toml] [--set LINE]... [--px N] \
//!     [--variant NAME=SETLINE[;SETLINE]]... [--seconds 0] [--fps 60] [--warm 20] \
//!     [--crop NAME=X0,Y0,X1,Y1]... [--every 1] [--mp4] [--comparison] [--costs-only]
//!     [--view-x COLUMNS]
//! ```
//!
//! `--view-x` turns the picture that many raster columns round the ring, as the hosted
//! view's pan does (to put the world's seam mid-frame).
//!
//! `--mp4` pipes each variant's frames (and each crop's) to `ffmpeg` instead of PNGs:
//! `NAME.mp4` and `NAME-CROP.mp4`, x264 yuv444p crf 12 at `--fps`, real time.
//!
//! `--comparison --mp4` writes `comparison.mp4`: classic | crisp | smooth on one row,
//! then a 2x nearest crop of the same three. The first `--crop` supplies the close-up;
//! absent one, it uses the central half of the raster. Labels live in the capture only.
//! `--costs-only` renders the same frames without writing pictures.
//! `--specimen capgnawer --quarter 0 --out DIR` writes a controlled whole-pixel
//! stamped/model still and absolute pixel diff, without a save or simulation.
//!
//! The save (a session save directory, read only) is loaded once and stepped by one sim;
//! every tick is staged into every variant's sink, and every frame (`--fps` a second of sim
//! time, `--seconds` long, after `--warm` ticks) is drawn by each at the same clock and
//! written as `NAME/fKKKKK.png` (whole raster) and `NAME-CROP/fKKKKK.png` per crop. With
//! `--seconds 0` it writes one still per variant, `NAME.png`. Each variant's costs go to
//! `costs.txt`: GPU ms per frame (all, the model pass), CPU ms to draw a frame, pack ms per
//! tick. No window is ever opened.

#[path = "figure_capture/fixture.rs"]
mod fixture;

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
        Self::labelled(path, w, h, fps, None)
    }

    fn labelled(
        path: &std::path::Path,
        w: u32,
        h: u32,
        fps: f64,
        names: Option<&[String]>,
    ) -> Result<Encoder> {
        let mut command = std::process::Command::new("ffmpeg");
        command
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-s",
            ])
            .arg(format!("{w}x{h}"))
            .args(["-r"])
            .arg(format!("{fps}"))
            .args([
                "-i",
                "-",
                "-color_range",
                "pc",
                "-colorspace",
                "bt709",
                "-color_primaries",
                "bt709",
                "-color_trc",
                "iec61966-2-1",
                "-threads",
                "4",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv444p",
                "-crf",
                "12",
                "-preset",
                "fast",
            ]);
        let mut filters =
            vec!["scale=in_range=full:out_range=full:out_color_matrix=bt709".to_string()];
        if let Some(names) = names {
            let panel = w / 3;
            let labels: Vec<_> = names
                .iter()
                .enumerate()
                .flat_map(|(i, name)| {
                    let name: String = name
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    [4, h / 2 + 4].map(|y| {
                        format!(
                            "drawtext=text='{name}':x={}:y={y}:fontsize=18:fontcolor=white",
                            i as u32 * panel + 10
                        )
                    })
                })
                .collect();
            filters.extend(labels);
        }
        command.args(["-vf", &filters.join(",")]);
        let child = command
            .arg(path)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .context("ffmpeg")?;
        Ok(Encoder { child, w, h })
    }

    fn put(&mut self, rgba: &[u8]) -> Result<()> {
        debug_assert_eq!(rgba.len(), (self.w * self.h * 4) as usize);
        self.child
            .stdin
            .as_mut()
            .context("ffmpeg's stdin")?
            .write_all(rgba)?;
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
    let (mut seconds, mut fps, mut warm, mut px, mut every) =
        (0.0f64, 60.0f64, 20u64, None::<u32>, 1u64);
    let mut specimen = None::<String>;
    let mut quarter = 0u8;
    let (mut mp4, mut comparison, mut costs_only) = (false, false, false);
    let mut view_x = 0i32;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().context("missing value");
        match k.as_str() {
            "--specimen" => specimen = Some(v()?),
            "--quarter" => quarter = v()?.parse()?,
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
            "--view-x" => view_x = v()?.parse()?,
            "--comparison" => comparison = true,
            "--costs-only" => costs_only = true,
            "--variant" => {
                let s = v()?;
                let (name, lines) = s.split_once('=').context("--variant NAME=SET[;SET]")?;
                variants.push((
                    name.into(),
                    lines
                        .split(';')
                        .filter(|l| !l.is_empty())
                        .map(String::from)
                        .collect(),
                ));
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
    let out = out.context("--out DIR")?;
    if let Some(name) = specimen {
        return specimen_still(&name, quarter, &out);
    }
    let save = save.context("--save DIR")?;
    if !seconds.is_finite() || seconds < 0.0 || !fps.is_finite() || fps <= 0.0 || every == 0 {
        bail!("--seconds must be nonnegative, --fps and --every positive");
    }
    if variants.is_empty() {
        variants.extend(["classic", "crisp", "smooth"].map(|n| (n.into(), Vec::new())));
    }
    if comparison && (!mp4 || variants.len() != 3) {
        bail!("--comparison needs --mp4 and exactly three variants");
    }
    std::fs::create_dir_all(&out)?;
    let base = configured(&config, &sets)?;
    let prepared =
        cubarium_session::prepare_loaded(&base.sim, &save).map_err(anyhow::Error::msg)?;
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
        if ["classic", "crisp", "smooth"].contains(&name.as_str()) && lines.is_empty() {
            cfg.figures = cfg.figures.with_overrides(Some(name), None, None)?;
        }
        cfg.sim.world = world_cfg.clone();
        if let Some(px) = px {
            cfg.px_per_voxel = px;
        } else if cfg.px_per_voxel == cubarium::voxel::PX_AUTO {
            cfg.px_per_voxel = cubarium::voxel::cover_px_per_voxel(
                cfg.tilt_degrees,
                cfg.raster_height,
                &world_cfg,
                (1920, 1080),
            );
        }
        let proj = Projection::new(
            cfg.tilt_degrees,
            cfg.px_per_voxel,
            cfg.raster_height,
            &world_cfg,
        )?;
        let sink = VoxelGpuSink::new(
            &cfg,
            proj,
            VoxelGpuSinkOptions {
                target: GpuTargetKind::Headless,
                models: cubarium::voxel::load_models(&cfg),
                ..VoxelGpuSinkOptions::default()
            },
        )?;
        let mut sink = sink;
        sink.set_view_x(view_x);
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
    for v in &mut sinks {
        let started = Instant::now();
        loop {
            v.sink.set_clock(sim.world().tick(), 0.0);
            let staged = v.sink.stage_world(sim.world(), sim.flora(), sim.fauna());
            v.sink.render()?;
            if staged && !v.sink.light_pending() {
                break;
            }
            if started.elapsed().as_secs() > 120 {
                bail!("{}: the sky plane never arrived within 120 s", v.name);
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
    let p0 = sinks
        .iter()
        .map(|v| v.sink.draw_split().0)
        .collect::<Vec<_>>();
    let gpu0: Vec<_> = sinks
        .iter()
        .map(|v| (v.sink.gpu_stage_totals(), v.sink.figure_gpu()))
        .collect();
    let ticks0 = sim.world().tick();
    let frames = if seconds > 0.0 {
        (seconds * fps).round() as u64
    } else {
        1
    };
    let hz = f64::from(cubarium_voxel::TICK_HZ);
    let mut tick = sim.world().tick();
    let p = sinks[0].sink.params();
    let (w, h) = (p.raster_w, p.raster_h);
    let close = crops
        .first()
        .map_or([w / 4, h / 4, w / 4 + w / 2, h / 4 + h / 2], |c| c.1);
    if comparison
        && (close[2] <= close[0]
            || close[3] <= close[1]
            || close[2] > w
            || close[3] > h
            || 2 * (close[2] - close[0]) > w
            || 2 * (close[3] - close[1]) > h
            || sinks.iter().any(|v| {
                let p = v.sink.params();
                (p.raster_w, p.raster_h) != (w, h)
            }))
    {
        bail!(
            "comparison needs matching rasters and an in-bounds crop no larger than half a raster"
        );
    }
    let mut combined = if comparison && seconds > 0.0 && !costs_only {
        let names = sinks.iter().map(|v| v.name.clone()).collect::<Vec<_>>();
        Some(Encoder::labelled(
            &out.join("comparison.mp4"),
            3 * w,
            2 * (h + 28),
            fps / every as f64,
            Some(&names),
        )?)
    } else {
        None
    };
    for v in &mut sinks {
        if !comparison && !costs_only && mp4 && seconds > 0.0 {
            let p = v.sink.params();
            v.video = Some(Encoder::new(
                &out.join(format!("{}.mp4", v.name)),
                p.raster_w,
                p.raster_h,
                fps / every as f64,
            )?);
            for (c, b) in &crops {
                let (w, h) = (b[2].min(p.raster_w) - b[0], b[3].min(p.raster_h) - b[1]);
                v.crops.push(Encoder::new(
                    &out.join(format!("{}-{c}.mp4", v.name)),
                    w,
                    h,
                    fps / every as f64,
                )?);
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
        let mut row = Vec::new();
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
            if costs_only || k % every != 0 {
                continue;
            }
            let p = v.sink.params();
            let rgba = v.sink.read_raster()?;
            if combined.is_some() {
                row.push(rgba);
                continue;
            }
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
                let (x0, y0, x1, y1) = (
                    b[0].min(p.raster_w),
                    b[1].min(p.raster_h),
                    b[2].min(p.raster_w),
                    b[3].min(p.raster_h),
                );
                let (w, h) = (x1 - x0, y1 - y0);
                let mut px = Vec::with_capacity((w * h * 4) as usize);
                for y in y0..y1 {
                    let row = (y * p.raster_w + x0) as usize * 4;
                    px.extend_from_slice(&rgba[row..row + w as usize * 4]);
                }
                let path = if seconds > 0.0 {
                    out.join(format!("{}-{c}", v.name))
                        .join(format!("f{k:05}.png"))
                } else {
                    out.join(format!("{}-{c}.png", v.name))
                };
                cubarium_gpu::target::write_png(&path, w, h, &px)?;
            }
        }
        if let Some(e) = &mut combined {
            if !row.is_empty() {
                let pixels = comparison_frame(&row, w, h, close);
                e.put(&pixels)?;
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
    if let Some(e) = combined {
        e.finish()?;
    }
    let mut costs = std::fs::File::create(out.join("costs.txt"))?;
    let ticks = (sim.world().tick() - ticks0).max(1) as f64;
    for ((v, p0), ((n0, stages0), (figure0, figure_n0))) in sinks.iter_mut().zip(p0).zip(gpu0) {
        let (n, stages) = v.sink.gpu_stage_totals();
        let n = n - n0;
        let stages: [f64; 3] = std::array::from_fn(|i| stages[i] - stages0[i]);
        let _ = v.gpu;
        let (figure_ms, figure_n) = v.sink.figure_gpu();
        let fig_n = figure_n - figure_n0;
        let fig_ms =
            (figure_ms * figure_n as f64 - figure0 * figure_n0 as f64) / fig_n.max(1) as f64;
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

/// Whole rasters above; a nearest 2x crop below, centred in equally sized panels.
fn comparison_frame(frames: &[Vec<u8>], w: u32, h: u32, crop: [u32; 4]) -> Vec<u8> {
    let width = 3 * w;
    let mut out = vec![0; (width * 2 * (h + 28) * 4) as usize];
    for a in out.chunks_exact_mut(4) {
        a[3] = 255;
    }
    let (cw, ch) = (crop[2] - crop[0], crop[3] - crop[1]);
    for (i, frame) in frames.iter().enumerate() {
        for y in 0..h {
            let dst = (((y + 28) * width + i as u32 * w) * 4) as usize;
            let src = (y * w * 4) as usize;
            out[dst..dst + (w * 4) as usize].copy_from_slice(&frame[src..src + (w * 4) as usize]);
        }
        for y in 0..2 * ch {
            for x in 0..2 * cw {
                let src = (((crop[1] + y / 2) * w + crop[0] + x / 2) * 4) as usize;
                let dst = (((h + 56 + (h - 2 * ch) / 2 + y) * width
                    + i as u32 * w
                    + (w - 2 * cw) / 2
                    + x)
                    * 4) as usize;
                out[dst..dst + 4].copy_from_slice(&frame[src..src + 4]);
            }
        }
    }
    out
}

/// One baked animal at a quarter heading, without outside occluders or shadow-map
/// approximation: isolates the face tones and local crease from world lighting.
fn specimen_still(name: &str, quarter: u8, out: &std::path::Path) -> Result<()> {
    use cubarium::voxel::{colours, model::ModelLibrary};
    let cfg = fixture::config(0.5);
    let lib = ModelLibrary::load(
        std::path::Path::new("assets/voxel-models"),
        cfg.sim.world.voxel_m,
    )?;
    let model = lib.specimen(name).context("unknown --specimen")?;
    let cells = model.select(0.5).context("empty specimen")?;
    let top = cells.iter().map(|c| c.offset[1]).max().unwrap_or(0);
    let cells: Vec<_> = cells
        .iter()
        .map(|m| {
            let sw = colours::specimen(name, m.material, colours::band(m.offset[1], top))
                .context("no specimen coat")?;
            let rgb = [sw.shadow, sw.body, sw.glint];
            Ok(fixture::Cell {
                offset: m.offset.map(|v| v as i8),
                colours: rgb.map(cubarium::present::srgb_linear),
            })
        })
        .collect::<Result<_>>()?;
    let stamped = fixture::draw(&cells, 0, quarter, true, &cfg)?;
    let crisp = fixture::draw(&cells, 0, quarter, false, &cfg)?;
    let (w, h) = (
        u32::from(stamped.projection.raster_w),
        u32::from(stamped.projection.raster_h),
    );
    let mut diff = vec![0u8; stamped.pixels.len()];
    let (mut changed, mut max, mut sum) = (0, 0, 0u64);
    for ((a, b), d) in stamped
        .pixels
        .chunks_exact(4)
        .zip(crisp.pixels.chunks_exact(4))
        .zip(diff.chunks_exact_mut(4))
    {
        let mut any = false;
        for k in 0..3 {
            d[k] = a[k].abs_diff(b[k]);
            any |= d[k] != 0;
            max = max.max(d[k]);
            sum += u64::from(d[k]);
        }
        d[3] = 255;
        changed += u64::from(any);
    }
    std::fs::create_dir_all(out)?;
    for (name, px) in [
        ("stamped", &stamped.pixels),
        ("crisp", &crisp.pixels),
        ("diff", &diff),
    ] {
        cubarium_gpu::target::write_png(&out.join(format!("{name}.png")), w, h, px)?;
    }
    let report = format!(
        "{name}, quarter {quarter}: {changed} / {} pixels differ; max RGB code delta {max}, mean absolute RGB delta {:.6}\nControlled open sky, no sun/shadows, haze, terrain/plants or poses; same baked voxels and coat.\n",
        w * h,
        sum as f64 / f64::from(w * h * 3)
    );
    print!("{report}");
    std::fs::write(out.join("diff.txt"), report)?;
    Ok(())
}
