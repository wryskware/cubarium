//! `plant_grow_capture`: plants arriving, growing a step, changing foliage state and
//! leaving, drawn stamped (left: the stamp's cell-by-cell cross-fade) and as models (right:
//! the plant pass's staggered per-voxel grow-in, swaying), headless, side by side.
//!
//! ```text
//! cargo run -p cubarium --release --example plant_grow_capture -- OUT.mp4 [--slow K] [--px N] [--scale N]
//! ```
//!
//! The script, in sim seconds: an adult bloomcrown and an adult umbrellafrond, full; at
//! 1 s a young lanternberry appears; at 3.5 s the two adults are browsed to 0.4 of their
//! foliage (a foliage state change); at 5.5 s the lanternberry grows to full wood (a model
//! step); at 7.5 s the adults are back to full; at 9.5 s the lanternberry goes; 12 s long.
//! The sim clock runs at 20 ticks a second of sim time and the video at 60 frames a second;
//! `--slow K` draws K times as many frames for the same sim time (`--slow 4`: 0.25x).
//! Lit tier, the default `[plants]` and `foliage_fade_s`. Writes x264 yuv444p crf 12.
//! Nothing is simulated; no window is opened.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::figure::PlantDraw;
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{Lighting, OrganismLook, VoxelConfig};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, FloraView, Species};

const VOXEL_M: f64 = 0.125;
const LENGTH_S: f64 = 12.0;
const FPS: u32 = 60;

fn world() -> World {
    let cfg = Config { width: 30, height: 24, depth: 15, voxel_m: VOXEL_M, ..Config::default() };
    let mut w = World::empty(cfg.clone());
    for z in 0..cfg.depth {
        for x in 0..i64::from(cfg.width) {
            for y in 0..2 {
                let material = if y == 0 { Material::Bedrock } else { Material::Soil };
                w.apply(VoxelCommand::SetMaterial { x, y, z, material });
            }
        }
    }
    w
}

/// The adults' total foliage fraction at `t`.
fn foliage_at(t: f64) -> f64 {
    if (3.5..7.5).contains(&t) { 0.4 } else { 1.0 }
}

