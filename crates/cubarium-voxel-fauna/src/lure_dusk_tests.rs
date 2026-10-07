//! **LURE: bellwings fly at dusk and dawn** (Wrysk 2026-10-06: "the point of having the
//! population boom was so that they can be eaten. dusk/dawn is fine for them to fly as well as
//! day"). A lineage's [`crate::ActiveWindow`] keeps it awake for `after_set_h` after sunset and
//! `before_rise_h` before sunrise on top of its rhythm, so a diurnal bellwing is up while the
//! dark-active lanternjaw's lure is lit. A child of `step`, sharing the predation fixture.
//!
//! Every case builds its scene by hand and sets the window it checks on its own config (a
//! test value, not the default); the hour is read from the world's own day clock, so no
//! default number is asserted.

use super::predation_fixture::*;
use crate::{
    ActiveWindow, Activity, Command, Fauna, FaunaConfig, Founder, MobilityMode, NightClock, Pose,
    StartingStores, State,
};
use cubarium_voxel::World;
use cubarium_voxel::weather::DayClock;
use cubarium_voxel_flora::{Flora, FloraConfig};

const BW: Founder = Founder::Bellwing;
const LJ: Founder = Founder::Lanternjaw;
/// The window every case gives the bellwing, hours (a test value).
const WINDOW_H: f64 = 1.0;

/// Sunset and sunrise of the default cycle, hours on its 24-hour clock.
fn set_rise_h() -> (f64, f64) {
    let (rise, set) = DayClock::default().cycle_rise_set();
    (set * 24.0, rise * 24.0)
}

/// Half way through the window after sunset: dusk, inside the window, the sky already dark.
fn dusk_hour() -> f64 {
    set_rise_h().0 + 0.5 * WINDOW_H
}

/// Half way through the night's middle, out of both windows.
fn deep_night_hour() -> f64 {
    let (set, rise) = set_rise_h();
    let night = (rise + 24.0 - set).rem_euclid(24.0);
    (set + 0.5 * night).rem_euclid(24.0)
}

/// The default table with the bellwing's window `w` and its live senses read every tick.
fn config(w: ActiveWindow) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.founders[BW.index()].active_window = w;
    c
}

fn window(h: f64) -> ActiveWindow {
    ActiveWindow { before_rise_h: h, after_set_h: h }
}

/// A bellwing on its own heuristic perched on the ground at `x`, its eating latch on and
/// its reserve full enough that hunger never wakes it.
fn perched_bellwing(fauna: &mut Fauna, world: &World, x: f64) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft {
            pose: Pose { x, z: LANE_Z, heading_rad: EAST, y_m: SURFACE_M },
            founder: BW,
            stores: StartingStores { body: 1.0, reserve: 0.6 },
        },
    ));
    let id = fauna.view().ledger.births - 1;
    let teachers = fauna.config.teachers;
    fauna.controllers.set(id, crate::controller::heuristic(BW, teachers));
    animal_mut(fauna, id).eating = true;
    id
}

/// The world held at `hour`, the bellwing perched, `ticks` fauna ticks: its last state and
/// whether it flew on any of them.
fn bellwing_at(hour: f64, w: ActiveWindow, ticks: usize) -> (State, bool) {
    let world = at_hour(hour, 71);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(w));
    let bw = perched_bellwing(&mut fauna, &world, 4.0);
    let mut flew = false;
    for _ in 0..ticks {
        fauna.step_with(&world, &mut flora, 1);
        flew |= animal(&fauna, bw).mobility.mode == MobilityMode::Fly;
    }
    assert_closes(&fauna, "after the bellwing case");
    (animal(&fauna, bw).state, flew)
}

/// **The window's rule**: a rhythm is awake on its own phase or inside its window — up to
/// `after_set_h` after sunset and `before_rise_h` before sunrise on the day's 24-hour clock —
/// and a window of zero is the rhythm alone.
#[test]
fn a_window_keeps_a_rhythm_awake_just_after_sunset_and_just_before_sunrise() {
    let life = crate::LifeConfig::default();
    let day = DayClock::default();
    let (rise, set) = day.cycle_rise_set();
    let sun_at = |h: f64| cubarium_voxel::weather::Sun::at(&day, (h / 24.0).rem_euclid(1.0), rise, set);
    let (set_h, rise_h) = (set * 24.0, rise * 24.0);
    let w = window(WINDOW_H);
    let cases = [
        (set_h + 0.5 * WINDOW_H, true, "inside the window after sunset"),
        (rise_h - 0.5 * WINDOW_H, true, "inside the window before sunrise"),
        (set_h + 1.5 * WINDOW_H, false, "past the window after sunset"),
        (rise_h - 1.5 * WINDOW_H, false, "before the window ahead of sunrise"),
    ];
    for (h, awake, what) in cases {
        let sun = sun_at(h);
        let night = NightClock::of(&sun);
        assert!(!Activity::Diurnal.awake(true, sun.daylight, &life), "fixture: {what} is past the day line");
        assert_eq!(Activity::Diurnal.awake_in(true, sun.daylight, 1.0, night, w, &life), awake, "{what} ({h} h)");
        assert!(!Activity::Diurnal.awake_in(true, sun.daylight, 1.0, night, window(0.0), &life), "no window: {what}");
    }
    // By day the window changes nothing, and the clock off wakes everyone.
    let noon = sun_at(12.0);
    assert!(Activity::Diurnal.awake_in(true, noon.daylight, 1.0, NightClock::of(&noon), w, &life));
    assert!(!Activity::Nocturnal.awake_in(true, noon.daylight, 1.0, NightClock::of(&noon), w, &life));
    assert!(Activity::Diurnal.awake_in(false, 0.0, 1.0, NightClock::DAY, window(0.0), &life));
}

