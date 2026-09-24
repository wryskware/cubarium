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
use super::remote::RemotePool;
use super::task::{self, Prepared, Stage};
use super::voxel_schema_digest;
use rustc_hash::FxHashMap;

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
    /// The stage-specific arena fixture revision, which for Stage B names the successor
    /// separation band ([`task::stage_b_arena_protocol`]). Stage B changes when its
    /// stock or geometry changes, so an old centre cannot silently resume on a different
    /// task, and a near-rung run hashes differently from a landed one.
    pub arena_protocol: String,
    /// Stage B's successor separation band, `near` or `landed`; `none` on Stage A.
    pub successor_band: String,
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
    /// P5-C's mixed set (C1, C2), when the run trains on one. Absent — and so absent
    /// from the hash — on an arena-only run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mix: Option<MixProtocol>,
}

/// What a P5-C run's generations and checkpoints ran on besides the arenas
/// (`design/handoffs/voxel-retrain-c-2026-09-22.md`, C1–C2). Part of the protocol hash.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MixProtocol {
    /// [`super::landscape::LANDSCAPE_PROTOCOL`].
    pub landscape_protocol: String,
    pub landscape_horizon: u64,
    /// Whether the landscapes carried replayed litter production (S1).
    pub landscape_live_plants: bool,
    /// The grid of each arena layout, in `layout_seeds` order.
    pub arena_grids: Vec<String>,
    /// Every training landscape fixture the draw chooses from: `preset/base/water@world`.
    pub pool: Vec<String>,
    /// Landscapes drawn per generation.
    pub per_generation: usize,
    /// How the draw is made ([`super::landscape::generation_draw`]).
    pub draw: String,
    /// The held-out fixtures the checkpoints are scored on.
    pub held_out: Vec<String>,
    /// Updates between held-out checkpoints.
    pub held_out_every: u32,
}

/// The draw's name in [`MixProtocol::draw`].
pub const LANDSCAPE_DRAW: &str = "p5c-per-generation-without-replacement-1";

impl VoxelProtocol {
    pub fn new(
        founder: Founder,
        stage: Stage,
        band: task::Band,
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
            arena_protocol: task::arena_protocol(founder, stage, band),
            successor_band: match stage {
                Stage::A => "none".into(),
                Stage::B => band.as_str().into(),
            },
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
            mix: None,
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
    /// Who ran the generation, when remotes took part: `local` first, then each
    /// remote. Empty on a local-only generation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shares: Vec<Share>,
    /// Units a lost remote handed back to the local workers.
    #[serde(default)]
    pub requeued: u64,
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
    /// Remote workers that take units alongside the local ones, and the index every
    /// layout has in the fixture list they founded. `None` runs local-only.
    pub remote: Option<RemotePlan<'a>>,
}

/// The remote half of a [`GenerationPlan`].
#[derive(Clone, Copy)]
pub struct RemotePlan<'a> {
    pub pool: &'a RemotePool,
    /// Per layout of the plan, its index in the fixture list the remotes founded
    /// ([`super::remote::FixtureSpec::build`]).
    pub fixture_ids: &'a [usize],
}

/// Who ran how much of one generation: `local`, or a remote's name. Only the results
/// the reduction used are counted.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Share {
    pub executor: String,
    /// Units: both signs of one pair on one fixture, or the centre on one.
    pub units: u64,
    pub episodes: u64,
    pub ticks: u64,
    /// Thread-seconds the units took where they ran: ticks / busy is one thread's rate.
    pub busy_seconds: f64,
}

