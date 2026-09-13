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
use cubarium_core::care::{CareCommand, CareKind, CareOutcome, CareReceipt, CareTarget};
use cubarium_core::{LifeEvent, Telemetry, World, WorldConfig, encode_snapshot};
use cubarium_render::Canvas;

use crate::art::ArtPack;
use crate::care;
use crate::art_present::ArtPresenter;
use crate::cli::{Run, RunSinkArg};
use crate::clock::{Clock, Step, TICK_HZ};
use crate::present::Presenter;
use crate::sink::{FanOutSink, FrameSink, PngSink, PreviewSink, ShimSink, WebSink, web};
use crate::state::{self, Checkpointer};

#[path = "care_effects.rs"]
mod care_effects;

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
            r#"{{"tick":{},"n":{},"p":{},"d":{},"de":{},"f":{},"w":{},"organisms":{organisms}}}"#,
            dump.tick,
            field_array(&dump.n),
            field_array(&dump.p),
            field_array(&dump.d),
            field_array(&dump.de),
            field_array(&dump.f),
            field_array(&dump.w),
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
    // Forms are listed up to the last one alive, so a four-kind world prints four numbers
    // and a v1 world (all hue terciles) three, without eight trailing zeros.
    let last_form = sample
        .population_by_form
        .iter()
        .rposition(|&n| n > 0)
        .map_or(0, |i| i + 1);
    let forms = sample.population_by_form[..last_form]
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join("/");
    format!(
        "cubarium: tick {} pop {} births {} deaths {} forms {} fruit {:.2} water {:.1} residual {:.3e} hash {:016x}",
        sample.tick,
        sample.population,
        sample.births,
        sample.deaths_starvation + sample.deaths_age + sample.deaths_collapse,
        if forms.is_empty() { "-".to_string() } else { forms },
        sample.fruit,
        sample.water,
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
        // Every failure is collected as well as printed: whether *any* snapshot file was
        // present and simply would not load decides between "a new world" and an error.
        let mut failures: Vec<String> = Vec::new();
        let loaded = {
            let mut report = |path: &Path, failure: &state::LoadFailure| {
                eprintln!("cubarium: skipping {}: {failure}", path.display());
                failures.push(format!("{}: {failure}", path.display()));
            };
            state::load_newest(&run.state, &mut report)
        };
        if let Some(loaded) = loaded {
            let state::Loaded { path, tick, mut state } = loaded;
            eprintln!("cubarium: resuming {} at tick {tick}", path.display());
            if let Some(file_config) = &from_file {
                merge_operational(&mut state.config, file_config);
            }
            let world = World::from_state(state)
                .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
            return Ok((world, Some(path), Some(tick)));
        }
        // A directory holding snapshot files, none of which load, is a damaged world, not
        // an empty one. Starting a new world there would put tick 0 beside eight higher
        // ticks the pruner keeps in preference to it, and the operator would find their
        // world gone rather than merely unreadable. Only a directory with no snapshot
        // files at all is genuinely a new world.
        if !failures.is_empty() {
            anyhow::bail!(
                "{}: {} snapshot file(s) are present and none of them loaded:\n  {}\n\
                 This is a damaged world, not an empty directory, so no new world is created \
                 here. Recover or move those files aside deliberately, or choose a different \
                 --state directory.",
                run.state.display(),
                failures.len(),
                failures.join("\n  "),
            );
        }
        anyhow::ensure!(
            !run.require_resume,
            "--require-resume: {} holds no snapshot to resume from",
            run.state.display()
        );
        eprintln!("cubarium: no loadable snapshot in {}; creating a new world", run.state.display());
    }

    let mut config = from_file.unwrap_or_default();
    if let Some(seed) = run.seed {
        config.seed = seed;
    }
    let world = World::new(config).map_err(|e| anyhow::anyhow!("invalid world config: {e}"))?;
    Ok((world, None, None))
}

/// The read-only identity `--mirror-web`'s viewer serves at `/status`. Host facts only —
/// the pid, the state directory, the build, how the pixels also leave this process — none
/// of which the world knows about or can be changed through.
fn source_of(run: &Run, start_tick: u64, resumed_from: Option<&Path>) -> web::Source {
    // Canonical when the directory exists (it is created before this runs); a path that
    // cannot be canonicalized is still reported absolute rather than dropped.
    let state_dir = std::fs::canonicalize(&run.state)
        .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(&run.state)))
        .unwrap_or_else(|_| run.state.clone());
    web::Source {
        pid: std::process::id(),
        state_dir: state_dir.display().to_string(),
        build_id: state::build_id(),
        sink: run.sink.name().to_string(),
        resumed_from: resumed_from.map(|p| p.display().to_string()),
        start_tick,
        speed: run.speed,
    }
}

