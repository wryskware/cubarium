//! **Provenance for the adopted pursuit stopping rule** (workstream V, deliverable 3).
//!
//! The reach envelope became the shipped rule on 2026-09-16
//! (`design/7_Research/ecology-v1-predicate-adoption-2026-09-16.md`). Every artifact this
//! crate writes or reads predates that, so the question each test below answers is the same
//! one: *when a file does not say which rule it ran under, what did it run under?* The answer
//! is never "today's default".
//!
//! The last two tests are the finding the brief asked for as a finding: **no training or
//! held-out layout founds a hunter**, so no episode can observe the rule, so the ES
//! `Protocol` and `PolicyFile` do not record it and every protocol hash is unmoved.

use cubarium_core::OrganismId;
use cubarium_core::hunter::{AttemptOutcome, PursuitStop, StrikeClass, StrikeFrame, StrikeRecord};
use cubarium_search::calibrate::StagePlan;
use cubarium_search::population::PopulationPlan;
use cubarium_search::es::fixture::{self, Ecology};
use cubarium_search::es::trainer::Protocol;
use cubarium_search::evaluate::RunOptions;
use cubarium_surface::{Face, SurfacePoint, Vec2};

// ---------------------------------------------------------------- the shipped default

/// Everything this crate builds a world through must land on the shipped rule when nobody
/// names one, and the shipped rule is the envelope.
#[test]
fn the_shipped_rule_is_what_an_unnamed_run_gets() {
    assert_eq!(PursuitStop::default(), PursuitStop::ReachEnvelope);
    assert_eq!(RunOptions::default().pursuit_stop, PursuitStop::ReachEnvelope);
    for layout in fixture::training_layouts().into_iter().chain(fixture::holdout_layouts()) {
        let (world, _) = layout.build().expect("a layout builds its world");
        assert_eq!(world.pursuit_stop(), PursuitStop::ReachEnvelope, "{}", layout.name);
    }
}

// ---------------------------------------------------------------- what silence means

fn frame(tick: u64) -> StrikeFrame {
    StrikeFrame {
        tick,
        hunter_pos: SurfacePoint::new(Face::Front, 20.0, 32.0),
        hunter_heading: Vec2::new(1.0, 0.0),
        scale: 1.0,
        advertised_reach: 14.82,
        capture_forward: 13.28,
        target: Some(OrganismId { slot: 1, generation: 1 }),
        target_pos: Some(SurfacePoint::new(Face::Front, 26.0, 32.0)),
        target_heading: Some(Vec2::new(1.0, 0.0)),
        target_extent: Some(0.03),
        root_distance: Some(6.0),
        effector_distance: Some(9.0),
        tolerance: Some(4.0),
        in_reach: false,
        body_forward: Some(6.0),
        body_side: Some(0.0),
        grasp_mapped: true,
    }
}

/// A `StrikeRecord` written before its `stop` field existed ran under the forward half-space,
/// and must keep saying so now that the default has moved. The serde default is a **named
/// function** in `hunter::strike`, not `PursuitStop::default()`, exactly so that adopting a
/// new shipped rule cannot relabel every retained artifact's held/delivered reading — which
/// is the reading the whole paired note is built on.
#[test]
fn a_record_with_no_recorded_rule_reads_as_the_rule_before_the_adoption() {
    let record = StrikeRecord {
        hunter: OrganismId { slot: 0, generation: 1 },
        attack_counter: 1,
        intent: Some(frame(0)),
        strike: Some(frame(12)),
        resolution: frame(32),
        outcome: AttemptOutcome::OutOfReach,
        energy_paid: 0.08,
        target_changed: false,
        target_missing_at_resolution: false,
        target_crossed_face: false,
        hunter_crossed_face: false,
        target_speed_windup: Some(1.0),
        target_speed_strike: Some(1.0),
        hunter_speed_windup: Some(0.0),
        hunter_speed_strike: Some(0.2),
        target_turn_windup: Some(0.0),
        target_turn_strike: Some(0.0),
        hunter_turn_windup: Some(0.0),
        hunter_turn_strike: Some(0.0),
        class: StrikeClass::BeganOutOfReach,
        stop: PursuitStop::ForwardHalfSpace,
    };

    let mut json = serde_json::to_value(record).expect("a record serialises");
    assert!(
        json.as_object_mut().expect("an object").remove("stop").is_some(),
        "a record written today states the rule it ran under"
    );
    let old: StrikeRecord = serde_json::from_value(json).expect("a record with no rule reads");
    assert_eq!(
        old.stop,
        PursuitStop::ForwardHalfSpace,
        "a record from before the field existed ran under the half-space"
    );
    assert_ne!(
        old.stop,
        PursuitStop::default(),
        "and it must not be following today's default, which is what the named serde default \
         is for"
    );
    assert_eq!(old, record, "nothing else moved in the round trip");
    // Both names still round-trip, so an arm is legible in either direction.
    for stop in [PursuitStop::ForwardHalfSpace, PursuitStop::ReachEnvelope] {
        let mut r = record;
        r.stop = stop;
        let text = serde_json::to_string(&r).expect("serialises");
        assert_eq!(serde_json::from_str::<StrikeRecord>(&text).expect("reads").stop, stop);
    }
}

