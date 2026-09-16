//! Workstream M's counter split and window budget, checked on hand-built sequences.
//!
//! Every definition written into `design/7_Research/ecology-v1-plant-budget-2026-09-16.md`
//! before the campaign ran is checked here against a sequence built by hand, exactly as
//! workstream I's `ladder_measures.rs` checks its own. Nothing here steps a world: the
//! tracker is arithmetic over a probe sequence, which is what makes it testable at all.
//!
//! The definitions under test:
//!
//! - a crossing is **with exact withdrawal** iff the cell's cumulative exact withdrawal has
//!   risen by more than `EPSILON` since the cell last recovered — or since the record opened,
//!   for a cell that has never recovered;
//! - the **window budget** is `foliage_in − foliage_out` over the ticks from the newest
//!   600-tick boundary at or before `tick − 6,000` to the crossing tick, with the window's
//!   actual length reported beside it;
//! - the window's **effective light and nutrient** are means over the ticks the cell was
//!   *alive* inside the window, and are absent — not zero — when it was alive for none of it;
//! - I's probe-based "ever visited" is carried unchanged beside the new split.

use cubarium_search::plant_budget::{
    BUDGET_WINDOW_TICKS, CellSample, CrossingClass, MAX_CROSSINGS, PlantBudgetTracker,
    WINDOW_EVERY, WINDOW_SLOTS,
};

const CELLS: usize = 4;

fn p_ref() -> Vec<f64> {
    vec![1.0, 2.0, 0.5, 0.0]
}

fn l_mu() -> Vec<f64> {
    vec![0.4, 0.3, 0.2, 0.1]
}

fn tracker() -> PlantBudgetTracker {
    PlantBudgetTracker::new(&p_ref(), &l_mu(), 0)
}

/// A cumulative sample: every field is a running total from the tick the record opened.
fn sample(foliage_in: f64, foliage_out: f64, withdrawal: f64, ticks_alive: u64) -> CellSample {
    CellSample {
        foliage_in,
        foliage_out,
        withdrawal,
        income: 2.0 * foliage_in,
        maintenance_unpaid: 0.25 * foliage_out,
        death_foliage: 0.0,
        wood_sum: 3.0 * ticks_alive as f64,
        light_sum: 0.5 * ticks_alive as f64,
        nutrient_sum: 0.4 * ticks_alive as f64,
        ticks_alive,
    }
}

fn flat(s: CellSample) -> Vec<CellSample> {
    vec![s; CELLS]
}

// --- the split ---------------------------------------------------------------------------

/// A crossing in a cell no mouth has ever touched is `WithoutWithdrawal`, whatever the probe
/// saw. This is the case I read as over-seeding and Astra would not accept from sampled
/// visits: here it is the exact counter that says so.
#[test]
fn a_crossing_with_no_exact_withdrawal_at_all_is_without_withdrawal() {
    let mut t = tracker();
    t.observe(0, &flat(sample(0.0, 0.0, 0.0, 0)), &[]);
    t.observe(20, &flat(sample(0.1, 0.3, 0.0, 20)), &[]);
    t.depleted(1, 20, 0.4);
    let summary = t.finish(&flat(sample(0.1, 0.3, 0.0, 20)), &[0.4; CELLS], 0.0, 20);

    assert_eq!(summary.total_crossings, 1);
    assert_eq!(summary.without_withdrawal, 1);
    assert_eq!(summary.with_withdrawal, 0);
    assert_eq!(summary.fraction_with_withdrawal, 0.0);
    let row = summary.crossings[0];
    assert_eq!(row.class, Some(CrossingClass::WithoutWithdrawal));
    assert_eq!(row.withdrawal_since_recovery, 0.0);
    assert!(!row.ever_visited);
}

