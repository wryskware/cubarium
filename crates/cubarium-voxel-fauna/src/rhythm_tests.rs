//! **RHYTHMS-CADENCE** (Wrysk 2026-10-09; `design/training-plan-2026-10-08.md` §6 item 18),
//! written from the brief's spec: activity as a leaning with random rests, a resting herd's
//! sentinels and the alarm that wakes its sleepers, stop-and-go pace, and the still body's
//! senses. A child of `step` beside the predation files, sharing their fixture.
//!
//! Every case sets its own rhythm, cadence or table and checks a mechanism against it; no
//! default number, ratio or population is asserted (the defaults case checks only the
//! orderings Wrysk's words give).

use super::predation_fixture::*;
use crate::perception::{self, HEARING, SIGHT};
use crate::rhythm::{self, Cadence, Horizon, Rhythm};
use crate::{Actions, Activity, Fauna, FaunaConfig, Founder, SenseTable, StartingStores, State, Steer};
use cubarium_voxel::World;
use cubarium_voxel::weather::DayClock;
use cubarium_voxel_flora::{Flora, FloraConfig};

const GRAZER: Founder = Founder::Browser;

/// The default day's tick on the 24-hour clock.
fn hours() -> f64 {
    rhythm::hours_per_tick(&DayClock::default())
}

const NOON: Horizon = Horizon { up: true, edge_h: 6.0 };
const MIDNIGHT: Horizon = Horizon { up: false, edge_h: 4.0 };
const SUNSET: Horizon = Horizon { up: true, edge_h: 0.0 };

/// A rhythm with the given chances and short rests (so a run of a few thousand ticks holds
/// many bouts).
fn leaning(day: f64, night: f64, twilight: f64) -> Rhythm {
    Rhythm { day, night, twilight, twilight_h: 1.0, rest_mean_h: 0.05, rest_min_h: 0.005, shade_is_night: false }
}

/// `bodies` bodies run `ticks` ticks of rest bouts at `h`: the awake share after the first
/// tenth, and the rests begun.
fn run(r: &Rhythm, h: Horizon, bodies: u64, ticks: u64) -> (f64, u64) {
    let p = r.chance(h, false);
    let (mut awake, mut counted, mut rests) = (0u64, 0u64, 0u64);
    for id in 1..=bodies {
        let mut left = 0u32;
        for tick in 0..ticks {
            let next = rhythm::advance(r, left, p, hours(), 7, id, tick);
            if left == 0 && next > 0 {
                rests += 1;
            }
            left = next;
            if tick >= ticks / 10 {
                counted += 1;
                if left == 0 {
                    awake += 1;
                }
            }
        }
    }
    (awake as f64 / counted as f64, rests)
}

// ------------------------------------------------------------------ the leaning

#[test]
fn a_flat_leaning_is_awake_alike_at_noon_and_midnight_and_still_rests() {
    let r = leaning(0.6, 0.6, 0.6);
    let (noon, noon_rests) = run(&r, NOON, 24, 20_000);
    let (night, night_rests) = run(&r, MIDNIGHT, 24, 20_000);
    assert!((noon - night).abs() < 0.1, "noon {noon}, midnight {night}");
    assert!(noon_rests > 0 && night_rests > 0, "rests {noon_rests} / {night_rests}");
    assert!(noon < 1.0 && night < 1.0 && noon > 0.0 && night > 0.0);
}

#[test]
fn a_night_leaning_is_awake_more_by_night_than_by_day() {
    let r = leaning(0.2, 0.8, 0.5);
    let (noon, _) = run(&r, NOON, 24, 20_000);
    let (night, _) = run(&r, MIDNIGHT, 24, 20_000);
    assert!(night > noon + 0.2, "midnight {night}, noon {noon}");
}

