//! **The ecology half**: a world-level search over the ecology parameter vector
//! (`design/handoffs/voxel-ecology-search-2026-09-24.md`).
//!
//! A candidate is an [`EcologyParams`] vector, searched in log space by CMA-ES
//! ([`crate::es::cma`]). Each candidate is scored on **freshly founded** worlds — the
//! training world (`small`, still water), coarse water — run for a horizon with the
//! brains held at fixed centres, and the score is a [`penalty`]: zero inside the band
//! (animals per world within `[lo, hi]` over the second half, never above `peak`,
//! browsers moving at least `moving_min` of their minutes), growing with the normalised
//! distance outside it, and a large constant per lineage or plant species lost.
//!
//! Common random numbers: every candidate of a generation (and the search mean,
//! evaluated beside them for the curve) runs on the same founding seeds; the seeds change
//! from one generation to the next.
//!
//! A world that booms past [`EvalSpec::abort_animals`] or loses a lineage stops early;
//! its trace carries its last count to the horizon, which is the trend the penalty reads.
//!
//! **Unified mode** (`--brain-pairs > 0`, not used by default): the same worlds also
//! carry antithetic brain variants, and every lineage's centre takes one ES step per
//! generation from the pooled body scores, as the live trainer does.

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use cubarium_voxel_fauna::Founder;
use cubarium_voxel_flora::Species;
use serde::Serialize;

use super::ecology::{EcologyParams, PARAMS};
use super::readout::Scan;
use super::trainer::{ascend, parallel, pooled_scores};
use super::variants::VariantTable;
use super::window::{self, BodyScore, TICKS_PER_MIN, Window};
use super::world;
use crate::es::cma::Cma;
use crate::es::optimizer::Adam;
use crate::es::voxel::EpisodeDriver;
use crate::es::voxel::store::VoxelPolicyFile;

/// The target band.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Band {
    /// Animals per world, second half of the horizon.
    pub lo: f64,
    pub hi: f64,
    /// Animals per world, never above.
    pub peak: f64,
    /// Browser moving share (body-minutes over 0.1 BL/s), second half.
    pub moving_min: f64,
    /// Per lineage or plant species lost by the end.
    pub missing_cost: f64,
}

impl Default for Band {
    fn default() -> Band {
        Band {
            lo: 30.0,
            hi: 80.0,
            peak: 120.0,
            moving_min: 0.30,
            missing_cost: 10.0,
        }
    }
}

/// What a candidate world did, as the penalty reads it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Trace {
    pub horizon_min: f64,
    /// Sample times, simulated minutes (evenly spaced).
    pub minutes: Vec<f64>,
    /// Animals in the world at each sample.
    pub animals: Vec<usize>,
    /// Browser moving share over the second half (or all it ran, if it stopped before).
    pub browser_moving: f64,
    /// Lineages and plant species (present at the start) absent at the end.
    pub missing: usize,
    /// Lineages with no birth in the second half of what was run: the stop-breeding
    /// bound, refused as hard as a loss (`missing_cost` each).
    #[serde(default)]
    pub silent_lineages: usize,
}

/// The penalty and its parts.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Penalty {
    pub bound: f64,
    pub peak: f64,
    pub lively: f64,
    pub persist: f64,
    pub total: f64,
}

/// Zero inside the band; outside it the normalised distance: the mean over the second
/// half's samples of `(lo − n)/lo` below and `(n − hi)/hi` above, plus
/// `(peak_n − peak)/peak`, plus `(moving_min − m)/moving_min`, plus `missing_cost` per
/// thing lost. A trace that stopped early is carried at its last count to the horizon.
pub fn penalty(t: &Trace, band: &Band) -> Penalty {
    let step = if t.minutes.len() >= 2 {
        t.minutes[1] - t.minutes[0]
    } else {
        1.0
    };
    let mut samples: Vec<(f64, f64)> = t
        .minutes
        .iter()
        .zip(&t.animals)
        .map(|(m, n)| (*m, *n as f64))
        .collect();
    if let Some(&(mut m, n)) = samples.last() {
        while m + step <= t.horizon_min + 1e-9 {
            m += step;
            samples.push((m, n));
        }
    }
    let half = t.horizon_min / 2.0;
    let late: Vec<f64> = samples
        .iter()
        .filter(|(m, _)| *m > half)
        .map(|(_, n)| (band.lo - n).max(0.0) / band.lo + (n - band.hi).max(0.0) / band.hi)
        .collect();
    let bound = if late.is_empty() {
        0.0
    } else {
        late.iter().sum::<f64>() / late.len() as f64
    };
    let peak_n = samples.iter().map(|s| s.1).fold(0.0, f64::max);
    let peak = (peak_n - band.peak).max(0.0) / band.peak;
    let lively = (band.moving_min - t.browser_moving).max(0.0) / band.moving_min;
    let persist = band.missing_cost * (t.missing + t.silent_lineages) as f64;
    Penalty {
        bound,
        peak,
        lively,
        persist,
        total: bound + peak + lively + persist,
    }
}

