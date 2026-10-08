//! `roost_rain_capture`: ROOSTS-RAIN (backlog §12) in the live world as real-time MP4s,
//! headless (offscreen GPU, no window).
//!
//! ```text
//! roost_rain_capture OUT_DIR seedporter|bellwing [--config config/desktop/terrarium.toml]
//!     [--seed 41] [--px 8] [--scale 2] [--start-hour 19.0] [--max-s 60]
//! ```
//!
//! Founds the config's world (dry season off, the day clock at `--start-hour`) and steps it.
//! - `seedporter`: from the tick dusk falls (daylight under the day line), follows every
//!   awake seedporter in its own crop until it has slept 2 s or `--max-s` pass, and keeps the
//!   videos of those that end in a vaulttree crown or on a ledge top
//!   (`seedporter-<id>-<where>.mp4`; the rest are deleted).
//! - `bellwing`: one minute into daylight it starts following every flying bellwing, forces a
//!   shower 2 s later, and keeps each that lands within `--max-s`
//!   (`bellwing-<id>-<perch>.mp4`).
//!
//! Frames: 3 a tick (60 fps, real time), x264 yuv444p crf 12, upscaled `--scale`
//! nearest-neighbour. Prints each kept file's path.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{LayerConfig, OrganismLook, Seeds, found, load_config, params_of};
use cubarium_voxel::weather::RainMode;
use cubarium_voxel_fauna::{Founder, MobilityMode, PerchAnchor, State, TICK_HZ};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::{Sim, SimConfig};

/// The crop, in voxels: wide and tall enough to hold a walk to a roost and the climb.
const CROP_W: i32 = 40;
const CROP_H: i32 = 28;

struct Track {
    id: u64,
    ff: Child,
    path: PathBuf,
    centre: (f64, f64),
    slept_ticks: u64,
    done: bool,
    landed: Option<String>,
}

