//! **The minimal gut and realistic metabolism**, written before the rule
//! (`design/handoffs/gut-meals-2026-10-03.md`; the design is
//! `design/metabolism-meals-2026-10-02.md`, its "Repair 2" section binding, item 12 the
//! tests).
//!
//! A child of `step`, on the predation fixture's plain. Every case is a **controlled
//! fixture**: a fixed adult body unless the case is about growth, breeding off, lifespan
//! off, and — where food is in the case — ample carrion on every face of the plain.
//!
//! **The cadence fixture eats carrion through the walker's path, whatever the lineage.**
//! The interval the design predicts, `(1 − s0)·k·yield / E`, is the gut's: the stomach
//! `k`, the appetite floor `s0`, the yield and the rhythm-aware basal spend `E`. Food
//! access is not in it. So each lineage keeps its own gut, upkeep, sleep factor and
//! rhythm, and its diet, food class and flight are set to a standing walker's carrion,
//! at yield 0.5 and the lineage's own `τ`, with mineral enough that the yield is not
//! mineral-limited. The flyer's and the glider's own feeding paths are gated by the
//! same latch ([`a_full_stomach_on_food_takes_no_bite`] covers the walker's).
//!
//! Readings the tests fix where the spec leaves a choice:
//! - **A meal starts** when the eating latch goes on; an interval is the time between two
//!   successive starts.
//! - **A sleep flip** is any change of [`State::Asleep`] in either direction.
//! - **"Wanting food"** is the eating latch ([`crate::Animal::eating`]).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::predation_fixture::*;
use crate::controller::{Actions, Controller, Response, Scripted};
use crate::{
    Activity, Attachment, BoutKind, Command, DT, Diet, Fauna, FaunaConfig, Food, Founder,
    PackMember, StartingStores, State,
};
use cubarium_voxel::World;
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site, Taken};

/// Astra's exact intervals (repair-1 check M3), world days, in the design's table order.
const ASTRA: [(Founder, f64); 9] = [
    (Founder::Bellwing, 0.1012),
    (Founder::Seedporter, 0.3162),
    (Founder::Browser, 0.5060),
    (Founder::Loftstrider, 0.6324),
    (Founder::Blind, 0.5407),
    (Founder::Capgnawer, 0.5407),
    (Founder::RippleSnail, 0.4866),
    (Founder::Chorister, 1.2649),
    (Founder::Lanternjaw, 3.0415),
];

/// The ecological day, seconds (the default 120-minute cycle).
const DAY_S: f64 = 7200.0;

/// Carrion's mineral and energy per unit organic in the fixtures: mineral enough that no
/// lineage's yield is mineral-limited (`0.1 / n_tissue ≥ 0.5`), energy at 2.
const CARRION_MINERAL: f64 = 0.1;
const CARRION_ENERGY: f64 = 2.0;

/// `organic` of carrion on every face of the plain.
fn carrion_everywhere(world: &World, flora: &mut Flora, organic: f64) {
    let c = world.view().config;
    for z in 0..c.depth {
        for x in 0..c.width {
            assert!(flora.deposit(
                Site { x, y: SURFACE_Y, z },
                Deposit {
                    kind: DepositKind::Carrion,
                    organic,
                    mineral: CARRION_MINERAL * organic,
                    energy: CARRION_ENERGY * organic,
                },
            ));
        }
    }
}

/// Take every face's carrion away.
fn clear_carrion(world: &World, flora: &mut Flora) {
    let c = world.view().config;
    for z in 0..c.depth {
        for x in 0..c.width {
            let _ = flora.take_carrion(Site { x, y: SURFACE_Y, z }, 1e12);
        }
    }
}

/// The lineage's own food class: the first of its diet.
fn own_food(config: &FaunaConfig, f: Founder) -> Food {
    config.founder(f).diet.foods().next().expect("a diet")
}

/// The cadence fixture's table for `f`: its own gut, upkeep, sleep and rhythm; a
/// standing walker's carrion diet at yield 0.5 and its own food's `τ`; no lifespan, no
/// breeding.
fn carrion_config(f: Founder) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    let tau = c.founder(f).gut.digestion_s[own_food(&c, f).index()];
    let p = &mut c.founders[f.index()];
    p.diet = Diet::only(Food::Carrion);
    p.yield_by_food[Food::Carrion.index()] = 0.5;
    p.gut.digestion_s[Food::Carrion.index()] = tau;
    p.flight = None;
    p.glider = None;
    p.gut_seeds = None;
    p.aquatic = false;
    p.bite_spill_fraction = 0.0;
    p.core.lifespan_median_s = 0.0;
    p.core.reproduction.surplus_floor = 1e9;
    c
}

/// A fast gut on `f` for short tests: every class at `τ` seconds and `b = 2k / τ`.
fn fast_gut(config: &mut FaunaConfig, f: Founder, tau: f64) {
    let g = &mut config.founders[f.index()].gut;
    g.digestion_s = [tau; Food::COUNT];
    g.bite_per_body_s = 2.0 * g.stomach_k / tau;
}

/// A controller that stands and holds `feed`.
fn stand_and_feed() -> Box<dyn Controller> {
    Box::new(Scripted::new(vec![Actions {
        forward: 0.0,
        turn: 0.0,
        feed: 1.0,
    }]))
}

/// A controller that stands, holds `feed` and lights a lure.
struct Lurer;
impl Controller for Lurer {
    fn drive(&mut self, _o: &[f64]) -> Response {
        Response::Bounded(Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        })
    }
    fn reset(&mut self) {}
    fn lure(&self) -> f64 {
        1.0
    }
}

/// Body `id`'s stomach capacity `K = k · body`.
fn capacity(fauna: &Fauna, id: u64) -> f64 {
    let a = animal(fauna, id);
    let f = a.founder.expect("a founder");
    fauna.config.founder(f).gut.stomach_k * a.body
}

/// Fill `id`'s stomach to `fraction` of its capacity with fixture carrion, booked as
/// introduced so the books still close.
fn fill_stomach(fauna: &mut Fauna, id: u64, fraction: f64) {
    let k = capacity(fauna, id);
    let a = animal_mut(fauna, id);
    let old = a.stomach.total();
    let organic = fraction * k;
    let new = Taken {
        organic,
        mineral: CARRION_MINERAL * organic,
        energy: CARRION_ENERGY * organic,
    };
    a.stomach = crate::Stomach::default();
    a.stomach.pools[Food::Carrion.index()] = new;
    a.eating = false;
    let l = &mut fauna.ledger;
    l.introduced_organic_in += new.organic - old.organic;
    l.introduced_mineral_in += new.mineral - old.mineral;
    l.introduced_energy_in += new.energy - old.energy;
}

/// Set `id`'s reserve to `fraction` of its cap, booked as introduced.
fn set_reserve(fauna: &mut Fauna, id: u64, fraction: f64) {
    let a = animal(fauna, id);
    let f = a.founder.expect("a founder");
    let cap = fauna.config.founder(f).core.reserve_of(a.body);
    let want = fraction * cap;
    let d = want - a.reserve;
    // Mineral and energy at the body's own density, so its composition does not move.
    let (dm, de) = (a.mineral / a.organic() * d, a.energy / a.organic() * d);
    let a = animal_mut(fauna, id);
    a.reserve = want;
    a.mineral += dm;
    a.energy += de;
    fauna.ledger.introduced_organic_in += d;
    fauna.ledger.introduced_mineral_in += dm;
    fauna.ledger.introduced_energy_in += de;
}

fn bites(fauna: &Fauna, f: Founder) -> u64 {
    fauna.ledger.bites_by_founder[f.index()]
}

fn meals(fauna: &Fauna, f: Founder) -> u64 {
    fauna.ledger.meals_by_founder_kind[f.index()].iter().sum()
}

/// The books closed after a run of `ticks`: [`assert_closes`]'s residual, with a float
/// allowance that grows with the run. Steady small increments into a large accumulator
/// round the same way each tick (measured: about 4e-18 of mineral a tick on a 0.08 stock,
/// a famine recovery), so a 50,000-tick run drifts a few parts in 1e12 with nothing lost.
/// The scale is the larger of the stock and the flows through it, so a layer emptied by a
/// death is judged against what passed through it, not against zero.
fn assert_books(fauna: &Fauna, when: &str, ticks: u64) {
    let v = fauna.view();
    let l = v.ledger;
    for (held, expected, through, what) in [
        (v.organic(), l.expected_organic(), l.introduced_organic_in + l.eaten_organic_in, "organic"),
        (v.mineral(), l.expected_mineral(), l.introduced_mineral_in + l.eaten_mineral_in, "mineral"),
        (v.energy(), l.expected_energy(), l.introduced_energy_in + l.eaten_energy_in, "energy"),
    ] {
        let scale = held.abs().max(through.abs()).max(1e-6);
        let allow = (1e-12 + 1e-16 * ticks as f64) * scale;
        assert!(
            (held - expected).abs() <= allow,
            "{when}: fauna {what} holds {held}, the ledger expects {expected} (allowance {allow:e})"
        );
    }
}

