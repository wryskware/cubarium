//! `cubarium run`: the persistent world loop.
//!
//! The loop owns nothing the world needs: it advances `World::step`, hands encoded
//! snapshots to the checkpoint worker, appends telemetry, and — unless headless — turns
//! each published [`RenderView`] into one `Canvas::encode` per rendered frame. Wall time
//! enters only through [`crate::clock::Clock`].
//!
//! Stopping. A clean stop — the simulated `--seconds` limit, or the preview window being
//! closed — always writes a final snapshot before the process exits. This iteration
//! installs no SIGINT handler (that would need a signal crate, and the host's dependency
//! set is fixed), so Ctrl-C terminates the process immediately and the world resumes
//! from the last checkpoint instead: `capacity.checkpoint_seconds` is the recovery bound
//! for an unclean stop, and nothing else is lost, because every snapshot is written
//! atomically.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cube_proto::Frame;
use cubarium_core::view::RenderView;
use cubarium_core::{Telemetry, World, WorldConfig, encode_snapshot};
use cubarium_render::Canvas;

use crate::cli::{Run, RunSinkArg};
use crate::clock::{Clock, Step, TICK_HZ};
use crate::present::Presenter;
use crate::sink::{FrameSink, PngSink, PreviewSink, ShimSink};
use crate::state::{self, Checkpointer};

/// What one `run` produced. Returned so tests can drive the host through the library
/// instead of a subprocess.
#[derive(Clone, Debug)]
pub struct RunOutcome {
    /// The tick the world was at when this run started (nonzero when resumed).
    pub start_tick: u64,
    pub final_tick: u64,
    pub population: usize,
    /// FNV-1a over the postcard encoding of the final state.
    pub state_hash: u64,
    pub mass_residual: f64,
    pub telemetry_samples: u64,
    pub checkpoints_queued: u64,
    pub frames: u64,
    /// The snapshot this run resumed from, if any.
    pub loaded_from: Option<PathBuf>,
    pub loaded_tick: Option<u64>,
    /// The config used, after the precedence rules.
    pub config: WorldConfig,
}

/// How the simulation is paced against the wall clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pace {
    /// `--speed 0`: no clock, no sleeping, no rendering.
    Unlimited,
    /// `round(speed)` simulation ticks per wall tick (`--speed N >= 0.5`).
    TicksPerWallTick(u64),
    /// One simulation tick every `round(1 / speed)` wall ticks (`0 < speed < 0.5`).
    WallTicksPerTick(u64),
}

impl Pace {
    fn of(speed: f64) -> Pace {
        if speed <= 0.0 {
            return Pace::Unlimited;
        }
        let per_wall_tick = speed.round();
        if per_wall_tick >= 1.0 {
            Pace::TicksPerWallTick(per_wall_tick as u64)
        } else {
            // Below half speed, `round(speed)` is zero and the world would never advance;
            // slow the wall tick down instead of rounding the simulation to a halt.
            Pace::WallTicksPerTick((1.0 / speed).round().max(1.0) as u64)
        }
    }
}

/// Simulated seconds to whole ticks, rounded up so a cadence is never zero.
fn ticks_of(seconds: f64) -> u64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 1;
    }
    (seconds * f64::from(TICK_HZ)).ceil() as u64
}

/// Load a `WorldConfig` from TOML. Missing fields take defaults and unknown fields are
/// errors, both by the config's own serde attributes.
pub fn load_config(path: &Path) -> Result<WorldConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the config {}", path.display()))?;
    let config: WorldConfig = toml::from_str(&text)
        .with_context(|| format!("parsing the config {}", path.display()))?;
    config
        .validate()
        .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    Ok(config)
}

/// Apply the README's precedence: a loaded world's config wins over `--config` except
/// for `capacity` and `weather.moving`, which are operational.
fn merge_operational(loaded: &mut WorldConfig, from_file: &WorldConfig) {
    loaded.capacity = from_file.capacity.clone();
    loaded.weather.moving = from_file.weather.moving;
}

/// Append one JSON line per telemetry sample and flush, so an interrupted run still
/// leaves every sample it reported on disk.
struct TelemetryLog {
    path: PathBuf,
    file: std::fs::File,
    samples: u64,
}

