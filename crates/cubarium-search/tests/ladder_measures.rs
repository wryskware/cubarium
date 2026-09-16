//! Workstream I's definitions, checked on hand-built samples.
//!
//! Every test here is written from the pre-registration in
//! `design/7_Research/ecology-v1-ladder-2026-09-16.md` and was committed **before** the module
//! it exercises did anything. A definition written after seeing a row is not a measurement,
//! and a measure whose only check is the campaign that used it has not been checked.
//!
//! Nothing here simulates a world: the per-depleted-cell record, the derived `(L·μ)_crit`, the
//! four-way classification, the founder-brood counter and the margin accumulator are all
//! arithmetic over sequences that can be written by hand.

use std::collections::BTreeMap;

use cubarium_core::ids::OrganismId;
use cubarium_core::organism::DeathCause;
use cubarium_core::{BodyBudget, CHANNELS, FOLIAGE, LITTER};
use cubarium_search::depletion::{
    CLASSES, DepletedCell, DepletedClass, DepletionTracker, MAX_RECORDS, MAX_TRAJECTORY,
    PlantConstants, PRESSURE_FRACTION, ServedAttributor, TRAJECTORY_EVERY, l_mu_crit,
    monod_reference,
};
use cubarium_search::movement::{
    CensusKey, CrossingCounter, CrossingKind, DEPLETION_FRACTION, FounderBroods, MarginAccumulator,
    RECOVERY_FRACTION,
};

/// The shipped plant constants, written out rather than read from the config, so a test that
/// says "0.447" is checking arithmetic and not echoing a struct.
fn shipped() -> PlantConstants {
    PlantConstants {
        growth: 0.008,
        mortality: 0.001,
        maintenance: 0.0002,
        build: 0.2,
        monod_ref: 0.615_384_615_384_615_4,
        deplete_fraction: 0.25,
    }
}

/// `fast-leaf` halves `plant.maintenance`; nothing else in `(L·μ)_crit` moves.
fn fast_leaf() -> PlantConstants {
    PlantConstants { maintenance: 0.0001, ..shipped() }
}

/// An α-limited cell: `P₀ = 0.4 · α · W₀` with `α = 2`, so `P₀ = 0.8 · W₀`.
fn alpha_limited(w0: f64) -> (f64, f64) {
    (0.8 * w0, w0)
}

// ---------------------------------------------------------------------------------------
// (L·μ)_crit
// ---------------------------------------------------------------------------------------

#[test]
fn the_monod_reference_is_the_contracts_own_0_615() {
    // §13: "both with `N = 0.4` (Monod factor 0.615)" at the shipped `K_N = 0.25`.
    let m = monod_reference(0.25);
    assert!((m - 0.615).abs() < 5e-4, "monod reference {m} is not the contract's 0.615");
}

#[test]
fn l_mu_crit_is_the_contracts_breakeven_at_the_depletion_threshold() {
    // (1 + c_g)·m_p + m_w·W₀/(0.25·P₀)  over  g·monod
    //   = (1.2·0.001 + 0.0002/(0.25·0.8)) / (0.008·0.615…)
    //   = (0.0012 + 0.001) / 0.0049230…  = 0.4469…
    let (p0, w0) = alpha_limited(0.3);
    let crit = l_mu_crit(p0, w0, &shipped());
    assert!((crit - 0.4469).abs() < 1e-3, "crit {crit} is not the hand-computed 0.447");
}

#[test]
fn l_mu_crit_falls_when_wood_maintenance_falls() {
    let (p0, w0) = alpha_limited(0.3);
    let shipped_crit = l_mu_crit(p0, w0, &shipped());
    let fast = l_mu_crit(p0, w0, &fast_leaf());
    assert!((fast - 0.3455).abs() < 1e-3, "fast-leaf crit {fast} is not the hand-computed 0.346");
    assert!(fast < shipped_crit, "halving m_w must lower the threshold");
}

