//! **BELLWING: the lure mimics a flower** (`light.rs` module note, "Lures"; Wrysk 2026-10-06:
//! "make lure mimic flowers"): a lit lure also shows UV ([`crate::LightConfig::lure_uv`]), so an
//! eye with the `uv` channel ranks it as a flower — the UV class — and a visit that finds no
//! nectar there is a failed UV visit; an eye without UV sees its cyan alone. A child of
//! `step`, sharing the predation fixture.
//!
//! Every case builds its scene by hand and checks the mechanism against the config it sets,
//! each against a control arm with the lure's UV off (`lure_uv` 0, the cyan lure); no default
//! is asserted.

use super::predation_fixture::*;
use crate::light::{self, BeaconKind, CYAN, UV};
use crate::{Actions, Command, Fauna, FaunaConfig, Founder, MobilityMode, Pose, StartingStores, State};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig};

const BW: Founder = Founder::Bellwing;
const LJ: Founder = Founder::Lanternjaw;
const NORTH: f64 = 0.0;
const WANDER: Actions = Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
/// The bellwing's place on the lane and the lanternjaw's offset to its right (+x).
const BW_X: f64 = 3.0;
const LJ_DX: f64 = 2.5;

/// The default tables with remote senses read every tick, learning on, no forgetting, short
/// inspections and marks; the lure's UV at `lure_uv` (a test value, not the default), its
/// scent off.
fn config(lure_uv: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.light.learn = true;
    c.light.learn_rate = 0.3;
    c.light.forget_s = 1e9;
    c.light.visit_s = 1.0;
    c.light.mark_s = 1.0;
    c.light.lure_uv = lure_uv;
    // The light alone: since NECTAR-SCENT a lit lure's plume is smelled at once (no field to
    // settle); the lure's scent has its own case.
    c.light.lure_scent = 0.0;
    c
}

/// A hungry bellwing hovering over the lane at `x`, facing north, not wedged.
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

/// A still lanternjaw at `x` on the lane facing west, awake, its lure fully lit.
fn still_lure(fauna: &mut Fauna, world: &World, x: f64) -> u64 {
    let lj = introduce(fauna, world, LJ, StartingStores::FULL);
    place(fauna, lj, x, LANE_Z, WEST);
    drive(fauna, lj, 0.0, false);
    let a = animal_mut(fauna, lj);
    a.pose.y_m = SURFACE_M;
    a.state = State::Resting;
    a.rest = None;
    a.founder_state.lure = 1.0;
    a.still_ticks = u64::MAX / 2;
    a.speed_m_s = 0.0;
    lj
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("alive")
}

fn steer(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64) -> Option<(Actions, f64)> {
    fauna.tick += 1;
    let i = index(fauna, id);
    light::steer_flyer(fauna, i, &world.view(), &flora.view(), 0.0, WANDER, 0.0)
}

fn reading(fauna: &Fauna, id: u64) -> crate::perception::LightReading {
    fauna.percept(id).expect("a record").light
}

/// Set the body's learned expectation of class `c` to `value` (the other class innate).
fn expect_class(fauna: &mut Fauna, id: u64, c: usize, value: f64) {
    let cfg = fauna.config.light;
    let innate = if c == UV { cfg.uv_innate } else { cfg.cyan_innate };
    let now = fauna.tick;
    let p = fauna.percepts.entry(id).or_default();
    p.expect = [0.0; light::CLASSES];
    p.expect[c] = value - innate;
    p.expect_tick = now;
}

fn expectation(fauna: &Fauna, id: u64, c: usize) -> f64 {
    let p = fauna.percept(id).copied().unwrap_or_default();
    light::expectation_of(&fauna.config.light, &p, c, fauna.tick)
}

