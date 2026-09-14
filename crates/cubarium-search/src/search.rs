//! The bounded genetic search: deterministic initialization, bounded mutation and crossover,
//! elitism, a fixed seed schedule, and hard evaluation / tick / worker / wall-time limits.
//!
//! Candidate configuration and inherited organism genomes are kept strictly apart: this module
//! only ever writes [`crate::params`] into a [`cubarium_core::WorldConfig`] and the apex
//! profile. The world's own genomes are inherited and mutated by the core, under the core's
//! own rules, and are never touched here.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::evaluate::{Evaluation, Protocol, Status, evaluate};
use crate::metrics::{Objectives, Scoring};
use crate::params;
use crate::rng;

/// The fixed training seed schedule. A search takes the first `Budget::seeds` of these; the
/// rest are held out on purpose, for the separately capped follow-up the handoff asks for.
pub const TRAINING_SEEDS: [u64; 8] = [
    1_001, 1_002, 1_003, 1_004, 1_005, 1_006, 1_007, 1_008,
];

/// Seeds deliberately **not** used for training, reserved for retesting a promising candidate.
pub const HELDOUT_SEEDS: [u64; 8] = [
    9_001, 9_002, 9_003, 9_004, 9_005, 9_006, 9_007, 9_008,
];

/// Every hard limit a search runs under. All of them are checked before any work starts, and
/// every one of them is enforced while it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    /// Hard cap on **actual simulations**. A cached repeat of an elite costs nothing and is
    /// not counted; nothing else is exempt.
    pub max_evaluations: u64,
    pub workers: usize,
    pub wall_seconds: u64,
    pub population: usize,
    pub elite: usize,
    pub generations: u32,
    /// How many of [`TRAINING_SEEDS`] each candidate is evaluated on.
    pub seeds: usize,
    /// Hard cap on result rows written, so a run cannot fill the disk.
    pub max_rows: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Budget {
            max_evaluations: 8,
            workers: 4,
            wall_seconds: 600,
            population: 4,
            elite: 1,
            generations: 3,
            seeds: 1,
            max_rows: 4_096,
        }
    }
}

impl Budget {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_evaluations == 0 {
            return Err("budget.max_evaluations is zero".into());
        }
        if self.workers == 0 {
            return Err("budget.workers is zero".into());
        }
        if self.workers > 32 {
            return Err(format!(
                "budget.workers {} exceeds the 32 logical cores this host has",
                self.workers
            ));
        }
        if self.population < 2 {
            return Err(format!("budget.population {} is below two", self.population));
        }
        if self.elite == 0 {
            return Err("budget.elite is zero: without elitism a generation can lose its best".into());
        }
        if self.elite >= self.population {
            return Err(format!(
                "budget.elite {} must be below budget.population {}",
                self.elite, self.population
            ));
        }
        if self.generations == 0 {
            return Err("budget.generations is zero".into());
        }
        if self.seeds == 0 || self.seeds > TRAINING_SEEDS.len() {
            return Err(format!(
                "budget.seeds {} is outside 1..={}",
                self.seeds,
                TRAINING_SEEDS.len()
            ));
        }
        if self.wall_seconds == 0 {
            return Err("budget.wall_seconds is zero".into());
        }
        if self.max_rows == 0 {
            return Err("budget.max_rows is zero".into());
        }
        Ok(())
    }

    pub fn seed_schedule(&self) -> &[u64] {
        &TRAINING_SEEDS[..self.seeds]
    }
}

/// Bounded variation operators. Both rates are fractions; the mutation step is a fraction of
/// each parameter's own declared range, so no parameter can jump its box in one step.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Variation {
    pub mutation_rate: f64,
    pub mutation_sigma: f64,
    pub crossover_rate: f64,
}

impl Default for Variation {
    fn default() -> Self {
        Variation { mutation_rate: 0.25, mutation_sigma: 0.15, crossover_rate: 0.5 }
    }
}

