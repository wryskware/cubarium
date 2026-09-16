//! Workstream Y — the depth ladder's definitions, checked before it ran.
//!
//! Every test here is written from
//! `design/7_Research/ecology-v1-depth-ladder-2026-09-16.md`'s pre-registration, which was
//! committed before this file existed, and this file was committed against a module that has
//! none of the API it names, so the authoring order is on the record rather than asserted.
//!
//! Three things are checked and nothing else is: that the **ladder is the six rungs the
//! pre-registration declares** and each is a legal genotype; that the **two new measures
//! measure what they say** — a synthetic ledger with one founder and one descendant bins
//! apart and sums back to E's own bins, and a body with known served quantities is reported
//! to the quantity; and that the **acceptance rule is Astra's rule**, read on cells of six
//! one-arm runs, which simulates nothing.

use cubarium_core::{
    BodyBudget, CARRION, CHANNEL_NAMES, CHANNELS, FOLIAGE, FRUIT, LITTER, OrganismId,
};
use cubarium_core::organism::DeathCause;
use cubarium_search::calibrate;
use cubarium_search::census::{
    self, Assessment, DEPTH_CONTROL, DEPTH_LEVELS, DEPTH_TREATMENT, DESCENDANT, FOUNDER,
    GenerationMarginAccumulator, LADDER_ARM, RunFacts, ServedProfile,
};
use cubarium_search::factorial::{GLIDER, GRAZER, Roster, SKIMMER};
use cubarium_search::movement::{CensusKey, MarginAccumulator};

const E_R: f64 = 2.0;
const DT: f64 = cubarium_core::DT;

fn world_of(candidate: &str, seed: u64) -> cubarium_core::World {
    let c = calibrate::candidate(candidate).expect("a declared candidate");
    cubarium_core::World::new(c.config(seed).expect("the candidate's config"))
        .expect("an ordinary world")
}

// ---------------------------------------------------------------------------------------
// The ladder is the six declared rungs
// ---------------------------------------------------------------------------------------

#[test]
fn the_ladder_is_the_six_declared_rungs() {
    assert_eq!(
        DEPTH_LEVELS,
        [0.10f32, 0.20, 0.30, 0.40, 0.55, 0.75],
        "the pre-registered ladder, in order"
    );
    assert_eq!(DEPTH_LEVELS[0], DEPTH_CONTROL, "the bottom rung is R's control");
    assert!(DEPTH_LEVELS.contains(&DEPTH_TREATMENT), "R's treatment is a rung of this ladder");
    for d in DEPTH_LEVELS {
        assert!((0.0..=1.0).contains(&d), "{d} is inside the genome's declared bounds");
    }
    for pair in DEPTH_LEVELS.windows(2) {
        assert!(pair[0] < pair[1], "the rungs increase, so a table reads bottom to top");
    }
}

#[test]
fn the_ends_of_the_ladder_are_roster_values_and_the_top_rung_is_nobodys() {
    let world = world_of("baseline", 1001);
    let roster = Roster::of(&world).expect("the four roster kinds");
    assert_eq!(roster.genome(SKIMMER).depth, DEPTH_LEVELS[0], "the bottom is the skimmer's own");
    assert_eq!(roster.genome(GRAZER).depth, DEPTH_LEVELS[4], "0.55 is the grazer's own");
    assert_eq!(roster.genome(GLIDER).depth, 1.0, "the glider sits at 1.00");
    assert!(
        !DEPTH_LEVELS.contains(&roster.genome(GLIDER).depth),
        "1.00 is off the ladder: it would re-import the confound one kind further up"
    );
    // 0.75 is the rung that tests `off the rim` against `into the grazer's niche`.
    let top = DEPTH_LEVELS[5];
    for kind in [SKIMMER, GRAZER, GLIDER] {
        assert_ne!(roster.genome(kind).depth, top, "0.75 is nobody's value");
    }
    // h_pref = -1 + 2*depth, the only thing `depth` decodes to.
    let cfg = world.config().organism.clone();
    let mut skimmer = roster.genome(SKIMMER).clone();
    for d in DEPTH_LEVELS {
        skimmer.depth = d;
        let p = cubarium_core::genome::decode(&skimmer, &cfg);
        assert!((p.h_pref - (-1.0 + 2.0 * f64::from(d))).abs() < 1e-6, "h_pref at {d}");
    }
}