#[test]
fn l_mu_crit_is_wood_independent_while_alpha_limits_and_rises_when_p_max_does() {
    let a = l_mu_crit(alpha_limited(0.2).0, 0.2, &shipped());
    let b = l_mu_crit(alpha_limited(0.5).0, 0.5, &shipped());
    assert!((a - b).abs() < 1e-12, "in the α-limited regime W₀ cancels: {a} vs {b}");
    // A cell whose foliage is capped by `P_max = 1.5` instead of `α·W` carries the same
    // maintenance on less leaf, so it needs a brighter habitat.
    let capped = l_mu_crit(0.4 * 1.5, 1.0, &shipped());
    assert!(capped > a, "a P_max-limited cell must need more, not less: {capped} vs {a}");
}

#[test]
fn a_cell_with_no_opening_foliage_has_no_breakeven() {
    assert!(l_mu_crit(0.0, 0.3, &shipped()).is_infinite());
}

// ---------------------------------------------------------------------------------------
// The crossing counter reports exactly what it counts
// ---------------------------------------------------------------------------------------

#[test]
fn observe_reporting_reports_exactly_the_crossings_observe_counts() {
    let reference = vec![1.0, 2.0, 0.0, 0.5];
    let series = vec![
        vec![1.0, 2.0, 0.0, 0.5],
        vec![0.1, 1.9, 3.0, 0.5],
        vec![0.9, 0.4, 0.0, 0.1],
        vec![0.9, 1.5, 0.0, 0.3],
        vec![0.2, 0.2, 0.0, 0.4],
    ];

    let mut quiet = CrossingCounter::new(&reference, DEPLETION_FRACTION, RECOVERY_FRACTION);
    let mut loud = CrossingCounter::new(&reference, DEPLETION_FRACTION, RECOVERY_FRACTION);
    let (mut depletions, mut recoveries) = (0u64, 0u64);
    for p in &series {
        quiet.observe(p);
        loud.observe_reporting(p, |_, kind| match kind {
            CrossingKind::Depleted => depletions += 1,
            CrossingKind::Recovered => recoveries += 1,
        });
    }
    assert_eq!(quiet.depletions(), loud.depletions());
    assert_eq!(quiet.recoveries(), loud.recoveries());
    assert_eq!(depletions, quiet.depletions(), "every depletion is reported once");
    assert_eq!(recoveries, quiet.recoveries(), "every recovery is reported once");
}

// ---------------------------------------------------------------------------------------
// The per-depleted-cell record
// ---------------------------------------------------------------------------------------

/// Four watched cells with `P₀ = 1.0` and a habitat that makes cells 0 and 1 adequate and
/// cells 2 and 3 marginal under the shipped constants.
fn tracker() -> DepletionTracker {
    let p_ref = vec![1.0, 1.0, 1.0, 1.0];
    let light = vec![0.9, 0.9, 0.5, 0.5];
    let moisture = vec![0.8, 0.8, 0.4, 0.4];
    let w0 = vec![1.25, 1.25, 1.25, 1.25]; // P₀ = 1.0 = 0.8·W₀: α-limited
    DepletionTracker::new(&p_ref, &light, &moisture, &w0, &shipped())
}

fn occupancy(pairs: &[(u16, u32)]) -> BTreeMap<u16, u32> {
    pairs.iter().copied().collect()
}

fn served(pairs: &[(u16, f64)]) -> BTreeMap<u16, f64> {
    pairs.iter().copied().collect()
}

#[test]
fn a_record_opens_on_the_first_depletion_and_is_never_re_opened() {
    let mut t = tracker();
    let p = vec![1.0, 1.0, 1.0, 1.0];
    t.note_probe(20, &p, &occupancy(&[]), &served(&[]));
    t.depleted(0, 20);
    t.depleted(0, 400); // a second crossing, after a recovery
    t.recovered(0, 300);
    let s = t.finish(&p);
    assert_eq!(s.records.len(), 1, "one cell, one record");
    assert_eq!(s.cells_with_record, 1);
    let r = &s.records[0];
    assert_eq!(r.cell, 0);
    assert_eq!(r.first_depletion_tick, 20, "the record keeps its first crossing");
    assert_eq!(r.depletions, 2);
    assert_eq!(r.recoveries, 1);
}

