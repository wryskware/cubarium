//! Antithetic Gaussian evolution strategies with centred-rank utilities, and the Adam ascent
//! that consumes its gradient estimate.
//!
//! The mechanics are the ones described in Salimans et al. (2017) §2.1 — mirrored sampling, a
//! fixed noise scale and a rank transformation of the returns. Nothing about their
//! infrastructure or their architecture is imported, and the rank transformation is an
//! optimisation device: it says nothing about whether the ecological objective below it is
//! the right one.
//!
//! # The update, exactly
//!
//! For `n` pairs, draw `epsilon_i ~ N(0, I)` (one per pair, shared by both signs) and evaluate
//! the `2n` candidates `theta ± sigma·epsilon_i`.
//!
//! 1. Rank the `2n` **aggregate** candidate scores ascending, zero-based, with **average
//!    ranks** for exact ties.
//! 2. `u = rank / (2n - 1) - 0.5`, so the utilities are centred on zero and span `[-0.5, 0.5]`.
//! 3. `g = Σ_i (u_plus_i - u_minus_i) · epsilon_i / (2 n sigma)`.
//! 4. `theta <- Adam-ascent(theta, g)`.
//!
//! Every constant is a fixed starting default: `sigma = 0.02`, `lr = 0.01`, `beta1 = 0.9`,
//! `beta2 = 0.999`, `eps = 1e-8`, bias correction on, no weight decay. There is no adaptive
//! sigma, no mutation rescaling and no automatic retry with different hyperparameters.

use serde::{Deserialize, Serialize};

/// Fixed perturbation scale.
pub const SIGMA: f64 = 0.02;
/// Adam learning rate (ascent).
pub const LEARNING_RATE: f64 = 0.01;
/// Adam first-moment decay.
pub const BETA1: f64 = 0.9;
/// Adam second-moment decay.
pub const BETA2: f64 = 0.999;
/// Adam denominator floor.
pub const ADAM_EPS: f64 = 1e-8;

/// Adam, applied as **ascent**: `theta += lr · m̂ / (sqrt(v̂) + eps)`.
///
/// `step` is the number of updates already applied, so the bias correction uses `t = step + 1`
/// on the next one. It is persisted with the moments, which is what makes a resumed run
/// identical to an uninterrupted one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Adam {
    #[serde(with = "crate::es::bits::hex_f64s")]
    pub m: Vec<f64>,
    #[serde(with = "crate::es::bits::hex_f64s")]
    pub v: Vec<f64>,
    pub step: u64,
    pub learning_rate: f64,
    pub beta1: f64,
    pub beta2: f64,
    pub eps: f64,
}

impl Adam {
    pub fn new(dim: usize) -> Adam {
        Adam {
            m: vec![0.0; dim],
            v: vec![0.0; dim],
            step: 0,
            learning_rate: LEARNING_RATE,
            beta1: BETA1,
            beta2: BETA2,
            eps: ADAM_EPS,
        }
    }

    /// One ascent step. Returns the RMS of the applied displacement, purely as a reported
    /// diagnostic.
    pub fn ascend(&mut self, theta: &mut [f64], gradient: &[f64]) -> f64 {
        assert_eq!(theta.len(), self.m.len(), "parameter/state dimension mismatch");
        assert_eq!(gradient.len(), self.m.len(), "gradient/state dimension mismatch");
        self.step += 1;
        let t = self.step as i32;
        let bc1 = 1.0 - self.beta1.powi(t);
        let bc2 = 1.0 - self.beta2.powi(t);
        let mut sq = 0.0;
        for i in 0..theta.len() {
            let g = gradient[i];
            self.m[i] = self.beta1 * self.m[i] + (1.0 - self.beta1) * g;
            self.v[i] = self.beta2 * self.v[i] + (1.0 - self.beta2) * g * g;
            let m_hat = self.m[i] / bc1;
            let v_hat = self.v[i] / bc2;
            let delta = self.learning_rate * m_hat / (v_hat.sqrt() + self.eps);
            theta[i] += delta;
            sq += delta * delta;
        }
        (sq / theta.len() as f64).sqrt()
    }