/// One metre of exact withdrawal before the crossing puts it on the other side of the split,
/// even when no probe ever saw a body in the cell — which is precisely the case a sampled
/// visit misses and the reason the split was rebuilt.
#[test]
fn any_exact_withdrawal_since_the_last_recovery_makes_the_crossing_with_withdrawal() {
    let mut t = tracker();
    t.observe(0, &flat(sample(0.0, 0.0, 0.0, 0)), &[]);
    t.observe(20, &flat(sample(0.1, 0.1, 0.25, 20)), &[]);
    t.depleted(2, 20, 0.1);
    let summary = t.finish(&flat(sample(0.1, 0.1, 0.25, 20)), &[0.1; CELLS], 0.0, 20);

    let row = summary.crossings[0];
    assert_eq!(row.class, Some(CrossingClass::WithWithdrawal));
    assert_eq!(row.withdrawal_since_recovery, 0.25);
    assert_eq!(row.withdrawal_cumulative, 0.25);
    assert!(
        !row.ever_visited,
        "the fixture's whole point is that the probe saw nothing and the counter did"
    );
    assert_eq!(summary.never_visited, 1, "I's split is carried unchanged beside the new one");
}

/// "Since the last recovery" means since the last recovery. Withdrawal taken before a recovery
/// does not make the *next* crossing a grazed one, and withdrawal after it does.
#[test]
fn the_withdrawal_clock_restarts_at_every_recovery() {
    let mut t = tracker();
    t.observe(0, &flat(sample(0.0, 0.0, 0.0, 0)), &[]);

    // First crossing: 0.4 m withdrawn.
    t.observe(20, &flat(sample(0.0, 0.0, 0.4, 20)), &[]);
    t.depleted(0, 20, 0.2);

    // Recovery at 40, with no further withdrawal.
    t.observe(40, &flat(sample(0.2, 0.0, 0.4, 40)), &[]);
    t.recovered(0, 40);

    // Second crossing at 60: the cumulative withdrawal has not moved since the recovery.
    t.observe(60, &flat(sample(0.2, 0.3, 0.4, 60)), &[]);
    t.depleted(0, 60, 0.2);

    // A third: 0.1 m taken after the recovery.
    t.observe(80, &flat(sample(0.2, 0.3, 0.5, 80)), &[]);
    t.depleted(0, 80, 0.2);

    let last = flat(sample(0.2, 0.3, 0.5, 80));
    let summary = t.finish(&last, &[0.2; CELLS], 0.0, 80);
    let rows = &summary.crossings;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].class, Some(CrossingClass::WithWithdrawal));
    assert_eq!(rows[0].index, 0);
    assert_eq!(rows[1].class, Some(CrossingClass::WithoutWithdrawal));
    assert_eq!(rows[1].index, 1);
    assert_eq!(rows[1].withdrawal_since_recovery, 0.0);
    assert_eq!(rows[1].withdrawal_cumulative, 0.4, "the cumulative counter never restarts");
    assert_eq!(rows[2].class, Some(CrossingClass::WithWithdrawal));
    assert!((rows[2].withdrawal_since_recovery - 0.1).abs() < 1e-12);
    assert_eq!(summary.with_withdrawal, 2);
    assert_eq!(summary.without_withdrawal, 1);
    assert_eq!(summary.fraction_with_withdrawal, 2.0 / 3.0);
    assert_eq!(summary.cells[0].crossings, 3);
    assert_eq!(summary.cells[0].recoveries, 1);
    assert_eq!(summary.crossing_cells, vec![0]);
}

/// A withdrawal below the epsilon is not a withdrawal; the epsilon is the same one the watched
/// cell rule uses, so a rounding crumb cannot reclassify a crossing.
#[test]
fn a_withdrawal_below_the_epsilon_does_not_reclassify_a_crossing() {
    let mut t = tracker();
    t.observe(0, &flat(sample(0.0, 0.0, 0.0, 0)), &[]);
    t.observe(20, &flat(sample(0.0, 0.1, 1e-12, 20)), &[]);
    t.depleted(0, 20, 0.2);
    let summary = t.finish(&flat(sample(0.0, 0.1, 1e-12, 20)), &[0.2; CELLS], 0.0, 20);
    assert_eq!(summary.crossings[0].class, Some(CrossingClass::WithoutWithdrawal));
}