#[test]
fn a_dawn_and_dusk_peak_shows_in_the_twilight() {
    let r = leaning(0.3, 0.3, 0.9);
    let at = |up: bool, edge_h: f64| r.chance(Horizon { up, edge_h }, false);
    assert!(at(true, 0.0) > at(true, 3.0) && at(false, 0.0) > at(false, 3.0));
    assert!(at(true, 0.25) > at(true, 0.75), "the peak eases off away from the horizon");
    let (dusk, _) = run(&r, SUNSET, 24, 20_000);
    let (noon, _) = run(&r, NOON, 24, 20_000);
    let (night, _) = run(&r, MIDNIGHT, 24, 20_000);
    assert!(dusk > noon + 0.2 && dusk > night + 0.2, "dusk {dusk}, noon {noon}, midnight {night}");
}

#[test]
fn shade_reads_as_night_by_day_only_for_a_lineage_that_takes_it() {
    let mut r = leaning(0.2, 0.9, 0.5);
    assert_eq!(r.chance(NOON, true), r.chance(NOON, false), "shade is unread without the switch");
    r.shade_is_night = true;
    assert_eq!(r.chance(NOON, true), r.chance(MIDNIGHT, false));
    assert_eq!(r.chance(MIDNIGHT, true), r.chance(MIDNIGHT, false));
}

#[test]
fn rest_bouts_begin_and_end_at_irregular_times() {
    let r = leaning(0.5, 0.5, 0.5);
    let p = r.chance(NOON, false);
    let starts = |id: u64| -> Vec<u64> {
        let mut left = 0u32;
        let mut out = Vec::new();
        for tick in 0..20_000u64 {
            let next = rhythm::advance(&r, left, p, hours(), 7, id, tick);
            if left == 0 && next > 0 {
                out.push(tick);
            }
            left = next;
        }
        out
    };
    let (a, b) = (starts(1), starts(2));
    assert!(a.len() > 3 && b.len() > 3, "{} and {} rests", a.len(), b.len());
    assert_ne!(a, b, "two bodies share no schedule");
    let mut gaps: Vec<u64> = a.windows(2).map(|w| w[1] - w[0]).collect();
    gaps.sort_unstable();
    gaps.dedup();
    assert!(gaps.len() >= 3, "the gaps between one body's rests vary: {gaps:?}");
}

// ------------------------------------------------------------------ the circadian stage

/// `n` grazers in a world held at `hour` with `r` as their leaning, full stores, no controller.
fn herd(hour: f64, r: Rhythm, n: usize) -> (World, Fauna, Vec<u64>) {
    let world = at_hour(hour, 3);
    let mut c = FaunaConfig::default();
    c.founders[GRAZER.index()].active = Activity::Leaning;
    c.founders[GRAZER.index()].rhythm = r;
    let mut fauna = Fauna::new(c);
    let ids = (0..n)
        .map(|k| {
            let id = introduce(&mut fauna, &world, GRAZER, StartingStores::FULL);
            place(&mut fauna, id, 1.0 + k as f64, LANE_Z, EAST);
            id
        })
        .collect();
    (world, fauna, ids)
}

#[test]
fn a_leaning_body_sleeps_in_its_rest_bouts_and_never_with_the_clock_off() {
    let (world, mut fauna, ids) = herd(12.0, leaning(0.5, 0.5, 0.5), 4);
    let mut flora = Flora::new(FloraConfig::default());
    let (mut asleep, mut awake) = (0u32, 0u32);
    for _ in 0..3000 {
        fauna.step_with(&world, &mut flora, 1);
        for &id in &ids {
            if animal(&fauna, id).state == State::Asleep {
                asleep += 1;
            } else {
                awake += 1;
            }
        }
    }
    assert!(asleep > 0 && awake > 0, "asleep {asleep}, awake {awake}");

    let off = clock_off(3);
    let mut fauna = Fauna::new(fauna.config().clone());
    let id = introduce(&mut fauna, &off, GRAZER, StartingStores::FULL);
    for _ in 0..3000 {
        fauna.step_with(&off, &mut flora, 1);
        assert_ne!(animal(&fauna, id).state, State::Asleep, "the clock off: every rhythm awake");
    }
}

