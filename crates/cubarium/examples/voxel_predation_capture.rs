//! `voxel_predation_capture`: a lanternjaw waiting, a strike, and the carcass being eaten
//! (P1, `design/predation-and-flight-2026-09-28.md` §2), headless, straight to an MP4.
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_predation_capture -- OUT.mp4 \
//!     [--preset small] [--seed 1] [--introduce 3] [--px 8] [--scale 2] \
//!     [--search-min 90] [--lead-s 20] [--tail-s 20]
//! ```
//!
//! Pass one runs the founded world (the shipped trained controllers, the lanternjaws
//! introduced hungry) with no drawing and notes the ticks of the first strike, the first
//! kill and the first carrion the lanternjaw eats after it, giving up at `--search-min`
//! simulated minutes. Pass two founds the same world again on one thread, so it repeats,
//! and draws 60 frames a second of the world's own time (three per 20 Hz tick) from
//! `--lead-s` before the first strike to `--tail-s` after the eating begins, piping raw
//! RGB to ffmpeg (x264, yuv444p, crf 12, nearest-neighbour upscale). If no kill happens,
//! the window is the first strike, or failing that the first hour of the lanternjaws'
//! waiting, and the report says so. The GPU renderer is offscreen; no window is opened,
//! and no frame PNG is written.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{Lighting, OrganismLook, VoxelConfig, install_founders_with};
use cubarium_voxel::Command as WorldCommand;
use cubarium_voxel_fauna::{Departure, Founder, Senses, TICK_HZ};
use cubarium_voxel_flora::FloraConfig;
use cubarium_voxel_sim::{Sim, SimConfig, habitat};

#[derive(Default, Debug, Clone, Copy)]
struct Events {
    strike: Option<u64>,
    kill: Option<u64>,
    eat: Option<u64>,
    /// The first tick after the eating that a lanternjaw lies down in its satiated rest.
    rest: Option<u64>,
}

fn found(preset: &str, seed: u64, introduce: usize) -> Result<Sim> {
    let preset = cubarium_voxel::Preset::find(preset).context("no such preset")?;
    let f = cubarium::voxel::ambient_habitat(
        &preset.config(),
        seed,
        FloraConfig::for_voxel_size,
        habitat::FOUNDER_COUNTS,
    );
    let (world, flora, mut fauna) = (f.world, f.flora, f.fauna);
    install_founders_with(&mut fauna, &[])?;
    match habitat::introduce_lanternjaws(&world, &flora, &mut fauna, introduce) {
        Ok(ids) => eprintln!("introduced {} adult lanternjaws", ids.len()),
        Err(why) => bail!("introduce refused: {why}"),
    }
    let mut senses = Senses::with_cue(fauna.config().cue);
    senses.settle(&world.view(), &flora.view());
    let mut sim = Sim::new(world, flora, fauna, SimConfig::default(), Some(senses));
    if sim.world().config().closed_water_budget && !sim.world().outlet_open() {
        sim.world_mut().apply(WorldCommand::SetOutlet { open: true });
    }
    Ok(sim)
}

/// Read the three events' first ticks off the ledger, as they happen.
fn note(sim: &Sim, tick: u64, ev: &mut Events) {
    let l = sim.fauna().view().ledger;
    let lj = Founder::Lanternjaw.index();
    if ev.strike.is_none() && l.strikes_by_founder[lj] > 0 {
        ev.strike = Some(tick);
    }
    if ev.kill.is_none() {
        let killed: u64 = Founder::ALL
            .iter()
            .map(|&p| l.departed_founder(p, Departure::Killed { by: Founder::Lanternjaw }))
            .sum();
        if killed > 0 {
            ev.kill = Some(tick);
        }
    }
    if ev.kill.is_some() && ev.eat.is_none() && l.eaten_carrion_by_founder[lj] > 0.0 {
        ev.eat = Some(tick);
    }
    if ev.eat.is_some() && ev.rest.is_none() {
        let satiated = sim.fauna().view().animals.iter().any(|a| {
            a.founder == Some(Founder::Lanternjaw) && a.rest == Some(cubarium_voxel_fauna::Rest::Satiated)
        });
        if satiated {
            ev.rest = Some(tick);
        }
    }
}

