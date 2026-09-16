//! Workstream X: **the turn deadband alone** — a paired replay of one frozen weight set under
//! both action adapters, and the gradient-direction stability of the retained sixteen pairs
//! (`design/handoffs/ecology-v1-turn-deadband-opus-2026-09-16.md`).
//!
//! Two experiments, neither of which trains anything.
//!
//! # 1. The replay
//!
//! Generation 9's centre and its own `2n` candidates, on their **own** training layouts, with
//! the weights untouched and only the adapter changed. `tensor::policy_in` restamps a theta for
//! the other adapter's schema digest and changes nothing else, and `Layout::with_adapter` puts
//! the world on the same adapter, so the two arms of a pair differ in exactly one constant: the
//! deadband width on the `TURN` channel.
//!
//! Per trajectory — one `(candidate, layout)` episode — the replay reports turn activity (the
//! fraction of measurable ticks with a non-zero resolved turn, and mean `|ω|`), on-food
//! fraction, dwell bouts, and the ticks survived; per candidate it reports `t_min` and the
//! protocol's own score. Every one of those numbers comes from [`super::episode::run_traced`],
//! which is the trainer's own rollout with the world's own per-tick intake trace switched on.
//!
//! **The decision rule is Astra's, stated before the replay was run** (and recorded in
//! `design/7_Research/ecology-v1-turn-deadband-2026-09-16.md` before the numbers existed):
//!
//! - a metric **rises** when the exact two-sided sign test on its paired differences gives
//!   `p ≤ 0.1` *and* strictly more than half of the non-zero differences are positive;
//! - turn release is **falsified as the bottleneck** if turn activity rises while on-food
//!   fraction, mean dwell bout and `t_min` all fail to (`p > 0.1` on all three);
//! - it is **supported** if on-food fraction or `t_min` rises;
//! - anything else is **mixed**, and the report says so rather than choosing.
//!
//! The two branches are disjoint by construction: "supported" needs `p ≤ 0.1` on a metric that
//! "falsified" needs `p > 0.1` on.
//!
//! # 2. Gradient-direction stability
//!
//! Astra's cheap check, from the **retained** sixteen pair contributions rather than from any
//! new simulation: `runs/ecology-v1-es-antithetic/pairs.json`'s `weight` column for generation
//! 9 (workstream Q's `u_plus − u_minus`), with the perturbations regenerated from the run's own
//! seed by `rng::perturbation`. The direction a subset `S` of pairs estimates is
//! `Σ_{i∈S} wᵢ εᵢ`; the whole estimate is that sum over all sixteen, and the gradient the
//! trainer applied is the same vector divided by the positive constant `2nσ`, which no cosine
//! can see.
//!
//! - **Split-half**: every balanced 8/8 split of the sixteen pairs — all 6,435 of them, not a
//!   sample — and the cosine between the two halves' directions.
//! - **Bootstrap**: 1,000 resamples of the sixteen pairs with replacement, and each resampled
//!   direction's cosine with the whole estimate and with the **recorded update**
//!   `θ₁₀ − θ₉`, read from the run's own centre files.
//!
//! Both are computed exactly from the 16×16 Gram matrix `Gᵢⱼ = wᵢwⱼ⟨εᵢ, εⱼ⟩` rather than by
//! summing 10,215-vectors per sample, so "all 6,435 splits" costs less than a hundred of them
//! would have.
//!
//! # What neither establishes
//!
//! One run, one ecology, one seed, one generation, four training layouts, one horizon. The
//! replay says what these 33 weight sets did on these four patches under the two adapters. It
//! is not a training result and does not say what a policy *trained* under `cub-act-2` would
//! do; that is deliverable 3's question, and it is gated on this one.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cubarium_core::neural::ActionAdapter;
use serde::{Deserialize, Serialize};

use super::antithetic::PairRow;
use super::episode::{self, Driver, Episode, Limits, Trajectory};
use super::fixture::{self, Ecology, Layout};
use super::rng::perturbation;
use super::tensor;
use super::trainer::{Candidate, Checkpoint, score_by};
use crate::evaluate::BUILD_ID;

type Boxed = Box<dyn std::error::Error>;

/// The significance the brief's rule is read at. Not a tuning knob: it is Astra's `0.1`, and it
/// is written down here so the rule and the code cannot disagree.
pub const ALPHA: f64 = 0.1;

// -------------------------------------------------------------------------------------------
// The exact sign test.
// -------------------------------------------------------------------------------------------

/// An exact two-sided sign test on paired differences.
///
/// Zero differences are **ties**: they are dropped, and `n` is the number that remain, which is
/// what an exact sign test's null distribution is over. `p_value` is
/// `min(1, 2·min(P(X ≤ k), P(X ≥ k)))` for `X ~ Binomial(n, ½)` and `k` positives, computed by
/// exact summation of binomial terms in `f64` — `n` here is at most a few hundred, so the terms
/// are formed in log space to keep the sum from underflowing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignTest {
    pub positive: usize,
    pub negative: usize,
    pub ties: usize,
    pub p_value: f64,
    /// Median of the non-zero differences, for size rather than direction.
    pub median_difference: f64,
    /// Mean of **all** the differences, ties included.
    pub mean_difference: f64,
}

impl SignTest {
    pub fn n(&self) -> usize {
        self.positive + self.negative
    }

    /// The brief's "rises": significant at [`ALPHA`] **and** pointing up.
    pub fn rises(&self) -> bool {
        self.p_value <= ALPHA && self.positive > self.negative
    }