/// How one candidate world is run.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct EvalSpec {
    pub horizon_ticks: u64,
    pub window_ticks: u64,
    pub water_every: u32,
    /// Stop a world that holds more animals than this.
    pub abort_animals: usize,
    pub band: Band,
    /// The horizon the band is scored on, ticks. Past what was run (`horizon_ticks`, or
    /// an early stop) the animal count is projected on its last hour's trend
    /// ([`project`]). Equal to `horizon_ticks` is no projection.
    pub score_horizon_ticks: u64,
}

/// Extend a trace to `to_min` on the exponential trend of its last hour: rate
/// `ln(n_end / n_end−60) / 60` per minute, clamped to ±0.05, the count capped at 2000.
/// Returns the minute the projection starts from.
pub fn project(t: &mut Trace, to_min: f64) -> f64 {
    let Some(&last_min) = t.minutes.last() else {
        return 0.0;
    };
    let n_end = *t.animals.last().expect("paired") as f64;
    let back = t
        .minutes
        .iter()
        .position(|m| *m >= last_min - 60.0)
        .unwrap_or(0);
    let span = (last_min - t.minutes[back]).max(1.0);
    let rate = if span >= 30.0 {
        ((n_end.max(1.0)) / (t.animals[back] as f64).max(1.0)).ln() / span
    } else {
        0.0
    };
    let rate = rate.clamp(-0.05, 0.05);
    let step = if t.minutes.len() >= 2 {
        t.minutes[1] - t.minutes[0]
    } else {
        1.0
    };
    let mut m = last_min;
    while m + step <= to_min + 1e-9 {
        m += step;
        t.minutes.push(m);
        t.animals
            .push((n_end * (rate * (m - last_min)).exp()).min(2000.0).round() as usize);
    }
    t.horizon_min = to_min;
    last_min
}

/// One candidate world's result.
#[derive(Clone, Debug, Serialize)]
pub struct Evaluation {
    pub base_seed: u64,
    pub world_seed: u64,
    pub trace: Trace,
    pub penalty: Penalty,
    /// Why it stopped early, if it did.
    pub stopped: Option<String>,
    /// The minute the scored trace turns from measured to projected, if it does.
    pub projected_from_min: Option<f64>,
    /// Every readout factor, averaged over the windows it ran.
    pub readout_mean: Vec<(String, f64)>,
    /// And as the last window left it.
    pub readout_end: Vec<(String, f64)>,
    pub species_start: Vec<usize>,
    pub species_end: Vec<usize>,
    pub lineages_end: Vec<usize>,
    /// Births per lineage in the second half of what was run.
    pub late_births: Vec<u64>,
    pub ticks: u64,
    pub wall_s: f64,
    #[serde(skip)]
    pub body_scores: Vec<BodyScore>,
}

