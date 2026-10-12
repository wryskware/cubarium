//! **The checks a run can switch on** (§11.7 "Checks", §10.3 "Noise check first", §10.4
//! "Readouts"), for the matched comparison:
//!
//! - **frozen-brain noise**: null pairs ([`super::matched::null_pair`]), two copies with
//!   identical brains; their gap is the noise floor a signal is read against. (On the live
//!   per-body comparison the frozen-brain run is the live loop at `σ = 0`, as before.)
//! - **signal**: repeated matched differences in a consistent direction, how often the
//!   focal's decisions change between its two copies, and candidate coverage
//!   ([`SignalReadout`]);
//! - **held-out energy improvement**: the focal on the new centre against the old one, on
//!   matched starts the update never saw ([`held_out`]).
//!
//! The raw own, relatives' and combined scores per body are in every [`super::KinParts`]
//! (`BodyScore::kin` in the live loop's log; [`super::matched::CopyOutcome::scores`] here).

use cubarium_voxel::registry::{Range, Registry, Row};
use cubarium_voxel_fauna::Founder;
use serde::{Deserialize, Serialize};

use super::config::KinConfig;
use super::matched::{self, CopyPlan, MatchedPair, NullPair, Scene};
use crate::es::voxel::live::variants::VariantTable;

/// How much checking a matched generation does. Counts, per generation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MatchedChecks {
    /// Matched pairs run per scene (rotating focal, pair and sign), count ≥ 1.
    pub rounds_per_scene: f64,
    /// Null pairs per scene for the frozen-brain noise floor (0: off), count.
    pub null_pairs_per_scene: f64,
    /// Held-out scenes the new centre is compared on against the old (0: off), count. The
    /// caller runs [`held_out`] on this many scenes it kept out of the update.
    pub held_out_scenes: f64,
    /// Worker threads for a pair's two copies (1 or 2), count.
    pub copy_threads: f64,
}

impl Default for MatchedChecks {
    fn default() -> MatchedChecks {
        MatchedChecks { rounds_per_scene: 1.0, null_pairs_per_scene: 0.0, held_out_scenes: 0.0, copy_threads: 2.0 }
    }
}

/// The matched checks' registry rows (root `matched`).
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        Registry::new(
            "matched",
            vec![
                Row::new("matched.rounds_per_scene", "count", Range::at_least(1.0), "KIN-TRAIN: matched pairs run per scene each generation (focal, pair and sign rotate)"),
                Row::new("matched.null_pairs_per_scene", "count", Range::nonneg(), "KIN-TRAIN: null pairs (identical brains) per scene for the frozen-brain noise floor; 0 = off"),
                Row::new("matched.held_out_scenes", "count", Range::nonneg(), "KIN-TRAIN: held-out scenes the updated centre is compared on against the old; 0 = off"),
                Row::new("matched.copy_threads", "count", Range::closed(1.0, 2.0), "KIN-TRAIN: threads running a matched pair's two copies"),
            ],
            Vec::new(),
            Vec::new(),
        )
    })
}

/// The noise floor from null pairs: the spread of the focal's gap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct NoiseReadout {
    pub pairs: usize,
    /// Root mean square of the focal's combined-score gap.
    pub rms_gap: f64,
    pub max_gap: f64,
    /// Null pairs whose copies were bit-identical in every body's score and decision.
    pub identical: usize,
}

impl NoiseReadout {
    pub fn of(pairs: &[NullPair]) -> NoiseReadout {
        let n = pairs.len();
        NoiseReadout {
            pairs: n,
            rms_gap: (pairs.iter().map(|p| p.focal_gap * p.focal_gap).sum::<f64>() / n.max(1) as f64).sqrt(),
            max_gap: pairs.iter().map(|p| p.focal_gap).fold(0.0, f64::max),
            identical: pairs
                .iter()
                .filter(|p| p.max_combined_gap == 0.0 && p.unmatched_bodies == 0 && p.decisions_changed == 0.0)
                .count(),
        }
    }
}

/// The signal check over one generation's matched pairs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct SignalReadout {
    /// Matched pairs with both signs scored.
    pub complete: usize,
    /// Of the generation's pairs, the share with at least one complete matched result.
    pub coverage: f64,
    /// Mean `|combined(+) − combined(−)|` over complete results.
    pub mean_abs_difference: f64,
    /// `mean_abs_difference` over the noise floor's rms gap (infinite over a zero floor;
    /// NaN without a noise readout).
    pub signal_to_noise: f64,
    /// Over pairs with two or more complete results, the share of results whose sign
    /// agrees with their pair's mean difference (0.5 is chance, 1 every repeat agrees).
    pub consistent_share: f64,
    /// Results that entered `consistent_share`.
    pub repeated: usize,
    /// Mean share of the focal's samples whose decision differed between its copies.
    pub decisions_changed: f64,
}

