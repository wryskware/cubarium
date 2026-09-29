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
use serde::Serialize;

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
    /// The ecology every world is founded under (training only; defaults: the shipped
    /// world).
    pub ecology: EcologyParams,
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
    let cfg = world::training_config();
    let ecology = spec.ecology.clone();
    let (mut sim, info) = world::found_world(
        &cfg,
        base,
        cubarium_voxel_sim::habitat::FOUNDER_COUNTS,
        &ecology,
    );
    prepare(&mut sim, spec, lineages);
    LiveWorld {
        id: slot,
        sim,
        ecology,
        info,
        windows: 0,
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

fn save_centres(spec: &LiveSpec, lineages: &[Lineage], generation: u32, tag: &str) {
    let dir = spec.out.join("centres");
    let _ = std::fs::create_dir_all(&dir);
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
        if let Err(e) = file.write(&path) {
            eprintln!("WARNING: cannot save {}: {e}", path.display());
        }
    }
}

/// Run the loop. Returns the records it wrote.
pub fn train(spec: &LiveSpec) -> Result<Vec<GenerationRecord>, String> {
    let run_started = Instant::now();
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

    // Found the worlds, in parallel.
    let t = Instant::now();
    let bases: Vec<(usize, u64)> = (0..spec.worlds)
        .map(|i| (i, spec.world_seed + i as u64 * WORLD_SEED_STRIDE))
        .collect();
    let mut worlds: Vec<LiveWorld> = parallel(bases, spec.threads, |(slot, base)| {
        found(spec, slot, base, &lineages)
    });
    let mut next_fresh_seed = spec.world_seed + spec.worlds as u64 * WORLD_SEED_STRIDE;
    println!(
        "# founded {} worlds in {:.1} s: seeds {:?}",
        worlds.len(),
        t.elapsed().as_secs_f64(),
        worlds.iter().map(|w| w.info.seed).collect::<Vec<_>>()
    );
    let mut log = std::fs::File::create(spec.out.join("generations.jsonl"))
        .map_err(|e| format!("cannot write the generation log: {e}"))?;
    save_centres(spec, &lineages, 0, "gen0");

    let mut records = Vec::new();
    let candidates = 2 * spec.pairs;
    for generation in 0..spec.generations {
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
        let results: Vec<WindowResult> = parallel(jobs, spec.threads, |(w, first)| {
            window::run_window(w, tables_ref, &first, spec.window_ticks)
        });

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
            worlds[i] = new;
            replaced.push(Replacement {
                world: i,
                reason,
                with: format!("fresh seed {seed}"),
            });
        }

        let ticks: u64 = results.iter().map(|r| r.ticks).sum();
        let record = GenerationRecord {
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
        let _ = writeln!(
            log,
            "{}",
            serde_json::to_string(&record).map_err(|e| e.to_string())?
        );
        let _ = log.flush();
        records.push(record);
        if spec.save_every > 0 && (generation + 1) % spec.save_every == 0 {
            save_centres(
                spec,
                &lineages,
                generation + 1,
                &format!("gen{}", generation + 1),
            );
        }
        if run_started.elapsed().as_secs_f64() > spec.wall_seconds {
            println!("# wall budget reached after generation {generation}");
            break;
        }
    }
    let last = records.last().map_or(0, |r| r.generation + 1);
    save_centres(spec, &lineages, last, "final");
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
            " | {n}: pop={:.0} mov={:.2} pace={:.3} b/h={:.1} st/h={:.1} dr/h={:.1} sc={:.3}",
            get(&format!("{n}.population")),
            get(&format!("{n}.moving_share")),
            get(&format!("{n}.pace_bl_per_s")),
            get(&format!("{n}.births_per_h")),
            get(&format!("{n}.starved_per_h")),
            get(&format!("{n}.drowned_per_h")),
            get(&format!("{n}.mean_score")),
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
