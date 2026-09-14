//! Focused tests for the search harness: determinism, parameter and budget validation, and
//! the resource invariants the real core is supposed to hold.
//!
//! These are deliberately *not* tests of whether any ecology is sustainable. They test the
//! tool. Horizons are short so the suite stays fast; the core's own `debug_assertions` energy,
//! water and invariant audits run on every tick of every world built here.

use std::sync::Mutex;

use cubarium_search::evaluate::{Protocol, Status, base_config, evaluate};
use cubarium_search::metrics::{Objectives, Scoring};
use cubarium_search::params;
use cubarium_search::search::{self, Budget, StopReason, TRAINING_SEEDS, Variation};

/// Short enough for a fast suite, long enough that founders move, feed and steer.
const SHORT: Protocol = Protocol {
    horizon_ticks: 400,
    sample_every: 50,
    apex_founders: 2,
    apex_introduce_tick: 0,
};

#[test]
fn the_default_vector_round_trips_and_sits_inside_its_own_box() {
    let defaults = params::defaults();
    assert_eq!(defaults.len(), params::PARAMS.len());
    for (v, p) in defaults.iter().zip(params::PARAMS) {
        assert!(
            p.lo <= *v && *v <= p.hi,
            "{} default {v} is outside [{}, {}]",
            p.name,
            p.lo,
            p.hi
        );
        assert!(p.lo < p.hi, "{} has an empty range", p.name);
    }

    let mut config = base_config(TRAINING_SEEDS[0]);
    let mut profile =
        cubarium_core::hunter::FixedHunterProfile::lanternjaw_trial(&config);
    params::apply(&defaults, &mut config, &mut profile).unwrap();
    assert_eq!(params::read(&config, &profile), defaults);
    // The defaults must be exactly the shipped world, so a search always reports what it moved
    // away from.
    assert_eq!(config, base_config(TRAINING_SEEDS[0]));

    // A vector of the wrong length, or a non-finite component, is refused rather than applied.
    assert!(params::apply(&defaults[..2], &mut config, &mut profile).is_err());
    let mut broken = defaults.clone();
    broken[0] = f64::NAN;
    assert!(params::apply(&broken, &mut config, &mut profile).is_err());

    // Clamping repairs the search's own proposals and nothing else.
    let mut wild: Vec<f64> = params::PARAMS.iter().map(|p| p.hi * 10.0).collect();
    wild[1] = f64::NAN;
    params::clamp(&mut wild);
    for (v, p) in wild.iter().zip(params::PARAMS) {
        assert!(p.lo <= *v && *v <= p.hi, "{} was not clamped: {v}", p.name);
    }

    // Fingerprints separate vectors that differ anywhere, and only those.
    let mut moved = defaults.clone();
    moved[3] += 1e-12;
    assert_ne!(params::fingerprint(&defaults), params::fingerprint(&moved));
    assert_eq!(params::fingerprint(&defaults), params::fingerprint(&params::defaults()));
}

#[test]
fn the_same_candidate_and_seed_reproduce_bit_for_bit_and_different_seeds_do_not() {
    let values = params::defaults();
    let a = evaluate(&values, TRAINING_SEEDS[0], SHORT);
    let b = evaluate(&values, TRAINING_SEEDS[0], SHORT);
    assert_eq!(a.status, Status::Completed, "{:?}", a.reason);
    assert_eq!(a.metrics, b.metrics, "a replay of the same (candidate, seed) must be identical");

    let other = evaluate(&values, TRAINING_SEEDS[1], SHORT);
    assert_eq!(other.status, Status::Completed);
    let (ha, hb) = (
        a.metrics.as_ref().unwrap().final_ecology_hash,
        other.metrics.as_ref().unwrap().final_ecology_hash,
    );
    assert_ne!(ha, hb, "a different seed must be a different world");

    // And a different candidate is a different world on the same seed.
    let mut moved = values.clone();
    moved[0] = params::PARAMS[0].hi;
    let shifted = evaluate(&moved, TRAINING_SEEDS[0], SHORT);
    assert_eq!(shifted.status, Status::Completed);
    assert_ne!(ha, shifted.metrics.as_ref().unwrap().final_ecology_hash);
}

