//! **TEST-PASS (independent), S2 / HUNKER / GAIT**: tests written from the packages'
//! hand-backs and doc comments by a separate pass, aimed at boundaries, switch-off arms and
//! the joins between packages. A child of `step` beside the predation files, sharing their
//! fixture.
//!
//! Every case sets the config it relies on and checks the mechanism against it; no shipped
//! default, ratio or verdict is asserted.

use std::sync::atomic::Ordering;

use super::predation_fixture::*;
use crate::perception;
use crate::{Actions, Activity, Fauna, FaunaConfig, Founder, FounderState, Hunker, Percept, SenseTable, Sprint, StartingStores, State, Track, Vigil, VigilMode, Vigilance};
use cubarium_voxel::World;
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel_flora::{Flora, FloraConfig};

const GRAZER: Founder = Founder::Browser;
const VH: Founder = Founder::Chorister;
const LJ: Founder = Founder::Lanternjaw;
const CROP: Actions = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };

fn world(seed: u64) -> World {
    plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, seed, 16)
}

fn ring_of(world: &World) -> f64 {
    f64::from(world.view().config.width) * world.view().config.voxel_m
}

/// Every lineage awake, remote senses every tick, a grazer that sees all round to 8 m (still
/// bodies too) and hears nothing, its hunker off unless the case sets one.
fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[GRAZER.index()].hunker.enabled = false;
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.founders[GRAZER.index()].senses =
        SenseTable { live: true, sight_m: 8.0, sight_night_m: 8.0, sight_still_fraction: 1.0, sight_half_angle_deg: 180.0, ..SenseTable::default() };
    c
}

fn flight_only(start: f64, stop: f64, calm_s: f64) -> Vigilance {
    Vigilance { mode: VigilMode::Herd, flee_start_m: start, flee_stop_m: stop, calm_s, group_min: 1, run_noise: 1.0, ..Vigilance::default() }
}

/// A hunker entered at `enter`, left beyond `leave` after `calm_s`, hunger not shortening the
/// calm, no condition floor; the rest the lineage's.
fn hunker(enter: f64, leave: f64, calm_s: f64) -> Hunker {
    Hunker { enabled: true, enter_m: enter, leave_m: leave, calm_s, hunger_calm_exp: 0.0, min_condition: 0.0, herd_enabled: false, ..crate::vigil::hunker_of(GRAZER) }
}

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

fn condition(fauna: &mut Fauna, id: u64, fraction: f64) {
    let cap = fauna.config.founder(GRAZER).core.reserve_of(animal(fauna, id).body);
    animal_mut(fauna, id).reserve = fraction * cap;
}

fn adult(fauna: &mut Fauna, id: u64) {
    let birth = fauna.config.founder(GRAZER).core.birth_body;
    animal_mut(fauna, id).body = animal(fauna, id).body.max(birth);
}

fn sense(fauna: &mut Fauna, world: &World) {
    fauna.tick += 1;
    perception::sense_pass(fauna, &world.view(), false, 1.0, 1);
}

fn respond(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, held: Actions) -> Option<Actions> {
    let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
    let want = perception::Want { tremor: true, ..Default::default() };
    perception::refresh(fauna, i, &world.view(), &flora.view(), None, want);
    crate::vigil::respond(fauna, i, &world.view(), &flora.view(), held)
}

fn step(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64) -> Option<Actions> {
    sense(fauna, world);
    respond(fauna, world, flora, id, CROP)
}

fn vigil(fauna: &Fauna, id: u64) -> Vigil {
    fauna.percept(id).map(|p| p.vigil).unwrap_or_default()
}

fn set_hunkered(fauna: &mut Fauna, id: u64) {
    fauna.percepts.entry(id).or_default().vigil.hunkered = true;
    animal_mut(fauna, id).hunkered = true;
}

// ------------------------------------------------------------------ S2: flight's edges

/// **The start line is inclusive**: a moving hunter exactly at `flee_start_m` starts a flight;
/// the same hunter a hair beyond it does not.
#[test]
fn a_flight_starts_at_exactly_the_start_distance_and_not_a_hair_beyond() {
    let world = world(101);
    let flora = Flora::new(FloraConfig::default());
    let (start, stop, calm_s) = (1.5, 3.0, 1.0);
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = flight_only(start, stop, calm_s);
    for (off, flees) in [(start, true), (start + 0.01, false)] {
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
        body(&mut fauna, &world, VH, 2.0 + off, WEST, 1.0);
        step(&mut fauna, &world, &flora, g);
        assert_eq!(vigil(&fauna, g).fleeing, flees, "a hunter {off} m off (start {start} m)");
    }
}

