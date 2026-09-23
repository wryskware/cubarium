//! The `voxel-*` command family: check, bench, train and evaluate — the plan's one
//! discoverable family for the phase-one arena, following the `es-*` subcommand
//! conventions (clap subcommands of `cubarium-search`, printed tables, JSON stores under
//! `runs/`).
//!
//! Every command attaches the same way — the episode's controller through the fauna's
//! own controller table (`Fauna::set_controller`), the fauna tick sampling, holding and
//! resolving — so `--controller gru|no-intake|stationary-feeding|heuristic` means one
//! attachment everywhere.
//!
//! - [`check`]: arena validity for both founders across the training seeds, then a
//!   controller smoke through the real driver — including the seam's own behavioural
//!   check: the GRU must move the body, rest must not.
//! - [`bench`]: measured setup cost (rebuild vs prepared-clone) and episode throughput.
//! - [`imitate`]: record the heuristic's `(observation, adapted action)` streams on the
//!   training layouts ([`super::imitate`]).
//! - [`train`]: the bounded ES run ([`super::trainer::train`]).
//! - [`evaluate`]: a saved policy or a disclosed control over the training or the
//!   held-out layout set, with the score components per layout.
//!
//! The commands require arena/founder, controller, seed, episode limit, worker count and
//! wall-time cap as applicable. The worker count reserves ten percent of logical CPUs and
//! stops at the measured sixteen-worker saturation point everywhere. The score-counter
//! state is named in every run's output.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_voxel_fauna::Founder;

use super::controller::{EpisodeDriver, VoxelControl};
use super::driver::{self, EpisodeError, ScoreCounters};
use super::imitate;
use super::task;
use super::task::Stage;
use super::trainer::{self, TrainSpec};
use super::{parse_founder, voxel_schema_digest};
use crate::evaluate::BUILD_ID;

type Boxed = Box<dyn std::error::Error>;

/// The one line every run prints about its score-counter state.
fn print_counter_state() {
    println!(
        "# score counters: {} — settled intake excludes digestive respiration",
        ScoreCounters::Landed.as_str()
    );
}

/// `voxel-check`: arena validity plus a heuristic/GRU smoke, through the real driver.
pub fn check(
    founder: Option<String>,
    stage: String,
    seeds: usize,
    ticks: u64,
) -> Result<(), Boxed> {
    let stage = task::parse_stage(&stage)?;
    let founders: Vec<Founder> = match founder.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => vec![parse_founder(name)?],
        None => Founder::ALL.into(),
    };
    let seeds = seeds.clamp(1, task::TRAINING_LAYOUT_SEEDS.len());

    println!("# voxel arena check — stage {}", stage.as_str());
    println!("# build {BUILD_ID}");
    println!();

    let mut failures = 0usize;
    for f in &founders {
        let manifest = f.manifest();
        println!(
            "## {} ({}) — {} inputs, cadence {} ticks, digest {:#018x}",
            f.name(),
            f.role(),
            manifest.inputs(),
            manifest.cadence_ticks(),
            voxel_schema_digest(*f)
        );

        // 1. Arena validity: every training layout builds, places the body, and holds a
        //    finite, nonzero, takeable stock. This is the fixture's own validation
        //    surface — the one place outside the driver allowed to read the settlement
        //    APIs, because checking the fixture is what they exist for. The episode
        //    driver never reads them.
        for seed in &task::TRAINING_LAYOUT_SEEDS[..seeds] {
            let prepared = task::Prepared::build_stage(*f, *seed, stage);
            let arena = prepared.fixture_arena();
            let stock = arena.resource_stock();
            let pose = arena.animal_pose();
            let stock_ok = stock.is_finite() && stock > 0.0;
            let pose_ok = pose.is_some_and(|p| p.is_finite());
            let births_ok = !arena.fauna.births_enabled();
            let ok = stock_ok && pose_ok && births_ok;
            if !ok {
                failures += 1;
            }
            println!(
                "  seed {seed:>3}  stock {stock:>8.4}  body {}  pose {}  births-off {}  {}",
                arena
                    .animal_id
                    .map_or("none".into(), |id| format!("id {id}")),
                pose_ok.then(|| "finite").unwrap_or("MISSING"),
                births_ok.then(|| "yes").unwrap_or("NO"),
                if ok { "ok" } else { "FAIL" },
            );
        }

        // 2. Controller smoke: the GRU centre and the three disclosed controls each run
        //    one short episode through the real driver on the first training layout —
        //    attached through the fauna's own controller table. The seam's behavioural
        //    check is in the table: the GRU must move the body, rest must not.
        let prepared = task::Prepared::build_stage(*f, task::TRAINING_LAYOUT_SEEDS[0], stage);
        let start = prepared.fixture_arena().animal_pose().expect("placed");
        let cancel = AtomicBool::new(false);
        let theta = if *f == Founder::Blind {
            crate::es::tensor::initial_center_shape::<23, 3>(task::TRAINING_SEED)
        } else {
            crate::es::tensor::initial_center_shape::<37, 3>(task::TRAINING_SEED)
        };
        let mut drivers: Vec<(String, EpisodeDriver)> = vec![(
            "gru-centre".into(),
            EpisodeDriver::gru(&theta, *f)
                .map_err(|e| format!("the seeded centre is not a policy: {e}"))?,
        )];
        for (name, control) in [
            ("no-intake", VoxelControl::NoIntake),
            ("stationary-feeding", VoxelControl::StationaryFeeding),
            ("cruise", VoxelControl::Cruise),
            ("heuristic", VoxelControl::Heuristic),
        ] {
            drivers.push((name.into(), EpisodeDriver::control(control, *f)));
        }
        println!(
            "  {:<20} {:>7} {:>6} {:>7} {:>8} {:>9} {:>9} {:>8}",
            "driver", "ticks", "alive", "moved", "updates", "fwd", "|turn|", "feed"
        );
        for (name, d) in &drivers {
            let out = driver::run_prepared(
                &prepared,
                d,
                ticks,
                super::driver::Limits::new(&cancel),
                &format!("check/{}/{name}", f.name()),
            );
            match out {
                Ok(e) => {
                    let moved = (e.pose_x - start.x).abs() + (e.pose_z - start.z).abs();
                    println!(
                        "  {:<20} {:>7} {:>6} {:>7.3} {:>8} {:>9.3} {:>9.3} {:>8.3}",
                        name,
                        e.ticks,
                        e.alive,
                        moved,
                        e.updates,
                        e.mean_forward,
                        e.mean_turn_abs,
                        e.mean_feed,
                    );
                    // The seam check: the GRU holds real forward effort and must have
                    // moved the body; the no-intake control holds rest and must not
                    // have. A control's mean actions are its own, from the body.
                    let seam_ok = match name.as_str() {
                        "gru-centre" => moved > 1e-6 && e.mean_forward > 0.05,
                        "no-intake" => moved == 0.0 && e.mean_forward == 0.0,
                        _ => e.updates == ticks / manifest.cadence_ticks(),
                    };
                    if !seam_ok || !e.score.score.is_finite() {
                        failures += 1;
                        println!("    FAIL: the seam or the measurements are not sane");
                    }
                }
                Err(err) => {
                    failures += 1;
                    println!("  {name:<20} FAIL: {err}");
                }
            }
        }
        println!();
    }

    print_counter_state();
    if failures == 0 {
        println!("check passed: both founders' arenas, GRU and controls run end to end.");
        println!();
        println!("Next: voxel-bench, then voxel-train --founder blind --stage a --updates 2.");
        Ok(())
    } else {
        Err(format!("{failures} check failure(s) above").into())
    }
}

