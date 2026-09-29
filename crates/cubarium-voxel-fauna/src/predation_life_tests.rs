//! P1 (predation substrate), tests first: the satiated rest, the buried egg bank and its
//! hatching trigger, and the snapshot (`design/predation-and-flight-2026-09-28.md` §2.8,
//! §2.12, §2.13; brief P1 cases 10, 14, 15 and 16). Red by design until the
//! implementation commit adds:
//!
//! - `Founder::Lanternjaw`, its physiology and `Hunter { rest_at, torpor_upkeep,
//!   wake_at, .. }` (manifest.rs, body.rs);
//! - `Rest::{Sleep, Satiated}` (predation.rs or lib.rs, re-exported) and `Animal::rest:
//!   Option<Rest>`, `Some` exactly while `state == State::Asleep`: WX4's rest state with
//!   a reason. A satiated rest runs in any phase, with the clock off too;
//! - `Command::BuryClutch { site: Site, lineage: Founder, eggs: u32 }` (lib.rs): the
//!   viewer key's and the founding bank's act, booked `introduced_*_in`, refused off a
//!   support face;
//! - a lanternjaw clutch is laid or buried as **one `Clutch` per egg** (`count` 1), so
//!   each egg keeps its own dormancy: `Clutch::dormancy: Option<Dormancy>` (lib.rs),
//!   `Dormancy { threshold: u32, viable_until_tick: u64, .. }` — the per-egg footfall
//!   threshold (drawn, mean 6, spread wide) and the tick its viability runs out (drawn,
//!   median about 12 h);
//! - `EGG_UPKEEP_PER_S: f64` (lib.rs): organic matter an egg respires per unit of its own
//!   organic matter per second. The note says "a tiny upkeep" and pins no value;
//! - `FaunaLedger::eggs_died: u64` (lib.rs): eggs whose viability ran out, gone to the
//!   litter of their face;
//! - `egg_hatches(threshold: u32, footfall: u32, dusk: bool, nearest_adult_m:
//!   Option<f64>) -> bool` (predation.rs, re-exported): the hatching trigger behind one
//!   function (Wrysk's choice, 2026-09-28);
//! - "Dusk" is read as the tick `life_daylight` first falls below `Activity::DAY_LINE`,
//!   when a nocturnal lineage wakes; footfall is distinct prey heard (the tremor rule)
//!   within 2 m over the last clock-day.
//!
//! - `apply_hit(fauna: &mut Fauna, flora: &mut Flora, prey: u64, damage: f64, by:
//!   Founder)` (predation.rs, re-exported): the serial pass's application of one `Hit`
//!   event, called directly by a case; a hit wakes the prey, ending any rest.
//!
//! The snapshot case also names `Animal::{wound, strike}` and `StrikePhase` (see
//! `predation_tests.rs`) and expects the fauna `SCHEMA` bumped past 19.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::predation_fixture::*;
use crate::controller::{Actions, Controller, Response};
use crate::senses::{Fine, cone_occupancy, ray_first_hit};
use crate::{
    Activity, Command, DT, Fauna, apply_hit, FaunaConfig, Founder, EGG_UPKEEP_PER_S, Rest, SCHEMA,
    StartingStores, State, StrikePhase, egg_hatches,
};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const EGG_SITE: Site = Site {
    x: 24,
    y: SURFACE_Y,
    z: 6,
};

/// The egg's column centre, metres.
fn egg_xz() -> (f64, f64) {
    (
        (f64::from(EGG_SITE.x) + 0.5) * VOXEL_M,
        (f64::from(EGG_SITE.z) + 0.5) * VOXEL_M,
    )
}

fn bury(fauna: &mut Fauna, world: &World, site: Site, eggs: u32) {
    assert!(
        fauna.apply(
            world,
            Command::BuryClutch {
                site,
                lineage: Founder::Lanternjaw,
                eggs,
            }
        ),
        "burying {eggs} eggs at {site:?}"
    );
}

/// The lanternjaw eggs, one clutch each.
fn eggs(fauna: &Fauna) -> Vec<crate::Clutch> {
    fauna
        .clutches
        .iter()
        .filter(|c| c.lineage == Founder::Lanternjaw)
        .copied()
        .collect()
}

struct Counting(Arc<AtomicUsize>);

impl Controller for Counting {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        self.0.fetch_add(1, Ordering::Relaxed);
        Response::Bounded(Actions {
            forward: 1.0,
            ..Actions::REST
        })
    }
    fn reset(&mut self) {}
}

