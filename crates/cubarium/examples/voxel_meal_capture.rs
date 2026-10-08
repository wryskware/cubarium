//! `voxel_meal_capture`: two grown frondgrazers side-on, headless, eating in bouts, for
//! judging the meal flash (`VoxelConfig::meal_flash`, `cubarium::voxel::animal`).
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_meal_capture -- OUT_DIR [--px N] [--lit]
//! ```
//!
//! Writes `OUT_DIR/frames/f-0000.png …` at 60 frames a second of world time. Both
//! frondgrazers crop on the same script — a long bout, a pause, a quick nibble, a short
//! pause, another bout, then rest — so the flash's onset, hold and release play twice
//! over. The state is forced on a copy of the view (`State::Cropping`); nothing is
//! simulated. It prints the ffmpeg line for the MP4. No window is opened.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{Lighting, OrganismLook, VoxelConfig};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{
    Command as FaunaCommand, Fauna, FaunaConfig, FaunaView, Founder, StartingStores, State,
};

/// The cropping script, in seconds from the start: `(from, to)` spans the bodies crop.
const BOUTS: [(f64, f64); 3] = [(1.0, 3.0), (4.0, 4.3), (4.8, 6.0)];
/// Seconds the capture runs.
const LENGTH_S: f64 = 7.5;
use cubarium_voxel_flora::{Flora, FloraConfig};

const VOXEL_M: f64 = 0.125;

fn world() -> World {
    let cfg = Config {
        width: 22,
        height: 8,
        depth: 5,
        voxel_m: VOXEL_M,
        ..Config::default()
    };
    let mut w = World::empty(cfg.clone());
    for z in 0..cfg.depth {
        for x in 0..i64::from(cfg.width) {
            for y in 0..2 {
                let material = if y == 0 {
                    Material::Bedrock
                } else {
                    Material::Soil
                };
                w.apply(VoxelCommand::SetMaterial { x, y, z, material });
            }
        }
    }
    w
}

fn main() -> Result<()> {
    let mut out = None;
    let mut px = 12u32;
    let mut lit = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--lit" => lit = true,
            "--px" => px = args.next().context("--px N")?.parse()?,
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let out = out.context("usage: voxel_meal_capture OUT_DIR [--px N]")?;
    let frames = out.join("frames");
    std::fs::create_dir_all(&frames)?;

    let w = world();
    let mut fauna = Fauna::new(FaunaConfig::default());
    for x in [6, 15] {
        anyhow::ensure!(
            fauna.apply(
                &w,
                FaunaCommand::IntroduceFounder {
                    x,
                    z: 2,
                    founder: Founder::Browser,
                    stores: StartingStores::FULL,
                    heading_rad: std::f64::consts::FRAC_PI_2,
                }
            ),
            "could not place a frondgrazer at x {x}"
        );
    }
    let flora = Flora::new(FloraConfig::default());
    let fv = fauna.view();
    let adult = fv.config.founder(Founder::Browser).core.body_max;
    let mut animals = fv.animals.to_vec();
    for a in &mut animals {
        a.body = adult;
        a.state = State::Resting;
    }

    let world_cfg = w.config().clone();
    let cfg = VoxelConfig {
        world: world_cfg.clone(),
        raster_height: 0,
        px_per_voxel: px,
        lighting: if lit { Lighting::Lit } else { Lighting::Flat },
        organisms: OrganismLook::Models,
        ..VoxelConfig::default()
    };
    let models =
        Arc::new(ModelLibrary::load(&cfg.models_dir, VOXEL_M).context("the baked models")?);
    let proj = Projection::new(cfg.tilt_degrees, px, 0, &world_cfg)?;
    let (rw, rh) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
    let new_sink = || {
        VoxelGpuSink::new(
            &cfg,
            proj,
            VoxelGpuSinkOptions {
                models: Some(models.clone()),
                ..Default::default()
            },
        )
    };
    let draw = |sink: &mut VoxelGpuSink,
                animals: &[cubarium_voxel_fauna::Animal],
                tick: u64,
                k: u32|
     -> Result<Vec<u8>> {
        if k == 0 {
            let mut view = w.view();
            view.tick = tick;
            let fview = FaunaView { animals, ..fv };
            anyhow::ensure!(
                sink.stage_view(&view, flora.view(), fview),
                "no staging buffer"
            );
        }
        sink.set_clock(tick, f64::from(k) / 3.0);
        sink.render()?;
        let rgba = sink.read_raster()?;
        Ok(rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect())
    };

    let mut sink = new_sink()?;
    let last = (LENGTH_S * 20.0).round() as u64;
    let mut frame = 0u32;
    for tick in 1..=last {
        let t = tick as f64 / 20.0;
        let eating = BOUTS.iter().any(|&(from, to)| t >= from && t < to);
        for a in &mut animals {
            a.state = if eating { State::Cropping } else { State::Resting };
        }
        for k in 0..3 {
            let rgb = draw(&mut sink, &animals, tick, k)?;
            write_png(&frames.join(format!("f-{frame:04}.png")), rw, rh, &rgb)?;
            frame += 1;
        }
    }
    println!(
        "{}x{rh}; {frame} frames: ffmpeg -y -framerate 60 -i {}/f-%04d.png \
         -vf scale=iw*3:ih*3:flags=neighbor -c:v libx264 -preset slow -crf 12 -pix_fmt yuv444p OUT.mp4",
        rw,
        frames.display()
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
