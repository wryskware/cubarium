//! The generation loop: stable job identities, bounded workers, a scheduling-independent
//! reduction, and the checkpoint that makes a resumed run identical to an uninterrupted one.
//!
//! # Job identity
//!
//! Every episode in a generation is named by `(generation, pair, sign, layout)` and placed at a
//! fixed index computed from that name:
//!
//! ```text
//! candidate = 2·pair + sign      (sign 0 = +sigma·epsilon, 1 = -sigma·epsilon; the centre,
//!                                 when it is evaluated, is candidate 2n)
//! job       = candidate · layouts + layout
//! ```
//!
//! Results are written into a pre-sized slot vector at that index, so the order workers happen
//! to finish in cannot reach the scores, the ranks, the gradient or the centre. The
//! serial-versus-parallel test in this module is the check on that claim, not the comment.
//!
//! # What both signs see
//!
//! One `epsilon_i` per pair, and the *same* list of layouts. A layout's world seed is its own
//! frozen constant, so `theta + sigma·epsilon_i` and `theta - sigma·epsilon_i` are scored on
//! bit-identical initial worlds. Nothing in an episode consumes a training draw, and nothing
//! in the trainer consumes a world draw.
//!
//! # Cancellation
//!
//! One shared `AtomicBool`, read inside each episode every
//! [`super::episode::CANCEL_CHECK_TICKS`] ticks. A generation that does not complete every job
//! **does not update the centre** and writes no checkpoint: a partial generation is a stopped
//! run, not a cheaper one.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_core::neural::Policy;
use serde::{Deserialize, Serialize};

use super::episode::{self, Driver, Episode, EpisodeError, Fault, Limits};
use super::fixture::{HORIZON_TICKS, Layout, fnv1a};
use super::optimizer::{ADAM_EPS, Adam, BETA1, BETA2, LEARNING_RATE, SIGMA, gradient};
use super::rng::perturbation;
use super::tensor::{self, PARAMS};

/// Weight of the terminal-stores tiebreak in the scalar score (brief §4).
///
/// `score = t_min + 0.25 · mean normalised usable terminal stores`. The normalised mean is in
/// `[0, 1]`, so the whole secondary term is at most 0.25 of a tick: **no store bonus can ever
/// outweigh one tick of survival**. Frozen before the smoke.
pub const STORE_WEIGHT: f64 = 0.25;

/// How a candidate's per-layout survival ticks combine into one scalar.
///
/// `Min` is the R2a convention (frozen: `t_min`). `Mean` is the R2c screen's alternative,
/// where a policy that solves three layouts and dies early on the fourth gets credit for the
/// three. Only `Mean` is serialized, so every `Min` protocol keeps the hash it already had.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Aggregate {
    Min,
    Mean,
}

impl Aggregate {
    pub fn is_min(&self) -> bool {
        *self == Aggregate::Min
    }

    pub fn parse(s: &str) -> Result<Aggregate, String> {
        match s {
            "min" => Ok(Aggregate::Min),
            "mean" => Ok(Aggregate::Mean),
            other => Err(format!("aggregate must be `min` or `mean`, not `{other}`")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Aggregate::Min => "min",
            Aggregate::Mean => "mean",
        }
    }
}

impl Default for Aggregate {
    fn default() -> Self {
        Aggregate::Min
    }
}

/// The frozen protocol: everything a score depends on that is not the policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Protocol {
    pub schema: String,
    pub horizon_ticks: u64,
    pub pairs: usize,
    pub sigma: f64,
    pub learning_rate: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub adam_eps: f64,
    pub store_weight: f64,
    pub train_seed: u64,
    pub layouts: Vec<String>,
    pub layout_hashes: Vec<u64>,
    pub policy_digest: u64,
    pub init: String,
    /// Absent from the JSON (and so from the hash) when it is the R2a `min`.
    #[serde(default, skip_serializing_if = "Aggregate::is_min")]
    pub aggregate: Aggregate,
}

impl Protocol {
    /// The same protocol with a different survival aggregate: a different task, a different
    /// hash.
    pub fn with_aggregate(mut self, aggregate: Aggregate) -> Protocol {
        self.aggregate = aggregate;
        self
    }

