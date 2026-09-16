//! **Where the ES search loses candidate variation** — the antithetic-pair reduction over the
//! retained generation reports, and the deadband occupancy of a σ-scale perturbation
//! (`design/handoffs/ecology-v1-es-antithetic-opus-2026-09-16.md`).
//!
//! Workstream L left one question standing: the perturbations *do* produce material score and
//! feeding variation (generation 9 spans 6,459–8,915 ticks), so the open question is where that
//! variation is lost — in the four-layout minimum, in the centred-rank reduction, in the update,
//! or in the mapping from weights to residence.
//!
//! Everything in the first half of this module is **arithmetic on already-recorded rows**. It
//! simulates nothing, it trains nothing, and it changes no part of the trainer: the score is
//! [`super::trainer::score_by`], the utilities are [`super::optimizer::centered_rank_utilities`],
//! the gradient is [`super::optimizer::gradient`], the perturbations are
//! [`super::rng::perturbation`] and the ascent is [`super::optimizer::Adam`]. No copy of any of
//! them lives here; if this module and the trainer ever disagree, the trainer is right and this
//! module is broken.
//!
//! # What a pair is, and what its weight is
//!
//! A generation evaluates `2n` candidates `theta ± sigma·epsilon_i`, each on every training
//! layout. [`super::trainer::GenerationReport`] persists the `2n` aggregate scores in candidate
//! index order (`2·pair + sign`) and **every** candidate-layout episode, so a pair's score
//! difference, its per-layout survival differences, its intake and its opening residence are all
//! recoverable without running anything.
//!
//! The estimator uses a pair only through one scalar: `w_i = u(+i) − u(−i)`, the difference of
//! the two members' centred-rank utilities over the whole `2n` population. That is the pair's
//! **entire** signed contribution to the update — the gradient is `Σ_i w_i · epsilon_i / (2 n
//! sigma)` and nothing else about the pair reaches Adam. So `w_i` is what this module relates to
//! intake and residence.
//!
//! # The three places variation can be lost, and the column that measures each
//!
//! - **The four-layout minimum.** `score = t_min + 0.25·stores`, so a candidate that lives
//!   longer on three layouts and shorter on the binding one scores *worse*.
//!   [`PairRow::minimum_masked`] is exactly that disagreement: the sign of the score difference
//!   against the sign of the mean-over-layouts survival difference.
//! - **The centred-rank reduction.** Rank is scale-free: a 2,400-tick spread and a 24-tick
//!   spread produce the same utilities. What survives is the *ordering*, and
//!   [`PairRow::weight`] is what the ordering leaves. Whether the ordering is the right one is
//!   [`GenerationRow::concordant_intake_rate`] and friends.
//! - **The update.** [`Cancellation`] separates two different things that both look like
//!   "cancellation": the loss forced by near-orthogonal random directions (which is geometry,
//!   not a defect) and any *excess* loss from pairs genuinely pointing against each other.
//!   [`Geometry`] then asks the brief's question directly — is the applied step moving the
//!   centre toward the generation's best candidate, or past it?
//!
//! # Deadband occupancy
//!
//! The second half samples `theta ± sigma·epsilon` and runs each forward over workstream L's
//! recorded observation set, counting the ticks whose **thrust** and **turn** channels land
//! inside the adapter's deadband. Membership is read off the core's own adapter: `band` maps
//! anything under [`cubarium_core::neural::action::DEADBAND`] to exactly `0.0`, and neither
//! movement channel is capability-masked, so `Action7::squash(&head, cap).0[THRUST] == 0.0`
//! *is* the deadband predicate. No copy of the squash lives here.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use cubarium_core::neural::action::{DEADBAND, THRUST, TURN};
use cubarium_core::neural::obs::OBS_LEN;
use cubarium_core::neural::{Action7, Capability, Gru32, HIDDEN};
use serde::{Deserialize, Serialize};

use super::episode::Episode;
use super::optimizer::{self, Adam};
use super::rng::perturbation;
use super::scorecheck::Sample;
use super::trainer::{Aggregate, GenerationReport, Protocol};

type Boxed = Box<dyn std::error::Error>;

// ---------------------------------------------------------------------------------------
// The reduction: one row per antithetic pair, no simulation.
// ---------------------------------------------------------------------------------------

/// One antithetic pair of one generation, reduced to everything the brief asks about it.
///
/// Every per-layout vector is in the report's own layout order ([`layout_order`]), which is the
/// order the trainer's plan dispatched them in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairRow {
    pub generation: u64,
    pub pair: usize,
    /// Aggregate score of `theta + sigma·epsilon`, as the report recorded it.
    pub score_plus: f64,
    /// Aggregate score of `theta − sigma·epsilon`.
    pub score_minus: f64,
    /// `score_plus − score_minus`.
    pub d_score: f64,
    pub ticks_plus: Vec<u64>,
    pub ticks_minus: Vec<u64>,
    /// `ticks_plus[l] − ticks_minus[l]`, per layout.
    pub d_ticks: Vec<i64>,
    /// How many layouts the plus member outlived the minus member on.
    pub layouts_favouring_plus: usize,
    /// Index of the layout that *set* the plus member's `t_min` (the first minimum on a tie).
    pub binding_plus: usize,
    pub binding_minus: usize,
    /// Mean-over-layouts survival difference: what the score would have used under
    /// [`Aggregate::Mean`], minus the store tiebreak.
    pub d_mean_ticks: f64,
    /// The minimum hid a gain: `d_score` and `d_mean_ticks` disagree in sign, both nonzero.
    pub minimum_masked: bool,
    /// Mean-over-layouts difference in `Episode::intake_producer` (material through the mouth).
    pub d_intake: f64,
    /// The same per *lived tick*, so a longer life is not counted as better feeding.
    pub d_intake_rate: f64,
    /// Mean-over-layouts difference in `Episode::ticks_in_opening`.
    pub d_opening: f64,
    /// The same as a fraction of lived ticks.
    pub d_opening_fraction: f64,
    /// Centred-rank utility of the plus member over the whole `2n` population.
    pub u_plus: f64,
    pub u_minus: f64,
    /// `u_plus − u_minus`: the pair's whole signed contribution to the gradient estimate.
    pub weight: f64,
}

impl PairRow {
    /// A pair the estimator can use at all: a zero weight contributes exactly nothing, so it
    /// carries no evidence either way about concordance.
    pub fn informative(&self) -> bool {
        self.weight != 0.0
    }

    /// Whether the member the estimator *prefers* is also the member with more of `delta`
    /// (`delta` measured plus-minus). `None` when either side carries no information.
    pub fn concordant(&self, delta: f64) -> Option<bool> {
        if self.weight == 0.0 || delta == 0.0 {
            return None;
        }
        Some((self.weight > 0.0) == (delta > 0.0))
    }
}

/// One generation, reduced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenerationRow {
    pub generation: u64,
    pub pairs: usize,
    pub score_min: f64,
    pub score_max: f64,
    pub score_mean: f64,
    /// Population standard deviation (divisor `2n`), which is what Astra's 643 is.
    pub score_sd: f64,
    /// Sample standard deviation (divisor `2n − 1`), reported so the two are never confused.
    pub score_sd_sample: f64,
    pub center_score: Option<f64>,
    pub best_candidate: f64,
    pub best_candidate_pair: usize,
    /// `true` when the best candidate is the plus member.
    pub best_candidate_plus: bool,
    pub informative_pairs: usize,
    pub zero_weight_pairs: usize,
    /// Pairs whose preferred member also has more total producer intake.
    pub concordant_intake: usize,
    /// The same on intake per lived tick.
    pub concordant_intake_rate: usize,
    pub concordant_opening: usize,
    pub concordant_opening_fraction: usize,
    /// Pairs where the minimum and the mean disagree about which member is better.
    pub minimum_masked: usize,
    /// How often each layout set some candidate's `t_min`, over all `2n` candidates.
    pub binding_counts: Vec<usize>,
    pub sum_abs_weight: f64,
    pub root_sum_sq_weight: f64,
    pub gradient_norm: f64,
    pub update_rms: f64,
}

