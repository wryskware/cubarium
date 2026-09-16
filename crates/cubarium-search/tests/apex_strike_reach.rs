//! The reach diagnostic's **aggregation**: that a pile of per-attempt records becomes the two
//! tables the verdict is read off, without losing or double-counting an attempt.
//!
//! The classification itself belongs to `cubarium-core` and is fixed there
//! (`crates/cubarium-core/tests/hunter_strike_record.rs`). What this file fixes is what the
//! audit does with it: one bucket per class and one per outcome, means taken only over the
//! attempts that carried the quantity, extremes preserved so a deciding row is findable, the
//! energy bill summed per bucket, and the raw-record cap counted rather than silent
//! (`design/handoffs/ecology-v1-apex-reach-opus-2026-09-16.md`, deliverable 2).

use cubarium_core::OrganismId;
use cubarium_core::hunter::{AttemptOutcome, StrikeClass, StrikeFrame, StrikeRecord};
use cubarium_search::apex_audit::{MAX_KEPT_RECORDS, Series, StrikeAudit};
use cubarium_surface::{Face, SurfacePoint, Vec2};

const TOLERANCE: f64 = 2.6;

fn id(slot: u32) -> OrganismId {
    OrganismId { slot, generation: 1 }
}

fn frame(tick: u64, separation: Option<f64>) -> StrikeFrame {
    let spot = SurfacePoint::new(Face::Front, 20.0, 32.0);
    StrikeFrame {
        tick,
        hunter_pos: spot,
        hunter_heading: Vec2::new(1.0, 0.0),
        scale: 1.0,
        advertised_reach: 14.83,
        target: separation.map(|_| id(1)),
        target_pos: separation.map(|_| spot),
        target_heading: separation.map(|_| Vec2::new(1.0, 0.0)),
        target_extent: separation.map(|_| 1.1),
        root_distance: separation,
        effector_distance: separation,
        tolerance: separation.map(|_| TOLERANCE),
        in_reach: separation.is_some_and(|d| d <= TOLERANCE),
        grasp_mapped: true,
    }
}

/// One record with stated separations and stated realised speeds.
fn record(
    class: StrikeClass,
    outcome: AttemptOutcome,
    intent: f64,
    strike: f64,
    resolution: f64,
    prey_speed: f64,
    hunter_speed: f64,
) -> StrikeRecord {
    StrikeRecord {
        hunter: id(0),
        attack_counter: 1,
        intent: Some(frame(0, Some(intent))),
        strike: Some(frame(12, Some(strike))),
        resolution: frame(32, Some(resolution)),
        outcome,
        energy_paid: 0.08,
        target_changed: false,
        target_missing_at_resolution: false,
        target_crossed_face: false,
        hunter_crossed_face: false,
        target_speed_windup: Some(prey_speed),
        target_speed_strike: Some(prey_speed),
        hunter_speed_windup: Some(0.0),
        hunter_speed_strike: Some(hunter_speed),
        target_turn_windup: Some(0.0),
        target_turn_strike: Some(0.0),
        class,
    }
}

fn audit(records: Vec<StrikeRecord>) -> StrikeAudit {
    let mut a = StrikeAudit::default();
    for r in records {
        a.sample(r);
    }
    a
}

/// Every attempt lands in exactly one class bucket and exactly one outcome bucket, and the
/// per-bucket energy is the bill those attempts actually paid.
#[test]
fn every_attempt_is_counted_once_in_each_table() {
    let a = audit(vec![
        record(StrikeClass::ResolvedInReach, AttemptOutcome::Captured, 1.0, 1.0, 1.0, 0.0, 16.0),
        record(StrikeClass::ResolvedInReach, AttemptOutcome::Missed, 1.0, 1.0, 2.0, 0.0, 16.0),
        record(StrikeClass::PreyOutran, AttemptOutcome::OutOfReach, 9.0, 8.0, 12.0, 10.0, 6.0),
        record(StrikeClass::BeganOutOfReach, AttemptOutcome::OutOfReach, 9.0, 8.0, 7.0, 1.0, 6.0),
    ]);
    assert_eq!(a.recorded, 4);
    assert_eq!((a.dropped, a.unreadable, a.omitted), (0, 0, 0));
    let by_class: u64 = a.by_class.iter().map(|(_, s)| s.count).sum();
    let by_outcome: u64 = a.by_outcome.iter().map(|(_, s)| s.count).sum();
    assert_eq!((by_class, by_outcome), (4, 4));

    let out = a.stats_for_outcome(AttemptOutcome::OutOfReach).expect("the reach refusals");
    assert_eq!(out.count, 2);
    assert!((out.energy_paid - 0.16).abs() < 1e-12, "{}", out.energy_paid);
    // Captures and reach refusals are readable against each other on the same fields, which is
    // the comparison the workstream exists to make.
    let caught = a.stats_for_outcome(AttemptOutcome::Captured).expect("the captures");
    assert_eq!(caught.resolved_in_reach, 1);
    assert_eq!(out.resolved_in_reach, 0);
    assert!(caught.intent_separation.mean().unwrap() < out.intent_separation.mean().unwrap());
}

