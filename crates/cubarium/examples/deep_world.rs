//! Live-water depth cost study, using the host's founding, controllers and GPU sink.
//! Run through scripts/deep-world-batch.py; all output is headless and disposable.
//! `deep_world config=... out=... threads=16 seed=1 [minutes=60 frames=3 raster=2560x1600]`
//! A config with a fixed `px_per_voxel` is drawn at that size; `"auto"` fits the raster.
//! `minutes=0` renders founding only. A short smoke can use `ticks=40`.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, ensure};
use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::project::Projection;
use cubarium::voxel::{LayerConfig, Seeds};
use cubarium_voxel::profile::{self, Phase};
use cubarium_voxel_sim::{Sim, SimConfig};
use serde::Serialize;

#[derive(Serialize)]
struct Row {
    window: String,
    ticks: u64,
    frames: u64,
    step_ms: f64,
    step_max_ms: f64,
    world_ms: f64,
    flora_ms: f64,
    fauna_ms: f64,
    pack_ms: f64,
    render_ms: f64,
    gpu_raster_ms: f64,
    tick_speed: f64,
    live_speed_60fps: f64,
    stands: usize,
    ground: usize,
    animals: usize,
    phase_ms: Vec<(&'static str, f64)>,
    counters_per_tick: Vec<(&'static str, f64)>,
}

#[derive(Serialize)]
struct Report {
    config: String,
    seed: u64,
    depth: u32,
    threads: usize,
    cpus: String,
    water_every: u32,
    overlap: bool,
    founding_accepted: bool,
    founding_warnings: Vec<String>,
    raster: [u32; 2],
    px_per_voxel: u32,
    sim_minutes: f64,
    rows: Vec<Row>,
}

#[derive(Default)]
struct Window {
    ticks: u64,
    frames: u64,
    step_ms: f64,
    max_ms: f64,
    pack_ms: f64,
    render_ms: f64,
    gpu_ms: f64,
}

impl Window {
    fn row(&self, label: String, sim: &Sim) -> Row {
        let n = self.ticks.max(1) as f64;
        let step_ms = self.step_ms / n;
        let pack_ms = self.pack_ms / n;
        let render_ms = self.render_ms / self.frames.max(1) as f64;
        let phase = |p| profile::nanos(p) as f64 / 1e6 / n;
        Row {
            window: label,
            ticks: self.ticks,
            frames: self.frames,
            step_ms,
            step_max_ms: self.max_ms,
            world_ms: phase(Phase::WaterLeg),
            flora_ms: phase(Phase::FloraStep),
            fauna_ms: phase(Phase::FaunaStep),
            pack_ms,
            render_ms,
            gpu_raster_ms: self.gpu_ms / self.frames.max(1) as f64,
            tick_speed: 50.0 / step_ms.max(f64::EPSILON),
            live_speed_60fps: (1000.0 - 60.0 * render_ms).max(0.0)
                / (20.0 * (step_ms + pack_ms)).max(f64::EPSILON),
            stands: sim.flora().view().stands.len(),
            ground: sim.flora().view().ground.len(),
            animals: sim.fauna().view().animals.len(),
            phase_ms: Phase::ALL
                .into_iter()
                .map(|p| (p.name(), phase(p)))
                .collect(),
            counters_per_tick: profile::Count::ALL
                .into_iter()
                .map(|c| (c.name(), profile::count(c) as f64 / n))
                .collect(),
        }
    }
}

fn settle(sink: &mut VoxelGpuSink, sim: &Sim) -> Result<()> {
    let (world, flora, fauna) = sim.layers();
    let at = Instant::now();
    // The lit sink bakes the sky asynchronously. No simulation advances during this
    // founding/capture setup, and none of this cost enters a measured window.
    loop {
        ensure!(
            sink.stage_world(world, flora, fauna),
            "headless sink refused a pack"
        );
        if !sink.light_pending() {
            break;
        }
        ensure!(at.elapsed().as_secs() < 60, "sky bake did not finish");
        std::thread::yield_now();
    }
    sink.set_clock(world.tick(), 0.0);
    sink.render()?;
    Ok(())
}

fn capture(sink: &mut VoxelGpuSink, sim: &Sim, path: &std::path::Path, size: (u32, u32)) -> Result<()> {
    settle(sink, sim)?;
    cubarium_gpu::target::write_png(path, size.0, size.1, &sink.read_headless_output()?)?;
    eprintln!("picture: {}", path.display());
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |key: &str| args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")));
    let config = PathBuf::from(get("config").context("config=<toml>")?);
    let out = PathBuf::from(get("out").context("out=<directory>")?);
    let threads: usize = get("threads").unwrap_or("16").parse()?;
    let seed: u64 = get("seed").unwrap_or("1").parse()?;
    let minutes: u64 = get("minutes").unwrap_or("60").parse()?;
    let frames: u64 = get("frames").unwrap_or("3").parse()?;
    let pictures: bool = get("pictures").unwrap_or("true").parse()?;
    let raster: (u32, u32) = {
        let r = get("raster").unwrap_or("2560x1600");
        let (w, h) = r.split_once('x').context("raster=WxH")?;
        (w.parse()?, h.parse()?)
    };
    ensure!(
        threads > 0 && frames > 0,
        "threads and frames must be positive"
    );
    let ticks_per_minute = 60 * u64::from(cubarium_voxel::TICK_HZ);
    let ticks: u64 = get("ticks")
        .map(str::parse)
        .transpose()?
        .unwrap_or(minutes * ticks_per_minute);
    std::fs::create_dir_all(&out)?;
    cubarium_voxel::set_thread_override(threads);
    cubarium_voxel_sim::prepare_compute_pool(threads);
    // The six-thread arm is also restricted to three physical cores by the batch.
    let mut cfg = cubarium::voxel::load_config(&config)?;
    let params = cubarium::voxel::params_of(&cfg, &[])?;
    ensure!(
        cfg.sim.water_every == 1,
        "cost study needs the desktop's live water"
    );
    let founded = cubarium::voxel::found(
        &cfg.sim.world,
        &LayerConfig::new(&params, cfg.sim.world.voxel_m),
        params.fauna.founder_counts(),
        Seeds::Asked(seed),
        false,
    );
    let accepted = founded.accepted;
    eprintln!(
        "founding: seed {} land {:.1} m2, land scale {:.3}, founders {:?}",
        founded.seed, founded.seeded.land_m2, founded.seeded.land_scale, founded.seeded.wanted
    );
    let warnings = founded.warnings;
    let mut fauna = founded.fauna;
    cubarium::voxel::install_default_founders(&mut fauna)?;
    let mut world = founded.world;
    world.set_water_every(1);
    let mut sim = Sim::ambient(
        world,
        founded.flora,
        fauna,
        SimConfig::with_threads(threads),
    );
    if cfg.px_per_voxel == cubarium::voxel::PX_AUTO {
        cfg.px_per_voxel =
            cubarium::voxel::auto_px_per_voxel(cfg.tilt_degrees, 0, &cfg.sim.world, raster);
    }
    cfg.raster_height = raster.1 as u16;
    let proj = Projection::new(
        cfg.tilt_degrees,
        cfg.px_per_voxel,
        cfg.raster_height,
        &cfg.sim.world,
    )?;
    let mut sink = VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            models: cubarium::voxel::load_models(&cfg),
            ..VoxelGpuSinkOptions::default()
        },
    )?;
    sink.set_headless_size(raster)?;
    if pictures {
        capture(&mut sink, &sim, &out.join("founding.png"), raster)?;
    } else {
        settle(&mut sink, &sim)?;
    }
    let mut report = Report {
        config: config.display().to_string(),
        seed,
        depth: cfg.sim.world.depth,
        threads,
        cpus: std::fs::read_to_string("/proc/self/status")?
            .lines()
            .find_map(|l| l.strip_prefix("Cpus_allowed_list:"))
            .unwrap_or("unknown")
            .trim()
            .to_owned(),
        water_every: sim.world().water_every(),
        overlap: sim.config().overlap,
        founding_accepted: accepted,
        founding_warnings: warnings,
        raster: [raster.0, raster.1],
        px_per_voxel: cfg.px_per_voxel,
        sim_minutes: ticks as f64 / ticks_per_minute as f64,
        rows: Vec::new(),
    };
    let mut window = Window::default();
    let mut was_measuring = false;
    for tick in 0..ticks {
        let measuring = tick < 5 * ticks_per_minute
            || (55 * ticks_per_minute..60 * ticks_per_minute).contains(&tick);
        if measuring && !was_measuring {
            if tick > 0 {
                settle(&mut sink, &sim)?;
            }
            profile::reset();
            window = Window::default();
        }
        let started = Instant::now();
        sim.step();
        let step_ms = started.elapsed().as_secs_f64() * 1e3;
        if measuring {
            window.ticks += 1;
            window.step_ms += step_ms;
            window.max_ms = window.max_ms.max(step_ms);
            let (world, flora, fauna) = sim.layers();
            let (pack0, _) = sink.draw_split();
            ensure!(
                sink.stage_world(world, flora, fauna),
                "headless sink refused a tick"
            );
            let (pack1, _) = sink.draw_split();
            window.pack_ms += pack1 - pack0;
            for frame in 0..frames {
                sink.set_clock(world.tick(), frame as f64 / frames as f64);
                let (_, render0) = sink.draw_split();
                let (_, gpu0) = sink.gpu_stage_totals();
                sink.render()?;
                let (_, render1) = sink.draw_split();
                let (_, gpu1) = sink.gpu_stage_totals();
                window.render_ms += render1 - render0;
                window.gpu_ms += gpu1.iter().sum::<f64>() - gpu0.iter().sum::<f64>();
                window.frames += 1;
            }
        }
        if pictures && tick + 1 == 30 * ticks_per_minute {
            capture(&mut sink, &sim, &out.join("m30.png"), raster)?;
        }
        if measuring
            && (tick + 1 == 5 * ticks_per_minute
                || tick + 1 == ticks
                || tick + 1 == 60 * ticks_per_minute)
        {
            let label = if tick < 5 * ticks_per_minute {
                "0–5"
            } else {
                "55–60"
            };
            let row = window.row(label.into(), &sim);
            eprintln!(
                "d{} t{threads} {label}: step {:.3} world {:.3} flora {:.3} fauna {:.3} pack {:.3} render {:.3} ms; {:.2}x tick budget",
                cfg.sim.world.depth,
                row.step_ms,
                row.world_ms,
                row.flora_ms,
                row.fauna_ms,
                row.pack_ms,
                row.render_ms,
                row.tick_speed
            );
            report.rows.push(row);
            // A partial result remains readable if the batch's single long timeout expires.
            std::fs::write(out.join("cost.json"), serde_json::to_vec_pretty(&report)?)?;
        }
        was_measuring = measuring;
    }
    std::fs::write(out.join("cost.json"), serde_json::to_vec_pretty(&report)?)?;
    sink.finish()?;
    Ok(())
}