/// Everything the reduction produced over a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reduction {
    pub layouts: Vec<String>,
    pub generations: Vec<GenerationRow>,
    pub pairs: Vec<PairRow>,
}

/// The layout names of a report, in the order its jobs were dispatched.
///
/// The trainer writes `candidates.len() × layouts.len()` jobs, layout-innermost, so first
/// appearance over the whole job list is the plan's order. It is taken over *every* job rather
/// than over the first candidate's block so that a candidate missing a layout is caught by
/// [`candidate_episodes`] instead of quietly shrinking the layout set — and with it the
/// minimum that every score in the report was taken over.
pub fn layout_order(report: &GenerationReport) -> Result<Vec<String>, String> {
    if report.jobs.is_empty() {
        return Err(format!("generation {} has no jobs", report.generation));
    }
    let mut out: Vec<String> = Vec::new();
    for job in &report.jobs {
        if !out.contains(&job.layout) {
            out.push(job.layout.clone());
        }
    }
    Ok(out)
}

/// One candidate's episodes, in layout order. Errors rather than guessing if a layout is
/// missing: a partial candidate would silently change a minimum.
pub fn candidate_episodes<'a>(
    report: &'a GenerationReport,
    label: &str,
    layouts: &[String],
) -> Result<Vec<&'a Episode>, String> {
    let mut out = Vec::with_capacity(layouts.len());
    for want in layouts {
        let job = report
            .jobs
            .iter()
            .find(|j| j.candidate == label && &j.layout == want)
            .ok_or_else(|| {
                format!(
                    "generation {}: candidate {label} has no episode on layout {want}",
                    report.generation
                )
            })?;
        out.push(&job.episode);
    }
    Ok(out)
}

/// Reduce one generation report. `aggregate` must be the protocol's, because the score's
/// aggregation is what [`PairRow::minimum_masked`] is measured against.
pub fn reduce_generation(
    report: &GenerationReport,
    aggregate: Aggregate,
) -> Result<(GenerationRow, Vec<PairRow>), String> {
    let layouts = layout_order(report)?;
    let m = report.candidate_scores.len();
    if m < 2 || m % 2 != 0 {
        return Err(format!("generation {} has {m} candidate scores, not 2n", report.generation));
    }
    let n = m / 2;

    // Every candidate's episodes, in candidate index order, checked against the score the
    // report recorded. A disagreement means the rows and the scores are not the same run.
    let mut episodes: Vec<Vec<&Episode>> = Vec::with_capacity(m);
    for ci in 0..m {
        let label = if ci % 2 == 0 {
            super::trainer::Candidate::Plus(ci / 2).label()
        } else {
            super::trainer::Candidate::Minus(ci / 2).label()
        };
        let eps = candidate_episodes(report, &label, &layouts)?;
        let seen = report.jobs.iter().filter(|j| j.candidate == label).count();
        if seen != layouts.len() {
            return Err(format!(
                "generation {}: {label} has {seen} episodes over {} layouts",
                report.generation,
                layouts.len()
            ));
        }
        let owned: Vec<Episode> = eps.iter().map(|e| (*e).clone()).collect();
        let recomputed = super::trainer::score_by(aggregate, &owned);
        if recomputed != report.candidate_scores[ci] {
            return Err(format!(
                "generation {}: {label} scores {} from its episodes but the report says {}",
                report.generation, recomputed, report.candidate_scores[ci]
            ));
        }
        episodes.push(eps);
    }

    let u = optimizer::centered_rank_utilities(
        &(0..n)
            .map(|p| report.candidate_scores[2 * p])
            .chain((0..n).map(|p| report.candidate_scores[2 * p + 1]))
            .collect::<Vec<f64>>(),
    );

    let mut binding_counts = vec![0usize; layouts.len()];
    let mut pairs = Vec::with_capacity(n);
    for p in 0..n {
        let plus = &episodes[2 * p];
        let minus = &episodes[2 * p + 1];
        let mean = |e: &[&Episode], f: &dyn Fn(&Episode) -> f64| -> f64 {
            e.iter().map(|x| f(x)).sum::<f64>() / e.len() as f64
        };
        let ticks_plus: Vec<u64> = plus.iter().map(|e| e.ticks).collect();
        let ticks_minus: Vec<u64> = minus.iter().map(|e| e.ticks).collect();
        let d_ticks: Vec<i64> = ticks_plus
            .iter()
            .zip(&ticks_minus)
            .map(|(a, b)| *a as i64 - *b as i64)
            .collect();
        let binding = |t: &[u64]| -> usize {
            let mut best = 0;
            for (i, v) in t.iter().enumerate() {
                if *v < t[best] {
                    best = i;
                }
            }
            best
        };
        let binding_plus = binding(&ticks_plus);
        let binding_minus = binding(&ticks_minus);
        binding_counts[binding_plus] += 1;
        binding_counts[binding_minus] += 1;

        let intake = |e: &Episode| e.intake_producer;
        let intake_rate = |e: &Episode| {
            if e.ticks == 0 { 0.0 } else { e.intake_producer / e.ticks as f64 }
        };
        let opening = |e: &Episode| e.ticks_in_opening as f64;
        let opening_fraction = |e: &Episode| {
            if e.ticks == 0 { 0.0 } else { e.ticks_in_opening as f64 / e.ticks as f64 }
        };

        let d_score = report.candidate_scores[2 * p] - report.candidate_scores[2 * p + 1];
        let d_mean_ticks = mean(plus, &|e| e.ticks as f64) - mean(minus, &|e| e.ticks as f64);
        let minimum_masked =
            d_score != 0.0 && d_mean_ticks != 0.0 && (d_score > 0.0) != (d_mean_ticks > 0.0);

        pairs.push(PairRow {
            generation: report.generation,
            pair: p,
            score_plus: report.candidate_scores[2 * p],
            score_minus: report.candidate_scores[2 * p + 1],
            d_score,
            ticks_plus,
            ticks_minus,
            d_ticks: d_ticks.clone(),
            layouts_favouring_plus: d_ticks.iter().filter(|d| **d > 0).count(),
            binding_plus,
            binding_minus,
            d_mean_ticks,
            minimum_masked,
            d_intake: mean(plus, &intake) - mean(minus, &intake),
            d_intake_rate: mean(plus, &intake_rate) - mean(minus, &intake_rate),
            d_opening: mean(plus, &opening) - mean(minus, &opening),
            d_opening_fraction: mean(plus, &opening_fraction) - mean(minus, &opening_fraction),
            u_plus: u[p],
            u_minus: u[n + p],
            weight: u[p] - u[n + p],
        });
    }

    let scores = &report.candidate_scores;
    let score_mean = scores.iter().sum::<f64>() / m as f64;
    let ss: f64 = scores.iter().map(|s| (s - score_mean).powi(2)).sum();
    let mut best = 0;
    for (i, s) in scores.iter().enumerate() {
        if *s > scores[best] {
            best = i;
        }
    }
    let count = |f: &dyn Fn(&PairRow) -> f64| {
        pairs.iter().filter(|r| r.concordant(f(r)) == Some(true)).count()
    };
    let row = GenerationRow {
        generation: report.generation,
        pairs: n,
        score_min: scores.iter().cloned().fold(f64::INFINITY, f64::min),
        score_max: scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        score_mean,
        score_sd: (ss / m as f64).sqrt(),
        score_sd_sample: (ss / (m - 1) as f64).sqrt(),
        center_score: report.center_score,
        best_candidate: scores[best],
        best_candidate_pair: best / 2,
        best_candidate_plus: best % 2 == 0,
        informative_pairs: pairs.iter().filter(|r| r.informative()).count(),
        zero_weight_pairs: pairs.iter().filter(|r| !r.informative()).count(),
        concordant_intake: count(&|r| r.d_intake),
        concordant_intake_rate: count(&|r| r.d_intake_rate),
        concordant_opening: count(&|r| r.d_opening),
        concordant_opening_fraction: count(&|r| r.d_opening_fraction),
        minimum_masked: pairs.iter().filter(|r| r.minimum_masked).count(),
        binding_counts,
        sum_abs_weight: pairs.iter().map(|r| r.weight.abs()).sum(),
        root_sum_sq_weight: pairs.iter().map(|r| r.weight * r.weight).sum::<f64>().sqrt(),
        gradient_norm: report.gradient_norm,
        update_rms: report.update_rms,
    };
    Ok((row, pairs))
}