#[test]
fn the_last_visit_before_depletion_is_the_probe_the_body_was_standing_there() {
    let mut t = tracker();
    // Probe 20: a body stands in cell 0 with the cell still full.
    t.note_probe(20, &[1.0, 1.0, 1.0, 1.0], &occupancy(&[(0, 1)]), &served(&[]));
    // Probe 40: the body is still there and the cell crosses.
    t.note_probe(40, &[0.2, 1.0, 1.0, 1.0], &occupancy(&[(0, 1)]), &served(&[]));
    t.depleted(0, 40);
    let s = t.finish(&[0.2, 1.0, 1.0, 1.0]);
    let r = &s.records[0];
    assert_eq!(r.last_visit_tick, Some(40), "positions are read before the crossing");
    assert_eq!(r.last_visit_stock, Some(0.2), "the stock standing at that probe");
    assert_eq!(r.ticks_since_visit_at_depletion, Some(0), "it was standing there as it crossed");
    assert_eq!(r.post_visit_probes, 0, "that visit is before, not after");
}

#[test]
fn a_cell_no_body_ever_stood_in_has_no_last_visit_and_no_post_depletion_pressure() {
    let mut t = tracker();
    t.note_probe(20, &[1.0, 1.0, 1.0, 1.0], &occupancy(&[(1, 3)]), &served(&[(1, 0.5)]));
    t.depleted(0, 20);
    t.note_probe(40, &[0.2, 1.0, 1.0, 1.0], &occupancy(&[(1, 3)]), &served(&[(1, 0.5)]));
    let s = t.finish(&[0.2, 1.0, 1.0, 1.0]);
    let r = s.records.iter().find(|r| r.cell == 0).expect("cell 0 has a record");
    assert_eq!(r.last_visit_tick, None);
    assert_eq!(r.last_visit_stock, None);
    assert_eq!(r.ticks_since_visit_at_depletion, None);
    assert_eq!(r.post_visit_probes, 0);
    assert_eq!(r.post_served_foliage, 0.0, "another cell's bites are not this cell's");
    assert_eq!(s.fraction_no_post_visit, 1.0);
}

#[test]
fn post_depletion_visits_and_bites_count_only_probes_strictly_after_the_crossing() {
    let mut t = tracker();
    t.note_probe(20, &[1.0, 1.0, 1.0, 1.0], &occupancy(&[(0, 2)]), &served(&[(0, 0.10)]));
    t.depleted(0, 20);
    t.note_probe(40, &[0.2, 1.0, 1.0, 1.0], &occupancy(&[(0, 3)]), &served(&[(0, 0.20)]));
    t.note_probe(60, &[0.2, 1.0, 1.0, 1.0], &occupancy(&[]), &served(&[]));
    t.note_probe(80, &[0.2, 1.0, 1.0, 1.0], &occupancy(&[(0, 1)]), &served(&[(0, 0.05)]));
    let s = t.finish(&[0.2, 1.0, 1.0, 1.0]);
    let r = &s.records[0];
    assert_eq!(r.post_visit_probes, 2, "probes 40 and 80, not 20 and not 60");
    assert_eq!(r.post_visit_body_probes, 4, "3 bodies at probe 40 and 1 at probe 80");
    assert!(
        (r.post_served_foliage - 0.25).abs() < 1e-12,
        "0.20 + 0.05; the 0.10 at the depleting probe is before, not after"
    );
    assert!((r.post_served_over_p0 - 0.25).abs() < 1e-12, "P₀ = 1.0");
}