/// **The calm runs exactly `calm_s`**: once the hunter has gone beyond the stop distance, the
/// flight holds for `calm_s` worth of ticks after the last close tick and ends on that tick.
#[test]
fn a_flight_ends_on_the_tick_its_calm_has_run_and_not_one_before() {
    let world = world(102);
    let flora = Flora::new(FloraConfig::default());
    let (start, stop, calm_s) = (1.5, 3.0, 0.5);
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = flight_only(start, stop, calm_s);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    let vh = body(&mut fauna, &world, VH, 3.0, WEST, 1.0);
    step(&mut fauna, &world, &flora, g);
    assert!(vigil(&fauna, g).fleeing, "the flight starts");
    let close = vigil(&fauna, g).close_tick;
    move_to(&mut fauna, vh, 2.0 + stop + 1.0);
    let calm = cubarium_voxel::ticks_at_least_one(calm_s);
    while fauna.tick + 1 < close + calm {
        step(&mut fauna, &world, &flora, g);
        assert!(vigil(&fauna, g).fleeing, "tick {}: inside the calm ({} ticks from {close})", fauna.tick, calm);
    }
    step(&mut fauna, &world, &flora, g);
    assert_eq!(fauna.tick, close + calm);
    assert!(!vigil(&fauna, g).fleeing, "the calm has run: the flight ends on tick {}", fauna.tick);
}

// ------------------------------------------------------------------ S2: the freeze's turn

/// **The shredder turns away from the side the footfall comes from**: a walker on its left
/// turns it one way, on its right the other, and in both cases the turn carries its heading
/// away from the walker.
#[test]
fn a_withdrawing_shredder_turns_away_from_the_side_the_footfall_comes_from() {
    let world = world(103);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[Founder::Blind.index()].senses.live = true;
    c.founders[Founder::Blind.index()].vigilance =
        Vigilance { mode: VigilMode::Freeze, freeze_tremor: 1e-9, withdraw_tremor: 2e-9, freeze_s: 1.0, ..Vigilance::default() };
    for side in [1.0, -1.0] {
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let s = body(&mut fauna, &world, Founder::Blind, 3.0, EAST, 0.0);
        let w = body(&mut fauna, &world, GRAZER, 3.0, EAST, 0.3);
        place(&mut fauna, w, 3.2, LANE_Z + side * 0.3, EAST);
        let act = respond(&mut fauna, &world, &flora, s, Actions { forward: 1.0, turn: 0.0, feed: 1.0 }).expect("it freezes");
        assert!(act.turn != 0.0, "side {side}: it withdraws");
        let h = animal(&fauna, s).pose.heading_rad;
        let toward = |heading: f64| 0.2 * heading.sin() + side * 0.3 * heading.cos();
        let turned = h + 0.1 * act.turn;
        assert!(toward(turned) < toward(h), "side {side}: turn {} carries it toward the walker", act.turn);
    }
}

// ------------------------------------------------------------------ S2: awareness

/// **Awareness's window is inclusive at `aware_within_s`**, a body in flight from the striker
/// is aware of it whatever its record holds, and an asleep live prey reads the table's asleep
/// value though its record holds the striker.
#[test]
fn awareness_holds_to_its_window_a_flight_keeps_it_and_sleep_overrides_the_record() {
    let mut c = config();
    c.predation.awareness_sensed = 0.9;
    c.predation.awareness_unaware = 0.1;
    let world = world(104);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    let striker = 777;
    let within = cubarium_voxel::ticks_at_least_one(fauna.config.predation.aware_within_s);
    let now = 10 * within;
    let mut rec = Percept::default();
    rec.tracks[0] = Track { known: true, id: striker, tick: now - within, ..Track::default() };
    rec.n = 1;
    let cfg = &fauna.config;
    let prey = animal(&fauna, g);
    assert_eq!(crate::vigil::awareness(cfg, &prey, Some(&rec), striker, now), cfg.predation.awareness_sensed, "at the window's edge");
    assert_eq!(crate::vigil::awareness(cfg, &prey, Some(&rec), striker, now + 1), cfg.predation.awareness_unaware, "one tick past");
    let mut fleeing = Percept::default();
    fleeing.vigil.fleeing = true;
    fleeing.vigil.flee_id = striker;
    assert_eq!(crate::vigil::awareness(cfg, &prey, Some(&fleeing), striker, now), cfg.predation.awareness_sensed, "in flight from it");
    assert_eq!(crate::vigil::awareness(cfg, &prey, Some(&fleeing), striker + 1, now), cfg.predation.awareness_unaware, "another striker");
    let mut asleep = prey;
    asleep.state = State::Asleep;
    assert_eq!(crate::vigil::awareness(cfg, &asleep, Some(&rec), striker, now), cfg.predation.awareness(State::Asleep));
}

// ------------------------------------------------------------------ HUNKER: entry, exit

