//! Night survival, Part 1a of `design/ecology-timescales-2026-10-02.md` (brief
//! `design/handoffs/night-survival-2026-10-02.md`, tests 5–7): sleep budgeted by
//! `sleep_upkeep` and `wake_hunger`, the emergency wake, and the first hit on a sleeper.
//! A child of `step`, on the predation fixture's plain held at one hour (its tick never
//! advances, so a case is at midnight for as long as it steps).

use std::sync::atomic::Ordering;

use super::predation_fixture::*;
use crate::{
    Activity, DT, Fauna, FaunaConfig, Founder, LifeConfig, Rest, StartingStores, State, dodge_chance,
};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species as Plant};

/// The brief's defaults: `sleep_upkeep` 0.3 for the ectotherm-like lineages (shredder,
/// ripple-snail, capgnawer, lanternjaw), 0.5 for the rest; `wake_hunger` 0.9 for every
/// lineage; the hunter's satiated `wake_at` untouched.
#[test]
fn the_sleep_defaults_are_the_briefs() {
    let c = FaunaConfig::default();
    for f in Founder::ALL {
        let p = c.founder(f);
        let want = match f {
            Founder::Blind | Founder::RippleSnail | Founder::Capgnawer | Founder::Lanternjaw => 0.3,
            _ => 0.5,
        };
        assert_eq!(p.sleep_upkeep, want, "{}: sleep_upkeep", f.name());
        assert_eq!(p.wake_hunger, 0.9, "{}: wake_hunger", f.name());
    }
    // The minimal gut: the satiated rest is gone; the emergency latch clears at 0.6.
    for f in Founder::ALL {
        assert_eq!(c.founder(f).emergency_clear_hunger, 0.6, "{}", f.name());
    }
}

/// The longest run of a day a lineage of `active` sleeps through on `day`, seconds, from
/// the clock itself (two days scanned, so a window across midnight is whole).
fn sleep_window_s(day: &DayClock, life: &LifeConfig, active: Activity) -> f64 {
    let n = (day.length_min * 60.0 / DT).round() as u64;
    let (mut best, mut run) = (0u64, 0u64);
    for t in 0..2 * n {
        if active.awake(true, day.sun(t, None).daylight, life) {
            run = 0;
        } else {
            run += 1;
            best = best.max(run);
        }
    }
    best as f64 * DT
}

/// A frondgrazer founded full on a plain with no food, held at midnight for its whole
/// sleeping phase (the default cycle's, measured from the clock): with `wake_hunger` 0.9 it
/// sleeps through without waking for hunger.
///
/// The fixture's basal upkeep is the one at which the optimistic adult screen of 1a passes
/// with its 1.2 margin from a full reserve (`h · c / (m · s) = 1.2 · T`). At the code
/// default (0.001/s) the screen gives 15 min, short of the ~38-min night: the design says
/// so (1a, "size of the change at today's upkeep"), and moving upkeep is the search's job,
/// not this package's. The same fixture at the old `wake_hunger` 0.5 wakes for hunger
/// before dawn, so the case tells the two lines apart. Breeding is off: a full adult
/// opens a gestation in its sleep, which 1a budgets separately.
#[test]
fn a_full_frondgrazer_sleeps_through_the_night_without_waking_for_hunger() {
    let life = LifeConfig::default();
    let night_s = sleep_window_s(&DayClock::default(), &life, Activity::Diurnal);
    assert!(night_s > 30.0 * 60.0 && night_s < 45.0 * 60.0, "the diurnal night: {night_s} s");
    let ticks = (night_s / DT).round() as u64;
    for (wake_hunger, sleeps_through) in [(0.9, true), (0.5, false)] {
        let world = at_hour(0.0, 1);
        assert_eq!(world.life_daylight(), 0.0);
        let mut flora = Flora::new(FloraConfig::default());
        let mut config = FaunaConfig::default();
        let p = &mut config.founders[Founder::Browser.index()];
        p.wake_hunger = wake_hunger;
        p.core.maintenance_per_s = 0.9 * p.core.reserve_cap / (1.2 * night_s * p.sleep_upkeep);
        // No breeding: a full adult otherwise opens a gestation within the night and pays
        // ~0.3 of its reserve into it, a whole-cycle cost (1a's list), not the sleep's.
        p.core.reproduction.surplus_floor = 1e9;
        let mut fauna = Fauna::new(config);
        let id = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
        let mut woke = None;
        for t in 1..=ticks {
            fauna.step_with(&world, &mut flora, 1);
            let a = animal(&fauna, id);
            if a.state != State::Asleep || a.rest != Some(Rest::Sleep) {
                woke = Some((t, a));
                break;
            }
        }
        match (sleeps_through, woke) {
            (true, None) => {
                let a = animal(&fauna, id);
                let p = fauna.config.founder(Founder::Browser);
                let hunger = 1.0 - a.reserve / p.core.reserve_of(a.body);
                assert!(hunger < wake_hunger, "it ends the night under the wake line: {hunger}");
            }
            (true, Some((t, a))) => panic!(
                "woke at {:.0} s of a {night_s:.0} s night (reserve {} of cap {}, body {}, state {:?}, rest {:?}, alert {}, repro {:?})",
                t as f64 * DT,
                a.reserve,
                fauna.config.founder(Founder::Browser).core.reserve_of(a.body),
                a.body,
                a.state,
                a.rest,
                a.alert_ticks,
                a.reproduction,
            ),
            (false, Some((_, a))) => {
                let p = fauna.config.founder(Founder::Browser);
                let hunger = 1.0 - a.reserve / p.core.reserve_of(a.body);
                assert!(hunger >= wake_hunger - 1e-9, "woken for hunger: {hunger}");
            }
            (false, None) => panic!("at wake_hunger {wake_hunger} it should wake for hunger before dawn"),
        }
    }
}

