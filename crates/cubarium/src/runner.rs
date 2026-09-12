//! `cubarium run`: the persistent world loop.
//!
//! The loop owns nothing the world needs: it advances `World::step`, hands encoded
//! snapshots to the checkpoint worker, appends telemetry and (when
//! `capacity.field_dump_seconds` asks for them) field dumps, and — unless headless —
//! turns each published [`RenderView`] into one `Canvas::encode` per rendered frame.
//! Wall time enters only through [`crate::clock::Clock`].
//!
//! Stopping. A clean stop — the simulated `--seconds` limit, the preview window being
//! closed, or the shared stop flag being set — always writes a final snapshot before the
//! process exits. The binary sets that flag from a SIGINT handler (see [`crate::run`]),
//! so Ctrl-C leaves the loop between ticks and takes the ordinary shutdown path. The
//! flag is only ever read here, once per loop iteration; the loop never blocks for long
//! enough to make the latency visible. A stop that is *not* clean (a second Ctrl-C,
//! `SIGKILL`, power loss) still loses at most `capacity.checkpoint_seconds` of simulated
//! time, because every snapshot is written atomically.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cube_proto::Frame;
use cubarium_core::view::{FieldDump, RenderView};
use cubarium_core::{LifeEvent, Telemetry, World, WorldConfig, encode_snapshot};
use cubarium_render::Canvas;

use crate::cli::{Run, RunSinkArg};
use crate::clock::{Clock, Step, TICK_HZ};
use crate::present::Presenter;
use crate::sink::{FrameSink, PngSink, PreviewSink, ShimSink, WebSink};
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

/// Four decimals, the precision `design/m2-world-spec.md` "Observer" asks field dumps
/// for. Non-finite values become JSON `null` rather than a lie a reader cannot detect.
fn rounded(values: &[f64]) -> Vec<serde_json::Value> {
    values
        .iter()
        .map(|x| {
            serde_json::Number::from_f64((x * 1e4).round() / 1e4)
                .map_or(serde_json::Value::Null, serde_json::Value::Number)
        })
        .collect()
}

/// One field array as a JSON array of numbers. Written by hand rather than through a
/// map so the keys keep the spec's order (`tick, n, p, d, de, organisms`); `serde_json`
/// maps would sort them and put `tick` last, where nobody tailing the file expects it.
fn field_array(values: &[f64]) -> String {
    serde_json::Value::Array(rounded(values)).to_string()
}

/// `fields.jsonl`: a header naming each cell's graph neighbors, then one line per dump
/// on the telemetry cadence rule. Append-only like the telemetry log, and equally
/// non-fatal: a dump that cannot be written is reported and the world goes on.
struct FieldLog {
    path: PathBuf,
    file: std::fs::File,
    /// True when this process created (or found empty) the file, so it owes a header.
    fresh: bool,
    lines: u64,
}

impl FieldLog {
    fn open(path: &Path) -> Result<FieldLog> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating the field dump directory {}", parent.display()))?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening the field dump file {}", path.display()))?;
        // An existing file already carries its header; only a new or empty one is owed
        // one, because the neighbor graph never changes within a world.
        let fresh = file.metadata().map(|m| m.len() == 0).unwrap_or(true);
        Ok(FieldLog { path: path.to_path_buf(), file, fresh, lines: 0 })
    }

    fn append(&mut self, line: &str) {
        if let Err(e) = writeln!(self.file, "{line}").and_then(|()| self.file.flush()) {
            eprintln!("cubarium: cannot append to {}: {e}", self.path.display());
            return;
        }
        self.lines += 1;
    }

    /// `{"cells": [[n0, n1, n2, n3], …]}` in `Edge` order, `null` at the rim.
    fn write_header(&mut self, neighbors: &[[Option<u16>; 4]]) {
        let cells: Vec<serde_json::Value> = neighbors
            .iter()
            .map(|cell| {
                serde_json::Value::Array(
                    cell.iter()
                        .map(|n| n.map_or(serde_json::Value::Null, serde_json::Value::from))
                        .collect(),
                )
            })
            .collect();
        self.append(&serde_json::json!({ "cells": cells }).to_string());
    }

    fn write(&mut self, dump: &FieldDump) {
        let organisms = serde_json::Value::from(dump.organisms.as_slice()).to_string();
        self.append(&format!(
            r#"{{"tick":{},"n":{},"p":{},"d":{},"de":{},"organisms":{organisms}}}"#,
            dump.tick,
            field_array(&dump.n),
            field_array(&dump.p),
            field_array(&dump.d),
            field_array(&dump.de),
        ));
    }
}

