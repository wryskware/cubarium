//! Lanternjaw study route: the code-native multipart body of [`cubarium::lanternjaw`] on the
//! real five-face output path, as explicit development choreography.
//!
//! This is a *study*, not the world: no simulation, no fields, no organisms, no founders and
//! no saved state are touched, and nothing here is evidence of hunting, gestation or any
//! other ecology. Scenes are scripted: `gallery` shows the four modes side by side (with one
//! ordinary atelier creature beside them for scale), `seams` places bodies on seams, a top
//! vertex and the open rim, `walk` travels one body across several seams through the shared
//! `travel`/`interpolate` transport, and `phases` scripts the **real** hunting phases of
//! [`cubarium::lanternjaw::AttackPhase`] held far longer than the study's six-second loop.
//! Positions are never rounded: the body reads its own fractional anchor and the presentation
//! clock `(tick − 1 + f) · DT`.
//!
//! # Study modes and the living body
//!
//! `Rest` and `Move` are drawn through [`Lanternjaw::draw_living`] from a [`LivingPose`]
//! ([`mode_pose`]: movement 0 and movement 1, with each body's own phase offset), so the
//! semantic entry point is what the gallery actually exercises; `Hunt` and `Bud` keep the
//! study's [`Lanternjaw::draw`], because a synthetic six-second strike and an unconditional
//! cocoon are exactly what the living pose refuses to invent, and the gallery is meant to stay
//! the picture it has always been. Either way, nothing here observes a world.
//!
//! # `--scale`
//!
//! `--scale <f64>` (default 1, admitted range [`SCALE_MIN`]`..=`[`SCALE_MAX`]) is the
//! whole-rig scale handed to [`Lanternjaw::draw_living`] for every body that is drawn that
//! way — offsets, pivots, the lattice, the lunge and the query radius shrink together, so a
//! juvenile is the same rig, not a detached one. It therefore affects the `Rest`/`Move` bodies
//! of `gallery`, `seams` and `walk` and every body of `phases` **except** the two in the
//! juvenile/adult comparison row, which carry explicit scales of their own so that one frame
//! shows both sizes whatever `--scale` says. It does not affect the `Hunt`/`Bud` bodies, which
//! still go through the unscaled study path.
//!
//! # PNG manifest
//!
//! With `--sink png` each *saved* frame's index and presentation instant are printed to
//! stdout (`frame 000048 t=1.550123`), because the frame schedule is wall-clock paced: a
//! contact sheet that claims to show t = 1.55 s has to read the instant the frame was drawn
//! at rather than assume one. The frame files themselves are `frame_NNNNNN.png` in `--out`.

use anyhow::{Result, ensure};
use clap::{Parser, ValueEnum};
use cubarium::{
    art::ArtPack,
    art_present::{SOIL_HIGH_SRGB, WATER_LOW_SRGB, present_seconds},
    clock::{Clock, DT, Step},
    lanternjaw::{
        AttackEpisode, AttackPhase, Lanternjaw, LivingPose, Mode, Reach, SCALE_MAX, SCALE_MIN,
        attack_channels,
    },
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
    /// Real hunting phases, each held longer than the study's whole loop: perched, stalking,
    /// one scripted attack, a funded escrow, and a juvenile beside an adult.
    Phases,
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
    /// Whole-rig scale for every body drawn through `draw_living` (see the module header).
    #[arg(long, default_value_t = 1.0)]
    scale: f64,
}

/// What a scripted body shows.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pose {
    /// A study mode (see the module header): `Rest` and `Move` through the living entry
    /// point, `Hunt` and `Bud` through the study's own `draw`.
    Study(Mode),
    /// One row of the `phases` scene's scripted real-phase timeline.
    Script(Row),
}

/// The rows of the `phases` scene. Every one of them is held for the whole run, which is
/// twice the study's loop, so nothing in the picture can have come from a synthetic cycle.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Row {
    /// Perched: the rest wave, planted legs, quiet lanterns, no episode at all.
    Perched,
    /// Stalking: a restrained move wave and gait, arms still folded.
    Stalking,
    /// One scripted attack, chained phase to phase from each phase's displayed reach, then
    /// `Handling` with a real gut and no cocoon.
    Attack,
    /// A funded escrow whose gestation rises 0 → 1 over the run.
    Escrow,
}