// ------------------------------------------------------------------ the defaults

/// The design's tables: `f` (as `maintenance_per_s · 7200`), `k`, `τ` of the lineage's
/// own food, `b = 2k/τ`, `s0` 0.15, `full_tolerance` 0.98, `emergency_clear_hunger` 0.6,
/// motor at its basal ratio, the repair-2 lifespans as Wrysk changed them on 2026-10-03
/// (loftstrider 16 days, seedporter 8), and the rider's take 0.09.
#[test]
fn the_gut_defaults_are_the_designs() {
    let c = FaunaConfig::default();
    let table = [
        (Founder::Bellwing, 0.15, 0.03, 30.0, 6.0),
        (Founder::Seedporter, 0.08, 0.05, 60.0, 8.0),
        (Founder::Browser, 0.06, 0.06, 120.0, 8.0),
        (Founder::Loftstrider, 0.04, 0.05, 120.0, 16.0),
        (Founder::Blind, 0.03, 0.02, 120.0, 6.0),
        (Founder::Capgnawer, 0.03, 0.02, 120.0, 6.0),
        (Founder::RippleSnail, 0.02, 0.012, 120.0, 6.0),
        (Founder::Chorister, 0.08, 0.20, 300.0, 12.0),
        (Founder::Lanternjaw, 0.04, 0.15, 300.0, 12.0),
    ];
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * b.abs().max(1e-12);
    for (f, daily, k, tau, life_days) in table {
        let p = c.founder(f);
        let n = f.name();
        assert!(close(p.core.maintenance_per_s * DAY_S, daily), "{n}: f");
        assert!(close(p.motor_respiration_per_s, p.core.maintenance_per_s), "{n}: motor tie");
        assert!(close(p.gut.stomach_k, k), "{n}: k");
        assert!(close(p.gut.digestion_s[own_food(&c, f).index()], tau), "{n}: tau");
        assert!(close(p.gut.bite_per_body_s, 2.0 * k / tau), "{n}: b = 2k/tau");
        assert!(close(p.gut.s0, 0.15), "{n}: s0");
        assert!(close(p.gut.full_tolerance, 0.98), "{n}: full_tolerance");
        assert!(close(p.emergency_clear_hunger, 0.6), "{n}: emergency_clear_hunger");
        assert!(close(p.core.lifespan_median_s, life_days * DAY_S), "{n}: lifespan");
    }
    assert_eq!(c.parasite_take_per_day, 0.09);
    assert_eq!(c.life.ecological_day_s, DAY_S);
}

// ------------------------------------------------------------------ the cadence

/// **The interval** (item 12): a fixed, non-breeding adult standing on ample carrion
/// under its own rhythm completes at least three meal intervals, and their mean is within
/// ±30 % of Astra's exact interval for its lineage.
fn meal_interval(f: Founder) {
    let want = ASTRA.iter().find(|(g, _)| *g == f).expect("in the table").1;
    let config = carrion_config(f);
    // Start where the lineage is awake: mid-morning for a day-active body, midnight for
    // a night-active one.
    let start_hour = match config.founder(f).active {
        Activity::Nocturnal => 0.0,
        _ => 10.0,
    };
    let mut world = plain(
        DayClock {
            start_hour,
            ..DayClock::default()
        },
        7,
    );
    let mut flora = Flora::new(FloraConfig::default());
    carrion_everywhere(&world, &mut flora, 10.0);
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores::FULL);
    place(&mut fauna, id, 6.0, 1.5, EAST);
    fauna.controllers.set(id, stand_and_feed());
    let body0 = animal(&fauna, id).body;

    let max_ticks = ((3.0 * 1.3 + 1.5) * want * DAY_S / DT) as u64 + (DAY_S / DT) as u64;
    let mut starts: Vec<u64> = Vec::new();
    // Meals completed, and normal ones, at each start (Astra's finding 10: each counted
    // interval spans one completed normal bout, and no start is emergency chatter).
    let mut at_start: Vec<(u64, u64)> = Vec::new();
    let mut was = animal(&fauna, id).eating;
    if was {
        starts.push(0);
        at_start.push((0, 0));
    }
    for t in 1..=max_ticks {
        world.step();
        fauna.step_with(&world, &mut flora, 1);
        let a = animal(&fauna, id);
        if a.eating && !was {
            starts.push(t);
            let k = &fauna.ledger.meals_by_founder_kind[f.index()];
            at_start.push((k.iter().sum(), k[BoutKind::Normal.index()]));
            assert!(!a.emergency, "{}: a meal started in an emergency at tick {t}", f.name());
        }
        was = a.eating;
        if starts.len() >= 4 {
            break;
        }
    }
    for w in at_start.windows(2) {
        assert_eq!(w[1].0 - w[0].0, 1, "{}: one completed bout between starts: {at_start:?}", f.name());
        assert_eq!(w[1].1 - w[0].1, 1, "{}: and it was a normal one: {at_start:?}", f.name());
    }
    let a = animal(&fauna, id);
    assert!(
        (a.body - body0).abs() <= 1e-9 * body0,
        "{}: the adult's body is fixed ({body0} → {})",
        f.name(),
        a.body
    );
    assert!(
        starts.len() >= 4,
        "{}: {} meal starts in {:.2} days (want 4 for 3 intervals)",
        f.name(),
        starts.len(),
        max_ticks as f64 * DT / DAY_S
    );
    let intervals: Vec<f64> = starts.windows(2).map(|w| (w[1] - w[0]) as f64 * DT / DAY_S).collect();
    let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
    eprintln!(
        "{}: intervals {intervals:.4?} days, mean {mean:.4} against {want} ({:+.1} %)",
        f.name(),
        100.0 * (mean / want - 1.0)
    );
    assert!(
        (mean / want - 1.0).abs() <= 0.30,
        "{}: mean interval {mean:.4} days, Astra's {want} (±30 %)",
        f.name()
    );
    assert_books(&fauna, "after the cadence run", starts.last().copied().unwrap_or(max_ticks));
}

// The bellwing's interval is `flight_tests::meal_interval_bellwing`: its observation is
// a flyer's, so it cannot stand in for a walker here; it sips a bloomcrown instead.
#[test]
#[ignore = "study: run by name (multi-day cadence)"]
fn meal_interval_seedporter() {
    meal_interval(Founder::Seedporter);
}
#[test]
#[ignore = "study: run by name (multi-day cadence)"]
fn meal_interval_frondgrazer() {
    meal_interval(Founder::Browser);
}
#[test]
#[ignore = "study: run by name (multi-day cadence)"]
fn meal_interval_loftstrider() {
    meal_interval(Founder::Loftstrider);
}
#[test]
#[ignore = "study: run by name (multi-day cadence)"]
fn meal_interval_littershredder() {
    meal_interval(Founder::Blind);
}
#[test]
#[ignore = "study: run by name (multi-day cadence)"]
fn meal_interval_capgnawer() {
    meal_interval(Founder::Capgnawer);
}
#[test]
#[ignore = "study: run by name (multi-day cadence)"]
fn meal_interval_ripple_snail() {
    meal_interval(Founder::RippleSnail);
}
#[test]
#[ignore = "study: run by name (multi-day cadence)"]
fn meal_interval_chorister() {
    meal_interval(Founder::Chorister);
}
#[test]
#[ignore = "study: run by name (multi-day cadence)"]
fn meal_interval_lanternjaw() {
    meal_interval(Founder::Lanternjaw);
}

// ------------------------------------------------------------------ the stomach

