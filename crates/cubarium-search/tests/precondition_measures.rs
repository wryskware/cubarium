//! Workstream S's preconditioning operator, tested from its definitions
//! (`design/handoffs/ecology-v1-precondition-opus-2026-09-16.md`, deliverables 2 and 3).
//!
//! The campaign's whole claim is that a comparison arm founded at age `T` opens on **the**
//! plant-only field at age `T` — the one the operator measured and saved — and that the age-0
//! arm is the status quo and not a near-copy of it. Neither is safe to assume: the arm builds
//! its world inside `evaluate::run` and the operator builds its own, so the two are separate
//! code paths over the same two functions, and the only honest way to know they agree is to
//! run both and compare the hashes. That is what this file does, at ages small enough to run
//! in a test.
//!
//! 1. **The operator is plant-only, and exactly so.** No population, no withdrawal in any
//!    reading, and the core plant record's own identity closes.
//! 2. **The saved state is the state.** Every age's snapshot decodes back to the hash the
//!    operator recorded, and founding the roster onto the decoded state reproduces the
//!    founding hash — so the files under `states/` are usable evidence, not decoration.
//! 3. **Age 0 is the constructor.** The operator's founding hash at age 0 is the hash of the
//!    world `World::new` builds from the untouched config.
//! 4. **The arm opens on the operator's state.** `founding_state_hash` agrees across the two
//!    paths at every age tested.
//! 5. **The status-quo arm is the shipped path.** `precondition: Some(0)` and the shipped
//!    `None` produce the same metrics and the same `final_state_hash`.
//! 6. **The readings tile the run** and the window measures are differences over a window.
//! 7. **The two exclusive options refuse each other by name.**

use std::path::PathBuf;

use cubarium_core::World;
use cubarium_core::snapshot::{decode_snapshot, state_hash};
use cubarium_search::calibrate::candidate;
use cubarium_search::evaluate::{Protocol, RunOptions, Status, evaluate_with};
use cubarium_search::precondition::{self, FieldRun};

const SEED: u64 = 1_001;
/// Small enough to run in a test, large enough that the field has genuinely moved: 600 ticks
/// is one window boundary and 1,200 is two.
const AGES: [u64; 3] = [0, 600, 1_200];

fn dir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("precondition-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("the test's own directory");
    d
}

fn operator(name: &str) -> (FieldRun, PathBuf) {
    let out = dir(name);
    let c = candidate("baseline").expect("baseline is a declared candidate");
    let run = precondition::field_run(c, SEED, &AGES, &out).expect("the operator runs");
    (run, out)
}

// --- 1. plant-only, exactly ---------------------------------------------------------------

#[test]
fn the_operator_runs_the_plants_alone_and_its_accounting_closes() {
    let (run, _out) = operator("plant-only");
    assert_eq!(run.ticks, 1_200, "it ran to the last declared age and no further");
    assert!(run.watched_cells > 0, "the §11 seeding gives the counter a denominator");
    assert!(
        run.max_identity_residual.abs() < 1e-9,
        "the per-cell plant identity must close: {}",
        run.max_identity_residual
    );
    for r in &run.readings {
        assert_eq!(
            r.withdrawal, 0.0,
            "no mouth exists, so the exact withdrawal at tick {} must be exactly zero",
            r.tick
        );
    }
    assert!(
        run.readings.iter().any(|r| r.income > 0.0),
        "the stands did draw income; a run that measured nothing would pass everything else"
    );
}

// --- 2 and 3. the saved states, and age 0 -------------------------------------------------

#[test]
fn every_saved_state_is_the_state_it_claims_to_be() {
    let (run, out) = operator("saved-states");
    assert_eq!(run.ages.len(), AGES.len(), "one saved state per declared age");
    for age in &run.ages {
        assert_eq!(
            age.snapshot_state_hash, age.plant_state_hash,
            "age {}: the file decodes back to the state that was saved",
            age.age
        );
        let bytes = std::fs::read(out.join(&age.snapshot)).expect("the saved state is on disk");
        assert_eq!(bytes.len() as u64, age.snapshot_bytes, "and is the size recorded");

        // Founding onto the decoded state reproduces the founding hash: the saved file is
        // enough, on its own, to rebuild the world a comparison arm opens on.
        let (_, state) = decode_snapshot(&bytes).expect("a saved state decodes");
        let mut w = World::from_state(state).expect("and rebuilds");
        w.state.config.founders = cubarium_search::evaluate::base_config(SEED).founders;
        w.found_roster().expect("an empty preconditioned world accepts the roster");
        assert_eq!(
            state_hash(&w.state),
            age.founding_state_hash,
            "age {}: founding onto the saved state is the founding the operator recorded",
            age.age
        );
    }
}

#[test]
fn founding_at_age_zero_is_the_constructors_own_world() {
    let (run, _out) = operator("age-zero");
    let zero = run.ages.iter().find(|a| a.age == 0).expect("age 0 was declared");
    let reference = World::new(cubarium_search::evaluate::base_config(SEED))
        .expect("the shipped defaults build a world");
    assert_eq!(
        zero.founding_state_hash,
        state_hash(&reference.state),
        "the status-quo arm's opening is the world the constructor builds, bit for bit"
    );
    let later = run.ages.iter().find(|a| a.age == 1_200).expect("age 1,200 was declared");
    assert_ne!(
        later.founding_state_hash, zero.founding_state_hash,
        "and a later age is a different world, or the comparison compares nothing"
    );
}

// --- 4. the arm opens on the operator's state ---------------------------------------------