// ------------------------------------------------------------------ case 10

/// Case 10: a full lanternjaw rests (satiated, in any phase: here with the clock off) at
/// the torpor factor 0.25 × its basal upkeep, is not consulted while it rests, stays down
/// above half its cap and wakes below it; an awake one does not lie down below 0.9.
#[test]
fn a_full_lanternjaw_rests_at_the_torpor_factor_and_wakes_at_half_its_cap() {
    let world = clock_off(10);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    let calls = Arc::new(AtomicUsize::new(0));
    fauna.controllers.set(lj, Box::new(Counting(calls.clone())));
    let phys = *fauna.config.founder(Founder::Lanternjaw);
    fauna.step_with(&world, &mut flora, 1);
    let a = animal(&fauna, lj);
    assert_eq!(a.state, State::Asleep);
    assert_eq!(a.rest, Some(Rest::Satiated));
    assert_eq!(a.state.awareness(), 0.0, "a resting animal is prey too");

    let before = fauna.ledger.respired_out;
    fauna.step_with(&world, &mut flora, 1);
    let paid = fauna.ledger.respired_out - before;
    let basal = phys.core.maintenance_per_s * animal(&fauna, lj).body * DT;
    assert!(
        (paid - 0.25 * basal).abs() <= 1e-15,
        "resting paid {paid}, the torpor factor gives {}",
        0.25 * basal
    );
    let period = Founder::Lanternjaw.manifest().cadence_ticks();
    for _ in 0..(3 * period) {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0, "consulted while resting");
    assert_closes(&fauna, "resting");

    // Hysteresis on the reserve fraction of its cap (`reserve_cap · body`).
    let set = |fauna: &mut Fauna, id: u64, fraction: f64| {
        let cap = phys.core.reserve_of(animal(fauna, id).body);
        animal_mut(fauna, id).reserve = fraction * cap;
    };
    set(&mut fauna, lj, 0.7);
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(animal(&fauna, lj).rest, Some(Rest::Satiated), "0.7 of cap: still down");
    set(&mut fauna, lj, 0.52);
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(animal(&fauna, lj).state, State::Asleep, "0.52 of cap: still down");
    set(&mut fauna, lj, 0.48);
    fauna.step_with(&world, &mut flora, 1);
    let a = animal(&fauna, lj);
    assert_ne!(a.state, State::Asleep, "below half its cap it wakes");
    assert_eq!(a.rest, None);

    // A satiated lanternjaw that is struck leaves its rest.
    let struck = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(animal(&fauna, struck).rest, Some(Rest::Satiated));
    apply_hit(&mut fauna, &mut flora, struck, 0.1, Founder::Lanternjaw);
    let a = animal(&fauna, struck);
    assert_ne!(a.state, State::Asleep, "a hit ends a satiated rest");
    assert_eq!(a.rest, None);

    // An awake lanternjaw at 0.7 of its cap does not lie down.
    let awake = introduce(
        &mut fauna,
        &world,
        Founder::Lanternjaw,
        StartingStores {
            body: 1.0,
            reserve: 0.7,
        },
    );
    for _ in 0..5 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert_ne!(animal(&fauna, awake).state, State::Asleep);
}

/// WX4's sleep is the other reason on the same state: a diurnal grazer asleep at
/// midnight rests for `Sleep`.
#[test]
fn wx4_sleep_is_a_rest_with_its_own_reason() {
    let world = at_hour(0.0, 10);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let g = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    fauna.step_with(&world, &mut flora, 1);
    let a = animal(&fauna, g);
    assert_eq!((a.state, a.rest), (State::Asleep, Some(Rest::Sleep)));
}

// ------------------------------------------------------------------ case 15

/// Case 15, laying: a lanternjaw in sustained surplus lays a clutch of 3 buried eggs on
/// its own face, each dormant; the books close.
#[test]
fn a_lanternjaw_in_surplus_lays_three_dormant_eggs() {
    let world = clock_off(15);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let rule = fauna.config.founder(Founder::Lanternjaw).core.reproduction;
    assert_eq!(rule.mode, crate::BirthMode::Eggs);
    assert_eq!(rule.clutch_size, 3);
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    animal_mut(&mut fauna, lj).reproduction.surplus_ticks = rule.hold_ticks();
    let site = animal(&fauna, lj).site;
    step_until(&mut fauna, &world, &mut flora, 3, |f| !eggs(f).is_empty())
        .expect("a full lanternjaw past its hold lays");
    let laid = eggs(&fauna);
    assert_eq!(laid.len(), 3);
    assert_eq!(fauna.ledger.eggs_laid, 3);
    for c in &laid {
        assert_eq!((c.site, c.count), (site, 1));
        assert!(c.dormancy.is_some(), "a laid egg is dormant");
    }
    assert_eq!(fauna.view().eggs_by_founder(Founder::Lanternjaw), 3);
    assert_closes(&fauna, "laid");
}