/// A full stomach on food takes no bite: a fed adult with a full stomach and a full
/// reserve, standing on carrion with `feed` held and awake, takes nothing for 400 ticks,
/// and its eating latch stays off.
#[test]
fn a_full_stomach_on_food_takes_no_bite() {
    let f = Founder::Browser;
    let world = clock_off(1);
    let mut flora = Flora::new(FloraConfig::default());
    carrion_everywhere(&world, &mut flora, 1.0);
    let mut fauna = Fauna::new(carrion_config(f));
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores::FULL);
    place(&mut fauna, id, 6.0, 1.5, EAST);
    fill_stomach(&mut fauna, id, 0.99);
    fauna.controllers.set(id, stand_and_feed());
    for _ in 0..400 {
        fauna.step_with(&world, &mut flora, 1);
        assert!(!animal(&fauna, id).eating, "the latch stays off on a full stomach");
    }
    assert_eq!(bites(&fauna, f), 0, "no bite on a full stomach");
    assert_books(&fauna, "full stomach", 400);

    // The same body with an empty stomach does eat (the case is not vacuous).
    let mut fauna = Fauna::new(carrion_config(f));
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores::FULL);
    place(&mut fauna, id, 6.0, 1.5, EAST);
    fill_stomach(&mut fauna, id, 0.0);
    fauna.controllers.set(id, stand_and_feed());
    for _ in 0..400 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert!(bites(&fauna, f) > 0, "an empty stomach eats");
    let s = animal(&fauna, id).stomach.organic();
    assert!(s > 0.0 && s <= capacity(&fauna, id) * (1.0 + 1e-12), "0 < S ≤ K: {s}");
}

/// Interrupted food access keeps the latch on: an eating body whose food is taken away
/// before its stomach is full keeps wanting, and finishes its meal when food returns.
#[test]
fn interrupted_access_keeps_the_latch_on() {
    let f = Founder::Browser;
    let world = clock_off(2);
    let mut flora = Flora::new(FloraConfig::default());
    carrion_everywhere(&world, &mut flora, 1.0);
    // A fast gut (τ 6 s, b = 2k/τ), so the meal fills in about 80 ticks.
    let mut config = carrion_config(f);
    fast_gut(&mut config, f, 6.0);
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores::FULL);
    place(&mut fauna, id, 6.0, 1.5, EAST);
    fill_stomach(&mut fauna, id, 0.0);
    fauna.controllers.set(id, stand_and_feed());
    for _ in 0..20 {
        fauna.step_with(&world, &mut flora, 1);
    }
    let a = animal(&fauna, id);
    assert!(a.eating, "it is eating");
    assert!(a.stomach.organic() > 0.0 && a.stomach.organic() < 0.9 * capacity(&fauna, id));
    clear_carrion(&world, &mut flora);
    for _ in 0..200 {
        fauna.step_with(&world, &mut flora, 1);
        assert!(animal(&fauna, id).eating, "losing access does not clear the latch");
    }
    carrion_everywhere(&world, &mut flora, 1.0);
    let full = step_until(&mut fauna, &world, &mut flora, 400, |fa| {
        !animal(fa, id).eating
    });
    assert!(full.is_some(), "food back: it eats to the full event and stops");
    let a = animal(&fauna, id);
    assert!(a.stomach.organic() >= 0.98 * capacity(&fauna, id) * (1.0 - 1e-9));
    assert_eq!(meals(&fauna, f), 1, "one meal, completed");
    assert_books(&fauna, "interrupted meal", 700);
}

/// Famine recovery: a starved adult on ample food keeps `0 ≤ S ≤ K` every tick, refills
/// its reserve out of the emergency, records its bouts with their durations, and the books
/// close.
#[test]
#[ignore = "study: run by name (long run; repair 3 item 9)"]
fn famine_recovery_keeps_the_stomach_bounded_and_the_books_closed() {
    famine_case(None, (40.0 * 60.0 / DT) as u64);
}

/// [`famine_recovery_keeps_the_stomach_bounded_and_the_books_closed`] on a fast gut (τ 1 s)
/// for 600 ticks: the same claims, short.
#[test]
fn famine_recovery_on_a_fast_gut() {
    famine_case(Some(1.0), 600);
}

fn famine_case(tau: Option<f64>, ticks: u64) {
    let f = Founder::Browser;
    let world = clock_off(3);
    let mut flora = Flora::new(FloraConfig::default());
    carrion_everywhere(&world, &mut flora, 1.0);
    let mut config = carrion_config(f);
    if let Some(t) = tau {
        fast_gut(&mut config, f, t);
    }
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores { body: 1.0, reserve: 0.0 });
    place(&mut fauna, id, 6.0, 1.5, EAST);
    fill_stomach(&mut fauna, id, 0.0);
    fauna.controllers.set(id, stand_and_feed());
    for _ in 0..ticks {
        fauna.step_with(&world, &mut flora, 1);
        let a = animal(&fauna, id);
        let s = a.stomach.organic();
        assert!(s >= 0.0 && s <= capacity(&fauna, id) * (1.0 + 1e-12), "0 ≤ S ≤ K: {s}");
        for p in a.stomach.pools {
            assert!(p.organic >= 0.0 && p.mineral >= 0.0 && p.energy >= 0.0);
        }
    }
    let a = animal(&fauna, id);
    let cap = fauna.config.founder(f).core.reserve_of(a.body);
    let hunger = 1.0 - a.reserve / cap;
    assert!(hunger < 0.6, "recovered out of the emergency: hunger {hunger}");
    assert!(!a.emergency, "the emergency latch cleared below 0.6");
    let l = &fauna.ledger;
    let e = BoutKind::Emergency.index();
    assert!(l.meals_by_founder_kind[f.index()][e] >= 1, "an emergency bout is recorded");
    assert!(l.meal_ticks_by_founder_kind[f.index()][e] > 0, "with its duration");
    assert_books(&fauna, "famine recovery", ticks);
}

// ------------------------------------------------------------------ sleep

/// A fed sleeper sleeps through its phase: a fed frondgrazer held at midnight and a fed
/// littershredder held at noon sleep for the whole sleeping window without one wake.
#[test]
#[ignore = "study: run by name (long run; repair 3 item 9)"]
fn a_fed_sleeper_sleeps_through_its_phase() {
    for (f, hour) in [(Founder::Browser, 0.0), (Founder::Blind, 12.0)] {
        let world = at_hour(hour, 4);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(carrion_config(f));
        fauna.set_births_enabled(false);
        let id = introduce(&mut fauna, &world, f, StartingStores::FULL);
        fauna.controllers.set(id, stand_and_feed());
        // The longest sleeping phase of the default cycle is under 45 minutes.
        for t in 0..(45.0 * 60.0 / DT) as u64 {
            fauna.step_with(&world, &mut flora, 1);
            let a = animal(&fauna, id);
            assert_eq!(a.state, State::Asleep, "{}: awake at tick {t}", f.name());
        }
        assert_eq!(fauna.ledger.emergency_wakes_by_founder[f.index()], 0);
    }
}

/// A starving sleeper wakes, refills and does not flicker: a frondgrazer at hunger 0.95
/// with an empty stomach, held at midnight on ample carrion, wakes on the emergency
/// latch, eats, and over the hour makes at most two sleep flips.
#[test]
#[ignore = "study: run by name (long run; repair 3 item 9)"]
fn a_starving_sleeper_wakes_refills_and_flips_at_most_twice_an_hour() {
    let f = Founder::Browser;
    let world = at_hour(0.0, 5);
    let mut flora = Flora::new(FloraConfig::default());
    carrion_everywhere(&world, &mut flora, 1.0);
    let mut fauna = Fauna::new(carrion_config(f));
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores::FULL);
    place(&mut fauna, id, 6.0, 1.5, EAST);
    fill_stomach(&mut fauna, id, 0.0);
    fauna.controllers.set(id, stand_and_feed());
    // Fed, it falls asleep at midnight; then it is starved in its sleep.
    set_reserve(&mut fauna, id, 1.0);
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(animal(&fauna, id).state, State::Asleep, "asleep first");
    set_reserve(&mut fauna, id, 0.05);
    let flips0 = fauna.ledger.sleep_flips_by_founder[f.index()];
    let r0 = animal(&fauna, id).reserve;
    let mut flips = 0u32;
    let mut was = animal(&fauna, id).state == State::Asleep;
    for _ in 0..(3600.0 / DT) as u64 {
        fauna.step_with(&world, &mut flora, 1);
        let asleep = animal(&fauna, id).state == State::Asleep;
        if asleep != was {
            flips += 1;
        }
        was = asleep;
    }
    assert!(fauna.ledger.emergency_wakes_by_founder[f.index()] >= 1, "it woke on the emergency");
    assert!(bites(&fauna, f) > 0, "and ate");
    assert!(animal(&fauna, id).reserve > r0, "its reserve refilled");
    assert!(flips <= 2, "{flips} sleep flips in the hour");
    assert_eq!(fauna.ledger.sleep_flips_by_founder[f.index()] - flips0, u64::from(flips));
    assert_books(&fauna, "emergency refill", 72_001);
}

