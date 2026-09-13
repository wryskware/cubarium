//! Lanternjaw study route: the code-native multipart body of [`cubarium::lanternjaw`] on the
//! real five-face output path, as explicit development choreography.
//!
//! This is a *study*, not the world: no simulation, no fields, no organisms, no founders and
//! no saved state are touched, and nothing here is evidence of hunting, gestation or any
//! other ecology. Scenes are scripted: `gallery` shows the four modes side by side (with one
//! ordinary atelier creature beside them for scale), `seams` places bodies on seams, a top
//! vertex and the open rim, and `walk` travels one body across several seams through the
//! shared `travel`/`interpolate` transport. Positions are never rounded: the body reads its
//! own fractional anchor and the presentation clock `(tick − 1 + f) · DT`.

use anyhow::{Result, ensure};
use clap::{Parser, ValueEnum};
use cubarium::{
    art::ArtPack,
    art_present::{SOIL_HIGH_SRGB, WATER_LOW_SRGB, present_seconds},
    clock::{Clock, DT, Step},
    lanternjaw::{Lanternjaw, Mode},
    present::{draw_floor, interpolate, srgb_linear},
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
    /// The four modes on Front, heading +x, with an ordinary creature for scale.
    Gallery,
    /// Seams, a top vertex approach and the open rim.
    Seams,
    /// One body travelling across several seams, plus a hunting body on Top.
    Walk,
}

/// What the body is composited over. The study was drawn on its own dark purple; the cube's
/// real ground is neither that nor black, so every backdrop the carapace has to hold up
/// against is available here (Astra's "compare black, study-purple, soil and water").
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq)]
enum Ground {
    /// The world's own quiet floor and ground wave, exactly as `art_study.rs` draws them.
    World,
    /// Nothing at all: the silhouette and every translucent pixel on black.
    Black,
    /// Flat `#0B0525`, the study's own background.
    Study,
    /// Flat soil.
    Soil,
    /// Flat water.
    Water,
}

#[derive(Parser)]
#[command(
    about = "Cubarium Lanternjaw study: the multipart rig on real cube geometry, normal speed"
)]
struct Args {
    #[arg(long, default_value = "assets/atelier")]
    art: PathBuf,
    #[arg(long, value_enum, default_value = "web")]
    sink: Sink,
    #[arg(long, value_enum, default_value = "gallery")]
    scene: Scene,
    #[arg(long, value_enum, default_value = "world")]
    ground: Ground,
    #[arg(long, default_value_t = 7399)]
    web_port: u16,
    #[arg(long, default_value_t = 0.0)]
    seconds: f64,
    #[arg(long, default_value_t = 60)]
    fps: u32,
    #[arg(long, default_value = "captures/lanternjaw")]
    out: PathBuf,
    #[arg(long, default_value_t = 30)]
    every: u64,
    #[arg(long, default_value = "127.0.0.1:7392")]
    addr: String,
}

/// One scripted body. `anchor`/`heading` are the tick state; `moved` is the path the last
/// tick travelled, which the frame fraction interpolates along exactly as the world does.
struct Body {
    anchor: SurfacePoint,
    heading: Vec2,
    moved: Vec<PathSegment>,
    mode: Mode,
    /// Body-local phase offset in seconds, so two bodies in one mode are not in lockstep.
    phase: f64,
    /// Chart pixels per second along the heading; 0 stands still.
    speed: f64,
    /// Radians per second the heading turns (a gently curving walk).
    turn: f64,
}

impl Body {
    fn still(anchor: SurfacePoint, heading: Vec2, mode: Mode) -> Body {
        Body { anchor, heading, moved: vec![], mode, phase: 0.0, speed: 0.0, turn: 0.0 }
    }
}

/// The atelier creature drawn beside the gallery's Lanternjaw for scale: the common rig at
/// rest, at its own anchor and clip time.
struct Scale {
    anchor: SurfacePoint,
    form: usize,
    state: usize,
}

fn scene_bodies(scene: Scene) -> Vec<Body> {
    match scene {
        // Four modes, one per row, all facing +x at u = 32.
        Scene::Gallery => Mode::ALL
            .into_iter()
            .zip([12.0, 26.0, 40.0, 54.0])
            .map(|(mode, v)| {
                Body::still(SurfacePoint::new(Face::Front, 32.0, v), Vec2::new(1.0, 0.0), mode)
            })
            .collect(),
        // A side/side seam, a side/top seam, a top vertex, the open rim, and the far face.
        Scene::Seams => vec![
            Body::still(
                SurfacePoint::new(Face::Front, 63.0, 32.0),
                Vec2::new(1.0, 0.0),
                Mode::Move,
            ),
            Body::still(
                SurfacePoint::new(Face::Right, 32.0, 3.0),
                Vec2::new(0.0, -1.0),
                Mode::Move,
            ),
            // The heading turns one full turn per 40 s, so every orientation of a body at a
            // top vertex is captured in a twelve-second sweep of a longer run.
            Body {
                turn: std::f64::consts::TAU / 40.0,
                ..Body::still(
                    SurfacePoint::new(Face::Top, 2.5, 2.5),
                    Vec2::new(1.0, 0.0),
                    Mode::Rest,
                )
            },
            Body::still(
                SurfacePoint::new(Face::Front, 32.0, 61.5),
                Vec2::new(0.6, 0.8),
                Mode::Hunt,
            ),
            Body::still(
                SurfacePoint::new(Face::Back, 32.0, 32.0),
                Vec2::new(-1.0, 0.0),
                Mode::Bud,
            ),
        ],
        // One body walking a gently curving path through several seams, and one hunting.
        Scene::Walk => vec![
            // 1.5 px/s crosses a whole face only in forty seconds, so the walk starts near
            // the Front/Right seam heading up and across: it crosses that side/side seam at
            // about 3.3 s and the Right/Top seam at about 7 s, which a twelve-second capture
            // shows. A longer run keeps turning and keeps crossing.
            Body {
                speed: 1.5,
                turn: 0.05,
                ..Body::still(
                    SurfacePoint::new(Face::Front, 60.0, 7.0),
                    Vec2::new(0.8, -0.6),
                    Mode::Move,
                )
            },
            Body {
                phase: 3.0,
                ..Body::still(
                    SurfacePoint::new(Face::Top, 32.0, 32.0),
                    Vec2::new(0.0, 1.0),
                    Mode::Hunt,
                )
            },
        ],
    }
}