/// Reduce a whole run. Every report must name the same layouts.
pub fn reduce(reports: &[GenerationReport], aggregate: Aggregate) -> Result<Reduction, String> {
    let mut layouts: Option<Vec<String>> = None;
    let mut generations = Vec::with_capacity(reports.len());
    let mut pairs = Vec::new();
    for report in reports {
        let mine = layout_order(report)?;
        match &layouts {
            None => layouts = Some(mine),
            Some(first) if *first != mine => {
                return Err(format!(
                    "generation {} names layouts {mine:?}, the run names {first:?}",
                    report.generation
                ));
            }
            Some(_) => {}
        }
        let (row, mut rows) = reduce_generation(report, aggregate)?;
        generations.push(row);
        pairs.append(&mut rows);
    }
    Ok(Reduction { layouts: layouts.unwrap_or_default(), generations, pairs })
}

/// Read a run's `generations.jsonl`, one [`GenerationReport`] per line.
pub fn read_reports(path: &Path) -> Result<Vec<GenerationReport>, Boxed> {
    let mut out = Vec::new();
    for (i, line) in BufReader::new(File::open(path)?).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let report: GenerationReport = serde_json::from_str(&line)
            .map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
        out.push(report);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// Cancellation: the geometry of `Σ w_i ε_i`, with its two reference values.
// ---------------------------------------------------------------------------------------

/// How much of the pairs' summed contribution survives the sum, against the two values that
/// say whether any of the loss is a defect.
///
/// `no_cancellation_norm` is `Σ |w_i| · ‖ε_i‖`: what the sum would be if every pair pointed the
/// same way. `orthogonal_norm` is `sqrt(Σ w_i² ‖ε_i‖²)`: what it is when the directions are
/// mutually orthogonal, which is what independent draws in 10,215 dimensions very nearly are.
/// The first ratio is therefore **not** evidence of anything on its own; the second one is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cancellation {
    pub summed_norm: f64,
    pub no_cancellation_norm: f64,
    pub orthogonal_norm: f64,
    /// `summed_norm / no_cancellation_norm` in `[0, 1]`.
    pub retained: f64,
    /// `summed_norm / orthogonal_norm`. At 1 the pairs neither help nor fight each other; below
    /// 1 they genuinely cancel beyond what orthogonality forces.
    pub excess: f64,
}

/// [`Cancellation`] for one generation's weights and perturbations. `eps(i, out)` writes pair
/// `i`'s perturbation, exactly as [`super::optimizer::gradient`] takes it.
pub fn cancellation<F>(weights: &[f64], dim: usize, eps: F) -> Cancellation
where
    F: FnMut(usize, &mut [f64]),
{
    let mut eps = eps;
    let mut sum = vec![0.0; dim];
    let mut one = vec![0.0; dim];
    let mut no_cancellation_norm = 0.0;
    let mut orthogonal_sq = 0.0;
    for (i, w) in weights.iter().enumerate() {
        eps(i, &mut one);
        let len = one.iter().map(|x| x * x).sum::<f64>().sqrt();
        no_cancellation_norm += w.abs() * len;
        orthogonal_sq += w * w * len * len;
        for j in 0..dim {
            sum[j] += w * one[j];
        }
    }
    let summed_norm = sum.iter().map(|x| x * x).sum::<f64>().sqrt();
    let orthogonal_norm = orthogonal_sq.sqrt();
    Cancellation {
        summed_norm,
        no_cancellation_norm,
        orthogonal_norm,
        retained: if no_cancellation_norm > 0.0 { summed_norm / no_cancellation_norm } else { 0.0 },
        excess: if orthogonal_norm > 0.0 { summed_norm / orthogonal_norm } else { 0.0 },
    }
}

// ---------------------------------------------------------------------------------------
// The update's geometry: toward the best candidate, or past it?
// ---------------------------------------------------------------------------------------

/// One generation's applied step, measured against the candidates it was estimated from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    pub generation: u64,
    /// `‖g‖` as the report recorded it.
    pub gradient_norm_recorded: f64,
    /// `‖g‖` recomputed here from the recorded scores and the regenerated perturbations. A
    /// disagreement means this module has the pairing or the seeds wrong.
    pub gradient_norm_recomputed: f64,
    pub cancellation: Cancellation,
    /// `‖theta_after − theta_before‖`.
    pub step_norm: f64,
    /// RMS of the same displacement, which is the report's `update_rms`.
    pub step_rms: f64,
    /// `sigma · ‖ε_best‖`: how far the generation's best candidate stood from the centre.
    pub best_distance: f64,
    /// Cosine between the applied step and the direction of the best candidate.
    pub cos_step_best: f64,
    /// The step's component along the best candidate, as a fraction of `best_distance`. 0 is
    /// "did not move toward it at all", 1 is "landed on it", above 1 is "went past it".
    pub progress_toward_best: f64,
    /// `‖theta_after − best_candidate‖ / ‖theta_before − best_candidate‖`.
    pub distance_ratio_to_best: f64,
    /// Mean over informative pairs of the step's signed progress along each pair's own
    /// *preferred* direction (`sign(w_i)·sigma·epsilon_i`), so the best candidate is not the
    /// only witness to where the step went.
    pub mean_progress_toward_preferred: f64,
}

/// Replay the run's updates from `theta0` using only the retained reports, and measure each
/// step against the candidates it came from.
///
/// This is the trainer's own arithmetic — [`super::optimizer::gradient`] and [`Adam::ascend`]
/// over [`super::rng::perturbation`] — so the final `theta` must equal the run's checkpoint bit
/// for bit. That equality is the whole reason to trust every number in [`Geometry`].
pub fn replay(
    theta0: &[f64],
    reports: &[GenerationReport],
    protocol: &Protocol,
    aggregate: Aggregate,
) -> Result<(Vec<Geometry>, Vec<f64>, Adam), String> {
    let dim = theta0.len();
    let mut theta = theta0.to_vec();
    let mut adam = Adam::new(dim);
    let mut out = Vec::with_capacity(reports.len());
    let mut eps = vec![0.0; dim];

    for report in reports {
        let (row, pairs) = reduce_generation(report, aggregate)?;
        let n = pairs.len();
        if n != protocol.pairs {
            return Err(format!(
                "generation {} holds {n} pairs, the protocol names {}",
                report.generation, protocol.pairs
            ));
        }
        let generation = report.generation;
        let seed = protocol.train_seed;
        let sigma = protocol.sigma;
        let plus: Vec<f64> = (0..n).map(|p| report.candidate_scores[2 * p]).collect();
        let minus: Vec<f64> = (0..n).map(|p| report.candidate_scores[2 * p + 1]).collect();
        let g = optimizer::gradient(&plus, &minus, dim, sigma, |i, out| {
            perturbation(seed, generation, i as u64, out);
        });
        let gradient_norm_recomputed = g.iter().map(|x| x * x).sum::<f64>().sqrt();
        let weights: Vec<f64> = pairs.iter().map(|r| r.weight).collect();
        let cancel = cancellation(&weights, dim, |i, out| {
            perturbation(seed, generation, i as u64, out);
        });

        let before = theta.clone();
        adam.ascend(&mut theta, &g);
        let step: Vec<f64> = theta.iter().zip(&before).map(|(a, b)| a - b).collect();
        let step_norm = step.iter().map(|x| x * x).sum::<f64>().sqrt();

        // The offset from the centre to the generation's best candidate.
        let sign = if row.best_candidate_plus { 1.0 } else { -1.0 };
        perturbation(seed, generation, row.best_candidate_pair as u64, &mut eps);
        let offset: Vec<f64> = eps.iter().map(|e| sign * sigma * e).collect();
        let best_sq: f64 = offset.iter().map(|x| x * x).sum();
        let best_distance = best_sq.sqrt();
        let dot: f64 = step.iter().zip(&offset).map(|(a, b)| a * b).sum();
        let missed: f64 = step
            .iter()
            .zip(&offset)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>()
            .sqrt();

        let mut progress_sum = 0.0;
        let mut informative = 0.0;
        for (i, w) in weights.iter().enumerate() {
            if *w == 0.0 {
                continue;
            }
            perturbation(seed, generation, i as u64, &mut eps);
            let s = w.signum() * sigma;
            let d: f64 = step.iter().zip(&eps).map(|(a, e)| a * s * e).sum();
            let len_sq: f64 = eps.iter().map(|e| (s * e) * (s * e)).sum();
            progress_sum += d / len_sq;
            informative += 1.0;
        }

        out.push(Geometry {
            generation,
            gradient_norm_recorded: report.gradient_norm,
            gradient_norm_recomputed,
            cancellation: cancel,
            step_norm,
            step_rms: step_norm / (dim as f64).sqrt(),
            best_distance,
            cos_step_best: if step_norm > 0.0 && best_distance > 0.0 {
                dot / (step_norm * best_distance)
            } else {
                0.0
            },
            progress_toward_best: if best_sq > 0.0 { dot / best_sq } else { 0.0 },
            distance_ratio_to_best: if best_distance > 0.0 { missed / best_distance } else { 0.0 },
            mean_progress_toward_preferred: if informative > 0.0 {
                progress_sum / informative
            } else {
                0.0
            },
        });
    }
    Ok((out, theta, adam))
}

