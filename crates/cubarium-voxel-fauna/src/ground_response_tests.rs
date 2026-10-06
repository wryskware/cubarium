//! **S2 GROUND RESPONSES** (`design/7_Research/senses-review-2026-10-06.md` §1, §4, §5 S2):
//! the herd's alarm by hearing, flight with hysteresis, the scan pause's blind window, the
//! Veilhound's stalk under the prey's hearing, the senses as the dodge's awareness (D5), the
//! shredder's freeze, the seedporter's still-object sight, and each lineage's `live` switch
//! restoring S1. A child of `step` beside the predation files, sharing their fixture.
//!
//! Every case builds its scene by hand and checks a mechanism against the config it sets or
//! reads; no default, ratio or population is asserted.

use super::predation_fixture::*;
use crate::perception::{self, HEARING, SIGHT};
use crate::{Actions, Activity, Fauna, FaunaConfig, Founder, Percept, SenseTable, StartingStores, State, Track, Vigil, VigilMode, Vigilance};
use cubarium_voxel::World;
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel_flora::{Flora, FloraConfig};

const GRAZER: Founder = Founder::Browser;
const VH: Founder = Founder::Chorister;
/// A grazer cropping where it stands.
const CROP: Actions = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };

fn world(seed: u64) -> World {
    plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, seed, 16)
}

/// Every lineage awake, remote senses every tick.
fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c
}

/// A herd response with only flight (no scan, orientation or alarm).
fn flight_only(start: f64, stop: f64, calm_s: f64) -> Vigilance {
    Vigilance { mode: VigilMode::Herd, flee_start_m: start, flee_stop_m: stop, calm_s, group_min: 1, run_noise: 1.0, ..Vigilance::default() }
}

/// A body of `f` at `(x, LANE_Z)` heading `heading`, moving at `speed` (0: still).
fn body(fauna: &mut Fauna, world: &World, f: Founder, x: f64, heading: f64, speed: f64) -> u64 {
    let id = introduce(fauna, world, f, StartingStores::FULL);
    place(fauna, id, x, LANE_Z, heading);
    drive(fauna, id, 0.0, false);
    set_speed(fauna, id, speed);
    id
}

fn set_speed(fauna: &mut Fauna, id: u64, speed: f64) {
    let a = animal_mut(fauna, id);
    a.speed_m_s = speed;
    a.still_ticks = if speed > 0.0 { 0 } else { u64::MAX / 2 };
}

fn move_to(fauna: &mut Fauna, id: u64, x: f64) {
    let h = animal(fauna, id).pose.heading_rad;
    place(fauna, id, x, LANE_Z, h);
}

/// One remote sensing for every live body, at the next tick.
fn sense(fauna: &mut Fauna, world: &World) {
    fauna.tick += 1;
    perception::sense_pass(fauna, &world.view(), false, 1.0, 1);
}

/// The body's ground response to `held`, after its tick-rate senses (the tremor).
fn respond(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, held: Actions) -> Option<Actions> {
    let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
    let want = perception::Want { tremor: true, ..Default::default() };
    perception::refresh(fauna, i, &world.view(), &flora.view(), None, want);
    crate::vigil::respond(fauna, i, &world.view(), &flora.view(), held)
}

fn vigil(fauna: &Fauna, id: u64) -> Vigil {
    fauna.percept(id).map(|p| p.vigil).unwrap_or_default()
}

fn modes(fauna: &Fauna, who: u64, of: u64) -> u8 {
    fauna.percept(who).and_then(|p| p.track(of)).map_or(0, |t| t.modes)
}

fn cruise(fauna: &Fauna, f: Founder) -> f64 {
    fauna.config.manifest(f).cruise_m_per_s
}

// ------------------------------------------------------------------ the herd alarm