impl TelemetryLog {
    fn open(path: &Path) -> Result<TelemetryLog> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating the telemetry directory {}", parent.display()))?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening the telemetry file {}", path.display()))?;
        Ok(TelemetryLog { path: path.to_path_buf(), file, samples: 0 })
    }

    /// A telemetry write failure is an observer problem, never a reason to stop the world.
    fn write(&mut self, sample: &Telemetry) {
        let line = match serde_json::to_string(sample) {
            Ok(line) => line,
            Err(e) => {
                eprintln!("cubarium: cannot encode telemetry: {e}");
                return;
            }
        };
        if let Err(e) = writeln!(self.file, "{line}").and_then(|()| self.file.flush()) {
            eprintln!("cubarium: cannot append to {}: {e}", self.path.display());
            return;
        }
        self.samples += 1;
    }
}

/// The one-line stderr digest headless runs get for every sample, so a `--sink none` run
/// is observable without opening the telemetry file.
fn headless_line(sample: &Telemetry) -> String {
    format!(
        "cubarium: tick {} pop {} births {} deaths {} residual {:.3e} hash {:016x}",
        sample.tick,
        sample.population,
        sample.births,
        sample.deaths_starvation + sample.deaths_age + sample.deaths_collapse,
        sample.mass_residual,
        sample.state_hash,
    )
}

/// Start (or resume) the world named by `run`'s options, obeying the config precedence.
fn open_world(run: &Run) -> Result<(World, Option<PathBuf>, Option<u64>)> {
    let from_file = match &run.config {
        Some(path) => Some(load_config(path)?),
        None => None,
    };

    if !run.fresh {
        let mut report = |path: &Path, failure: &state::LoadFailure| {
            eprintln!("cubarium: skipping {}: {failure}", path.display());
        };
        if let Some(loaded) = state::load_newest(&run.state, &mut report) {
            let state::Loaded { path, tick, mut state } = loaded;
            eprintln!("cubarium: resuming {} at tick {tick}", path.display());
            if let Some(file_config) = &from_file {
                merge_operational(&mut state.config, file_config);
            }
            let world = World::from_state(state)
                .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
            return Ok((world, Some(path), Some(tick)));
        }
        eprintln!("cubarium: no loadable snapshot in {}; creating a new world", run.state.display());
    }

    let mut config = from_file.unwrap_or_default();
    if let Some(seed) = run.seed {
        config.seed = seed;
    }
    let world = World::new(config).map_err(|e| anyhow::anyhow!("invalid world config: {e}"))?;
    Ok((world, None, None))
}

fn open_sink(run: &Run) -> Result<Option<Box<dyn FrameSink>>> {
    Ok(match run.sink {
        RunSinkArg::None => None,
        RunSinkArg::Preview => Some(Box::new(PreviewSink::new(run.scale, &run.out)?)),
        RunSinkArg::Shim => Some(Box::new(ShimSink::new(run.addr.clone()))),
        RunSinkArg::Png => Some(Box::new(PngSink::new(&run.out, run.every)?)),
    })
}