impl Variation {
    pub fn validate(&self) -> Result<(), String> {
        for (name, v) in [
            ("variation.mutation_rate", self.mutation_rate),
            ("variation.crossover_rate", self.crossover_rate),
        ] {
            if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                return Err(format!("{name} must be in [0, 1], got {v}"));
            }
        }
        if !self.mutation_sigma.is_finite() || !(0.0..=0.5).contains(&self.mutation_sigma) {
            return Err(format!(
                "variation.mutation_sigma must be in [0, 0.5], got {}",
                self.mutation_sigma
            ));
        }
        Ok(())
    }
}

/// Why the search stopped. Always recorded: a truncated search must never read as a finished one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Generations,
    Evaluations,
    WallTime,
    RowLimit,
    NoViableParents,
}

/// One `(candidate, seed)` row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub candidate: u64,
    pub generation: u32,
    pub origin: String,
    /// Readable values. A decimal round trip may lose a bit, so a replay uses `param_bits`.
    pub params: serde_json::Map<String, serde_json::Value>,
    /// The exact `f64` bit patterns, in `params::PARAMS` order: what a replay actually reads.
    pub param_bits: Vec<String>,
    /// Fingerprint of the same vector, so a replay can refuse a row it would misread.
    pub param_fingerprint: u64,
    #[serde(flatten)]
    pub evaluation: Evaluation,
    pub objectives: Option<Objectives>,
    pub fitness: Option<f64>,
}

/// A candidate's result across its seeds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateResult {
    pub candidate: u64,
    pub generation: u32,
    pub origin: String,
    pub params: serde_json::Map<String, serde_json::Value>,
    pub values: Vec<f64>,
    pub completed: usize,
    pub invalid: usize,
    pub failed: usize,
    /// First refusal reason, so a rejected region of the box is legible in the summary.
    pub reason: Option<String>,
    /// Mean over completed seeds. `None` when any seed did not complete.
    pub fitness: Option<f64>,
    /// Worst seed, so a candidate that wins on average and dies on one seed is visible.
    pub worst_seed_fitness: Option<f64>,
    pub objectives: Option<Objectives>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchReport {
    pub build_id: String,
    pub protocol: Protocol,
    pub budget: Budget,
    pub variation: Variation,
    pub scoring: Scoring,
    pub search_seed: u64,
    pub seed_schedule: Vec<u64>,
    pub stop_reason: StopReason,
    pub generations_run: u32,
    pub evaluations_run: u64,
    pub cached_reuses: u64,
    pub wall_seconds: f64,
    pub ticks_simulated: u64,
    pub ticks_per_second: f64,
    pub candidates: Vec<CandidateResult>,
    /// Indices into `candidates` that are nondominated on [`Objectives`].
    pub nondominated: Vec<usize>,
    pub best_by_scalar: Option<usize>,
}

