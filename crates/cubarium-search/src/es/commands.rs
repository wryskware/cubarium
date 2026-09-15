//! The trainer's development commands: the frozen protocol, the fixture controls, the
//! plumbing smoke, a throughput measurement, the learning run and the policy export.
//!
//! Every one of these builds its own isolated worlds and throws them away. None of them
//! touches the display world, migrates a world, or attaches a policy to anything outside its
//! own fixture.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::episode::{self, Control, Driver, Episode, EpisodeError, Limits};
use super::export::PolicyFile;
use super::fixture::{self, HORIZON_TICKS};
use super::optimizer::Adam;
use super::tensor::{self, PARAMS};
use super::trainer::{
    self, CenterRecord, Checkpoint, GenerationError, GenerationReport, Plan, Protocol,
    run_generation, score,
};
use crate::evaluate::BUILD_ID;

/// The three controls, in the order the table reports them.
pub const CONTROLS: [Control; 3] =
    [Control::NoIntake, Control::StationaryGrazing, Control::MobileScript];

type Boxed = Box<dyn std::error::Error>;

/// Print the frozen protocol: everything a score depends on that is not the policy.
pub fn protocol() {
    let training = fixture::training_layouts();
    let holdout = fixture::holdout_layouts();
    let p = Protocol::new(16, HORIZON_TICKS, 20_260_915, &training);

    println!("# R2a training protocol (frozen)");
    println!("build                {BUILD_ID}");
    println!("schema               {}", p.schema);
    println!("protocol hash        {:#018x}", p.hash());
    println!("policy schema digest {:#018x}", p.policy_digest);
    println!("parameters           {PARAMS}");
    println!();
    println!("## optimizer: antithetic Gaussian ES, centred-rank utilities, Adam ascent");
    println!("sigma                {} (fixed; no adaptive sigma, no mutation rescaling)", p.sigma);
    println!("learning rate        {}", p.learning_rate);
    println!("beta1 / beta2 / eps  {} / {} / {} (bias-corrected, no weight decay)", p.beta1, p.beta2, p.adam_eps);
    println!("utilities            average ranks over 2n scores, u = rank/(2n-1) - 0.5");
    println!("gradient             g = sum_i (u+_i - u-_i) eps_i / (2 n sigma)");
    println!("flatten order        w_i, w_h, b_i, b_h, w_o, b_o; each row-major, gates (r,z,n)");
    println!("initialisation       {}", p.init);
    println!();
    println!("## score (frozen before the smoke)");
    println!("score = t_min + {} * mean(clip(usable terminal stores / body capacity, 0, 1))", p.store_weight);
    println!("t_min is the minimum survival ticks over the candidate's layouts; a dead animal");
    println!("contributes zero stores. The whole secondary term is at most {} of one tick.", p.store_weight);
    println!();
    println!("## episode");
    println!("horizon              {} ticks ({:.0} s at 20 Hz)", p.horizon_ticks, fixture::seconds(p.horizon_ticks));
    println!("body                 one mature founder (hue {}), structure = structure_adult,", fixture::FOUNDER_HUE);
    println!("                     reserve {} of R_max, energy {} of E_max, births disabled", fixture::START_RESERVE, fixture::START_ENERGY);
    println!("                     equally through the diagnostic seam's `bud: Some(false)`");
    println!();
    println!("## training layouts (optimisation sees only these)");
    for l in &training {
        println!("{}", fixture::describe(l));
    }
    println!();
    println!("## held-out layouts (kept out of optimisation and selection; not evaluated in R2a)");
    for l in &holdout {
        println!("{}", fixture::describe(l));
    }
}