/// The primary sink, plus — with `--mirror-web` — the loopback viewer behind a
/// [`FanOutSink`]. The loop above still encodes exactly once per rendered frame; the
/// fan-out hands that one `&Frame` to both, so the cube and the browser are never looking
/// at different pixels.
fn open_sink(
    run: &Run,
    source: &web::Source,
    care: Option<std::sync::Arc<care::CareShared>>,
) -> Result<Option<Box<dyn FrameSink>>> {
    let primary: Box<dyn FrameSink> = match run.sink {
        RunSinkArg::None => return Ok(None),
        RunSinkArg::Preview => Box::new(PreviewSink::new(run.scale, &run.out)?),
        RunSinkArg::Shim => Box::new(ShimSink::new(run.addr.clone())),
        RunSinkArg::Png => Box::new(PngSink::new(&run.out, run.every)?),
        RunSinkArg::Web => Box::new(WebSink::with_care(
            run.web_port,
            speed_note(run.speed),
            source.clone(),
            care.clone(),
        )?),
    };
    if !run.mirror_web {
        return Ok(Some(primary));
    }
    let web = WebSink::with_care(run.web_port, speed_note(run.speed), source.clone(), care)?;
    // `--web-port 0` binds an ephemeral port, so the URL has to be reported to be usable.
    eprintln!("cubarium: mirroring the same frames to the viewer at {}", web.url());
    Ok(Some(Box::new(FanOutSink::new(vec![primary, Box::new(web)]))))
}

/// The viewer's HUD note for a `--speed`. `f64`'s own `Display` is the shortest decimal
/// that reads back as the same number, so this is `1× time`, `8× time`, `0.5× time`,
/// `2.5× time` — never `1.0×`. Without it a reviewer cannot tell a 1× world from an 8×
/// one by looking at it.
fn speed_note(speed: f64) -> String {
    format!("{speed}× time")
}

/// The presentation the run is using. The two presenters have the same two-method
/// contract, so the loop calls them at the same two sites and neither knows about the
/// other; `--art` is the only thing that chooses.
enum Show {
    /// The decided M2 image: procedural bodies, trails, feeding flash.
    Plain(Presenter),
    /// The authored sprite image, from a baked art pack.
    Art(Box<ArtPresenter>),
}

impl Show {
    /// Once per completed tick.
    fn observe(&mut self, view: &RenderView) {
        match self {
            Show::Plain(p) => p.observe(view),
            Show::Art(p) => p.observe(view),
        }
    }

    /// Once per rendered frame.
    fn draw(&mut self, view: &RenderView, f: f64, canvas: &mut Canvas) {
        match self {
            Show::Plain(p) => p.draw(view, f, canvas),
            Show::Art(p) => p.draw(view, f, canvas),
        }
    }
}

/// `--art <dir>` loads the baked pack; without it the image is the decided M2 one,
/// pixel for pixel. A pack that will not load is fatal: a run that silently fell back
/// to discs would be a review of the wrong image.
fn open_show(run: &Run) -> Result<Show> {
    match &run.art {
        None => Ok(Show::Plain(Presenter::new())),
        Some(dir) => {
            let pack = ArtPack::load(dir)
                .with_context(|| format!("loading the art pack {}", dir.display()))?;
            Ok(Show::Art(Box::new(ArtPresenter::new(pack))))
        }
    }
}

// --- optional care -------------------------------------------------------------------

/// This run's epoch: the stamp the journal's first record carries and every client
/// identity embeds. Nanoseconds plus the pid, so two runs a millisecond apart on the same
/// machine still retire each other's identities.
fn care_epoch() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}-{}", std::process::id())
}

/// One command as the core wants it. The host keeps whole-pixel targets because that is
/// what a click on the net *is*; the core takes chart coordinates as `f64`.
fn to_core(command: &care::PlannedCommand) -> CareCommand {
    CareCommand {
        seq: command.seq,
        apply_after_tick: command.apply_after_tick,
        kind: match command.kind {
            care::CareKind::Feed => CareKind::Feed,
            care::CareKind::Rain => CareKind::Rain,
            care::CareKind::Clean => CareKind::Clean,
        },
        target: CareTarget {
            face: command.target.face,
            u: f64::from(command.target.u),
            v: f64::from(command.target.v),
        },
    }
}

