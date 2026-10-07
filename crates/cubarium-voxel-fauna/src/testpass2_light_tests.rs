//! **TEST-PASS-2 (independent), BELLWING**: the lure's scent against S3b's failed-visit mark,
//! the lure's UV switched off, the naive readout by the eye, the lure's scent by its lit
//! fraction and its sleep, a lure kill through the ledgers, and a clutch from surplus waiting
//! for its full count. A child of `step` sharing the predation fixture.
//!
//! Every case builds its scene by hand and sets the config it relies on; no shipped default,
//! ratio or verdict is asserted.

use super::predation_fixture::*;
use crate::light::{self, Beacon, BeaconKind, CYAN, UV};
use crate::{Actions, Command, Fauna, FaunaConfig, Founder, MobilityMode, Pose, Reproduction, ScentChannel, StartingStores, State};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig};

const BW: Founder = Founder::Bellwing;
const LJ: Founder = Founder::Lanternjaw;
const NORTH: f64 = 0.0;

/// Remote senses every tick, learning on with no forgetting, short inspections; the lure's
/// UV and scent on (test values); marks of `mark_s`.
fn config(mark_s: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.light.learn = true;
    c.light.learn_rate = 0.3;
    c.light.forget_s = 1e9;
    c.light.visit_s = 1.0;
    c.light.mark_s = mark_s;
    c.light.lure_uv = 1.0;
    c.light.lure_scent = 1.0;
    c
}

/// A hungry bellwing hovering over the lane at `x`, facing north.
fn bellwing(fauna: &mut Fauna, world: &World, x: f64) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft { pose: Pose { x, z: LANE_Z, heading_rad: NORTH, y_m: SURFACE_M + 0.3 }, founder: BW, stores: StartingStores::FULL },
    ));
    let id = fauna.view().ledger.births - 1;
    let a = animal_mut(fauna, id);
    a.eating = true;
    a.still_ticks = 0;
    assert_eq!(a.mobility.mode, MobilityMode::Fly, "fixture: aloft");
    id
}

/// A still lanternjaw at `x` on the lane facing west, awake, its lure lit at `lit`.
fn lanternjaw(fauna: &mut Fauna, world: &World, x: f64, lit: f64) -> u64 {
    let lj = introduce(fauna, world, LJ, StartingStores::FULL);
    place(fauna, lj, x, LANE_Z, WEST);
    drive(fauna, lj, 0.0, false);
    let a = animal_mut(fauna, lj);
    a.pose.y_m = SURFACE_M;
    a.state = State::Resting;
    a.rest = None;
    a.founder_state.lure = lit;
    a.still_ticks = u64::MAX / 2;
    a.speed_m_s = 0.0;
    lj
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("alive")
}

// ------------------------------------------------------------------ the scent and the mark