#[test]
fn the_trajectory_is_sampled_on_the_600_tick_grid_strictly_after_depletion() {
    let mut t = tracker();
    t.depleted(0, 600);
    for tick in [600u64, 1_200, 1_800, 2_400] {
        let p = vec![0.1 * (tick / 600) as f64, 1.0, 1.0, 1.0];
        t.sample(tick, &p);
    }
    // A tick off the grid contributes nothing.
    t.sample(1_500, &[9.0, 1.0, 1.0, 1.0]);
    let s = t.finish(&[0.4, 1.0, 1.0, 1.0]);
    let r = &s.records[0];
    assert_eq!(r.trajectory.len(), 3, "1,200, 1,800 and 2,400; not 600, which is not *after*");
    assert!((r.trajectory[0] - 0.2).abs() < 1e-6);
    assert!((r.trajectory[2] - 0.4).abs() < 1e-6);
    assert!((r.p_final_over_p0 - 0.4).abs() < 1e-12);
    assert!(TRAJECTORY_EVERY == 600 && MAX_TRAJECTORY == 300, "the declared cadence and cap");
}

#[test]
fn the_extremes_are_taken_over_the_samples_after_depletion() {
    let mut t = tracker();
    t.sample(600, &[1.0, 1.0, 1.0, 1.0]); // before any depletion: not an extreme
    t.depleted(0, 600);
    t.sample(1_200, &[0.10, 1.0, 1.0, 1.0]);
    t.sample(1_800, &[0.45, 1.0, 1.0, 1.0]);
    t.sample(2_400, &[0.30, 1.0, 1.0, 1.0]);
    let s = t.finish(&[0.30, 1.0, 1.0, 1.0]);
    let r = &s.records[0];
    assert!((r.p_max_over_p0 - 0.45).abs() < 1e-6);
    assert!((r.p_min_over_p0 - 0.10).abs() < 1e-6);
}

#[test]
fn first_recovery_and_first_redepletion_are_the_first_crossings_after_the_first_depletion() {
    let mut t = tracker();
    t.depleted(0, 100);
    t.recovered(0, 300);
    t.depleted(0, 500);
    t.recovered(0, 900);
    // Cell 1 depletes and never comes back.
    t.depleted(1, 200);
    let s = t.finish(&[0.2, 0.2, 1.0, 1.0]);
    let zero = s.records.iter().find(|r| r.cell == 0).expect("cell 0");
    assert_eq!(zero.first_recovery_tick, Some(300));
    assert_eq!(zero.first_redepletion_tick, Some(500));
    assert_eq!(zero.recoveries, 2);
    let one = s.records.iter().find(|r| r.cell == 1).expect("cell 1");
    assert_eq!(one.first_recovery_tick, None);
    assert_eq!(one.first_redepletion_tick, None);
    assert!(
        (s.mean_recovery_latency_ticks - 200.0).abs() < 1e-12,
        "over the recovered records only: 300 − 100"
    );
}

#[test]
fn records_past_the_cap_are_counted_and_not_silently_truncated() {
    let n = MAX_RECORDS + 3;
    let p_ref = vec![1.0; n];
    let light = vec![0.9; n];
    let moisture = vec![0.8; n];
    let w0 = vec![1.25; n];
    let mut t = DepletionTracker::new(&p_ref, &light, &moisture, &w0, &shipped());
    for cell in 0..n {
        t.depleted(cell, 20);
    }
    let s = t.finish(&vec![0.2; n]);
    assert_eq!(s.records.len(), MAX_RECORDS);
    assert_eq!(s.records_dropped, 3);
}

// ---------------------------------------------------------------------------------------
// The four-way classification
// ---------------------------------------------------------------------------------------

fn record(recovered: bool, post_served: f64, l_mu: f64) -> DepletedCell {
    DepletedCell {
        cell: 0,
        l_mu,
        l_mu_crit: 0.447,
        marginal: l_mu < 0.447,
        p0: 1.0,
        first_recovery_tick: recovered.then_some(500),
        post_served_foliage: post_served,
        post_served_over_p0: post_served,
        ..DepletedCell::default()
    }
}

#[test]
fn a_recovered_cell_is_recovered_whatever_its_pressure_and_habitat() {
    assert_eq!(record(true, 0.0, 0.9).classify(), DepletedClass::Recovered);
    assert_eq!(record(true, 5.0, 0.1).classify(), DepletedClass::Recovered);
}

