//! The lake's reed margin, measured: shallow-water area, saturated-ground area and
//! siphonreed stands at founding and after `minutes` of simulated time, headless.
//! `lake_margin config=... out=... seed=1 [threads=16 minutes=30 raster=WxH pictures=true]`
//!
//! Per open column (its highest solid voxel and the standing water over it):
//! - **shallow**: standing water between the siphonreed's `water_depth_min_m` and its
//!   `drown_depth_m`, the depths a reed stands in;
//! - **deep**: standing water deeper than `drown_depth_m`;
//! - **bank**: no standing water of its own but beside shallow water whose surface
//!   reaches its ground (the flora's own beside-the-water rule);
//! - **saturated**: no standing water and its top voxel's pore water at or above the
//!   siphonreed's `saturated_pore`.
//!
//! Writes `margin.json` and, with pictures, `founding.png` and `mNN.png`.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, ensure};
use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::project::Projection;
use cubarium::voxel::{LayerConfig, Seeds};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::{Sim, SimConfig};
use serde::Serialize;

#[derive(Serialize, Clone, Copy, Default)]
struct Margin {
    minute: f64,
    shallow_m2: f64,
    deep_m2: f64,
    bank_m2: f64,
    saturated_m2: f64,
    reed_stands: usize,
    stands: usize,
}

fn measure(sim: &Sim, minute: f64, min_m: f64, drown_m: f64, sat: f64) -> Margin {
    let stands = &sim.flora().view().stands;
    Margin {
        stands: stands.len(),
        reed_stands: stands.iter().filter(|s| s.species == Species::Siphonreed).count(),
        ..measure_world(sim.world(), minute, min_m, drown_m, sat)
    }
}

fn measure_world(world: &cubarium_voxel::World, minute: f64, min_m: f64, drown_m: f64, sat: f64) -> Margin {
    let view = world.view();
    let c = view.config;
    let (w, d) = (c.width as i64, c.depth);
    let a = c.voxel_m * c.voxel_m;
    let top = |x: i64, z: u32| (0..c.height).rev().find(|&y| view.material_at(x.rem_euclid(w), y, z).is_solid());
    let mut m = Margin { minute, ..Margin::default() };
    for z in 0..d {
        for x in 0..w {
            let Some(y) = top(x, z) else { continue };
            let depth = view.standing_depth_m(x, y, z);
            if depth > drown_m {
                m.deep_m2 += a;
            } else if depth >= min_m {
                m.shallow_m2 += a;
            } else {
                let ground = (y + 1) as f64 * c.voxel_m;
                let beside = [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)].into_iter().any(|(dx, dz)| {
                    let nz = z as i64 + dz;
                    if nz < 0 || nz >= d as i64 {
                        return false;
                    }
                    let nx = (x + dx).rem_euclid(w);
                    let Some(ny) = top(nx, nz as u32) else { return false };
                    let nd = view.standing_depth_m(nx, ny, nz as u32);
                    ny <= y && nd >= min_m && (ny + 1) as f64 * c.voxel_m + nd >= ground
                });
                if beside {
                    m.bank_m2 += a;
                } else if view.pore_at(x, y, z) >= sat {
                    m.saturated_m2 += a;
                }
            }
        }
    }
    m
}

