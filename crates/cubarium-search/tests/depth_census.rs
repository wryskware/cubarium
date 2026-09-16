//! Workstream R — the definitions of the depth census, checked before it ran.
//!
//! Every test here is written from
//! `design/7_Research/ecology-v1-depth-census-2026-09-16.md`'s pre-registration and was
//! committed against a stub that returns nothing, so the authoring order is on the record
//! rather than asserted.
//!
//! Three things are checked and nothing else is: that the **treatment is the one locus it
//! claims to be** (the override tests), that the **control is the ordinary harness's world**
//! (the reproduction test against `evaluate::evaluate_with`), and that **Astra's rule is the
//! rule the note says it is** (the assessment tests, which simulate nothing).

use cubarium_core::World;
use cubarium_search::calibrate;
use cubarium_search::census::{
    self, Assessment, DEPTH_BANDS, DEPTH_CONTROL, DEPTH_TREATMENT, RunFacts, Verdict,
};
use cubarium_search::evaluate::{Protocol, RunOptions, evaluate_with};
use cubarium_search::factorial::{GLIDER, GRAZER, BURROWER, Roster, SKIMMER};

fn world_of(candidate: &str, seed: u64) -> World {
    let c = calibrate::candidate(candidate).expect("a declared candidate");
    World::new(c.config(seed).expect("the candidate's config")).expect("an ordinary world")
}

/// Every body's `(genome, phenotype)`, in slot order: the thing an override must not move.
fn bodies(world: &World) -> Vec<(cubarium_core::genome::Genome, cubarium_core::genome::Phenotype)> {
    world.state.organisms.iter().map(|(_, o)| (o.genome.clone(), o.phenotype.clone())).collect()
}

// ---------------------------------------------------------------------------------------
// The treatment is one locus of the roster, and both levels are roster values
// ---------------------------------------------------------------------------------------

#[test]
fn the_two_levels_are_the_roster_skimmers_own_and_the_roster_grazers_own() {
    let world = world_of("baseline", 1001);
    let roster = Roster::of(&world).expect("the four roster kinds");
    assert_eq!(roster.genome(SKIMMER).depth, DEPTH_CONTROL, "the control is the skimmer's own");
    assert_eq!(roster.genome(GRAZER).depth, DEPTH_TREATMENT, "the treatment is the grazer's own");
    for d in [DEPTH_CONTROL, DEPTH_TREATMENT] {
        assert!((0.0..=1.0).contains(&d), "{d} is inside the genome's declared bounds");
    }
    // `h_pref = -1 + 2 * depth`: the rim against just above the equator.
    let cfg = world.config().organism.clone();
    let p_low = cubarium_core::genome::decode(roster.genome(SKIMMER), &cfg);
    let p_high = cubarium_core::genome::decode(roster.genome(GRAZER), &cfg);
    assert!((p_low.h_pref - -0.8).abs() < 1e-6, "the skimmer seeks the low rim");
    assert!((p_high.h_pref - 0.1).abs() < 1e-6, "the grazer sits just above the equator");
}

#[test]
fn the_ordinary_founding_places_the_declared_twenty_four_founders() {
    for candidate in census::CONFIGURATIONS {
        let world = world_of(candidate, 1001);
        assert_eq!(world.population(), 24, "{candidate}: the ordinary roster");
        let mut by_form = [0u64; 5];
        for (_, o) in world.state.organisms.iter() {
            by_form[usize::from(o.phenotype.form).min(4)] += 1;
        }
        assert_eq!(
            by_form,
            [10, 5, 4, 5, 0],
            "{candidate}: grazer 10 form 0, glider 5 form 1, burrower 4 form 2, skimmer 5 form 3"
        );
    }
    // The form mapping is measured, not assumed, and the same in both configurations.
    let world = world_of("fast-leaf", 1003);
    let roster = Roster::of(&world).expect("the four roster kinds");
    for form in [GRAZER, GLIDER, BURROWER, SKIMMER] {
        let _ = roster.genome(form);
    }
}

