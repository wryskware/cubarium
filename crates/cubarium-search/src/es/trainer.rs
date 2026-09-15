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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_core::neural::Policy;
use serde::{Deserialize, Serialize};

use super::episode::{self, Detail, Driver, Episode};
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
}

impl Protocol {
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
    assert!(!episodes.is_empty(), "a candidate is scored on at least one layout");
    let t_min = episodes.iter().map(|e| e.ticks).min().expect("nonempty") as f64;
    let stores: f64 = episodes.iter().map(Episode::normalized_stores).sum::<f64>()
        / episodes.len() as f64;
    t_min + STORE_WEIGHT * stores
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
    /// Every completed generation's centre score, when the centre was evaluated.
    pub center_scores: Vec<(u64, f64)>,
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
            center_scores: Vec::new(),
        }
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
pub struct Plan<'a> {
    pub layouts: &'a [Layout],
    pub horizon: u64,
    pub workers: usize,
    pub detail: Detail,
    pub evaluate_center: bool,
    pub deadline: Option<Instant>,
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
) -> Result<GenerationReport, episode::Cancelled> {
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
    let slots: Mutex<Vec<Option<Episode>>> = Mutex::new(vec![None; jobs_total]);
    let cursor = AtomicUsize::new(0);
    let workers = plan.workers.max(1).min(jobs_total);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = cursor.fetch_add(1, Ordering::SeqCst);
                    if index >= jobs_total {
                        return;
                    }
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    if plan.deadline.is_some_and(|d| Instant::now() >= d) {
                        cancel.store(true, Ordering::Relaxed);
                        return;
                    }
                    let layout = &layouts[index % layouts.len()];
                    let policy = &policies[index / layouts.len()];
                    let driver = Driver::Policy(Box::new(policy.clone()));
                    match episode::run(layout, &driver, plan.horizon, plan.detail, cancel) {
                        Ok(e) => slots.lock().expect("slot mutex")[index] = Some(e),
                        Err(episode::Cancelled) => return,
                    }
                }
            });
        }
    });

    let slots = slots.into_inner().expect("slot mutex");
    if slots.iter().any(Option::is_none) {
        return Err(episode::Cancelled);
    }
    let episodes: Vec<Episode> = slots.into_iter().map(|s| s.expect("checked")).collect();

    // 3. Reduce in the fixed index order. Scheduling cannot reach this.
    let mut jobs = Vec::with_capacity(jobs_total);
    let mut scores = vec![0.0; candidates.len()];
    for (ci, c) in candidates.iter().enumerate() {
        let slice = &episodes[ci * layouts.len()..(ci + 1) * layouts.len()];
        scores[ci] = score(slice);
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
            upkeep_paid: 0.0,
            motion_paid: 0.0,
            store_start: 0.0,
            travelled_px: 0.0,
            body_lengths: 0.0,
            distinct_cells: 0,
            ticks_in_opening: 0,
            turn_sweep_rad: 0.0,
            route_p_start: 0.0,
            route_p_end: 0.0,
            route_p_grown: 0.0,
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
                detail: Detail::Full,
                evaluate_center: false,
                deadline: None,
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
            detail: Detail::Score,
            evaluate_center: false,
            deadline: None,
        };
        let out = run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel);
        assert_eq!(out.err(), Some(episode::Cancelled));
        assert_eq!(theta, before);
        assert_eq!(adam.step, 0);
    }

    #[test]
    fn a_checkpoint_round_trips_and_names_its_protocol() {
        let cp = Checkpoint::fresh(smoke_protocol(), 20_260_915, "test");
        cp.validate().expect("fresh checkpoint is valid");
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
            detail: Detail::Score,
            evaluate_center: false,
            deadline: None,
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