/// The receipt's quantities, for the journal's diagnostic record and for the viewer.
fn applied_json(outcome: &CareOutcome) -> serde_json::Value {
    match outcome.applied() {
        None => serde_json::Value::Null,
        Some(q) => serde_json::json!({
            "material_in": q.material_in,
            "energy_in": q.energy_in,
            "water_depth": q.water_depth,
            "material_out": q.material_out,
            "energy_out": q.energy_out,
            "cells": q.cells,
            "ends_tick": q.ends_tick,
        }),
    }
}

/// Everything `--care` adds to the loop: the service the HTTP server talks to, the journal
/// worker, the recovered schedule, and the hold.
///
/// The whole of the contract's admission protocol lives in [`CareRuntime::boundary`],
/// which the loop calls at each tick boundary and obeys: while it answers `false` the world
/// does not advance, and everything else — rendering, `/frame`, `/status`, `/care/status` —
/// keeps running.
struct CareRuntime {
    service: care::CareService,
    worker: care::JournalWorker,
    /// Receipt-driven presentation only; never persisted or consulted by ecology.
    effects: care_effects::CareEffects,
    /// The next sequence number to allocate: after the journal's maximum, not after the
    /// snapshot's cursor. A record in the journal has reserved its sequence already.
    next_seq: u64,
    /// Recovered commands still to apply, in sequence order.
    replay: std::collections::VecDeque<care::PlannedCommand>,
    /// Handed to the journal worker, not yet acknowledged.
    inflight: Vec<care::PlannedCommand>,
    /// The boundary the world is held at.
    holding_at: Option<u64>,
    /// Set once an uncertain write has happened. The world never advances again: the only
    /// way out is a clean stop, which still writes the final snapshot at this boundary.
    failed: bool,
    /// False without `--care`. Recovery of *already accepted* history never depends on this
    /// flag — a durable command at `B ≥ S` must be replayed whether or not this run is
    /// willing to accept new ones, or restarting without the flag would advance past `B`
    /// and checkpoint a history the journal disagrees with. The flag only decides whether
    /// anything *new* may be admitted.
    intake_allowed: bool,
    /// A hold ended this iteration, so the clock owes itself a re-base.
    released: bool,
}

impl CareRuntime {
    /// True while the world is held at a boundary and must not advance. The presentation
    /// freezes with it: see the render arm of the loop.
    fn is_holding(&self) -> bool {
        self.failed || self.holding_at.is_some()
    }

    /// True when the world may step. Never blocks: a pending commit is discovered by
    /// polling the worker, once per loop iteration.
    fn boundary(&mut self, world: &mut World) -> Result<bool> {
        if self.failed {
            return Ok(false);
        }
        self.drain_acks(world)?;
        if self.failed || self.holding_at.is_some() {
            return Ok(false);
        }
        // The recovered schedule owns the boundary until it is exhausted. Intake is gated
        // `replaying` meanwhile, so nothing new can interleave with it.
        if self.apply_replay(world)? {
            return Ok(true);
        }
        if !self.replay.is_empty() || !self.intake_allowed {
            return Ok(true);
        }
        let boundary = world.tick();
        let planned = self.service.drain_prepared(self.next_seq, boundary);
        if planned.is_empty() {
            return Ok(true);
        }
        self.next_seq += planned.len() as u64;
        self.inflight = planned.clone();
        self.holding_at = Some(boundary);
        self.service.hold_at(boundary);
        eprintln!(
            "cubarium: holding at tick {boundary} while {} care command(s) are made durable",
            planned.len()
        );
        if !self.worker.submit(care::JournalJob::Accept(planned)) {
            self.fail(boundary, "the care journal worker is gone");
        }
        Ok(false)
    }

    /// Apply every recovered command whose boundary is this one. Returns true when at least
    /// one was applied.
    fn apply_replay(&mut self, world: &mut World) -> Result<bool> {
        let mut applied = false;
        while let Some(head) = self.replay.front() {
            if head.apply_after_tick > world.tick() {
                break;
            }
            // The schedule was validated whole before the first step, so this cannot happen
            // from a well-formed journal; if it does, something else is wrong and stopping
            // is the only answer that does not invent history.
            anyhow::ensure!(
                head.apply_after_tick == world.tick(),
                "care recovery: seq {} belongs at boundary {} but the world is already at \
                 tick {}",
                head.seq,
                head.apply_after_tick,
                world.tick()
            );
            let command = self.replay.pop_front().expect("just inspected");
            let receipt = world.apply_care(&to_core(&command));
            if let Some(reason) = receipt.outcome.reason()
                && (reason == "out of order" || reason == "wrong boundary")
            {
                anyhow::bail!(
                    "care recovery: the world refused seq {} at boundary {} with \"{reason}\". \
                     A replayed command the core will not admit is a recovery error, not \
                     something to skip past.",
                    command.seq,
                    command.apply_after_tick
                );
            }
            eprintln!(
                "cubarium: replayed care seq {} ({}) at tick {}: {}",
                command.seq,
                command.kind.as_str(),
                receipt.tick,
                receipt.outcome.as_str()
            );
            self.record(&command, &receipt);
            applied = true;
        }
        if applied && self.replay.is_empty() && self.intake_allowed {
            eprintln!("cubarium: the recovered care schedule is exhausted; care is open");
            self.service.open_intake();
        }
        Ok(applied)
    }