#[test]
fn a_control_override_changes_no_genome_and_no_phenotype_bit() {
    for candidate in census::CONFIGURATIONS {
        let mut world = world_of(candidate, 1002);
        let before = bodies(&world);
        let applied =
            census::apply_depth_override(&mut world, DEPTH_CONTROL).expect("the control override");
        assert_eq!(applied.bodies, 5, "{candidate}: the five roster skimmers, and only them");
        assert_eq!(applied.depth_before, DEPTH_CONTROL);
        assert_eq!(applied.depth_after, DEPTH_CONTROL);
        assert_eq!(
            bodies(&world),
            before,
            "{candidate}: writing the value that was already there moves no bit"
        );
    }
}

#[test]
fn a_treatment_override_changes_only_the_skimmers_depth_and_h_pref() {
    let mut world = world_of("fast-leaf", 1004);
    let before = bodies(&world);
    let applied =
        census::apply_depth_override(&mut world, DEPTH_TREATMENT).expect("the treatment override");
    assert_eq!(applied.bodies, 5);
    assert!((applied.h_pref_before - -0.8).abs() < 1e-6);
    assert!((applied.h_pref_after - 0.1).abs() < 1e-6);

    let after = bodies(&world);
    assert_eq!(before.len(), after.len());
    let mut moved = 0;
    for ((g0, p0), (g1, p1)) in before.iter().zip(&after) {
        if p0.form != SKIMMER {
            assert_eq!((g0, p0), (g1, p1), "a body that is not the skimmer is untouched");
            continue;
        }
        moved += 1;
        // The genome: `depth` moved, every other locus held.
        assert_eq!(g0.depth, DEPTH_CONTROL);
        assert_eq!(g1.depth, DEPTH_TREATMENT);
        let mut restored = g1.clone();
        restored.depth = g0.depth;
        assert_eq!(&restored, g0, "put `depth` back and the genome is the roster skimmer's");

        // The phenotype: `h_pref` moved, every other field held. Spelled out for the fields
        // the verdict could possibly rest on, then checked wholesale.
        assert_eq!(p0.cap_foliage, p1.cap_foliage, "no capability changed");
        assert_eq!(p0.cap_detrital, p1.cap_detrital, "no capability changed");
        assert_eq!(p0.diet, p1.diet, "no diet changed");
        assert_eq!(p0.swim, p1.swim, "`swim` is held at the roster's 1.0");
        assert_eq!(p0.mouth_rate, p1.mouth_rate, "no rate changed");
        assert_eq!(p0.maintenance, p1.maintenance, "no bill changed");
        assert_eq!(p0.structure_adult, p1.structure_adult, "no capacity changed");
        assert_eq!(p0.reserve_max, p1.reserve_max, "no capacity changed");
        assert_eq!(p0.energy_max, p1.energy_max, "no capacity changed");
        assert_eq!(p0.speed_max, p1.speed_max, "no speed changed");
        assert_eq!(p0.sense_radius, p1.sense_radius);
        assert_eq!(p0.drives, p1.drives, "`w_depth`, the gain, is held while its target moves");
        assert_eq!(p0.form, p1.form);
        let mut restored = p1.clone();
        restored.h_pref = p0.h_pref;
        assert_eq!(&restored, p0, "put `h_pref` back and the phenotype is identical");
        assert!((p0.h_pref - -0.8).abs() < 1e-6 && (p1.h_pref - 0.1).abs() < 1e-6);
    }
    assert_eq!(moved, 5, "all five skimmers moved");
}

#[test]
fn a_depth_outside_the_genomes_bounds_is_refused_rather_than_clamped() {
    let mut world = world_of("baseline", 1001);
    for bad in [-0.1f32, 1.5, f32::NAN] {
        assert!(
            census::apply_depth_override(&mut world, bad).is_err(),
            "{bad} is not a legal genotype and is refused before a world runs under it"
        );
    }
    assert_eq!(bodies(&world), bodies(&world_of("baseline", 1001)), "a refusal wrote nothing");
}

// ---------------------------------------------------------------------------------------
// The control is the ordinary harness's world
// ---------------------------------------------------------------------------------------