/// Run the persistent world. Fatal errors (an unwritable state directory, an invalid
/// config, a window that will not open) are returned; disk errors inside the checkpoint
/// worker and telemetry are logged and the world continues.
pub fn run_world(run: &Run) -> Result<RunOutcome> {
    run.validate()?;

    std::fs::create_dir_all(&run.state)
        .with_context(|| format!("creating the state directory {}", run.state.display()))?;

    let (mut world, loaded_from, loaded_tick) = open_world(run)?;
    let config = world.config().clone();
    let checkpoint_ticks = ticks_of(config.capacity.checkpoint_seconds);
    let telemetry_ticks = ticks_of(config.capacity.telemetry_seconds);

    let mut telemetry = TelemetryLog::open(&run.telemetry_path())?;
    let mut checkpoints = Checkpointer::spawn(&run.state);
    let build_id = state::build_id();

    let mut sink = open_sink(run)?;
    let headless = sink.is_none();
    let pace = Pace::of(run.speed);

    let start_tick = world.tick();
    let tick_limit = (run.seconds > 0.0).then(|| (run.seconds * f64::from(TICK_HZ)).round() as u64);

    let mut presenter = Presenter::new();
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    let mut view: Option<RenderView> = None;
    let mut frames = 0u64;
    let started = Instant::now();

    // Advance the world one tick and do everything that hangs off a completed tick.
    let mut ticks_done = 0u64;
    let advance = |world: &mut World,
                       presenter: &mut Presenter,
                       view: &mut Option<RenderView>,
                       telemetry: &mut TelemetryLog,
                       checkpoints: &mut Checkpointer,
                       ticks_done: &mut u64| {
        world.step();
        *ticks_done += 1;
        let tick = world.tick();
        if !headless {
            // Trails are simulated history: they are fed per tick, not per frame.
            let published = world.render_view();
            presenter.observe(&published);
            *view = Some(published);
        }
        // Cadences are anchored on the absolute tick, so a resumed world keeps the same
        // checkpoint and telemetry instants an uninterrupted one would have hit.
        if tick.is_multiple_of(telemetry_ticks) {
            let sample = world.telemetry();
            if headless {
                eprintln!("{}", headless_line(&sample));
            }
            telemetry.write(&sample);
        }
        if tick.is_multiple_of(checkpoint_ticks) {
            checkpoints.queue(tick, encode_snapshot(&world.state, &build_id));
        }
    };

    let reached = |ticks_done: u64| tick_limit.is_some_and(|l| ticks_done >= l);

    match pace {
        Pace::Unlimited => {
            while !reached(ticks_done) {
                advance(
                    &mut world,
                    &mut presenter,
                    &mut view,
                    &mut telemetry,
                    &mut checkpoints,
                    &mut ticks_done,
                );
            }
        }
        _ => {
            let mut clock = Clock::new(Instant::now());
            let mut wall_ticks = 0u64;
            loop {
                if reached(ticks_done) {
                    break;
                }
                if let Some(s) = sink.as_mut()
                    && s.should_quit()
                {
                    break;
                }
                match clock.next_step(Instant::now()) {
                    Step::Tick => {
                        wall_ticks += 1;
                        let due = match pace {
                            Pace::TicksPerWallTick(n) => n,
                            Pace::WallTicksPerTick(n) => u64::from(wall_ticks.is_multiple_of(n)),
                            Pace::Unlimited => 1,
                        };
                        for _ in 0..due {
                            if reached(ticks_done) {
                                break;
                            }
                            advance(
                                &mut world,
                                &mut presenter,
                                &mut view,
                                &mut telemetry,
                                &mut checkpoints,
                                &mut ticks_done,
                            );
                        }
                    }
                    Step::Render => {
                        if let (Some(s), Some(v)) = (sink.as_mut(), view.as_ref()) {
                            presenter.draw(v, &mut canvas);
                            // Exactly one encode per rendered frame; the identical bytes
                            // reach whichever sink is active.
                            canvas.encode(&mut frame);
                            s.submit(&frame)?;
                            frames += 1;
                        }
                    }
                    Step::Sleep(d) => std::thread::sleep(d),
                    Step::Lagged { behind, log } => {
                        if log {
                            eprintln!(
                                "cubarium: behind by {:.0} ms; dropping render work",
                                behind.as_secs_f64() * 1e3
                            );
                        }
                    }
                    Step::Paused { gap } => eprintln!(
                        "cubarium: clock re-based after a {:.1} s pause",
                        gap.as_secs_f64()
                    ),
                }
            }
        }
    }

    // A final telemetry sample is not written here: telemetry is a fixed cadence, and a
    // short run must not produce a sample an uninterrupted run would not have.
    let final_tick = world.tick();

    if let Some(s) = sink.as_mut() {
        s.finish()?;
    }
    // The clean-shutdown snapshot, always: the checkpoint interval is the recovery
    // bound only for an *un*clean stop.
    checkpoints.queue(final_tick, encode_snapshot(&world.state, &build_id));
    let checkpoints_queued = checkpoints.queued();
    checkpoints.shutdown(None);

    let outcome = RunOutcome {
        start_tick,
        final_tick,
        population: world.population(),
        state_hash: cubarium_core::snapshot::state_hash(&world.state),
        mass_residual: world.mass_residual(),
        telemetry_samples: telemetry.samples,
        checkpoints_queued,
        frames,
        loaded_from,
        loaded_tick,
        config,
    };

    let wall = started.elapsed().max(Duration::from_nanos(1));
    eprintln!(
        "cubarium: tick {final_tick} ({} ticks, {frames} frames in {:.2} s, {:.0}x real time), population {}, residual {:.3e}",
        final_tick - start_tick,
        wall.as_secs_f64(),
        (final_tick - start_tick) as f64 / f64::from(TICK_HZ) / wall.as_secs_f64(),
        outcome.population,
        outcome.mass_residual,
    );
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacing_follows_the_documented_rounding() {
        assert_eq!(Pace::of(0.0), Pace::Unlimited);
        assert_eq!(Pace::of(1.0), Pace::TicksPerWallTick(1));
        assert_eq!(Pace::of(20.0), Pace::TicksPerWallTick(20));
        assert_eq!(Pace::of(2.4), Pace::TicksPerWallTick(2));
        assert_eq!(Pace::of(2.6), Pace::TicksPerWallTick(3));
        assert_eq!(Pace::of(0.5), Pace::TicksPerWallTick(1));
        // Below half speed the wall tick slows instead of the simulation stalling.
        assert_eq!(Pace::of(0.25), Pace::WallTicksPerTick(4));
        assert_eq!(Pace::of(0.1), Pace::WallTicksPerTick(10));
    }

    #[test]
    fn cadences_never_round_to_zero_ticks() {
        assert_eq!(ticks_of(60.0), 1200);
        assert_eq!(ticks_of(5.0), 100);
        assert_eq!(ticks_of(0.01), 1);
        assert_eq!(ticks_of(0.0), 1);
        assert_eq!(ticks_of(f64::NAN), 1);
    }

    #[test]
    fn only_capacity_and_moving_weather_come_from_the_file() {
        let mut loaded = WorldConfig { seed: 99, ..WorldConfig::default() };
        loaded.producer.growth = 0.123;
        let mut file = WorldConfig::default();
        file.capacity.max_organisms = 64;
        file.capacity.checkpoint_seconds = 5.0;
        file.weather.moving = false;

        merge_operational(&mut loaded, &file);
        assert_eq!(loaded.seed, 99, "the loaded world keeps its own seed");
        assert_eq!(loaded.producer.growth, 0.123, "the loaded world keeps its own rates");
        assert_eq!(loaded.capacity.max_organisms, 64);
        assert_eq!(loaded.capacity.checkpoint_seconds, 5.0);
        assert!(!loaded.weather.moving);
    }

    #[test]
    fn a_config_with_an_unknown_field_is_rejected() {
        let dir = std::env::temp_dir().join(format!("cubarium-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.toml");
        std::fs::write(&path, "seed = 7\nnot_a_field = 1\n").unwrap();
        let err = load_config(&path).unwrap_err().to_string();
        assert!(err.contains("parsing"), "{err}");

        std::fs::write(&path, "seed = 7\n").unwrap();
        let cfg = load_config(&path).unwrap();
        assert_eq!(cfg.seed, 7);
        // Everything unmentioned is the default.
        assert_eq!(cfg.producer.growth, WorldConfig::default().producer.growth);

        // Values the world cannot use are rejected before any state is touched.
        std::fs::write(&path, "[capacity]\nmax_organisms = 0\n").unwrap();
        assert!(load_config(&path).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_headless_digest_names_the_numbers_the_contract_asks_for() {
        let sample = Telemetry {
            tick: 1200,
            population: 84,
            births: 3,
            deaths_age: 1,
            mass_residual: 1.5e-9,
            state_hash: 0xdead_beef,
            ..Telemetry::default()
        };
        let line = headless_line(&sample);
        for part in ["tick 1200", "pop 84", "births 3", "deaths 1", "residual", "hash"] {
            assert!(line.contains(part), "{line}");
        }
    }
}