/// **The condition floor**: a grazer whose reserve is under `min_condition` of its cap does not
/// hunker at a threat inside `enter_m`; at the floor it does.
#[test]
fn a_grazer_under_its_condition_floor_does_not_hunker_and_one_at_it_does() {
    let world = world(105);
    let flora = Flora::new(FloraConfig::default());
    let floor = 0.4;
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = flight_only(2.0, 4.0, 1.0);
    c.founders[GRAZER.index()].hunker = Hunker { min_condition: floor, ..hunker(1.5, 3.0, 1.0) };
    for (cond, hunkers) in [(floor - 0.05, false), (floor + 1e-9, true)] {
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
        condition(&mut fauna, g, cond);
        body(&mut fauna, &world, VH, 3.0, WEST, 1.0);
        step(&mut fauna, &world, &flora, g);
        assert_eq!(vigil(&fauna, g).hunkered, hunkers, "condition {cond}, floor {floor}");
        assert!(!vigil(&fauna, g).fleeing, "condition {cond}: the hunker lineage never flees");
    }
}

/// **The hunker replaces flight for every age**: a juvenile with a hunter inside `enter_m`
/// hunkers (not flees) though its herd rule would have a juvenile flee from one.
#[test]
fn a_juvenile_hunkers_instead_of_fleeing() {
    let world = world(106);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = Vigilance { group_min: 3, ..flight_only(2.0, 4.0, 1.0) };
    c.founders[GRAZER.index()].hunker = hunker(1.5, 3.0, 1.0);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    let birth = fauna.config.founder(GRAZER).core.birth_body;
    animal_mut(&mut fauna, g).body = 0.4 * birth;
    condition(&mut fauna, g, 1.0);
    body(&mut fauna, &world, VH, 3.0, WEST, 1.0);
    let act = step(&mut fauna, &world, &flora, g).expect("it answers");
    let v = vigil(&fauna, g);
    assert!(v.hunkered && !v.fleeing, "hunkered {} fleeing {}", v.hunkered, v.fleeing);
    assert_eq!((act.forward, act.feed), (0.0, 0.0));
}

/// **The leave line is inclusive**: an approaching hunter held at exactly `leave_m` keeps a
/// hunkered grazer down past any calm; a hair beyond it lets the calm run.
#[test]
fn a_hunter_at_exactly_the_leave_distance_holds_the_hunker() {
    let world = world(107);
    let flora = Flora::new(FloraConfig::default());
    let (enter, leave, calm_s) = (1.5, 3.0, 0.5);
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = flight_only(enter, leave, calm_s);
    c.founders[GRAZER.index()].hunker = hunker(enter, leave, calm_s);
    let calm = cubarium_voxel::ticks_at_least_one(calm_s);
    for (off, held) in [(leave, true), (leave + 0.01, false)] {
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
        condition(&mut fauna, g, 1.0);
        let vh = body(&mut fauna, &world, VH, 2.0 + enter, WEST, 1.0);
        step(&mut fauna, &world, &flora, g);
        assert!(vigil(&fauna, g).hunkered, "it hunkers");
        move_to(&mut fauna, vh, 2.0 + off);
        for _ in 0..calm + 3 {
            step(&mut fauna, &world, &flora, g);
        }
        assert_eq!(vigil(&fauna, g).hunkered, held, "a moving hunter {off} m off (leave {leave} m)");
    }
}

/// **A fresh hunker starts closed**: a grazer turned over by the pack, out of its hunker once
/// the pack has gone, that clamps down again at a new threat is under its shield, not bared
/// by the earlier turn-over (its pry restarts from 0 on entry).
#[test]
fn a_fresh_hunker_starts_under_its_shield_not_bared_by_an_earlier_turn_over() {
    let world = world(108);
    let flora = Flora::new(FloraConfig::default());
    let (enter, leave, calm_s) = (1.5, 3.0, 0.25);
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = flight_only(enter, leave, calm_s);
    c.founders[GRAZER.index()].hunker = Hunker { opened_s: 60.0, ..hunker(enter, leave, calm_s) };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    adult(&mut fauna, g);
    condition(&mut fauna, g, 1.0);
    let vh = body(&mut fauna, &world, VH, 2.0 + enter, WEST, 1.0);
    step(&mut fauna, &world, &flora, g);
    assert!(vigil(&fauna, g).hunkered);
    // The pack turned it over just now.
    let now = fauna.tick;
    fauna.percepts.entry(g).or_default().vigil.opened_until = now + cubarium_voxel::ticks_at_least_one(60.0);
    // The pack goes; the calm runs and it comes out.
    move_to(&mut fauna, vh, 2.0 + leave + 2.0);
    for _ in 0..cubarium_voxel::ticks_at_least_one(calm_s) + 3 {
        step(&mut fauna, &world, &flora, g);
    }
    assert!(!vigil(&fauna, g).hunkered, "out of the hunker");
    // A hunter comes in again: it clamps down.
    move_to(&mut fauna, vh, 2.0 + enter);
    step(&mut fauna, &world, &flora, g);
    assert!(vigil(&fauna, g).hunkered, "hunkered again");
    let guard = crate::vigil::guard(&fauna.config, &animal(&fauna, g), fauna.percept(g), fauna.tick);
    assert!(matches!(guard, crate::vigil::Guard::Shield(_)), "a fresh hunker meets the strike with its shield: {guard:?}");
}

