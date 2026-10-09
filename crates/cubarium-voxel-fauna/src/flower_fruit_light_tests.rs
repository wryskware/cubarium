//! **FLOWER-FRUIT, the light** (Wrysk 2026-10-09, `design/training-plan-2026-10-08.md` §6
//! item 20), written from the brief:
//!
//! - Nectar is near-IR reflective: with its nectar drained a flower's brightness to the eye
//!   falls; full, it is the old value (`flower_reflectance × sky × daylight`). The lure still
//!   reads as a full flower in full light.
//! - Glowcaps draw a bellwing only up close: an unlearned bellwing is drawn to a glowcap
//!   within its `cyan_m`, not to one beyond it (that it would otherwise see), and one that
//!   just failed on a glowcap is not drawn until its cyan expectation relaxes.
//!
//! A child of `step`, sharing the predation fixture. Distances and times come from the
//! config; the spec's inequalities (a range under the light range, a nectar share above 0)
//! are the only defaults asserted.

use super::predation_fixture::*;
use crate::light::{self, Beacon, BeaconKind};
use crate::{Actions, Command, Fauna, FaunaConfig, Founder, MobilityMode, Pose, StartingStores};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species};

const BW: Founder = Founder::Bellwing;
const NORTH: f64 = 0.0;
const WANDER: Actions = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
const MID_X: f64 = 3.0;

fn flower(power: f64, sky: f64) -> Beacon {
    Beacon { kind: BeaconKind::Flower, x: 0.0, z: 0.0, y_m: 1.0, sky, power, moving: false, body: 0 }
}

// ------------------------------------------------------------------ nectar reflects near-IR

#[test]
fn a_drained_flower_is_dimmer_a_full_one_is_as_before_and_the_lure_reads_as_a_full_flower() {
    let cfg = FaunaConfig::default().light;
    assert!(cfg.nectar_nir_share > 0.0, "the spec: nectar reflects near-IR");
    for (sky, day) in [(1.0, 1.0), (0.6, 0.8), (0.3, 0.25)] {
        let old = cfg.flower_reflectance * sky * day;
        let full = flower(1.0, sky).brightness_seen(&cfg, day, true);
        assert!((full - old).abs() <= 1e-12, "full: the old value, {full} against {old}");
        let mut was = full;
        for fill in [0.8, 0.5, 0.2, 0.01] {
            let b = flower(fill, sky).brightness_seen(&cfg, day, true);
            assert!(b < was && b > 0.0, "drained to {fill}: dimmer ({b} after {was})");
            was = b;
        }
    }
    // Dark at night, full or not.
    assert_eq!(flower(1.0, 1.0).brightness_seen(&cfg, 0.0, true), 0.0);
    // The lure: its near-IR is `lure_nir` × a full flower's in full light, whatever it shows.
    let lure = Beacon { kind: BeaconKind::Lure, power: 1.0, ..flower(1.0, 0.0) };
    let mimic = lure.brightness_seen(&cfg, 0.0, true) - lure.brightness_seen(&cfg, 0.0, false);
    let full_flower = flower(1.0, 1.0).brightness_seen(&cfg, 1.0, true);
    assert!((mimic - cfg.lure_nir * full_flower).abs() <= 1e-12, "the lure's near-IR {mimic}: a full flower's {full_flower}");
}