/// **A failed-visit mark on a lure holds against the lure's scent** (BELLWING's scent against
/// S3b's mark: "the place is marked 'no reward here' for `mark_s` and the bellwing goes back to
/// its search"): a bellwing a metre from a still lanternjaw with its lure lit, the lure's
/// nectar scent laid round it (no flower anywhere), and the lure's place marked by a failed
/// visit. The light rule passes the marked lure by; the bellwing's answer must not lead it
/// back toward the marked place. (The control, no mark: the lure is its beacon.)
///
/// FINDING (evidence): marked, `beacon` is false and step 6 answers forward 1.0, turn -0.096
/// (straight at the lure), one scent-led sample. Marks are read only by the light rule
/// (light.rs:527); the smell step (light.rs:771-779) has no mark check. Flowers lay scent only
/// while they hold nectar; the lure lays it with none. Decision: skip a smell lead under a mark,
/// or accept that the mimic's scent overrides it.
#[test]
#[ignore = "FINDING: the lure's nectar scent leads a bellwing straight back to a lure it has marked as a failed visit; S3b's smell step reads no mark"]
fn a_failed_visit_mark_on_a_lure_holds_against_the_lures_scent() {
    let world = clock_off(501);
    let flora = Flora::new(FloraConfig::default());
    let arm = |marked: bool| {
        let mut fauna = Fauna::new(config(60.0));
        fauna.set_births_enabled(false);
        let lj_x = 4.0;
        let lj = lanternjaw(&mut fauna, &world, lj_x, 1.0);
        let (view, fv) = (world.view(), flora.view());
        for _ in 0..400 {
            fauna.scent.update(&view, &fv, &fauna.animals, &fauna.config);
        }
        let bw = bellwing(&mut fauna, &world, lj_x - 1.0);
        fauna.beacons.fixed(&view, &fv, Vec::new());
        let (lx, lz, _) = light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).expect("a lure");
        let smell_min = fauna.config.founder(BW).senses.smell_min;
        let a = animal(&fauna, bw);
        let s = fauna.scent.read_below(&view, a.pose.x, a.pose.z, a.pose.y_m)[ScentChannel::Nectar.index()];
        assert!(s.level >= smell_min && s.gx > 0.0, "fixture: it smells the lure up the gradient toward it: {s:?} (smell_min {smell_min})");
        if marked {
            let until = fauna.tick + cubarium_voxel::ticks_at_least_one(fauna.config.light.mark_s);
            fauna.percepts.entry(bw).or_default().mark(lx, lz, until);
        }
        // Facing the lure, its heuristic holding still: a walk toward it is the light's or the
        // smell's.
        animal_mut(&mut fauna, bw).pose.heading_rad = EAST;
        fauna.tick += 1;
        let i = index(&fauna, bw);
        let got = light::steer_flyer(&mut fauna, i, &view, &fv, 0.0, Actions::REST, 0.0);
        let l = fauna.percept(bw).unwrap().light;
        (got, l, fauna.ledger.scent_led_samples[BW.index()])
    };
    let (got, l, _) = arm(false);
    assert!(l.beacon && l.beacon_kind == BeaconKind::Lure.index() as u8, "control: the unmarked lure is its beacon: {l:?}");
    assert!(got.is_some(), "control: drawn");
    let (got, l, led) = arm(true);
    assert!(!l.beacon, "the marked lure is passed by: {l:?}");
    // Facing the lure: led toward it is forward with little turn.
    let toward = got.is_some_and(|(act, _)| act.forward > 0.0 && act.turn.abs() < 0.5);
    assert!(!toward, "led back toward the marked lure ({led} scent-led samples): {got:?}");
}

// ------------------------------------------------------------------ the lure's UV switched off

fn lure_beacon(power: f64) -> Beacon {
    Beacon { kind: BeaconKind::Lure, x: 1.0, z: 1.0, y_m: 1.0, sky: 0.4, power, moving: false, body: 7 }
}

/// **With `lure_uv` 0 the lure is the cyan lure to every eye**: its brightness and contrast
/// to a UV eye are a non-UV eye's, and its class is cyan; with it on, a UV eye reads the cyan
/// plus `lure_uv` × `flower_reflectance`, all × the lit fraction, and the class UV, while a
/// non-UV eye's reading is unchanged. A flower and a glowcap read the same to both eyes.
#[test]
fn with_lure_uv_zero_the_lure_is_the_cyan_lure_to_every_eye() {
    let mut cfg = config(1.0).light;
    for daylight in [0.0, 0.6] {
        for power in [0.3, 1.0] {
            let b = lure_beacon(power);
            cfg.lure_uv = 0.0;
            assert_eq!(b.brightness_seen(&cfg, daylight, true), b.brightness_seen(&cfg, daylight, false), "lure_uv 0: one brightness");
            assert_eq!(b.contrast_seen(&cfg, daylight, true), b.contrast_seen(&cfg, daylight, false), "lure_uv 0: one contrast");
            assert_eq!(BeaconKind::Lure.class_seen(&cfg, true), CYAN, "lure_uv 0: cyan to a UV eye");
            let cyan = b.brightness_seen(&cfg, daylight, false);
            cfg.lure_uv = 0.7;
            assert_eq!(b.brightness_seen(&cfg, daylight, false), cyan, "a non-UV eye reads the cyan alone");
            let expected = (cfg.lure_emission + cfg.lure_uv * cfg.flower_reflectance) * power;
            assert!((b.brightness_seen(&cfg, daylight, true) - expected).abs() < 1e-12, "a UV eye: cyan plus the mimic's UV, by the lit fraction");
            assert_eq!(BeaconKind::Lure.class_seen(&cfg, true), UV);
            assert_eq!(BeaconKind::Lure.class_seen(&cfg, false), CYAN);
            for kind in [BeaconKind::Flower, BeaconKind::Glowcap] {
                let o = Beacon { kind, ..b };
                assert_eq!(o.brightness_seen(&cfg, daylight, true), o.brightness_seen(&cfg, daylight, false), "{kind:?}: one brightness");
            }
        }
    }
}

