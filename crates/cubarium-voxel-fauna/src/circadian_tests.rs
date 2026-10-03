//! Animal circadian rhythms. A child of `step`, so a case can reach the controller
//! table. Tiny cases only: a world held at one hour (its tick never advances), one or a
//! few fauna ticks, and a fast day for the one case that has to see the phase change.

use crate::controller::{Actions, Controller, Response};
use crate::{Activity, Command, DT, Fauna, FaunaConfig, Founder, StartingStores, State};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A flat soil plain with the given day clock.
fn plain(day: DayClock) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 12,
        height: 6,
        depth: 6,
        voxel_m: 0.25,
        seed: 5,
        day,
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..12 {
            for y in 1..=2 {
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

fn at_hour(start_hour: f64) -> World {
    plain(DayClock {
        start_hour,
        ..DayClock::default()
    })
}

fn founder(fauna: &mut Fauna, world: &World, which: Founder, reserve: f64) -> usize {
    assert!(fauna.apply(
        world,
        Command::IntroduceFounder {
            x: 4,
            z: 3,
            founder: which,
            stores: StartingStores { body: 1.0, reserve },
            heading_rad: std::f64::consts::FRAC_PI_2,
        }
    ));
    fauna.animals.len() - 1
}

/// What one tick of `fauna` respired, organic matter.
fn respired_by_one_tick(fauna: &mut Fauna, world: &World, flora: &mut Flora) -> f64 {
    let before = fauna.view().ledger.respired_out;
    fauna.step_with(world, flora, 1);
    fauna.view().ledger.respired_out - before
}

fn assert_closes(fauna: &Fauna, when: &str) {
    let v = fauna.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() <= 1e-12 * held.abs().max(1e-6),
            "{when}: fauna {what} holds {held}, the ledger expects {expected}"
        );
    }
}

/// A controller that counts how often it is asked and always asks to walk forward.
struct Counting(Arc<AtomicUsize>);

impl Controller for Counting {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        self.0.fetch_add(1, Ordering::Relaxed);
        Response::Bounded(Actions {
            forward: 1.0,
            turn: 0.0,
            feed: 0.0,
        })
    }
    fn reset(&mut self) {}
}

#[test]
fn the_founders_have_their_activity_and_the_rest_state_costs_half() {
    // Night survival 1a: the shredder, an ectotherm-like lineage, sleeps at 0.3.
    let c = FaunaConfig::default();
    assert_eq!(c.founder(Founder::Browser).active, Activity::Diurnal);
    assert_eq!(c.founder(Founder::Blind).active, Activity::Nocturnal);
    assert_eq!(c.founder(Founder::Browser).sleep_upkeep, 0.5);
    assert_eq!(c.founder(Founder::Blind).sleep_upkeep, 0.3);
}

#[test]
fn an_activity_is_awake_in_its_own_phase_and_always_with_the_clock_off() {
    // Diurnal: awake above 0.3 daylight; nocturnal: below it.
    assert!(Activity::Diurnal.awake(true, 1.0, &crate::LifeConfig::default()));
    assert!(!Activity::Diurnal.awake(true, 0.0, &crate::LifeConfig::default()));
    assert!(!Activity::Diurnal.awake(true, 0.29, &crate::LifeConfig::default()));
    assert!(Activity::Diurnal.awake(true, 0.31, &crate::LifeConfig::default()));
    assert!(Activity::Nocturnal.awake(true, 0.0, &crate::LifeConfig::default()));
    assert!(!Activity::Nocturnal.awake(true, 1.0, &crate::LifeConfig::default()));
    assert!(Activity::Nocturnal.awake(true, 0.29, &crate::LifeConfig::default()));
    assert!(!Activity::Nocturnal.awake(true, 0.31, &crate::LifeConfig::default()));
    // Crepuscular: in the twilight of either end, not in full night or full day.
    assert!(!Activity::Crepuscular.awake(true, 0.0, &crate::LifeConfig::default()));
    assert!(!Activity::Crepuscular.awake(true, 1.0, &crate::LifeConfig::default()));
    assert!(Activity::Crepuscular.awake(true, 0.5, &crate::LifeConfig::default()));
    // Always is always.
    for d in [0.0, 0.5, 1.0] {
        assert!(Activity::Always.awake(true, d, &crate::LifeConfig::default()));
    }
    // The clock off: everyone, whatever the number says.
    for a in [
        Activity::Diurnal,
        Activity::Nocturnal,
        Activity::Crepuscular,
        Activity::Always,
    ] {
        for d in [0.0, 0.5, 1.0] {
            assert!(a.awake(false, d, &crate::LifeConfig::default()), "{a:?} sleeps with the clock off at {d}");
        }
    }
}

/// A diurnal browser at midnight is asleep, and pays half its basal upkeep for it.
#[test]
fn a_diurnal_browser_sleeps_at_midnight_and_pays_half_upkeep() {
    let world = at_hour(0.0);
    assert_eq!(world.life_daylight(), 0.0);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let i = founder(&mut fauna, &world, Founder::Browser, 1.0);
    let phys = *fauna.config.founder(Founder::Browser);
    let body = fauna.animals[i].body;
    let paid = respired_by_one_tick(&mut fauna, &world, &mut flora);
    assert_eq!(fauna.animals[i].state, State::Asleep);
    let basal = phys.core.maintenance_per_s * body * DT;
    assert!(
        (paid - phys.sleep_upkeep * basal).abs() <= 1e-15,
        "asleep paid {paid}, half of basal is {}",
        0.5 * basal
    );
    assert_closes(&fauna, "asleep");
}