/// **A bellwing is awake and flying at dusk, inside its window**; the same bellwing with no
/// window sleeps there.
#[test]
fn a_bellwing_is_awake_and_flying_at_dusk_inside_its_window() {
    let (state, flew) = bellwing_at(dusk_hour(), window(WINDOW_H), 60);
    assert_ne!(state, State::Asleep, "awake at dusk inside its window");
    assert!(flew, "and it takes off");
    let (state, flew) = bellwing_at(dusk_hour(), window(0.0), 60);
    assert_eq!(state, State::Asleep, "control: no window, asleep at the same hour");
    assert!(!flew, "control: it stays perched");
}

/// **A bellwing is asleep in deep night**, out of its window.
#[test]
fn a_bellwing_is_asleep_in_deep_night() {
    let (state, flew) = bellwing_at(deep_night_hour(), window(WINDOW_H), 60);
    assert_eq!(state, State::Asleep, "asleep in deep night");
    assert!(!flew);
}

/// **A bellwing at dusk approaches a lit lure and can be struck there.** At dusk, inside the
/// bellwing's window, a hungry dark-active lanternjaw on its own patrol heuristic waits still
/// with its lure lit (its wait made long, a test value); a hungry bellwing on its own heuristic
/// perched behind it, with no flower anywhere, takes off, comes to the lure and the lanternjaw
/// strikes it: a strike begun on a lure visit. Control: the same bellwing with no window
/// sleeps where it perched and is never drawn.
#[test]
fn a_bellwing_at_dusk_approaches_a_lit_lure_and_can_be_struck_there() {
    let arm = |w: ActiveWindow| {
        // A plain 6 m tall, so the bellwing's flight band is not cut by the world's top.
        let world = plain_of_height(DayClock { start_hour: dusk_hour(), ..DayClock::default() }, 72, 24);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config(w);
        c.hunt.patrol = true;
        c.hunt.patch_wait_s = 1e9;
        let mut fauna = Fauna::new(c);
        assert_eq!(fauna.config.founder(LJ).active, Activity::DarkActive, "fixture: the lanternjaw is dark-active");
        let lj = introduce(&mut fauna, &world, LJ, HUNTING);
        place(&mut fauna, lj, STRIKER_X, LANE_Z, EAST);
        let teachers = fauna.config.teachers;
        fauna.controllers.set(lj, crate::controller::heuristic(LJ, teachers));
        {
            let a = animal_mut(&mut fauna, lj);
            a.pose.y_m = SURFACE_M;
            a.eating = true;
            a.hunt.mode = crate::hunt::HuntMode::Wait;
            a.hunt.patch = (STRIKER_X, LANE_Z);
            a.hunt.samples = 0;
            a.still_ticks = u64::MAX / 2;
        }
        // A bellwing perched on the ground 1.5 m behind the lanternjaw, out of its sight.
        let bw = perched_bellwing(&mut fauna, &world, STRIKER_X - 1.5);
        let mut lit = false;
        let mut drawn = false;
        let mut lure_strike = false;
        for _ in 0..(60 * crate::TICK_HZ as usize) {
            fauna.step_with(&world, &mut flora, 1);
            lit |= fauna.animals.iter().any(|a| a.id == lj && a.founder_state.lure > 0.0);
            drawn |= fauna.percept(bw).is_some_and(|p| {
                p.light.beacon && p.light.beacon_kind == crate::light::BeaconKind::Lure.index() as u8
            });
            lure_strike |= fauna.animals.iter().any(|a| a.id == lj && a.strike.is_some_and(|s| s.lure));
            if fauna.ledger.lure_kills[LJ.index()] + fauna.ledger.lure_kills[BW.index()] > 0 {
                break;
            }
        }
        assert_closes(&fauna, "after the lure case");
        (lit, drawn, fauna.ledger.strikes_on_by_founder[BW.index()], lure_strike)
    };
    let (lit, drawn, strikes, lure_strike) = arm(window(WINDOW_H));
    assert!(lit, "the lure is lit at dusk");
    assert!(drawn, "the lure is the bellwing's beacon");
    assert!(strikes > 0, "and is struck there");
    assert!(lure_strike, "a strike begun on a lure visit");
    let (_, drawn, strikes, _) = arm(window(0.0));
    assert!(!drawn && strikes == 0, "control: asleep, never drawn ({drawn}, {strikes})");
}