fn ffmpeg(path: &Path, w: u32, h: u32) -> Result<Child> {
    Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &format!("{w}x{h}"), "-r", "60", "-i", "-"])
        .args(["-c:v", "libx264", "-preset", "slow", "-crf", "12", "-pix_fmt", "yuv444p"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .context("ffmpeg")
}

fn main() -> Result<()> {
    let mut a = std::env::args().skip(1);
    let out = PathBuf::from(a.next().context("OUT_DIR")?);
    let what = a.next().context("seedporter|bellwing")?;
    let (mut config, mut seed, mut px, mut scale, mut start_hour, mut max_s) =
        ("config/desktop/terrarium.toml".to_string(), 41u64, 8u32, 2u32, 19.0f64, 60u64);
    while let Some(k) = a.next() {
        let mut val = || a.next().with_context(|| format!("{k} VALUE"));
        match k.as_str() {
            "--config" => config = val()?,
            "--seed" => seed = val()?.parse()?,
            "--px" => px = val()?.parse()?,
            "--scale" => scale = val()?.parse()?,
            "--start-hour" => start_hour = val()?.parse()?,
            "--max-s" => max_s = val()?.parse()?,
            _ => bail!("unexpected argument {k}"),
        }
    }
    let bellwing = match what.as_str() {
        "seedporter" => false,
        "bellwing" => true,
        _ => bail!("seedporter|bellwing"),
    };
    std::fs::create_dir_all(&out)?;
    let mut cfg = load_config(Path::new(&config))?;
    cfg.sim.world.day.start_hour = start_hour;
    cfg.sim.world.climate.dry_season = false;
    let params = params_of(&cfg, &[])?;
    let world_cfg = params.world.clone();
    let founded = found(&world_cfg, &LayerConfig::new(&params, world_cfg.voxel_m), params.fauna.founder_counts(), Seeds::from(seed), false);
    eprintln!("terrain seed {} founders {:?}", founded.seed, founded.seeded.founders);
    let mut fauna = founded.fauna;
    cubarium::voxel::install_default_founders(&mut fauna)?;
    let mut sim = Sim::ambient(founded.world, founded.flora, fauna, SimConfig::default());
    sim.set_overlap(false);
    let world_cfg = sim.world().config().clone();
    cfg.sim.world = world_cfg.clone();
    cfg.raster_height = 0;
    cfg.px_per_voxel = px;
    cfg.organisms = OrganismLook::Models;
    let models = Arc::new(ModelLibrary::load(&cfg.models_dir, world_cfg.voxel_m).context("the baked models")?);
    let proj = Projection::new(cfg.tilt_degrees, px, 0, &world_cfg)?;
    let mut sink = VoxelGpuSink::new(&cfg, proj, VoxelGpuSinkOptions { models: Some(models), ..Default::default() })?;
    let (rw, rh) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
    let s = proj.s as i32;
    let (cw, ch) = ((CROP_W * s) as u32, (CROP_H * s) as u32);
    let v = world_cfg.voxel_m;
    let hz = u64::from(TICK_HZ);
    let day_line = sim.fauna().config().life.day_line;

    // Run up to the start: dusk for the seedporter, a minute of daylight for the bellwing.
    let mut tick = 0u64;
    let mut last_day = sim.world().life_daylight();
    loop {
        sim.step();
        tick += 1;
        let d = sim.world().life_daylight();
        let go = if bellwing { d >= day_line && tick >= 60 * hz } else { last_day >= day_line && d < day_line };
        last_day = d;
        if go {
            break;
        }
        if tick > 6 * 3600 * hz {
            bail!("no start within six simulated hours");
        }
    }
    let rain_tick = tick + 2 * hz;
    let mut tracks: Vec<Track> = sim
        .fauna()
        .view()
        .animals
        .iter()
        .filter(|a| {
            if bellwing {
                a.founder == Some(Founder::Bellwing) && a.mobility.mode == MobilityMode::Fly
            } else {
                a.founder == Some(Founder::Seedporter) && a.state != State::Asleep
            }
        })
        .take(12)
        .map(|a| -> Result<Track> {
            let path = out.join(format!("{what}-{}.mp4", a.id));
            Ok(Track { id: a.id, ff: ffmpeg(&path, cw * scale, ch * scale)?, path, centre: (a.pose.x, a.pose.y_m), slept_ticks: 0, done: false, landed: None })
        })
        .collect::<Result<_>>()?;
    eprintln!("start at tick {tick} ({:.1} min), following {} bodies", tick as f64 / (60 * hz) as f64, tracks.len());
    let end = tick + max_s * hz;
    while tick < end && tracks.iter().any(|t| !t.done) {
        if bellwing && tick == rain_tick {
            let forced = sim.world_mut().force_rain(RainMode::Shower);
            eprintln!("shower forced at tick {tick}: {forced}");
        }
        sim.step();
        tick += 1;
        anyhow::ensure!(sink.stage_view(&sim.world().view(), sim.flora().view(), sim.fauna().view()), "no staging buffer");
        let fv = sim.flora().view();
        let mut frames: Vec<Vec<u8>> = Vec::with_capacity(3);
        for k in 0..3 {
            sink.set_clock(tick, f64::from(k) / 3.0);
            sink.render()?;
            frames.push(sink.read_raster()?);
        }
        let fauna = sim.fauna().view();
        for t in tracks.iter_mut().filter(|t| !t.done) {
            let Some(a) = fauna.animal(t.id) else {
                t.done = true;
                continue;
            };
            // A smoothed follow of the body's column and height.
            t.centre.0 += 0.05 * (a.pose.x - t.centre.0);
            t.centre.1 += 0.05 * (a.pose.y_m - t.centre.1);
            let (vx, vz, vy) = ((t.centre.0 / v).floor() as i64, (a.pose.z / v).floor().max(0.0) as u32, (t.centre.1 / v).floor().max(0.0) as u32);
            let (cx, cy) = (proj.col(vx) + s / 2, proj.front_row(vy, vz));
            let x0 = (cx - cw as i32 / 2).clamp(0, rw as i32 - cw as i32) as u32;
            let y0 = (cy - ch as i32 / 2 - 2 * s).clamp(0, rh as i32 - ch as i32) as u32;
            let stdin = t.ff.stdin.as_mut().context("ffmpeg stdin")?;
            for rgba in &frames {
                let mut buf = Vec::with_capacity((cw * ch * scale * scale * 3) as usize);
                for y in 0..ch * scale {
                    let sy = y0 + y / scale;
                    for x in 0..cw * scale {
                        let i = ((sy * rw + x0 + x / scale) * 4) as usize;
                        buf.extend_from_slice(&rgba[i..i + 3]);
                    }
                }
                stdin.write_all(&buf)?;
            }
            if bellwing {
                if tick > rain_tick && a.mobility.mode == MobilityMode::Perched {
                    t.landed.get_or_insert_with(|| format!("{:?}", a.mobility.perch.map(|p| p.kind)).to_lowercase());
                    t.slept_ticks += 1;
                }
            } else if a.state == State::Asleep {
                t.slept_ticks += 1;
                let place = match (a.mobility.mode, a.mobility.perch.map(|p| p.anchor)) {
                    (MobilityMode::Perched, Some(PerchAnchor::Stand { site, .. }))
                        if fv.stands_on(site).iter().any(|st| st.species == Species::Vaulttree) => "vaulttree",
                    (MobilityMode::Perched, Some(PerchAnchor::Face(_))) => "ledge-top",
                    (MobilityMode::Perched, Some(PerchAnchor::Stand { .. })) => "crown",
                    _ => "other",
                };
                t.landed.get_or_insert_with(|| place.to_string());
            }
            if t.slept_ticks >= 3 * hz {
                t.done = true;
            }
        }
    }
    for mut t in tracks {
        drop(t.ff.stdin.take());
        t.ff.wait()?;
        let keep = match (&t.landed, bellwing) {
            (Some(p), false) => p == "vaulttree" || p == "ledge-top",
            (Some(_), true) => true,
            (None, _) => false,
        };
        if keep {
            let name = out.join(format!("{what}-{}-{}.mp4", t.id, t.landed.as_deref().unwrap_or("none")));
            std::fs::rename(&t.path, &name)?;
            println!("kept {}", name.display());
        } else {
            let _ = std::fs::remove_file(&t.path);
            println!("dropped {what} {} ({:?})", t.id, t.landed);
        }
    }
    Ok(())
}