    pub fn new(pairs: usize, horizon_ticks: u64, train_seed: u64, layouts: &[Layout]) -> Protocol {
        Protocol {
            schema: "cub-es-1".into(),
            horizon_ticks,
            pairs,
            sigma: SIGMA,
            learning_rate: LEARNING_RATE,
            beta1: BETA1,
            beta2: BETA2,
            adam_eps: ADAM_EPS,
            store_weight: STORE_WEIGHT,
            train_seed,
            layouts: layouts.iter().map(|l| l.name.clone()).collect(),
            layout_hashes: layouts.iter().map(|l| l.hash(&l.config())).collect(),
            policy_digest: cubarium_core::neural::schema_digest(),
            init: tensor::init_description(),
            aggregate: Aggregate::Min,
        }
    }

    /// FNV-1a over the protocol's canonical JSON. Stored in every checkpoint and every result.
    pub fn hash(&self) -> u64 {
        fnv1a(serde_json::to_string(self).expect("the protocol serializes").as_bytes())
    }
}

impl Default for Protocol {
    fn default() -> Self {
        Protocol::new(16, HORIZON_TICKS, 20_260_915, &super::fixture::training_layouts())
    }
}

/// `score = t_min + 0.25 · mean normalised usable terminal stores`.
///
/// `t_min` is the **minimum** survival ticks over the candidate's layouts, so a policy that
/// solves three layouts and dies immediately on the fourth is ordered by the fourth. The
/// secondary term is a mean over the same layouts, with a dead animal contributing zero.
pub fn score(episodes: &[Episode]) -> f64 {
    score_by(Aggregate::Min, episodes)
}

/// [`score`] under a chosen survival aggregate: `t_min` for [`Aggregate::Min`], the mean
/// survival ticks over the layouts for [`Aggregate::Mean`]. The stores tiebreak is the same.
pub fn score_by(aggregate: Aggregate, episodes: &[Episode]) -> f64 {
    assert!(!episodes.is_empty(), "a candidate is scored on at least one layout");
    let survival = match aggregate {
        Aggregate::Min => episodes.iter().map(|e| e.ticks).min().expect("nonempty") as f64,
        Aggregate::Mean => {
            episodes.iter().map(|e| e.ticks as f64).sum::<f64>() / episodes.len() as f64
        }
    };
    let stores: f64 = episodes.iter().map(Episode::normalized_stores).sum::<f64>()
        / episodes.len() as f64;
    survival + STORE_WEIGHT * stores
}

/// Which candidate a job belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Candidate {
    /// `theta + sigma·epsilon_pair`.
    Plus(usize),
    /// `theta - sigma·epsilon_pair`.
    Minus(usize),
    /// The unperturbed centre. A sampled perturbation's score is not the centre's, so this is
    /// measured, never inferred.
    Center,
}

impl Candidate {
    /// The stable candidate index: `2·pair + sign`, with the centre after every pair.
    pub fn index(self, pairs: usize) -> usize {
        match self {
            Candidate::Plus(p) => 2 * p,
            Candidate::Minus(p) => 2 * p + 1,
            Candidate::Center => 2 * pairs,
        }
    }

    pub fn label(self) -> String {
        match self {
            Candidate::Plus(p) => format!("pair{p}+"),
            Candidate::Minus(p) => format!("pair{p}-"),
            Candidate::Center => "center".into(),
        }
    }
}

/// One named episode: the stable `(generation, pair, sign, layout)` identity and its result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub generation: u64,
    pub candidate: String,
    pub layout: String,
    pub episode: Episode,
}

/// What one generation produced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenerationReport {
    pub generation: u64,
    /// `2n` perturbation scores, in candidate index order.
    pub candidate_scores: Vec<f64>,
    /// The unperturbed centre's score, when it was evaluated this generation.
    pub center_score: Option<f64>,
    pub jobs: Vec<Job>,
    pub gradient_norm: f64,
    pub update_rms: f64,
    pub episodes_run: u64,
    pub ticks_run: u64,
    pub wall_seconds: f64,
}