/// **The naive readout reads the lure's class to the eye**: with the lure's UV on, a UV-eyed
/// bellwing that has learned UV down (under `NAIVE_FRACTION` of its innate value) is not naive
/// though cyan is innate, and one that has learned cyan down is; with the lure's UV off, the
/// reverse.
#[test]
fn the_naive_readout_reads_the_lures_class_to_the_eye() {
    let world = clock_off(502);
    let check = |lure_uv: f64, learned: usize| -> bool {
        let mut c = config(1.0);
        c.light.lure_uv = lure_uv;
        let mut fauna = Fauna::new(c);
        let bw = bellwing(&mut fauna, &world, 3.0);
        assert!(fauna.config.founder(BW).senses.uv, "fixture: a UV eye");
        let cfg = fauna.config.light;
        let innate = if learned == UV { cfg.uv_innate } else { cfg.cyan_innate };
        let now = fauna.tick;
        let p = fauna.percepts.entry(bw).or_default();
        p.expect = [0.0; light::CLASSES];
        p.expect[learned] = 0.25 * light::NAIVE_FRACTION * innate - innate;
        p.expect_tick = now;
        light::naive(&fauna, bw, BW)
    };
    assert!(!check(1.0, UV), "mimic on, UV learned down: not naive");
    assert!(check(1.0, CYAN), "mimic on, cyan learned down: naive (UV innate)");
    assert!(check(0.0, UV), "mimic off, UV learned down: naive (cyan innate)");
    assert!(!check(0.0, CYAN), "mimic off, cyan learned down: not naive");
}

// ------------------------------------------------------------------ the lure's scent

/// The nectar level over a still lanternjaw after `updates` field updates.
fn lure_smell(lit: f64, asleep: bool, updates: usize) -> f64 {
    let world = clock_off(503);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(1.0));
    let lj = lanternjaw(&mut fauna, &world, 3.0, lit);
    if asleep {
        animal_mut(&mut fauna, lj).state = State::Asleep;
    }
    let (view, fv) = (world.view(), flora.view());
    for _ in 0..updates {
        fauna.scent.update(&view, &fv, &fauna.animals, &fauna.config);
    }
    let a = animal(&fauna, lj);
    fauna.scent.read_below(&view, a.pose.x, a.pose.z, a.pose.y_m + 1.0)[ScentChannel::Nectar.index()].level
}

/// **The lure's scent is laid by its lit fraction and not in its sleep**: half lit, a still
/// lanternjaw lays half the nectar scent of a fully lit one (read over it after the same
/// updates); asleep with its lure still lit, none.
#[test]
fn the_lures_scent_is_laid_by_its_lit_fraction_and_not_in_its_sleep() {
    let full = lure_smell(1.0, false, 6);
    let half = lure_smell(0.5, false, 6);
    assert!(full > 0.0, "fixture: a lit lure lays scent");
    assert!((half / full - 0.5).abs() < 1e-9, "half lit: {half} of {full}");
    assert_eq!(lure_smell(1.0, true, 6), 0.0, "asleep: none");
}

// ------------------------------------------------------------------ a lure kill

/// The lure-kill scene: at midnight, a hungry bellwing on its heuristic hovering 1.5 m from a
/// lanternjaw on its own heuristic (its ambush, the lure lit in the dark, no patrol). Returns
/// the fauna once the bellwing is gone (within a minute), with the bellwing's visit as it last
/// stood before (whether a lure visit had been opened).
fn lure_kill() -> (Fauna, bool) {
    let world = at_hour(0.0, 504);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config(1.0);
    for f in Founder::ALL {
        c.founders[f.index()].active = crate::Activity::Always;
    }
    c.hunt.patrol = false;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let lj = lanternjaw(&mut fauna, &world, 4.0, 1.0);
    let teachers = fauna.config.teachers;
    fauna.controllers.set(lj, crate::controller::heuristic(LJ, teachers));
    animal_mut(&mut fauna, lj).eating = true;
    let bw = bellwing(&mut fauna, &world, 2.5);
    fauna.controllers.set(bw, crate::controller::heuristic(BW, teachers));
    let lure = BeaconKind::Lure.index() as u8;
    let mut visited = false;
    let killed = step_until(&mut fauna, &world, &mut flora, cubarium_voxel::ticks_at_least_one(60.0) as u32, |f| {
        visited |= f.percept(bw).is_some_and(|p| p.visit.active && p.visit.kind == lure);
        f.view().animal(bw).is_none()
    });
    assert!(killed.is_some(), "fixture: the bellwing is taken within a minute (visits {:?})", fauna.ledger.beacon_visits[BW.index()]);
    assert!(fauna.ledger.hits_by_founder[LJ.index()] >= 1, "fixture: by the lanternjaw's strike");
    (fauna, visited)
}

