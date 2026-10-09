//! **The live-training loop**: many persistent worlds, every lineage with a centre at
//! once, one antithetic ES update per lineage per window.
//!
//! - One world per worker, one simulation thread each. Worlds are founded once and are
//!   **not reset** between generations.
//! - A generation is a window of simulated time ([`LiveSpec::window_ticks`]). Each
//!   lineage's `2·pairs` variants are placed on its living bodies at the window's start,
//!   twins on neighbours ([`super::window::place_twins`]), and on its newborns as they
//!   appear ([`super::window::newborn_candidate`]). Pair numbering runs on from one
//!   world to the next, so every pair is placed somewhere before any is placed twice.
//! - **Normalised within the world before pooling**: a body's score is z-scored against
//!   its own world's bodies of its lineage, a candidate's pooled score is the mean z of
//!   its bodies over all worlds (zero with none), and the lineage's update is the frozen
//!   trainer's own centred-rank antithetic gradient and Adam step over those scores.
//! - **Population bounds, training only**: a world that has lost a trained lineage, or
//!   holds more animals than [`LiveSpec::population_cap`], is replaced by a freshly
//!   founded one on a new seed. Every replacement is counted and logged.
//!
//! Nothing is hard-wired to two lineages: centres are given per [`Founder`], a lineage
//! without one is not trained (its bodies keep what the seeder gave them), and every
//! table is indexed over [`Founder::ALL`].

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use cubarium_voxel_fauna::Founder;
use cubarium_voxel_sim::Sim;
use serde::{Deserialize, Serialize};

use super::ecology::EcologyParams;
use super::readout;
use super::variants::VariantTable;
use super::window::{self, BodyScore, LiveWorld, WindowResult};
use super::world;
use crate::es::optimizer::{self, Adam};
use crate::es::rng::perturbation;
use crate::es::voxel::store::VoxelPolicyFile;

/// The protocol label written into the centres this loop saves.
pub const LIVE_PROTOCOL: &str = "live-world-1";

/// How far apart two worlds' founding seed bases start (the landscapes' own stride).
pub const WORLD_SEED_STRIDE: u64 = 1_000;

/// One run.
#[derive(Clone, Debug)]
pub struct LiveSpec {
    /// The starting centre per trained lineage.
    pub centres: Vec<(Founder, PathBuf)>,
    pub worlds: usize,
    /// Worker threads (one world at a time each).
    pub threads: usize,
    pub pairs: usize,
    /// One generation's window, ticks.
    pub window_ticks: u64,
    pub generations: u32,
    /// Stop after the generation that crosses this.
    pub wall_seconds: f64,
    /// Calibration deadline for the first boundary, wall seconds including setup.
    pub first_generation_seconds: f64,
    pub sigma: f64,
    pub learning_rate: f64,
    pub train_seed: u64,
    /// The first world's founding seed base; world `i` starts at `+ i·1000`.
    pub world_seed: u64,
    /// Training-only coarse water ([`cubarium_voxel::World::set_water_every`]).
    pub water_every: u32,
    /// A world holding more animals than this is replaced.
    pub population_cap: usize,
    /// Write the centres every this many generations (and at the end).
    pub save_every: u32,
    pub out: PathBuf,
    /// Restore centres, Adam, persistent worlds and curiosity from this run.
    pub resume: Option<PathBuf>,
    /// The ecology every world is founded under (training only; defaults: the shipped
    /// world).
    pub ecology: EcologyParams,
    /// The world every training world is founded from: a host file's `[world]`
    /// (`--world`, e.g. the desktop terrarium), or the still `small` preset when `None`.
    pub world: Option<cubarium_voxel::Config>,
    /// Each lineage's curiosity (`--curiosity`; weight 0 everywhere without).
    pub curiosity: super::curiosity::Curiosity,
    /// Experimental training reward, false keeps tissue/reserve/escrow accounting.
    pub stomach_aware_reward: bool,
}

/// One lineage's side of a generation.
#[derive(Clone, Debug, Serialize)]
pub struct LineageUpdate {
    pub lineage: String,
    pub bodies: usize,
    pub newborn_bodies: usize,
    /// Candidates (of `2·pairs`) that had at least one body.
    pub candidates_placed: usize,
    pub mean_raw_score: f64,
    pub gradient_norm: f64,
    pub update_rms: f64,
    /// Pooled (within-world z) score per candidate, in candidate order.
    pub candidate_scores: Vec<f64>,
}