/// Case 15, dormancy: a buried egg pays its upkeep out of itself, is invisible to the
/// cone, and nothing eats it — a shredder and a lanternjaw feeding on its face leave it
/// exactly as an egg alone.
#[test]
fn a_buried_egg_pays_its_upkeep_and_is_invisible_and_inedible() {
    let run = |feeders: bool| -> (World, Flora, Fauna) {
        let world = clock_off(15);
        let flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(hunting_config());
        assert!(!fauna.apply(
            &world,
            Command::BuryClutch {
                site: Site { y: 5, ..EGG_SITE },
                lineage: Founder::Lanternjaw,
                eggs: 3,
            }
        ), "no burying in the air");
        bury(&mut fauna, &world, EGG_SITE, 3);
        assert_closes(&fauna, "buried");
        if feeders {
            let (x, z) = egg_xz();
            for (founder, stores) in [(Founder::Blind, StartingStores::HUNGRY), (Founder::Lanternjaw, HUNTING)] {
                let id = introduce(&mut fauna, &world, founder, stores);
                place(&mut fauna, id, x, z, EAST);
                drive(&mut fauna, id, 0.0, true);
            }
        }
        (world, flora, fauna)
    };

    // One tick of upkeep, out of the egg and into the ledger.
    let (world, mut flora, mut fauna) = run(false);
    let before: f64 = eggs(&fauna).iter().map(|c| c.organic).sum();
    let respired = fauna.ledger.respired_out;
    fauna.step_with(&world, &mut flora, 1);
    let after: f64 = eggs(&fauna).iter().map(|c| c.organic).sum();
    let want = EGG_UPKEEP_PER_S * before * DT;
    assert!(want > 0.0);
    assert!(
        ((before - after) - want).abs() <= 1e-9 * want,
        "the eggs paid {}, their upkeep is {want}",
        before - after
    );
    assert!(((fauna.ledger.respired_out - respired) - (before - after)).abs() <= 1e-15);
    assert_closes(&fauna, "one tick of upkeep");

    // Invisible: a ray skimming the egg's face finds no body.
    let occupancy = cone_occupancy(&world.view(), &flora.view(), &fauna.view());
    let (x, z) = egg_xz();
    let hit = ray_first_hit(
        &world.view(),
        &occupancy,
        u64::MAX,
        (x - 1.0, SURFACE_M + 0.05, z),
        (1.0, 0.0, 0.0),
        2.0,
    );
    assert_ne!(hit.map(|(_, f)| f), Some(Fine::Body), "the cone sees an egg");

    // Inedible: the same 60 ticks with and without mouths on its face.
    let (world, mut flora, mut alone) = run(false);
    let (world2, mut flora2, mut fed_on) = run(true);
    for _ in 0..60 {
        alone.step_with(&world, &mut flora, 1);
        fed_on.step_with(&world2, &mut flora2, 1);
    }
    let held = |f: &Fauna| -> Vec<f64> { eggs(f).iter().map(|c| c.organic).collect() };
    assert_eq!(held(&fed_on).len(), 3);
    for (a, b) in held(&alone).iter().zip(held(&fed_on)) {
        assert!((a - b).abs() <= 1e-15, "an egg under two mouths holds {b}, alone {a}");
    }
    assert_closes(&fed_on, "fed on");
}

/// Case 15, death: an egg whose viability runs out goes to the litter of its face, and
/// the books close.
#[test]
fn a_dead_egg_goes_to_litter_and_the_books_close() {
    let world = clock_off(15);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    bury(&mut fauna, &world, EGG_SITE, 3);
    let tick = fauna.tick();
    let doomed = fauna
        .clutches
        .iter()
        .position(|c| c.lineage == Founder::Lanternjaw)
        .unwrap();
    fauna.clutches[doomed]
        .dormancy
        .as_mut()
        .expect("dormant")
        .viable_until_tick = tick + 1;
    let litter = |flora: &Flora| flora.view().ground_at(EGG_SITE).map_or(0.0, |g| g.litter);
    let before = litter(&flora);
    let mut last = fauna.clutches[doomed].organic;
    for _ in 0..3 {
        if eggs(&fauna).len() == 3 {
            last = fauna.clutches[doomed].organic;
        }
        fauna.step_with(&world, &mut flora, 1);
    }
    assert_eq!(eggs(&fauna).len(), 2);
    assert_eq!(fauna.ledger.eggs_died, 1);
    let gone = litter(&flora) - before;
    assert!(
        (gone - last).abs() <= 1e-6 * last,
        "the litter rose by {gone}; the egg held {last}"
    );
    assert_closes(&fauna, "egg died");
}