    /// Answer the journal worker. This is where an acknowledgement turns into application.
    fn drain_acks(&mut self, world: &mut World) -> Result<()> {
        while let Some(ack) = self.worker.poll_ack() {
            match ack {
                care::JournalAck::Accepted { commands, result: Ok(()) } => {
                    // Durable. Only now does anything reach the world.
                    self.service.commit_accepted(&commands);
                    for command in &commands {
                        let receipt = world.apply_care(&to_core(command));
                        self.record(command, &receipt);
                    }
                    self.inflight.clear();
                    self.holding_at = None;
                    self.released = true;
                    self.service.release_hold();
                }
                care::JournalAck::Accepted { commands, result: Err(e) } if e.is_full() => {
                    // Nothing was attempted, so nothing is uncertain: the sequence numbers
                    // go back, the clients are told, and the world resumes. This is the one
                    // journal failure that does not stop the world.
                    eprintln!("cubarium: care refused: {e}");
                    self.next_seq = self.next_seq.saturating_sub(commands.len() as u64);
                    for command in &commands {
                        self.service.record_outcome(
                            command.seq,
                            "rejected",
                            "journal full",
                            serde_json::Value::Null,
                        );
                    }
                    self.inflight.clear();
                    self.holding_at = None;
                    self.released = true;
                    self.service.release_hold();
                }
                care::JournalAck::Accepted { result: Err(e), .. } => {
                    self.fail(world.tick(), format!("{e}"));
                }
                care::JournalAck::Outcome { result: Err(e) } => {
                    if e.is_full() {
                        // Diagnostic only; replay never needs it.
                        eprintln!("cubarium: could not record a care outcome: {e}");
                    } else {
                        // The hold was already released when this was submitted, so the
                        // world has advanced. It holds *here*, at the tick it has reached.
                        self.fail(world.tick(), format!("{e}"));
                    }
                }
                care::JournalAck::Outcome { result: Ok(()) } => {}
            }
        }
        Ok(())
    }

    /// Hand a receipt to the waiting client and to the journal's diagnostic record.
    fn record(&mut self, command: &care::PlannedCommand, receipt: &CareReceipt) {
        // Both fresh durable application and boundary-correct journal replay enter
        // here. Snapshot history is not replayed, so old inputs do not retrigger.
        self.effects.observe(&to_core(command), receipt);
        let reason = receipt.outcome.reason().unwrap_or_default().to_string();
        let applied = applied_json(&receipt.outcome);
        self.service.record_outcome(
            command.seq,
            receipt.outcome.as_str(),
            &reason,
            applied.clone(),
        );
        self.worker.submit(care::JournalJob::Outcome(vec![care::OutcomeRecord {
            seq: command.seq,
            tick: receipt.tick,
            outcome: receipt.outcome.as_str(),
            reason,
            applied,
        }]));
    }

    /// An uncertain write. The world holds where it is for the rest of the process.
    ///
    /// `observed_at` is the world's *actual* tick right now, which is not always the
    /// acceptance boundary: a diagnostic outcome record is written after the hold was
    /// released, so its failure can arrive with the world several ticks further on. The
    /// world holds at the tick it has actually reached, and that is the tick the final
    /// snapshot will carry — claiming otherwise would send an operator looking for a
    /// checkpoint that does not exist.
    fn fail(&mut self, observed_at: u64, reason: impl Into<String>) {
        let reason = reason.into();
        if self.failed {
            return;
        }
        self.failed = true;
        self.holding_at = Some(observed_at);
        self.service.hold_at(observed_at);
        eprintln!(
            "cubarium: care failed, observed at tick {observed_at}: {reason}\n\
             cubarium: the record may or may not be on disk, so the world will not advance \
             past tick {observed_at}. Nothing more will be appended to the journal in this \
             process. Stop cleanly (Ctrl-C) — the final snapshot is written at tick \
             {observed_at} — and restart: recovery applies whatever accepted records \
             survived, each at its own boundary."
        );
        self.service.fail(reason);
    }