/// **A UV-sighted bellwing approaches a lure ranked as a UV beacon.** A bellwing that has
/// learned cyan does not pay (under `expect_min`) and keeps UV at its innate value: with the
/// lure's UV on, the still lit lure is its beacon and it turns toward it — the lure is ranked
/// by the UV expectation; with the lure's UV off (control) the same bellwing passes the lure
/// by.
#[test]
fn a_uv_sighted_bellwing_approaches_a_lure_ranked_as_a_uv_beacon() {
    let world = clock_off(401);
    let flora = Flora::new(FloraConfig::default());
    let arm = |lure_uv: f64, cyan: f64| {
        let mut fauna = Fauna::new(config(lure_uv));
        assert!(fauna.config.founder(BW).senses.uv, "fixture: the bellwing's eye has UV");
        let bw = bellwing(&mut fauna, &world, BW_X);
        fauna.beacons.fixed(&world.view(), &flora.view(), Vec::new());
        still_lure(&mut fauna, &world, BW_X + LJ_DX);
        expect_class(&mut fauna, bw, CYAN, cyan);
        let got = steer(&mut fauna, &world, &flora, bw);
        (got, reading(&fauna, bw))
    };
    let low_cyan = 0.5 * config(1.0).light.expect_min;
    assert!(config(1.0).light.uv_innate >= config(1.0).light.expect_min, "fixture: UV innate is approached");
    assert_eq!(BeaconKind::Lure.class_seen(&config(1.0).light, true), UV, "the mimic is UV to a UV eye");

    let (got, l) = arm(1.0, low_cyan);
    assert!(l.beacon && l.beacon_kind == BeaconKind::Lure.index() as u8, "the mimic lure is its beacon though cyan does not pay: {l:?}");
    let (act, _) = got.expect("a still UV light attracts");
    assert!(act.turn > 0.0, "toward it, on its right: {act:?}");

    let (got, l) = arm(0.0, low_cyan);
    assert!(!l.beacon, "control, the cyan lure: passed by once cyan does not pay: {l:?}");
    assert!(got.is_none(), "control: its heuristic's answer stands: {got:?}");

}

/// **A visit to a lure with no nectar marks a failed UV visit**: the bellwing inspects the
/// mimic lure for `visit_s` with nothing to sip; the inspection fails, the place is marked, the
/// failure is booked to the lure, and its **UV** expectation falls while cyan stays innate.
/// Control (the lure's UV off): the same failure teaches cyan, not UV.
#[test]
fn a_visit_to_a_lure_with_no_nectar_marks_a_failed_uv_visit() {
    let world = clock_off(403);
    let flora = Flora::new(FloraConfig::default());
    let arm = |lure_uv: f64| {
        let mut fauna = Fauna::new(config(lure_uv));
        fauna.set_births_enabled(false);
        let bw = bellwing(&mut fauna, &world, BW_X);
        fauna.beacons.fixed(&world.view(), &flora.view(), Vec::new());
        let lj = still_lure(&mut fauna, &world, BW_X + LJ_DX);
        let (lx, lz, _) = light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).expect("a lure");
        // Hover over the lure point (no nectar here).
        let a = animal_mut(&mut fauna, bw);
        a.pose.x = lx;
        a.pose.z = lz;
        assert!(steer(&mut fauna, &world, &flora, bw).is_some(), "inspecting the lure");
        assert!(fauna.percept(bw).unwrap().visit.active, "a visit is open");
        fauna.tick += cubarium_voxel::ticks_at_least_one(fauna.config.light.visit_s);
        animal_mut(&mut fauna, bw).still_ticks = 0;
        let _ = steer(&mut fauna, &world, &flora, bw);
        let p = *fauna.percept(bw).unwrap();
        let ring = f64::from(world.view().config.width) * VOXEL_M;
        let marked = p.marked(lx, lz, fauna.tick, fauna.config.light.mark_radius_m, ring);
        let failed = fauna.ledger.failed_visits[BW.index()][BeaconKind::Lure.index()];
        (marked, failed, expectation(&fauna, bw, UV), expectation(&fauna, bw, CYAN), fauna.config.light)
    };
    let (marked, failed, uv, cyan, cfg) = arm(1.0);
    assert!(marked, "the lure's place is marked 'no reward here'");
    assert_eq!(failed, 1, "one failed visit, booked to the lure");
    assert!(uv < cfg.uv_innate, "a failed UV visit: UV falls ({uv} < {})", cfg.uv_innate);
    assert_eq!(cyan, cfg.cyan_innate, "cyan untaught");

    let (marked, failed, uv, cyan, cfg) = arm(0.0);
    assert!(marked && failed == 1, "control: marked and booked the same");
    assert_eq!(uv, cfg.uv_innate, "control: the cyan lure leaves UV untaught");
    assert!(cyan < cfg.cyan_innate, "control: cyan falls");
}

