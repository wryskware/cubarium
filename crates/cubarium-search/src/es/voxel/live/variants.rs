//! **Every body carries its own brain variant**: one lineage's per-generation table of
//! validated policies, and the controller a body runs — a handle into that table plus the
//! body's own hidden state, so the weights are held once per variant and never copied per
//! body.
//!
//! The table's shape follows the lineage's manifest (its input count), so a new lineage
//! joins by having a manifest and a centre; every manifest shape is supported. A vector that
//! has the wrong length or non-finite values is refused by name.

use std::sync::Arc;

use crate::es::voxel::controller::EpisodeGru;
use crate::es::voxel::trainer::{Candidate, candidate_theta};
use cubarium_voxel_fauna::{Controller, Founder};

/// One lineage's variants for one generation, built from its manifest (I, H, heads).
#[derive(Clone)]
pub struct VariantTable {
    founder: Founder,
    policies: Arc<[Arc<EpisodeGru>]>,
}

impl VariantTable {
    pub fn build(founder: Founder, thetas: &[Vec<f64>]) -> Result<VariantTable, String> {
        let manifest = founder.manifest();
        let policies = thetas
            .iter()
            .map(|theta| EpisodeGru::from_manifest(theta, &manifest).map(Arc::new))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            founder,
            policies: Arc::from(policies),
        })
    }

    /// The generation's table from the lineage centre: `2·pairs` antithetic variants in
    /// candidate order (`pair0+`, `pair0-`, `pair1+`, …), from the trainer's own keyed
    /// perturbation stream.
    pub fn antithetic(
        founder: Founder,
        centre: &[f64],
        train_seed: u64,
        sigma: f64,
        generation: u32,
        pairs: usize,
    ) -> Result<VariantTable, String> {
        let mut eps = vec![0.0; centre.len()];
        let thetas: Vec<Vec<f64>> = (0..pairs)
            .flat_map(|p| [Candidate::Plus(p), Candidate::Minus(p)])
            .map(|c| candidate_theta(centre, train_seed, sigma, generation, c, &mut eps))
            .collect();
        VariantTable::build(founder, &thetas)
    }

    pub fn founder(&self) -> Founder {
        self.founder
    }
    pub fn len(&self) -> usize {
        self.policies.len()
    }
    pub fn is_empty(&self) -> bool {
        self.policies.is_empty()
    }

    /// A fresh controller with shared weights and independent memory.
    pub fn controller(&self, index: usize) -> Box<dyn Controller> {
        Box::new(self.policies[index].controller(false))
    }
}