/// One world replaced after a generation.
#[derive(Clone, Debug, Serialize)]
pub struct Replacement {
    pub world: usize,
    pub reason: String,
    /// `copy of world N` or `fresh seed S`.
    pub with: String,
}

/// One generation's record (one JSON line in `generations.jsonl`).
#[derive(Clone, Debug, Serialize)]
pub struct GenerationRecord {
    pub generation: u32,
    pub wall_s: f64,
    pub elapsed_s: f64,
    pub ticks: u64,
    pub body_windows: usize,
    pub body_scores: Vec<BodyScore>,
    pub lineages: Vec<LineageUpdate>,
    pub replaced: Vec<Replacement>,
    pub intake_anomalies: u64,
    pub readout: Vec<(String, f64)>,
}

/// A lineage being trained.
struct Lineage {
    founder: Founder,
    theta: Vec<f64>,
    adam: Adam,
    template: VoxelPolicyFile,
}

/// Z-score each body within its own world and lineage, then pool: a candidate's score is
/// the mean z of its bodies across worlds, zero for a candidate no body ran.
///
/// Returns `(scores by candidate, bodies by candidate)`.
pub fn pooled_scores(per_world: &[Vec<(usize, f64)>], candidates: usize) -> (Vec<f64>, Vec<usize>) {
    let mut sum = vec![0.0; candidates];
    let mut n = vec![0usize; candidates];
    for bodies in per_world {
        if bodies.is_empty() {
            continue;
        }
        let m = bodies.iter().map(|b| b.1).sum::<f64>() / bodies.len() as f64;
        let var = bodies.iter().map(|b| (b.1 - m).powi(2)).sum::<f64>() / bodies.len() as f64;
        let sd = var.sqrt();
        for &(c, s) in bodies {
            let z = if sd > 1e-12 { (s - m) / sd } else { 0.0 };
            sum[c] += z;
            n[c] += 1;
        }
    }
    let scores = sum
        .iter()
        .zip(&n)
        .map(|(s, &k)| if k > 0 { s / k as f64 } else { 0.0 })
        .collect();
    (scores, n)
}

/// The antithetic update from pooled candidate scores (`2p` = plus, `2p+1` = minus): the
/// frozen trainer's centred-rank gradient and Adam ascent. Returns `(‖g‖, update rms)`.
pub fn ascend(
    theta: &mut [f64],
    adam: &mut Adam,
    scores: &[f64],
    sigma: f64,
    train_seed: u64,
    generation: u32,
) -> (f64, f64) {
    let pairs = scores.len() / 2;
    let plus: Vec<f64> = (0..pairs).map(|p| scores[2 * p]).collect();
    let minus: Vec<f64> = (0..pairs).map(|p| scores[2 * p + 1]).collect();
    let g = optimizer::gradient(&plus, &minus, theta.len(), sigma, |i, out| {
        perturbation(train_seed, u64::from(generation), i as u64, out);
    });
    let norm = g.iter().map(|x| x * x).sum::<f64>().sqrt();
    let rms = adam.ascend(theta, &g);
    (norm, rms)
}

fn found(spec: &LiveSpec, slot: usize, base: u64, lineages: &[Lineage]) -> LiveWorld {
    let cfg = spec.world.clone().unwrap_or_else(world::training_config);
    let ecology = spec.ecology.clone();
    let (mut sim, info) = world::found_world(
        &cfg,
        base,
        ecology.fauna().founder_counts_without_egg_bank(),
        &ecology,
    );
    prepare(&mut sim, spec, lineages);
    LiveWorld {
        id: slot,
        sim,
        ecology,
        info,
        windows: 0,
        memory: Default::default(),
    }
}

/// The per-world setup a founded or copied world gets: coarse water, and every trained
/// lineage's current centre as its birth factory (newborns are reassigned a variant the
/// tick they appear, so this only fills the gap a newborn's first tick could leave).
fn prepare(sim: &mut Sim, spec: &LiveSpec, lineages: &[Lineage]) {
    sim.world_mut().set_water_every(spec.water_every);
    let drivers: Vec<_> = lineages
        .iter()
        .map(|l| {
            (
                l.founder,
                crate::es::voxel::EpisodeDriver::gru(&l.theta, l.founder)
                    .expect("a validated centre"),
            )
        })
        .collect();
    super::install_centres(sim.fauna_mut(), &drivers);
}