#[test]
fn an_unrecovered_cell_that_kept_being_bitten_is_pressure() {
    assert_eq!(record(false, PRESSURE_FRACTION, 0.9).classify(), DepletedClass::Pressure);
    assert_eq!(record(false, 1.0, 0.1).classify(), DepletedClass::Pressure);
    assert!(record(false, PRESSURE_FRACTION, 0.9).under_pressure());
    assert!(!record(false, PRESSURE_FRACTION - 1e-6, 0.9).under_pressure());
}

#[test]
fn an_unrecovered_unpressured_adequate_cell_is_plant_limited() {
    assert_eq!(record(false, 0.0, 0.9).classify(), DepletedClass::PlantLimited);
    assert_eq!(record(false, 0.1, 0.5).classify(), DepletedClass::PlantLimited);
}

#[test]
fn an_unrecovered_unpressured_dim_cell_is_marginal() {
    assert_eq!(record(false, 0.0, 0.3).classify(), DepletedClass::Marginal);
}

#[test]
fn the_four_classes_are_exhaustive_and_disjoint_over_a_hand_built_set() {
    let mut t = tracker();
    // cell 0: adequate, no pressure, recovers          -> Recovered
    // cell 1: adequate, heavy post-depletion bites     -> Pressure
    // cell 2: marginal, bitten hard                    -> Pressure (priority over Marginal)
    // cell 3: marginal, untouched                      -> Marginal
    let p = vec![0.2, 0.2, 0.2, 0.2];
    t.depleted(0, 20);
    t.depleted(1, 20);
    t.depleted(2, 20);
    t.depleted(3, 20);
    t.note_probe(40, &p, &occupancy(&[(1, 1), (2, 1)]), &served(&[(1, 0.9), (2, 0.9)]));
    t.recovered(0, 60);
    let s = t.finish(&p);
    let class_of = |cell: u16| {
        s.records.iter().find(|r| r.cell == cell).expect("record").class.expect("classified")
    };
    assert_eq!(class_of(0), DepletedClass::Recovered);
    assert_eq!(class_of(1), DepletedClass::Pressure);
    assert_eq!(class_of(2), DepletedClass::Pressure);
    assert_eq!(class_of(3), DepletedClass::Marginal);

    assert_eq!(s.by_class.iter().sum::<u32>(), 4, "exhaustive: every record has exactly one");
    assert_eq!(s.by_class, [1, 2, 0, 1]);
    let total: f64 = s.fraction_by_class.iter().sum();
    assert!((total - 1.0).abs() < 1e-12, "the fractions sum to one");
    assert!((s.fraction_marginal - 0.5).abs() < 1e-12, "cells 2 and 3 are below crit");
    assert!((s.fraction_any_post_bite - 0.5).abs() < 1e-12);
    assert_eq!(CLASSES.len(), 4);
    for (i, c) in CLASSES.iter().enumerate() {
        assert_eq!(c.index(), i, "the report order is the array order");
    }
}

// ---------------------------------------------------------------------------------------
// Attributing served material to cells
// ---------------------------------------------------------------------------------------

fn id(slot: u32) -> OrganismId {
    OrganismId { slot, generation: 1 }
}

#[test]
fn a_body_seen_for_the_first_time_is_differenced_against_zero() {
    let mut a = ServedAttributor::default();
    let mut out = BTreeMap::new();
    a.observe(id(1), 7, 0.4, &mut out);
    assert_eq!(out.get(&7).copied(), Some(0.4), "everything it ate since birth");
    out.clear();
    a.observe(id(1), 7, 0.6, &mut out);
    assert!((out[&7] - 0.2).abs() < 1e-12, "only the difference the second time");
}

#[test]
fn a_moving_body_credits_the_cell_it_is_observed_in_now() {
    let mut a = ServedAttributor::default();
    let mut out = BTreeMap::new();
    a.observe(id(1), 7, 0.0, &mut out);
    out.clear();
    a.observe(id(1), 9, 0.3, &mut out);
    assert_eq!(out.get(&7), None, "the cell it left gets nothing");
    assert!((out[&9] - 0.3).abs() < 1e-12);
}

