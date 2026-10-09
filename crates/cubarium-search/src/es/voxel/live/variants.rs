//! **Every body carries its own brain variant**: one lineage's per-generation table of
//! validated policies, and the controller a body runs — a handle into that table plus the
//! body's own hidden state, so the weights are held once per variant and never copied per
//! body.
//!
//! The table's shape follows the lineage's manifest (its input count), so a new lineage
//! joins by having a manifest and a centre; a manifest with an input width no table has
//! yet is refused by name here, which is the one place a new width is added.

use std::sync::Arc;

use cubarium_core::neural::ShapePolicy;
use cubarium_core::neural::gru::HIDDEN;
use cubarium_voxel_fauna::{Controller, Founder, Response};

use crate::es::tensor;
use crate::es::voxel::trainer::{Candidate, candidate_theta};
use crate::es::voxel::{BLIND_I, BLIND_O, BROWSER_I, BROWSER_O, voxel_schema_digest};

/// A body's controller: a handle into its lineage's table for this generation, and its
/// own memory.
pub struct TableGru<const I: usize, const O: usize> {
    table: Arc<[ShapePolicy<I, O>]>,
    index: usize,
    hidden: [f64; HIDDEN],
}

impl<const I: usize, const O: usize> Controller for TableGru<I, O> {
    fn drive(&mut self, observation: &[f64]) -> Response {
        let mut x = [0.0; I];
        x.copy_from_slice(observation);
        let logits = self.table[self.index].weights.forward(&x, &mut self.hidden);
        Response::logits(&logits)
    }

    fn reset(&mut self) {
        self.hidden = [0.0; HIDDEN];
    }
}

/// One lineage's variants for one generation, validated against its manifest digest.
#[derive(Clone)]
pub enum VariantTable {
    /// The littershredder's v3 shape ([`BLIND_I`] → [`BLIND_O`]).
    I23(Founder, Arc<[ShapePolicy<BLIND_I, BLIND_O>]>),
    /// The frondgrazer's ([`BROWSER_I`] → [`BROWSER_O`]).
    I37(Founder, Arc<[ShapePolicy<BROWSER_I, BROWSER_O>]>),
}

impl VariantTable {
    /// A table of `thetas`, in order, for `founder`. Refuses a vector the manifest does
    /// not accept (length, finiteness, digest) and a manifest width no table serves.
    pub fn build(founder: Founder, thetas: &[Vec<f64>]) -> Result<VariantTable, String> {
        let digest = voxel_schema_digest(founder);
        fn policies<const I: usize, const O: usize>(
            thetas: &[Vec<f64>],
            digest: u64,
        ) -> Result<Arc<[ShapePolicy<I, O>]>, String> {
            thetas
                .iter()
                .map(|t| tensor::shape_policy::<I, O>(t, digest))
                .collect::<Result<Vec<_>, _>>()
                .map(Arc::from)
        }
        let m = founder.manifest();
        match (m.inputs(), m.outputs()) {
            (BLIND_I, BLIND_O) => Ok(VariantTable::I23(founder, policies(thetas, digest)?)),
            (BROWSER_I, BROWSER_O) => Ok(VariantTable::I37(founder, policies(thetas, digest)?)),
            (n, o) => Err(format!(
                "the {} manifest has {n} inputs and {o} outputs; live training has no table of \
                 that shape yet (contract v3's P3 makes the runtime generic)",
                founder.name()
            )),
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
        match self {
            VariantTable::I23(f, _) | VariantTable::I37(f, _) => *f,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            VariantTable::I23(_, t) => t.len(),
            VariantTable::I37(_, t) => t.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A fresh controller running variant `index`: fresh memory, the table shared.
    pub fn controller(&self, index: usize) -> Box<dyn Controller> {
        assert!(index < self.len(), "variant {index} of {}", self.len());
        match self {
            VariantTable::I23(_, t) => Box::new(TableGru::<BLIND_I, BLIND_O> {
                table: Arc::clone(t),
                index,
                hidden: [0.0; HIDDEN],
            }),
            VariantTable::I37(_, t) => Box::new(TableGru::<BROWSER_I, BROWSER_O> {
                table: Arc::clone(t),
                index,
                hidden: [0.0; HIDDEN],
            }),
        }
    }
}
