//! **S3b: which light pays** (`light.rs` module note, "Learning"): a bellwing keeps a reward
//! expectation per beacon class — UV (flowers) and cyan (glowcaps, lures) — that a sip raises,
//! a failed visit lowers and time relaxes back to the innate value; a still beacon draws it by
//! its level × that expectation. Also S3b's two repairs: a beacon it cannot reach is given up,
//! and a moving body looms only while it closes in; and the flight ceiling stops at the
//! world's top. A child of `step`, sharing the predation fixture.
//!
//! Every case builds its scene by hand (hand-placed beacons on the bare plain) and checks the
//! mechanism against the config it sets; no default is asserted.

use super::predation_fixture::*;
use crate::light::{self, Beacon, BeaconKind};
use crate::{Actions, Command, Fauna, FaunaConfig, Founder, MobilityMode, Pose, StartingStores, State};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig};

const BW: Founder = Founder::Bellwing;
const LJ: Founder = Founder::Lanternjaw;
const NORTH: f64 = 0.0;
const WANDER: Actions = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
/// Hover height over the plain, metres: the beacons are placed at the bellwing's eye.
const UP_M: f64 = 0.3;
/// The bellwing's place on the lane, and the two beacons' offsets either side of it.
const MID_X: f64 = 3.0;
const SIDE_M: f64 = 1.5;

/// The default tables with remote senses (and so the beacons) read every tick; learning on
/// with the given rate, forgetting time and inspection/mark times short.
fn config(learn_rate: f64, forget_s: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.light.learn = true;
    c.light.learn_rate = learn_rate;
    c.light.forget_s = forget_s;
    c.light.visit_s = 1.0;
    c.light.mark_s = 1.0;
    c
}

fn bellwing(fauna: &mut Fauna, world: &World, x: f64, z: f64, heading: f64) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft { pose: Pose { x, z, heading_rad: heading, y_m: SURFACE_M + UP_M }, founder: BW, stores: StartingStores::FULL },
    ));
    let id = fauna.view().ledger.births - 1;
    let a = animal_mut(fauna, id);
    a.eating = true;
    // Flying, not wedged (an introduced body starts still).
    a.still_ticks = 0;
    assert_eq!(a.mobility.mode, MobilityMode::Fly, "fixture: aloft");
    id
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("alive")
}

/// The eye height of the bellwing as placed: the beacons are put there, so a beacon `d` to
/// its side is `d` from its eye.
fn eye_y(fauna: &Fauna, id: u64) -> f64 {
    let a = animal(fauna, id);
    let g = crate::body_of(&fauna.config, &a).expect("a body");
    a.pose.y_m + g.eye_m
}

fn beacon(kind: BeaconKind, x: f64, y_m: f64, sky: f64) -> Beacon {
    Beacon { kind, x, z: LANE_Z, y_m, sky, power: 1.0, moving: false, body: 0 }
}

/// The scene: a UV flower in full light `SIDE_M` to the bellwing's right (+x) and a glowcap
/// in deep shade (sky 0) the same distance to its left, both at its eye.
fn scene(fauna: &mut Fauna, world: &World, flora: &Flora, bw: u64) -> (Beacon, Beacon) {
    let y = eye_y(fauna, bw);
    let flower = beacon(BeaconKind::Flower, MID_X + SIDE_M, y, 1.0);
    let glowcap = beacon(BeaconKind::Glowcap, MID_X - SIDE_M, y, 0.0);
    fauna.beacons.fixed(&world.view(), &flora.view(), vec![flower, glowcap]);
    (flower, glowcap)
}

/// The bellwing's steer this tick (the tick advanced first), its heuristic wandering.
fn steer(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, daylight: f64) -> Option<(Actions, f64)> {
    fauna.tick += 1;
    let i = index(fauna, id);
    light::steer_flyer(fauna, i, &world.view(), &flora.view(), daylight, WANDER, 0.0)
}

fn reading(fauna: &Fauna, id: u64) -> crate::perception::LightReading {
    fauna.percept(id).expect("a record").light
}

fn put(fauna: &mut Fauna, id: u64, x: f64) {
    let a = animal_mut(fauna, id);
    a.pose.x = x;
    a.pose.z = LANE_Z;
    a.still_ticks = 0;
}

