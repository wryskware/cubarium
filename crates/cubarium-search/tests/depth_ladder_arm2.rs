//! Workstream XY2, Part 2 — the three arguments the arm-2 ladder needed, checked from the
//! definitions in `design/7_Research/ecology-v1-round5-followups-2026-09-16.md`'s
//! pre-registration.
//!
//! The campaign itself is workstream Y's, unchanged, and is checked by `tests/depth_ladder.rs`;
//! nothing here re-checks a rung, a clause or a measure. What is new is the plumbing that lets
//! Y's ladder run **at another arm** and **under another pursuit predicate**, and the one
//! property that makes the whole design honest: the reproduction run and the ladder run must
//! differ in exactly the predicate, and at an arm with no apex the predicate must not be able
//! to change anything at all.

use cubarium_core::hunter::PursuitStop;
use cubarium_search::census::{
    self, DEPTH_CONTROL, DEPTH_LEVELS, DEPTH_TREATMENT, parse_levels, parse_pursuit_stop,
};
use cubarium_search::evaluate::Protocol;

/// A short horizon: these tests are about the plumbing, not the ecology. The apex is
/// introduced early enough that an arm-2 world actually carries predators for most of it.
fn short(arm: u32) -> Protocol {
    Protocol { horizon_ticks: 2_000, sample_every: 100, apex_founders: arm, apex_introduce_tick: 200 }
}

// ---------------------------------------------------------------------------------------
// The design: one arm, the rungs asked for, every cell once
// ---------------------------------------------------------------------------------------

#[test]
fn the_plan_runs_the_arm_it_is_given_and_only_the_rungs_it_is_given() {
    let seeds = [1001u64, 1002, 1003, 1004, 1005, 1006];

    let ladder = census::plan(&seeds, 2, &DEPTH_LEVELS);
    assert_eq!(ladder.len(), 72, "6 rungs x 2 configurations x 6 seeds x 1 arm");
    assert!(ladder.iter().all(|j| j.arm == 2), "arm 2 only");

    let repro = census::plan(&seeds, 2, &[DEPTH_CONTROL, DEPTH_TREATMENT]);
    assert_eq!(repro.len(), 24, "2 levels x 2 configurations x 6 seeds x 1 arm");
    assert!(repro.iter().all(|j| j.arm == 2));
    assert!(
        repro
            .iter()
            .all(|j| j.depth == DEPTH_CONTROL || j.depth == DEPTH_TREATMENT),
        "only the two levels R ran"
    );

    // The reproduction run's cells are a subset of the ladder's, which is what makes the two
    // runs comparable cell by cell.
    for job in &repro {
        assert!(
            ladder.iter().any(|l| l.candidate == job.candidate
                && l.seed == job.seed
                && l.arm == job.arm
                && l.depth == job.depth),
            "{job:?} is a cell of the ladder too"
        );
    }

    for jobs in [&ladder, &repro] {
        let mut keys: Vec<(String, u32, u64, u32)> = jobs
            .iter()
            .map(|j| (j.candidate.to_string(), j.depth.to_bits(), j.seed, j.arm))
            .collect();
        keys.sort();
        let unique = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), unique, "every cell appears exactly once");
    }
}

#[test]
fn the_levels_list_is_the_ladders_own_rungs_in_the_ladders_own_order() {
    assert_eq!(parse_levels("0.55,0.10").expect("two rungs"), vec![DEPTH_CONTROL, DEPTH_TREATMENT]);
    assert_eq!(parse_levels("0.10, 0.55 ,0.10").expect("a repeat is one rung"), vec![0.10, 0.55]);
    assert_eq!(parse_levels(&DEPTH_LEVELS.map(|d| d.to_string()).join(",")).expect("all six"), DEPTH_LEVELS.to_vec());

    // A rung off the ladder is refused rather than added: a seventh height is a different
    // campaign, not a wider one.
    assert!(parse_levels("0.45").is_err(), "0.45 is not a rung");
    assert!(parse_levels("1.00").is_err(), "the glider's own value is deliberately not a rung");
    assert!(parse_levels("").is_err(), "naming no rung is an error, not the whole ladder");
    assert!(parse_levels("nope").is_err());
}

