//! Workstream W's flag on `apex-audit`: the arm names the contract its *member* runs, the world
//! runs it, and the record says which contract the member ran
//! (`design/handoffs/ecology-v1-apex-motor-isolation-opus-2026-09-16.md`).
//!
//! The pair this workstream reports is only a measurement if one arm's record can be told from
//! the other's afterwards, and if a contract name that is not one of the two is refused rather
//! than silently defaulted to the world's own.
//!
//! **Where the contract is recorded, and why.** The brief allows the strike record's motor name
//! or a new per-row field. The strike record lives in `crates/cubarium-core/src/hunter/`, which
//! this workstream must not touch, so the contract the member ran is a new per-row and
//! per-report field `apex_motor`. `None` means "the world's own `motor`", which is exactly what
//! every row written before the override existed means, so an older artifact still reads
//! correctly without a second name for the same thing.

use cubarium_core::hunter::PursuitStop;
use cubarium_core::motor::ApexTurnRadius;
use cubarium_core::{MotorModel, World, WorldConfig};
use cubarium_search::apex_audit::{AuditReport, StrikeAudit, parse_apex_motor};

/// The flag's two names round-trip, and anything else is an error naming both of them.
#[test]
fn an_unrecognised_apex_motor_is_refused_rather_than_defaulted() {
    assert_eq!(parse_apex_motor("sweep").expect("the shipped contract"), MotorModel::Sweep);
    assert_eq!(parse_apex_motor("inertial").expect("the disc model"), MotorModel::Inertial);
    assert_eq!(parse_apex_motor(" inertial ").expect("trimmed"), MotorModel::Inertial);
    let err = parse_apex_motor("disc").expect_err("an unknown contract is refused");
    assert!(err.contains("sweep") && err.contains("inertial"), "{err}");
    assert!(parse_apex_motor("").is_err(), "an empty name is not the world's own contract");
}

/// The contract the arm carries is the contract the world runs: `run_one` sets it on the `World`
/// the same way it sets the world's motor, the pursuit stop and the apex turn radius, it is
/// independent of all three, and `None` is what a world that was never told anything already has.
#[test]
fn the_contract_an_arm_names_is_the_contract_its_world_runs() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    assert_eq!(world.apex_motor_model(), None, "the shipped default is no override");
    world.set_apex_motor_model(Some(MotorModel::Inertial));
    assert_eq!(world.apex_motor_model(), Some(MotorModel::Inertial));
    world.set_apex_motor_model(None);
    assert_eq!(world.apex_motor_model(), None);

    world.set_motor_model(MotorModel::Sweep);
    world.set_pursuit_stop(PursuitStop::ReachEnvelope);
    world.set_apex_turn_radius(ApexTurnRadius::Grasp);
    world.set_apex_motor_model(Some(MotorModel::Inertial));
    assert_eq!(world.motor_model(), MotorModel::Sweep, "the world's own contract is untouched");
    assert_eq!(world.pursuit_stop(), PursuitStop::ReachEnvelope);
    assert_eq!(world.apex_turn_radius(), ApexTurnRadius::Grasp);
    assert_eq!(world.apex_motor_model(), Some(MotorModel::Inertial));
}

fn report(apex_motor: Option<String>) -> AuditReport {
    AuditReport {
        build: "test".to_string(),
        configs: vec!["fast-leaf".to_string()],
        seeds: vec![1],
        apex_founders: 2,
        introduce_tick: 6_000,
        founder_age_seconds: 0.0,
        ledger: true,
        pursuit_stop: PursuitStop::ReachEnvelope,
        motor: MotorModel::Sweep.name().to_string(),
        apex_turn_radius: ApexTurnRadius::Grasp.name().to_string(),
        apex_motor,
        horizon_ticks: 180_000,
        workers: 8,
        wall_seconds: 0.0,
        verdict: String::new(),
        strikes: StrikeAudit::default(),
        rows: Vec::new(),
    }
}

/// The arm's own contract is recorded, not assumed, and it is recorded *beside* the world's own —
/// the two arms of this pair differ in exactly this one field.
#[test]
fn the_record_says_which_contract_the_member_ran() {
    let off = report(Some(MotorModel::Sweep.name().to_string()));
    let on = report(Some(MotorModel::Inertial.name().to_string()));
    assert_eq!(off.motor, on.motor, "the world's own contract is the same in both arms");
    assert_ne!(off.apex_motor, on.apex_motor, "and the member's is not");
    assert_eq!(off.apex_motor_ran(), "sweep");
    assert_eq!(on.apex_motor_ran(), "inertial");
    for r in [&off, &on] {
        let text = serde_json::to_string(r).expect("serialises");
        let back: AuditReport = serde_json::from_str(&text).expect("round-trips");
        assert_eq!(back.apex_motor, r.apex_motor);
        assert_eq!(back.motor, r.motor);
    }
}

/// A record from **before** the override existed carries no `apex_motor`, and that means the
/// member ran the world's own contract — which is what it did. Taken by deleting the key from a
/// report this build serialised, so the shape is the real one and not a guess.
#[test]
fn a_record_from_before_the_override_reads_as_the_worlds_own_contract() {
    let text = serde_json::to_string(&report(None)).expect("serialises");
    let mut value: serde_json::Value = serde_json::from_str(&text).expect("as a value");
    let object = value.as_object_mut().expect("an object");
    object.remove("apex_motor");
    let older: AuditReport = serde_json::from_value(value).expect("an older report still decodes");
    assert_eq!(older.apex_motor, None, "no override recorded is no override");
    assert_eq!(older.apex_motor_ran(), older.motor, "the member ran the world's own contract");
    assert_eq!(older.apex_motor_ran(), "sweep");
}
