//! The voxel ES: shape-aware antithetic pairs over the two founder schemas, bounded
//! workers, a scheduling-independent reduction, and the run's bounded store.
//!
//! This is the flat trainer's discipline ([`super::trainer`]) over the voxel episode
//! driver, reusing the same optimizer arithmetic ([`super::optimizer`]), the same
//! positional perturbation stream ([`super::rng`]) and the same float-exact persistence
//! ([`super::bits`]) — one neural implementation and one ES arithmetic in the crate, not
//! a second divergent one.
//!
//! # Job identity and reduction
//!
//! Every episode is named `(generation, pair, sign, layout seed)` and lands at the fixed
//! index `candidate · layouts + layout`, so the order workers finish in cannot reach the
//! scores, the gradient or the update. Both signs of a pair share one `epsilon`, drawn as
//! a pure function of `(train_seed, generation, pair)`, and see the **same** layout set —
//! a layout's arena is a pure function of its seed, so the pair runs on bit-identical
//! worlds.
//!
//! # The generation's shape (plan §3)
//!
//! Eight antithetic pairs on the four training layouts — 64 perturbation episodes — plus
//! the unperturbed centre evaluated on the same four (measured, never inferred: a sampled
//! perturbation's score is not the centre's). A candidate's score is the **mean** of its
//! episodes' [`ScoreComponents::score`] over its layouts, a reduction the fixed index
//! order owns. At most 32 updates per archetype.
//!
//! # Cancellation counts its work
//!
//! One shared flag, read inside every episode ([`super::driver`]). A generation that does
//! not complete every job updates nothing, but the episodes it attempted and the ticks it
//! simulated are carried in [`Discarded`] — budget on *work*, not on optimizer progress.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cubarium_voxel_fauna::Founder;
use serde::{Deserialize, Serialize};

use super::super::optimizer::{self, Adam};
use super::super::rng::perturbation;
use super::controller::EpisodeDriver;
use super::driver::{self, Episode, EpisodeError, Limits};
use super::task::{self, Prepared, Stage};
use super::voxel_schema_digest;

/// The checkpoint's schema token.
pub const CHECKPOINT_SCHEMA: &str = "cub-voxel-es-checkpoint-1";

/// Everything one generation's episodes run under that is not the centre. Serialized into
/// every checkpoint; [`VoxelProtocol::hash`] is FNV-1a 64 over its canonical JSON, so a
/// checkpoint trained under one protocol is refused under another.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoxelProtocol {
    pub schema: String,
    /// The founder lineage name (`littershredder` / `frondgrazer`).
    pub founder: String,
    /// The manifest digest every candidate policy was validated against.
    pub digest: u64,
    pub horizon_ticks: u64,
    /// Which arena task the run trained on: `a` (acquire) or `b` (deplete and
    /// reacquire). Part of the hash — they are different tasks.
    pub stage: String,
    /// How full the arenas introduced the founder ([`task::STARTING_STORES_PROTOCOL`]).
    /// Part of the hash: a founder with no headroom is playing a different game.
    pub starting_stores: String,
    /// The Stage-A start-heading convention the arenas were built under
    /// ([`task::START_HEADING_PROTOCOL`]). Part of the hash: aiming the founder at its
    /// food is a different task from placing it with a free heading.
    pub start_heading: String,
    pub pairs: usize,
    pub sigma: f64,
    pub learning_rate: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub adam_eps: f64,
    pub survival_weight: f64,
    pub train_seed: u64,
    /// The training layout seeds, in the frozen order the reduction walks.
    pub layout_seeds: Vec<u64>,
    /// The founder's fixed reference body mass the intake and motor terms divide by.
    pub reference_mass: f64,
    /// The trained controller body: `gru` is the only trainable one today; the heuristic
    /// slot carries no evolvable parameters.
    pub controller: String,
    /// How a candidate's per-layout scores combine: `mean` over its layouts.
    pub aggregate: String,
}

impl VoxelProtocol {
    pub fn new(
        founder: Founder,
        stage: Stage,
        pairs: usize,
        horizon: u64,
        train_seed: u64,
        layout_seeds: &[u64],
    ) -> VoxelProtocol {
        let manifest = founder.manifest();
        VoxelProtocol {
            schema: "cub-voxel-es-1".into(),
            founder: founder.name().into(),
            digest: voxel_schema_digest(founder),
            horizon_ticks: horizon,
            stage: stage.as_str().into(),
            starting_stores: task::STARTING_STORES_PROTOCOL.into(),
            start_heading: task::START_HEADING_PROTOCOL.into(),
            pairs,
            sigma: optimizer::SIGMA,
            learning_rate: optimizer::LEARNING_RATE,
            beta1: optimizer::BETA1,
            beta2: optimizer::BETA2,
            adam_eps: optimizer::ADAM_EPS,
            survival_weight: super::score::SURVIVAL_WEIGHT,
            train_seed,
            layout_seeds: layout_seeds.to_vec(),
            reference_mass: manifest.body_reference,
            controller: "gru".into(),
            aggregate: "mean".into(),
        }
    }

