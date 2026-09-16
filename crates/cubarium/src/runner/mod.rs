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

use cubarium_surface::{Scale, Topology};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use cubarium_core::view::RenderView;
use cubarium_core::{World, WorldConfig, encode_snapshot};
use cubarium_render::Canvas;
use cube_proto::Frame;

use crate::art::ArtPack;
use crate::art_present::ArtPresenter;
use crate::care;
use crate::cli::{Run, RunSinkArg, topology_name};
use crate::clock::{Clock, Step, TICK_HZ};
use crate::present::Presenter;
use crate::sink::{
    FanOutSink, FrameSink, Output, PngSink, PreviewSink, ShimSink, WebSink, WorldShape, web,
};
use crate::state::{self, Checkpointer};

mod care_runtime;
mod logging;

#[cfg(test)]
mod tests;

use care_runtime::*;
use logging::*;

/// What one `run` produced. Returned so tests can drive the host through the library
/// instead of a subprocess.
#[derive(Clone, Debug)]
pub struct RunOutcome {
    /// The tick the world was at when this run started (nonzero when resumed).
    pub start_tick: u64,
    pub final_tick: u64,
    pub population: usize,
    /// Live organisms running a recurrent policy at the end of the run.
    pub neural_animals: usize,
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
    let config: WorldConfig =
        toml::from_str(&text).with_context(|| format!("parsing the config {}", path.display()))?;
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
            let state::Loaded {
                path,
                tick,
                mut state,
            } = loaded;
            eprintln!("cubarium: resuming {} at tick {tick}", path.display());
            if let Some(file_config) = &from_file {
                merge_operational(&mut state.config, file_config);
            }
            // A world's shape is not an operational override: `merge_operational` copies
            // `capacity` and `weather.moving` out of the file and nothing else, so a
            // `--config` or `--topology` naming a different surface would be silently
            // ignored. Refused by name instead, like every other schema refusal — the
            // standing rule is fresh, never migrate.
            let asked = run
                .topology()
                .or_else(|| from_file.as_ref().map(|c| c.topology));
            if let Some(asked) = asked
                && asked != state.config.topology
            {
                anyhow::bail!(
                    "{}: this world is a {} world and the run asked for a {} world. A world's \
                     shape is fixed when it is created, and nothing migrates one surface onto \
                     another: resume it as it is, or use --fresh with a --state directory of \
                     its own.",
                    path.display(),
                    topology_name(state.config.topology),
                    topology_name(asked),
                );
            }
            let world =
                World::from_state(state).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
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
        eprintln!(
            "cubarium: no loadable snapshot in {}; creating a new world",
            run.state.display()
        );
    }

    let mut config = from_file.unwrap_or_default();
    if let Some(seed) = run.seed {
        config.seed = seed;
    }
    // The command line wins over the file for a world being created, exactly as `--seed`
    // does. Validation is the config's own: `WorldConfig::validate` checks the
    // (topology, scale) pair before any bound below it.
    if let Some(topology) = run.topology() {
        config.topology = topology;
    }
    if let Some(scale) = run.scale() {
        config.world_scale = scale;
    }
    let world = World::new(config).map_err(|e| anyhow::anyhow!("invalid world config: {e}"))?;
    Ok((world, None, None))
}

// --- `--neural`: seeding a new world with trained foragers ----------------------------

/// The cell a seeded copy aims for on its face: the middle of the 16x16 chart. Cell (8, 8)
/// rather than the exact geometric centre, so the start is a whole cell the clearance check
/// can be about.
const SEED_CELL: u16 = 8;

/// Is this cell clear ground to start a grazer on? Standing water above `water.flood` is
/// the world's own "this is not ground" threshold — it is where producer growth starts
/// drowning — so a copy aimed into a pond walks to the nearest cell that is not one.
fn is_clear_ground(world: &World, cell: cubarium_surface::CellId) -> bool {
    world.state.fields.w[cell.index()] <= world.state.config.water.flood
}

