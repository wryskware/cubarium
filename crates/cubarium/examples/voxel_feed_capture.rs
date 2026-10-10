//! `voxel_feed_capture`: the FEED-RIG method trial (`design/handoffs/feeding-motion-rig-2026-10-09.md`):
//! two grown frondgrazers (or, `--founder chorister`, two adult Veilhounds: round 2) cropping
//! on one bout script, headless, lit, real time, in one arm:
//!
//! - `--arm a`: the **flipbook**, the clip re-voxelized at every sampled frame by the bake's
//!   own rule (`scripts/blender/feed_flipbook.py`, its JSON passed with `--flipbook`), drawn
//!   by swapping the model at 30 frames a second (`VoxelGpuSink::set_figure_cells`);
//! - `--arm b`: the bone rig, each cube turned with its bone (`[figures] rig = "turn"`);
//! - `--arm c`: the bone rig, each cube upright (`rig = "upright"`).
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_feed_capture -- OUT_DIR --arm b \
//!     [--px N] [--heading DEG] [--flipbook PATH] [--founder frondgrazer|chorister]
//! cargo run -p cubarium --release --example voxel_feed_capture -- OUT_DIR --perf N [--arm b]
//! ```
//!
//! Writes `OUT_DIR/frames/f-0000.png …` at 60 frames a second of world time. The heading is
//! degrees from +z toward +x (90: side-on, facing right). Both bodies crop on the same
//! script — a long bout, a pause, a short one, rest — and keep their own clip phases. The
//! state is forced on a copy of the view (`State::Cropping`); nothing is simulated. No
//! window is opened.
//!
//! `--perf N`: N grown frondgrazers all cropping in a wider world, no PNGs; prints the
//! frame's milliseconds (the clock's host work, and the render with its read-back) for the
//! arm and for the rig turned off, over the same frames.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::figure::RigDraw;
use cubarium::voxel::model::{ModelCell, ModelLibrary, Tag, palette_index};
use cubarium::voxel::project::Projection;
use cubarium::voxel::rig;
use cubarium::voxel::{Lighting, OrganismLook, VoxelConfig};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{
    Animal, Command as FaunaCommand, Fauna, FaunaConfig, FaunaView, Founder, StartingStores, State,
};
use cubarium_voxel_flora::{Flora, FloraConfig};

/// The cropping script, in seconds from the start: `(from, to)` spans the bodies crop.
const BOUTS: [(f64, f64); 2] = [(0.5, 4.5), (5.3, 6.0)];
/// Seconds the capture runs.
const LENGTH_S: f64 = 7.5;
const VOXEL_M: f64 = 0.125;
const HZ: f64 = cubarium_voxel::TICK_HZ as f64;

