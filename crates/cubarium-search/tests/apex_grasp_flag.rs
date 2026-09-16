//! Workstream U's flag on `apex-audit`: the arm names a rule, the world runs it, and the
//! record says which one it ran under
//! (`design/handoffs/ecology-v1-apex-grasp-opus-2026-09-16.md`).
//!
//! The pair this workstream reports is only a measurement if the arm's own record can be told
//! from the other arm's afterwards, and if a rule that is not one of the two is refused rather
//! than silently defaulted to the shipped half.

use cubarium_core::hunter::PursuitStop;
use cubarium_core::motor::ApexTurnRadius;
use cubarium_core::{MotorModel, World, WorldConfig};
use cubarium_search::apex_audit::{AuditReport, StrikeAudit, parse_apex_turn_radius};

/// The flag's two names round-trip, and anything else is an error naming both of them.
#[test]
fn an_unrecognised_apex_turn_radius_is_refused_rather_than_defaulted() {
    assert_eq!(parse_apex_turn_radius("grasp").expect("the shipped rule"), ApexTurnRadius::Grasp);
    assert_eq!(parse_apex_turn_radius("lobes").expect("the variant"), ApexTurnRadius::Lobes);
    assert_eq!(parse_apex_turn_radius(" lobes ").expect("trimmed"), ApexTurnRadius::Lobes);
    let err = parse_apex_turn_radius("claws").expect_err("an unknown rule is refused");
    assert!(err.contains("grasp") && err.contains("lobes"), "{err}");
    assert!(parse_apex_turn_radius("").is_err(), "an empty name is not the shipped rule");
}

/// The rule the arm carries is the rule the world runs: `run_one` sets it on the `World` the
/// same way it sets the motor contract and the pursuit stop, it is independent of both, and the
/// default is what a world that was never told anything already has.
#[test]
fn the_rule_an_arm_names_is_the_rule_its_world_runs() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    assert_eq!(world.apex_turn_radius(), ApexTurnRadius::Grasp, "the shipped default");
    world.set_apex_turn_radius(ApexTurnRadius::Lobes);
    assert_eq!(world.apex_turn_radius(), ApexTurnRadius::Lobes);
    world.set_apex_turn_radius(ApexTurnRadius::Grasp);
    assert_eq!(world.apex_turn_radius(), ApexTurnRadius::Grasp);
    // Independent of the other two transients of this experiment family.
    world.set_motor_model(MotorModel::Inertial);
    world.set_pursuit_stop(PursuitStop::ReachEnvelope);
    world.set_apex_turn_radius(ApexTurnRadius::Lobes);
    assert_eq!(world.motor_model(), MotorModel::Inertial);
    assert_eq!(world.pursuit_stop(), PursuitStop::ReachEnvelope);
    assert_eq!(world.apex_turn_radius(), ApexTurnRadius::Lobes);
}

/// A report written **before** the switch existed carries no rule, and that reads as the
/// shipped `grasp` — there was exactly one rule in this workspace when P's and T's artifacts
/// were written, which is why the field defaults rather than refusing. Taken by deleting the
/// key from a report this build serialised, so the shape is the real one and not a guess.
#[test]
fn a_record_from_before_the_switch_reads_as_the_shipped_rule() {
    let report = AuditReport {
        build: "test".to_string(),
        configs: vec!["fast-leaf".to_string()],
        seeds: vec![1],
        apex_founders: 2,
        introduce_tick: 6_000,
        founder_age_seconds: 0.0,
        ledger: true,
        pursuit_stop: PursuitStop::ReachEnvelope,
        motor: MotorModel::Sweep.name().to_string(),
        apex_turn_radius: ApexTurnRadius::Lobes.name().to_string(),
        apex_motor: None,
        horizon_ticks: 180_000,
        workers: 8,
        wall_seconds: 0.0,
        verdict: String::new(),
        strikes: StrikeAudit::default(),
        rows: Vec::new(),
    };
    let text = serde_json::to_string(&report).expect("serialises");
    let back: AuditReport = serde_json::from_str(&text).expect("round-trips");
    assert_eq!(back.apex_turn_radius, "lobes", "the arm's own rule is recorded, not assumed");
    assert_eq!(back.pursuit_stop, PursuitStop::ReachEnvelope);

    let mut value: serde_json::Value = serde_json::from_str(&text).expect("as a value");
    let object = value.as_object_mut().expect("an object");
    object.remove("apex_turn_radius").expect("the key was there");
    object.remove("motor").expect("the key was there");
    let older: AuditReport = serde_json::from_value(value).expect("an older report still decodes");
    assert_eq!(older.apex_turn_radius, "grasp", "no rule recorded is the shipped rule");
    assert_eq!(older.motor, "sweep", "and no contract recorded is the shipped contract");
}
