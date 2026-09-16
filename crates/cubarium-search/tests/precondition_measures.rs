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
                motor: cubarium_core::MotorModel::Sweep,
                pursuit_stop: cubarium_core::hunter::PursuitStop::default(),
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
    let base = RunOptions {
        ledger: true,
        plant_record: true,
        no_animals: false,
        precondition: None,
        motor: cubarium_core::MotorModel::Sweep,
        pursuit_stop: cubarium_core::hunter::PursuitStop::default(),
    };
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
            motor: cubarium_core::MotorModel::Sweep,
            pursuit_stop: cubarium_core::hunter::PursuitStop::default(),
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

// =========================================================================================
// workstream Z: the coupled grazed opening
// =========================================================================================
//
// The grazed stage's claims are different from S's and are tested from their own definitions,
// pre-registered in
// `design/7_Research/ecology-v1-grazed-opening-preregistration-2026-09-16.md`:
//
// 8.  **The status-quo arm is the constructor's world**, untouched: no burn-in, no removal,
//     and a founding hash equal to `World::new`'s. That is what makes it a reproduction of
//     S's retained age-0 rows and not a near-copy.
// 9.  **A burnt-in arm opens on a genuinely grazed field**, its removal is booked, and the
//     world's own conservation audits still close at the founding instant and at the horizon.
// 10. **The burn-in stage and the arm agree**: the opening the arm founds is the opening the
//     stage saved, by `founding_state_hash`, and the saved file decodes back to it.
// 11. **The common reference does not move with the opening**: it is the ordinary world's
//     tick-0 foliage, the same vector at every age, which is what makes the second crossing
//     count and the terminal starved cells comparable across openings at all.
// 12. **A coupled burn-in refuses an empty world by name**, because a "burn-in" with nobody
//     in it is the plant-only prefix this stage exists to be different from.

/// A burn-in small enough to run in a test, on the probe cadence, and long enough that the
/// coupled field has genuinely moved away from its §11 seeding.
const GRAZED_AGES: [u64; 2] = [600, 1_200];
const GRAZED_HORIZON: u64 = 600;

fn grazed(age: u64) -> cubarium_search::precondition::GrazedRow {
    let c = candidate("baseline").expect("baseline is a declared candidate");
    precondition::grazed_run(
        c,
        SEED,
        age,
        GRAZED_HORIZON,
        100,
        cubarium_core::hunter::PursuitStop::default(),
    )
}

// --- 8. the status-quo arm ----------------------------------------------------------------

#[test]
fn the_status_quo_arm_is_the_constructors_world_untouched() {
    let c = candidate("baseline").expect("baseline is a declared candidate");
    let row = grazed(0);
    assert!(row.completed, "the status-quo arm runs: {:?}", row.reason);
    assert!(row.burn_in.is_none(), "age 0 has no burn-in population to remove");

    let reference = World::new(c.config(SEED).expect("its config builds"))
        .expect("the shipped defaults build a world");
    assert_eq!(
        row.opening.founding_state_hash,
        state_hash(&reference.state),
        "the status-quo arm opens on the world the constructor builds, bit for bit"
    );
    let horizon = row.horizon.expect("it ran to its horizon");
    assert_eq!(horizon.ticks_run, GRAZED_HORIZON, "and for the declared number of ticks");
    assert!(horizon.max_abs_mass_residual < 1e-9, "with the material box closed");
    assert!(horizon.max_abs_water_residual < 1e-9, "and the water budget too");
    assert_eq!(
        horizon.founder_broods.founders_by_form.iter().sum::<u64>(),
        24,
        "the ordinary 24 founders are the cohort the broods are counted over"
    );
}

// --- 9. a burnt-in arm --------------------------------------------------------------------