/// `voxel-bench`: setup cost and episode throughput at a bounded worker count.
pub fn bench(
    founder: String,
    stage: String,
    ticks: u64,
    episodes: usize,
    workers: usize,
) -> Result<(), Boxed> {
    let stage = task::parse_stage(&stage)?;
    let worker_limit = task::episode_worker_limit();
    if workers == 0 || workers > worker_limit {
        return Err(format!("--workers must be in 1..={worker_limit}").into());
    }
    let founder = parse_founder(&founder)?;
    let fixture = task::PreparedArena::build_stage(founder, task::TRAINING_LAYOUT_SEEDS[0], stage);
    let prepared = task::Prepared::from(fixture.clone());
    let theta = if founder == Founder::Blind {
        crate::es::tensor::initial_center_shape::<23, 3>(task::TRAINING_SEED)
    } else {
        crate::es::tensor::initial_center_shape::<37, 3>(task::TRAINING_SEED)
    };
    let gru = EpisodeDriver::gru(&theta, founder)?;

    println!(
        "# voxel episode throughput — {} ({}), stage {}",
        founder.name(),
        founder.role(),
        stage.as_str(),
    );
    println!("# build {BUILD_ID}, {ticks} ticks per episode, controllers attached");
    println!();

    // Setup cost: rebuild from the seed vs clone the prepared layout, per episode.
    let builds = 8;
    let t = Instant::now();
    for _ in 0..builds {
        let _ = task::Prepared::build_stage(founder, task::TRAINING_LAYOUT_SEEDS[0], stage);
    }
    let rebuild = t.elapsed().as_secs_f64() / builds as f64;
    let t = Instant::now();
    for _ in 0..builds {
        let _ = fixture.episode_arena();
    }
    let clone = t.elapsed().as_secs_f64() / builds as f64;
    let t = Instant::now();
    for _ in 0..builds {
        let _ = fixture.rebuilt();
    }
    let arena_rebuild = t.elapsed().as_secs_f64() / builds as f64;
    println!(
        "setup per episode: rebuild {rebuild:.2} ms, arena-rebuild {arena_rebuild:.2} ms, \
         prepared-clone {clone:.2} ms"
    );

    // Single-episode throughput at one worker.
    let cancel = AtomicBool::new(false);
    let limits = super::driver::Limits::new(&cancel);
    let episodes = episodes.max(1);
    let t = Instant::now();
    let mut ticks_done = 0u64;
    for i in 0..episodes {
        let e = driver::run_prepared(&prepared, &gru, ticks, limits, &format!("bench/{i}"))
            .map_err(|err| format!("bench episode {i}: {err}"))?;
        ticks_done += e.ticks;
    }
    let single = t.elapsed().as_secs_f64();
    println!(
        "{episodes} episodes at 1 worker: {single:.3} s wall, {:.0} ticks/s, \
         {:.2} episodes/s, {:.2} ms per {ticks}-tick episode",
        ticks_done as f64 / single,
        episodes as f64 / single,
        single / episodes as f64 * 1000.0,
    );

    // The same episodes at `workers` episode workers.
    let jobs: Vec<usize> = (0..episodes * workers).collect();
    let cursor = AtomicUsize::new(0);
    let done = AtomicU64::new(0);
    let t = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= jobs.len() {
                        return;
                    }
                    match driver::run_prepared(
                        &prepared,
                        &gru,
                        ticks,
                        limits,
                        &format!("bench-multi/{}", jobs[i]),
                    ) {
                        Ok(e) => {
                            done.fetch_add(e.ticks, Ordering::SeqCst);
                        }
                        Err(EpisodeError::Cancelled { .. }) => return,
                        Err(EpisodeError::Invalid { detail, .. }) => {
                            panic!("bench episode invalid: {detail}");
                        }
                    }
                }
            });
        }
    });
    let multi = t.elapsed().as_secs_f64();
    let total = done.load(Ordering::SeqCst) as f64;
    println!(
        "{} episodes at {workers} workers: {multi:.3} s wall, {:.0} ticks/s aggregate, \
         {:.2} episodes/s aggregate (×{:.2} vs 1 worker)",
        jobs.len(),
        total / multi,
        jobs.len() as f64 / multi,
        (jobs.len() as f64 / multi) / (episodes as f64 / single),
    );
    println!();
    println!(
        "A full-horizon ({HORIZON}) episode costs about {:.3} s of one worker's time, and \
         {:.3} s of wall time at {workers} workers.",
        stage.horizon() as f64 / (ticks_done as f64 / single),
        stage.horizon() as f64 / (total / multi),
        HORIZON = stage.horizon(),
    );
    print_counter_state();
    Ok(())
}

/// `voxel-train`: the bounded ES run. Requires founder, controller, seed, episode limit,
/// worker count and wall cap; the heuristic slot carries no evolvable parameters, so it
/// is refused as a training controller by name.
#[allow(clippy::too_many_arguments)]
pub fn train(
    founder: String,
    stage: String,
    band: String,
    init_center: Option<PathBuf>,
    controller: String,
    pairs: usize,
    layouts: usize,
    updates: u32,
    horizon: Option<u64>,
    workers: usize,
    wall_seconds: u64,
    episode_limit: u64,
    train_seed: u64,
    center_eval: bool,
    out: PathBuf,
    p5: bool,
    per_generation: usize,
    held_out_every: u32,
) -> Result<(), Boxed> {
    let controller = controller.trim().to_ascii_lowercase();
    if controller != "gru" {
        return Err(format!(
            "--controller must be `gru` for a training run: the open-loop controls and \
             the heuristic slot carry no evolvable parameters. They are diagnostics in \
             voxel-check and voxel-evaluate."
        )
        .into());
    }
    let founder = parse_founder(&founder)?;
    let stage = task::parse_stage(&stage)?;
    let band = task::parse_band(&band)?;
    let horizon = horizon.unwrap_or_else(|| stage.horizon());
    let init_center = init_center
        .map(|path| trainer::InitCenter::load(&path, founder))
        .transpose()?;
    if out.exists() && std::fs::read_dir(&out).map_or(false, |d| d.count() > 0) {
        return Err(format!(
            "{} already holds a run; pass --overwrite to discard it",
            out.display()
        )
        .into());
    }
    let mix = if p5 {
        let t = Instant::now();
        let pool = super::landscape::training_pool(founder, workers)?;
        let held_out = super::landscape::held_out_pool(founder, workers)?;
        println!(
            "# P5-C mix: {} training landscape fixtures, {per_generation} drawn per \
             generation; {} held-out fixtures every {held_out_every} updates; founded in \
             {:.1} s",
            pool.len(),
            held_out.len(),
            t.elapsed().as_secs_f64()
        );
        Some(trainer::LandscapeMix {
            pool,
            per_generation,
            held_out,
            held_out_every,
            collapse_checkpoints: 4,
        })
    } else {
        None
    };
    let spec = TrainSpec {
        founder,
        stage,
        band,
        init_center,
        pairs,
        layouts,
        updates,
        horizon,
        workers,
        wall_seconds,
        episode_limit,
        train_seed,
        evaluate_center: center_eval,
        out: out.clone(),
        mix,
    };
    let cancel = AtomicBool::new(false);
    let report = trainer::train(&spec, &cancel)?;
    println!();
    println!(
        "# voxel training — {} ({}), stage {}{}, horizon {horizon}, seed {train_seed}",
        founder.name(),
        founder.role(),
        stage.as_str(),
        match stage {
            Stage::A => String::new(),
            Stage::B => format!(" band {}", band.as_str()),
        },
    );
    println!(
        "generations {}/{}  {}  episodes {} attempted / {} completed  ticks {}  \
         discarded {} ep / {} ticks",
        report.generations_completed,
        report.updates_requested,
        report.stop,
        report.episodes_attempted,
        report.episodes_completed,
        report.ticks_run,
        report.discarded.episodes_attempted,
        report.discarded.ticks_run,
    );
    if let Some(initial) = report.initial_center_score {
        println!("initial centre score {initial:.4}");
    }
    let evaluation = report
        .best
        .as_ref()
        .map(|best| evaluation_command(&out, founder, stage, band, &best.file));
    if let Some(best) = &report.best {
        println!(
            "best centre: generation {} score {:.4} → {}",
            best.generation, best.score, best.file
        );
    }
    for h in &report.held_out {
        println!(
            "held-out upd {:>3}  score {:.4}  survived {:.3}  {}",
            h.updates, h.score, h.survived, h.file
        );
    }
    if let Some(best) = &report.best_held_out {
        println!(
            "best held-out checkpoint: {} updates, score {:.4} → {}",
            best.updates, best.score, best.file
        );
    }
    if report.score_spreads.iter().all(|s| *s == 0.0) {
        println!();
        println!(
            "# no score spread: the candidates tied, so there was no ranking information \
             and the gradient was zero."
        );
    }
    println!();
    print_counter_state();
    println!("checkpoint {}", report.checkpoint);
    println!("wall {:.2} s", report.wall_seconds);
    if let Some(evaluation) = evaluation {
        println!("{evaluation}");
    }
    Ok(())
}

