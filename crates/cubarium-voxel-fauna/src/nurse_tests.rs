//! **SP-NURSE, from the spec** (Wrysk 2026-10-11: "sure lets add nursing"): a dependent young
//! within reach of its living mother gains reserve as she loses it, with the three currencies
//! adding up (received = efficiency × given, the rest respired); she never goes below her
//! floor; nothing passes past weaning, beyond the radius, to a young not hers, or after she
//! dies; twins share by room; a nursed young grows more than an un-nursed one; a dependent
//! young that senses its mother is pulled toward her rather than the troop; and with nursing
//! off the step is unchanged.
//!
//! Every scene sets the knobs it relies on; no shipped number, ratio or world hash is asserted.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use crate::controller::Seek;
use crate::{Actions, Activity, Fauna, FaunaConfig, Founder, Herding, Nursing, SenseTable, StartingStores, State, Vigilance};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig};
use fixture::*;

const SP: Founder = Founder::Seedporter;
const NORTH: f64 = 0.0;
const WALK: Actions = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
const EFF: f64 = 0.7;
const FLOOR: f64 = 0.3;
const RADIUS: f64 = 0.5;
const WEAN_S: f64 = 1000.0;

/// The seedporter's nursing for these scenes: a rate that never binds unless a case lowers it.
fn nursing() -> Nursing {
    Nursing {
        weaning_age_s: WEAN_S,
        nurse_radius_m: RADIUS,
        nurse_rate_per_s: 1.0,
        nurse_reserve_floor: FLOOR,
        nurse_efficiency: EFF,
        follow_m: 0.4,
        follow_pull: 0.8,
        // The drift alone: SP-FOLLOW's walk to the mother is `follow_tests`'.
        leash_m: 0.0,
        mother_memory_s: 0.0,
    }
}