    /// The founder this protocol names.
    pub fn founder_parsed(&self) -> Result<Founder, String> {
        super::parse_founder(&self.founder)
    }

    /// FNV-1a 64 over the protocol's canonical JSON, the flat trainer's convention.
    pub fn hash(&self) -> u64 {
        crate::es::fixture::fnv1a(
            serde_json::to_string(self)
                .expect("the protocol serializes")
                .as_bytes(),
        )
    }
}

/// One named episode: the stable identity and its result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub generation: u32,
    pub candidate: String,
    pub layout_seed: u64,
    pub episode: Episode,
}

/// Which candidate a job belongs to. The centre is candidate `2·pairs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Candidate {
    Plus(usize),
    Minus(usize),
    Center,
}

impl Candidate {
    /// `2·pair + sign`, with the centre after every pair.
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

/// What one generation produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationReport {
    pub generation: u32,
    /// `2n` perturbation scores, in candidate index order.
    pub candidate_scores: Vec<f64>,
    /// The unperturbed centre's score, when it was evaluated this generation.
    pub center_score: Option<f64>,
    /// The mean absolute spread of the perturbation scores: the ranking information the
    /// generation carried. Zero means no ranking information (the tests plan's check
    /// before the first update).
    pub score_spread: f64,
    pub jobs: Vec<Job>,
    pub gradient_norm: f64,
    pub update_rms: f64,
    pub episodes_run: u64,
    pub ticks_run: u64,
    pub wall_seconds: f64,
}

/// Work a discarded generation actually performed, counted against the budget and
/// deliberately not against optimizer progress.
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
#[derive(Clone, Debug)]
pub enum GenerationError {
    /// The deadline or the flag stopped it. The centre and Adam are untouched; the work
    /// it did is reported for the budget.
    Cancelled(Discarded),
    /// An episode found an invalid fixture or state. An experiment error, not a score.
    Invalid {
        job: String,
        detail: String,
        discarded: Discarded,
    },
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
                write!(f, "invalid episode in job {job}: {detail}")
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

/// Everything one generation needs that is not the centre.
pub struct GenerationPlan<'a> {
    pub layouts: &'a [Prepared],
    pub horizon: u64,
    /// Episode workers, capped at [`super::task::episode_worker_limit`] — one simulation
    /// thread per episode, never nested episode/world parallelism.
    pub workers: usize,
    pub evaluate_center: bool,
    pub deadline: Option<Instant>,
}