// --- the window budget --------------------------------------------------------------------

/// The window is the 6,000 ticks before the crossing, measured between 600-tick boundaries,
/// and `budget_net` is `in − out` over it with no consumer term. The withdrawal over the same
/// window is reported beside it, never folded in.
#[test]
fn the_window_budget_is_in_minus_out_over_the_six_thousand_ticks_before_the_crossing() {
    let mut t = tracker();
    // Twenty boundaries: cumulative income rises by 1.0 per boundary, loss by 1.5, withdrawal
    // by 0.25, and the cell is alive for every tick.
    for k in 0..=20u64 {
        let tick = k * WINDOW_EVERY;
        let s = sample(k as f64, 1.5 * k as f64, 0.25 * k as f64, tick);
        t.observe(tick, &flat(s), &[]);
    }
    let tick = 20 * WINDOW_EVERY;
    t.depleted(0, tick, 0.2);
    let last = sample(20.0, 30.0, 5.0, tick);
    let summary = t.finish(&flat(last), &[0.2; CELLS], 0.0, tick);

    let row = summary.crossings[0];
    assert_eq!(row.budget_window_ticks, BUDGET_WINDOW_TICKS);
    // Ten boundaries of income and loss.
    assert!((row.budget_in - 10.0).abs() < 1e-12, "in {}", row.budget_in);
    assert!((row.budget_out - 15.0).abs() < 1e-12, "out {}", row.budget_out);
    assert!((row.budget_net + 5.0).abs() < 1e-12, "net {}", row.budget_net);
    assert!((row.withdrawal_window - 2.5).abs() < 1e-12);
    assert!(row.budget_net < 0.0, "this cell cannot hold what it carries");
    assert_eq!(row.window_ticks_alive, BUDGET_WINDOW_TICKS);
    assert_eq!(row.light_effective, Some(0.5));
    assert_eq!(row.nutrient, Some(0.4));
    assert_eq!(row.wood, Some(3.0));
    // The "why" terms are differenced over the same window as the budget.
    assert!((row.income - 20.0).abs() < 1e-12, "income {}", row.income);
    assert!((row.maintenance_unpaid - 3.75).abs() < 1e-12, "unpaid {}", row.maintenance_unpaid);
    assert_eq!(row.death_foliage, 0.0);
    // This fixture's cell *was* grazed, so a negative budget is not by itself Astra's
    // confirmation case: the two counts are kept apart.
    assert_eq!(row.class, Some(CrossingClass::WithWithdrawal));
    assert!(!row.is_plant_budget_failure());
    assert_eq!(summary.with_withdrawal_negative_budget, 1);
    assert_eq!(summary.without_withdrawal_negative_budget, 0);
}

/// Astra's confirmation case, isolated: a negative window budget with no exact withdrawal at
/// all. This is the only combination that confirms the over-seeding reading for a crossing.
#[test]
fn a_negative_window_budget_with_no_withdrawal_is_the_confirmation_case() {
    let mut t = tracker();
    for k in 0..=12u64 {
        let tick = k * WINDOW_EVERY;
        t.observe(tick, &flat(sample(k as f64, 1.5 * k as f64, 0.0, tick)), &[]);
    }
    let tick = 12 * WINDOW_EVERY;
    t.depleted(0, tick, 0.2);
    let last = sample(12.0, 18.0, 0.0, tick);
    let summary = t.finish(&flat(last), &[0.2; CELLS], 0.0, tick);
    let row = summary.crossings[0];
    assert_eq!(row.class, Some(CrossingClass::WithoutWithdrawal));
    assert_eq!(row.withdrawal_window, 0.0);
    assert!((row.budget_net + 5.0).abs() < 1e-12);
    assert!(row.is_plant_budget_failure());
    assert_eq!(summary.without_withdrawal_negative_budget, 1);
    assert_eq!(summary.with_withdrawal_negative_budget, 0);
}