#[test]
fn the_ladder_plan_is_seventy_two_arm_zero_cells() {
    let seeds = [1001u64, 1002, 1003, 1004, 1005, 1006];
    let jobs = census::plan(&seeds, census::LADDER_ARM, &census::DEPTH_LEVELS);
    assert_eq!(jobs.len(), 72, "6 rungs x 2 configurations x 6 seeds x 1 arm");
    assert!(jobs.iter().all(|j| j.arm == LADDER_ARM), "arm 0 only");
    assert_eq!(LADDER_ARM, 0);
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
            for depth in DEPTH_LEVELS {
                assert_eq!(
                    jobs.iter()
                        .filter(|j| j.candidate == candidate && j.seed == seed && j.depth == depth)
                        .count(),
                    1,
                    "{candidate}/{seed}/depth {depth} is run once"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// R's first cheap measure: E's margin split by generation
// ---------------------------------------------------------------------------------------

fn id(slot: u32) -> OrganismId {
    OrganismId { slot, generation: 1 }
}

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

fn skimmer_key() -> CensusKey {
    CensusKey { form: 3, diet_bin: 1, guild: 2 }
}

#[test]
fn a_synthetic_ledger_with_one_founder_and_one_descendant_bins_correctly() {
    // The founder starves: it is billed more than it ate.
    let mut founder = budget(1);
    founder.battery_credit[FOLIAGE] = 1.0;
    founder.bill_total = 5.0;
    founder.bill_paid = 4.0;
    founder.closed_tick = Some(1_000);
    founder.death_cause = Some(DeathCause::Starvation);

    // The descendant is fed: it ate more than its bill, and it is alive at the horizon.
    let mut descendant = budget(2);
    descendant.battery_credit[FOLIAGE] = 4.0;
    descendant.reserve_credit[FOLIAGE] = 3.0; // e_r = 2 -> 6.0 e
    descendant.bill_total = 2.0;
    descendant.bill_paid = 2.0;

    let mut acc = GenerationMarginAccumulator::default();
    acc.add(FOUNDER, skimmer_key(), &founder, E_R, 2_000, DT, false);
    acc.add(DESCENDANT, skimmer_key(), &descendant, E_R, 2_000, DT, true);
    let out = acc.finish();

    assert_eq!(out.bins.len(), 2, "one founder bin and one descendant bin, not one merged bin");
    let f = out.bin(FOUNDER as u8, 3, 1).expect("the founder bin");
    let d = out.bin(DESCENDANT as u8, 3, 1).expect("the descendant bin");

    assert_eq!(f.bodies, 1);
    assert_eq!(f.deaths, 1);
    assert_eq!(f.alive, 0);
    assert!((f.margin_mean - (1.0 - 5.0)).abs() < 1e-12, "food in minus owed, bill_total not paid");
    let f_seconds = 1_000.0 * DT;
    assert!((f.margin_rate_mean - (-4.0 / f_seconds)).abs() < 1e-12);
    assert!((f.recorded_seconds_mean - f_seconds).abs() < 1e-12);

    assert_eq!(d.bodies, 1);
    assert_eq!(d.deaths, 0);
    assert_eq!(d.alive, 1, "a live record is censored at `now`, not dropped");
    assert!((d.margin_mean - (4.0 + 2.0 * 3.0 - 2.0)).abs() < 1e-12);
    let d_seconds = 2_000.0 * DT;
    assert!((d.margin_rate_mean - (8.0 / d_seconds)).abs() < 1e-12);
}

#[test]
fn the_generation_split_sums_to_es_own_bins() {
    // Four bodies over two groups, two generations, so neither the grouping nor the split can
    // be right by accident.
    let mut bodies: Vec<(usize, CensusKey, BodyBudget)> = Vec::new();
    for (slot, generation, form, diet_bin) in
        [(1u32, FOUNDER, 3u8, 1u8), (2, DESCENDANT, 3, 1), (3, DESCENDANT, 3, 2), (4, FOUNDER, 0, 2)]
    {
        let mut b = budget(slot);
        b.battery_credit[FOLIAGE] = f64::from(slot);
        b.reserve_credit[LITTER] = 0.5 * f64::from(slot);
        b.bill_total = 1.5 * f64::from(slot);
        b.bill_paid = b.bill_total;
        b.growth_energy = 0.25;
        b.closed_tick = Some(500 * u64::from(slot));
        bodies.push((generation, CensusKey { form, diet_bin, guild: 2 }, b));
    }

    let mut es = MarginAccumulator::default();
    let mut mine = GenerationMarginAccumulator::default();
    for (generation, key, b) in &bodies {
        es.add(*key, b, E_R, 4_000, DT, false);
        mine.add(*generation, *key, b, E_R, 4_000, DT, false);
    }
    let es = es.finish(true);
    let mine = mine.finish();

    for bin in &es.bins {
        let split: Vec<_> =
            mine.bins.iter().filter(|b| b.form == bin.form && b.diet_bin == bin.diet_bin).collect();
        let bodies: u64 = split.iter().map(|b| b.bodies).sum();
        assert_eq!(bodies, bin.bodies, "form {} bin {}: bodies", bin.form, bin.diet_bin);
        let margin: f64 = split.iter().map(|b| b.margin_mean * b.bodies as f64).sum();
        assert!(
            (margin - bin.margin_mean * bin.bodies as f64).abs() < 1e-9,
            "form {} bin {}: the split margins sum to E's",
            bin.form,
            bin.diet_bin
        );
        let served: f64 = split.iter().map(|b| b.served_total_mean * b.bodies as f64).sum();
        assert!((served - bin.served_total_mean * bin.bodies as f64).abs() < 1e-9);
    }
    assert!(mine.bins.len() > es.bins.len(), "a split that never splits is not a split");
}

// ---------------------------------------------------------------------------------------
// R's second cheap measure: foliage and litter served, per body, by channel
// ---------------------------------------------------------------------------------------

#[test]
fn a_body_with_known_served_quantities_is_reported_to_the_quantity() {
    let mut b = budget(1);
    b.served[FOLIAGE] = 1.5;
    b.served[FRUIT] = 0.25;
    b.served[LITTER] = 3.0;
    b.served[CARRION] = 0.0;

    let mut p = ServedProfile::default();
    p.add(&b);
    assert_eq!(p.bodies, 1);
    assert!((p.mean(FOLIAGE) - 1.5).abs() < 1e-12, "the foliage it took off the field");
    assert!((p.mean(LITTER) - 3.0).abs() < 1e-12, "the litter it took off the field");
    assert!((p.mean(FRUIT) - 0.25).abs() < 1e-12);
    assert!((p.mean(CARRION) - 0.0).abs() < 1e-12);
    assert!((p.mean_total() - 4.75).abs() < 1e-12);
    assert!(
        (p.mean_total() - b.served_total()).abs() < 1e-12,
        "the profile's total is the ledger's own served_total"
    );

    // A second body halves the mean of the first and adds its own.
    let mut c = budget(2);
    c.served[FOLIAGE] = 2.5;
    p.add(&c);
    assert_eq!(p.bodies, 2);
    assert!((p.mean(FOLIAGE) - 2.0).abs() < 1e-12, "a mean over bodies, not a sum");
    assert!((p.mean(LITTER) - 1.5).abs() < 1e-12);
}

#[test]
fn the_served_channels_are_the_ledgers_own_order() {
    assert_eq!(CHANNELS, 4);
    assert_eq!((FOLIAGE, FRUIT, LITTER, CARRION), (0, 1, 2, 3), "budget.rs:61-67");
    // The names the note's columns carry are the core's own, not a second list that could
    // drift out of that order.
    assert_eq!(CHANNEL_NAMES.len(), CHANNELS);
    assert_eq!(CHANNEL_NAMES[FOLIAGE], "foliage");
    assert_eq!(CHANNEL_NAMES[LITTER], "litter");
    assert_eq!(ServedProfile::default().served.len(), CHANNELS, "one slot per stock channel");
}

#[test]
fn a_served_profile_merges_without_losing_a_body() {
    let mut a = ServedProfile::default();
    let mut b = ServedProfile::default();
    let mut x = budget(1);
    x.served[FOLIAGE] = 1.0;
    let mut y = budget(2);
    y.served[LITTER] = 3.0;
    a.add(&x);
    b.add(&y);
    a.merge(&b);
    assert_eq!(a.bodies, 2);
    assert!((a.mean(FOLIAGE) - 0.5).abs() < 1e-12);
    assert!((a.mean(LITTER) - 1.5).abs() < 1e-12);
    assert_eq!(ServedProfile::default().bodies, 0);
    assert!((ServedProfile::default().mean(FOLIAGE)).abs() < 1e-12, "no bodies is zero, not NaN");
}

// ---------------------------------------------------------------------------------------
// Astra's rule at one arm per seed, which simulates nothing
// ---------------------------------------------------------------------------------------

fn establishing(seed: u64) -> RunFacts {
    RunFacts {
        seed,
        arm: LADDER_ARM,
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

fn lost(seed: u64) -> RunFacts {
    RunFacts {
        seed,
        arm: LADDER_ARM,
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

/// One cell of this campaign: six seeds, one arm each.
fn cell(f: impl Fn(u64) -> RunFacts) -> Vec<RunFacts> {
    (1001..=1006u64).map(f).collect()
}

#[test]
fn a_seed_with_one_run_agrees_when_that_run_does() {
    // At R's three arms a seed needed two of them; at one arm per seed a fixed `two runs` rule
    // would make every seed disagree and no clause could ever hold.
    let a = Assessment::of(&cell(lost), &cell(establishing));
    assert_eq!(a.runs, 6);
    assert_eq!(a.seeds, 6);
    assert_eq!(a.runs_needed, 4, "two thirds of six");
    assert_eq!(a.seeds_needed, 5, "five sixths of six");
    assert_eq!(a.lineage.runs_agreeing, 6);
    assert_eq!(a.lineage.seeds_agreeing, 6, "one run of one arm is that seed agreeing");
    assert!(a.lineage.holds);

    // Five of six is the binding threshold, and four of six is not enough.
    let five = Assessment::of(
        &cell(lost),
        &cell(|seed| if seed <= 1005 { establishing(seed) } else { lost(seed) }),
    );
    assert_eq!(five.lineage.seeds_agreeing, 5);
    assert!(five.lineage.holds);
    let four = Assessment::of(
        &cell(lost),
        &cell(|seed| if seed <= 1004 { establishing(seed) } else { lost(seed) }),
    );
    assert_eq!(four.lineage.runs_agreeing, 4, "the run threshold alone would pass this");
    assert_eq!(four.lineage.seeds_agreeing, 4);
    assert!(!four.lineage.holds, "four worlds is not `across seeds`");
}

#[test]
fn a_rung_is_acceptable_only_when_the_lineage_holds_and_neither_m_nor_v_does() {
    let good = Assessment::of(&cell(lost), &cell(establishing));
    assert!(good.lineage.holds && !good.monoculture.holds && !good.variety_harmed.holds);
    assert!(good.acceptable(), "L and not M and not V");

    // The lineage never establishes: not acceptable, whatever else is true.
    let no_lineage = Assessment::of(&cell(lost), &cell(lost));
    assert!(!no_lineage.lineage.holds);
    assert!(!no_lineage.acceptable());

    // The lineage establishes and the grazer is taken: not acceptable. This is R's result.
    let grazer_taken = Assessment::of(
        &cell(lost),
        &cell(|seed| RunFacts { alive_by_form: [4, 8, 10, 9, 0], ..establishing(seed) }),
    );
    assert!(grazer_taken.lineage.holds);
    assert!(grazer_taken.variety_harmed.holds, "4.0 against 12.0 is 0.33x, under 0.60x");
    assert!(!grazer_taken.acceptable());

    // The lineage establishes and the world becomes one kind: not acceptable.
    let mono = Assessment::of(
        &cell(lost),
        &cell(|seed| RunFacts {
            founder_forms_alive: 1,
            top_form_share: 0.95,
            alive_by_form: [12, 8, 10, 90, 0],
            ..establishing(seed)
        }),
    );
    assert!(mono.monoculture.holds);
    assert!(!mono.acceptable());
}

#[test]
fn the_grazer_ratio_is_reported_beside_every_rung() {
    let a = Assessment::of(
        &cell(lost),
        &cell(|seed| RunFacts { alive_by_form: [9, 8, 10, 9, 0], ..establishing(seed) }),
    );
    assert!((a.grazer_control_mean - 12.0).abs() < 1e-12);
    assert!((a.grazer_mean - 9.0).abs() < 1e-12);
    assert!((a.grazer_ratio - 0.75).abs() < 1e-12, "the number the acceptance turns on");
    assert!(!a.variety_harmed.holds, "0.75x is above the 0.60x line");
    assert!(a.acceptable());

    // A control with no grazers at all reports a ratio rather than a division by zero.
    let none = Assessment::of(
        &cell(|seed| RunFacts { alive_by_form: [0, 8, 10, 0, 0], ..lost(seed) }),
        &cell(establishing),
    );
    assert!(none.grazer_ratio.is_finite(), "no grazer in the control is not a NaN");
}

#[test]
fn f_and_d_are_reported_and_are_not_read_by_the_acceptance() {
    // R disclosed both clauses as defective and carried them unrepaired. Acceptance is Astra's
    // rule — L and not M and not V — so neither can change it.
    let base = Assessment::of(&cell(lost), &cell(establishing));
    let short_founders = Assessment::of(
        &cell(lost),
        &cell(|seed| RunFacts {
            founder_skimmer_mean_lifetime_seconds: 300.0,
            ..establishing(seed)
        }),
    );
    assert!(short_founders.rescue_gone.holds, "F: the founders live shorter than the control's");
    assert_eq!(
        base.acceptable(),
        short_founders.acceptable(),
        "F is reported, and the acceptance does not read it"
    );
    let no_drift = Assessment::of(
        &cell(lost),
        &cell(|seed| RunFacts { skimmer_bin2_entered: 0, ..establishing(seed) }),
    );
    assert!(!no_drift.diet_drift.holds);
    assert_eq!(base.acceptable(), no_drift.acceptable(), "D is evidence, not a gate");
}