    /// Significant at [`ALPHA`] and pointing down.
    pub fn falls(&self) -> bool {
        self.p_value <= ALPHA && self.negative > self.positive
    }
}

/// [`SignTest`] over a set of paired differences. A difference that is not finite is a tie:
/// there is no direction in it.
pub fn sign_test(differences: &[f64]) -> SignTest {
    let positive = differences.iter().filter(|d| d.is_finite() && **d > 0.0).count();
    let negative = differences.iter().filter(|d| d.is_finite() && **d < 0.0).count();
    let ties = differences.len() - positive - negative;
    let n = positive + negative;
    let p_value = if n == 0 { 1.0 } else { two_sided_binomial(positive, n) };
    let mut nonzero: Vec<f64> =
        differences.iter().copied().filter(|d| d.is_finite() && *d != 0.0).collect();
    nonzero.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let median_difference = median(&nonzero);
    let finite: Vec<f64> = differences.iter().copied().filter(|d| d.is_finite()).collect();
    let mean_difference = if finite.is_empty() {
        0.0
    } else {
        finite.iter().sum::<f64>() / finite.len() as f64
    };
    SignTest { positive, negative, ties, p_value, median_difference, mean_difference }
}

fn median(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let m = sorted.len();
    if m % 2 == 1 { sorted[m / 2] } else { 0.5 * (sorted[m / 2 - 1] + sorted[m / 2]) }
}

/// `min(1, 2·min(P(X ≤ k), P(X ≥ k)))` for `X ~ Binomial(n, ½)`.
pub fn two_sided_binomial(k: usize, n: usize) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let lower: f64 = (0..=k.min(n)).map(|i| binomial_pmf_half(i, n)).sum();
    let upper: f64 = (k.min(n)..=n).map(|i| binomial_pmf_half(i, n)).sum();
    (2.0 * lower.min(upper)).min(1.0)
}

/// `C(n, i) / 2ⁿ`, through `ln Γ` so a large `n` neither overflows the binomial coefficient nor
/// underflows the power of two before they are combined.
fn binomial_pmf_half(i: usize, n: usize) -> f64 {
    let ln_c = ln_gamma((n + 1) as f64) - ln_gamma((i + 1) as f64) - ln_gamma((n - i + 1) as f64);
    (ln_c - (n as f64) * std::f64::consts::LN_2).exp()
}

/// Lanczos `ln Γ(x)` for `x > 0`, accurate to about 1e-13 in this range — far finer than any
/// decision at `p = 0.1`.
fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_93,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        // Reflection; not reached by this module's integer arguments, kept so the helper is
        // total rather than conditionally correct.
        return (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

// -------------------------------------------------------------------------------------------
// Deliverable 2: the paired replay.
// -------------------------------------------------------------------------------------------

/// One `(candidate, layout, adapter)` episode, reduced to the columns the rule reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReplayRow {
    pub candidate: String,
    pub layout: String,
    pub adapter: String,
    pub ticks: u64,
    pub alive: bool,
    /// The two fields the protocol's own `score_by` reads besides the survival ticks, carried
    /// so the replayed score is the **protocol's** score and not the survival term alone.
    pub terminal_stores: f64,
    pub store_capacity: f64,
    pub turn_measured_ticks: u64,
    pub turn_active_ticks: u64,
    pub turn_active_fraction: f64,
    pub mean_abs_omega: f64,
    pub traced_ticks: u64,
    pub on_food_ticks: u64,
    pub on_food_fraction: f64,
    pub dwell_bouts: u64,
    pub mean_dwell_bout: f64,
    pub dwell_bout_max: u64,
    pub intake_producer: f64,
    pub ticks_in_opening: u64,
    pub turn_sweep_rad: f64,
    pub trace_rows_dropped: u64,
}

impl ReplayRow {
    fn of(candidate: &str, adapter: ActionAdapter, e: &Episode, t: &Trajectory) -> ReplayRow {
        ReplayRow {
            candidate: candidate.to_string(),
            layout: e.layout.clone(),
            adapter: adapter.name().to_string(),
            ticks: e.ticks,
            alive: e.alive,
            terminal_stores: e.terminal_stores,
            store_capacity: e.store_capacity,
            turn_measured_ticks: t.turn_measured_ticks,
            turn_active_ticks: t.turn_active_ticks,
            turn_active_fraction: t.turn_active_fraction().unwrap_or(0.0),
            mean_abs_omega: t.mean_abs_omega().unwrap_or(0.0),
            traced_ticks: t.traced_ticks,
            on_food_ticks: t.on_food_ticks,
            on_food_fraction: t.on_food_fraction().unwrap_or(0.0),
            dwell_bouts: t.dwell_bouts,
            mean_dwell_bout: t.mean_dwell_bout().unwrap_or(0.0),
            dwell_bout_max: t.dwell_bout_max,
            intake_producer: e.intake_producer,
            ticks_in_opening: e.ticks_in_opening,
            turn_sweep_rad: e.turn_sweep_rad,
            trace_rows_dropped: t.trace_rows_dropped,
        }
    }
}

/// One trajectory's `cub-act-2 − cub-act-1` differences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairedTrajectory {
    pub candidate: String,
    pub layout: String,
    pub d_turn_active_fraction: f64,
    pub d_mean_abs_omega: f64,
    pub d_on_food_fraction: f64,
    pub d_mean_dwell_bout: f64,
    pub d_ticks: f64,
    pub d_intake_producer: f64,
    pub d_opening_fraction: f64,
}

