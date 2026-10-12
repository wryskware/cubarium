//! **Training in stages** (§10.4, topic 3): stage 1 perturbs only a two-layer brain's top,
//! the bottom frozen at its copy, so choices improve without wrecking movement; stage 2
//! perturbs both. The stage is a per-lineage knob ([`super::BrainStage`]).
//!
//! One [`Perturber`] draws a pair's `ε` for both the variants and the update, so the
//! gradient of a stage-1 lineage is zero on the bottom by construction. A plain brain has
//! no top: its perturber is today's draw exactly, whatever the stage says.

use std::sync::Arc;

use super::config::BrainStage;
use crate::es::rng::perturbation;
use crate::es::voxel::two_layer::{Layer, TwoLayerLayout, TwoLayerNet, TwoLayerPolicyFile};

/// How a lineage's antithetic perturbations are drawn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Perturber {
    /// The layout to restrict to the top (stage 1 of a brain with a top); `None` perturbs
    /// every weight (a plain brain, or stage 2).
    top_only: Option<TwoLayerLayout>,
}

impl Perturber {
    /// Today's draw over the whole vector.
    pub fn plain() -> Perturber {
        Perturber::default()
    }

    /// A two-layer brain's draw at `stage`: the top only at stage 1 (when it has a top).
    pub fn two_layer(layout: &TwoLayerLayout, stage: BrainStage) -> Perturber {
        let restrict = stage == BrainStage::Top && !layout.range(Layer::Top).is_empty();
        Perturber { top_only: restrict.then(|| layout.clone()) }
    }

    /// Pair `pair`'s perturbation in `generation`: the trainer's keyed stream, zeroed
    /// outside the top at stage 1.
    pub fn eps(&self, train_seed: u64, generation: u32, pair: usize, out: &mut [f64]) {
        perturbation(train_seed, u64::from(generation), pair as u64, out);
        if let Some(layout) = &self.top_only {
            layout.restrict(Layer::Top, out);
        }
    }

    /// The `2·pairs` antithetic vectors (`pair0+`, `pair0−`, …) around `centre`.
    pub fn antithetic(&self, centre: &[f64], train_seed: u64, sigma: f64, generation: u32, pairs: usize) -> Vec<Vec<f64>> {
        let mut eps = vec![0.0; centre.len()];
        let mut out = Vec::with_capacity(2 * pairs);
        for p in 0..pairs {
            self.eps(train_seed, generation, p, &mut eps);
            for sign in [1.0, -1.0] {
                out.push(centre.iter().zip(&eps).map(|(t, e)| t + sign * sigma * e).collect());
            }
        }
        out
    }
}

/// A two-layer lineage's generation of variants from its centre file, at `stage`.
pub fn two_layer_variants(
    file: &TwoLayerPolicyFile,
    stage: BrainStage,
    train_seed: u64,
    sigma: f64,
    generation: u32,
    pairs: usize,
) -> Result<Vec<Arc<TwoLayerNet>>, String> {
    let centre = file.net()?;
    let p = Perturber::two_layer(&file.layout(), stage);
    p.antithetic(centre.theta(), train_seed, sigma, generation, pairs)
        .into_iter()
        .map(|theta| TwoLayerNet::new(file.shape.clone(), theta).map(Arc::new))
        .collect()
}
