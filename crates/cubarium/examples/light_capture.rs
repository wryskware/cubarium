//! `light_capture`: one founded world drawn headless by the GPU renderer at several
//! scales, plain and textured, with the pack and GPU times of each (package L's evidence,
//! `design/handoffs/presentation-plan-2026-09-24.md`).
//!
//! ```text
//! cargo run --release -p cubarium --example light_capture -- --out DIR \
//!     [--config config/desktop/terrarium.toml] [--set 'lighting = "lit"']... \
//!     [--seed 1] [--ticks 0] [--state DIR] [--px 6 --px 13 | --px auto] [--textures] \
//!     [--frames 60] [--name NAME]
//! ```
//!
//! The world is founded exactly as the live run founds one from a seed
//! (`ambient_habitat`), stepped `--ticks` more, and then every variant is drawn from that
//! one state, so two pictures differ only by what they were asked to differ by.
//! `--state DIR` saves that state on the first run and loads it on the next, so a second
//! build (main, say) draws the same world. Each `--set` line replaces that top-level key
//! of the config file (or adds it) for one run. Each variant writes
//! `NAME-<px>px[-tex].png` into `--out` and prints its mean pack and GPU times over
//! `--frames` packed-and-drawn frames. No window is ever opened.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::project::Projection;
use cubarium::voxel::{VoxelConfig, ambient_habitat, habitat};
use cubarium_voxel::World;
use cubarium_voxel_fauna::Fauna;
use cubarium_voxel_flora::{Flora, FloraConfig};
use cubarium_voxel_sim::{Sim, SimConfig};

struct Args {
    config: PathBuf,
    set: Vec<String>,
    seed: u64,
    ticks: u64,
    state: Option<PathBuf>,
    out: PathBuf,
    px: Vec<Option<u32>>,
    textures: bool,
    frames: u32,
    name: String,
}

fn args() -> Result<Args> {
    let mut a = Args {
        config: PathBuf::from("config/desktop/terrarium.toml"),
        set: Vec::new(),
        seed: 1,
        ticks: 0,
        state: None,
        out: PathBuf::new(),
        px: Vec::new(),
        textures: false,
        frames: 60,
        name: "world".into(),
    };
    let mut out = None;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().with_context(|| format!("{k} needs a value"));
        match k.as_str() {
            "--config" => a.config = PathBuf::from(v()?),
            "--set" => a.set.push(v()?),
            "--seed" => a.seed = v()?.parse()?,
            "--ticks" => a.ticks = v()?.parse()?,
            "--state" => a.state = Some(PathBuf::from(v()?)),
            "--out" => out = Some(PathBuf::from(v()?)),
            "--px" => {
                let s = v()?;
                a.px.push(if s == "auto" { None } else { Some(s.parse()?) });
            }
            "--textures" => a.textures = true,
            "--frames" => a.frames = v()?.parse()?,
            "--name" => a.name = v()?,
            _ => bail!("unexpected argument {k}"),
        }
    }
    a.out = out.context("--out DIR")?;
    if a.px.is_empty() {
        a.px.push(Some(6));
    }
    Ok(a)
}

fn config(a: &Args) -> Result<VoxelConfig> {
    let text = std::fs::read_to_string(&a.config)
        .with_context(|| format!("reading {}", a.config.display()))?;
    let mut table: toml::Table = text
        .parse()
        .with_context(|| format!("parsing {}", a.config.display()))?;
    for line in &a.set {
        let set: toml::Table = line.parse().with_context(|| format!("--set {line:?}"))?;
        table.extend(set);
    }
    toml::Value::Table(table)
        .try_into()
        .with_context(|| format!("{} with {:?}", a.config.display(), a.set))
}