/// The key order `design/m2-world-spec.md` "Observer" prints a birth record in. Keys the
/// world adds later are not dropped: they are appended after these.
const BIRTH_KEYS: [&str; 8] =
    ["kind", "tick", "id", "parent", "parent_age_ticks", "parent_births", "genome", "origin"];
/// The same for a death record.
const DEATH_KEYS: [&str; 7] = ["kind", "tick", "id", "age_ticks", "cause", "births", "genome"];

/// `slot:generation` for an `OrganismId` the world serialized as `{slot, generation}`.
/// Anything else (a string the world already renders itself, a missing field) is left
/// exactly as it came, so this can never invent an id.
fn organism_id(value: &serde_json::Value) -> Option<serde_json::Value> {
    let (slot, generation) = (value.get("slot")?, value.get("generation")?);
    Some(serde_json::Value::from(format!("{slot}:{generation}")))
}

/// One life event as the spec's line: ids as `slot:generation`, `origin` and `cause`
/// lowercase, and the spec's key order so a reader sees `kind` and `tick` first.
fn event_line(event: &LifeEvent) -> Result<String> {
    let value = serde_json::to_value(event).context("encoding a life event")?;
    let mut fields = match value {
        serde_json::Value::Object(map) => map,
        other => anyhow::bail!("a life event is not a JSON object: {other}"),
    };
    for key in ["id", "parent"] {
        if let Some(rendered) = fields.get(key).and_then(organism_id) {
            fields.insert(key.to_string(), rendered);
        }
    }
    for key in ["origin", "cause"] {
        if let Some(name) = fields.get(key).and_then(|v| v.as_str()).map(str::to_lowercase) {
            fields.insert(key.to_string(), serde_json::Value::from(name));
        }
    }
    let spec_order: &[&str] = match fields.get("kind").and_then(|v| v.as_str()) {
        Some("death") => &DEATH_KEYS,
        _ => &BIRTH_KEYS,
    };
    // Assembled as text: a `serde_json` map is sorted, which would bury `kind` and
    // `tick` in the middle of the line.
    let pair = |key: &str, value: &serde_json::Value| {
        format!("{}:{value}", serde_json::Value::from(key))
    };
    let mut parts = Vec::with_capacity(fields.len());
    for key in spec_order {
        if let Some(value) = fields.remove(*key) {
            parts.push(pair(key, &value));
        }
    }
    // Whatever the world grew since this list was written goes on the end, in key order,
    // rather than being silently lost.
    parts.extend(fields.iter().map(|(key, value)| pair(key, value)));
    Ok(format!("{{{}}}", parts.join(",")))
}

/// `events.jsonl`: one line per birth and death, appended in the order the world
/// reported them. Non-fatal like the other observer files.
struct EventLog {
    path: PathBuf,
    file: std::fs::File,
    lines: u64,
}

impl EventLog {
    fn open(path: &Path) -> Result<EventLog> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating the event log directory {}", parent.display()))?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening the event log {}", path.display()))?;
        Ok(EventLog { path: path.to_path_buf(), file, lines: 0 })
    }

    /// Write one tick's events and flush once. A tick with no events touches no disk.
    fn write_tick(&mut self, events: &[LifeEvent]) {
        if events.is_empty() {
            return;
        }
        let mut written = 0u64;
        for event in events {
            let line = match event_line(event) {
                Ok(line) => line,
                Err(e) => {
                    eprintln!("cubarium: cannot encode a life event: {e:#}");
                    continue;
                }
            };
            if let Err(e) = writeln!(self.file, "{line}") {
                eprintln!("cubarium: cannot append to {}: {e}", self.path.display());
                return;
            }
            written += 1;
        }
        if let Err(e) = self.file.flush() {
            eprintln!("cubarium: cannot flush {}: {e}", self.path.display());
            return;
        }
        self.lines += written;
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
        RunSinkArg::Web => Some(Box::new(WebSink::new(run.web_port)?)),
    })
}

/// Run the persistent world. Fatal errors (an unwritable state directory, an invalid
/// config, a window that will not open) are returned; disk errors inside the checkpoint
/// worker and telemetry are logged and the world continues.
pub fn run_world(run: &Run) -> Result<RunOutcome> {
    run_world_until(run, &AtomicBool::new(false))
}