/// One failed inspection of the beacon at `x`: the bellwing is put on it, inspects it for
/// `visit_s` with no sip, and the visit fails. The mark is then let run out.
fn fail_at(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, x: f64, daylight: f64) {
    put(fauna, id, x);
    let before: u64 = fauna.ledger.failed_visits[BW.index()].iter().sum();
    assert!(steer(fauna, world, flora, id, daylight).is_some(), "at the beacon: inspecting");
    fauna.tick += cubarium_voxel::ticks_at_least_one(fauna.config.light.visit_s);
    let _ = steer(fauna, world, flora, id, daylight);
    let after: u64 = fauna.ledger.failed_visits[BW.index()].iter().sum();
    assert_eq!(after, before + 1, "the visit failed");
    fauna.tick += cubarium_voxel::ticks_at_least_one(fauna.config.light.mark_s);
}

/// The cyan class's expectation after `n` failures from innate, by the config's own rule
/// (each failure moves it `learn_rate` of the way to 0).
fn cyan_after(c: &FaunaConfig, n: u32) -> f64 {
    c.light.cyan_innate * (1.0 - c.light.learn_rate).powi(n as i32)
}

/// The number of failed cyan visits after which the flower outdraws the glowcap in [`scene`]
/// (`level × expectation`, or the cyan class dropping under `expect_min`), from the config
/// and the two contrasts.
fn failures_to_flip(c: &FaunaConfig, flower: &Beacon, glowcap: &Beacon) -> u32 {
    let (cf, cg) = (flower.contrast(&c.light, 1.0), glowcap.contrast(&c.light, 1.0));
    (0..200)
        .find(|&n| {
            let e = cyan_after(c, n);
            e < c.light.expect_min || e * cg < c.light.uv_innate * cf
        })
        .expect("the cyan expectation falls toward 0")
}

// ------------------------------------------------------- learning which light pays

/// Naive, the bellwing goes to the brighter glowcap; after the configured number of failed
/// cyan visits it goes to the UV flower the same distance away instead.
#[test]
fn after_failed_cyan_visits_a_bellwing_chooses_a_uv_flower_over_a_brighter_glowcap() {
    let world = clock_off(41);
    let flora = Flora::new(FloraConfig::default());
    let c = config(0.3, 1e9);
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world, MID_X, LANE_Z, NORTH);
    let (flower, glowcap) = scene(&mut fauna, &world, &flora, bw);
    assert!(glowcap.contrast(&c.light, 1.0) > flower.contrast(&c.light, 1.0), "fixture: the glowcap is the brighter");
    assert!(c.light.uv_innate >= c.light.cyan_innate, "fixture: innate values do not already favour cyan");
    let (act, _) = steer(&mut fauna, &world, &flora, bw, 1.0).expect("a still light draws it");
    let l = reading(&fauna, bw);
    assert_eq!(l.beacon_kind, BeaconKind::Glowcap.index() as u8, "naive: the brighter glowcap: {l:?}");
    assert!(act.turn < 0.0, "to its left: {act:?}");
    let n = failures_to_flip(&c, &flower, &glowcap);
    assert!(n > 0, "fixture: one failure is not already enough");
    for k in 0..n {
        fail_at(&mut fauna, &world, &flora, bw, glowcap.x, 1.0);
        if k + 1 < n {
            put(&mut fauna, bw, MID_X);
            let _ = steer(&mut fauna, &world, &flora, bw, 1.0);
            assert_eq!(reading(&fauna, bw).beacon_kind, BeaconKind::Glowcap.index() as u8, "after {} failures: still the glowcap", k + 1);
        }
    }
    put(&mut fauna, bw, MID_X);
    animal_mut(&mut fauna, bw).pose.heading_rad = NORTH;
    let (act, _) = steer(&mut fauna, &world, &flora, bw, 1.0).expect("the flower draws it");
    let l = reading(&fauna, bw);
    assert_eq!(l.beacon_kind, BeaconKind::Flower.index() as u8, "after {n} failed cyan visits: the flower: {l:?}");
    assert!(act.turn > 0.0, "to its right: {act:?}");
}