#[test]
fn a_resting_herd_keeps_some_members_awake_at_night() {
    let (world, mut fauna, ids) = herd(0.0, leaning(0.9, 0.2, 0.5), 8);
    let mut flora = Flora::new(FloraConfig::default());
    let (mut watched, mut sampled, mut someone_asleep) = (0u32, 0u32, false);
    for t in 0..4000 {
        fauna.step_with(&world, &mut flora, 1);
        if t < 400 {
            continue;
        }
        let awake = ids.iter().filter(|&&id| animal(&fauna, id).state != State::Asleep).count();
        sampled += 1;
        if awake > 0 {
            watched += 1;
        }
        someone_asleep |= awake < ids.len();
    }
    assert!(someone_asleep, "the herd rests");
    assert!(watched * 2 > sampled, "a member awake at {watched} of {sampled} night ticks");
}

/// A sleeping grazer and a neighbour 1.5 m off, the neighbour bolting (`bolting`) at a walk's
/// speed, the sleeper keeping `keep` of its hearing asleep; one circadian stage. Whether the
/// sleeper is awake after it.
fn alarm_scene(bolting: bool, keep: f64) -> bool {
    let world = at_hour(0.0, 4);
    let flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    let g = &mut c.founders[GRAZER.index()];
    g.active = Activity::Leaning;
    g.rhythm = Rhythm { day: 0.0, night: 0.0, twilight: 0.0, twilight_h: 0.0, rest_mean_h: 100.0, rest_min_h: 100.0, shade_is_night: false };
    g.senses = SenseTable { live: true, hearing_m: 8.0, hearing_min: 0.05, ..SenseTable::default() };
    g.vigilance.sleep_hearing = keep;
    g.vigilance.run_noise = 3.0;
    let mut fauna = Fauna::new(c);
    let sleeper = introduce(&mut fauna, &world, GRAZER, StartingStores::FULL);
    place(&mut fauna, sleeper, 2.0, LANE_Z, EAST);
    let runner = introduce(&mut fauna, &world, GRAZER, StartingStores::FULL);
    place(&mut fauna, runner, 3.5, LANE_Z, EAST);
    let view = world.view();
    fauna.horizon = Horizon::of(&world.weather().sun());
    let daylight = world.life_daylight();
    // Fall asleep (both: neither is hungry and the hour is off its chance).
    super::circadian(&mut fauna, &view, &flora.view(), true, daylight);
    assert_eq!(animal(&fauna, sleeper).state, State::Asleep, "the scene's sleeper sleeps");
    {
        let r = animal_mut(&mut fauna, runner);
        r.state = State::Resting;
        r.rest = None;
        r.rest_ticks = 0;
        r.speed_m_s = 1.0;
    }
    if bolting {
        let now = fauna.tick;
        fauna.percepts.entry(runner).or_default().vigil.bolt_until = now + 100;
    }
    super::circadian(&mut fauna, &view, &flora.view(), true, daylight);
    animal(&fauna, sleeper).state != State::Asleep
}

#[test]
fn a_heard_alarm_wakes_a_sleeper() {
    assert!(alarm_scene(true, 0.5), "a bolting neighbour wakes it");
    assert!(!alarm_scene(false, 0.5), "a neighbour standing quietly does not");
    assert!(!alarm_scene(true, 0.0), "a sleeper that keeps no hearing sleeps through");
}

// ------------------------------------------------------------------ stop and go

const PACED: Cadence = Cadence { cycle_s: 6.0, pause_min_s: 0.5, pause_max_s: 2.0, speed_min: 0.6, speed_max: 1.0 };

#[test]
fn a_paced_body_varies_its_speed_and_pauses() {
    let (mut paused, mut speeds) = (0u32, Vec::new());
    for tick in 0..20_000u64 {
        let p = PACED.at(9, 42, tick);
        if p.paused {
            paused += 1;
        } else {
            speeds.push((p.speed * 1e6).round() as i64);
        }
    }
    speeds.sort_unstable();
    speeds.dedup();
    assert!(paused > 0 && paused < 20_000, "{paused} paused ticks");
    assert!(speeds.len() >= 3, "{} bout speeds", speeds.len());
    assert!(!Cadence::default().on());
}