/// One candidate's `cub-act-2 − cub-act-1` differences, over its four layouts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairedCandidate {
    pub candidate: String,
    pub t_min_one: u64,
    pub t_min_two: u64,
    pub d_t_min: f64,
    pub score_one: f64,
    pub score_two: f64,
    pub d_score: f64,
}

/// A named sign test, so the report reads without the rule being reconstructed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedTest {
    pub metric: String,
    /// `trajectory` (one per candidate-layout) or `candidate` (one per weight set).
    pub unit: String,
    pub test: SignTest,
    pub rises: bool,
    pub falls: bool,
}

fn named(metric: &str, unit: &str, differences: &[f64]) -> NamedTest {
    let test = sign_test(differences);
    NamedTest {
        metric: metric.to_string(),
        unit: unit.to_string(),
        rises: test.rises(),
        falls: test.falls(),
        test,
    }
}

/// Everything deliverable 2 produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayReport {
    pub build: String,
    pub run: String,
    pub generation: u64,
    pub config: String,
    pub config_hash: String,
    pub protocol_hash: u64,
    pub train_seed: u64,
    pub sigma: f64,
    pub pairs: usize,
    pub aggregate: String,
    pub motor: String,
    pub horizon_ticks: u64,
    pub layouts: Vec<String>,
    /// The two schema digests the arms ran under. They must differ, and the `cub-act-1` one
    /// must be the digest the retained centre file carries.
    pub digest_cub_act_1: u64,
    pub digest_cub_act_2: u64,
    pub centre_file_digest: u64,
    pub alpha: f64,
    /// The rule, in the report, in the words it was registered in.
    pub decision_rule: String,
    pub tests: Vec<NamedTest>,
    pub verdict: String,
    pub verdict_detail: String,
    pub wall_seconds: f64,
    pub rows: Vec<ReplayRow>,
    pub paired_trajectories: Vec<PairedTrajectory>,
    pub paired_candidates: Vec<PairedCandidate>,
}

const RULE: &str = "A metric RISES when the exact two-sided sign test on its paired \
    (cub-act-2 minus cub-act-1) differences gives p <= 0.1 and strictly more than half of the \
    non-zero differences are positive. Turn release is FALSIFIED as the bottleneck if turn \
    activity rises while on-food fraction, mean dwell bout and t_min all fail to (p > 0.1 on \
    all three). It is SUPPORTED if on-food fraction or t_min rises. Anything else is MIXED.";

