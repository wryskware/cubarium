//! `fog_capture`: a founded world stepped live and drawn headless frame by frame, to follow
//! the ground fog as it forms and clears (FOG-MOTION, `design/handoffs/fog-motion-2026-10-10.md`).
//!
//! ```text
//! cargo run --release -p cubarium --example fog_capture -- \
//!     [--config config/desktop/terrarium.toml] [--set 'KEY = VALUE']... [--seed 1] \
//!     [--state DIR] [--load DIR] [--rh R] [--warm TICKS] [--until-fog F [--max-ticks N]] \
//!     [--seconds S] [--fps 60] [--speed K] [--px N|auto] [--crop X0,Y0,X1,Y1] \
//!     [--preview loop|day|fixed:PHASE[,CLOUD[,RAIN[,FOG]]]] \
//!     [--log FILE.csv] [--video FILE.mp4] [--png-every N --out DIR]
//! ```
//!
//! The world is founded as the live run founds one (or loaded from `--load`, or from
//! `--state`, which a first run saves once the wait below is over), stepped `--warm` ticks and then on until the view's fog reaches
//! `--until-fog` (at most `--max-ticks`), headless and undrawn; `--rh` tops the air's
//! moisture up to that RH first (and every 2000 ticks of the wait), so the fog can form. Then `--seconds` of video
//! time are drawn at `--fps`, the sim stepping a tick each time the frame clock crosses one,
//! as the live loop does; `--speed K` runs the sim (and so the weather) K times faster than
//! the video. Every frame logs the view's fog and RH, the drawn look's fog, its floor and
//! clock, and how far the picture's mean luminance moved since the frame before (`--log`);
//! `--video` pipes the frames to ffmpeg (x264, yuv444p, crf 12). No window is ever opened.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::project::Projection;
use cubarium::voxel::{LayerConfig, Seeds, VoxelConfig, found as found_habitat, params_of};
use cubarium_voxel_sim::{Sim, SimConfig};

struct Args {
    config: PathBuf,
    set: Vec<String>,
    seed: u64,
    state: Option<PathBuf>,
    load: Option<PathBuf>,
    warm: u64,
    until_fog: Option<f32>,
    rh: Option<f64>,
    max_ticks: u64,
    seconds: f64,
    fps: f64,
    speed: f64,
    px: Option<u32>,
    crop: Option<[u32; 4]>,
    preview: Option<cubarium::sink::gpu::weather::WeatherPreview>,
    log: Option<PathBuf>,
    video: Option<PathBuf>,
    png_every: u32,
    out: Option<PathBuf>,
}

fn args() -> Result<Args> {
    let mut a = Args {
        config: PathBuf::from("config/desktop/terrarium.toml"),
        set: Vec::new(),
        seed: 1,
        state: None,
        load: None,
        warm: 0,
        until_fog: None,
        rh: None,
        max_ticks: 200_000,
        seconds: 10.0,
        fps: 60.0,
        speed: 1.0,
        px: Some(6),
        crop: None,
        preview: None,
        log: None,
        video: None,
        png_every: 0,
        out: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().with_context(|| format!("{k} needs a value"));
        match k.as_str() {
            "--config" => a.config = PathBuf::from(v()?),
            "--set" => a.set.push(v()?),
            "--seed" => a.seed = v()?.parse()?,
            "--state" => a.state = Some(PathBuf::from(v()?)),
            "--load" => a.load = Some(PathBuf::from(v()?)),
            "--warm" => a.warm = v()?.parse()?,
            "--until-fog" => a.until_fog = Some(v()?.parse()?),
            "--rh" => a.rh = Some(v()?.parse()?),
            "--max-ticks" => a.max_ticks = v()?.parse()?,
            "--seconds" => a.seconds = v()?.parse()?,
            "--fps" => a.fps = v()?.parse()?,
            "--speed" => a.speed = v()?.parse()?,
            "--px" => {
                let s = v()?;
                a.px = if s == "auto" { None } else { Some(s.parse()?) };
            }
            "--crop" => {
                let s = v()?;
                let n: Vec<u32> = s.split(',').map(str::parse).collect::<Result<_, _>>()?;
                let [x0, y0, x1, y1] = n[..] else { bail!("--crop X0,Y0,X1,Y1, not {s:?}") };
                a.crop = Some([x0, y0, x1, y1]);
            }
            "--preview" => a.preview = Some(v()?.parse()?),
            "--log" => a.log = Some(PathBuf::from(v()?)),
            "--video" => a.video = Some(PathBuf::from(v()?)),
            "--png-every" => a.png_every = v()?.parse()?,
            "--out" => a.out = Some(PathBuf::from(v()?)),
            _ => bail!("unexpected argument {k}"),
        }
    }
    Ok(a)
}