fn evaluation_command(
    out: &Path,
    founder: Founder,
    stage: Stage,
    band: task::Band,
    best_file: &str,
) -> String {
    format!(
        "evaluate with: cargo run --release -p cubarium-search -- voxel-evaluate --policy \
         {} --founder {} --stage {}{} --set training",
        out.join(best_file).display(),
        founder.name(),
        stage.as_str(),
        match stage {
            Stage::A => String::new(),
            Stage::B => format!(" --band {}", band.as_str()),
        },
    )
}

fn evaluation_jobs(layout_count: usize) -> Vec<(usize, String)> {
    (0..layout_count)
        .map(|li| (li, format!("eval/{li}")))
        .collect()
}

#[cfg(test)]
mod train_output_tests {
    use super::*;

    #[test]
    fn evaluation_command_names_the_selected_best_generation() {
        let command = evaluation_command(
            Path::new("/tmp/voxel-pilot"),
            Founder::Browser,
            Stage::B,
            task::Band::Near,
            "centers/gen7-center.json",
        );
        assert!(command.contains("/tmp/voxel-pilot/centers/gen7-center.json"));
        assert!(command.contains("--founder frondgrazer"));
        assert!(command.contains("--stage b"));
        assert!(command.contains("--band near"), "{command}");
        assert!(!command.contains("gen0-center.json"));
        // Stage A has no successor, so it carries no band on the command line.
        let a = evaluation_command(
            Path::new("/tmp/voxel-pilot"),
            Founder::Blind,
            Stage::A,
            task::Band::Landed,
            "centers/gen7-center.json",
        );
        assert!(!a.contains("--band"), "{a}");
    }

    #[test]
    fn evaluation_dispatches_each_layout_once_not_once_per_worker() {
        assert_eq!(
            evaluation_jobs(4),
            vec![
                (0, "eval/0".into()),
                (1, "eval/1".into()),
                (2, "eval/2".into()),
                (3, "eval/3".into()),
            ]
        );
    }
}