/// Found a training world under `params` from seed base `base`, drive its lineages with
/// `drivers`, and run it for the horizon (unified mode: with `tables`).
pub fn evaluate(
    params: &EcologyParams,
    base: u64,
    drivers: &[(Founder, EpisodeDriver)],
    tables: &[Option<VariantTable>],
    spec: &EvalSpec,
) -> Evaluation {
    let started = Instant::now();
    let cfg = world::training_config();
    let (mut sim, info) = world::found_world(
        &cfg,
        base,
        cubarium_voxel_sim::habitat::FOUNDER_COUNTS,
        params,
    );
    super::install_centres(sim.fauna_mut(), drivers);
    sim.world_mut().set_water_every(spec.water_every);
    let species_start = Scan::of(&sim).stands.to_vec();
    let horizon_min = spec.horizon_ticks as f64 / TICKS_PER_MIN as f64;
    let half_ticks = spec.horizon_ticks / 2;
    let mut trace = Trace {
        horizon_min,
        ..Trace::default()
    };
    let (mut late_moving, mut late_minutes, mut all_moving, mut all_minutes) =
        (0u64, 0u64, 0u64, 0u64);
    let mut late_births = vec![0u64; Founder::COUNT];
    let mut readouts: Vec<Vec<(String, super::readout::Pool, f64)>> = Vec::new();
    let mut body_scores = Vec::new();
    let mut stopped = None;
    let mut elapsed = 0u64;
    let zeros = vec![0usize; Founder::COUNT];
    'run: while elapsed < spec.horizon_ticks {
        let ticks = spec.window_ticks.min(spec.horizon_ticks - elapsed);
        let window_start = elapsed;
        let mut w = Window::begin(&mut sim, tables, &zeros, ticks);
        let mut halt = None;
        while !w.done() {
            w.step(&mut sim);
            elapsed += 1;
            if elapsed.is_multiple_of(TICKS_PER_MIN) {
                let pop = window::population(&sim);
                let total = sim.fauna().view().animals.len();
                trace.minutes.push((elapsed / TICKS_PER_MIN) as f64);
                trace.animals.push(total);
                if total > spec.abort_animals {
                    halt = Some(format!(
                        "{total} animals at minute {}",
                        elapsed / TICKS_PER_MIN
                    ));
                } else if let Some(f) = Founder::ALL.iter().find(|f| pop[f.index()] == 0) {
                    halt = Some(format!("the {} lineage died out", f.name()));
                }
                if halt.is_some() {
                    break;
                }
            }
        }
        let r = w.finish(&sim, 0);
        let b = &r.lineages[Founder::Browser.index()];
        all_moving += b.moving_minutes;
        all_minutes += b.body_minutes;
        if window_start >= half_ticks {
            late_moving += b.moving_minutes;
            late_minutes += b.body_minutes;
            for f in Founder::ALL {
                late_births[f.index()] += r.lineages[f.index()].births;
            }
        }
        body_scores.extend(r.scores);
        readouts.push(r.readout);
        if let Some(h) = halt {
            stopped = Some(h);
            break 'run;
        }
    }
    trace.browser_moving = if late_minutes > 0 {
        late_moving as f64 / late_minutes as f64
    } else if all_minutes > 0 {
        all_moving as f64 / all_minutes as f64
    } else {
        0.0
    };
    let pop = window::population(&sim);
    let species_end = Scan::of(&sim).stands.to_vec();
    trace.missing = Founder::ALL.iter().filter(|f| pop[f.index()] == 0).count()
        + species_start
            .iter()
            .zip(&species_end)
            .filter(|(s, e)| **s > 0 && **e == 0)
            .count();
    let score_min = spec.score_horizon_ticks.max(spec.horizon_ticks) as f64 / TICKS_PER_MIN as f64;
    let projected_from_min = (trace.minutes.last().copied().unwrap_or(0.0) < score_min)
        .then(|| project(&mut trace, score_min));
    trace.silent_lineages = if elapsed >= half_ticks {
        late_births.iter().filter(|b| **b == 0).count()
    } else {
        // Stopped before the second half: judged by the lineages it still has.
        Founder::ALL.len()
    };
    let penalty = penalty(&trace, &spec.band);
    let readout_mean = super::readout::pool(&readouts)
        .into_iter()
        .map(|(name, v)| {
            // `pool` sums `Sum` factors over the list; here the list is one world's
            // windows, so every factor is averaged instead.
            let vals: Vec<f64> = readouts
                .iter()
                .filter_map(|r| r.iter().find(|x| x.0 == name).map(|x| x.2))
                .filter(|x| x.is_finite())
                .collect();
            let m = if vals.is_empty() {
                v
            } else {
                vals.iter().sum::<f64>() / vals.len() as f64
            };
            (name, m)
        })
        .collect();
    let readout_end = readouts
        .last()
        .map(|r| r.iter().map(|(n, _, v)| (n.clone(), *v)).collect())
        .unwrap_or_default();
    Evaluation {
        base_seed: base,
        world_seed: info.seed,
        trace,
        penalty,
        stopped,
        projected_from_min,
        readout_mean,
        readout_end,
        species_start,
        species_end,
        lineages_end: pop.to_vec(),
        late_births,
        ticks: elapsed,
        wall_s: started.elapsed().as_secs_f64(),
        body_scores,
    }
}