/// [`run_world`], plus a stop flag any other thread may set to ask for a clean stop.
/// The binary hands this the flag its SIGINT handler sets; tests set it directly.
pub fn run_world_until(run: &Run, stop: &AtomicBool) -> Result<RunOutcome> {
    run.validate()?;

    std::fs::create_dir_all(&run.state)
        .with_context(|| format!("creating the state directory {}", run.state.display()))?;

    let (mut world, loaded_from, loaded_tick) = open_world(run)?;
    let config = world.config().clone();
    let checkpoint_ticks = ticks_of(config.capacity.checkpoint_seconds);
    let telemetry_ticks = ticks_of(config.capacity.telemetry_seconds);

    let mut telemetry = TelemetryLog::open(&run.telemetry_path())?;
    // `capacity.field_dump_seconds == 0` turns field dumps off entirely: no file is
    // created, and the world never pays for a dump it does not write.
    let field_ticks = ticks_of(config.capacity.field_dump_seconds);
    let mut fields = match config.capacity.field_dump_seconds > 0.0 {
        true => Some(FieldLog::open(&run.fields_path())?),
        false => None,
    };
    // `capacity.event_log` off means no file and no lines; the world's event buffer is
    // drained and dropped either way (see the loop).
    let mut events = match config.capacity.event_log {
        true => Some(EventLog::open(&run.events_path())?),
        false => None,
    };
    let mut checkpoints = Checkpointer::spawn(&run.state);
    let build_id = state::build_id();

    let mut sink = open_sink(run)?;
    let headless = sink.is_none();
    let pace = Pace::of(run.speed);

    let start_tick = world.tick();
    if let Some(log) = fields.as_mut()
        && log.fresh
    {
        log.write_header(&world.cell_neighbors());
        // The loop only ever dumps a tick it has just completed, so the world's opening
        // state would be missing from a file this run started. It is written here when
        // that tick is on the cadence — for a new world, tick 0. A run appending to an
        // existing dump file skips this, so a resumed world never repeats a tick.
        if start_tick.is_multiple_of(field_ticks) {
            log.write(&world.field_dump());
        }
    }
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
                       fields: &mut Option<FieldLog>,
                       events: &mut Option<EventLog>,
                       checkpoints: &mut Checkpointer,
                       ticks_done: &mut u64| {
        world.step();
        *ticks_done += 1;
        let tick = world.tick();
        // Drained every tick even when nothing logs them: `World` records births and
        // deaths whatever `capacity.event_log` says, and a buffer the host never takes
        // grows without bound for the life of the process. Written in the order the
        // world committed them.
        let committed = world.drain_events();
        if let Some(log) = events.as_mut() {
            log.write_tick(&committed);
        }
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
        if let Some(log) = fields.as_mut()
            && tick.is_multiple_of(field_ticks)
        {
            log.write(&world.field_dump());
        }
        if tick.is_multiple_of(checkpoint_ticks) {
            checkpoints.queue(tick, encode_snapshot(&world.state, &build_id));
        }
    };

    let reached = |ticks_done: u64| tick_limit.is_some_and(|l| ticks_done >= l);

    match pace {
        Pace::Unlimited => {
            while !reached(ticks_done) && !stop.load(Ordering::Relaxed) {
                advance(
                    &mut world,
                    &mut presenter,
                    &mut view,
                    &mut telemetry,
                    &mut fields,
                    &mut events,
                    &mut checkpoints,
                    &mut ticks_done,
                );
            }
        }
        _ => {
            let mut clock = Clock::with_fps(Instant::now(), run.fps);
            let mut wall_ticks = 0u64;
            loop {
                if reached(ticks_done) || stop.load(Ordering::Relaxed) {
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
                                &mut fields,
                                &mut events,
                                &mut checkpoints,
                                &mut ticks_done,
                            );
                        }
                    }
                    Step::Render { f } => {
                        if let (Some(s), Some(v)) = (sink.as_mut(), view.as_ref()) {
                            // `f` walks each body along the path of the last completed
                            // tick, so a 20 Hz world reads as continuous at `--fps`.
                            presenter.draw(v, f, &mut canvas);
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
    fn field_values_are_four_decimals_and_non_finite_values_are_null() {
        let v = rounded(&[0.0, 1.0 / 3.0, 0.123_449, 0.123_45, -2.5, f64::NAN, f64::INFINITY]);
        let n = |i: usize| v[i].as_f64();
        assert_eq!(n(0), Some(0.0));
        assert_eq!(n(1), Some(0.3333));
        assert_eq!(n(2), Some(0.1234));
        assert_eq!(n(3), Some(0.1235));
        assert_eq!(n(4), Some(-2.5));
        assert!(v[5].is_null(), "NaN must not be written as a number: {:?}", v[5]);
        assert!(v[6].is_null(), "an infinity must not be written as a number: {:?}", v[6]);
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