/// A sip raises the class it came to: the UV expectation after a sip bout is above innate.
#[test]
fn a_sip_raises_the_expectation_of_its_class() {
    let world = clock_off(43);
    let flora = Flora::new(FloraConfig::default());
    let c = config(0.3, 1e9);
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world, MID_X, LANE_Z, NORTH);
    scene(&mut fauna, &world, &flora, bw);
    let i = index(&fauna, bw);
    fauna.tick += 1;
    let sip = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };
    assert!(light::steer_flyer(&mut fauna, i, &world.view(), &flora.view(), 1.0, sip, 0.0).is_none(), "a sip stands");
    let p = *fauna.percept(bw).expect("a record");
    let uv = light::expectation(&c.light, &p, BeaconKind::Flower, fauna.tick);
    let cyan = light::expectation(&c.light, &p, BeaconKind::Glowcap, fauna.tick);
    assert!(uv > c.light.uv_innate, "the UV expectation rose: {uv}");
    assert!((cyan - c.light.cyan_innate).abs() < 1e-12, "the cyan one did not move: {cyan}");
}

/// The learned cyan expectation relaxes back toward innate over the forgetting time (the
/// offset shrinks by e each `forget_s`), and once it has, the glowcap draws it again.
#[test]
fn the_preference_relaxes_back_toward_innate_over_the_forgetting_time() {
    let world = clock_off(47);
    let flora = Flora::new(FloraConfig::default());
    let forget_s = 60.0;
    let c = config(0.3, forget_s);
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world, MID_X, LANE_Z, NORTH);
    let (flower, glowcap) = scene(&mut fauna, &world, &flora, bw);
    let n = failures_to_flip(&c, &flower, &glowcap);
    for _ in 0..n {
        fail_at(&mut fauna, &world, &flora, bw, glowcap.x, 1.0);
    }
    let p = *fauna.percept(bw).expect("a record");
    let t0 = fauna.tick;
    let e0 = light::expectation(&c.light, &p, BeaconKind::Glowcap, t0);
    assert!(e0 < c.light.cyan_innate, "learned: below innate ({e0})");
    let tau = forget_s * f64::from(crate::TICK_HZ);
    let e1 = light::expectation(&c.light, &p, BeaconKind::Glowcap, t0 + tau as u64);
    let want = c.light.cyan_innate - (c.light.cyan_innate - e0) * (-1.0f64).exp();
    assert!((e1 - want).abs() < 1e-6, "one forgetting time: the offset shrinks by e ({e1} vs {want})");
    // Long after: back to innate, and the brighter glowcap draws it again.
    fauna.tick = t0 + (20.0 * tau) as u64;
    put(&mut fauna, bw, MID_X);
    let _ = steer(&mut fauna, &world, &flora, bw, 1.0);
    assert_eq!(reading(&fauna, bw).beacon_kind, BeaconKind::Glowcap.index() as u8, "forgotten: the glowcap again");
}