    /// Whether a hold ended since this was last asked, so the clock can re-base instead of
    /// fast-forwarding through the time the world spent waiting on a disk.
    fn take_released(&mut self) -> bool {
        std::mem::take(&mut self.released)
    }
}

/// Open the journal, validate the recovered schedule, and build the runtime. Everything
/// that can refuse to start does so here, before a log, a worker or a sink is opened.
fn open_care(
    run: &Run,
    world: &World,
    build_id: &str,
    lock: &std::sync::Arc<state::StateLock>,
) -> Result<CareRuntime> {
    let epoch = care_epoch();
    let journal = care::Journal::open(&run.state, &epoch, build_id)?;
    let admitted = world.care().admitted_seq;
    let plan = journal.replay_plan(admitted, world.tick())?;
    // After the journal's maximum, never after the snapshot's cursor: the journal has
    // already reserved those numbers even where the world has not reached them yet.
    let next_seq = journal.max_seq().unwrap_or(admitted).max(admitted) + 1;
    if !plan.is_empty() {
        eprintln!(
            "cubarium: {} care command(s) to replay from {}, seq {}..={} at boundaries {}..={}",
            plan.len(),
            journal.path().display(),
            plan[0].seq,
            plan[plan.len() - 1].seq,
            plan[0].apply_after_tick,
            plan[plan.len() - 1].apply_after_tick,
        );
    }
    let status = journal.status();
    let worker = care::JournalWorker::spawn_holding(journal, Some(std::sync::Arc::clone(lock)));
    let service = care::CareService::new(epoch, status);
    Ok(CareRuntime {
        service,
        worker,
        effects: care_effects::CareEffects::default(),
        next_seq,
        replay: plan.into(),
        inflight: Vec::new(),
        holding_at: None,
        failed: false,
        intake_allowed: run.care,
        released: false,
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

    // Ownership first, before the occupancy check, before loading, and before any
    // persistent write: an OS advisory lock held by an open handle for the whole run is
    // what makes "exactly one owner" atomic. Declared here so it is dropped *after* the
    // checkpoint worker has stopped (drop order is reverse declaration order).
    let lock = std::sync::Arc::new(state::StateLock::acquire(&run.state)?);

    let occupied = state::occupancy(&run.state).with_context(|| {
        format!("checking what the state directory {} already holds", run.state.display())
    })?;
    if run.fresh && occupied.is_occupied() {
        // The defect this guard exists for: `--fresh` only ever skipped *loading*. The
        // pruner keeps the eight highest ticks with no notion of world identity, so a new
        // world beside eight higher-tick snapshots has its own checkpoints deleted as fast
        // as it writes them — and `load_newest` would then resume the old world.
        anyhow::bail!(
            "--fresh refuses to start in {}: it already holds {}.\n\
             A new world in an occupied directory mixes two histories, and the snapshot \
             pruner — which ranks by tick, not by world — would delete the new world's \
             checkpoints in favour of the old world's higher ticks. Choose a new --state \
             directory.",
            run.state.display(),
            occupied.describe()
        );
    }
    if !run.fresh && occupied.journal.is_some() && occupied.snapshots.is_empty() {
        anyhow::bail!(
            "{} holds a care journal but no snapshot.\n\
             Those journaled commands were accepted against a world this directory can no \
             longer produce, and creating one from today's defaults would replay them against \
             the wrong world. Restore the snapshot, or move the journal aside deliberately.",
            run.state.display()
        );
    }

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
    // The worker holds a share of the same lock handle, so the directory stays owned for
    // as long as a thread can still write into it — including after `shutdown` gives up
    // waiting and detaches one.
    let mut checkpoints =
        Checkpointer::spawn_holding(&run.state, Some(std::sync::Arc::clone(&lock)));
    let build_id = state::build_id();

    // Care opens before the sink, because the sink serves it and because everything that
    // can refuse the run — an unreadable journal, an inconsistent recovery schedule — must
    // refuse before a socket is listening.
    // Opened whenever this run may need care at all: because `--care` asked for intake, or
    // because the directory already holds journaled history that must be recovered whether
    // or not this run accepts anything new.
    let mut care = match run.care || occupied.journal.is_some() {
        false => None,
        true => Some(open_care(run, &world, &build_id, &lock)?),
    };
    if let Some(rt) = care.as_mut() {
        if !rt.intake_allowed {
            if rt.replay.is_empty() {
                // Nothing to recover and no intake: the journal was opened, validated and
                // left alone. Drop it rather than carry a service nobody can reach.
                let rt = care.take().expect("just matched");
                let _ = rt.worker.shutdown();
            } else {
                eprintln!(
                    "cubarium: {} command(s) of accepted care history will be replayed even \
                     though --care is not set; recovering already accepted care does not \
                     depend on accepting new care",
                    rt.replay.len()
                );
            }
        }
    }
    if let Some(rt) = care.as_mut() {
        if loaded_from.is_none() && rt.intake_allowed {
            // A world this run created owes a durable opening checkpoint before it accepts
            // anything: a crash before the first periodic checkpoint would otherwise leave
            // journaled commands with no persisted world to replay them against.
            rt.service.gate_until_opened();
            match state::write_snapshot(
                &run.state,
                world.tick(),
                &encode_snapshot(&world.state, &build_id),
            ) {
                Ok(path) => {
                    eprintln!(
                        "cubarium: wrote the opening checkpoint {} before enabling care",
                        path.display()
                    );
                    rt.service.open_intake();
                }
                Err(e) => eprintln!(
                    "cubarium: the opening checkpoint could not be written ({e}); care stays \
                     closed for this run and the world runs on without it"
                ),
            }
        }
        if !rt.replay.is_empty() || !rt.intake_allowed {
            rt.service.gate_until_replayed();
        }
        rt.service.publish_tick(world.tick());
    }

    // Read before the sink opens: `--mirror-web`'s `/status` names the tick this run
    // started at, and nothing steps the world between here and the loop.
    let start_tick = world.tick();
    let mut sink = open_sink(
        run,
        &source_of(run, start_tick, loaded_from.as_deref()),
        // Only an intake-enabled run serves the care routes; a recovery-only run leaves
        // `/care/status` reporting `enabled: false`, which is the truth.
        care.as_ref().filter(|rt| rt.intake_allowed).map(|rt| rt.service.shared()),
    )?;
    let headless = sink.is_none();
    let pace = Pace::of(run.speed);

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

    let mut presenter = open_show(run)?;
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    let mut view: Option<RenderView> = None;
    let mut frames = 0u64;
    let started = Instant::now();

    // Advance the world one tick and do everything that hangs off a completed tick.
    let mut ticks_done = 0u64;
    let advance = |world: &mut World,
                       presenter: &mut Show,
                       view: &mut Option<RenderView>,
                       telemetry: &mut TelemetryLog,
                       fields: &mut Option<FieldLog>,
                       events: &mut Option<EventLog>,
                       checkpoints: &mut Checkpointer,
                       sink: &mut Option<Box<dyn FrameSink>>,
                       ticks_done: &mut u64| {
        world.step();
        *ticks_done += 1;
        let tick = world.tick();
        // Observation only, and only ever in this direction: a sink is told what the
        // world did, and has no way to tell the world anything.
        if let Some(s) = sink.as_mut() {
            s.observe_tick(tick);
        }
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
                if let Some(rt) = care.as_mut()
                    && !rt.boundary(&mut world)?
                {
                    // Held. `--speed 0` has no clock to sleep against, so a short sleep
                    // keeps a held world from spinning a core while it waits on a disk.
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                advance(
                    &mut world,
                    &mut presenter,
                    &mut view,
                    &mut telemetry,
                    &mut fields,
                    &mut events,
                    &mut checkpoints,
                    &mut sink,
                    &mut ticks_done,
                );
                if let Some(rt) = care.as_ref() {
                    rt.service.publish_tick(world.tick());
                }
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
                            // The contract's held boundary. While a record is being made
                            // durable this refuses to let the world advance; rendering,
                            // `/frame`, `/status` and `/care/status` all keep serving,
                            // because they are handled below and on other threads.
                            if let Some(rt) = care.as_mut()
                                && !rt.boundary(&mut world)?
                            {
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
                                &mut sink,
                                &mut ticks_done,
                            );
                            if let Some(rt) = care.as_ref() {
                                rt.service.publish_tick(world.tick());
                            }
                        }
                        // A hold is time the world really did spend not stepping. Re-base
                        // rather than fast-forward: replaying it as catch-up ticks would
                        // turn an `fsync` into a visible lurch.
                        if care.as_mut().is_some_and(CareRuntime::take_released) {
                            clock.rebase_now(Instant::now());
                        }
                    }
                    Step::Render { f } => {
                        if let (Some(s), Some(v)) = (sink.as_mut(), view.as_ref()) {
                            // `f` walks each body along the path of the last completed
                            // tick, so a 20 Hz world reads as continuous at `--fps`.
                            //
                            // Except while care holds the boundary. The world is not
                            // advancing, so `f` cycling 0→1 every wall tick would replay
                            // the last movement segment over and over and make a stopped
                            // world jitter at 20 Hz. Held time is presented at its
                            // endpoint: a fixed `f = 1.0`, which is where the held tick
                            // actually finished and where the next one will start.
                            let f = match care.as_ref().is_some_and(CareRuntime::is_holding) {
                                true => 1.0,
                                false => f,
                            };
                            presenter.draw(v, f, &mut canvas);
                            if let Some(rt) = care.as_mut() {
                                // A brief local receipt flourish, not a persistent food
                                // inventory or a second world. Use the same held-time
                                // fraction as the bodies and encode it for every sink.
                                rt.effects.draw(v.tick, f, &mut canvas);
                            }
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
    // Care stops before the final snapshot: the journal worker has to be off the disk
    // before the checkpoint that this world's recovery will be measured against, and a
    // world held at `B` writes its final snapshot at exactly `B`.
    if let Some(rt) = care.take() {
        if let Some(boundary) = rt.holding_at {
            eprintln!(
                "cubarium: stopping while care is held at tick {boundary}; the final snapshot is \
                 written there"
            );
        }
        let CareRuntime { worker, service, .. } = rt;
        drop(service);
        if let Some(journal) = worker.shutdown()
            && let Some(reason) = journal.poisoned()
        {
            eprintln!(
                "cubarium: {} accepted nothing after an uncertain write ({reason}); its bytes \
                 are exactly as that write left them",
                journal.path().display()
            );
        }
    }

    // The clean-shutdown snapshot, always: the checkpoint interval is the recovery
    // bound only for an *un*clean stop.
    checkpoints.queue(final_tick, encode_snapshot(&world.state, &build_id));
    let checkpoints_queued = checkpoints.queued();
    checkpoints.shutdown(None);
    // Only now is the directory free: the lock outlives every writer it protects.
    drop(lock);

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
    use clap::Parser;

    /// Exercise the runner's receipt hook, not just the effect renderer. These
    /// scratch journals have explicit private hooks, independent of process-wide
    /// failure injection used by other tests.
    #[test]
    fn care_flourishes_follow_durable_application_and_boundary_replay_only() {
        for route in ["durable", "replay", "uncertain"] {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "cubarium-receipt-visual-{}-{nonce}-{route}",
                std::process::id()
            ));
            std::fs::create_dir(&dir).unwrap();
            let hooks = care::JournalHooks::default();
            if route == "uncertain" {
                hooks.fail_after_sync.store(1, Ordering::Relaxed);
            }
            let journal =
                care::Journal::open_with_hooks(&dir, "visual-test", "test", hooks).unwrap();
            let service = care::CareService::new("visual-test".to_string(), journal.status());
            let mut rt = CareRuntime {
                service,
                worker: care::JournalWorker::spawn(journal),
                effects: care_effects::CareEffects::default(),
                next_seq: 2,
                replay: Default::default(),
                inflight: Vec::new(),
                holding_at: None,
                failed: false,
                intake_allowed: false,
                released: false,
            };
            let mut world = World::new(WorldConfig::default()).unwrap();
            world.step();
            let opening_hash = cubarium_core::snapshot::state_hash(&world.state);
            let command = care::PlannedCommand {
                seq: 1,
                apply_after_tick: world.tick(),
                kind: care::CareKind::Feed,
                target: care::CareTarget {
                    face: 0,
                    u: 32,
                    v: 32,
                },
                client: "visual-test".to_string(),
                request: 1,
            };
            let sample = |rt: &mut CareRuntime| {
                let mut canvas = Canvas::new();
                rt.effects
                    .draw(command.apply_after_tick + 11, 0.5, &mut canvas);
                let mut frame = Frame::black();
                canvas.encode(&mut frame);
                frame
            };
            assert!(sample(&mut rt).as_bytes().iter().all(|&b| b == 0));
            if route == "replay" {
                rt.replay.push_back(command.clone());
                assert!(rt.apply_replay(&mut world).unwrap());
            } else {
                rt.holding_at = Some(world.tick());
                rt.inflight.push(command.clone());
                assert!(
                    rt.worker
                        .submit(care::JournalJob::Accept(vec![command.clone()]))
                );
                // Durable bytes alone do not paint: only the runner consuming a
                // successful acknowledgement may apply the command and its visual.
                assert!(sample(&mut rt).as_bytes().iter().all(|&b| b == 0));
                let deadline = Instant::now() + Duration::from_secs(5);
                while rt.holding_at.is_some() && !rt.failed {
                    assert!(
                        Instant::now() < deadline,
                        "journal acknowledgement timed out"
                    );
                    rt.drain_acks(&mut world).unwrap();
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            let after_application = cubarium_core::snapshot::state_hash(&world.state);
            let frame = sample(&mut rt);
            if route == "uncertain" {
                assert!(rt.failed);
                assert_eq!(after_application, opening_hash);
                assert!(frame.as_bytes().iter().all(|&b| b == 0));
            } else {
                assert_ne!(after_application, opening_hash);
                assert!(frame.as_bytes().iter().any(|&b| b != 0));
                assert_eq!(
                    frame.as_bytes(),
                    sample(&mut rt).as_bytes(),
                    "held visual changed"
                );
            }
            assert_eq!(
                cubarium_core::snapshot::state_hash(&world.state),
                after_application
            );
            rt.worker.shutdown().unwrap();
            // Only this test's unique scratch journal is removed; no live state.
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// Reproducible native-resolution review images, using real applied care and
    /// the actual art presenter. No HTTP, live state or display transport.
    #[test]
    #[ignore = "writes isolated native-resolution care review captures"]
    fn capture_care_flourishes_on_the_authored_world() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pack = ArtPack::load(&root.join("assets/atelier")).unwrap();
        let mut presenter = ArtPresenter::new(pack);
        let mut effects = care_effects::CareEffects::default();
        let mut world = World::new(WorldConfig::default()).unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cubarium-care-flourishes-{nonce}"));
        std::fs::create_dir(&dir).unwrap();
        let mut receipt_log = Vec::new();
        for _ in 0..510 {
            if world.tick() == 400 || world.tick() == 470 {
                let command = CareCommand {
                    seq: if world.tick() == 400 { 1 } else { 2 },
                    apply_after_tick: world.tick(),
                    kind: if world.tick() == 400 {
                        CareKind::Feed
                    } else {
                        CareKind::Clean
                    },
                    target: CareTarget {
                        face: 0,
                        u: 32.0,
                        v: 48.0,
                    },
                };
                let receipt = world.apply_care(&command);
                effects.observe(&command, &receipt);
                receipt_log.push(serde_json::json!({
                    "seq":receipt.seq,"tick":receipt.tick,"kind":command.kind.as_str(),
                    "outcome":receipt.outcome.as_str(),"applied":applied_json(&receipt.outcome),
                }));
            }
            world.step();
            world.drain_events();
            let view = world.render_view();
            presenter.observe(&view);
            if ![
                401, 403, 407, 411, 419, 431, 447, 455, 471, 475, 483, 495, 507,
            ]
            .contains(&world.tick())
            {
                continue;
            }
            let hash = cubarium_core::snapshot::state_hash(&world.state);
            let mut canvas = Canvas::new();
            presenter.draw(&view, 0.5, &mut canvas);
            for variant in ["base", "flourish"] {
                if variant == "flourish" {
                    effects.draw(view.tick, 0.5, &mut canvas);
                }
                let mut frame = Frame::black();
                canvas.encode(&mut frame);
                let mut rgb = Vec::new();
                crate::net::net_rgb8(&frame, &mut rgb);
                crate::sink::png::write_net_png(
                    &dir.join(format!("{}-{variant}.png", world.tick())),
                    &rgb,
                )
                .unwrap();
            }
            assert_eq!(cubarium_core::snapshot::state_hash(&world.state), hash);
        }
        std::fs::write(
            dir.join("receipts.json"),
            serde_json::to_vec_pretty(&receipt_log).unwrap(),
        )
        .unwrap();
        eprintln!(
            "care-flourish captures: {} (Front32,48; base and flourish share ecology)",
            dir.display()
        );
    }

    #[test]
    fn the_viewer_note_prints_the_speed_without_trailing_zeros() {
        assert_eq!(speed_note(1.0), "1× time");
        assert_eq!(speed_note(8.0), "8× time");
        assert_eq!(speed_note(0.5), "0.5× time");
        assert_eq!(speed_note(2.5), "2.5× time");
        assert_eq!(speed_note(20.0), "20× time");
    }

    #[test]
    fn an_art_directory_that_will_not_load_is_fatal_and_names_itself() {
        let mut run = Run::parse_from(["cubarium"]);
        assert!(matches!(open_show(&run), Ok(Show::Plain(_))), "no --art is the M2 image");
        run.art = Some(PathBuf::from("/nonexistent/atelier"));
        let err = match open_show(&run) {
            Err(e) => e,
            Ok(_) => panic!("a missing pack must not fall back to discs"),
        };
        assert!(
            format!("{err:#}").contains("/nonexistent/atelier"),
            "the error must name the directory: {err:#}"
        );
    }

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