/// Run one generation and, if every job completed, apply the Adam ascent to `theta`.
///
/// Returns `Err(Cancelled)` without touching `theta` or `adam` when the run was stopped;
/// the work it did is carried in the error for the budget.
pub fn run_generation(
    theta: &mut [f64],
    adam: &mut Adam,
    protocol: &VoxelProtocol,
    generation: u32,
    plan: &GenerationPlan<'_>,
    cancel: &AtomicBool,
) -> Result<GenerationReport, GenerationError> {
    let started = Instant::now();
    let founder = protocol
        .founder_parsed()
        .expect("the protocol names a founder");
    let n = protocol.pairs;
    let layouts = plan.layouts;
    assert!(
        n > 0 && !layouts.is_empty(),
        "a generation needs pairs and layouts"
    );
    let params = theta.len();
    assert_eq!(
        params,
        founder.manifest().parameter_count(),
        "the centre is the manifest's shape"
    );

    // 1. Every candidate's parameter vector, in candidate index order, on this thread.
    //    Both signs of a pair share one epsilon, regenerated from its position.
    let candidates: Vec<Candidate> = (0..n)
        .flat_map(|p| [Candidate::Plus(p), Candidate::Minus(p)])
        .chain(plan.evaluate_center.then_some(Candidate::Center))
        .collect();
    let mut eps = vec![0.0; params];
    let mut drivers: Vec<EpisodeDriver> = Vec::with_capacity(candidates.len());
    for c in &candidates {
        let theta_c = match c {
            Candidate::Center => theta.to_vec(),
            Candidate::Plus(p) | Candidate::Minus(p) => {
                perturbation(
                    protocol.train_seed,
                    u64::from(generation),
                    *p as u64,
                    &mut eps,
                );
                let sign = if matches!(c, Candidate::Plus(_)) {
                    1.0
                } else {
                    -1.0
                };
                (0..params)
                    .map(|j| theta[j] + sign * protocol.sigma * eps[j])
                    .collect()
            }
        };
        drivers.push(EpisodeDriver::gru(&theta_c, founder).map_err(|e| {
            GenerationError::Invalid {
                job: format!("gen{generation}/{}", c.label()),
                detail: e,
                discarded: Discarded::default(),
            }
        })?);
    }

    // 2. One job per (candidate, layout), at its stable index, into a pre-sized slot.
    let jobs_total = candidates.len() * layouts.len();
    let names: Vec<String> = (0..jobs_total)
        .map(|i| {
            format!(
                "gen{generation}/{}/seed{}",
                candidates[i / layouts.len()].label(),
                layouts[i % layouts.len()].layout_seed
            )
        })
        .collect();
    let slots: Mutex<Vec<Option<Episode>>> = Mutex::new(vec![None; jobs_total]);
    let cursor = AtomicUsize::new(0);
    let attempted = AtomicU64::new(0);
    let completed = AtomicU64::new(0);
    let ticks = AtomicU64::new(0);
    let failure: Mutex<Option<(String, String)>> = Mutex::new(None);
    let limits = Limits {
        cancel,
        deadline: plan.deadline,
    };
    let workers = plan
        .workers
        .max(1)
        .min(task::episode_worker_limit())
        .min(jobs_total.max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = cursor.fetch_add(1, Ordering::SeqCst);
                    if index >= jobs_total {
                        return;
                    }
                    if limits.expired() {
                        return;
                    }
                    attempted.fetch_add(1, Ordering::SeqCst);
                    let candidate = candidates[index / layouts.len()];
                    match driver::run_prepared(
                        &layouts[index % layouts.len()],
                        &drivers[index / layouts.len()],
                        plan.horizon,
                        limits,
                        &names[index],
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
                            // Stop every other worker: an invalid episode ends the
                            // experiment, and continuing would burn budget on a broken run.
                            cancel.store(true, Ordering::SeqCst);
                            let mut slot = failure.lock().expect("failure");
                            if slot.is_none() {
                                *slot = Some((names[index].clone(), detail));
                            }
                            return;
                        }
                    }
                    let _ = candidate;
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
        return Err(GenerationError::Invalid {
            job,
            detail,
            discarded,
        });
    }
    let slots = slots.into_inner().expect("slots");
    if slots.iter().any(Option::is_none) {
        return Err(GenerationError::Cancelled(discarded));
    }

    // 3. The update is committed only if the run stayed inside its limits: a generation
    //    that finished its last episode *after* the deadline has not earned an update.
    if limits.expired() {
        return Err(GenerationError::Cancelled(Discarded {
            episodes_attempted: jobs_total as u64,
            episodes_completed: jobs_total as u64,
            ticks_run: slots.iter().flatten().map(|e| e.ticks).sum(),
        }));
    }

    // 4. Reduce in the fixed index order; scheduling cannot reach this. A candidate's
    //    score is the mean of its episodes' scores over its layouts.
    let episodes: Vec<Episode> = slots.into_iter().map(|s| s.expect("checked")).collect();
    let mut jobs = Vec::with_capacity(jobs_total);
    let mut scores = vec![0.0; candidates.len()];
    for (ci, c) in candidates.iter().enumerate() {
        let slice = &episodes[ci * layouts.len()..(ci + 1) * layouts.len()];
        scores[ci] = slice.iter().map(|e| e.score.score).sum::<f64>() / slice.len() as f64;
        for (li, e) in slice.iter().enumerate() {
            jobs.push(Job {
                generation,
                candidate: c.label(),
                layout_seed: layouts[li].layout_seed,
                episode: e.clone(),
            });
        }
    }
    let plus: Vec<f64> = (0..n).map(|p| scores[2 * p]).collect();
    let minus: Vec<f64> = (0..n).map(|p| scores[2 * p + 1]).collect();
    let center_score = plan.evaluate_center.then(|| scores[2 * n]);
    let perturbation_scores = &scores[..2 * n];
    let lo = perturbation_scores
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min);
    let hi = perturbation_scores
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);
    let score_spread = if hi > lo { hi - lo } else { 0.0 };

    // 5. The antithetic gradient estimate and the Adam ascent, the flat trainer's own
    //    arithmetic, over this shape's parameter count.
    let g = optimizer::gradient(&plus, &minus, params, protocol.sigma, |i, out| {
        perturbation(protocol.train_seed, u64::from(generation), i as u64, out);
    });
    assert!(
        g.iter().all(|x| x.is_finite()),
        "a non-finite gradient is an experiment error"
    );
    let gradient_norm = g.iter().map(|x| x * x).sum::<f64>().sqrt();
    let update_rms = adam.ascend(theta, &g);
    assert!(
        theta.iter().all(|x| x.is_finite()),
        "the updated centre must stay finite"
    );

    Ok(GenerationReport {
        generation,
        candidate_scores: scores[..2 * n].to_vec(),
        center_score,
        score_spread,
        jobs,
        gradient_norm,
        update_rms,
        episodes_run: jobs_total as u64,
        ticks_run: episodes.iter().map(|e| e.ticks).sum(),
        wall_seconds: started.elapsed().as_secs_f64(),
    })
}