#[test]
fn a_configuration_the_core_refuses_is_recorded_as_rejected_not_repaired() {
    // `drives.bud_reserve` below 0.60 cannot fund a child: the core refuses it, and the
    // declared search box deliberately contains that region.
    let mut values = params::defaults();
    let index = params::PARAMS.iter().position(|p| p.name == "drives.bud_reserve").unwrap();
    assert!(params::PARAMS[index].lo < 0.60, "the box must straddle the constraint");
    values[index] = params::PARAMS[index].lo;

    let refused = evaluate(&values, TRAINING_SEEDS[0], SHORT);
    assert_eq!(refused.status, Status::Invalid);
    let reason = refused.reason.expect("a refusal must carry its reason");
    assert!(reason.contains("child material"), "unexpected refusal: {reason}");
    assert!(refused.metrics.is_none(), "a refused candidate must not carry metrics");
    // The refusal is the world's decision, not a repair: the value went through untouched.
    let mut config = base_config(TRAINING_SEEDS[0]);
    let mut profile = cubarium_core::hunter::FixedHunterProfile::lanternjaw_trial(&config);
    params::apply(&values, &mut config, &mut profile).unwrap();
    assert_eq!(config.drives.bud_reserve, params::PARAMS[index].lo);
}

#[test]
fn the_protocol_and_the_budget_refuse_every_limit_they_cannot_enforce() {
    SHORT.validate().unwrap();
    Protocol::default().validate().unwrap();
    let cases: Vec<(&str, Protocol)> = vec![
        ("horizon_ticks", Protocol { horizon_ticks: 0, ..SHORT }),
        ("sample_every", Protocol { sample_every: 0, ..SHORT }),
        ("sample_every", Protocol { sample_every: 10_000, ..SHORT }),
        ("apex_founders", Protocol { apex_founders: 3, ..SHORT }),
        ("apex_introduce_tick", Protocol { apex_introduce_tick: 400, ..SHORT }),
    ];
    for (expect, protocol) in cases {
        let err = protocol.validate().unwrap_err();
        assert!(err.contains(expect), "expected {expect}, got {err}");
    }

    let ok = Budget::default();
    ok.validate().unwrap();
    let cases: Vec<(&str, Budget)> = vec![
        ("max_evaluations", Budget { max_evaluations: 0, ..ok }),
        ("workers", Budget { workers: 0, ..ok }),
        ("workers", Budget { workers: 64, ..ok }),
        ("population", Budget { population: 1, ..ok }),
        ("elite", Budget { elite: 0, ..ok }),
        ("elite", Budget { elite: 4, population: 4, ..ok }),
        ("generations", Budget { generations: 0, ..ok }),
        ("seeds", Budget { seeds: 0, ..ok }),
        ("seeds", Budget { seeds: 99, ..ok }),
        ("wall_seconds", Budget { wall_seconds: 0, ..ok }),
        ("max_rows", Budget { max_rows: 0, ..ok }),
    ];
    for (expect, budget) in cases {
        let err = budget.validate().unwrap_err();
        assert!(err.contains(expect), "expected {expect}, got {err}");
    }
    assert_eq!(ok.seed_schedule(), &TRAINING_SEEDS[..ok.seeds]);

    Variation::default().validate().unwrap();
    for bad in [
        Variation { mutation_rate: 1.5, ..Variation::default() },
        Variation { crossover_rate: -0.1, ..Variation::default() },
        Variation { mutation_sigma: 0.9, ..Variation::default() },
        Variation { mutation_sigma: f64::NAN, ..Variation::default() },
    ] {
        assert!(bad.validate().is_err(), "{bad:?} should have been refused");
    }
    // A search refuses an unenforceable limit before it starts any work.
    let err = search::run(
        SHORT,
        Budget { workers: 0, ..ok },
        Variation::default(),
        Scoring::default(),
        1,
        |_| panic!("no work may start"),
    )
    .unwrap_err();
    assert!(err.contains("workers"), "{err}");
}