/// The beacons built from the plant layer carry a flower's nectar fill: a bloomcrown full of
/// nectar shows fill 1, drained to a part of `nectar_full` it shows that part, and is dimmer.
#[test]
fn a_built_flower_beacon_carries_its_nectar_fill() {
    let mut world = clock_off(71);
    let mut fc = FloraConfig::default();
    fc.bloomcrown.nectar_per_s = 1.0;
    let full_n = fc.bloomcrown.nectar_cap;
    let mut flora = Flora::new(fc);
    let wood = flora.config().bloomcrown.wood_max;
    assert!(flora.apply(&world, FloraCommand::Seed { x: 4, z: 2, species: Species::Bloomcrown, wood }));
    let site = flora.view().stands[0].site;
    for _ in 0..200 {
        flora.step(&mut world);
    }
    let cfg = FaunaConfig::default();
    let nectar_full = cfg.scent.nectar_full;
    assert!(nectar_full > 0.0 && full_n >= nectar_full, "fixture: the flower fills to a full flower's nectar");
    assert!((flora.view().stand_at(site).expect("the bloomcrown").nectar - full_n).abs() <= 1e-12, "fixture: full");
    let fill_of = |flora: &Flora| {
        let (list, _) = light::Beacons::build(&world.view(), &flora.view(), nectar_full);
        let b = *list.iter().find(|b| b.kind == BeaconKind::Flower).expect("the flower");
        (b.power, b.brightness_seen(&cfg.light, 1.0, true))
    };
    let (p_full, b_full) = fill_of(&flora);
    assert_eq!(p_full, 1.0);
    let keep = 0.3 * nectar_full;
    let n = flora.view().stand_at(site).expect("the bloomcrown").nectar;
    assert!(flora.take_stand_nectar(site, n - keep).is_some());
    let (p, b) = fill_of(&flora);
    assert!((p - 0.3).abs() <= 1e-9, "fill {p}");
    assert!(b < b_full, "drained, dimmer: {b} against {b_full}");
}

// ------------------------------------------------------------------ glowcaps only up close

/// The shipped light and senses, read every tick, with a short inspection and mark so a
/// failed visit is quick and its mark runs out.
fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.light.visit_s = 1.0;
    c.light.mark_s = 1.0;
    c
}

fn bellwing(fauna: &mut Fauna, world: &World) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft { pose: Pose { x: MID_X, z: LANE_Z, heading_rad: NORTH, y_m: SURFACE_M + 0.3 }, founder: BW, stores: StartingStores::FULL },
    ));
    let id = fauna.view().ledger.births - 1;
    let a = animal_mut(fauna, id);
    a.eating = true;
    a.still_ticks = 0;
    assert_eq!(a.mobility.mode, MobilityMode::Fly, "fixture: aloft");
    id
}

fn eye_y(fauna: &Fauna, id: u64) -> f64 {
    let a = animal(fauna, id);
    a.pose.y_m + crate::body_of(&fauna.config, &a).expect("a body").eye_m
}

/// A glowcap in deep shade `d` metres to the bellwing's left at its eye, alone in the scene.
fn glowcap_at(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, d: f64) -> Beacon {
    let g = Beacon { kind: BeaconKind::Glowcap, x: MID_X - d, z: LANE_Z, y_m: eye_y(fauna, id), sky: 0.0, power: 1.0, moving: false, body: 0 };
    fauna.beacons.fixed(&world.view(), &flora.view(), vec![g]);
    g
}

/// Put the bellwing back on its mark facing north, step its light steer once (the clock
/// advanced first); whether a glowcap draws it.
fn drawn(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64) -> bool {
    {
        let a = animal_mut(fauna, id);
        a.pose.x = MID_X;
        a.pose.z = LANE_Z;
        a.pose.heading_rad = NORTH;
        a.still_ticks = 0;
    }
    fauna.tick += 1;
    let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
    let _ = light::steer_flyer(fauna, i, &world.view(), &flora.view(), 1.0, WANDER, 0.0);
    let l = fauna.percept(id).expect("a record").light;
    l.beacon && l.beacon_kind == BeaconKind::Glowcap.index() as u8
}