/// `voxel-evaluate`: one saved policy, or a disclosed control, over the training or the
/// held-out layout set, with the score components per layout.
#[allow(clippy::too_many_arguments)]
pub fn evaluate(
    policy: Option<PathBuf>,
    founder: Option<String>,
    controller: String,
    ablate_senses: bool,
    stage: String,
    band: String,
    policy_band: Option<String>,
    set: String,
    horizon: Option<u64>,
    workers: usize,
    wall_seconds: u64,
    episode_limit: u64,
    out: Option<PathBuf>,
) -> Result<(), Boxed> {
    let worker_limit = task::episode_worker_limit();
    if workers == 0 || workers > worker_limit {
        return Err(format!("--workers must be in 1..={worker_limit}").into());
    }
    let stage = task::parse_stage(&stage)?;
    let band = task::parse_band(&band)?;
    // Which band the *policy* was trained on. Absent means "this one": the evaluation is
    // unqualified and a centre from the other band is refused. Naming a different band
    // declares a deliberate transfer measurement and says so in the header.
    let policy_band = policy_band
        .map(|b| task::parse_band(&b))
        .transpose()?
        .unwrap_or(band);
    let horizon = horizon.unwrap_or_else(|| stage.horizon());
    let controller = controller.trim().to_ascii_lowercase();
    let (driver, founder) = match (policy, controller.as_str()) {
        (Some(path), "gru") => {
            let file = super::store::VoxelPolicyFile::load(&path)?;
            file.validate_for_band(&path.display().to_string(), policy_band)?;
            let f = file.founder()?;
            if let Some(named) = &founder {
                let want = parse_founder(named)?;
                if want != f {
                    return Err(format!(
                        "--founder {named} does not match the policy's lineage {}",
                        f.name()
                    )
                    .into());
                }
            }
            let driver = file.driver()?;
            let driver = if ablate_senses {
                driver.with_ablated_senses()
            } else {
                driver
            };
            (driver, f)
        }
        (Some(_), other) => {
            return Err(format!(
                "--policy was given but --controller is `{other}`; drop --policy to run a \
                 control, or use --controller gru to run the policy"
            )
            .into());
        }
        (None, other) => {
            let control = match other {
                "no-intake" => VoxelControl::NoIntake,
                "stationary-feeding" => VoxelControl::StationaryFeeding,
                "cruise" => VoxelControl::Cruise,
                "heuristic" => VoxelControl::Heuristic,
                "gru" => {
                    return Err("--controller gru needs --policy <path>".into());
                }
                other => {
                    return Err(format!(
                        "unknown --controller `{other}`; use `gru` (with --policy), \
                         `no-intake`, `stationary-feeding`, `cruise` or `heuristic`"
                    )
                    .into());
                }
            };
            let named = founder.as_deref().ok_or_else(|| {
                "--founder is required when a control runs (no policy names one)".to_string()
            })?;
            (
                EpisodeDriver::control(control, parse_founder(named)?),
                parse_founder(named)?,
            )
        }
    };
    let set = set.trim().to_ascii_lowercase();
    let prepared_layouts: Vec<(String, task::Prepared)> = match set.as_str() {
        "training" => task::training_layouts(founder, stage, band)
            .into_iter()
            .map(|p| (p.layout_seed().to_string(), p))
            .collect(),
        "holdout" | "evaluation" => task::evaluation_layouts(founder, stage, band)
            .into_iter()
            .map(|p| (p.layout_seed().to_string(), p))
            .collect(),
        "offset-sweep" | "offsets" => {
            if founder != Founder::Blind || stage != Stage::B {
                return Err("--set offset-sweep is the blind Stage-B diagnostic".into());
            }
            task::offset_sweep_layouts()
        }
        other => {
            return Err(format!(
                "unknown --set `{other}`; use `training`, `holdout` or `offset-sweep`"
            )
            .into());
        }
    };
    if episode_limit < prepared_layouts.len() as u64 {
        return Err(format!(
            "--episode-limit {episode_limit} is smaller than this set's {} layouts",
            prepared_layouts.len()
        )
        .into());
    }

    println!(
        "# voxel evaluate — {} ({}), driver `{}`, stage {}{}, set `{set}`, {} layouts",
        founder.name(),
        founder.role(),
        driver.name(),
        stage.as_str(),
        match stage {
            Stage::A => String::new(),
            Stage::B => format!(" band {}", band.as_str()),
        },
        prepared_layouts.len(),
    );
    if stage == Stage::B && policy_band != band {
        println!(
            "# DISCLOSED TRANSFER: the policy was trained on the `{}` band and is being \
             read on the `{}` one. Its protocol string is not this arena's.",
            policy_band.as_str(),
            band.as_str(),
        );
    }
    println!("# build {BUILD_ID}, horizon {horizon}, workers {workers}");
    println!();

    let deadline = Instant::now() + std::time::Duration::from_secs(wall_seconds.max(1));
    let cancel = AtomicBool::new(false);
    // Exactly one episode per case. Before P3 this accidentally queued one duplicate per
    // worker, so the default 64-episode cap covered only four of eight layouts at 16
    // workers and evaluation failed unless callers manually raised the cap.
    let jobs = evaluation_jobs(prepared_layouts.len());
    let slots: std::sync::Mutex<Vec<Option<driver::Episode>>> =
        std::sync::Mutex::new(vec![None; prepared_layouts.len()]);
    let cursor = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = cursor.fetch_add(1, Ordering::SeqCst);
                    if index >= jobs.len() || Instant::now() >= deadline {
                        return;
                    }
                    let (li, name) = &jobs[index];
                    match driver::run_prepared(
                        &prepared_layouts[*li].1,
                        &driver,
                        horizon,
                        super::driver::Limits::until(&cancel, deadline),
                        name,
                    ) {
                        Ok(e) => {
                            slots.lock().expect("slots")[*li] = Some(e);
                        }
                        Err(EpisodeError::Cancelled { .. }) => return,
                        Err(EpisodeError::Invalid { detail, .. }) => {
                            panic!("evaluation episode invalid: {detail}");
                        }
                    }
                }
            });
        }
    });
    let slots = slots.into_inner()?;
    if slots.iter().any(Option::is_none) {
        return Err(
            "the evaluation was stopped by its wall cap before every layout ran; \
             raise --wall-seconds or lower --horizon"
                .into(),
        );
    }

    // The start geometry beside each row: fixture-side, recomputed after the episode,
    // so a reader can see what separates the layouts a policy fed on from the rest
    // (P2-D step 3). Nothing here was ever in an observation.
    // The browser's own column: bites taken off a crown one voxel above its head, the
    // food only the vertical mouth reach can get at. Fixture-side, never in the score.
    println!(
        "{:<8} {:>7} {:>6} {:>10} {:>10} {:>8} {:>9} {:>6} {:>8} {:>5} {:>7} {:>7} {:>5} {:>5}",
        "case",
        "ticks",
        "alive",
        "intake",
        "motor",
        "survive",
        "score",
        "dist",
        "turn",
        "ate",
        "walk_m",
        "area_m2",
        "blk",
        "edge",
    );
    let mut rows = Vec::new();
    let mut geometry = Vec::new();
    for (li, (label, prepared)) in prepared_layouts.iter().enumerate() {
        let e = slots[li].as_ref().expect("checked complete");
        let g = prepared.start_geometry();
        println!(
            "{:<8} {:>7} {:>6} {:>10.4} {:>10.4} {:>8.3} {:>9.4} {:>6} {:>8} {:>5} {:>7.2} {:>7.2} {:>5.2} {:>5.2}",
            label,
            e.ticks,
            e.alive,
            e.score.intake_normalized,
            e.score.motor_loss_normalized,
            e.score.survival_term,
            e.score.score,
            g.map_or("-".into(), |g| format!("{:.2}", g.distance_m)),
            g.map_or("-".into(), |g| format!(
                "{:+.0}",
                g.turn_to_target_rad.to_degrees()
            )),
            if e.eaten_organic > 0.0 { "yes" } else { "no" },
            e.diagnostics.walked_m,
            e.diagnostics.unique_area_m2,
            e.diagnostics.blocked_motor_share,
            e.diagnostics.near_drop_or_edge_share,
        );
        rows.push(e.clone());
        geometry.push(g);
    }
    let mean_score = rows.iter().map(|e| e.score.score).sum::<f64>() / rows.len() as f64;
    let mean_intake =
        rows.iter().map(|e| e.score.intake_normalized).sum::<f64>() / rows.len() as f64;
    let alive = rows.iter().filter(|e| e.alive).count();
    let mut scores: Vec<f64> = rows.iter().map(|e| e.score.score).collect();
    scores.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let median = (scores[(scores.len() - 1) / 2] + scores[scores.len() / 2]) / 2.0;
    // Acquisition is a real bite: gross organic that crossed the mouth, read off the
    // ledger's own boundary counter. Settled intake is a conservation reading and
    // carries float residue of order 1e-15, which must never be reported as a founder
    // having found food.
    let fed = rows.iter().filter(|e| e.eaten_organic > 0.0).count();
    println!();
    println!(
        "mean score {mean_score:.4}  median score {median:.4}  mean intake {mean_intake:.4}  \
         acquired {fed}/{}  survived {alive}/{}",
        rows.len(),
        rows.len()
    );

    if founder == Founder::Blind && stage == Stage::B {
        let mut bins = [(0usize, 0usize); 4];
        for (row, start) in rows.iter().zip(&geometry) {
            let Some(start) = start else { continue };
            let turn = start.turn_to_target_rad.to_degrees();
            let side = usize::from(turn >= 0.0);
            let range = usize::from(turn.abs() > 90.0);
            let bin = side * 2 + range;
            bins[bin].1 += 1;
            bins[bin].0 += usize::from(row.eaten_organic > 0.0);
        }
        println!();
        println!(
            "signed acquisition: neg-near {}/{}  neg-far {}/{}  pos-near {}/{}  pos-far {}/{}",
            bins[0].0, bins[0].1, bins[1].0, bins[1].1, bins[2].0, bins[2].1, bins[3].0, bins[3].1,
        );
        if matches!(set.as_str(), "holdout" | "evaluation") {
            let negative = bins[0].0 + bins[1].0;
            let positive = bins[2].0 + bins[3].0;
            let gate = fed >= 6 && negative >= 2 && positive >= 2;
            println!(
                "balanced acquisition gate (>=6/8 total and >=2/4 on each side): {}",
                if gate { "MET" } else { "UNMET" }
            );
        }
    }

    // Stage B's accounting, reported *beside* the score and never inside it.
    let reacquisition: Vec<_> = rows.iter().filter_map(|e| e.reacquisition).collect();
    if !reacquisition.is_empty() {
        println!();
        println!(
            "{:<8} {:>11} {:>11} {:>10} {:>12} {:>10} {:>7} {:>9} {:>7}",
            "case",
            "patch1-take",
            "patch2-take",
            "depleted",
            "first-bite2",
            "reacquired",
            "sep-m",
            "min-succ-m",
            "sense-t",
        );
        for ((label, _), r) in prepared_layouts.iter().zip(&reacquisition) {
            let tick = |t: Option<u64>| t.map_or("-".into(), |t| t.to_string());
            println!(
                "{:<8} {:>11.5} {:>11.5} {:>10} {:>12} {:>10} {:>7.2} {:>9} {:>7}",
                label,
                r.initial_taken,
                r.successor_taken,
                tick(r.depleted_tick),
                tick(r.successor_first_bite_tick),
                r.reacquired,
                r.separation_m,
                r.min_successor_distance_m
                    .map_or("-".into(), |d| format!("{d:.2}")),
                r.sense_ticks,
            );
        }
        let depleted = reacquisition
            .iter()
            .filter(|r| r.depleted_tick.is_some())
            .count();
        let touched = reacquisition
            .iter()
            .filter(|r| r.successor_first_bite_tick.is_some())
            .count();
        let reacquired = reacquisition.iter().filter(|r| r.reacquired).count();
        println!(
            "initial patch depleted (below {:.0}% of its start) {depleted}/{n}  \
             successor bitten {touched}/{n}  reacquired {reacquired}/{n}",
            100.0 * task::DEPLETION_FRACTION,
            n = reacquisition.len(),
        );
        // The departure accounting (P3-B step 1). Fixture geometry read after the fact:
        // how far apart the two patches are, how close the founder actually got to the
        // successor once its own patch was empty, and how long it spent where it could
        // sense the successor at all. None of it is in an observation or the score.
        let separations: Vec<f64> = reacquisition.iter().map(|r| r.separation_m).collect();
        let approaches: Vec<f64> = reacquisition
            .iter()
            .filter_map(|r| r.min_successor_distance_m)
            .collect();
        let sense_ticks: u64 = reacquisition.iter().map(|r| r.sense_ticks).sum();
        let within = reacquisition.iter().filter(|r| r.sense_ticks > 0).count();
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
        let least = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
        println!(
            "separation {:.2}-{:.2} m (mean {:.2})  \
             post-depletion closest approach {}  \
             sensed radius {:.2} m: {within}/{n} layouts entered it, {sense_ticks} ticks total",
            least(&separations),
            separations.iter().copied().fold(0.0, f64::max),
            mean(&separations),
            if approaches.is_empty() {
                "- (nothing depleted)".to_string()
            } else {
                format!(
                    "min {:.2} m, mean {:.2} m over {}/{} depleted",
                    least(&approaches),
                    mean(&approaches),
                    approaches.len(),
                    depleted,
                )
            },
            reacquisition.first().map_or(0.0, |r| r.sensed_radius_m),
            n = reacquisition.len(),
        );
    }
    print_counter_state();
    if let Some(path) = out {
        let report = serde_json::json!({
            "build_id": BUILD_ID,
            "founder": founder.name(),
            "driver": driver.name(),
            "stage": stage.as_str(),
            "band": band.as_str(),
            "policy_band": policy_band.as_str(),
            "set": set,
            "cases": prepared_layouts.iter().map(|(label, _)| label).collect::<Vec<_>>(),
            "median_score": median,
            "horizon_ticks": horizon,
            "workers": workers,
            "counters": ScoreCounters::Landed,
            "rows": rows,
            "start_geometry": geometry,
            "mean_score": mean_score,
            "mean_intake": mean_intake,
            "alive": alive,
        });
        std::fs::create_dir_all(&path)?;
        std::fs::write(
            path.join("evaluation.json"),
            serde_json::to_string_pretty(&report)?,
        )?;
        println!("rows {}", path.join("evaluation.json").display());
    }
    Ok(())
}