/// The persisted optimizer state: everything needed to continue a run exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoxelCheckpoint {
    pub schema: String,
    pub build: String,
    pub protocol: VoxelProtocol,
    pub protocol_hash: u64,
    /// The exact central weights in [`super::super::tensor`]'s shape order, little-endian
    /// IEEE-754 hex ([`super::super::bits`]).
    #[serde(with = "crate::es::bits::hex_f64s")]
    pub theta: Vec<f64>,
    pub adam: Adam,
    pub train_seed: u64,
    pub generations_completed: u32,
    /// Episodes dispatched, including discarded and cancelled work: the budget's unit.
    pub episodes_attempted: u64,
    pub episodes_completed: u64,
    pub ticks_run: u64,
    pub discarded: Discarded,
    /// One record per centre the run evaluated, oldest first.
    pub centers: Vec<CenterRecord>,
}

/// One evaluated centre in the run's history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CenterRecord {
    pub generation: u32,
    pub score: f64,
    /// Path, relative to the run directory, of the policy file holding those weights.
    pub file: String,
    /// FNV-1a 64 over the weights' little-endian hex, so a file can be checked against
    /// the record.
    pub weights_fnv1a: u64,
}

impl VoxelCheckpoint {
    pub fn fresh(protocol: VoxelProtocol, build: &str) -> VoxelCheckpoint {
        let founder = protocol
            .founder_parsed()
            .expect("the protocol names a founder");
        let train_seed = protocol.train_seed;
        let theta = if founder == Founder::Blind {
            super::super::tensor::initial_center_shape::<23, 3>(protocol.train_seed)
        } else {
            super::super::tensor::initial_center_shape::<37, 3>(protocol.train_seed)
        };
        let hash = protocol.hash();
        VoxelCheckpoint {
            schema: CHECKPOINT_SCHEMA.into(),
            build: build.to_string(),
            protocol_hash: hash,
            adam: Adam::new(theta.len()),
            protocol,
            theta,
            train_seed,
            generations_completed: 0,
            episodes_attempted: 0,
            episodes_completed: 0,
            ticks_run: 0,
            discarded: Discarded::default(),
            centers: Vec::new(),
        }
    }

    /// Shape, finiteness, protocol-hash agreement, and a centre that validates against
    /// the recorded digest.
    pub fn validate(&self) -> Result<(), String> {
        let founder = self.protocol.founder_parsed()?;
        let want = founder.manifest().parameter_count();
        if self.theta.len() != want {
            return Err(format!(
                "checkpoint theta has {} values, expected {want}",
                self.theta.len()
            ));
        }
        if !self.theta.iter().all(|x| x.is_finite()) || !self.adam.is_finite() {
            return Err("checkpoint holds a non-finite value".into());
        }
        if self.adam.m.len() != want || self.adam.v.len() != want {
            return Err("checkpoint Adam moments have the wrong dimension".into());
        }
        if self.protocol.hash() != self.protocol_hash {
            return Err("checkpoint protocol hash does not match its protocol".into());
        }
        if self.train_seed != self.protocol.train_seed {
            return Err("checkpoint train seed does not match its protocol".into());
        }
        let digest = voxel_schema_digest(founder);
        if self.protocol.digest != digest {
            return Err(format!(
                "checkpoint was authored against manifest digest {:#018x}, this build's \
                 {} manifest is {digest:#018x}: the schemas differ and the weights cannot \
                 be reinterpreted",
                self.protocol.digest,
                founder.name()
            ));
        }
        Ok(())
    }
}

