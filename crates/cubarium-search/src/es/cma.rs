//! A small **CMA-ES** (Hansen's `(μ/μ_w, λ)` with rank-one and rank-μ updates and
//! cumulative step-size adaptation), for low-dimensional searches where every
//! evaluation is expensive: the ecology search's five parameters.
//!
//! Minimises. `ask` draws `λ` candidates, `tell` takes their fitness in the same order.
//! Pure `f64`, no external linear algebra: the covariance is kept as a dense matrix and
//! eigendecomposed by cyclic Jacobi, which is exact enough and instant at this size.

/// The search state. Serializable whole, so a run checkpoints and resumes exactly
/// (`serde_json`'s `float_roundtrip` keeps every `f64`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Cma {
    n: usize,
    lambda: usize,
    mu: usize,
    weights: Vec<f64>,
    mueff: f64,
    cc: f64,
    cs: f64,
    c1: f64,
    cmu: f64,
    damps: f64,
    chin: f64,
    mean: Vec<f64>,
    sigma: f64,
    c: Vec<Vec<f64>>,
    b: Vec<Vec<f64>>,
    d: Vec<f64>,
    pc: Vec<f64>,
    ps: Vec<f64>,
    generation: u64,
    rng: u64,
}

impl Cma {
    /// Start at `mean` with step size `sigma`, `lambda` candidates per generation, and a
    /// seed for the sampling stream.
    pub fn new(mean: Vec<f64>, sigma: f64, lambda: usize, seed: u64) -> Cma {
        let n = mean.len();
        assert!(n > 0 && lambda >= 2 && sigma > 0.0);
        let mu = lambda / 2;
        let raw: Vec<f64> = (0..mu)
            .map(|i| ((mu as f64) + 0.5).ln() - ((i + 1) as f64).ln())
            .collect();
        let sum: f64 = raw.iter().sum();
        let weights: Vec<f64> = raw.iter().map(|w| w / sum).collect();
        let mueff = 1.0 / weights.iter().map(|w| w * w).sum::<f64>();
        let nf = n as f64;
        let cc = (4.0 + mueff / nf) / (nf + 4.0 + 2.0 * mueff / nf);
        let cs = (mueff + 2.0) / (nf + mueff + 5.0);
        let c1 = 2.0 / ((nf + 1.3).powi(2) + mueff);
        let cmu = (1.0 - c1).min(2.0 * (mueff - 2.0 + 1.0 / mueff) / ((nf + 2.0).powi(2) + mueff));
        let damps = 1.0 + 2.0 * (((mueff - 1.0) / (nf + 1.0)).sqrt() - 1.0).max(0.0) + cs;
        let chin = nf.sqrt() * (1.0 - 1.0 / (4.0 * nf) + 1.0 / (21.0 * nf * nf));
        let identity: Vec<Vec<f64>> = (0..n)
            .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
            .collect();
        Cma {
            n,
            lambda,
            mu,
            weights,
            mueff,
            cc,
            cs,
            c1,
            cmu,
            damps,
            chin,
            mean,
            sigma,
            c: identity.clone(),
            b: identity,
            d: vec![1.0; n],
            pc: vec![0.0; n],
            ps: vec![0.0; n],
            generation: 0,
            rng: seed ^ 0x5DEE_CE66_D1CE_4E5B,
        }
    }

    pub fn mean(&self) -> &[f64] {
        &self.mean
    }

    pub fn sigma(&self) -> f64 {
        self.sigma
    }

    pub fn lambda(&self) -> usize {
        self.lambda
    }

    /// The standard deviation along each coordinate: `σ·√C_ii`.
    pub fn spread(&self) -> Vec<f64> {
        (0..self.n)
            .map(|i| self.sigma * self.c[i][i].sqrt())
            .collect()
    }

