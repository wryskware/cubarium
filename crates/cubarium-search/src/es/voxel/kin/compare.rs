//! **The comparison switch** (§11.7, topic 10): how a lineage's bodies' kin-weighted scores
//! become one score per antithetic candidate, and the update from them.
//!
//! - `per_body`: today's [`pooled_scores`], z-scored within each world and lineage, over
//!   the kin-weighted scores. At every share 0 this is today's training exactly, a candidate
//!   no body ran still scored 0 as before. With shares above 0 and one pack per world the
//!   shared term cancels ([`super::tests`] shows it).
//! - `family`: one variant per family ([`place_families`]); a family's score is the mean of
//!   its members' kin-weighted scores, compared **raw** across worlds (no within-world
//!   normalising, which would zero a world holding one family).
//! - `matched`: each pair's two signs come from the matched runner ([`super::matched`]).
//! - `twins`: each pair's `(d̄/2, −d̄/2)` from its all-sign twins ([`super::twins`]).
//!
//! **Missing signs are never scored as zero returns** on `family`, `matched` and `twins`: a pair
//! whose `+` or `−` has no score is left out of the update ([`PairScores`],
//! [`ascend_pairs`]).

use rustc_hash::FxHashMap;
use serde::Serialize;

use super::roster::Roster;
use crate::es::optimizer::{self, Adam};
use crate::es::voxel::live::trainer::pooled_scores;
use crate::es::voxel::live::window::{BodyScore, Placed, place_twins};

/// **Family placement**: each family of `bodies` (by the roster) is one unit, units sorted
/// around the ring by their first member, twins on neighbouring units, pairs from
/// `first_pair` (as [`place_twins`] places bodies). Every member gets its unit's candidate.
pub fn place_families(bodies: &[Placed], roster: &Roster, pairs: usize, first_pair: usize) -> Vec<(u64, usize)> {
    let here: FxHashMap<u64, f64> = bodies.iter().map(|b| (b.id, b.x_m)).collect();
    let mut units: Vec<Vec<u64>> = roster
        .families()
        .into_iter()
        .map(|f| f.into_iter().filter(|id| here.contains_key(id)).collect::<Vec<_>>())
        .filter(|f| !f.is_empty())
        .collect();
    // A body off the roster (not possible in a window, kept for callers) is its own unit.
    for b in bodies {
        if !roster.contains(b.id) {
            units.push(vec![b.id]);
        }
    }
    let reps: Vec<Placed> = units.iter().map(|u| Placed { id: u[0], x_m: here[&u[0]] }).collect();
    let by_rep: FxHashMap<u64, usize> = place_twins(&reps, pairs, first_pair).into_iter().collect();
    units
        .iter()
        .flat_map(|u| {
            let c = by_rep[&u[0]];
            u.iter().map(move |id| (*id, c))
        })
        .collect()
}

/// One lineage's candidate scores for one update, by pair: `(pair, plus, minus)` for every
/// pair that has both signs. Pairs missing a sign are counted, never zero-filled.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct PairScores {
    pub pairs: Vec<(usize, f64, f64)>,
    /// Pairs left out for a missing sign.
    pub missing: usize,
}

impl PairScores {
    /// From per-candidate scores where `None` is a sign nobody scored.
    pub fn from_candidates(scores: &[Option<f64>]) -> PairScores {
        let mut out = PairScores::default();
        for p in 0..scores.len() / 2 {
            match (scores[2 * p], scores[2 * p + 1]) {
                (Some(a), Some(b)) => out.pairs.push((p, a, b)),
                _ => out.missing += 1,
            }
        }
        out
    }

    /// Every candidate in order, `None` where its pair was left out.
    pub fn candidates(&self, n: usize) -> Vec<Option<f64>> {
        let mut v = vec![None; n];
        for &(p, a, b) in &self.pairs {
            v[2 * p] = Some(a);
            v[2 * p + 1] = Some(b);
        }
        v
    }
}

/// The `per_body` scores: today's [`pooled_scores`] over each body's kin-weighted
/// (`kin.combined`) score. Returns `(scores by candidate, bodies by candidate)`.
pub fn per_body(per_world: &[&[BodyScore]], candidates: usize) -> (Vec<f64>, Vec<usize>) {
    let per_world: Vec<Vec<(usize, f64)>> = per_world
        .iter()
        .map(|bodies| bodies.iter().map(|b| (b.candidate, b.kin.combined)).collect())
        .collect();
    pooled_scores(&per_world, candidates)
}

/// The `family` scores: a family (bodies of one candidate that share a roster family in
/// one world) scores the mean of its members' kin-weighted scores; a candidate scores the
/// mean over its families in every world, raw; `None` for a candidate no family ran.
pub fn family(per_world: &[(&[BodyScore], &Roster)], candidates: usize) -> Vec<Option<f64>> {
    let mut sum = vec![0.0; candidates];
    let mut n = vec![0usize; candidates];
    for (bodies, roster) in per_world {
        let by_id: FxHashMap<u64, &BodyScore> = bodies.iter().map(|b| (b.body_id, b)).collect();
        for fam in roster.families() {
            // Group the family's scored members by candidate (a newborn whose mother died
            // before its birth may sit on another candidate).
            let mut groups: FxHashMap<usize, (f64, usize)> = FxHashMap::default();
            for id in fam {
                if let Some(b) = by_id.get(&id) {
                    let g = groups.entry(b.candidate).or_default();
                    g.0 += b.kin.combined;
                    g.1 += 1;
                }
            }
            for (c, (s, k)) in groups {
                sum[c] += s / k as f64;
                n[c] += 1;
            }
        }
    }
    sum.iter().zip(&n).map(|(s, &k)| (k > 0).then(|| s / k as f64)).collect()
}

/// The antithetic update over the pairs that have both signs: the frozen trainer's
/// centred-rank gradient and Adam ascent, each kept pair's perturbation drawn by `eps`
/// from its own pair index. Returns `(‖g‖, update rms)`; nothing moves with no pair.
pub fn ascend_pairs(
    theta: &mut [f64],
    adam: &mut Adam,
    scores: &PairScores,
    sigma: f64,
    eps: impl Fn(usize, &mut [f64]),
) -> (f64, f64) {
    if scores.pairs.is_empty() || sigma <= 0.0 {
        return (0.0, 0.0);
    }
    let plus: Vec<f64> = scores.pairs.iter().map(|p| p.1).collect();
    let minus: Vec<f64> = scores.pairs.iter().map(|p| p.2).collect();
    let g = optimizer::gradient(&plus, &minus, theta.len(), sigma, |i, out| eps(scores.pairs[i].0, out));
    let norm = g.iter().map(|x| x * x).sum::<f64>().sqrt();
    let rms = adam.ascend(theta, &g);
    (norm, rms)
}