/// `voxel-imitate`: record the fauna's own foraging heuristic on the **training**
/// layouts, at the controller period, as `(observation, adapted action)` streams — then
/// fit a GRU to them and save it as an ordinary centre.
///
/// The held-out seeds are not recorded and not fitted: a clone has seen the training task
/// only. Streams are disposable run output, one file per layout per stage; the clone is a
/// [`super::store::VoxelPolicyFile`] carrying its imitation provenance, loadable by
/// `voxel-train --init-center` and by `voxel-evaluate --policy`.
#[allow(clippy::too_many_arguments)]
pub fn imitate(
    founder: String,
    stages: String,
    band: String,
    layouts: usize,
    horizon: Option<u64>,
    workers: usize,
    fit: bool,
    updates: u32,
    learning_rate: f64,
    chunk: usize,
    clip: f64,
    fit_seed: u64,
    out: PathBuf,
    p5: bool,
    write: bool,
) -> Result<(), Boxed> {
    let worker_limit = task::episode_worker_limit();
    if workers == 0 || workers > worker_limit {
        return Err(format!("--workers must be in 1..={worker_limit}").into());
    }
    let founder = parse_founder(&founder)?;
    let band = task::parse_band(&band)?;
    let mut held_out: Vec<task::Prepared> = Vec::new();
    let stages: Vec<Stage> = stages
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(task::parse_stage)
        .collect::<Result<_, _>>()?;
    if stages.is_empty() {
        return Err("--stages needs at least one of `a`, `b`".into());
    }

    println!(
        "# voxel imitation streams — {} ({}), teacher `{}`, band {}, {} training layouts",
        founder.name(),
        founder.role(),
        imitate::TEACHER_CONTROLLER,
        band.as_str(),
        layouts.min(task::TRAINING_LAYOUT_SEEDS.len()),
    );
    println!("# build {BUILD_ID}, schema {}", imitate::TEACHER_SCHEMA);
    println!(
        "# held-out seeds {:?} are not recorded",
        task::EVALUATION_LAYOUT_SEEDS
    );
    println!();

    let mut all = Vec::new();
    let recorded = stages.clone();
    // P5-C (C4): the arena layouts on both grids per stage, then every training
    // landscape, one stream per acting body.
    let mut groups: Vec<(String, Vec<task::Prepared>)> = Vec::new();
    for &stage in &stages {
        let fixtures = if p5 {
            task::p5_arena_layouts(founder, stage, band)
        } else {
            task::training_layouts(founder, stage, band)
        };
        let n = layouts.min(fixtures.len());
        groups.push((
            format!("stage {}", stage.as_str()),
            fixtures.into_iter().take(n).collect(),
        ));
    }
    if p5 {
        let t = Instant::now();
        let pool = super::landscape::training_pool(founder, workers)?;
        held_out = super::landscape::held_out_pool(founder, workers)?;
        println!(
            "# founded {} training and {} held-out landscape fixtures in {:.1} s",
            pool.len(),
            held_out.len(),
            t.elapsed().as_secs_f64()
        );
        groups.push(("landscapes".into(), pool));
    }
    for (group, fixtures) in groups {
        let t = Instant::now();
        let streams = imitate::record_fixtures(founder, &fixtures, horizon, workers)?;
        let paths = if write {
            imitate::write_streams(&out.join("streams"), &streams)?
        } else {
            Vec::new()
        };
        let steps: usize = streams.iter().map(|s| s.steps).sum();
        let mut moving = 0usize;
        let mut feeding = 0usize;
        let mut turning = 0usize;
        for s in &streams {
            for t in 0..s.steps {
                let a = s.action(t);
                moving += usize::from(a[0] > 0.0);
                turning += usize::from(a[1] != 0.0);
                feeding += usize::from(a[2] > 0.0);
            }
        }
        let f = |n: usize| 100.0 * n as f64 / steps.max(1) as f64;
        println!(
            "{group}  {} fixtures  {} streams  {steps} steps  \
             forward {:.1}%  turning {:.1}%  feeding {:.1}%  {:.2} s  -> {}",
            fixtures.len(),
            streams.len(),
            f(moving),
            f(turning),
            f(feeding),
            t.elapsed().as_secs_f64(),
            paths.first().map_or_else(
                || out.display().to_string(),
                |p| p.parent().unwrap_or(&out).display().to_string()
            ),
        );
        all.extend(streams);
    }
    let steps: usize = all.iter().map(|s| s.steps).sum();
    let digest = imitate::streams_digest(&all);
    println!();
    println!(
        "{} streams, {steps} teacher steps, digest {digest:#018x}",
        all.len()
    );
    if !fit {
        return Ok(());
    }

    // The fit. Teacher-forced over the recorded streams: no episode runs here, so the
    // cost is the arithmetic and nothing else.
    let spec = imitate::FitSpec {
        updates,
        learning_rate,
        chunk,
        clip,
        seed: fit_seed,
        workers,
    };
    println!();
    println!(
        "# fitting a clone — {updates} Adam updates, lr {learning_rate}, TBPTT window \
         {chunk} steps, clip {clip}, seed {fit_seed}"
    );
    let t = Instant::now();
    let (theta, report) = imitate::fit(founder, &all, &spec);
    println!(
        "initial MSE  forward {:.5}  turn {:.5}  feed {:.5}",
        report.initial_mse[0], report.initial_mse[1], report.initial_mse[2]
    );
    println!(
        "final   MSE  forward {:.5}  turn {:.5}  feed {:.5}  (mean {:.5})",
        report.mse[0], report.mse[1], report.mse[2], report.mse_mean
    );
    println!(
        "turn-sign agreement {:.1}% of {} teacher steps   {:.1} s",
        100.0 * report.turn_sign_agreement,
        report.steps,
        t.elapsed().as_secs_f64(),
    );
    let tail: Vec<String> = report
        .loss_history
        .iter()
        .rev()
        .take(5)
        .rev()
        .map(|l| format!("{l:.5}"))
        .collect();
    println!("loss, last sampled updates: {}", tail.join(" -> "));

    // The clone declares the arena it is a policy *for*: Stage B under the recorded band
    // when Stage B was recorded, Stage A otherwise. That is what a later load checks.
    let clone_stage = if recorded.contains(&Stage::B) {
        Stage::B
    } else {
        Stage::A
    };
    let file = super::store::VoxelPolicyFile {
        schema: super::store::POLICY_SCHEMA.into(),
        build: BUILD_ID.into(),
        founder: founder.name().into(),
        digest: voxel_schema_digest(founder),
        train_seed: fit_seed,
        generation: None,
        score: None,
        start_heading: task::START_HEADING_PROTOCOL.into(),
        starting_stores: task::STARTING_STORES_PROTOCOL.into(),
        arena_protocol: task::arena_protocol(founder, clone_stage, band),
        protocol_hash: None,
        imitation: Some(super::store::ImitationProvenance {
            provenance: imitate::IMITATION_PROVENANCE.into(),
            teacher: imitate::TEACHER_CONTROLLER.into(),
            streams_fnv1a: digest,
            streams: all.len(),
            steps: report.steps,
            updates: report.updates,
            mse: report.mse,
            mse_mean: report.mse_mean,
            turn_sign_agreement: report.turn_sign_agreement,
        }),
        stage: clone_stage.as_str().into(),
        theta,
    };
    let clone_path = out.join("clone-center.json");
    file.write(&clone_path)?;
    // A clone that its own loader would refuse is not a seed. Check it here, not later.
    super::store::VoxelPolicyFile::load(&clone_path)?
        .validate_for_band(&clone_path.display().to_string(), band)?;
    println!();
    println!("clone {}", clone_path.display());
    if p5 {
        // C4: clone against its teacher, and against standing still, on the held-out
        // landscapes before any search.
        println!();
        let arms: Vec<HoldoutArm> = vec![
            (
                "heuristic".into(),
                EpisodeDriver::control(VoxelControl::Heuristic, founder),
            ),
            ("clone".into(), EpisodeDriver::gru(&file.theta, founder)?),
            (
                "stationary-feeding".into(),
                EpisodeDriver::control(VoxelControl::StationaryFeeding, founder),
            ),
        ];
        holdout_table(founder, &arms, &held_out, workers, false)?;
    }
    println!(
        "evaluate with: cargo run --release -p cubarium-search -- voxel-evaluate --policy {} \
         --founder {} --stage {} --band {} --set holdout",
        clone_path.display(),
        founder.name(),
        clone_stage.as_str(),
        band.as_str(),
    );
    Ok(())
}