fn main() -> Result<()> {
    let (mut out, mut preset, mut seed, mut introduce) = (None, "small".to_string(), 1u64, 3usize);
    let (mut px, mut scale, mut search_min, mut lead_s, mut tail_s) = (8u32, 2u32, 90u64, 20.0f64, 20.0f64);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = |k: &str| args.next().with_context(|| format!("{k} VALUE"));
        match a.as_str() {
            "--preset" => preset = val("--preset")?,
            "--seed" => seed = val("--seed")?.parse()?,
            "--introduce" => introduce = val("--introduce")?.parse()?,
            "--px" => px = val("--px")?.parse()?,
            "--scale" => scale = val("--scale")?.parse()?,
            "--search-min" => search_min = val("--search-min")?.parse()?,
            "--lead-s" => lead_s = val("--lead-s")?.parse()?,
            "--tail-s" => tail_s = val("--tail-s")?.parse()?,
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let out = out.context("usage: voxel_predation_capture OUT.mp4 [options]")?;
    // One thread, so pass two repeats pass one.
    cubarium_voxel::set_thread_override(1);
    let hz = u64::from(TICK_HZ);
    let (lead, tail) = ((lead_s * hz as f64) as u64, (tail_s * hz as f64) as u64);

    // Pass one: find the events.
    let mut sim = found(&preset, seed, introduce)?;
    let (mut ev, mut tick) = (Events::default(), 0u64);
    let limit = search_min * 60 * hz;
    while tick < limit {
        sim.step();
        tick += 1;
        note(&sim, tick, &mut ev);
        if let Some(e) = ev.eat {
            if tick >= e + tail || ev.rest.is_some_and(|r| tick >= r + 5 * hz) {
                break;
            }
        }
    }
    eprintln!(
        "pass one, {:.1} min simulated: first strike {:?}, first kill {:?}, first carrion eaten {:?}, first satiated rest after it {:?} (ticks)",
        tick as f64 / (60 * hz) as f64,
        ev.strike,
        ev.kill,
        ev.eat,
        ev.rest
    );
    let (first, last, what) = match (ev.strike, ev.kill, ev.eat) {
        (s, Some(k), e) => (
            s.unwrap_or(k).min(k).saturating_sub(lead),
            ev.rest.map_or(e.unwrap_or(k) + tail, |r| r + 5 * hz),
            "strike, kill and carcass",
        ),
        (Some(s), None, _) => (s.saturating_sub(lead), s + tail, "a strike (no kill within the search)"),
        _ => (0, (60 * hz).min(limit), "waiting only (no strike within the search)"),
    };
    // No clip longer than about a minute.
    let last = last.min(first + 60 * hz);
    eprintln!("drawing ticks {first}..={last}: {what}");

    // Pass two: draw.
    let mut sim = found(&preset, seed, introduce)?;
    let world_cfg = sim.world().config().clone();
    let cfg = VoxelConfig {
        world: world_cfg.clone(),
        raster_height: 0,
        px_per_voxel: px,
        lighting: Lighting::Flat,
        organisms: OrganismLook::Models,
        ..VoxelConfig::default()
    };
    let models = Arc::new(ModelLibrary::load(&cfg.models_dir, world_cfg.voxel_m).context("the baked models")?);
    let proj = Projection::new(cfg.tilt_degrees, px, 0, &world_cfg)?;
    let mut sink = VoxelGpuSink::new(&cfg, proj, VoxelGpuSinkOptions { models: Some(models), ..Default::default() })?;
    let (rw, rh) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
    let mut ff = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24"])
        .args(["-s", &format!("{rw}x{rh}"), "-framerate", "60", "-i", "-"])
        .args(["-vf", &format!("scale=iw*{scale}:ih*{scale}:flags=neighbor")])
        .args(["-c:v", "libx264", "-preset", "slow", "-crf", "12", "-pix_fmt", "yuv444p"])
        .arg(&out)
        .stdin(Stdio::piped())
        .spawn()
        .context("ffmpeg")?;
    let mut pipe = ff.stdin.take().context("ffmpeg stdin")?;
    let mut frames = 0u32;
    for t in 1..=last {
        sim.step();
        if t < first {
            continue;
        }
        for k in 0..3 {
            if k == 0 {
                anyhow::ensure!(
                    sink.stage_view(&sim.world().view(), sim.flora().view(), sim.fauna().view()),
                    "no staging buffer"
                );
            }
            sink.set_clock(t, f64::from(k) / 3.0);
            sink.render()?;
            let rgba = sink.read_raster()?;
            let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
            pipe.write_all(&rgb)?;
            frames += 1;
        }
    }
    drop(pipe);
    anyhow::ensure!(ff.wait()?.success(), "ffmpeg failed");
    let mut check = Events::default();
    note(&sim, last, &mut check);
    eprintln!("pass two reached tick {last}: strike {:?}, kill {:?}, eat {:?}", check.strike.is_some(), check.kill.is_some(), check.eat.is_some());
    println!("{frames} frames, {rw}x{rh} x{scale}, {:.1} s: {}", f64::from(frames) / 60.0, out.display());
    Ok(())
}
