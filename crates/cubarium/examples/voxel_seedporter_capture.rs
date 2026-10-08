//! `voxel_seedporter_capture`: the seedporter in the live world (F3), headless GPU stills.
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_seedporter_capture -- OUT_DIR \
//!     [--config config/desktop/terrarium.toml] [--seed 1] [--px 8] [--scale 4] [--search-min 30]
//! ```
//!
//! Founds the config's world as `founder_tally` does (the asked seed, heuristics, one
//! thread), steps it, and the first time a seedporter is **perched in a crown**, **gliding**
//! or **climbing** it renders the world once with the GPU renderer, offscreen (no window),
//! and writes the whole raster and a crop around that seedporter, upscaled `--scale` times
//! nearest-neighbour: `perched.png`/`perched-crop.png`, `glide.*`, `climb.*`. It stops when
//! it has all three or at `--search-min` simulated minutes, and prints each one's tick,
//! body and place.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{LayerConfig, OrganismLook, Seeds, found, load_config, params_of};
use cubarium_voxel_fauna::{Animal, Founder, MobilityMode, PerchAnchor, TICK_HZ};
use cubarium_voxel_sim::{Sim, SimConfig};

fn main() -> Result<()> {
    let (mut out, mut config, mut seed) = (None, "config/desktop/terrarium.toml".to_string(), 1u64);
    let (mut px, mut scale, mut search_min) = (8u32, 4u32, 30u64);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = |k: &str| args.next().with_context(|| format!("{k} VALUE"));
        match a.as_str() {
            "--config" => config = val("--config")?,
            "--seed" => seed = val("--seed")?.parse()?,
            "--px" => px = val("--px")?.parse()?,
            "--scale" => scale = val("--scale")?.parse()?,
            "--search-min" => search_min = val("--search-min")?.parse()?,
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let out = out.context("usage: voxel_seedporter_capture OUT_DIR [options]")?;
    std::fs::create_dir_all(&out)?;
    cubarium_voxel::set_thread_override(1);
    let mut cfg = load_config(Path::new(&config))?;
    let params = params_of(&cfg, &[])?;
    let world_cfg = params.world.clone();
    let founded = found(
        &world_cfg,
        &LayerConfig::new(&params, world_cfg.voxel_m),
        params.fauna.founder_counts(),
        Seeds::Asked(seed),
        false,
    );
    eprintln!("seed {} founders {:?}", founded.seed, founded.seeded.founders);
    let mut sim = Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::default());
    sim.set_overlap(false);

    cfg.sim.world = world_cfg.clone();
    cfg.raster_height = 0;
    cfg.px_per_voxel = px;
    cfg.organisms = OrganismLook::Models;
    let models = Arc::new(ModelLibrary::load(&cfg.models_dir, world_cfg.voxel_m).context("the baked models")?);
    let proj = Projection::new(cfg.tilt_degrees, px, 0, &world_cfg)?;
    let mut sink = VoxelGpuSink::new(&cfg, proj, VoxelGpuSinkOptions { models: Some(models), ..Default::default() })?;

    let hz = u64::from(TICK_HZ);
    let wanted = ["perched", "glide", "climb"];
    let mut done = [false; 3];
    for tick in 1..=search_min * 60 * hz {
        sim.step();
        let hit = sim.fauna().view().animals.iter().find_map(|a| {
            if a.founder != Some(Founder::Seedporter) {
                return None;
            }
            let k = match (a.mobility.mode, a.mobility.perch.map(|p| p.anchor)) {
                (MobilityMode::Perched, Some(PerchAnchor::Stand { .. })) => 0,
                (MobilityMode::Glide, _) => 1,
                (MobilityMode::Climb, _) => 2,
                _ => return None,
            };
            (!done[k]).then_some((k, *a))
        });
        let Some((k, a)) = hit else {
            continue;
        };
        done[k] = true;
        shoot(&mut sink, &sim, &proj, tick, &a, &out, wanted[k], scale)?;
        if done.iter().all(|d| *d) {
            break;
        }
    }
    for (k, name) in wanted.iter().enumerate() {
        if !done[k] {
            println!("{name}: none within {search_min} min");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn shoot(
    sink: &mut VoxelGpuSink,
    sim: &Sim,
    proj: &Projection,
    tick: u64,
    a: &Animal,
    out: &Path,
    name: &str,
    scale: u32,
) -> Result<()> {
    anyhow::ensure!(
        sink.stage_view(&sim.world().view(), sim.flora().view(), sim.fauna().view()),
        "no staging buffer"
    );
    sink.set_clock(tick, 0.0);
    sink.render()?;
    let rgba = sink.read_raster()?;
    let (w, h) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
    cubarium_gpu::target::write_png(&out.join(format!("{name}.png")), w, h, &rgba)?;
    // The crop: 16 voxels wide, 14 tall, around the body's column at its height.
    let v = sim.world().config().voxel_m;
    let (vx, vz) = ((a.pose.x / v).floor() as i64, (a.pose.z / v).floor().max(0.0) as u32);
    let vy = (a.pose.y_m / v).floor().max(0.0) as u32;
    let s = proj.s as i32;
    let (cx, cy) = (proj.col(vx) + s / 2, proj.front_row(vy, vz));
    let (half_w, half_h) = (8 * s, 7 * s);
    let x0 = (cx - half_w).clamp(0, w as i32) as u32;
    let x1 = (cx + half_w).clamp(0, w as i32) as u32;
    let y0 = (cy - half_h - s * 2).clamp(0, h as i32) as u32;
    let y1 = (cy + half_h - s * 2).clamp(0, h as i32) as u32;
    let (cw, ch) = (x1 - x0, y1 - y0);
    let mut crop = Vec::with_capacity((cw * ch * scale * scale * 4) as usize);
    for y in 0..ch * scale {
        let sy = y0 + y / scale;
        for x in 0..cw * scale {
            let sx = x0 + x / scale;
            let i = ((sy * w + sx) * 4) as usize;
            crop.extend_from_slice(&rgba[i..i + 4]);
        }
    }
    let path = out.join(format!("{name}-crop.png"));
    cubarium_gpu::target::write_png(&path, cw * scale, ch * scale, &crop)?;
    println!(
        "{name}: tick {tick} ({:.1} min) id {} mode {:?} perch {:?} at x={:.2} y={:.2} z={:.2} -> {}",
        tick as f64 / (60 * TICK_HZ) as f64,
        a.id,
        a.mobility.mode,
        a.mobility.perch.map(|p| p.kind),
        a.pose.x,
        a.pose.y_m,
        a.pose.z,
        path.display()
    );
    Ok(())
}