/// Every lineage awake, no ground response; the seedporter's senses live and all-round, its
/// troop pull at 2 m with half its turn, its nursing [`nursing`] edited by `edit`.
fn config(edit: impl FnOnce(&mut Nursing)) -> FaunaConfig {
    let mut c = hunting_config();
    for g in Founder::ALL {
        c.founders[g.index()].active = Activity::Always;
        c.founders[g.index()].vigilance = Vigilance::default();
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let p = &mut c.founders[SP.index()];
    p.senses = SenseTable {
        live: true,
        sight_m: 7.0,
        sight_night_m: 7.0,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: 180.0,
        kin_tracks: 8,
        ..SenseTable::default()
    };
    p.herding = Herding { comfort_m: 2.0, pull: 0.5, herd_max: 0, memory_s: 0.0, young_only: false, follow_moving: false };
    let mut n = nursing();
    edit(&mut n);
    p.nursing = n;
    c
}

fn cap(f: &Fauna, id: u64) -> f64 {
    let a = animal(f, id);
    crate::body::effective_config(&f.config, &a).reserve_of(a.body)
}

/// A grown seedporter at `(x, z)` with its reserve at `fill` of its cap, held still.
fn adult(f: &mut Fauna, w: &World, x: f64, z: f64, fill: f64) -> u64 {
    let id = introduce(f, w, SP, StartingStores { body: 1.0, reserve: fill });
    place(f, id, x, z, NORTH);
    drive(f, id, 0.0, false);
    id
}

/// A seedporter at `(x, z)` of `stores`, `age_s` old, the young of `mother` (if any), held
/// still. Its stores come from the introduction, so the books stay closed.
fn young_with(f: &mut Fauna, w: &World, x: f64, z: f64, mother: Option<u64>, age_s: f64, stores: StartingStores) -> u64 {
    let id = introduce(f, w, SP, stores);
    place(f, id, x, z, NORTH);
    let a = animal_mut(f, id);
    a.mother = mother;
    a.age_ticks = (age_s * f64::from(crate::TICK_HZ)) as u64;
    drive(f, id, 0.0, false);
    id
}

/// A newborn-sized young (`body_min`) with its reserve at `fill` of its cap.
fn newborn(f: &Fauna, fill: f64) -> StartingStores {
    let core = f.config.founder(SP).core;
    StartingStores { body: core.body_min / core.body_max, reserve: fill }
}

/// A newborn-sized young of `mother` at `(x, z)`, its reserve at a tenth of its cap.
fn young(f: &mut Fauna, w: &World, x: f64, z: f64, mother: Option<u64>, age_s: f64) -> u64 {
    let stores = newborn(f, 0.1);
    young_with(f, w, x, z, mother, age_s, stores)
}

/// A mother at 3 m with her reserve at 0.9 of her cap and her young `gap_m` east of her.
fn pair(edit: impl FnOnce(&mut Nursing), gap_m: f64, age_s: f64) -> (World, Fauna, u64, u64) {
    let w = clock_off(51);
    let mut f = Fauna::new(config(edit));
    f.set_births_enabled(false);
    let m = adult(&mut f, &w, 3.0, LANE_Z, 0.9);
    let y = young(&mut f, &w, 3.0 + gap_m, LANE_Z, Some(m), age_s);
    (w, f, m, y)
}

fn nurse(f: &mut Fauna, w: &World) {
    crate::nurse::nurse(f, &w.view());
}

#[test]
fn a_dependent_young_in_reach_gains_what_its_mother_loses_less_the_respired_share() {
    let (w, mut f, m, y) = pair(|_| {}, 0.2, 10.0);
    let (m0, y0, l0) = (animal(&f, m), animal(&f, y), f.ledger);
    nurse(&mut f, &w);
    let (m1, y1, l1) = (animal(&f, m), animal(&f, y), f.ledger);
    let given = m0.reserve - m1.reserve;
    let received = y1.reserve - y0.reserve;
    assert!(given > 0.0 && received > 0.0, "she loses reserve ({given}) and it gains ({received})");
    let close = |a: f64, b: f64, what: &str| assert!((a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1e-12), "{what}: {a} vs {b}");
    close(received, EFF * given, "received organic = efficiency × given");
    let (dm_mother, dm_young) = (m0.mineral - m1.mineral, y1.mineral - y0.mineral);
    let (de_mother, de_young) = (m0.energy - m1.energy, y1.energy - y0.energy);
    assert!(dm_mother > 0.0 && de_mother > 0.0, "her mineral and energy go with the reserve");
    close(dm_young, EFF * dm_mother, "received mineral = efficiency × given");
    close(de_young, EFF * de_mother, "received energy = efficiency × given");
    close(m1.gut.mineral - m0.gut.mineral, (1.0 - EFF) * dm_mother, "the rest's mineral is shed to her gut");
    close(l1.respired_out - l0.respired_out, (1.0 - EFF) * given, "the rest's organic is respired");
    close(l1.heat_out - l0.heat_out, (1.0 - EFF) * de_mother, "the rest's energy leaves as heat");
    let i = SP.index();
    close(l1.nursed_given_by_founder[i] - l0.nursed_given_by_founder[i], given, "nursed given booked");
    close(l1.nursed_received_by_founder[i] - l0.nursed_received_by_founder[i], received, "nursed received booked");
    assert_closes(&f, "after nursing");
}

#[test]
fn nursing_never_takes_the_mother_below_her_floor() {
    let (w, mut f, m, _) = pair(|n| n.nurse_rate_per_s = 1.0e3, 0.2, 10.0);
    // A young with a deep room: a full-sized body with an empty reserve.
    let y = young_with(&mut f, &w, 3.0 - 0.2, LANE_Z, Some(m), 10.0, StartingStores { body: 1.0, reserve: 0.0 });
    let floor = FLOOR * cap(&f, m);
    for _ in 0..50 {
        nurse(&mut f, &w);
        assert!(animal(&f, m).reserve >= floor * (1.0 - 1e-12), "never below the floor: {} < {floor}", animal(&f, m).reserve);
    }
    assert!((animal(&f, m).reserve - floor).abs() <= 1e-9 * floor, "fixture: she was nursed down to the floor");
    let before = animal(&f, y).reserve;
    nurse(&mut f, &w);
    assert_eq!(animal(&f, y).reserve, before, "at her floor she passes nothing");
    assert_closes(&f, "at the floor");
}

#[test]
fn nothing_passes_past_weaning_beyond_reach_to_a_young_not_hers_or_after_she_dies() {
    // Past weaning.
    let (w, mut f, m, y) = pair(|_| {}, 0.2, WEAN_S + 1.0);
    let (rm, ry) = (animal(&f, m).reserve, animal(&f, y).reserve);
    nurse(&mut f, &w);
    assert_eq!((animal(&f, m).reserve, animal(&f, y).reserve), (rm, ry), "past weaning: nothing");
    // Beyond the radius.
    let (w, mut f, m, y) = pair(|_| {}, RADIUS + 0.1, 10.0);
    let (rm, ry) = (animal(&f, m).reserve, animal(&f, y).reserve);
    nurse(&mut f, &w);
    assert_eq!((animal(&f, m).reserve, animal(&f, y).reserve), (rm, ry), "beyond the radius: nothing");
    // Not hers: beside another adult, its own mother far off.
    let (w, mut f, other, y) = pair(|_| {}, 0.2, 10.0);
    let mother = adult(&mut f, &w, 20.0, LANE_Z, 0.9);
    animal_mut(&mut f, y).mother = Some(mother);
    let (ro, rm, ry) = (animal(&f, other).reserve, animal(&f, mother).reserve, animal(&f, y).reserve);
    nurse(&mut f, &w);
    assert_eq!(
        (animal(&f, other).reserve, animal(&f, mother).reserve, animal(&f, y).reserve),
        (ro, rm, ry),
        "a young not hers: the adult beside it gives nothing (and its own is out of reach)"
    );
    // After she dies.
    let (w, mut f, m, y) = pair(|_| {}, 0.2, 10.0);
    f.animals.retain(|a| a.id != m);
    let ry = animal(&f, y).reserve;
    nurse(&mut f, &w);
    assert_eq!(animal(&f, y).reserve, ry, "after she dies: nothing");
}

#[test]
fn twins_share_by_room_and_the_emptier_gets_more() {
    let (w, mut f, m, y1) = pair(|n| n.nurse_rate_per_s = 1.0e-4, 0.2, 10.0);
    let fuller = newborn(&f, 0.6); // less room
    let y2 = young_with(&mut f, &w, 3.0 - 0.2, LANE_Z, Some(m), 10.0, fuller);
    let (a1, a2, rm) = (animal(&f, y1).reserve, animal(&f, y2).reserve, animal(&f, m).reserve);
    let (room1, room2) = (cap(&f, y1) - a1, cap(&f, y2) - a2);
    nurse(&mut f, &w);
    let (g1, g2) = (animal(&f, y1).reserve - a1, animal(&f, y2).reserve - a2);
    assert!(g1 > 0.0 && g2 > 0.0, "both twins are fed ({g1}, {g2})");
    assert!(g1 > g2, "the emptier twin gets more ({g1} vs {g2})");
    assert!(((g1 / g2) - room1 / room2).abs() <= 1e-9 * room1 / room2, "in proportion to their rooms");
    let given = rm - animal(&f, m).reserve;
    let rate = 1.0e-4 * animal(&f, m).body * cubarium_voxel::DT;
    // A difference of two reserves: cancellation, so a relative 1e-9.
    assert!(given <= rate * (1.0 + 1e-9), "the twins share one mother's rate ({given} vs {rate})");
    assert_closes(&f, "twins");
}

/// The young's body after `secs` of full steps, both held still beside each other, with its
/// mother link on or off.
fn grown_after(linked: bool, secs: u32) -> f64 {
    let (w, mut f, m, y) = pair(|n| n.nurse_rate_per_s = 1.0e-3, 0.2, 10.0);
    // Her own growth is out of the way: she is full grown ([`adult`]).
    let _ = m;
    if !linked {
        animal_mut(&mut f, y).mother = None;
    }
    let mut flora = Flora::new(FloraConfig::default());
    for _ in 0..secs * crate::TICK_HZ {
        f.step_with(&w, &mut flora, 1);
    }
    assert_closes(&f, "after the span");
    animal(&f, y).body
}

#[test]
fn a_nursed_young_grows_more_than_an_identical_unnursed_one() {
    let (nursed, alone) = (grown_after(true, 60), grown_after(false, 60));
    assert!(nursed > alone, "nursed {nursed} vs alone {alone}");
}

fn ring(w: &World) -> f64 {
    f64::from(w.view().config.width) * w.view().config.voxel_m
}

fn sense(f: &mut Fauna, w: &World) {
    f.tick += 1;
    crate::perception::sense_pass(f, &w.view(), false, 1.0, 1);
}

fn index(f: &Fauna, id: u64) -> usize {
    f.animals.iter().position(|a| a.id == id).expect("alive")
}

/// A walking young facing north at x = 6 m, its mother 2.5 m east, two troop-mates 3 m west
/// (all beyond the troop's 2 m comfort); the drift back's answer after one sensing.
fn follow_scene(linked: bool) -> Option<Actions> {
    let w = clock_off(52);
    let mut f = Fauna::new(config(|_| {}));
    f.set_births_enabled(false);
    let m = adult(&mut f, &w, 8.5, LANE_Z, 0.9);
    adult(&mut f, &w, 3.0, LANE_Z, 0.9);
    adult(&mut f, &w, 3.0, LANE_Z + 0.3, 0.9);
    let y = young(&mut f, &w, 6.0, LANE_Z, linked.then_some(m), 10.0);
    animal_mut(&mut f, y).state = State::Walking;
    sense(&mut f, &w);
    let i = index(&f, y);
    crate::herding::steer(&mut f, i, WALK, Seek::Search, ring(&w))
}

#[test]
fn a_dependent_young_away_from_its_mother_turns_toward_her_not_the_troop() {
    let go = follow_scene(true).expect("beyond the follow distance it is pulled");
    assert!(go.turn > 0.0, "it turns east, toward its mother: {go:?}");
    let troop = follow_scene(false).expect("without a mother link the troop pulls it");
    assert!(troop.turn < 0.0, "fixture: unlinked, it turns west to the troop: {troop:?}");
}

#[test]
fn a_dependent_young_away_from_its_mother_moves_toward_her() {
    // Its heuristic walks it (no food anywhere); the mother is held still 2.5 m east.
    let run = |follow: bool| -> f64 {
        let w = clock_off(53);
        let mut f = Fauna::new(config(|n| {
            if !follow {
                n.follow_m = 0.0;
            }
        }));
        f.set_births_enabled(false);
        let m = adult(&mut f, &w, 6.0, LANE_Z, 0.9);
        let y = young(&mut f, &w, 3.5, LANE_Z, Some(m), 10.0);
        let teachers = f.config.teachers;
        f.controllers.set(y, crate::heuristic(SP, teachers));
        let mut flora = Flora::new(FloraConfig::default());
        let r = ring(&w);
        let (mut sum, ticks) = (0.0, 30 * crate::TICK_HZ);
        for _ in 0..ticks {
            f.step_with(&w, &mut flora, 1);
            let (a, b) = (animal(&f, y).pose, animal(&f, m).pose);
            let (dx, dz) = crate::pose::wrapped_delta(a.x, a.z, b.x, b.z, r);
            sum += dx.hypot(dz);
        }
        sum / f64::from(ticks)
    };
    let (with, without) = (run(true), run(false));
    assert!(with < without, "following, it keeps nearer its mother ({with:.2} m vs {without:.2} m)");
    assert!(with < 2.5, "and nearer than it started ({with:.2} m)");
}

/// A mixed scene stepped `secs`: a seedporter mother and young (linked or not), a loftstrider
/// mother and calf linked, and a grazer; the layer at the end with the links cleared.
fn mixed(seedporter_on: bool, linked: bool, secs: u32) -> Fauna {
    let w = clock_off(54);
    let mut c = config(|_| {});
    if !seedporter_on {
        c.founders[SP.index()].nursing = Nursing::default();
    }
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let m = adult(&mut f, &w, 3.0, LANE_Z, 0.9);
    young(&mut f, &w, 3.2, LANE_Z, linked.then_some(m), 10.0);
    let lm = introduce(&mut f, &w, Founder::Loftstrider, StartingStores::FULL);
    place(&mut f, lm, 10.0, LANE_Z, NORTH);
    let calf = introduce(&mut f, &w, Founder::Loftstrider, StartingStores { body: 0.2, reserve: 0.1 });
    place(&mut f, calf, 10.3, LANE_Z, NORTH);
    animal_mut(&mut f, calf).mother = Some(lm);
    let g = introduce(&mut f, &w, Founder::Browser, StartingStores::FULL);
    place(&mut f, g, 14.0, LANE_Z, NORTH);
    let mut flora = Flora::new(FloraConfig::default());
    for _ in 0..secs * crate::TICK_HZ {
        f.step_with(&w, &mut flora, 1);
    }
    for a in f.animals.iter_mut() {
        a.mother = None;
    }
    f
}

#[test]
fn with_nursing_off_the_step_is_unchanged() {
    // The seedporter's table off: a mother link changes nothing; and with no seedporter link,
    // turning its table on changes nothing for any lineage (the loftstrider's calf is linked,
    // its lineage's table off).
    let (a, b) = (mixed(false, true, 20), mixed(false, false, 20));
    assert_eq!(a.animals, b.animals, "table off: the link alone changes no body");
    assert_eq!(a.ledger, b.ledger, "table off: the link alone changes no book");
    let (c, d) = (mixed(true, false, 20), mixed(false, false, 20));
    assert_eq!(c.animals, d.animals, "no seedporter link: the table on changes no body");
    assert_eq!(c.ledger, d.ledger, "no seedporter link: the table on changes no book");
}