fn main() -> Result<()> {
    let (mut out, mut slow, mut px, mut scale) = (None, 1u32, 8u32, 3u32);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--slow" => slow = args.next().context("--slow K")?.parse()?,
            "--px" => px = args.next().context("--px N")?.parse()?,
            "--scale" => scale = args.next().context("--scale N")?.parse()?,
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let out = out.context("usage: plant_grow_capture OUT.mp4 [--slow K] [--px N] [--scale N]")?;
    anyhow::ensure!(slow > 0 && scale > 0, "--slow and --scale are positive");

    let w = world();
    let fc = FloraConfig::default();
    let mut flora = Flora::new(fc.clone());
    for (x, sp, wood) in [
        (7, Species::Bloomcrown, 1.0),
        (22, Species::Umbrellafrond, 1.0),
        (15, Species::Lanternberry, 0.35),
    ] {
        let wood = wood * fc.species(sp).wood_max;
        anyhow::ensure!(flora.apply(&w, FloraCommand::Seed { x, z: 7, species: sp, wood }), "could not seed {sp:?}");
    }
    let fauna = Fauna::new(FaunaConfig::default());
    let world_cfg = w.config().clone();
    let base = VoxelConfig {
        sim: cubarium_session::SimParams { world: world_cfg.clone(), ..Default::default() },
        raster_height: 0,
        px_per_voxel: px,
        organisms: OrganismLook::Models,
        textures: false,
        lighting: Lighting::Lit,
        ..VoxelConfig::default()
    };
    let models = Arc::new(ModelLibrary::load(&base.models_dir, VOXEL_M).context("the baked models")?);
    let proj = Projection::new(base.tilt_degrees, px, 0, &world_cfg)?;
    let (rw, rh) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
    let mut sinks = Vec::new();
    for plants in [PlantDraw::Stamped, PlantDraw::Models] {
        let mut cfg = base.clone();
        cfg.figures.plants = plants;
        sinks.push(VoxelGpuSink::new(
            &cfg,
            proj,
            VoxelGpuSinkOptions { models: Some(models.clone()), ..Default::default() },
        )?);
    }

    let gutter = 4u32;
    let (ow, oh) = (rw * 2 + gutter, rh);
    let mut ffmpeg = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s"])
        .arg(format!("{ow}x{oh}"))
        .args(["-r", &FPS.to_string(), "-i", "-", "-vf"])
        .arg(format!(
            "scale=iw*{scale}:ih*{scale}:flags=neighbor,drawtext=text='stamped':x=8:y=6:fontsize=16:fontcolor=white,\
             drawtext=text='models':x={}:y=6:fontsize=16:fontcolor=white,\
             drawtext=text='%{{eif\\:t/{slow}\\:d}}.%{{eif\\:mod(t*10/{slow}\\,10)\\:d}} s sim':x=8:y=h-24:fontsize=14:fontcolor=white",
            (rw + gutter) * scale + 8
        ))
        .args(["-c:v", "libx264", "-pix_fmt", "yuv444p", "-crf", "12", "-preset", "slow"])
        .arg(&out)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("ffmpeg")?;
    let mut pipe = ffmpeg.stdin.take().context("ffmpeg's stdin")?;

    let t0 = Instant::now();
    let mut staged_tick = None;
    let n = (LENGTH_S * f64::from(FPS * slow)).round() as u32;
    for frame in 0..n {
        let t = f64::from(frame) / f64::from(FPS * slow);
        let ticks = t * f64::from(cubarium_voxel::TICK_HZ);
        let tick = ticks.floor() as u64 + 1;
        let mut stands = Vec::new();
        for s in flora.view().stands {
            let mut s = *s;
            let sc = fc.species(s.species);
            s.moisture = 1.0;
            let f = if s.species == Species::Lanternberry {
                if !(1.0..9.5).contains(&t) {
                    continue;
                }
                if t >= 5.5 {
                    s.wood = sc.wood_max;
                }
                1.0
            } else {
                foliage_at(t)
            };
            let want = f * sc.alpha * s.wood;
            let k = if s.foliage > 0.0 { want / s.foliage } else { 0.0 };
            for v in &mut s.layer_stock {
                *v *= k;
            }
            s.foliage = want;
            stands.push(s);
        }
        let mut halves = Vec::new();
        for sink in &mut sinks {
            sink.set_presenter_clock(Some(t0 + Duration::from_secs_f64(t)));
            sink.set_clock(tick, ticks.fract());
            if staged_tick != Some(tick) || frame == 0 {
                let view = FloraView { stands: &stands, crowns: cubarium_voxel_flora::CrownCache::none(), ..flora.view() };
                let mut wv = w.view();
                wv.tick = tick;
                let started = Instant::now();
                loop {
                    let staged = sink.stage_view(&wv, view, fauna.view());
                    sink.render()?;
                    if staged && !sink.light_pending() {
                        break;
                    }
                    if started.elapsed().as_secs() > 60 {
                        bail!("the sky plane never arrived");
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
            } else {
                sink.render()?;
            }
            halves.push(sink.read_raster()?);
        }
        staged_tick = Some(tick);
        let mut rgb = vec![0u8; (ow * oh * 3) as usize];
        for y in 0..rh {
            for (k, half) in halves.iter().enumerate() {
                for x in 0..rw {
                    let s = ((y * rw + x) * 4) as usize;
                    let d = ((y * ow + x + k as u32 * (rw + gutter)) * 3) as usize;
                    rgb[d..d + 3].copy_from_slice(&half[s..s + 3]);
                }
            }
        }
        pipe.write_all(&rgb)?;
    }
    drop(pipe);
    anyhow::ensure!(ffmpeg.wait()?.success(), "ffmpeg failed");
    println!("{} frames, {}x{} -> {}", n, ow * scale, oh * scale, out.display());
    Ok(())
}
