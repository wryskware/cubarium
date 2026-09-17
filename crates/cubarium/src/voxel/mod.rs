//! `cubarium voxel`: run a [`cubarium_voxel::World`] at a fixed 20 Hz and draw it as
//! pixel art into a ring raster, through the same sinks the rest of the host uses.
//!
//! The loop is [`crate::run::drive`]'s shape — one [`Clock`], one `encode_raster` per
//! rendered frame, one sink — with two differences: the world is a voxel strip rather
//! than the fixture scenes, and a stdin reader thread lets the run be poked while it is
//! running (pause, single-step, speed, rain, set a cell's material, save, load, inspect,
//! outlet, quit).
//!
//! `--speed` scales world ticks per clock tick through an accumulator, so a fractional
//! speed slows the world down without touching the clock: the picture is still drawn at
//! `--fps` and the simulation still advances in whole 20 Hz ticks.
//!
//! The projection, the palette and the autotiling live in [`project`] and [`present`];
//! the hand-authored fixture in [`scene`]. The preview sink is deliberately absent: the
//! minifb window refuses ring rasters and that refusal is correct.

pub mod present;
pub mod project;
pub mod scene;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use cubarium_render::Canvas;
use cubarium_surface::{Scale, Topology};
use cubarium_voxel::{Command as VoxelCommand, Material, World};
use serde::{Deserialize, Serialize};

use crate::clock::{Clock, Step};
use crate::cli::{Voxel, VoxelSceneArg, VoxelSinkArg};
use crate::sink::{FrameSink, Output, PngSink, WebSink, WorldShape};

use present::VoxelPresenter;
use project::Projection;

/// The stdin commands, in the one place both the banner and the usage line read them
/// from, so a new command cannot be added to only one of the two.
const COMMANDS: &str = "p pause/resume, s step, +/- speed, r [m3] rain, \
                        m X Y Z air|rock|soil|bedrock set material, w PATH save, \
                        l PATH load, i X Y Z inspect, o outlet, q quit";

/// Default rain volume for the `r` command, in cubic metres.
const DEFAULT_RAIN_M3: f64 = 1.0;
/// `--speed` is clamped to this range; zero is `p` (pause), not a speed.
const MIN_SPEED: f64 = 1.0 / 64.0;
const MAX_SPEED: f64 = 64.0;

// --- The config file -----------------------------------------------------------------

/// `--config`: the presentation, plus the world to build. Missing fields take the
/// defaults below and unknown fields are errors, so a typo is not silently ignored.
///
/// ```toml
/// tilt_degrees = 30.0   # the chosen camera
/// px_per_voxel = 4      # the chosen camera
/// raster_height = 0     # 0 derives it from the world
/// haze = 0.55
/// water_alpha = 0.5
///
/// [world]
/// width = 128
/// height = 48
/// # depth is deliberately absent: it comes from `cubarium_voxel::Config::default()`, so
/// # the habitat's chosen depth lives in the core and the presenter never pins its own.
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VoxelConfig {
    /// Elevation of the orthographic view, in degrees. Reaches the picture only as
    /// `round(px_per_voxel · tan(tilt))` whole pixels of lift per voxel of depth. The
    /// chosen camera is 30°.
    pub tilt_degrees: f64,
    /// Pixels per voxel edge. The chosen camera is 4: below that the sprite atlas stops
    /// reading.
    pub px_per_voxel: u32,
    /// Ring raster height in pixels; `0` derives it so the whole strip fits. The width is
    /// always `world.width · px_per_voxel`, so the strip fills it exactly.
    pub raster_height: u16,
    /// How far the back wall fades toward the haze colour, `0..=1`.
    pub haze: f32,
    /// Opacity of one voxel of free water, `0..=1`.
    pub water_alpha: f32,
    /// The world to build. Every field optional, every default the core's own — the
    /// habitat's extent, `depth` included, is the core's decision and not the presenter's.
    pub world: cubarium_voxel::Config,
}

