//! **HUNKER** (Wrysk 2026-10-06, the frondgrazer's armour defence, and S2's bite collapse):
//! the hunker's hysteresis and its cost (no feeding), hunger shortening it, the shield on a
//! strike, armour growing with the body, the pack's work on a hunkered target, the alarm
//! bunching kin, the switch restoring S2, a still hunter no longer holding a flight, and the
//! scan pause costing no more than its duty share of cropping.
//!
//! Every case builds its scene by hand and checks a mechanism against the config it sets or
//! reads; no default, ratio or population is asserted.

use std::sync::atomic::Ordering;

use super::predation_fixture::*;
use crate::perception;
use crate::{Actions, Activity, Fauna, FaunaConfig, Founder, Hunker, SenseTable, StartingStores, Vigil, VigilMode, Vigilance};
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

/// Every lineage awake, remote senses every tick, a grazer that sees all round (still bodies
/// too) and hears nothing, its hunker off unless the case sets one.
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

/// A herd response with only flight.
fn flight_only(start: f64, stop: f64, calm_s: f64) -> Vigilance {
    Vigilance { mode: VigilMode::Herd, flee_start_m: start, flee_stop_m: stop, calm_s, group_min: 1, run_noise: 1.0, ..Vigilance::default() }
}

/// A hunker entered at `enter`, left beyond `leave` after `calm_s` (at full condition), with
/// hunger shortening the calm linearly and no condition floor.
fn hunker(enter: f64, leave: f64, calm_s: f64) -> Hunker {
    Hunker { enabled: true, enter_m: enter, leave_m: leave, calm_s, hunger_calm_exp: 1.0, min_condition: 0.0, herd_enabled: false, ..crate::vigil::hunker_of(GRAZER) }
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

/// Its condition (reserve over its cap) set to `fraction`.
fn condition(fauna: &mut Fauna, id: u64, fraction: f64) {
    let cap = fauna.config.founder(GRAZER).core.reserve_of(animal(fauna, id).body);
    animal_mut(fauna, id).reserve = fraction * cap;
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

fn vigil(fauna: &Fauna, id: u64) -> Vigil {
    fauna.percept(id).map(|p| p.vigil).unwrap_or_default()
}

// ------------------------------------------------------------------ the hunker

/// A grazer that senses a Veilhound within the enter distance hunkers: it stands, does not
/// crop, and its body shows the flag; it stays hunkered with the threat back between enter
/// and leave, and beyond leave until the calm time has passed; then it comes out.
#[test]
fn a_grazer_hunkers_within_the_enter_distance_and_leaves_beyond_the_leave_distance_after_the_calm() {
    let world = world(41);
    let flora = Flora::new(FloraConfig::default());
    let (enter, leave, calm_s) = (1.5, 3.0, 1.0);
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = flight_only(enter, leave, calm_s);
    c.founders[GRAZER.index()].hunker = hunker(enter, leave, calm_s);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    condition(&mut fauna, g, 1.0);
    let vh = body(&mut fauna, &world, VH, 2.0 + 0.5 * (enter + leave), WEST, 1.0);
    let step = |fauna: &mut Fauna| {
        sense(fauna, &world);
        respond(fauna, &world, &flora, g, CROP)
    };
    step(&mut fauna);
    assert!(!vigil(&fauna, g).hunkered, "between enter and leave: not yet");
    move_to(&mut fauna, vh, 2.0 + 0.5 * enter);
    let act = step(&mut fauna).expect("within the enter distance it hunkers");
    assert!(vigil(&fauna, g).hunkered && animal(&fauna, g).hunkered, "hunkered, and the body shows it");
    assert!(act.feed == 0.0 && act.forward == 0.0, "a hunkered grazer stands and does not eat");
    assert!(!vigil(&fauna, g).fleeing, "it hunkers instead of fleeing");
    move_to(&mut fauna, vh, 2.0 + 0.5 * (enter + leave));
    let act = step(&mut fauna).expect("still hunkered");
    assert!(vigil(&fauna, g).hunkered && act.feed == 0.0, "between enter and leave it stays down");
    move_to(&mut fauna, vh, 2.0 + leave + 1.0);
    step(&mut fauna);
    assert!(vigil(&fauna, g).hunkered, "beyond leave, before the calm: still down");
    for _ in 0..cubarium_voxel::ticks_at_least_one(calm_s) + 1 {
        step(&mut fauna);
    }
    assert!(!vigil(&fauna, g).hunkered && !animal(&fauna, g).hunkered, "after the calm it comes out");
    assert_eq!(fauna.ledger.vigil.hunkers[GRAZER.index()], 1, "one hunker");
}

/// Risk allocation: with the threat gone beyond the leave distance, a hungry grazer comes out
/// of its hunker sooner than a sated one, all else equal.
#[test]
fn a_hungrier_grazer_leaves_the_hunker_sooner() {
    let world = world(42);
    let flora = Flora::new(FloraConfig::default());
    let (enter, leave, calm_s) = (1.5, 3.0, 2.0);
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = flight_only(enter, leave, calm_s);
    c.founders[GRAZER.index()].hunker = hunker(enter, leave, calm_s);
    let samples_to_leave = |fullness: f64| -> u64 {
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
        condition(&mut fauna, g, fullness);
        let vh = body(&mut fauna, &world, VH, 2.0 + 0.5 * enter, WEST, 1.0);
        sense(&mut fauna, &world);
        respond(&mut fauna, &world, &flora, g, CROP);
        assert!(vigil(&fauna, g).hunkered, "fullness {fullness}: it hunkers");
        move_to(&mut fauna, vh, 2.0 + leave + 1.0);
        for n in 1..=10 * cubarium_voxel::ticks_at_least_one(calm_s) {
            sense(&mut fauna, &world);
            respond(&mut fauna, &world, &flora, g, CROP);
            if !vigil(&fauna, g).hunkered {
                return n;
            }
        }
        panic!("fullness {fullness}: never came out");
    };
    let (hungry, sated) = (samples_to_leave(0.25), samples_to_leave(1.0));
    assert!(hungry < sated, "hungry {hungry} samples, sated {sated}");
}

// ------------------------------------------------------------------ the shield

/// The same lanternjaw strike on the same adult grazer: hunkered, under a shield the config
/// makes certain, it takes no wound; cropping with its neck out (no dodge possible) it does.
/// A juvenile's shield is lower than an adult's.
#[test]
fn a_strike_on_a_hunkered_adult_harms_less_than_on_a_cropping_one_and_juvenile_armour_is_lower() {
    let wound_after_strike = |hunkered: bool| -> f64 {
        let world = clock_off(43);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = hunting_config();
        c.founders[GRAZER.index()].agility = 0.0;
        c.founders[GRAZER.index()].hunker = Hunker { enabled: true, shield: 1.0, wound_opens: 0.0, herd_enabled: false, ..crate::vigil::hunker_of(GRAZER) };
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let g = introduce(&mut fauna, &world, GRAZER, StartingStores::FULL);
        let birth = fauna.config.founder(GRAZER).core.birth_body;
        animal_mut(&mut fauna, g).body = animal(&fauna, g).body.max(birth);
        place(&mut fauna, g, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
        drive(&mut fauna, g, 0.0, !hunkered);
        if hunkered {
            fauna.percepts.entry(g).or_default().vigil.hunkered = true;
            animal_mut(&mut fauna, g).hunkered = true;
        }
        let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, HUNTING);
        place(&mut fauna, lj, STRIKER_X, LANE_Z, EAST);
        let feed = drive(&mut fauna, lj, 0.0, false);
        feed.store(true, Ordering::Relaxed);
        let before = strikes(&fauna);
        let resolved = |f: &Fauna| strikes(f) > before && phase(f, lj) == Some(crate::StrikePhase::Recovery);
        step_until(&mut fauna, &world, &mut flora, 60, resolved).expect("the lanternjaw strikes");
        assert!(hits(&fauna) + fauna.ledger.vigil.glances[GRAZER.index()] > 0, "the strike met the grazer");
        animal(&fauna, g).wound
    };
    let (down, out) = (wound_after_strike(true), wound_after_strike(false));
    assert!(down < out, "hunkered wound {down}, cropping wound {out}");

    let h = Hunker { herd_enabled: false, ..crate::vigil::hunker_of(GRAZER) };
    let birth = FaunaConfig::default().founder(GRAZER).core.birth_body;
    assert!(h.shield_of(0.3 * birth, birth, 0.0) < h.shield_of(birth, birth, 0.0), "a juvenile's armour is not hard yet");
    assert!(h.shield_of(birth, birth, 0.5) <= h.shield_of(birth, birth, 0.0), "a wound does not harden it");
}

// ------------------------------------------------------------------ the pack's work

/// A hunkered adult worked by one Veilhound in contact is not opened within a long window;
/// worked by enough members (more than the config's resistance over one member's rate) it is.
#[test]
fn one_veilhound_cannot_open_a_hunkered_adult_and_enough_pack_members_can() {
    let world = world(44);
    let mut c = config();
    let h = Hunker { enabled: true, pry_per_member_s: 0.5, resist_per_s: 0.8, ring_resist_per_kin: 0.0, herd_enabled: false, ..crate::vigil::hunker_of(GRAZER) };
    c.founders[GRAZER.index()].hunker = h;
    assert!(h.pry_per_member_s < h.resist_per_s, "the case needs one member below the resistance");
    let enough = (h.resist_per_s / h.pry_per_member_s).floor() as usize + 1;
    let net = enough as f64 * h.pry_per_member_s - h.resist_per_s;
    let opens_in = cubarium_voxel::ticks_at_least_one(1.0 / net);
    let ring = f64::from(world.view().config.width) * world.view().config.voxel_m;
    let opened_within = |members: usize, ticks: u64| -> bool {
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let g = body(&mut fauna, &world, GRAZER, 3.0, EAST, 0.0);
        let birth = fauna.config.founder(GRAZER).core.birth_body;
        animal_mut(&mut fauna, g).body = animal(&fauna, g).body.max(birth);
        fauna.percepts.entry(g).or_default().vigil.hunkered = true;
        animal_mut(&mut fauna, g).hunkered = true;
        for k in 0..members {
            // In contact: beside it, nose to its flank, around it.
            let id = body(&mut fauna, &world, VH, 3.0, EAST, 0.0);
            let (a, r) = (k as f64 * std::f64::consts::TAU / members as f64, 0.5);
            let z = LANE_Z + r * a.cos();
            let x = 3.0 + r * a.sin();
            place(&mut fauna, id, x, z, a + std::f64::consts::PI);
        }
        for _ in 0..ticks {
            fauna.tick += 1;
            crate::vigil::pry(&mut fauna, ring);
            if crate::vigil::opened(&fauna, g) {
                return true;
            }
        }
        false
    };
    assert!(!opened_within(1, 20 * opens_in), "one member: not opened in {} ticks", 20 * opens_in);
    assert!(opened_within(enough, 2 * opens_in), "{enough} members: opened within {} ticks", 2 * opens_in);
}

/// Whether a grazer of `body_fraction` × the adult body, hunkered alone under `h` and worked
/// by `members` Veilhounds in contact around it, is opened within `ticks` (the pry alone).
fn opened_by(h: Hunker, members: usize, body_fraction: f64, ticks: u64) -> bool {
    opened_ringed(h, members, 0, body_fraction, ticks)
}

/// As [`opened_by`], with `kin` hunkered grazers standing beside the worked body, inside its
/// `ring_m`.
fn opened_ringed(h: Hunker, members: usize, kin: usize, body_fraction: f64, ticks: u64) -> bool {
    let world = world(44);
    let mut c = config();
    c.founders[GRAZER.index()].hunker = h;
    let ring = f64::from(world.view().config.width) * world.view().config.voxel_m;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 3.0, EAST, 0.0);
    let birth = fauna.config.founder(GRAZER).core.birth_body;
    animal_mut(&mut fauna, g).body = body_fraction * birth;
    fauna.percepts.entry(g).or_default().vigil.hunkered = true;
    animal_mut(&mut fauna, g).hunkered = true;
    for k in 0..kin {
        let a = k as f64 * std::f64::consts::TAU / kin.max(1) as f64;
        let r = 0.5 * h.ring_m;
        let id = body(&mut fauna, &world, GRAZER, 3.0, EAST, 0.0);
        place(&mut fauna, id, 3.0 + r * a.cos(), LANE_Z + r * a.sin(), 0.0);
        fauna.percepts.entry(id).or_default().vigil.hunkered = true;
        animal_mut(&mut fauna, id).hunkered = true;
    }
    for k in 0..members {
        let id = body(&mut fauna, &world, VH, 3.0, EAST, 0.0);
        let (a, r) = (k as f64 * std::f64::consts::TAU / members as f64, 0.5);
        place(&mut fauna, id, 3.0 + r * a.sin(), LANE_Z + r * a.cos(), a + std::f64::consts::PI);
    }
    for _ in 0..ticks {
        fauna.tick += 1;
        crate::vigil::pry(&mut fauna, ring);
        if crate::vigil::opened(&fauna, g) {
            return true;
        }
    }
    false
}

/// **The armour rule** (PACK-TUNE, Wrysk: "one Veilhound can't open an adult; a pack can,
/// slowly"): with its own pry and resistance rates, `n` members in contact open a hunkered
/// adult exactly when `n × pry_per_member_s` exceeds `resist_per_s × (1 + ring_resist_per_kin
/// × kin)`, alone and with hunkered kin beside it. No shipped rate is assumed.
#[test]
fn members_open_a_hunkered_adult_exactly_when_their_pry_beats_its_ringed_resistance() {
    let (pry, resist, per_kin) = (0.5, 1.2, 0.5);
    let h = Hunker {
        enabled: true,
        pry_per_member_s: pry,
        resist_per_s: resist,
        ring_resist_per_kin: per_kin,
        ring_m: 1.5,
        ring_kin_max: 3,
        opened_s: 10.0,
        herd_enabled: false, ..crate::vigil::hunker_of(GRAZER)
    };
    let long = cubarium_voxel::ticks_at_least_one(120.0);
    let mut seen = [false; 2];
    for kin in 0..=1usize {
        let bar = resist * (1.0 + per_kin * kin as f64);
        for n in 1..=5usize {
            let opens = n as f64 * pry > bar;
            seen[usize::from(opens)] = true;
            assert_eq!(opened_ringed(h, n, kin, 1.0, long), opens, "{n} members, {kin} kin: pry {} against {bar}", n as f64 * pry);
        }
        let need = h.members_to_open(1.0, 1.0, kin as u32).unwrap() as f64;
        assert!(need * pry > bar && (need - 1.0) * pry <= bar, "members_to_open agrees with the rule");
    }
    assert!(seen[0] && seen[1], "the case covers both sides of the rule");
}

/// **The young are soft to prying too**: the resistance grows with the body as the shield does,
/// so a half-grown juvenile needs fewer members than an adult ([`Hunker::members_to_open`]),
/// and that many open it.
#[test]
fn a_hunkered_juvenile_needs_fewer_members_than_an_adult_and_that_many_open_it() {
    let h = Hunker { enabled: true, pry_per_member_s: 0.5, resist_per_s: 1.2, ring_resist_per_kin: 0.0, herd_enabled: false, ..crate::vigil::hunker_of(GRAZER) };
    let birth = config().founder(GRAZER).core.birth_body;
    let adult = h.members_to_open(birth, birth, 0).expect("some number opens an adult");
    let young = h.members_to_open(0.5 * birth, birth, 0).expect("some number opens a juvenile");
    assert!(young < adult, "juvenile needs {young}, adult {adult}");
    let long = cubarium_voxel::ticks_at_least_one(600.0);
    assert!(opened_by(h, young as usize, 0.5, long), "{young} members did not open the juvenile");
    assert!(!opened_by(h, young as usize, 1.0, long), "{young} members opened the adult");
    // Kin hunkered beside it raise the bar.
    let ringed = Hunker { ring_resist_per_kin: 0.5, ..h };
    assert!(ringed.members_to_open(birth, birth, 2).unwrap() > ringed.members_to_open(birth, birth, 0).unwrap());
}

// ------------------------------------------------------------------ bunching

/// An alarmed grazer with kin in hearing range moves toward them, not away: the alarm's run
/// heads for its kin, not along the bolting neighbour's heading.
#[test]
fn an_alarmed_grazer_with_kin_in_hearing_moves_toward_them() {
    let world = world(45);
    let flora = Flora::new(FloraConfig::default());
    let hearing_m = 4.0;
    let mut c = config();
    c.founders[GRAZER.index()].senses = SenseTable { live: true, hearing_m, hearing_min: 1e-9, kin_tracks: 3, ..SenseTable::default() };
    c.founders[GRAZER.index()].vigilance =
        Vigilance { run_noise: 3.0, alarm_emission_factor: 2.0, alarm_bolt_s: 2.0, alarm_refractory_s: 10.0, ..flight_only(0.0, 0.0, 0.0) };
    c.founders[GRAZER.index()].hunker = Hunker { enabled: true, ring_m: 0.5, herd_enabled: false, ..crate::vigil::hunker_of(GRAZER) };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let listener = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    let speed = fauna.config.manifest(GRAZER).cruise_m_per_s;
    // The bolting neighbour stands east of it and runs west: S2's bolt would follow it west,
    // the bunching run heads east, to it.
    let runner = body(&mut fauna, &world, GRAZER, 2.0 + 0.6 * hearing_m, WEST, speed);
    fauna.percepts.entry(runner).or_default().vigil.bolt_until = u64::MAX / 2;
    sense(&mut fauna, &world);
    let act = respond(&mut fauna, &world, &flora, listener, CROP).expect("an alarm overrides cropping");
    let v = vigil(&fauna, listener);
    assert!(v.bolt_until > fauna.tick, "the alarm moves it");
    let to_kin = EAST;
    let off = (v.bolt_heading - to_kin).rem_euclid(std::f64::consts::TAU);
    let off = off.min(std::f64::consts::TAU - off);
    assert!(off < 0.3, "it heads for its kin ({to_kin}), not along the runner's heading: {}", v.bolt_heading);
    assert!(act.forward > 0.0 && act.feed == 0.0, "it moves toward them and does not crop");
}

// ------------------------------------------------------------------ the switch

/// With the hunker switch off, S2's flight returns: a Veilhound within the start distance
/// starts a flight and the grazer does not hunker.
#[test]
fn with_the_hunker_switch_off_s2s_flight_returns() {
    let world = world(46);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = flight_only(1.5, 3.0, 1.0);
    c.founders[GRAZER.index()].hunker = Hunker { enabled: false, ..hunker(1.5, 3.0, 1.0) };
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    body(&mut fauna, &world, VH, 2.75, WEST, 1.0);
    sense(&mut fauna, &world);
    respond(&mut fauna, &world, &flora, g, CROP);
    let v = vigil(&fauna, g);
    assert!(v.fleeing && !v.hunkered && !animal(&fauna, g).hunkered, "S2: it flees, no hunker");
}

// ------------------------------------------------------------------ S2's bite collapse

/// A hunter it can see lying still does not hold a flight: once the calm has passed with no
/// approaching threat, the flight ends though the still hunter stays within the stop distance.
#[test]
fn a_still_hunter_within_the_stop_distance_does_not_hold_a_flight() {
    let world = world(47);
    let flora = Flora::new(FloraConfig::default());
    let (start, stop, calm_s) = (1.5, 4.0, 1.0);
    let mut c = config();
    c.founders[GRAZER.index()].vigilance = flight_only(start, stop, calm_s);
    let near = c.predation.still_threat_near_m;
    assert!(near < start, "the case needs the still hunter beyond the still-threat line");
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
    let vh = body(&mut fauna, &world, VH, 2.0 + 0.5 * (near + start), WEST, 1.0);
    sense(&mut fauna, &world);
    respond(&mut fauna, &world, &flora, g, CROP);
    assert!(vigil(&fauna, g).fleeing, "a moving hunter inside the start distance starts a flight");
    set_speed(&mut fauna, vh, 0.0);
    for _ in 0..cubarium_voxel::ticks_at_least_one(calm_s) + 2 {
        sense(&mut fauna, &world);
        respond(&mut fauna, &world, &flora, g, CROP);
    }
    assert!(!vigil(&fauna, g).fleeing, "the still hunter, still within {stop} m, does not hold it");
}

/// The scan pause costs no more than its duty share of cropping: over a window sampled at the
/// controller's cadence, a scanning grazer crops on at least (1 − scan_s / (scan_every_s +
/// scan_s)) of the samples a non-scanning grazer crops on, less one scan's samples for the
/// window's edge.
#[test]
fn the_scan_pause_costs_at_most_its_duty_share_of_cropping() {
    let world = world(48);
    let flora = Flora::new(FloraConfig::default());
    let (every, scan) = (6.0, 1.5);
    let cadence = fauna_cadence();
    let crops = |scans: bool| -> u64 {
        let mut c = config();
        c.founders[GRAZER.index()].vigilance =
            if scans { Vigilance { scan_every_s: every, scan_s: scan, ..flight_only(0.0, 0.0, 0.0) } } else { flight_only(0.0, 0.0, 0.0) };
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let g = body(&mut fauna, &world, GRAZER, 2.0, EAST, 0.0);
        let mut n = 0;
        for _ in 0..cubarium_voxel::ticks_at_least_one(120.0) / cadence {
            fauna.tick += cadence;
            let act = respond(&mut fauna, &world, &flora, g, CROP).unwrap_or(CROP);
            if act.feed > 0.5 {
                n += 1;
            }
        }
        n
    };
    let (with, without) = (crops(true), crops(false));
    let duty = scan / (every + scan);
    let edge = cubarium_voxel::ticks_at_least_one(scan) / cadence + 1;
    assert!(without > 0);
    assert!(
        with as f64 >= (1.0 - duty) * without as f64 - edge as f64,
        "scanning crops {with} of {without} samples; duty {duty}"
    );
}

fn fauna_cadence() -> u64 {
    GRAZER.manifest().cadence_ticks()
}