fn world(width: u32, depth: u32) -> World {
    let cfg = Config {
        width,
        height: 8,
        depth,
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

fn eating(tick: u64) -> bool {
    let t = tick as f64 / HZ;
    BOUTS.iter().any(|&(from, to)| t >= from && t < to)
}

/// The flipbook (`feed_flipbook.py`'s JSON): distinct frames, and per weight step `k` and
/// clip frame `j` the frame drawn.
struct Flipbook {
    frames: Vec<Arc<Vec<ModelCell>>>,
    grid: Vec<Vec<usize>>,
    fps: f64,
    levels: usize,
}

impl Flipbook {
    fn load(path: &std::path::Path) -> Result<Flipbook> {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let num = |k: &str| v[k].as_f64().with_context(|| format!("flipbook {k}"));
        let frames = v["frames"]
            .as_array()
            .context("flipbook frames")?
            .iter()
            .map(|f| {
                let cells = f.as_array().context("a frame")?.iter().map(|c| {
                    let c = c.as_array().context("a cell")?;
                    let i = |k: usize| c[k].as_i64().context("an offset").map(|v| v as i32);
                    let name = c[3].as_str().context("a material")?;
                    Ok(ModelCell {
                        offset: [i(0)?, i(1)?, i(2)?],
                        material: palette_index(name)
                            .with_context(|| format!("material {name}"))?,
                        tag: Tag::from_byte(c[4].as_u64().context("a tag")? as u8)
                            .context("a tag")?,
                    })
                });
                Ok(Arc::new(cells.collect::<Result<Vec<_>>>()?))
            })
            .collect::<Result<Vec<_>>>()?;
        let grid = v["grid"]
            .as_array()
            .context("flipbook grid")?
            .iter()
            .map(|row| {
                row.as_array().map(|r| {
                    r.iter()
                        .filter_map(|x| x.as_u64().map(|x| x as usize))
                        .collect()
                })
            })
            .collect::<Option<Vec<Vec<usize>>>>()
            .context("flipbook grid rows")?;
        Ok(Flipbook {
            frames,
            grid,
            fps: num("fps")?,
            levels: num("levels")? as usize,
        })
    }

    /// The frame drawn at clip weight `w` (linear, before the ease) and `phase_s` into the
    /// loop: the weight's nearest step, the loop's sampled frame at or before the phase.
    fn at(&self, w: f64, phase_s: f64) -> &Arc<Vec<ModelCell>> {
        let k = ((w * self.levels as f64).round() as usize).min(self.levels);
        let row = &self.grid[k];
        let j = ((phase_s * self.fps).floor() as i64).rem_euclid(row.len() as i64) as usize;
        &self.frames[row[j]]
    }
}

/// Each tick's clip weight and rate as `Animals::feed_weight` keeps them, for the bout
/// script: what the renderer extrapolates on the frame clock.
fn weights(last: u64) -> Vec<Option<(f64, f64)>> {
    let dt = 1.0 / HZ;
    let r = dt / rig::FEED_EASE_S;
    let mut out = vec![None; last as usize + 2];
    let mut entry: Option<(f32, bool)> = None;
    for tick in 1..=last + 1 {
        let feeding = eating(tick);
        let w = entry.map_or(0.0, |(w, was)| rig::advance_weight(w, was, dt));
        entry = (w > 0.0 || feeding).then_some((w, feeding));
        out[tick as usize] = entry.map(|(w, f)| {
            let w = f64::from(w);
            let rate = if f && w < 1.0 {
                r
            } else if !f && w > 0.0 {
                -r
            } else {
                0.0
            };
            (w, rate)
        });
    }
    out
}

/// The linear clip weight drawn at `td` ticks (the track draws one tick behind the clock):
/// the older tick's entry run on, else the newer's.
fn weight_at(ws: &[Option<(f64, f64)>], td: f64) -> f64 {
    let n = td.floor().max(0.0) as usize;
    match (
        ws.get(n).copied().flatten(),
        ws.get(n + 1).copied().flatten(),
    ) {
        (Some((w, r)), _) => (w + r * (td - n as f64)).clamp(0.0, 1.0),
        (None, Some((w, _))) => w,
        _ => 0.0,
    }
}

fn main() -> Result<()> {
    let mut out = None;
    let mut px = 6u32;
    let mut heading_deg = 90.0f64;
    let mut arm = "b".to_string();
    let mut flip_path = None;
    let mut perf = None;
    let mut founder = Founder::Browser;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--px" => px = args.next().context("--px N")?.parse()?,
            "--heading" => heading_deg = args.next().context("--heading DEG")?.parse()?,
            "--arm" => arm = args.next().context("--arm a|b|c")?,
            "--flipbook" => {
                flip_path = Some(PathBuf::from(args.next().context("--flipbook PATH")?))
            }
            "--perf" => perf = Some(args.next().context("--perf N")?.parse::<usize>()?),
            "--founder" => {
                founder = match args.next().context("--founder NAME")?.as_str() {
                    "frondgrazer" | "browser" => Founder::Browser,
                    "chorister" | "veilhound" => Founder::Chorister,
                    f => bail!("--founder frondgrazer or chorister, not {f}"),
                }
            }
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let rig_draw = match arm.as_str() {
        "a" => RigDraw::Off,
        "b" => RigDraw::Turn,
        "c" => RigDraw::Upright,
        a => bail!("--arm a, b or c, not {a}"),
    };
    let flip = match (&*arm, &flip_path) {
        ("a", Some(p)) => Some(Flipbook::load(p)?),
        ("a", None) => bail!("arm a needs --flipbook PATH (scripts/blender/feed_flipbook.py)"),
        _ => None,
    };
    if let Some(n) = perf {
        return run_perf(n, px, rig_draw, founder);
    }
    let out =
        out.context("usage: voxel_feed_capture OUT_DIR --arm a|b|c [--px N] [--heading DEG]")?;
    let frames = out.join("frames");
    std::fs::create_dir_all(&frames)?;

    // the Veilhound is 2.2 m with its tail, 18 voxels: a wider and deeper floor
    let (w, xs, z) = match founder {
        Founder::Chorister => (world(48, 16), [15, 36], 8),
        _ => (world(22, 7), [6, 15], 3),
    };
    let mut fauna = Fauna::new(FaunaConfig::default());
    for x in xs {
        anyhow::ensure!(
            fauna.apply(
                &w,
                FaunaCommand::IntroduceFounder {
                    x,
                    z,
                    founder,
                    stores: StartingStores::FULL,
                    heading_rad: heading_deg.to_radians(),
                }
            ),
            "could not place a {} at x {x}",
            founder.name()
        );
    }
    let flora = Flora::new(FloraConfig::default());
    let fv = fauna.view();
    let adult = fv.config.founder(founder).core.body_max;
    let mut animals = fv.animals.to_vec();
    for a in &mut animals {
        a.body = adult;
        a.state = State::Resting;
    }

    let world_cfg = w.config().clone();
    let mut cfg = VoxelConfig {
        sim: cubarium_session::SimParams {
            world: world_cfg.clone(),
            ..Default::default()
        },
        raster_height: 0,
        px_per_voxel: px,
        lighting: Lighting::Lit,
        organisms: OrganismLook::Models,
        ..VoxelConfig::default()
    };
    cfg.figures.rig = rig_draw;
    if flip.is_some() {
        // the flipbook swaps its model outright, as a flipbook does
        cfg.figures.swap_s = 0.0;
    }
    let models =
        Arc::new(ModelLibrary::load(&cfg.models_dir, VOXEL_M).context("the baked models")?);
    let loop_s = models
        .rig(founder)
        .with_context(|| format!("the {} has no rig: rebake it", founder.name()))?
        .loop_s();
    let proj = Projection::new(cfg.tilt_degrees, px, 0, &world_cfg)?;
    let (rw, rh) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
    let mut sink = VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions {
            models: Some(models.clone()),
            ..Default::default()
        },
    )?;

    let last = (LENGTH_S * HZ).round() as u64;
    let ws = weights(last);
    let mut frame = 0u32;
    let mut shown: Vec<Option<*const Vec<ModelCell>>> = vec![None; animals.len()];
    for tick in 1..=last {
        for a in &mut animals {
            a.state = if eating(tick) {
                State::Cropping
            } else {
                State::Resting
            };
        }
        for k in 0..3 {
            let t = tick as f64 + f64::from(k) / 3.0;
            let mut restage = k == 0;
            if let Some(fb) = &flip {
                // the drawn time, one tick behind the clock
                let td = t - 1.0;
                for (i, a) in animals.iter().enumerate() {
                    let cells = fb.at(
                        weight_at(&ws, td),
                        td / HZ + rig::phase_offset_s(a.id, loop_s),
                    );
                    if shown[i] != Some(Arc::as_ptr(cells)) {
                        shown[i] = Some(Arc::as_ptr(cells));
                        sink.set_figure_cells(a.id, Some(cells.clone()));
                        restage = true;
                    }
                }
            }
            if restage {
                stage(&mut sink, &w, &flora, fv, &animals, tick)?;
            }
            sink.set_clock(tick, f64::from(k) / 3.0);
            sink.render()?;
            let rgb: Vec<u8> = sink
                .read_raster()?
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            write_png(&frames.join(format!("f-{frame:04}.png")), rw, rh, &rgb)?;
            frame += 1;
        }
    }
    println!(
        "{rw}x{rh}; {frame} frames at 60 a second in {}",
        frames.display()
    );
    Ok(())
}

fn stage(
    sink: &mut VoxelGpuSink,
    w: &World,
    flora: &Flora,
    fv: FaunaView<'_>,
    animals: &[Animal],
    tick: u64,
) -> Result<()> {
    let mut view = w.view();
    view.tick = tick;
    anyhow::ensure!(
        sink.stage_view(&view, flora.view(), FaunaView { animals, ..fv }),
        "no staging buffer"
    );
    Ok(())
}

/// `n` grown frondgrazers (or `founder`s) cropping: each frame's milliseconds with the rig
/// drawn and with it off, over the same 240 frames.
fn run_perf(n: usize, px: u32, rig_draw: RigDraw, founder: Founder) -> Result<()> {
    let (width, depth) = (160u32, 48u32);
    let w = world(width, depth);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let mut placed = 0;
    'rows: for z in (2..depth - 2).step_by(3) {
        for x in (4..width as i64 - 4).step_by(8) {
            if placed == n {
                break 'rows;
            }
            let heading_rad = (placed as f64 * 0.7).rem_euclid(std::f64::consts::TAU);
            if fauna.apply(
                &w,
                FaunaCommand::IntroduceFounder {
                    x,
                    z,
                    founder,
                    stores: StartingStores::FULL,
                    heading_rad,
                },
            ) {
                placed += 1;
            }
        }
    }
    anyhow::ensure!(placed == n, "placed {placed} of {n} {}s", founder.name());
    let flora = Flora::new(FloraConfig::default());
    let fv = fauna.view();
    let adult = fv.config.founder(founder).core.body_max;
    let mut animals = fv.animals.to_vec();
    for a in &mut animals {
        a.body = adult;
        a.state = State::Cropping;
    }
    let world_cfg = w.config().clone();
    let base = VoxelConfig {
        sim: cubarium_session::SimParams {
            world: world_cfg.clone(),
            ..Default::default()
        },
        raster_height: 0,
        px_per_voxel: px,
        lighting: Lighting::Lit,
        organisms: OrganismLook::Models,
        ..VoxelConfig::default()
    };
    let models = Arc::new(ModelLibrary::load(&base.models_dir, VOXEL_M)?);
    for draw in [rig_draw, RigDraw::Off] {
        let mut cfg = base.clone();
        cfg.figures.rig = draw;
        let proj = Projection::new(cfg.tilt_degrees, px, 0, &world_cfg)?;
        let mut sink = VoxelGpuSink::new(
            &cfg,
            proj,
            VoxelGpuSinkOptions {
                models: Some(models.clone()),
                ..Default::default()
            },
        )?;
        let (mut clock_ms, mut render_ms, mut frames) = (0.0, 0.0, 0);
        for tick in 1..=80u64 {
            stage(&mut sink, &w, &flora, fv, &animals, tick)?;
            for k in 0..3 {
                let t0 = Instant::now();
                sink.set_clock(tick, f64::from(k) / 3.0);
                let t1 = Instant::now();
                sink.render()?;
                let _ = sink.read_raster()?;
                let t2 = Instant::now();
                // the first second eases every body in; time the rest
                if tick > 20 {
                    clock_ms += (t1 - t0).as_secs_f64() * 1e3;
                    render_ms += (t2 - t1).as_secs_f64() * 1e3;
                    frames += 1;
                }
            }
        }
        let f = f64::from(frames);
        println!(
            "{n} cropping, rig {draw:?}: clock (host figures + bones) {:.3} ms, render + read-back {:.3} ms a frame, over {frames} frames; {}",
            clock_ms / f,
            render_ms / f,
            {
                let s = sink.figure_atlas_stats();
                format!(
                    "{} tiles, AA {}, dropped {}",
                    s.tiles, s.supersample, s.dropped
                )
            },
        );
    }
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
