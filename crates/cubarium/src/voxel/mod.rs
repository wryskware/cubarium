//! `cubarium voxel`: run a [`cubarium_voxel::World`] at a fixed 20 Hz and draw it as
//! pixel art into a ring raster, through the same sinks the rest of the host uses.
//!
//! The loop is [`crate::run::drive`]'s shape — one [`Clock`], one `encode_raster` per
//! rendered frame, one sink — with two differences: the world is a voxel strip rather
//! than the fixture scenes, and a stdin reader thread lets the run be poked while it is
//! running (pause, single-step, speed, rain, save, load, inspect, outlet, quit).
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
use cubarium_voxel::{Command as VoxelCommand, World};
use serde::{Deserialize, Serialize};

use crate::clock::{Clock, Step};
use crate::cli::{Voxel, VoxelSceneArg, VoxelSinkArg};
use crate::sink::{FrameSink, Output, PngSink, WebSink, WorldShape};

use present::VoxelPresenter;
use project::Projection;

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
/// tilt_degrees = 30.0
/// px_per_voxel = 4
/// raster_height = 0     # 0 derives it from the world
/// haze = 0.55
/// water_alpha = 0.5
///
/// [world]
/// width = 128
/// height = 48
/// depth = 16
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VoxelConfig {
    /// Elevation of the orthographic view, in degrees. Reaches the picture only as
    /// `round(px_per_voxel · tan(tilt))` whole pixels of lift per voxel of depth.
    pub tilt_degrees: f64,
    /// Pixels per voxel edge.
    pub px_per_voxel: u32,
    /// Ring raster height in pixels; `0` derives it so the whole strip fits. The width is
    /// always `world.width · px_per_voxel`, so the strip fills it exactly.
    pub raster_height: u16,
    /// How far the back wall fades toward the haze colour, `0..=1`.
    pub haze: f32,
    /// Opacity of one voxel of free water, `0..=1`.
    pub water_alpha: f32,
    pub world: WorldToml,
}

impl Default for VoxelConfig {
    fn default() -> VoxelConfig {
        VoxelConfig {
            tilt_degrees: 30.0,
            px_per_voxel: 4,
            raster_height: 0,
            haze: 0.55,
            water_alpha: 0.5,
            world: WorldToml::default(),
        }
    }
}

/// `[world]`: [`cubarium_voxel::Config`] with every field optional.
///
/// The fields are restated rather than nesting the core's `Config` directly because that
/// one has no serde defaults, so a partial `[world]` table would be refused for the
/// fields it left out. The conversion below is the only place the two are tied together.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldToml {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub voxel_m: f64,
    pub seed: u64,
    pub rain_m_per_s: f64,
    pub evaporation_m_per_s: f64,
    pub water_substeps: u32,
}

impl Default for WorldToml {
    fn default() -> WorldToml {
        WorldToml::from(&cubarium_voxel::Config::default())
    }
}

impl From<&cubarium_voxel::Config> for WorldToml {
    fn from(c: &cubarium_voxel::Config) -> WorldToml {
        WorldToml {
            width: c.width,
            height: c.height,
            depth: c.depth,
            voxel_m: c.voxel_m,
            seed: c.seed,
            rain_m_per_s: c.rain_m_per_s,
            evaporation_m_per_s: c.evaporation_m_per_s,
            water_substeps: c.water_substeps,
        }
    }
}

impl From<&WorldToml> for cubarium_voxel::Config {
    fn from(t: &WorldToml) -> cubarium_voxel::Config {
        cubarium_voxel::Config {
            width: t.width,
            height: t.height,
            depth: t.depth,
            voxel_m: t.voxel_m,
            seed: t.seed,
            rain_m_per_s: t.rain_m_per_s,
            evaporation_m_per_s: t.evaporation_m_per_s,
            water_substeps: t.water_substeps,
        }
    }
}

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

    let mut world = match &args.load {
        Some(path) => {
            let bytes = std::fs::read(path)
                .with_context(|| format!("reading the world {}", path.display()))?;
            World::load(&bytes).with_context(|| format!("loading {}", path.display()))?
        }
        None => {
            let world_cfg = cubarium_voxel::Config::from(&cfg.world);
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
    eprintln!(
        "cubarium voxel: stdin commands — p pause/resume, s step, +/- speed, \
         r [m3] rain, w PATH save, l PATH load, i X Y Z inspect, o outlet, q quit"
    );

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
                self.outlet = !self.outlet;
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
                            eprintln!("cubarium voxel: loaded {path} at tick {}", world.tick());
                        }
                    }
                    Err(e) => eprintln!("cubarium voxel: loading {path}: {e:#}"),
                },
                None => eprintln!("cubarium voxel: `l PATH` needs a path"),
            },
            "i" | "inspect" => {
                let parsed = (
                    rest.first().and_then(|t| t.parse::<i64>().ok()),
                    rest.get(1).and_then(|t| t.parse::<u32>().ok()),
                    rest.get(2).and_then(|t| t.parse::<u32>().ok()),
                );
                let Some((x, y, z)) = (match parsed {
                    (Some(x), Some(y), Some(z)) => Some((x, y, z)),
                    _ => None,
                }) else {
                    eprintln!("cubarium voxel: `i X Y Z` wants three integers");
                    return;
                };
                let view = world.view();
                if y >= view.config.height || z >= view.config.depth {
                    eprintln!(
                        "cubarium voxel: ({x}, {y}, {z}) is outside a {}x{}x{} world",
                        view.config.width, view.config.height, view.config.depth
                    );
                    return;
                }
                eprintln!(
                    "cubarium voxel: ({}, {y}, {z}) {:?} free {:.4} pore {:.4}",
                    x.rem_euclid(i64::from(view.config.width)),
                    view.material_at(x, y, z),
                    view.free_at(x, y, z),
                    view.pore_at(x, y, z),
                );
            }
            other => eprintln!(
                "cubarium voxel: `{other}`? — p pause/resume, s step, +/- speed, \
                 r [m3] rain, w PATH save, l PATH load, i X Y Z inspect, o outlet, q quit"
            ),
        }
    }
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
        assert_eq!(cubarium_voxel::Config::from(&d.world), cubarium_voxel::Config::default());

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

    /// The committed example config parses and is the default picture.
    #[test]
    fn the_example_config_parses() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("voxel.example.toml");
        let cfg = load_config(&path).unwrap();
        assert_eq!(cfg, VoxelConfig::default());
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