/// `want` if it is clear, else the nearest clear cell on the same face, searched outward in
/// Chebyshev rings and in a fixed order within a ring, so the choice is deterministic.
fn nearest_clear_cell(
    world: &World,
    want: cubarium_surface::CellId,
) -> Option<cubarium_surface::CellId> {
    use cubarium_surface::{CELLS_PER_FACE_EDGE, CellId};
    let edge = CELLS_PER_FACE_EDGE as i32;
    let (face, cx, cy) = (want.face(Topology::Cube, Scale::ONE), i32::from(want.cx(Topology::Cube, Scale::ONE)), i32::from(want.cy(Topology::Cube, Scale::ONE)));
    for r in 0..edge {
        let mut best: Option<CellId> = None;
        for y in (cy - r).max(0)..=(cy + r).min(edge - 1) {
            for x in (cx - r).max(0)..=(cx + r).min(edge - 1) {
                // Only the ring itself; the interior was searched at a smaller `r`.
                if (x - cx).abs().max((y - cy).abs()) != r {
                    continue;
                }
                let cell = CellId::new(Topology::Cube, Scale::ONE, face, x as u16, y as u16);
                if is_clear_ground(world, cell) && best.is_none() {
                    best = Some(cell);
                }
            }
        }
        if best.is_some() {
            return best;
        }
    }
    None
}