fn found(a: &Args, cfg: &VoxelConfig) -> Result<(World, Flora, Fauna)> {
    let files = a.state.as_ref().map(|d| {
        (
            d.join("world.voxel"),
            d.join("flora.bin"),
            d.join("fauna.bin"),
        )
    });
    if let Some((w, f, b)) = &files
        && w.exists()
    {
        eprintln!(
            "light_capture: loading the state in {}",
            a.state.as_ref().unwrap().display()
        );
        let world = World::load(&std::fs::read(w)?)?;
        let flora = cubarium_voxel_flora::snapshot::decode(&std::fs::read(f)?)?;
        let fauna = Fauna::load(&std::fs::read(b)?)?;
        return Ok((world, flora, fauna));
    }
    let started = Instant::now();
    let founded = ambient_habitat(
        &cfg.world,
        a.seed,
        FloraConfig::for_voxel_size,
        habitat::FOUNDER_COUNTS,
    );
    eprintln!(
        "light_capture: founded seed {} in {:.1} s ({} stands)",
        founded.seed,
        started.elapsed().as_secs_f64(),
        founded.flora.view().stands.len(),
    );
    let (mut world, mut flora, mut fauna) = (founded.world, founded.flora, founded.fauna);
    if a.ticks > 0 {
        let threads = cubarium_voxel::default_threads();
        let mut sim = Sim::new(world, flora, fauna, SimConfig { threads }, None);
        for _ in 0..a.ticks {
            sim.step();
        }
        (world, flora, fauna) = (
            sim.world().clone(),
            sim.flora().clone(),
            sim.fauna().clone(),
        );
    }
    if let Some((w, f, b)) = &files {
        std::fs::create_dir_all(a.state.as_ref().unwrap())?;
        std::fs::write(w, world.save())?;
        std::fs::write(f, cubarium_voxel_flora::snapshot::encode(&flora))?;
        std::fs::write(b, fauna.save())?;
    }
    Ok((world, flora, fauna))
}

fn main() -> Result<()> {
    let a = args()?;
    let base = config(&a)?;
    let (world, flora, fauna) = found(&a, &base)?;
    std::fs::create_dir_all(&a.out)?;
    let models = cubarium::voxel::load_models(&base);
    for px in &a.px {
        let s = px.unwrap_or_else(|| {
            let screen = cubarium::voxel::desktop_screen().unwrap_or((1920, 1080));
            cubarium::voxel::auto_px_per_voxel(
                base.tilt_degrees,
                base.raster_height,
                &base.world,
                screen,
            )
        });
        let looks: &[bool] = if a.textures { &[false, true] } else { &[false] };
        for &textures in looks {
            let cfg = VoxelConfig {
                px_per_voxel: s,
                textures,
                ..base.clone()
            };
            let tag = format!("{}-{s}px{}", a.name, if textures { "-tex" } else { "" });
            capture(&a, &cfg, &tag, &world, &flora, &fauna, models.clone())?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn capture(
    a: &Args,
    cfg: &VoxelConfig,
    tag: &str,
    world: &World,
    flora: &Flora,
    fauna: &Fauna,
    models: Option<std::sync::Arc<cubarium::voxel::model::ModelLibrary>>,
) -> Result<()> {
    let proj = Projection::new(
        cfg.tilt_degrees,
        cfg.px_per_voxel,
        cfg.raster_height,
        &cfg.world,
    )?;
    let mut sink = VoxelGpuSink::new(
        cfg,
        proj,
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            models,
            ..VoxelGpuSinkOptions::default()
        },
    )?;
    // The lit tier's sky plane is computed off this thread: pack until it has arrived,
    // so the picture shows the finished light.
    let started = Instant::now();
    loop {
        if !sink.stage_world(world, flora, fauna) {
            bail!("the renderer refused a pack with no frame in flight");
        }
        if !sink.light_pending() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let light_s = started.elapsed().as_secs_f64();
    sink.render()?;
    let p = sink.params();
    let rgba = sink.read_raster()?;
    let path = a.out.join(format!("{tag}.png"));
    cubarium_gpu::target::write_png(&path, p.raster_w, p.raster_h, &rgba)?;

    // Timing: every frame packs the world and draws it, as a tick's frame does.
    let (pack0, _) = sink.draw_split();
    let (frames0, stages0) = sink.gpu_stage_totals();
    for _ in 0..a.frames {
        sink.stage_world(world, flora, fauna);
        sink.render()?;
    }
    let (pack1, _) = sink.draw_split();
    let (frames1, stages1) = sink.gpu_stage_totals();
    let n = (frames1 - frames0).max(1) as f64;
    println!(
        "{tag}: {}x{} — pack {:.3} ms/tick, GPU upload {:.3} + slab walk {:.3} + present {:.3} ms/frame \
         over {} frames; sky plane ready after {:.2} s -> {}",
        p.raster_w,
        p.raster_h,
        (pack1 - pack0) / f64::from(a.frames.max(1)),
        (stages1[0] - stages0[0]) / n,
        (stages1[1] - stages0[1]) / n,
        (stages1[2] - stages0[2]) / n,
        a.frames,
        light_s,
        display(&path),
    );
    Ok(())
}

fn display(p: &Path) -> String {
    std::fs::canonicalize(p)
        .unwrap_or_else(|_| p.to_path_buf())
        .display()
        .to_string()
}
