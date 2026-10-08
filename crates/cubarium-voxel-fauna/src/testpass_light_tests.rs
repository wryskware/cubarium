//! **TEST-PASS (independent), S1 / S3 / S3b and the save**: the learning's joins with the
//! failed-visit mark and the sip, the mark's decay edge, the capgnawer's mark without
//! learning, the lure-kill readouts with the books closing, the kin-track cap, and a postcard
//! round trip of every state field today's packages added. A child of `step`, sharing the
//! predation fixture.
//!
//! Every case sets the config it relies on; no shipped default, ratio or verdict is asserted.

use super::predation_fixture::*;
use crate::light::{self, Beacon, BeaconKind, CYAN, UV};
use crate::perception::{self, Mark, Percept, class};
use crate::{Actions, Command, Fauna, FaunaConfig, Founder, MobilityMode, Pose, SenseTable, StartingStores, State, Track};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const BW: Founder = Founder::Bellwing;
const LJ: Founder = Founder::Lanternjaw;
const CAP: Founder = Founder::Capgnawer;
const NORTH: f64 = 0.0;
const WANDER: Actions = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
const SIP: Actions = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };
const UP_M: f64 = 0.3;

/// Remote senses every tick, learning on at `learn_rate` with no forgetting to speak of, short
/// inspections and marks.
fn config(learn_rate: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.light.learn = true;
    c.light.learn_rate = learn_rate;
    c.light.forget_s = 1e9;
    c.light.visit_s = 1.0;
    c.light.mark_s = 1.0;
    // The light alone: since NECTAR-SCENT a hand-placed flower beacon gives off its nectar
    // plume, which would lead a bellwing these cases pass by.
    c.scent.nectar_per_s = 0.0;
    c
}

fn bellwing(fauna: &mut Fauna, world: &World, x: f64) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft { pose: Pose { x, z: LANE_Z, heading_rad: NORTH, y_m: SURFACE_M + UP_M }, founder: BW, stores: StartingStores::FULL },
    ));
    let id = fauna.view().ledger.births - 1;
    let a = animal_mut(fauna, id);
    a.eating = true;
    a.still_ticks = 0;
    assert_eq!(a.mobility.mode, MobilityMode::Fly);
    id
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("alive")
}

fn eye_y(fauna: &Fauna, id: u64) -> f64 {
    let a = animal(fauna, id);
    a.pose.y_m + crate::body_of(&fauna.config, &a).expect("a body").eye_m
}

fn beacon(kind: BeaconKind, x: f64, y_m: f64, sky: f64) -> Beacon {
    Beacon { kind, x, z: LANE_Z, y_m, sky, power: 1.0, moving: false, body: 0 }
}

fn steer_with(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, asked: Actions) -> Option<(Actions, f64)> {
    fauna.tick += 1;
    let i = index(fauna, id);
    light::steer_flyer(fauna, i, &world.view(), &flora.view(), 1.0, asked, 0.0)
}

fn expect(fauna: &Fauna, id: u64, kind: BeaconKind) -> f64 {
    let p = fauna.percept(id).copied().unwrap_or_default();
    light::expectation(&fauna.config.light, &p, kind, fauna.tick)
}

// ------------------------------------------------------------------ S3b × the visit

/// **A sip far from a stale visit teaches UV, not the stale visit's class**: a bellwing whose
/// record still holds an inspection of a glowcap metres away (it left it fed, or was turned
/// from it) and that sips at a flower its heuristic found raises its UV expectation; its cyan
/// expectation does not move.
#[test]
fn a_sip_far_from_a_stale_glowcap_visit_teaches_uv_not_cyan() {
    let world = clock_off(301);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(0.5));
    let bw = bellwing(&mut fauna, &world, 2.0);
    let far = 2.0 + 4.0;
    assert!(4.0 > fauna.config.light.visit_m + fauna.config.light.mark_radius_m, "the stale visit is well away");
    let since = fauna.tick;
    fauna.percepts.entry(bw).or_default().visit =
        perception::Visit { active: true, kind: BeaconKind::Glowcap.index() as u8, x: far, z: LANE_Z, since };
    let (uv0, cyan0) = (expect(&fauna, bw, BeaconKind::Flower), expect(&fauna, bw, BeaconKind::Glowcap));
    assert!(steer_with(&mut fauna, &world, &flora, bw, SIP).is_none(), "the sip stands");
    let (uv1, cyan1) = (expect(&fauna, bw, BeaconKind::Flower), expect(&fauna, bw, BeaconKind::Glowcap));
    assert!(uv1 > uv0, "the flower sip teaches UV: {uv0} -> {uv1}");
    assert_eq!(cyan1, cyan0, "and leaves cyan alone");
}