/// The persisted optimizer state. Everything needed to continue a run exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub schema: String,
    pub build: String,
    pub protocol: Protocol,
    pub protocol_hash: u64,
    /// The exact central weights, in [`super::tensor`]'s order, stored as little-endian
    /// IEEE-754 hex so a reload is bit-identical (see [`super::bits`]).
    #[serde(with = "crate::es::bits::hex_f64s")]
    pub theta: Vec<f64>,
    pub adam: Adam,
    /// The training seed. Perturbations are *positional* — `epsilon` for
    /// `(generation, pair, element)` is a pure function of `(train_seed, generation, pair,
    /// element)` — so the seed together with `generation_completed` **is** the perturbation
    /// state. There is no hidden stream cursor to lose.
    pub train_seed: u64,
    pub generation_completed: u64,
    pub episodes_run: u64,
    pub ticks_run: u64,
    /// Work performed by generations that were discarded (cancelled or failed). Counted
    /// against the budget, and deliberately **not** counted as optimizer progress.
    pub discarded: Discarded,
    /// One record per centre this run has held, oldest first: generation `g` is the centre
    /// *before* update `g` is applied, and the last record is the centre the run ended on.
    ///
    /// Each record names the file holding that centre's exact weights, so the evaluation
    /// assignment can select **any** centre the run passed through, not only the latest. The
    /// checkpoint's own `theta` is always the last record's weights.
    pub centers: Vec<CenterRecord>,
}

/// One centre in the run's history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CenterRecord {
    pub generation: u64,
    /// Its score, when it was evaluated. `None` means it was never evaluated — never 0.
    pub score: Option<f64>,
    /// Path, relative to the run directory, of the exported weights for this centre.
    pub file: String,
    /// FNV-1a over the weights' little-endian hex, so a file can be checked against the record.
    pub weights_fnv1a: u64,
}

impl Checkpoint {
    pub fn fresh(protocol: Protocol, train_seed: u64, build: &str) -> Checkpoint {
        let protocol_hash = protocol.hash();
        Checkpoint {
            schema: "cub-es-checkpoint-1".into(),
            build: build.to_string(),
            protocol,
            protocol_hash,
            theta: tensor::initial_center(train_seed),
            adam: Adam::new(PARAMS),
            train_seed,
            generation_completed: 0,
            episodes_run: 0,
            ticks_run: 0,
            discarded: Discarded::default(),
            centers: Vec::new(),
        }
    }

    /// The generations whose centre this run recorded, oldest first.
    pub fn center_generations(&self) -> Vec<u64> {
        self.centers.iter().map(|c| c.generation).collect()
    }

    /// An already-recorded score for this generation's centre, if there is one. Resuming must
    /// reuse it rather than paying for the same evaluation twice.
    pub fn recorded_center_score(&self, generation: u64) -> Option<f64> {
        self.centers
            .iter()
            .find(|c| c.generation == generation)
            .and_then(|c| c.score)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.theta.len() != PARAMS {
            return Err(format!("checkpoint theta has {} values, expected {PARAMS}", self.theta.len()));
        }
        if self.adam.m.len() != PARAMS || self.adam.v.len() != PARAMS {
            return Err("checkpoint Adam moments have the wrong dimension".into());
        }
        if !self.theta.iter().all(|x| x.is_finite()) || !self.adam.is_finite() {
            return Err("checkpoint holds a non-finite value".into());
        }
        if self.protocol.hash() != self.protocol_hash {
            return Err("checkpoint protocol hash does not match its protocol".into());
        }
        if self.protocol.policy_digest != cubarium_core::neural::schema_digest() {
            return Err(format!(
                "checkpoint was trained against schema digest {:#018x}, this build is {:#018x}: \
                 the observation layout, action set, recurrence convention, motor contract or \
                 controller rate differs and the weights cannot be reinterpreted",
                self.protocol.policy_digest,
                cubarium_core::neural::schema_digest()
            ));
        }
        tensor::policy(&self.theta).map(|_| ())
    }

    /// The self-contained policy the core can attach.
    pub fn policy(&self) -> Result<Policy, String> {
        tensor::policy(&self.theta)
    }
}

/// Everything one generation needs that is not the centre.
///
/// `deadline` is carried into every episode through [`Limits`], not consulted only when a job
/// is dequeued: a cap that a running rollout cannot see is not a cap.
pub struct Plan<'a> {
    pub layouts: &'a [Layout],
    pub horizon: u64,
    pub workers: usize,
    pub evaluate_center: bool,
    pub deadline: Option<Instant>,
    /// Test-only: corrupt a running world, so an error path can be exercised through the code
    /// that will actually run it rather than through a copy of it. `None` everywhere else.
    pub fault: Option<Fault<'a>>,
}