/// Deliverable 2: replay one generation's centre and candidates under both adapters.
#[allow(clippy::too_many_arguments)]
pub fn run_replay(
    run: &Path,
    generation: u64,
    config: &Path,
    horizon: u64,
    workers: usize,
    wall_seconds: u64,
    out: &Path,
) -> Result<ReplayReport, Boxed> {
    let started = Instant::now();
    let eco = Ecology::load(config)?;
    let checkpoint: Checkpoint =
        serde_json::from_str(&std::fs::read_to_string(run.join("checkpoint.json"))?)?;
    let file: super::export::PolicyFile = serde_json::from_str(&std::fs::read_to_string(
        run.join(format!("centers/center-{generation:05}.json")),
    )?)?;
    file.check_ecology(&eco)?;
    file.check_motor(checkpoint.protocol.motor)?;
    // The retained run is `cub-act-1`'s; replaying it under `cub-act-2` is exactly the
    // experiment, so this is the one place the adapter check is deliberately not made.
    file.check_adapter(ActionAdapter::CubAct1)?;
    if file.generation != generation {
        return Err(Boxed::from(format!("that file is generation {}", file.generation)));
    }
    let theta = file.theta.clone();
    let n = checkpoint.protocol.pairs;
    let sigma = checkpoint.protocol.sigma;

    // Every candidate's theta, in the trainer's own candidate order, built the trainer's own
    // way: one epsilon per pair, regenerated from its position.
    let mut eps = vec![0.0; theta.len()];
    let mut thetas: Vec<(String, Vec<f64>)> = Vec::with_capacity(2 * n + 1);
    thetas.push((Candidate::Center.label(), theta.clone()));
    for p in 0..n {
        perturbation(checkpoint.train_seed, generation, p as u64, &mut eps);
        for (c, sign) in [(Candidate::Plus(p), 1.0f64), (Candidate::Minus(p), -1.0)] {
            thetas.push((
                c.label(),
                theta.iter().zip(&eps).map(|(t, e)| t + sign * sigma * e).collect(),
            ));
        }
    }

    // One job per (adapter, candidate, layout). The two arms share one queue so a slow layout
    // cannot bias one adapter's wall time against the other's.
    let mut jobs: Vec<(ActionAdapter, usize, usize)> = Vec::new();
    for adapter in [ActionAdapter::CubAct1, ActionAdapter::CubAct2] {
        for c in 0..thetas.len() {
            for l in 0..fixture::training_layouts_on(&eco).len() {
                jobs.push((adapter, c, l));
            }
        }
    }
    let layouts_of = |adapter: ActionAdapter| -> Vec<Layout> {
        fixture::training_layouts_on(&eco)
            .into_iter()
            .map(|l| l.with_motor(checkpoint.protocol.motor).with_adapter(adapter))
            .collect()
    };
    let one = layouts_of(ActionAdapter::CubAct1);
    let two = layouts_of(ActionAdapter::CubAct2);

    let cursor = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, ReplayRow)>> = Mutex::new(Vec::new());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let cancel = AtomicBool::new(false);
    let limits = Limits::until(&cancel, started + Duration::from_secs(wall_seconds));
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let next = cursor.fetch_add(1, Ordering::Relaxed);
                    let Some((adapter, c, l)) = jobs.get(next).copied() else { return };
                    let layout =
                        if adapter == ActionAdapter::CubAct1 { &one[l] } else { &two[l] };
                    let (label, theta_c) = &thetas[c];
                    let policy = match tensor::policy_in(theta_c, adapter) {
                        Ok(p) => p,
                        Err(e) => {
                            failures.lock().expect("f").push(e);
                            continue;
                        }
                    };
                    let job = format!("turnband/{}/{label}/{}", adapter.name(), layout.name);
                    match episode::run_traced(
                        layout,
                        &Driver::Policy(Box::new(policy)),
                        horizon,
                        limits,
                        &job,
                    ) {
                        Ok((e, t)) => done
                            .lock()
                            .expect("d")
                            .push((next, ReplayRow::of(label, adapter, &e, &t))),
                        Err(e) => failures.lock().expect("f").push(format!("{job}: {e}")),
                    }
                }
            });
        }
    });
    let failures = failures.into_inner().expect("f");
    if !failures.is_empty() {
        return Err(Boxed::from(format!(
            "{} episode(s) failed: {}",
            failures.len(),
            failures.iter().take(4).cloned().collect::<Vec<_>>().join("; ")
        )));
    }
    let mut done = done.into_inner().expect("d");
    // Back into job order, so the report is a property of the experiment and not of the
    // scheduling.
    done.sort_by_key(|(i, _)| *i);
    let rows: Vec<ReplayRow> = done.into_iter().map(|(_, r)| r).collect();
    if let Some(bad) = rows.iter().find(|r| r.trace_rows_dropped > 0) {
        return Err(Boxed::from(format!(
            "the intake recorder dropped {} rows on {}/{}: the on-food columns would be \
             incomplete",
            bad.trace_rows_dropped, bad.candidate, bad.layout
        )));
    }

    let report = reduce_replay(
        rows,
        &thetas,
        &checkpoint,
        &eco,
        &file,
        run,
        config,
        generation,
        horizon,
        started.elapsed().as_secs_f64(),
    );
    write_json(out, &report)?;
    print_replay(&report);
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn reduce_replay(
    rows: Vec<ReplayRow>,
    thetas: &[(String, Vec<f64>)],
    checkpoint: &Checkpoint,
    eco: &Ecology,
    file: &super::export::PolicyFile,
    run: &Path,
    config: &Path,
    generation: u64,
    horizon: u64,
    wall: f64,
) -> ReplayReport {
    let find = |adapter: &str, candidate: &str, layout: &str| -> &ReplayRow {
        rows.iter()
            .find(|r| r.adapter == adapter && r.candidate == candidate && r.layout == layout)
            .expect("every (adapter, candidate, layout) ran")
    };
    let layouts: Vec<String> = {
        let mut v: Vec<String> = Vec::new();
        for r in &rows {
            if !v.contains(&r.layout) {
                v.push(r.layout.clone());
            }
        }
        v
    };

    let mut paired_trajectories = Vec::new();
    for (label, _) in thetas {
        for layout in &layouts {
            let a = find(ActionAdapter::CubAct1.name(), label, layout);
            let b = find(ActionAdapter::CubAct2.name(), label, layout);
            let opening =
                |r: &ReplayRow| if r.ticks == 0 { 0.0 } else { r.ticks_in_opening as f64 / r.ticks as f64 };
            paired_trajectories.push(PairedTrajectory {
                candidate: label.clone(),
                layout: layout.clone(),
                d_turn_active_fraction: b.turn_active_fraction - a.turn_active_fraction,
                d_mean_abs_omega: b.mean_abs_omega - a.mean_abs_omega,
                d_on_food_fraction: b.on_food_fraction - a.on_food_fraction,
                d_mean_dwell_bout: b.mean_dwell_bout - a.mean_dwell_bout,
                d_ticks: b.ticks as f64 - a.ticks as f64,
                d_intake_producer: b.intake_producer - a.intake_producer,
                d_opening_fraction: opening(b) - opening(a),
            });
        }
    }

    let mut paired_candidates = Vec::new();
    for (label, _) in thetas {
        let episodes = |adapter: &str| -> (u64, f64) {
            let e: Vec<Episode> = layouts
                .iter()
                .map(|l| {
                    let r = find(adapter, label, l);
                    // Exactly the four fields `score_by` reads, each recorded on the row:
                    // the survival ticks, whether the body was alive, and the two that make
                    // `Episode::normalized_stores`. Every other field is a diagnostic the
                    // score never touches, and is left at zero rather than reconstructed.
                    Episode {
                        layout: r.layout.clone(),
                        ticks: r.ticks,
                        alive: r.alive,
                        terminal_stores: r.terminal_stores,
                        store_capacity: r.store_capacity,
                        ..blank_episode()
                    }
                })
                .collect();
            let t_min = e.iter().map(|x| x.ticks).min().expect("four layouts");
            (t_min, score_by(checkpoint.protocol.aggregate, &e))
        };
        let (t_min_one, score_one) = episodes(ActionAdapter::CubAct1.name());
        let (t_min_two, score_two) = episodes(ActionAdapter::CubAct2.name());
        paired_candidates.push(PairedCandidate {
            candidate: label.clone(),
            t_min_one,
            t_min_two,
            d_t_min: t_min_two as f64 - t_min_one as f64,
            score_one,
            score_two,
            d_score: score_two - score_one,
        });
    }

    let col = |f: fn(&PairedTrajectory) -> f64| -> Vec<f64> {
        paired_trajectories.iter().map(f).collect()
    };
    let tests = vec![
        named("turn_active_fraction", "trajectory", &col(|p| p.d_turn_active_fraction)),
        named("mean_abs_omega", "trajectory", &col(|p| p.d_mean_abs_omega)),
        named("on_food_fraction", "trajectory", &col(|p| p.d_on_food_fraction)),
        named("mean_dwell_bout", "trajectory", &col(|p| p.d_mean_dwell_bout)),
        named("ticks", "trajectory", &col(|p| p.d_ticks)),
        named("intake_producer", "trajectory", &col(|p| p.d_intake_producer)),
        named("opening_fraction", "trajectory", &col(|p| p.d_opening_fraction)),
        named(
            "t_min",
            "candidate",
            &paired_candidates.iter().map(|c| c.d_t_min).collect::<Vec<_>>(),
        ),
        named(
            "score",
            "candidate",
            &paired_candidates.iter().map(|c| c.d_score).collect::<Vec<_>>(),
        ),
    ];

    let get = |metric: &str| tests.iter().find(|t| t.metric == metric).expect("named above");
    let turn = get("turn_active_fraction");
    let on_food = get("on_food_fraction");
    let dwell = get("mean_dwell_bout");
    let t_min = get("t_min");
    let (verdict, detail) = if on_food.rises || t_min.rises {
        (
            "supported",
            format!(
                "on-food fraction {} (p = {:.4}) and t_min {} (p = {:.4}): the rule's \
                 support branch is met.",
                if on_food.rises { "rises" } else { "does not rise" },
                on_food.test.p_value,
                if t_min.rises { "rises" } else { "does not rise" },
                t_min.test.p_value,
            ),
        )
    } else if turn.rises
        && on_food.test.p_value > ALPHA
        && dwell.test.p_value > ALPHA
        && t_min.test.p_value > ALPHA
    {
        (
            "falsified",
            format!(
                "turn activity rises (p = {:.4}) while on-food fraction (p = {:.4}), mean \
                 dwell bout (p = {:.4}) and t_min (p = {:.4}) all stay above 0.1.",
                turn.test.p_value,
                on_food.test.p_value,
                dwell.test.p_value,
                t_min.test.p_value,
            ),
        )
    } else {
        (
            "mixed",
            format!(
                "neither branch is met: turn activity {} (p = {:.4}), on-food fraction p = \
                 {:.4}, mean dwell bout p = {:.4}, t_min p = {:.4}.",
                if turn.rises { "rises" } else { "does not rise" },
                turn.test.p_value,
                on_food.test.p_value,
                dwell.test.p_value,
                t_min.test.p_value,
            ),
        )
    };

    ReplayReport {
        build: BUILD_ID.to_string(),
        run: run.display().to_string(),
        generation,
        config: config.display().to_string(),
        config_hash: eco.hex(),
        protocol_hash: checkpoint.protocol_hash,
        train_seed: checkpoint.train_seed,
        sigma: checkpoint.protocol.sigma,
        pairs: checkpoint.protocol.pairs,
        aggregate: checkpoint.protocol.aggregate.as_str().to_string(),
        motor: checkpoint.protocol.motor.name().to_string(),
        horizon_ticks: horizon,
        layouts,
        digest_cub_act_1: cubarium_core::neural::schema_digest_in(ActionAdapter::CubAct1),
        digest_cub_act_2: cubarium_core::neural::schema_digest_in(ActionAdapter::CubAct2),
        centre_file_digest: file.policy_digest,
        alpha: ALPHA,
        decision_rule: RULE.to_string(),
        tests,
        verdict: verdict.to_string(),
        verdict_detail: detail,
        wall_seconds: wall,
        rows,
        paired_trajectories,
        paired_candidates,
    }
}