/// One grazer under `cadence`, walking forward on the given steer: the forwards its pace gate
/// leaves over `ticks` ticks, and the ledger's paced ticks.
fn gated(cadence: Cadence, src: Steer, ticks: u64) -> (Vec<f64>, u64) {
    let world = clock_off(5);
    let mut c = FaunaConfig::default();
    c.founders[GRAZER.index()].cadence = cadence;
    let mut fauna = Fauna::new(c);
    let id = introduce(&mut fauna, &world, GRAZER, StartingStores::FULL);
    let i = fauna.animals.iter().position(|a| a.id == id).unwrap();
    let ask = Actions { forward: 1.0, turn: 0.2, feed: 0.0 };
    let mut out = Vec::new();
    for _ in 0..ticks {
        fauna.tick += 1;
        out.push(super::pace(&mut fauna, i, 5, src, ask, 1).forward);
    }
    (out, fauna.ledger.paced_ticks_by_founder[GRAZER.index()])
}

#[test]
fn a_foraging_walker_pauses_and_a_fleeing_one_does_not() {
    let (searching, paced) = gated(PACED, Steer::Search, 4000);
    assert!(searching.iter().any(|&f| f == 0.0), "it pauses");
    let mut moving: Vec<i64> = searching.iter().filter(|&&f| f > 0.0).map(|f| (f * 1e6).round() as i64).collect();
    moving.sort_unstable();
    moving.dedup();
    assert!(moving.len() >= 2, "its speed varies: {moving:?}");
    assert!(paced > 0);
    let (fleeing, paced) = gated(PACED, Steer::Flee, 4000);
    assert!(fleeing.iter().all(|&f| f == 1.0), "a flight keeps a steady run");
    assert_eq!(paced, 0);
}

#[test]
fn a_heuristic_walker_in_the_world_stops_and_goes() {
    let world = clock_off(6);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    c.founders[GRAZER.index()].cadence = PACED;
    let mut fauna = Fauna::new(c.clone());
    let id = introduce(&mut fauna, &world, GRAZER, StartingStores::FULL);
    fauna.controllers.set(id, crate::heuristic(GRAZER, c.teachers));
    let mut held = Vec::new();
    for _ in 0..4000 {
        fauna.step_with(&world, &mut flora, 1);
        held.push(animal(&fauna, id).founder_state.held.forward);
    }
    let l = fauna.ledger;
    assert!(l.paced_ticks_by_founder[GRAZER.index()] > 0, "its search is paced");
    assert!(l.paused_ticks_by_founder[GRAZER.index()] > 0, "and it pauses");
    let mut moving: Vec<i64> = held.iter().filter(|&&f| f > 0.0).map(|f| (f * 1e6).round() as i64).collect();
    moving.sort_unstable();
    moving.dedup();
    assert!(moving.len() >= 2, "its pace varies: {moving:?}");
}

// ------------------------------------------------------------------ the still body's senses

/// A Veilhound observer with `table` and a grazer walking 5 m ahead of it; one sensing with
/// the observer still or moving. The modes it senses the grazer by.
fn sensed(table: SenseTable, observer_still: bool) -> u8 {
    let world = clock_off(8);
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
        c.founders[f.index()].senses.live = false;
    }
    c.founders[Founder::Chorister.index()].senses = table;
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let mut fauna = Fauna::new(c);
    let me = introduce(&mut fauna, &world, Founder::Chorister, StartingStores::FULL);
    place(&mut fauna, me, 1.0, LANE_Z, EAST);
    let prey = introduce(&mut fauna, &world, GRAZER, StartingStores::FULL);
    place(&mut fauna, prey, 6.0, LANE_Z, EAST);
    {
        let p = animal_mut(&mut fauna, prey);
        p.speed_m_s = 0.75;
        p.still_ticks = 0;
    }
    {
        let o = animal_mut(&mut fauna, me);
        o.speed_m_s = if observer_still { 0.0 } else { 1.4 };
        o.still_ticks = if observer_still { u64::MAX / 2 } else { 0 };
    }
    fauna.tick += 1;
    perception::sense_pass(&mut fauna, &world.view(), false, 1.0, 1);
    fauna.percept(me).and_then(|p| p.track(prey)).map_or(0, |t| t.modes)
}