#[test]
fn a_body_that_dies_is_credited_to_the_cell_it_was_last_observed_in() {
    let mut a = ServedAttributor::default();
    let mut out = BTreeMap::new();
    a.observe(id(2), 4, 1.0, &mut out);
    out.clear();
    a.close(id(2), 4, 1.25, &mut out);
    assert!((out[&4] - 0.25).abs() < 1e-12, "the tail between the last probe and the death");
    out.clear();
    a.close(id(2), 4, 9.0, &mut out);
    assert!(out.is_empty(), "a closed body is forgotten, not double-counted");
}

#[test]
fn two_bodies_in_one_cell_add_their_bites() {
    let mut a = ServedAttributor::default();
    let mut out = BTreeMap::new();
    a.observe(id(1), 3, 0.2, &mut out);
    a.observe(id(2), 3, 0.5, &mut out);
    assert!((out[&3] - 0.7).abs() < 1e-12);
}

// ---------------------------------------------------------------------------------------
// The founder grazer's brood
// ---------------------------------------------------------------------------------------

#[test]
fn only_a_founders_birth_is_a_founder_brood_and_the_first_tick_is_the_first() {
    let mut b = FounderBroods::default();
    for _ in 0..10 {
        b.found(0);
    }
    for _ in 0..5 {
        b.found(1);
    }
    b.brood(0, 4_200, true);
    b.brood(0, 5_000, false); // the same founder again
    b.brood(0, 6_000, true); // a second founder
    assert_eq!(b.broods_by_form[0], 3);
    assert_eq!(b.parents_by_form[0], 2, "distinct founders, not births");
    assert_eq!(b.first_brood_tick_by_form[0], Some(4_200));
    assert_eq!(b.founders_by_form[0], 10);
    assert!(b.grazer_bred());

    let quiet = FounderBroods::default();
    assert!(!quiet.grazer_bred(), "no birth is not a brood");
    assert_eq!(quiet.first_brood_tick_by_form[1], None);
}

// ---------------------------------------------------------------------------------------
// The net energy margin from E's ledger
// ---------------------------------------------------------------------------------------

fn budget(slot: u32) -> BodyBudget {
    BodyBudget {
        id: id(slot),
        opened_tick: 0,
        born_tick: 0,
        closed_tick: None,
        death_cause: None,
        start_structure: 1.0,
        start_reserve: 0.0,
        start_energy: 0.0,
        end_structure: 1.0,
        end_reserve: 0.0,
        end_energy: 0.0,
        served: [0.0; CHANNELS],
        digestible: [0.0; CHANNELS],
        reserve_credit: [0.0; CHANNELS],
        battery_credit: [0.0; CHANNELS],
        gut_reserve_credit: 0.0,
        gut_battery_credit: 0.0,
        oxidation_reserve_burned: 0.0,
        oxidation_battery_credit: 0.0,
        upkeep_billed: 0.0,
        motor_translation_billed: 0.0,
        motor_turn_billed: 0.0,
        bill_total: 0.0,
        bill_paid: 0.0,
        other_energy_paid: 0.0,
        growth_material: 0.0,
        growth_energy: 0.0,
        reproduction_material: 0.0,
        reproduction_energy: 0.0,
        injury_structure: 0.0,
        billed_ticks: 0,
    }
}