impl Default for VoxelConfig {
    fn default() -> VoxelConfig {
        VoxelConfig {
            // The camera chosen by the storyboard study: 30°, 4 px per voxel.
            tilt_degrees: 30.0,
            px_per_voxel: 4,
            raster_height: 0,
            haze: 0.55,
            water_alpha: 0.5,
            // Including the depth: `cubarium_voxel` owns how deep the habitat is.
            world: cubarium_voxel::Config::default(),
        }
    }
}

/// `[world]`: [`cubarium_voxel::Config`] with every field optional.
///
/// Load a [`VoxelConfig`] from TOML.
pub fn load_config(path: &Path) -> Result<VoxelConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the voxel config {}", path.display()))?;
    toml::from_str(&text)
        .with_context(|| format!("parsing the voxel config {}", path.display()))
}

// --- The run -------------------------------------------------------------------------

/// Run `cubarium voxel`.
pub fn run_voxel(args: &Voxel, stop: &AtomicBool) -> Result<()> {
    let cfg = match &args.config {
        Some(path) => load_config(path)?,
        None => VoxelConfig::default(),
    };
    if args.sink == VoxelSinkArg::Png && args.seconds <= 0.0 {
        bail!("`--sink png` needs `--seconds N`, or it would capture until interrupted");
    }
    // `World::new` and `World::empty` panic on a config no world can be built from —
    // building one from nonsense is a programming error. A config *file* is input, so the
    // boundary is here: a bad file is reported, not a backtrace.
    cfg.world
        .validate()
        .with_context(|| match &args.config {
            Some(path) => format!("the `[world]` in {}", path.display()),
            None => "the built-in world defaults".to_string(),
        })?;

    let mut world = match &args.load {
        Some(path) => {
            let bytes = std::fs::read(path)
                .with_context(|| format!("reading the world {}", path.display()))?;
            World::load(&bytes).with_context(|| format!("loading {}", path.display()))?
        }
        None => {
            let world_cfg = cfg.world.clone();
            match args.scene {
                VoxelSceneArg::Authored => scene::authored(world_cfg),
                VoxelSceneArg::Generated => World::new(world_cfg),
            }
        }
    };

    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, world.config())?;
    let topology = Topology::Ring { w: proj.raster_w, h: proj.raster_h };
    let shape = WorldShape::new(topology, Scale::ONE);

    let speed = args.speed.clamp(MIN_SPEED, MAX_SPEED);
    let mut sink: Box<dyn FrameSink> = match args.sink {
        VoxelSinkArg::Png => Box::new(PngSink::new(&args.out, args.every)?),
        VoxelSinkArg::Web => Box::new(WebSink::with_world(
            args.web_port,
            "voxel strip",
            crate::sink::web::Source { speed, ..Default::default() },
            None,
            shape,
        )?),
    };

    let c = world.config();
    eprintln!(
        "cubarium voxel: {} world {}x{}x{} at {} m/voxel -> ring:{}x{} \
         ({} px/voxel, tilt {:.0} deg, depth step {} px)",
        match (&args.load, args.scene) {
            (Some(p), _) => p.display().to_string(),
            (None, VoxelSceneArg::Authored) => "authored".to_string(),
            (None, VoxelSceneArg::Generated) => "generated".to_string(),
        },
        c.width,
        c.height,
        c.depth,
        c.voxel_m,
        proj.raster_w,
        proj.raster_h,
        proj.s,
        cfg.tilt_degrees,
        proj.rise,
    );
    eprintln!("cubarium voxel: stdin commands — {COMMANDS}");

    let mut presenter = VoxelPresenter::new(cfg, proj);
    let mut canvas = Canvas::new(topology, Scale::ONE);
    let mut raster = shape.raster().expect("a ring topology has a raster");
    let commands = spawn_stdin_reader();

    let limit = (args.seconds > 0.0).then(|| Duration::from_secs_f64(args.seconds));
    let start = Instant::now();
    let mut clock = Clock::with_fps(start, args.fps);
    let (mut ticks, mut frames) = (0u64, 0u64);
    let mut ctl = Control::new(speed, proj);
    let mut debt = 0.0f64;

    while !ctl.quit {
        let now = Instant::now();
        if let Some(l) = limit
            && clock.elapsed(now) >= l
        {
            break;
        }
        if sink.should_quit() || stop.load(Ordering::Relaxed) {
            break;
        }

        while let Ok(line) = commands.try_recv() {
            ctl.handle(&mut world, &line);
        }
        if ctl.quit {
            break;
        }

        match clock.next_step(now) {
            Step::Tick => {
                if ctl.paused {
                    // A single `s` still advances exactly one world tick.
                    if ctl.pending_steps > 0 {
                        ctl.pending_steps -= 1;
                        world.step();
                        ticks += 1;
                    }
                } else {
                    debt += ctl.speed;
                    while debt >= 1.0 {
                        debt -= 1.0;
                        world.step();
                        ticks += 1;
                    }
                }
                sink.observe_tick(world.tick());
            }
            Step::Render { .. } => {
                presenter.draw(&world.view(), &mut canvas);
                canvas.encode_raster(&mut raster);
                sink.submit(Output::Ring(&raster))?;
                frames += 1;
            }
            Step::Sleep(d) => std::thread::sleep(d),
            Step::Lagged { behind, log } => {
                if log {
                    eprintln!(
                        "cubarium voxel: behind by {:.0} ms; dropping render work",
                        behind.as_secs_f64() * 1e3
                    );
                }
            }
            Step::Paused { gap } => {
                eprintln!("cubarium voxel: clock re-based after a {:.1} s pause", gap.as_secs_f64());
            }
        }
    }

    sink.finish()?;
    let elapsed = clock.elapsed(Instant::now()).as_secs_f64();
    let view = world.view();
    eprintln!(
        "cubarium voxel: {ticks} ticks, {frames} frames in {elapsed:.2} s ({:.1} fps); \
         stored {:.3} m3, residual {:.3e} m3",
        frames as f64 / elapsed.max(1e-9),
        view.stored_m3(),
        view.stored_m3() - view.ledger.expected_stored(),
    );
    Ok(())
}