/// Near the start of a run there is no boundary 6,000 ticks back, so the window is the whole
/// record so far and says so rather than pretending to be 6,000 ticks long.
#[test]
fn a_crossing_before_the_window_has_elapsed_reports_the_shorter_window_it_actually_measured() {
    let mut t = tracker();
    for k in 0..=3u64 {
        let tick = k * WINDOW_EVERY;
        t.observe(tick, &flat(sample(k as f64, 0.0, 0.0, tick)), &[]);
    }
    t.depleted(0, 1_800, 0.2);
    let summary = t.finish(&flat(sample(3.0, 0.0, 0.0, 1_800)), &[0.2; CELLS], 0.0, 1_800);
    let row = summary.crossings[0];
    assert_eq!(row.budget_window_ticks, 1_800);
    assert!((row.budget_in - 3.0).abs() < 1e-12);
    assert!(row.budget_net > 0.0);
    assert!(!row.is_plant_budget_failure(), "a positive budget is not a plant-budget failure");
}

/// The ring keeps exactly the slots the window needs: the eleventh boundary back is still
/// there and the twelfth is gone, so a window can never silently grow.
#[test]
fn the_ring_holds_exactly_the_window_and_a_crossing_between_boundaries_reports_its_own_length() {
    assert_eq!(WINDOW_SLOTS, 11);
    let mut t = tracker();
    for k in 0..=15u64 {
        let tick = k * WINDOW_EVERY;
        t.observe(tick, &flat(sample(k as f64, 0.0, 0.0, tick)), &[]);
    }
    // A crossing 200 ticks past a boundary: the window starts at the newest boundary at or
    // before `tick − 6,000`, so it is 6,200 ticks long and the row says so.
    let tick = 15 * WINDOW_EVERY + 200;
    t.observe(tick, &flat(sample(15.5, 0.0, 0.0, tick)), &[]);
    t.depleted(0, tick, 0.2);
    let summary = t.finish(&flat(sample(15.5, 0.0, 0.0, tick)), &[0.2; CELLS], 0.0, tick);
    let row = summary.crossings[0];
    assert_eq!(row.budget_window_ticks, BUDGET_WINDOW_TICKS + 200);
    assert!((row.budget_in - 10.5).abs() < 1e-12);
}

/// A cell that was alive for none of the window has no light or nutrient the plant step read,
/// and the row says *absent* rather than zero — a zero would read as darkness.
#[test]
fn a_cell_alive_for_none_of_the_window_reports_no_light_rather_than_zero_light() {
    let mut t = tracker();
    for k in 0..=12u64 {
        let tick = k * WINDOW_EVERY;
        t.observe(tick, &flat(sample(0.0, 0.0, 0.0, 0)), &[]);
    }
    let tick = 12 * WINDOW_EVERY;
    t.depleted(3, tick, 0.0);
    let summary = t.finish(&flat(sample(0.0, 0.0, 0.0, 0)), &[0.0; CELLS], 0.0, tick);
    let row = summary.crossings[0];
    assert_eq!(row.window_ticks_alive, 0);
    assert_eq!(row.light_effective, None);
    assert_eq!(row.nutrient, None);
    assert_eq!(row.wood, None);
    assert_eq!(summary.cells[3].light_effective, None);
    assert_eq!(summary.cells[3].wood, None);
}

// --- the visited flag, the cap, and the all-cell summary ---------------------------------

