//! Predation, the life half: the satiated rest, the buried egg bank and its hatching
//! trigger, and the snapshot (`design/predation-and-flight-2026-09-28.md` §2.8, §2.12,
//! §2.13). "Dusk" is the tick `life_daylight` first falls below `crate::LifeConfig::default().day_line`;
//! footfall is distinct prey heard (the tremor rule) within 2 m over the last clock-day.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::predation_fixture::*;
use crate::controller::{Actions, Controller, Response};
use crate::senses::{Fine, cone_occupancy, ray_first_hit};
use crate::{
    Command, DT, Fauna, FaunaConfig, Founder, Rest, SCHEMA,
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

/// Case 10, **changed by the minimal gut** (repair 2 item 7: the hunters' satiated rest
/// goes): a full lanternjaw with the clock off does not lie down. It rests awake at its
/// full basal upkeep, consulted as any awake body is, its eating latch off on a full
/// stomach; a hit leaves it awake.
#[test]
fn a_full_lanternjaw_rests_awake_at_its_basal_upkeep() {
    let world = clock_off(10);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(crate::rhythm::fixed_config());
    fauna.set_births_enabled(false);
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    let calls = Arc::new(AtomicUsize::new(0));
    fauna.controllers.set(lj, Box::new(Counting(calls.clone())));
    let phys = *fauna.config.founder(Founder::Lanternjaw);
    // A full stomach: its eating latch stays off.
    {
        let k = phys.gut.capacity(animal(&fauna, lj).body);
        let a = animal_mut(&mut fauna, lj);
        let food = &mut a.stomach.pools[crate::Food::Carrion.index()];
        let extra = k - food.organic;
        food.organic += extra;
        food.mineral += 0.05 * extra;
        food.energy += 2.0 * extra;
        fauna.ledger.introduced_organic_in += extra;
        fauna.ledger.introduced_mineral_in += 0.05 * extra;
        fauna.ledger.introduced_energy_in += 2.0 * extra;
    }
    fauna.step_with(&world, &mut flora, 1);
    let a = animal(&fauna, lj);
    assert_ne!(a.state, State::Asleep, "no satiated rest");
    assert_eq!(a.rest, None);
    assert!(!a.eating, "a full stomach does not want food");

    let before = fauna.ledger.respired_maintenance_out;
    fauna.step_with(&world, &mut flora, 1);
    let paid = fauna.ledger.respired_maintenance_out - before;
    let basal = phys.core.maintenance_per_s * animal(&fauna, lj).body * DT;
    assert!((paid - basal).abs() <= 1e-15, "awake upkeep {paid}, basal {basal}");
    let period = Founder::Lanternjaw.manifest().cadence_ticks();
    for _ in 0..(3 * period) {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert!(calls.load(Ordering::Relaxed) > 0, "an awake body is consulted");
    assert_closes(&fauna, "resting awake");
}

/// Under its own heuristic, a hungry lanternjaw (an empty stomach) beside a carcass eats
/// to the full event and stops: one meal, its latch off.
#[test]
fn the_ambush_heuristic_eats_a_carcass_to_the_full_event() {
    let world = clock_off(10);
    let mut flora = Flora::new(FloraConfig::default());
    // A fast gut (τ 5 s, b = 2k/τ), so the meal fills inside a couple of hundred ticks.
    let mut config = crate::rhythm::fixed_config();
    let g = &mut config.founders[Founder::Lanternjaw.index()].gut;
    g.digestion_s = [5.0; crate::Food::COUNT];
    g.bite_per_body_s = 2.0 * g.stomach_k / 5.0;
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    {
        let a = animal_mut(&mut fauna, lj);
        let s = a.stomach.total();
        a.stomach = crate::Stomach::default();
        fauna.ledger.introduced_organic_in -= s.organic;
        fauna.ledger.introduced_mineral_in -= s.mineral;
        fauna.ledger.introduced_energy_in -= s.energy;
    }
    let site = animal(&fauna, lj).site;
    assert!(flora.deposit(
        site,
        cubarium_voxel_flora::Deposit {
            kind: cubarium_voxel_flora::DepositKind::Carrion,
            // A grazer carcass at the shipped mass scale (x30 since main's 2026-09-29).
            organic: crate::SHIPPED_MASS_SCALE * 0.075,
            mineral: crate::SHIPPED_MASS_SCALE * 0.05 * 0.075,
            energy: crate::SHIPPED_MASS_SCALE * 2.0 * 0.075,
        }
    ));
    fauna
        .controllers
        .set(lj, Box::new(crate::LanternjawAmbush::new()));
    let full = step_until(&mut fauna, &world, &mut flora, 300, |f| {
        f.ledger.meals_by_founder_kind[Founder::Lanternjaw.index()].iter().sum::<u64>() > 0
    });
    assert!(full.is_some(), "it never ate to the full event");
    assert!(!animal(&fauna, lj).eating);
    assert!(fauna.ledger.eaten_carrion_by_founder[Founder::Lanternjaw.index()] > 0.0);
    assert_closes(&fauna, "fed");
}

/// the sleep is the other reason on the same state: a diurnal grazer asleep at
/// midnight rests for `Sleep`.
#[test]
fn wx4_sleep_is_a_rest_with_its_own_reason() {
    let world = at_hour(0.0, 10);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(crate::rhythm::fixed_config());
    let g = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    fauna.step_with(&world, &mut flora, 1);
    let a = animal(&fauna, g);
    assert_eq!((a.state, a.rest), (State::Asleep, Some(Rest::Sleep)));
}

// ------------------------------------------------------------------ case 15

/// Case 15, laying: a lanternjaw in sustained surplus lays a clutch of its
/// `clutch_size` buried eggs on its own face, each dormant; the books close.
#[test]
fn a_lanternjaw_in_surplus_lays_its_clutch_of_dormant_eggs() {
    let world = clock_off(15);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(crate::rhythm::fixed_config());
    let rule = fauna.config.founder(Founder::Lanternjaw).core.reproduction;
    assert_eq!(rule.mode, crate::BirthMode::Eggs);
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    // The clutch from surplus (LANTERNJAW-LIFE's many offspring): what a full reserve pays.
    let n = if rule.clutch_max > 0 {
        rule.clutch_for(animal(&fauna, lj).reserve)
    } else {
        rule.clutch_size
    } as usize;
    assert!(n > 0, "the fixture needs a clutch");
    // Main's breeding window (schema 20) reads the hold off the surplus history, so the
    // history is filled rather than the counter set.
    for _ in 0..rule.hold_ticks() {
        animal_mut(&mut fauna, lj).reproduction.window.tick(true, rule.hold_ticks());
    }
    let site = animal(&fauna, lj).site;
    step_until(&mut fauna, &world, &mut flora, 3, |f| !eggs(f).is_empty())
        .expect("a full lanternjaw past its hold lays");
    let laid = eggs(&fauna);
    assert_eq!(laid.len(), n);
    assert_eq!(fauna.ledger.eggs_laid as usize, n);
    for c in &laid {
        assert_eq!((c.site, c.count), (site, 1));
        assert!(c.dormancy.is_some(), "a laid egg is dormant");
    }
    assert_eq!(fauna.view().eggs_by_founder(Founder::Lanternjaw) as usize, n);
    assert_closes(&fauna, "laid");
}

/// The parent counts what it gave its offspring (the brain reward's `E_given`,
/// `design/handoffs/cotrain-stage0-2026-10-01.md` §1): after a laying, exactly the
/// clutch's organic matter.
#[test]
fn a_laying_counts_its_clutch_as_given() {
    let world = clock_off(15);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(crate::rhythm::fixed_config());
    let rule = fauna.config.founder(Founder::Lanternjaw).core.reproduction;
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    assert_eq!(animal(&fauna, lj).reproduction.given, 0.0);
    for _ in 0..rule.hold_ticks() {
        animal_mut(&mut fauna, lj).reproduction.window.tick(true, rule.hold_ticks());
    }
    step_until(&mut fauna, &world, &mut flora, 3, |f| !eggs(f).is_empty())
        .expect("a full lanternjaw past its hold lays");
    let laid: f64 = eggs(&fauna).iter().map(|c| c.organic).sum();
    let given = animal(&fauna, lj).reproduction.given;
    assert!(laid > 0.0);
    assert!((given - laid).abs() <= 1e-12 * laid, "given {given} vs laid {laid}");
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
    let want = crate::PredationConfig::default().egg_upkeep_per_s * before * DT;
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
    let mut fauna = Fauna::new(crate::rhythm::fixed_config());
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

/// Case 15, the draws: with the test's own 12 h median viability and footfall
/// thresholds drawn from 2..=10, each egg's viability has a median of about 12 h and its
/// threshold a mean of about 6, spread wide.
#[test]
fn eggs_draw_a_12_h_median_viability_and_thresholds_around_6() {
    let world = clock_off(15);
    let mut c = crate::rhythm::fixed_config();
    c.predation.egg_viability_median_s = 12.0 * 3600.0;
    c.predation.egg_footfall_min = 2;
    c.predation.egg_footfall_max = 10;
    let mut fauna = Fauna::new(c);
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

/// The hatching trigger itself: footfall at its threshold, at dusk, no adult within the
/// test's own 4 m suppression radius.
#[test]
fn the_hatching_trigger_needs_footfall_dusk_and_no_adult_within_4_m() {
    let cfg = crate::PredationConfig {
        hatch_suppress_m: 4.0,
        ..crate::PredationConfig::default()
    };
    assert!(egg_hatches(&cfg, 3, 3, true, None));
    assert!(egg_hatches(&cfg, 3, 5, true, None));
    assert!(!egg_hatches(&cfg, 3, 2, true, None), "not before its threshold");
    assert!(!egg_hatches(&cfg, 3, 3, false, None), "not by day");
    assert!(!egg_hatches(&cfg, 3, 3, true, Some(3.9)), "an adult within 4 m suppresses");
    assert!(egg_hatches(&cfg, 3, 3, true, Some(4.1)));
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
    assert!(last >= crate::LifeConfig::default().day_line, "16:00 is day");
    for t in 1..=120u64 {
        probe.step();
        let now = probe.life_daylight();
        if now < crate::LifeConfig::default().day_line && last >= crate::LifeConfig::default().day_line {
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
    // Every egg past its gate hatches at this dusk: the per-dusk draw is not what this
    // case is about.
    let mut fauna = Fauna::new(FaunaConfig {
        hatch_chance_per_dusk: 1.0,
        ..crate::rhythm::fixed_config()
    });
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
    // A `body_min` hatchling: its length the cube root of its share of the adult body.
    let want = (phys.core.body_min / phys.core.body_max).cbrt();
    assert!((length - want).abs() < 0.03, "a hatchling {length} of the adult's length, not {want}");
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
    let mut config = crate::rhythm::fixed_config();
    // It strikes a grown grazer: the lanternjaw's diet rules off (LANTERNJAW-DIET).
    no_diet(&mut config);
    let mut fauna = Fauna::new(config);
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
    // The minimal gut: no satiated rest; the stomach rides in the snapshot.
    assert_eq!(get(full).stomach, fauna.view().animal(full).unwrap().stomach);
    assert!(get(full).stomach.organic() > 0.0);
    assert_eq!(eggs(&loaded).len(), 3);
    assert!(eggs(&loaded).iter().all(|c| c.dormancy.is_some()));
}