/// **The ledgers close through a lure kill**: the bellwing comes to the lit lure and the
/// lanternjaw kills it; the fauna's organic matter, mineral and energy match the ledger.
#[test]
fn the_ledgers_close_through_a_lure_kill() {
    let (fauna, visited) = lure_kill();
    assert!(visited, "fixture: it inspected the lure");
    assert_closes(&fauna, "after the lure kill");
}

/// **A bellwing killed inspecting a lure is booked a lure kill** (`FaunaLedger::lure_kills`,
/// the readout BELLWING's table reads): the same kill, the bellwing's lure visit open when the
/// strike began.
///
/// FINDING (evidence, traced): the visit opens at the windup (kind lure); at the lunge the loom
/// (0.08 -> 0.42) closes it through `rewarded` (light.rs:715-719) and the lunging lure is no
/// still beacon; the kill reads `drawn_to_lure` false (step.rs:2606, light.rs:837), so
/// `lure_kills` stays 0. Decision: remember the lure draw at the strike's start for the readout.
#[test]
#[ignore = "FINDING: a bellwing killed at a lure is not booked a lure kill: the lunge's loom closes its visit and the lunging lure is no still beacon, so drawn_to_lure is false at the kill"]
fn a_bellwing_killed_inspecting_a_lure_is_booked_a_lure_kill() {
    let (fauna, visited) = lure_kill();
    assert!(visited, "fixture: it inspected the lure");
    assert!(fauna.ledger.lure_kills.iter().sum::<u64>() >= 1, "not booked a lure kill: {:?}", fauna.ledger.lure_kills);
}

// ------------------------------------------------------------------ the clutch from surplus

/// The clutch-wait scene: a lanternjaw (dormant eggs: no nest site needed) with no stomach to
/// digest, a reserve paying `eggs_paid` eggs over its floor, laying from surplus up to
/// `clutch_max` 4 with a wait of `wait_s`. Returns the tick of its first clutch (within
/// `wait_s` and two seconds) and the eggs in it.
fn first_clutch(eggs_paid: f64, wait_s: f64) -> Option<(u64, u64)> {
    let world = clock_off(505);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    c.founders[LJ.index()].active = crate::Activity::Always;
    c.founders[LJ.index()].gut.s0 = 0.0;
    let (egg, floor) = {
        let core = &mut c.founders[LJ.index()].core;
        core.reserve_cap = 10.0;
        core.reproduction = Reproduction {
            surplus_hold_s: 0.1,
            birth_interval_s: 1e6,
            clutch_max: 4,
            clutch_hold_max_s: wait_s,
            litters_per_year: 0,
            ..core.reproduction
        };
        (core.reproduction.egg_organic, core.reproduction.surplus_floor)
    };
    let sc = c.founder(LJ).core;
    let reserve = (floor + eggs_paid * egg) / sc.reserve_of(sc.body_max);
    assert!(reserve < 1.0, "fixture: the reserve fits the cap");
    let mut fauna = Fauna::new(c);
    let lj = introduce(&mut fauna, &world, LJ, StartingStores { body: 1.0, reserve });
    drive(&mut fauna, lj, 0.0, false);
    let n = cubarium_voxel::ticks_at_least_one(wait_s + 2.0) as u32;
    let t = step_until(&mut fauna, &world, &mut flora, n, |f| f.ledger.clutches_laid > 0)?;
    Some((u64::from(t), fauna.ledger.eggs_laid))
}

/// **A clutch short of `clutch_max` waits `clutch_hold_max_s`, and a full one does not**: a
/// lanternjaw whose reserve pays two and a half eggs lays its two no sooner than the wait
/// after its hold; one whose reserve pays the full four lays them as soon as its hold is met.
#[test]
fn a_short_clutch_waits_clutch_hold_max_s_and_a_full_one_does_not() {
    let wait_s = 2.0;
    let wait = cubarium_voxel::ticks_at_least_one(wait_s);
    let (t, eggs) = first_clutch(2.5, wait_s).expect("the short clutch is laid at the wait's end");
    assert!(t >= wait, "the short clutch laid after {t} ticks, the wait {wait}");
    assert_eq!(eggs, 2, "what the reserve pays");
    let (t, eggs) = first_clutch(4.5, wait_s).expect("the full clutch is laid");
    assert!(t < wait, "the full clutch waited {t} ticks");
    assert_eq!(eggs, 4, "the full clutch");
}