#[test]
fn the_margin_is_food_energy_in_minus_energy_owed() {
    let mut b = budget(1);
    b.battery_credit[FOLIAGE] = 3.0;
    b.reserve_credit[FOLIAGE] = 2.0; // e_r = 2 -> 4.0 e
    b.battery_credit[LITTER] = 1.0;
    b.bill_total = 6.0;
    b.bill_paid = 5.0;
    b.growth_energy = 0.5;
    b.reproduction_energy = 0.5;
    b.closed_tick = Some(2_000);
    b.death_cause = Some(DeathCause::Starvation);

    let mut acc = MarginAccumulator::default();
    acc.add(CensusKey { form: 0, diet_bin: 2, guild: 0 }, &b, 2.0, 2_000, 0.05, false);
    let m = acc.finish(true);
    assert_eq!(m.bodies, 1);
    assert_eq!(m.bins.len(), 1);
    let bin = m.bins[0];
    // food in = 3 + 2·2 + 1 = 8; owed = 6 + 0.5 + 0.5 = 7; margin = 1
    assert!((bin.food_energy_in_mean - 8.0).abs() < 1e-12);
    assert!((bin.energy_owed_mean - 7.0).abs() < 1e-12);
    assert!((bin.margin_mean - 1.0).abs() < 1e-12);
    assert!((m.margin_mean - 1.0).abs() < 1e-12);
    assert!((bin.bill_unpaid_mean - 1.0).abs() < 1e-12, "bill_total − bill_paid");
    assert!(
        (bin.recorded_seconds_mean - 100.0).abs() < 1e-12,
        "2,000 ticks at dt = 0.05 s"
    );
    assert!((bin.margin_rate_mean - 0.01).abs() < 1e-12, "1 e over 100 s");
    assert_eq!(bin.deaths, 1);
    assert_eq!(bin.alive, 0);
    assert!(m.ledger_on);
}

#[test]
fn the_margin_charges_what_was_owed_not_what_was_paid() {
    let mut owed_more = budget(1);
    owed_more.bill_total = 10.0;
    owed_more.bill_paid = 1.0;
    let mut acc = MarginAccumulator::default();
    acc.add(CensusKey { form: 0, diet_bin: 2, guild: 0 }, &owed_more, 2.0, 100, 0.05, false);
    let m = acc.finish(true);
    assert!(
        (m.bins[0].margin_mean + 10.0).abs() < 1e-12,
        "the deficit is the whole bill, not the ninth of it the body could raise"
    );
}

#[test]
fn margins_bin_by_form_and_diet_and_the_bins_stay_disjoint() {
    let mut acc = MarginAccumulator::default();
    let mut grazer = budget(1);
    grazer.battery_credit[FOLIAGE] = 4.0;
    let mut burrower = budget(2);
    burrower.battery_credit[LITTER] = 1.0;
    burrower.bill_total = 3.0;
    burrower.bill_paid = 3.0;
    burrower.motor_translation_billed = 1.0;
    burrower.motor_turn_billed = 0.5;
    acc.add(CensusKey { form: 0, diet_bin: 2, guild: 0 }, &grazer, 2.0, 400, 0.05, true);
    acc.add(CensusKey { form: 2, diet_bin: 0, guild: 1 }, &burrower, 2.0, 400, 0.05, false);
    acc.note_dropped(2);
    let m = acc.finish(true);
    assert_eq!(m.bodies, 2);
    assert_eq!(m.records_dropped, 2, "a dropped record is reported, never silently zero");
    assert_eq!(m.bins.len(), 2);
    let g = m.bins.iter().find(|b| b.form == 0).expect("grazer bin");
    let d = m.bins.iter().find(|b| b.form == 2).expect("burrower bin");
    assert_eq!(g.diet_bin, 2);
    assert_eq!(g.bodies, 1);
    assert_eq!(g.alive, 1);
    assert!((g.margin_mean - 4.0).abs() < 1e-12);
    assert_eq!(d.diet_bin, 0);
    assert!((d.margin_mean + 2.0).abs() < 1e-12, "1 in, 3 owed");
    assert!((d.motor_share_mean - 0.5).abs() < 1e-12, "(1.0 + 0.5) / 3.0");
    assert!((g.motor_share_mean - 0.0).abs() < 1e-12, "a body with no bill contributes none");
}

#[test]
fn a_run_without_the_ledger_reports_that_rather_than_a_zero() {
    let m = MarginAccumulator::default().finish(false);
    assert!(!m.ledger_on);
    assert_eq!(m.bodies, 0);
    assert!(m.bins.is_empty());
}