impl<'a> Plan<'a> {
    /// The ordinary plan: no fault.
    pub fn new(
        layouts: &'a [Layout],
        horizon: u64,
        workers: usize,
        evaluate_center: bool,
        deadline: Option<Instant>,
    ) -> Plan<'a> {
        Plan { layouts, horizon, workers, evaluate_center, deadline, fault: None }
    }
}

/// Work a discarded generation actually performed.
///
/// A cancelled generation updates nothing, but it did occupy workers and simulate ticks, and
/// the brief's budgets are budgets on *work*. So the counts survive the cancellation and are
/// carried separately from the optimizer's own progress, which stays where it was.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Discarded {
    pub episodes_attempted: u64,
    pub episodes_completed: u64,
    pub ticks_run: u64,
}

impl Discarded {
    pub fn add(&mut self, other: Discarded) {
        self.episodes_attempted += other.episodes_attempted;
        self.episodes_completed += other.episodes_completed;
        self.ticks_run += other.ticks_run;
    }
}

/// Why a generation produced no update.
#[derive(Clone, Debug, PartialEq)]
pub enum GenerationError {
    /// The deadline or the cancellation flag stopped it. The centre and Adam are untouched;
    /// the work it did is reported so the budget can account for it.
    Cancelled(Discarded),
    /// A rollout found an invalid world. This is an experiment error, not a low score.
    Invalid { job: String, detail: String, discarded: Discarded },
}

impl std::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenerationError::Cancelled(d) => write!(
                f,
                "generation cancelled after {} of {} episodes ({} ticks)",
                d.episodes_completed, d.episodes_attempted, d.ticks_run
            ),
            GenerationError::Invalid { job, detail, .. } => {
                write!(f, "invalid world in job {job}: {detail}")
            }
        }
    }
}

impl std::error::Error for GenerationError {}

impl GenerationError {
    pub fn discarded(&self) -> Discarded {
        match self {
            GenerationError::Cancelled(d) | GenerationError::Invalid { discarded: d, .. } => *d,
        }
    }
}

/// Evaluate one parameter vector on every layout and return its score with its episodes.
///
/// Used for the unperturbed centre, whose score is measured rather than inferred: a sampled
/// perturbation's score is not the centre's. The reduction is the fixed layout order, so the
/// worker count cannot reach it.
pub fn evaluate(
    theta: &[f64],
    protocol: &Protocol,
    plan: &Plan<'_>,
    generation: u64,
    cancel: &AtomicBool,
) -> Result<(f64, Vec<Episode>), GenerationError> {
    let policy = tensor::policy(theta).expect("a finite centre is a policy");
    let names: Vec<String> = plan
        .layouts
        .iter()
        .map(|l| format!("gen{generation}/center/{}", l.name))
        .collect();
    let episodes = dispatch(plan, plan.layouts.len(), cancel, |i| {
        (Driver::Policy(Box::new(policy.clone())), i, names[i].clone())
    })?;
    Ok((score_by(protocol.aggregate, &episodes), episodes))
}