/// Where the scale creature stands in the gallery: one per row, beside the Lanternjaw's head.
fn scene_scale(scene: Scene) -> Vec<Scale> {
    if scene != Scene::Gallery {
        return vec![];
    }
    [12.0, 26.0, 40.0, 54.0]
        .into_iter()
        .map(|v| Scale { anchor: SurfacePoint::new(Face::Front, 52.0, v), form: 0, state: 0 })
        .collect()
}

fn draw_ground(canvas: &mut Canvas, ground: Ground) {
    let flat = match ground {
        Ground::Black => return,
        Ground::World => {
            draw_floor(canvas);
            // Broad quiet ground, no field-cell grid or analytical overlays.
            let base = srgb_linear(0x0017_1b35);
            for face in Face::ALL {
                for y in 0..64u8 {
                    for x in 0..64u8 {
                        let wave =
                            0.32 + 0.06 * ((f64::from(x) + f64::from(y) * 0.7) * 0.12).sin() as f32;
                        canvas.add(face, x, y, base.map(|c| c * wave));
                    }
                }
            }
            return;
        }
        // The study's own background, and the two real bands whose brightness the
        // translucent carapace has to survive.
        Ground::Study => srgb_linear(0x000B_0525),
        Ground::Soil => srgb_linear(SOIL_HIGH_SRGB).map(|c| c * 0.35),
        Ground::Water => srgb_linear(WATER_LOW_SRGB).map(|c| c * 0.45),
    };
    for face in Face::ALL {
        for y in 0..64u8 {
            for x in 0..64u8 {
                canvas.add(face, x, y, flat);
            }
        }
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
    let scale = scene_scale(args.scene);
    // The ordinary creature beside the gallery body comes from the shipped pack; nothing
    // about the Lanternjaw is in it, and loading it changes no form index.
    let art = if scale.is_empty() { None } else { Some(ArtPack::load(&args.art)?) };
    let mut sink: Box<dyn FrameSink> = match args.sink {
        Sink::Web => {
            let web = WebSink::with_note(args.web_port, "1× time · Lanternjaw study")?;
            eprintln!(
                "Cubarium Lanternjaw study: {} · {:?} · 1× time",
                web.url(),
                args.scene
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

    let rig = Lanternjaw::new();
    let mut bodies = scene_bodies(args.scene);
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    let mut scratch = Vec::new();
    let mut parts = Vec::new();
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
                for body in &mut bodies {
                    body.moved.clear();
                    if body.turn != 0.0 {
                        let angle = body.heading.screen_angle() + body.turn * DT;
                        body.heading = Vec2::from_screen_angle(angle);
                    }
                    if body.speed > 0.0 {
                        // The same shared transport the world's bodies use: the root
                        // trajectory travels, its heading is mapped, and the frame fraction
                        // interpolates along the path actually walked.
                        let step = travel(body.anchor, body.heading * (body.speed * DT));
                        body.anchor = step.end;
                        body.heading = step.map.apply(body.heading);
                        body.moved = step.segments;
                    }
                }
            }
            Step::Render { f } => {
                let seconds = present_seconds(tick, f);
                canvas.clear();
                draw_ground(&mut canvas, args.ground);
                if let Some(art) = &art {
                    for s in &scale {
                        stamp_sprite(
                            &mut canvas,
                            s.anchor,
                            Vec2::new(1.0, 0.0),
                            art.creature(s.form, s.state, seconds),
                            1.0,
                            1.0,
                            &mut scratch,
                        );
                    }
                }
                for body in &bodies {
                    let (anchor, heading) =
                        interpolate(&body.moved, body.anchor, body.heading, f);
                    rig.draw(
                        &mut canvas,
                        anchor,
                        heading,
                        seconds + body.phase,
                        body.mode,
                        1.0,
                        &mut parts,
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
                    eprintln!(
                        "Lanternjaw study behind by {:.0}ms",
                        behind.as_secs_f64() * 1000.0
                    );
                }
            }
            Step::Paused { gap } => {
                eprintln!("Lanternjaw study resumed after {:.1}s", gap.as_secs_f64())
            }
        }
    }
    sink.finish()?;
    eprintln!(
        "Lanternjaw study: {tick} ticks, {frames} frames · {:?} · development choreography",
        args.scene
    );
    Ok(())
}
