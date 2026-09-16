//! The movement measures, checked against their **definitions** on hand-built samples.
//!
//! These tests were written from the pre-registration in
//! `design/7_Research/ecology-v1-movement-2026-09-16.md` ("The measures") **before** the
//! implementation existed, and they are the reason the definitions in that note and the code
//! in `crates/cubarium-search/src/movement.rs` are the same definitions. Every expected number
//! below is derived by hand from the prose, not read off a run.
//!
//! Nothing here starts a world: a spatial measure is a function of a probe sequence, and a
//! crossing counter is a function of a field series, so both are testable without simulating
//! anything.

use cubarium_search::movement::{
    BodyTrack, CrossingCounter, DEPLETION_FRACTION, RECOVERY_FRACTION, Visit, diet_bin,
    residence_ticks, revisit_intervals, visits,
};

/// One probe interval, in ticks. Matches `evaluate::PROBE_EVERY`; passed explicitly so the
/// definition is checkable without the constant.
const P: u64 = 20;

/// The worked example the pre-registration's definitions are read against.
///
/// probe:   0       1       2       3      4       5
/// cell:    1       1       2       —      2       1
///
/// Visits, by "a maximal run of consecutive probes at which the body was observed in the same
/// cell", with a missing probe breaking the run:
///   cell 1 from probe 0, 2 probes
///   cell 2 from probe 2, 1 probe
///   cell 2 from probe 4, 1 probe   (probe 3 broke the earlier run)
///   cell 1 from probe 5, 1 probe   (truncated by the end of the window, still counted)
fn worked_example() -> Vec<Option<u16>> {
    vec![Some(1), Some(1), Some(2), None, Some(2), Some(1)]
}

#[test]
fn a_visit_is_a_maximal_run_of_consecutive_probes_in_one_cell() {
    assert_eq!(
        visits(&worked_example()),
        vec![
            Visit { cell: 1, start_probe: 0, probes: 2 },
            Visit { cell: 2, start_probe: 2, probes: 1 },
            Visit { cell: 2, start_probe: 4, probes: 1 },
            Visit { cell: 1, start_probe: 5, probes: 1 },
        ]
    );
}

#[test]
fn a_gap_in_the_probe_sequence_breaks_a_visit() {
    // Same cell either side of an unobserved probe is two visits, not one: the definition
    // says a probe at which the body was not observed breaks the run.
    let v = visits(&[Some(7), None, Some(7)]);
    assert_eq!(v.len(), 2, "a gap splits the run");
    assert_eq!(v[0], Visit { cell: 7, start_probe: 0, probes: 1 });
    assert_eq!(v[1], Visit { cell: 7, start_probe: 2, probes: 1 });
}

#[test]
fn residence_time_is_the_probe_count_times_the_probe_interval() {
    let v = visits(&worked_example());
    assert_eq!(residence_ticks(&v, P), vec![2 * P, P, P, P]);
    // The estimator is stated as the midpoint of `((n-1)·P, (n+1)·P)`; a single-probe visit is
    // therefore one probe interval and not zero.
    assert_eq!(residence_ticks(&visits(&[Some(3)]), P), vec![P]);
}

#[test]
fn the_last_visit_in_a_window_is_counted_although_it_is_truncated() {
    // The window ends while the body is still in cell 1; the definition counts that visit.
    let v = visits(&worked_example());
    assert_eq!(v.last().copied(), Some(Visit { cell: 1, start_probe: 5, probes: 1 }));
    assert_eq!(residence_ticks(&v, P).len(), 4, "four visits, four residence times");
}

#[test]
fn a_revisit_interval_is_measured_between_the_starts_of_successive_visits_to_one_cell() {
    // cell 1: visits start at probes 0 and 5 -> 5 probes = 100 ticks.
    // cell 2: visits start at probes 2 and 4 -> 2 probes =  40 ticks.
    // Ordered by cell, then by visit start.
    assert_eq!(revisit_intervals(&visits(&worked_example()), P), vec![5 * P, 2 * P]);
}

#[test]
fn consecutive_probes_in_one_cell_are_not_a_revisit() {
    // Ten probes standing still is one visit and no revisit at all: residence, not return.
    let still: Vec<Option<u16>> = (0..10).map(|_| Some(4u16)).collect();
    let v = visits(&still);
    assert_eq!(v, vec![Visit { cell: 4, start_probe: 0, probes: 10 }]);
    assert_eq!(residence_ticks(&v, P), vec![10 * P]);
    assert!(revisit_intervals(&v, P).is_empty(), "standing still is not revisiting");
}

#[test]
fn a_body_that_never_returns_contributes_no_revisit_interval() {
    let walk: Vec<Option<u16>> = (0..20u16).map(Some).collect();
    let v = visits(&walk);
    assert_eq!(v.len(), 20, "twenty cells, twenty one-probe visits");
    assert!(revisit_intervals(&v, P).is_empty(), "a body that never returns has no interval");
}

#[test]
fn three_visits_to_one_cell_give_two_successive_intervals() {
    // cell 1 at probes 0, 2, 6; cell 9 at probes 1, 3..5.
    let seq = vec![Some(1), Some(9), Some(1), Some(9), Some(9), Some(9), Some(1)];
    // cell 1 starts 0, 2, 6 -> intervals 2 and 4 probes.
    // cell 9 starts 1, 3    -> interval 2 probes.
    assert_eq!(revisit_intervals(&visits(&seq), P), vec![2 * P, 4 * P, 2 * P]);
}

