//! The `voxel-*` command family: check, bench, train and evaluate — the plan's one
//! discoverable family for the phase-one arena, following the `es-*` subcommand
//! conventions (clap subcommands of `cubarium-search`, printed tables, JSON stores under
//! `runs/`).
//!
//! - [`check`]: arena validity for both founders across the training seeds, then a
//!   controller smoke (GRU centre and the disclosed controls) through the real episode
//!   driver.
//! - [`bench`]: measured setup cost (rebuild vs prepared-clone), episode throughput, and
//!   one versus four episode workers.
//! - [`train`]: the bounded ES run ([`super::trainer::train`]).
//! - [`evaluate`]: a saved policy or a disclosed control over the training or the
//!   held-out layout set, with the score components per layout.
//!
//! The commands require arena/founder, controller, seed, episode limit, worker count and
//! wall-time cap as applicable, and the worker count is capped at the plan's four
//! everywhere.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_voxel_fauna::Founder;

use super::controller::{EpisodeDriver, ObservationSource, VoxelControl};
use super::driver::{self, EpisodeError};
use super::task;
use super::trainer::{self, TrainSpec};
use super::{parse_founder, voxel_schema_digest};
use crate::evaluate::BUILD_ID;

type Boxed = Box<dyn std::error::Error>;

/// The observation source a command runs under. `self-only` is the interim default until
/// P1-C lands the real samplers; `zeros` is the honest sensory floor.
fn parse_source(name: &str) -> Result<&'static dyn ObservationSource, String> {
    match name.trim().to_ascii_lowercase().as_str() {
        "self-only" | "self" | "default" => Ok(&super::controller::SELF_ONLY),
        "zeros" | "none" => Ok(&super::controller::ZEROS),
        other => Err(format!(
            "unknown observation source `{other}`; use `self-only` (interim default) or \
             `zeros`"
        )),
    }
}