#[test]
fn the_evaluation_cap_is_the_number_of_simulations_actually_run() {
    let budget = Budget {
        max_evaluations: 3,
        workers: 2,
        wall_seconds: 600,
        population: 4,
        elite: 1,
        generations: 3,
        seeds: 1,
        max_rows: 64,
    };
    let rows = Mutex::new(0u64);
    let report = search::run(
        Protocol { horizon_ticks: 200, ..SHORT },
        budget,
        Variation::default(),
        Scoring::default(),
        4_242,
        |_| *rows.lock().unwrap() += 1,
    )
    .unwrap();

    assert_eq!(report.evaluations_run, 3, "the cap is a hard limit, not a target");
    assert_eq!(report.stop_reason, StopReason::Evaluations);
    assert_eq!(report.generations_run, 1);
    assert_eq!(*rows.lock().unwrap(), 3, "one row per simulation actually run");
    // The candidate the budget never reached is reported, unscored, rather than dropped.
    assert_eq!(report.candidates.len(), 4);
    // Exactly one candidate was never reached; it is reported as unevaluated rather than
    // dropped. Others may legitimately be unscored because the core refused them.
    assert_eq!(
        report
            .candidates
            .iter()
            .filter(|c| c.completed + c.invalid + c.failed == 0)
            .count(),
        1,
        "the candidate the budget never reached must still appear"
    );
}

#[test]
fn a_search_is_reproducible_from_its_seed_and_its_rows_are_ordered() {
    let budget = Budget {
        max_evaluations: 6,
        workers: 3,
        wall_seconds: 600,
        population: 3,
        elite: 1,
        generations: 2,
        seeds: 1,
        max_rows: 64,
    };
    let once = collect(budget, 77);
    let twice = collect(budget, 77);
    assert_eq!(once, twice, "the same search seed must produce the same search");
    let different = collect(budget, 78);
    assert_ne!(once, different, "a different search seed must explore differently");

    // Rows come out in (generation, candidate, seed) order whatever the workers did.
    let mut keys: Vec<(u32, u64)> = once.iter().map(|(g, c, _)| (*g, *c)).collect();
    let sorted = {
        let mut k = keys.clone();
        k.sort();
        k
    };
    assert_eq!(keys, sorted);
    keys.dedup();
    assert_eq!(keys.len(), once.len(), "one row per (candidate, seed)");
}

fn collect(budget: Budget, seed: u64) -> Vec<(u32, u64, Option<u64>)> {
    let rows = Mutex::new(Vec::new());
    search::run(
        Protocol { horizon_ticks: 200, ..SHORT },
        budget,
        Variation::default(),
        Scoring::default(),
        seed,
        |row| {
            rows.lock().unwrap().push((
                row.generation,
                row.candidate,
                row.evaluation.metrics.as_ref().map(|m| m.final_ecology_hash),
            ));
        },
    )
    .unwrap();
    rows.into_inner().unwrap()
}