#[test]
fn a_burnt_in_arm_opens_on_a_grazed_field_and_its_removal_is_booked() {
    let status_quo = grazed(0);
    let row = grazed(GRAZED_AGES[1]);
    assert!(row.completed, "the burnt-in arm runs: {:?}", row.reason);

    let burn = row.burn_in.as_ref().expect("a burnt-in arm removed a population");
    assert_eq!(burn.ticks, GRAZED_AGES[1], "it burnt in for the declared age");
    assert!(burn.population > 0, "and had a population to remove");
    assert_eq!(
        burn.population_by_form.iter().sum::<u32>(),
        burn.population,
        "whose composition accounts for every body"
    );
    assert!(burn.material_removed > 0.0, "the bodies were made of something");
    assert!(
        burn.mass_residual.abs() < 1e-9,
        "and the material box closes across the removal: {}",
        burn.mass_residual
    );
    assert_ne!(
        burn.coupled_state_hash, burn.emptied_state_hash,
        "the removal changed the world it was applied to"
    );

    assert_ne!(
        row.opening.founding_state_hash, status_quo.opening.founding_state_hash,
        "a burnt-in arm does not open on the constructor's world"
    );
    assert_ne!(
        row.opening.foliage, status_quo.opening.foliage,
        "the field the founders meet is the one the burn-in grazed"
    );
    let horizon = row.horizon.expect("it ran to its horizon");
    assert!(horizon.max_abs_mass_residual < 1e-9, "the horizon's material box closes too");
    assert!(horizon.max_abs_water_residual < 1e-9, "as does its water budget");
    assert!(
        horizon.depletion_split.max_identity_residual < 1e-9,
        "and the plant record's own identity: {}",
        horizon.depletion_split.max_identity_residual
    );
    assert!(
        !row.trajectory.is_empty() && row.trajectory[0].ticks_since_founding == 0,
        "the opening hour's trajectory starts at the founding"
    );
}

// --- 10. the burn-in stage and the arm agree ----------------------------------------------

#[test]
fn the_arm_opens_on_the_state_the_burn_in_stage_saved() {
    let out = dir("grazed-openings");
    let c = candidate("baseline").expect("baseline is a declared candidate");
    let run = precondition::grazed_burn_in_run(
        c,
        SEED,
        &GRAZED_AGES,
        cubarium_core::hunter::PursuitStop::default(),
        &out,
    )
    .expect("the coupled burn-in runs");
    assert_eq!(run.ages.len(), GRAZED_AGES.len(), "one saved opening per declared age");
    assert_eq!(run.removals.len(), GRAZED_AGES.len(), "and one accounted removal per age");

    for saved in &run.ages {
        // The file is the state it claims to be.
        let bytes = std::fs::read(out.join(&saved.snapshot)).expect("the saved opening is on disk");
        let (_, decoded) = decode_snapshot(&bytes).expect("it decodes");
        assert_eq!(
            state_hash(&decoded),
            saved.snapshot_state_hash,
            "the saved opening decodes back to the hash it was written under"
        );
        assert_eq!(
            saved.snapshot_state_hash, saved.founding_state_hash,
            "and what was saved is the founded opening itself"
        );
        // And the arm, which re-runs its own burn-in rather than loading the file, lands on it.
        let row = grazed(saved.age);
        assert!(row.completed, "the arm at age {} runs: {:?}", saved.age, row.reason);
        assert_eq!(
            row.opening.founding_state_hash, saved.founding_state_hash,
            "the arm at age {} opens on the state the stage saved for it",
            saved.age
        );
    }
}

// --- 11. the common reference is fixed ----------------------------------------------------

#[test]
fn the_common_reference_does_not_move_with_the_opening() {
    // Two arms of the same (candidate, seed) at different ages read their terminal starved
    // cells against the same §11 seeding. The counts may differ — that is the measurement —
    // but the denominator must not, and the cells it watches must not either.
    let a = grazed(0);
    let b = grazed(GRAZED_AGES[1]);
    assert_eq!(
        a.opening.watched_cells, b.opening.watched_cells,
        "the common reference watches the same cells whatever the opening is"
    );
    let ha = a.horizon.expect("the status-quo arm ran");
    let hb = b.horizon.expect("the burnt-in arm ran");
    assert_eq!(
        ha.crossings_common.cells_watched, hb.crossings_common.cells_watched,
        "and the common counter's denominator is the same in both"
    );
    // The arm's own reference is the thing that moves, which is exactly why both are reported.
    assert_ne!(
        a.opening.foliage, b.opening.foliage,
        "the openings the two arms' own references are taken from differ, which is the whole \
         reason a common one is needed"
    );
}

// --- 12. the refusal ----------------------------------------------------------------------

#[test]
fn a_coupled_burn_in_refuses_a_world_with_nobody_in_it() {
    // A "coupled" burn-in of a plant-only world is the plant-only prefix this stage exists to
    // be different from, so it must not be possible to run one by accident.
    let c = candidate("baseline").expect("baseline is a declared candidate");
    let mut config = c.config(SEED).expect("its config builds");
    config.founders.kinds.clear();
    config.founders.count = 0;
    let mut world = World::new(config).expect("a world with no founders is an ordinary world");
    let e = precondition::burn_in(&mut world, 20, 1_000)
        .expect_err("a burn-in with nobody in the world must refuse");
    assert!(e.contains("holds nobody"), "and the refusal must say why: {e}");
}
