//! `voxel_foliage_fade_capture`: an adult bloomcrown and an adult umbrellafrond whose total
//! foliage is scripted down through their foliage states and back, headless, for judging
//! the cross-fade (`design/handoffs/foliage-states-implementation-2026-10-08.md`).
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_foliage_fade_capture -- OUT_DIR [--px N]
//! ```
//!
//! Writes `OUT_DIR/frames/f-0000.png …` at 60 frames a second of real time, each frame
//! two pictures side by side: **before** (left: the species' thresholds emptied, so the
//! models thin layer by layer as they did) and **after** (right: the foliage states). The
//! script, in total foliage over capacity, every layer alike: full for 1 s, 0.6 (bloomcrown
//! browsed, umbrellafrond browsed A), 0.4 (browsed, browsed B), 0.1 (both stripped), then
//! straight back to full, each held 2.5 s. The presenter's clock is the frame's time, so
//! the fades run at `VoxelConfig::foliage_fade_s`. Nothing is simulated. It prints the
//! ffmpeg line for the MP4. No window is opened.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{OrganismLook, VoxelConfig};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, FloraView, Species};

const VOXEL_M: f64 = 0.125;
/// `(from s, total foliage fraction)`, held until the next entry.
const SCRIPT: [(f64, f64); 5] = [(0.0, 1.0), (1.0, 0.6), (3.5, 0.4), (6.0, 0.1), (8.5, 1.0)];
const LENGTH_S: f64 = 11.5;
const FPS: u32 = 60;

fn world() -> World {
    let cfg = Config {
        width: 30,
        height: 24,
        depth: 15,
        voxel_m: VOXEL_M,
        ..Config::default()
    };
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

fn fraction_at(t: f64) -> f64 {
    SCRIPT.iter().rev().find(|(from, _)| t >= *from).map_or(1.0, |&(_, f)| f)
}

fn main() -> Result<()> {
    let mut out = None;
    let mut px = 8u32;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--px" => px = args.next().context("--px N")?.parse()?,
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let out = out.context("usage: voxel_foliage_fade_capture OUT_DIR [--px N]")?;
    let frames = out.join("frames");
    std::fs::create_dir_all(&frames)?;

    let w = world();
    let after_cfg = FloraConfig::default();
    let mut before_cfg = after_cfg.clone();
    for sp in [Species::Bloomcrown, Species::Umbrellafrond] {
        before_cfg.species_mut(sp).foliage_state_below.clear();
    }
    let mut flora = Flora::new(after_cfg.clone());
    for (x, sp) in [(8, Species::Bloomcrown), (20, Species::Umbrellafrond)] {
        let wood = after_cfg.species(sp).wood_max;
        anyhow::ensure!(
            flora.apply(&w, FloraCommand::Seed { x, z: 7, species: sp, wood }),
            "could not seed {sp:?}"
        );
    }
    let fauna = Fauna::new(FaunaConfig::default());

    let world_cfg = w.config().clone();
    let cfg = VoxelConfig {
        sim: cubarium_session::SimParams { world: world_cfg.clone(), ..Default::default() },
        raster_height: 0,
        px_per_voxel: px,
        organisms: OrganismLook::Models,
        textures: false,
        ..VoxelConfig::default()
    };
    let models = Arc::new(ModelLibrary::load(&cfg.models_dir, VOXEL_M).context("the baked models")?);
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
    let mut sinks = [new_sink()?, new_sink()?];
    let t0 = Instant::now();
    let n = (LENGTH_S * f64::from(FPS)).round() as u32;
    for frame in 0..n {
        let t = f64::from(frame) / f64::from(FPS);
        let f = fraction_at(t);
        let mut stands = flora.view().stands.to_vec();
        for s in &mut stands {
            let sc = after_cfg.species(s.species);
            s.moisture = 1.0;
            let want = f * sc.alpha * s.wood;
            let scale = if s.foliage > 0.0 { want / s.foliage } else { 0.0 };
            for v in &mut s.layer_stock {
                *v *= scale;
            }
            s.foliage = want;
        }
        let mut halves = Vec::new();
        for (sink, fc) in sinks.iter_mut().zip([&before_cfg, &after_cfg]) {
            let view = FloraView {
                stands: &stands,
                config: fc,
                crowns: cubarium_voxel_flora::CrownCache::none(),
                ..flora.view()
            };
            sink.set_presenter_clock(Some(t0 + Duration::from_secs_f64(t)));
            let mut wv = w.view();
            wv.tick = u64::from(frame) + 1;
            anyhow::ensure!(sink.stage_view(&wv, view, fauna.view()), "no staging buffer");
            sink.set_clock(u64::from(frame) + 1, 0.0);
            sink.render()?;
            halves.push(sink.read_raster()?);
        }
        // Side by side, a 4 px gutter.
        let gutter = 4u32;
        let ow = rw * 2 + gutter;
        let mut rgb = vec![0u8; (ow * rh * 3) as usize];
        for y in 0..rh {
            for (k, half) in halves.iter().enumerate() {
                for x in 0..rw {
                    let s = ((y * rw + x) * 4) as usize;
                    let d = ((y * ow + x + k as u32 * (rw + gutter)) * 3) as usize;
                    rgb[d..d + 3].copy_from_slice(&half[s..s + 3]);
                }
            }
        }
        write_png(&frames.join(format!("f-{frame:04}.png")), ow, rh, &rgb)?;
    }
    println!(
        "{}x{rh}; {n} frames: ffmpeg -y -framerate {FPS} -i {}/f-%04d.png \
         -vf scale=iw*2:ih*2:flags=neighbor -c:v libx264 -preset slow -crf 12 -pix_fmt yuv444p OUT.mp4",
        rw * 2 + 4,
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