#[test]
fn a_completed_run_keeps_the_worlds_material_energy_and_water_identities() {
    let protocol = Protocol { horizon_ticks: 600, sample_every: 60, ..SHORT };
    let evaluation = evaluate(&params::defaults(), TRAINING_SEEDS[2], protocol);
    assert_eq!(evaluation.status, Status::Completed, "{:?}", evaluation.reason);
    let m = evaluation.metrics.unwrap();

    // The apex cohort is an accounted input, not free material.
    assert_eq!(m.apex_introduced, 2);
    assert!(evaluation.apex_material_in > 0.0, "apex founders must book their material");
    assert!(evaluation.apex_energy_in > 0.0, "apex founders must book their energy");

    // Scaled to the world's own stocks, these are rounding, not drift.
    let scale = m.producer_capacity.max(1.0);
    assert!(
        m.max_abs_mass_residual < 1e-9 * scale,
        "mass residual {} is drift, not rounding",
        m.max_abs_mass_residual
    );
    assert!(
        m.max_abs_energy_residual < 1e-9 * scale,
        "energy residual {} is drift, not rounding",
        m.max_abs_energy_residual
    );
    assert!(
        m.max_abs_water_residual < 1e-9 * scale,
        "water residual {} is drift, not rounding",
        m.max_abs_water_residual
    );

    // The run really ran, on a real world, with ordinary founders.
    assert_eq!(m.ticks_run, protocol.horizon_ticks);
    assert!(!m.collapsed);
    assert_eq!(m.survived_ticks, protocol.horizon_ticks);
    assert!(m.final_population >= 20, "the default world places 24 prey founders plus the apex");
    assert_eq!(m.min_forms_present, 5, "four founder kinds plus the apex rig");
    assert!(m.feeding_fraction > 0.0, "ordinary founders feed; frozen ones would not");
    assert!(m.mean_producer > 0.0 && m.gross_production_per_hour > 0.0);
}

#[test]
fn a_prey_only_protocol_runs_with_no_apex_and_scores_no_apex() {
    let protocol = Protocol { apex_founders: 0, ..SHORT };
    let evaluation = evaluate(&params::defaults(), TRAINING_SEEDS[0], protocol);
    assert_eq!(evaluation.status, Status::Completed, "{:?}", evaluation.reason);
    let m = evaluation.metrics.unwrap();
    assert_eq!(m.apex_introduced, 0);
    assert_eq!(evaluation.apex_material_in, 0.0);
    assert_eq!(m.apex_alive_final, 0);
    assert_eq!(m.min_forms_present, 4, "the four founder kinds, and no apex rig");
    let scoring = Scoring::default();
    let objectives = m.objectives(&scoring);
    assert!(
        objectives.apex <= scoring.component_floor * 1.001,
        "a world with no apex scores no apex, got {}",
        objectives.apex
    );
    assert!(objectives.persistence > 0.0);
}

#[test]
fn the_scalar_rank_cannot_hide_a_dead_or_idle_world() {
    let scoring = Scoring::default();
    let protocol = Protocol { horizon_ticks: 600, sample_every: 60, ..SHORT };
    let live = evaluate(&params::defaults(), TRAINING_SEEDS[0], protocol)
        .metrics
        .expect("the default world completes");

    // A world of bodies that never feed is ranked out, however many of them there are.
    let mut idle = live.clone();
    idle.feeding_fraction = 0.0;
    assert!(idle.fitness(&scoring) < live.fitness(&scoring) * 0.01);

    // So is one that collapsed halfway, even with every other component intact.
    let mut collapsed = live.clone();
    collapsed.collapsed = true;
    collapsed.survived_ticks = live.horizon_ticks / 2;
    assert!(collapsed.fitness(&scoring) < live.fitness(&scoring));

    // And a single dominant form loses to an even one.
    let mut monoculture = live.clone();
    monoculture.mean_form_evenness = 0.0;
    monoculture.min_forms_present = 1;
    assert!(monoculture.fitness(&scoring) < live.fitness(&scoring));

    // Dormant apex presence is not apex life.
    let mut only_dormant = live.clone();
    only_dormant.apex_active_sample_fraction = 0.0;
    only_dormant.apex_dormant_mean = 2.0;
    assert!(only_dormant.objectives(&scoring).apex <= live.objectives(&scoring).apex);
}

#[test]
fn dominance_is_a_partial_order_over_all_seven_objectives() {
    let base = Objectives {
        persistence: 0.5,
        plants: 0.5,
        prey_turnover: 0.5,
        maturation: 0.5,
        lineage: 0.5,
        variety: 0.5,
        apex: 0.5,
    };
    let better = Objectives { apex: 0.6, ..base };
    // A trade: strictly better on apex, strictly worse on variety.
    let traded = Objectives { apex: 0.7, variety: 0.4, ..base };
    assert!(better.dominates(&base));
    assert!(!base.dominates(&better));
    assert!(!base.dominates(&base), "an identical vector does not dominate itself");
    assert!(!traded.dominates(&better) && !better.dominates(&traded));
}