// ---------------------------------------------------------------------------------------
// Deliverable 2: deadband occupancy under a σ-scale perturbation.
// ---------------------------------------------------------------------------------------

/// The raw head value at which each movement channel enters the adapter's deadband, in **head**
/// units, derived from the adapter's own [`DEADBAND`] by inverting the documented squash.
///
/// `thrust` uses a sigmoid, so it is inside the band below `ln(d / (1 − d))`; `turn` uses a
/// tanh, so it is inside below `atanh(d)` in absolute value. These are reported as margins only
/// — membership itself is always decided by the core's own adapter, never by these numbers.
pub fn deadband_head_edges() -> (f64, f64) {
    (((DEADBAND / (1.0 - DEADBAND)).ln()), 0.5 * ((1.0 + DEADBAND) / (1.0 - DEADBAND)).ln())
}

/// One driver's occupancy over one observation set, from one hidden-state start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeadbandRow {
    pub candidate: String,
    /// `"reset"` or `"carried"`.
    pub start: String,
    pub samples: u64,
    /// Ticks whose held thrust is exactly zero, i.e. inside the deadband.
    pub thrust_inside: u64,
    pub turn_inside: u64,
    pub both_inside: u64,
    /// Mean raw head value on the thrust channel, before squashing.
    pub mean_head_thrust: f64,
    /// Mean `|raw head|` on the turn channel, before squashing.
    pub mean_abs_head_turn: f64,
}

impl DeadbandRow {
    pub fn thrust_fraction(&self) -> f64 {
        if self.samples == 0 { 0.0 } else { self.thrust_inside as f64 / self.samples as f64 }
    }
    pub fn turn_fraction(&self) -> f64 {
        if self.samples == 0 { 0.0 } else { self.turn_inside as f64 / self.samples as f64 }
    }
}

/// Count deadband occupancy for one weight set over one sample set, from both hidden starts.
///
/// The carried start is the hidden state *the centre's* trajectory held at that tick: a
/// perturbed policy would have built a different one. That mismatch is the point — the question
/// is what a σ-sized perturbation does to the head at states the run actually visited — and it
/// is stated rather than hidden.
pub fn deadband_rows(
    candidate: &str,
    weights: &Gru32,
    cap: &Capability,
    samples: &[Sample],
) -> Vec<DeadbandRow> {
    let mut rows = Vec::with_capacity(2);
    for start in ["reset", "carried"] {
        let mut row = DeadbandRow {
            candidate: candidate.to_string(),
            start: start.to_string(),
            samples: 0,
            thrust_inside: 0,
            turn_inside: 0,
            both_inside: 0,
            mean_head_thrust: 0.0,
            mean_abs_head_turn: 0.0,
        };
        let mut sum_thrust = 0.0;
        let mut sum_abs_turn = 0.0;
        for s in samples {
            assert_eq!(s.obs.len(), OBS_LEN, "a sample's observation is the world's own");
            assert_eq!(s.hidden.len(), HIDDEN, "a sample's hidden state is the animal's own");
            let mut x = [0.0f64; OBS_LEN];
            x.copy_from_slice(&s.obs);
            let mut h = [0.0f64; HIDDEN];
            if start == "carried" {
                h.copy_from_slice(&s.hidden);
            }
            let (thrust, turn, head_thrust, head_turn) = movement_inside(weights, cap, &x, &h);
            row.samples += 1;
            row.thrust_inside += u64::from(thrust);
            row.turn_inside += u64::from(turn);
            row.both_inside += u64::from(thrust && turn);
            sum_thrust += head_thrust;
            sum_abs_turn += head_turn.abs();
        }
        if row.samples > 0 {
            row.mean_head_thrust = sum_thrust / row.samples as f64;
            row.mean_abs_head_turn = sum_abs_turn / row.samples as f64;
        }
        rows.push(row);
    }
    rows
}

/// Whether one forward pass lands each movement channel inside the deadband, and the raw head
/// values behind it. Decided by [`Action7::squash`], not by a copy of it.
pub fn movement_inside(
    weights: &Gru32,
    cap: &Capability,
    x: &[f64; OBS_LEN],
    hidden: &[f64; HIDDEN],
) -> (bool, bool, f64, f64) {
    let mut h = *hidden;
    let head = weights.forward(x, &mut h);
    // `band` maps anything under DEADBAND to exactly 0.0 and neither movement channel is
    // capability-masked, so the adapter's own zero *is* the membership test.
    let held = Action7::squash(&head, cap).0;
    (held[THRUST] == 0.0, held[TURN] == 0.0, head[THRUST], head[TURN])
}


// ---------------------------------------------------------------------------------------
// The two experiments, as callable functions. They read a retained run and write one JSON
// each; neither trains, and neither touches the trainer's score, update, sampler or protocol.
// ---------------------------------------------------------------------------------------

/// Everything deliverable 1 produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AntitheticReport {
    pub build: String,
    pub run: String,
    pub protocol_hash: u64,
    pub train_seed: u64,
    pub sigma: f64,
    pub learning_rate: f64,
    pub aggregate: String,
    pub generations_completed: u64,
    pub layouts: Vec<String>,
    /// `theta0` came from the run's own `centers/center-00000.json`, and this says whether it
    /// also equals `tensor::initial_center(train_seed)` bit for bit.
    pub theta0_is_the_seeded_center: bool,
    /// The largest absolute disagreement between the replayed final centre and the
    /// checkpoint's. Anything but exactly 0 means the replay is not the run.
    pub replay_max_abs_theta_error: f64,
    pub replay_matches_checkpoint: bool,
    pub generations: Vec<GenerationRow>,
    pub geometry: Vec<Geometry>,
    pub pairs: Vec<PairRow>,
}