/// The one worker pool every batch in this module uses.
///
/// `job(i)` names the driver, the layout index and the job identity for slot `i`. Results land
/// at their own index, so the order workers finish in cannot reach the reduction. Every episode
/// carries the plan's deadline, so a cap is enforced inside the rollout.
fn dispatch<F>(
    plan: &Plan<'_>,
    jobs_total: usize,
    cancel: &AtomicBool,
    job: F,
) -> Result<Vec<Episode>, GenerationError>
where
    F: Fn(usize) -> (Driver, usize, String) + Sync,
{
    let slots: Mutex<Vec<Option<Episode>>> = Mutex::new(vec![None; jobs_total]);
    let cursor = AtomicUsize::new(0);
    let attempted = AtomicU64::new(0);
    let completed = AtomicU64::new(0);
    let ticks = AtomicU64::new(0);
    let failure: Mutex<Option<(String, String)>> = Mutex::new(None);
    let limits = Limits { cancel, deadline: plan.deadline };
    std::thread::scope(|scope| {
        for _ in 0..plan.workers.max(1).min(jobs_total.max(1)) {
            scope.spawn(|| {
                loop {
                    let index = cursor.fetch_add(1, Ordering::SeqCst);
                    if index >= jobs_total {
                        return;
                    }
                    if limits.expired() {
                        return;
                    }
                    let (driver, layout, name) = job(index);
                    attempted.fetch_add(1, Ordering::SeqCst);
                    match episode::run_with_fault(
                        &plan.layouts[layout],
                        &driver,
                        plan.horizon,
                        limits,
                        &name,
                        plan.fault,
                    ) {
                        Ok(e) => {
                            completed.fetch_add(1, Ordering::SeqCst);
                            ticks.fetch_add(e.ticks, Ordering::SeqCst);
                            slots.lock().expect("slots")[index] = Some(e);
                        }
                        Err(EpisodeError::Cancelled { ticks: t }) => {
                            ticks.fetch_add(t, Ordering::SeqCst);
                            return;
                        }
                        Err(EpisodeError::Invalid { ticks: t, detail }) => {
                            ticks.fetch_add(t, Ordering::SeqCst);
                            // Stop every other worker: an invalid world ends the experiment,
                            // and continuing would only burn budget on a broken run.
                            cancel.store(true, Ordering::SeqCst);
                            let mut slot = failure.lock().expect("failure");
                            if slot.is_none() {
                                *slot = Some((name, detail));
                            }
                            return;
                        }
                    }
                }
            });
        }
    });
    let discarded = Discarded {
        episodes_attempted: attempted.load(Ordering::SeqCst),
        episodes_completed: completed.load(Ordering::SeqCst),
        ticks_run: ticks.load(Ordering::SeqCst),
    };
    if let Some((job, detail)) = failure.into_inner().expect("failure") {
        return Err(GenerationError::Invalid { job, detail, discarded });
    }
    let slots = slots.into_inner().expect("slots");
    if slots.iter().any(Option::is_none) {
        return Err(GenerationError::Cancelled(discarded));
    }
    Ok(slots.into_iter().map(|s| s.expect("checked")).collect())
}

