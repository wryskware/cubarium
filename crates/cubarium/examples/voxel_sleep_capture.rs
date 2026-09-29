//! `voxel_sleep_capture`: one browser through a fast day, headless, for judging the sleep
//! pose (WX4): awake, falling asleep at dusk, breathing through the night, waking at dawn.
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_sleep_capture -- OUT_DIR [--glyphs] [--px N]
//!     [--day-s SECONDS] [--lead-s SECONDS]
//! ```
//!
//! Writes `OUT_DIR/f-0000.png …` at 60 frames a second of the world's own time (the tick is
//! 20 Hz, three frames a tick), from `--lead-s` (default 3) before the browser falls asleep
//! to `--lead-s` after it has woken, and prints the ffmpeg line. The sleep is the real
//! rule: a `[world.day]` clock of `--day-s` seconds (default 30) and the browser's own
//! diurnal rhythm, nothing forced. The GPU renderer is offscreen; no window is opened.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{Lighting, OrganismLook, VoxelConfig};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Founder, State, StartingStores};
use cubarium_voxel_flora::{Flora, FloraConfig};

const VOXEL_M: f64 = 0.125;

fn world(day_s: f64) -> World {
    let cfg = Config {
        width: 20,
        height: 12,
        depth: 6,
        voxel_m: VOXEL_M,
        day: DayClock {
            length_min: day_s / 60.0,
            // Late afternoon: dusk comes soon after the start.
            start_hour: 15.0,
            ..DayClock::default()
        },
        ..Config::default()
    };
    let mut w = World::empty(cfg.clone());
    for z in 0..cfg.depth {
        for x in 0..i64::from(cfg.width) {
            w.apply(VoxelCommand::SetMaterial {
                x,
                y: 0,
                z,
                material: Material::Bedrock,
            });
            for y in 1..3 {
                w.apply(VoxelCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

fn main() -> Result<()> {
    let mut out = None;
    let mut glyphs = false;
    let mut px = 14u32;
    let mut day_s = 30.0f64;
    let mut lead_s = 3.0f64;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--glyphs" => glyphs = true,
            "--px" => px = args.next().context("--px N")?.parse()?,
            "--day-s" => day_s = args.next().context("--day-s S")?.parse()?,
            "--lead-s" => lead_s = args.next().context("--lead-s S")?.parse()?,
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let out = out.context("usage: voxel_sleep_capture OUT_DIR [--glyphs] [--px N]")?;
    std::fs::create_dir_all(&out)?;

    let place = |world: &World| -> Result<Fauna> {
        let mut fauna = Fauna::new(FaunaConfig::default());
        anyhow::ensure!(
            fauna.apply(
                world,
                FaunaCommand::IntroduceFounder {
                    x: 10,
                    z: 3,
                    founder: Founder::Browser,
                    stores: StartingStores::FULL,
                    heading_rad: std::f64::consts::FRAC_PI_2,
                }
            ),
            "could not place the browser"
        );
        Ok(fauna)
    };
    let step = |world: &mut World, fauna: &mut Fauna, flora: &mut Flora| {
        world.step();
        fauna.step_with(world, flora, 1);
    };

    // Pass one, no drawing: the ticks it falls asleep and wakes at.
    let (mut fall, mut wake) = (None, None);
    {
        let mut w = world(day_s);
        let mut fauna = place(&w)?;
        let mut flora = Flora::new(FloraConfig::default());
        let mut asleep_before = false;
        for tick in 1..=(day_s * 20.0 * 2.0) as u64 {
            step(&mut w, &mut fauna, &mut flora);
            let asleep = fauna.view().animals[0].state == State::Asleep;
            if asleep && !asleep_before && fall.is_none() {
                fall = Some(tick);
            }
            if !asleep && asleep_before && fall.is_some() && wake.is_none() {
                wake = Some(tick);
            }
            asleep_before = asleep;
        }
    }
    let (fall, wake) = (
        fall.context("the browser never fell asleep")?,
        wake.context("the browser never woke")?,
    );
    let lead = (lead_s * 20.0) as u64;
    let (first, last) = (fall.saturating_sub(lead), wake + lead);
    eprintln!(
        "asleep from tick {fall} to {wake} ({:.1} s); drawing ticks {first}..={last}",
        (wake - fall) as f64 / 20.0
    );

    let mut w = world(day_s);
    let mut fauna = place(&w)?;
    let mut flora = Flora::new(FloraConfig::default());
    let world_cfg = w.config().clone();
    let cfg = VoxelConfig {
        world: world_cfg.clone(),
        raster_height: 0,
        px_per_voxel: px,
        lighting: Lighting::Flat,
        organisms: if glyphs {
            OrganismLook::Glyphs
        } else {
            OrganismLook::Models
        },
        ..VoxelConfig::default()
    };
    let models: Option<Arc<ModelLibrary>> = if glyphs {
        None
    } else {
        Some(Arc::new(
            ModelLibrary::load(&cfg.models_dir, VOXEL_M).context("the baked models")?,
        ))
    };
    let proj = Projection::new(cfg.tilt_degrees, px, 0, &world_cfg)?;
    let mut sink = VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions {
            models,
            ..Default::default()
        },
    )?;
    let (rw, rh) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
    let mut frame = 0u32;
    for tick in 1..=last {
        step(&mut w, &mut fauna, &mut flora);
        if tick < first {
            continue;
        }
        // Three frames a tick, so 60 a second.
        for k in 0..3 {
            if k == 0 {
                anyhow::ensure!(
                    sink.stage_view(&w.view(), flora.view(), fauna.view()),
                    "no staging buffer"
                );
            }
            sink.set_clock(tick, f64::from(k) / 3.0);
            sink.render()?;
            let rgba = sink.read_raster()?;
            let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
            write_png(&out.join(format!("f-{frame:04}.png")), rw, rh, &rgb)?;
            frame += 1;
        }
    }
    println!(
        "{frame} frames, {rw}x{rh}: ffmpeg -framerate 60 -i {}/f-%04d.png -vf scale=iw*3:ih*3:flags=neighbor \
         -c:v libx264 -preset slow -crf 12 -pix_fmt yuv444p OUT.mp4",
        out.display()
    );
    Ok(())
}

fn write_png(path: &std::path::Path, w: u32, h: u32, rgb: &[u8]) -> Result<()> {
    let file = std::fs::File::create(path).with_context(|| format!("{}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgb)?;
    Ok(())
}