/// The campaign runs its own loop, because the depth override has to happen between
/// `World::new` and the first `step` and `evaluate::run` has no seam there. That loop is only
/// legitimate if it *is* the ordinary one, so this compares the whole recorded result — the
/// state hash, the census, the founder broods and the ledger's margins — against
/// `evaluate_with` at the same protocol, with the override writing the value that is already
/// on the roster.
#[test]
fn a_control_run_reproduces_the_ordinary_harness_world() {
    let options = RunOptions {
        ledger: true,
        plant_record: false,
        no_animals: false,
        precondition: None,
        motor: cubarium_core::MotorModel::Sweep,
        // The shipped rule, which is what `census`'s own loop builds its worlds under: the
        // control is only a control if both sides run the same predicate.
        pursuit_stop: cubarium_core::hunter::PursuitStop::default(),
    };
    for (candidate, seed, arm, introduce) in
        [("baseline", 1001u64, 0u32, 500u64), ("fast-leaf", 1002, 2, 500)]
    {
        let protocol = Protocol {
            horizon_ticks: 2_000,
            sample_every: 100,
            apex_founders: arm,
            apex_introduce_tick: introduce,
        };
        let values =
            calibrate::candidate(candidate).expect("a declared candidate").vector().expect("a vector");
        let reference = evaluate_with(&values, seed, protocol, options);
        let m = reference.metrics.as_ref().expect("the reference completed");
        let mv = reference.movement.as_ref().expect("the reference recorded movement");

        let row = census::run_one(candidate, seed, arm, DEPTH_CONTROL, protocol)
            .expect("the control run");

        assert_eq!(row.final_state_hash, m.final_state_hash, "{candidate}/{seed}/arm {arm}: hash");
        assert_eq!(row.ticks_run, m.ticks_run, "the same horizon was actually run");
        assert_eq!(row.prey_births, m.prey_births, "the same births");
        assert_eq!(row.prey_deaths, m.prey_deaths, "the same deaths");
        assert_eq!(row.deaths_by_cause[0], m.deaths_starvation);
        assert_eq!(row.deaths_by_cause[1], m.deaths_age);
        assert_eq!(row.deaths_by_cause[3], m.deaths_predation);
        assert_eq!(row.census, mv.census, "the same variety census, cell for cell");
        assert_eq!(row.founder_broods, mv.founder_broods, "the same completed broods");
        assert_eq!(row.crossings, mv.crossings, "the same depletion/recovery crossings");
        assert_eq!(
            row.margins,
            *mv.margins.as_ref().expect("the reference ran with the ledger on"),
            "the same per-body margins, bin for bin"
        );
    }
}

#[test]
fn a_treatment_run_is_a_different_world_and_a_control_run_is_not() {
    let protocol = Protocol {
        horizon_ticks: 2_000,
        sample_every: 100,
        apex_founders: 0,
        apex_introduce_tick: 500,
    };
    let control = census::run_one("fast-leaf", 1001, 0, DEPTH_CONTROL, protocol).expect("control");
    let again = census::run_one("fast-leaf", 1001, 0, DEPTH_CONTROL, protocol).expect("control");
    let treatment =
        census::run_one("fast-leaf", 1001, 0, DEPTH_TREATMENT, protocol).expect("treatment");
    assert_eq!(control.final_state_hash, again.final_state_hash, "the control is deterministic");
    assert_ne!(
        control.final_state_hash, treatment.final_state_hash,
        "one genome value moved is a different world; a measure that could not see it would \
         not be measuring the treatment"
    );
    assert_eq!(control.override_bodies, 5);
    assert_eq!(treatment.override_bodies, 5);
}

// ---------------------------------------------------------------------------------------
// The design and the habitat bands
// ---------------------------------------------------------------------------------------

