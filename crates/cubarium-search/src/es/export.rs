//! Exporting a trained centre as a self-contained policy the core can attach.
//!
//! The file holds the provenance a development artefact needs — the build, the protocol hash,
//! the generation it came from, and this build's schema digest — together with the exact
//! weights, in [`super::bits`]'s little-endian hex. Reading it back gives a
//! [`cubarium_core::neural::Policy`] that is equal weight for weight to the trained centre,
//! and `World::attach_neural_policy` accepts it like any other.
//!
//! The optimizer's own state is deliberately **not** in here: an exported policy is a set of
//! weights, and an animal's lifetime `hidden`/`held`/`feedback` is the world's, created fresh
//! at attachment. A checkpoint resumes training; an export runs a body.

use cubarium_core::neural::Policy;
use serde::{Deserialize, Serialize};

use super::tensor;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolicyFile {
    pub schema: String,
    pub build: String,
    /// [`cubarium_core::neural::schema_digest`] at export time. A policy whose digest differs
    /// from the loading build's is refused by name, never reinterpreted.
    pub policy_digest: u64,
    pub protocol_hash: u64,
    pub generation: u64,
    pub parameters: usize,
    /// Exact weights in [`super::tensor`]'s flatten order.
    #[serde(with = "crate::es::bits::hex_f64s")]
    pub theta: Vec<f64>,
}

impl PolicyFile {
    pub fn new(
        theta: &[f64],
        build: &str,
        protocol_hash: u64,
        generation: u64,
    ) -> Result<PolicyFile, String> {
        // Refuse to export something the core would not accept.
        tensor::policy(theta)?;
        Ok(PolicyFile {
            schema: "cub-es-policy-1".into(),
            build: build.to_string(),
            policy_digest: cubarium_core::neural::schema_digest(),
            protocol_hash,
            generation,
            parameters: theta.len(),
            theta: theta.to_vec(),
        })
    }

    /// The policy, rebuilt exactly.
    ///
    /// The **compatibility digest is checked first**. `tensor::policy` stamps whatever it
    /// builds with the *current* build's digest, so without this check a file written against
    /// another observation layout, action set, recurrence convention, motor contract or
    /// controller rate — with the same 10,215 weights — would be silently reinterpreted rather
    /// than refused. That is precisely the failure contract §5 has the digest for.
    pub fn policy(&self) -> Result<Policy, String> {
        if self.schema != "cub-es-policy-1" {
            return Err(format!("unknown policy file schema {:?}", self.schema));
        }
        let current = cubarium_core::neural::schema_digest();
        if self.policy_digest != current {
            return Err(format!(
                "policy file was written against schema digest {:#018x}, this build is \
                 {:#018x}: the observation layout, action set, recurrence convention, motor \
                 contract or controller rate differs and the weights cannot be reinterpreted",
                self.policy_digest, current
            ));
        }
        if self.parameters != self.theta.len() {
            return Err("policy file parameter count disagrees with its weights".into());
        }
        tensor::policy(&self.theta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::fixture::training_layouts;

    #[test]
    fn an_exported_policy_round_trips_weight_for_weight_and_the_core_runs_it() {
        let theta = tensor::initial_center(20_260_915);
        let file = PolicyFile::new(&theta, "test", 0xdead_beef, 3).expect("exportable");
        let json = serde_json::to_string(&file).expect("write");
        let back: PolicyFile = serde_json::from_str(&json).expect("read");
        assert_eq!(back, file);

        let original = tensor::policy(&theta).expect("policy");
        let loaded = back.policy().expect("policy");
        assert_eq!(loaded, original, "an exported policy must be the trained one, exactly");
        for (a, b) in tensor::flatten(&loaded.weights).iter().zip(&theta) {
            assert_eq!(a.to_bits(), b.to_bits());
        }

        // Ordinary core inference: attach it to a real body and step the real world.
        let (mut world, id) = training_layouts()[0].build().expect("built");
        world.attach_neural_policy(id, loaded).expect("the core accepts it");
        for _ in 0..120 {
            world.step();
            world.drain_events();
        }
        world.check_invariants().expect("the world stays consistent");
        assert!(world.state.organisms.get(id).is_some(), "the body survived 6 s");
        assert!(world.neural().get(id).is_some(), "and it is still neural");
    }

    #[test]
    fn a_foreign_schema_or_a_mismatched_count_is_refused() {
        let theta = tensor::initial_center(1);
        let mut file = PolicyFile::new(&theta, "test", 0, 0).expect("exportable");
        file.schema = "something-else".into();
        assert!(file.policy().is_err());

        let mut file = PolicyFile::new(&theta, "test", 0, 0).expect("exportable");
        file.parameters = 7;
        assert!(file.policy().is_err());
    }

    /// Review finding 2: a saved compatibility digest that is not this build's must be refused
    /// **by name**, before the weights are rebuilt. Before the repair the field was written,
    /// never read, and `tensor::policy` restamped the file with the current digest.
    #[test]
    fn a_foreign_compatibility_digest_is_refused_by_name() {
        let theta = tensor::initial_center(1);
        let mut file = PolicyFile::new(&theta, "test", 0, 0).expect("exportable");
        assert!(file.policy().is_ok(), "this build's own digest is accepted");

        file.policy_digest ^= 1;
        let err = file.policy().expect_err("a foreign digest must be refused");
        assert!(err.contains("cannot be reinterpreted"), "{err}");
        assert!(
            err.contains(&format!("{:#018x}", file.policy_digest)),
            "the refusal names the file's digest: {err}"
        );

        // A whole-word difference, not just a flipped bit, is refused the same way.
        file.policy_digest = 0;
        assert!(file.policy().is_err());
    }
}