// ------------------------------------------------------------------ hunters

/// A lanternjaw facing a grazer in its capsule, with `feed` held and a lit lure, on the
/// default table awake at every hour. `stomach` is its stomach's fill.
fn hunter_case(stomach: f64, with_prey: bool) -> Fauna {
    let world = clock_off(6);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(hunting_config());
    fauna.set_births_enabled(false);
    let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores::FULL);
    place(&mut fauna, lj, STRIKER_X, LANE_Z, EAST);
    fill_stomach(&mut fauna, lj, stomach);
    fauna.controllers.set(lj, Box::new(Lurer));
    if with_prey {
        let prey = introduce(&mut fauna, &world, Founder::Browser, StartingStores { body: 0.4, reserve: 1.0 });
        place(&mut fauna, prey, STRIKER_X + IN_CAPSULE_M, LANE_Z, EAST);
        let _off: Arc<AtomicBool> = drive(&mut fauna, prey, 0.0, false);
    }
    for _ in 0..80 {
        fauna.step_with(&world, &mut flora, 1);
    }
    fauna
}

/// A sated hunter starts no lure, target or strike; the same body hungry does.
#[test]
fn a_sated_hunter_starts_no_lure_or_strike() {
    let sated = hunter_case(0.99, true);
    assert_eq!(strikes(&sated), 0, "a full stomach strikes nothing");
    assert_eq!(sated.ledger.respired_hunt_out, 0.0, "and pays no lure");
    let lj = sated.animals.iter().find(|a| a.founder == Some(Founder::Lanternjaw)).expect("alive");
    assert_eq!(lj.founder_state.lure, 0.0, "its lure is dark");

    let hungry = hunter_case(0.0, true);
    assert!(strikes(&hungry) > 0, "an empty stomach strikes a prey in its capsule");
    assert!(hungry.ledger.respired_hunt_out > 0.0, "and pays its lure and strike");
}

/// No prey, no strike: a hungry hunter with nothing in reach strikes nothing, eats
/// nothing and makes no reserve.
#[test]
fn a_hungry_hunter_with_no_prey_makes_no_strike() {
    let fauna = hunter_case(0.0, false);
    assert_eq!(strikes(&fauna), 0);
    assert_eq!(bites(&fauna, Founder::Lanternjaw), 0);
    assert_eq!(meals(&fauna, Founder::Lanternjaw), 0);
    let lj = fauna.animals.iter().find(|a| a.founder == Some(Founder::Lanternjaw)).expect("alive");
    assert!(lj.eating, "it still wants food");
    assert_eq!(lj.stomach.organic(), 0.0);
    let cap = fauna.config.founder(Founder::Lanternjaw).core.reserve_of(lj.body);
    assert!(lj.reserve <= cap, "no reserve created");
    assert!(fauna.ledger.respired_hunt_out > 0.0, "a hungry hunter may light its lure");
}

/// **The pack hunt start** (repair 2 item 7): a pack of four choristers under their pack
/// heuristic with a grazer in sight takes it as a target when at least half its adults
/// want food, or any adult is in emergency, and not when only one wants food.
#[test]
fn a_pack_hunts_when_half_want_food_or_one_is_in_emergency() {
    // (wanting adults, adults in emergency with a full stomach) → a target is taken.
    for (wanting, emergency, hunts) in [(0, 0, false), (1, 0, false), (2, 0, true), (0, 1, true)] {
        let world = clock_off(8);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = FaunaConfig::default();
        c.founders[Founder::Chorister.index()].active = Activity::Always;
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let den = Site { x: 8, y: SURFACE_Y, z: 6 };
        assert!(fauna.apply(
            &world,
            Command::FoundPack {
                den,
                founder: Founder::Chorister,
                stores: StartingStores::FULL,
                heading_rad: EAST,
            }
        ));
        let adults: Vec<u64> = fauna
            .animals
            .iter()
            .filter(|a| a.pack.is_some_and(|m: PackMember| !m.pup))
            .map(|a| a.id)
            .collect();
        assert_eq!(adults.len(), 4);
        for (k, &id) in adults.iter().enumerate() {
            place(&mut fauna, id, 2.0, 0.8 + 0.5 * k as f64, EAST);
            let teacher = crate::controller::heuristic(Founder::Chorister, fauna.config.teachers);
            fauna.controllers.set(id, teacher);
            if k < wanting {
                fill_stomach(&mut fauna, id, 0.0);
            } else {
                fill_stomach(&mut fauna, id, 0.99);
                if k < wanting + emergency {
                    set_reserve(&mut fauna, id, 0.05);
                }
            }
        }
        let prey = introduce(&mut fauna, &world, Founder::Browser, StartingStores { body: 0.5, reserve: 1.0 });
        // Inside every adult's 2 m cone ahead.
        place(&mut fauna, prey, 3.5, 1.5, EAST);
        let _off = drive(&mut fauna, prey, 0.0, false);
        for _ in 0..40 {
            fauna.step_with(&world, &mut flora, 1);
        }
        assert_eq!(
            fauna.ledger.pack_chases > 0,
            hunts,
            "{wanting} wanting, {emergency} in emergency: chases {}",
            fauna.ledger.pack_chases
        );
    }
}

// ------------------------------------------------------------------ provisioning

/// **Regurgitation** (repair 2 item 6): an adult at its den with carrion in its stomach
/// passes it raw to a pup that wants food, into the pup's stomach, capped by its room: the
/// food keeps its composition, the two stomachs' sum is conserved, nothing goes through
/// the reserve route, and the books close.
#[test]
fn regurgitation_conserves_and_keeps_composition() {
    let world = clock_off(9);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    c.founders[Founder::Chorister.index()].active = Activity::Always;
    let mut p = c.founders[Founder::Chorister.index()].pack.expect("a pack lineage");
    p.pack_size = 1;
    c.founders[Founder::Chorister.index()].pack = Some(p);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let den = Site { x: 8, y: SURFACE_Y, z: 6 };
    assert!(fauna.apply(
        &world,
        Command::FoundPack {
            den,
            founder: Founder::Chorister,
            stores: StartingStores::FULL,
            heading_rad: EAST,
        }
    ));
    let pack = fauna.packs()[0].id;
    let adult = fauna.animals[0].id;
    place(&mut fauna, adult, 2.125, 1.625, EAST);
    fill_stomach(&mut fauna, adult, 0.6);
    let pup = introduce(&mut fauna, &world, Founder::Chorister, StartingStores { body: 0.3, reserve: 0.2 });
    animal_mut(&mut fauna, pup).pack = Some(PackMember {
        pack,
        pup: true,
        calling_ticks: 0,
    });
    place(&mut fauna, pup, 2.125, 2.125, EAST);
    fill_stomach(&mut fauna, pup, 0.0);
    let before = animal(&fauna, adult).stomach.total().organic + animal(&fauna, pup).stomach.total().organic;
    for _ in 0..20 {
        fauna.step_with(&world, &mut flora, 1);
    }
    let (a, p) = (animal(&fauna, adult), animal(&fauna, pup));
    let got = p.stomach.total();
    assert!(got.organic > 0.0, "the pup's stomach received raw food");
    assert!(got.organic <= capacity(&fauna, pup) * (1.0 + 1e-12), "capped by its room");
    assert!((got.mineral / got.organic - CARRION_MINERAL).abs() <= 1e-9, "mineral kept");
    assert!((got.energy / got.organic - CARRION_ENERGY).abs() <= 1e-9, "energy kept");
    assert!(fauna.ledger.regurgitated.organic > 0.0, "booked regurgitated");
    assert_eq!(fauna.ledger.provision_given_organic, 0.0, "not the reserve route");
    // Digestion moves stomach food on, so compare with what was digested out.
    let after = a.stomach.total().organic + got.organic + fauna.ledger.digested.organic;
    assert!((after - before).abs() <= 1e-9 * before, "conserved: {before} → {after}");
    assert_books(&fauna, "regurgitation", 20);
}

// ------------------------------------------------------------------ riders

