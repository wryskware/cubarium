//! **The learner** (PG-PILOT): one generation's decisions in, one clipped policy-gradient
//! update of the top's intent head out.
//!
//! - **Returns and advantages.** Per body, decisions `i` at times `t_i` with reward `r_i`
//!   earned until the next one; between decisions `Δt` apart the return discounts by
//!   `γ = exp(−Δt/discount_horizon_s)` and GAE's trace by `λ = exp(−Δt/lambda_horizon_s)`:
//!   `δ_i = r_i + γ_i V(s_{i+1}) − V(s_i)`, `A_i = δ_i + γ_i λ_i A_{i+1}`. A body that died
//!   ends with `V = 0` after its last decision; a body alive at the window's end bootstraps
//!   from its last decision's value (that decision is not trained).
//! - **The value baseline** reads the top's input `u = [GRU state | held intent | time]`
//!   through one tanh layer, trained by regression on `A + V`.
//! - **The update** maximises PPO's clipped surrogate `mean(min(ρA, clip(ρ, 1±ε)A)) + c·H`
//!   with `ρ = π(a|s)/π_old(a|s)` at the arbitration temperature and advantages normalised
//!   over the generation, by Adam over minibatches and passes. Only the intent head
//!   (`top.w1`, `top.b1`, `top.w2`, `top.b2`) receives gradient; the bottom and the pointer
//!   do not move.
//! - **The pull toward the clone** (SCORE-V2, research topic 6(c)): beside the entropy bonus,
//!   `− β · mean(KL(π_clone ‖ π))`, the KL from the clone's intent proposal on the same
//!   input at the same temperature (`π_clone` from [`PgState::anchor`], the centre the
//!   lineage started from) to the policy's; its gradient on the logits is `β (π_clone − π)/τ`,
//!   zero at the clone and pulling toward it. `β` is `knobs.clone_kl` (the caller fades it
//!   by generation, [`PgKnobs::clone_kl_at`]); no anchor, no pull.
//! - **The yardstick**: the split-half cosine of the plain advantage-weighted gradient at
//!   the sampling policy, over random halves of the worlds and of the bodies, beside the
//!   same with the advantages shuffled (the noise floor) and the cosine with the previous
//!   generation's.

use serde::{Deserialize, Serialize};

use super::knobs::PgKnobs;
use crate::es::optimizer::Adam;
use crate::es::voxel::two_layer::net::{
    TopForward, intent_head_backward, masked_argmax, masked_softmax, top_forward, top_hidden_backward,
};
use crate::es::voxel::two_layer::shape::TopOffsets;
use crate::es::voxel::two_layer::{TwoLayerLayout, TwoLayerShape};

/// What the top read at one decision.
#[derive(Clone, Debug, PartialEq)]
pub struct TopInput {
    /// The GRU state (`hidden` long).
    pub hidden: Vec<f64>,
    /// The intent held before the decision.
    pub held: Option<usize>,
    /// The squashed time in that intent.
    pub time: f64,
    /// Which intents were valid (`M'` long).
    pub valid: Vec<bool>,
}

/// One trained decision.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub input: TopInput,
    /// The sampled intent.
    pub action: usize,
    /// `ln π_old(action)` as sampled.
    pub logp: f64,
    /// Seconds to the next decision (or to the death that ended the body's window).
    pub dt_s: f64,
    /// The reward earned over those seconds (the reward interface's sum).
    pub reward: f64,
}

/// One body's decisions in one window.
#[derive(Clone, Debug, PartialEq)]
pub struct Trajectory {
    /// The world it lived in (the split readout's unit).
    pub world: usize,
    pub body: u64,
    pub steps: Vec<Step>,
    /// The state its return is bootstrapped from after the last step: its last decision
    /// when it lived to the window's end, `None` when it died (the return ends there).
    pub bootstrap: Option<TopInput>,
}

