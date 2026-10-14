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
//! - **The yardstick**: the split-half cosine of the plain advantage-weighted gradient at
//!   the sampling policy, over random halves of the worlds and of the bodies, beside the
//!   same with the advantages shuffled (the noise floor) and the cosine with the previous
//!   generation's.

use serde::{Deserialize, Serialize};

use super::knobs::PgKnobs;
use crate::es::optimizer::Adam;
use crate::es::voxel::two_layer::net::{TopForward, top_forward};
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

impl Policy<'_> {
    /// The masked proposal probabilities at the temperature into `p` (`M'` long).
    pub fn probs(&self, theta: &[f64], input: &TopInput, p: &mut [f64]) {
        let _ = (theta, input, p);
        todo!()
    }

    /// `ln π(action | input)`.
    pub fn log_prob(&self, theta: &[f64], input: &TopInput, action: usize) -> f64 {
        let _ = (theta, input, action);
        todo!()
    }

    /// The indices of the parameters the policy gradient reaches: the intent head.
    pub fn trained(&self) -> std::ops::Range<usize> {
        todo!()
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
        let _ = (theta, steps, adv, clip, entropy, grad);
        todo!()
    }
}

/// The value baseline's input for one decision: the top's own input
/// `[GRU state | held intent one-hot | time]`.
pub fn features(shape: &TwoLayerShape, input: &TopInput) -> Vec<f64> {
    let _ = (shape, input);
    todo!()
}

impl ValueNet {
    /// A seeded baseline: fan-in-scaled first layer, zero output layer (`V = 0` to start).
    pub fn new(inputs: usize, hidden: usize, seed: u64) -> ValueNet {
        let _ = (inputs, hidden, seed);
        todo!()
    }

    pub fn value(&self, u: &[f64]) -> f64 {
        let _ = u;
        todo!()
    }

    /// Mean squared error to `targets` over `us`; with `grad`, its gradient (to descend)
    /// added in.
    pub fn loss(&self, us: &[&[f64]], targets: &[f64], grad: Option<&mut [f64]>) -> f64 {
        let _ = (us, targets, grad);
        todo!()
    }
}

/// GAE advantages of one trajectory from its steps' `values` and the bootstrap's value
/// (ignored without a bootstrap).
pub fn advantages(traj: &Trajectory, values: &[f64], bootstrap: f64, knobs: &PgKnobs) -> Vec<f64> {
    let _ = (traj, values, bootstrap, knobs);
    todo!()
}

impl PgState {
    pub fn new(shape: &TwoLayerShape, knobs: &PgKnobs, seed: u64) -> PgState {
        let _ = (shape, knobs, seed);
        todo!()
    }
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
    let _ = (policy, theta, adam, state, trajs, knobs, seed, threads);
    todo!()
}

/// Cosine of two vectors (0 when either is zero).
pub fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let _ = (a, b);
    todo!()
}

#[allow(dead_code)]
fn _uses(_: TopForward) {
    let _ = top_forward;
}