/// **The switch off clears a stale hunker**: with the lineage's hunker off, a body loaded with
/// the hunker flags set meets a strike open (dodge and armour), is not pried, and its next
/// herd sample clears the flag.
#[test]
fn with_the_hunker_off_a_stale_hunkered_flag_is_open_unpried_and_cleared() {
    let world = world(109);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = flight_only(1.5, 3.0, 1.0);
    c.founders[GRAZER.index()].hunker = Hunker { enabled: false, pry_per_member_s: 10.0, resist_per_s: 0.0, ..hunker(1.5, 3.0, 1.0) };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 3.0, EAST, 0.0);
    set_hunkered(&mut fauna, g);
    for k in 0..3 {
        body(&mut fauna, &world, VH, 3.0 + 0.3 * (k as f64 - 1.0), WEST, 0.0);
    }
    let guard = crate::vigil::guard(&fauna.config, &animal(&fauna, g), fauna.percept(g), fauna.tick);
    assert_eq!(guard, crate::vigil::Guard::Open);
    let ring = ring_of(&world);
    for _ in 0..40 {
        fauna.tick += 1;
        crate::vigil::pry(&mut fauna, ring);
    }
    assert_eq!(vigil(&fauna, g).pry, 0.0, "no pry with the switch off");
    assert_eq!(vigil(&fauna, g).opened_until, 0);
    step(&mut fauna, &world, &flora, g);
    assert!(!vigil(&fauna, g).hunkered && !animal(&fauna, g).hunkered, "the herd sample clears it");
}

// ------------------------------------------------------------------ HUNKER: the pry

/// A hunkered adult grazer at x = 3 with Veilhounds at the given offsets along the lane, all
/// in contact; returns the fauna, the grazer and the hunters.
fn pry_scene(c: FaunaConfig, world: &World, offsets: &[f64]) -> (Fauna, u64, Vec<u64>) {
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, world, GRAZER, 3.0, EAST, 0.0);
    adult(&mut fauna, g);
    set_hunkered(&mut fauna, g);
    let hs = offsets.iter().map(|&o| body(&mut fauna, world, VH, 3.0 + o, WEST, 0.0)).collect();
    (fauna, g, hs)
}

/// **Who pries**: of three Veilhounds in contact, an asleep one and a pup add nothing, so the
/// progress over a window is the config's rate with one member: `n·p − R` per second.
#[test]
fn an_asleep_veilhound_and_a_pup_in_contact_add_no_pry() {
    let world = world(110);
    let (p, r) = (0.5, 0.2);
    let mut c = config();
    c.founders[GRAZER.index()].hunker = Hunker { pry_per_member_s: p, resist_per_s: r, ring_resist_per_kin: 0.0, opened_s: 1.0, ..hunker(1.5, 3.0, 1.0) };
    let (mut fauna, g, hs) = pry_scene(c, &world, &[0.4, -0.4, 0.0]);
    animal_mut(&mut fauna, hs[1]).state = State::Asleep;
    animal_mut(&mut fauna, hs[2]).pack = Some(crate::PackMember { pack: 999, pup: true, calling_ticks: 0 });
    let ring = ring_of(&world);
    let n = 10u64;
    for _ in 0..n {
        fauna.tick += 1;
        crate::vigil::pry(&mut fauna, ring);
    }
    let want = n as f64 * cubarium_voxel::DT * (p - r);
    let got = vigil(&fauna, g).pry;
    assert!((got - want).abs() < 1e-9, "progress {got}, one member's {want}");
}

/// **Only hunkered kin ring it**: a kin standing beside it unhunkered adds no resistance; the
/// same kin hunkered adds `ring_resist_per_kin` × the resistance.
#[test]
fn only_a_hunkered_kin_within_the_ring_adds_resistance() {
    let world = world(111);
    let (p, r, k) = (0.5, 0.2, 0.5);
    let mut c = config();
    c.founders[GRAZER.index()].hunker = Hunker { pry_per_member_s: p, resist_per_s: r, ring_resist_per_kin: k, ring_kin_max: 3, ring_m: 1.0, ..hunker(1.5, 3.0, 1.0) };
    let ring = ring_of(&world);
    let n = 10u64;
    for kin_hunkered in [false, true] {
        let (mut fauna, g, _) = pry_scene(c.clone(), &world, &[0.4]);
        let kin = body(&mut fauna, &world, GRAZER, 3.0, EAST, 0.0);
        adult(&mut fauna, kin);
        place(&mut fauna, kin, 3.0, LANE_Z + 0.6, EAST);
        if kin_hunkered {
            set_hunkered(&mut fauna, kin);
        }
        for _ in 0..n {
            fauna.tick += 1;
            crate::vigil::pry(&mut fauna, ring);
        }
        let resist = if kin_hunkered { r * (1.0 + k) } else { r };
        let want = (n as f64 * cubarium_voxel::DT * (p - resist)).max(0.0);
        let got = vigil(&fauna, g).pry;
        assert!((got - want).abs() < 1e-9, "kin hunkered {kin_hunkered}: progress {got}, want {want}");
    }
}