/// Deliverable 1: the antithetic-pair reduction over a retained run. No simulation.
pub fn run_reduction(run: &Path, out: &Path) -> Result<AntitheticReport, Boxed> {
    let checkpoint: super::trainer::Checkpoint =
        serde_json::from_str(&std::fs::read_to_string(run.join("checkpoint.json"))?)?;
    let reports = read_reports(&run.join("generations.jsonl"))?;
    let aggregate = checkpoint.protocol.aggregate;
    let reduction = reduce(&reports, aggregate)?;

    let first: super::export::PolicyFile =
        serde_json::from_str(&std::fs::read_to_string(run.join("centers/center-00000.json"))?)?;
    let theta0 = first.theta.clone();
    let seeded = super::tensor::initial_center(checkpoint.train_seed);
    let (geometry, theta, _) = replay(&theta0, &reports, &checkpoint.protocol, aggregate)?;
    let err = theta
        .iter()
        .zip(&checkpoint.theta)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f64, f64::max);

    let report = AntitheticReport {
        build: crate::evaluate::BUILD_ID.to_string(),
        run: run.display().to_string(),
        protocol_hash: checkpoint.protocol_hash,
        train_seed: checkpoint.train_seed,
        sigma: checkpoint.protocol.sigma,
        learning_rate: checkpoint.protocol.learning_rate,
        aggregate: aggregate.as_str().to_string(),
        generations_completed: checkpoint.generation_completed,
        layouts: reduction.layouts.clone(),
        theta0_is_the_seeded_center: theta0 == seeded,
        replay_max_abs_theta_error: err,
        replay_matches_checkpoint: theta == checkpoint.theta,
        generations: reduction.generations,
        geometry,
        pairs: reduction.pairs,
    };
    write_json(out, &report)?;
    print_reduction(&report);
    Ok(report)
}

/// Everything deliverable 2 produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeadbandReport {
    pub build: String,
    pub run: String,
    pub generation: u64,
    pub config: String,
    pub config_hash: String,
    pub sigma: f64,
    pub pairs: usize,
    pub horizon_ticks: u64,
    pub per_stratum: usize,
    pub layouts: Vec<String>,
    pub samples: usize,
    pub on_food_samples: usize,
    /// Reproduced from the recording, so this set can be checked against workstream L's.
    pub max_reconstruction_residual: f64,
    pub thrust_head_edge: f64,
    pub turn_head_edge: f64,
    pub wall_seconds: f64,
    pub rows: Vec<DeadbandRow>,
}

/// Deliverable 2: deadband occupancy of `theta ± sigma·epsilon` over workstream L's recorded
/// observation set, against the centre's.
///
/// The observation set is rebuilt rather than reloaded: L's `sweep.json` retains the sweep rows
/// and the per-trajectory counters, not the 768 observation/hidden pairs themselves. The
/// recording call, the policy, the horizon, the layout list and the per-stratum count are L's,
/// and [`DeadbandReport::max_reconstruction_residual`] is the column that says so.
#[allow(clippy::too_many_arguments)]
pub fn run_deadband(
    run: &Path,
    generation: u64,
    config: &Path,
    horizon: u64,
    per_stratum: usize,
    workers: usize,
    wall_seconds: u64,
    out: &Path,
) -> Result<DeadbandReport, Boxed> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    let started = Instant::now();
    let eco = super::fixture::Ecology::load(config)?;
    let checkpoint: super::trainer::Checkpoint =
        serde_json::from_str(&std::fs::read_to_string(run.join("checkpoint.json"))?)?;
    let file: super::export::PolicyFile = serde_json::from_str(&std::fs::read_to_string(
        run.join(format!("centers/center-{generation:05}.json")),
    )?)?;
    file.check_ecology(&eco)?;
    if file.generation != generation {
        return Err(Boxed::from(format!("that file is generation {}", file.generation)));
    }
    let theta = file.theta.clone();
    let center = super::tensor::policy(&theta)?;

    let mut layouts = super::fixture::training_layouts_on(&eco);
    layouts.extend(super::fixture::holdout_layouts_on(&eco));

    // 1. Record the centre's own trajectories: exactly workstream L's observation set.
    let cursor = AtomicUsize::new(0);
    let done: Mutex<Vec<super::scorecheck::Recording>> = Mutex::new(Vec::new());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let cancel = AtomicBool::new(false);
    let limits = super::episode::Limits::until(&cancel, started + Duration::from_secs(wall_seconds));
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let next = cursor.fetch_add(1, Ordering::Relaxed);
                    let Some(layout) = layouts.get(next) else { return };
                    let cap = match super::scorecheck::capability_of(layout) {
                        Ok(c) => c,
                        Err(e) => {
                            failures.lock().expect("f").push(e.to_string());
                            continue;
                        }
                    };
                    let job = format!("deadband/center-{generation}/{}", layout.name);
                    match super::scorecheck::record(
                        layout, &center, &cap, horizon, limits, per_stratum, &job,
                    ) {
                        Ok(r) => done.lock().expect("d").push(r),
                        Err(e) => failures.lock().expect("f").push(e.to_string()),
                    }
                }
            });
        }
    });
    let failures = failures.into_inner().expect("f");
    if !failures.is_empty() {
        return Err(Boxed::from(format!("{} episode(s) failed: {}", failures.len(), failures.join("; "))));
    }
    let mut done = done.into_inner().expect("d");
    done.sort_by(|a, b| a.layout.cmp(&b.layout));
    let residual = done.iter().map(|r| r.max_reconstruction_residual).fold(0.0f64, f64::max);
    let samples: Vec<Sample> = done.iter().flat_map(|r| r.samples.iter().cloned()).collect();
    let on_food = samples.iter().filter(|s| s.on_food).count();

    // 2. The centre and generation `generation`'s own 2n candidates, forward over that set.
    // Neither movement channel is capability-masked, so one capability serves every row; it is
    // taken from a real body rather than assumed.
    let cap = super::scorecheck::capability_of(&layouts[0])?;
    let n = checkpoint.protocol.pairs;
    let sigma = checkpoint.protocol.sigma;
    let mut rows = deadband_rows("center", &center.weights, &cap, &samples);
    let mut eps = vec![0.0; theta.len()];
    for p in 0..n {
        perturbation(checkpoint.train_seed, generation, p as u64, &mut eps);
        for (label, sign) in [(super::trainer::Candidate::Plus(p), 1.0f64), (super::trainer::Candidate::Minus(p), -1.0)] {
            let moved: Vec<f64> =
                theta.iter().zip(&eps).map(|(t, e)| t + sign * sigma * e).collect();
            let policy = super::tensor::policy(&moved)?;
            rows.extend(deadband_rows(&label.label(), &policy.weights, &cap, &samples));
        }
    }

    let (thrust_head_edge, turn_head_edge) = deadband_head_edges();
    let report = DeadbandReport {
        build: crate::evaluate::BUILD_ID.to_string(),
        run: run.display().to_string(),
        generation,
        config: config.display().to_string(),
        config_hash: eco.hex(),
        sigma,
        pairs: n,
        horizon_ticks: horizon,
        per_stratum,
        layouts: done.iter().map(|r| r.layout.clone()).collect(),
        samples: samples.len(),
        on_food_samples: on_food,
        max_reconstruction_residual: residual,
        thrust_head_edge,
        turn_head_edge,
        wall_seconds: started.elapsed().as_secs_f64(),
        rows,
    };
    write_json(out, &report)?;
    print_deadband(&report);
    Ok(report)
}