/// `voxel-landscapes` (P5-B item 6): found the training (or held-out) landscape sets,
/// report what the founding kept and what preparing them cost, then measure episode
/// throughput — arena on both grids and landscape — at one worker and at `workers`.
/// A measurement, not a training run: the GRU is the seeded centre, the episodes are
/// `ticks` long, nothing is written.
pub fn landscapes(
    founder: String,
    presets: String,
    held_out: bool,
    ticks: u64,
    episodes: usize,
    workers: usize,
) -> Result<(), Boxed> {
    use super::landscape::{self, LandscapeSet};

    let founder = parse_founder(&founder)?;
    let worker_limit = task::episode_worker_limit();
    if workers == 0 || workers > worker_limit {
        return Err(format!("--workers must be in 1..={worker_limit}").into());
    }
    let wanted: Vec<&str> = presets.split(',').map(str::trim).collect();
    let sets: Vec<LandscapeSet> = if held_out {
        LandscapeSet::held_out()
    } else {
        LandscapeSet::training()
    }
    .into_iter()
    .filter(|s| wanted.contains(&s.preset.as_str()))
    .collect();
    if sets.is_empty() {
        return Err(format!("no landscape set on presets {presets:?}").into());
    }
    println!(
        "# voxel landscapes — {} ({}), {} sets, protocol {}, build {BUILD_ID}",
        founder.name(),
        founder.role(),
        sets.len(),
        landscape::LANDSCAPE_PROTOCOL
    );

    let t = Instant::now();
    let lands = landscape::found_landscapes(&sets, workers)?;
    let founding_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let fixtures = landscape::prepare_sets(&lands, &sets, founder);
    let prepare_s = t.elapsed().as_secs_f64();
    println!(
        "founded {} landscapes in {founding_s:.1} s wall at {workers} workers; prepared {} \
         {} fixtures in {prepare_s:.1} s",
        lands.len(),
        fixtures.len(),
        founder.name()
    );
    println!("landscape,preset,seed_base,world_seed,accepted,acting,bystanders,mid_shower");
    for l in &lands {
        let acting = l
            .placements()
            .iter()
            .filter(|p| p.founder == founder)
            .count();
        println!(
            "landscape,{},{},{},{},{acting},{},{}",
            l.preset,
            l.seed_base,
            l.world_seed,
            l.accepted,
            l.placements().len() - acting,
            l.has_mid_shower()
        );
    }

    let theta = if founder == Founder::Blind {
        crate::es::tensor::initial_center_shape::<23, 3>(task::TRAINING_SEED)
    } else {
        crate::es::tensor::initial_center_shape::<37, 3>(task::TRAINING_SEED)
    };
    let gru = EpisodeDriver::gru(&theta, founder)?;
    let cancel = AtomicBool::new(false);
    let limits = super::driver::Limits::new(&cancel);

    // One pool of fixtures per kind; the throughput is measured on the same pool at one
    // worker and at `workers`.
    let arena = |grid| -> Vec<task::Prepared> {
        task::TRAINING_LAYOUT_SEEDS
            .iter()
            .take(4)
            .map(|&seed| {
                task::PreparedArena::build_stage_on(
                    founder,
                    seed,
                    Stage::A,
                    task::Band::Landed,
                    grid,
                )
                .into()
            })
            .collect()
    };
    let pools: Vec<(String, Vec<task::Prepared>)> = vec![
        (
            "arena-0.25m".into(),
            arena(cubarium_voxel_sim::ArenaGrid::Standard),
        ),
        (
            "arena-0.125m".into(),
            arena(cubarium_voxel_sim::ArenaGrid::Fine),
        ),
    ]
    .into_iter()
    .chain(landscape::LANDSCAPE_PRESETS.iter().filter_map(|preset| {
        let pool: Vec<task::Prepared> = fixtures
            .iter()
            .filter(|f| f.preset == *preset)
            .cloned()
            .map(task::Prepared::from)
            .collect();
        (!pool.is_empty()).then(|| (format!("landscape-{preset}"), pool))
    }))
    .collect();

    println!();
    println!(
        "throughput,pool,bodies_acting,ticks_per_s_1_worker,ticks_per_s_per_worker_at_{workers},\
         aggregate_ticks_per_s_at_{workers},horizon_s_1_worker"
    );
    for (name, pool) in &pools {
        let run_one = |i: usize| -> Result<(u64, usize), Boxed> {
            let p = &pool[i % pool.len()];
            let e = driver::run_prepared_seeded(
                p,
                &gru,
                ticks,
                limits,
                &format!("{name}/{i}"),
                i as u64,
            )
            .map_err(|err| format!("{name} episode {i}: {err}"))?;
            Ok((e.ticks, e.bodies.len()))
        };
        let t = Instant::now();
        let mut done = 0u64;
        let mut bodies = 0usize;
        for i in 0..episodes.max(1) {
            let (ticks_run, n) = run_one(i)?;
            done += ticks_run;
            bodies += n;
        }
        let single = done as f64 / t.elapsed().as_secs_f64();
        let jobs = episodes.max(1) * workers;
        let cursor = AtomicUsize::new(0);
        let total = AtomicU64::new(0);
        let t = Instant::now();
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        let i = cursor.fetch_add(1, Ordering::SeqCst);
                        if i >= jobs {
                            return;
                        }
                        let (ticks_run, _) = run_one(i).expect("a measured episode completes");
                        total.fetch_add(ticks_run, Ordering::SeqCst);
                    }
                });
            }
        });
        let aggregate = total.load(Ordering::SeqCst) as f64 / t.elapsed().as_secs_f64();
        let horizon = pool[0].horizon().unwrap_or(Stage::A.horizon());
        println!(
            "throughput,{name},{:.1},{single:.0},{:.0},{aggregate:.0},{:.2}",
            bodies as f64 / episodes.max(1) as f64,
            aggregate / workers as f64,
            horizon as f64 / single,
        );
    }
    Ok(())
}

/// One arm of a held-out comparison: a name and the driver it runs.
pub type HoldoutArm = (String, EpisodeDriver);