/// The `phases` scene's own schedule, in seconds of presentation time. The strike's *phase*
/// lasts [`STRIKE_SECONDS`] and is then **held** for [`SETTLEMENT_WAIT`] more so the sheet can
/// show that full extension does not recoil on its own.
const SCRIPT_SECONDS: f64 = 12.0;
const WINDUP_SECONDS: f64 = 0.6;
const STRIKE_SECONDS: f64 = 1.0;
const SETTLEMENT_WAIT: f64 = 2.0;
const RECOVER_SECONDS: f64 = 5.0;
/// The gut fill shown during `Handling`: the meal is this breath, not a flourish.
const HANDLING_GUT: f64 = 0.8;
/// The stalking rows' movement blend: enough gait to read, well short of a sprint.
const STALK_MOVEMENT: f64 = 0.6;

/// One scripted body. `anchor`/`heading` are the tick state; `moved` is the path the last
/// tick travelled, which the frame fraction interpolates along exactly as the world does.
struct Body {
    anchor: SurfacePoint,
    heading: Vec2,
    moved: Vec<PathSegment>,
    pose: Pose,
    /// Body-local phase offset in seconds, so two bodies in one mode are not in lockstep.
    phase: f64,
    /// Chart pixels per second along the heading; 0 stands still.
    speed: f64,
    /// Radians per second the heading turns (a gently curving walk).
    turn: f64,
    /// A whole-rig scale of this body's own, overriding `--scale`: the juvenile/adult
    /// comparison row needs both sizes in one frame.
    scale: Option<f64>,
}

impl Body {
    fn still(anchor: SurfacePoint, heading: Vec2, mode: Mode) -> Body {
        Body {
            anchor,
            heading,
            moved: vec![],
            pose: Pose::Study(mode),
            phase: 0.0,
            speed: 0.0,
            turn: 0.0,
            scale: None,
        }
    }

    /// One `phases` row, facing +x at chart `(u, v)` on Front.
    fn script(u: f64, v: f64, row: Row) -> Body {
        Body {
            pose: Pose::Script(row),
            ..Body::still(
                SurfacePoint::new(Face::Front, u, v),
                Vec2::new(1.0, 0.0),
                Mode::Rest,
            )
        }
    }
}

/// A study mode as a living pose: `Rest` is movement 0 and `Move` is full locomotion, with
/// nothing else — no episode, no meal, no escrow — so both are the gallery's own picture.
fn mode_pose(mode: Mode, seconds: f64) -> LivingPose {
    LivingPose {
        ambient: seconds,
        movement: if mode == Mode::Move { 1.0 } else { 0.0 },
        attack: None,
        gut: 0.0,
        cocoon: None,
    }
}

/// One `phases` row's semantic pose at presentation `seconds`.
fn script_pose(row: Row, seconds: f64) -> LivingPose {
    let t = if seconds.is_finite() {
        seconds.max(0.0)
    } else {
        0.0
    };
    let quiet = LivingPose {
        ambient: seconds,
        movement: 0.0,
        attack: None,
        gut: 0.0,
        cocoon: None,
    };
    match row {
        Row::Perched => quiet,
        Row::Stalking => LivingPose {
            movement: STALK_MOVEMENT,
            ..quiet
        },
        // A funded escrow, and only that: the cocoon's reveal is the real gestation.
        Row::Escrow => LivingPose {
            cocoon: Some((t / SCRIPT_SECONDS).clamp(0.0, 1.0)),
            ..quiet
        },
        Row::Attack => {
            let (attack, gut) = attack_script(t);
            LivingPose {
                attack: Some(attack),
                gut,
                ..quiet
            }
        }
    }
}