/// An `Episode` with every diagnostic zeroed. The caller fills in exactly the fields
/// [`score_by`] reads — `ticks`, `alive`, `terminal_stores`, `store_capacity` — and nothing
/// else, so a reconstructed score is the protocol's score and cannot pick up a diagnostic.
fn blank_episode() -> Episode {
    Episode {
        layout: String::new(),
        ticks: 0,
        alive: false,
        terminal_stores: 0.0,
        store_capacity: 0.0,
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
    }
}

// -------------------------------------------------------------------------------------------
// Deliverable 4: gradient-direction stability, from the retained pair contributions.
// -------------------------------------------------------------------------------------------

/// A sample of cosines, reduced to the spread rather than to one number.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spread {
    pub samples: usize,
    pub mean: f64,
    pub sd: f64,
    pub min: f64,
    pub p05: f64,
    pub p25: f64,
    pub median: f64,
    pub p75: f64,
    pub p95: f64,
    pub max: f64,
    /// How many samples point the *other* way: a cosine at or below zero.
    pub non_positive: usize,
}

impl Spread {
    pub fn of(values: &mut Vec<f64>) -> Spread {
        values.sort_by(|a, b| a.partial_cmp(b).expect("finite cosines"));
        let n = values.len();
        let mean = values.iter().sum::<f64>() / n as f64;
        let var = values.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n as f64;
        let q = |f: f64| -> f64 {
            if n == 0 {
                return 0.0;
            }
            let i = ((n - 1) as f64 * f).round() as usize;
            values[i.min(n - 1)]
        };
        Spread {
            samples: n,
            mean,
            sd: var.sqrt(),
            min: values[0],
            p05: q(0.05),
            p25: q(0.25),
            median: median(values),
            p75: q(0.75),
            p95: q(0.95),
            max: values[n - 1],
            non_positive: values.iter().filter(|x| **x <= 0.0).count(),
        }
    }
}