/// A rider on a fixed 1.5-ou adult grazer, all awake, reaches drop-off in about 1.19
/// world days (Astra's integration), ±30 %, at `parasite_take_per_day` 0.09.
#[test]
#[ignore = "study: run by name (long run; repair 3 item 9)"]
fn a_rider_on_a_fixed_host_reaches_dropoff_in_about_1_19_days() {
    let world = clock_off(10);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    fauna.set_births_enabled(false);
    let host = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    let host_body = animal(&fauna, host).body;
    assert!((host_body - 1.5).abs() < 1e-9, "the adult grazer is 1.5 ou");
    let lj = fauna.config.founder(Founder::Lanternjaw).core;
    // The default egg's hatchling: body_min of structure, the rest of the egg as reserve.
    let egg = lj.reproduction.egg_organic;
    let rider = introduce(
        &mut fauna,
        &world,
        Founder::Lanternjaw,
        StartingStores {
            body: lj.body_min / lj.body_max,
            reserve: (egg - lj.body_min) / lj.reserve_of(lj.body_min),
        },
    );
    {
        let h = animal(&fauna, host);
        let r = animal_mut(&mut fauna, rider);
        r.attached = Some(Attachment { host });
        r.site = h.site;
        r.pose = h.pose;
    }
    let cap = (2.0 * DAY_S / DT) as u64;
    let mut dropped = None;
    for t in 1..=cap {
        fauna.step_with(&world, &mut flora, 1);
        // The host is held fixed: its structure and a full reserve, every tick.
        let full = fauna.config.founder(Founder::Browser).core.reserve_of(host_body);
        let h = animal_mut(&mut fauna, host);
        h.body = host_body;
        h.reserve = full;
        if animal(&fauna, rider).attached.is_none() {
            dropped = Some(t);
            break;
        }
    }
    let t = dropped.expect("the rider dropped off within two days");
    let days = t as f64 * DT / DAY_S;
    eprintln!("rider drop-off at {days:.3} world days (Astra 1.19)");
    assert!((days / 1.19 - 1.0).abs() <= 0.30, "drop-off at {days:.3} days, not ~1.19");
}

// ------------------------------------------------------------------ growth

/// A juvenile's maturation is **measured**: a newborn-sized frondgrazer on ample food
/// under its own rhythm reaches breeding size, and its time is reported against the
/// plan's 1.5-day expectation (asserted only loosely: it matures, and within 4.5 days).
#[test]
#[ignore = "study: run by name (multi-day growth)"]
fn a_juvenile_reaches_breeding_size_and_its_time_is_measured() {
    let f = Founder::Browser;
    let config = carrion_config(f);
    let mut world = plain(
        DayClock {
            start_hour: 10.0,
            ..DayClock::default()
        },
        11,
    );
    let mut flora = Flora::new(FloraConfig::default());
    carrion_everywhere(&world, &mut flora, 10.0);
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let sc = fauna.config.founder(f).core;
    let id = introduce(&mut fauna, &world, f, StartingStores { body: sc.body_min / sc.body_max, reserve: 1.0 });
    place(&mut fauna, id, 6.0, 1.5, EAST);
    fill_stomach(&mut fauna, id, 0.0);
    fauna.controllers.set(id, stand_and_feed());
    let cap = (4.5 * DAY_S / DT) as u64;
    let mut at = None;
    for t in 1..=cap {
        world.step();
        fauna.step_with(&world, &mut flora, 1);
        if animal(&fauna, id).body >= sc.birth_body {
            at = Some(t);
            break;
        }
    }
    let t = at.expect("it reaches breeding size within 4.5 days");
    let days = t as f64 * DT / DAY_S;
    eprintln!("frondgrazer maturation: {days:.3} world days (plan ~1.5)");
    assert_eq!(fauna.ledger.matured_by_founder[f.index()], 1, "the maturation is booked");
}

// ------------------------------------------------------------------ the books

/// The books close over one world day for organic matter, mineral and energy, with food
/// in stomachs, digestion, regurgitation and a rider's take all running: what was swallowed
/// is what was digested, what left in stomachs and what is in them now, in every
/// currency, and the layer's residual is closed.
#[test]
#[ignore = "study: run by name (a whole world day; repair 3 item 9)"]
fn the_books_close_over_a_world_day() {
    books_case((DAY_S / DT) as u64);
}

/// [`the_books_close_over_a_world_day`]'s scene for 300 ticks: eating, digestion,
/// regurgitation and a rider's take all run from the first ticks.
#[test]
fn the_books_close_with_every_gut_flow_running() {
    books_case(300);
}

fn books_case(ticks: u64) {
    let world = clock_off(12);
    let mut flora = Flora::new(FloraConfig::default());
    carrion_everywhere(&world, &mut flora, 5.0);
    let mut c = carrion_config(Founder::Browser);
    c.founders[Founder::Chorister.index()].active = Activity::Always;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    // An eating grazer, carrying a rider.
    let g = introduce(&mut fauna, &world, Founder::Browser, StartingStores { body: 1.0, reserve: 0.5 });
    place(&mut fauna, g, 6.0, 1.5, EAST);
    fauna.controllers.set(g, stand_and_feed());
    let r = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores { body: 0.1, reserve: 0.5 });
    {
        let h = animal(&fauna, g);
        let a = animal_mut(&mut fauna, r);
        a.attached = Some(Attachment { host: g });
        a.site = h.site;
        a.pose = h.pose;
    }
    // A chorister adult at its den with a full stomach, and a hungry pup there.
    let mut p = fauna.config.founder(Founder::Chorister).pack.expect("a pack");
    p.pack_size = 1;
    fauna.config.founders[Founder::Chorister.index()].pack = Some(p);
    let den = Site { x: 32, y: SURFACE_Y, z: 6 };
    assert!(fauna.apply(
        &world,
        Command::FoundPack {
            den,
            founder: Founder::Chorister,
            stores: StartingStores::FULL,
            heading_rad: EAST,
        }
    ));
    let pack = fauna.packs()[0].id;
    let adult = fauna.animals.iter().find(|a| a.pack.is_some()).expect("the adult").id;
    place(&mut fauna, adult, 8.125, 1.625, EAST);
    fill_stomach(&mut fauna, adult, 0.9);
    let pup = introduce(&mut fauna, &world, Founder::Chorister, StartingStores { body: 0.3, reserve: 0.1 });
    animal_mut(&mut fauna, pup).pack = Some(PackMember {
        pack,
        pup: true,
        calling_ticks: 0,
    });
    place(&mut fauna, pup, 8.125, 2.125, EAST);
    fill_stomach(&mut fauna, pup, 0.0);
    let held0: Taken = fauna.animals.iter().fold(Taken::default(), |s, a| {
        let t = a.stomach.total();
        Taken {
            organic: s.organic + t.organic,
            mineral: s.mineral + t.mineral,
            energy: s.energy + t.energy,
        }
    });
    let l0 = fauna.ledger;
    for _ in 0..ticks {
        fauna.step_with(&world, &mut flora, 1);
    }
    let l = fauna.ledger;
    let held: Taken = fauna.animals.iter().fold(Taken::default(), |s, a| {
        let t = a.stomach.total();
        Taken {
            organic: s.organic + t.organic,
            mineral: s.mineral + t.mineral,
            energy: s.energy + t.energy,
        }
    });
    assert!(l.swallowed.organic > l0.swallowed.organic, "food went in");
    assert!(l.digested.organic > l0.digested.organic, "and was digested");
    assert!(l.regurgitated.organic > 0.0, "and was regurgitated");
    assert!(l.parasite_taken_organic > 0.0, "and the rider took its share");
    for (what, held0, held, in_, dig, out) in [
        ("organic", held0.organic, held.organic, l.swallowed.organic - l0.swallowed.organic, l.digested.organic - l0.digested.organic, l.stomach_out.organic - l0.stomach_out.organic),
        ("mineral", held0.mineral, held.mineral, l.swallowed.mineral - l0.swallowed.mineral, l.digested.mineral - l0.digested.mineral, l.stomach_out.mineral - l0.stomach_out.mineral),
        ("energy", held0.energy, held.energy, l.swallowed.energy - l0.swallowed.energy, l.digested.energy - l0.digested.energy, l.stomach_out.energy - l0.stomach_out.energy),
    ] {
        let lhs = held0 + in_;
        let rhs = dig + out + held;
        assert!((lhs - rhs).abs() <= 1e-9 * lhs.abs().max(1e-9), "{what}: stomach in {lhs} ≠ out {rhs}");
    }
    assert_books(&fauna, "after the run", ticks);
}