    pub fn is_finite(&self) -> bool {
        self.m.iter().chain(self.v.iter()).all(|x| x.is_finite())
    }
}

/// Zero-based ascending ranks, with the **average** rank shared by every member of an exact
/// tie. `[3, 1, 3]` ranks as `[1.5, 0, 1.5]`.
///
/// Ties are compared by exact `f64` equality. Two candidates that both died on the same tick
/// with both stores empty genuinely produce the same `f64`, and that is the case the average
/// rank exists for.
pub fn average_ranks(scores: &[f64]) -> Vec<f64> {
    assert!(scores.iter().all(|s| s.is_finite()), "a non-finite score is an experiment error");
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|a, b| {
        scores[*a]
            .partial_cmp(&scores[*b])
            .expect("scores are finite")
            .then(a.cmp(b))
    });
    let mut ranks = vec![0.0; scores.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i + 1;
        while j < order.len() && scores[order[j]] == scores[order[i]] {
            j += 1;
        }
        // Positions i..j all hold the same score: they share the mean of their positions.
        let mean = ((i + j - 1) as f64) / 2.0;
        for k in i..j {
            ranks[order[k]] = mean;
        }
        i = j;
    }
    ranks
}

/// Centred-rank utilities: `u = rank / (m - 1) - 0.5` over `m = 2n` candidates.
///
/// With a single candidate the utility is 0 — there is no ordering information in one sample,
/// and the alternative would be a division by zero.
pub fn centered_rank_utilities(scores: &[f64]) -> Vec<f64> {
    let m = scores.len();
    if m < 2 {
        return vec![0.0; m];
    }
    let denominator = (m - 1) as f64;
    average_ranks(scores)
        .into_iter()
        .map(|r| r / denominator - 0.5)
        .collect()
}

