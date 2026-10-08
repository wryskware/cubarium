//! `voxel_hunker_capture`: two grown frondgrazers side-on, headless, for judging the hunker
//! pose (HUNKER, `cubarium::voxel::hunker`).
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_hunker_capture -- OUT_DIR [--px N] [--lit]
//!     [--hunker-s SECONDS] [--lead-s SECONDS] [--hold-s SECONDS]
//! ```
//!
//! Writes `OUT_DIR/still.png`: the left one standing, the right one hunkered (fully down).
//! Then `OUT_DIR/frames/f-0000.png …` at 60 frames a second of world time: the left one
//! stands for `--lead-s` (default 1.5), hunkers and holds down for `--hold-s` (default 2);
//! the right one is seen hunkered from the start and comes up at the same moment the left
//! goes down, so the down and up moves play side by side. It prints the ffmpeg line for
//! the MP4. The hunker is forced (`Animal::hunkered` set on a copy of the view); nothing
//! is simulated, so nothing else moves but the live idle. No window is opened.

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
    let mut hunker_s = cubarium::voxel::hunker::DEFAULT_S;
    let mut lead_s = 1.5f64;
    let mut hold_s = 2.0f64;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--lit" => lit = true,
            "--px" => px = args.next().context("--px N")?.parse()?,
            "--hunker-s" => hunker_s = args.next().context("--hunker-s S")?.parse()?,
            "--lead-s" => lead_s = args.next().context("--lead-s S")?.parse()?,
            "--hold-s" => hold_s = args.next().context("--hold-s S")?.parse()?,
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let out = out.context("usage: voxel_hunker_capture OUT_DIR [--px N]")?;
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
        sim: cubarium_session::SimParams { world: world_cfg.clone(), ..Default::default() },
        raster_height: 0,
        px_per_voxel: px,
        lighting: if lit { Lighting::Lit } else { Lighting::Flat },
        organisms: OrganismLook::Models,
        hunker_s,
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

    // The still: left standing, right hunkered, both long settled.
    {
        let mut sink = new_sink()?;
        let mut still = animals.clone();
        still[1].hunkered = true;
        let mut rgb = Vec::new();
        for tick in 1..=40u64 {
            rgb = draw(&mut sink, &still, tick, 0)?;
        }
        write_png(&out.join("still.png"), rw, rh, &rgb)?;
    }

    // The move: left down at `lead`, right up at the same tick.
    let mut sink = new_sink()?;
    let lead = (lead_s * 20.0).round() as u64;
    let last = lead + ((hunker_s + hold_s) * 20.0).round() as u64;
    let mut frame = 0u32;
    for tick in 1..=last {
        animals[0].hunkered = tick >= lead;
        animals[1].hunkered = tick < lead;
        for k in 0..3 {
            let rgb = draw(&mut sink, &animals, tick, k)?;
            write_png(&frames.join(format!("f-{frame:04}.png")), rw, rh, &rgb)?;
            frame += 1;
        }
    }
    println!(
        "still {}x{rh} at {}; {frame} frames: ffmpeg -y -framerate 60 -i {}/f-%04d.png \
         -vf scale=iw*3:ih*3:flags=neighbor -c:v libx264 -preset slow -crf 12 -pix_fmt yuv444p OUT.mp4",
        rw,
        out.join("still.png").display(),
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