/// The top's intent head as a policy at the arbitration temperature.
#[derive(Clone, Copy, Debug)]
pub struct Policy<'a> {
    pub shape: &'a TwoLayerShape,
    pub layout: &'a TwoLayerLayout,
    pub temperature: f64,
}

/// The value baseline: `V(u) = w2 · tanh(W1 u + b1) + b2`, its own flat parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValueNet {
    pub inputs: usize,
    pub hidden: usize,
    #[serde(with = "crate::es::bits::hex_f64s")]
    pub phi: Vec<f64>,
}

/// A lineage's learner state beside its centre: the baseline, its optimizer, and the last
/// generation's plain gradient (the `next` readout).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PgState {
    pub value: ValueNet,
    pub value_adam: Adam,
    #[serde(with = "crate::es::bits::hex_f64s")]
    pub last_gradient: Vec<f64>,
    /// **The clone** (SCORE-V2): the centre the lineage started from, the KL pull's anchor;
    /// kept in the checkpoint so a resumed run pulls toward the same clone. Empty: no pull.
    #[serde(default, with = "crate::es::bits::hex_f64s")]
    pub anchor: Vec<f64>,
}

/// The clipped surrogate's value and its parts over a batch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surrogate {
    /// `mean(min(ρA, clip(ρ)A)) + c·mean(H)`: what the update ascends.
    pub objective: f64,
    /// Mean entropy of the proposal (nats).
    pub entropy: f64,
    /// Share of decisions whose ratio sits outside the clip on the side its advantage pushes.
    pub clip_fraction: f64,
    /// `mean(ln π_old − ln π)`, the approximate KL from the sampling policy.
    pub approx_kl: f64,
    /// `mean(KL(π_clone ‖ π))`, nats (0 without the pull).
    pub clone_kl: f64,
}

/// One generation's readout (`generations.jsonl`, the lineage's `pg` field).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct PgReadout {
    pub worlds: usize,
    pub bodies: usize,
    /// Trained decisions.
    pub decisions: usize,
    /// Bodies whose window ended in death.
    pub terminal: usize,
    /// Share of decisions whose sampled intent was not the argmax.
    pub off_argmax: f64,
    /// Mean reward summed over a body's trained decisions (body references), and its terms.
    pub mean_return: f64,
    pub mean_terms: Vec<(String, f64)>,
    /// sd of the raw advantages (before normalising).
    pub advantage_sd: f64,
    /// Value regression loss (mean squared error to `A + V`) before and after the update,
    /// and the explained variance of the targets before it.
    pub value_loss_before: f64,
    pub value_loss_after: f64,
    pub explained_variance: f64,
    /// The surrogate's parts at the end of the update.
    pub entropy: f64,
    /// Mean `KL(π_clone ‖ π)` at the end of the update (nats; 0 without the pull), and the
    /// pull's weight this generation (SCORE-V2).
    pub clone_kl: f64,
    pub clone_kl_weight: f64,
    /// The clone's mean entropy on the generation's decisions (nats; NaN without an
    /// anchor): what `entropy` should stay near.
    pub clone_entropy: f64,
    /// Mean judge reward summed over a body's trained decisions (body references): the
    /// [`super::reward::JUDGE_TERMS`] of `mean_terms` (SCORE-V2; the trainer fills it).
    pub judge_return: f64,
    pub approx_kl: f64,
    pub clip_fraction: f64,
    /// Norm of the plain gradient at the sampling policy (normalised advantages).
    pub gradient_norm: f64,
    /// RMS of the centre's change over the generation.
    pub update_rms: f64,
    /// The yardstick: split-half cosines over worlds and over bodies, their shuffled-advantage
    /// floors, and the cosine with the previous generation's gradient (NaN at the first).
    pub split_worlds: f64,
    pub split_worlds_null: f64,
    pub split_bodies: f64,
    pub split_bodies_null: f64,
    pub next: f64,
}

/// Scratch for one decision's forward and backward pass.
struct Work {
    top: TopForward,
    p: Vec<f64>,
    dl: Vec<f64>,
    dz: Vec<f64>,
}