/// R's design was 2 depths × 2 configurations × 6 seeds × **3 arms**. Workstream Y widened
/// the depths to a six-rung ladder and dropped the arms to 0, so `plan` is now the ladder's
/// (`tests/depth_ladder.rs` checks it as a whole). What this test still owns is the part of
/// R's design that has to survive that widening: **R's own two levels at arm 0 are still
/// cells of the plan**, which is what makes the row-for-row reproduction of R's arm-0 rows
/// possible at all.
#[test]
fn the_plan_still_carries_rs_two_levels_at_arm_zero() {
    let seeds = [1001u64, 1002, 1003, 1004, 1005, 1006];
    let jobs = census::plan(&seeds);
    let mut keys: Vec<(String, u32, u64, u32)> = jobs
        .iter()
        .map(|j| (j.candidate.to_string(), j.depth.to_bits(), j.seed, j.arm))
        .collect();
    keys.sort();
    let unique = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), unique, "every cell appears exactly once");
    for candidate in census::CONFIGURATIONS {
        for seed in seeds {
            for depth in [DEPTH_CONTROL, DEPTH_TREATMENT] {
                assert_eq!(
                    jobs.iter()
                        .filter(|j| j.candidate == candidate
                            && j.arm == 0
                            && j.seed == seed
                            && j.depth == depth)
                        .count(),
                    1,
                    "{candidate}/{seed}/arm 0/depth {depth} is R's cell and is run once"
                );
            }
        }
    }
    assert_eq!(
        jobs.iter().filter(|j| j.depth == DEPTH_CONTROL || j.depth == DEPTH_TREATMENT).count(),
        24,
        "R's 24 arm-0 rows are the reproduction target"
    );
}

#[test]
fn the_water_depth_bands_partition_the_column_on_os_own_edges() {
    assert_eq!(DEPTH_BANDS.len(), 4);
    for pair in DEPTH_BANDS.windows(2) {
        assert!(pair[0] < pair[1], "the bands are increasing");
    }
    assert!(DEPTH_BANDS[3].is_infinite(), "the last band is open, so no depth falls outside");
    assert_eq!(census::depth_band(0.0), 0, "bone dry");
    assert_eq!(census::depth_band(1e-3), 0, "the dry edge is inclusive");
    assert_eq!(census::depth_band(0.02), 1, "damp");
    assert_eq!(census::depth_band(0.05), 1);
    assert_eq!(census::depth_band(0.10), 2, "standing water");
    assert_eq!(census::depth_band(0.9), 3, "the deep part of a pool");
}

// ---------------------------------------------------------------------------------------
// Astra's rule, which simulates nothing
// ---------------------------------------------------------------------------------------

/// A run that establishes: skimmers alive at the horizon, descendants among them, four kinds
/// still in the world and no form holding four fifths of it.
fn establishing(seed: u64, arm: u32) -> RunFacts {
    RunFacts {
        seed,
        arm,
        skimmer_alive_final: 9,
        skimmer_births: 14,
        founder_forms_alive: 4,
        top_form_share: 0.40,
        alive_by_form: [12, 8, 10, 9, 0],
        founder_skimmer_mean_lifetime_seconds: 4_000.0,
        skimmer_entered: 40,
        skimmer_bin2_entered: 20,
    }
}

/// F's measured control: the skimmer rig is gone, everyone else is there.
fn lost(seed: u64, arm: u32) -> RunFacts {
    RunFacts {
        seed,
        arm,
        skimmer_alive_final: 0,
        skimmer_births: 3,
        founder_forms_alive: 3,
        top_form_share: 0.45,
        alive_by_form: [12, 8, 10, 0, 0],
        founder_skimmer_mean_lifetime_seconds: 900.0,
        skimmer_entered: 13,
        skimmer_bin2_entered: 1,
    }
}

fn cell(f: impl Fn(u64, u32) -> RunFacts) -> Vec<RunFacts> {
    let mut v = Vec::new();
    for seed in 1001..=1006u64 {
        for arm in 0..3u32 {
            v.push(f(seed, arm));
        }
    }
    v
}

#[test]
fn the_rule_confirms_a_lineage_that_establishes_without_a_monoculture() {
    let a = Assessment::of(&cell(lost), &cell(establishing));
    assert!(a.lineage.holds, "L: alive and breeding in 18 of 18 runs");
    assert_eq!(a.lineage.runs_agreeing, 18);
    assert_eq!(a.lineage.seeds_agreeing, 6);
    assert!(!a.monoculture.holds, "M: four kinds at the horizon, top share 0.40");
    assert!(!a.variety_harmed.holds, "V: no other kind lost ground");
    assert!(!a.rescue_gone.holds, "F: 4,000 s against the control's 900 s");
    assert_eq!(a.verdict, Verdict::Confirmed);
}