/// **A body without UV sees the lure as cyan**: a bellwing whose table lacks the `uv` channel
/// reads the mimic lure exactly as the cyan lure — the same level as with the lure's UV off,
/// below what a UV eye reads — and ranks it by cyan: having learned cyan does not pay, it
/// passes the lure by though UV is innate; a UV eye in the same state is drawn.
#[test]
fn a_body_without_uv_sees_the_lure_as_cyan() {
    let world = clock_off(405);
    let flora = Flora::new(FloraConfig::default());
    let arm = |lure_uv: f64, uv_eye: bool, cyan: f64| {
        let mut c = config(lure_uv);
        c.founders[BW.index()].senses.uv = uv_eye;
        let mut fauna = Fauna::new(c);
        let bw = bellwing(&mut fauna, &world, BW_X);
        fauna.beacons.fixed(&world.view(), &flora.view(), Vec::new());
        still_lure(&mut fauna, &world, BW_X + LJ_DX);
        expect_class(&mut fauna, bw, CYAN, cyan);
        let got = steer(&mut fauna, &world, &flora, bw);
        (got, reading(&fauna, bw))
    };
    let cfg = config(1.0).light;
    assert_eq!(BeaconKind::Lure.class_seen(&cfg, false), CYAN, "cyan to an eye without UV");
    // Cyan innate: every eye takes the lure; the level shows what each eye reads.
    let (_, no_uv) = arm(1.0, false, cfg.cyan_innate);
    let (_, cyan_lure) = arm(0.0, false, cfg.cyan_innate);
    let (_, uv_eye) = arm(1.0, true, cfg.cyan_innate);
    assert!(no_uv.beacon && no_uv.beacon_kind == BeaconKind::Lure.index() as u8, "{no_uv:?}");
    assert_eq!(no_uv.beacon_level, cyan_lure.beacon_level, "without UV: the cyan lure's level");
    assert!(uv_eye.beacon_level > no_uv.beacon_level, "a UV eye sees more of the mimic");
    // Cyan learned not to pay: ranked by cyan, it passes the lure by; a UV eye does not.
    let low = 0.5 * cfg.expect_min;
    let (got, l) = arm(1.0, false, low);
    assert!(!l.beacon && got.is_none(), "without UV: passed by on cyan: {l:?}");
    let (got, l) = arm(1.0, true, low);
    assert!(l.beacon && got.is_some(), "control, a UV eye: drawn on UV: {l:?}");
}

/// **The lure's scent** (`lure_scent`): a still lanternjaw with its lure lit gives off nectar
/// scent at its lure (NECTAR-SCENT: a plume in the air); the same with the lure dark, or with
/// `lure_scent` 0, gives none. Read a metre over the lanternjaw, with no flower anywhere.
#[test]
fn a_lit_lure_lays_nectar_scent_and_a_dark_one_or_a_scentless_one_does_not() {
    let world = clock_off(407);
    let flora = Flora::new(FloraConfig::default());
    let smell = |lure_scent: f64, lit: f64| {
        let mut c = config(1.0);
        c.light.lure_scent = lure_scent;
        let mut fauna = Fauna::new(c);
        let lj = still_lure(&mut fauna, &world, BW_X);
        animal_mut(&mut fauna, lj).founder_state.lure = lit;
        let (view, fv) = (world.view(), flora.view());
        for _ in 0..4 {
            fauna.scent.update(&view, &fv, &fauna.animals, &fauna.config);
        }
        let a = animal(&fauna, lj);
        let sources = crate::scent::nectar_sources(&fauna, &view, None);
        crate::scent::nectar_plume(&view, &fauna.config, &sources, (a.pose.x, a.pose.y_m + 1.0, a.pose.z), 0, 0.0).level
    };
    let lit = smell(1.0, 1.0);
    assert!(lit > 0.0, "a lit lure lays nectar scent: {lit}");
    assert_eq!(smell(1.0, 0.0), 0.0, "a dark lure lays none");
    assert_eq!(smell(0.0, 1.0), 0.0, "lure_scent 0: none");
}