/// The probe-based flag is I's, unchanged: a body observed in the cell at any probe before the
/// crossing sets it, and it is reported beside the exact split, never instead of it.
#[test]
fn a_body_observed_at_a_probe_sets_the_visited_flag_the_old_split_used() {
    let mut t = tracker();
    t.observe(0, &flat(sample(0.0, 0.0, 0.0, 0)), &[]);
    t.observe(20, &flat(sample(0.0, 0.1, 0.0, 20)), &[1]);
    t.observe(40, &flat(sample(0.0, 0.2, 0.0, 40)), &[]);
    t.depleted(1, 40, 0.2);
    t.depleted(0, 40, 0.2);
    let summary = t.finish(&flat(sample(0.0, 0.2, 0.0, 40)), &[0.2; CELLS], 0.0, 40);
    let visited: Vec<bool> = summary.crossings.iter().map(|r| r.ever_visited).collect();
    assert_eq!(visited, vec![true, false]);
    assert_eq!(summary.ever_visited, 1);
    assert_eq!(summary.never_visited, 1);
    // Both are `WithoutWithdrawal`: the two splits disagree, which is the comparison the note
    // reports rather than assumes.
    assert_eq!(summary.without_withdrawal, 2);
}

/// The cap is a storage guard and is never silently zero.
#[test]
fn crossings_past_the_cap_are_dropped_and_counted() {
    let mut t = tracker();
    t.observe(0, &flat(sample(0.0, 0.0, 0.0, 0)), &[]);
    for _ in 0..(MAX_CROSSINGS + 7) {
        t.depleted(0, 0, 0.1);
    }
    let summary = t.finish(&flat(sample(0.0, 0.0, 0.0, 0)), &[0.1; CELLS], 0.0, 0);
    assert_eq!(summary.crossings.len(), MAX_CROSSINGS);
    assert_eq!(summary.crossings_dropped, 7);
    assert_eq!(summary.total_crossings, (MAX_CROSSINGS + 7) as u64);
}

/// The coarse all-cell summary covers every cell, crossing or not, so a cell that crossed in
/// one arm can be looked up in the other — which is what the overlap number is made of.
#[test]
fn the_all_cell_summary_covers_every_cell_and_the_overlap_is_the_shared_crossing_cells() {
    let mut a = tracker();
    a.observe(0, &flat(sample(0.0, 0.0, 0.0, 0)), &[]);
    a.observe(600, &flat(sample(1.0, 2.0, 0.0, 600)), &[]);
    a.depleted(0, 600, 0.2);
    a.depleted(2, 600, 0.1);
    let a = a.finish(&flat(sample(1.0, 2.0, 0.0, 600)), &[0.2; CELLS], 1.5e-13, 600);

    let mut b = tracker();
    b.observe(0, &flat(sample(0.0, 0.0, 0.0, 0)), &[]);
    b.observe(600, &flat(sample(1.0, 2.0, 0.0, 600)), &[]);
    b.depleted(2, 600, 0.1);
    b.depleted(3, 600, 0.0);
    let b = b.finish(&flat(sample(1.0, 2.0, 0.0, 600)), &[0.2; CELLS], 0.0, 600);

    assert_eq!(a.cells.len(), CELLS);
    assert_eq!(a.cells.iter().map(|c| c.cell).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    assert_eq!(a.cells[0].p_open, 1.0);
    assert_eq!(a.cells[1].l_mu, 0.3);
    assert!((a.cells[1].budget_net + 1.0).abs() < 1e-12, "whole-run net is in − out");
    assert_eq!(a.cells[1].crossings, 0, "a cell that never crossed is still in the summary");
    assert!((a.cells[1].income - 2.0).abs() < 1e-12, "whole-run income rides with the cell");
    assert_eq!(a.cells[1].wood, Some(3.0));
    assert_eq!(a.max_identity_residual, 1.5e-13, "the core's residual rides with the row");

    assert_eq!(a.crossing_cells, vec![0, 2]);
    assert_eq!(b.crossing_cells, vec![2, 3]);
    assert_eq!(a.overlap(&b), vec![2]);
    assert_eq!(b.overlap(&a), vec![2]);
}
