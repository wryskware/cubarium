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
    policies: Policies,
}

/// A table's networks: today's GRU, or two-layer brains (KIN-TRAIN, §10.4) with the
/// lineage's holding knobs.
#[derive(Clone)]
enum Policies {
    Plain(Arc<[Arc<EpisodeGru>]>),
    TwoLayer {
        nets: Arc<[Arc<crate::es::voxel::two_layer::TwoLayerNet>]>,
        arbitration: crate::es::voxel::two_layer::IntentArbitration,
    },
    /// PG-PILOT: no variants; every body runs the centre with an exploring top, logged by
    /// the window's [`crate::es::voxel::pg::Recorder`].
    Explore(crate::es::voxel::pg::ExplorePlan),
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
            policies: Policies::Plain(Arc::from(policies)),
        })
    }

    /// A two-layer lineage's generation from its centre file at `stage` (stage 1: only
    /// the top perturbed, [`crate::es::voxel::kin::stage`]), in candidate order.
    pub fn antithetic_two_layer(
        file: &crate::es::voxel::two_layer::TwoLayerPolicyFile,
        stage: crate::es::voxel::kin::BrainStage,
        arbitration: crate::es::voxel::two_layer::IntentArbitration,
        train_seed: u64,
        sigma: f64,
        generation: u32,
        pairs: usize,
    ) -> Result<VariantTable, String> {
        arbitration.validate()?;
        let founder = crate::es::voxel::parse_founder(&file.founder)?;
        let nets = crate::es::voxel::kin::stage::two_layer_variants(file, stage, train_seed, sigma, generation, pairs)?;
        Ok(Self {
            founder,
            policies: Policies::TwoLayer { nets: Arc::from(nets), arbitration },
        })
    }

    /// A policy-gradient lineage's generation (PG-PILOT): the centre, exploring. It has no
    /// candidates ([`VariantTable::len`] is 0); the window hands it to a recorder instead of
    /// placing variants.
    pub fn explore(founder: Founder, plan: crate::es::voxel::pg::ExplorePlan) -> VariantTable {
        Self { founder, policies: Policies::Explore(plan) }
    }

    /// The exploring plan, for a policy-gradient lineage's table.
    pub fn explore_plan(&self) -> Option<&crate::es::voxel::pg::ExplorePlan> {
        match &self.policies {
            Policies::Explore(p) => Some(p),
            _ => None,
        }
    }

    /// Candidate `index`'s weights (for checks and reports).
    pub fn theta(&self, index: usize) -> &[f64] {
        match &self.policies {
            Policies::Plain(p) => p[index].theta(),
            Policies::TwoLayer { nets, .. } => nets[index].theta(),
            Policies::Explore(p) => p.net.theta(),
        }
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
        match &self.policies {
            Policies::Plain(p) => p.len(),
            Policies::TwoLayer { nets, .. } => nets.len(),
            Policies::Explore(_) => 0,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A fresh controller with shared weights and independent memory.
    pub fn controller(&self, index: usize) -> Box<dyn Controller> {
        match &self.policies {
            Policies::Plain(p) => Box::new(p[index].controller(false)),
            Policies::TwoLayer { nets, arbitration } => Box::new(
                nets[index].brain(*arbitration, self.founder.manifest().controller_period_s),
            ),
            Policies::Explore(_) => panic!("an exploring table has no candidates; its window records it"),
        }
    }
}
