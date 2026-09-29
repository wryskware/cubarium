//! `voxel_idle_capture`: one browser walking in, standing idle, and walking off, headless,
//! for judging the idle pose (WX4): its breath and its head moves.
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_idle_capture -- OUT_DIR [--px N] [--idle-s S]
//! cargo run -p cubarium --release --example voxel_idle_capture -- --bench [--lit] [--glyphs] [--px N]
//! ```
//!
//! Writes `OUT_DIR/f-0000.png …` at 60 frames a second of the world's own time (three frames
//! a 20 Hz tick): the flat and lit tiers, each with the baked model and the glyph, at 2x, over
//! a lit 4x close-up of the model. The browser is scripted (the presenter reads only its pose
//! and state): it walks in, stands `--idle-s` seconds (default 12) at a stop chosen so its own
//! head-move schedule holds a dip and a glance, and walks off. Also writes
//! `OUT_DIR/strip.png`, the lit close-ups (model over glyph) walking, idling, at the peak of a
//! dip and of a glance, and walking off, and prints the ffmpeg line. No window is opened.
//!
//! `--bench` instead times the slab walk on a wider world (256 x 48 x 32, 64 browsers at
//! `--px`, default 6): every browser walking in place (no pose), every one idle, every one
//! asleep, each over 300 frames at 60 fps, and prints the GPU milliseconds a frame (a frame
//! the tier does not redraw costs nothing: the lit tier redraws every frame while a body
//! poses, and only at the tick otherwise).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::idle::{self, Kind};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{Lighting, OrganismLook, VoxelConfig};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{
    Animal, Command as FaunaCommand, DT, Fauna, FaunaConfig, FaunaView, Founder, State,
    StartingStores,
};
use cubarium_voxel_flora::{Flora, FloraConfig};

const VOXEL_M: f64 = 0.125;
/// Walking pace, metres a second (about half a body length).
const PACE_M_S: f64 = 0.4;

fn ground(cfg: Config) -> World {
    let mut w = World::empty(cfg.clone());
    for z in 0..cfg.depth {
        for x in 0..i64::from(cfg.width) {
            for y in 0..3 {
                w.apply(VoxelCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: if y == 0 { Material::Bedrock } else { Material::Soil },
                });
            }
        }
    }
    w
}

fn sink_for(world: &Config, px: u32, lit: bool, glyphs: bool) -> Result<VoxelGpuSink> {
    let cfg = VoxelConfig {
        world: world.clone(),
        raster_height: 0,
        px_per_voxel: px,
        lighting: if lit { Lighting::Lit } else { Lighting::Flat },
        organisms: if glyphs { OrganismLook::Glyphs } else { OrganismLook::Models },
        ..VoxelConfig::default()
    };
    let models = if glyphs {
        None
    } else {
        Some(Arc::new(
            ModelLibrary::load(&cfg.models_dir, VOXEL_M).context("the baked models")?,
        ))
    };
    let proj = Projection::new(cfg.tilt_degrees, px, 0, world)?;
    VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions {
            models,
            ..Default::default()
        },
    )
}

/// Stage a view, drawing a frame between refusals (the lit tier refuses a pack until its
/// founding sky plane has arrived).
fn stage(sink: &mut VoxelGpuSink, w: &World, flora: &Flora, fauna: FaunaView<'_>) -> Result<()> {
    for _ in 0..200 {
        if sink.stage_view(&w.view(), flora.view(), fauna) {
            return Ok(());
        }
        sink.render()?;
    }
    bail!("no staging buffer")
}