/// **A visit it flew away from closes as failed** (FINDINGS-FIX): a bellwing whose open
/// glowcap inspection lies farther than `visit_m` + `visit_leave_m` closes it on its next
/// non-sip sample (counted failed and left, marked, cyan taught toward 0); one just inside
/// that distance stays open. A later sip then teaches UV and leaves cyan alone.
#[test]
fn an_open_visit_it_flew_away_from_closes_as_failed() {
    let world = clock_off(311);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config(0.5);
    c.light.visit_m = 0.25;
    c.light.visit_leave_m = 0.5;
    let mut fauna = Fauna::new(c);
    let bw = bellwing(&mut fauna, &world, 2.0);
    let glowcap = BeaconKind::Glowcap.index();
    let open_at = |fauna: &mut Fauna, x: f64| {
        let since = fauna.tick;
        fauna.percepts.entry(bw).or_default().visit = perception::Visit { active: true, kind: glowcap as u8, x, z: LANE_Z, since };
    };
    // Just inside visit_m + visit_leave_m: still open.
    open_at(&mut fauna, 2.0 + 0.25 + 0.5 - 0.05);
    steer_with(&mut fauna, &world, &flora, bw, WANDER);
    assert!(fauna.percept(bw).unwrap().visit.active, "within reach of the visit it stays open");
    assert_eq!(fauna.ledger.left_visits[BW.index()][glowcap], 0);
    // Beyond it: closed as failed.
    open_at(&mut fauna, 2.0 + 0.25 + 0.5 + 0.05);
    let cyan0 = expect(&fauna, bw, BeaconKind::Glowcap);
    let failed0 = fauna.ledger.failed_visits[BW.index()][glowcap];
    steer_with(&mut fauna, &world, &flora, bw, WANDER);
    let p = fauna.percept(bw).copied().unwrap();
    assert!(!p.visit.active, "the left visit is closed");
    assert_eq!(fauna.ledger.left_visits[BW.index()][glowcap], 1);
    assert_eq!(fauna.ledger.failed_visits[BW.index()][glowcap], failed0 + 1);
    assert!(p.marked(2.0 + 0.8, LANE_Z, fauna.tick, fauna.config.light.mark_radius_m, 1e9), "its place is marked");
    let cyan1 = expect(&fauna, bw, BeaconKind::Glowcap);
    assert!(cyan1 < cyan0, "cyan taught toward 0: {cyan0} -> {cyan1}");
    // The next sip is a flower's.
    let uv1 = expect(&fauna, bw, BeaconKind::Flower);
    steer_with(&mut fauna, &world, &flora, bw, SIP);
    assert!(expect(&fauna, bw, BeaconKind::Flower) > uv1, "the sip teaches UV");
    let cyan2 = expect(&fauna, bw, BeaconKind::Glowcap);
    assert!((cyan2 - cyan1).abs() < 1e-6, "and leaves cyan alone (bar the slow relaxation): {cyan1} -> {cyan2}");
}

