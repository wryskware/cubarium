//! What the **paired predicate arm** adds to the reach audit: the delivered/held split and the
//! initial-gap bins Astra's confirmation rule is stated in
//! (`design/7_Research/ecology-v1-round3-review-2026-09-16.md`, next steps item 1 — "record …
//! initial gap, relative closure, … capture opportunity by initial gap bin").
//!
//! `apex_strike_reach.rs` already fixes the class and outcome buckets. These tests fix only
//! what is new, and they fix it as **partitions**: an attempt is held or delivered or has no
//! strike frame, never two of those; it lands in exactly one gap bin; and both splits survive
//! the merge across seeds with nothing lost or double-counted.
//!
//! Every fixture is a hand-built record with stated geometry. Nothing here runs a world.

use cubarium_core::OrganismId;
use cubarium_core::hunter::{AttemptOutcome, PursuitStop, StrikeClass, StrikeFrame, StrikeRecord};
use cubarium_search::apex_audit::{GAP_BIN_EDGES, GAP_BINS, StrikeAudit, gap_bin_of};
use cubarium_surface::{Face, SurfacePoint, Vec2};

fn id(slot: u32) -> OrganismId {
    OrganismId { slot, generation: 1 }
}

/// A frame at `tick` whose prey sits `separation` from the grasp centre and `forward` along the
/// hunter's own heading. The tolerance is a stated 4.0 px, close to the trial profile's 3.97.
fn frame(tick: u64, separation: f64, forward: f64) -> StrikeFrame {
    let pos = SurfacePoint::new(Face::Front, 20.0, 32.0);
    StrikeFrame {
        tick,
        hunter_pos: pos,
        hunter_heading: Vec2::new(1.0, 0.0),
        scale: 1.0,
        advertised_reach: 14.82,
        capture_forward: 13.28,
        target: Some(id(1)),
        target_pos: Some(pos),
        target_heading: Some(Vec2::new(1.0, 0.0)),
        target_extent: Some(0.03),
        root_distance: Some(forward.abs()),
        effector_distance: Some(separation),
        tolerance: Some(4.0),
        in_reach: separation <= 4.0,
        body_forward: Some(forward),
        body_side: Some(0.0),
        grasp_mapped: true,
    }
}

/// One attempt: the prey `intent_sep` from the grasp when the gesture began, `strike_sep` when
/// the burst was paid for, `res_sep` when it settled, at a stated forward coordinate, under a
/// stated rule.
#[allow(clippy::too_many_arguments)]
fn record(
    stop: PursuitStop,
    outcome: AttemptOutcome,
    class: StrikeClass,
    intent_sep: f64,
    strike_sep: f64,
    res_sep: f64,
    forward: f64,
    hunter_speed: f64,
) -> StrikeRecord {
    StrikeRecord {
        hunter: id(0),
        attack_counter: 1,
        intent: Some(frame(0, intent_sep, forward)),
        strike: Some(frame(12, strike_sep, forward)),
        resolution: frame(32, res_sep, forward),
        outcome,
        energy_paid: 0.08,
        target_changed: false,
        target_missing_at_resolution: false,
        target_crossed_face: false,
        hunter_crossed_face: false,
        target_speed_windup: Some(1.0),
        target_speed_strike: Some(1.0),
        hunter_speed_windup: Some(0.0),
        hunter_speed_strike: Some(hunter_speed),
        target_turn_windup: Some(0.0),
        target_turn_strike: Some(0.0),
        hunter_turn_windup: Some(0.0),
        hunter_turn_strike: Some(0.0),
        class,
        stop,
    }
}

fn audit(records: Vec<StrikeRecord>) -> StrikeAudit {
    let mut a = StrikeAudit::default();
    for r in records {
        a.sample(r);
    }
    a
}

// ---------------------------------------------------------------- the delivered / held split

