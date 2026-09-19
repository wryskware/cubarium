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
    let prepared = task::Prepared::build_stage(founder, task::TRAINING_LAYOUT_SEEDS[0], stage);
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
        let _ = prepared.episode_arena();
    }
    let clone = t.elapsed().as_secs_f64() / builds as f64;
    let t = Instant::now();
    for _ in 0..builds {
        let _ = prepared.rebuilt();
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
    let horizon = horizon.unwrap_or_else(|| stage.horizon());
    if out.exists() && std::fs::read_dir(&out).map_or(false, |d| d.count() > 0) {
        return Err(format!(
            "{} already holds a run; pass --overwrite to discard it",
            out.display()
        )
        .into());
    }
    let spec = TrainSpec {
        founder,
        stage,
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
    };
    let cancel = AtomicBool::new(false);
    let report = trainer::train(&spec, &cancel)?;
    println!();
    println!(
        "# voxel training — {} ({}), stage {}, horizon {horizon}, seed {train_seed}",
        founder.name(),
        founder.role(),
        stage.as_str(),
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
        .map(|best| evaluation_command(&out, founder, stage, &best.file));
    if let Some(best) = &report.best {
        println!(
            "best centre: generation {} score {:.4} → {}",
            best.generation, best.score, best.file
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

fn evaluation_command(out: &Path, founder: Founder, stage: Stage, best_file: &str) -> String {
    format!(
        "evaluate with: cargo run --release -p cubarium-search -- voxel-evaluate --policy \
         {} --founder {} --stage {} --set training",
        out.join(best_file).display(),
        founder.name(),
        stage.as_str(),
    )
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
            "centers/gen7-center.json",
        );
        assert!(command.contains("/tmp/voxel-pilot/centers/gen7-center.json"));
        assert!(command.contains("--founder frondgrazer"));
        assert!(command.contains("--stage b"));
        assert!(!command.contains("gen0-center.json"));
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
    let horizon = horizon.unwrap_or_else(|| stage.horizon());
    let controller = controller.trim().to_ascii_lowercase();
    let (driver, founder) = match (policy, controller.as_str()) {
        (Some(path), "gru") => {
            let file = super::store::VoxelPolicyFile::load(&path)?;
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
    let seeds: &[u64] = match set.as_str() {
        "training" => &task::TRAINING_LAYOUT_SEEDS,
        "holdout" | "evaluation" => &task::EVALUATION_LAYOUT_SEEDS,
        other => {
            return Err(format!("unknown --set `{other}`; use `training` or `holdout`").into());
        }
    };
    let prepared_layouts = task::evaluation_layouts(founder, stage)
        .into_iter()
        .chain(task::training_layouts(founder, stage))
        .filter(|p| seeds.contains(&p.layout_seed))
        .collect::<Vec<_>>();
    assert_eq!(
        prepared_layouts.len(),
        seeds.len(),
        "the frozen sets cover every seed"
    );

    println!(
        "# voxel evaluate — {} ({}), driver `{}`, stage {}, set `{set}`, {} layouts",
        founder.name(),
        founder.role(),
        driver.name(),
        stage.as_str(),
        seeds.len(),
    );
    println!("# build {BUILD_ID}, horizon {horizon}, workers {workers}");
    println!();

    let deadline = Instant::now() + std::time::Duration::from_secs(wall_seconds.max(1));
    let cancel = AtomicBool::new(false);
    let mut jobs: Vec<(usize, String)> = (0..prepared_layouts.len())
        .flat_map(|li| (0..workers).map(move |w| (li, format!("eval/{li}/{w}"))))
        .collect();
    jobs.truncate(episode_limit.max(1) as usize);
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
                        &prepared_layouts[*li],
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
    println!(
        "{:<8} {:>7} {:>6} {:>10} {:>10} {:>8} {:>9} {:>6} {:>8} {:>5}",
        "seed", "ticks", "alive", "intake", "motor", "survive", "score", "dist", "turn", "ate"
    );
    let mut rows = Vec::new();
    let mut geometry = Vec::new();
    for (li, seed) in seeds.iter().enumerate() {
        let e = slots[li].as_ref().expect("checked complete");
        let g = prepared_layouts[li].start_geometry();
        println!(
            "{:<8} {:>7} {:>6} {:>10.4} {:>10.4} {:>8.3} {:>9.4} {:>6} {:>8} {:>5}",
            seed,
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

    // Stage B's accounting, reported *beside* the score and never inside it.
    let reacquisition: Vec<_> = rows.iter().filter_map(|e| e.reacquisition).collect();
    if !reacquisition.is_empty() {
        println!();
        println!(
            "{:<8} {:>11} {:>11} {:>10} {:>12} {:>12}",
            "seed", "patch1-take", "patch2-take", "depleted", "first-bite2", "reacquired"
        );
        for (seed, r) in seeds.iter().zip(&reacquisition) {
            let tick = |t: Option<u64>| t.map_or("-".into(), |t| t.to_string());
            println!(
                "{:<8} {:>11.5} {:>11.5} {:>10} {:>12} {:>12}",
                seed,
                r.initial_taken,
                r.successor_taken,
                tick(r.depleted_tick),
                tick(r.successor_first_bite_tick),
                r.reacquired,
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
    }
    print_counter_state();
    if let Some(path) = out {
        let report = serde_json::json!({
            "build_id": BUILD_ID,
            "founder": founder.name(),
            "driver": driver.name(),
            "stage": stage.as_str(),
            "set": set,
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
