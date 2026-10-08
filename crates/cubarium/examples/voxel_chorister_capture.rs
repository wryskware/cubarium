//! `voxel_chorister_capture`: a chorister pack in the live world (roster R3), headless GPU
//! stills.
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_chorister_capture -- OUT_DIR \
//!     [--config config/desktop/terrarium.toml] [--seed 1] [--px 8] [--scale 3] [--search-min 30]
//! ```
//!
//! Founds the config's world as `founder_tally` does (the asked seed, heuristics, one
//! thread, the preset's one pack), steps it, and renders the world once with the GPU
//! renderer, offscreen (no window), after its first 10 s, the first time the pack is
//! **together** (three or more awake members within 4 m of their centre, each 0.8 m from
//! the others) and the first time it is **on a chase** (a pack target, two such members
//! within 4 m of it). Writes the whole raster
//! and a crop around the members, upscaled `--scale` times nearest-neighbour:
//! `pack.png`/`pack-crop.png`, `chase.*`. Stops when it has both or at `--search-min`
//! simulated minutes, and prints each one's tick and place.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{LayerConfig, OrganismLook, Seeds, found, load_config, params_of};
use cubarium_voxel_fauna::{Animal, Founder, State, TICK_HZ};
use cubarium_voxel_sim::{Sim, SimConfig};

fn main() -> Result<()> {
    let (mut out, mut config, mut seed) = (None, "config/desktop/terrarium.toml".to_string(), 1u64);
    let (mut px, mut scale, mut search_min) = (8u32, 3u32, 30u64);
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
    let out = out.context("usage: voxel_chorister_capture OUT_DIR [options]")?;
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
    let ring = f64::from(world_cfg.width) * world_cfg.voxel_m;
    let wrap = |d: f64| {
        let d = d.rem_euclid(ring);
        if d > 0.5 * ring { d - ring } else { d }
    };
    let mut done = [false; 2];
    for tick in 1..=search_min * 60 * hz {
        sim.step();
        let fauna = sim.fauna();
        let Some(pack) = fauna.packs().first().copied() else {
            continue;
        };
        let members: Vec<Animal> = fauna
            .view()
            .animals
            .iter()
            .filter(|a| a.founder == Some(Founder::Chorister) && a.pack.is_some_and(|m| m.pack == pack.id))
            .copied()
            .collect();
        if members.is_empty() {
            continue;
        }
        // The members' centre on the ring, relative to the first.
        let x0 = members[0].pose.x;
        let n = members.len() as f64;
        let cx = x0 + members.iter().map(|a| wrap(a.pose.x - x0)).sum::<f64>() / n;
        let cz = members.iter().map(|a| a.pose.z).sum::<f64>() / n;
        // Members within `r` of a point, each at least 0.8 m from every other one counted
        // (bodies standing on one face would draw as one).
        let apart = |x: f64, z: f64, r: f64| {
            let mut kept: Vec<&Animal> = Vec::new();
            for a in members.iter().filter(|a| a.state != State::Asleep) {
                let d = (wrap(a.pose.x - x).powi(2) + (a.pose.z - z).powi(2)).sqrt();
                let clear = kept
                    .iter()
                    .all(|b| (wrap(a.pose.x - b.pose.x).powi(2) + (a.pose.z - b.pose.z).powi(2)).sqrt() >= 0.8);
                if d <= r && clear {
                    kept.push(a);
                }
            }
            kept.len()
        };
        let settled = tick >= 10 * hz;
        let shot = if !done[0] && settled && apart(cx, cz, 4.0) >= 3 {
            Some((0, "pack", cx, cz))
        } else if !done[1] && settled {
            pack.target
                .and_then(|t| fauna.view().animal(t).copied())
                .filter(|t| apart(t.pose.x, t.pose.z, 4.0) >= 2)
                .map(|t| (1, "chase", t.pose.x, t.pose.z))
        } else {
            None
        };
        let Some((k, name, x, z)) = shot else {
            continue;
        };
        done[k] = true;
        let y = members.iter().map(|a| a.pose.y_m).fold(f64::INFINITY, f64::min);
        shoot(&mut sink, &sim, &proj, tick, (x.rem_euclid(ring), y, z), &members, &out, name, scale)?;
        if done.iter().all(|d| *d) {
            break;
        }
    }
    for (k, name) in ["pack", "chase"].iter().enumerate() {
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
    (x, y, z): (f64, f64, f64),
    members: &[Animal],
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
    // The crop: 40 voxels wide, 24 tall, around the point at the pack's height.
    let v = sim.world().config().voxel_m;
    let (vx, vz) = ((x / v).floor() as i64, (z / v).floor().max(0.0) as u32);
    let vy = (y / v).floor().max(0.0) as u32;
    let s = proj.s as i32;
    let (cx, cy) = (proj.col(vx) + s / 2, proj.front_row(vy, vz));
    let (half_w, half_h) = (20 * s, 12 * s);
    let x0 = (cx - half_w).clamp(0, w as i32) as u32;
    let x1 = (cx + half_w).clamp(0, w as i32) as u32;
    let y0 = (cy - half_h - s * 4).clamp(0, h as i32) as u32;
    let y1 = (cy + half_h - s * 4).clamp(0, h as i32) as u32;
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
    let at: Vec<String> = members
        .iter()
        .map(|a| format!("#{}({:.1},{:.1}{})", a.id, a.pose.x, a.pose.z, if a.pack.is_some_and(|m| m.pup) { " pup" } else { "" }))
        .collect();
    println!(
        "{name}: tick {tick} ({:.1} min) at x={x:.2} z={z:.2}, members {} -> {}",
        tick as f64 / (60 * TICK_HZ) as f64,
        at.join(" "),
        path.display()
    );
    Ok(())
}