/// One search run.
#[derive(Clone, Debug)]
pub struct SearchSpec {
    pub centres: Vec<(Founder, PathBuf)>,
    pub lambda: usize,
    pub seeds_per_generation: usize,
    pub generations: u32,
    pub wall_seconds: f64,
    pub sigma0: f64,
    pub threads: usize,
    /// Generation `g`'s seeds are `seed_base + (g·seeds_per_generation + s)·1000`.
    pub seed_base: u64,
    pub cma_seed: u64,
    pub eval: EvalSpec,
    /// Unified mode: brain antithetic pairs per lineage (0 = brains held, the default).
    pub brain_pairs: usize,
    pub brain_sigma: f64,
    pub brain_learning_rate: f64,
    pub out: PathBuf,
}

/// One candidate's record in a generation.
#[derive(Clone, Debug, Serialize)]
pub struct CandidateRecord {
    /// `c0..` a CMA draw, `mean` the search mean.
    pub label: String,
    pub multipliers: Vec<(String, f64)>,
    /// Mean penalty over the seeds plus the box penalty (what CMA was told).
    pub fitness: f64,
    pub box_penalty: f64,
    pub evaluations: Vec<Evaluation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchRecord {
    pub generation: u32,
    pub seeds: Vec<u64>,
    pub sigma: f64,
    pub wall_s: f64,
    pub elapsed_s: f64,
    pub candidates: Vec<CandidateRecord>,
}

fn multipliers(e: &EcologyParams) -> Vec<(String, f64)> {
    PARAMS
        .iter()
        .zip(e.values())
        .map(|(p, v)| (p.name.to_string(), *v))
        .collect()
}

/// Re-score `e` with persistence judged against `reference`, the **default** vector's
/// world on the same seed: a species counts as lost only if the default world kept it
/// (and `e`'s own world started with it), since some still-water worlds lose a species
/// whatever the vector (siphonreed, 2026-09-25). Lineages lost always count.
pub fn rescore(e: &mut Evaluation, reference: &Evaluation, band: &Band) {
    let lineages = e.lineages_end.iter().filter(|n| **n == 0).count();
    let species = e
        .species_start
        .iter()
        .zip(&e.species_end)
        .zip(&reference.species_end)
        .filter(|((start, end), kept)| **start > 0 && **kept > 0 && **end == 0)
        .count();
    e.trace.missing = lineages + species;
    e.penalty = penalty(&e.trace, band);
}

/// The box penalty for a log vector outside the search box: squared log distance.
fn box_penalty(x: &[f64]) -> f64 {
    EcologyParams::log_bounds()
        .iter()
        .zip(x)
        .map(|((lo, hi), v)| (lo - v).max(0.0).powi(2) + (v - hi).max(0.0).powi(2))
        .sum::<f64>()
        * 10.0
}

/// One lineage's brain for a check arm: `heuristic` (the fauna's own observation-only
/// forager), `herding` (the browser's forager with cohesion,
/// [`crate::es::voxel::controller::HerdingBrowser`]), or a centre file validated against
/// this build and the lineage.
pub fn brain_driver(founder: Founder, spec: &str) -> Result<EpisodeDriver, String> {
    use crate::es::voxel::controller::VoxelControl;
    match spec {
        "heuristic" => Ok(EpisodeDriver::control(VoxelControl::Heuristic, founder)),
        "herding" if founder == Founder::Browser => {
            Ok(EpisodeDriver::control(VoxelControl::Herding, founder))
        }
        "herding" => Err(format!(
            "herding reads the browser's cone; the {} lineage has none",
            founder.name()
        )),
        path => {
            let file = VoxelPolicyFile::load(std::path::Path::new(path))?;
            if file.founder()? != founder {
                return Err(format!("{path} is not a {} centre", founder.name()));
            }
            file.driver()
        }
    }
}

fn load_centres(centres: &[(Founder, PathBuf)]) -> Result<Vec<(Founder, VoxelPolicyFile)>, String> {
    let mut out = Vec::new();
    for (f, path) in centres {
        let file = VoxelPolicyFile::load(path)?;
        if file.founder()? != *f {
            return Err(format!("{} is not a {} centre", path.display(), f.name()));
        }
        file.driver()?;
        out.push((*f, file));
    }
    out.sort_by_key(|(f, _)| f.index());
    Ok(out)
}

/// Run the search. Writes `generations.jsonl`, `best.toml` (the lowest-fitness vector
/// evaluated so far) and `mean.toml` (the search mean) under `out`.
pub fn search(spec: &SearchSpec) -> Result<Vec<SearchRecord>, String> {
    let run_started = Instant::now();
    std::fs::create_dir_all(&spec.out)
        .map_err(|e| format!("cannot create {}: {e}", spec.out.display()))?;
    let files = load_centres(&spec.centres)?;
    let mut thetas: Vec<(Founder, Vec<f64>, Adam)> = files
        .iter()
        .map(|(f, file)| {
            let mut adam = Adam::new(file.theta.len());
            adam.learning_rate = spec.brain_learning_rate;
            (*f, file.theta.clone(), adam)
        })
        .collect();
    let mut cma = Cma::new(
        vec![0.0; PARAMS.len()],
        spec.sigma0,
        spec.lambda,
        spec.cma_seed,
    );
    let mut log = std::fs::File::create(spec.out.join("generations.jsonl"))
        .map_err(|e| format!("cannot write the log: {e}"))?;
    let mut best: Option<(f64, EcologyParams, u32)> = None;
    let mut records = Vec::new();
    let mut last_gen_wall = 0.0;
    for generation in 0..spec.generations {
        if generation > 0 && run_started.elapsed().as_secs_f64() + last_gen_wall > spec.wall_seconds
        {
            println!("# wall budget: stopping before generation {generation}");
            break;
        }
        let started = Instant::now();
        let seeds: Vec<u64> = (0..spec.seeds_per_generation)
            .map(|s| {
                spec.seed_base
                    + (u64::from(generation) * spec.seeds_per_generation as u64 + s as u64) * 1000
            })
            .collect();
        let xs = cma.ask();
        let mut candidates: Vec<(String, Vec<f64>)> = xs
            .iter()
            .enumerate()
            .map(|(k, x)| (format!("c{k}"), x.clone()))
            .collect();
        // The default vector on the same seeds: the persistence reference and the
        // baseline, in the slot the search mean held.
        candidates.push(("default".into(), vec![0.0; PARAMS.len()]));
        let drivers: Vec<(Founder, EpisodeDriver)> = thetas
            .iter()
            .map(|(f, t, _)| (*f, EpisodeDriver::gru(t, *f).expect("a validated centre")))
            .collect();
        let mut tables: Vec<Option<VariantTable>> = vec![None; Founder::COUNT];
        if spec.brain_pairs > 0 {
            for (f, t, _) in &thetas {
                tables[f.index()] = Some(VariantTable::antithetic(
                    *f,
                    t,
                    spec.cma_seed,
                    spec.brain_sigma,
                    generation,
                    spec.brain_pairs,
                )?);
            }
        }
        let jobs: Vec<(usize, u64)> = (0..candidates.len())
            .flat_map(|k| seeds.iter().map(move |&s| (k, s)))
            .collect();
        let params: Vec<EcologyParams> = candidates
            .iter()
            .map(|(_, x)| EcologyParams::from_log(x))
            .collect();
        let mut evals: Vec<Evaluation> = parallel(jobs.clone(), spec.threads, |(k, seed)| {
            evaluate(&params[k], seed, &drivers, &tables, &spec.eval)
        });
        let reference_k = candidates.len() - 1;
        let references: Vec<Evaluation> = jobs
            .iter()
            .zip(&evals)
            .filter(|((k, _), _)| *k == reference_k)
            .map(|(_, e)| e.clone())
            .collect();
        for ((_, seed), e) in jobs.iter().zip(evals.iter_mut()) {
            let r = references
                .iter()
                .find(|r| r.base_seed == *seed)
                .expect("a reference per seed");
            rescore(e, r, &spec.eval.band);
        }
        let mut recs: Vec<CandidateRecord> = Vec::new();
        for (k, (label, x)) in candidates.iter().enumerate() {
            let mine: Vec<Evaluation> = jobs
                .iter()
                .zip(&evals)
                .filter(|((j, _), _)| *j == k)
                .map(|(_, e)| e.clone())
                .collect();
            let mean_pen =
                mine.iter().map(|e| e.penalty.total).sum::<f64>() / mine.len().max(1) as f64;
            let bp = box_penalty(x);
            recs.push(CandidateRecord {
                label: label.clone(),
                multipliers: multipliers(&params[k]),
                fitness: mean_pen + bp,
                box_penalty: bp,
                evaluations: mine,
            });
        }
        let fitness: Vec<f64> = recs[..xs.len()].iter().map(|r| r.fitness).collect();
        // Best so far, by the in-box penalty, over the draws and the mean.
        for (k, r) in recs.iter().enumerate() {
            let pen = r.fitness - r.box_penalty;
            if best.as_ref().is_none_or(|b| pen < b.0) {
                best = Some((pen, params[k].clone(), generation));
            }
        }
        cma.tell(&xs, &fitness);
        // Unified mode: one brain step per lineage from every world's bodies.
        if spec.brain_pairs > 0 {
            for (f, theta, adam) in thetas.iter_mut() {
                let per_world: Vec<Vec<(usize, f64)>> = evals
                    .iter()
                    .map(|e| {
                        e.body_scores
                            .iter()
                            .filter(|b| b.founder == *f)
                            .map(|b| (b.candidate, b.score.score))
                            .collect()
                    })
                    .collect();
                let (scores, _) = pooled_scores(&per_world, 2 * spec.brain_pairs);
                ascend(
                    theta,
                    adam,
                    &scores,
                    spec.brain_sigma,
                    spec.cma_seed,
                    generation,
                );
            }
        }
        let record = SearchRecord {
            generation,
            seeds,
            sigma: cma.sigma(),
            wall_s: started.elapsed().as_secs_f64(),
            elapsed_s: run_started.elapsed().as_secs_f64(),
            candidates: recs,
        };
        last_gen_wall = record.wall_s;
        println!("{}", summary(&record, best.as_ref().map(|b| b.0)));
        let _ = writeln!(
            log,
            "{}",
            serde_json::to_string(&record).map_err(|e| e.to_string())?
        );
        let _ = log.flush();
        let mean = EcologyParams::from_log(cma.mean());
        mean.save(
            &spec.out.join("mean.toml"),
            &format!("Ecology search mean after generation {generation}. Training only."),
        )?;
        if let Some((pen, e, g)) = &best {
            e.save(
                &spec.out.join("best.toml"),
                &format!(
                    "Lowest-penalty ecology vector so far ({pen:.4}, generation {g}). Training only;\nthe live defaults are unchanged."
                ),
            )?;
        }
        if spec.brain_pairs > 0 {
            for ((f, theta, _), (_, file)) in thetas.iter().zip(&files) {
                let out = VoxelPolicyFile {
                    theta: theta.clone(),
                    generation: Some(u64::from(generation + 1)),
                    ..file.clone()
                };
                out.write(&spec.out.join(format!("centre-{}.json", f.name())))?;
            }
        }
        records.push(record);
    }
    Ok(records)
}

fn summary(r: &SearchRecord, best: Option<f64>) -> String {
    let mut f: Vec<f64> = r.candidates.iter().map(|c| c.fitness).collect();
    f.sort_by(f64::total_cmp);
    let mean = r.candidates.iter().find(|c| c.label == "default");
    let mut s = format!(
        "gen {:>3} {:>6.0}s σ={:.3} fit min={:.3} med={:.3} | best so far {:.3}",
        r.generation,
        r.wall_s,
        r.sigma,
        f[0],
        f[f.len() / 2],
        best.unwrap_or(f64::NAN)
    );
    if let Some(m) = mean {
        let end: Vec<String> = m
            .evaluations
            .iter()
            .map(|e| {
                format!(
                    "{}{}",
                    e.trace.animals.last().copied().unwrap_or(0),
                    if e.stopped.is_some() { "!" } else { "" }
                )
            })
            .collect();
        s.push_str(&format!(
            " | default fit={:.3} animals_end={} mult=[{}]",
            m.fitness,
            end.join(","),
            m.multipliers
                .iter()
                .map(|(_, v)| format!("{v:.2}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    s
}

/// The held-out check: `arms` (a name and a vector each) × `seeds`, `hours` of the
/// training world with the given water cadence, one CSV per world under `out` with one
/// row per window.
pub fn check(
    arms: &[(String, EcologyParams)],
    seeds: &[u64],
    hours: f64,
    window_min: f64,
    water_every: u32,
    centres: &[(Founder, PathBuf)],
    threads: usize,
    out: &std::path::Path,
) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    let drivers: Vec<(Founder, EpisodeDriver)> = centres
        .iter()
        .map(|(f, p)| Ok((*f, brain_driver(*f, &p.display().to_string())?)))
        .collect::<Result<_, String>>()?;
    let jobs: Vec<(usize, u64)> = (0..arms.len())
        .flat_map(|a| seeds.iter().map(move |&s| (a, s)))
        .collect();
    let horizon = (hours * 3600.0 * f64::from(cubarium_voxel_fauna::TICK_HZ)).round() as u64;
    let window_ticks = (window_min * TICKS_PER_MIN as f64).round() as u64;
    let results: Vec<Result<(), String>> = parallel(jobs, threads, |(a, seed)| {
        let (name, params) = &arms[a];
        let cfg = world::training_config();
        let (mut sim, info) = world::found_world(
            &cfg,
            seed,
            cubarium_voxel_sim::habitat::FOUNDER_COUNTS,
            params,
        );
        super::install_centres(sim.fauna_mut(), &drivers);
        sim.world_mut().set_water_every(water_every);
        let path = out.join(format!("{name}__seed{seed}.csv"));
        let mut f = std::fs::File::create(&path)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        let mut header = vec!["minute".to_string(), "world_seed".into(), "animals".into()];
        for l in Founder::ALL {
            header.push(format!("{}.population", l.name()));
            header.push(format!("{}.births", l.name()));
            for d in cubarium_voxel_fauna::Departure::ALL {
                header.push(format!("{}.{}", l.name(), d.name()));
            }
            header.push(format!("{}.moving_share", l.name()));
            header.push(format!("{}.pace_bl_per_s", l.name()));
            for c in ["intake_per_body_h", "walked_m_per_body_h", "nn_m"] {
                header.push(format!("{}.{c}", l.name()));
            }
        }
        for s in Species::ALL {
            header.push(format!("stands.{}", s.name()));
        }
        header.extend(
            [
                "foliage",
                "litter",
                "lake_level_m",
                "soil_moisture",
                "tick_ms",
            ]
            .map(String::from),
        );
        let _ = writeln!(f, "{}", header.join(","));
        let tables = vec![None; Founder::COUNT];
        let zeros = vec![0usize; Founder::COUNT];
        let mut elapsed = 0u64;
        while elapsed < horizon {
            let ticks = window_ticks.min(horizon - elapsed);
            let mut w = Window::begin(&mut sim, &tables, &zeros, ticks);
            while !w.done() {
                w.step(&mut sim);
            }
            elapsed += ticks;
            let r = w.finish(&sim, 0);
            let pop = window::population(&sim);
            let get = |n: &str| {
                r.readout
                    .iter()
                    .find(|x| x.0 == n)
                    .map_or(f64::NAN, |x| x.2)
            };
            let mut row = vec![
                format!("{:.0}", elapsed as f64 / TICKS_PER_MIN as f64),
                info.seed.to_string(),
                sim.fauna().view().animals.len().to_string(),
            ];
            for l in Founder::ALL {
                let s = &r.lineages[l.index()];
                row.push(pop[l.index()].to_string());
                row.push(s.births.to_string());
                for d in s.deaths {
                    row.push(d.to_string());
                }
                row.push(format!(
                    "{:.4}",
                    if s.body_minutes > 0 {
                        s.moving_minutes as f64 / s.body_minutes as f64
                    } else {
                        f64::NAN
                    }
                ));
                row.push(format!(
                    "{:.4}",
                    get(&format!("{}.pace_bl_per_s", l.name()))
                ));
                for c in ["intake_per_body_h", "walked_m_per_body_h", "nn_m"] {
                    row.push(format!("{:.6}", get(&format!("{}.{c}", l.name()))));
                }
            }
            let scan = Scan::of(&sim);
            for s in Species::ALL {
                row.push(scan.stands[s.index()].to_string());
            }
            row.push(format!("{:.3}", scan.foliage));
            row.push(format!("{:.3}", scan.litter));
            row.push(format!("{:.3}", scan.lake_level_m));
            row.push(format!("{:.4}", scan.soil_moisture));
            row.push(format!("{:.3}", get("cost.tick_ms")));
            let _ = writeln!(f, "{}", row.join(","));
            let _ = f.flush();
        }
        println!("{name} seed {seed} done");
        Ok(())
    });
    results.into_iter().collect()
}