fn settle(sink: &mut VoxelGpuSink, sim: &Sim) -> Result<()> {
    let (world, flora, fauna) = sim.layers();
    let at = Instant::now();
    loop {
        ensure!(sink.stage_world(world, flora, fauna), "headless sink refused a pack");
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
    let minutes: u64 = get("minutes").unwrap_or("30").parse()?;
    let pictures: bool = get("pictures").unwrap_or("true").parse()?;
    let raster: (u32, u32) = {
        let r = get("raster").unwrap_or("3840x832");
        let (w, h) = r.split_once('x').context("raster=WxH")?;
        (w.parse()?, h.parse()?)
    };
    std::fs::create_dir_all(&out)?;
    cubarium_voxel::set_thread_override(threads);
    cubarium_voxel_sim::prepare_compute_pool(threads);
    let mut cfg = cubarium::voxel::load_config(&config)?;
    let params = cubarium::voxel::params_of(&cfg, &[])?;
    let reed = params.flora.siphonreed.clone();
    let (min_m, drown_m, sat) = (reed.water_depth_min_m, reed.drown_depth_m, reed.saturated_pore);
    if get("gen") == Some("true") {
        // The generated world alone, out of `generate_with_a_lake`: no founding, no flora.
        let (world, _, _) = cubarium_voxel_sim::found::generate_with_a_lake(&cfg.sim.world, Some(seed), 1, || 0);
        let r = measure_world(&world, 0.0, min_m, drown_m, sat);
        if get("hist") == Some("true") {
            // Standing water depth by whole voxels over every open column.
            let view = world.view();
            let c = view.config;
            let mut hist = std::collections::BTreeMap::<i64, usize>::new();
            for z in 0..c.depth {
                for x in 0..c.width as i64 {
                    let Some(y) = (0..c.height).rev().find(|&y| view.material_at(x, y, z).is_solid()) else { continue };
                    let d = view.standing_depth_m(x, y, z);
                    if d > 0.0 {
                        *hist.entry((d / c.voxel_m).round() as i64).or_default() += 1;
                    }
                }
            }
            eprintln!("depth voxels -> columns: {hist:?}");
        }
        eprintln!(
            "gen seed {seed}: shallow {:7.2} m2 deep {:7.2} bank {:7.2} saturated {:7.2}",
            r.shallow_m2, r.deep_m2, r.bank_m2, r.saturated_m2
        );
        return Ok(());
    }
    let founded = cubarium::voxel::found(
        &cfg.sim.world,
        &LayerConfig::new(&params, cfg.sim.world.voxel_m),
        params.fauna.founder_counts(),
        Seeds::Asked(seed),
        false,
    );
    let mut fauna = founded.fauna;
    cubarium::voxel::install_default_founders(&mut fauna)?;
    let mut world = founded.world;
    world.set_water_every(cfg.sim.water_every.max(1));
    let mut sim = Sim::ambient(world, founded.flora, fauna, SimConfig::with_threads(threads));
    let mut sink = if pictures {
        if cfg.px_per_voxel == cubarium::voxel::PX_AUTO {
            cfg.px_per_voxel = cubarium::voxel::auto_px_per_voxel(cfg.tilt_degrees, 0, &cfg.sim.world, raster);
        }
        cfg.raster_height = raster.1 as u16;
        let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &cfg.sim.world)?;
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
        capture(&mut sink, &sim, &out.join("founding.png"), raster)?;
        Some(sink)
    } else {
        None
    };
    // The lake's middle column round the ring (a circular mean of its drain), for crops.
    let width = cfg.sim.world.width;
    let lake_x = {
        let drain = sim.world().lake_drain();
        let tau = std::f64::consts::TAU;
        let (sx, cx) = drain.iter().fold((0.0, 0.0), |(s, c), &(x, _, _)| {
            let a = tau * x as f64 / width as f64;
            (s + a.sin(), c + a.cos())
        });
        (sx.atan2(cx).rem_euclid(tau) / tau * width as f64).round() as u32
    };
    let mut rows = vec![measure(&sim, 0.0, min_m, drown_m, sat)];
    let tpm = 60 * u64::from(cubarium_voxel::TICK_HZ);
    for tick in 0..minutes * tpm {
        sim.step();
        if (tick + 1) % (10 * tpm) == 0 || tick + 1 == minutes * tpm {
            rows.push(measure(&sim, (tick + 1) as f64 / tpm as f64, min_m, drown_m, sat));
        }
    }
    if let Some(sink) = sink.as_mut()
        && minutes > 0
    {
        capture(sink, &sim, &out.join(format!("m{minutes}.png")), raster)?;
    }
    for r in &rows {
        eprintln!(
            "seed {seed} m{:>4.0}: shallow {:7.2} m2 deep {:7.2} bank {:7.2} saturated {:7.2} reeds {:4} stands {}",
            r.minute, r.shallow_m2, r.deep_m2, r.bank_m2, r.saturated_m2, r.reed_stands, r.stands
        );
    }
    std::fs::write(
        out.join("margin.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "config": config.display().to_string(),
            "seed": seed,
            "reed_depth_m": [min_m, drown_m],
            "lake_x": lake_x,
            "width": width,
            "px_per_voxel": cfg.px_per_voxel,
            "saturated_pore": sat,
            "rows": rows,
        }))?,
    )?;
    if let Some(mut sink) = sink {
        sink.finish()?;
    }
    Ok(())
}