/// A naive bellwing approaches a still lit lure in the dark; one that has learned cyan does
/// not pay (its cyan expectation under `expect_min`) does not.
#[test]
fn a_naive_bellwing_approaches_a_still_lit_lure() {
    let world = clock_off(53);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config(0.3, 1e9);
    // The cyan lure (BELLWING: a lure that mimics a flower is UV to this eye; see
    // `bellwing_lure_tests`).
    c.light.lure_uv = 0.0;
    // The light alone: since NECTAR-SCENT the lit lure's plume is smelled at once.
    c.light.lure_scent = 0.0;
    let lure_scene = |fauna: &mut Fauna| {
        let lj = introduce(fauna, &world, LJ, StartingStores::FULL);
        place(fauna, lj, MID_X + 2.5, LANE_Z, WEST);
        drive(fauna, lj, 0.0, false);
        let a = animal_mut(fauna, lj);
        a.pose.y_m = SURFACE_M;
        a.state = State::Resting;
        a.rest = None;
        a.founder_state.lure = 1.0;
        a.still_ticks = u64::MAX / 2;
        a.speed_m_s = 0.0;
    };
    // Naive.
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world, MID_X, LANE_Z, NORTH);
    fauna.beacons.fixed(&world.view(), &flora.view(), Vec::new());
    lure_scene(&mut fauna);
    let (act, _) = steer(&mut fauna, &world, &flora, bw, 0.0).expect("the still lure draws a naive bellwing");
    let l = reading(&fauna, bw);
    assert!(l.beacon && l.beacon_kind == BeaconKind::Lure.index() as u8, "the lure is its beacon: {l:?}");
    assert!(act.turn > 0.0, "toward it, on its right: {act:?}");
    // Experienced: cyan failures at a glowcap elsewhere until the class is under expect_min.
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world, MID_X, LANE_Z, NORTH);
    let y = eye_y(&fauna, bw);
    let glowcap = beacon(BeaconKind::Glowcap, MID_X + 6.0, y, 0.0);
    fauna.beacons.fixed(&world.view(), &flora.view(), vec![glowcap]);
    let n = (0..200).find(|&n| cyan_after(&c, n) < c.light.expect_min).expect("falls under the floor");
    for _ in 0..n {
        let a = animal_mut(&mut fauna, bw);
        a.pose.x = glowcap.x;
        a.pose.z = glowcap.z;
        a.still_ticks = 0;
        assert!(steer(&mut fauna, &world, &flora, bw, 0.0).is_some(), "inspecting the glowcap");
        fauna.tick += cubarium_voxel::ticks_at_least_one(c.light.visit_s);
        let _ = steer(&mut fauna, &world, &flora, bw, 0.0);
        fauna.tick += cubarium_voxel::ticks_at_least_one(c.light.mark_s);
    }
    put(&mut fauna, bw, MID_X);
    lure_scene(&mut fauna);
    let got = steer(&mut fauna, &world, &flora, bw, 0.0);
    let l = reading(&fauna, bw);
    assert!(!l.beacon, "experienced: the lure is no beacon to it: {l:?}");
    assert!(got.is_none(), "its heuristic's answer stands: {got:?}");
}

/// With learning off, the failed cyan visits change nothing: after as many as turn a learning
/// bellwing to the flower, it still goes to the brighter glowcap, exactly as a naive one does.
#[test]
fn with_learning_off_s3s_behaviour_returns() {
    let world = clock_off(59);
    let flora = Flora::new(FloraConfig::default());
    let on = config(0.3, 1e9);
    let mut off = on;
    off.light.learn = false;
    let run = |c: FaunaConfig, failures: bool| {
        let mut fauna = Fauna::new(c);
        let bw = bellwing(&mut fauna, &world, MID_X, LANE_Z, NORTH);
        let (flower, glowcap) = scene(&mut fauna, &world, &flora, bw);
        if failures {
            for _ in 0..failures_to_flip(&on, &flower, &glowcap) {
                fail_at(&mut fauna, &world, &flora, bw, glowcap.x, 1.0);
            }
        }
        put(&mut fauna, bw, MID_X);
        animal_mut(&mut fauna, bw).pose.heading_rad = NORTH;
        let got = steer(&mut fauna, &world, &flora, bw, 1.0);
        (got, reading(&fauna, bw).beacon_kind)
    };
    let (off_after, kind) = run(off, true);
    assert_eq!(kind, BeaconKind::Glowcap.index() as u8, "learning off: still the brighter glowcap");
    let (naive_on, _) = run(on, false);
    assert_eq!(off_after, naive_on, "the same steer as a naive bellwing");
    let (learned_on, kind) = run(on, true);
    assert_eq!(kind, BeaconKind::Flower.index() as u8, "control: learning on, the same failures turn it to the flower");
    assert_ne!(learned_on, off_after);
}

// ------------------------------------------------------- a beacon it cannot reach