/// The full search. `sink` receives every row as it is produced, in a deterministic order
/// (generation, then candidate, then seed), regardless of which worker finished first.
pub fn run(
    protocol: Protocol,
    budget: Budget,
    variation: Variation,
    scoring: Scoring,
    search_seed: u64,
    mut sink: impl FnMut(&Row),
) -> Result<SearchReport, String> {
    protocol.validate()?;
    budget.validate()?;
    variation.validate()?;
    if scoring.component_floor <= 0.0 || scoring.component_floor > 0.1 {
        return Err(format!(
            "scoring.component_floor must be in (0, 0.1], got {}",
            scoring.component_floor
        ));
    }

    let start = Instant::now();
    let deadline = start + Duration::from_secs(budget.wall_seconds);
    let seeds = budget.seed_schedule().to_vec();
    let spent = AtomicU64::new(0);
    let out_of_time = AtomicBool::new(false);
    let mut cache: BTreeMap<(u64, u64), Evaluation> = BTreeMap::new();
    let mut cached_reuses = 0u64;
    let mut rows_written = 0u64;
    let mut ticks_simulated = 0u64;

    let mut next_id = 0u64;
    let mut generation_values = initial_population(search_seed, budget.population);
    let mut generation_origin: Vec<&'static str> = (0..budget.population)
        .map(|i| if i == 0 { "default" } else { "random" })
        .collect();
    let mut all: Vec<CandidateResult> = Vec::new();
    let mut stop = StopReason::Generations;
    let mut generations_run = 0;

    'generations: for generation in 0..budget.generations {
        let ids: Vec<u64> = (0..generation_values.len())
            .map(|i| next_id + i as u64)
            .collect();
        next_id += generation_values.len() as u64;

        // Work out which simulations this generation actually needs.
        let mut jobs: Vec<(usize, usize)> = Vec::new();
        for (c, values) in generation_values.iter().enumerate() {
            let fp = params::fingerprint(values);
            for (s, seed) in seeds.iter().enumerate() {
                if cache.contains_key(&(fp, *seed)) {
                    cached_reuses += 1;
                } else {
                    jobs.push((c, s));
                }
            }
        }

        let done: Mutex<Vec<((usize, usize), Evaluation)>> = Mutex::new(Vec::new());
        let cursor = AtomicU64::new(0);
        std::thread::scope(|scope| {
            for _ in 0..budget.workers.min(jobs.len().max(1)) {
                scope.spawn(|| {
                    loop {
                        let index = cursor.fetch_add(1, Ordering::SeqCst) as usize;
                        if index >= jobs.len() {
                            return;
                        }
                        if Instant::now() >= deadline {
                            out_of_time.store(true, Ordering::SeqCst);
                            return;
                        }
                        // Claim an evaluation slot, or stop: the cap is on simulations
                        // started, so it can never be exceeded by a race.
                        if spent
                            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                                (n < budget.max_evaluations).then_some(n + 1)
                            })
                            .is_err()
                        {
                            return;
                        }
                        let (c, s) = jobs[index];
                        let evaluation = evaluate(&generation_values[c], seeds[s], protocol);
                        done.lock().expect("result mutex").push(((c, s), evaluation));
                    }
                });
            }
        });

        for (key, evaluation) in done.into_inner().expect("result mutex") {
            let fp = params::fingerprint(&generation_values[key.0]);
            ticks_simulated += evaluation.metrics.as_ref().map_or(0, |m| m.ticks_run);
            cache.insert((fp, seeds[key.1]), evaluation);
        }

        // Collate in a fixed order, so the output does not depend on worker scheduling.
        let mut scored_this_generation = 0usize;
        for (c, values) in generation_values.iter().enumerate() {
            let fp = params::fingerprint(values);
            let labelled = params::labelled(values);
            let mut per_seed = Vec::new();
            for seed in &seeds {
                // A missing entry means the budget or the clock stopped first; the candidate
                // is then reported as unscored rather than scored on a partial seed set.
                if let Some(e) = cache.get(&(fp, *seed)) {
                    per_seed.push(e.clone());
                }
            }
            let completed = per_seed.iter().filter(|e| e.status == Status::Completed).count();
            let invalid = per_seed.iter().filter(|e| e.status == Status::Invalid).count();
            let failed = per_seed.iter().filter(|e| e.status == Status::Failed).count();
            let fully_scored = per_seed.len() == seeds.len() && completed == seeds.len();

            let per_seed_fitness: Vec<f64> = per_seed
                .iter()
                .filter_map(|e| e.metrics.as_ref())
                .map(|m| m.fitness(&scoring))
                .collect();
            let per_seed_objectives: Vec<Objectives> = per_seed
                .iter()
                .filter_map(|e| e.metrics.as_ref())
                .map(|m| m.objectives(&scoring))
                .collect();

            for (s, evaluation) in per_seed.iter().enumerate() {
                if rows_written >= budget.max_rows {
                    stop = StopReason::RowLimit;
                    break;
                }
                let row = Row {
                    candidate: ids[c],
                    generation,
                    origin: generation_origin[c].to_string(),
                    params: labelled.clone(),
                    param_bits: params::bit_labels(values),
                    param_fingerprint: fp,
                    evaluation: evaluation.clone(),
                    objectives: evaluation.metrics.as_ref().map(|m| m.objectives(&scoring)),
                    fitness: evaluation.metrics.as_ref().map(|m| m.fitness(&scoring)),
                };
                sink(&row);
                rows_written += 1;
                let _ = s;
            }

            let fitness = fully_scored
                .then(|| per_seed_fitness.iter().sum::<f64>() / per_seed_fitness.len() as f64);
            if fitness.is_some() {
                scored_this_generation += 1;
            }
            all.push(CandidateResult {
                candidate: ids[c],
                generation,
                origin: generation_origin[c].to_string(),
                params: labelled,
                values: values.clone(),
                completed,
                invalid,
                failed,
                reason: per_seed.iter().find_map(|e| e.reason.clone()),
                fitness,
                worst_seed_fitness: fully_scored
                    .then(|| per_seed_fitness.iter().copied().fold(f64::INFINITY, f64::min)),
                objectives: fully_scored.then(|| mean_objectives(&per_seed_objectives)),
            });
        }

        generations_run = generation + 1;
        if stop == StopReason::RowLimit {
            break 'generations;
        }
        if out_of_time.load(Ordering::SeqCst) || Instant::now() >= deadline {
            stop = StopReason::WallTime;
            break 'generations;
        }
        if spent.load(Ordering::SeqCst) >= budget.max_evaluations {
            stop = StopReason::Evaluations;
            break 'generations;
        }
        if generation + 1 == budget.generations {
            stop = StopReason::Generations;
            break 'generations;
        }
        if scored_this_generation == 0 && all.iter().all(|c| c.fitness.is_none()) {
            stop = StopReason::NoViableParents;
            break 'generations;
        }

        let (values, origin) = next_generation(&all, &budget, &variation, search_seed, generation + 1);
        generation_values = values;
        generation_origin = origin;
    }

    let nondominated = nondominated(&all);
    let best_by_scalar = all
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.fitness.map(|f| (i, f)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i);
    let wall = start.elapsed().as_secs_f64();

    Ok(SearchReport {
        build_id: crate::evaluate::BUILD_ID.to_string(),
        protocol,
        budget,
        variation,
        scoring,
        search_seed,
        seed_schedule: seeds,
        stop_reason: stop,
        generations_run,
        evaluations_run: spent.load(Ordering::SeqCst),
        cached_reuses,
        wall_seconds: wall,
        ticks_simulated,
        ticks_per_second: if wall > 0.0 { ticks_simulated as f64 / wall } else { 0.0 },
        candidates: all,
        nondominated,
        best_by_scalar,
    })
}