/// Run the three controls on every training layout and judge each layout against the three
/// requirements.
pub fn controls(
    workers: usize,
    wall_seconds: u64,
    out: Option<PathBuf>,
) -> Result<(), Boxed> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(wall_seconds);
    let layouts = fixture::training_layouts();
    let jobs: Vec<(usize, Control)> =
        (0..layouts.len()).flat_map(|l| CONTROLS.map(|c| (l, c))).collect();

    println!("# R2a fixture controls");
    println!(
        "# {} episodes, {} ticks each ({:.0} s), {workers} workers, {wall_seconds} s budget",
        jobs.len(),
        HORIZON_TICKS,
        fixture::seconds(HORIZON_TICKS)
    );
    println!("# build {BUILD_ID}");

    let cancel = AtomicBool::new(false);
    let limits = Limits::until(&cancel, deadline);
    let slots: Mutex<Vec<Option<Episode>>> = Mutex::new(vec![None; jobs.len()]);
    let failed: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let cursor = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1).min(jobs.len()) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= jobs.len() {
                        return;
                    }
                    if limits.expired() {
                        return;
                    }
                    let (l, c) = jobs[i];
                    let name = format!("control/{}/{}", layouts[l].name, c.name());
                    match episode::run(
                        &layouts[l],
                        &Driver::Control(c),
                        HORIZON_TICKS,
                        limits,
                        &name,
                    ) {
                        Ok(e) => slots.lock().expect("slots")[i] = Some(e),
                        Err(EpisodeError::Cancelled { .. }) => return,
                        Err(e @ EpisodeError::Invalid { .. }) => {
                            cancel.store(true, Ordering::SeqCst);
                            failed.lock().expect("failed").push(e.to_string());
                            return;
                        }
                    }
                }
            });
        }
    });
    let slots = slots.into_inner().expect("slots");
    let failed = failed.into_inner().expect("failed");
    let wall = started.elapsed().as_secs_f64();

    println!();
    println!(
        "{:<13} {:<19} {:>7} {:>8} {:>6} {:>10} {:>10} {:>10} {:>10} {:>8} {:>6} {:>9} {:>9}",
        "layout", "control", "ticks", "seconds", "alive", "stores_end", "intake_P", "upkeep",
        "motion", "px", "cells", "route_P0", "route_P1"
    );
    for (i, (l, c)) in jobs.iter().enumerate() {
        match &slots[i] {
            None => println!("{:<13} {:<19} {:>7}", layouts[*l].name, c.name(), "CANCELLED"),
            Some(e) => println!(
                "{:<13} {:<19} {:>7} {:>8.1} {:>6} {:>10.4} {:>10.4} {:>10.4} {:>10.4} {:>8.1} \
                 {:>6} {:>9.3} {:>9.3}",
                layouts[*l].name,
                c.name(),
                e.ticks,
                e.seconds(),
                e.alive,
                e.terminal_stores,
                e.intake_producer,
                e.upkeep_billed,
                e.motion_billed,
                e.travelled_px,
                e.distinct_cells,
                e.route_p_start,
                e.route_p_end,
            ),
        }
    }

    if !failed.is_empty() {
        println!();
        for f in &failed {
            println!("EXPERIMENT ERROR: {f}");
        }
    }

    println!();
    println!("## requirements, per layout");
    println!("R1 starting stores alone cannot survive the horizon (no-intake dies)");
    println!("R2 stationary grazing cannot pass the relocation task (it dies too)");
    println!("R3 a paid mobile strategy can exploit the food (funded at the horizon)");
    println!();
    println!("{:<13} {:>4} {:>4} {:>4}  verdict", "layout", "R1", "R2", "R3");
    let mut all_pass = true;
    for (l, layout) in layouts.iter().enumerate() {
        let get = |c: Control| {
            jobs.iter()
                .position(|(li, ci)| *li == l && *ci == c)
                .and_then(|i| slots[i].clone())
        };
        let r1 = get(Control::NoIntake).map(|e| !e.alive);
        let r2 = get(Control::StationaryGrazing).map(|e| !e.alive);
        let r3 = get(Control::MobileScript).map(|e| e.alive && e.terminal_stores > 0.0);
        let mark = |b: Option<bool>| match b {
            Some(true) => "pass",
            Some(false) => "FAIL",
            None => "----",
        };
        let pass = r1 == Some(true) && r2 == Some(true) && r3 == Some(true);
        all_pass &= pass;
        println!(
            "{:<13} {:>4} {:>4} {:>4}  {}",
            layout.name,
            mark(r1),
            mark(r2),
            mark(r3),
            if pass { "learning-ready" } else { "NOT learning-ready" }
        );
    }
    println!();
    if !all_pass {
        println!(
            "At least one layout failed its controls. That exact failure is the finding: no \
             layout, seed or config search follows from it, and no layout that failed may be \
             called learning-ready."
        );
    }
    println!("wall time {wall:.1} s of the {wall_seconds} s budget");
    if !failed.is_empty() {
        return Err(failed.join("; ").into());
    }

    if let Some(path) = out {
        let rows: Vec<serde_json::Value> = jobs
            .iter()
            .enumerate()
            .map(|(i, (l, c))| {
                serde_json::json!({
                    "layout": layouts[*l].name,
                    "layout_hash": format!("{:#018x}", layouts[*l].hash(&layouts[*l].config())),
                    "control": c.name(),
                    "episode": slots[i],
                })
            })
            .collect();
        write_json(&path, &serde_json::json!({
            "build": BUILD_ID,
            "horizon_ticks": HORIZON_TICKS,
            "workers": workers,
            "wall_seconds": wall,
            "rows": rows,
        }))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

/// The plumbing smoke: two perturbation pairs on one training layout, 2,000 ticks an episode,
/// four episodes — then the same four again at a different worker count, and a byte comparison
/// of the scores and the update.
///
/// This exercises **ES**, not foraging and not learning. The short horizon need not outlast
/// the starting reserves, no held-out layout is touched, and nothing it produces is a trained
/// founder.
pub fn smoke(out: Option<PathBuf>) -> Result<(), Boxed> {
    const SMOKE_TICKS: u64 = 2_000;
    const SMOKE_PAIRS: usize = 2;
    const SMOKE_BUDGET_SECONDS: u64 = 60;
    let started = Instant::now();
    let layouts = fixture::training_layouts();
    let one = &layouts[..1];
    let protocol = Protocol::new(SMOKE_PAIRS, SMOKE_TICKS, 20_260_915, one);
    let cancel = AtomicBool::new(false);
    // The brief's budget is 60 s including the repeat, and a budget the running rollouts
    // cannot see is not a budget. Both halves share this one deadline.
    let deadline = started + Duration::from_secs(SMOKE_BUDGET_SECONDS);

    println!("# R2a plumbing smoke");
    println!(
        "# {SMOKE_PAIRS} pairs x 2 signs x 1 layout ({}) x {SMOKE_TICKS} ticks = 4 episodes, twice",
        one[0].name
    );
    println!("# build {BUILD_ID}   protocol hash {:#018x}", protocol.hash());

    let run = |workers: usize| -> (Vec<f64>, Adam, GenerationReport) {
        let mut theta = tensor::initial_center(protocol.train_seed);
        let mut adam = Adam::new(PARAMS);
        let plan = Plan {
            layouts: one,
            horizon: SMOKE_TICKS,
            workers,
            evaluate_center: false,
            deadline: Some(deadline),
            fault: None,
        };
        let report = run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel)
            .expect("the smoke fits its budget many times over");
        (theta, adam, report)
    };

    let (theta_a, adam_a, report_a) = run(2);
    let (theta_b, adam_b, report_b) = run(4);

    println!();
    println!("## the four episodes (workers = 2)");
    println!(
        "{:<9} {:<13} {:>7} {:>6} {:>12} {:>12} {:>10} {:>8} {:>6}",
        "candidate", "layout", "ticks", "alive", "stores_end", "score", "intake_P", "px", "cells"
    );
    for job in &report_a.jobs {
        let e = &job.episode;
        println!(
            "{:<9} {:<13} {:>7} {:>6} {:>12.6} {:>12.6} {:>10.4} {:>8.1} {:>6}",
            job.candidate,
            job.layout,
            e.ticks,
            e.alive,
            e.terminal_stores,
            score(std::slice::from_ref(e)),
            e.intake_producer,
            e.travelled_px,
            e.distinct_cells,
        );
    }

    println!();
    println!("## the update");
    println!("candidate scores  {:?}", report_a.candidate_scores);
    println!("gradient L2 norm  {:.12e}", report_a.gradient_norm);
    println!("update RMS        {:.12e}", report_a.update_rms);
    println!("centre L2 change  {:.12e}", l2_delta(&tensor::initial_center(protocol.train_seed), &theta_a));

    println!();
    println!("## the repeat at four workers");
    let scores_equal = report_a.candidate_scores == report_b.candidate_scores;
    let jobs_equal = report_a.jobs == report_b.jobs;
    let theta_equal = super::bits::encode(&theta_a) == super::bits::encode(&theta_b);
    let adam_equal = adam_a == adam_b;
    println!("scores identical          {scores_equal}");
    println!("every episode identical   {jobs_equal}");
    println!("centre bytes identical    {theta_equal}  (little-endian IEEE-754 hex)");
    println!("Adam state identical      {adam_equal}");
    let ok = scores_equal && jobs_equal && theta_equal && adam_equal;
    println!(
        "verdict                   {}",
        if ok { "deterministic" } else { "NOT DETERMINISTIC" }
    );

    let wall = started.elapsed().as_secs_f64();
    println!();
    println!(
        "episodes {} ({} ticks), wall time {wall:.1} s of the {SMOKE_BUDGET_SECONDS} s budget",
        report_a.episodes_run + report_b.episodes_run,
        report_a.ticks_run + report_b.ticks_run
    );

    if let Some(path) = out {
        write_json(&path, &serde_json::json!({
            "build": BUILD_ID,
            "protocol": protocol,
            "protocol_hash": format!("{:#018x}", protocol.hash()),
            "workers_a": 2, "workers_b": 4,
            "report_a": report_a,
            "report_b": report_b,
            "theta_a_hex_fnv1a": format!("{:#018x}", fixture::fnv1a(super::bits::encode(&theta_a).as_bytes())),
            "theta_b_hex_fnv1a": format!("{:#018x}", fixture::fnv1a(super::bits::encode(&theta_b).as_bytes())),
            "deterministic": ok,
            "wall_seconds": wall,
        }))?;
        println!("wrote {}", path.display());
    }
    if !ok {
        return Err("the smoke's repeat did not reproduce".into());
    }
    Ok(())
}