/// The emergency wake: a sleeper whose reserve falls to `1 − wake_hunger` of its cap wakes
/// at that line, and, food within reach, feeds.
#[test]
fn a_sleeper_at_the_wake_line_wakes_and_feeds() {
    let world = at_hour(0.0, 2);
    let mut flora = Flora::new(FloraConfig::default());
    let wood = 0.5 * flora.config().species(Plant::Springturf).wood_max;
    // The fixture's introduction face (x 8, z 6): the turf under the browser's mouth.
    assert!(flora.apply(
        &world,
        FloraCommand::Seed {
            x: 8,
            z: 6,
            species: Plant::Springturf,
            wood,
        }
    ));
    let mut fauna = Fauna::new(FaunaConfig::default());
    let h = fauna.config.founder(Founder::Browser).wake_hunger;
    let id = introduce(
        &mut fauna,
        &world,
        Founder::Browser,
        StartingStores {
            body: 1.0,
            reserve: (1.0 - h) + 0.002,
        },
    );
    let feed = drive(&mut fauna, id, 0.0, true);
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(animal(&fauna, id).state, State::Asleep, "above the wake line it sleeps");
    let woke = step_until(&mut fauna, &world, &mut flora, 2000, |f| {
        f.view().animal(id).is_some_and(|a| a.state != State::Asleep)
    })
    .expect("it wakes before its reserve is gone");
    let a = animal(&fauna, id);
    let cap = fauna.config.founder(Founder::Browser).core.reserve_of(a.body);
    assert!(
        a.reserve / cap <= (1.0 - h) + 1e-6,
        "woken at the line, after {woke} ticks: reserve fraction {}",
        a.reserve / cap
    );
    let bites = fauna.ledger.bites_by_founder[Founder::Browser.index()];
    let assimilated = fauna.ledger.assimilated_by_founder[Founder::Browser.index()];
    step_until(&mut fauna, &world, &mut flora, 40, |f| {
        f.ledger.bites_by_founder[Founder::Browser.index()] > bites
    })
    .expect("awake, with food at its mouth, it bites");
    assert!(fauna.ledger.assimilated_by_founder[Founder::Browser.index()] > assimilated);
    feed.store(false, Ordering::Relaxed);
    assert_closes(&fauna, "after the emergency meal");
}

/// First-hit exposure: a sleeping grazer hit once wakes and goes alert, stays up while
/// alert, and the second strike reads its waking state's awareness, not the asleep 0.
#[test]
fn a_sleeper_hit_once_meets_the_second_strike_alert() {
    let world = at_hour(0.0, 3);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(hunting_config());
    let grazer = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    place(&mut fauna, grazer, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    let mut strikers = Vec::new();
    for _ in 0..2 {
        let id = introduce(&mut fauna, &world, Founder::Lanternjaw, HUNTING);
        place(&mut fauna, id, STRIKER_X, LANE_Z, EAST);
        strikers.push(drive(&mut fauna, id, 0.0, false));
    }
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(animal(&fauna, grazer).state, State::Asleep, "fixture: a sleeper");

    strikers[0].store(true, Ordering::Relaxed);
    step_until(&mut fauna, &world, &mut flora, 40, |f| hits(f) > 0).expect("the first strike hits the sleeper");
    strikers[0].store(false, Ordering::Relaxed);
    let g = animal(&fauna, grazer);
    assert_ne!(g.state, State::Asleep, "the hit wakes it");
    assert_eq!(g.rest, None);
    assert!(g.alert_ticks > 0, "and leaves it alert");

    strikers[1].store(true, Ordering::Relaxed);
    let resolved = |f: &Fauna| hits(f) + dodged(f);
    let before = resolved(&fauna);
    let mut read = g.state;
    let mut done = false;
    for _ in 0..40 {
        fauna.step_with(&world, &mut flora, 1);
        if resolved(&fauna) > before {
            done = true;
            break;
        }
        let g = animal(&fauna, grazer);
        assert_ne!(g.state, State::Asleep, "an alert body does not lie down again");
        assert!(g.alert_ticks > 0, "still alert when the second strike comes");
        read = g.state;
    }
    assert!(done, "the second strike resolves");
    // An alert body is not free to sleep, so the tick's own circadian step leaves it in the
    // state it ended the tick before in: the state the strike's dodge read.
    let cfg = &fauna.config.predation;
    assert_ne!(read, State::Asleep);
    assert!(cfg.awareness(read) > cfg.awareness(State::Asleep), "{read:?}");
    let agility = fauna.config.founder(Founder::Browser).agility;
    assert!(dodge_chance(cfg, agility, read, false, 1.0) > dodge_chance(cfg, agility, State::Asleep, false, 1.0));
}