#[test]
fn the_rule_refutes_a_rescue_that_disappears_under_reproduction() {
    // The treatment behaves exactly like the control: the founding effect did not survive.
    let a = Assessment::of(&cell(lost), &cell(lost));
    assert!(a.rescue_gone.holds, "F: the founder lifetime did not move");
    assert!(!a.lineage.holds);
    assert_eq!(a.verdict, Verdict::Refuted);
}

#[test]
fn the_rule_refutes_a_treatment_that_costs_another_kind_its_horizon() {
    // The skimmer establishes, but the grazer (form 0) is gone from the horizon.
    let a = Assessment::of(
        &cell(lost),
        &cell(|seed, arm| RunFacts {
            alive_by_form: [0, 8, 10, 9, 0],
            founder_forms_alive: 3,
            ..establishing(seed, arm)
        }),
    );
    assert!(a.lineage.holds, "the lineage did establish");
    assert!(a.variety_harmed.holds, "V: a kind present in the control is absent here");
    assert_eq!(a.verdict, Verdict::Refuted, "V outranks a successful lineage");
}

#[test]
fn a_new_monoculture_is_not_a_confirmation() {
    let a = Assessment::of(
        &cell(lost),
        &cell(|seed, arm| RunFacts {
            alive_by_form: [1, 1, 1, 60, 0],
            top_form_share: 0.95,
            ..establishing(seed, arm)
        }),
    );
    assert!(a.lineage.holds);
    assert!(a.monoculture.holds, "M: one form holds 95 % of the horizon population");
    assert_ne!(a.verdict, Verdict::Confirmed);
}

#[test]
fn a_rig_that_survives_without_breeding_is_not_a_lineage() {
    let a = Assessment::of(
        &cell(lost),
        &cell(|seed, arm| RunFacts { skimmer_births: 0, ..establishing(seed, arm) }),
    );
    assert!(!a.lineage.holds, "survival of the founders is not establishment of a lineage");
    assert_eq!(a.verdict, Verdict::Partial);
}

#[test]
fn a_seed_agrees_when_two_of_its_three_arms_do() {
    // Twelve runs agree — enough for the run threshold — but they are spread two-per-seed over
    // six seeds in one arrangement and three-per-seed over four seeds in the other.
    let spread: Vec<RunFacts> = cell(|seed, arm| {
        if arm < 2 { establishing(seed, arm) } else { lost(seed, arm) }
    });
    let clumped: Vec<RunFacts> = cell(|seed, arm| {
        if seed <= 1004 { establishing(seed, arm) } else { lost(seed, arm) }
    });
    let a = Assessment::of(&cell(lost), &spread);
    assert_eq!(a.lineage.runs_agreeing, 12);
    assert_eq!(a.lineage.seeds_agreeing, 6, "two of three arms makes the seed agree");
    assert!(a.lineage.holds);

    let b = Assessment::of(&cell(lost), &clumped);
    assert_eq!(b.lineage.runs_agreeing, 12);
    assert_eq!(b.lineage.seeds_agreeing, 4, "two worlds never establish at all");
    assert!(!b.lineage.holds, "the same run count in four worlds is not `across seeds`");
}

#[test]
fn the_diet_drift_is_reported_and_is_not_part_of_the_verdict() {
    let drifting = Assessment::of(&cell(lost), &cell(establishing));
    let flat = Assessment::of(
        &cell(lost),
        &cell(|seed, arm| RunFacts { skimmer_bin2_entered: 0, ..establishing(seed, arm) }),
    );
    assert!(drifting.diet_drift.holds, "20 of 40 entrants in the foliage bin against 1 of 13");
    assert!(!flat.diet_drift.holds);
    assert_eq!(
        drifting.verdict, flat.verdict,
        "O's prediction is evidence, not a gate: the verdict does not read it"
    );
}