/// Measure single-animal episode throughput, which is what the compute checkpoint needs.
pub fn bench(ticks: u64, workers: usize) -> Result<(), Boxed> {
    let layouts = fixture::training_layouts();
    let cancel = AtomicBool::new(false);
    let policy = tensor::policy(&tensor::initial_center(20_260_915))?;

    println!("# R2a single-animal episode throughput");
    println!("# build {BUILD_ID}, {ticks} ticks per episode");
    println!();
    let limits = Limits::new(&cancel);
    println!("{:<13} {:>9} {:>12} {:>12} {:>9}", "layout", "seconds", "ticks/s", "validations", "alive");
    for l in &layouts {
        let t = Instant::now();
        let e = episode::run(l, &Driver::Policy(Box::new(policy.clone())), ticks, limits, "bench")
            .expect("not cancelled");
        let s = t.elapsed().as_secs_f64();
        println!(
            "{:<13} {:>9.3} {:>12.0} {:>12} {:>9}",
            l.name,
            s,
            e.ticks as f64 / s,
            e.validations,
            e.alive
        );
    }

    // Parallel: one episode per layout, repeated so every worker is busy.
    let jobs: Vec<usize> = (0..workers * 2).map(|i| i % layouts.len()).collect();
    let cursor = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let t = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= jobs.len() {
                        return;
                    }
                    let e = episode::run(
                        &layouts[jobs[i]],
                        &Driver::Policy(Box::new(policy.clone())),
                        ticks,
                        limits,
                        "bench",
                    )
                    .expect("not cancelled");
                    done.fetch_add(e.ticks as usize, Ordering::SeqCst);
                }
            });
        }
    });
    let s = t.elapsed().as_secs_f64();
    let total = done.load(Ordering::SeqCst) as f64;
    println!();
    println!(
        "{} episodes at {workers} workers: {:.3} s, {:.0} ticks/s aggregate, {:.3} s per episode",
        jobs.len(),
        s,
        total / s,
        s / jobs.len() as f64 * workers as f64,
    );
    println!();
    println!("A full-horizon ({HORIZON_TICKS}-tick) episode therefore costs about {:.2} s of one", HORIZON_TICKS as f64 / (total / s) * workers as f64);
    println!("worker's time, and {:.2} s of wall time at {workers} workers.", HORIZON_TICKS as f64 / (total / s));
    Ok(())
}