/// Everything deliverable 4 produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StabilityReport {
    pub build: String,
    pub run: String,
    pub pairs_file: String,
    pub generation: u64,
    pub pairs: usize,
    pub sigma: f64,
    pub train_seed: u64,
    /// The retained `u_plus − u_minus` per pair, in pair order.
    pub weights: Vec<f64>,
    /// How many of them are exactly zero: a tied pair contributes nothing and carries no
    /// direction.
    pub zero_weight_pairs: usize,
    /// Every balanced 8/8 split, not a sample.
    pub splits_evaluated: usize,
    pub split_half_cosine: Spread,
    pub bootstrap_samples: usize,
    pub bootstrap_cosine_with_full: Spread,
    pub bootstrap_cosine_with_update: Spread,
    /// The whole sixteen-pair estimate's own cosine with the recorded update `θ_{g+1} − θ_g`.
    /// Adam rescales every coordinate, so this is well below 1 even though the update *is* the
    /// estimator's: it is reported so the bootstrap spread has its own centre to be read
    /// against.
    pub full_cosine_with_update: f64,
    pub update_norm: f64,
    pub full_direction_norm: f64,
    pub wall_seconds: f64,
}

/// Deliverable 4: split-half and bootstrap stability of the retained pair contributions.
pub fn run_stability(
    run: &Path,
    pairs_file: &Path,
    generation: u64,
    bootstrap: usize,
    out: &Path,
) -> Result<StabilityReport, Boxed> {
    let started = Instant::now();
    let checkpoint: Checkpoint =
        serde_json::from_str(&std::fs::read_to_string(run.join("checkpoint.json"))?)?;
    let text = std::fs::read_to_string(pairs_file)?;
    let retained: serde_json::Value = serde_json::from_str(&text)?;
    let all: Vec<PairRow> = serde_json::from_value(
        retained
            .get("pairs")
            .cloned()
            .ok_or_else(|| Boxed::from("that file has no `pairs` array"))?,
    )?;
    let mut rows: Vec<PairRow> =
        all.into_iter().filter(|p| p.generation == generation).collect();
    rows.sort_by_key(|p| p.pair);
    let n = checkpoint.protocol.pairs;
    if rows.len() != n {
        return Err(Boxed::from(format!(
            "generation {generation} has {} retained pair rows, the protocol has {n} pairs",
            rows.len()
        )));
    }
    let weights: Vec<f64> = rows.iter().map(|p| p.weight).collect();

    // The perturbations, regenerated from the run's own seed exactly as the trainer does.
    let dim = checkpoint.theta.len();
    let mut eps: Vec<Vec<f64>> = Vec::with_capacity(n);
    let mut scratch = vec![0.0; dim];
    for p in 0..n {
        perturbation(checkpoint.train_seed, generation, p as u64, &mut scratch);
        eps.push(scratch.clone());
    }

    // `G[i][j] = w_i w_j <eps_i, eps_j>`. Every direction below is a signed sum of the
    // `w_i eps_i`, so every norm and every inner product between two such sums is a sum over
    // this 16x16 matrix — exact, and without forming a 10,215-vector per sample.
    let mut g = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in i..n {
            let dot: f64 = eps[i].iter().zip(&eps[j]).map(|(a, b)| a * b).sum();
            let v = weights[i] * weights[j] * dot;
            g[i][j] = v;
            g[j][i] = v;
        }
    }
    let quad = |c: &[f64]| -> f64 {
        let mut acc = 0.0;
        for i in 0..n {
            if c[i] == 0.0 {
                continue;
            }
            for j in 0..n {
                acc += c[i] * c[j] * g[i][j];
            }
        }
        acc
    };
    let bilinear = |a: &[f64], b: &[f64]| -> f64 {
        let mut acc = 0.0;
        for i in 0..n {
            if a[i] == 0.0 {
                continue;
            }
            for j in 0..n {
                acc += a[i] * b[j] * g[i][j];
            }
        }
        acc
    };

    // 1. Split-half: every balanced 8/8 split, each counted once (the complement of a split is
    //    the same split with the halves exchanged, and the cosine is symmetric).
    let half = n / 2;
    let mut split_cosines: Vec<f64> = Vec::new();
    let mut splits = 0usize;
    for mask in 0u32..(1u32 << n) {
        if mask.count_ones() as usize != half {
            continue;
        }
        // Keep one of each complementary pair: the one that contains pair 0.
        if mask & 1 == 0 {
            continue;
        }
        splits += 1;
        let mut a = vec![0.0f64; n];
        let mut b = vec![0.0f64; n];
        for i in 0..n {
            if mask & (1 << i) != 0 {
                a[i] = 1.0;
            } else {
                b[i] = 1.0;
            }
        }
        let na = quad(&a).sqrt();
        let nb = quad(&b).sqrt();
        if na > 0.0 && nb > 0.0 {
            split_cosines.push(bilinear(&a, &b) / (na * nb));
        }
    }

    // 2. The recorded update, from the run's own centre files.
    let theta_g = read_center(run, generation)?;
    let theta_next = read_center(run, generation + 1)?;
    let update: Vec<f64> = theta_next.iter().zip(&theta_g).map(|(a, b)| a - b).collect();
    let update_norm = update.iter().map(|x| x * x).sum::<f64>().sqrt();
    // `<w_i eps_i, update>` per pair, so a resampled direction's inner product with the update
    // is again a weighted sum of sixteen numbers.
    let with_update: Vec<f64> = (0..n)
        .map(|i| weights[i] * eps[i].iter().zip(&update).map(|(a, b)| a * b).sum::<f64>())
        .collect();

    let ones = vec![1.0f64; n];
    let full_norm = quad(&ones).sqrt();
    let full_cosine_with_update = if full_norm > 0.0 && update_norm > 0.0 {
        with_update.iter().sum::<f64>() / (full_norm * update_norm)
    } else {
        0.0
    };

    // 3. Bootstrap: 1,000 resamples of the sixteen pairs with replacement, from a stream keyed
    //    on the run's own seed and generation, so the sample is reproducible.
    let mut boot_full: Vec<f64> = Vec::with_capacity(bootstrap);
    let mut boot_update: Vec<f64> = Vec::with_capacity(bootstrap);
    let mut state = checkpoint
        .train_seed
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add(generation.wrapping_mul(0x0000_0100_0000_01b3))
        | 1;
    let mut next_u64 = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..bootstrap {
        let mut c = vec![0.0f64; n];
        for _ in 0..n {
            c[(next_u64() % n as u64) as usize] += 1.0;
        }
        let norm = quad(&c).sqrt();
        if norm <= 0.0 {
            continue;
        }
        if full_norm > 0.0 {
            boot_full.push(bilinear(&c, &ones) / (norm * full_norm));
        }
        if update_norm > 0.0 {
            let dot: f64 = (0..n).map(|i| c[i] * with_update[i]).sum();
            boot_update.push(dot / (norm * update_norm));
        }
    }

    let report = StabilityReport {
        build: BUILD_ID.to_string(),
        run: run.display().to_string(),
        pairs_file: pairs_file.display().to_string(),
        generation,
        pairs: n,
        sigma: checkpoint.protocol.sigma,
        train_seed: checkpoint.train_seed,
        zero_weight_pairs: weights.iter().filter(|w| **w == 0.0).count(),
        weights,
        splits_evaluated: splits,
        split_half_cosine: Spread::of(&mut split_cosines),
        bootstrap_samples: bootstrap,
        bootstrap_cosine_with_full: Spread::of(&mut boot_full),
        bootstrap_cosine_with_update: Spread::of(&mut boot_update),
        full_cosine_with_update,
        update_norm,
        full_direction_norm: full_norm,
        wall_seconds: started.elapsed().as_secs_f64(),
    };
    write_json(out, &report)?;
    print_stability(&report);
    Ok(report)
}