/// Run one generation and, if every job completed, apply the Adam ascent to `theta`.
///
/// Returns `Err(Cancelled)` without touching `theta` or `adam` when the run was cancelled or
/// the deadline passed.
pub fn run_generation(
    theta: &mut [f64],
    adam: &mut Adam,
    protocol: &Protocol,
    generation: u64,
    plan: &Plan<'_>,
    cancel: &AtomicBool,
) -> Result<GenerationReport, GenerationError> {
    let started = Instant::now();
    let n = protocol.pairs;
    let layouts = plan.layouts;
    assert!(n > 0 && !layouts.is_empty(), "a generation needs pairs and layouts");

    // 1. Build every candidate's policy up front, in candidate index order, on this thread.
    //    Both signs of a pair use one `epsilon`, regenerated from its position rather than
    //    stored twice.
    let candidates: Vec<Candidate> = (0..n)
        .flat_map(|p| [Candidate::Plus(p), Candidate::Minus(p)])
        .chain(plan.evaluate_center.then_some(Candidate::Center))
        .collect();
    let mut eps = vec![0.0; PARAMS];
    let mut policies: Vec<Policy> = Vec::with_capacity(candidates.len());
    for c in &candidates {
        let theta_c = match c {
            Candidate::Center => theta.to_vec(),
            Candidate::Plus(p) | Candidate::Minus(p) => {
                perturbation(protocol.train_seed, generation, *p as u64, &mut eps);
                let sign = if matches!(c, Candidate::Plus(_)) { 1.0 } else { -1.0 };
                (0..PARAMS).map(|j| theta[j] + sign * protocol.sigma * eps[j]).collect()
            }
        };
        policies.push(
            tensor::policy(&theta_c)
                .expect("a perturbed centre is a finite policy; a non-finite one is an error"),
        );
    }

    // 2. One job per (candidate, layout), at its stable index.
    let jobs_total = candidates.len() * layouts.len();
    let names: Vec<String> = (0..jobs_total)
        .map(|i| {
            format!(
                "gen{generation}/{}/{}",
                candidates[i / layouts.len()].label(),
                layouts[i % layouts.len()].name
            )
        })
        .collect();
    let episodes = dispatch(plan, jobs_total, cancel, |i| {
        (
            Driver::Policy(Box::new(policies[i / layouts.len()].clone())),
            i % layouts.len(),
            names[i].clone(),
        )
    })?;

    // 3. The update is committed only if the run is still inside its limits. A generation that
    //    finished its last episode *after* the deadline has not earned an update.
    let limits = Limits { cancel, deadline: plan.deadline };
    if limits.expired() {
        return Err(GenerationError::Cancelled(Discarded {
            episodes_attempted: jobs_total as u64,
            episodes_completed: jobs_total as u64,
            ticks_run: episodes.iter().map(|e| e.ticks).sum(),
        }));
    }

    // 4. Reduce in the fixed index order. Scheduling cannot reach this.
    let mut jobs = Vec::with_capacity(jobs_total);
    let mut scores = vec![0.0; candidates.len()];
    for (ci, c) in candidates.iter().enumerate() {
        let slice = &episodes[ci * layouts.len()..(ci + 1) * layouts.len()];
        scores[ci] = score_by(protocol.aggregate, slice);
        for (li, e) in slice.iter().enumerate() {
            jobs.push(Job {
                generation,
                candidate: c.label(),
                layout: layouts[li].name.clone(),
                episode: e.clone(),
            });
        }
        debug_assert_eq!(ci, c.index(n).min(2 * n));
    }

    let plus: Vec<f64> = (0..n).map(|p| scores[2 * p]).collect();
    let minus: Vec<f64> = (0..n).map(|p| scores[2 * p + 1]).collect();
    let center_score = plan.evaluate_center.then(|| scores[2 * n]);

    let g = gradient(&plus, &minus, PARAMS, protocol.sigma, |i, out| {
        perturbation(protocol.train_seed, generation, i as u64, out);
    });
    assert!(g.iter().all(|x| x.is_finite()), "a non-finite gradient is an experiment error");
    let gradient_norm = g.iter().map(|x| x * x).sum::<f64>().sqrt();
    let update_rms = adam.ascend(theta, &g);
    assert!(theta.iter().all(|x| x.is_finite()), "the updated centre must stay finite");

    let ticks_run: u64 = episodes.iter().map(|e| e.ticks).sum();
    Ok(GenerationReport {
        generation,
        candidate_scores: scores[..2 * n].to_vec(),
        center_score,
        jobs,
        gradient_norm,
        update_rms,
        episodes_run: jobs_total as u64,
        ticks_run,
        wall_seconds: started.elapsed().as_secs_f64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::fixture::training_layouts;

    fn smoke_protocol() -> Protocol {
        Protocol::new(2, 2_000, 20_260_915, &training_layouts()[..1])
    }

    #[test]
    fn the_mean_aggregate_is_a_different_task_and_the_min_hash_is_untouched() {
        let base = Episode {
            layout: "x".into(),
            ticks: 10,
            alive: false,
            terminal_stores: 0.0,
            store_capacity: 1.0,
            intake_producer: 0.0,
            intake_fruit: 0.0,
            intake_detritus: 0.0,
            upkeep_billed: 0.0,
            motion_billed: 0.0,
            store_start: 0.0,
            travelled_px: 0.0,
            body_lengths: 0.0,
            distinct_cells: 0,
            ticks_in_opening: 0,
            turn_sweep_rad: 0.0,
            seam_crossing_ticks: 0,
            turn_unmeasured_ticks: 0,
            motion_billed_partial: false,
            died_on_last_tick: false,
            route_p_start: 0.0,
            route_p_end: 0.0,
            route_p_grown: 0.0,
            validations: 0,
        };
        let long = Episode { ticks: 30, ..base.clone() };
        let both = [base.clone(), long.clone()];
        assert!((score_by(Aggregate::Min, &both) - 10.0).abs() < 1e-12);
        assert!((score_by(Aggregate::Mean, &both) - 20.0).abs() < 1e-12);
        assert!((score(&both) - score_by(Aggregate::Min, &both)).abs() < 1e-12);
        let p = Protocol::default();
        let json = serde_json::to_string(&p).expect("serializes");
        assert!(!json.contains("aggregate"), "min must not appear in the hashed JSON: {json}");
        let mean = p.clone().with_aggregate(Aggregate::Mean);
        assert_ne!(p.hash(), mean.hash());
        let back: Protocol = serde_json::from_str(&json).expect("round trip");
        assert_eq!(back.aggregate, Aggregate::Min);
        assert_eq!(Aggregate::parse("mean"), Ok(Aggregate::Mean));
        assert!(Aggregate::parse("max").is_err());
    }

    #[test]
    fn the_score_puts_one_tick_above_every_possible_store_bonus() {
        let base = Episode {
            layout: "x".into(),
            ticks: 100,
            alive: true,
            terminal_stores: 1.0,
            store_capacity: 1.0,
            intake_producer: 0.0,
            intake_fruit: 0.0,
            intake_detritus: 0.0,
            upkeep_billed: 0.0,
            motion_billed: 0.0,
            store_start: 0.0,
            travelled_px: 0.0,
            body_lengths: 0.0,
            distinct_cells: 0,
            ticks_in_opening: 0,
            turn_sweep_rad: 0.0,
            seam_crossing_ticks: 0,
            turn_unmeasured_ticks: 0,
            motion_billed_partial: false,
            died_on_last_tick: false,
            route_p_start: 0.0,
            route_p_end: 0.0,
            route_p_grown: 0.0,
            validations: 0,
        };
        // Full stores on 100 ticks must still lose to 101 ticks with empty stores.
        let full = score(std::slice::from_ref(&base));
        let mut longer = base.clone();
        longer.ticks = 101;
        longer.terminal_stores = 0.0;
        assert!(score(std::slice::from_ref(&longer)) > full);
        assert!((full - 100.25).abs() < 1e-12);

        // `t_min` is the minimum, not the mean.
        let mut short = base.clone();
        short.ticks = 10;
        assert!((score(&[base.clone(), short]) - 10.25).abs() < 1e-12);

        // A dead animal contributes zero stores whatever it is holding.
        let mut dead = base;
        dead.alive = false;
        assert!((score(std::slice::from_ref(&dead)) - 100.0).abs() < 1e-12);
    }

    #[test]
    fn a_generation_reduces_identically_whatever_the_worker_count() {
        let protocol = smoke_protocol();
        let layouts = training_layouts();
        let cancel = AtomicBool::new(false);

        let run = |workers: usize| {
            let mut theta = tensor::initial_center(protocol.train_seed);
            let mut adam = Adam::new(PARAMS);
            let plan = Plan {
                layouts: &layouts[..1],
                horizon: protocol.horizon_ticks,
                workers,
                evaluate_center: false,
                deadline: None,
                fault: None,
            };
            let report = run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel)
                .expect("not cancelled");
            (theta, adam, report)
        };

        let (theta_1, adam_1, report_1) = run(1);
        let (theta_4, adam_4, report_4) = run(4);

        assert_eq!(report_1.candidate_scores, report_4.candidate_scores);
        assert_eq!(report_1.jobs, report_4.jobs);
        assert_eq!(report_1.gradient_norm, report_4.gradient_norm);
        assert_eq!(theta_1, theta_4, "the centre must not depend on the worker count");
        assert_eq!(adam_1, adam_4);
    }

    #[test]
    fn both_signs_of_a_pair_see_the_same_perturbation_and_the_same_layouts() {
        let protocol = smoke_protocol();
        let mut a = vec![0.0; PARAMS];
        let mut b = vec![0.0; PARAMS];
        perturbation(protocol.train_seed, 3, 1, &mut a);
        perturbation(protocol.train_seed, 3, 1, &mut b);
        assert_eq!(a, b, "one epsilon per pair, shared by both signs");

        // And the mirrored candidates are exactly symmetric about the centre.
        let theta = tensor::initial_center(7);
        let plus: Vec<f64> = (0..PARAMS).map(|j| theta[j] + protocol.sigma * a[j]).collect();
        let minus: Vec<f64> = (0..PARAMS).map(|j| theta[j] - protocol.sigma * a[j]).collect();
        for j in 0..PARAMS {
            assert!(((plus[j] + minus[j]) / 2.0 - theta[j]).abs() < 1e-15);
        }

        // A layout's world seed is a frozen constant, so both signs build identical worlds.
        let l = &training_layouts()[0];
        let (w1, _) = l.build().expect("built");
        let (w2, _) = l.build().expect("built");
        assert_eq!(
            cubarium_core::snapshot::state_hash(&w1.state),
            cubarium_core::snapshot::state_hash(&w2.state)
        );
    }

    #[test]
    fn a_cancelled_generation_leaves_the_centre_alone() {
        let protocol = smoke_protocol();
        let layouts = training_layouts();
        let cancel = AtomicBool::new(true);
        let mut theta = tensor::initial_center(protocol.train_seed);
        let before = theta.clone();
        let mut adam = Adam::new(PARAMS);
        let plan = Plan {
            layouts: &layouts[..1],
            horizon: protocol.horizon_ticks,
            workers: 2,
            evaluate_center: false,
            deadline: None,
            fault: None,
        };
        let out = run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel);
        assert!(matches!(out, Err(GenerationError::Cancelled(_))));
        assert_eq!(theta, before);
        assert_eq!(adam.step, 0);
    }

    #[test]
    fn a_checkpoint_round_trips_and_names_its_protocol() {
        let cp = Checkpoint::fresh(smoke_protocol(), 20_260_915, "test");
        cp.validate().expect("fresh checkpoint is valid");
        assert!(cp.centers.is_empty());
        assert_eq!(cp.discarded, Discarded::default());
        let json = serde_json::to_string(&cp).expect("serialize");
        let back: Checkpoint = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, cp);
        back.validate().expect("round-tripped checkpoint is valid");

        // A checkpoint whose protocol was edited after the fact is refused.
        let mut tampered = back.clone();
        tampered.protocol.sigma = 0.05;
        assert!(tampered.validate().is_err());

        // And the exported policy is exactly the stored centre.
        let p = back.policy().expect("policy");
        assert_eq!(tensor::flatten(&p.weights), back.theta);
    }

    #[test]
    fn resuming_at_a_generation_boundary_matches_an_uninterrupted_run() {
        let protocol = Protocol::new(2, 400, 20_260_915, &training_layouts()[..1]);
        let layouts = training_layouts();
        let cancel = AtomicBool::new(false);
        let plan = |workers| Plan {
            layouts: &layouts[..1],
            horizon: 400,
            workers,
            evaluate_center: false,
            deadline: None,
            fault: None,
        };

        let mut cp = Checkpoint::fresh(protocol.clone(), protocol.train_seed, "test");
        for generation in 0..2u64 {
            let r = run_generation(
                &mut cp.theta,
                &mut cp.adam,
                &protocol,
                generation,
                &plan(2),
                &cancel,
            )
            .expect("not cancelled");
            cp.generation_completed = generation + 1;
            cp.episodes_run += r.episodes_run;
            cp.ticks_run += r.ticks_run;
        }
        let uninterrupted = cp.clone();

        // Now the same two generations, with a save and a reload between them.
        let mut cp = Checkpoint::fresh(protocol.clone(), protocol.train_seed, "test");
        let r = run_generation(&mut cp.theta, &mut cp.adam, &protocol, 0, &plan(1), &cancel)
            .expect("not cancelled");
        cp.generation_completed = 1;
        cp.episodes_run += r.episodes_run;
        cp.ticks_run += r.ticks_run;
        let json = serde_json::to_string(&cp).expect("serialize");
        let mut resumed: Checkpoint = serde_json::from_str(&json).expect("deserialize");
        resumed.validate().expect("valid");
        let r = run_generation(
            &mut resumed.theta,
            &mut resumed.adam,
            &protocol,
            resumed.generation_completed,
            &plan(3),
            &cancel,
        )
        .expect("not cancelled");
        resumed.generation_completed += 1;
        resumed.episodes_run += r.episodes_run;
        resumed.ticks_run += r.ticks_run;

        assert_eq!(resumed.theta, uninterrupted.theta, "resume must be exact");
        assert_eq!(resumed.adam, uninterrupted.adam);
        assert_eq!(resumed.episodes_run, uninterrupted.episodes_run);
        assert_eq!(resumed.ticks_run, uninterrupted.ticks_run);
    }
}