fn main() -> Result<()> {
    let mut out = None;
    let mut px = 14u32;
    let mut idle_s = 12.0f64;
    let mut bench = false;
    let mut lit = false;
    let mut glyphs = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--bench" => bench = true,
            "--lit" => lit = true,
            "--glyphs" => glyphs = true,
            "--px" => px = args.next().context("--px N")?.parse()?,
            "--idle-s" => idle_s = args.next().context("--idle-s S")?.parse()?,
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    if bench {
        return run_bench(if px == 14 { 6 } else { px }, lit, glyphs);
    }
    let out = out.context("usage: voxel_idle_capture OUT_DIR [--px N] [--idle-s S]")?;
    std::fs::create_dir_all(&out)?;

    let cfg = Config {
        width: 24,
        height: 9,
        depth: 6,
        voxel_m: VOXEL_M,
        day: DayClock {
            start_hour: 10.0,
            ..DayClock::default()
        },
        ..Config::default()
    };
    let mut w = ground(cfg.clone());
    let mut fauna = Fauna::new(FaunaConfig::default());
    anyhow::ensure!(
        fauna.apply(
            &w,
            FaunaCommand::IntroduceFounder {
                x: 3,
                z: 3,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: std::f64::consts::FRAC_PI_2,
            }
        ),
        "could not place the browser"
    );
    let flora = Flora::new(FloraConfig::default());
    let start: Animal = fauna.view().animals[0];
    let id = start.id;

    // The script, in ticks: walk in, stand, walk off. The stop is the first after 2 s whose
    // idle (from a second after stopping, once the hold and the fade are through) holds a
    // dip and a glance.
    let hz = 1.0 / DT;
    let idle_ticks = (idle_s * hz) as u64;
    let stop = (40..2000u64)
        .find(|&s| {
            let m = idle::moves_between(id, s + hz as u64, s + idle_ticks - 10, DT);
            m.iter().any(|m| matches!(m.kind, Kind::Dip(_)))
                && m.iter().any(|m| matches!(m.kind, Kind::Glance(_)))
        })
        .context("no stop with a dip and a glance")?;
    let walk_in = (2.0 * hz) as u64;
    let first = stop - walk_in;
    let go = stop + idle_ticks;
    let last = go + (2.5 * hz) as u64;
    let moves = idle::moves_between(id, stop, go, DT);
    eprintln!("stop at tick {stop}, walk off at {go}; head moves {moves:?}");
    let step_m = PACE_M_S * DT;
    let at_tick = |tick: u64| -> Animal {
        let mut a = start;
        let walked = tick.saturating_sub(first).min(walk_in) + tick.saturating_sub(go);
        a.pose.x = start.pose.x + walked as f64 * step_m;
        a.state = if tick > first && (tick <= stop || tick > go) {
            State::Walking
        } else {
            State::Resting
        };
        a
    };

    // The panels: flat model, flat glyph, lit model, lit glyph.
    let mut sinks = Vec::new();
    for (lit, glyphs) in [(false, false), (false, true), (true, false), (true, true)] {
        sinks.push(sink_for(&cfg, px, lit, glyphs)?);
    }
    let proj = Projection::new(VoxelConfig::default().tilt_degrees, px, 0, &cfg)?;
    let (rw, rh) = (usize::from(proj.raster_w), usize::from(proj.raster_h));
    // The close-up: the stop's neighbourhood, found from where the lit model's picture
    // differs from the empty ground.
    let crop = {
        let empty = FaunaView {
            animals: &[],
            ..fauna.view()
        };
        let mut s = sink_for(&cfg, px, true, false)?;
        stage(&mut s, &w, &flora, empty)?;
        s.set_clock(0, 0.0);
        for _ in 0..8 {
            s.render()?;
        }
        let bare = s.read_raster()?;
        let a = at_tick(stop);
        let one = FaunaView {
            animals: std::slice::from_ref(&a),
            ..fauna.view()
        };
        stage(&mut s, &w, &flora, one)?;
        s.set_clock(0, 0.0);
        for _ in 0..8 {
            s.render()?;
        }
        let body = s.read_raster()?;
        let (mut x0, mut y0, mut x1, mut y1) = (rw, rh, 0, 0);
        for y in 0..rh {
            for x in 0..rw {
                let i = (y * rw + x) * 4;
                if bare[i..i + 3] != body[i..i + 3] {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
            }
        }
        anyhow::ensure!(x1 >= x0, "the browser is not in the picture");
        eprintln!("the browser at rest spans x {x0}..={x1}, y {y0}..={y1}");
        let m = px as usize * 3;
        let cw = (2 * rw / 4).max(x1 - x0 + 2 * m) & !1;
        let ch = (y1 - y0 + 2 * m) & !1;
        let cx = ((x0 + x1) / 2).saturating_sub(cw / 2).min(rw - cw);
        let cy = ((y0 + y1) / 2).saturating_sub(ch / 2).min(rh - ch);
        (cx, cy, cw, ch)
    };
    let (_, _, cw, ch) = crop;
    let ow = (4 * rw).max(4 * cw);
    let oh = 4 * rh + 4 * ch;
    eprintln!("panels {rw}x{rh} at 2x, close-up {cw}x{ch} at 4x: {ow}x{oh}");

    // The strip's moments: walking, idling, a dip's and a glance's peak, walking off.
    let peak = |want: fn(&Kind) -> bool| {
        moves
            .iter()
            .find(|m| want(&m.kind))
            .map(|m| (m.start_tick + m.ticks / 2) as u64)
    };
    let moments = [
        first + walk_in / 2,
        stop + (1.5 * hz) as u64,
        peak(|k| matches!(k, Kind::Dip(_))).context("a dip")?,
        peak(|k| matches!(k, Kind::Glance(_))).context("a glance")?,
        go + (1.5 * hz) as u64,
    ];
    let mut strip: Vec<Vec<u8>> = Vec::new();

    let mut canvas = vec![0u8; ow * oh * 3];
    let mut frame = 0u32;
    let mut w_tick = 0u64;
    for tick in first..=last {
        while w_tick < tick {
            w.step();
            w_tick += 1;
        }
        let a = at_tick(tick);
        let view = FaunaView {
            animals: std::slice::from_ref(&a),
            ..fauna.view()
        };
        for s in &mut sinks {
            stage(s, &w, &flora, view)?;
        }
        for k in 0..3 {
            canvas.fill(0);
            for (i, s) in sinks.iter_mut().enumerate() {
                s.set_clock(tick, f64::from(k) / 3.0);
                s.render()?;
                let rgba = s.read_raster()?;
                let (ox, oy) = ((i % 2) * 2 * rw, (i / 2) * 2 * rh);
                blit(&mut canvas, ow, &rgba, rw, (0, 0, rw, rh), (ox, oy), 2);
                if i == 2 {
                    blit(&mut canvas, ow, &rgba, rw, crop, (0, 4 * rh), 4);
                }
                if k == 0 && moments.contains(&tick) && i >= 2 {
                    let mut c = vec![0u8; 4 * cw * 4 * ch * 3];
                    blit(&mut c, 4 * cw, &rgba, rw, crop, (0, 0), 4);
                    strip.push(c);
                }
            }
            write_png(&out.join(format!("f-{frame:04}.png")), ow, oh, &canvas)?;
            frame += 1;
        }
    }
    // The strip: the lit model over the lit glyph, one column a moment.
    let (sw, sh) = (4 * cw * moments.len(), 4 * ch * 2);
    let mut sp = vec![0u8; sw * sh * 3];
    for (n, c) in strip.iter().enumerate() {
        let (col, row) = (n / 2, n % 2);
        for y in 0..4 * ch {
            let d = ((row * 4 * ch + y) * sw + col * 4 * cw) * 3;
            sp[d..d + 4 * cw * 3].copy_from_slice(&c[y * 4 * cw * 3..(y + 1) * 4 * cw * 3]);
        }
    }
    write_png(&out.join("strip.png"), sw, sh, &sp)?;
    println!(
        "{frame} frames, {ow}x{oh}: ffmpeg -framerate 60 -i {}/f-%04d.png -c:v libx264 -preset slow \
         -crf 12 -pix_fmt yuv444p OUT.mp4",
        out.display()
    );
    Ok(())
}

/// Copy `src`'s rectangle `(x, y, w, h)` (RGBA, `sw` wide) into `dst` (RGB, `dw` wide) at
/// `at`, each pixel `k` x `k`.
fn blit(
    dst: &mut [u8],
    dw: usize,
    src: &[u8],
    sw: usize,
    (x, y, w, h): (usize, usize, usize, usize),
    at: (usize, usize),
    k: usize,
) {
    for sy in 0..h {
        for sx in 0..w {
            let i = ((y + sy) * sw + x + sx) * 4;
            let p = [src[i], src[i + 1], src[i + 2]];
            for dy in 0..k {
                let row = at.1 + sy * k + dy;
                for dx in 0..k {
                    let d = (row * dw + at.0 + sx * k + dx) * 3;
                    dst[d..d + 3].copy_from_slice(&p);
                }
            }
        }
    }
}

/// The slab walk's GPU time a redraw: every browser walking in place, then every one idle,
/// then every one asleep.
fn run_bench(px: u32, lit: bool, glyphs: bool) -> Result<()> {
    let cfg = Config {
        width: 256,
        height: 48,
        depth: 32,
        voxel_m: VOXEL_M,
        ..Config::default()
    };
    let w = ground(cfg.clone());
    let mut fauna = Fauna::new(FaunaConfig::default());
    for i in 0..64i64 {
        fauna.apply(
            &w,
            FaunaCommand::IntroduceFounder {
                x: 4 + (i % 16) * 16,
                z: 3 + (i / 16) as u32 * 7,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: std::f64::consts::FRAC_PI_2,
            },
        );
    }
    let flora = Flora::new(FloraConfig::default());
    let fv = fauna.view();
    for (name, state) in [
        ("walking", State::Walking),
        ("idle   ", State::Resting),
        ("asleep ", State::Asleep),
    ] {
        let mut sink = sink_for(&cfg, px, lit, glyphs)?;
        let mut animals = fv.animals.to_vec();
        for a in &mut animals {
            a.state = state;
        }
        let view = FaunaView {
            animals: &animals,
            ..fv
        };
        let mut start = None;
        let mut glancing = 0u32;
        for frame in 0..420u64 {
            let tick = frame / 3;
            if frame % 3 == 0 {
                let mut v = w.view();
                v.tick = tick;
                anyhow::ensure!(sink.stage_view(&v, flora.view(), view), "no staging buffer");
                if frame >= 120 {
                    glancing += animals
                        .iter()
                        .filter(|a| {
                            idle::move_near(a.id, tick, DT)
                                .is_some_and(|m| matches!(m.kind, Kind::Glance(_)))
                        })
                        .count() as u32;
                }
            }
            sink.set_clock(tick, (frame % 3) as f64 / 3.0);
            sink.render()?;
            let _ = sink.read_raster()?;
            // Past the hold and the fade: every idle body is fully idle.
            if frame == 119 {
                start = Some(sink.gpu_stage_totals());
            }
        }
        let (n0, t0) = start.expect("warm-up");
        let (n1, t1) = sink.gpu_stage_totals();
        let n = (n1 - n0).max(1) as f64;
        println!(
            "{} {} {name} px {px}: walk {:.3} ms, upload {:.3} ms a frame over {} frames at 60 fps{}",
            if lit { "lit " } else { "flat" },
            if glyphs { "glyph" } else { "model" },
            (t1[1] - t0[1]) / n,
            (t1[0] - t0[0]) / n,
            n1 - n0,
            if state == State::Resting {
                format!(", {:.1} bodies glancing a tick", f64::from(glancing) / 100.0)
            } else {
                String::new()
            }
        );
    }
    Ok(())
}

fn write_png(path: &Path, w: usize, h: usize, rgb: &[u8]) -> Result<()> {
    let file = std::fs::File::create(path).with_context(|| format!("{}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgb)?;
    Ok(())
}