/// `voxel-check`: arena validity plus a heuristic/GRU smoke, through the real driver.
pub fn check(
    founder: Option<String>,
    seeds: usize,
    ticks: u64,
) -> Result<(), Boxed> {
    let founders: Vec<Founder> = match founder.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => vec![parse_founder(name)?],
        None => Founder::ALL.into(),
    };
    let seeds = seeds.clamp(1, task::TRAINING_LAYOUT_SEEDS.len());

    println!("# voxel phase-one arena check");
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
            let prepared = task::Prepared::build(*f, *seed);
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
                arena.animal_id.map_or("none".into(), |id| format!("id {id}")),
                pose_ok.then(|| "finite").unwrap_or("MISSING"),
                births_ok.then(|| "yes").unwrap_or("NO"),
                if ok { "ok" } else { "FAIL" },
            );
        }

        // 2. Controller smoke: the GRU centre and the three disclosed controls each run
        //    one short episode through the real driver on the first training layout.
        let prepared = task::Prepared::build(*f, task::TRAINING_LAYOUT_SEEDS[0]);
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
            ("heuristic", VoxelControl::Stub),
        ] {
            drivers.push((name.into(), EpisodeDriver::Control(control)));
        }
        println!(
            "  {:<20} {:>7} {:>7} {:>8} {:>9} {:>9} {:>8}",
            "driver", "ticks", "alive", "updates", "fwd", "|turn|", "feed"
        );
        for (name, d) in &drivers {
            let started = Instant::now();
            let out = driver::run_prepared(
                &prepared,
                d,
                &super::controller::SELF_ONLY,
                ticks,
                super::driver::Limits::new(&cancel),
                &format!("check/{}/{name}", f.name()),
            );
            match out {
                Ok(e) => {
                    println!(
                        "  {:<20} {:>7} {:>7} {:>8} {:>9.3} {:>9.3} {:>8.3}  ({:.3}s)",
                        name,
                        e.ticks,
                        e.alive,
                        e.updates,
                        e.mean_forward,
                        e.mean_turn_abs,
                        e.mean_feed,
                        started.elapsed().as_secs_f64(),
                    );
                    let bounds_ok = e
                        .score
                        .score
                        .is_finite()
                        && e.updates > 0
                        && e.mean_forward.is_finite();
                    if !bounds_ok {
                        failures += 1;
                        println!("    FAIL: the smoke episode's measurements are not sane");
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

    if failures == 0 {
        println!("check passed: both founders' arenas, GRU and controls run end to end.");
        println!();
        println!("Next: voxel-bench, then voxel-train --founder blind --updates 2.");
        Ok(())
    } else {
        Err(format!("{failures} check failure(s) above").into())
    }
}

/// `voxel-bench`: setup cost, episode throughput, one versus four episode workers.
pub fn bench(founder: String, ticks: u64, episodes: usize, workers: usize, source: String) -> Result<(), Boxed> {
    if workers == 0 || workers > task::MAX_EPISODE_WORKERS {
        return Err(format!("--workers must be in 1..={}", task::MAX_EPISODE_WORKERS).into());
    }
    let founder = parse_founder(&founder)?;
    let source_name = source.clone();
    let source = parse_source(&source)?;
    let prepared = task::Prepared::build(founder, task::TRAINING_LAYOUT_SEEDS[0]);
    let theta = if founder == Founder::Blind {
        crate::es::tensor::initial_center_shape::<23, 3>(task::TRAINING_SEED)
    } else {
        crate::es::tensor::initial_center_shape::<37, 3>(task::TRAINING_SEED)
    };
    let gru = EpisodeDriver::gru(&theta, founder)?;

    println!("# voxel episode throughput — {} ({})", founder.name(), founder.role());
    println!("# build {BUILD_ID}, {ticks} ticks per episode, observation source `{source_name}`");
    println!();

    // Setup cost: rebuild from the seed vs clone the prepared layout, per episode.
    let builds = 8;
    let t = Instant::now();
    for _ in 0..builds {
        let _ = task::Prepared::build(founder, task::TRAINING_LAYOUT_SEEDS[0]);
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
        let e = driver::run_prepared(
            &prepared,
            &gru,
            source,
            ticks,
            limits,
            &format!("bench/{i}"),
        )
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
            scope.spawn(|| loop {
                let i = cursor.fetch_add(1, Ordering::SeqCst);
                if i >= jobs.len() {
                    return;
                }
                match driver::run_prepared(
                    &prepared,
                    &gru,
                    source,
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
        ticks as f64 / (ticks_done as f64 / single),
        ticks as f64 / (total / multi),
        HORIZON = task::HORIZON_TICKS,
    );
    Ok(())
}

/// `voxel-train`: the bounded ES run. Requires founder, controller, seed, episode limit,
/// worker count and wall cap; the heuristic slot carries no evolvable parameters, so it
/// is refused as a training controller by name.
#[allow(clippy::too_many_arguments)]
pub fn train(
    founder: String,
    controller: String,
    pairs: usize,
    layouts: usize,
    updates: u32,
    horizon: u64,
    workers: usize,
    wall_seconds: u64,
    episode_limit: u64,
    train_seed: u64,
    center_eval: bool,
    source: String,
    out: PathBuf,
) -> Result<(), Boxed> {
    let controller = controller.trim().to_ascii_lowercase();
    if controller != "gru" {
        return Err(format!(
            "--controller must be `gru` for a training run: the heuristic slot carries no \
             evolvable parameters. The heuristic is a diagnostic in voxel-check and \
             voxel-evaluate."
        )
        .into());
    }
    let founder = parse_founder(&founder)?;
    let source = parse_source(&source)?;
    if out.exists() && std::fs::read_dir(&out).map_or(false, |d| d.count() > 0) {
        return Err(format!(
            "{} already holds a run; pass --overwrite to discard it",
            out.display()
        )
        .into());
    }
    let spec = TrainSpec {
        founder,
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
    let report = trainer::train(&spec, source, &cancel)?;
    println!();
    println!(
        "# {} training — {} ({}), seed {train_seed}",
        "voxel",
        founder.name(),
        founder.role()
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
             and the gradient was zero. With the interim `self-only` observations and \
             P1-B's action intake not yet landed, an idle body survives the horizon and \
             every candidate scores its survival term. This is the expected state until \
             the required P1-B/P1-C interfaces land, not a trained result."
        );
    }
    println!();
    println!("checkpoint {}", report.checkpoint);
    println!("wall {:.2} s", report.wall_seconds);
    println!(
        "evaluate with: cargo run --release -p cubarium-search -- voxel-evaluate --policy \
         {}/centers/gen0-center.json --founder {} --set training",
        out.display(),
        founder.name()
    );
    Ok(())
}

/// `voxel-evaluate`: one saved policy, or a disclosed control, over the training or the
/// held-out layout set, with the score components per layout.
#[allow(clippy::too_many_arguments)]
pub fn evaluate(
    policy: Option<PathBuf>,
    founder: Option<String>,
    controller: String,
    set: String,
    horizon: u64,
    workers: usize,
    wall_seconds: u64,
    episode_limit: u64,
    source: String,
    out: Option<PathBuf>,
) -> Result<(), Boxed> {
    if workers == 0 || workers > task::MAX_EPISODE_WORKERS {
        return Err(format!("--workers must be in 1..={}", task::MAX_EPISODE_WORKERS).into());
    }
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
            (file.driver()?, f)
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
                "heuristic" => VoxelControl::Stub,
                "gru" => {
                    return Err("--controller gru needs --policy <path>".into());
                }
                other => {
                    return Err(format!(
                        "unknown --controller `{other}`; use `gru` (with --policy), \
                         `no-intake`, `stationary-feeding` or `heuristic`"
                    )
                    .into());
                }
            };
            let named = founder.as_deref().ok_or_else(|| {
                "--founder is required when a control runs (no policy names one)".to_string()
            })?;
            (EpisodeDriver::Control(control), parse_founder(named)?)
        }
    };
    let set = set.trim().to_ascii_lowercase();
    let seeds: &[u64] = match set.as_str() {
        "training" => &task::TRAINING_LAYOUT_SEEDS,
        "holdout" | "evaluation" => &task::EVALUATION_LAYOUT_SEEDS,
        other => return Err(format!("unknown --set `{other}`; use `training` or `holdout`").into()),
    };
    let source = parse_source(&source)?;
    let prepared_layouts = task::evaluation_layouts(founder)
        .into_iter()
        .chain(task::training_layouts(founder))
        .filter(|p| seeds.contains(&p.layout_seed))
        .collect::<Vec<_>>();
    assert_eq!(prepared_layouts.len(), seeds.len(), "the frozen sets cover every seed");

    println!(
        "# voxel evaluate — {} ({}), driver `{}`, set `{set}`, {} layouts",
        founder.name(),
        founder.role(),
        driver.name(),
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
            scope.spawn(|| loop {
                let index = cursor.fetch_add(1, Ordering::SeqCst);
                if index >= jobs.len() || Instant::now() >= deadline {
                    return;
                }
                let (li, name) = &jobs[index];
                match driver::run_prepared(
                    &prepared_layouts[*li],
                    &driver,
                    source,
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
            });
        }
    });
    let slots = slots.into_inner()?;
    if slots.iter().any(Option::is_none) {
        return Err("the evaluation was stopped by its wall cap before every layout ran; \
                    raise --wall-seconds or lower --horizon"
            .into());
    }

    println!(
        "{:<8} {:>7} {:>6} {:>10} {:>10} {:>8} {:>9}",
        "seed", "ticks", "alive", "intake", "motor", "survive", "score"
    );
    let mut rows = Vec::new();
    for (li, seed) in seeds.iter().enumerate() {
        let e = slots[li].as_ref().expect("checked complete");
        println!(
            "{:<8} {:>7} {:>6} {:>10.4} {:>10.4} {:>8.3} {:>9.4}",
            seed,
            e.ticks,
            e.alive,
            e.score.intake_normalized,
            e.score.motor_loss_normalized,
            e.score.survival_term,
            e.score.score,
        );
        rows.push(e.clone());
    }
    let mean_score = rows.iter().map(|e| e.score.score).sum::<f64>() / rows.len() as f64;
    let mean_intake = rows.iter().map(|e| e.score.intake_normalized).sum::<f64>() / rows.len() as f64;
    let alive = rows.iter().filter(|e| e.alive).count();
    println!();
    println!(
        "mean score {mean_score:.4}  mean intake {mean_intake:.4}  survived {alive}/{}",
        rows.len()
    );
    if let Some(path) = out {
        let report = serde_json::json!({
            "build_id": BUILD_ID,
            "founder": founder.name(),
            "driver": driver.name(),
            "set": set,
            "horizon_ticks": horizon,
            "workers": workers,
            "rows": rows,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The source names parse, and an unknown name is refused with the two options.
    #[test]
    fn the_observation_source_names_parse() {
        let self_only = parse_source("self-only").expect("ok");
        let again = parse_source("self").expect("ok");
        assert!(std::ptr::eq(self_only, again), "the same static, by every name");
        let zeros = parse_source("zeros").expect("ok");
        let again = parse_source("none").expect("ok");
        assert!(std::ptr::eq(zeros, again));
        assert!(!std::ptr::eq(self_only, zeros), "the two prototypes are distinct");
        assert!(parse_source("chem").is_err());
    }
}