/// The learning run. **R2a does not execute this**; the command exists so the proposed
/// first-learning command is exact rather than described.
///
/// Resuming is a *continuation*, not a second run into the same folder: the generation log is
/// appended to, an already-recorded centre evaluation is reused rather than paid for twice,
/// and every centre the run passes through keeps its exact weights in `centers/`, so the
/// evaluation assignment can select any of them and not just the last.
#[allow(clippy::too_many_arguments)]
pub fn train(
    pairs: usize,
    generations: u64,
    horizon: u64,
    workers: usize,
    wall_seconds: u64,
    train_seed: u64,
    center_eval: bool,
    resume: Option<PathBuf>,
    overwrite: bool,
    out: PathBuf,
) -> Result<(), Boxed> {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(wall_seconds);
    let layouts = fixture::training_layouts();
    let protocol = Protocol::new(pairs, horizon, train_seed, &layouts);
    let log_path = out.join("generations.jsonl");
    let checkpoint_path = out.join("checkpoint.json");

    let resuming = resume.is_some();
    let mut checkpoint = match resume {
        Some(path) => {
            let cp: Checkpoint = serde_json::from_str(&fs::read_to_string(&path)?)?;
            cp.validate()?;
            if cp.protocol_hash != protocol.hash() {
                return Err(format!(
                    "checkpoint {} was written under protocol {:#018x}, this command is \
                     {:#018x}: resuming across a protocol change would compare two tasks",
                    path.display(),
                    cp.protocol_hash,
                    protocol.hash()
                )
                .into());
            }
            println!("# resumed from {} at generation {}", path.display(), cp.generation_completed);
            cp
        }
        None => {
            // A fresh run into a directory that already holds one would truncate its log and
            // overwrite its checkpoint. Refuse unless the caller says so out loud.
            if !overwrite && (log_path.exists() || checkpoint_path.exists()) {
                return Err(format!(
                    "{} already holds a run (checkpoint or generation log). Pass --resume \
                     {} to continue it, choose another --out, or pass --overwrite to discard it.",
                    out.display(),
                    checkpoint_path.display()
                )
                .into());
            }
            if overwrite {
                let _ = fs::remove_file(&log_path);
                let _ = fs::remove_dir_all(out.join("centers"));
            }
            Checkpoint::fresh(protocol.clone(), train_seed, BUILD_ID)
        }
    };

    fs::create_dir_all(&out)?;
    fs::create_dir_all(out.join("centers"))?;
    let cancel = AtomicBool::new(false);
    // Always append: the previous generations are the run's history, and `File::create` would
    // truncate them. A fresh run either has no log (it was refused otherwise) or had its old
    // one removed above by an explicit `--overwrite`.
    let _ = resuming;
    let mut log =
        std::io::BufWriter::new(fs::OpenOptions::new().create(true).append(true).open(&log_path)?);

    println!("# R2a learning run");
    println!("# build {BUILD_ID}   protocol hash {:#018x}", protocol.hash());
    println!(
        "# {pairs} pairs ({} candidates) x {} layouts x {horizon} ticks, up to {generations} \
         updates, {workers} workers, {wall_seconds} s cap",
        2 * pairs,
        layouts.len()
    );
    println!(
        "{:>4} {:>8} {:>12} {:>12} {:>12} {:>12} {:>9}",
        "gen", "episodes", "best", "median", "centre", "|g|", "seconds"
    );

    let first = checkpoint.generation_completed;
    let mut stop = "generations".to_string();
    for generation in first..(first + generations) {
        if Instant::now() >= deadline {
            stop = "wall-time cap".into();
            break;
        }
        // The centre about to be perturbed is this generation's centre. Record its exact
        // weights before anything touches them.
        record_center(&mut checkpoint, &out, generation)?;
        let reused = checkpoint.recorded_center_score(generation);
        let plan = Plan {
            layouts: &layouts,
            horizon,
            workers,
            evaluate_center: center_eval && reused.is_none(),
            deadline: Some(deadline),
            fault: None,
        };
        let report = match run_generation(
            &mut checkpoint.theta,
            &mut checkpoint.adam,
            &protocol,
            generation,
            &plan,
            &cancel,
        ) {
            Ok(r) => r,
            Err(e) => {
                // An incomplete generation never updates the centre and never writes a new
                // checkpoint; the last saved state is the last *completed* generation. Its
                // work is still counted, separately from optimizer progress.
                checkpoint.discarded.add(e.discarded());
                stop = match &e {
                    GenerationError::Cancelled(_) => {
                        "cancelled mid-generation (centre not updated)".into()
                    }
                    GenerationError::Invalid { .. } => format!("EXPERIMENT ERROR: {e}"),
                };
                write_json_atomic(&checkpoint_path, &checkpoint)?;
                println!();
                println!("stopped: {stop}");
                println!(
                    "discarded work: {} of {} episodes, {} ticks",
                    checkpoint.discarded.episodes_completed,
                    checkpoint.discarded.episodes_attempted,
                    checkpoint.discarded.ticks_run
                );
                println!("checkpoint {}", checkpoint_path.display());
                return match e {
                    GenerationError::Invalid { .. } => Err(Box::new(e) as Boxed),
                    GenerationError::Cancelled(_) => Ok(()),
                };
            }
        };
        let mut sorted = report.candidate_scores.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        let center_shown = report.center_score.or(reused);
        println!(
            "{:>4} {:>8} {:>12.3} {:>12.3} {:>12} {:>12.4e} {:>9.1}",
            generation,
            report.episodes_run,
            sorted.last().copied().unwrap_or(0.0),
            sorted[sorted.len() / 2],
            center_shown.map_or_else(
                || "-".to_string(),
                |s| if report.center_score.is_some() {
                    format!("{s:.3}")
                } else {
                    format!("{s:.3}*")
                }
            ),
            report.gradient_norm,
            report.wall_seconds,
        );
        checkpoint.generation_completed = generation + 1;
        checkpoint.episodes_run += report.episodes_run;
        checkpoint.ticks_run += report.ticks_run;
        if let Some(s) = report.center_score {
            score_center(&mut checkpoint, generation, s);
        }
        writeln!(log, "{}", serde_json::to_string(&report)?)?;
        log.flush()?;
        write_json_atomic(&checkpoint_path, &checkpoint)?;
    }

    // The final updated centre. The loop evaluates the centre it is *about* to perturb, so
    // without this the last update's centre would never be scored — and a sampled
    // perturbation's score is not the centre's. Recording it here is also what lets the next
    // resume reuse it instead of paying for it again.
    if checkpoint.generation_completed > first {
        let final_generation = checkpoint.generation_completed;
        record_center(&mut checkpoint, &out, final_generation)?;
    }
    if center_eval
        && checkpoint.generation_completed > first
        && checkpoint.recorded_center_score(checkpoint.generation_completed).is_none()
        && Instant::now() < deadline
    {
        let plan = Plan::new(&layouts, horizon, workers, false, Some(deadline));
        match finalize_center(&mut checkpoint, &plan, &checkpoint_path, &cancel)? {
            Some(s) => println!(
                "{:>4} {:>8} {:>12} {:>12} {:>12.3} {:>12} {:>9}",
                "fin", layouts.len(), "-", "-", s, "-", "-"
            ),
            None => stop = "wall-time cap before the final centre evaluation".into(),
        }
    }

    println!();
    println!(
        "stopped: {stop}. {} generations completed, {} episodes, {} ticks, {:.1} s wall.",
        checkpoint.generation_completed - first,
        checkpoint.episodes_run,
        checkpoint.ticks_run,
        started.elapsed().as_secs_f64()
    );
    if checkpoint.discarded != Default::default() {
        println!(
            "discarded work (not optimizer progress): {} of {} episodes, {} ticks",
            checkpoint.discarded.episodes_completed,
            checkpoint.discarded.episodes_attempted,
            checkpoint.discarded.ticks_run
        );
    }
    println!("centres recorded: {:?}", checkpoint.center_generations());
    println!("checkpoint {}", checkpoint_path.display());
    Ok(())
}