/// A body dying with a full stomach hands its contents to the carrion with the rest of
/// it, and the books close.
#[test]
fn a_dead_bodys_stomach_goes_to_its_carrion() {
    let world = clock_off(13);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(carrion_config(Founder::Browser));
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    fill_stomach(&mut fauna, id, 0.8);
    // Old age, now: the stomach is full as it dies.
    animal_mut(&mut fauna, id).lifespan_ticks = 1;
    let deposited0 = fauna.ledger.deposited_organic_out;
    let held = animal(&fauna, id).stored_organic();
    let s = animal(&fauna, id).stomach.total();
    assert!(held > s.organic && s.organic > 0.0, "the stored total counts the stomach");
    fauna.step_with(&world, &mut flora, 1);
    assert!(fauna.view().animal(id).is_none(), "it died");
    // What the last tick's digestion took is in `digested`; the rest left with it.
    assert!(
        fauna.ledger.stomach_out.organic + fauna.ledger.digested.organic >= s.organic * (1.0 - 1e-12),
        "its stomach left with it"
    );
    assert!(
        fauna.ledger.deposited_organic_out - deposited0 >= held * (1.0 - 1e-3),
        "into the carrion, with the rest of it"
    );
    assert_books(&fauna, "a death with a full stomach", 1);
}

#[allow(dead_code)]
fn _clock_modes(_: DayMode) {}


// ================================================================== repair 3

/// Short sleep transitions (repair 3 item 9; the long phase runs are studies): a fed
/// frondgrazer at midnight and a fed littershredder at noon stay asleep for 300 ticks.
#[test]
fn a_fed_sleeper_stays_asleep() {
    for (f, hour) in [(Founder::Browser, 0.0), (Founder::Blind, 12.0)] {
        let world = at_hour(hour, 4);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(carrion_config(f));
        fauna.set_births_enabled(false);
        let id = introduce(&mut fauna, &world, f, StartingStores::FULL);
        fauna.controllers.set(id, stand_and_feed());
        for t in 0..300 {
            fauna.step_with(&world, &mut flora, 1);
            assert_eq!(animal(&fauna, id).state, State::Asleep, "{}: awake at tick {t}", f.name());
        }
        assert_eq!(fauna.ledger.emergency_wakes_by_founder[f.index()], 0);
    }
}

/// The emergency latch's transitions (short): a fed frondgrazer asleep at midnight wakes
/// on the tick its hunger reaches 0.9, stays awake while its hunger is between 0.6 and 0.9,
/// and sleeps again below 0.6 — two flips, one emergency wake.
#[test]
fn the_emergency_latch_wakes_holds_and_releases() {
    let f = Founder::Browser;
    let world = at_hour(0.0, 5);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(carrion_config(f));
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores::FULL);
    fauna.controllers.set(id, stand_and_feed());
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(animal(&fauna, id).state, State::Asleep);
    let flips0 = fauna.ledger.sleep_flips_by_founder[f.index()];
    set_reserve(&mut fauna, id, 0.05);
    fauna.step_with(&world, &mut flora, 1);
    let a = animal(&fauna, id);
    assert!(a.emergency && a.state != State::Asleep, "woken on the emergency");
    assert_eq!(fauna.ledger.emergency_wakes_by_founder[f.index()], 1);
    set_reserve(&mut fauna, id, 0.3);
    for _ in 0..20 {
        fauna.step_with(&world, &mut flora, 1);
        assert_ne!(animal(&fauna, id).state, State::Asleep, "the latch holds at hunger 0.7");
    }
    set_reserve(&mut fauna, id, 0.45);
    fauna.step_with(&world, &mut flora, 1);
    let a = animal(&fauna, id);
    assert!(!a.emergency && a.state == State::Asleep, "below 0.6 it sleeps again");
    assert_eq!(fauna.ledger.sleep_flips_by_founder[f.index()] - flips0, 2);
}

/// **Passage** (repair 3 item 1): raw food whose effective yield is zero (no mineral)
/// passes at `S·dt/τ` — no reserve gain, the undigested share to egesta, the unfunded
/// share respired — whether or not the reserve has room. In a mixed stomach with no room
/// the funded pool holds and the mineral-free one still passes; after a few τ the
/// capacity is free again. All three currencies close.
#[test]
fn mineral_free_food_passes_and_frees_the_stomach() {
    let f = Founder::Browser;
    let world = clock_off(20);
    let mut flora = Flora::new(FloraConfig::default());
    let mut config = carrion_config(f);
    let p = &mut config.founders[f.index()];
    p.core.maintenance_per_s = 0.0;
    p.motor_respiration_per_s = 0.0;
    p.gut.digestion_s[Food::Nectar.index()] = 2.0;
    let tau = 2.0;
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    for (reserve, mixed) in [(1.0, false), (0.5, false), (1.0, true)] {
        let id = introduce(&mut fauna, &world, f, StartingStores { body: 1.0, reserve });
        fill_stomach(&mut fauna, id, 0.0);
        let k = capacity(&fauna, id);
        {
            let a = animal_mut(&mut fauna, id);
            a.stomach.pools[Food::Nectar.index()] = Taken {
                organic: 0.5 * k,
                mineral: 0.0,
                energy: 2.0 * 0.5 * k,
            };
            if mixed {
                a.stomach.pools[Food::Carrion.index()] = Taken {
                    organic: 0.3 * k,
                    mineral: CARRION_MINERAL * 0.3 * k,
                    energy: CARRION_ENERGY * 0.3 * k,
                };
            }
        }
        let fed = animal(&fauna, id).stomach.total();
        fauna.ledger.introduced_organic_in += fed.organic;
        fauna.ledger.introduced_mineral_in += fed.mineral;
        fauna.ledger.introduced_energy_in += fed.energy;
        let a0 = animal(&fauna, id);
        let (dig0, resp0) = (fauna.ledger.digested.organic, fauna.ledger.respired_digestion_out);
        fauna.step_with(&world, &mut flora, 1);
        let a1 = animal(&fauna, id);
        let d = 0.5 * k * DT / tau;
        let nectar = a1.stomach.pools[Food::Nectar.index()].organic;
        assert!((nectar - (0.5 * k - d)).abs() <= 1e-12 * k, "it passes at S dt / τ: {nectar}");
        assert!((a1.reserve - a0.reserve).abs() <= 1e-15, "no reserve gain");
        assert!((a1.gut.organic - a0.gut.organic - 0.5 * d).abs() <= 1e-12 * d, "half to egesta");
        assert!(
            (fauna.ledger.respired_digestion_out - resp0 - 0.5 * d).abs() <= 1e-12 * d,
            "the unfunded yield share respired"
        );
        if mixed {
            assert_eq!(
                a1.stomach.pools[Food::Carrion.index()].organic,
                0.3 * k,
                "the funded pool holds with no room"
            );
            assert!((fauna.ledger.digested.organic - dig0 - d).abs() <= 1e-12 * d);
        }
        for _ in 0..(10.0 * tau / DT) as u64 {
            fauna.step_with(&world, &mut flora, 1);
        }
        let s = animal(&fauna, id).stomach.pools[Food::Nectar.index()].organic;
        assert!(s < 1e-3 * k, "after ten τ the mineral-free pool is all but gone: {s}");
        assert_books(&fauna, "passage", 400);
        let _ = fauna.apply(&world, Command::Remove { x: 8, z: 6 });
    }
}