/// One decision's share of the surrogate.
#[derive(Clone, Copy, Default)]
struct Term {
    objective: f64,
    entropy: f64,
    clipped: bool,
    kl: f64,
    clone_kl: f64,
}

/// The pull toward the clone over a batch: its weight and, per step, the clone's
/// probabilities on that step's input.
pub type Pull<'a> = Option<(f64, &'a [Vec<f64>])>;

impl Policy<'_> {
    fn offsets(&self) -> TopOffsets {
        self.layout.top.expect("a policy-gradient brain has a top")
    }

    fn work(&self) -> Work {
        let m = self.shape.intents();
        Work { top: TopForward::new(self.shape), p: vec![0.0; m], dl: vec![0.0; m], dz: vec![0.0; self.shape.top_hidden] }
    }

    fn forward(&self, theta: &[f64], input: &TopInput, w: &mut Work) {
        let t = self.offsets();
        top_forward(self.shape, &t, theta, &input.hidden, input.held, input.time, &mut w.top);
        masked_softmax(&w.top.logits, &input.valid, self.temperature, &mut w.p);
    }

    /// The masked proposal probabilities at the temperature into `p` (`M'` long): the
    /// exploring brain's own arithmetic ([`crate::es::voxel::two_layer::TwoLayerBrain`]).
    pub fn probs(&self, theta: &[f64], input: &TopInput, p: &mut [f64]) {
        let mut w = self.work();
        self.forward(theta, input, &mut w);
        p.copy_from_slice(&w.p);
    }

    /// `ln π(action | input)`.
    pub fn log_prob(&self, theta: &[f64], input: &TopInput, action: usize) -> f64 {
        let mut w = self.work();
        self.forward(theta, input, &mut w);
        w.p[action].ln()
    }

    /// The indices of the parameters the policy gradient reaches: the intent head
    /// (`top.w1`, `top.b1`, `top.w2`, `top.b2`, contiguous).
    pub fn trained(&self) -> std::ops::Range<usize> {
        let t = self.offsets();
        t.w1..t.wr
    }

    /// One decision: its clipped term, entropy, its KL from the clone's `pull` (weight and
    /// the clone's probabilities on this input), and (with `grad`) `scale ×` its gradient.
    #[allow(clippy::too_many_arguments)]
    fn term(
        &self,
        theta: &[f64],
        st: &Step,
        adv: f64,
        clip: f64,
        entropy: f64,
        pull: Option<(f64, &[f64])>,
        scale: f64,
        grad: Option<&mut [f64]>,
        w: &mut Work,
    ) -> Term {
        self.forward(theta, &st.input, w);
        let logp = w.p[st.action].ln();
        let ratio = (logp - st.logp).exp();
        let unclipped = ratio * adv;
        let clipped = ratio.clamp(1.0 - clip, 1.0 + clip) * adv;
        let (objective, g_logp, out) =
            if unclipped <= clipped { (unclipped, ratio * adv, false) } else { (clipped, 0.0, true) };
        let mut h = 0.0;
        for (&p, &v) in w.p.iter().zip(&st.input.valid) {
            if v && p > 0.0 {
                h -= p * p.ln();
            }
        }
        let mut clone_kl = 0.0;
        if let Some((_, c)) = pull {
            for k in 0..w.p.len() {
                if st.input.valid[k] && c[k] > 0.0 && w.p[k] > 0.0 {
                    clone_kl += c[k] * (c[k].ln() - w.p[k].ln());
                }
            }
        }
        if let Some(grad) = grad {
            let tau = if self.temperature > 0.0 { self.temperature } else { 1.0 };
            for k in 0..w.dl.len() {
                let p = w.p[k];
                w.dl[k] = if st.input.valid[k] {
                    let pg = g_logp * (f64::from(u8::from(k == st.action)) - p);
                    let ent = if p > 0.0 { -entropy * p * (p.ln() + h) } else { 0.0 };
                    let toward = pull.map_or(0.0, |(beta, c)| beta * (c[k] - p));
                    scale * (pg + ent + toward) / tau
                } else {
                    0.0
                };
            }
            w.dz.fill(0.0);
            let t = self.offsets();
            intent_head_backward(&t, theta, &w.top, &w.dl, grad, &mut w.dz);
            top_hidden_backward(&t, theta, &w.top, &w.dz, grad, None);
        }
        Term { objective, entropy: h, clipped: out, kl: st.logp - logp, clone_kl }
    }

    /// The clipped surrogate over `steps` with advantages `adv`; with `grad`, its gradient
    /// (to ascend) added in.
    pub fn surrogate(
        &self,
        theta: &[f64],
        steps: &[&Step],
        adv: &[f64],
        clip: f64,
        entropy: f64,
        grad: Option<&mut [f64]>,
    ) -> Surrogate {
        self.surrogate_pulled(theta, steps, adv, clip, entropy, None, grad)
    }

    /// [`Policy::surrogate`] with the pull toward the clone: `− β · mean(KL(π_clone ‖ π))`
    /// added to the objective, `pull` holding `β` and the clone's probabilities per step.
    #[allow(clippy::too_many_arguments)]
    pub fn surrogate_pulled(
        &self,
        theta: &[f64],
        steps: &[&Step],
        adv: &[f64],
        clip: f64,
        entropy: f64,
        pull: Pull<'_>,
        mut grad: Option<&mut [f64]>,
    ) -> Surrogate {
        assert_eq!(steps.len(), adv.len());
        let n = steps.len().max(1) as f64;
        let mut w = self.work();
        let mut s = Surrogate::default();
        for (i, (st, &a)) in steps.iter().zip(adv).enumerate() {
            let p = pull.map(|(beta, c)| (beta, c[i].as_slice()));
            let t = self.term(theta, st, a, clip, entropy, p, 1.0 / n, grad.as_deref_mut(), &mut w);
            s.objective += t.objective;
            s.entropy += t.entropy;
            s.clip_fraction += f64::from(u8::from(t.clipped));
            s.approx_kl += t.kl;
            s.clone_kl += t.clone_kl;
        }
        s.objective /= n;
        s.entropy /= n;
        s.clip_fraction /= n;
        s.approx_kl /= n;
        s.clone_kl /= n;
        s.objective += entropy * s.entropy - pull.map_or(0.0, |(beta, _)| beta) * s.clone_kl;
        s
    }

    /// The surrogate's gradient over `items` (indices into `steps`), on `threads` workers,
    /// each decision weighted `scale`; and the surrogate's parts summed (not averaged).
    #[allow(clippy::too_many_arguments)]
    fn gradient_par(
        &self,
        theta: &[f64],
        steps: &[&Step],
        adv: &[f64],
        items: &[usize],
        clip: f64,
        entropy: f64,
        pull: Pull<'_>,
        scale: f64,
        threads: usize,
    ) -> (Vec<f64>, Surrogate) {
        let chunks: Vec<&[usize]> = items.chunks(items.len().div_ceil(threads.max(1)).max(1)).collect();
        let parts: Vec<(Vec<f64>, Surrogate)> = std::thread::scope(|scope| {
            let handles: Vec<_> = chunks
                .iter()
                .map(|chunk| {
                    scope.spawn(move || {
                        let mut g = vec![0.0; theta.len()];
                        let mut w = self.work();
                        let mut s = Surrogate::default();
                        for &i in chunk.iter() {
                            let p = pull.map(|(beta, c)| (beta, c[i].as_slice()));
                            let t = self.term(theta, steps[i], adv[i], clip, entropy, p, scale, Some(&mut g), &mut w);
                            s.objective += t.objective;
                            s.entropy += t.entropy;
                            s.clip_fraction += f64::from(u8::from(t.clipped));
                            s.approx_kl += t.kl;
                            s.clone_kl += t.clone_kl;
                        }
                        (g, s)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("gradient worker")).collect()
        });
        let mut g = vec![0.0; theta.len()];
        let mut s = Surrogate::default();
        for (pg, ps) in parts {
            for (a, b) in g.iter_mut().zip(&pg) {
                *a += b;
            }
            s.objective += ps.objective;
            s.entropy += ps.entropy;
            s.clip_fraction += ps.clip_fraction;
            s.approx_kl += ps.approx_kl;
            s.clone_kl += ps.clone_kl;
        }
        (g, s)
    }
}

/// The value baseline's input for one decision: the top's own input
/// `[GRU state | held intent one-hot | time]`.
pub fn features(shape: &TwoLayerShape, input: &TopInput) -> Vec<f64> {
    let (h, m) = (shape.hidden, shape.intents());
    let mut u = vec![0.0; h + m + 1];
    u[..h].copy_from_slice(&input.hidden);
    if let Some(i) = input.held {
        u[h + i] = 1.0;
    }
    u[h + m] = input.time;
    u
}

/// A seeded standard normal stream (splitmix64 and Box-Muller).
struct Normal(u64);

impl Normal {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn normal(&mut self) -> f64 {
        let (a, b) = (self.uniform(), self.uniform());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
    fn below(&mut self, n: usize) -> usize {
        ((self.uniform() * n as f64) as usize).min(n.saturating_sub(1))
    }
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

impl ValueNet {
    /// A seeded baseline: fan-in-scaled first layer, zero output layer (`V = 0` to start).
    pub fn new(inputs: usize, hidden: usize, seed: u64) -> ValueNet {
        let mut phi = vec![0.0; hidden * inputs + 2 * hidden + 1];
        let mut rng = Normal(seed ^ 0x7661_6c75_652d_6e65);
        let scale = 1.0 / (inputs.max(1) as f64).sqrt();
        for w in &mut phi[..hidden * inputs] {
            *w = scale * rng.normal();
        }
        ValueNet { inputs, hidden, phi }
    }

    fn hidden_layer(&self, u: &[f64], z: &mut [f64]) {
        let (n, h) = (self.inputs, self.hidden);
        let b1 = h * n;
        for (j, zj) in z.iter_mut().enumerate() {
            let row = &self.phi[j * n..(j + 1) * n];
            let mut a = self.phi[b1 + j];
            for (w, x) in row.iter().zip(u) {
                a += w * x;
            }
            *zj = a.tanh();
        }
    }

    pub fn value(&self, u: &[f64]) -> f64 {
        let mut z = vec![0.0; self.hidden];
        self.hidden_layer(u, &mut z);
        self.output(&z)
    }

    fn output(&self, z: &[f64]) -> f64 {
        let (n, h) = (self.inputs, self.hidden);
        let w2 = h * n + h;
        let mut v = self.phi[w2 + h];
        for (j, zj) in z.iter().enumerate() {
            v += self.phi[w2 + j] * zj;
        }
        v
    }

    /// Mean squared error to `targets` over `us`; with `grad`, its gradient (to descend)
    /// added in.
    pub fn loss(&self, us: &[&[f64]], targets: &[f64], mut grad: Option<&mut [f64]>) -> f64 {
        assert_eq!(us.len(), targets.len());
        let (n, h) = (self.inputs, self.hidden);
        let (b1, w2) = (h * n, h * n + h);
        let count = us.len().max(1) as f64;
        let mut z = vec![0.0; h];
        let mut sum = 0.0;
        for (u, &y) in us.iter().zip(targets) {
            self.hidden_layer(u, &mut z);
            let e = self.output(&z) - y;
            sum += e * e;
            if let Some(g) = grad.as_deref_mut() {
                let dv = 2.0 * e / count;
                g[w2 + h] += dv;
                for j in 0..h {
                    g[w2 + j] += dv * z[j];
                    let da = dv * self.phi[w2 + j] * (1.0 - z[j] * z[j]);
                    g[b1 + j] += da;
                    for (c, x) in u.iter().enumerate() {
                        g[j * n + c] += da * x;
                    }
                }
            }
        }
        sum / count
    }
}

/// GAE advantages of one trajectory from its steps' `values` and the bootstrap's value
/// (ignored without a bootstrap).
pub fn advantages(traj: &Trajectory, values: &[f64], bootstrap: f64, knobs: &PgKnobs) -> Vec<f64> {
    let n = traj.steps.len();
    assert_eq!(values.len(), n);
    let mut a = vec![0.0; n];
    let mut next_a = 0.0;
    for i in (0..n).rev() {
        let st = &traj.steps[i];
        let (gamma, lambda) = knobs.factors(st.dt_s);
        let next_v = if i + 1 < n {
            values[i + 1]
        } else if traj.bootstrap.is_some() {
            bootstrap
        } else {
            0.0
        };
        let delta = st.reward + gamma * next_v - values[i];
        let carry = if i + 1 < n { gamma * lambda * next_a } else { 0.0 };
        a[i] = delta + carry;
        next_a = a[i];
    }
    a
}

impl PgState {
    pub fn new(shape: &TwoLayerShape, knobs: &PgKnobs, seed: u64) -> PgState {
        let value = ValueNet::new(shape.top_inputs(), knobs.value_hidden, seed);
        let mut value_adam = Adam::new(value.phi.len());
        value_adam.learning_rate = knobs.value_lr;
        PgState { value, value_adam, last_gradient: Vec::new(), anchor: Vec::new() }
    }
}

/// Run `f` over `0..n` in `threads` contiguous chunks, results in order.
fn par_map<R: Send>(n: usize, threads: usize, f: impl Fn(usize) -> R + Sync) -> Vec<R> {
    let size = n.div_ceil(threads.max(1)).max(1);
    let f = &f;
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..n)
            .step_by(size)
            .map(|lo| scope.spawn(move || (lo..(lo + size).min(n)).map(f).collect::<Vec<R>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("worker")).collect()
    })
}

fn mean_sd(x: &[f64]) -> (f64, f64) {
    let n = x.len().max(1) as f64;
    let m = x.iter().sum::<f64>() / n;
    let v = x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / n;
    (m, v.sqrt())
}

/// Cosine of two vectors (0 when either is zero).
pub fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    if aa > 0.0 && bb > 0.0 { ab / (aa.sqrt() * bb.sqrt()) } else { 0.0 }
}

/// The mean split-half cosine over `splits` random halves of `groups` (each a gradient
/// sum): one half's sum against the other's. NaN with fewer than two groups.
fn split_cosine(groups: &[Vec<f64>], splits: usize, rng: &mut Normal) -> f64 {
    if groups.len() < 2 {
        return f64::NAN;
    }
    let dim = groups[0].len();
    let mut order: Vec<usize> = (0..groups.len()).collect();
    let mut sum = 0.0;
    for _ in 0..splits.max(1) {
        rng.shuffle(&mut order);
        let (mut a, mut b) = (vec![0.0; dim], vec![0.0; dim]);
        for (k, &g) in order.iter().enumerate() {
            let side = if k < groups.len() / 2 { &mut a } else { &mut b };
            for (s, v) in side.iter_mut().zip(&groups[g]) {
                *s += v;
            }
        }
        sum += cosine(&a, &b);
    }
    sum / splits.max(1) as f64
}

/// **One generation's update**: advantages, the yardstick, then `epochs × minibatches`
/// Adam steps of the intent head (on `adam`, at `knobs.policy_lr`) and of the baseline.
#[allow(clippy::too_many_arguments)]
pub fn update(
    policy: &Policy<'_>,
    theta: &mut [f64],
    adam: &mut Adam,
    state: &mut PgState,
    trajs: &[Trajectory],
    knobs: &PgKnobs,
    seed: u64,
    threads: usize,
) -> PgReadout {
    let threads = threads.max(1);
    let mut rng = Normal(seed ^ 0x7067_2d6c_6561_726e);
    let steps: Vec<&Step> = trajs.iter().flat_map(|t| t.steps.iter()).collect();
    let n = steps.len();
    let mut r = PgReadout {
        bodies: trajs.len(),
        decisions: n,
        terminal: trajs.iter().filter(|t| t.bootstrap.is_none()).count(),
        next: f64::NAN,
        split_worlds: f64::NAN,
        split_worlds_null: f64::NAN,
        split_bodies: f64::NAN,
        split_bodies_null: f64::NAN,
        ..PgReadout::default()
    };
    let mut worlds: Vec<usize> = trajs.iter().map(|t| t.world).collect();
    worlds.sort_unstable();
    worlds.dedup();
    r.worlds = worlds.len();
    if n == 0 {
        return r;
    }
    r.mean_return = steps.iter().map(|s| s.reward).sum::<f64>() / trajs.len() as f64;

    // 1. Values, advantages and the baseline's targets.
    let shape = policy.shape;
    let feats: Vec<Vec<f64>> = par_map(n, threads, |i| features(shape, &steps[i].input));
    let value = &state.value;
    let values: Vec<f64> = par_map(n, threads, |i| value.value(&feats[i]));
    let mut adv = Vec::with_capacity(n);
    let mut at = 0;
    for t in trajs {
        let k = t.steps.len();
        let boot = t.bootstrap.as_ref().map_or(0.0, |b| state.value.value(&features(shape, b)));
        adv.extend(advantages(t, &values[at..at + k], boot, knobs));
        at += k;
    }
    let targets: Vec<f64> = adv.iter().zip(&values).map(|(a, v)| a + v).collect();
    let (_, sd_a) = mean_sd(&adv);
    r.advantage_sd = sd_a;
    let refs: Vec<&[f64]> = feats.iter().map(Vec::as_slice).collect();
    r.value_loss_before = state.value.loss(&refs, &targets, None);
    let (_, sd_t) = mean_sd(&targets);
    r.explained_variance = if sd_t > 0.0 { 1.0 - r.value_loss_before / (sd_t * sd_t) } else { f64::NAN };
    let (m_a, _) = mean_sd(&adv);
    let norm: Vec<f64> = adv.iter().map(|a| if sd_a > 0.0 { (a - m_a) / sd_a } else { 0.0 }).collect();

    // 2. The yardstick, at the sampling policy: the plain advantage-weighted gradient per
    // body, summed by world and by body, beside the same with the advantages shuffled.
    let range = policy.trained();
    let mut shuffled = norm.clone();
    rng.shuffle(&mut shuffled);
    let mut bounds = Vec::with_capacity(trajs.len());
    let mut lo = 0;
    for t in trajs {
        bounds.push(lo..lo + t.steps.len());
        lo += t.steps.len();
    }
    let per_body = |a: &[f64]| -> Vec<Vec<f64>> {
        par_map(trajs.len(), threads, |b| {
            let mut g = vec![0.0; theta.len()];
            let mut w = policy.work();
            for i in bounds[b].clone() {
                policy.term(theta, steps[i], a[i], f64::INFINITY, 0.0, None, 1.0 / n as f64, Some(&mut g), &mut w);
            }
            g[range.clone()].to_vec()
        })
    };
    let bodies = per_body(&norm);
    let bodies_null = per_body(&shuffled);
    let by_world = |g: &[Vec<f64>]| -> Vec<Vec<f64>> {
        worlds
            .iter()
            .map(|&w| {
                let mut s = vec![0.0; range.len()];
                for (t, gb) in trajs.iter().zip(g) {
                    if t.world == w {
                        for (x, y) in s.iter_mut().zip(gb) {
                            *x += y;
                        }
                    }
                }
                s
            })
            .collect()
    };
    r.split_worlds = split_cosine(&by_world(&bodies), knobs.splits, &mut rng);
    r.split_worlds_null = split_cosine(&by_world(&bodies_null), knobs.splits, &mut rng);
    r.split_bodies = split_cosine(&bodies, knobs.splits, &mut rng);
    r.split_bodies_null = split_cosine(&bodies_null, knobs.splits, &mut rng);
    let mut full = vec![0.0; range.len()];
    for gb in &bodies {
        for (x, y) in full.iter_mut().zip(gb) {
            *x += y;
        }
    }
    r.gradient_norm = full.iter().map(|x| x * x).sum::<f64>().sqrt();
    if state.last_gradient.len() == full.len() {
        r.next = cosine(&full, &state.last_gradient);
    }
    state.last_gradient = full;
    let mut off = 0usize;
    let mut w = policy.work();
    for st in &steps {
        policy.forward(theta, &st.input, &mut w);
        off += usize::from(masked_argmax(&w.top.logits, &st.input.valid) != Some(st.action));
    }
    r.off_argmax = off as f64 / n as f64;

    // 3. The clipped update of the intent head and the baseline's regression, pulled toward
    // the clone's proposal on each decision's input when the lineage has one.
    let clone_p: Vec<Vec<f64>> = if state.anchor.len() == theta.len() {
        let anchor = &state.anchor;
        par_map(n, threads, |i| {
            let mut p = vec![0.0; shape.intents()];
            policy.probs(anchor, &steps[i].input, &mut p);
            p
        })
    } else {
        Vec::new()
    };
    if !clone_p.is_empty() {
        let h: f64 = clone_p
            .iter()
            .map(|p| p.iter().filter(|&&q| q > 0.0).map(|&q| -q * q.ln()).sum::<f64>())
            .sum();
        r.clone_entropy = h / n as f64;
    } else {
        r.clone_entropy = f64::NAN;
    }
    let pull: Pull<'_> = (knobs.clone_kl > 0.0 && !clone_p.is_empty()).then_some((knobs.clone_kl, clone_p.as_slice()));
    r.clone_kl_weight = pull.map_or(0.0, |(beta, _)| beta);
    let before = theta.to_vec();
    adam.learning_rate = knobs.policy_lr;
    state.value_adam.learning_rate = knobs.value_lr;
    let mut order: Vec<usize> = (0..n).collect();
    let batches = knobs.minibatches.clamp(1, n);
    for _ in 0..knobs.epochs.max(1) {
        rng.shuffle(&mut order);
        for b in 0..batches {
            let items = &order[b * n / batches..(b + 1) * n / batches];
            if items.is_empty() {
                continue;
            }
            let scale = 1.0 / items.len() as f64;
            let (g, _) = policy.gradient_par(theta, &steps, &norm, items, knobs.clip, knobs.entropy, pull, scale, threads);
            adam.ascend(theta, &g);
            let us: Vec<&[f64]> = items.iter().map(|&i| feats[i].as_slice()).collect();
            let ys: Vec<f64> = items.iter().map(|&i| targets[i]).collect();
            let mut gv = vec![0.0; state.value.phi.len()];
            state.value.loss(&us, &ys, Some(&mut gv));
            for x in &mut gv {
                *x = -*x;
            }
            state.value_adam.ascend(&mut state.value.phi, &gv);
        }
    }

    // 4. Where it ended.
    let all: Vec<usize> = (0..n).collect();
    // The clone's KL is read whenever there is a clone, pulled or not (weight 0 adds nothing).
    let read: Pull<'_> = (!clone_p.is_empty()).then_some((r.clone_kl_weight, clone_p.as_slice()));
    let (_, s) = policy.gradient_par(theta, &steps, &norm, &all, knobs.clip, knobs.entropy, read, 0.0, threads);
    r.entropy = s.entropy / n as f64;
    r.clone_kl = s.clone_kl / n as f64;
    r.approx_kl = s.approx_kl / n as f64;
    r.clip_fraction = s.clip_fraction / n as f64;
    r.value_loss_after = state.value.loss(&refs, &targets, None);
    r.update_rms = (theta.iter().zip(&before).map(|(a, b)| (a - b) * (a - b)).sum::<f64>() / theta.len() as f64).sqrt();
    r
}