/// **A sip at the visited beacon teaches its class** (the control for the case above): the
/// same sip with the bellwing at the glowcap it is inspecting raises cyan.
#[test]
fn a_sip_at_the_inspected_beacon_teaches_its_class() {
    let world = clock_off(302);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(0.5));
    let bw = bellwing(&mut fauna, &world, 2.0);
    let since = fauna.tick;
    fauna.percepts.entry(bw).or_default().visit =
        perception::Visit { active: true, kind: BeaconKind::Glowcap.index() as u8, x: 2.0, z: LANE_Z, since };
    let cyan0 = expect(&fauna, bw, BeaconKind::Glowcap);
    steer_with(&mut fauna, &world, &flora, bw, SIP);
    let want = cyan0 + fauna.config.light.learn_rate * (1.0 - cyan0);
    assert!((expect(&fauna, bw, BeaconKind::Glowcap) - want).abs() < 1e-9, "cyan moves learn_rate of the way to 1");
    assert_eq!(fauna.ledger.sipped_visits[BW.index()][BeaconKind::Glowcap.index()], 1);
    assert!(!fauna.percept(bw).unwrap().visit.active, "the visit is rewarded and closed");
}

/// **One bout teaches once**: sips within `visit_s` of the last are one bout and move the
/// expectation once; a sip after a gap longer than `visit_s` is a new bout.
#[test]
fn sips_within_visit_s_are_one_bout_and_teach_once() {
    let world = clock_off(303);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(0.5));
    let bw = bellwing(&mut fauna, &world, 2.0);
    let gap = cubarium_voxel::ticks_at_least_one(fauna.config.light.visit_s);
    steer_with(&mut fauna, &world, &flora, bw, SIP);
    let after_one = expect(&fauna, bw, BeaconKind::Flower);
    for _ in 0..3 {
        steer_with(&mut fauna, &world, &flora, bw, SIP);
    }
    assert!((expect(&fauna, bw, BeaconKind::Flower) - after_one).abs() < 1e-6, "one bout");
    fauna.tick += gap + 1;
    steer_with(&mut fauna, &world, &flora, bw, SIP);
    let want = after_one + fauna.config.light.learn_rate * (1.0 - after_one);
    assert!((expect(&fauna, bw, BeaconKind::Flower) - want).abs() < 1e-6, "a new bout teaches again");
}

/// **A class under `expect_min` is passed by, and only while learning is on**: the only beacon
/// in sight, a flower in full light, is not approached by a bellwing that has learned UV
/// under `expect_min`; with learning off the same record approaches it.
#[test]
fn a_class_learned_under_expect_min_is_not_approached_unless_learning_is_off() {
    let world = clock_off(304);
    let flora = Flora::new(FloraConfig::default());
    for learn in [true, false] {
        let mut c = config(0.5);
        c.light.learn = learn;
        c.light.expect_min = 0.2;
        let mut fauna = Fauna::new(c);
        let bw = bellwing(&mut fauna, &world, 3.0);
        let y = eye_y(&fauna, bw);
        fauna.beacons.fixed(&world.view(), &flora.view(), vec![beacon(BeaconKind::Flower, 4.5, y, 1.0)]);
        let now = fauna.tick;
        let uv_innate = fauna.config.light.uv_innate;
        let p = fauna.percepts.entry(bw).or_default();
        p.expect[UV] = 0.5 * 0.2 - uv_innate;
        p.expect_tick = now;
        let got = steer_with(&mut fauna, &world, &flora, bw, WANDER);
        let r = fauna.percept(bw).unwrap().light;
        assert_eq!(r.beacon, !learn, "learn {learn}: beacon {}", r.beacon);
        assert_eq!(got.is_some(), !learn, "learn {learn}: steer {got:?}");
    }
}

