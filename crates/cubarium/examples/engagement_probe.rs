//! Thin CLI for the reusable ENGAGE harness; renderer follows the existing headless
//! capture path and design/art-direction/Cubarium_Art_Direction_v0.1.md.
use anyhow::{Context, Result, ensure};
use cubarium::sink::gpu::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::project::Projection;
use cubarium_voxel_sim::engagement::{self, Config};
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn film(
    mut episode: engagement::Episode,
    path: &std::path::Path,
    seconds: f64,
    fps: f64,
) -> Result<()> {
    let mut cfg = cubarium::voxel::load_config(std::path::Path::new("config/desktop/voxel.toml"))?;
    cfg.sim.world = episode.sim.world().config().clone();
    let projection = Projection::new(
        cfg.tilt_degrees,
        cfg.px_per_voxel,
        cfg.raster_height,
        &cfg.sim.world,
    )?;
    let models = cubarium::voxel::load_models(&cfg);
    let mut sink = VoxelGpuSink::new(
        &cfg,
        projection,
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            models,
            ..Default::default()
        },
    )?;
    let film0 = Instant::now();
    let bake0 = Instant::now();
    loop {
        ensure!(
            sink.stage_world(
                episode.sim.world(),
                episode.sim.flora(),
                episode.sim.fauna()
            ),
            "headless pack refused"
        );
        if !sink.light_pending() {
            break;
        }
        ensure!(
            bake0.elapsed().as_secs() < 60,
            "headless light bake timed out"
        );
        std::thread::yield_now();
    }
    let p = sink.params();
    let (w, h) = (p.raster_w, p.raster_h);
    let mut encoder = Command::new("ffmpeg")
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
        .arg("-r")
        .arg(fps.to_string())
        .args([
            "-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv444p", "-crf", "12", "-preset", "fast",
        ])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .context("starting ffmpeg")?;
    let mut input = encoder.stdin.take().context("encoder input")?;
    for frame in 0..(seconds * fps).ceil() as u64 {
        let t = frame as f64 / fps * f64::from(cubarium_voxel_fauna::TICK_HZ);
        while episode.sim.world().tick() < t.floor() as u64 {
            // Preserve the same episode termination and readout as the numerical run.
            if !episode.step() {
                break;
            }
            ensure!(
                sink.stage_world(
                    episode.sim.world(),
                    episode.sim.flora(),
                    episode.sim.fauna()
                ),
                "headless pack refused"
            );
        }
        sink.set_presenter_clock(Some(film0 + Duration::from_secs_f64(frame as f64 / fps)));
        sink.set_clock(
            episode.sim.world().tick(),
            if episode.done() { 0.0 } else { t.fract() },
        );
        sink.render()?;
        input.write_all(&sink.read_raster()?)?;
        if episode.done() {
            break;
        }
    }
    drop(input);
    ensure!(encoder.wait()?.success(), "ffmpeg failed");
    println!("{}", serde_json::to_string(&episode.row())?);
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |key: &str| args.iter().find_map(|a| a.strip_prefix(key));
    let mut cfg = Config::read(&PathBuf::from(
        get("config=").unwrap_or("config/engagement.toml"),
    ))
    .map_err(anyhow::Error::msg)?;
    if let Some(s) = get("seconds=") {
        cfg.duration_s = s.parse()?;
    }
    if let Some(s) = get("episodes=") {
        cfg.episodes = s.parse()?;
    }
    if args.iter().any(|a| a == "usual-prey") {
        cfg.usual_prey = true;
    }
    cfg.validate().map_err(anyhow::Error::msg)?;
    let seed: u64 = get("seed=").unwrap_or("1").parse()?;
    let selected = get("arm=");
    let arms: Vec<_> = cfg
        .arms
        .iter()
        .filter(|a| selected.is_none_or(|s| a.name == s))
        .collect();
    ensure!(!arms.is_empty(), "unknown engagement arm");
    if let Some(path) = get("video=") {
        ensure!(arms.len() == 1, "video needs one arm");
        let fps: f64 = get("fps=").unwrap_or("30").parse()?;
        ensure!(fps.is_finite() && fps > 0.0, "positive fps required");
        let episode = engagement::found(&cfg, seed, arms[0]).map_err(anyhow::Error::msg)?;
        return film(episode, std::path::Path::new(path), cfg.duration_s, fps);
    }
    for offset in 0..cfg.episodes {
        for arm in &arms {
            let row = engagement::run(
                &cfg,
                seed.checked_add(offset as u64).context("seed overflow")?,
                arm,
            )
            .map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string(&row)?);
        }
    }
    Ok(())
}