/// A grazer that hears a neighbour bolt bolts too, along the neighbour's heading; one whose
/// neighbour is beyond its hearing does not; and a neighbour walking at the same speed (not
/// running, so not loud) raises no alarm.
#[test]
fn a_grazer_that_hears_a_bolting_neighbour_bolts_and_one_out_of_hearing_does_not() {
    let world = world(21);
    let flora = Flora::new(FloraConfig::default());
    let hearing_m = 3.0;
    let mut c = config();
    c.founders[GRAZER.index()].senses = SenseTable { live: true, hearing_m, hearing_min: 1e-9, kin_tracks: 3, ..SenseTable::default() };
    c.founders[GRAZER.index()].vigilance = Vigilance {
        run_noise: 3.0,
        alarm_emission_factor: 2.0,
        alarm_bolt_s: 2.0,
        alarm_refractory_s: 10.0,
        ..flight_only(0.0, 0.0, 0.0)
    };
    for (d, running, bolts) in [(0.6 * hearing_m, true, true), (1.5 * hearing_m, true, false), (0.6 * hearing_m, false, false)] {
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let listener = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
        let speed = cruise(&fauna, GRAZER);
        let runner = body(&mut fauna, &world, GRAZER, 2.0 + d, 0.0, speed);
        if running {
            fauna.percepts.entry(runner).or_default().vigil.bolt_until = u64::MAX / 2;
        }
        sense(&mut fauna, &world);
        let got = respond(&mut fauna, &world, &flora, listener, CROP);
        let v = vigil(&fauna, listener);
        let now = fauna.tick;
        assert_eq!(v.bolt_until > now, bolts, "d {d} m, running {running}: bolts {bolts}");
        if bolts {
            let act = got.expect("a bolt overrides cropping");
            assert!(act.feed == 0.0, "a bolt does not crop");
            assert_eq!(v.bolt_heading, animal(&fauna, runner).pose.heading_rad, "along the neighbour's heading");
            assert_eq!(fauna.ledger.vigil.alarm_bolts[GRAZER.index()], 1);
        }
    }
}

/// After a bolt the alarm is refractory: the same bolting neighbour heard again before the
/// refractory ends sets no second bolt.
#[test]
fn an_alarm_bolt_is_refractory() {
    let world = world(22);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[GRAZER.index()].senses = SenseTable { live: true, hearing_m: 3.0, hearing_min: 1e-9, kin_tracks: 3, ..SenseTable::default() };
    c.founders[GRAZER.index()].vigilance =
        Vigilance { run_noise: 3.0, alarm_emission_factor: 2.0, alarm_bolt_s: 0.5, alarm_refractory_s: 10.0, ..flight_only(0.0, 0.0, 0.0) };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let listener = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    let speed = cruise(&fauna, GRAZER);
    let runner = body(&mut fauna, &world, GRAZER, 3.5, EAST, speed);
    fauna.percepts.entry(runner).or_default().vigil.bolt_until = u64::MAX / 2;
    let refractory = cubarium_voxel::ticks_at_least_one(10.0);
    for _ in 0..refractory / 2 {
        sense(&mut fauna, &world);
        respond(&mut fauna, &world, &flora, listener, CROP);
    }
    assert_eq!(fauna.ledger.vigil.alarm_bolts[GRAZER.index()], 1, "one bolt per refractory");
}

// ------------------------------------------------------------------ flight

/// Flight starts at the start distance and stops only beyond the stop distance after the
/// calm-down: a moving threat between the two does not start it, inside the start does, back
/// between the two keeps it, beyond the stop keeps it until the calm-down has passed.
#[test]
fn flight_starts_and_stops_with_hysteresis() {
    let world = world(23);
    let flora = Flora::new(FloraConfig::default());
    let (start, stop, calm_s) = (1.5, 3.0, 1.0);
    let mut c = config();
    c.founders[GRAZER.index()].senses =
        SenseTable { live: true, sight_m: 5.0, sight_night_m: 5.0, sight_still_fraction: 1.0, sight_half_angle_deg: 180.0, ..SenseTable::default() };
    c.founders[GRAZER.index()].vigilance = flight_only(start, stop, calm_s);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    let vh = body(&mut fauna, &world, VH, 2.0 + 0.5 * (start + stop), WEST, 1.0);
    let step = |fauna: &mut Fauna| {
        sense(fauna, &world);
        respond(fauna, &world, &flora, g, CROP)
    };
    step(&mut fauna);
    assert!(!vigil(&fauna, g).fleeing, "between start and stop: no flight yet");
    move_to(&mut fauna, vh, 2.0 + 0.5 * start);
    let act = step(&mut fauna).expect("inside the start distance it flees");
    assert!(vigil(&fauna, g).fleeing && act.feed == 0.0);
    assert!(act.turn != 0.0 || act.forward > 0.0, "it moves off");
    move_to(&mut fauna, vh, 2.0 + 0.5 * (start + stop));
    step(&mut fauna);
    assert!(vigil(&fauna, g).fleeing, "back between start and stop: still fleeing");
    move_to(&mut fauna, vh, 2.0 + 0.5 * (stop + 5.0));
    step(&mut fauna);
    assert!(vigil(&fauna, g).fleeing, "beyond the stop, before the calm-down: still fleeing");
    let calm = cubarium_voxel::ticks_at_least_one(calm_s);
    for _ in 0..calm + 1 {
        step(&mut fauna);
    }
    assert!(!vigil(&fauna, g).fleeing, "beyond the stop after the calm-down: it stops");
    assert_eq!(fauna.ledger.vigil.flees[GRAZER.index()], 1, "one flight");
}