fn mean_objectives(list: &[Objectives]) -> Objectives {
    let n = list.len().max(1) as f64;
    let mut sum = [0.0f64; 7];
    for o in list {
        for (acc, v) in sum.iter_mut().zip(o.as_array()) {
            *acc += v;
        }
    }
    Objectives {
        persistence: sum[0] / n,
        plants: sum[1] / n,
        prey_turnover: sum[2] / n,
        maturation: sum[3] / n,
        lineage: sum[4] / n,
        variety: sum[5] / n,
        apex: sum[6] / n,
    }
}

/// Candidate 0 is the shipped default vector, so every search reports what it moved away from.
/// The rest are drawn uniformly from the declared box by the search's own keyed RNG.
pub fn initial_population(search_seed: u64, size: usize) -> Vec<Vec<f64>> {
    let bounds = params::bounds();
    (0..size)
        .map(|i| {
            if i == 0 {
                return params::defaults();
            }
            let mut v: Vec<f64> = bounds
                .iter()
                .enumerate()
                .map(|(k, (lo, hi))| {
                    lo + rng::unit(search_seed, rng::stream::INIT, i as u64, k as u64) * (hi - lo)
                })
                .collect();
            params::clamp(&mut v);
            v
        })
        .collect()
}

/// Elites unchanged, then offspring from binary tournaments with uniform crossover and a
/// bounded Gaussian step. Deterministic in `(search_seed, generation, slot, gene)`, so the
/// generation does not depend on which worker finished first.
fn next_generation(
    all: &[CandidateResult],
    budget: &Budget,
    variation: &Variation,
    search_seed: u64,
    generation: u32,
) -> (Vec<Vec<f64>>, Vec<&'static str>) {
    let mut pool: Vec<&CandidateResult> = all.iter().filter(|c| c.fitness.is_some()).collect();
    // Best first; ties broken by candidate id so the order never depends on timing.
    pool.sort_by(|a, b| {
        b.fitness
            .unwrap()
            .total_cmp(&a.fitness.unwrap())
            .then(a.candidate.cmp(&b.candidate))
    });

    let mut values = Vec::with_capacity(budget.population);
    let mut origin = Vec::with_capacity(budget.population);
    for elite in pool.iter().take(budget.elite) {
        values.push(elite.values.clone());
        origin.push("elite");
    }
    let g = u64::from(generation);
    while values.len() < budget.population {
        let slot = values.len() as u64;
        let mut child = if pool.len() >= 2 {
            let a = tournament(&pool, search_seed, g, slot, 0);
            let b = tournament(&pool, search_seed, g, slot, 1);
            let mut child = a.values.clone();
            for (k, gene) in child.iter_mut().enumerate() {
                let r = rng::unit(search_seed, rng::stream::CROSSOVER, g * 1000 + slot, k as u64);
                if r < variation.crossover_rate {
                    *gene = b.values[k];
                }
            }
            origin.push("offspring");
            child
        } else if let Some(only) = pool.first() {
            origin.push("mutant");
            only.values.clone()
        } else {
            // Nothing scored yet: draw fresh, rather than mutate an unscored vector.
            origin.push("random");
            params::bounds()
                .iter()
                .enumerate()
                .map(|(k, (lo, hi))| {
                    lo + rng::unit(search_seed, rng::stream::INIT, g * 1000 + slot, k as u64)
                        * (hi - lo)
                })
                .collect()
        };
        for (k, (lo, hi)) in params::bounds().iter().enumerate() {
            let key = g * 1000 + slot;
            if rng::unit(search_seed, rng::stream::MUTATION, key, k as u64)
                < variation.mutation_rate
            {
                let step = rng::normal(search_seed, rng::stream::MUTATION, key, 1_000 + k as u64);
                child[k] += step * variation.mutation_sigma * (hi - lo);
            }
        }
        params::clamp(&mut child);
        values.push(child);
    }
    (values, origin)
}

fn tournament<'a>(
    pool: &[&'a CandidateResult],
    search_seed: u64,
    generation: u64,
    slot: u64,
    which: u64,
) -> &'a CandidateResult {
    let key = generation * 1000 + slot;
    let pick = |c: u64| {
        let r = rng::unit(search_seed, rng::stream::TOURNAMENT, key, which * 10 + c);
        pool[((r * pool.len() as f64) as usize).min(pool.len() - 1)]
    };
    let (a, b) = (pick(0), pick(1));
    // `pool` is sorted best first, so the lower index is the better candidate.
    if a.fitness.unwrap() >= b.fitness.unwrap() { a } else { b }
}

/// Indices of the candidates no other candidate dominates on all seven objectives.
pub fn nondominated(all: &[CandidateResult]) -> Vec<usize> {
    let scored: Vec<(usize, Objectives)> = all
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.objectives.map(|o| (i, o)))
        .collect();
    scored
        .iter()
        .filter(|(_, o)| !scored.iter().any(|(_, other)| other.dominates(o)))
        .map(|(i, _)| *i)
        .collect()
}
