//! The voxel policy file and its load path: one trained centre as a self-contained file
//! the commands and the later viewer can load.
//!
//! A policy file is exact: the weights are little-endian IEEE-754 hex
//! ([`crate::es::bits`]), so a load is bit-identical to the save, and the file carries the
//! founder lineage, the manifest digest the weights were authored against, and the train
//! seed. [`VoxelPolicyFile::validate`] refuses another founder's name, a wrong-length
//! vector, a non-finite weight, and a digest mismatch **by name** — the schema-digest
//! rule: a policy is refused, never reinterpreted.

use std::path::Path;

use cubarium_voxel_fauna::Founder;
use serde::{Deserialize, Serialize};

use super::controller::EpisodeDriver;
use super::voxel_schema_digest;
use crate::es::bits::hex_f64s;

/// The policy file's schema token.
pub const POLICY_SCHEMA: &str = "cub-voxel-policy-1";

/// One trained centre, self-contained.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoxelPolicyFile {
    pub schema: String,
    pub build: String,
    /// The founder lineage name the weights were authored against.
    pub founder: String,
    /// [`cubarium_voxel_fauna::Manifest::digest`] of that founder's manifest at the time
    /// the policy was written.
    pub digest: u64,
    pub train_seed: u64,
    /// The generation this centre came from, when the file is a training run's centre.
    pub generation: Option<u64>,
    /// Its training-set score, when recorded.
    pub score: Option<f64>,
    /// The exact weights, in [`crate::es::tensor`]'s shape order for this founder.
    #[serde(with = "hex_f64s")]
    pub theta: Vec<f64>,
}

impl VoxelPolicyFile {
    /// Write the file as pretty JSON.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    /// Read a file and refuse it by name when it is not a valid policy for its own
    /// declared founder: wrong schema token, unknown founder, wrong-length vector, a
    /// non-finite weight, or a digest that is not that founder manifest's.
    pub fn load(path: &Path) -> Result<VoxelPolicyFile, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let file: VoxelPolicyFile =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if file.schema != POLICY_SCHEMA {
            return Err(format!(
                "{}: schema `{}` is not {POLICY_SCHEMA}",
                path.display(),
                file.schema
            ));
        }
        file.validate_named(&path.display().to_string())?;
        Ok(file)
    }

    /// [`VoxelPolicyFile::load`]'s checks, against an explicit name for the messages.
    pub fn validate_named(&self, name: &str) -> Result<(), String> {
        let founder = super::parse_founder(&self.founder).map_err(|e| format!("{name}: {e}"))?;
        let manifest = founder.manifest();
        let want = manifest.parameter_count();
        if self.theta.len() != want {
            return Err(format!(
                "{name}: theta has {} values, expected {want} for {}",
                self.theta.len(),
                founder.name()
            ));
        }
        if let Some(i) = self.theta.iter().position(|x| !x.is_finite()) {
            return Err(format!("{name}: theta[{i}] is not finite"));
        }
        let digest = voxel_schema_digest(founder);
        if self.digest != digest {
            return Err(format!(
                "{name}: policy digest {:#018x} is not this build's {} manifest digest \
                 {digest:#018x}: the schemas differ and the weights cannot be reinterpreted",
                self.digest,
                founder.name()
            ));
        }
        Ok(())
    }

    /// The validated founder this file names.
    pub fn founder(&self) -> Result<Founder, String> {
        self.validate_named("policy")?;
        super::parse_founder(&self.founder)
    }

    /// The controller this file drives one episode with — a validated GRU over this
    /// founder's shape and digest.
    pub fn driver(&self) -> Result<EpisodeDriver, String> {
        self.validate_named("policy")?;
        let founder = super::parse_founder(&self.founder)?;
        EpisodeDriver::gru(&self.theta, founder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::voxel::trainer::CHECKPOINT_SCHEMA;

    fn sample(founder: Founder) -> VoxelPolicyFile {
        let theta = if founder == Founder::Blind {
            crate::es::tensor::initial_center_shape::<23, 3>(5)
        } else {
            crate::es::tensor::initial_center_shape::<37, 3>(5)
        };
        VoxelPolicyFile {
            schema: POLICY_SCHEMA.into(),
            build: "test".into(),
            founder: founder.name().into(),
            digest: voxel_schema_digest(founder),
            train_seed: 20_260_918,
            generation: Some(3),
            score: Some(0.25),
            theta,
        }
    }

    /// A policy round-trips exactly and loads into a validated driver.
    #[test]
    fn a_policy_round_trips_exactly_and_drives() {
        for founder in Founder::ALL {
            let file = sample(founder);
            let dir =
                std::env::temp_dir().join(format!("cubarium-voxel-policy-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("temp dir");
            let path = dir.join(format!("{}.json", founder.name()));
            file.write(&path).expect("written");
            let back = VoxelPolicyFile::load(&path).expect("loaded");
            assert_eq!(back, file, "bit-identical, not one ULP off");
            let driver = back.driver().expect("a valid policy drives");
            let _ = driver;
            std::fs::remove_dir_all(&dir).ok();
        }
    }

    /// The refusals are by name: wrong founder lineage, wrong length, non-finite weight,
    /// and a digest that is not the named founder manifest's.
    #[test]
    fn a_policy_is_refused_by_name_never_reinterpreted() {
        // A blind file claiming the browser's lineage: wrong length for the named founder.
        let mut file = sample(Founder::Blind);
        file.founder = Founder::Browser.name().into();
        let err = file.validate_named("p.json").expect_err("refused");
        assert!(err.contains("5571") && err.contains("frondgrazer"), "{err}");

        // A right-length file whose digest is the other founder's.
        let mut file = sample(Founder::Browser);
        file.digest = voxel_schema_digest(Founder::Blind);
        let err = file.validate_named("p.json").expect_err("refused");
        assert!(err.contains("schemas differ"), "{err}");

        // A non-finite weight.
        let mut file = sample(Founder::Blind);
        file.theta[0] = f64::NAN;
        let err = file.validate_named("p.json").expect_err("refused");
        assert!(err.contains("not finite"), "{err}");

        // An unknown lineage name.
        let mut file = sample(Founder::Blind);
        file.founder = "krill".into();
        assert!(file.validate_named("p.json").is_err());
    }

    /// The checkpoint schema token and the policy schema token are distinct beasts.
    #[test]
    fn the_two_store_tokens_do_not_collide() {
        assert_ne!(POLICY_SCHEMA, CHECKPOINT_SCHEMA);
        let mut file = sample(Founder::Blind);
        file.schema = CHECKPOINT_SCHEMA.into();
        assert!(
            file.validate_named("p.json").is_ok(),
            "the token is not checked by validate"
        );
        let dir =
            std::env::temp_dir().join(format!("cubarium-voxel-policy2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("wrong-token.json");
        file.write(&path).expect("written");
        let err = VoxelPolicyFile::load(&path).expect_err("refused");
        assert!(
            err.contains("schema") && err.contains(CHECKPOINT_SCHEMA),
            "{err}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