/// Read `--neural`'s exported policy and put `--neural-count` copies of the training animal
/// into a **newly created** world, one per face in `Face` index order (Front, Right, Back,
/// Left, Top), heading east in that face's chart.
///
/// Returns how many were seeded. Nothing here can happen on a resume: the caller refuses
/// `--neural` there, because a seeding control that ran again on every restart would add a
/// fresh cohort to a world that already has one.
fn seed_neural_animals(run: &Run, world: &mut World) -> Result<usize> {
    let Some(path) = run.neural.as_ref() else {
        return Ok(0);
    };
    // Every constant below is a cube's: one copy per face in `Face` index order, aimed at
    // cell (8, 8) of a 16×16 chart, searched outward inside that chart's edge. A ring has
    // one chart and a different cell grid, so none of them means anything there. Refused
    // by name rather than quietly seeding five animals into one corner of the ring.
    anyhow::ensure!(
        world.topology() == Topology::Cube,
        "--neural seeds one trained animal per cube face, at the centre of that face's \
         16x16 cell chart; this world is a ring, which has one chart and a different grid. \
         Seeding a ring world is not written yet: start the world without --neural."
    );
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the policy file {}", path.display()))?;
    let file: cubarium_search::es::export::PolicyFile = serde_json::from_str(&text)
        .with_context(|| format!("parsing the policy file {}", path.display()))?;
    // `PolicyFile::policy` is what refuses a foreign schema digest by name; decoding once
    // here means a bad file stops the run before any world is written.
    let policy = file
        .policy()
        .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    // The ecology, too: a policy trained in one set of plant, animal and detrital
    // constants is a different animal in another, and the digest says nothing about that.
    // The identity is `--config` as loaded (the shipped defaults without it), which is
    // exactly what `es-train --config` hashed; `--seed` and the operational merges are not
    // part of it. Refused by name, as `es-evaluate` and `es-population` refuse it.
    let ecology = cubarium_search::es::fixture::Ecology::from_option(run.config.as_deref())
        .map_err(|e| anyhow::anyhow!("--neural: {e}"))?;
    file.check_ecology(&ecology)
        .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    // And the motor contract, for the same reason: a policy trained under one envelope and
    // one price per radian is a different animal under another, and the digest says nothing
    // about that either. The host **never sets a motor model**, so `world.motor_model()` is
    // always the shipped `Sweep` and this check can only ever refuse an `inertial` policy —
    // which is the point. It is the contract existing rather than a change of behaviour: no
    // world the host builds moves because of this line. Refused by name, as `es-evaluate`
    // and `es-population` refuse it.
    file.check_motor(world.motor_model())
        .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;

    let east = cubarium_surface::Vec2::new(1.0, 0.0);
    for k in 0..run.neural_count {
        let face = cubarium_surface::Face::from_index((k % 5) as u8).expect("five faces");
        let want = cubarium_surface::CellId::new(Topology::Cube, Scale::ONE, face, SEED_CELL, SEED_CELL);
        let cell = nearest_clear_cell(world, want).ok_or_else(|| {
            anyhow::anyhow!(
                "--neural: no clear ground anywhere on {face:?} to start a neural animal on"
            )
        })?;
        if cell != want {
            eprintln!(
                "cubarium: --neural copy {k} on {face:?}: the centre cell ({}, {}) is not clear                  ground; starting at ({}, {}) instead",
                want.cx(Topology::Cube, Scale::ONE),
                want.cy(Topology::Cube, Scale::ONE),
                cell.cx(Topology::Cube, Scale::ONE),
                cell.cy(Topology::Cube, Scale::ONE)
            );
        }
        world
            .found_neural_animal(cell.center(Topology::Cube, Scale::ONE), east, policy.clone())
            .map_err(|e| anyhow::anyhow!("--neural: seeding copy {k} on {face:?}: {e}"))?;
    }
    eprintln!(
        "cubarium: seeded {} neural animals from {} (generation {}, digest {:#018x})",
        run.neural_count,
        path.display(),
        file.generation,
        file.policy_digest,
    );
    Ok(run.neural_count)
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
    shape: WorldShape,
) -> Result<Option<Box<dyn FrameSink>>> {
    let primary: Box<dyn FrameSink> = match run.sink {
        RunSinkArg::None => return Ok(None),
        RunSinkArg::Preview => Box::new(PreviewSink::new(run.scale, &run.out, shape)?),
        RunSinkArg::Shim => Box::new(ShimSink::new(run.addr.clone())),
        RunSinkArg::Png => Box::new(PngSink::new(&run.out, run.every)?),
        RunSinkArg::Web => Box::new(WebSink::with_world(
            run.web_port,
            speed_note(run.speed),
            source.clone(),
            care.clone(),
            shape,
        )?),
        RunSinkArg::Gpu => {
            // The GPU sink draws from the art pack, so it needs one. `--art` is the same
            // flag the CPU art presenter takes, and naming it here rather than defaulting
            // to `assets/atelier` keeps one answer to "which art is this run showing?".
            let art = run.art.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "--sink gpu draws the baked sprite art; pass --art <dir> (e.g. assets/atelier)"
                )
            })?;
            Box::new(crate::sink::GpuSink::new(
                shape,
                art,
                run.gpu_target.unwrap_or_else(crate::sink::GpuTargetKind::detect),
                run.gpu_bend_substep,
                run.gpu_filter == crate::cli::GpuFilterArg::Bilinear,
                run.gpu_art_scale,
                run.gpu_capture.clone(),
            )?)
        }
    };
    if !run.mirror_web {
        return Ok(Some(primary));
    }
    let web =
        WebSink::with_world(run.web_port, speed_note(run.speed), source.clone(), care, shape)?;
    // `--web-port 0` binds an ephemeral port, so the URL has to be reported to be usable.
    eprintln!(
        "cubarium: mirroring the same frames to the viewer at {}",
        web.url()
    );
    Ok(Some(Box::new(FanOutSink::new(vec![
        primary,
        Box::new(web),
    ]))))
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

    /// Once per completed tick, after `observe`: the world's own hunter membership and the
    /// hunter events it committed this tick. The plain image ignores both (the decided disc
    /// path is unchanged); the art image draws each listed member once as the Lanternjaw. A
    /// member the art cannot honour is a named error — the profile was validated at load, so
    /// this cannot happen for a world that started.
    fn observe_hunters(
        &mut self,
        view: &RenderView,
        hunters: &[cubarium_core::HunterView],
        events: &[cubarium_core::HunterEvent],
    ) -> Result<()> {
        if let Show::Art(p) = self {
            p.observe_hunters(view, hunters, events)
                .map_err(|e| anyhow::anyhow!("the art cannot draw this world's hunters: {e}"))?;
        }
        Ok(())
    }

    /// The renderer's capability against a world's hunter profile, asked at load and before
    /// any tick: the plain image needs none; the art image refuses by name a profile whose
    /// geometry or scale range it cannot draw, so the caller can stop before mutating the
    /// world, or run without `--art`.
    fn validate_hunters(&self, world: &World) -> Result<()> {
        if let (Show::Art(p), Some(profile)) = (self, world.hunters().profile()) {
            p.validate_hunter_profile(profile).map_err(|e| {
                anyhow::anyhow!(
                    "the art cannot draw this world's hunter profile ({e}); run without --art \
                     to opt out of the art, or use a profile the Lanternjaw can draw"
                )
            })?;
        }
        Ok(())
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
        format!(
            "checking what the state directory {} already holds",
            run.state.display()
        )
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
    // A seeding control applies to the world it creates, never to one it found. Refused
    // rather than ignored: silently dropping `--neural` on a restart would leave the
    // operator believing a cohort was added, and honouring it would add a second one every
    // time the runner came back up.
    if loaded_from.is_some() && run.neural.is_some() {
        anyhow::bail!(
            "--neural seeds a new world, and this run resumed {}. Use --fresh with a state \
             directory of its own to start a seeded world, or drop --neural to carry on with \
             the animals this world already has.",
            loaded_from.as_ref().expect("just checked").display()
        );
    }
    seed_neural_animals(run, &mut world)?;
    // The presentation, and its capability against the world's hunter profile, before
    // anything below (care's opening checkpoint, the sink, the loop) touches the world: a
    // saved world whose profile the art cannot draw fails here by name, unmutated.
    let mut presenter = open_show(run)?;
    presenter.validate_hunters(&world)?;
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
    let shape = WorldShape::new(world.topology(), world.scale());
    let mut sink = open_sink(
        run,
        &source_of(run, start_tick, loaded_from.as_deref()),
        // Only an intake-enabled run serves the care routes; a recovery-only run leaves
        // `/care/status` reporting `enabled: false`, which is the truth.
        care.as_ref()
            .filter(|rt| rt.intake_allowed)
            .map(|rt| rt.service.shared()),
        shape,
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

    // The canvas *is* the world's raster: five 64×64 charts for a cube, one `w×h` image
    // for a ring. One of the two encode targets below is live, never both — a cube world
    // never allocates a raster and a ring world never allocates a `Frame`.
    let mut canvas = Canvas::new(world.topology(), world.scale());
    let mut frame = Frame::black();
    let mut raster = shape.raster();
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
            s.observe_counts(world.population(), world.neural_population());
        }
        // Drained every tick even when nothing logs them: `World` records births and
        // deaths whatever `capacity.event_log` says, and a buffer the host never takes
        // grows without bound for the life of the process. Written in the order the
        // world committed them.
        let committed = world.drain_events();
        if let Some(log) = events.as_mut() {
            log.write_tick(&committed);
        }
        // Hunter events are drained on the same rule, headless or not: the world's own
        // transient buffer must never grow for the life of the process. They are handed to
        // the presentation (a capture's settlement position) before they are dropped; this
        // run does not journal them.
        let hunted = world.drain_hunter_events();
        if !headless {
            // Trails are simulated history: they are fed per tick, not per frame.
            let published = world.render_view();
            presenter.observe(&published);
            // The owning world's hunter membership, by full id (empty without a trial).
            presenter.observe_hunters(&published, &world.hunter_view(), &hunted)?;
            // A sink that draws the world itself gets the same two things at the same
            // instant, one way only.
            if let Some(s) = sink.as_mut() {
                s.observe_world(&published, &world.hunter_view(), &hunted);
            }
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
        Ok::<(), anyhow::Error>(())
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
                )?;
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
                            )?;
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
                            // A sink that draws the world itself is handed the view at
                            // exactly the instant the presenter would have been.
                            s.observe_view(v, crate::art_present::present_seconds(v.tick, f), f)?;
                            // ... and then the canvas is skipped entirely for it. A GPU
                            // sink that still paid for a CPU rasterisation it threw away
                            // would be slower than the CPU path, not faster.
                            if s.wants_pixels() {
                                presenter.draw(v, f, &mut canvas);
                                if let Some(rt) = care.as_mut() {
                                    // A brief local receipt flourish, not a persistent food
                                    // inventory or a second world. Use the same held-time
                                    // fraction as the bodies and encode it for every sink.
                                    rt.effects.draw(v.tick, f, &mut canvas);
                                }
                                // Exactly one encode per rendered frame; the identical bytes
                                // reach whichever sink is active.
                                match raster.as_mut() {
                                    Some(r) => {
                                        canvas.encode_raster(r);
                                        s.submit(Output::Ring(r))?;
                                    }
                                    None => {
                                        canvas.encode(&mut frame);
                                        s.submit(Output::Cube(&frame))?;
                                    }
                                }
                            }
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
        let CareRuntime {
            worker, service, ..
        } = rt;
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
        neural_animals: world.neural_population(),
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