#[test]
fn a_still_body_detects_a_target_the_same_body_moving_does_not() {
    let base = SenseTable {
        live: true,
        sight_half_angle_deg: 180.0,
        sight_still_fraction: 1.0,
        still_gain: 1.5,
        still_settle_s: 0.5,
        ..SenseTable::default()
    };
    // Sight: 4 m moving, so 5 m off is seen only still.
    let sight = SenseTable { sight_m: 4.0, sight_night_m: 4.0, ..base };
    assert_eq!(sensed(sight, false) & SIGHT, 0, "moving, it does not see 5 m off");
    assert_ne!(sensed(sight, true) & SIGHT, 0, "still, it does");
    // Hearing: in range, the walk's loudness just under the moving floor.
    let walk = 0.75f64.powf(1.0) * crate::FounderPhysiology::frozen(GRAZER).adult_length_m;
    let loud = walk / 5.0;
    let hearing = SenseTable { hearing_m: 6.0, hearing_min: loud * 1.2, ..base };
    assert_eq!(sensed(hearing, false) & HEARING, 0, "moving, it does not hear it");
    assert_ne!(sensed(hearing, true) & HEARING, 0, "still, it does");
    // No gain, no difference.
    let flat = SenseTable { still_gain: 1.0, ..sight };
    assert_eq!(sensed(flat, true) & SIGHT, 0);
}

// ------------------------------------------------------------------ the defaults' direction

/// Wrysk's words of 2026-10-09 as orderings, no number pinned.
#[test]
fn the_defaults_lean_as_wrysk_described() {
    let c = FaunaConfig::default();
    let r = |f: Founder| c.founder(f).rhythm;
    for f in [Founder::Blind, Founder::Capgnawer, Founder::Browser, Founder::Loftstrider, Founder::Bellwing, Founder::Seedporter, Founder::Chorister] {
        assert_eq!(c.founder(f).active, Activity::Leaning, "{}", f.name());
        assert!(r(f).rest_mean_h > 0.0, "{} rests", f.name());
    }
    assert_ne!(c.founder(Founder::Lanternjaw).active, Activity::Leaning, "the lanternjaw keeps its rule");
    assert_ne!(c.founder(Founder::RippleSnail).active, Activity::Leaning, "the snail keeps its rule");
    assert!(r(Founder::Capgnawer).night > r(Founder::Capgnawer).day);
    assert!(r(Founder::Browser).day > r(Founder::Browser).night && r(Founder::Browser).night > 0.0, "grazers: day, a night share awake");
    assert!(r(Founder::Loftstrider).day > r(Founder::Loftstrider).night);
    let bw = r(Founder::Bellwing);
    assert!(bw.night > bw.day && bw.day > 0.0 && bw.twilight > bw.night, "bellwings: night, some day, dawn and dusk peaks");
    assert!(r(Founder::Seedporter).day > r(Founder::Seedporter).night);
    assert_eq!(r(Founder::Chorister).day, r(Founder::Chorister).night, "Veilhounds: any hour");
    assert!(r(Founder::Chorister).rest_mean_h > r(Founder::Blind).rest_mean_h, "Veilhounds rest long, shredders briefly");
    for f in [Founder::Blind, Founder::Browser, Founder::Seedporter, Founder::Chorister] {
        assert!(c.founder(f).cadence.on(), "{} walks in bouts", f.name());
    }
    assert!(!c.founder(Founder::Bellwing).cadence.on(), "a flyer is not paced");
}