/// **The expectation relaxes over `forget_s`**: after one failed cyan visit, the offset decays
/// as `exp(−t / forget_s)`, and a `forget_s` of 0 is back to innate at once.
#[test]
fn the_expectation_relaxes_exponentially_and_a_zero_forgetting_time_forgets_at_once() {
    let mut c = config(0.5);
    c.light.forget_s = 10.0;
    let innate = c.light.cyan_innate;
    let now = 100;
    let mut p = Percept::default();
    p.expect[CYAN] = -0.5 * innate;
    p.expect_tick = now;
    let cfg = c.light;
    let later = now + cubarium_voxel::ticks_at_least_one(10.0);
    let got = light::expectation(&cfg, &p, BeaconKind::Glowcap, later);
    let want = innate - 0.5 * innate * (-1.0f64).exp();
    assert!((got - want).abs() < 1e-9, "one forgetting time: {got} against {want}");
    let mut zero = cfg;
    zero.forget_s = 0.0;
    assert_eq!(light::expectation(&zero, &p, BeaconKind::Glowcap, now + 1), innate);
}

/// **A stall marks and does not teach**: a wedged bellwing that gives a beacon up marks the
/// place but leaves both expectations where they were.
#[test]
fn a_stalled_visit_marks_the_place_and_leaves_the_expectation_alone() {
    let world = clock_off(306);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(0.5));
    let bw = bellwing(&mut fauna, &world, 3.0);
    let y = eye_y(&fauna, bw);
    fauna.beacons.fixed(&world.view(), &flora.view(), vec![beacon(BeaconKind::Glowcap, 4.5, y, 0.0)]);
    animal_mut(&mut fauna, bw).still_ticks = cubarium_voxel::ticks_at_least_one(fauna.config.light.visit_s);
    let (uv0, cyan0) = (expect(&fauna, bw, BeaconKind::Flower), expect(&fauna, bw, BeaconKind::Glowcap));
    assert!(steer_with(&mut fauna, &world, &flora, bw, WANDER).is_none());
    assert_eq!(fauna.ledger.stalled_visits[BW.index()], 1, "the stall is counted");
    let p = *fauna.percept(bw).unwrap();
    let ring = 48.0 * VOXEL_M;
    assert!(p.marked(4.5, LANE_Z, fauna.tick, fauna.config.light.mark_radius_m, ring), "the place is marked");
    assert_eq!((expect(&fauna, bw, BeaconKind::Flower), expect(&fauna, bw, BeaconKind::Glowcap)), (uv0, cyan0));
}

// ------------------------------------------------------------------ S3: the mark

/// **A mark holds through `until_tick − 1` and is gone at `until_tick`**, and a ninth mark
/// takes the slot that decays first.
#[test]
fn a_mark_decays_on_its_until_tick_and_a_new_mark_takes_the_soonest_slot() {
    let ring = 12.0;
    let mut p = Percept::default();
    p.mark(1.0, 1.0, 50);
    assert!(p.marked(1.0, 1.0, 49, 0.1, ring));
    assert!(!p.marked(1.0, 1.0, 50, 0.1, ring));
    let mut p = Percept::default();
    for k in 0..perception::MAX_MARKS {
        p.mark(k as f64, 0.0, 100 + 10 * k as u64);
    }
    p.mark(9.5, 0.0, 1000);
    assert!(!p.marked(0.0, 0.0, 1, 0.1, ring), "the soonest mark was replaced");
    assert!(p.marked(9.5, 0.0, 1, 0.1, ring) && p.marked(1.0, 0.0, 1, 0.1, ring));
    assert_eq!(p.marks.iter().filter(|m: &&Mark| m.until_tick > 0).count(), perception::MAX_MARKS);
}