/// Run every arm on the held-out landscapes (C2) and print one summary row per arm —
/// mean score, per preset, survival, intake, walking, blocked motor, time near a drop,
/// departures by cause — then, with `per_fixture`, one row per fixture per arm.
pub fn holdout_table(
    founder: Founder,
    arms: &[HoldoutArm],
    fixtures: &[task::Prepared],
    workers: usize,
    per_fixture: bool,
) -> Result<Vec<(String, f64)>, Boxed> {
    let cancel = AtomicBool::new(false);
    println!(
        "# held-out landscapes — {} ({}), {} fixtures, horizon {} ticks, protocol {}",
        founder.name(),
        founder.role(),
        fixtures.len(),
        super::landscape::LANDSCAPE_HORIZON_TICKS,
        super::landscape::LANDSCAPE_PROTOCOL,
    );
    println!(
        "holdout,arm,score,small,default,wide,survived,intake,motor,walked_m,unique_m2,\
         blocked_motor_share,near_drop_share,starved,drowned,bodies"
    );
    let mut rows = Vec::new();
    let mut per = Vec::new();
    for (name, d) in arms {
        let episodes = trainer::evaluate_fixtures(
            d,
            fixtures,
            workers,
            driver::Limits::new(&cancel),
            &format!("holdout/{name}"),
        )?;
        let (score, by_preset, survived) = trainer::held_out_summary(fixtures, &episodes);
        let n = episodes.len().max(1) as f64;
        let mean = |f: &dyn Fn(&driver::Episode) -> f64| episodes.iter().map(f).sum::<f64>() / n;
        let preset = |p: &str| {
            by_preset
                .iter()
                .find(|(k, _)| k == p)
                .map_or(f64::NAN, |(_, s)| *s)
        };
        let deaths = |i: usize| -> u64 {
            episodes
                .iter()
                .map(|e| e.diagnostics.deaths_by_cause[i])
                .sum()
        };
        let bodies: usize = episodes.iter().map(|e| e.bodies.len()).sum();
        println!(
            "holdout,{name},{score:.4},{:.4},{:.4},{:.4},{survived:.3},{:.4},{:.4},{:.2},{:.2},\
             {:.3},{:.3},{},{},{bodies}",
            preset("small"),
            preset("default"),
            preset("wide"),
            mean(&|e| e.intake_organic),
            mean(&|e| e.motor_organic),
            mean(&|e| e.diagnostics.walked_m),
            mean(&|e| e.diagnostics.unique_area_m2),
            mean(&|e| e.diagnostics.blocked_motor_share),
            mean(&|e| e.diagnostics.near_drop_or_edge_share),
            deaths(cubarium_voxel_fauna::Departure::Starved.index()),
            deaths(cubarium_voxel_fauna::Departure::Drowned.index()),
        );
        rows.push((name.clone(), score));
        if per_fixture {
            per.push((name.clone(), episodes));
        }
    }
    if per_fixture {
        println!(
            "fixture,arm,label,score,survived,intake,walked_m,blocked_motor_share,starved,drowned"
        );
        for (name, episodes) in &per {
            for e in episodes {
                println!(
                    "fixture,{name},{},{:.4},{:.3},{:.4},{:.2},{:.3},{},{}",
                    e.fixture,
                    e.score.score,
                    e.survived_fraction(),
                    e.intake_organic,
                    e.diagnostics.walked_m,
                    e.diagnostics.blocked_motor_share,
                    e.diagnostics.deaths_by_cause[cubarium_voxel_fauna::Departure::Starved.index()],
                    e.diagnostics.deaths_by_cause[cubarium_voxel_fauna::Departure::Drowned.index()],
                );
            }
        }
    }
    Ok(rows)
}

/// `voxel-holdout` (P5-C): the held-out landscapes against the controls named in
/// `controls` (comma-separated: `heuristic`, `stationary-feeding`, `no-intake`,
/// `cruise`) and each policy file.
pub fn holdout(
    founder: String,
    policies: Vec<PathBuf>,
    controls: String,
    workers: usize,
    per_fixture: bool,
) -> Result<(), Boxed> {
    let founder = parse_founder(&founder)?;
    let worker_limit = task::episode_worker_limit();
    if workers == 0 || workers > worker_limit {
        return Err(format!("--workers must be in 1..={worker_limit}").into());
    }
    let mut arms: Vec<HoldoutArm> = Vec::new();
    for c in controls.split(',').map(str::trim).filter(|c| !c.is_empty()) {
        let control = match c {
            "heuristic" => VoxelControl::Heuristic,
            "stationary-feeding" | "stationary" => VoxelControl::StationaryFeeding,
            "no-intake" => VoxelControl::NoIntake,
            "cruise" => VoxelControl::Cruise,
            other => return Err(format!("unknown control `{other}`").into()),
        };
        arms.push((c.to_string(), EpisodeDriver::control(control, founder)));
    }
    for path in &policies {
        let file = super::store::VoxelPolicyFile::load(path)?;
        let d = file.driver()?;
        if d.founder() != founder {
            return Err(format!("{} is not a {} policy", path.display(), founder.name()).into());
        }
        arms.push((path.display().to_string(), d));
    }
    let t = Instant::now();
    let fixtures = super::landscape::held_out_pool(founder, workers)?;
    println!(
        "# founded and prepared {} held-out fixtures in {:.1} s",
        fixtures.len(),
        t.elapsed().as_secs_f64()
    );
    holdout_table(founder, &arms, &fixtures, workers, per_fixture)?;
    Ok(())
}