impl SignalReadout {
    pub fn of(results: &[MatchedPair], pairs: usize, noise: Option<&NoiseReadout>) -> SignalReadout {
        let diffs: Vec<(usize, f64)> = results.iter().filter_map(|r| Some((r.plan.pair, r.difference()?))).collect();
        let complete = diffs.len();
        let mean_abs = diffs.iter().map(|d| d.1.abs()).sum::<f64>() / complete.max(1) as f64;
        let covered = (0..pairs).filter(|p| diffs.iter().any(|d| d.0 == *p)).count();
        let (mut agree, mut repeated) = (0usize, 0usize);
        for p in 0..pairs {
            let mine: Vec<f64> = diffs.iter().filter(|d| d.0 == p).map(|d| d.1).collect();
            if mine.len() < 2 {
                continue;
            }
            let mean = mine.iter().sum::<f64>() / mine.len() as f64;
            repeated += mine.len();
            agree += mine.iter().filter(|d| d.signum() == mean.signum() && **d != 0.0).count();
        }
        SignalReadout {
            complete,
            coverage: covered as f64 / pairs.max(1) as f64,
            mean_abs_difference: mean_abs,
            signal_to_noise: noise.map_or(f64::NAN, |n| if n.rms_gap > 0.0 { mean_abs / n.rms_gap } else { f64::INFINITY }),
            consistent_share: if repeated > 0 { agree as f64 / repeated as f64 } else { f64::NAN },
            repeated,
            decisions_changed: results.iter().map(|r| r.decisions_changed).sum::<f64>() / results.len().max(1) as f64,
        }
    }
}

/// The held-out comparison: on each scene, every focal on the old centre and on the new,
/// all else identical; the mean gain of the focal's combined score.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct HeldOut {
    pub comparisons: usize,
    pub mean_gain: f64,
    /// Comparisons the new centre won.
    pub wins: usize,
}

/// Run the held-out comparison of `old` against `new` for `lineage` on `scenes`.
pub fn held_out<S: Scene>(
    scenes: &[S],
    lineage: Founder,
    old: &[f64],
    new: &[f64],
    kin: &KinConfig,
    threads: usize,
) -> Result<HeldOut, String> {
    let table = VariantTable::build(lineage, &[old.to_vec(), new.to_vec()])?;
    let mut gains = Vec::new();
    for scene in scenes {
        let focals = {
            let copy = scene.open()?;
            scene.focals(&copy, lineage)
        };
        for focal in focals {
            let plans = vec![CopyPlan { focal, candidate: Some(0) }, CopyPlan { focal, candidate: Some(1) }];
            let mut out = crate::es::voxel::live::trainer::parallel(plans, threads.clamp(1, 2), |p| {
                matched::run_copy(scene, p, Some(&table), kin)
            })
            .into_iter();
            let (a, b) = (out.next().expect("old")?, out.next().expect("new")?);
            if let (Some(x), Some(y)) = (a.focal(), b.focal()) {
                gains.push(y.combined - x.combined);
            }
        }
    }
    Ok(HeldOut {
        comparisons: gains.len(),
        mean_gain: gains.iter().sum::<f64>() / gains.len().max(1) as f64,
        wins: gains.iter().filter(|g| **g > 0.0).count(),
    })
}

/// One matched generation's results and checks.
#[derive(Clone, Debug, Serialize)]
pub struct MatchedGeneration {
    pub pairs: Vec<MatchedPair>,
    pub noise: Option<NoiseReadout>,
    pub null_pairs: Vec<NullPair>,
    pub signal: SignalReadout,
}

/// Run one generation of the matched comparison for `lineage` over `scenes`: the rotation
/// on each, then the noise check if switched on. The update's scores are
/// [`matched::pair_scores`] of `pairs`.
pub fn matched_generation<S: Scene>(
    scenes: &[S],
    lineage: Founder,
    table: &VariantTable,
    kin: &KinConfig,
    checks: &MatchedChecks,
) -> Result<MatchedGeneration, String> {
    registry().check(checks)?;
    let pairs_n = table.len() / 2;
    let threads = checks.copy_threads as usize;
    let (mut pairs, mut nulls) = (Vec::new(), Vec::new());
    let mut round = 0usize;
    for scene in scenes {
        let focals = {
            let copy = scene.open()?;
            scene.focals(&copy, lineage)
        };
        let rounds = checks.rounds_per_scene as usize;
        // The rotation runs on across scenes, so pairs and sign slots spread over them.
        let plans = matched::rotate(&focals, pairs_n, round + rounds);
        for plan in plans.into_iter().skip(round) {
            pairs.push(matched::run_pair(scene, plan, table, kin, threads)?);
        }
        round += rounds;
        for k in 0..checks.null_pairs_per_scene as usize {
            if focals.is_empty() {
                break;
            }
            let focal = focals[k % focals.len()];
            let c = (2 * k) % table.len().max(1);
            nulls.push(matched::null_pair(scene, CopyPlan { focal, candidate: Some(c) }, Some(table), kin, threads)?);
        }
    }
    let noise = (!nulls.is_empty()).then(|| NoiseReadout::of(&nulls));
    let signal = SignalReadout::of(&pairs, pairs_n, noise.as_ref());
    Ok(MatchedGeneration { pairs, noise, null_pairs: nulls, signal })
}