/// One bounded ES training run: the initial centre evaluation, then up to `updates`
/// generations, honouring the wall cap, the episode limit and the flag throughout.
///
/// The plan's accounting is the run's: the initial centre evaluation is generation 0's
/// centre record, and every attempted episode — including discarded and cancelled work —
/// counts against `spec.episode_limit`.
pub struct TrainSpec {
    pub founder: Founder,
    pub stage: Stage,
    pub pairs: usize,
    /// How many of the frozen training layouts to run, from the front.
    pub layouts: usize,
    /// Updates, capped at [`super::task::MAX_UPDATES`].
    pub updates: u32,
    pub horizon: u64,
    pub workers: usize,
    pub wall_seconds: u64,
    pub episode_limit: u64,
    pub train_seed: u64,
    pub evaluate_center: bool,
    pub out: PathBuf,
}

/// Why a training run stopped.
#[derive(Clone, Debug)]
pub enum TrainStop {
    /// Every requested update ran.
    Updates,
    /// The wall cap or the cancellation flag stopped it between or inside generations.
    Wall,
    /// The episode limit stopped it before the next generation.
    EpisodeLimit,
}

impl std::fmt::Display for TrainStop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrainStop::Updates => write!(f, "completed every update"),
            TrainStop::Wall => write!(f, "stopped at the wall cap"),
            TrainStop::EpisodeLimit => write!(f, "stopped at the episode limit"),
        }
    }
}

/// What a completed-or-stopped run reports.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainReport {
    pub protocol_hash: u64,
    pub generations_completed: u32,
    pub updates_requested: u32,
    pub stop: String,
    pub episodes_attempted: u64,
    pub episodes_completed: u64,
    pub ticks_run: u64,
    pub discarded: Discarded,
    pub wall_seconds: f64,
    pub initial_center_score: Option<f64>,
    pub best: Option<CenterRecord>,
    /// The mean absolute spread of each generation's perturbation scores.
    pub score_spreads: Vec<f64>,
    pub checkpoint: String,
}

