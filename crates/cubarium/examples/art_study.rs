//! Godot-authored sprite/animation study on the real five-face output path.
//! Explicit development choreography; this is not an evolving population.

use anyhow::{Result, ensure};
use clap::{Parser, ValueEnum};
use cubarium::{
    art::ArtPack,
    clock::{Clock, DT, Step},
    present::{draw_floor, interpolate, srgb_linear},
    rng::SplitMix64,
    sink::{FrameSink, PngSink, PreviewSink, ShimSink, WebSink},
};
use cubarium_render::{Canvas, stamp_sprite};
use cubarium_surface::{PathSegment, SurfacePoint, Vec2, travel};
use cube_proto::{Face, Frame};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq)]
enum Sink {
    Web,
    Preview,
    Png,
    Shim,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq)]
enum Scene {
    Garden,
    Gallery,
}

#[derive(Parser)]
#[command(
    about = "Cubarium creature-art study: scripted specimens, real cube geometry, normal speed"
)]
struct Args {
    #[arg(long, default_value = "assets/atelier")]
    art: PathBuf,
    #[arg(long, value_enum, default_value = "web")]
    sink: Sink,
    #[arg(long, value_enum, default_value = "garden")]
    scene: Scene,
    #[arg(long, default_value_t = 7394)]
    web_port: u16,
    #[arg(long, default_value_t = 0.0)]
    seconds: f64,
    #[arg(long, default_value_t = 60)]
    fps: u32,
    #[arg(long, default_value_t = 7)]
    seed: u64,
    #[arg(long, default_value = "captures/atelier")]
    out: PathBuf,
    #[arg(long, default_value_t = 30)]
    every: u64,
    #[arg(long, default_value = "127.0.0.1:7392")]
    addr: String,
}

struct Specimen {
    anchor: SurfacePoint,
    heading: Vec2,
    moved: Vec<PathSegment>,
    form: usize,
    offset: f64,
    state: usize,
    clip_time: f64,
}