/// The run state a stdin command may change, and the one function that changes it.
///
/// It owns the scalars and borrows the world per call rather than the other way round,
/// so the loop can read `paused` and `speed` while it still holds the world.
struct Control {
    speed: f64,
    paused: bool,
    pending_steps: u32,
    outlet: bool,
    quit: bool,
    proj: Projection,
}

impl Control {
    fn new(speed: f64, proj: Projection) -> Control {
        Control { speed, paused: false, pending_steps: 0, outlet: false, quit: false, proj }
    }

    /// Apply one stdin line. Anything unrecognised prints the usage and changes nothing;
    /// a bad argument is reported and the world is left alone.
    fn handle(&mut self, world: &mut World, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let mut parts = line.split_whitespace();
        let verb = parts.next().unwrap_or_default();
        let rest: Vec<&str> = parts.collect();

        match verb {
            "q" | "quit" => self.quit = true,
            "p" | "pause" => {
                self.paused = !self.paused;
                eprintln!("cubarium voxel: {}", if self.paused { "paused" } else { "running" });
            }
            "s" | "step" => {
                self.paused = true;
                self.pending_steps += 1;
            }
            "+" => {
                self.speed = (self.speed * 2.0).min(MAX_SPEED);
                eprintln!("cubarium voxel: speed {}", self.speed);
            }
            "-" => {
                self.speed = (self.speed / 2.0).max(MIN_SPEED);
                eprintln!("cubarium voxel: speed {}", self.speed);
            }
            "r" | "rain" => {
                let volume = match rest.first() {
                    None => DEFAULT_RAIN_M3,
                    Some(text) => match text.parse::<f64>() {
                        Ok(v) if v.is_finite() && v > 0.0 => v,
                        _ => {
                            eprintln!("cubarium voxel: `r [m3]` wants a positive volume");
                            return;
                        }
                    },
                };
                let took = world.apply(VoxelCommand::RainPulse { volume_m3: volume });
                eprintln!("cubarium voxel: rain {volume} m3, accepted {took} m3");
            }
            "o" | "outlet" => {
                self.outlet = !world.outlet_open();
                world.apply(VoxelCommand::SetOutlet { open: self.outlet });
                eprintln!(
                    "cubarium voxel: outlet {}",
                    if self.outlet { "open" } else { "closed" }
                );
            }
            "w" | "save" => match rest.first() {
                Some(path) => match std::fs::write(path, world.save()) {
                    Ok(()) => eprintln!("cubarium voxel: saved {path}"),
                    Err(e) => eprintln!("cubarium voxel: saving {path}: {e}"),
                },
                None => eprintln!("cubarium voxel: `w PATH` needs a path"),
            },
            "l" | "load" => match rest.first() {
                Some(path) => match std::fs::read(path).map_err(anyhow::Error::from).and_then(|b| World::load(&b)) {
                    Ok(loaded) => {
                        // The projection is fixed for the life of the run, so a world of
                        // a different shape would draw into the wrong raster.
                        let c = loaded.config();
                        if (c.width, c.height, c.depth)
                            != (self.proj.width, self.proj.height, self.proj.depth)
                        {
                            eprintln!(
                                "cubarium voxel: {path} is {}x{}x{}, this run draws {}x{}x{}; \
                                 restart with a matching config",
                                c.width, c.height, c.depth,
                                self.proj.width, self.proj.height, self.proj.depth
                            );
                        } else {
                            *world = loaded;
                            self.outlet = world.outlet_open();
                            eprintln!("cubarium voxel: loaded {path} at tick {}", world.tick());
                        }
                    }
                    Err(e) => eprintln!("cubarium voxel: loading {path}: {e:#}"),
                },
                None => eprintln!("cubarium voxel: `l PATH` needs a path"),
            },
            "i" | "inspect" => {
                let Some((x, y, z)) = coords(world, "i X Y Z", &rest) else { return };
                eprintln!("cubarium voxel: {}", cell_state(world, x, y, z));
            }
            "m" | "material" => {
                let usage = "m X Y Z air|rock|soil|bedrock";
                let Some((x, y, z)) = coords(world, usage, &rest) else { return };
                let material = match rest.get(3).map(|w| w.to_ascii_lowercase()).as_deref() {
                    Some("air") => Material::Air,
                    Some("rock") => Material::Rock,
                    Some("soil") => Material::Soil,
                    Some("bedrock") => Material::Bedrock,
                    other => {
                        eprintln!(
                            "cubarium voxel: `{usage}` — `{}` is not a material",
                            other.unwrap_or("")
                        );
                        return;
                    }
                };
                // The core moves the water the old material held; the new state is the
                // receipt, so a displaced cell says so itself.
                world.apply(VoxelCommand::SetMaterial { x, y, z, material });
                eprintln!(
                    "cubarium voxel: set {material:?}, now {}",
                    cell_state(world, x, y, z)
                );
            }
            other => eprintln!("cubarium voxel: `{other}`? — {COMMANDS}"),
        }
    }
}

