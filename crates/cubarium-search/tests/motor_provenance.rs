//! **Provenance for the motor contract** (workstream T, deliverable 1).
//!
//! The ES protocol and every exported policy must say which motor model the weights were
//! trained under, and a policy trained under one must be refused *by name* under the other —
//! the envelope a body moves in and the price of every radian it turns are not the same, so a
//! score carried across that change compares two tasks.
//!
//! The default must cost nothing. `MotorModel::Sweep` is skipped from the protocol's JSON, so
//! every protocol, checkpoint and policy written before the switch existed keeps the hash it
//! has always had: the pinned value below was printed by commit 2eb8a9f.

use cubarium_core::MotorModel;
use cubarium_search::es::export::PolicyFile;
use cubarium_search::es::fixture::{self, Ecology};
use cubarium_search::es::trainer::Protocol;

/// `Protocol::default().hash()` on commit 2eb8a9f, before `MotorModel` existed.
const PINNED_SWEEP_PROTOCOL_HASH: u64 = 0x65c5_1e05_060f_0d5a;

/// Adding the motor to the protocol must not move a single existing hash: `sweep` is skipped
/// from the JSON exactly as the R2a `min` aggregate is.
#[test]
fn the_sweep_protocol_keeps_the_hash_it_has_always_had() {
    let p = Protocol::default();
    assert_eq!(p.motor, MotorModel::Sweep);
    assert_eq!(
        p.hash(),
        PINNED_SWEEP_PROTOCOL_HASH,
        "the default protocol's hash moved; every checkpoint and policy on disk is now foreign"
    );
    let json = serde_json::to_string(&p).expect("the protocol serializes");
    assert!(!json.contains("motor"), "the shipped contract is absent from the JSON: {json}");
}

/// And naming the other contract is a different task, with a different hash that says so.
#[test]
fn an_inertial_protocol_is_a_different_task_with_a_different_hash() {
    let eco = Ecology::defaults();
    let sweep: Vec<_> = fixture::training_layouts_on(&eco);
    let inertial: Vec<_> =
        sweep.iter().cloned().map(|l| l.with_motor(MotorModel::Inertial)).collect();
    let a = Protocol::new(16, 4_000, 20_260_915, &sweep);
    let b = Protocol::new(16, 4_000, 20_260_915, &inertial);
    assert_eq!(a.motor, MotorModel::Sweep);
    assert_eq!(b.motor, MotorModel::Inertial);
    assert_ne!(a.hash(), b.hash(), "two motor contracts must not share one protocol hash");
    // The geometry is untouched: only the contract moved, so every layout hash is the same.
    assert_eq!(a.layout_hashes, b.layout_hashes);
    assert_eq!(a.config_hash, b.config_hash, "the motor is not in the config hash");
    let json = serde_json::to_string(&b).expect("serializes");
    assert!(json.contains("\"motor\":\"inertial\""), "the variant names itself: {json}");
}

/// A policy file records its contract, accepts its own, and refuses the other by name.
#[test]
fn a_policy_is_refused_by_name_under_the_other_motor_contract() {
    let theta = vec![0.0; cubarium_search::es::tensor::PARAMS];
    let eco = Ecology::defaults();
    for trained in [MotorModel::Sweep, MotorModel::Inertial] {
        let file = PolicyFile::new(&theta, "test", 0, 0, &eco.label, eco.hash, trained)
            .expect("exportable");
        assert_eq!(file.motor.as_deref(), Some(trained.name()));
        file.check_motor(trained).expect("its own contract is accepted");
        let other =
            if trained.is_sweep() { MotorModel::Inertial } else { MotorModel::Sweep };
        let err = file.check_motor(other).expect_err("the other contract is refused");
        assert!(err.contains(trained.name()) && err.contains(other.name()), "{err}");
    }
}

/// A policy written before the switch existed records no motor — and that is `sweep`, not
/// "unknown". There was one contract in this workspace when it was written, which is why this
/// differs from the ecology check, where `None` is refused.
#[test]
fn a_policy_from_before_the_switch_reads_as_the_shipped_contract() {
    let theta = vec![0.0; cubarium_search::es::tensor::PARAMS];
    let eco = Ecology::defaults();
    let mut file = PolicyFile::new(&theta, "test", 0, 0, &eco.label, eco.hash, MotorModel::Sweep)
        .expect("exportable");
    file.motor = None;
    file.check_motor(MotorModel::Sweep).expect("the one contract it can have ran");
    let err = file.check_motor(MotorModel::Inertial).expect_err("but not the other one");
    assert!(err.contains("sweep") && err.contains("inertial"), "{err}");
}

/// The contract rides on the layout set, so a fixture world is actually built under it — the
/// protocol's record is not a label on a world that ignored it.
#[test]
fn a_layout_builds_its_world_under_the_contract_it_carries() {
    let eco = Ecology::defaults();
    let layouts = fixture::training_layouts_on(&eco);
    let shipped = &layouts[0];
    assert_eq!(shipped.motor, MotorModel::Sweep);
    let (world, _) = shipped.build().expect("the frozen layout builds");
    assert_eq!(world.motor_model(), MotorModel::Sweep);

    let variant = shipped.clone().with_motor(MotorModel::Inertial);
    let (world, _) = variant.build().expect("builds");
    assert_eq!(world.motor_model(), MotorModel::Inertial);
    // The geometry and the staged world are the same: only the contract differs.
    assert_eq!(variant.hash(&variant.config()), shipped.hash(&shipped.config()));
}