/// Held, delivered and "no strike frame" partition every recorded attempt, and each half
/// carries the same fields the class table does — so the closure a *delivered* burst achieved
/// is readable without subsetting the raw records by hand.
#[test]
fn held_and_delivered_partition_every_recorded_attempt() {
    // Under the shipped rule a prey short of the claws is held; one past them is not.
    let held = record(
        PursuitStop::ForwardHalfSpace,
        AttemptOutcome::OutOfReach,
        StrikeClass::BeganOutOfReach,
        12.0, 12.0, 11.0, 6.0, 0.0,
    );
    let delivered = record(
        PursuitStop::ForwardHalfSpace,
        AttemptOutcome::Captured,
        StrikeClass::ResolvedInReach,
        9.0, 9.0, 2.0, 18.0, 7.0,
    );
    let mut blind = delivered;
    blind.strike = None;

    let a = audit(vec![held, delivered, blind]);
    assert_eq!(a.recorded, 3);
    assert_eq!(a.held.count, 1);
    assert_eq!(a.delivered.count, 1);
    assert_eq!(a.no_strike_frame, 1);
    assert_eq!(a.held.count + a.delivered.count + a.no_strike_frame, a.recorded);

    // The delivered burst closed 7 px and moved the hunter; the held one did neither.
    assert_eq!(a.delivered.separation_change_over_strike.mean(), Some(-7.0));
    assert_eq!(a.delivered.hunter_speed_strike.mean(), Some(7.0));
    assert_eq!(a.held.separation_change_over_strike.mean(), Some(-1.0));
    assert_eq!(a.held.hunter_speed_strike.mean(), Some(0.0));
}

/// The split is taken under the rule the record was made under, never under the reader's. The
/// same geometry is held by the half-space and delivered by the envelope.
#[test]
fn the_split_is_taken_under_the_rule_each_record_was_made_under() {
    let geometry = |stop| {
        record(
            stop,
            AttemptOutcome::OutOfReach,
            StrikeClass::BeganOutOfReach,
            10.0, 10.0, 10.0, 6.0, 0.0,
        )
    };
    let shipped = audit(vec![geometry(PursuitStop::ForwardHalfSpace)]);
    assert_eq!((shipped.held.count, shipped.delivered.count), (1, 0));
    let variant = audit(vec![geometry(PursuitStop::ReachEnvelope)]);
    assert_eq!((variant.held.count, variant.delivered.count), (0, 1));
}

// ---------------------------------------------------------------- the initial-gap bins

/// The bin edges are half-open `[lo, hi)`, cover the line, and are the stated ones.
#[test]
fn the_gap_bins_are_half_open_cover_the_line_and_are_the_stated_edges() {
    assert_eq!(GAP_BIN_EDGES, [4.0, 8.0, 12.0, 16.0]);
    assert_eq!(GAP_BINS, GAP_BIN_EDGES.len() + 1);
    assert_eq!(gap_bin_of(0.0), 0);
    assert_eq!(gap_bin_of(3.999), 0);
    assert_eq!(gap_bin_of(4.0), 1, "an edge belongs to the bin above it");
    assert_eq!(gap_bin_of(7.999), 1);
    assert_eq!(gap_bin_of(8.0), 2);
    assert_eq!(gap_bin_of(12.0), 3);
    assert_eq!(gap_bin_of(16.0), 4);
    assert_eq!(gap_bin_of(1.0e6), 4, "the last bin is unbounded above");
}

/// Every attempt with an intent frame lands in exactly one bin, and the bins' captures and
/// contacts sum to the audit's own totals.
#[test]
fn every_attempt_lands_in_exactly_one_gap_bin_and_the_bins_sum_to_the_totals() {
    let at = |sep: f64, outcome, class| {
        record(PursuitStop::ReachEnvelope, outcome, class, sep, sep, sep, 6.0, 0.0)
    };
    let a = audit(vec![
        at(1.0, AttemptOutcome::Captured, StrikeClass::ResolvedInReach),
        at(2.0, AttemptOutcome::Missed, StrikeClass::ResolvedInReach),
        at(5.0, AttemptOutcome::OutOfReach, StrikeClass::BeganOutOfReach),
        at(9.0, AttemptOutcome::OutOfReach, StrikeClass::BeganOutOfReach),
        at(13.0, AttemptOutcome::OutOfReach, StrikeClass::BeganOutOfReach),
        at(21.0, AttemptOutcome::OutOfReach, StrikeClass::BeganOutOfReach),
    ]);
    assert_eq!(a.by_gap_bin.len(), GAP_BINS, "every bin is reported, even an empty one");
    let attempts: u64 = a.by_gap_bin.iter().map(|b| b.attempts).sum();
    assert_eq!(attempts, a.recorded, "no attempt is lost or double-counted");
    assert_eq!(a.by_gap_bin[0].attempts, 2);
    assert_eq!(a.by_gap_bin[0].captures, 1);
    assert_eq!(a.by_gap_bin[0].resolved_in_reach, 2, "both were in reach at resolution");
    assert_eq!(a.by_gap_bin[1].attempts, 1);
    assert_eq!(a.by_gap_bin[2].attempts, 1);
    assert_eq!(a.by_gap_bin[3].attempts, 1);
    assert_eq!(a.by_gap_bin[4].attempts, 1);
    let captures: u64 = a.by_gap_bin.iter().map(|b| b.captures).sum();
    assert_eq!(captures, 1);
    // The bin is chosen from the **intent** separation: what the member committed at, which is
    // the axis Astra's rule is stated on, not the settlement's.
    let far = record(
        PursuitStop::ReachEnvelope,
        AttemptOutcome::Captured,
        StrikeClass::ResolvedInReach,
        13.0, 5.0, 1.0, 6.0, 12.0,
    );
    let b = audit(vec![far]);
    assert_eq!(b.by_gap_bin[3].attempts, 1);
    assert_eq!(b.by_gap_bin[3].captures, 1);
    assert_eq!(b.by_gap_bin[3].closure_over_strike.mean(), Some(4.0), "strike 5 → resolution 1");
}