/// **The capgnawer's failed visit marks but teaches nothing** (learning is the bellwing's):
/// a hungry capgnawer inspecting a glowcap with no food there fails after `visit_s`, the place
/// is marked, and its record's expectations stay naive.
#[test]
fn a_capgnawers_failed_visit_marks_without_learning() {
    let world = clock_off(307);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(0.5));
    let cap = introduce(&mut fauna, &world, CAP, StartingStores::FULL);
    place(&mut fauna, cap, 3.0, LANE_Z, NORTH);
    drive(&mut fauna, cap, 0.0, false);
    animal_mut(&mut fauna, cap).eating = true;
    let y = SURFACE_M + 0.1;
    fauna.beacons.fixed(&world.view(), &flora.view(), vec![beacon(BeaconKind::Glowcap, 3.0, y, 0.0)]);
    let walk = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
    let glow = |fauna: &mut Fauna| {
        fauna.tick += 1;
        let i = index(fauna, cap);
        light::steer_glow(fauna, i, &world.view(), &flora.view(), 0.0, walk, None)
    };
    assert!(glow(&mut fauna).is_some(), "at the glowcap: inspecting");
    fauna.tick += cubarium_voxel::ticks_at_least_one(fauna.config.light.visit_s);
    assert!(glow(&mut fauna).is_none(), "the inspection fails");
    assert_eq!(fauna.ledger.failed_visits[CAP.index()][BeaconKind::Glowcap.index()], 1);
    let p = *fauna.percept(cap).unwrap();
    assert!(p.marked(3.0, LANE_Z, fauna.tick, fauna.config.light.mark_radius_m, 12.0));
    assert_eq!(p.expect, [0.0; light::CLASSES], "no learning");
}

// ------------------------------------------------------------------ the lure kill

fn books_close(fauna: &Fauna, flora: &Flora) {
    let (av, fv) = (fauna.view(), flora.view());
    for (name, got, want) in [
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
    ] {
        assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "the {name} ledger: {got} against {want}");
    }
}

/// **A kill of a bellwing drawn to a lure** is a lure kill and, naive about cyan, a naive kill;
/// the same kill of a bellwing that has learned cyan under the naive cut is a lure kill only.
/// The books close through both.
#[test]
fn a_lure_kill_is_counted_naive_only_while_the_prey_is_naive_and_the_books_close() {
    let world = clock_off(308);
    for learned in [false, true] {
        let mut flora = Flora::new(FloraConfig::default());
        // The cyan lure (BELLWING: one that mimics a flower is UV to a bellwing's eye).
        let mut c = config(0.5);
        c.light.lure_uv = 0.0;
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let bw = bellwing(&mut fauna, &world, 3.0);
        let now = fauna.tick;
        let innate = fauna.config.light.cyan_innate;
        let p = fauna.percepts.entry(bw).or_default();
        p.visit = perception::Visit { active: true, kind: BeaconKind::Lure.index() as u8, x: 3.0, z: LANE_Z, since: now };
        if learned {
            p.expect[CYAN] = 0.5 * light::NAIVE_FRACTION * innate - innate;
            p.expect_tick = now;
        }
        books_close(&fauna, &flora);
        super::apply_hit(&mut fauna, &mut flora, bw, 1.0, LJ);
        assert!(fauna.view().animal(bw).is_none(), "killed");
        assert_eq!(fauna.ledger.lure_kills[BW.index()], 1, "learned {learned}: a lure kill");
        assert_eq!(fauna.ledger.naive_kills[LJ.index()][BW.index()], u64::from(!learned), "learned {learned}: naive?");
        books_close(&fauna, &flora);
    }
}

// ------------------------------------------------------------------ S1: the record

/// **Kin take their own cap**: an observer with `kin_tracks` 2 and room for one prey-or-threat
/// candidate records the two nearest kin and the nearest threat, though four kin and two
/// threats stand within its sight.
#[test]
fn kin_tracks_cap_kin_apart_from_the_candidate_cap() {
    let world = clock_off(309);
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = crate::Activity::Always;
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.perception.max_candidates = 1;
    let g = Founder::Browser;
    c.founders[g.index()].senses =
        SenseTable { live: true, sight_m: 8.0, sight_night_m: 8.0, sight_still_fraction: 1.0, sight_half_angle_deg: 180.0, kin_tracks: 2, ..SenseTable::default() };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let put = |fauna: &mut Fauna, f: Founder, x: f64| {
        let id = introduce(fauna, &world, f, StartingStores::FULL);
        place(fauna, id, x, LANE_Z, NORTH);
        drive(fauna, id, 0.0, false);
        id
    };
    let me = put(&mut fauna, g, 1.0);
    let kin: Vec<u64> = [1.6, 2.2, 2.8, 3.4].iter().map(|&x| put(&mut fauna, g, x)).collect();
    let vh: Vec<u64> = [1.9, 2.5].iter().map(|&x| put(&mut fauna, Founder::Chorister, x)).collect();
    fauna.tick += 1;
    perception::sense_pass(&mut fauna, &world.view(), false, 1.0, 1);
    let p = *fauna.percept(me).expect("a record");
    let ids = |c: u8| -> Vec<u64> { p.tracks().iter().filter(|t| t.class == c).map(|t| t.id).collect() };
    assert_eq!(ids(class::KIN), kin[..2].to_vec(), "the two nearest kin");
    assert_eq!(ids(class::THREAT), vh[..1].to_vec(), "the nearest threat");
}