/// Evaluate the run's final centre and fold the result into the checkpoint.
///
/// The two failure modes are **not** the same thing, and the generation loop already
/// distinguishes them. This does too:
///
/// - `Cancelled` is an ordinary budget stop. The centre simply has no score, the work it did is
///   counted as discarded, the checkpoint is saved and the run ends normally (`Ok(None)`).
/// - `Invalid` is an **experiment error**. The job identity and the detail are preserved, the
///   discarded work is counted, the completed optimizer state is saved unchanged — `theta`,
///   `adam` and `generation_completed` are still the last completed generation's, because
///   nothing here updates them — and the error is returned so `es-train` exits non-zero.
///
/// Before repair cycle 2 both arms wrote "wall-time cap before the final centre evaluation"
/// and returned `Ok(())`, so an invariant failure in the last four episodes of a run was
/// reported as a successful timeout.
pub fn finalize_center(
    checkpoint: &mut Checkpoint,
    plan: &Plan<'_>,
    checkpoint_path: &Path,
    cancel: &AtomicBool,
) -> Result<Option<f64>, Boxed> {
    let generation = checkpoint.generation_completed;
    match trainer::evaluate(&checkpoint.theta, plan, generation, cancel) {
        Ok((score, episodes)) => {
            score_center(checkpoint, generation, score);
            checkpoint.episodes_run += episodes.len() as u64;
            checkpoint.ticks_run += episodes.iter().map(|e| e.ticks).sum::<u64>();
            write_json_atomic(checkpoint_path, checkpoint)?;
            Ok(Some(score))
        }
        Err(e) => {
            checkpoint.discarded.add(e.discarded());
            write_json_atomic(checkpoint_path, checkpoint)?;
            match e {
                GenerationError::Cancelled(_) => Ok(None),
                GenerationError::Invalid { .. } => {
                    println!();
                    println!("stopped: EXPERIMENT ERROR: {e}");
                    println!(
                        "discarded work: {} of {} episodes, {} ticks",
                        checkpoint.discarded.episodes_completed,
                        checkpoint.discarded.episodes_attempted,
                        checkpoint.discarded.ticks_run
                    );
                    println!("checkpoint {}", checkpoint_path.display());
                    Err(Box::new(e) as Boxed)
                }
            }
        }
    }
}