/// The wall clock is a limit, not a suggestion: a worker checks it before claiming the next
/// simulation, so an overrunning search stops between evaluations rather than at the end.
///
/// Timing-dependent by nature, so the horizon is set well above the cutoff and the assertions
/// only require that the search stopped early and said so.
#[test]
fn the_wall_clock_stops_a_search_between_evaluations() {
    let budget = Budget {
        max_evaluations: 16,
        workers: 1,
        wall_seconds: 1,
        population: 4,
        elite: 1,
        generations: 2,
        seeds: 1,
        max_rows: 64,
    };
    let started = std::time::Instant::now();
    let report = search::run(
        Protocol { horizon_ticks: 40_000, sample_every: 500, ..SHORT },
        budget,
        Variation::default(),
        Scoring::default(),
        9,
        |_| {},
    )
    .unwrap();
    assert_eq!(report.stop_reason, StopReason::WallTime);
    assert!(
        report.evaluations_run < budget.max_evaluations,
        "the clock, not the evaluation cap, must have stopped it"
    );
    assert_eq!(report.generations_run, 1, "it must not have started another generation");
    assert!(started.elapsed().as_secs_f64() >= 1.0);
}

/// A recorded row must hand the simulation back exactly the numbers it was given.
///
/// Regression: the first version of this harness replayed from the row's readable decimals, and
/// `replay` caught itself diverging — `serde_json` 1.0.151 returned `fruit.ripen` one ULP away
/// from the value that was written, and one ULP is a different world. Rows now carry the exact
/// IEEE-754 bit patterns, and this test pins that they are what a replay reads.
#[test]
fn a_recorded_row_replays_from_its_exact_bits() {
    let budget = Budget {
        max_evaluations: 4,
        workers: 2,
        wall_seconds: 600,
        population: 4,
        elite: 1,
        generations: 1,
        seeds: 1,
        max_rows: 16,
    };
    let protocol = Protocol { horizon_ticks: 300, sample_every: 50, ..SHORT };
    let rows = Mutex::new(Vec::new());
    search::run(
        protocol,
        budget,
        Variation::default(),
        Scoring::default(),
        1_234,
        |row| rows.lock().unwrap().push(row.clone()),
    )
    .unwrap();
    let rows = rows.into_inner().unwrap();
    assert_eq!(rows.len(), 4);

    let mut decimals_were_lossy = false;
    for row in &rows {
        let exact = params::from_bit_labels(&row.param_bits).expect("rows carry exact bits");
        assert_eq!(params::fingerprint(&exact), row.param_fingerprint);

        // The readable decimals are allowed to lose a bit; the row must not depend on them.
        let readable: Vec<f64> = params::PARAMS
            .iter()
            .map(|p| row.params[p.name].as_f64().unwrap())
            .collect();
        decimals_were_lossy |= readable != exact;

        let again = evaluate(&exact, row.evaluation.seed, row.evaluation.protocol);
        assert_eq!(again.status, row.evaluation.status);
        assert_eq!(
            again.metrics.as_ref().map(|m| m.final_ecology_hash),
            row.evaluation.metrics.as_ref().map(|m| m.final_ecology_hash),
            "row for candidate {} did not reproduce from its recorded bits",
            row.candidate
        );
    }
    // Not asserted either way: whether this particular set of decimals round-tripped is a
    // property of the JSON writer, not of the harness. Reported so a reader knows it is real.
    println!("decimal round trip lost a bit on at least one row: {decimals_were_lossy}");

    // A malformed or truncated bit vector is refused, not guessed at.
    assert!(params::from_bit_labels(&rows[0].param_bits[..2]).is_err());
    let mut junk = rows[0].param_bits.clone();
    junk[0] = "not-hex".to_string();
    assert!(params::from_bit_labels(&junk).is_err());
}
