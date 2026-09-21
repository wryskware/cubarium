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

/// How a centre was fitted, when it came from **imitation** rather than from a search.
///
/// A clone is an ordinary centre — same schema, same digest and protocol checks, loadable
/// by `--init-center` and by an evaluation — and this block is the record of where its
/// weights came from: which teacher, over which exact streams, and how close the fit got.
/// Absent on every centre a training run wrote, which is what it means for a file to be a
/// search product rather than a clone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImitationProvenance {
    /// [`super::imitate::IMITATION_PROVENANCE`].
    pub provenance: String,
    /// The controller slot imitated ([`super::imitate::TEACHER_CONTROLLER`]).
    pub teacher: String,
    /// FNV-1a 64 over the exact streams fitted, in the order fitted
    /// ([`super::imitate::streams_digest`]).
    pub streams_fnv1a: u64,
    pub streams: usize,
    /// Teacher steps in those streams.
    pub steps: usize,
    /// Adam steps taken over the full stream set.
    pub updates: u32,
    /// Final mean squared error against the teacher's **adapted** action, per action:
    /// forward, turn, feed.
    pub mse: [f64; 3],
    pub mse_mean: f64,
    /// Fraction of teacher steps whose turn sign the clone matches.
    pub turn_sign_agreement: f64,
}

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
    /// The Stage-A start-heading convention the arenas this policy trained on were built
    /// under ([`super::task::START_HEADING_PROTOCOL`]). Absent in a phase-one file, which
    /// is exactly what makes it refusable: those centres were trained on a start aimed at
    /// the food and are not this task's policies.
    #[serde(default)]
    pub start_heading: String,
    /// How full the arenas introduced the founder when these weights were trained
    /// ([`super::task::STARTING_STORES_PROTOCOL`]). Absent in a P2-B file, whose
    /// founders arrived full and could only eat back their own upkeep.
    #[serde(default)]
    pub starting_stores: String,
    /// The stage-specific arena revision, including Stage B's successor separation band
    /// ([`super::task::stage_b_arena_protocol`]). Required for Stage-B centres because
    /// patch stocks and geometry define their task; Stage-A centres remain transferable.
    #[serde(default)]
    pub arena_protocol: String,
    /// FNV-1a 64 of the training run's full protocol, when the file came from one. Kept
    /// as provenance for a warm start; absent in files written before P3-B.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_hash: Option<u64>,
    /// Where a clone's weights came from, when the centre was fitted to a teacher
    /// rather than searched. Absent on a training run's centre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imitation: Option<ImitationProvenance>,
    /// Which arena task the weights were trained on (`a` or `b`). Recorded, not
    /// validated: running a Stage-A centre on Stage B is a transfer measurement worth
    /// taking, not an error.
    #[serde(default)]
    pub stage: String,
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
        file.validate_common(&path.display().to_string())?;
        Ok(file)
    }

    /// [`VoxelPolicyFile::load`]'s checks plus the landed Stage-B band, against an
    /// explicit name for the messages. An evaluation or a warm start that means some
    /// other band says so with [`VoxelPolicyFile::validate_for_band`].
    pub fn validate_named(&self, name: &str) -> Result<(), String> {
        self.validate_for_band(name, super::task::Band::Landed)
    }

    /// The common checks plus the Stage-B arena protocol of `band`: a Stage-B centre is
    /// a policy for one separation band and is refused, never reinterpreted, under
    /// another. Stage-A centres stay transferable.
    pub fn validate_for_band(&self, name: &str, band: super::task::Band) -> Result<(), String> {
        self.validate_common(name)?;
        // The founder is part of the Stage-B string: the browser's arenas carry their own
        // crown-height revision and the blind founder's do not.
        let founder = super::parse_founder(&self.founder).map_err(|e| format!("{name}: {e}"))?;
        let want = super::task::arena_protocol(founder, super::task::Stage::B, band);
        if self.stage == "b" && self.arena_protocol != want {
            let had = if self.arena_protocol.is_empty() {
                "none (an equal-patch pre-P3 Stage-B file)"
            } else {
                self.arena_protocol.as_str()
            };
            return Err(format!(
                "{name}: policy Stage-B arena protocol is {had}, this run's `{}` band \
                 uses `{want}`: the depletion task differs and the weights are not \
                 transferable. Retrain, or say which band the centre came from.",
                band.as_str()
            ));
        }
        Ok(())
    }

    /// The founder, shape, digest and start-convention checks every load makes.
    fn validate_common(&self, name: &str) -> Result<(), String> {
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
        if self.starting_stores != super::task::STARTING_STORES_PROTOCOL {
            let had = if self.starting_stores.is_empty() {
                "none (the founder arrived with a full body and a full reserve, so \
                 eating could only repay its own upkeep)"
            } else {
                self.starting_stores.as_str()
            };
            return Err(format!(
                "{name}: policy starting-stores protocol is {had}, this build's arena \
                 uses `{}`: the task differs and the weights are not transferable. \
                 Retrain.",
                super::task::STARTING_STORES_PROTOCOL
            ));
        }
        if self.start_heading != super::task::START_HEADING_PROTOCOL {
            let had = if self.start_heading.is_empty() {
                "none (a phase-one file: the founder was aimed at its food with a \
                 +/-5 degree jitter)"
            } else {
                self.start_heading.as_str()
            };
            return Err(format!(
                "{name}: policy start-heading protocol is {had}, this build's arena uses \
                 `{}`: the start is a different task and the weights are not \
                 transferable. Retrain.",
                super::task::START_HEADING_PROTOCOL
            ));
        }
        Ok(())
    }

    /// The validated founder this file names.
    pub fn founder(&self) -> Result<Founder, String> {
        self.validate_common("policy")?;
        super::parse_founder(&self.founder)
    }

    /// The controller this file drives one episode with — a validated GRU over this
    /// founder's shape and digest.
    pub fn driver(&self) -> Result<EpisodeDriver, String> {
        self.validate_common("policy")?;
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
            start_heading: crate::es::voxel::task::START_HEADING_PROTOCOL.into(),
            starting_stores: crate::es::voxel::task::STARTING_STORES_PROTOCOL.into(),
            arena_protocol: crate::es::voxel::task::arena_protocol(
                founder,
                crate::es::voxel::task::Stage::A,
                crate::es::voxel::task::Band::Landed,
            ),
            protocol_hash: None,
            imitation: None,
            stage: crate::es::voxel::task::Stage::A.as_str().into(),
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

    /// A phase-one centre — written before the start heading was freed, so its file
    /// carries no `start_heading` — is refused by name rather than silently rerun on a
    /// task it never trained for.
    #[test]
    fn a_phase_one_centre_is_refused_because_its_start_was_a_different_task() {
        // A P2-B centre: the heading was already freed, but its founder arrived full.
        let mut file = sample(Founder::Blind);
        file.starting_stores = String::new();
        let err = file
            .validate_named("gen27-center.json")
            .expect_err("refused");
        assert!(err.contains("full reserve"), "{err}");

        let mut file = sample(Founder::Blind);
        file.start_heading = String::new();
        let err = file
            .validate_named("gen26-center.json")
            .expect_err("refused");
        assert!(err.contains("phase-one"), "{err}");
        assert!(
            err.contains(crate::es::voxel::task::START_HEADING_PROTOCOL),
            "{err}"
        );

        // And a file claiming some other convention is refused too, naming it.
        let mut file = sample(Founder::Browser);
        file.start_heading = "aimed-at-food".into();
        let err = file.validate_named("p.json").expect_err("refused");
        assert!(err.contains("aimed-at-food"), "{err}");

        // The current convention is accepted and drives.
        assert!(sample(Founder::Browser).driver().is_ok());
    }

    #[test]
    fn an_equal_patch_stage_b_policy_is_refused_but_stage_a_remains_transferable() {
        let mut old_b = sample(Founder::Blind);
        old_b.stage = "b".into();
        old_b.arena_protocol.clear();
        let err = old_b.validate_named("old-b.json").expect_err("refused");
        assert!(err.contains("equal-patch"), "{err}");
        assert!(err.contains(crate::es::voxel::task::STAGE_B_LANDED_ARENA_PROTOCOL));

        let mut old_a = sample(Founder::Blind);
        old_a.arena_protocol.clear();
        assert!(old_a.validate_named("old-a.json").is_ok());

        let mut current_b = sample(Founder::Blind);
        current_b.stage = "b".into();
        current_b.arena_protocol = crate::es::voxel::task::STAGE_B_LANDED_ARENA_PROTOCOL.into();
        assert!(current_b.validate_named("current-b.json").is_ok());
    }

    /// A near-rung centre and a landed centre are policies for different tasks: each is
    /// refused by name under the other's band, and accepted under its own.
    #[test]
    fn a_near_rung_centre_is_refused_by_an_unqualified_landed_evaluation() {
        use crate::es::voxel::task::{Band, Stage, arena_protocol};
        let protocol = |band| arena_protocol(Founder::Browser, Stage::B, band);
        let mut near = sample(Founder::Browser);
        near.stage = "b".into();
        near.arena_protocol = protocol(Band::Near);
        let err = near.validate_named("near.json").expect_err("refused");
        assert!(err.contains(&protocol(Band::Near)), "{err}");
        assert!(err.contains(&protocol(Band::Landed)), "{err}");
        assert!(near.validate_for_band("near.json", Band::Near).is_ok());

        let mut landed = sample(Founder::Browser);
        landed.stage = "b".into();
        landed.arena_protocol = protocol(Band::Landed);
        assert!(landed.validate_named("landed.json").is_ok());
        assert!(
            landed.validate_for_band("landed.json", Band::Near).is_err(),
            "a landed centre is not a near-rung policy either"
        );
        // Either way the weights themselves still load — the refusal is about the task.
        assert!(near.driver().is_ok() && landed.driver().is_ok());
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
