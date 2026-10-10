//! `voxel_rider_capture`: a lanternjaw larva riding a grown frondgrazer that walks, stops
//! and turns, then lets go and stands as a free juvenile while the host walks off;
//! headless, for judging the rider's model (`figure::specs`' rider branch) and the
//! drop-off's model-swap fade.
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_rider_capture -- OUT_DIR [--px N] [--lit]
//! ```
//!
//! Writes `OUT_DIR/frames/f-0000.png …` at 60 frames a second of world time. Nothing is
//! simulated: the two bodies are introduced, the lanternjaw is made a rider of the host
//! (`Animal::attached`, as the hatching rule leaves it) and both are scripted on a copy of
//! the view; the drop-off is the fauna's release (the rider takes the host's place and
//! pose). It prints the ffmpeg line for the MP4. No window is opened.

use std::f64::consts::{FRAC_PI_2, PI};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{Lighting, OrganismLook, VoxelConfig};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{
    Animal, Attachment, Command as FaunaCommand, Fauna, FaunaConfig, FaunaView, Founder,
    StartingStores, State,
};
use cubarium_voxel_flora::{Flora, FloraConfig};

const VOXEL_M: f64 = 0.125;
/// Seconds the capture runs.
const LENGTH_S: f64 = 13.0;
/// The host's walking pace, m/s.
const PACE: f64 = 0.4;
/// When the rider lets go, seconds.
const DROP_S: f64 = 9.0;

fn world() -> World {
    let cfg = Config { width: 30, height: 8, depth: 14, voxel_m: VOXEL_M, ..Config::default() };
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

/// The host's script at `t` seconds: (moving, heading). Stand, walk +x, stop, turn a
/// quarter toward +z, walk, stop; after the drop-off stand a second, then turn back and
/// walk off along -x.
fn host_script(t: f64) -> (bool, f64) {
    let ease = |a: f64, b: f64, from: f64, to: f64| {
        let u = ((t - from) / (to - from)).clamp(0.0, 1.0);
        a + (b - a) * (0.5 - 0.5 * (PI * u).cos())
    };
    match t {
        t if t < 1.0 => (false, FRAC_PI_2),
        t if t < 3.5 => (true, FRAC_PI_2),
        t if t < 4.5 => (false, FRAC_PI_2),
        t if t < 6.0 => (false, ease(FRAC_PI_2, 0.0, 4.5, 6.0)),
        t if t < 7.5 => (true, 0.0),
        t if t < DROP_S + 1.0 => (false, 0.0),
        t if t < DROP_S + 2.5 => (false, ease(0.0, -FRAC_PI_2, DROP_S + 1.0, DROP_S + 2.5)),
        _ => (true, -FRAC_PI_2),
    }
}

fn place(a: &mut Animal, x_m: f64, z_m: f64, heading: f64) {
    a.pose.x = x_m;
    a.pose.z = z_m;
    a.pose.heading_rad = heading;
    a.site.x = (x_m / VOXEL_M).floor() as _;
    a.site.z = (z_m / VOXEL_M).floor() as _;
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
    let out = out.context("usage: voxel_rider_capture OUT_DIR [--px N] [--lit]")?;
    let frames = out.join("frames");
    std::fs::create_dir_all(&frames)?;

    let w = world();
    let mut fauna = Fauna::new(FaunaConfig::default());
    for (x, founder) in [(7, Founder::Browser), (22, Founder::Lanternjaw)] {
        anyhow::ensure!(
            fauna.apply(
                &w,
                FaunaCommand::IntroduceFounder {
                    x,
                    z: 4,
                    founder,
                    stores: StartingStores::FULL,
                    heading_rad: FRAC_PI_2,
                }
            ),
            "could not place a {founder:?} at x {x}"
        );
    }
    let flora = Flora::new(FloraConfig::default());
    let fv = fauna.view();
    let mut animals = fv.animals.to_vec();
    animals.sort_by_key(|a| a.id);
    let hi = animals.iter().position(|a| a.founder == Some(Founder::Browser)).unwrap();
    let ri = animals.iter().position(|a| a.founder == Some(Founder::Lanternjaw)).unwrap();
    animals[hi].body = fv.config.founder(Founder::Browser).core.body_max;
    animals[ri].body = fv.config.founder(Founder::Lanternjaw).core.body_min;
    let host_id = animals[hi].id;
    animals[ri].attached = Some(Attachment { host: host_id });
    let (mut hx, mut hz) = (6.0 * VOXEL_M, 4.5 * VOXEL_M);
    for a in &mut animals {
        a.state = State::Resting;
        place(a, hx, hz, FRAC_PI_2);
    }

    let world_cfg = w.config().clone();
    let cfg = VoxelConfig {
        sim: cubarium_session::SimParams { world: world_cfg.clone(), ..Default::default() },
        raster_height: 0,
        px_per_voxel: px,
        lighting: if lit { Lighting::Lit } else { Lighting::Flat },
        organisms: OrganismLook::Models,
        ..VoxelConfig::default()
    };
    let models = Arc::new(ModelLibrary::load(&cfg.models_dir, VOXEL_M).context("the baked models")?);
    anyhow::ensure!(models.specimen("lanternjaw-larva").is_some(), "no baked lanternjaw-larva");
    let proj = Projection::new(cfg.tilt_degrees, px, 0, &world_cfg)?;
    let (rw, rh) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
    let mut sink = VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions { models: Some(models.clone()), ..Default::default() },
    )?;

    let last = (LENGTH_S * 20.0).round() as u64;
    let mut frame = 0u32;
    let mut dropped = false;
    for tick in 1..=last {
        let t = tick as f64 / 20.0;
        let (moving, heading) = host_script(t);
        if moving {
            hx += heading.sin() * PACE / 20.0;
            hz += heading.cos() * PACE / 20.0;
        }
        let host = &mut animals[hi];
        host.state = if moving { State::Walking } else { State::Resting };
        place(host, hx, hz, heading);
        let host = animals[hi];
        let rider = &mut animals[ri];
        if !dropped && t >= DROP_S {
            // the fauna's release: a free juvenile where the host stands
            rider.attached = None;
            rider.site = host.site;
            rider.pose = host.pose;
            dropped = true;
        } else if !dropped {
            rider.site = host.site;
            rider.pose = host.pose;
        }
        for k in 0..3 {
            if k == 0 {
                let mut view = w.view();
                view.tick = tick;
                let fview = FaunaView { animals: &animals, ..fv };
                anyhow::ensure!(sink.stage_view(&view, flora.view(), fview), "no staging buffer");
            }
            sink.set_clock(tick, f64::from(k) / 3.0);
            sink.render()?;
            let rgb: Vec<u8> = sink.read_raster()?.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
            write_png(&frames.join(format!("f-{frame:04}.png")), rw, rh, &rgb)?;
            frame += 1;
        }
    }
    println!(
        "{rw}x{rh}; {frame} frames; drop-off at {DROP_S} s = frame {}: ffmpeg -y -framerate 60 -i {}/f-%04d.png \
         -vf scale=iw*3:ih*3:flags=neighbor -c:v libx264 -preset slow -crf 12 -pix_fmt yuv444p OUT.mp4",
        (DROP_S * 60.0) as u32,
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