/// An attempt with no intent frame has no initial gap and is counted as such rather than
/// guessed into a bin.
#[test]
fn an_attempt_with_no_intent_frame_has_no_initial_gap_and_is_counted() {
    let mut blind = record(
        PursuitStop::ReachEnvelope,
        AttemptOutcome::OutOfReach,
        StrikeClass::Unreadable,
        9.0, 9.0, 9.0, 6.0, 0.0,
    );
    blind.intent = None;
    let a = audit(vec![blind]);
    assert_eq!(a.recorded, 1);
    assert_eq!(a.no_initial_gap, 1);
    let attempts: u64 = a.by_gap_bin.iter().map(|b| b.attempts).sum();
    assert_eq!(attempts + a.no_initial_gap, a.recorded);
}

// ---------------------------------------------------------------- the merge

/// Two seeds' audits merge into one table with every new counter added, the bins still in
/// order, and the split still a partition.
#[test]
fn the_merge_adds_both_new_splits_without_losing_an_attempt() {
    let one = audit(vec![
        record(PursuitStop::ReachEnvelope, AttemptOutcome::Captured, StrikeClass::ResolvedInReach,
               2.0, 2.0, 1.0, 14.0, 6.0),
        record(PursuitStop::ReachEnvelope, AttemptOutcome::OutOfReach, StrikeClass::BeganOutOfReach,
               10.0, 10.0, 12.0, 6.0, 0.0),
    ]);
    let two = audit(vec![
        record(PursuitStop::ReachEnvelope, AttemptOutcome::Missed, StrikeClass::ResolvedInReach,
               6.0, 6.0, 3.0, 15.0, 5.0),
    ]);
    let mut merged = StrikeAudit::default();
    merged.add(&one);
    merged.add(&two);

    assert_eq!(merged.recorded, 3);
    assert_eq!(merged.held.count + merged.delivered.count + merged.no_strike_frame, 3);
    assert_eq!(merged.by_gap_bin.len(), GAP_BINS);
    let attempts: u64 = merged.by_gap_bin.iter().map(|b| b.attempts).sum();
    assert_eq!(attempts, 3);
    assert_eq!(merged.by_gap_bin[0].attempts, 1);
    assert_eq!(merged.by_gap_bin[1].attempts, 1);
    assert_eq!(merged.by_gap_bin[2].attempts, 1);
    for (i, bin) in merged.by_gap_bin.iter().enumerate() {
        assert_eq!(bin.bin as usize, i, "the bins stay in their fixed reporting order");
    }
    assert_eq!(merged.delivered.count, 2, "two of the three were past the claws");
    assert_eq!(merged.held.count, 1);
}

// ---------------------------------------------------------------- the CLI name

/// The arm's rule is named on the command line by a stated string, and an unknown one is
/// refused rather than silently defaulted to the shipped rule.
#[test]
fn the_pursuit_stop_is_named_by_a_stated_string_and_an_unknown_one_is_refused() {
    use cubarium_search::apex_audit::parse_pursuit_stop;
    assert_eq!(parse_pursuit_stop("half-space"), Ok(PursuitStop::ForwardHalfSpace));
    assert_eq!(parse_pursuit_stop("reach-envelope"), Ok(PursuitStop::ReachEnvelope));
    assert!(parse_pursuit_stop("envelope").is_err());
    assert!(parse_pursuit_stop("").is_err());
    let err = parse_pursuit_stop("in_contact").expect_err("refused");
    assert!(err.contains("half-space") && err.contains("reach-envelope"), "{err}");
}