fn write_json<T: Serialize>(out: &Path, value: &T) -> Result<(), Boxed> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(out, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn print_reduction(r: &AntitheticReport) {
    println!("# the antithetic-pair reduction on {} ({})", r.run, r.build);
    println!(
        "# protocol {:016x}, seed {}, sigma {}, lr {}, aggregate {}, layouts {:?}",
        r.protocol_hash, r.train_seed, r.sigma, r.learning_rate, r.aggregate, r.layouts
    );
    println!(
        "# replay from centers/center-00000.json: matches checkpoint {} (max |dtheta| {:e}), theta0 is the seeded centre {}",
        r.replay_matches_checkpoint, r.replay_max_abs_theta_error, r.theta0_is_the_seeded_center
    );
    println!(
        "{:>3} {:>7} {:>7} {:>7} {:>6} {:>7} {:>7} {:>6} {:>6} {:>6} {:>6} {:>6} {:>7} {:>7} {:>7}",
        "gen", "min", "max", "sd", "centre", "best", "next", "cnc-i", "cnc-r", "cnc-o", "mask", "w=0",
        "retain", "excess", "prog"
    );
    for (i, g) in r.generations.iter().enumerate() {
        let geo = &r.geometry[i];
        let next = r.generations.get(i + 1).and_then(|n| n.center_score);
        println!(
            "{:>3} {:>7.0} {:>7.0} {:>7.0} {:>6} {:>7.0} {:>7} {:>5}/{} {:>5}/{} {:>5}/{} {:>6} {:>6} {:>7.3} {:>7.3} {:>7.4}",
            g.generation,
            g.score_min,
            g.score_max,
            g.score_sd,
            g.center_score.map(|s| format!("{s:.0}")).unwrap_or_else(|| "-".into()),
            g.best_candidate,
            next.map(|s| format!("{s:.0}")).unwrap_or_else(|| "-".into()),
            g.concordant_intake,
            g.informative_pairs,
            g.concordant_intake_rate,
            g.informative_pairs,
            g.concordant_opening_fraction,
            g.informative_pairs,
            g.minimum_masked,
            g.zero_weight_pairs,
            geo.cancellation.retained,
            geo.cancellation.excess,
            geo.progress_toward_best
        );
    }
}

fn print_deadband(r: &DeadbandReport) {
    println!(
        "# deadband occupancy, generation {} +/- {} eps, {} samples ({} on food) over {} layouts",
        r.generation, r.sigma, r.samples, r.on_food_samples, r.layouts.len()
    );
    println!(
        "# head edges: thrust {:.4}, |turn| {:.6}; reconstruction residual {:e}; {:.1} s",
        r.thrust_head_edge, r.turn_head_edge, r.max_reconstruction_residual, r.wall_seconds
    );
    println!("{:>10} {:>8} {:>8} {:>8} {:>8} {:>10} {:>10}", "candidate", "start", "thrust", "turn", "both", "head-thr", "|head-turn|");
    for row in &r.rows {
        println!(
            "{:>10} {:>8} {:>8.4} {:>8.4} {:>8.4} {:>10.4} {:>10.5}",
            row.candidate,
            row.start,
            row.thrust_fraction(),
            row.turn_fraction(),
            row.both_inside as f64 / row.samples.max(1) as f64,
            row.mean_head_thrust,
            row.mean_abs_head_turn
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::optimizer::{self, SIGMA};
    use crate::es::rng::perturbation;
    use crate::es::tensor::{self, PARAMS};
    use crate::es::trainer::{Candidate, Job};

    /// An episode with everything irrelevant to this module zeroed. The four columns the
    /// reduction reads are named; nothing else is allowed to influence a row.
    fn ep(layout: &str, ticks: u64, stores: f64, intake: f64, opening: u64) -> Episode {
        Episode {
            layout: layout.into(),
            ticks,
            alive: stores > 0.0,
            terminal_stores: stores,
            store_capacity: 4.0,
            intake_producer: intake,
            intake_fruit: 0.0,
            intake_detritus: 0.0,
            upkeep_billed: 0.0,
            motion_billed: 0.0,
            store_start: 2.5,
            travelled_px: 0.0,
            body_lengths: 0.0,
            distinct_cells: 0,
            ticks_in_opening: opening,
            turn_sweep_rad: 0.0,
            seam_crossing_ticks: 0,
            turn_unmeasured_ticks: 0,
            motion_billed_partial: false,
            died_on_last_tick: true,
            route_p_start: 0.0,
            route_p_end: 0.0,
            route_p_grown: 0.0,
            validations: 0,
        }
    }

    /// A report built exactly the way `run_generation` builds one: candidates in index order,
    /// layouts innermost, scores from the trainer's own `score_by`.
    ///
    /// `per_candidate[c][l]` is `(ticks, intake, opening)` for candidate index `c` on layout
    /// `l`; candidate index is `2·pair + sign`, the centre after every pair.
    fn report(
        generation: u64,
        layouts: &[&str],
        per_candidate: &[Vec<(u64, f64, u64)>],
    ) -> GenerationReport {
        let n = per_candidate.len() / 2;
        assert_eq!(per_candidate.len(), 2 * n, "a report holds whole pairs");
        let mut jobs = Vec::new();
        let mut scores = Vec::new();
        for (ci, cells) in per_candidate.iter().enumerate() {
            assert_eq!(cells.len(), layouts.len());
            let label = if ci % 2 == 0 {
                Candidate::Plus(ci / 2).label()
            } else {
                Candidate::Minus(ci / 2).label()
            };
            let eps: Vec<Episode> = layouts
                .iter()
                .zip(cells)
                .map(|(l, (t, i, o))| ep(l, *t, 0.0, *i, *o))
                .collect();
            scores.push(crate::es::trainer::score_by(Aggregate::Min, &eps));
            for e in eps {
                jobs.push(Job {
                    generation,
                    candidate: label.clone(),
                    layout: e.layout.clone(),
                    episode: e,
                });
            }
        }
        GenerationReport {
            generation,
            candidate_scores: scores,
            center_score: None,
            jobs,
            gradient_norm: 0.0,
            update_rms: 0.0,
            episodes_run: (2 * n * layouts.len()) as u64,
            ticks_run: 0,
            wall_seconds: 0.0,
        }
    }

    /// Two pairs, two layouts, chosen so every utility, weight and per-layout difference can be
    /// written down by hand. This is the whole pair table on a case with known answers.
    ///
    /// Scores are `t_min`: `[100, 90, 50, 300]` in candidate index order, so the population in
    /// the estimator's `[all +, all −]` order is `[100, 50, 90, 300]`, ranks `[2, 0, 1, 3]` and
    /// utilities `[1/6, −1/2, −1/6, 1/2]`.
    #[test]
    fn the_pair_table_is_the_hand_computed_one() {
        let r = report(
            7,
            &["a", "b"],
            &[
                vec![(100, 1.0, 10), (200, 1.0, 10)], // pair0+
                vec![(90, 2.0, 20), (400, 2.0, 20)],  // pair0−
                vec![(50, 0.0, 0), (60, 0.0, 0)],     // pair1+
                vec![(300, 0.0, 0), (310, 0.0, 0)],   // pair1−
            ],
        );
        let (row, pairs) = reduce_generation(&r, Aggregate::Min).expect("reduced");
        assert_eq!(pairs.len(), 2);

        let sixth = 1.0 / 6.0;
        let p0 = &pairs[0];
        assert_eq!((p0.score_plus, p0.score_minus, p0.d_score), (100.0, 90.0, 10.0));
        assert!((p0.u_plus - sixth).abs() < 1e-15, "u+ {}", p0.u_plus);
        assert!((p0.u_minus + sixth).abs() < 1e-15, "u− {}", p0.u_minus);
        assert!((p0.weight - 2.0 * sixth).abs() < 1e-15, "w {}", p0.weight);
        assert_eq!(p0.d_ticks, vec![10, -200]);
        assert_eq!(p0.layouts_favouring_plus, 1);
        assert_eq!((p0.binding_plus, p0.binding_minus), (0, 0));
        assert_eq!(p0.d_mean_ticks, 150.0 - 245.0);

        let p1 = &pairs[1];
        assert_eq!(p1.d_score, -250.0);
        assert!((p1.u_plus + 0.5).abs() < 1e-15);
        assert!((p1.u_minus - 0.5).abs() < 1e-15);
        assert!((p1.weight + 1.0).abs() < 1e-15);
        assert_eq!(p1.d_mean_ticks, 55.0 - 305.0);

        assert_eq!(row.generation, 7);
        assert_eq!((row.score_min, row.score_max), (50.0, 300.0));
        assert_eq!(row.best_candidate, 300.0);
        assert_eq!((row.best_candidate_pair, row.best_candidate_plus), (1, false));
        assert_eq!(row.informative_pairs, 2);
        assert_eq!(row.zero_weight_pairs, 0);
        // Both members of both pairs are bound by layout `a`.
        assert_eq!(row.binding_counts, vec![4, 0]);
        assert!((row.sum_abs_weight - (2.0 * sixth + 1.0)).abs() < 1e-15);
    }

    /// The four-layout minimum hiding a gain on three layouts: the plus member outlives the
    /// minus member on three of four and still loses the score, so the estimator is told the
    /// opposite of what the mean says. That disagreement is the column, and it must not fire on
    /// the pair where the two agree.
    #[test]
    fn the_minimum_masks_a_pair_that_wins_three_layouts_of_four() {
        let r = report(
            0,
            &["a", "b", "c", "d"],
            &[
                // plus wins b, c and d by a lot and loses a by 10.
                vec![(90, 0.0, 0), (900, 0.0, 0), (900, 0.0, 0), (900, 0.0, 0)],
                vec![(100, 0.0, 0), (110, 0.0, 0), (110, 0.0, 0), (110, 0.0, 0)],
                // plus loses everywhere: no disagreement to find.
                vec![(50, 0.0, 0), (50, 0.0, 0), (50, 0.0, 0), (50, 0.0, 0)],
                vec![(70, 0.0, 0), (70, 0.0, 0), (70, 0.0, 0), (70, 0.0, 0)],
            ],
        );
        let (row, pairs) = reduce_generation(&r, Aggregate::Min).expect("reduced");

        assert_eq!(pairs[0].layouts_favouring_plus, 3);
        assert_eq!(pairs[0].d_score, -10.0, "the minimum decides against the plus member");
        assert_eq!(pairs[0].d_mean_ticks, 697.5 - 107.5);
        assert!(pairs[0].weight < 0.0, "so the estimator is pushed away from it");
        assert!(pairs[0].minimum_masked);

        assert_eq!(pairs[1].layouts_favouring_plus, 0);
        assert!(!pairs[1].minimum_masked);
        assert_eq!(row.minimum_masked, 1);
        // Every candidate of pair 0 is bound by layout `a`; pair 1 ties on all four, so the
        // first minimum is layout `a` there too.
        assert_eq!(row.binding_counts, vec![4, 0, 0, 0]);
    }

    /// Concordance is about the member the estimator *prefers*, not about the plus member: on a
    /// pair with a negative weight the minus member is the one that must carry the intake.
    #[test]
    fn concordance_follows_the_weight_not_the_sign_of_the_pair() {
        let r = report(
            0,
            &["a"],
            &[
                // pair0: plus scores higher (weight > 0) but eats less -> discordant.
                vec![(200, 1.0, 100)],
                vec![(100, 9.0, 900)],
                // pair1: minus scores higher (weight < 0) and eats more -> concordant.
                vec![(50, 1.0, 10)],
                vec![(300, 7.0, 70)],
            ],
        );
        let (row, pairs) = reduce_generation(&r, Aggregate::Min).expect("reduced");

        assert!(pairs[0].weight > 0.0);
        assert_eq!(pairs[0].d_intake, 1.0 - 9.0);
        assert_eq!(pairs[0].concordant(pairs[0].d_intake), Some(false));
        assert!(pairs[1].weight < 0.0);
        assert_eq!(pairs[1].concordant(pairs[1].d_intake), Some(true));

        assert_eq!(row.concordant_intake, 1);
        assert_eq!(row.concordant_opening, 1);
        // Rate columns divide by lived ticks: pair 0's plus member eats 1/200 against the minus
        // member's 9/100, so the rate disagrees with the estimator too.
        assert_eq!(pairs[0].d_intake_rate, 1.0 / 200.0 - 9.0 / 100.0);
        assert_eq!(pairs[0].d_opening_fraction, 100.0 / 200.0 - 900.0 / 100.0);
        assert_eq!(row.concordant_intake_rate, 1);
    }

    /// An exact tie inside a pair makes its weight exactly zero: the pair contributes nothing
    /// to the update, so it must not be counted as evidence for or against concordance either.
    #[test]
    fn a_tied_pair_is_weightless_and_carries_no_evidence() {
        let r = report(
            0,
            &["a"],
            &[
                vec![(100, 1.0, 10)],
                vec![(100, 9.0, 90)], // identical score, very different feeding
                vec![(50, 0.0, 0)],
                vec![(300, 1.0, 1)],
            ],
        );
        let (row, pairs) = reduce_generation(&r, Aggregate::Min).expect("reduced");

        assert_eq!(pairs[0].weight, 0.0, "an exact tie shares its average rank");
        assert!(!pairs[0].informative());
        assert_eq!(pairs[0].concordant(pairs[0].d_intake), None);
        assert_eq!(row.zero_weight_pairs, 1);
        assert_eq!(row.informative_pairs, 1);
        assert_eq!(row.concordant_intake, 1, "only the second pair can be counted");
    }

    /// The weights this module extracts must be the ones the trainer's own estimator uses. The
    /// check is against `optimizer::gradient` itself on the same report: same perturbations,
    /// same sigma, same answer to the last bit of the norm.
    #[test]
    fn the_weights_reproduce_the_trainers_own_gradient() {
        let dim = 64;
        let r = report(
            3,
            &["a", "b"],
            &[
                vec![(100, 0.0, 0), (400, 0.0, 0)],
                vec![(90, 0.0, 0), (900, 0.0, 0)],
                vec![(700, 0.0, 0), (20, 0.0, 0)],
                vec![(310, 0.0, 0), (310, 0.0, 0)],
            ],
        );
        let (_, pairs) = reduce_generation(&r, Aggregate::Min).expect("reduced");
        let weights: Vec<f64> = pairs.iter().map(|p| p.weight).collect();

        let n = pairs.len();
        let plus: Vec<f64> = (0..n).map(|p| r.candidate_scores[2 * p]).collect();
        let minus: Vec<f64> = (0..n).map(|p| r.candidate_scores[2 * p + 1]).collect();
        let g = optimizer::gradient(&plus, &minus, dim, SIGMA, |i, out| {
            perturbation(5, 3, i as u64, out);
        });
        let truth = g.iter().map(|x| x * x).sum::<f64>().sqrt();

        // Rebuilt from the extracted weights alone.
        let mut mine = vec![0.0; dim];
        let mut e = vec![0.0; dim];
        for (i, w) in weights.iter().enumerate() {
            perturbation(5, 3, i as u64, &mut e);
            for j in 0..dim {
                mine[j] += w * e[j];
            }
        }
        let denom = 2.0 * n as f64 * SIGMA;
        let got = mine.iter().map(|x| (x / denom).powi(2)).sum::<f64>().sqrt();
        assert!((got - truth).abs() < 1e-12 * truth.max(1.0), "{got} vs {truth}");
    }

    /// The two ends of the cancellation scale, hand-built: two pairs on the *same* direction
    /// with opposite weights lose everything, and with equal weights lose nothing. The
    /// orthogonal reference is what separates those from the ordinary case.
    #[test]
    fn cancellation_reports_both_ends_and_its_orthogonal_reference() {
        let dim = 4;
        let one = |_: usize, out: &mut [f64]| {
            out.copy_from_slice(&[1.0, 0.0, 0.0, 0.0]);
        };
        let opposed = cancellation(&[1.0, -1.0], dim, one);
        assert_eq!(opposed.summed_norm, 0.0);
        assert_eq!(opposed.no_cancellation_norm, 2.0);
        assert_eq!(opposed.retained, 0.0);
        assert_eq!(opposed.excess, 0.0);

        let aligned = cancellation(&[1.0, 1.0], dim, one);
        assert_eq!(aligned.summed_norm, 2.0);
        assert_eq!(aligned.retained, 1.0);
        // Two aligned unit directions beat the orthogonal reference by sqrt(2).
        assert!((aligned.excess - 2.0f64.sqrt()).abs() < 1e-15, "{}", aligned.excess);

        // Genuinely orthogonal directions sit exactly on the reference.
        let axes = |i: usize, out: &mut [f64]| {
            out.iter_mut().for_each(|v| *v = 0.0);
            out[i] = 1.0;
        };
        let orth = cancellation(&[1.0, 1.0], dim, axes);
        assert!((orth.excess - 1.0).abs() < 1e-15);
        assert!((orth.retained - 1.0 / 2.0f64.sqrt()).abs() < 1e-15);
    }

    /// The replay must be the trainer's own arithmetic, not a second implementation of it: two
    /// hand-built generations replayed here must land on exactly the centre that
    /// `optimizer::gradient` plus `Adam::ascend` produce directly, bit for bit.
    #[test]
    fn the_replay_is_the_trainers_own_gradient_and_ascent() {
        let protocol = Protocol::default();
        let n = protocol.pairs;
        let layouts = ["a", "b"];
        let mut reports = Vec::new();
        for g in 0..2u64 {
            let cells: Vec<Vec<(u64, f64, u64)>> = (0..2 * n)
                .map(|c| {
                    let t = 1_000 + 37 * (c as u64) + 11 * g;
                    vec![(t, 0.0, 0), (t + 5, 0.0, 0)]
                })
                .collect();
            reports.push(report(g, &layouts, &cells));
        }

        let theta0 = tensor::initial_center(protocol.train_seed);
        let (geo, theta, adam) =
            replay(&theta0, &reports, &protocol, Aggregate::Min).expect("replayed");
        assert_eq!(geo.len(), 2);
        assert_eq!(adam.step, 2);

        let mut want = theta0.clone();
        let mut want_adam = Adam::new(PARAMS);
        for (g, r) in reports.iter().enumerate() {
            let plus: Vec<f64> = (0..n).map(|p| r.candidate_scores[2 * p]).collect();
            let minus: Vec<f64> = (0..n).map(|p| r.candidate_scores[2 * p + 1]).collect();
            let grad = optimizer::gradient(&plus, &minus, PARAMS, protocol.sigma, |i, out| {
                perturbation(protocol.train_seed, g as u64, i as u64, out);
            });
            let norm = grad.iter().map(|x| x * x).sum::<f64>().sqrt();
            assert_eq!(geo[g].gradient_norm_recomputed, norm);
            want_adam.ascend(&mut want, &grad);
        }
        assert_eq!(theta, want, "the replay must be bit-identical to the trainer's ascent");
        assert_eq!(adam, want_adam);
    }

    /// The step's geometry against a known answer: with only one informative pair the applied
    /// step is Adam's sign-like displacement along that one direction, so the progress toward
    /// the best candidate is a number that can be checked rather than merely reported.
    #[test]
    fn the_geometry_measures_the_step_against_the_best_candidate() {
        let protocol = Protocol::default();
        let n = protocol.pairs;
        // Every pair ties except pair 0, whose plus member is the population's best.
        let mut cells: Vec<Vec<(u64, f64, u64)>> =
            (0..2 * n).map(|_| vec![(1_000, 0.0, 0)]).collect();
        cells[0] = vec![(9_000, 0.0, 0)];
        let r = report(0, &["a"], &cells);
        let theta0 = tensor::initial_center(protocol.train_seed);
        let (geo, theta, _) =
            replay(&theta0, &[r], &protocol, Aggregate::Min).expect("replayed");
        let g = &geo[0];

        assert_eq!(g.generation, 0);
        assert!(g.cos_step_best > 0.0, "the step must point at the best candidate at all");
        let mut eps = vec![0.0; PARAMS];
        perturbation(protocol.train_seed, 0, 0, &mut eps);
        let radius = protocol.sigma * eps.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((g.best_distance - radius).abs() < 1e-12 * radius);

        // The recorded progress is the displacement's own projection, checked independently.
        let dot: f64 =
            theta.iter().zip(&theta0).zip(&eps).map(|((a, b), e)| (a - b) * e).sum();
        let want = dot / (protocol.sigma * eps.iter().map(|x| x * x).sum::<f64>());
        assert!((g.progress_toward_best - want).abs() < 1e-12 * want.abs().max(1.0));
        // One Adam step is `lr` per coordinate, so it cannot reach a candidate `sigma` away in
        // 10,215 dimensions: the centre moves a fixed fraction of the radius, never past it.
        assert!(g.progress_toward_best < 1.0, "{}", g.progress_toward_best);
        assert!((g.step_rms - protocol.learning_rate).abs() < 1e-6, "{}", g.step_rms);
    }

    /// Deadband membership is the adapter's own, on weights whose head is known exactly: with
    /// every matrix zero the head *is* `b_o`, so each channel's side of the band is chosen by
    /// hand and the count has one right answer.
    #[test]
    fn the_deadband_predicate_is_the_adapters_own_zero() {
        let cap = Capability::ordinary(1.0, 1.0, true, true);
        let x = [0.25f64; OBS_LEN];
        let h = [0.0f64; HIDDEN];

        let mut inside = Gru32::zeros();
        inside.b_o[THRUST] = -5.0; // sigmoid(−5) = 0.0067 < 0.05
        inside.b_o[TURN] = 0.0; // tanh(0) = 0
        let (t_in, r_in, head_t, head_r) = movement_inside(&inside, &cap, &x, &h);
        assert!(t_in && r_in);
        assert_eq!((head_t, head_r), (-5.0, 0.0));

        let mut outside = Gru32::zeros();
        outside.b_o[THRUST] = 5.0; // sigmoid(5) = 0.993
        outside.b_o[TURN] = 1.0; // tanh(1) = 0.76
        let (t_out, r_out, _, _) = movement_inside(&outside, &cap, &x, &h);
        assert!(!t_out && !r_out);

        // And the reported head edges really are the band's, inverted from the adapter's own
        // constant rather than written down twice.
        let (edge_t, edge_r) = deadband_head_edges();
        assert!((1.0 / (1.0 + (-edge_t).exp()) - DEADBAND).abs() < 1e-12);
        assert!((edge_r.tanh() - DEADBAND).abs() < 1e-12);
    }

    /// Two samples, two starts, counts that can be read off by hand — and a row whose mean head
    /// values come from the same passes as the counts.
    #[test]
    fn deadband_rows_count_both_starts_over_the_sample_set() {
        let cap = Capability::ordinary(1.0, 1.0, true, true);
        let mut w = Gru32::zeros();
        w.b_o[THRUST] = -5.0;
        w.b_o[TURN] = 2.0;
        let samples = vec![
            Sample { tick: 1, on_food: true, obs: vec![0.0; OBS_LEN], hidden: vec![0.5; HIDDEN] },
            Sample { tick: 2, on_food: false, obs: vec![1.0; OBS_LEN], hidden: vec![0.0; HIDDEN] },
        ];
        let rows = deadband_rows("hand-built", &w, &cap, &samples);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].start, "reset");
        assert_eq!(rows[1].start, "carried");
        for row in &rows {
            assert_eq!(row.samples, 2);
            assert_eq!(row.thrust_inside, 2, "sigmoid(−5) is inside the band at every state");
            assert_eq!(row.turn_inside, 0, "tanh(2) is not");
            assert_eq!(row.both_inside, 0);
            assert_eq!(row.mean_head_thrust, -5.0);
            assert_eq!(row.mean_abs_head_turn, 2.0);
            assert_eq!(row.thrust_fraction(), 1.0);
            assert_eq!(row.turn_fraction(), 0.0);
        }
    }

    /// A report whose jobs are in the trainer's order must hand back the plan's layout order,
    /// and a candidate missing a layout must be refused rather than silently scored on fewer.
    #[test]
    fn the_layout_order_is_the_plans_and_a_short_candidate_is_refused() {
        let r = report(
            0,
            &["zulu", "alpha"],
            &[vec![(10, 0.0, 0), (20, 0.0, 0)], vec![(30, 0.0, 0), (40, 0.0, 0)]],
        );
        let layouts = layout_order(&r).expect("order");
        assert_eq!(layouts, vec!["zulu".to_string(), "alpha".to_string()]);
        assert_eq!(
            candidate_episodes(&r, "pair0+", &layouts).expect("episodes")[0].ticks,
            10
        );

        let mut short = r.clone();
        short.jobs.retain(|j| !(j.candidate == "pair0+" && j.layout == "alpha"));
        assert!(candidate_episodes(&short, "pair0+", &layouts).is_err());
        assert!(reduce_generation(&short, Aggregate::Min).is_err());
    }
}