/// The same question for a calibration stage. Every retained `StagePlan` was written before
/// this field existed and every one of those stages evaluated the half-space, so that — not
/// the envelope — is what a plan with no `pursuit_stop` reports.
#[test]
fn a_stage_plan_with_no_rule_reports_the_rule_before_the_adoption() {
    let plan = StagePlan {
        stage: "screen".into(),
        prices: vec![0.00036],
        ledger: false,
        plant_record: false,
        no_animals: false,
        motor: "sweep".into(),
        pursuit_stop: PursuitStop::ReachEnvelope.as_str().to_string(),
        build_id: "test".into(),
        horizon_ticks: 125_000,
        sample_every: 500,
        apex_introduce_tick: 6_000,
        seed_set: "training".into(),
        seeds: vec![1001],
        arms: vec![0, 1, 2],
        candidates: vec!["baseline".into()],
        trials: 3,
        workers: 8,
        wall_seconds_cap: 1_800,
    };
    let mut json = serde_json::to_value(&plan).expect("a plan serialises");
    assert_eq!(json["pursuit_stop"], "reach_envelope");
    json.as_object_mut().expect("an object").remove("pursuit_stop");
    let retained: StagePlan = serde_json::from_value(json).expect("a retained plan reads");
    assert_eq!(
        retained.pursuit_stop, "forward_half_space",
        "every retained stage ran the rule before the adoption, and must keep saying so"
    );
    assert_ne!(
        retained.pursuit_stop,
        PursuitStop::default().as_str(),
        "not today's default"
    );
}

/// And the same question for a population comparison, which is the one report in this crate
/// whose **result depends on the rule**: every arm above zero introduces an apex cohort, so the
/// hunt-intent pass is reached and the predicate is a variable of the experiment. The rule is
/// recorded once on the plan — one report, one rule — and a retained report that names none ran
/// the half-space.
#[test]
fn a_population_plan_with_no_rule_reports_the_rule_before_the_adoption() {
    let plan = PopulationPlan {
        build_id: "test".into(),
        config: "fast-leaf".into(),
        config_hash: "09e244392ec91768".into(),
        policy_file: "policy.json".into(),
        policy_generation: 9,
        policy_digest: "0x0".into(),
        protocol_hash: "0x0".into(),
        copies: 4,
        horizon_ticks: 180_000,
        sample_every: 600,
        apex_introduce_tick: 6_000,
        pursuit_stop: PursuitStop::ReachEnvelope.as_str().to_string(),
        seeds: vec![1001, 1002],
        arms: vec![0, 1, 2],
        mixes: vec!["neural".into(), "legacy".into()],
        trials: 12,
        workers: 8,
        wall_seconds_cap: 600,
    };
    let mut json = serde_json::to_value(&plan).expect("a plan serialises");
    assert_eq!(json["pursuit_stop"], "reach_envelope");
    let round_tripped: PopulationPlan =
        serde_json::from_value(json.clone()).expect("a plan written today reads back");
    assert_eq!(round_tripped.pursuit_stop, plan.pursuit_stop);

    json.as_object_mut().expect("an object").remove("pursuit_stop");
    let retained: PopulationPlan = serde_json::from_value(json).expect("a retained report reads");
    assert_eq!(
        retained.pursuit_stop, "forward_half_space",
        "every retained population report ran the rule before the adoption"
    );
    assert_ne!(retained.pursuit_stop, PursuitStop::default().as_str(), "not today's default");
    // Both names round-trip, so an arm is legible in either direction.
    for stop in [PursuitStop::ForwardHalfSpace, PursuitStop::ReachEnvelope] {
        let mut p = plan.clone();
        p.pursuit_stop = stop.as_str().to_string();
        let text = serde_json::to_string(&p).expect("serialises");
        let back: PopulationPlan = serde_json::from_str(&text).expect("reads");
        assert_eq!(back.pursuit_stop, stop.as_str());
    }
}

// ---------------------------------------------------------------- the layouts hold no hunter

/// **The finding.** Every training and held-out layout clears `founders` and places exactly one
/// grazer, so no episode world holds a hunter and the hunt-intent pass the pursuit rule lives
/// in is unreachable. That is why the ES `Protocol` and `PolicyFile` do **not** record the
/// rule: a trained policy is predicate-independent. If this ever fails, the field is owed.
#[test]
fn no_training_or_holdout_layout_founds_a_hunter() {
    let mut checked = 0;
    for layout in fixture::training_layouts().into_iter().chain(fixture::holdout_layouts()) {
        let cfg = layout.config();
        assert!(
            cfg.founders.kinds.is_empty() && cfg.founders.count == 0,
            "layout {} founds a roster of its own",
            layout.name
        );
        let (world, grazer) = layout.build().expect("a layout builds its world");
        assert!(
            world.hunters().members.is_empty(),
            "layout {} founded a hunter; the ES protocol now owes a pursuit-stop field",
            layout.name
        );
        assert!(world.hunters().profile.is_none(), "layout {} declares an apex profile", layout.name);
        assert_eq!(world.state.organisms.len(), 1, "layout {} holds one body", layout.name);
        assert!(world.state.organisms.get(grazer).is_some(), "and it is the grazer");
        checked += 1;
    }
    assert_eq!(checked, 12, "four training layouts and eight held-out ones");
}

/// And the consequence, stated as a hash: **nothing about the pursuit rule reaches the ES
/// protocol at all**, so no protocol hash moved when the shipped rule did. The `sweep`
/// protocol keeps the value workstream T pinned for it, which is the check that the adoption
/// left the trainer's task alone.
#[test]
fn the_adoption_moved_no_protocol_hash() {
    let eco = Ecology::defaults();
    let base: Vec<_> = fixture::training_layouts_on(&eco);
    let protocol = Protocol::new(16, 1_000, 20_260_915, &base);
    let text = serde_json::to_string(&protocol).expect("a protocol serialises");
    assert!(
        !text.contains("pursuit"),
        "the protocol must not record a rule no episode can observe: {text}"
    );
    assert_eq!(
        Protocol::default().hash(),
        0x65c5_1e05_060f_0d5a,
        "the shipped protocol keeps the hash it has always had"
    );
}