/// Below its group an adult stands its ground: with `group_min` 2 one threat inside the start
/// distance does not start a flight, two do.
#[test]
fn an_adult_stands_its_ground_below_its_group() {
    let world = world(24);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    let f = Founder::Loftstrider;
    c.founders[f.index()].senses =
        SenseTable { live: true, sight_m: 5.0, sight_night_m: 5.0, sight_still_fraction: 1.0, sight_half_angle_deg: 180.0, ..SenseTable::default() };
    c.founders[f.index()].vigilance = Vigilance { group_min: 2, ..flight_only(3.0, 4.0, 1.0) };
    for n in [1usize, 2] {
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let me = body(&mut fauna, &world, f, 2.0, EAST, 0.0);
        let birth = fauna.config.founder(f).core.birth_body;
        animal_mut(&mut fauna, me).body = animal(&fauna, me).body.max(birth);
        for k in 0..n {
            body(&mut fauna, &world, VH, 3.5 + 0.5 * k as f64, WEST, 1.0);
        }
        sense(&mut fauna, &world);
        respond(&mut fauna, &world, &flora, me, CROP);
        assert_eq!(vigil(&fauna, me).fleeing, n >= 2, "{n} threats");
    }
}

// ------------------------------------------------------------------ the scan pause

/// Cropping, the grazer pauses to scan every `scan_every_s` for `scan_s`, standing still and
/// not biting; between scans its head is down and a moving threat within its full sight but
/// beyond `feeding_sight_fraction` of it is not seen, while during a scan it is.
#[test]
fn the_scan_pause_opens_a_blind_window() {
    let world = world(25);
    let flora = Flora::new(FloraConfig::default());
    let sight = 4.0;
    let mut c = config();
    c.founders[GRAZER.index()].senses = SenseTable {
        live: true,
        sight_m: sight,
        sight_night_m: sight,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: 180.0,
        feeding_sight_fraction: 0.25,
        ..SenseTable::default()
    };
    c.founders[GRAZER.index()].vigilance = Vigilance { scan_every_s: 1.0, scan_s: 0.5, ..flight_only(0.0, 0.0, 0.0) };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    let vh = body(&mut fauna, &world, VH, 2.0 + 0.5 * sight, WEST, 1.0);
    let (mut blind, mut seen, mut scans) = (0, 0, 0);
    for _ in 0..cubarium_voxel::ticks_at_least_one(4.0) {
        let head_down = vigil(&fauna, g).head_down(fauna.tick + 1);
        sense(&mut fauna, &world);
        let sees = modes(&fauna, g, vh) & SIGHT != 0;
        assert_eq!(sees, !head_down, "seen exactly when the head is up");
        if head_down {
            blind += 1;
        } else {
            seen += 1;
        }
        if let Some(act) = respond(&mut fauna, &world, &flora, g, CROP) {
            assert!(act.forward == 0.0 && act.feed == 0.0, "a scan stands still, no bite");
            scans += 1;
        }
    }
    assert!(blind > 0 && seen > 0 && scans > 0, "blind {blind}, seen {seen}, scan ticks {scans}");
    assert!(fauna.ledger.vigil.scans[GRAZER.index()] > 1, "it scans repeatedly");
}

// ------------------------------------------------------------------ the stalk