/// Case 15, the draws: each egg's viability has a median of about 12 h and its footfall
/// threshold a mean of about 6, spread wide.
#[test]
fn eggs_draw_a_12_h_median_viability_and_thresholds_around_6() {
    let world = clock_off(15);
    let mut fauna = Fauna::new(FaunaConfig::default());
    for k in 0..60u32 {
        let site = Site {
            x: (k * 7) % 48,
            y: SURFACE_Y,
            z: k % 12,
        };
        bury(&mut fauna, &world, site, 3);
    }
    let tick = fauna.tick();
    let dormant: Vec<_> = eggs(&fauna)
        .iter()
        .map(|c| c.dormancy.expect("dormant"))
        .collect();
    assert_eq!(dormant.len(), 180);
    let mut spans: Vec<f64> = dormant
        .iter()
        .map(|d| (d.viable_until_tick - tick) as f64 * DT / 3600.0)
        .collect();
    spans.sort_by(f64::total_cmp);
    let median = spans[spans.len() / 2];
    assert!((median - 12.0).abs() < 0.35 * 12.0, "median viability {median} h");
    let thresholds: Vec<u32> = dormant.iter().map(|d| d.threshold).collect();
    let mean = thresholds.iter().map(|&t| f64::from(t)).sum::<f64>() / 180.0;
    assert!((mean - 6.0).abs() < 1.5, "mean threshold {mean}");
    let (lo, hi) = (
        *thresholds.iter().min().unwrap(),
        *thresholds.iter().max().unwrap(),
    );
    assert!(lo < 6 && hi > 6, "thresholds span {lo}..={hi}");
    let mut distinct = thresholds.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert!(distinct.len() >= 4, "thresholds {distinct:?} are not spread");
}

// ------------------------------------------------------------------ case 16

/// The hatching trigger itself: footfall at its threshold, at dusk, no adult within 4 m.
#[test]
fn the_hatching_trigger_needs_footfall_dusk_and_no_adult_within_4_m() {
    assert!(egg_hatches(3, 3, true, None));
    assert!(egg_hatches(3, 5, true, None));
    assert!(!egg_hatches(3, 2, true, None), "not before its threshold");
    assert!(!egg_hatches(3, 3, false, None), "not by day");
    assert!(!egg_hatches(3, 3, true, Some(3.9)), "an adult within 4 m suppresses");
    assert!(egg_hatches(3, 3, true, Some(4.1)));
}

/// A fast day (6 s, 120 ticks) from 16:00, and the step at which its daylight first
/// falls below the day line.
fn fast_afternoon() -> (World, u64) {
    let world = plain(
        DayClock {
            length_min: 0.1,
            start_hour: 16.0,
            ..DayClock::default()
        },
        16,
    );
    let mut probe = world.clone();
    let mut last = probe.life_daylight();
    assert!(last >= Activity::DAY_LINE, "16:00 is day");
    for t in 1..=120u64 {
        probe.step();
        let now = probe.life_daylight();
        if now < Activity::DAY_LINE && last >= Activity::DAY_LINE {
            assert!(t >= 8, "dusk after only {t} ticks");
            return (world, t);
        }
        last = now;
    }
    panic!("no dusk within a day");
}