/// One generation's antithetic gradient estimate.
///
/// `plus[i]` and `minus[i]` are the aggregate scores of `theta + sigma·epsilon_i` and
/// `theta - sigma·epsilon_i`. `epsilon(i, out)` writes pair `i`'s perturbation; it is a
/// callback so a 10,215-dimensional vector is regenerated rather than stored `2n` times.
pub fn gradient<F>(plus: &[f64], minus: &[f64], dim: usize, sigma: f64, mut epsilon: F) -> Vec<f64>
where
    F: FnMut(usize, &mut [f64]),
{
    assert_eq!(plus.len(), minus.len(), "an antithetic pair has two scores");
    let n = plus.len();
    assert!(n > 0, "a generation needs at least one pair");
    // Utilities are assigned over the whole 2n population, in the order `[all +, all -]`.
    let mut all = Vec::with_capacity(2 * n);
    all.extend_from_slice(plus);
    all.extend_from_slice(minus);
    let u = centered_rank_utilities(&all);

    let mut g = vec![0.0; dim];
    let mut eps = vec![0.0; dim];
    let denominator = 2.0 * n as f64 * sigma;
    for i in 0..n {
        let weight = u[i] - u[n + i];
        if weight == 0.0 {
            // Nothing to add, but the perturbation is still *defined*; skipping it changes no
            // value because the summand is exactly zero.
            continue;
        }
        epsilon(i, &mut eps);
        for j in 0..dim {
            g[j] += weight * eps[j];
        }
    }
    for x in &mut g {
        *x /= denominator;
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::rng::perturbation;

    #[test]
    fn ranks_are_zero_based_ascending_with_average_ties() {
        assert_eq!(average_ranks(&[10.0, 20.0, 30.0]), vec![0.0, 1.0, 2.0]);
        assert_eq!(average_ranks(&[30.0, 10.0, 20.0]), vec![2.0, 0.0, 1.0]);
        assert_eq!(average_ranks(&[3.0, 1.0, 3.0]), vec![1.5, 0.0, 1.5]);
        assert_eq!(average_ranks(&[5.0; 4]), vec![1.5; 4]);
        // A three-way tie in the middle.
        assert_eq!(average_ranks(&[0.0, 2.0, 2.0, 2.0, 9.0]), vec![0.0, 2.0, 2.0, 2.0, 4.0]);
    }

    #[test]
    fn utilities_are_centred_and_span_the_unit_interval() {
        let u = centered_rank_utilities(&[1.0, 2.0, 3.0, 4.0]);
        let want = [-0.5, -1.0 / 6.0, 1.0 / 6.0, 0.5];
        for (a, b) in u.iter().zip(want) {
            assert!((a - b).abs() < 1e-15, "{a} vs {b}");
        }
        assert!((u.iter().sum::<f64>()).abs() < 1e-15);
    }

    /// Every candidate scoring the same must produce **exactly** zero gradient: no centre
    /// drift on a generation that learned nothing.
    #[test]
    fn equal_scores_give_a_zero_gradient_and_no_centre_movement() {
        let dim = 32;
        let n = 4;
        let plus = vec![7.5; n];
        let minus = vec![7.5; n];
        let g = gradient(&plus, &minus, dim, SIGMA, |i, out| {
            perturbation(99, 0, i as u64, out);
        });
        assert!(g.iter().all(|x| *x == 0.0), "a tie must give exactly zero, saw {g:?}");

        let mut theta = vec![0.25; dim];
        let before = theta.clone();
        let mut adam = Adam::new(dim);
        adam.ascend(&mut theta, &g);
        assert_eq!(theta, before, "a zero gradient must not move the centre");
    }

    /// The exact arithmetic of the estimator, against a hand-computed value.
    #[test]
    fn the_gradient_matches_the_formula_value_for_value() {
        let dim = 3;
        let n = 2;
        // Fixed, distinct perturbations so the ranks are unambiguous.
        let eps = [[1.0, 0.0, -2.0], [0.5, 1.5, 0.25]];
        // Scores, in the order [u+0, u+1, u-0, u-1] = [4, 1, 3, 2] -> ranks [3, 0, 2, 1].
        let plus = [4.0, 1.0];
        let minus = [3.0, 2.0];
        let u: Vec<f64> = [3.0, 0.0, 2.0, 1.0].iter().map(|r| r / 3.0 - 0.5).collect();
        let w0 = u[0] - u[2];
        let w1 = u[1] - u[3];
        let sigma = 0.02;
        let denominator = 2.0 * n as f64 * sigma;
        let want: Vec<f64> = (0..dim)
            .map(|j| (w0 * eps[0][j] + w1 * eps[1][j]) / denominator)
            .collect();

        let got = gradient(&plus, &minus, dim, sigma, |i, out| out.copy_from_slice(&eps[i]));
        for j in 0..dim {
            assert!((got[j] - want[j]).abs() < 1e-15, "element {j}: {} vs {}", got[j], want[j]);
        }
        // And the documented `1 / sigma` scaling: halving sigma doubles the estimate.
        let half = gradient(&plus, &minus, dim, sigma / 2.0, |i, out| out.copy_from_slice(&eps[i]));
        for j in 0..dim {
            assert!((half[j] - 2.0 * want[j]).abs() < 1e-14);
        }
    }

    /// On a smooth analytic objective the estimate must point uphill. `f(x) = -||x - c||²` has
    /// the exact gradient `2(c - x)`; the rank transform destroys the magnitude, so direction
    /// is what is checked, and it is checked against the true direction, not against itself.
    #[test]
    fn the_estimate_points_uphill_on_an_analytic_objective() {
        let dim = 16;
        let target: Vec<f64> = (0..dim).map(|i| 0.1 * (i as f64) - 0.8).collect();
        let theta = vec![0.0; dim];
        let truth: Vec<f64> = (0..dim).map(|j| 2.0 * (target[j] - theta[j])).collect();

        let n = 256;
        let sigma = SIGMA;
        let f = |x: &[f64]| -> f64 {
            -x.iter().zip(&target).map(|(a, b)| (a - b) * (a - b)).sum::<f64>()
        };
        let mut eps = vec![0.0; dim];
        let mut plus = Vec::with_capacity(n);
        let mut minus = Vec::with_capacity(n);
        let mut cand = vec![0.0; dim];
        for i in 0..n {
            perturbation(4242, 0, i as u64, &mut eps);
            for j in 0..dim {
                cand[j] = theta[j] + sigma * eps[j];
            }
            plus.push(f(&cand));
            for j in 0..dim {
                cand[j] = theta[j] - sigma * eps[j];
            }
            minus.push(f(&cand));
        }
        let g = gradient(&plus, &minus, dim, sigma, |i, out| {
            perturbation(4242, 0, i as u64, out);
        });

        let dot: f64 = g.iter().zip(&truth).map(|(a, b)| a * b).sum();
        let ng = g.iter().map(|x| x * x).sum::<f64>().sqrt();
        let nt = truth.iter().map(|x| x * x).sum::<f64>().sqrt();
        let cosine = dot / (ng * nt);
        assert!(cosine > 0.9, "cosine to the true gradient was {cosine}");

        // And ascent actually climbs: 60 steps from the origin must reduce the distance.
        let mut x = theta.clone();
        let mut adam = Adam::new(dim);
        let before = f(&x);
        for g_i in 0..60u64 {
            let mut p = Vec::with_capacity(16);
            let mut m = Vec::with_capacity(16);
            for i in 0..16u64 {
                perturbation(4242, g_i, i, &mut eps);
                for j in 0..dim {
                    cand[j] = x[j] + sigma * eps[j];
                }
                p.push(f(&cand));
                for j in 0..dim {
                    cand[j] = x[j] - sigma * eps[j];
                }
                m.push(f(&cand));
            }
            let g = gradient(&p, &m, dim, sigma, |i, out| perturbation(4242, g_i, i as u64, out));
            adam.ascend(&mut x, &g);
        }
        assert!(f(&x) > before, "ascent did not improve: {before} -> {}", f(&x));
    }

    /// The Adam state is the whole state: stopping after two steps and resuming must produce
    /// the same centre as three uninterrupted steps, bit for bit.
    #[test]
    fn adam_continues_a_resumed_run_exactly() {
        let dim = 12;
        let gradients: Vec<Vec<f64>> = (0..3)
            .map(|k| (0..dim).map(|j| ((k * dim + j) as f64).sin()).collect())
            .collect();

        let mut theta_a = vec![0.3; dim];
        let mut adam_a = Adam::new(dim);
        for g in &gradients {
            adam_a.ascend(&mut theta_a, g);
        }

        let mut theta_b = vec![0.3; dim];
        let mut adam_b = Adam::new(dim);
        for g in &gradients[..2] {
            adam_b.ascend(&mut theta_b, g);
        }
        // Round-trip the optimizer state exactly as a checkpoint would.
        let json = serde_json::to_string(&adam_b).expect("serialize");
        let mut resumed: Adam = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(resumed, adam_b);
        assert_eq!(resumed.step, 2);
        resumed.ascend(&mut theta_b, &gradients[2]);

        assert_eq!(theta_a, theta_b, "a resumed run must match an uninterrupted one exactly");
        assert_eq!(adam_a, resumed);
    }

    #[test]
    fn a_finite_gradient_keeps_the_centre_finite() {
        let dim = 64;
        let mut theta = vec![0.0; dim];
        let mut adam = Adam::new(dim);
        for g_i in 0..25u64 {
            let g: Vec<f64> = (0..dim).map(|j| ((g_i * 7 + j as u64) as f64).cos()).collect();
            adam.ascend(&mut theta, &g);
        }
        assert!(theta.iter().all(|x| x.is_finite()));
        assert!(adam.is_finite());
    }
}