/// Beyond its commit distance a stalking Veilhound walks slowly enough that the prey does not
/// hear it; within, it rushes at what it asked, and the prey hears the rush.
#[test]
fn a_stalking_veilhound_under_the_hearing_floor_is_not_heard_and_the_rush_is() {
    let world = world(26);
    let mut c = config();
    let shipped = c.founders[GRAZER.index()].senses;
    c.founders[GRAZER.index()].senses = SenseTable { live: true, hearing_m: shipped.hearing_m, hearing_min: shipped.hearing_min, ..SenseTable::default() };
    let pk = c.founders[VH.index()].pack.expect("a pack lineage");
    assert!(pk.stalk_margin > 0.0 && pk.stalk_margin < 1.0, "the shipped pack stalks under the floor");
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let vh = body(&mut fauna, &world, VH, 1.0, EAST, 0.0);
    let geometry = crate::body_of(&fauna.config, &animal(&fauna, vh)).expect("a body");
    let full = cruise(&fauna, VH);
    let pc = fauna.config.perception;
    let audible = full.powf(pc.hearing_speed_exp) * geometry.length_m.powf(pc.hearing_size_exp) / shipped.hearing_min;
    let d_stalk = pk.commit_m + 0.5 * (shipped.hearing_m.min(5.5) - pk.commit_m);
    assert!(d_stalk > pk.commit_m && d_stalk < shipped.hearing_m, "a stalk distance inside the prey's hearing");
    let d_rush = 0.9 * pk.commit_m.min(audible).max(pc.hearing_near_m);
    for (d, stalks) in [(d_stalk, true), (d_rush, false)] {
        let g = body(&mut fauna, &world, GRAZER, 1.0 + d, EAST, 0.0);
        let t = Track { known: true, id: g, x: 1.0 + d, z: LANE_Z, ..Track::default() };
        let a = animal(&fauna, vh);
        let forward = crate::vigil::stalk(&mut fauna, &a, &t, d, 1.0).expect("the pack stalks");
        if stalks {
            assert!(forward < 1.0, "beyond the commit distance it slows: {forward}");
        } else {
            assert_eq!(forward, crate::gait::sprint_forward(&fauna.config, VH), "within it, the rush: its sprint (GAIT)");
        }
        set_speed(&mut fauna, vh, forward * full);
        sense(&mut fauna, &world);
        let heard = modes(&fauna, g, vh) & HEARING != 0;
        assert_eq!(heard, !stalks, "d {d}: forward {forward}, heard {heard}");
        fauna.animals.retain(|o| o.id != g);
    }
}

/// While a prey it sees is in a scan pause, the stalker stands still (with `stalk_freeze`).
#[test]
fn the_stalker_freezes_while_its_prey_looks_up() {
    let world = world(27);
    let c = config();
    let pk = c.founders[VH.index()].pack.expect("a pack lineage");
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    fauna.config.founders[VH.index()].pack = Some(crate::Pack { stalk_freeze: true, ..pk });
    let vh = body(&mut fauna, &world, VH, 1.0, EAST, 0.0);
    let d = pk.commit_m + 1.0;
    let g = body(&mut fauna, &world, GRAZER, 1.0 + d, EAST, 0.0);
    let t = Track { known: true, id: g, x: 1.0 + d, z: LANE_Z, modes: SIGHT, ..Track::default() };
    fauna.percepts.entry(g).or_default().vigil.scan_until = fauna.tick + 10;
    let a = animal(&fauna, vh);
    assert_eq!(crate::vigil::stalk(&mut fauna, &a, &t, d, 1.0), Some(0.0), "still while it looks up");
    fauna.percepts.entry(g).or_default().vigil.scan_until = 0;
    assert!(crate::vigil::stalk(&mut fauna, &a, &t, d, 1.0).is_some_and(|f| f > 0.0), "moving again once it crops");
}

// ------------------------------------------------------------------ D5: awareness

/// A prey of a live lineage whose record holds its striker gets the aware dodge; one whose
/// record does not, the unaware one. The senses replace the state column (a cropping prey
/// that sensed its striker dodges at the sensed value, not the cropping one).
#[test]
fn a_prey_whose_record_holds_the_striker_gets_the_aware_dodge() {
    let world = world(28);
    let mut c = config();
    c.predation.awareness_sensed = 1.0;
    c.predation.awareness_unaware = 0.0;
    c.predation.awareness_cropping = 0.4;
    c.founders[GRAZER.index()].senses.live = true;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    let striker = 999;
    animal_mut(&mut fauna, g).state = State::Cropping;
    let prey = animal(&fauna, g);
    let now = 100;
    let mut holds = Percept::default();
    holds.tracks[0] = Track { known: true, id: striker, tick: now, ..Track::default() };
    holds.n = 1;
    let cfg = &fauna.config;
    let aware = crate::vigil::awareness(cfg, &prey, Some(&holds), striker, now);
    let unaware = crate::vigil::awareness(cfg, &prey, Some(&Percept::default()), striker, now);
    assert_eq!(aware, cfg.predation.awareness_sensed);
    assert_eq!(unaware, cfg.predation.awareness_unaware);
    let stale = crate::vigil::awareness(cfg, &prey, Some(&holds), striker, now + 10 * cubarium_voxel::ticks_at_least_one(cfg.predation.aware_within_s));
    assert_eq!(stale, cfg.predation.awareness_unaware, "sensed too long ago: unaware");
    let p = &cfg.predation;
    assert!(crate::dodge_chance_with(p, 0.5, aware, false, 1.0) > crate::dodge_chance_with(p, 0.5, unaware, false, 1.0));
}

// ------------------------------------------------------------------ the shredder