/// **An asleep body does not sense, and keeps its record**: its tracks are as they were.
#[test]
fn an_asleep_body_keeps_its_record_unsensed() {
    let world = clock_off(310);
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let g = Founder::Browser;
    c.founders[g.index()].senses = SenseTable { live: true, sight_m: 8.0, sight_night_m: 8.0, sight_still_fraction: 1.0, sight_half_angle_deg: 180.0, ..SenseTable::default() };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let me = introduce(&mut fauna, &world, g, StartingStores::FULL);
    place(&mut fauna, me, 1.0, LANE_Z, NORTH);
    let old = Track { known: true, id: 4242, class: class::THREAT, tick: 1, ..Track::default() };
    {
        let p = fauna.percepts.entry(me).or_default();
        p.tracks[0] = old;
        p.n = 1;
    }
    animal_mut(&mut fauna, me).state = State::Asleep;
    let vh = introduce(&mut fauna, &world, Founder::Chorister, StartingStores::FULL);
    place(&mut fauna, vh, 2.0, LANE_Z, NORTH);
    fauna.tick += 1;
    perception::sense_pass(&mut fauna, &world.view(), false, 1.0, 1);
    let p = fauna.percept(me).expect("kept");
    assert_eq!(p.tracks(), &[old], "asleep: no sensing, the record kept");
}

// ------------------------------------------------------------------ the save