/// The scripted attack at attack-script time `t`, and the gut fill that goes with it.
///
/// Each phase is entered with the reach the previous phase **actually displayed** at its final
/// instant, exactly as an adapter would carry it, so the motion is continuous across every
/// boundary and nothing snaps to a study keyframe.
fn attack_script(t: f64) -> (AttackEpisode, f64) {
    let windup = |elapsed: f64| AttackEpisode {
        phase: AttackPhase::Windup,
        elapsed,
        duration: WINDUP_SECONDS,
        from: Reach::FOLDED,
    };
    if t < WINDUP_SECONDS {
        return (windup(t), 0.0);
    }
    // The strike's phase ends at `WINDUP + STRIKE`, but it is held to `held` so the settled
    // full extension is on screen for two more seconds without a recoil.
    let held = WINDUP_SECONDS + STRIKE_SECONDS + SETTLEMENT_WAIT;
    let from = attack_channels(Some(&windup(WINDUP_SECONDS))).reach();
    let strike = |elapsed: f64| AttackEpisode {
        phase: AttackPhase::Strike,
        elapsed,
        duration: STRIKE_SECONDS,
        from,
    };
    if t < held {
        return (strike(t - WINDUP_SECONDS), 0.0);
    }
    let from = attack_channels(Some(&strike(held - WINDUP_SECONDS))).reach();
    let recovering = |elapsed: f64| AttackEpisode {
        phase: AttackPhase::Recovering,
        elapsed,
        duration: RECOVER_SECONDS,
        from,
    };
    if t < held + RECOVER_SECONDS {
        return (recovering(t - held), 0.0);
    }
    let start = held + RECOVER_SECONDS;
    let from = attack_channels(Some(&recovering(RECOVER_SECONDS))).reach();
    (
        AttackEpisode {
            phase: AttackPhase::Handling,
            elapsed: t - start,
            duration: (SCRIPT_SECONDS - start).max(0.1),
            from,
        },
        HANDLING_GUT,
    )
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
                Body::still(
                    SurfacePoint::new(Face::Front, 32.0, v),
                    Vec2::new(1.0, 0.0),
                    mode,
                )
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
        // Real phases, one per row, all facing +x on Front. Every row is held for the whole
        // run — twice the study's loop — so a single synthetic strike anywhere would show.
        Scene::Phases => vec![
            // (a) perched for twelve seconds.
            Body::script(32.0, 8.0, Row::Perched),
            // (b) stalking, and actually walking: 0.9 px/s of real root travel through the
            // shared transport, so the gait is driven by movement rather than implying it.
            Body {
                speed: 0.9,
                ..Body::script(32.0, 20.0, Row::Stalking)
            },
            // (c) the scripted attack: windup, strike, held settlement, recovery, handling.
            Body::script(32.0, 32.0, Row::Attack),
            // (d) a funded escrow revealing over the whole run.
            Body::script(32.0, 44.0, Row::Escrow),
            // (e) a juvenile beside an adult, both stalking in place so the row is a size
            // comparison rather than a chase. Their scales are explicit, not `--scale`: the
            // juvenile spans u ≈ 13..25 and the adult u ≈ 35..58, so they never overlap.
            Body {
                scale: Some(SCALE_MIN),
                ..Body::script(18.0, 56.0, Row::Stalking)
            },
            Body {
                scale: Some(SCALE_MAX),
                ..Body::script(45.0, 56.0, Row::Stalking)
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
        .map(|v| Scale {
            anchor: SurfacePoint::new(Face::Front, 52.0, v),
            form: 0,
            state: 0,
        })
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
    ensure!(
        args.scale.is_finite() && (SCALE_MIN..=SCALE_MAX).contains(&args.scale),
        "--scale must be within the admitted {SCALE_MIN}..={SCALE_MAX}"
    );
    let scale = scene_scale(args.scene);
    // The ordinary creature beside the gallery body comes from the shipped pack; nothing
    // about the Lanternjaw is in it, and loading it changes no form index.
    let art = if scale.is_empty() {
        None
    } else {
        Some(ArtPack::load(&args.art)?)
    };
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
                    let (anchor, heading) = interpolate(&body.moved, body.anchor, body.heading, f);
                    let t = seconds + body.phase;
                    let scale = body.scale.unwrap_or(args.scale);
                    match body.pose {
                        // The gallery's synthetic loop and unconditional cocoon stay on the
                        // study path, unscaled, so the gallery is exactly what it was.
                        Pose::Study(mode @ (Mode::Hunt | Mode::Bud)) => rig.draw(
                            &mut canvas,
                            anchor,
                            heading,
                            t,
                            mode,
                            1.0,
                            &mut parts,
                            &mut scratch,
                        ),
                        Pose::Study(mode) => rig.draw_living(
                            &mut canvas,
                            anchor,
                            heading,
                            &mode_pose(mode, t),
                            scale,
                            1.0,
                            &mut parts,
                            &mut scratch,
                        ),
                        Pose::Script(row) => rig.draw_living(
                            &mut canvas,
                            anchor,
                            heading,
                            &script_pose(row, t),
                            scale,
                            1.0,
                            &mut parts,
                            &mut scratch,
                        ),
                    }
                }
                canvas.encode(&mut frame);
                sink.submit(&frame)?;
                // The frame schedule is wall-clock paced, so a capture's own instant is the
                // only honest label for it: print the manifest the contact sheets read.
                if args.sink == Sink::Png && frames.is_multiple_of(args.every) {
                    println!("frame {:06} t={seconds:.6}", frames / args.every);
                }
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