/// The seed one generation's episodes on one layout draw their body sizes from (D11):
/// the same for every candidate of the generation — both signs of a pair, and the centre
/// — so a pair still runs identical worlds, and different from one generation to the
/// next, so the sizes a centre is ranked on keep moving. From the trainer's own stream.
pub fn episode_seed(train_seed: u64, generation: u32, layout_index: usize) -> u64 {
    let mut z = train_seed
        ^ u64::from(generation).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (layout_index as u64).rotate_left(40);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// One candidate's parameter vector: the centre, or the centre plus or minus `sigma`
/// times the pair's positional perturbation. The one place it is computed — the
/// coordinator and a remote worker ([`super::remote`]) both call it, so a pair evaluated
/// anywhere evaluates the same weights. `eps` is scratch of the centre's length.
pub fn candidate_theta(
    theta: &[f64],
    train_seed: u64,
    sigma: f64,
    generation: u32,
    candidate: Candidate,
    eps: &mut [f64],
) -> Vec<f64> {
    match candidate {
        Candidate::Center => theta.to_vec(),
        Candidate::Plus(p) | Candidate::Minus(p) => {
            perturbation(train_seed, u64::from(generation), p as u64, eps);
            let sign = if matches!(candidate, Candidate::Plus(_)) {
                1.0
            } else {
                -1.0
            };
            theta
                .iter()
                .zip(eps.iter())
                .map(|(t, e)| t + sign * sigma * e)
                .collect()
        }
    }
}

/// One unit of dispatch: **both signs of one pair on one fixture**, or the centre on one.
/// A pair never splits across machines, so whatever one machine's floats do, they do to
/// both signs of the difference the gradient is made of.
#[derive(Clone, Copy, Debug)]
struct Unit {
    pair: Option<usize>,
    layout: usize,
}

impl Unit {
    fn candidates(self) -> Vec<Candidate> {
        match self.pair {
            Some(p) => vec![Candidate::Plus(p), Candidate::Minus(p)],
            None => vec![Candidate::Center],
        }
    }
}

/// The generation's shared board: who may take which unit, and the results.
struct Board {
    /// Units anyone may take, longest first.
    pending: std::collections::VecDeque<usize>,
    /// Units a lost remote had in hand: local workers only.
    local_only: std::collections::VecDeque<usize>,
    /// Units out on remotes right now. A local worker with nothing to take waits while
    /// this is non-zero, because a remote that dies hands its units back.
    on_remote: usize,
    slots: Vec<Option<Episode>>,
    /// `local`, then each active remote in pool order.
    shares: Vec<Share>,
    requeued: u64,
}

/// Run one generation and, if every job completed, apply the Adam ascent to `theta`.
///
/// Returns `Err(Cancelled)` without touching `theta` or `adam` when the run was stopped;
/// the work it did is carried in the error for the budget.
///
/// # Dispatch
///
/// The generation is cut into [`Unit`]s — both signs of one pair on one fixture, or the
/// centre on one — queued longest first (a landscape's horizon before an arena's) so the
/// generation's tail is short. Local workers and each active remote in the plan take
/// units from the same queue, so jobs go to whichever machine is idle. A remote that
/// dies or stalls hands its outstanding units back to the local workers, with one loud
/// line, and the generation completes. Every result lands at its fixed index, so the
/// reduction cannot see who ran what.
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
        let theta_c = candidate_theta(
            theta,
            protocol.train_seed,
            protocol.sigma,
            generation,
            *c,
            &mut eps,
        );
        drivers.push(EpisodeDriver::gru(&theta_c, founder).map_err(|e| {
            GenerationError::Invalid {
                job: format!("gen{generation}/{}", c.label()),
                detail: e,
                discarded: Discarded::default(),
            }
        })?);
    }

    // 2. The units, longest first, and the fixed slot every episode lands in:
    //    `candidate · layouts + layout`.
    let lcount = layouts.len();
    let jobs_total = candidates.len() * lcount;
    let names: Vec<String> = (0..jobs_total)
        .map(|i| {
            format!(
                "gen{generation}/{}/seed{}",
                candidates[i / lcount].label(),
                layouts[i % lcount].layout_seed()
            )
        })
        .collect();
    let slot_of = |c: Candidate, layout: usize| c.index(n) * lcount + layout;
    let horizon_of = |layout: usize| layouts[layout].horizon().unwrap_or(plan.horizon);
    let units: Vec<Unit> = (0..n)
        .flat_map(|p| {
            (0..lcount).map(move |layout| Unit {
                pair: Some(p),
                layout,
            })
        })
        .chain(
            (0..lcount)
                .filter(|_| plan.evaluate_center)
                .map(|layout| Unit { pair: None, layout }),
        )
        .collect();
    let mut order: Vec<usize> = (0..units.len()).collect();
    order.sort_by_key(|&u| {
        std::cmp::Reverse(horizon_of(units[u].layout) * units[u].candidates().len() as u64)
    });

    let remotes: Vec<&super::remote::Remote> = plan
        .remote
        .map(|r| r.pool.active_remotes())
        .unwrap_or_default();
    let mut shares = vec![Share {
        executor: "local".into(),
        ..Share::default()
    }];
    shares.extend(remotes.iter().map(|r| Share {
        executor: r.name().to_string(),
        ..Share::default()
    }));
    let board = Mutex::new(Board {
        pending: order.into_iter().collect(),
        local_only: std::collections::VecDeque::new(),
        on_remote: 0,
        slots: vec![None; jobs_total],
        shares,
        requeued: 0,
    });
    let wakeup = std::sync::Condvar::new();
    let attempted = AtomicU64::new(0);
    let completed = AtomicU64::new(0);
    let ticks = AtomicU64::new(0);
    let failure: Mutex<Option<(String, String)>> = Mutex::new(None);
    let limits = Limits {
        cancel,
        deadline: plan.deadline,
    };

    // Land one unit's episodes in their slots, once, and count who ran them.
    let fill = |b: &mut Board, unit: usize, episodes: Vec<Episode>, share: usize, busy: f64| {
        let u = units[unit];
        let slots: Vec<usize> = u
            .candidates()
            .into_iter()
            .map(|c| slot_of(c, u.layout))
            .collect();
        if slots.iter().any(|&s| b.slots[s].is_some()) {
            return;
        }
        let s = &mut b.shares[share];
        s.units += 1;
        s.episodes += episodes.len() as u64;
        s.ticks += episodes.iter().map(|e| e.ticks).sum::<u64>();
        s.busy_seconds += busy;
        for (slot, e) in slots.into_iter().zip(episodes) {
            b.slots[slot] = Some(e);
        }
    };

    // 3. Remotes first: each is told the generation's centre, then claims its first
    //    batch of units before the local workers start.
    let mut batches: Vec<Vec<usize>> = Vec::with_capacity(remotes.len());
    {
        let mut b = board.lock().expect("board");
        for r in &remotes {
            let mut batch = Vec::new();
            while batch.len() < r.capacity() {
                let Some(u) = b.pending.pop_front() else {
                    break;
                };
                batch.push(u);
            }
            b.on_remote += batch.len();
            batches.push(batch);
        }
    }
    let remote_unit = |id: u64, u: usize| super::remote::UnitJob {
        id,
        generation,
        pair: units[u].pair,
        fixture: plan
            .remote
            .expect("remote units have a remote plan")
            .fixture_ids[units[u].layout],
        horizon: horizon_of(units[u].layout),
        episode_seed: episode_seed(protocol.train_seed, generation, units[u].layout),
        names: units[u]
            .candidates()
            .into_iter()
            .map(|c| names[slot_of(c, units[u].layout)].clone())
            .collect(),
    };

    let workers = plan
        .workers
        .max(1)
        .min(task::episode_worker_limit())
        .min(units.len().max(1));
    let centre: &[f64] = theta;
    std::thread::scope(|scope| {
        for (k, (remote, batch)) in remotes.iter().zip(batches).enumerate() {
            let (board, wakeup, attempted, completed, ticks) =
                (&board, &wakeup, &attempted, &completed, &ticks);
            let fill = &fill;
            let remote_unit = &remote_unit;
            let units = &units;
            scope.spawn(move || {
                use super::remote::{FromWorker, Incoming};
                let mut out: FxHashMap<u64, (usize, Instant)> = FxHashMap::default();
                // Hand every unit this remote holds back to the local workers.
                let lose = |out: &mut FxHashMap<u64, (usize, Instant)>, reason: String| {
                    remote.lose(&reason);
                    let back: Vec<usize> = out.drain().map(|(_, (u, _))| u).collect();
                    let episodes: usize = back.iter().map(|&u| units[u].candidates().len()).sum();
                    {
                        let mut b = board.lock().expect("board");
                        b.on_remote -= back.len();
                        b.requeued += back.len() as u64;
                        b.local_only.extend(back.iter().copied());
                    }
                    wakeup.notify_all();
                    println!(
                        "# REMOTE {} LOST in generation {generation}: {reason}; {} outstanding \
                         units ({episodes} episodes) re-queued locally, the run goes on",
                        remote.name(),
                        back.len(),
                    );
                };
                let send = |out: &mut FxHashMap<u64, (usize, Instant)>, u: usize| {
                    let id = remote.next_id();
                    out.insert(id, (u, Instant::now()));
                    attempted.fetch_add(units[u].candidates().len() as u64, Ordering::SeqCst);
                    remote.send_unit(&remote_unit(id, u))
                };
                if let Err(e) = remote.send_generation(generation, protocol, centre) {
                    for u in batch {
                        out.insert(remote.next_id(), (u, Instant::now()));
                    }
                    return lose(&mut out, e);
                }
                for u in batch {
                    if let Err(e) = send(&mut out, u) {
                        return lose(&mut out, e);
                    }
                }
                loop {
                    if limits.expired() {
                        return;
                    }
                    // Top up to the remote's thread count from the shared queue.
                    let mut claimed = Vec::new();
                    {
                        let mut b = board.lock().expect("board");
                        while out.len() + claimed.len() < remote.capacity() {
                            let Some(u) = b.pending.pop_front() else {
                                break;
                            };
                            claimed.push(u);
                        }
                        b.on_remote += claimed.len();
                    }
                    for u in claimed {
                        if let Err(e) = send(&mut out, u) {
                            return lose(&mut out, e);
                        }
                    }
                    if out.is_empty() {
                        return;
                    }
                    match remote.recv_timeout(Duration::from_millis(100)) {
                        Some(Incoming::Message(FromWorker::Result {
                            id,
                            episodes,
                            busy_seconds,
                        })) => {
                            let Some((u, _)) = out.remove(&id) else {
                                continue; // a unit of an earlier generation
                            };
                            if episodes.len() != units[u].candidates().len() {
                                out.insert(id, (u, Instant::now()));
                                return lose(
                                    &mut out,
                                    format!("unit {id} came back with {} episodes", episodes.len()),
                                );
                            }
                            completed.fetch_add(episodes.len() as u64, Ordering::SeqCst);
                            ticks.fetch_add(
                                episodes.iter().map(|e| e.ticks).sum::<u64>(),
                                Ordering::SeqCst,
                            );
                            {
                                let mut b = board.lock().expect("board");
                                b.on_remote -= 1;
                                fill(&mut b, u, episodes, 1 + k, busy_seconds);
                            }
                            wakeup.notify_all();
                        }
                        Some(Incoming::Message(FromWorker::Failed { id, detail })) => {
                            if out.contains_key(&id) {
                                // Whether the episode is really invalid is for a local run
                                // to say: re-queued, it stops the run there if it is.
                                return lose(&mut out, format!("unit {id} failed there: {detail}"));
                            }
                        }
                        Some(Incoming::Unreadable { id, detail }) => {
                            if let Some((u, _)) = id.and_then(|id| out.remove(&id)) {
                                {
                                    let mut b = board.lock().expect("board");
                                    b.on_remote -= 1;
                                    b.requeued += 1;
                                    b.local_only.push_back(u);
                                }
                                wakeup.notify_all();
                                println!(
                                    "# remote {}: an unreadable result ({detail}); that unit \
                                     re-queued locally",
                                    remote.name()
                                );
                            }
                        }
                        Some(Incoming::Closed(reason)) => return lose(&mut out, reason),
                        Some(Incoming::Message(_)) => {}
                        None => {
                            let options = remote.options();
                            let silent = remote.silence();
                            if silent > options.stall {
                                return lose(
                                    &mut out,
                                    format!("no word for {:.1} s", silent.as_secs_f64()),
                                );
                            }
                            if let Some(oldest) = out.values().map(|(_, t)| t.elapsed()).max()
                                && oldest > options.unit_timeout
                            {
                                return lose(
                                    &mut out,
                                    format!("a unit outstanding for {:.0} s", oldest.as_secs_f64()),
                                );
                            }
                        }
                    }
                }
            });
        }
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let unit = {
                        let mut b = board.lock().expect("board");
                        loop {
                            if limits.expired() {
                                return;
                            }
                            if let Some(u) =
                                b.local_only.pop_front().or_else(|| b.pending.pop_front())
                            {
                                break u;
                            }
                            if b.on_remote == 0 {
                                return;
                            }
                            b = wakeup
                                .wait_timeout(b, Duration::from_millis(100))
                                .expect("board")
                                .0;
                        }
                    };
                    let u = units[unit];
                    let t = Instant::now();
                    let mut episodes = Vec::with_capacity(2);
                    for c in u.candidates() {
                        attempted.fetch_add(1, Ordering::SeqCst);
                        let slot = slot_of(c, u.layout);
                        match driver::run_prepared_seeded(
                            &layouts[u.layout],
                            &drivers[c.index(n)],
                            horizon_of(u.layout),
                            limits,
                            &names[slot],
                            episode_seed(protocol.train_seed, generation, u.layout),
                        ) {
                            Ok(e) => {
                                completed.fetch_add(1, Ordering::SeqCst);
                                ticks.fetch_add(e.ticks, Ordering::SeqCst);
                                episodes.push(e);
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
                                let mut first = failure.lock().expect("failure");
                                if first.is_none() {
                                    *first = Some((names[slot].clone(), detail));
                                }
                                wakeup.notify_all();
                                return;
                            }
                        }
                    }
                    let busy = t.elapsed().as_secs_f64();
                    fill(&mut board.lock().expect("board"), unit, episodes, 0, busy);
                    wakeup.notify_all();
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
    let Board {
        slots,
        shares,
        requeued,
        ..
    } = board.into_inner().expect("board");
    if slots.iter().any(Option::is_none) {
        return Err(GenerationError::Cancelled(discarded));
    }

    // 4. The update is committed only if the run stayed inside its limits: a generation
    //    that finished its last episode *after* the deadline has not earned an update.
    if limits.expired() {
        return Err(GenerationError::Cancelled(Discarded {
            episodes_attempted: jobs_total as u64,
            episodes_completed: jobs_total as u64,
            ticks_run: slots.iter().flatten().map(|e| e.ticks).sum(),
        }));
    }

    // 5. Reduce in the fixed index order; scheduling cannot reach this. A candidate's
    //    score is the mean of its episodes' scores over its layouts.
    let episodes: Vec<Episode> = slots.into_iter().map(|s| s.expect("checked")).collect();
    let mut jobs = Vec::with_capacity(jobs_total);
    let mut scores = vec![0.0; candidates.len()];
    for (ci, c) in candidates.iter().enumerate() {
        let slice = &episodes[ci * lcount..(ci + 1) * lcount];
        scores[ci] = slice.iter().map(|e| e.score.score).sum::<f64>() / slice.len() as f64;
        for (li, e) in slice.iter().enumerate() {
            jobs.push(Job {
                generation,
                candidate: c.label(),
                layout_seed: layouts[li].layout_seed(),
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

    // 6. The antithetic gradient estimate and the Adam ascent, the flat trainer's own
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
        shares: if remotes.is_empty() {
            Vec::new()
        } else {
            shares
        },
        requeued,
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
    /// Where this run's starting weights came from, when it was warm-started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_center: Option<InitProvenance>,
    /// P5-C (C2): the held-out checkpoints, oldest first. Empty on an arena-only run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub held_out: Vec<HeldOutRecord>,
}

/// One held-out checkpoint (C2): the centre after `updates` updates, scored on the
/// held-out landscapes, and the file holding exactly those weights.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeldOutRecord {
    pub updates: u32,
    /// Mean episode score over the held-out fixtures.
    pub score: f64,
    /// Mean score per preset, in the order the presets first appear.
    pub by_preset: Vec<(String, f64)>,
    /// Mean fraction of the horizon the acting bodies lived.
    pub survived: f64,
    pub file: String,
    pub weights_fnv1a: u64,
}

/// Where a warm-started run's initial weights came from. Provenance only: the run's own
/// protocol, hash and Adam state are its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InitProvenance {
    /// The source policy file, as it was named on the command line.
    pub file: String,
    /// FNV-1a 64 over the source weights' little-endian hex, so the file can be checked
    /// against the record.
    pub weights_fnv1a: u64,
    /// The source run's protocol hash, when its file recorded one (files written before
    /// P3-B did not).
    pub protocol_hash: Option<u64>,
    /// The stage and arena protocol the source was trained under, so a reader can see
    /// which rung of the curriculum this run followed.
    pub stage: String,
    pub arena_protocol: String,
    pub generation: Option<u64>,
    pub score: Option<f64>,
    /// A **transfer start** (`--transfer-from`): the manifest digest the source weights
    /// were authored against, which is not this build's. Absent on an ordinary warm
    /// start, whose digest is this build's by construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer_digest: Option<u64>,
}

/// A loaded warm-start centre: the weights plus what to record about them.
#[derive(Clone, Debug)]
pub struct InitCenter {
    pub theta: Vec<f64>,
    pub provenance: InitProvenance,
}

impl InitCenter {
    /// Load `path` as a warm start for `founder`.
    ///
    /// The founder is a refusal, not a warning: another lineage's weights are the wrong
    /// shape for this manifest and would be a different animal. The stage and band are
    /// **not** refused — walking a centre up a curriculum is the point — but both are
    /// recorded, and the new run gets its own protocol hash regardless.
    pub fn load(path: &std::path::Path, founder: Founder) -> Result<InitCenter, String> {
        let file = super::store::VoxelPolicyFile::load(path)?;
        let named = super::parse_founder(&file.founder)?;
        if named != founder {
            return Err(format!(
                "{}: --init-center is a {} centre but this run trains {}: the weights \
                 are another lineage's and cannot be reinterpreted",
                path.display(),
                named.name(),
                founder.name()
            ));
        }
        let protocol_hash = file.protocol_hash.or_else(|| sibling_protocol_hash(path));
        Ok(InitCenter {
            provenance: InitProvenance {
                file: path.display().to_string(),
                weights_fnv1a: fnv1a_hex(&file.theta),
                protocol_hash,
                stage: file.stage.clone(),
                arena_protocol: file.arena_protocol.clone(),
                generation: file.generation,
                score: file.score,
                transfer_digest: None,
            },
            theta: file.theta,
        })
    }

    /// Load `path` as a **transfer start** for `founder` (`--transfer-from`): a centre
    /// whose vector is this manifest's length, taken even when the manifest digest it was
    /// authored against is not this build's — the anchors moved, the shape did not. The
    /// source file and its digest go into the run's provenance, and the run itself is
    /// this build's in every other respect: its protocol, its digest, the centres it
    /// writes. A training start only; `--init-center`, the live loader and the
    /// shipped-centre checks stay strict, and the gate judges what comes out.
    ///
    /// Refused by name: another lineage's centre, and a vector of another length.
    pub fn transfer(path: &std::path::Path, founder: Founder) -> Result<InitCenter, String> {
        let name = path.display().to_string();
        let file = super::store::VoxelPolicyFile::read(path)?;
        let named = super::parse_founder(&file.founder).map_err(|e| format!("{name}: {e}"))?;
        if named != founder {
            return Err(format!(
                "{name}: --transfer-from is a {} centre but this run trains {}: the \
                 weights are another lineage's and cannot be reinterpreted",
                named.name(),
                founder.name()
            ));
        }
        let want = founder.manifest().parameter_count();
        if file.theta.len() != want {
            return Err(format!(
                "{name}: --transfer-from holds {} weights but this build's {} manifest has \
                 {want} parameters: a transfer start needs the same vector length",
                file.theta.len(),
                founder.name()
            ));
        }
        file.validate_for_transfer(&name)?;
        let protocol_hash = file.protocol_hash.or_else(|| sibling_protocol_hash(path));
        Ok(InitCenter {
            provenance: InitProvenance {
                file: name,
                weights_fnv1a: fnv1a_hex(&file.theta),
                protocol_hash,
                stage: file.stage.clone(),
                arena_protocol: file.arena_protocol.clone(),
                generation: file.generation,
                score: file.score,
                transfer_digest: Some(file.digest),
            },
            theta: file.theta,
        })
    }
}

/// A centre file written before P3-B carries no protocol hash of its own; its run's
/// `checkpoint.json` sits two directories up (`<run>/centers/genN-center.json`), so read
/// the hash from there when it is available rather than recording nothing.
fn sibling_protocol_hash(center: &std::path::Path) -> Option<u64> {
    let checkpoint = center.parent()?.parent()?.join("checkpoint.json");
    let bytes = std::fs::read(checkpoint).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("protocol_hash")?.as_u64()
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
            init_center: None,
            held_out: Vec::new(),
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
    /// Stage B's successor separation band. Ignored on Stage A.
    pub band: task::Band,
    /// A saved centre to start this run's weights from, with a fresh Adam state
    /// ([`InitCenter`]). `None` starts from the seeded initial centre.
    pub init_center: Option<InitCenter>,
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
    /// P5-C's mixed set and held-out checkpoints. `None` trains on the arena layouts
    /// alone, as every run before P5-C did.
    pub mix: Option<LandscapeMix>,
    /// Remote episode workers (`--remote`), connected and founding their fixtures
    /// before this run founds its own ([`super::remote`]). `None` runs local-only.
    pub remotes: Option<RemotePool>,
}

/// P5-C's per-generation landscapes (C1) and held-out checkpoints (C2). With a mix, the
/// arena half is [`task::p5_arena_layouts`] (both grids) rather than the standard grid.
pub struct LandscapeMix {
    /// Every training landscape fixture of the lineage ([`super::landscape::training_pool`]).
    pub pool: Vec<Prepared>,
    /// Landscapes drawn from `pool` per generation.
    pub per_generation: usize,
    /// The held-out fixtures ([`super::landscape::held_out_pool`]).
    pub held_out: Vec<Prepared>,
    /// Score the centre on `held_out` at update 0 and every this many updates.
    pub held_out_every: u32,
    /// C5: stop when the held-out score has fallen at every one of this many consecutive
    /// checkpoints (4 × 32 = 128 updates). Zero never stops.
    pub collapse_checkpoints: usize,
    /// Stop when the held-out best has stopped rising ([`Plateau`]). `None` never stops.
    pub plateau: Option<Plateau>,
}

/// The plateau stop (`--plateau N,g`): the run ends when the held-out best has not
/// risen by a relative `gain` over the last `checkpoints` checkpoints
/// ([`held_out_plateaued`]). Most of a run's gain arrives in its first ~100 updates
/// (P5-C: browser 0.80 / 1.02 / 1.04 / 1.165 at updates 0 / 32 / 96 / 512), so a run
/// that has stopped gaining stops paying for it. A stop rule only: not in the
/// protocol hash, and C5's fall rule still applies beside it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plateau {
    pub checkpoints: usize,
    pub gain: f64,
}

impl Plateau {
    /// The default for a `--p5` run: three checkpoints, two percent.
    pub const P5_DEFAULT: Plateau = Plateau {
        checkpoints: 3,
        gain: 0.02,
    };

    /// `N,g` (say `3,0.02`), or `off`.
    pub fn parse(s: &str) -> Result<Option<Plateau>, String> {
        let t = s.trim();
        if t.eq_ignore_ascii_case("off") || t.eq_ignore_ascii_case("none") {
            return Ok(None);
        }
        let bad =
            || format!("--plateau wants `N,g` (checkpoints, relative gain) or `off`, not `{t}`");
        let (n, g) = t.split_once(',').ok_or_else(bad)?;
        let checkpoints: usize = n.trim().parse().map_err(|_| bad())?;
        let gain: f64 = g.trim().parse().map_err(|_| bad())?;
        if checkpoints == 0 || !gain.is_finite() || gain < 0.0 {
            return Err(bad());
        }
        Ok(Some(Plateau { checkpoints, gain }))
    }
}

/// The plateau rule over the held-out scores so far, oldest first: with `b_k` the best
/// of the first `k + 1` checkpoints, the run has plateaued at the latest checkpoint `k`
/// when `k >= n` and `b_k < b_{k-n} + g · |b_{k-n}|` — over the last `n` checkpoints the
/// best has not risen by a relative `g`. A fall is no rise; `n = 0` never stops.
pub fn held_out_plateaued(scores: &[f64], n: usize, g: f64) -> bool {
    if n == 0 || scores.len() <= n {
        return false;
    }
    let best = |upto: usize| {
        scores[..=upto]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let k = scores.len() - 1;
    let then = best(k - n);
    best(k) < then + g * then.abs()
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
    /// C5: the held-out score fell at every one of the last checkpoints.
    Collapse,
    /// The held-out best stopped rising ([`Plateau`]).
    Plateau(Plateau),
}

impl std::fmt::Display for TrainStop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrainStop::Updates => write!(f, "completed every update"),
            TrainStop::Wall => write!(f, "stopped at the wall cap"),
            TrainStop::EpisodeLimit => write!(f, "stopped at the episode limit"),
            TrainStop::Collapse => write!(
                f,
                "STOPPED: the held-out score fell at every one of the last checkpoints (C5)"
            ),
            TrainStop::Plateau(p) => write!(
                f,
                "stopped at a plateau: the held-out best rose by less than {} over the last \
                 {} checkpoints",
                p.gain, p.checkpoints
            ),
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
    /// The held-out checkpoints, and the best of them: the ship candidate (C2).
    pub held_out: Vec<HeldOutRecord>,
    pub best_held_out: Option<HeldOutRecord>,
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
    let mut protocol = VoxelProtocol::new(
        spec.founder,
        spec.stage,
        spec.band,
        spec.pairs,
        spec.horizon,
        spec.train_seed,
        &task::TRAINING_LAYOUT_SEEDS[..spec.layouts],
    );
    let arena_half = task::arena_half(
        spec.founder,
        spec.stage,
        spec.band,
        spec.mix.is_some(),
        spec.layouts,
    );
    let arena_layouts = &arena_half[..];
    if let Some(mix) = &spec.mix {
        if mix.pool.is_empty() || mix.per_generation == 0 {
            return Err("a landscape mix needs a pool and a per-generation draw".into());
        }
        if mix.held_out.is_empty() || mix.held_out_every == 0 {
            return Err("a landscape mix needs held-out fixtures and a checkpoint interval".into());
        }
        let label = |p: &Prepared| match p.landscape() {
            Some(l) => format!("{}@{}", l.label(), l.world_seed),
            None => p.label(),
        };
        protocol.mix = Some(MixProtocol {
            landscape_protocol: super::landscape::LANDSCAPE_PROTOCOL.into(),
            landscape_horizon: super::landscape::lineage_horizon(spec.founder),
            landscape_live_plants: super::landscape::lineage_production(spec.founder),
            arena_grids: arena_layouts
                .iter()
                .map(|p| {
                    p.arena()
                        .map_or("?".into(), |a| a.grid.as_str().to_string())
                })
                .collect(),
            pool: mix.pool.iter().map(label).collect(),
            per_generation: mix.per_generation,
            draw: LANDSCAPE_DRAW.into(),
            held_out: mix.held_out.iter().map(label).collect(),
            held_out_every: mix.held_out_every,
        });
    }
    // One generation's fixtures: the arena half, then (with a mix) that generation's draw;
    // and each one's index in the list a remote founded (the arena half, then the pool).
    let generation_set = |generation: u32| -> (Vec<Prepared>, Vec<usize>) {
        let mut set = arena_layouts.to_vec();
        let mut ids: Vec<usize> = (0..arena_layouts.len()).collect();
        if let Some(mix) = &spec.mix {
            for i in super::landscape::generation_draw(
                spec.train_seed,
                generation,
                mix.pool.len(),
                mix.per_generation,
            ) {
                set.push(mix.pool[i].clone());
                ids.push(arena_layouts.len() + i);
            }
        }
        (set, ids)
    };
    // The remotes founded the same list for themselves while this machine founded its
    // own; each joins once its per-fixture summary matches this machine's, and is refused
    // loudly when it does not.
    if let Some(pool) = &spec.remotes {
        let pool_fixtures = spec.mix.iter().flat_map(|m| m.pool.iter());
        pool.verify(arena_layouts.iter().chain(pool_fixtures), pool.ready_wait());
    }
    let run_dir = spec.out.clone();
    std::fs::create_dir_all(run_dir.join("centers"))
        .map_err(|e| format!("cannot create {}: {e}", run_dir.display()))?;
    let checkpoint_path = run_dir.join("checkpoint.json");
    let mut cp = VoxelCheckpoint::fresh(protocol, crate::evaluate::BUILD_ID);
    if let Some(init) = &spec.init_center {
        // The warm start: the source centre's exact weights, this run's own protocol
        // hash, and a **fresh** Adam state — the source's moments belong to the
        // gradient of a different task and carrying them would be a silent migration.
        cp.theta.clone_from(&init.theta);
        cp.adam = Adam::new(cp.theta.len());
        cp.init_center = Some(init.provenance.clone());
        if let Some(digest) = init.provenance.transfer_digest {
            println!(
                "# TRANSFER start from {}: its weights were authored against manifest digest \
                 {digest:#018x}, this build's is {:#018x}; same length ({} parameters), \
                 taken as they are. Weights {:#018x}; Adam state fresh, this run's protocol \
                 {:#018x}",
                init.provenance.file,
                cp.protocol.digest,
                cp.theta.len(),
                init.provenance.weights_fnv1a,
                cp.protocol_hash,
            );
        }
        println!(
            "# warm start from {} (stage {}, arena {}, protocol {}), weights {:#018x}; \
             Adam state fresh, this run's protocol {:#018x}",
            init.provenance.file,
            init.provenance.stage,
            init.provenance.arena_protocol,
            init.provenance
                .protocol_hash
                .map_or("unrecorded".to_string(), |h| format!("{h:#018x}")),
            init.provenance.weights_fnv1a,
            cp.protocol_hash,
        );
    }
    cp.validate()
        .map_err(|e| format!("fresh checkpoint: {e}"))?;
    let mut score_spreads = Vec::new();
    let mut stop = TrainStop::Updates;

    // The held-out checkpoint (C2): the centre's exact weights, scored on the held-out
    // landscapes, written beside the per-generation centres.
    let held_out_checkpoint = |cp: &mut VoxelCheckpoint, updates: u32| -> Result<(), String> {
        let Some(mix) = &spec.mix else {
            return Ok(());
        };
        let t = Instant::now();
        let driver = center_driver(cp)?;
        let episodes = evaluate_fixtures(
            &driver,
            &mix.held_out,
            spec.workers,
            Limits::new(cancel),
            &format!("heldout/upd{updates}"),
        )?;
        let (score, by_preset, survived) = held_out_summary(&mix.held_out, &episodes);
        let file = format!("centers/upd{updates}-heldout.json");
        write_center_policy(&run_dir, &file, cp, &cp.theta, updates, score)?;
        let record = HeldOutRecord {
            updates,
            score,
            by_preset,
            survived,
            file,
            weights_fnv1a: fnv1a_hex(&cp.theta),
        };
        println!(
            "held-out upd {updates:>3}  score {score:.4}  survived {survived:.3}  {}  {:.1}s",
            record
                .by_preset
                .iter()
                .map(|(p, s)| format!("{p} {s:.4}"))
                .collect::<Vec<_>>()
                .join("  "),
            t.elapsed().as_secs_f64(),
        );
        cp.held_out.push(record);
        Ok(())
    };
    let collapsed = |cp: &VoxelCheckpoint| -> bool {
        spec.mix.as_ref().is_some_and(|mix| {
            let scores: Vec<f64> = cp.held_out.iter().map(|h| h.score).collect();
            held_out_collapsed(&scores, mix.collapse_checkpoints)
        })
    };
    let plateaued = |cp: &VoxelCheckpoint| -> Option<Plateau> {
        let mix = spec.mix.as_ref()?;
        let p = mix.plateau?;
        let scores: Vec<f64> = cp.held_out.iter().map(|h| h.score).collect();
        held_out_plateaued(&scores, p.checkpoints, p.gain).then_some(p)
    };

    // 0. The initial centre evaluation — generation 0's centre record. The plan's episode
    //    accounting starts here ("2,180 episodes including the initial centre").
    let initial = {
        let driver = center_driver(&cp)?;
        let mut scores = Vec::new();
        let mut attempted = 0u64;
        let mut ticks = 0u64;
        let (set, _) = generation_set(0);
        for (li, layout) in set.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            attempted += 1;
            match driver::run_prepared_seeded(
                layout,
                &driver,
                layout.horizon().unwrap_or(spec.horizon),
                Limits {
                    cancel,
                    deadline: Some(deadline),
                },
                &format!("gen0/center/seed{}", layout.layout_seed()),
                episode_seed(spec.train_seed, 0, li),
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
                        layout.layout_seed()
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
            write_center_policy(&run_dir, file, &cp, &cp.theta, 0, score)?;
            cp.centers.push(CenterRecord {
                generation: 0,
                score,
                file: file.to_string(),
                weights_fnv1a: fnv1a_hex(&cp.theta),
            });
        }
        mean
    };
    held_out_checkpoint(&mut cp, 0)?;
    save_checkpoint(&checkpoint_path, &cp)?;

    // 1. The updates.
    let mut cancelled_at: Option<String> = None;
    for generation in 0..spec.updates {
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            stop = TrainStop::Wall;
            break;
        }
        let (set, fixture_ids) = generation_set(generation);
        let next_generation_episodes =
            (2 * spec.pairs as u64 + u64::from(spec.evaluate_center)) * set.len() as u64;
        if cp.episodes_attempted + next_generation_episodes > spec.episode_limit {
            stop = TrainStop::EpisodeLimit;
            break;
        }
        // A remote that finished founding since the last generation joins here.
        if let Some(pool) = &spec.remotes {
            pool.admit();
        }
        let plan = GenerationPlan {
            layouts: &set,
            horizon: spec.horizon,
            workers: spec.workers,
            evaluate_center: spec.evaluate_center,
            deadline: Some(deadline),
            remote: spec.remotes.as_ref().map(|pool| RemotePlan {
                pool,
                fixture_ids: &fixture_ids,
            }),
        };
        // The centre a generation scores is the one it started from; the file written
        // for it holds exactly those weights, not the updated ones.
        let evaluated = cp.theta.clone();
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
                    write_center_policy(&run_dir, &file, &cp, &evaluated, generation, score)?;
                    cp.centers.push(CenterRecord {
                        generation,
                        score,
                        file,
                        weights_fnv1a: fnv1a_hex(&evaluated),
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
                if !report.shares.is_empty() {
                    println!(
                        "        {}{}",
                        report
                            .shares
                            .iter()
                            .map(|s| format!(
                                "{} {} units {} ep {} ticks {:.1} busy-s",
                                s.executor, s.units, s.episodes, s.ticks, s.busy_seconds
                            ))
                            .collect::<Vec<_>>()
                            .join(" | "),
                        if report.requeued > 0 {
                            format!(" | {} units re-queued", report.requeued)
                        } else {
                            String::new()
                        }
                    );
                }
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
        let updates = generation + 1;
        if let Some(mix) = &spec.mix
            && updates.is_multiple_of(mix.held_out_every)
        {
            held_out_checkpoint(&mut cp, updates)?;
            if collapsed(&cp) {
                save_checkpoint(&checkpoint_path, &cp)?;
                stop = TrainStop::Collapse;
                break;
            }
            if let Some(p) = plateaued(&cp) {
                save_checkpoint(&checkpoint_path, &cp)?;
                stop = TrainStop::Plateau(p);
                break;
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
    let best_held_out = cp
        .held_out
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
        held_out: cp.held_out.clone(),
        best_held_out,
    })
}

/// C5's stop rule: the held-out score fell at every one of the last `k` checkpoints
/// (`k + 1` scores, each below the one before). `k = 0` never stops.
pub fn held_out_collapsed(scores: &[f64], k: usize) -> bool {
    k > 0
        && scores.len() > k
        && scores[scores.len() - k - 1..]
            .windows(2)
            .all(|w| w[1] < w[0])
}

/// Run `driver` once on every fixture, on up to `workers` threads, each on its own seed
/// ([`driver::run_prepared`]), and return the episodes in fixture order. A cancelled or
/// invalid episode is an error: a partial evaluation is not a score.
pub fn evaluate_fixtures(
    driver: &EpisodeDriver,
    fixtures: &[Prepared],
    workers: usize,
    limits: Limits<'_>,
    tag: &str,
) -> Result<Vec<Episode>, String> {
    let slots: Mutex<Vec<Option<Episode>>> = Mutex::new(vec![None; fixtures.len()]);
    let failure: Mutex<Option<String>> = Mutex::new(None);
    let cursor = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1).min(fixtures.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    let Some(fixture) = fixtures.get(i) else {
                        return;
                    };
                    if failure.lock().expect("failure").is_some() {
                        return;
                    }
                    match driver::run_prepared(
                        fixture,
                        driver,
                        fixture.horizon().unwrap_or(task::STAGE_B_HORIZON_TICKS),
                        limits,
                        &format!("{tag}/{}", fixture.label()),
                    ) {
                        Ok(e) => slots.lock().expect("slots")[i] = Some(e),
                        Err(e) => {
                            *failure.lock().expect("failure") =
                                Some(format!("{tag}/{}: {e}", fixture.label()));
                            return;
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = failure.into_inner().expect("failure") {
        return Err(e);
    }
    Ok(slots
        .into_inner()
        .expect("slots")
        .into_iter()
        .map(|e| e.expect("every fixture ran"))
        .collect())
}

/// The held-out summary: mean score, mean score per preset (landscapes; an arena counts
/// under `arena`), and the mean survived fraction.
pub fn held_out_summary(
    fixtures: &[Prepared],
    episodes: &[Episode],
) -> (f64, Vec<(String, f64)>, f64) {
    let n = episodes.len().max(1) as f64;
    let score = episodes.iter().map(|e| e.score.score).sum::<f64>() / n;
    let survived = episodes.iter().map(Episode::survived_fraction).sum::<f64>() / n;
    let mut by: Vec<(String, f64, usize)> = Vec::new();
    for (f, e) in fixtures.iter().zip(episodes) {
        let key = f
            .landscape()
            .map_or_else(|| "arena".to_string(), |l| l.preset.clone());
        match by.iter_mut().find(|(k, _, _)| *k == key) {
            Some(slot) => {
                slot.1 += e.score.score;
                slot.2 += 1;
            }
            None => by.push((key, e.score.score, 1)),
        }
    }
    (
        score,
        by.into_iter().map(|(k, s, c)| (k, s / c as f64)).collect(),
        survived,
    )
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
    theta: &[f64],
    generation: u32,
    score: f64,
) -> Result<(), String> {
    let founder = cp.protocol.founder_parsed()?;
    let file = super::store::VoxelPolicyFile {
        schema: crate::es::voxel::store::POLICY_SCHEMA.into(),
        build: crate::evaluate::BUILD_ID.into(),
        founder: founder.name().into(),
        digest: cp.protocol.digest,
        train_seed: cp.train_seed,
        generation: Some(u64::from(generation)),
        score: Some(score),
        start_heading: cp.protocol.start_heading.clone(),
        starting_stores: cp.protocol.starting_stores.clone(),
        arena_protocol: cp.protocol.arena_protocol.clone(),
        protocol_hash: Some(cp.protocol_hash),
        imitation: None,
        stage: cp.protocol.stage.clone(),
        theta: theta.to_vec(),
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
            band: task::Band::Landed,
            init_center: None,
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
            mix: None,
            remotes: None,
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

    /// The warm start takes the source centre's exact weights and nothing else: a fresh
    /// Adam state, this run's own protocol hash, and the source recorded as provenance.
    /// The other founder's centre is refused by name rather than reshaped.
    #[test]
    fn a_warm_start_takes_the_weights_and_leaves_the_optimizer_state_behind() {
        let dir = std::env::temp_dir().join(format!("cubarium-voxel-init-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");

        let theta = super::super::super::tensor::initial_center_shape::<23, 3>(77);
        let source = crate::es::voxel::store::VoxelPolicyFile {
            schema: crate::es::voxel::store::POLICY_SCHEMA.into(),
            build: "test".into(),
            founder: Founder::Blind.name().into(),
            digest: voxel_schema_digest(Founder::Blind),
            train_seed: 1,
            generation: Some(504),
            score: Some(0.9),
            start_heading: task::START_HEADING_PROTOCOL.into(),
            starting_stores: task::STARTING_STORES_PROTOCOL.into(),
            arena_protocol: task::arena_protocol(Founder::Blind, Stage::A, task::Band::Landed),
            protocol_hash: Some(0xdead_beef),
            imitation: None,
            stage: Stage::A.as_str().into(),
            theta: theta.clone(),
        };
        let path = dir.join("gen504-center.json");
        source.write(&path).expect("written");

        // Another lineage's centre is refused, by name, before anything runs.
        let err = InitCenter::load(&path, Founder::Browser).expect_err("refused");
        assert!(
            err.contains("littershredder") && err.contains("frondgrazer"),
            "{err}"
        );

        let init = InitCenter::load(&path, Founder::Blind).expect("loaded");
        assert_eq!(init.theta, theta, "the exact weights, not a reseed");
        assert_eq!(init.provenance.protocol_hash, Some(0xdead_beef));
        assert_eq!(init.provenance.weights_fnv1a, fnv1a_hex(&theta));
        assert_eq!(init.provenance.generation, Some(504));
        assert_eq!(init.provenance.stage, "a");

        // A Stage-B near run started from that Stage-A centre: the weights carry over,
        // the Adam moments do not, and the run's protocol is its own.
        let protocol = VoxelProtocol::new(
            Founder::Blind,
            Stage::B,
            task::Band::Near,
            1,
            40,
            20_260_918,
            &task::TRAINING_LAYOUT_SEEDS[..1],
        );
        let mut cp = VoxelCheckpoint::fresh(protocol, "test");
        let cold = cp.theta.clone();
        assert_ne!(cold, theta, "the seeded centre is not the source centre");
        cp.theta.clone_from(&init.theta);
        cp.adam = Adam::new(cp.theta.len());
        cp.init_center = Some(init.provenance.clone());
        assert_eq!(cp.theta, theta);
        assert_eq!(cp.adam.step, 0);
        assert!(cp.adam.m.iter().all(|x| *x == 0.0) && cp.adam.v.iter().all(|x| *x == 0.0));
        assert_ne!(cp.protocol_hash, 0xdead_beef, "the run hashes as itself");
        assert_eq!(
            cp.protocol.arena_protocol,
            task::arena_protocol(Founder::Blind, Stage::B, task::Band::Near)
        );
        cp.validate().expect("a warm-started checkpoint is valid");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The two bands are two protocols: the same founder, stage, pairs, horizon, seed
    /// and layouts hash differently, so a near checkpoint cannot be resumed as a landed
    /// one.
    #[test]
    fn the_separation_band_is_in_the_protocol_hash() {
        let of = |band| {
            VoxelProtocol::new(
                Founder::Browser,
                Stage::B,
                band,
                32,
                2_400,
                20_260_918,
                &task::TRAINING_LAYOUT_SEEDS,
            )
        };
        let near = of(task::Band::Near);
        let landed = of(task::Band::Landed);
        assert_ne!(near.hash(), landed.hash());
        assert_eq!(near.successor_band, "near");
        assert_eq!(landed.successor_band, "landed");
        // Stage A has no successor, so the flag cannot move its hash.
        let a = |band| {
            VoxelProtocol::new(
                Founder::Browser,
                Stage::A,
                band,
                32,
                1_200,
                20_260_918,
                &task::TRAINING_LAYOUT_SEEDS,
            )
        };
        assert_eq!(a(task::Band::Near).hash(), a(task::Band::Landed).hash());
    }

    /// A pre-cancelled generation dispatches no episodes and leaves optimizer state alone.
    #[test]
    fn a_cancelled_generation_leaves_the_centre_alone() {
        let cancel = AtomicBool::new(true);
        let founder = Founder::Blind;
        let protocol = VoxelProtocol::new(
            founder,
            Stage::A,
            task::Band::Landed,
            1,
            120,
            20_260_918,
            &task::TRAINING_LAYOUT_SEEDS[..1],
        );
        let layouts = task::training_layouts(founder, Stage::A, task::Band::Landed);
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
            remote: None,
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
            task::Band::Landed,
            1,
            40,
            20_260_918,
            &task::TRAINING_LAYOUT_SEEDS[..1],
        );
        let layouts = task::training_layouts(founder, Stage::A, task::Band::Landed);
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
                remote: None,
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
    #[test]
    fn the_collapse_rule_needs_every_one_of_the_last_checkpoints_to_fall() {
        assert!(held_out_collapsed(&[0.5, 0.4, 0.3, 0.2, 0.1], 4));
        assert!(held_out_collapsed(&[0.9, 0.5, 0.4, 0.3, 0.2, 0.1], 4));
        assert!(
            !held_out_collapsed(&[0.5, 0.4, 0.3, 0.2], 4),
            "only 96 updates"
        );
        assert!(
            !held_out_collapsed(&[0.5, 0.4, 0.45, 0.2, 0.1], 4),
            "one rise resets"
        );
        assert!(
            !held_out_collapsed(&[0.5, 0.4, 0.4, 0.3, 0.2], 4),
            "a tie is not a fall"
        );
        assert!(
            !held_out_collapsed(&[0.5, 0.4, 0.3, 0.2, 0.1], 0),
            "k = 0 never stops"
        );
    }

    fn tempfile_guard() -> PathBuf {
        let dir = PathBuf::from(format!(
            "/tmp/opencode/cubarium-voxel-es-test-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    /// A blind centre file authored against `digest`, holding `theta`.
    fn centre_file(dir: &std::path::Path, name: &str, digest: u64, theta: &[f64]) -> PathBuf {
        let file = crate::es::voxel::store::VoxelPolicyFile {
            schema: crate::es::voxel::store::POLICY_SCHEMA.into(),
            build: "test".into(),
            founder: Founder::Blind.name().into(),
            digest,
            train_seed: 1,
            generation: Some(288),
            score: Some(1.8),
            start_heading: task::START_HEADING_PROTOCOL.into(),
            starting_stores: task::STARTING_STORES_PROTOCOL.into(),
            arena_protocol: task::arena_protocol(Founder::Blind, Stage::B, task::Band::Landed),
            protocol_hash: Some(0xfeed),
            imitation: None,
            stage: Stage::B.as_str().into(),
            theta: theta.to_vec(),
        };
        let path = dir.join(name);
        file.write(&path).expect("written");
        path
    }

    /// Package S item 1: a transfer start takes a centre whose digest is another
    /// manifest's but whose vector is this manifest's length, and records the source
    /// file and its digest in the run's provenance; a wrong-length centre is refused by
    /// name; `--init-center` stays strict about the digest.
    #[test]
    fn a_transfer_start_takes_a_same_length_centre_across_a_digest_change() {
        let dir = std::env::temp_dir().join(format!("cubarium-transfer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let digest = voxel_schema_digest(Founder::Blind);
        let other = digest ^ 0x5a5a;
        let theta = super::super::super::tensor::initial_center_shape::<23, 3>(77);
        let moved = centre_file(&dir, "moved.json", other, &theta);

        // `--init-center` refuses the digest mismatch, as it always did.
        let strict = InitCenter::load(&moved, Founder::Blind).expect_err("strict");
        assert!(strict.contains("digest"), "{strict}");

        // The transfer takes it, exactly, and says where it came from.
        let init = InitCenter::transfer(&moved, Founder::Blind).expect("same length");
        assert_eq!(init.theta, theta, "the exact weights");
        assert_eq!(init.provenance.file, moved.display().to_string());
        assert_eq!(init.provenance.transfer_digest, Some(other));
        assert_eq!(init.provenance.weights_fnv1a, fnv1a_hex(&theta));

        // A run started from it records the source file and digest in its checkpoint,
        // under this build's own digest and protocol.
        let out = dir.join("run");
        let spec = TrainSpec {
            out: out.clone(),
            init_center: Some(init),
            ..smoke_spec()
        };
        let cancel = AtomicBool::new(false);
        train(&spec, &cancel).expect("the transferred run trains");
        let cp = load_checkpoint(&out.join("checkpoint.json"), None).expect("valid checkpoint");
        let recorded = cp.init_center.as_ref().expect("provenance recorded");
        assert_eq!(recorded.file, moved.display().to_string());
        assert_eq!(recorded.transfer_digest, Some(other));
        assert_eq!(cp.protocol.digest, digest, "the run is this manifest's");

        // A centre one weight short is refused by name, before anything runs.
        let short = centre_file(&dir, "short.json", other, &theta[..theta.len() - 1]);
        let err = InitCenter::transfer(&short, Founder::Blind).expect_err("wrong length");
        assert!(
            err.contains("short.json")
                && err.contains(&format!("{}", theta.len() - 1))
                && err.contains(&format!("{}", theta.len())),
            "{err}"
        );
        // Another lineage's centre is still another lineage's.
        assert!(InitCenter::transfer(&moved, Founder::Browser).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Cache study C: longest first, then fixture-major, every unit once, pairs whole.
    #[test]
    fn units_queue_longest_first_then_fixture_major() {
        let (pairs, layouts) = (3usize, 4usize);
        let units: Vec<Unit> = (0..pairs)
            .flat_map(|p| {
                (0..layouts).map(move |layout| Unit {
                    pair: Some(p),
                    layout,
                })
            })
            .chain((0..layouts).map(|layout| Unit { pair: None, layout }))
            .collect();
        // Layouts 2 and 3 are landscapes (longer), 0 and 1 arenas.
        let horizon = |layout: usize| if layout >= 2 { 4_800 } else { 2_400 };
        let order = dispatch_order(&units, horizon);
        let mut seen = order.clone();
        seen.sort_unstable();
        assert_eq!(
            seen,
            (0..units.len()).collect::<Vec<_>>(),
            "every unit once"
        );
        let cost = |u: usize| horizon(units[u].layout) * units[u].candidates().len() as u64;
        assert!(
            order.windows(2).all(|w| cost(w[0]) >= cost(w[1])),
            "longest first"
        );
        // Within one cost, one fixture's units run together.
        for w in order.windows(2) {
            if cost(w[0]) == cost(w[1]) {
                assert!(units[w[0]].layout <= units[w[1]].layout, "fixture-major");
            }
        }
        let head: Vec<usize> = order[..pairs].iter().map(|&u| units[u].layout).collect();
        assert_eq!(
            head,
            vec![2; pairs],
            "every pair on the first landscape first"
        );
        assert!(
            order
                .iter()
                .all(|&u| units[u].candidates().len() == 1 + usize::from(units[u].pair.is_some())),
            "a pair unit carries both signs"
        );
    }

    /// Package S item 2: the plateau rule stops when the held-out best has not risen by
    /// a relative `g` over the last `N` checkpoints, and never on a rising series.
    #[test]
    fn the_plateau_rule_stops_a_flat_series_and_not_a_rising_one() {
        // P5-C's browser shape: most of the gain by the first checkpoints, then a creep.
        let flat = [0.80, 1.02, 1.03, 1.035, 1.04];
        assert!(
            !held_out_plateaued(&flat[..4], 3, 0.02),
            "too few checkpoints"
        );
        assert!(held_out_plateaued(&flat, 3, 0.02), "1.04 < 1.02 · 1.02");
        let rising = [0.80, 0.90, 1.00, 1.10, 1.20, 1.30, 1.40];
        for k in 1..=rising.len() {
            assert!(!held_out_plateaued(&rising[..k], 3, 0.02), "rising at {k}");
        }
        // A fall after the best is still no rise; a late jump past the bar resets it.
        assert!(held_out_plateaued(&[1.0, 1.5, 1.4, 1.3, 1.2], 3, 0.02));
        assert!(!held_out_plateaued(&[1.0, 1.5, 1.4, 1.3, 1.6], 3, 0.02));
        // Negative scores rise towards zero.
        assert!(!held_out_plateaued(&[-1.0, -0.9, -0.8, -0.7], 3, 0.02));
        assert!(held_out_plateaued(&[-1.0, -1.0, -0.995, -0.99], 3, 0.02));
        // Zero checkpoints never stops.
        assert!(!held_out_plateaued(&flat, 0, 0.02));

        assert_eq!(
            Plateau::parse("3,0.02").expect("parses"),
            Some(Plateau {
                checkpoints: 3,
                gain: 0.02
            })
        );
        assert_eq!(Plateau::parse("off").expect("off"), None);
        assert!(Plateau::parse("3").is_err());
        assert!(Plateau::parse("0,0.02").is_err());
        assert!(Plateau::parse("3,-0.1").is_err());
        assert_eq!(
            Some(Plateau::P5_DEFAULT),
            Plateau::parse("3,0.02").expect("ok")
        );
    }
}