/// Run the bounded training.
pub fn train(spec: &TrainSpec, cancel: &AtomicBool) -> Result<TrainReport, String> {
    if spec.pairs == 0 {
        return Err("--pairs must be at least one".into());
    }
    if spec.layouts == 0 || spec.layouts > task::TRAINING_LAYOUT_SEEDS.len() {
        return Err(format!(
            "--layouts must be in 1..={}",
            task::TRAINING_LAYOUT_SEEDS.len()
        ));
    }
    let worker_limit = task::episode_worker_limit();
    if spec.workers == 0 || spec.workers > worker_limit {
        return Err(format!("--workers must be in 1..={}", worker_limit));
    }
    if spec.updates == 0 || spec.updates > task::MAX_UPDATES {
        return Err(format!("--updates must be in 1..={}", task::MAX_UPDATES));
    }
    if spec.horizon == 0 {
        return Err("--horizon must be at least one tick".into());
    }

    let started = Instant::now();
    let deadline = started + Duration::from_secs(spec.wall_seconds.max(1));
    let protocol = VoxelProtocol::new(
        spec.founder,
        spec.stage,
        spec.pairs,
        spec.horizon,
        spec.train_seed,
        &task::TRAINING_LAYOUT_SEEDS[..spec.layouts],
    );
    let layouts = task::training_layouts(spec.founder, spec.stage);
    let run_dir = spec.out.clone();
    std::fs::create_dir_all(run_dir.join("centers"))
        .map_err(|e| format!("cannot create {}: {e}", run_dir.display()))?;
    let checkpoint_path = run_dir.join("checkpoint.json");
    let mut cp = VoxelCheckpoint::fresh(protocol, crate::evaluate::BUILD_ID);
    cp.validate()
        .map_err(|e| format!("fresh checkpoint: {e}"))?;
    let mut score_spreads = Vec::new();
    let mut stop = TrainStop::Updates;

    // 0. The initial centre evaluation — generation 0's centre record. The plan's episode
    //    accounting starts here ("2,180 episodes including the initial centre").
    let initial = {
        let driver = center_driver(&cp)?;
        let mut scores = Vec::new();
        let mut attempted = 0u64;
        let mut ticks = 0u64;
        for layout in layouts.iter().take(spec.layouts) {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            attempted += 1;
            match driver::run_prepared(
                layout,
                &driver,
                spec.horizon,
                Limits {
                    cancel,
                    deadline: Some(deadline),
                },
                &format!("gen0/center/seed{}", layout.layout_seed),
            ) {
                Ok(e) => {
                    ticks += e.ticks;
                    scores.push(e.score.score);
                }
                Err(EpisodeError::Cancelled { ticks: t }) => {
                    ticks += t;
                    break;
                }
                Err(EpisodeError::Invalid { ticks: t, detail }) => {
                    return Err(format!(
                        "gen0/center/seed{}: invalid after {t} ticks: {detail}",
                        layout.layout_seed
                    ));
                }
            }
        }
        cp.episodes_attempted += attempted;
        cp.episodes_completed += scores.len() as u64;
        cp.ticks_run += ticks;
        let mean = (!scores.is_empty()).then(|| scores.iter().sum::<f64>() / scores.len() as f64);
        if let Some(score) = mean {
            let file = "centers/gen0-center.json";
            write_center_policy(&run_dir, file, &cp, 0, score)?;
            cp.centers.push(CenterRecord {
                generation: 0,
                score,
                file: file.to_string(),
                weights_fnv1a: fnv1a_hex(&cp.theta),
            });
        }
        mean
    };

    // 1. The updates.
    let mut cancelled_at: Option<String> = None;
    for generation in 0..spec.updates {
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            stop = TrainStop::Wall;
            break;
        }
        let next_generation_episodes =
            (2 * spec.pairs as u64 + u64::from(spec.evaluate_center)) * spec.layouts as u64;
        if cp.episodes_attempted + next_generation_episodes > spec.episode_limit {
            stop = TrainStop::EpisodeLimit;
            break;
        }
        let plan = GenerationPlan {
            layouts: &layouts[..spec.layouts],
            horizon: spec.horizon,
            workers: spec.workers,
            evaluate_center: spec.evaluate_center,
            deadline: Some(deadline),
        };
        match run_generation(
            &mut cp.theta,
            &mut cp.adam,
            &cp.protocol,
            generation,
            &plan,
            cancel,
        ) {
            Ok(report) => {
                cp.generations_completed = generation + 1;
                cp.episodes_attempted += report.episodes_run;
                cp.episodes_completed += report.episodes_run;
                cp.ticks_run += report.ticks_run;
                score_spreads.push(report.score_spread);
                if let Some(score) = report.center_score {
                    let file = format!("centers/gen{generation}-center.json");
                    write_center_policy(&run_dir, &file, &cp, generation, score)?;
                    cp.centers.push(CenterRecord {
                        generation,
                        score,
                        file,
                        weights_fnv1a: fnv1a_hex(&cp.theta),
                    });
                }
                println!(
                    "gen {generation:>3}  score(mean) {:.4}  center {:.4}  spread {:.2e}  |g| {:.2e}  Δ {:.2e}  {} ep  {:.2}s",
                    report.candidate_scores.iter().sum::<f64>()
                        / report.candidate_scores.len().max(1) as f64,
                    report.center_score.map_or(f64::NAN, |s| s),
                    report.score_spread,
                    report.gradient_norm,
                    report.update_rms,
                    report.episodes_run,
                    report.wall_seconds,
                );
            }
            Err(GenerationError::Cancelled(d)) => {
                cp.discarded.add(d);
                stop = TrainStop::Wall;
                cancelled_at = Some(format!("generation {generation} cancelled: {d:?}"));
                break;
            }
            Err(GenerationError::Invalid {
                job,
                detail,
                discarded,
            }) => {
                cp.discarded.add(discarded);
                save_checkpoint(&checkpoint_path, &cp)?;
                return Err(format!("invalid episode in {job}: {detail}"));
            }
        }
        save_checkpoint(&checkpoint_path, &cp)?;
    }
    if let Some(what) = cancelled_at {
        println!("# {what}");
    }

    save_checkpoint(&checkpoint_path, &cp)?;
    let best = cp
        .centers
        .iter()
        .max_by(|a, b| a.score.partial_cmp(&b.score).expect("scores are finite"))
        .cloned();
    Ok(TrainReport {
        protocol_hash: cp.protocol_hash,
        generations_completed: cp.generations_completed,
        updates_requested: spec.updates,
        stop: stop.to_string(),
        episodes_attempted: cp.episodes_attempted,
        episodes_completed: cp.episodes_completed,
        ticks_run: cp.ticks_run,
        discarded: cp.discarded,
        wall_seconds: started.elapsed().as_secs_f64(),
        initial_center_score: initial,
        best,
        score_spreads,
        checkpoint: checkpoint_path.display().to_string(),
    })
}

/// The centre's driver, rebuilt from the checkpoint's exact weights.
fn center_driver(cp: &VoxelCheckpoint) -> Result<EpisodeDriver, String> {
    let founder = cp.protocol.founder_parsed()?;
    EpisodeDriver::gru(&cp.theta, founder)
}