#[test]
fn the_pursuit_flag_defaults_to_the_shipped_rule_and_names_the_other_one() {
    assert_eq!(parse_pursuit_stop("reach-envelope").expect("shipped"), PursuitStop::ReachEnvelope);
    assert_eq!(parse_pursuit_stop("half-space").expect("R's rule"), PursuitStop::ForwardHalfSpace);
    assert_eq!(
        parse_pursuit_stop("reach-envelope").expect("shipped"),
        PursuitStop::default(),
        "the flag's default name is the core's own default, not a second opinion about it"
    );
    assert!(parse_pursuit_stop("forward-half-space-ish").is_err());

    // The names written into every row are the core's, so a row cannot disagree with the
    // rule it ran under.
    assert_eq!(PursuitStop::ReachEnvelope.as_str(), "reach_envelope");
    assert_eq!(PursuitStop::ForwardHalfSpace.as_str(), "forward_half_space");
}

// ---------------------------------------------------------------------------------------
// The predicate: unreachable without an apex, and the one difference with one
// ---------------------------------------------------------------------------------------

/// The claim Y's arm-0 reproduction rested on, made a test rather than an inference: with no
/// predator in the world the pursuit rule cannot reach anything, so the two rules produce the
/// **same world, hash for hash**. This is why Y's 24 rows reproduced R's across V's adoption.
#[test]
fn without_an_apex_the_two_pursuit_rules_are_the_same_world() {
    let protocol = short(0);
    let envelope =
        census::run_one("fast-leaf", 1001, 0, DEPTH_CONTROL, protocol, PursuitStop::ReachEnvelope)
            .expect("the shipped rule");
    let half =
        census::run_one("fast-leaf", 1001, 0, DEPTH_CONTROL, protocol, PursuitStop::ForwardHalfSpace)
            .expect("R's rule");
    assert_eq!(
        envelope.final_state_hash, half.final_state_hash,
        "arm 0 has no predator, so the pursuit rule is unreachable and cannot move a bit"
    );
    assert_eq!(envelope.prey_deaths, half.prey_deaths);
    assert_eq!(envelope.deaths_by_cause, half.deaths_by_cause);
    assert_eq!(envelope.pursuit_stop, "reach_envelope", "the row records the rule it ran");
    assert_eq!(half.pursuit_stop, "forward_half_space");
}

/// The other half of the same claim: a run is deterministic in the rule it is given, so a
/// difference between the two arm-2 runs is the rule and not the weather.
#[test]
fn a_run_is_deterministic_in_the_rule_it_is_given() {
    let protocol = short(2);
    for stop in [PursuitStop::ReachEnvelope, PursuitStop::ForwardHalfSpace] {
        let a = census::run_one("fast-leaf", 1001, 2, DEPTH_CONTROL, protocol, stop)
            .expect("an arm-2 run");
        let b = census::run_one("fast-leaf", 1001, 2, DEPTH_CONTROL, protocol, stop)
            .expect("the same arm-2 run");
        assert_eq!(a.final_state_hash, b.final_state_hash, "{}: deterministic", stop.as_str());
        assert_eq!(a.pursuit_stop, stop.as_str());
        assert_eq!(a.protocol.apex_founders, 2, "the arm reached the protocol");
    }
}

/// The census rows already carry a **cross-tabulation of deaths by cause and form** —
/// `movement::Census`'s cells, over every body and not only the founders — which is what
/// Astra's predation-by-prey-form question is answered out of. No new world state and no
/// rerun is added for it, and this test is what says so.
#[test]
fn the_rows_carry_predation_deaths_by_prey_form() {
    let protocol = short(2);
    let row = census::run_one("fast-leaf", 1001, 2, DEPTH_CONTROL, protocol, PursuitStop::ReachEnvelope)
        .expect("an arm-2 run");

    // Every cell carries all four causes in `DEATH_CAUSES` order, keyed by form.
    let by_form_predation: u64 =
        row.census.cells.iter().map(|c| c.cell.deaths_by_cause[3]).sum();
    let by_form_all: u64 = row
        .census
        .cells
        .iter()
        .map(|c| c.cell.deaths_by_cause.iter().sum::<u64>())
        .sum();

    // The cells' own totals must agree with the run's whole-world counters, or the cross-tab
    // would be a second and different count rather than a split of the same one. The
    // whole-world counters include the apex, which the census deliberately excludes, so the
    // cells' total is the prey total.
    assert_eq!(by_form_all, row.prey_deaths, "the cells split the prey deaths, they do not re-count them");
    assert!(
        by_form_predation <= row.deaths_by_cause[3],
        "prey lost to predation cannot exceed every body lost to predation"
    );
}
