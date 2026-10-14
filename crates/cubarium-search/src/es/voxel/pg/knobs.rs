//! **The policy-gradient learner's knobs** (PG-PILOT): one `[<lineage>.pg]` table of the kin
//! file, read only by a lineage on [`crate::es::voxel::kin::Comparison::PolicyGradient`].
//! Training-side, like the twins layout: nothing in the sim or a save.
//!
//! Discount and GAE's λ are given as **time horizons**, because a body's decisions are not
//! evenly spaced (4 Hz scheduled, plus forced ones, and none while it sleeps): between two
//! decisions `Δt` seconds apart the return is discounted by `γ = exp(−Δt / discount_horizon_s)`
//! and the advantage's trace by `λ = exp(−Δt / lambda_horizon_s)`.

use cubarium_voxel::registry::{Range, Row};
use serde::{Deserialize, Serialize};

/// One lineage's policy-gradient knobs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PgKnobs {
    /// Return discount horizon, simulated seconds: a reward `Δt` seconds after a decision
    /// counts `exp(−Δt / discount_horizon_s)` toward its return.
    pub discount_horizon_s: f64,
    /// GAE trace horizon, simulated seconds: λ between two decisions `Δt` apart is
    /// `exp(−Δt / lambda_horizon_s)` (long: closer to the Monte-Carlo return; short: leans on
    /// the value baseline).
    pub lambda_horizon_s: f64,
    /// PPO's ratio clip `ε` (dimensionless): a decision's probability ratio to the policy that
    /// sampled it counts only inside `[1 − ε, 1 + ε]` in the direction its advantage pushes.
    pub clip: f64,
    /// Entropy bonus weight (per decision, on the normalised-advantage scale): keeps the
    /// sampled choice from collapsing before the advantages say so.
    pub entropy: f64,
    /// Adam learning rate of the top's intent head (the lineage's own optimizer state).
    pub policy_lr: f64,
    /// Adam learning rate of the value baseline.
    pub value_lr: f64,
    /// Hidden width of the value baseline's one tanh layer (count). The baseline reads the
    /// top's input: the frozen bottom's GRU state, the held intent and the time held.
    pub value_hidden: usize,
    /// Passes over one generation's decisions (count).
    pub epochs: usize,
    /// Minibatches per pass, one Adam step of each net per minibatch (count).
    pub minibatches: usize,
    /// Random half-splits averaged into the split-half cosine readout (count; readout only).
    pub splits: usize,
}

impl Default for PgKnobs {
    fn default() -> PgKnobs {
        PgKnobs {
            discount_horizon_s: 300.0,
            lambda_horizon_s: 60.0,
            clip: 0.2,
            entropy: 0.01,
            policy_lr: 3e-4,
            value_lr: 1e-3,
            value_hidden: 32,
            epochs: 4,
            minibatches: 4,
            splits: 16,
        }
    }
}

impl PgKnobs {
    pub fn is_default(&self) -> bool {
        *self == PgKnobs::default()
    }

    /// `(γ, λ)` between two decisions `dt_s` seconds apart.
    pub fn factors(&self, dt_s: f64) -> (f64, f64) {
        let dt = dt_s.max(0.0);
        ((-dt / self.discount_horizon_s).exp(), (-dt / self.lambda_horizon_s).exp())
    }
}

/// The knobs' registry rows under `prefix` (the kin table mounts them at
/// `kin.lineages.*.pg`).
pub fn rows(prefix: &str) -> Vec<Row> {
    let row = |name: &str, unit: &'static str, range: Range, what: &'static str| {
        Row::new(format!("{prefix}.{name}"), unit, range, what)
    };
    vec![
        row("discount_horizon_s", "s", Range::positive(),
            "PG-PILOT: return discount horizon; a reward dt seconds after a decision counts exp(-dt/horizon)"),
        row("lambda_horizon_s", "s", Range::positive(),
            "PG-PILOT: GAE trace horizon; lambda between decisions dt apart is exp(-dt/horizon)"),
        row("clip", "ratio", Range::closed(0.0, 1.0).open_lo(),
            "PG-PILOT: PPO probability-ratio clip epsilon"),
        row("entropy", "weight", Range::nonneg(),
            "PG-PILOT: entropy bonus weight on the intent proposal, normalised-advantage scale"),
        row("policy_lr", "rate", Range::positive(),
            "PG-PILOT: Adam learning rate of the top's intent head"),
        row("value_lr", "rate", Range::positive(),
            "PG-PILOT: Adam learning rate of the value baseline"),
        row("value_hidden", "count", Range::closed(1.0, 1024.0),
            "PG-PILOT: hidden width of the value baseline's tanh layer"),
        row("epochs", "count", Range::closed(1.0, 64.0),
            "PG-PILOT: passes over a generation's decisions"),
        row("minibatches", "count", Range::closed(1.0, 4096.0),
            "PG-PILOT: minibatches per pass (one Adam step each)"),
        row("splits", "count", Range::closed(1.0, 1024.0),
            "PG-PILOT: random half-splits averaged into the split-half cosine readout"),
    ]
}