/// Write one centre's exact weights as a self-contained policy file.
fn write_center_policy(
    run_dir: &std::path::Path,
    relative: &str,
    cp: &VoxelCheckpoint,
    generation: u32,
    score: f64,
) -> Result<(), String> {
    let founder = cp.protocol.founder_parsed()?;
    let file = super::store::VoxelPolicyFile {
        schema: super::store::POLICY_SCHEMA.into(),
        build: crate::evaluate::BUILD_ID.into(),
        founder: founder.name().into(),
        digest: cp.protocol.digest,
        train_seed: cp.train_seed,
        generation: Some(u64::from(generation)),
        score: Some(score),
        start_heading: cp.protocol.start_heading.clone(),
        starting_stores: cp.protocol.starting_stores.clone(),
        stage: cp.protocol.stage.clone(),
        theta: cp.theta.clone(),
    };
    file.write(&run_dir.join(relative))
}

/// FNV-1a 64 over the weights' little-endian hex — [`crate::es::fixture::fnv1a`] over the
/// same bytes [`crate::es::bits::encode`] writes.
fn fnv1a_hex(theta: &[f64]) -> u64 {
    crate::es::fixture::fnv1a(crate::es::bits::encode(theta).as_bytes())
}

/// Save the checkpoint JSON at every generation boundary. It preserves the exact centre
/// and Adam state for inspection or a future explicit resume command.
pub fn save_checkpoint(path: &std::path::Path, cp: &VoxelCheckpoint) -> Result<(), String> {
    let json = serde_json::to_string_pretty(cp).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Load and validate a checkpoint, refusing one written under a different protocol hash
/// than `expected_hash` when one is given.
pub fn load_checkpoint(
    path: &std::path::Path,
    expected_hash: Option<u64>,
) -> Result<VoxelCheckpoint, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let cp: VoxelCheckpoint =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    if cp.schema != CHECKPOINT_SCHEMA {
        return Err(format!(
            "{}: schema `{}` is not {CHECKPOINT_SCHEMA}",
            path.display(),
            cp.schema
        ));
    }
    cp.validate()?;
    if let Some(want) = expected_hash
        && cp.protocol_hash != want
    {
        return Err(format!(
            "{}: protocol hash {:#018x} is not this run's {want:#018x}",
            path.display(),
            cp.protocol_hash
        ));
    }
    Ok(cp)
}

#[cfg(test)]
mod tests {
    use super::super::controller::{EpisodeKind, GruPolicy};
    use super::*;

    fn smoke_spec() -> TrainSpec {
        TrainSpec {
            founder: Founder::Blind,
            stage: Stage::A,
            pairs: 1,
            layouts: 1,
            updates: 1,
            horizon: 40,
            workers: 1,
            wall_seconds: 60,
            episode_limit: u64::MAX,
            train_seed: 20_260_918,
            evaluate_center: true,
            out: PathBuf::from("/tmp/opencode/cubarium-voxel-es-test"),
        }
    }

    /// The smoke train: one tiny generation runs the loop end to end — centre
    /// evaluation, perturbation generation, dispatch, reduction, Adam step, checkpoint
    /// and policy files on disk. This is a plumbing smoke only; score spread belongs in
    /// a deliberately bounded pilot command, not a long CI rollout.
    #[test]
    fn a_smoke_train_runs_the_whole_loop_and_writes_the_store() {
        let dir = tempfile_guard();
        let spec = TrainSpec {
            out: dir.clone(),
            ..smoke_spec()
        };
        let cancel = AtomicBool::new(false);
        let report = train(&spec, &cancel).expect("the smoke train runs");
        assert_eq!(report.generations_completed, 1);
        assert_eq!(
            report.episodes_attempted,
            // the initial centre evaluation + one pair and the generation centre
            1 + (2 + 1),
        );
        assert!(
            report.initial_center_score.is_some(),
            "the initial centre was evaluated and recorded"
        );
        assert_eq!(report.score_spreads.len(), 1);
        assert!(
            report.best.is_some(),
            "the centre was evaluated and recorded"
        );
        assert!(report.best.as_ref().is_some_and(|b| b.file.contains("gen")));
        let cp = load_checkpoint(&dir.join("checkpoint.json"), None).expect("valid checkpoint");
        assert_eq!(cp.generations_completed, 1);
        assert_eq!(
            cp.centers.len(),
            2,
            "initial centre + generation 0's centre"
        );
        assert!(cp.centers.iter().all(|c| c.score.is_finite()));
        assert_eq!(cp.theta.len(), Founder::Blind.manifest().parameter_count());
        assert_eq!(cp.adam.step, 1, "a completed generation advances Adam once");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A pre-cancelled generation dispatches no episodes and leaves optimizer state alone.
    #[test]
    fn a_cancelled_generation_leaves_the_centre_alone() {
        let cancel = AtomicBool::new(true);
        let founder = Founder::Blind;
        let protocol = VoxelProtocol::new(
            founder,
            Stage::A,
            1,
            120,
            20_260_918,
            &task::TRAINING_LAYOUT_SEEDS[..1],
        );
        let layouts = task::training_layouts(founder, Stage::A);
        let mut theta =
            super::super::super::tensor::initial_center_shape::<23, 3>(protocol.train_seed);
        let before = theta.clone();
        let mut adam = Adam::new(theta.len());
        let plan = GenerationPlan {
            layouts: &layouts[..1],
            horizon: 120,
            workers: 1,
            evaluate_center: false,
            deadline: None,
        };
        let out = run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel);
        let err = out.expect_err("the cancellation flag must stop it");
        let discarded = err.discarded();
        assert_eq!(discarded, Discarded::default());
        assert_eq!(theta, before, "a cancelled generation updates nothing");
        assert_eq!(adam.step, 0);
    }