/// One egg of `threshold`, a grazer walking past it within 2 m through the afternoon,
/// and optionally an adult lanternjaw `adult_m` away. The step a lanternjaw hatched, if
/// one did by 30 steps after dusk, and dusk.
fn afternoon(threshold: u32, adult_m: Option<f64>) -> (Option<u64>, u64, Fauna) {
    let (mut world, dusk) = fast_afternoon();
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    bury(&mut fauna, &world, EGG_SITE, 1);
    fauna
        .clutches
        .iter_mut()
        .find(|c| c.lineage == Founder::Lanternjaw)
        .unwrap()
        .dormancy
        .as_mut()
        .unwrap()
        .threshold = threshold;
    let (x, z) = egg_xz();
    let grazer = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    place(&mut fauna, grazer, x - 1.0, z + 0.5, EAST);
    drive(&mut fauna, grazer, 1.0, false);
    if let Some(d) = adult_m {
        let adult = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
        place(&mut fauna, adult, x - d, z, EAST);
    }
    let adults = lanternjaws(&fauna);
    for t in 1..=dusk + 30 {
        world.step();
        fauna.step_with(&world, &mut flora, 1);
        if lanternjaws(&fauna) > adults {
            return (Some(t), dusk, fauna);
        }
    }
    (None, dusk, fauna)
}

/// Case 16: an egg whose footfall reached its threshold in the afternoon hatches at dusk,
/// not before; one short of its threshold does not; an adult 3 m away suppresses it and
/// one 6 m away does not. The hatchling is 0.46 of the adult's length.
#[test]
fn an_egg_hatches_at_dusk_once_its_footfall_reaches_its_threshold() {
    let (hatched, dusk, fauna) = afternoon(1, None);
    let t = hatched.expect("heard one prey, threshold 1: hatches at dusk");
    assert!(
        t + 1 >= dusk && t <= dusk + 2,
        "hatched at step {t}, dusk is step {dusk}: not by day, and at dusk"
    );
    assert!(eggs(&fauna).is_empty());
    let young = fauna
        .animals
        .iter()
        .find(|a| a.founder == Some(Founder::Lanternjaw))
        .expect("the hatchling");
    let phys = fauna.config.founder(Founder::Lanternjaw);
    let length = phys.body_at(young.body).length_m / phys.adult_length_m;
    assert!((length - 0.46).abs() < 0.03, "a hatchling {length} of the adult's length");
    assert_closes(&fauna, "hatched");

    let (hatched, _, fauna) = afternoon(2, None);
    assert_eq!(hatched, None, "one prey heard, threshold 2");
    assert_eq!(eggs(&fauna).len(), 1);

    let (hatched, _, _) = afternoon(1, Some(3.0));
    assert_eq!(hatched, None, "an adult 3 m away suppresses hatching");

    let (hatched, dusk, _) = afternoon(1, Some(6.0));
    let t = hatched.expect("an adult 6 m away does not suppress it");
    assert!(t + 1 >= dusk && t <= dusk + 2, "hatched at {t}, dusk {dusk}");
}

// ------------------------------------------------------------------ case 14

/// Case 14: a snapshot round trip keeps wounds, strike and rest state and the dormant
/// eggs, and the schema has moved past WX4's.
#[test]
fn a_snapshot_keeps_wounds_rest_strikes_and_eggs() {
    assert!(SCHEMA >= 20, "P1 changes the animal record: schema {SCHEMA}");
    let world = at_hour(0.0, 14);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    // Midnight: the grazer sleeps, the nocturnal lanternjaws are up.
    let grazer = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    place(&mut fauna, grazer, STRIKER_X + IN_CAPSULE_M, LANE_Z, WEST);
    let striker = introduce(&mut fauna, &world, Founder::Lanternjaw, HUNTING);
    place(&mut fauna, striker, STRIKER_X, LANE_Z, EAST);
    drive(&mut fauna, striker, 0.0, true);
    let full = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    bury(&mut fauna, &world, EGG_SITE, 3);
    step_until(&mut fauna, &world, &mut flora, 40, |f| {
        f.view().animal(grazer).is_some_and(|g| g.wound > 0.0)
    })
    .expect("the sleeping grazer is wounded");
    // A few ticks on, the striker is certainly in its 3 s recovery.
    for _ in 0..3 {
        fauna.step_with(&world, &mut flora, 1);
    }

    let loaded = Fauna::load(&fauna.save()).expect("round trip");
    assert_eq!(loaded.animals, fauna.animals);
    assert_eq!(loaded.clutches, fauna.clutches);
    assert_eq!(loaded.ledger, fauna.ledger);
    let get = |id| *loaded.view().animal(id).unwrap();
    assert!(get(grazer).wound > 0.0);
    assert_eq!(get(striker).strike.map(|s| s.phase), Some(StrikePhase::Recovery));
    assert_eq!(get(full).rest, Some(Rest::Satiated));
    assert_eq!(eggs(&loaded).len(), 3);
    assert!(eggs(&loaded).iter().all(|c| c.dormancy.is_some()));
}