/// `voxel-start-probe` (P5-C diagnostic, coordinator's horizon-or-economy question): the
/// held-out landscapes from the drawn starts, at several horizons, with and without the
/// other lineage; then, at t = 0, each acting body's distance to the nearest stocked
/// face and its cue, and what the heuristic's first minute does with it. A measurement;
/// nothing here reaches training.
pub fn start_probe(founder: String, horizons: String, workers: usize) -> Result<(), Boxed> {
    use cubarium_voxel_fauna::{Departure, FaunaConfig, Food, RouteMap};
    let founder = parse_founder(&founder)?;
    let horizons: Vec<u64> = horizons
        .split(',')
        .map(|h| h.trim().parse::<u64>())
        .collect::<Result<_, _>>()?;
    let t = Instant::now();
    let base = super::landscape::held_out_pool(founder, workers)?;
    println!(
        "# start probe — {}, {} held-out fixtures founded in {:.1} s",
        founder.name(),
        base.len(),
        t.elapsed().as_secs_f64()
    );
    let lands: Vec<super::landscape::PreparedLandscape> = base
        .iter()
        .map(|p| p.landscape().expect("landscapes").clone())
        .collect();
    let cancel = AtomicBool::new(false);
    let phys = *FaunaConfig::default().founder(founder);
    let foods: Vec<Food> = match founder {
        Founder::Blind => vec![Food::Litter, Food::CapTissue, Food::Carrion],
        Founder::Browser => vec![Food::Foliage],
    };
    println!(
        "probe,bystanders,horizon,arm,score,survived,starved,drowned,{},motor,maintenance,\
         walked_m,intake_per_upkeep",
        foods
            .iter()
            .map(|f| format!("intake_{}", f.name()))
            .collect::<Vec<_>>()
            .join(",")
    );
    for bystanders in [true, false] {
        for &h in &horizons {
            let fixtures: Vec<task::Prepared> = lands
                .iter()
                .map(|l| {
                    let l = l.clone().with_horizon(h);
                    let l = if bystanders {
                        l
                    } else {
                        l.without_bystanders()
                    };
                    task::Prepared::from(l)
                })
                .collect();
            for (name, control) in [
                ("stationary", VoxelControl::StationaryFeeding),
                ("cruise", VoxelControl::Cruise),
                ("heuristic", VoxelControl::Heuristic),
            ] {
                let d = EpisodeDriver::control(control, founder);
                let eps = trainer::evaluate_fixtures(
                    &d,
                    &fixtures,
                    workers,
                    driver::Limits::new(&cancel),
                    "probe",
                )?;
                let bodies: usize = eps.iter().map(|e| e.bodies.len()).sum();
                let n = eps.len().max(1) as f64;
                let mean = |f: &dyn Fn(&driver::Episode) -> f64| eps.iter().map(f).sum::<f64>() / n;
                let deaths = |d: Departure| -> u64 {
                    eps.iter()
                        .map(|e| e.diagnostics.deaths_by_cause[d.index()])
                        .sum()
                };
                // Intake by class per body: the class split is the lineage's gross, so
                // divide by the episode's acting bodies.
                let by_food: Vec<String> = foods
                    .iter()
                    .map(|f| {
                        let per_body = eps
                            .iter()
                            .map(|e| {
                                e.diagnostics.intake_by_food[f.index()]
                                    / e.bodies.len().max(1) as f64
                            })
                            .sum::<f64>()
                            / n;
                        format!("{per_body:.5}")
                    })
                    .collect();
                let intake = mean(&|e| e.intake_organic);
                let upkeep = mean(&|e| e.maintenance_organic);
                println!(
                    "probe,{bystanders},{h},{name},{:.4},{:.3},{},{},{},{:.5},{:.5},{:.2},{:.3}  ({bodies} bodies)",
                    mean(&|e| e.score.score),
                    mean(&driver::Episode::survived_fraction),
                    deaths(Departure::Starved),
                    deaths(Departure::Drowned),
                    by_food.join(","),
                    mean(&|e| e.motor_organic),
                    upkeep,
                    mean(&|e| e.diagnostics.walked_m),
                    intake / upkeep.max(1e-12),
                );
            }
        }
    }

    // t = 0: distance from each start to the nearest stocked face, and the cue there.
    // The starts are the evaluation's own (`run_prepared` seeds on the base).
    let manifest = founder.manifest();
    let slot = |name: &str| {
        manifest
            .modules
            .iter()
            .find(|m| m.name == name)
            .map(|m| m.offset)
    };
    let chem = slot("Chem(detritus)");
    let taste = slot("Taste(1)").expect("a taste module");
    println!(
        "start,fixture,body,dist_nearest_stock_m,dist_nearest_litter_m,cue0,cue_max_1min,\
         taste_gate_s,ate_1min"
    );
    let mut dists = Vec::new();
    let mut cue_nonzero = 0usize;
    let mut cue_gate = 0usize;
    let mut reached = Vec::new();
    let mut ate = 0usize;
    let mut total = 0usize;
    for land in &lands {
        let world = land.world();
        let view = world.view();
        let voxel_m = world.config().voxel_m;
        let width_m = f64::from(world.config().width) * voxel_m;
        let map = RouteMap::for_founder(&view, &phys);
        let fv = land.flora().view();
        let stocked: Vec<(f64, f64, bool)> = map
            .faces
            .iter()
            .filter_map(|f| {
                let s = cubarium_voxel_sim::edible_stock(founder, land.flora(), voxel_m, f.y, *f);
                (s > 0.0).then(|| {
                    let litter = fv.ground_at(*f).is_some_and(|g| g.litter > 0.0);
                    (
                        (f64::from(f.x) + 0.5) * voxel_m,
                        (f64::from(f.z) + 0.5) * voxel_m,
                        litter,
                    )
                })
            })
            .collect();
        let starts = land.acting_starts(land.seed_base);
        let one = task::Prepared::from(land.clone().with_horizon(1_200));
        let sink = super::controller::teacher_sink();
        let d = EpisodeDriver::control(VoxelControl::Heuristic, founder).recording(sink.clone());
        let e = driver::run_prepared(&one, &d, 1_200, driver::Limits::new(&cancel), "probe/start")?;
        let buffers = sink.lock().expect("sink").clone();
        let cadence_s = manifest.cadence_ticks() as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ);
        for (k, p) in starts.iter().enumerate() {
            let (x, z) = (
                (f64::from(p.site.x) + 0.5) * voxel_m,
                (f64::from(p.site.z) + 0.5) * voxel_m,
            );
            let dist = |litter_only: bool| {
                stocked
                    .iter()
                    .filter(|s| !litter_only || s.2)
                    .map(|s| {
                        let dx = (s.0 - x).abs();
                        let dx = dx.min(width_m - dx);
                        (dx * dx + (s.1 - z).powi(2)).sqrt()
                    })
                    .fold(f64::INFINITY, f64::min)
            };
            let steps = buffers.get(k).cloned().unwrap_or_default();
            let cue =
                |i: usize| chem.map_or(0.0, |c| steps.get(i).map_or(0.0, |s| s.observation[c]));
            let cue0 = cue(0);
            let cue_max = (0..steps.len()).map(cue).fold(0.0, f64::max);
            let gate = steps
                .iter()
                .position(|s| s.observation[taste + 2] > 0.5 && s.observation[taste] > 0.2)
                .map(|i| i as f64 * cadence_s);
            let ate_1 = e.bodies.get(k).is_some_and(|b| b.intake_organic > 0.0);
            let d_any = dist(false);
            println!(
                "start,{},{k},{d_any:.2},{:.2},{cue0:.4},{cue_max:.4},{},{ate_1}",
                land.label(),
                dist(true),
                gate.map_or("never".to_string(), |g| format!("{g:.1}")),
            );
            dists.push(d_any);
            cue_nonzero += usize::from(cue0 > 0.0);
            cue_gate += usize::from(cue0 >= 0.02);
            if let Some(g) = gate {
                reached.push(g);
            }
            ate += usize::from(ate_1);
            total += 1;
        }
    }
    dists.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let q = |f: f64| dists[((dists.len() - 1) as f64 * f).round() as usize];
    reached.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    println!(
        "summary,{total} starts; nearest stocked face m: p10 {:.2} median {:.2} p90 {:.2} max {:.2}; \
         cue0 > 0: {cue_nonzero}; cue0 >= 0.02 (the heuristic's follow gate): {cue_gate}; \
         taste gate reached in the first minute: {} (median {:.1} s); ate in the first minute: {ate}",
        q(0.1),
        q(0.5),
        q(0.9),
        q(1.0),
        reached.len(),
        reached.get(reached.len() / 2).copied().unwrap_or(f64::NAN),
    );
    Ok(())
}

/// `voxel-flora-cost` (P5-C S1): the cost of one shredder landscape episode on the
/// held-out landscapes of `presets`, heuristic, one thread, with the plant leg off and
/// on, at `ticks`.
pub fn flora_cost(presets: String, ticks: u64, workers: usize) -> Result<(), Boxed> {
    let founder = Founder::Blind;
    let wanted: Vec<&str> = presets.split(',').map(str::trim).collect();
    let base: Vec<super::landscape::PreparedLandscape> =
        super::landscape::held_out_pool(founder, workers)?
            .into_iter()
            .filter_map(|p| p.landscape().cloned())
            .filter(|l| wanted.contains(&l.preset.as_str()))
            .collect();
    let cancel = AtomicBool::new(false);
    let d = EpisodeDriver::control(VoxelControl::Heuristic, founder);
    println!("flora_cost,preset,live_plants,episodes,ticks,seconds_per_episode,us_per_tick,score");
    for preset in &wanted {
        for live in [false, true] {
            let fixtures: Vec<task::Prepared> = base
                .iter()
                .filter(|l| l.preset == *preset)
                .map(|l| task::Prepared::from(l.clone().with_horizon(ticks).with_live_plants(live)))
                .collect();
            let t = Instant::now();
            let mut score = 0.0;
            for f in &fixtures {
                let e = driver::run_prepared(f, &d, ticks, driver::Limits::new(&cancel), "cost")?;
                score += e.score.score;
            }
            let s = t.elapsed().as_secs_f64() / fixtures.len().max(1) as f64;
            println!(
                "flora_cost,{preset},{live},{},{ticks},{s:.3},{:.1},{:.4}",
                fixtures.len(),
                1e6 * s / ticks as f64,
                score / fixtures.len().max(1) as f64
            );
        }
    }
    Ok(())
}