/// The reporting order is fixed, so two runs' tables line up, and a class with no attempts is
/// absent rather than a zero row that invites a division.
#[test]
fn the_class_table_is_printed_in_a_fixed_order() {
    let a = audit(vec![
        record(StrikeClass::BeganOutOfReach, AttemptOutcome::OutOfReach, 9.0, 8.0, 7.0, 1.0, 6.0),
        record(StrikeClass::ResolvedInReach, AttemptOutcome::Captured, 1.0, 1.0, 1.0, 0.0, 16.0),
        record(StrikeClass::PreyOutran, AttemptOutcome::OutOfReach, 9.0, 8.0, 12.0, 10.0, 6.0),
    ]);
    let order: Vec<StrikeClass> = a.classes_in_order().into_iter().map(|(c, _)| c).collect();
    assert_eq!(
        order,
        vec![StrikeClass::ResolvedInReach, StrikeClass::PreyOutran, StrikeClass::BeganOutOfReach]
    );
}

/// A mean is taken over the attempts that actually carried the quantity, and the extremes
/// survive summation, so a verdict can name the row it came from.
#[test]
fn a_series_averages_only_what_it_was_given_and_keeps_the_extremes() {
    let mut a = Series::default();
    assert_eq!(a.mean(), None, "an empty series has no mean to report");
    for v in [3.0, 1.0, 5.0] {
        a.add(&Series { n: 1, sum: v, min: v, max: v });
    }
    assert_eq!((a.n, a.min, a.max), (3, 1.0, 5.0));
    assert!((a.mean().unwrap() - 3.0).abs() < 1e-12);
    // A record with a missing frame contributes to `count` but not to that frame's series.
    let mut partial = record(
        StrikeClass::BeganOutOfReach,
        AttemptOutcome::OutOfReach,
        9.0,
        8.0,
        7.0,
        1.0,
        6.0,
    );
    partial.intent = None;
    let audited = audit(vec![
        partial,
        record(StrikeClass::BeganOutOfReach, AttemptOutcome::OutOfReach, 5.0, 4.0, 3.0, 1.0, 6.0),
    ]);
    let (_, stats) = audited.by_class[0];
    assert_eq!(stats.count, 2);
    assert_eq!(stats.intent_separation.n, 1, "the missing frame is not averaged as a zero");
    assert!((stats.intent_separation.mean().unwrap() - 5.0).abs() < 1e-12);
    assert_eq!(stats.resolution_separation.n, 2);
}

/// Two runs' audits merge without losing an attempt, which is how the report's totals are
/// built from per-seed rows.
#[test]
fn two_runs_merge_without_losing_an_attempt() {
    let mut a = audit(vec![record(
        StrikeClass::ResolvedInReach,
        AttemptOutcome::Captured,
        1.0,
        1.0,
        1.0,
        0.0,
        16.0,
    )]);
    let b = audit(vec![
        record(StrikeClass::ResolvedInReach, AttemptOutcome::Missed, 2.0, 2.0, 2.0, 0.0, 16.0),
        record(StrikeClass::PreyOutran, AttemptOutcome::OutOfReach, 9.0, 8.0, 12.0, 10.0, 6.0),
    ]);
    a.add(&b);
    assert_eq!(a.recorded, 3);
    let (_, resolved) = a.by_class.iter().find(|(c, _)| *c == StrikeClass::ResolvedInReach).unwrap();
    assert_eq!(resolved.count, 2);
    assert_eq!((resolved.intent_separation.min, resolved.intent_separation.max), (1.0, 2.0));
    assert_eq!(a.by_outcome.iter().map(|(_, s)| s.count).sum::<u64>(), 3);
}

/// The raw-record cap bounds the artifact and is counted; the aggregates still see every
/// attempt, so a capped run's table is complete even though its record list is not.
#[test]
fn the_raw_record_cap_is_counted_and_does_not_touch_the_aggregates() {
    let n = MAX_KEPT_RECORDS + 7;
    let a = audit(
        (0..n)
            .map(|_| {
                record(
                    StrikeClass::BeganOutOfReach,
                    AttemptOutcome::OutOfReach,
                    9.0,
                    8.0,
                    7.0,
                    1.0,
                    6.0,
                )
            })
            .collect(),
    );
    assert_eq!(a.records.len(), MAX_KEPT_RECORDS);
    assert_eq!(a.omitted, 7);
    assert_eq!(a.recorded, n as u64);
    let (_, stats) = a.by_class[0];
    assert_eq!(stats.count, n as u64);
    assert_eq!(stats.resolution_separation.n, n as u64);
}