/// `X Y Z` from a command's arguments, or `None` with the reason printed.
///
/// `x` wraps — the strip is a ring and has no end to fall off — so any integer is a
/// column. `y` and `z` have real ends, so one outside the world is refused and named
/// rather than folded into a cell the caller did not mean.
fn coords(world: &World, usage: &str, rest: &[&str]) -> Option<(i64, u32, u32)> {
    let triple = (
        rest.first().and_then(|t| t.parse::<i64>().ok()),
        rest.get(1).and_then(|t| t.parse::<u32>().ok()),
        rest.get(2).and_then(|t| t.parse::<u32>().ok()),
    );
    let (Some(x), Some(y), Some(z)) = triple else {
        eprintln!("cubarium voxel: `{usage}` wants three integers for X Y Z");
        return None;
    };
    let c = world.config();
    if y >= c.height || z >= c.depth {
        eprintln!(
            "cubarium voxel: ({x}, {y}, {z}) is outside a {}x{}x{} world",
            c.width, c.height, c.depth
        );
        return None;
    }
    Some((x, y, z))
}

/// One cell's state as `i` reports it, and as `m` echoes back after an edit.
fn cell_state(world: &World, x: i64, y: u32, z: u32) -> String {
    let view = world.view();
    format!(
        "({}, {y}, {z}) {:?} free {:.4} pore {:.4}",
        x.rem_euclid(i64::from(view.config.width)),
        view.material_at(x, y, z),
        view.free_at(x, y, z),
        view.pore_at(x, y, z),
    )
}