/// **Every state field today's packages added survives a postcard round trip** at the current
/// schema: the vigil (flight, hunker, pry, open), the light record (reading, marks, visit,
/// expectations, last sip), the sprint's stamina, the snail's clock and retraction, the last
/// rain, the pack's shun list, the hunkered flag, the new ledger lines and the config's new
/// knobs.
#[test]
fn every_new_state_field_round_trips_through_the_snapshot() {
    let world = clock_off(311);
    let mut c = FaunaConfig::default();
    c.founders[Founder::Browser.index()].hunker.pry_per_member_s = 0.123;
    c.founders[Founder::Chorister.index()].sprint.endurance_s = 17.0;
    c.founders[Founder::Chorister.index()].pack.as_mut().unwrap().select_kin_factor = 0.77;
    c.founders[Founder::Chorister.index()].pack.as_mut().unwrap().shun_s = 33.0;
    c.founders[Founder::RippleSnail.index()].amphibious.as_mut().unwrap().roam_search_m = 1.25;
    c.light.learn_rate = 0.21;
    c.light.loom_tau_s = 0.7;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let den = Site { x: 10, y: SURFACE_Y, z: 6 };
    assert!(fauna.apply(&world, Command::FoundPack { den, founder: Founder::Chorister, stores: HUNTING, heading_rad: EAST }));
    let g = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    let snail = introduce(&mut fauna, &world, Founder::RippleSnail, StartingStores::FULL);
    let bw = bellwing(&mut fauna, &world, 2.0);
    let vh = fauna.animals.iter().find(|a| a.founder == Some(Founder::Chorister)).unwrap().id;
    fauna.tick = 500;
    {
        let v = &mut fauna.percepts.entry(g).or_default().vigil;
        v.fleeing = true;
        v.flee_id = vh;
        v.flee_tick = 480;
        v.hunkered = true;
        v.hunker_close = 490;
        v.pry = 0.37;
        v.opened_until = 520;
        v.bolt_until = 505;
        v.bolt_heading = 1.25;
        v.alarm_ready = 600;
        v.orient_id = vh;
        v.orient_until = 510;
        v.scan_until = 503;
        v.next_scan = 650;
    }
    animal_mut(&mut fauna, g).hunkered = true;
    {
        let top = crate::gait::sprint_forward(&fauna.config, Founder::Chorister);
        let a = animal_mut(&mut fauna, vh);
        a.founder_state.sprint_spent = 0.42;
        a.founder_state.winded = true;
        a.founder_state.held = Actions { forward: top, turn: 0.0, feed: 0.0 };
    }
    {
        let m = &mut animal_mut(&mut fauna, snail).mobility;
        m.dryness = 0.71;
        m.retract_ticks = 7;
        m.ashore = true;
        m.thirsty = true;
    }
    {
        let p = fauna.percepts.entry(bw).or_default();
        p.mark(1.5, 2.5, 900);
        p.visit = perception::Visit { active: true, kind: 2, x: 3.5, z: 1.0, since: 499 };
        p.expect = [0.11, -0.22];
        p.expect_tick = 450;
        p.last_sip = 444;
        p.light.beacon = true;
        p.light.beacon_level = 0.33;
        p.light.looming = 0.2;
        p.light.tick = 499;
    }
    fauna.last_rain_tick = Some(321);
    let pack_id = fauna.packs()[0].id;
    fauna.pack_mut(pack_id).unwrap().shunned = [(g, 2000), (snail, 1500), (0, 0), (0, 0)];
    fauna.ledger.vigil.opens[0] = 3;
    fauna.ledger.vigil.glances[0] = 4;
    fauna.ledger.pack_break_offs = 5;
    fauna.ledger.stalled_visits[BW.index()] = 6;
    fauna.ledger.naive_kills[LJ.index()][BW.index()] = 7;
    fauna.ledger.winded_by_founder[Founder::Chorister.index()] = 8;

    let back = crate::snapshot::decode(&crate::snapshot::encode(&fauna)).expect("a live layer's own bytes load");
    assert_eq!(back.config, fauna.config, "the config's new knobs");
    assert_eq!(back.animals, fauna.animals, "every body, its stamina, clock, retraction and hunker flag");
    assert_eq!(back.percepts, fauna.percepts, "every record: vigil, light, marks, visit, expectations");
    assert_eq!(back.packs(), fauna.packs(), "the shun list");
    assert_eq!(back.last_rain_tick, Some(321));
    assert_eq!(back.ledger, fauna.ledger, "the new ledger lines");
}

/// **The loader takes a rush and refuses past it**: a Veilhound saved mid-rush (held forward
/// at its sprint's top) loads; one past its top, or a lineage with no sprint holding more
/// than cruise, is refused.
#[test]
fn the_loader_takes_a_held_rush_up_to_the_sprint_and_refuses_beyond_it() {
    let world = clock_off(312);
    let mut c = FaunaConfig::default();
    c.founders[Founder::Browser.index()].sprint = crate::Sprint::default();
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let vh = introduce(&mut fauna, &world, Founder::Chorister, StartingStores::FULL);
    let g = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    let top = crate::gait::sprint_forward(&fauna.config, Founder::Chorister);
    assert!(top > 1.0, "the case needs a sprint");
    let with = |fauna: &Fauna, id: u64, forward: f64| {
        let mut f = fauna.clone();
        animal_mut(&mut f, id).founder_state.held = Actions { forward, turn: 0.0, feed: 0.0 };
        crate::snapshot::decode(&crate::snapshot::encode(&f)).map(|_| ())
    };
    with(&fauna, vh, top).expect("at its sprint's top it loads");
    assert!(with(&fauna, vh, top + 0.01).is_err(), "past its top: refused");
    with(&fauna, g, 1.0).expect("a non-sprinter at cruise loads");
    assert!(with(&fauna, g, 1.01).is_err(), "a non-sprinter past cruise: refused");
}
