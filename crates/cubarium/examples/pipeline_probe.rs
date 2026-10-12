//! `pipeline_probe`: what holds a compiled shader pipeline. Opens a headless renderer for a
//! world of `--size` once per step, each on a Vulkan device of its own (as every preview card
//! and live world does), and prints how long each open took. A timing, not a check; no window.
//!
//! ```text
//! cargo run --release -p cubarium --example pipeline_probe -- \
//!     [--config config/desktop/terrarium.toml] [--size 288,138,48] [--screen 576x324] \
//!     [--steps still,still,all] [--together N] [--cache DIR]
//! ```
//!
//! A step is `still` or `all` ([`Pipelines`]); `--together N` opens N renderers of the first
//! step's kind at once instead (each on its own device and thread). `--cache DIR` keeps the
//! app's own pipeline cache there (`cubarium_gpu::pipeline_cache`). Point the NVIDIA driver at
//! an empty `__GL_SHADER_DISK_CACHE_PATH` for a cold driver cache.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use cubarium::sink::gpu::voxel::{Offscreen, OffscreenOptions, SkyBake, VoxelGpuSink};
use cubarium_gpu::voxel::Pipelines;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{PX_AUTO, VoxelConfig, cover_px_per_voxel, load_config, load_models};

fn main() -> Result<()> {
    let mut config = PathBuf::from("config/desktop/terrarium.toml");
    let (mut size, mut screen, mut steps, mut together) =
        ([288u32, 138, 48], (576u32, 324u32), "still,still,all".to_string(), 0usize);
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().with_context(|| format!("{k} needs a value"));
        match k.as_str() {
            "--config" => config = PathBuf::from(v()?),
            "--size" => {
                let n: Vec<u32> = v()?.split(',').map(str::parse).collect::<Result<_, _>>()?;
                let [w, h, d] = n[..] else { bail!("--size W,H,D") };
                size = [w, h, d];
            }
            "--screen" => {
                let s = v()?;
                let (w, h) = s.split_once('x').context("--screen WxH")?;
                screen = (w.parse()?, h.parse()?);
            }
            "--steps" => steps = v()?,
            "--together" => together = v()?.parse()?,
            "--cache" => cubarium_gpu::pipeline_cache::set_dir(Some(PathBuf::from(v()?))),
            _ => bail!("unexpected argument {k}"),
        }
    }
    let root = std::env::current_dir()?;
    let mut cfg = load_config(&config)?;
    cfg.models_dir = root.join(&cfg.models_dir);
    cfg.textures_dir = root.join(&cfg.textures_dir);
    cfg.sim.world.width = size[0];
    cfg.sim.world.height = size[1];
    cfg.sim.world.depth = size[2];
    if cfg.px_per_voxel == PX_AUTO {
        cfg.px_per_voxel = cover_px_per_voxel(cfg.tilt_degrees, cfg.raster_height, &cfg.sim.world, screen);
    }
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &cfg.sim.world)?;
    let kinds: Vec<Pipelines> = steps
        .split(',')
        .map(|s| match s {
            "still" => Ok(Pipelines::Still),
            "all" => Ok(Pipelines::All),
            o => bail!("no step {o} (still, all)"),
        })
        .collect::<Result<_>>()?;
    let t0 = Instant::now();
    if together > 0 {
        let kind = kinds[0];
        let ms: Vec<f64> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..together).map(|_| sc.spawn(|| open(&cfg, proj, kind))).collect();
            hs.into_iter().map(|h| h.join().expect("open thread")).collect::<Result<Vec<_>>>()
        })?;
        println!("PROBE together {together} x {kind:?}: {:?} ms, wall {:.0} ms", ms.iter().map(|m| m.round()).collect::<Vec<_>>(), t0.elapsed().as_secs_f64() * 1e3);
        return Ok(());
    }
    for (i, kind) in kinds.into_iter().enumerate() {
        let ms = open(&cfg, proj, kind)?;
        println!("PROBE step {i} {kind:?}: {ms:.0} ms (t {:.0} ms)", t0.elapsed().as_secs_f64() * 1e3);
    }
    Ok(())
}

/// Open a renderer on a new device, then drop it: milliseconds.
fn open(cfg: &VoxelConfig, proj: Projection, kind: Pipelines) -> Result<f64> {
    let started = Instant::now();
    let models = load_models(cfg);
    let o = OffscreenOptions { pipelines: kind, sky: SkyBake::Never, sky_fade_s: 0.0 };
    let sink: VoxelGpuSink<Offscreen> = VoxelGpuSink::offscreen_with(cfg, proj, models, o)?;
    let ms = started.elapsed().as_secs_f64() * 1e3;
    drop(sink);
    Ok(ms)
}