/// Read stdin lines on a thread and hand them to the loop.
///
/// Detached on purpose: a blocking `read_line` cannot be cancelled portably, and the
/// process is about to exit anyway. When stdin closes the thread ends and the loop
/// simply stops getting commands.
fn spawn_stdin_reader() -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("cubarium-voxel-stdin".into())
        .spawn(move || {
            use std::io::BufRead;
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(line) => {
                        if tx.send(line).is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
        })
        .expect("spawning the voxel stdin reader");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The documented defaults, and a partial config file that only overrides some of
    /// them: everything else keeps the default rather than being refused.
    #[test]
    fn the_config_defaults_are_the_documented_ones_and_a_partial_file_loads() {
        let d = VoxelConfig::default();
        assert_eq!(d.tilt_degrees, 30.0);
        assert_eq!(d.px_per_voxel, 4);
        assert_eq!(d.raster_height, 0);
        assert_eq!(d.haze, 0.55);
        assert_eq!(d.water_alpha, 0.5);
        assert_eq!(d.world, cubarium_voxel::Config::default());

        let cfg: VoxelConfig = toml::from_str(
            "tilt_degrees = 35.0\n[world]\nwidth = 64\ndepth = 8\n",
        )
        .unwrap();
        assert_eq!(cfg.tilt_degrees, 35.0);
        assert_eq!(cfg.px_per_voxel, 4, "an unmentioned field keeps its default");
        assert_eq!(cfg.world.width, 64);
        assert_eq!(cfg.world.depth, 8);
        assert_eq!(cfg.world.height, cubarium_voxel::Config::default().height);

        // A typo is an error, not a silently ignored key.
        assert!(toml::from_str::<VoxelConfig>("tilt_degree = 35.0\n").is_err());
        assert!(toml::from_str::<VoxelConfig>("[world]\nwidht = 64\n").is_err());
    }

    /// The committed example config parses, is the default picture, and does not pin the
    /// world depth: that follows `cubarium_voxel::Config::default()`, so changing the
    /// habitat's depth in the core changes the example without editing it.
    #[test]
    fn the_example_config_parses_and_leaves_the_depth_to_the_core() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("voxel.example.toml");
        let cfg = load_config(&path).unwrap();
        assert_eq!(cfg, VoxelConfig::default());
        assert_eq!(cfg.world.depth, cubarium_voxel::Config::default().depth);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.lines().any(|l| l.trim_start().starts_with("depth")),
            "the example must not set `depth`; it mirrors the core default"
        );
        // The chosen camera, written down where a reader of the file will find it.
        assert!(text.contains("tilt_degrees = 30.0") && text.contains("px_per_voxel = 4"));
    }

    /// A config file whose `[world]` no world can be built from is an error, not a panic:
    /// `World::new` asserts, so the run validates first.
    #[test]
    fn a_world_no_world_can_be_built_from_is_refused_not_panicked_on() {
        let cfg: VoxelConfig =
            toml::from_str("[world]\ndepth = 0\n").expect("it parses; it is just impossible");
        let err = cfg.world.validate().expect_err("a zero dimension must be refused");
        assert!(format!("{err}").contains("nonzero dimensions"), "{err}");

        let bad: VoxelConfig = toml::from_str("[world]\nvoxel_m = 0.0\n").unwrap();
        assert!(bad.world.validate().is_err());
        assert!(VoxelConfig::default().world.validate().is_ok());
    }

    /// Stdin commands reach the world: pause toggles, speed halves and doubles, a
    /// single step is owed, and a nonsense line changes nothing.
    #[test]
    fn stdin_commands_change_exactly_what_they_name() {
        let c = cubarium_voxel::Config { width: 16, height: 8, depth: 2, ..Default::default() };
        let mut world = World::empty(c.clone());
        let mut ctl = Control::new(1.0, Projection::new(30.0, 4, 0, &c).unwrap());

        ctl.handle(&mut world, "p");
        assert!(ctl.paused);
        ctl.handle(&mut world, "p");
        assert!(!ctl.paused);
        ctl.handle(&mut world, "-");
        assert_eq!(ctl.speed, 0.5);
        ctl.handle(&mut world, "+");
        ctl.handle(&mut world, "+");
        assert_eq!(ctl.speed, 2.0);
        ctl.handle(&mut world, "s");
        assert!(ctl.paused && ctl.pending_steps == 1);
        ctl.handle(&mut world, "o");
        assert!(ctl.outlet);
        // Nonsense, and a bad argument, change nothing.
        ctl.handle(&mut world, "nonsense 1 2 3");
        ctl.handle(&mut world, "r not-a-volume");
        ctl.handle(&mut world, "i 0 999 0");
        ctl.handle(&mut world, "i only-one");
        ctl.handle(&mut world, "w");
        assert_eq!((ctl.speed, ctl.pending_steps, ctl.outlet), (2.0, 1, true));
        assert!(!ctl.quit);
        ctl.handle(&mut world, "i 0 0 0");
        ctl.handle(&mut world, "q");
        assert!(ctl.quit);
    }

    /// Terrain editing from stdin: paused, `m` changes one cell, and the `i` that follows
    /// reads the new material off the same view it prints from. The interaction the local
    /// tool was missing — you can now dig a channel and watch the pool find it.
    #[test]
    fn a_paused_terrain_edit_changes_the_cell_the_next_inspect_reports() {
        let c = cubarium_voxel::Config { width: 16, height: 8, depth: 2, ..Default::default() };
        let mut world = World::empty(c.clone());
        let mut ctl = Control::new(1.0, Projection::new(30.0, 4, 0, &c).unwrap());

        ctl.handle(&mut world, "p");
        assert!(ctl.paused);
        assert_eq!(world.view().material_at(3, 2, 1), Material::Air);

        ctl.handle(&mut world, "m 3 2 1 soil");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Soil);
        // What the following `i` prints is this, off the edited world.
        let seen = cell_state(&world, 3, 2, 1);
        assert!(seen.contains("(3, 2, 1) Soil"), "{seen}");
        ctl.handle(&mut world, "i 3 2 1");

        // `x` wraps, as everything in the ring does.
        ctl.handle(&mut world, "m -13 2 1 rock");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Rock);

        // A `y` or `z` outside the world, an unknown material and a short line are all
        // refused, and leave the cell alone.
        for bad in ["m 3 99 1 air", "m 3 2 9 air", "m 3 2 1 lava", "m 3 2 1", "m", "material"] {
            ctl.handle(&mut world, bad);
            assert_eq!(world.view().material_at(3, 2, 1), Material::Rock, "`{bad}` edited a cell");
        }

        ctl.handle(&mut world, "m 3 2 1 air");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Air);
        // Both help texts really do offer the command that was just used.
        assert!(COMMANDS.contains("m X Y Z air|rock|soil|bedrock"));
    }

    /// `w` then `l` round-trips the world through the snapshot the core writes, and a
    /// world of a different shape is refused rather than drawn into the wrong raster.
    #[test]
    fn save_and_load_round_trip_and_a_mismatched_world_is_refused() {
        let c = cubarium_voxel::Config { width: 16, height: 8, depth: 2, ..Default::default() };
        let mut world = scene::authored(c.clone());
        let proj = Projection::new(30.0, 4, 0, &c).unwrap();
        let dir = std::env::temp_dir().join(format!("cubarium-voxel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("world.bin");

        let before = world.view().stored_m3();
        let mut ctl = Control::new(1.0, proj);
        ctl.handle(&mut world, &format!("w {}", path.display()));
        assert!(path.exists());

        let mut fresh = World::empty(c.clone());
        ctl.handle(&mut fresh, &format!("l {}", path.display()));
        assert!((fresh.view().stored_m3() - before).abs() < 1e-9, "the pool came back");

        // A world of another shape is named and refused.
        let other = dir.join("other.bin");
        let wide = cubarium_voxel::Config { width: 32, ..c.clone() };
        std::fs::write(&other, World::empty(wide).save()).unwrap();
        let mut keep = World::empty(c.clone());
        ctl.handle(&mut keep, &format!("l {}", other.display()));
        assert_eq!(keep.config().width, 16, "the run kept its own world");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