fn config(a: &Args) -> Result<VoxelConfig> {
    let text = std::fs::read_to_string(&a.config).with_context(|| format!("reading {}", a.config.display()))?;
    let mut table: toml::Table = text.parse().with_context(|| format!("parsing {}", a.config.display()))?;
    for line in &a.set {
        let set: toml::Table = line.parse().with_context(|| format!("--set {line:?}"))?;
        merge(&mut table, set);
    }
    toml::Value::Table(table).try_into().with_context(|| format!("{} with {:?}", a.config.display(), a.set))
}

fn merge(table: &mut toml::Table, set: toml::Table) {
    for (k, v) in set {
        match (table.get_mut(&k), v) {
            (Some(toml::Value::Table(t)), toml::Value::Table(s)) => merge(t, s),
            (_, v) => {
                table.insert(k, v);
            }
        }
    }
}

fn status(sim: &Sim) -> String {
    let w = sim.world();
    let v = w.view().weather;
    format!(
        "tick {} phase {:.3} fog {:.4} rh {:.4} mode {:?} daylight {:.2}",
        w.tick(),
        v.day_phase,
        v.fog,
        w.rh(),
        v.mode,
        v.daylight
    )
}

/// Capture-only: add atmosphere until the air's RH reads `target` (fog needs humid air,
/// which a fresh world takes days to reach).
fn humidify(sim: &mut Sim, target: Option<f64>) {
    let Some(target) = target else { return };
    let w = sim.world_mut();
    let rh = w.rh();
    if rh > 0.0 && rh < target {
        let add = w.view().atmosphere_m3 * (target / rh - 1.0);
        w.apply(cubarium_voxel::Command::AddAtmosphere { volume_m3: add });
    }
}