/// **A two-pool digestion, by the numbers** (Astra's finding 8): carrion (mineral-rich,
/// `y_eff` 0.5, τ 60 s) and foliage (mineral-poor, `y_eff` 0.2, τ 120 s) in one stomach,
/// with reserve room for only 0.3 of what their yields would place this tick. One tick
/// moves exactly the scaled raw shares out of the pools, places exactly the room, egests
/// the undigested halves, respires only the foliage's unfunded share (no surplus), keeps
/// the tissue's mineral and egests the rest, and splits the energy in proportion.
#[test]
fn a_two_pool_digestion_by_the_numbers() {
    let f = Founder::Browser;
    let world = clock_off(21);
    let mut flora = Flora::new(FloraConfig::default());
    let mut config = carrion_config(f);
    let p = &mut config.founders[f.index()];
    p.core.maintenance_per_s = 0.0;
    p.motor_respiration_per_s = 0.0;
    p.yield_by_food[Food::Foliage.index()] = 0.5;
    p.gut.digestion_s[Food::Carrion.index()] = 60.0;
    p.gut.digestion_s[Food::Foliage.index()] = 120.0;
    let n = p.core.n_tissue;
    assert_eq!(n, 0.05);
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores::FULL);
    fill_stomach(&mut fauna, id, 0.0);
    let (oc, of) = (0.04, 0.03);
    let (mc, mf) = (0.1 * oc, 0.01 * of);
    let (ec, ef) = (2.0 * oc, 1.5 * of);
    let dc = oc * DT / 60.0;
    let df = of * DT / 120.0;
    let yields = 0.5 * dc + 0.2 * df;
    let room = 0.3 * yields;
    {
        let a = animal_mut(&mut fauna, id);
        a.stomach.pools[Food::Carrion.index()] = Taken { organic: oc, mineral: mc, energy: ec };
        a.stomach.pools[Food::Foliage.index()] = Taken { organic: of, mineral: mf, energy: ef };
    }
    fauna.ledger.introduced_organic_in += oc + of;
    fauna.ledger.introduced_mineral_in += mc + mf;
    fauna.ledger.introduced_energy_in += ec + ef;
    let cap = fauna.config.founder(f).core.reserve_of(animal(&fauna, id).body);
    set_reserve(&mut fauna, id, (cap - room) / cap);
    let a0 = animal(&fauna, id);
    let l0 = fauna.ledger;
    fauna.step_with(&world, &mut flora, 1);
    let a1 = animal(&fauna, id);
    let l1 = fauna.ledger;
    let (tc, tf) = (0.3 * dc, 0.3 * df);
    let close = |got: f64, want: f64, what: &str| {
        assert!((got - want).abs() <= 1e-9 * want.abs().max(1e-18), "{what}: {got} against {want}");
    };
    close(oc - a1.stomach.pools[Food::Carrion.index()].organic, tc, "carrion out");
    close(of - a1.stomach.pools[Food::Foliage.index()].organic, tf, "foliage out");
    close(l1.digested.organic - l0.digested.organic, tc + tf, "digested");
    close(a1.reserve - a0.reserve, room, "the reserve took exactly its room");
    close(a1.gut.organic - a0.gut.organic, 0.5 * (tc + tf), "the undigested halves egested");
    close(
        l1.respired_digestion_out - l0.respired_digestion_out,
        0.3 * tf,
        "only foliage's unfunded share respired; no surplus",
    );
    let kept_mineral = n * room;
    close(a1.mineral - a0.mineral, kept_mineral, "tissue mineral kept");
    close(
        a1.gut.mineral - a0.gut.mineral,
        (mc / oc) * tc + (mf / of) * tf - kept_mineral,
        "the rest of the mineral egested",
    );
    let placed_c = 0.5 * tc;
    let placed_f = room - placed_c;
    close(a1.energy - a0.energy, 2.0 * placed_c + 1.5 * placed_f, "energy kept at food density");
    close(l1.heat_out - l0.heat_out, 1.5 * 0.3 * tf, "the unfunded share's energy as heat");
    assert_books(&fauna, "two pools", 1);
}

/// **The refill gap** (repair 3 item 3): `s_start` is capped at `full_tolerance −
/// refill_gap` (0.98 − 0.1), so a starving body just declared full (S ≈ 0.95 K) does not
/// want food again until its stomach falls to 0.88 K.
#[test]
fn a_starving_body_just_full_waits_for_the_refill_gap() {
    let f = Founder::Browser;
    let c = FaunaConfig::default();
    let gut = c.founder(f).gut;
    assert_eq!(gut.refill_gap, 0.1);
    assert!((gut.s_start(1.0) - 0.88).abs() < 1e-12, "{}", gut.s_start(1.0));
    assert!((gut.s_start(0.95) - 0.88).abs() < 1e-12);
    assert!((gut.s_start(0.5) - gut.s0).abs() < 1e-12);
    let world = clock_off(22);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(carrion_config(f));
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores { body: 1.0, reserve: 0.0 });
    fill_stomach(&mut fauna, id, 0.95);
    fauna.step_with(&world, &mut flora, 1);
    assert!(!animal(&fauna, id).eating, "at 0.95 K a starving body does not restart");
    fill_stomach(&mut fauna, id, 0.85);
    fauna.step_with(&world, &mut flora, 1);
    assert!(animal(&fauna, id).eating, "at 0.85 K it does");
}

/// **Safety** (repair 3 item 4): an attached rider and a falling body are awake in
/// their off phase.
#[test]
fn attached_and_falling_bodies_stay_awake() {
    // A nocturnal lanternjaw riding a grazer at noon.
    let world = at_hour(12.0, 23);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    fauna.set_births_enabled(false);
    let host = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    let rider = introduce(&mut fauna, &world, Founder::Lanternjaw, StartingStores { body: 0.2, reserve: 1.0 });
    animal_mut(&mut fauna, rider).attached = Some(Attachment { host });
    for _ in 0..10 {
        fauna.step_with(&world, &mut flora, 1);
        assert_ne!(animal(&fauna, rider).state, State::Asleep, "a rider is awake");
    }
    // A diurnal grazer falling at midnight.
    let world = at_hour(0.0, 24);
    let mut fauna = Fauna::new(FaunaConfig::default());
    fauna.set_births_enabled(false);
    let g = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    animal_mut(&mut fauna, g).mobility.falling = true;
    fauna.step_with(&world, &mut flora, 1);
    assert_ne!(animal(&fauna, g).state, State::Asleep, "a falling body is awake");
}

/// A pack of `n` adults under their heuristic on the clock-off plain, awake, and a
/// grazer standing at `prey`; the adults at `at`, each `hungry` or fed.
fn pack_scene(at: &[(f64, f64, bool)], prey: (f64, f64)) -> (World, Flora, Fauna, Vec<u64>, u64) {
    let world = clock_off(25);
    let flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    c.founders[Founder::Chorister.index()].active = Activity::Always;
    let mut pk = c.founders[Founder::Chorister.index()].pack.expect("a pack");
    pk.pack_size = at.len();
    c.founders[Founder::Chorister.index()].pack = Some(pk);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    assert!(fauna.apply(
        &world,
        Command::FoundPack {
            den: Site { x: 8, y: SURFACE_Y, z: 6 },
            founder: Founder::Chorister,
            stores: StartingStores::FULL,
            heading_rad: EAST,
        }
    ));
    let adults: Vec<u64> = fauna.animals.iter().filter(|a| a.pack.is_some()).map(|a| a.id).collect();
    for (&id, &(x, z, hungry)) in adults.iter().zip(at) {
        place(&mut fauna, id, x, z, EAST);
        let teacher = crate::controller::heuristic(Founder::Chorister, fauna.config.teachers);
        fauna.controllers.set(id, teacher);
        fill_stomach(&mut fauna, id, if hungry { 0.0 } else { 0.99 });
    }
    let pid = introduce(&mut fauna, &world, Founder::Browser, StartingStores { body: 0.5, reserve: 1.0 });
    place(&mut fauna, pid, prey.0, prey.1, EAST);
    let _off = drive(&mut fauna, pid, 0.0, false);
    (world, flora, fauna, adults, pid)
}

/// **One participant set** (repair 3 item 5): the nearest adult to the prey is fed; the
/// two that want food are the participants. The driver is the nearest participant, which
/// heads straight at the prey, and the body window counts the participants only.
#[test]
fn only_participants_drive_flank_and_count_in_the_window() {
    // Fed adult nearest the prey; hungry A on the prey's line behind it; hungry B beside A.
    let (world, mut flora, mut fauna, adults, _prey) =
        pack_scene(&[(2.0, 1.5, true), (2.9, 1.5, false), (2.0, 2.3, true)], (3.6, 1.5));
    let (a, fed, b) = (adults[0], adults[1], adults[2]);
    let pack = fauna.packs()[0].id;
    let set = step_until(&mut fauna, &world, &mut flora, 40, |f| f.pack(pack).is_some_and(|p| p.target.is_some()));
    assert!(set.is_some(), "the pack takes the grazer as its target");
    // The next sampling of A: it drives (straight at the prey, no turn), not a flank.
    for _ in 0..5 {
        fauna.step_with(&world, &mut flora, 1);
    }
    let held = animal(&fauna, a).founder_state.held;
    assert!(held.turn.abs() < 0.2 && held.forward > 0.5, "A drives straight at the prey: {held:?}");
    let ring = f64::from(world.view().config.width) * world.view().config.voxel_m;
    let i = fauna.animals.iter().position(|x| x.id == a).expect("A");
    let window = crate::pack::window_body(&fauna, i, ring);
    let want = animal(&fauna, a).body + animal(&fauna, b).body;
    assert!((window - want).abs() < 1e-9, "the window is the participants' {want}, not {window}");
    let _ = fed;
}