/// Slow explicit choreography makes each action inspectable. Existing ecology is
/// intentionally not used to assign pretend roles to these three visual studies.
fn action(time: f64) -> (usize, f64) {
    let t = time.rem_euclid(36.0);
    if t < 15.0 {
        (1, t)
    } else if t < 23.0 {
        (0, t - 15.0)
    } else if t < 31.0 {
        (2, t - 23.0)
    } else {
        (3, t - 31.0)
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        args.seconds.is_finite() && args.seconds >= 0.0,
        "--seconds must be finite and nonnegative"
    );
    ensure!((1..=240).contains(&args.fps), "--fps must be 1–240");
    ensure!(args.every > 0, "--every must be positive");
    ensure!(
        args.sink != Sink::Png || args.seconds > 0.0,
        "PNG capture requires --seconds"
    );
    let art = ArtPack::load(&args.art)?;
    let rigs = art.creature_count();
    // Gallery columns for up to four rigs at u = 8, 24, 40, 56; more rigs wrap onto
    // extra faces' worth of rows below, which the garden layout does not need.
    let column_u = |form: usize| 8.0 + (form % 4) as f64 * 16.0;
    let mut sink: Box<dyn FrameSink> = match args.sink {
        Sink::Web => {
            let web = WebSink::with_note(args.web_port, "1× time · art study")?;
            eprintln!(
                "Cubarium art study: {} · scripted gallery · 1× time",
                web.url()
            );
            Box::new(web)
        }
        Sink::Preview => Box::new(PreviewSink::new(4, &args.out)?),
        Sink::Png => Box::new(PngSink::new(&args.out, args.every)?),
        Sink::Shim => Box::new(ShimSink::new(args.addr.clone())),
    };
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    ctrlc::set_handler(move || flag.store(true, Ordering::Relaxed))?;
    let mut rng = SplitMix64::new(args.seed);
    let mut specimens = Vec::new();
    let mut plants = Vec::new();
    for face in Face::ALL {
        for index in 0..20 {
            // A sparse, authored motif vocabulary, stable placement for a fixed seed.
            let col = index % 5;
            let row = index / 5;
            let p = SurfacePoint::new(
                face,
                col as f64 * 13.0 + rng.range(0.0, 9.0),
                row as f64 * 16.0 + rng.range(1.0, 14.0),
            );
            let kind = if index % 4 == 0 {
                1
            } else if index % 3 == 0 {
                0
            } else {
                2
            };
            let heading = Vec2::from_screen_angle(rng.range(-0.5, 0.5));
            plants.push((p, kind, heading, rng.range(0.65, 1.0)));
        }
        let count = if args.scene == Scene::Gallery { 4 * rigs } else { 5 };
        for index in 0..count {
            let (u, v) = if args.scene == Scene::Gallery {
                // Columns are rigs, rows are rest/move/feed/bud.
                (column_u(index % rigs), 9.0 + (index / rigs) as f64 * 15.0)
            } else {
                (
                    (index % 3) as f64 * 21.0 + rng.range(4.0, 16.0),
                    (index / 3) as f64 * 30.0 + rng.range(10.0, 22.0),
                )
            };
            let offset = if args.scene == Scene::Gallery {
                0.0
            } else {
                rng.range(0.0, 36.0)
            };
            let (state, clip_time) = if args.scene == Scene::Gallery {
                (index / rigs, 0.0)
            } else {
                action(offset)
            };
            specimens.push(Specimen {
                anchor: SurfacePoint::new(face, u, v),
                heading: Vec2::new(1.0, 0.0),
                moved: vec![],
                form: (index + face as usize) % rigs,
                offset,
                state,
                clip_time,
            });
        }
    }
    // Two explicit crossings: Front→Right and Right→Top. Both use shared transport.
    if args.scene == Scene::Garden {
        specimens[0].anchor = SurfacePoint::new(Face::Front, 62.0, 32.0);
        specimens[0].offset = 0.0;
        specimens[5].anchor = SurfacePoint::new(Face::Right, 32.0, 2.0);
        specimens[5].heading = Vec2::new(0.0, -1.0);
        specimens[5].offset = 0.0;
    }
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    let mut scratch = Vec::new();
    let mut clock = Clock::with_fps(Instant::now(), args.fps);
    let mut tick = 0u64;
    let mut frames = 0u64;
    loop {
        let now = Instant::now();
        if stop.load(Ordering::Relaxed)
            || sink.should_quit()
            || (args.seconds > 0.0 && clock.elapsed(now).as_secs_f64() >= args.seconds)
        {
            break;
        }
        match clock.next_step(now) {
            Step::Tick => {
                tick += 1;
                for specimen in &mut specimens {
                    let t = tick as f64 * DT;
                    if args.scene == Scene::Garden {
                        (specimen.state, specimen.clip_time) = action(t + specimen.offset);
                    } else {
                        let clip = &art.clips[specimen.form * 4 + specimen.state];
                        specimen.clip_time = if specimen.state == 3 {
                            t.rem_euclid(7.0)
                        } else {
                            t.rem_euclid(clip.seconds)
                        };
                    }
                    specimen.moved.clear();
                    if specimen.state == 1 && args.scene == Scene::Garden {
                        // Travel pace per rig; rigs beyond the authored four reuse the cycle.
                        let speed = [1.1, 1.8, 0.7, 1.4][specimen.form % 4];
                        let bend = 0.025 * (t * 0.17 + specimen.offset).sin() * DT;
                        let a = specimen.heading.screen_angle() + bend;
                        specimen.heading = Vec2::from_screen_angle(a);
                        let step = travel(specimen.anchor, specimen.heading * (speed * DT));
                        specimen.anchor = step.end;
                        specimen.heading = step.map.apply(specimen.heading);
                        specimen.moved = step.segments;
                    }
                }
            }
            Step::Render { f } => {
                canvas.clear();
                draw_floor(&mut canvas);
                // Broad quiet ground, no field-cell grid or analytical overlays.
                let ground = srgb_linear(0x171b35);
                for face in Face::ALL {
                    for y in 0..64u8 {
                        for x in 0..64u8 {
                            let wave = 0.32
                                + 0.06 * ((f64::from(x) + f64::from(y) * 0.7) * 0.12).sin() as f32;
                            canvas.add(face, x, y, ground.map(|c| c * wave));
                        }
                    }
                }
                if args.scene == Scene::Garden {
                    for &(position, kind, heading, opacity) in &plants {
                        stamp_sprite(
                            &mut canvas,
                            position,
                            heading,
                            &art.habitat[kind],
                            1.0,
                            opacity as f32,
                            &mut scratch,
                        );
                    }
                }
                for s in &specimens {
                    let (position, heading) = interpolate(&s.moved, s.anchor, s.heading, f);
                    let sprite = art.creature(s.form, s.state, s.clip_time);
                    stamp_sprite(
                        &mut canvas,
                        position,
                        heading,
                        sprite,
                        1.0,
                        1.0,
                        &mut scratch,
                    );
                }
                canvas.encode(&mut frame);
                sink.submit(&frame)?;
                frames += 1;
            }
            Step::Sleep(duration) => std::thread::sleep(duration),
            Step::Lagged { behind, log } => {
                if log {
                    eprintln!("art study behind by {:.0}ms", behind.as_secs_f64() * 1000.0);
                }
            }
            Step::Paused { gap } => eprintln!("art study resumed after {:.1}s", gap.as_secs_f64()),
        }
    }
    sink.finish()?;
    eprintln!("Art study: {tick} ticks, {frames} frames · development choreography, 1× time");
    Ok(())
}