fn main() -> Result<()> {
    let a = args()?;
    let cfg = config(&a)?;
    let threads = cubarium_voxel::default_threads();
    let files = a.state.as_ref().map(|d| (d.join("world.voxel"), d.join("flora.bin"), d.join("fauna.bin")));
    let from = a.load.clone().or_else(|| files.as_ref().filter(|(w, _, _)| w.exists()).map(|_| a.state.clone().unwrap()));
    let (world, flora, fauna) = match &from {
        Some(dir) => {
            let s = cubarium_voxel_sim::saved::load(dir, &[]).map_err(anyhow::Error::msg)?;
            (s.world, s.flora, s.fauna)
        }
        _ => {
            let params = params_of(&cfg, &[])?;
            let f = found_habitat(
                &cfg.sim.world,
                &LayerConfig::new(&params, cfg.sim.world.voxel_m),
                cubarium_voxel_fauna::FaunaConfig::default().founder_counts(),
                Seeds::from(a.seed),
                false,
            );
            (f.world, f.flora, f.fauna)
        }
    };
    let mut sim = Sim::new(world, flora, fauna, SimConfig::with_threads(threads), None);
    let started = Instant::now();
    humidify(&mut sim, a.rh);
    for i in 0..a.warm {
        sim.step();
        if i % 2000 == 0 {
            eprintln!("fog_capture: warm {}", status(&sim));
        }
    }
    if let Some(f) = a.until_fog {
        let mut n = 0;
        while sim.world().view().weather.fog < f && n < a.max_ticks {
            sim.step();
            n += 1;
            if n % 2000 == 0 {
                humidify(&mut sim, a.rh);
                eprintln!("fog_capture: waiting {}", status(&sim));
            }
        }
    }
    eprintln!("fog_capture: start {} after {:.1} s", status(&sim), started.elapsed().as_secs_f64());
    if let Some((w, f, b)) = &files
        && !w.exists()
    {
        std::fs::create_dir_all(a.state.as_ref().unwrap())?;
        std::fs::write(w, sim.world().save())?;
        std::fs::write(f, cubarium_voxel_flora::snapshot::encode(sim.flora()))?;
        std::fs::write(b, sim.fauna().save())?;
    }

    let s = a.px.unwrap_or_else(|| {
        cubarium::voxel::auto_px_per_voxel(cfg.tilt_degrees, cfg.raster_height, &cfg.sim.world, (1920, 1080))
    });
    let cfg = VoxelConfig { px_per_voxel: s, ..cfg };
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &cfg.sim.world)?;
    let models = cubarium::voxel::load_models(&cfg);
    let mut sink = VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions { target: GpuTargetKind::Headless, models, ..VoxelGpuSinkOptions::default() },
    )?;
    sink.set_weather_preview(a.preview);
    loop {
        if !sink.stage_world(sim.world(), sim.flora(), sim.fauna()) {
            bail!("the renderer refused a pack");
        }
        if !sink.light_pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let p = sink.params();
    let [x0, y0, x1, y1] = a.crop.unwrap_or([0, 0, p.raster_w, p.raster_h]);
    let (x1, y1) = (x1.min(p.raster_w), y1.min(p.raster_h));
    let (cw, ch) = (x1 - x0, y1 - y0);
    eprintln!("fog_capture: {s} px a voxel, raster {}x{}, frame {cw}x{ch}", p.raster_w, p.raster_h);

    let mut ffmpeg = match &a.video {
        Some(path) => Some(
            Command::new("ffmpeg")
                .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
                .arg(format!("{cw}x{ch}"))
                .args(["-r"])
                .arg(format!("{}", a.fps))
                .args(["-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv444p", "-crf", "12", "-preset", "fast"])
                .arg(path)
                .stdin(Stdio::piped())
                .spawn()
                .context("starting ffmpeg")?,
        ),
        None => None,
    };
    let mut log = match &a.log {
        Some(path) => {
            let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
            writeln!(f, "frame,video_s,tick,view_fog,rh,mode,look_fog,floor,wx_s,drift,mean_l,dl_mean,dl_max,draw_ms")?;
            Some(f)
        }
        None => None,
    };
    if let Some(o) = &a.out {
        std::fs::create_dir_all(o)?;
    }

    let tick_hz = f64::from(cubarium_voxel::TICK_HZ);
    let tick0 = sim.world().tick();
    let frames = (a.seconds * a.fps).round() as u64;
    let mut prev: Option<Vec<f32>> = None;
    let mut draw_total = 0.0;
    let film0 = Instant::now();
    let (frames0, gpu0) = sink.gpu_stage_totals();
    let animal_start: std::collections::BTreeMap<_, _> = sim.fauna().view().animals.iter()
        .map(|a| (a.id, (a.pose.x, a.pose.z))).collect();
    for k in 0..frames {
        let video_s = k as f64 / a.fps;
        let t = video_s * a.speed * tick_hz;
        let want = tick0 + t.floor() as u64;
        let mut stepped = false;
        while sim.world().tick() < want {
            sim.step();
            stepped = true;
        }
        if stepped {
            sink.stage_world(sim.world(), sim.flora(), sim.fauna());
        }
        sink.set_presenter_clock(Some(film0 + std::time::Duration::from_secs_f64(video_s)));
        sink.set_clock(want, t.fract());
        let at = Instant::now();
        sink.render()?;
        let rgba = sink.read_raster()?;
        let draw_ms = at.elapsed().as_secs_f64() * 1e3;
        draw_total += draw_ms;
        let mut crop = Vec::with_capacity(cw as usize * ch as usize * 4);
        for y in y0..y1 {
            let row = (y * p.raster_w + x0) as usize * 4;
            crop.extend_from_slice(&rgba[row..row + cw as usize * 4]);
        }
        // Luminance per pixel (sRGB bytes, Rec. 709 weights): the mean, and how far it moved.
        let lum: Vec<f32> = crop
            .chunks_exact(4)
            .map(|c| 0.2126 * f32::from(c[0]) + 0.7152 * f32::from(c[1]) + 0.0722 * f32::from(c[2]))
            .collect();
        let mean = lum.iter().sum::<f32>() / lum.len().max(1) as f32;
        let (dmean, dmax) = match &prev {
            Some(q) => {
                let mut sum = 0.0f32;
                let mut max = 0.0f32;
                for (a, b) in lum.iter().zip(q) {
                    let d = (a - b).abs();
                    sum += d;
                    max = max.max(d);
                }
                (sum / lum.len().max(1) as f32, max)
            }
            None => (0.0, 0.0),
        };
        prev = Some(lum);
        if let Some(f) = &mut log {
            let w = sim.world();
            let v = w.view().weather;
            let (look, floor) = sink.look();
            writeln!(
                f,
                "{k},{video_s:.4},{},{:.5},{:.5},{:?},{:.5},{floor},{:.3},{:.3},{mean:.3},{dmean:.4},{dmax:.1},{draw_ms:.2}",
                w.tick(),
                v.fog,
                w.rh(),
                v.mode,
                look.fog / cubarium_gpu::weather::FOG_DENSITY,
                look.seconds,
                look.drift,
            )?;
        }
        if let Some(ff) = &mut ffmpeg {
            ff.stdin.as_mut().context("ffmpeg stdin")?.write_all(&crop)?;
        }
        if a.png_every > 0
            && k % u64::from(a.png_every) == 0
            && let Some(o) = &a.out
        {
            cubarium_gpu::target::write_png(&o.join(format!("fog-{k:05}.png")), cw, ch, &crop)?;
        }
    }
    if let Some(mut ff) = ffmpeg {
        drop(ff.stdin.take());
        ff.wait()?;
    }
    let (frames1, gpu1) = sink.gpu_stage_totals();
    let animals_moved = sim.fauna().view().animals.iter().filter(|a| {
        animal_start.get(&a.id).is_some_and(|&p| p != (a.pose.x, a.pose.z))
    }).count();
    eprintln!("fog_capture: liveness animals_moved={animals_moved}/{}", animal_start.len());
    let n = (frames1 - frames0).max(1) as f64;
    eprintln!(
        "fog_capture: {frames} frames, mean draw+readback {:.2} ms, GPU walk stage (walk, plants, \
         figures, weather, bloom) {:.3} ms; end {}",
        draw_total / frames.max(1) as f64,
        (gpu1[1] - gpu0[1]) / n,
        status(&sim)
    );
    Ok(())
}