/// **At balance it never opens**: members whose summed pry equals the resistance exactly make
/// no progress, and [`Hunker::members_to_open`] asks one more.
#[test]
fn members_whose_pry_exactly_balances_the_resistance_never_open_it() {
    let world = world(112);
    let p = 0.25;
    let h = Hunker { pry_per_member_s: p, resist_per_s: 2.0 * p, ring_resist_per_kin: 0.0, ..hunker(1.5, 3.0, 1.0) };
    let birth = config().founder(GRAZER).core.birth_body;
    assert_eq!(h.members_to_open(birth, birth, 0), Some(3), "two balance it; three open it");
    let mut c = config();
    c.founders[GRAZER.index()].hunker = h;
    let (mut fauna, g, _) = pry_scene(c, &world, &[0.4, -0.4]);
    let ring = ring_of(&world);
    for _ in 0..cubarium_voxel::ticks_at_least_one(60.0) {
        fauna.tick += 1;
        crate::vigil::pry(&mut fauna, ring);
    }
    assert!(!crate::vigil::opened(&fauna, g) && vigil(&fauna, g).pry < 1e-9, "progress {}", vigil(&fauna, g).pry);
}

/// **No progress while open, and it resumes after**: during the `opened_s` after a turn-over
/// the progress stays at 0 however many pry; once open time has run (still hunkered) it
/// builds again at the config's rate.
#[test]
fn progress_does_not_build_while_it_is_open_and_resumes_after() {
    let world = world(113);
    let (p, r, open_s) = (2.0, 0.5, 0.5);
    let mut c = config();
    c.founders[GRAZER.index()].hunker = Hunker { pry_per_member_s: p, resist_per_s: r, ring_resist_per_kin: 0.0, opened_s: open_s, ..hunker(1.5, 3.0, 1.0) };
    let (mut fauna, g, _) = pry_scene(c, &world, &[0.4]);
    let ring = ring_of(&world);
    let mut opened_at = None;
    for _ in 0..cubarium_voxel::ticks_at_least_one(10.0) {
        fauna.tick += 1;
        crate::vigil::pry(&mut fauna, ring);
        if crate::vigil::opened(&fauna, g) {
            opened_at = Some(fauna.tick);
            break;
        }
    }
    let at = opened_at.expect("one member above the resistance opens it");
    let until = vigil(&fauna, g).opened_until;
    assert_eq!(until, at + cubarium_voxel::ticks_at_least_one(open_s), "open for opened_s");
    while fauna.tick + 1 < until {
        fauna.tick += 1;
        crate::vigil::pry(&mut fauna, ring);
        assert_eq!(vigil(&fauna, g).pry, 0.0, "tick {}: no progress while open", fauna.tick);
    }
    fauna.tick += 1;
    crate::vigil::pry(&mut fauna, ring);
    let want = cubarium_voxel::DT * (p - r);
    assert!((vigil(&fauna, g).pry - want).abs() < 1e-12, "it resumes: {}", vigil(&fauna, g).pry);
    assert_eq!(fauna.ledger.vigil.opens[GRAZER.index()], 1);
}

// ------------------------------------------------------------------ HUNKER: the strike