    fn next_u64(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn gaussian(&mut self) -> f64 {
        let u1 = ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
        let u2 = ((self.next_u64() >> 11) as f64) / (1u64 << 53) as f64;
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    /// Draw this generation's `λ` candidates.
    pub fn ask(&mut self) -> Vec<Vec<f64>> {
        (0..self.lambda)
            .map(|_| {
                let z: Vec<f64> = (0..self.n).map(|_| self.gaussian()).collect();
                (0..self.n)
                    .map(|i| {
                        let y: f64 = (0..self.n).map(|j| self.b[i][j] * self.d[j] * z[j]).sum();
                        self.mean[i] + self.sigma * y
                    })
                    .collect()
            })
            .collect()
    }

    /// Update from the candidates `xs` (as `ask` returned them) and their fitness
    /// (lower is better).
    pub fn tell(&mut self, xs: &[Vec<f64>], fitness: &[f64]) {
        assert_eq!(xs.len(), fitness.len());
        assert!(xs.len() >= self.mu);
        let n = self.n;
        let mut order: Vec<usize> = (0..xs.len()).collect();
        order.sort_by(|&a, &b| fitness[a].total_cmp(&fitness[b]));
        let ys: Vec<Vec<f64>> = order[..self.mu]
            .iter()
            .map(|&k| {
                (0..n)
                    .map(|i| (xs[k][i] - self.mean[i]) / self.sigma)
                    .collect()
            })
            .collect();
        let yw: Vec<f64> = (0..n)
            .map(|i| ys.iter().zip(&self.weights).map(|(y, w)| w * y[i]).sum())
            .collect();
        for i in 0..n {
            self.mean[i] += self.sigma * yw[i];
        }
        // C^{-1/2} y_w = B D^{-1} Bᵀ y_w
        let bty: Vec<f64> = (0..n)
            .map(|j| (0..n).map(|i| self.b[i][j] * yw[i]).sum::<f64>() / self.d[j])
            .collect();
        let cinv_yw: Vec<f64> = (0..n)
            .map(|i| (0..n).map(|j| self.b[i][j] * bty[j]).sum())
            .collect();
        let ks = (self.cs * (2.0 - self.cs) * self.mueff).sqrt();
        for i in 0..n {
            self.ps[i] = (1.0 - self.cs) * self.ps[i] + ks * cinv_yw[i];
        }
        self.generation += 1;
        let ps_norm = self.ps.iter().map(|x| x * x).sum::<f64>().sqrt();
        let hsig = ps_norm / (1.0 - (1.0 - self.cs).powi(2 * self.generation as i32)).sqrt()
            < (1.4 + 2.0 / (n as f64 + 1.0)) * self.chin;
        let hs = if hsig { 1.0 } else { 0.0 };
        let kc = (self.cc * (2.0 - self.cc) * self.mueff).sqrt();
        for i in 0..n {
            self.pc[i] = (1.0 - self.cc) * self.pc[i] + hs * kc * yw[i];
        }
        let keep = 1.0 - self.c1 - self.cmu;
        let delta = (1.0 - hs) * self.cc * (2.0 - self.cc);
        for i in 0..n {
            for j in 0..n {
                let rank_mu: f64 = ys
                    .iter()
                    .zip(&self.weights)
                    .map(|(y, w)| w * y[i] * y[j])
                    .sum();
                self.c[i][j] = keep * self.c[i][j]
                    + self.c1 * (self.pc[i] * self.pc[j] + delta * self.c[i][j])
                    + self.cmu * rank_mu;
            }
        }
        self.sigma *= ((self.cs / self.damps) * (ps_norm / self.chin - 1.0)).exp();
        let (vals, vecs) = jacobi(&self.c);
        self.d = vals.iter().map(|v| v.max(1e-20).sqrt()).collect();
        self.b = vecs;
    }
}

/// Eigenvalues and eigenvectors (columns) of a symmetric matrix by cyclic Jacobi.
fn jacobi(m: &[Vec<f64>]) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = m.len();
    let mut a: Vec<Vec<f64>> = m.to_vec();
    // Symmetrise against rounding.
    for i in 0..n {
        for j in 0..i {
            let s = 0.5 * (a[i][j] + a[j][i]);
            a[i][j] = s;
            a[j][i] = s;
        }
    }
    let mut v: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();
    for _sweep in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| a[i][j] * a[i][j])
            .sum();
        if off < 1e-30 {
            break;
        }
        for p in 0..n {
            for q in (p + 1)..n {
                if a[p][q].abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let (akp, akq) = (a[k][p], a[k][q]);
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let (apk, aqk) = (a[p][k], a[q][k]);
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
                for k in 0..n {
                    let (vkp, vkq) = (v[k][p], v[k][q]);
                    v[k][p] = c * vkp - s * vkq;
                    v[k][q] = s * vkp + c * vkq;
                }
            }
        }
    }
    ((0..n).map(|i| a[i][i]).collect(), v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jacobi_recovers_a_symmetric_matrix() {
        let m = vec![
            vec![4.0, 1.0, 0.5],
            vec![1.0, 3.0, 0.2],
            vec![0.5, 0.2, 2.0],
        ];
        let (vals, v) = jacobi(&m);
        for i in 0..3 {
            for j in 0..3 {
                let r: f64 = (0..3).map(|k| v[i][k] * vals[k] * v[j][k]).sum();
                assert!((r - m[i][j]).abs() < 1e-10, "{i},{j}: {r} vs {}", m[i][j]);
            }
        }
    }
}