/// Write this generation's centre weights, once. A centre already recorded is left alone —
/// including its score — which is what makes a resume a continuation rather than a repeat.
fn record_center(cp: &mut Checkpoint, out: &Path, generation: u64) -> Result<(), Boxed> {
    if cp.centers.iter().any(|c| c.generation == generation) {
        return Ok(());
    }
    let relative = format!("centers/center-{generation:05}.json");
    let file = PolicyFile::new(&cp.theta, &cp.build, cp.protocol_hash, generation)?;
    write_json_atomic(&out.join(&relative), &file)?;
    cp.centers.push(CenterRecord {
        generation,
        score: None,
        file: relative,
        weights_fnv1a: fixture::fnv1a(super::bits::encode(&cp.theta).as_bytes()),
    });
    Ok(())
}

fn score_center(cp: &mut Checkpoint, generation: u64, score: f64) {
    if let Some(record) = cp.centers.iter_mut().find(|c| c.generation == generation) {
        record.score = Some(score);
    }
}

/// Export a checkpoint's centre as a policy, and verify the round trip and ordinary core
/// inference on a training layout.
pub fn export(checkpoint: PathBuf, out: PathBuf, verify_ticks: u64) -> Result<(), Boxed> {
    let cp: Checkpoint = serde_json::from_str(&fs::read_to_string(&checkpoint)?)?;
    cp.validate()?;
    let file = PolicyFile::new(
        &cp.theta,
        &cp.build,
        cp.protocol_hash,
        cp.generation_completed,
    )?;
    write_json(&out, &file)?;

    let back: PolicyFile = serde_json::from_str(&fs::read_to_string(&out)?)?;
    let loaded = back.policy()?;
    let original = cp.policy()?;
    let exact = tensor::flatten(&loaded.weights)
        .iter()
        .zip(tensor::flatten(&original.weights))
        .all(|(a, b)| a.to_bits() == b.to_bits());

    println!("# R2a policy export");
    println!("checkpoint          {}", checkpoint.display());
    println!("generation          {}", cp.generation_completed);
    println!("protocol hash       {:#018x}", cp.protocol_hash);
    println!("policy digest       {:#018x}", loaded.schema_digest);
    println!("parameters          {}", tensor::flatten(&loaded.weights).len());
    println!("wrote               {}", out.display());
    println!("weight round trip   {}", if exact { "exact, bit for bit" } else { "NOT EXACT" });
    if !exact {
        return Err("the exported policy does not round-trip exactly".into());
    }

    let layout = &fixture::training_layouts()[0];
    let (mut world, id) = layout.build()?;
    world.attach_neural_policy(id, loaded)?;
    for _ in 0..verify_ticks {
        world.step();
        world.drain_events();
    }
    world.check_invariants()?;
    println!(
        "core inference      {verify_ticks} ticks on {}: population {}, still neural {}, \
         world consistent",
        layout.name,
        world.population(),
        world.neural().get(id).is_some()
    );
    Ok(())
}

fn l2_delta(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt()
}

/// Write, then rename. A crash or a full disk during the write leaves the previous completed
/// checkpoint intact instead of a half-written file where the last good one used to be.
fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), Boxed> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("json.writing");
    fs::write(&temp, serde_json::to_string_pretty(value)?)?;
    fs::rename(&temp, path)?;
    Ok(())
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), Boxed> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}