/// One lanternjaw strike on an adult grazer that `hunkered` (and `opened`), with the config
/// edited by `edit`; `(hits, dodges, glances, wound)` after it resolves.
fn strike_on(hunkered: bool, opened: bool, edit: impl Fn(&mut FaunaConfig)) -> (u64, u64, u64, f64) {
    let world = clock_off(114);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = hunting_config();
    edit(&mut c);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = introduce(&mut fauna, &world, GRAZER, StartingStores::FULL);
    adult(&mut fauna, g);
    place(&mut fauna, g, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    drive(&mut fauna, g, 0.0, !hunkered);
    if hunkered {
        set_hunkered(&mut fauna, g);
    }
    if opened {
        fauna.percepts.entry(g).or_default().vigil.opened_until = u64::MAX / 2;
    }
    let lj = introduce(&mut fauna, &world, LJ, HUNTING);
    place(&mut fauna, lj, STRIKER_X, LANE_Z, EAST);
    let feed = drive(&mut fauna, lj, 0.0, false);
    feed.store(true, Ordering::Relaxed);
    let before = strikes(&fauna);
    let resolved = |f: &Fauna| strikes(f) > before && phase(f, lj) == Some(crate::StrikePhase::Recovery);
    step_until(&mut fauna, &world, &mut flora, 60, resolved).expect("the lanternjaw strikes");
    (hits(&fauna), dodged(&fauna), fauna.ledger.vigil.glances[GRAZER.index()], fauna.view().animal(g).map_or(1.0, |a| a.wound))
}

/// A grazer that dodges every strike it can: agility 1, full awareness whatever it knows.
fn sure_dodger(c: &mut FaunaConfig) {
    c.founders[GRAZER.index()].agility = 1.0;
    c.predation.awareness_sensed = 1.0;
    c.predation.awareness_unaware = 1.0;
    c.predation.dodge_rear_aspect = 1.0;
    c.predation.dodge_condition_floor = 1.0;
    c.founders[LJ.index()].hunter.as_mut().expect("a hunter").strike_power = 0.05;
}

/// **No dodge while hunkered**: a grazer that would dodge every strike open is hit through a
/// shield of 0 when hunkered (no glance, no dodge); the control open grazer dodges.
#[test]
fn a_hunkered_grazer_never_dodges_even_when_it_would_dodge_every_strike_open() {
    let edit = |c: &mut FaunaConfig| {
        sure_dodger(c);
        c.founders[GRAZER.index()].hunker = Hunker { enabled: true, shield: 0.0, herd_enabled: false, ..crate::vigil::hunker_of(GRAZER) };
    };
    let (hits_open, dodges_open, _, _) = strike_on(false, false, edit);
    assert_eq!((hits_open, dodges_open), (0, 1), "control: open, it dodges");
    let (hits_down, dodges_down, glances, wound) = strike_on(true, false, edit);
    assert_eq!((hits_down, dodges_down, glances), (1, 0, 0), "hunkered under a shield of 0: hit");
    assert!(wound > 0.0);
}

/// **Turned over, the armour is gone**: the same strike on a hunkered grazer through a shield
/// of 0 wounds it `1 / (1 − armour)` times less than on the same grazer turned over.
#[test]
fn a_turned_over_grazer_takes_the_strike_without_its_armour() {
    let armour = 0.5;
    let edit = |c: &mut FaunaConfig| {
        sure_dodger(c);
        c.founders[GRAZER.index()].armour = armour;
        c.founders[GRAZER.index()].hunker = Hunker { enabled: true, shield: 0.0, herd_enabled: false, ..crate::vigil::hunker_of(GRAZER) };
    };
    let (_, _, _, shielded) = strike_on(true, false, edit);
    let (_, dodges, glances, bared) = strike_on(true, true, edit);
    assert_eq!((dodges, glances), (0, 0), "turned over: no dodge, no glance");
    assert!(shielded > 0.0 && bared > 0.0);
    assert!((bared * (1.0 - armour) - shielded).abs() < 1e-9 * bared, "bared {bared}, under the shield {shielded}, armour {armour}");
}

/// **A shield of 1 always glances**: every strike on a hunkered grazer under a certain shield
/// glances off and wounds nothing.
#[test]
fn a_certain_shield_turns_every_strike() {
    let edit = |c: &mut FaunaConfig| {
        sure_dodger(c);
        c.founders[GRAZER.index()].hunker = Hunker { enabled: true, shield: 1.0, wound_opens: 0.0, herd_enabled: false, ..crate::vigil::hunker_of(GRAZER) };
    };
    let (hits, dodges, glances, wound) = strike_on(true, false, edit);
    assert_eq!((hits, dodges, glances), (0, 0, 1));
    assert_eq!(wound, 0.0);
}

// ------------------------------------------------------------------ HUNKER × GAIT × S2

/// **A sprinting Veilhound heard, not seen, within `enter_m` makes it hunker**: a grazer that
/// sees nothing and hears a running hunter clamps down.
#[test]
fn a_grazer_that_only_hears_a_sprinting_veilhound_inside_enter_hunkers() {
    let world = world(115);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[GRAZER.index()].senses = SenseTable { live: true, hearing_m: 6.0, hearing_min: 1e-9, ..SenseTable::default() };
    c.founders[GRAZER.index()].vigilance = flight_only(2.0, 4.0, 1.0);
    c.founders[GRAZER.index()].hunker = hunker(1.5, 3.0, 1.0);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    condition(&mut fauna, g, 1.0);
    let rush = crate::gait::sprint_forward(&fauna.config, VH) * fauna.config.manifest(VH).cruise_m_per_s;
    let vh = body(&mut fauna, &world, VH, 3.2, WEST, rush);
    step(&mut fauna, &world, &flora, g);
    let m = fauna.percept(g).and_then(|p| p.track(vh)).map_or(0, |t| t.modes);
    assert!(m & perception::HEARING != 0 && m & perception::SIGHT == 0, "heard only: modes {m}");
    assert!(vigil(&fauna, g).hunkered, "it hunkers");
}

/// **The stalk does not rush a hunkered grazer from afar**: a hunkered prey is not running, so
/// a Veilhound beyond its pack's `commit_m` stalks it (under the rush), and within it rushes
/// at its sprint.
#[test]
fn a_veilhound_stalks_a_hunkered_grazer_beyond_commit_and_rushes_within() {
    let world = world(116);
    let mut c = config();
    c.founders[GRAZER.index()].senses = SenseTable { live: true, hearing_m: 20.0, hearing_min: 1.0, ..SenseTable::default() };
    c.founders[GRAZER.index()].hunker = hunker(1.5, 3.0, 1.0);
    let pk = c.founders[VH.index()].pack.as_mut().expect("a pack lineage");
    pk.stalk_margin = 0.7;
    pk.stalk_freeze = false;
    pk.commit_m = 1.0;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    set_hunkered(&mut fauna, g);
    fauna.percepts.entry(g).or_default().vigil.bolt_until = fauna.tick;
    let vh = body(&mut fauna, &world, VH, 6.0, WEST, 1.0);
    let rush = crate::gait::sprint_forward(&fauna.config, VH);
    assert!(rush > 1.0, "the case needs a sprint");
    let a = animal(&fauna, vh);
    let t = Track { known: true, id: g, modes: perception::HEARING, ..Track::default() };
    let far = crate::vigil::stalk(&mut fauna, &a, &t, 4.0, 1.0).expect("the pack stalks");
    assert!(far < rush, "beyond commit: stalking at {far}, not the rush {rush}");
    let near = crate::vigil::stalk(&mut fauna, &a, &t, 0.5, 1.0).expect("the pack stalks");
    assert!((near - rush).abs() < 1e-12, "within commit: the rush {near}");
}

// ------------------------------------------------------------------ GAIT: the gate's edges

fn state() -> FounderState {
    FounderState::default()
}

/// **No sprint off its feet**: asked for a sprint while `may` is false (on a wall) the gate
/// holds it to cruise and its stamina rests at the config's rate.
#[test]
fn the_gate_refuses_a_sprint_off_its_feet_and_rests_instead() {
    let s = Sprint { body_lengths_per_s: 3.0, endurance_s: 2.0, recovery_s: 4.0, resume_fraction: 0.5 };
    let mut fs = state();
    fs.sprint_spent = 0.5;
    let dt = 0.05;
    let (f, w) = s.gate(1.0, &mut fs, 3.0, false, dt);
    assert_eq!((f, w), (1.0, false));
    assert!((fs.sprint_spent - (0.5 - dt / s.recovery_s)).abs() < 1e-12, "rested: {}", fs.sprint_spent);
}

/// **A top speed at the cruise is no sprint**: `effort` is 1 and the gate holds any ask to
/// cruise, spending nothing.
#[test]
fn a_sprint_no_faster_than_cruise_is_no_sprint() {
    for top in [0.5, 1.0] {
        let s = Sprint { body_lengths_per_s: top, endurance_s: 2.0, recovery_s: 4.0, resume_fraction: 0.5 };
        assert_eq!(s.effort(1.0), 1.0);
        let mut fs = state();
        let (f, w) = s.gate(1.0, &mut fs, 2.5, true, 0.05);
        assert_eq!((f, w, fs.sprint_spent), (1.0, false, 0.0), "top {top}");
    }
}

/// **The resume line is inclusive**: a winded body sprints again on the tick its stamina is
/// back to exactly `resume_fraction`; a recovery of 0 rests it at once.
#[test]
fn a_winded_body_resumes_at_exactly_its_resume_fraction_and_a_zero_recovery_rests_at_once() {
    let dt = 0.25;
    let s = Sprint { body_lengths_per_s: 3.0, endurance_s: 1.0, recovery_s: 2.0, resume_fraction: 0.5 };
    let mut fs = FounderState { sprint_spent: 1.0, winded: true, ..state() };
    // Four rests of 0.25 s at 1/2 per second: 1.0 → 0.5 spent, 0.5 rested = resume.
    for k in 0..3 {
        s.rest(&mut fs, dt);
        assert!(fs.winded, "rest {k}: spent {}", fs.sprint_spent);
    }
    s.rest(&mut fs, dt);
    assert!(!fs.winded, "rested to resume_fraction: spent {}", fs.sprint_spent);
    let z = Sprint { recovery_s: 0.0, ..s };
    let mut fs = FounderState { sprint_spent: 1.0, winded: true, ..state() };
    z.rest(&mut fs, dt);
    assert_eq!((fs.sprint_spent, fs.winded), (0.0, false));
}

/// **The wound's slow comes before the gate**: a wounded Veilhound asking its rush whose slow
/// brings the ask to cruise or under spends no stamina and moves at the slowed speed.
#[test]
fn a_wound_that_slows_a_rush_under_cruise_spends_no_stamina() {
    let world = clock_off(117);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    let bl = c.founder(VH).cruise_body_lengths_per_s;
    c.founders[VH.index()].sprint = Sprint { body_lengths_per_s: 2.0 * bl, endurance_s: 5.0, recovery_s: 10.0, resume_fraction: 0.5 };
    c.predation.wound_slow = 1.0;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let vh = introduce(&mut fauna, &world, VH, StartingStores::FULL);
    place(&mut fauna, vh, 2.0, LANE_Z, EAST);
    let rush = crate::gait::sprint_forward(&fauna.config, VH);
    // slow = 1 − wound: with the wound at 1 − 0.8/rush the slowed ask is 0.8 × cruise.
    animal_mut(&mut fauna, vh).wound = 1.0 - 0.8 / rush;
    let cad = VH.manifest().cadence_ticks().max(3);
    let cruise = fauna.config.manifest(VH).cruise_m_per_s;
    for _ in 0..5 {
        let a = animal_mut(&mut fauna, vh);
        a.age_ticks = 10 * cad + 1;
        a.founder_state.held = Actions { forward: rush, turn: 0.0, feed: 0.0 };
        fauna.step_with(&world, &mut flora, 1);
    }
    let a = animal(&fauna, vh);
    assert_eq!(a.founder_state.sprint_spent, 0.0, "no stamina spent on a slowed ask");
    assert!((a.speed_m_s - 0.8 * cruise).abs() < 0.02 * cruise, "speed {} against {}", a.speed_m_s, 0.8 * cruise);
    assert_eq!(fauna.ledger.sprint_ticks_by_founder[VH.index()], 0);
}

/// **A dash only while the threat is close**: [`crate::gait::dash`] multiplies the forward by
/// the lineage's sprint only when `close`.
#[test]
fn a_ground_dash_runs_at_the_sprint_only_while_the_threat_is_close() {
    let mut c = config();
    let bl = c.founder(Founder::Seedporter).cruise_body_lengths_per_s;
    c.founders[Founder::Seedporter.index()].sprint = Sprint { body_lengths_per_s: 3.0 * bl, endurance_s: 5.0, recovery_s: 10.0, resume_fraction: 0.5 };
    let act = Actions { forward: 0.5, turn: 0.25, feed: 0.0 };
    let near = crate::gait::dash(&c, Founder::Seedporter, true, act);
    let far = crate::gait::dash(&c, Founder::Seedporter, false, act);
    assert!((near.forward - 1.5).abs() < 1e-12 && near.turn == act.turn, "{near:?}");
    assert_eq!(far, act);
}

// ------------------------------------------------------------------ conservation

fn books_close(fauna: &Fauna, flora: &Flora, at: &str) {
    let (av, fv) = (fauna.view(), flora.view());
    for (name, got, want) in [
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
    ] {
        assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "{at}: the {name} ledger {got} against {want}");
    }
}