/// Run `f` over every item on `threads` workers, results in item order.
pub(crate) fn parallel<T: Send, R: Send>(
    items: Vec<T>,
    threads: usize,
    f: impl Fn(T) -> R + Sync,
) -> Vec<R> {
    let n = items.len();
    let queue: Mutex<Vec<(usize, T)>> = Mutex::new(items.into_iter().enumerate().rev().collect());
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1).min(n.max(1)) {
            scope.spawn(|| {
                loop {
                    let Some((i, item)) = queue.lock().expect("queue").pop() else {
                        return;
                    };
                    let r = f(item);
                    out.lock().expect("out")[i] = Some(r);
                }
            });
        }
    });
    out.into_inner()
        .expect("out")
        .into_iter()
        .map(|r| r.expect("every item ran"))
        .collect()
}

fn save_centres(spec: &LiveSpec, lineages: &[Lineage], generation: u32, tag: &str) -> Result<(), String> {
    let dir = spec.out.join("centres");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for l in lineages {
        let file = VoxelPolicyFile {
            build: crate::evaluate::BUILD_ID.into(),
            train_seed: spec.train_seed,
            generation: Some(u64::from(generation)),
            score: None,
            arena_protocol: LIVE_PROTOCOL.into(),
            protocol_hash: None,
            imitation: None,
            theta: l.theta.clone(),
            ..l.template.clone()
        };
        let path = dir.join(format!("{tag}-{}.json", l.founder.name()));
        file.write(&path)?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct TrainingCheckpoint {
    next_generation: u32,
    next_fresh_seed: u64,
    train_seed: u64,
    pairs: usize,
    window_ticks: u64,
    sigma: f64,
    curiosity: super::curiosity::Curiosity,
    stomach_aware_reward: bool,
    lineages: Vec<(Founder, Vec<f64>, Adam, VoxelPolicyFile)>,
    worlds: Vec<WorldState>,
}

#[derive(Serialize, Deserialize)]
struct WorldState {
    id: usize,
    windows: u32,
    info: world::FoundInfo,
    familiarity: super::curiosity::MemoryState,
}

fn save_checkpoint(spec: &LiveSpec, lineages: &[Lineage], worlds: &[LiveWorld], next_generation: u32, next_fresh_seed: u64) -> Result<(), String> {
    let pointer = spec.out.join("checkpoint.json");
    let old = std::fs::read_to_string(&pointer).ok();
    let slot = if old.as_deref().is_some_and(|s| s.trim() == "checkpoint-a") { "checkpoint-b" } else { "checkpoint-a" };
    let dir = spec.out.join(slot);
    if dir.exists() { std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?; }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for world in worlds {
        super::staging::save_world(&dir.join(format!("world-{}", world.id)), &world.sim)?;
    }
    let state = TrainingCheckpoint {
        next_generation, next_fresh_seed, train_seed: spec.train_seed, pairs: spec.pairs,
        window_ticks: spec.window_ticks, sigma: spec.sigma, curiosity: spec.curiosity.clone(),
        stomach_aware_reward: spec.stomach_aware_reward,
        lineages: lineages.iter().map(|l| (l.founder, l.theta.clone(), l.adam.clone(), l.template.clone())).collect(),
        worlds: worlds.iter().map(|w| WorldState { id: w.id, windows: w.windows, info: w.info.clone(), familiarity: w.memory.state() }).collect(),
    };
    super::staging::atomic_text(&dir.join("state.json"), &serde_json::to_string(&state).map_err(|e| e.to_string())?)?;
    // Commit only after every world and the matching optimizer/familiarity state exists.
    super::staging::atomic_text(&pointer, slot)?;
    if let Some(old) = old.filter(|s| s.trim() != slot) {
        let old = old.trim();
        if matches!(old, "checkpoint-a" | "checkpoint-b") {
            std::fs::remove_dir_all(spec.out.join(old)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn load_checkpoint(spec: &LiveSpec, dir: &Path, lineages: &mut Vec<Lineage>) -> Result<(Vec<LiveWorld>, u64, u32), String> {
    let slot = std::fs::read_to_string(dir.join("checkpoint.json")).map_err(|e| format!("training checkpoint: {e}"))?;
    if !matches!(slot.trim(), "checkpoint-a" | "checkpoint-b") { return Err("invalid training checkpoint pointer".into()); }
    let dir = dir.join(slot.trim());
    let state: TrainingCheckpoint = serde_json::from_str(&std::fs::read_to_string(dir.join("state.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if state.train_seed != spec.train_seed || state.pairs != spec.pairs || state.window_ticks != spec.window_ticks
        || state.sigma != spec.sigma || state.stomach_aware_reward != spec.stomach_aware_reward
        || serde_json::to_string(&state.curiosity).map_err(|e| e.to_string())? != serde_json::to_string(&spec.curiosity).map_err(|e| e.to_string())?
        || state.worlds.len() != spec.worlds {
        return Err("checkpoint training settings differ (seed, pairs, window, sigma, reward, curiosity or worlds)".into());
    }
    let policy: Vec<_> = lineages.iter().map(|l| l.founder).collect();
    if state.lineages.iter().map(|l| l.0).collect::<Vec<_>>() != policy { return Err("checkpoint trained lineages differ".into()); }
    *lineages = state.lineages.into_iter().map(|(founder, theta, adam, template)| Lineage { founder, theta, adam, template }).collect();
    let mut worlds = Vec::new();
    for state in state.worlds {
        let saved = cubarium_voxel_sim::saved::load(&dir.join(format!("world-{}", state.id)), &policy)?;
        let mut sim = Sim::ambient(saved.world, saved.flora, saved.fauna, cubarium_voxel_sim::SimConfig { threads: 1, overlap: false, water_every: 1 });
        prepare(&mut sim, spec, lineages);
        worlds.push(LiveWorld { id: state.id, sim, ecology: spec.ecology.clone(), info: state.info,
            windows: state.windows, memory: super::curiosity::Memory::from_state(state.familiarity) });
    }
    Ok((worlds, state.next_fresh_seed, state.next_generation))
}

fn check_world(run: &super::staging::Run, world: &LiveWorld, window: Option<&cubarium_voxel_sim::staged::Window>) -> Result<String, String> {
    let result = super::staging::instant(&world.sim).and_then(|()| match window {
        Some(w) => w.verdict(&world.sim, &super::staging::check_config(&run.dir)),
        None => Ok("drivers and ledgers OK".into()),
    });
    if let Err(why) = result {
        let dir = run.dir.join("worlds").join(format!("world-{}", world.id));
        let save = super::staging::save_world(&dir, &world.sim);
        let why = format!("world {} seed {}: {why}{}", dir.display(), world.info.seed,
            save.err().map_or(String::new(), |e| format!("; checkpoint failed: {e}")));
        run.fail(why.clone());
        Err(why)
    } else { result }
}

#[allow(clippy::too_many_arguments)]
fn staged_window(world: &mut LiveWorld, tables: &[Option<VariantTable>], first: &[usize], spec: &LiveSpec,
    run: &super::staging::Run, generation: u32, full: bool) -> Result<WindowResult, String> {
    run.check()?;
    check_world(run, world, None)?;
    let started = Instant::now();
    let mut check = full.then(|| cubarium_voxel_sim::staged::Window::open(&world.sim));
    let memory = std::mem::take(&mut world.memory);
    let mut w = window::Window::begin_with(&mut world.sim, tables, first, spec.window_ticks, &spec.curiosity, memory);
    w.stomach_aware_reward = spec.stomach_aware_reward;
    while !w.done() {
        if let Err(why) = run.check() {
            let mut r = w.finish(&world.sim, world.id);
            world.memory = std::mem::take(&mut r.memory);
            return Err(why);
        }
        w.step(&mut world.sim);
        if let Some(check) = &mut check { check.watch(&world.sim); }
    }
    let mut result = w.finish(&world.sim, world.id);
    world.memory = std::mem::take(&mut result.memory);
    world.windows += 1;
    let note = check_world(run, world, check.as_ref())?;
    let dir = run.dir.join("worlds").join(format!("world-{}", world.id));
    super::staging::save_world(&dir, &world.sim).and_then(|()| run.status(generation, u64::from(world.windows),
        spec.window_ticks as f64 / started.elapsed().as_secs_f64().max(1e-9), &note, "RUNNING"))
        .inspect_err(|why| run.fail(format!("world {} seed {}: checkpoint/status failed: {why}", dir.display(), world.info.seed)))?;
    Ok(result)
}

/// Run the loop. Returns the records it wrote.
pub fn train(spec: &LiveSpec) -> Result<Vec<GenerationRecord>, String> {
    super::staging::execute(&spec.out, spec.resume.is_some(), spec.wall_seconds, spec.first_generation_seconds, |run| train_inner(spec, run))
}

fn train_inner(spec: &LiveSpec, run: &std::sync::Arc<super::staging::Run>) -> Result<Vec<GenerationRecord>, String> {
    let run_started = Instant::now();
    if spec.worlds == 0 || spec.pairs == 0 || spec.window_ticks == 0 { return Err("worlds, pairs and window length must be positive".into()); }
    cubarium_voxel::world::set_thread_override(1);
    println!("# pools: workers={} world-threads=1 tail-cores=1", spec.threads.max(1).min(spec.worlds));
    std::fs::create_dir_all(&spec.out)
        .map_err(|e| format!("cannot create {}: {e}", spec.out.display()))?;
    let mut lineages: Vec<Lineage> = Vec::new();
    for (founder, path) in &spec.centres {
        let file = VoxelPolicyFile::load_or_transfer(path)?;
        let declared = file.founder()?;
        if declared != *founder {
            return Err(format!(
                "{} declares the {} lineage, not {}",
                path.display(),
                declared.name(),
                founder.name()
            ));
        }
        // Validates length, finiteness and this build's digest, and the world's mass.
        super::search::check_mass(&[(*founder, file.driver()?)], &spec.ecology)?;
        let mut adam = Adam::new(file.theta.len());
        adam.learning_rate = spec.learning_rate;
        lineages.push(Lineage {
            founder: *founder,
            theta: file.theta.clone(),
            adam,
            template: file,
        });
    }
    if lineages.is_empty() {
        return Err("live training needs at least one centre".into());
    }
    lineages.sort_by_key(|l| l.founder.index());

    let (mut worlds, mut next_fresh_seed, first_generation) = if let Some(dir) = &spec.resume {
        load_checkpoint(spec, dir, &mut lineages)?
    } else {
        let bases: Vec<_> = (0..spec.worlds).map(|i| (i, spec.world_seed + i as u64 * WORLD_SEED_STRIDE)).collect();
        (parallel(bases, spec.threads, |(slot, base)| found(spec, slot, base, &lineages)),
            spec.world_seed + spec.worlds as u64 * WORLD_SEED_STRIDE, 0)
    };
    for world in &worlds { check_world(run, world, None)?; }
    println!("# {} {} worlds at generation {first_generation}: seeds {:?}",
        if spec.resume.is_some() { "restored" } else { "founded" }, worlds.len(), worlds.iter().map(|w| w.info.seed).collect::<Vec<_>>());
    if spec.resume.is_some() {
        super::staging::repair_log(&spec.out.join("generations.jsonl"), |r| r["generation"].as_u64().is_some_and(|g| g < u64::from(first_generation)))?;
    }
    let mut log = std::fs::OpenOptions::new().create(true).write(true)
        .append(spec.resume.is_some()).truncate(spec.resume.is_none()).open(spec.out.join("generations.jsonl"))
        .map_err(|e| format!("cannot write the generation log: {e}"))?;
    save_centres(spec, &lineages, first_generation, "latest")?;
    save_checkpoint(spec, &lineages, &worlds, first_generation, next_fresh_seed)?;
    let mut last_gen_wall = 0.0;
    let mut next_generation = first_generation;

    let mut records = Vec::new();
    let candidates = 2 * spec.pairs;
    for generation in first_generation..spec.generations {
        if run.boundary(generation, last_gen_wall) { break; }
        run.check()?;
        run.status(generation, 0, 0.0, "drivers and ledgers OK", "RUNNING")?;
        let started = Instant::now();
        // 1. This generation's tables, and where each world's placement starts.
        let mut tables: Vec<Option<VariantTable>> = vec![None; Founder::COUNT];
        for l in &lineages {
            tables[l.founder.index()] = Some(VariantTable::antithetic(
                l.founder,
                &l.theta,
                spec.train_seed,
                spec.sigma,
                generation,
                spec.pairs,
            )?);
        }
        let mut first: Vec<Vec<usize>> = Vec::with_capacity(worlds.len());
        let mut cursor = vec![0usize; Founder::COUNT];
        for w in &worlds {
            first.push(cursor.clone());
            let pop = window::population(&w.sim);
            for f in Founder::ALL {
                cursor[f.index()] = (cursor[f.index()] + pop[f.index()].div_ceil(2)) % spec.pairs;
            }
        }

        // 2. Every world runs its window.
        let jobs: Vec<(&mut LiveWorld, Vec<usize>)> = worlds.iter_mut().zip(first).collect();
        let tables_ref = &tables;
        let results: Vec<Result<WindowResult, String>> = parallel(jobs, spec.threads, |(w, first)| {
            staged_window(w, tables_ref, &first, spec, run, generation, generation == first_generation || w.windows == 0)
        });
        let results: Vec<WindowResult> = results.into_iter().collect::<Result<_, _>>()?;
        run.check()?;

        // 3. One update per lineage, from the scores normalised within each world.
        let mut updates = Vec::new();
        for l in lineages.iter_mut() {
            let per_world: Vec<Vec<(usize, f64)>> = results
                .iter()
                .map(|r| {
                    r.scores
                        .iter()
                        .filter(|b| b.founder == l.founder)
                        .map(|b| (b.candidate, b.score.score))
                        .collect()
                })
                .collect();
            let (scores, n) = pooled_scores(&per_world, candidates);
            let mine: Vec<&BodyScore> = results
                .iter()
                .flat_map(|r| r.scores.iter())
                .filter(|b| b.founder == l.founder)
                .collect();
            // σ = 0 is the placement-noise control: every variant is the centre, so
            // there is no direction to estimate and the centre is left alone.
            let (gradient_norm, update_rms) = if mine.len() >= 2 && spec.sigma > 0.0 {
                ascend(
                    &mut l.theta,
                    &mut l.adam,
                    &scores,
                    spec.sigma,
                    spec.train_seed,
                    generation,
                )
            } else {
                (0.0, 0.0)
            };
            updates.push(LineageUpdate {
                lineage: l.founder.name().into(),
                bodies: mine.len(),
                newborn_bodies: mine.iter().filter(|b| b.newborn).count(),
                candidates_placed: n.iter().filter(|&&k| k > 0).count(),
                mean_raw_score: mine.iter().map(|b| b.score.score).sum::<f64>()
                    / mine.len().max(1) as f64,
                gradient_norm,
                update_rms,
                candidate_scores: scores.clone(),
            });
        }

        // 4. The readout, pooled.
        let per_world: Vec<_> = results.iter().map(|r| r.readout.clone()).collect();
        let pooled = readout::pool(&per_world);

        // 5. Population bounds.
        let mut replaced = Vec::new();
        let bad: Vec<(usize, String)> = worlds
            .iter()
            .enumerate()
            .filter_map(|(i, w)| {
                let pop = window::population(&w.sim);
                let total = w.sim.fauna().view().animals.len();
                if lineages.iter().any(|l| pop[l.founder.index()] == 0) {
                    Some((i, "lost a lineage".to_string()))
                } else if total > spec.population_cap {
                    Some((i, format!("{total} animals over the cap")))
                } else {
                    None
                }
            })
            .collect();
        // Every replacement is a fresh founding, in parallel, on its own seed. (Copying
        // the healthiest world, tried first, collapsed diversity: in the first trial the
        // worlds boomed past the cap within a generation of each other and one donor
        // ended up cloned into 11 of 16 slots.)
        let fresh: Vec<(usize, u64)> = bad
            .iter()
            .map(|(i, _)| {
                let seed = next_fresh_seed;
                next_fresh_seed += WORLD_SEED_STRIDE;
                (*i, seed)
            })
            .collect();
        let founded: Vec<LiveWorld> = parallel(fresh.clone(), spec.threads, |(slot, seed)| {
            found(spec, slot, seed, &lineages)
        });
        for (((i, reason), (_, seed)), new) in bad.into_iter().zip(fresh).zip(founded) {
            check_world(run, &new, None)?;
            worlds[i] = new;
            replaced.push(Replacement {
                world: i,
                reason,
                with: format!("fresh seed {seed}"),
            });
        }

        let ticks: u64 = results.iter().map(|r| r.ticks).sum();
        let record = GenerationRecord {
            body_scores: results.iter().flat_map(|r| r.scores.iter().copied()).collect(),
            generation,
            wall_s: started.elapsed().as_secs_f64(),
            elapsed_s: run_started.elapsed().as_secs_f64(),
            ticks,
            body_windows: results.iter().map(|r| r.scores.len()).sum(),
            lineages: updates,
            replaced,
            intake_anomalies: results.iter().map(|r| r.intake_anomalies).sum(),
            readout: pooled,
        };
        println!("{}", summary_line(&record));
        writeln!(log, "{}", serde_json::to_string(&record).map_err(|e| e.to_string())?)
            .map_err(|e| format!("generation log: {e}"))?;
        log.flush().map_err(|e| format!("generation log flush: {e}"))?;
        next_generation = generation + 1;
        records.push(record);
        if spec.save_every > 0 && next_generation % spec.save_every == 0 {
            save_centres(spec, &lineages, next_generation, "latest")?;
            save_checkpoint(spec, &lineages, &worlds, next_generation, next_fresh_seed)?;
        }
        last_gen_wall = started.elapsed().as_secs_f64();
        // STOP/wall pauses always commit the entire current boundary, even between saves.
        if (next_generation < spec.generations || run.dir.join("STOP").exists()) && run.boundary(next_generation, last_gen_wall) {
            save_checkpoint(spec, &lineages, &worlds, next_generation, next_fresh_seed)?;
            break;
        }
    }
    save_centres(spec, &lineages, next_generation, "final")?;
    save_checkpoint(spec, &lineages, &worlds, next_generation, next_fresh_seed)?;
    Ok(records)
}

/// One compact line per generation for the console.
pub fn summary_line(r: &GenerationRecord) -> String {
    let get = |name: &str| {
        r.readout
            .iter()
            .find(|(n, _)| n == name)
            .map_or(f64::NAN, |(_, v)| *v)
    };
    let mut s = format!(
        "gen {:>4} {:>6.1}s bw={:<5}",
        r.generation, r.wall_s, r.body_windows
    );
    for f in Founder::ALL {
        let n = f.name();
        s.push_str(&format!(
            " | {n}: pop={:.0} mov={:.2} pace={:.3} b/h={:.1} st/h={:.1} dr/h={:.1} sc={:.3} \
             nov={:.3} cur={:.3}",
            get(&format!("{n}.population")),
            get(&format!("{n}.moving_share")),
            get(&format!("{n}.pace_bl_per_s")),
            get(&format!("{n}.births_per_h")),
            get(&format!("{n}.starved_per_h")),
            get(&format!("{n}.drowned_per_h")),
            get(&format!("{n}.mean_score")),
            get(&format!("{n}.mean_novelty")),
            get(&format!("{n}.mean_curiosity")),
        ));
    }
    s.push_str(&format!(
        " | stands={:.0} tick_ms={:.3} repl={}",
        get("plants.stands"),
        get("cost.tick_ms"),
        r.replaced.len()
    ));
    s
}

/// Where a run's final centre for `founder` is written.
pub fn final_centre(out: &Path, founder: Founder) -> PathBuf {
    out.join("centres")
        .join(format!("final-{}.json", founder.name()))
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    fn setup(name: &str) -> (LiveSpec, Vec<Lineage>, Vec<LiveWorld>) {
        let out = std::env::temp_dir().join(format!("cub-live-recovery-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let path = crate::es::voxel::store::test_centre_path(Founder::Blind, name);
        let template = VoxelPolicyFile::load_or_transfer(&path).unwrap();
        let mut lineages = vec![Lineage { founder: Founder::Blind, theta: template.theta.clone(), adam: Adam::new(template.theta.len()), template }];
        let cfg = cubarium_voxel::Config { width: 64, height: 24, depth: 6, ..Default::default() };
        let mut world = cubarium_voxel_sim::scene::authored(cfg.clone());
        let mut flora = cubarium_voxel_flora::Flora::new(Default::default());
        let mut fauna = cubarium_voxel_fauna::Fauna::new(cubarium_voxel_fauna::FaunaConfig::placeholder());
        let mut counts = [0; Founder::COUNT];
        counts[Founder::Blind.index()] = 1;
        cubarium_voxel_sim::habitat::seed_with_founder_counts(&mut world, &mut flora, &mut fauna, counts);
        let spec = LiveSpec { centres: vec![(Founder::Blind, path)], worlds: 1, threads: 1, pairs: 1,
            window_ticks: 100, generations: 2, wall_seconds: 60.0, first_generation_seconds: 60.0,
            sigma: 0.05, learning_rate: 0.01, train_seed: 3, world_seed: 7, water_every: 1,
            population_cap: 100, save_every: 1, out, resume: None, ecology: EcologyParams::defaults(),
            world: Some(cfg), curiosity: Default::default(), stomach_aware_reward: false };
        let mut sim = Sim::ambient(world, flora, fauna, cubarium_voxel_sim::SimConfig { threads: 1, overlap: false, water_every: 1 });
        prepare(&mut sim, &spec, &lineages);
        for _ in 0..10 { sim.step(); }
        let info = world::FoundInfo { seed: 7, accepted: true, lake_rejected: 0, habitat_rejected: 0,
            founders: counts, stands: sim.flora().view().stands.len() };
        let mut w = LiveWorld { id: 0, sim, ecology: spec.ecology.clone(), info, windows: 1, memory: Default::default() };
        let id = w.sim.fauna().view().animals[0].id;
        let c = super::super::curiosity::LineageCuriosity { weight: 1.0, memory_min: 2.0, cell_bl: 1.0 };
        let g = super::super::curiosity::Geometry::of(w.sim.fauna().config().founder(Founder::Blind), &c);
        w.memory.observe(id, &g, 0.0, 0.0);
        w.memory.advance();
        w.memory.observe(id, &g, 1.0, 0.0);
        // Nonzero optimizer history and a changed centre must survive too.
        let scores = [1.0, -1.0];
        let l = &mut lineages[0];
        ascend(&mut l.theta, &mut l.adam, &scores, spec.sigma, spec.train_seed, 0);
        (spec, lineages, vec![w])
    }

    #[test]
    fn pause_restores_same_ids_ticks_familiarity_and_optimizer() {
        let (spec, mut lineages, worlds) = setup("pause");
        let ids: Vec<_> = worlds[0].sim.fauna().view().animals.iter().map(|a| a.id).collect();
        let tick = worlds[0].sim.world().tick();
        let optimizer = serde_json::to_string(&lineages[0].adam).unwrap();
        let theta = lineages[0].theta.clone();
        super::super::staging::execute(&spec.out, false, 60.0, 60.0, |run| {
            save_checkpoint(&spec, &lineages, &worlds, 1, 1007)?;
            std::fs::write(spec.out.join("STOP"), "pause").unwrap();
            assert!(run.boundary(1, 0.01));
            Ok(())
        }).unwrap();
        assert!(spec.out.join("PAUSED").is_file());
        super::super::staging::execute(&spec.out, true, 60.0, 60.0, |_| {
            let (mut restored, seed, generation) = load_checkpoint(&spec, &spec.out, &mut lineages)?;
            assert_eq!((seed, generation), (1007, 1));
            assert_eq!(restored[0].sim.world().tick(), tick);
            assert_eq!(restored[0].sim.fauna().view().animals.iter().map(|a| a.id).collect::<Vec<_>>(), ids);
            assert_eq!(lineages[0].theta, theta);
            assert_eq!(serde_json::to_string(&lineages[0].adam).unwrap(), optimizer);
            assert_eq!(restored[0].memory.now(), worlds[0].memory.now());
            assert!(restored[0].memory.knows(ids[0]));
            let c = super::super::curiosity::LineageCuriosity { weight: 1.0, memory_min: 2.0, cell_bl: 1.0 };
            let g = super::super::curiosity::Geometry::of(restored[0].sim.fauna().config().founder(Founder::Blind), &c);
            let mut before = worlds[0].memory.clone();
            assert_eq!(restored[0].memory.observe(ids[0], &g, 0.0, 0.0), before.observe(ids[0], &g, 0.0, 0.0));
            cubarium_voxel_sim::saved::driven(restored[0].sim.fauna())?;
            // Continue the restored world, rather than refounding it.
            restored[0].sim.step();
            assert_eq!(restored[0].sim.world().tick(), tick + 1);
            Ok(())
        }).unwrap();
        assert!(spec.out.join("DONE").is_file());
        std::fs::remove_dir_all(&spec.out).unwrap();
    }

    #[test]
    fn failed_training_checkpoint_keeps_the_previous_boundary() {
        let (spec, mut lineages, worlds) = setup("write-failure");
        save_checkpoint(&spec, &lineages, &worlds, 1, 1007).unwrap();
        let old = std::fs::read_to_string(spec.out.join("checkpoint.json")).unwrap();
        // This is a deterministic unwritable destination even when the suite runs as root.
        let target = if old == "checkpoint-a" { "checkpoint-b" } else { "checkpoint-a" };
        std::fs::write(spec.out.join(target), "obstruction").unwrap();
        let err = super::super::staging::execute(&spec.out, false, 60.0, 60.0, |_| save_checkpoint(&spec, &lineages, &worlds, 2, 2007)).unwrap_err();
        assert!(err.contains(target), "{err}");
        assert!(spec.out.join("FAILED").is_file());
        assert_eq!(std::fs::read_to_string(spec.out.join("checkpoint.json")).unwrap(), old);
        assert_eq!(load_checkpoint(&spec, &spec.out, &mut lineages).unwrap().2, 1);
        std::fs::remove_dir_all(&spec.out).unwrap();
    }
}