/// The same browser at noon is awake and pays the whole basal upkeep.
#[test]
fn the_same_browser_is_awake_at_noon_and_pays_full_upkeep() {
    let world = at_hour(12.0);
    assert_eq!(world.life_daylight(), 1.0);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let i = founder(&mut fauna, &world, Founder::Browser, 1.0);
    let phys = *fauna.config.founder(Founder::Browser);
    let body = fauna.animals[i].body;
    let paid = respired_by_one_tick(&mut fauna, &world, &mut flora);
    assert_ne!(fauna.animals[i].state, State::Asleep);
    let basal = phys.core.maintenance_per_s * body * DT;
    assert!(
        (paid - basal).abs() <= 1e-15,
        "awake paid {paid}, basal {basal}"
    );
}

/// Hunger overrides sleep: a browser with an empty reserve stays awake at midnight; a
/// fed one does not.
#[test]
fn a_starving_browser_stays_awake_at_midnight() {
    let world = at_hour(0.0);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let starving = founder(&mut fauna, &world, Founder::Browser, 0.0);
    let fed = founder(&mut fauna, &world, Founder::Browser, 1.0);
    fauna.step_with(&world, &mut flora, 1);
    assert_ne!(fauna.animals[starving].state, State::Asleep);
    assert_eq!(fauna.animals[fed].state, State::Asleep);
}

/// A nocturnal shredder is awake at midnight and asleep at noon.
#[test]
fn a_nocturnal_shredder_is_awake_at_midnight_and_asleep_at_noon() {
    for (hour, asleep) in [(0.0, false), (12.0, true)] {
        let world = at_hour(hour);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let i = founder(&mut fauna, &world, Founder::Blind, 1.0);
        fauna.step_with(&world, &mut flora, 1);
        assert_eq!(
            fauna.animals[i].state == State::Asleep,
            asleep,
            "shredder at hour {hour}"
        );
    }
}

/// With the clock off nothing sleeps, at any lineage's bad hour.
#[test]
fn with_the_clock_off_no_animal_sleeps() {
    let world = plain(DayClock {
        mode: DayMode::Off,
        start_hour: 0.0,
        ..DayClock::default()
    });
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let browser = founder(&mut fauna, &world, Founder::Browser, 1.0);
    let shredder = founder(&mut fauna, &world, Founder::Blind, 1.0);
    for _ in 0..5 {
        fauna.step_with(&world, &mut flora, 1);
        assert_ne!(fauna.animals[browser].state, State::Asleep);
        assert_ne!(fauna.animals[shredder].state, State::Asleep);
    }
}

/// An asleep animal's controller is not consulted, and it does not move; the same
/// browser awake is consulted and walks.
#[test]
fn an_asleep_animal_is_not_consulted_and_does_not_move() {
    for (hour, asleep) in [(0.0, true), (12.0, false)] {
        let world = at_hour(hour);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let i = founder(&mut fauna, &world, Founder::Browser, 1.0);
        let calls = Arc::new(AtomicUsize::new(0));
        let id = fauna.animals[i].id;
        fauna.controllers.set(id, Box::new(Counting(calls.clone())));
        let from = fauna.animals[i].pose;
        let period = Founder::Browser.manifest().cadence_ticks();
        for _ in 0..(3 * period) {
            fauna.step_with(&world, &mut flora, 1);
        }
        let moved = fauna.animals[i].pose != from;
        if asleep {
            assert_eq!(calls.load(Ordering::Relaxed), 0, "consulted while asleep");
            assert!(!moved, "walked while asleep");
            assert_eq!(fauna.animals[i].state, State::Asleep);
        } else {
            assert!(calls.load(Ordering::Relaxed) > 0, "never consulted awake");
            assert!(moved, "did not walk awake");
        }
    }
}

/// Through a whole fast day a browser sleeps its night and wakes for its day, and the
/// ledgers stay closed across every change of state.
#[test]
fn a_browser_sleeps_and_wakes_over_a_day_and_the_ledgers_stay_closed() {
    // A 6 s day: 120 ticks. Night is a third of it.
    let mut world = plain(DayClock {
        length_min: 0.1,
        start_hour: 0.0,
        ..DayClock::default()
    });
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let i = founder(&mut fauna, &world, Founder::Browser, 1.0);
    let (mut slept, mut woke) = (0u32, 0u32);
    let mut last = State::Resting;
    let mut changes = 0;
    for tick in 0..360 {
        world.step();
        fauna.step_with(&world, &mut flora, 1);
        let s = fauna.animals[i].state;
        if s == State::Asleep {
            slept += 1;
        } else {
            woke += 1;
        }
        if (s == State::Asleep) != (last == State::Asleep) {
            changes += 1;
        }
        last = s;
        if tick % 30 == 0 {
            assert_closes(&fauna, &format!("tick {tick}"));
        }
    }
    assert!(slept > 60 && woke > 60, "slept {slept}, awake {woke}");
    assert!(
        changes >= 4,
        "only {changes} changes of state in three days"
    );
    assert_closes(&fauna, "end");
}