/// A bellwing steering for a still beacon that has not moved for `visit_s` (wedged, its
/// motor refused) gives the beacon up: the place is marked and the stall counted; one that is
/// moving keeps approaching.
#[test]
fn a_beacon_it_cannot_reach_is_given_up() {
    let world = clock_off(61);
    let flora = Flora::new(FloraConfig::default());
    let c = config(0.3, 1e9);
    let stall = cubarium_voxel::ticks_at_least_one(c.light.visit_s);
    let arm = |still_ticks: u64| {
        let mut fauna = Fauna::new(c);
        let bw = bellwing(&mut fauna, &world, MID_X, LANE_Z, NORTH);
        let y = eye_y(&fauna, bw);
        fauna.beacons.fixed(&world.view(), &flora.view(), vec![beacon(BeaconKind::Flower, MID_X + SIDE_M, y, 1.0)]);
        animal_mut(&mut fauna, bw).still_ticks = still_ticks;
        let got = steer(&mut fauna, &world, &flora, bw, 1.0);
        let stalled = fauna.ledger.stalled_visits[BW.index()];
        let again = steer(&mut fauna, &world, &flora, bw, 1.0);
        (got, stalled, again, reading(&fauna, bw).beacon)
    };
    let (got, stalled, again, beacon_after) = arm(stall);
    assert!(got.is_none(), "wedged: it gives the beacon up ({got:?})");
    assert_eq!(stalled, 1, "and the stall is counted");
    assert!(again.is_none() && !beacon_after, "the place is marked: no approach while the mark lasts");
    let (got, stalled, _, _) = arm(0);
    assert!(got.is_some_and(|(a, _)| a.turn > 0.0), "moving: it keeps approaching ({got:?})");
    assert_eq!(stalled, 0);
}

// ------------------------------------------------------- looming is an approach

/// A body of another lineage within `loom_m` looms only while it closes on the bellwing
/// fast enough to reach it within `loom_tau_s`: coming at it, it repels (away and up); the
/// same body at the same place walking across, or coming slower than that, does not.
#[test]
fn a_moving_body_looms_only_while_it_closes_in() {
    let world = clock_off(67);
    let flora = Flora::new(FloraConfig::default());
    let c = config(0.3, 1e9);
    let (loom_m, tau) = (c.founder(BW).senses.loom_m, c.light.loom_tau_s);
    assert!(tau > 0.0, "fixture: the closing rule is on");
    let d = 0.5 * loom_m;
    let arm = |heading: f64, speed: f64| {
        let mut fauna = Fauna::new(c);
        let bw = bellwing(&mut fauna, &world, MID_X, LANE_Z, NORTH);
        fauna.beacons.fixed(&world.view(), &flora.view(), Vec::new());
        let g = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
        place(&mut fauna, g, MID_X + d, LANE_Z, heading);
        drive(&mut fauna, g, 0.0, false);
        let a = animal_mut(&mut fauna, g);
        a.pose.y_m = SURFACE_M;
        a.still_ticks = 0;
        a.speed_m_s = speed;
        let got = steer(&mut fauna, &world, &flora, bw, 1.0);
        (got, fauna.ledger.loom_samples[BW.index()])
    };
    // Coming at it (heading west, toward -x) at twice the speed that reaches it in tau.
    let fast = 2.0 * d / tau;
    let (at, n) = arm(WEST, fast);
    let (act, lift) = at.expect("closing in: it looms");
    assert!(act.turn < 0.0 && lift > 0.0, "away to the left and up: {act:?} lift {lift}");
    assert!(n > 0);
    let (across, n) = arm(NORTH, fast);
    assert!(across.is_none() && n == 0, "walking across: no looming ({across:?})");
    let (slow, n) = arm(WEST, 0.5 * d / tau);
    assert!(slow.is_none() && n == 0, "coming slower than reaches it in tau: no looming ({slow:?})");
}

// ------------------------------------------------------- the world's top

/// A flyer climbing under open sky with a band reaching past the world's top stops with its
/// body inside the world, and can still move sideways there.
#[test]
fn a_climbing_flyer_stops_at_the_worlds_top() {
    let world = clock_off(71);
    let flora = Flora::new(FloraConfig::default());
    let view = world.view();
    let top = f64::from(view.config.height) * view.config.voxel_m;
    let shape = crate::flight::Shape { r: 0.1, h: 0.2 };
    let mut pose = Pose { x: MID_X, z: LANE_Z, heading_rad: EAST, y_m: SURFACE_M + 0.1 };
    let up = crate::flight::move_through_air(&view, &flora.view(), shape, 10.0 * top, 0.0, &mut pose, 0.0, 10.0 * top);
    assert!(up.rise_m > 0.0, "it climbed");
    assert!(pose.y_m + shape.h <= top + 1e-9, "its body stays inside the world: top {} at {}", pose.y_m + shape.h, top);
    let side = crate::flight::move_through_air(&view, &flora.view(), shape, 10.0 * top, 0.0, &mut pose, 0.5, 0.0);
    assert!(side.distance_m > 0.0 && !side.blocked, "and moves sideways there: {side:?}");
}