#[test]
fn a_comparison_arm_opens_on_the_state_the_operator_saved() {
    let (run, _out) = operator("arm-opening");
    let c = candidate("baseline").expect("baseline is a declared candidate");
    let values = c.vector().expect("its vector builds");
    for age in &run.ages {
        let evaluation = evaluate_with(
            &values,
            SEED,
            Protocol {
                horizon_ticks: 2_000,
                sample_every: 100,
                apex_founders: 0,
                apex_introduce_tick: 0,
            },
            RunOptions {
                ledger: true,
                plant_record: true,
                no_animals: false,
                precondition: Some(age.age),
            },
        );
        assert_eq!(evaluation.status, Status::Completed, "the arm ran: {:?}", evaluation.reason);
        assert_eq!(
            evaluation.founding_state_hash,
            Some(age.founding_state_hash),
            "age {}: the arm's opening is the operator's saved state",
            age.age
        );
        let opening = evaluation.opening.as_ref().expect("a preconditioned arm records its hour");
        assert_eq!(
            opening.first().map(|o| o.ticks_since_founding),
            Some(0),
            "the trajectory opens at the founding, whatever tick the world is at"
        );
        assert!(
            opening.windows(2).all(|w| w[0].ticks_since_founding < w[1].ticks_since_founding),
            "and advances"
        );
        assert!(
            opening.last().is_some_and(|o| o.population > 0),
            "age {}: the cohort is in the world",
            age.age
        );
    }
}

// --- 5. the status quo is the shipped path ------------------------------------------------

#[test]
fn founding_at_age_zero_through_the_door_reproduces_the_shipped_run() {
    let c = candidate("baseline").expect("baseline is a declared candidate");
    let values = c.vector().expect("its vector builds");
    let protocol = Protocol {
        horizon_ticks: 3_000,
        sample_every: 100,
        apex_founders: 0,
        apex_introduce_tick: 0,
    };
    let base = RunOptions { ledger: true, plant_record: true, no_animals: false, precondition: None };
    let shipped = evaluate_with(&values, SEED, protocol, base);
    let through_door = evaluate_with(&values, SEED, protocol, RunOptions { precondition: Some(0), ..base });

    assert_eq!(shipped.status, Status::Completed);
    assert_eq!(through_door.status, Status::Completed);
    let a = shipped.metrics.as_ref().expect("metrics");
    let b = through_door.metrics.as_ref().expect("metrics");
    assert_eq!(
        a.final_state_hash, b.final_state_hash,
        "the door's age-0 arm must be the world the shipped path runs, or the campaign's \
         status quo is not the status quo"
    );
    assert_eq!(a, b, "and every component metric with it");
    assert_eq!(shipped.movement, through_door.movement, "and every movement measure");
    assert!(shipped.opening.is_none(), "the shipped path records no opening trajectory");
    assert!(through_door.opening.is_some(), "the preconditioned path does");
}

// --- 6. the readings ----------------------------------------------------------------------

#[test]
fn the_readings_tile_the_run_and_the_windows_are_differences() {
    let (run, _out) = operator("readings");
    let first = run.readings.first().expect("an opening reading");
    assert_eq!(first.tick, 0, "the first reading is the §11 seeding itself");
    assert_eq!(first.window_ticks, 0, "which has no window behind it");
    assert_eq!(first.income, 0.0, "and no flows in it");
    assert!(first.foliage.total > 0.0, "but it does hold the opening foliage");
    assert!(first.foliage_cv > 0.0, "which is not uniform across the cube");

    let ticks: Vec<u64> = run.readings.iter().map(|r| r.tick).collect();
    for age in AGES {
        assert!(ticks.contains(&age), "every declared age gets its own reading: {age}");
    }
    assert!(
        run.readings.windows(2).all(|w| w[0].tick < w[1].tick),
        "readings are emitted in order and never twice for one tick"
    );
    let last = run.readings.last().expect("a closing reading");
    assert_eq!(
        last.crossings_total,
        run.readings.iter().map(|r| r.crossings_in_window).sum::<u64>(),
        "the per-window crossings are a partition of the cumulative count"
    );
    assert!(
        last.foliage.total > 0.0 && last.alive_cells > 0,
        "the stands are alive at the end of a short run"
    );
}

// --- 7. the refusals ----------------------------------------------------------------------

#[test]
fn a_run_cannot_both_found_nobody_and_found_after_a_prefix() {
    let c = candidate("baseline").expect("baseline is a declared candidate");
    let values = c.vector().expect("its vector builds");
    let evaluation = evaluate_with(
        &values,
        SEED,
        Protocol { horizon_ticks: 600, sample_every: 100, apex_founders: 0, apex_introduce_tick: 0 },
        RunOptions {
            ledger: false,
            plant_record: false,
            no_animals: true,
            precondition: Some(600),
        },
    );
    assert_eq!(evaluation.status, Status::Invalid, "the two options exclude each other");
    assert!(
        evaluation.reason.unwrap_or_default().contains("found no animals"),
        "and the refusal says which two"
    );
}

#[test]
fn an_age_off_the_probe_cadence_is_refused_before_anything_runs() {
    let out = dir("bad-age");
    let e = precondition::run_field(
        &["baseline".to_string()],
        cubarium_search::calibrate::SeedSet::Training,
        1,
        &[0, 1_001],
        1,
        60,
        &out,
    )
    .expect_err("an age that cannot be read at must be refused");
    assert!(e.contains("probe cadence"), "and the refusal must say why: {e}");
}
