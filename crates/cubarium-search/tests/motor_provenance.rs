//! **Provenance for the motor contract** (workstream T, deliverable 1).
//!
//! The ES protocol and every exported policy must say which motor model the weights were
//! trained under, and a policy trained under one must be refused *by name* under the other —
//! the envelope a body moves in and the price of every radian it turns are not the same, so a
//! score carried across that change compares two tasks.
//!
//! The default must cost nothing. `MotorModel::Sweep` is skipped from the protocol's JSON, so
//! every protocol, checkpoint and policy written before the switch existed keeps the hash the
//! motor switch found it with.
//!
//! **The ring world moved it anyway, and by design** (SYNC-1, 2026-09-16,
//! `design/7_Research/flat-world-sync-main-2026-09-16.md`). A protocol hash covers
//! `config_hash` and every `layout_hash`, and both are taken over `serde_json` of the whole
//! `WorldConfig` — which now carries `topology`, `world_scale` and `version` 9. So **every ES
//! checkpoint and every exported policy written before the ring world is foreign to this
//! build**, exactly as every snapshot written before it is refused by schema. That is the
//! standing always-fresh rule reaching the trainer, and it is the right answer: a policy
//! trained on a cube has not been trained on a ring, and the protocol hash is what says so.
//! `the_protocol_hash_moved_only_because_the_config_json_gained_the_ring_world` below proves
//! that is the whole cause.

use cubarium_core::{MotorModel, WorldConfig};
use cubarium_search::calibrate::config_hash;
use cubarium_search::es::export::PolicyFile;
use cubarium_search::es::fixture::{self, Ecology};
use cubarium_search::es::trainer::Protocol;

/// `Protocol::default().hash()` on this build. Commit 2eb8a9f — before `MotorModel` existed —
/// and every build up to the ring world printed [`PRE_RING_PROTOCOL_HASH`]; see the module
/// docs for why that moved and the test below for the proof that nothing else did.
const PINNED_SWEEP_PROTOCOL_HASH: u64 = 0x831c_a195_c697_cec8;

/// What commit 2eb8a9f, and `main` at `15a2210`, printed for the same protocol.
const PRE_RING_PROTOCOL_HASH: u64 = 0x65c5_1e05_060f_0d5a;

/// `config_hash(&WorldConfig::default())` on `main` at `15a2210`, before the ring world.
const PRE_RING_CONFIG_HASH: u64 = 18_166_095_531_363_627_169;

/// **Why the protocol hash moved, proved rather than asserted.** Strike the ring world's two
/// appended fields from the config's JSON and put `version` back to 8, and the hash the
/// workspace had before the ring world comes back — so those three fields are the whole of the
/// difference, and no ecology value in the config moved with them.
#[test]
fn the_protocol_hash_moved_only_because_the_config_json_gained_the_ring_world() {
    let cfg = WorldConfig::default();
    let json = serde_json::to_string(&cfg).expect("the config serialises");
    assert!(json.contains(r#""topology":"Cube""#), "{json}");
    assert!(json.contains(r#""world_scale":1.0"#), "{json}");
    assert!(json.starts_with(r#"{"version":9,"#), "{json}");

    let pre_ring = json
        .replace(r#","topology":"Cube","world_scale":1.0"#, "")
        .replace(r#"{"version":9,"#, r#"{"version":8,"#);
    // `calibrate::config_hash`'s own loop, over the text it would have been given before the
    // ring world. Its multiplier is `0x1000_0000_01b3`, not the textbook FNV-1a prime; that is
    // what the retained rows were hashed with, so it is what this reproduction uses.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in pre_ring.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    assert_eq!(
        h, PRE_RING_CONFIG_HASH,
        "the config moved by more than the ring world's three fields"
    );
    assert_ne!(config_hash(&cfg), PRE_RING_CONFIG_HASH);
    assert_ne!(PINNED_SWEEP_PROTOCOL_HASH, PRE_RING_PROTOCOL_HASH);
}

/// Adding the motor to the protocol must not move a single existing hash: `sweep` is skipped
/// from the JSON exactly as the R2a `min` aggregate is. (The ring world did move it, for a
/// reason of its own: see the module docs and the test above.)
#[test]
fn the_sweep_protocol_keeps_the_hash_it_has_always_had() {
    let p = Protocol::default();
    assert_eq!(p.motor, MotorModel::Sweep);
    assert_eq!(
        p.hash(),
        PINNED_SWEEP_PROTOCOL_HASH,
        "the default protocol's hash moved again; a motor or aggregate default has leaked \
         into the JSON (the ring world's own move is already in the pinned value)"
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