/// **A usable feast first** (repair 3 item 5): a hungry pack at its carcass, with live
/// prey in sight, starts no new chase while the carcass has carrion.
#[test]
fn a_hungry_pack_at_its_feast_starts_no_new_chase() {
    let (world, mut flora, mut fauna, adults, _prey) =
        pack_scene(&[(2.0, 1.5, true), (2.0, 2.3, true)], (3.6, 1.5));
    let pack = fauna.packs()[0].id;
    let feast = Site { x: 8, y: SURFACE_Y, z: 6 };
    assert!(flora.deposit(
        feast,
        Deposit { kind: DepositKind::Carrion, organic: 2.0, mineral: 0.1 * 2.0, energy: 4.0 }
    ));
    fauna.pack_mut(pack).expect("the pack").feast = Some(feast);
    for _ in 0..60 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert_eq!(fauna.ledger.pack_chases, 0, "no new chase while the feast lasts");
    let _ = adults;
}

/// **The pack eats its kill** (`design/handoffs/bellwing-finding-2026-10-03.md` item 2):
/// two hungry adults a few body lengths from their feast, one behind the other so the rear
/// one sees its packmate close ahead (its own heuristic then stands with `feed` held, as
/// it does stalking a body), both walk to the carcass and eat there, and the pack's
/// feast books what they ate.
#[test]
fn a_hungry_pack_walks_to_its_feast_and_eats_it() {
    let (world, mut flora, mut fauna, adults, _prey) =
        pack_scene(&[(1.2, 1.6, true), (1.9, 1.6, true)], (9.0, 2.6));
    let pack = fauna.packs()[0].id;
    // Centre (3.375, 1.625): 2.2 m and 1.5 m ahead of the two.
    let feast = Site { x: 13, y: SURFACE_Y, z: 6 };
    assert!(flora.deposit(
        feast,
        Deposit { kind: DepositKind::Carrion, organic: 2.0, mineral: 0.1 * 2.0, energy: 4.0 }
    ));
    fauna.pack_mut(pack).expect("the pack").feast = Some(feast);
    let before: Vec<f64> = adults.iter().map(|&id| animal(&fauna, id).stomach.total().organic).collect();
    for _ in 0..400 {
        fauna.step_with(&world, &mut flora, 1);
    }
    for (&id, s0) in adults.iter().zip(before) {
        let s = animal(&fauna, id).stomach.total().organic;
        assert!(s > s0 + 1e-9, "adult {id} ate at the feast: stomach {s0} -> {s}");
    }
    let eaten = fauna.pack(pack).expect("the pack").feast_eaten;
    assert!(eaten > 0.0, "the feast books what the pack ate: {eaten}");
}

/// **The reserve fallback is the old transfer** (repair 3 item 6): a pup whose stomach is
/// full (its eating latch off) but whose reserve is low still receives reserve from an
/// adult with an empty stomach and a surplus reserve.
#[test]
fn the_reserve_fallback_does_not_need_the_pups_latch() {
    let world = clock_off(26);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    c.founders[Founder::Chorister.index()].active = Activity::Always;
    let mut pk = c.founders[Founder::Chorister.index()].pack.expect("a pack");
    pk.pack_size = 1;
    c.founders[Founder::Chorister.index()].pack = Some(pk);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    assert!(fauna.apply(
        &world,
        Command::FoundPack {
            den: Site { x: 8, y: SURFACE_Y, z: 6 },
            founder: Founder::Chorister,
            stores: StartingStores { body: 1.0, reserve: 0.9 },
            heading_rad: EAST,
        }
    ));
    let pack = fauna.packs()[0].id;
    let adult = fauna.animals[0].id;
    place(&mut fauna, adult, 2.125, 1.625, EAST);
    fill_stomach(&mut fauna, adult, 0.0);
    let pup = introduce(&mut fauna, &world, Founder::Chorister, StartingStores { body: 0.3, reserve: 0.0 });
    animal_mut(&mut fauna, pup).pack = Some(PackMember { pack, pup: true, calling_ticks: 0 });
    place(&mut fauna, pup, 2.125, 2.125, EAST);
    fill_stomach(&mut fauna, pup, 0.99);
    assert!(!animal(&fauna, pup).eating);
    fauna.step_with(&world, &mut flora, 1);
    assert!(fauna.ledger.provision_given_organic > 0.0, "the fallback fed the full-stomached pup");
    assert_eq!(fauna.ledger.regurgitated.organic, 0.0);
    assert_books(&fauna, "fallback", 1);
}

/// **Removal is export** (repair 3 item 7): a removed body leaves whole, stomach included,
/// booked `removed_*`; nothing is deposited.
#[test]
fn a_removed_body_exports_its_stomach() {
    let world = clock_off(27);
    let mut fauna = Fauna::new(carrion_config(Founder::Browser));
    let id = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    fill_stomach(&mut fauna, id, 0.7);
    let a = animal(&fauna, id);
    let (removed0, deposited0) = (fauna.ledger.removed_organic_out, fauna.ledger.deposited_organic_out);
    assert!(fauna.apply(&world, Command::Remove { x: i64::from(a.site.x), z: a.site.z }));
    assert!(fauna.view().animal(id).is_none());
    assert!((fauna.ledger.removed_organic_out - removed0 - a.stored_organic()).abs() <= 1e-12);
    assert_eq!(fauna.ledger.deposited_organic_out, deposited0, "nothing deposited");
    assert!((fauna.ledger.stomach_out.organic - a.stomach.organic()).abs() <= 1e-15);
    assert_closes(&fauna, "removal");
}

/// **Fed perching** (repair 3 item 8): a bellwing under its own heuristic, aloft over a
/// bare plain with a full stomach (its eating latch off), comes down and rests perched.
#[test]
fn a_fed_bellwing_perches_and_rests() {
    let world = clock_off(28);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    fauna.set_births_enabled(false);
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft {
            pose: crate::Pose { x: 3.0, z: 1.5, heading_rad: EAST, y_m: SURFACE_M + 1.0 },
            founder: Founder::Bellwing,
            stores: StartingStores::FULL,
        }
    ));
    let id = fauna.animals.last().expect("aloft").id;
    fill_stomach(&mut fauna, id, 0.99);
    let teacher = crate::controller::heuristic(Founder::Bellwing, fauna.config.teachers);
    fauna.controllers.set(id, teacher);
    let perched = step_until(&mut fauna, &world, &mut flora, 300, |f| {
        animal(f, id).mobility.mode == crate::MobilityMode::Perched
    });
    assert!(perched.is_some(), "a fed bellwing comes down to perch");
    for _ in 0..60 {
        fauna.step_with(&world, &mut flora, 1);
        let a = animal(&fauna, id);
        assert_eq!(a.mobility.mode, crate::MobilityMode::Perched, "and stays perched while fed");
    }
}

/// **A non-empty snapshot round trip, and validation on load** (Astra's finding 9): mixed
/// stomach pools, both latches set and an open growth bout come back exactly; a stomach
/// pool that is not a finite nonnegative stock is refused.
#[test]
fn a_snapshot_keeps_a_mixed_stomach_and_refuses_a_bad_one() {
    let world = clock_off(29);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = introduce(&mut fauna, &world, Founder::Browser, StartingStores::FULL);
    {
        let a = animal_mut(&mut fauna, id);
        a.stomach.pools[Food::Foliage.index()] = Taken { organic: 0.01, mineral: 2e-4, energy: 0.02 };
        a.stomach.pools[Food::Carrion.index()] = Taken { organic: 0.02, mineral: 1e-3, energy: 0.05 };
        a.eating = true;
        a.emergency = true;
        a.bout = Some(crate::Bout { start_tick: 7, kind: BoutKind::Growth });
    }
    let loaded = Fauna::load(&fauna.save()).expect("round trip");
    assert_eq!(loaded.animals, fauna.animals);
    let b = loaded.view().animal(id).copied().expect("back");
    assert!(b.eating && b.emergency && b.bout.is_some_and(|x| x.kind == BoutKind::Growth));
    for bad in [f64::NAN, -1e-3, f64::INFINITY] {
        let mut broken = fauna.clone();
        animal_mut(&mut broken, id).stomach.pools[Food::Carrion.index()].mineral = bad;
        assert!(Fauna::load(&broken.save()).is_err(), "a stomach mineral of {bad} is refused");
    }
}