fn read_center(run: &Path, generation: u64) -> Result<Vec<f64>, Boxed> {
    let path = run.join(format!("centers/center-{generation:05}.json"));
    let file: super::export::PolicyFile = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    Ok(file.theta)
}

// -------------------------------------------------------------------------------------------

fn write_json<T: Serialize>(out: &Path, value: &T) -> Result<(), Boxed> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(out, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn print_replay(r: &ReplayReport) {
    println!("# the turn-deadband replay on {} generation {} ({})", r.run, r.generation, r.build);
    println!(
        "# config {} ({}), protocol {:#018x}, seed {}, sigma {}, {} pairs, aggregate {}, motor {}",
        r.config, r.config_hash, r.protocol_hash, r.train_seed, r.sigma, r.pairs, r.aggregate,
        r.motor
    );
    println!(
        "# digests: cub-act-1 {:#018x} (the retained centre's: {}), cub-act-2 {:#018x}",
        r.digest_cub_act_1,
        r.centre_file_digest == r.digest_cub_act_1,
        r.digest_cub_act_2
    );
    println!("# rule: {}", r.decision_rule);
    println!(
        "{:<24} {:<10} {:>4} {:>4} {:>4} {:>9} {:>11} {:>11}",
        "metric", "unit", "+", "-", "tie", "p", "median d", "mean d"
    );
    for t in &r.tests {
        println!(
            "{:<24} {:<10} {:>4} {:>4} {:>4} {:>9.5} {:>11.5} {:>11.5}",
            t.metric,
            t.unit,
            t.test.positive,
            t.test.negative,
            t.test.ties,
            t.test.p_value,
            t.test.median_difference,
            t.test.mean_difference,
        );
    }
    println!("verdict: {} — {}", r.verdict, r.verdict_detail);
    println!("{} episodes in {:.1} s", r.rows.len(), r.wall_seconds);
}

fn print_stability(r: &StabilityReport) {
    println!("# gradient-direction stability, generation {} of {} ({})", r.generation, r.run, r.build);
    println!(
        "# {} pairs ({} with exactly zero weight), sigma {}, seed {}, pairs from {}",
        r.pairs, r.zero_weight_pairs, r.sigma, r.train_seed, r.pairs_file
    );
    println!(
        "{:<28} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>6}",
        "distribution", "n", "mean", "sd", "min", "p05", "median", "p95", "max", "<=0"
    );
    for (name, s) in [
        ("split-half cosine", &r.split_half_cosine),
        ("bootstrap vs full", &r.bootstrap_cosine_with_full),
        ("bootstrap vs update", &r.bootstrap_cosine_with_update),
    ] {
        println!(
            "{:<28} {:>7} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>6}",
            name, s.samples, s.mean, s.sd, s.min, s.p05, s.median, s.p95, s.max, s.non_positive
        );
    }
    println!(
        "full 16-pair direction vs the recorded update: cosine {:.4} (|update| {:.4e}, \
         |direction| {:.4e})",
        r.full_cosine_with_update, r.update_norm, r.full_direction_norm
    );
    println!("{:.2} s", r.wall_seconds);
}

/// Both experiments, one after the other, into one directory.
pub fn run(
    run_dir: PathBuf,
    config: PathBuf,
    generation: u64,
    horizon: u64,
    workers: usize,
    wall_seconds: u64,
    pairs_file: PathBuf,
    bootstrap: usize,
    out: PathBuf,
) -> Result<(), Boxed> {
    run_replay(
        &run_dir,
        generation,
        &config,
        horizon,
        workers,
        wall_seconds,
        &out.join("replay.json"),
    )?;
    println!();
    run_stability(&run_dir, &pairs_file, generation, bootstrap, &out.join("stability.json"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact sign test against values computed by hand from the binomial.
    #[test]
    fn the_sign_test_is_the_exact_two_sided_binomial() {
        // 10 of 10 positive: 2·(1/1024) = 0.001953125.
        let t = sign_test(&[1.0; 10]);
        assert_eq!((t.positive, t.negative, t.ties), (10, 0, 0));
        assert!((t.p_value - 2.0 / 1024.0).abs() < 1e-12, "{}", t.p_value);
        assert!(t.rises() && !t.falls());

        // 0 of 10: the same p, the other way.
        let t = sign_test(&[-1.0; 10]);
        assert!((t.p_value - 2.0 / 1024.0).abs() < 1e-12);
        assert!(t.falls() && !t.rises());

        // 5 of 10 is p = 1 exactly.
        let mut d = vec![1.0; 5];
        d.extend(vec![-1.0; 5]);
        let t = sign_test(&d);
        assert!((t.p_value - 1.0).abs() < 1e-12, "{}", t.p_value);
        assert!(!t.rises() && !t.falls());

        // 8 of 10: 2·(C(10,8)+C(10,9)+C(10,10))/1024 = 2·56/1024 = 0.109375, just above 0.1.
        let mut d = vec![1.0; 8];
        d.extend(vec![-1.0; 2]);
        let t = sign_test(&d);
        assert!((t.p_value - 112.0 / 1024.0).abs() < 1e-12, "{}", t.p_value);
        assert!(!t.rises(), "p = {} is above alpha = {ALPHA}", t.p_value);

        // 9 of 10: 2·11/1024 = 0.021484375, below it.
        let mut d = vec![1.0; 9];
        d.extend(vec![-1.0; 1]);
        let t = sign_test(&d);
        assert!((t.p_value - 22.0 / 1024.0).abs() < 1e-12, "{}", t.p_value);
        assert!(t.rises());
    }

    /// Ties are dropped, and a set of nothing but ties decides nothing.
    #[test]
    fn ties_are_dropped_and_decide_nothing() {
        let t = sign_test(&[0.0, 0.0, 0.0, 1.0, 1.0]);
        assert_eq!((t.positive, t.negative, t.ties), (2, 0, 3));
        assert!((t.p_value - 0.5).abs() < 1e-12, "{}", t.p_value);
        let t = sign_test(&[0.0; 40]);
        assert_eq!(t.n(), 0);
        assert_eq!(t.p_value, 1.0);
        assert!(!t.rises() && !t.falls());
    }

    /// At 132 trajectories the normal approximation and the exact test agree closely, which is
    /// the check that the log-space binomial terms do not drift at the sizes this replay uses.
    #[test]
    fn the_exact_test_is_sane_at_the_replay_size() {
        // 80 of 132 positive. Normal: z = (80 − 66)/sqrt(33) = 2.4372, two-sided p ≈ 0.0148.
        let mut d = vec![1.0; 80];
        d.extend(vec![-1.0; 52]);
        let t = sign_test(&d);
        assert!(t.p_value > 0.010 && t.p_value < 0.025, "p = {}", t.p_value);
        assert!(t.rises());
        // And 66/66 is exactly 1.
        let mut d = vec![1.0; 66];
        d.extend(vec![-1.0; 66]);
        assert!((sign_test(&d).p_value - 1.0).abs() < 1e-9);
    }

    /// The spread's quantiles and its "points the other way" count read off a known sample.
    #[test]
    fn the_spread_reports_the_sample_it_was_given() {
        let mut v: Vec<f64> = (0..=100).map(|i| (i as f64 - 50.0) / 50.0).collect();
        let s = Spread::of(&mut v);
        assert_eq!(s.samples, 101);
        assert!((s.median - 0.0).abs() < 1e-12);
        assert!((s.min + 1.0).abs() < 1e-12);
        assert!((s.max - 1.0).abs() < 1e-12);
        assert_eq!(s.non_positive, 51, "fifty negatives and the zero");
        assert!((s.p05 + 0.9).abs() < 1e-12, "{}", s.p05);
        assert!((s.p95 - 0.9).abs() < 1e-12, "{}", s.p95);
    }
}