    /// A tiny perturbation batch actually changes the evaluated weights: the two signs
    /// of one pair are symmetric about the centre, differ from it, and both validate
    /// against the digest; and the drivers they build act differently on one
    /// observation.
    #[test]
    fn a_tiny_perturbation_batch_changes_the_evaluated_weights() {
        let founder = Founder::Blind;
        let digest = voxel_schema_digest(founder);
        let params = founder.manifest().parameter_count();
        let theta = super::super::super::tensor::initial_center_shape::<23, 3>(9);
        let mut eps = vec![0.0; params];
        perturbation(20_260_918, 0, 0, &mut eps);
        let sigma = optimizer::SIGMA;
        let plus: Vec<f64> = (0..params).map(|j| theta[j] + sigma * eps[j]).collect();
        let minus: Vec<f64> = (0..params).map(|j| theta[j] - sigma * eps[j]).collect();
        assert_ne!(plus, theta);
        assert_ne!(minus, theta);
        assert_ne!(plus, minus);
        // Symmetric about the centre, element for element.
        for j in 0..params {
            assert!(((plus[j] + minus[j]) / 2.0 - theta[j]).abs() < 1e-15);
        }
        // And the evaluated policies are real, validated policies: the drivers build,
        // and the two signs evaluate different weights — different raw logits on the
        // same observation, which the fauna's shared adapter decodes into different
        // held actions.
        let d_plus = EpisodeDriver::gru(&plus, founder).expect("a finite perturbation is a policy");
        let d_minus = EpisodeDriver::gru(&minus, founder).expect("and its antithesis");
        let obs = [0.0; 23];
        let (EpisodeKind::Gru(_), EpisodeKind::Gru(_)) = (d_plus.kind(), d_minus.kind()) else {
            panic!("gru drivers");
        };
        use cubarium_voxel_fauna::Controller as _;
        let mut c_plus = GruPolicy::<23>::new(&plus, voxel_schema_digest(founder))
            .expect("the perturbed centre is a policy");
        let mut c_minus =
            GruPolicy::<23>::new(&minus, voxel_schema_digest(founder)).expect("and its antithesis");
        assert_ne!(
            c_plus.drive(&obs),
            c_minus.drive(&obs),
            "the two signs must evaluate different weights"
        );
        let _ = digest;
    }

    /// The worker count cannot reach the reduction: one worker and four produce the
    /// identical centre and identical per-candidate scores.
    #[test]
    fn a_generation_reduces_identically_whatever_the_worker_count() {
        let founder = Founder::Blind;
        let protocol = VoxelProtocol::new(
            founder,
            Stage::A,
            1,
            40,
            20_260_918,
            &task::TRAINING_LAYOUT_SEEDS[..1],
        );
        let layouts = task::training_layouts(founder, Stage::A);
        let run = |workers: usize| {
            let cancel = AtomicBool::new(false);
            let mut theta =
                super::super::super::tensor::initial_center_shape::<23, 3>(protocol.train_seed);
            let mut adam = Adam::new(theta.len());
            let plan = GenerationPlan {
                layouts: &layouts[..1],
                horizon: 40,
                workers,
                evaluate_center: false,
                deadline: None,
            };
            let report =
                run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel).expect("ok");
            (theta, adam, report)
        };
        let (t1, a1, r1) = run(1);
        let (t4, a4, r4) = run(4);
        assert_eq!(r1.candidate_scores, r4.candidate_scores);
        assert_eq!(r1.jobs, r4.jobs);
        assert_eq!(t1, t4, "the centre must not depend on the worker count");
        assert_eq!(a1, a4);
    }

    /// A temporary run directory under the approved external path.
    fn tempfile_guard() -> PathBuf {
        let dir = PathBuf::from(format!(
            "/tmp/opencode/cubarium-voxel-es-test-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }
}