/// A littershredder freezes on a tremor at its freeze line — no step, no bite — and does not
/// flee; at its withdraw line it also turns away in place.
#[test]
fn a_shredder_freezes_on_a_tremor_and_does_not_flee() {
    let world = world(29);
    let flora = Flora::new(FloraConfig::default());
    let c = config();
    let vg = c.founders[Founder::Blind.index()].vigilance;
    assert_eq!(vg.mode, VigilMode::Freeze, "the shipped shredder freezes");
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let s = body(&mut fauna, &world, Founder::Blind, 2.0, EAST, 0.0);
    let sp = cruise(&fauna, GRAZER);
    let walker = body(&mut fauna, &world, GRAZER, 2.4, EAST, sp);
    let felt = crate::tremor_of(&animal(&fauna, walker), 0.4, fauna.config.predation.tremor_near_m);
    assert!(felt >= vg.freeze_tremor, "the scene's footfall reaches the freeze line ({felt})");
    let act = respond(&mut fauna, &world, &flora, s, Actions { forward: 1.0, turn: 0.0, feed: 1.0 }).expect("it freezes");
    assert_eq!((act.forward, act.feed), (0.0, 0.0), "no step, no bite");
    assert_eq!(act.turn != 0.0, felt >= vg.withdraw_tremor, "turns away only at the withdraw line");
    assert_eq!(fauna.ledger.vigil.freezes[Founder::Blind.index()], 1);
}

// ------------------------------------------------------------------ the seedporter's eyes

/// The seedporter recognises a still threat farther than the frondgrazer does: a still
/// Veilhound ahead of both, between their still ranges, is seen by the seedporter only.
#[test]
fn a_seedporter_detects_a_still_threat_farther_than_a_frondgrazer() {
    let world = world(30);
    let c = config();
    let still = |f: Founder| {
        let t = c.founder(f).senses;
        t.sight_m * t.sight_still_fraction
    };
    let (g, s) = (still(GRAZER), still(Founder::Seedporter).min(5.5));
    assert!(s > g + 0.2, "the shipped seedporter's still range is the longer");
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let sp = body(&mut fauna, &world, Founder::Seedporter, 0.5, EAST, 0.0);
    let gr = body(&mut fauna, &world, GRAZER, 0.5, EAST, 0.0);
    let vh = body(&mut fauna, &world, VH, 0.5 + 0.5 * (g + s), WEST, 0.0);
    sense(&mut fauna, &world);
    assert!(modes(&fauna, sp, vh) & SIGHT != 0, "the seedporter sees it");
    assert!(modes(&fauna, gr, vh) & SIGHT == 0, "the frondgrazer does not");
}

// ------------------------------------------------------------------ the live switch

/// With its `live` switch off, each S2 lineage is back on S1: no record from the core, no
/// ground response, and the dodge reads the state column; the shredder flees its tremor by
/// the S1 rule instead of freezing.
#[test]
fn each_lineage_live_switch_off_restores_s1() {
    let world = world(31);
    let flora = Flora::new(FloraConfig::default());
    for f in [GRAZER, Founder::Loftstrider, Founder::Seedporter, Founder::Blind] {
        let mut c = config();
        c.founders[f.index()].senses.live = false;
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let me = body(&mut fauna, &world, f, 2.0, EAST, 0.0);
        let vh = body(&mut fauna, &world, VH, 3.0, WEST, 1.0);
        sense(&mut fauna, &world);
        assert!(fauna.percept(me).is_none_or(|p| p.tracks().is_empty()), "{}: the core leaves its record alone", f.name());
        assert!(respond(&mut fauna, &world, &flora, me, CROP).is_none(), "{}: no ground response", f.name());
        let mut holds = Percept::default();
        holds.tracks[0] = Track { known: true, id: vh, tick: fauna.tick, ..Track::default() };
        holds.n = 1;
        let prey = animal(&fauna, me);
        let cfg = &fauna.config;
        assert_eq!(crate::vigil::awareness(cfg, &prey, Some(&holds), vh, fauna.tick), cfg.predation.awareness(prey.state), "{}", f.name());
    }
    // The shredder: S1 flees its tremor, S2 freezes.
    for live in [false, true] {
        let mut c = config();
        c.founders[Founder::Blind.index()].senses.live = live;
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let s = body(&mut fauna, &world, Founder::Blind, 2.0, EAST, 0.0);
        let sp = 4.0 * cruise(&fauna, GRAZER);
        body(&mut fauna, &world, GRAZER, 2.3, EAST, sp);
        let i = fauna.animals.iter().position(|a| a.id == s).unwrap();
        let act = super::flee(&mut fauna, i, &world.view(), &flora.view(), None, CROP).expect("it answers the tremor");
        assert_eq!(act.forward > 0.0, !live, "live {live}: forward {}", act.forward);
    }
}