#[test]
fn an_unlearned_bellwing_is_drawn_to_a_glowcap_close_by_but_not_beyond_its_cyan_range() {
    let world = clock_off(73);
    let flora = Flora::new(FloraConfig::default());
    let c = config();
    let senses = c.founder(BW).senses;
    assert!(senses.cyan_m > 0.0 && senses.cyan_m < senses.light_m, "the spec: a cyan range inside the light range");
    let near = 0.8 * senses.cyan_m;
    let far = 0.5 * (senses.cyan_m + senses.light_m);
    // Close by: drawn.
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world);
    glowcap_at(&mut fauna, &world, &flora, bw, near);
    assert!(drawn(&mut fauna, &world, &flora, bw), "a glowcap {near} m away draws an unlearned bellwing");
    // Beyond the cyan range, inside the light range and bright enough to be read: not drawn.
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world);
    let g = glowcap_at(&mut fauna, &world, &flora, bw, far);
    assert!(light::received(&c.light, g.contrast(&c.light, 1.0), far) >= senses.light_min, "fixture: it would be read");
    assert!(!drawn(&mut fauna, &world, &flora, bw), "a glowcap {far} m away does not draw it");
    // Control: with the cyan range off it would.
    let mut off = c;
    off.founders[BW.index()].senses.cyan_m = 0.0;
    let mut fauna = Fauna::new(off);
    let bw = bellwing(&mut fauna, &world);
    glowcap_at(&mut fauna, &world, &flora, bw, far);
    assert!(drawn(&mut fauna, &world, &flora, bw), "control: without the range it is drawn");
}

/// One failed glowcap visit and the bellwing passes glowcaps by — a fresh one close by, once
/// the failed place's mark has run out — until its cyan expectation relaxes back over
/// `expect_min`, the time read from the config's own law; then the glowcap draws it again.
#[test]
fn a_bellwing_that_just_failed_on_a_glowcap_is_not_drawn_until_its_expectation_relaxes() {
    let world = clock_off(79);
    let flora = Flora::new(FloraConfig::default());
    let c = config();
    let near = 0.8 * c.founder(BW).senses.cyan_m;
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world);
    let g = glowcap_at(&mut fauna, &world, &flora, bw, near);
    assert!(drawn(&mut fauna, &world, &flora, bw), "fixture: unlearned, it is drawn");
    // The failed visit: on the glowcap, inspecting it for `visit_s` with no sip.
    {
        let a = animal_mut(&mut fauna, bw);
        a.pose.x = g.x;
        a.still_ticks = 0;
    }
    let failed0: u64 = fauna.ledger.failed_visits[BW.index()].iter().sum();
    let i = fauna.animals.iter().position(|a| a.id == bw).expect("alive");
    fauna.tick += 1;
    assert!(light::steer_flyer(&mut fauna, i, &world.view(), &flora.view(), 1.0, WANDER, 0.0).is_some(), "inspecting");
    fauna.tick += cubarium_voxel::ticks_at_least_one(c.light.visit_s);
    let _ = light::steer_flyer(&mut fauna, i, &world.view(), &flora.view(), 1.0, WANDER, 0.0);
    let failed: u64 = fauna.ledger.failed_visits[BW.index()].iter().sum();
    assert_eq!(failed, failed0 + 1, "the visit failed");
    let t0 = fauna.tick;
    let p = *fauna.percept(bw).expect("a record");
    let e0 = light::expectation(&c.light, &p, BeaconKind::Glowcap, t0);
    assert!(e0 < c.light.expect_min, "the spec: one failure is enough to pass glowcaps by ({e0})");
    // When the offset has relaxed to `expect_min`: innate + (e0 − innate)·exp(−t/τ).
    let innate = c.light.cyan_innate;
    let tau = c.light.forget_s * f64::from(crate::TICK_HZ);
    let t_back = (tau * ((innate - e0) / (innate - c.light.expect_min)).ln()) as u64;
    let mark = cubarium_voxel::ticks_at_least_one(c.light.mark_s);
    assert!(t_back > 2 * mark, "fixture: the mark runs out well before the expectation relaxes");
    // A fresh glowcap close by, the mark over: not drawn.
    fauna.tick = t0 + 2 * mark;
    glowcap_at(&mut fauna, &world, &flora, bw, near);
    assert!(!drawn(&mut fauna, &world, &flora, bw), "just failed: not drawn");
    fauna.tick = t0 + t_back * 9 / 10;
    assert!(!drawn(&mut fauna, &world, &flora, bw), "not yet relaxed: not drawn");
    fauna.tick = t0 + t_back + t_back / 10 + 2;
    assert!(drawn(&mut fauna, &world, &flora, bw), "relaxed: drawn again");
}
