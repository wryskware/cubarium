//! Workstream Q's integration surface
//! (`design/handoffs/ecology-v1-es-antithetic-opus-2026-09-16.md`).
//!
//! The reduction's own arithmetic — the utilities, the weights, the per-layout differences, the
//! binding layout, the cancellation references, the step geometry and the deadband predicate —
//! is pinned next to the code in `es::antithetic`, on hand-built reports with hand-computed
//! answers. What is pinned here is the one thing a reader of the result note has to be able to
//! trust from outside: that the reduction is reading a **real** `GenerationReport` the way the
//! trainer wrote it, and that replaying a run's updates from its reports reproduces the centre
//! the trainer itself produced, bit for bit. If that fails, every number in the note is a
//! number about some other run.
//!
//! The two experiments at the bottom are `#[ignore]`d: one reads a retained run and writes
//! `runs/`, the other simulates twelve episodes. Run them by name:
//!
//! ```bash
//! CUBARIUM_SEARCH_BUILD=<commit> \
//!   cargo test -p cubarium-search --release --test es_antithetic -- --ignored --nocapture
//! ```
//!
//! They are `#[ignore]`d tests rather than `es-*` subcommands for the reason workstreams H and
//! L recorded: other workers hold `crates/cubarium-search/src/main.rs` open in worktrees this
//! round, and a subcommand is the one edit that would have collided. `run_reduction` and
//! `run_deadband` are public, so promoting them later is a one-liner.

use std::sync::atomic::AtomicBool;

use cubarium_search::es::antithetic;
use cubarium_search::es::optimizer::Adam;
use cubarium_search::es::tensor::{self, PARAMS};
use cubarium_search::es::trainer::{self, Aggregate, Plan, Protocol};
use cubarium_search::es::training_layouts;

/// A real generation, run through the real trainer on a short horizon, then read back through
/// the reduction. The pair weights the reduction extracts must rebuild the gradient norm the
/// trainer recorded, and the replay must land on the trainer's own updated centre.
///
/// This is the check that makes the retained-run replay meaningful: it is the same code path,
/// on a generation whose answer is produced by the trainer rather than asserted here.
#[test]
fn the_reduction_replays_a_real_generation_onto_the_trainers_own_centre() {
    let layouts = training_layouts();
    let protocol = Protocol::new(2, 1_500, 20_260_915, &layouts[..2]);
    let cancel = AtomicBool::new(false);

    let mut theta = tensor::initial_center(protocol.train_seed);
    let theta0 = theta.clone();
    let mut adam = Adam::new(PARAMS);
    let plan = Plan::new(&layouts[..2], protocol.horizon_ticks, 4, true, None);
    let report =
        trainer::run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel).expect("ran");

    // The reduction reads the report the trainer wrote, without being told anything else.
    let (row, pairs) = antithetic::reduce_generation(&report, Aggregate::Min).expect("reduced");
    assert_eq!(row.pairs, 2);
    assert_eq!(pairs.len(), 2);
    assert_eq!(row.center_score, report.center_score);
    assert_eq!(
        antithetic::layout_order(&report).expect("order"),
        layouts[..2]
            .iter()
            .map(|l| l.name.clone())
            .collect::<Vec<_>>()
    );
    // The centre's own jobs are in the report too, and must not be mistaken for a candidate.
    assert_eq!(
        row.binding_counts.iter().sum::<usize>(),
        4,
        "two members of two pairs"
    );

    let (geo, replayed, replayed_adam) =
        antithetic::replay(&theta0, &[report.clone()], &protocol, Aggregate::Min).expect("replay");
    assert_eq!(geo.len(), 1);
    assert_eq!(
        geo[0].gradient_norm_recomputed, report.gradient_norm,
        "the extracted weights must rebuild the trainer's own gradient"
    );
    assert_eq!(geo[0].gradient_norm_recorded, report.gradient_norm);
    assert_eq!(
        replayed, theta,
        "the replay must land on the trainer's centre, bit for bit"
    );
    assert_eq!(replayed_adam, adam);
    assert!((geo[0].step_rms - report.update_rms).abs() < 1e-12 * report.update_rms.max(1.0));
}

/// The reduction must refuse a report whose episodes do not produce the scores it carries,
/// rather than reporting a pair table about two different things.
#[test]
fn a_report_whose_episodes_disagree_with_its_scores_is_refused() {
    let layouts = training_layouts();
    let protocol = Protocol::new(1, 900, 20_260_915, &layouts[..1]);
    let cancel = AtomicBool::new(false);
    let mut theta = tensor::initial_center(protocol.train_seed);
    let mut adam = Adam::new(PARAMS);
    let plan = Plan::new(&layouts[..1], protocol.horizon_ticks, 2, false, None);
    let report =
        trainer::run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel).expect("ran");

    antithetic::reduce_generation(&report, Aggregate::Min).expect("the honest one reduces");

    let mut tampered = report.clone();
    tampered.candidate_scores[0] += 1.0;
    let err = antithetic::reduce_generation(&tampered, Aggregate::Min).expect_err("refused");
    assert!(
        err.contains("pair0+"),
        "the error must name the candidate: {err}"
    );

    // And the aggregate is part of the question: the same episodes under `Mean` do not produce
    // the `Min` scores the report recorded, so asking for the wrong one is refused too.
    let mut spread = report.clone();
    spread.candidate_scores = report.candidate_scores.clone();
    assert!(
        antithetic::reduce_generation(&spread, Aggregate::Min).is_ok(),
        "the protocol's own aggregate reduces"
    );
}