#[test]
fn the_online_track_agrees_with_the_batch_decomposition() {
    // The recorder cannot keep 1,800 probes per body, so it accumulates online. The online
    // accumulator must produce exactly what the definition's batch decomposition produces.
    let seq = vec![
        Some(1), Some(1), Some(2), None, Some(2), Some(1), Some(1), Some(1), Some(3), Some(2),
    ];
    let mut track = BodyTrack::default();
    for (i, cell) in seq.iter().enumerate() {
        if let Some(c) = cell {
            track.observe(i as u64, *c);
        }
    }
    track.finish();

    let v = visits(&seq);
    let residence = residence_ticks(&v, P);
    let intervals = revisit_intervals(&v, P);
    let mean = |xs: &[u64]| xs.iter().sum::<u64>() as f64 / xs.len() as f64;

    assert_eq!(track.probes_seen(), seq.iter().filter(|c| c.is_some()).count() as u64);
    assert_eq!(track.visit_count(), v.len() as u64);
    assert_eq!(track.distinct_cells(), 3, "cells 1, 2 and 3");
    assert_eq!(track.mean_residence_ticks(P), Some(mean(&residence)));
    assert_eq!(track.mean_revisit_ticks(P), Some(mean(&intervals)));
}

#[test]
fn a_track_with_no_return_reports_no_mean_revisit_interval() {
    let mut track = BodyTrack::default();
    for i in 0..5u64 {
        track.observe(i, i as u16);
    }
    track.finish();
    assert_eq!(track.mean_revisit_ticks(P), None, "no interval, not a zero");
    assert_eq!(track.mean_residence_ticks(P), Some(P as f64));
}

#[test]
fn the_crossing_counter_applies_the_hysteresis_per_cell() {
    // Cell 0 opens at 1.0 m, cell 1 opens bare, cell 2 opens at 2.0 m.
    let mut c = CrossingCounter::new(&[1.0, 0.0, 2.0], DEPLETION_FRACTION, RECOVERY_FRACTION);
    assert_eq!(c.watched(), 2, "a cell that opened bare is not watched");

    c.observe(&[1.0, 0.0, 2.0]); // nothing
    c.observe(&[0.2, 0.0, 2.0]); // cell 0 falls below 0.25 -> depletion 1
    c.observe(&[0.1, 0.0, 2.0]); // already depleted: no second crossing
    c.observe(&[0.6, 0.0, 2.0]); // cell 0 rises above 0.50 -> recovery 1, cycle 1
    c.observe(&[0.4, 0.0, 2.0]); // inside the hysteresis band: nothing
    c.observe(&[0.1, 0.0, 0.4]); // cell 0 depletes again; cell 2 falls below 0.5 -> depletion

    assert_eq!(c.depletions(), 3);
    assert_eq!(c.recoveries(), 1);
    assert_eq!(c.cells_depleted(), 2);
    assert_eq!(c.cells_recovered(), 1);
    assert_eq!(c.cells_cycled(), 1, "one cell completed a depletion followed by a recovery");
    assert_eq!(c.max_cycles(), 1);
    assert_eq!(c.depleted_now(), 2);
}

#[test]
fn the_depletion_threshold_is_strict() {
    let mut c = CrossingCounter::new(&[1.0], DEPLETION_FRACTION, RECOVERY_FRACTION);
    c.observe(&[0.25]); // exactly at the threshold is not below it
    assert_eq!(c.depletions(), 0);
    c.observe(&[0.2499]);
    assert_eq!(c.depletions(), 1);
    c.observe(&[0.5]); // exactly at the recovery threshold is not above it
    assert_eq!(c.recoveries(), 0);
    c.observe(&[0.5001]);
    assert_eq!(c.recoveries(), 1);
}

#[test]
fn a_cell_can_complete_several_cycles_and_they_are_counted_per_cell() {
    let mut c = CrossingCounter::new(&[1.0, 1.0], DEPLETION_FRACTION, RECOVERY_FRACTION);
    for _ in 0..3 {
        c.observe(&[0.1, 1.0]);
        c.observe(&[0.9, 1.0]);
    }
    assert_eq!(c.depletions(), 3);
    assert_eq!(c.recoveries(), 3);
    assert_eq!(c.cells_depleted(), 1);
    assert_eq!(c.cells_cycled(), 1);
    assert_eq!(c.max_cycles(), 3, "the same cell cycled three times");
    assert_eq!(c.depleted_now(), 0);
}

#[test]
fn an_unwatched_cell_never_crosses_however_it_moves() {
    let mut c = CrossingCounter::new(&[0.0], DEPLETION_FRACTION, RECOVERY_FRACTION);
    c.observe(&[0.0]);
    c.observe(&[5.0]);
    c.observe(&[0.0]);
    assert_eq!(c.watched(), 0);
    assert_eq!(c.depletions(), 0);
    assert_eq!(c.recoveries(), 0);
}

#[test]
fn the_diet_bins_split_the_shipped_founder_roster_as_the_pre_registration_says() {
    // burrower 0.10 -> bin 0, skimmer 0.60 -> bin 1, grazer 0.85 and glider 0.90 -> bin 2
    // (`crates/cubarium-core/src/config.rs:490-519`).
    assert_eq!(diet_bin(0.10), 0);
    assert_eq!(diet_bin(0.60), 1);
    assert_eq!(diet_bin(0.85), 2);
    assert_eq!(diet_bin(0.90), 2);
    // The edges belong to the bin above, and the ends are inside.
    assert_eq!(diet_bin(0.0), 0);
    assert_eq!(diet_bin(0.35), 1);
    assert_eq!(diet_bin(0.65), 2);
    assert_eq!(diet_bin(1.0), 2);
}