/// **The books close through a hunker, a pry-open and the kill**: a hunkered adult worked by
/// Veilhounds in contact (a pry fast enough to open it in a few ticks) is turned over inside
/// the step, then killed by a hit from one of them; both layers' ledgers close on every tick
/// and after the kill, and the kill is booked to the adult class.
#[test]
fn the_books_close_through_a_hunker_a_pry_open_and_the_kill() {
    let world = clock_off(118);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[GRAZER.index()].hunker = Hunker { pry_per_member_s: 5.0, resist_per_s: 1.0, ring_resist_per_kin: 0.0, opened_s: 30.0, ..hunker(1.5, 3.0, 1.0) };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 3.0, EAST, 0.0);
    adult(&mut fauna, g);
    set_hunkered(&mut fauna, g);
    for o in [0.4, -0.4] {
        body(&mut fauna, &world, VH, 3.0 + o, WEST, 0.0);
    }
    books_close(&fauna, &flora, "before");
    let opened = step_until(&mut fauna, &world, &mut flora, 40, |f| {
        crate::vigil::opened(f, g)
    });
    assert!(opened.is_some(), "the pry opens it inside the step");
    books_close(&fauna, &flora, "opened");
    assert!(fauna.ledger.vigil.hunker_ticks[GRAZER.index()] > 0 && fauna.ledger.vigil.opens[GRAZER.index()] == 1);
    let birth = fauna.config.founder(GRAZER).core.birth_body;
    assert!(animal(&fauna, g).body >= birth);
    super::apply_hit(&mut fauna, &mut flora, g, 1.0, VH);
    assert!(fauna.view().animal(g).is_none(), "killed");
    assert_eq!(fauna.ledger.vigil.kills_adult[GRAZER.index()], 1);
    books_close(&fauna, &flora, "after the kill");
    for _ in 0..5 {
        fauna.step_with(&world, &mut flora, 1);
        books_close(&fauna, &flora, "after");
    }
}
