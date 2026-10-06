//! **The detritus web, package D1** (`design/lanternjaw-detritus-web-proposal-2026-10-05.md`
//! §3.1, Astra's review `design/7_Research/astra-detritus-web-review-2026-10-05.md`),
//! written before the mechanisms. A child of `step`, so a case can reach the controller
//! table and the layer's own fields.
//!
//! Every case checks a mechanism on hand-built inputs **against the config it sets**: no
//! default, ratio or population is asserted.
//!
//! - A: a blind forager tasting food holds still and feeds; it walks on from an exhausted
//!   patch; a refused motor's escape and the prey flee rule still win.
//! - B: a dark-active body is awake where its own face is dark and asleep where it is lit.
//! - C: eligibility is one affordable egg; the clutch is what the surplus pays for, capped,
//!   and none when not one egg is affordable.
//! - D: the nest-site brake refuses a clutch above its crowd limit and allows one at it.
//! - The body ceiling (a compute guard) is not reached in a normal fixture and refuses a
//!   clutch when forced.
//! - Ledgers conserve through each.

use super::predation_fixture::*;
use crate::controller::{BlindForager, Controller, Response};
use crate::{Activity, Fauna, FaunaConfig, Founder, Reproduction, StartingStores, State, TeacherConfig, DT};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Material, World};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site};

const SHREDDER: Founder = Founder::Blind;

/// A blind observation: a free motor, the mouth tasting `cue`, nothing else.
fn tasting(cue: f64) -> Vec<f64> {
    let m = SHREDDER.manifest();
    let mut o = vec![0.0; m.inputs()];
    o[7] = 1.0;
    let t = m.modules.iter().find(|m| m.name == "Taste(1)").expect("taste");
    o[t.offset] = cue;
    o[t.offset + 2] = 1.0;
    o
}

fn act(c: &mut dyn Controller, o: &[f64]) -> crate::Actions {
    match c.drive(o) {
        Response::Bounded(a) => a,
        Response::Logits(_) => unreachable!("the heuristic answers bounded actions"),
    }
}

// ------------------------------------------------------------------ A: stay on food

/// A forager tasting food over the gate holds the configured stay effort and feeds; on
/// crumbs under the gate (an exhausted patch) it walks on and turns back toward where the
/// food was; the stay effort is the config's, whatever it is.
#[test]
fn a_forager_tasting_food_stays_and_walks_on_from_an_exhausted_patch() {
    for stay in [0.0, 0.3] {
        let cfg = TeacherConfig {
            feed_gate: 0.02,
            forage_stay_forward: stay,
            forage_turn_back_samples: 2,
            ..TeacherConfig::default()
        };
        let mut c = BlindForager::with_config(cfg);
        for _ in 0..3 {
            let a = act(&mut c, &tasting(0.05));
            assert_eq!(a.feed, 1.0, "it feeds on food it tastes");
            assert_eq!(a.forward, stay, "and holds the configured stay effort");
        }
        // The face is eaten down below the gate: it leaves, turning back first.
        let a = act(&mut c, &tasting(0.01));
        assert_eq!(a.feed, 0.0);
        assert!(a.forward > stay, "an exhausted patch is left: forward {}", a.forward);
        assert!(a.turn != 0.0, "with a turn back toward the food");
    }
    // The old forager (stay at full walk) crosses food at full walk.
    let mut old = BlindForager::with_config(TeacherConfig {
        forage_stay_forward: 1.0,
        ..TeacherConfig::default()
    });
    assert_eq!(act(&mut old, &tasting(0.05)).forward, 1.0);
}

/// A refused motor's escape comes before staying on food.
#[test]
fn a_refused_motor_escapes_even_on_food() {
    let cfg = TeacherConfig {
        forage_stay_forward: 0.0,
        ..TeacherConfig::default()
    };
    let mut c = BlindForager::with_config(cfg);
    let mut o = tasting(0.05);
    o[7] = 0.0;
    let a = act(&mut c, &o);
    assert!(a.turn != 0.0, "the escape turns away");
    let a = act(&mut c, &tasting(0.05));
    assert_eq!(a.forward, cfg.escape_creep, "and creeps through its samples, not staying");
}

/// Litter on a site, at the composition the arenas deposit it at.
fn litter(flora: &mut Flora, site: Site, organic: f64) {
    assert!(flora.deposit(
        site,
        Deposit {
            kind: DepositKind::Litter,
            organic,
            mineral: 0.02 * organic,
            energy: 2.0 * organic,
        },
    ));
}

/// A shredder on a litter pile holds still to feed; a walking lanternjaw within its tremor
/// range and it flees at full walk, not feeding — its S1 rule, its senses off (S2's freeze
/// has its own case in `ground_response_tests`).
#[test]
fn a_forager_on_food_still_flees_a_walking_hunter() {
    let run = |with_hunter: bool| -> (Fauna, u64) {
        let world = clock_off(31);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = hunting_config();
        c.founders[SHREDDER.index()].senses.live = false;
        let mut fauna = Fauna::new(c);
        let s = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: 0.3 });
        place(&mut fauna, s, 4.1, 1.6, EAST);
        let site = animal(&fauna, s).site;
        for dx in 0..3 {
            litter(&mut flora, Site { x: site.x + dx, ..site }, 2.0);
        }
        let teachers = fauna.config.teachers;
        fauna.controllers.set(s, crate::controller::heuristic(SHREDDER, teachers));
        if with_hunter {
            let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, HUNTING);
            place(&mut fauna, lj, 3.6, 2.4, EAST);
            drive(&mut fauna, lj, 1.0, false);
        } else {
            // A hunter in the world, far off and still: the flee rule is armed, nothing
            // to flee.
            let lj = introduce(&mut fauna, &world, Founder::Lanternjaw, HUNTING);
            place(&mut fauna, lj, 10.0, 0.5, EAST);
            drive(&mut fauna, lj, 0.0, false);
        }
        for _ in 0..12 {
            fauna.step_with(&world, &mut flora, 1);
        }
        assert_closes(&fauna, "after the stay-on-food case");
        (fauna, s)
    };
    let (fauna, s) = run(false);
    let held = animal(&fauna, s).founder_state.held;
    assert_eq!(held.feed, 1.0, "on litter it feeds");
    assert_eq!(held.forward, fauna.config.teachers.forage_stay_forward, "holding the stay effort");
    assert!(fauna.ledger.feed_still_ticks_by_founder[SHREDDER.index()] > 0, "the readout counts it");

    let (fauna, s) = run(true);
    let held = animal(&fauna, s).founder_state.held;
    let felt = fauna.ledger.feed_still_ticks_by_founder[SHREDDER.index()];
    assert_eq!(held.forward, 1.0, "a walking hunter in range: it flees at full walk (stayed {felt} samples)");
    assert_eq!(held.feed, 0.0, "and does not feed");
}

// ------------------------------------------------------------------ B: dark-active

/// The plain held at `hour` with a rock roof one voxel over the standing layer, across
/// columns `x = 0..20` and every `z`: the faces under it see no sky.
fn roofed(hour: f64) -> World {
    let mut w = plain(
        DayClock {
            start_hour: hour,
            ..DayClock::default()
        },
        33,
    );
    let depth = w.config().depth;
    for z in 0..depth {
        for x in 0..20 {
            w.apply(WorldCommand::SetMaterial {
                x,
                y: SURFACE_Y + 2,
                z,
                material: Material::Rock,
            });
        }
    }
    w
}

/// A dark-active body is awake by day under the roof and asleep in the open, by the
/// config's own day line against its face's sky × daylight; at night both are awake. A
/// nocturnal body sleeps by day under the roof too.
#[test]
fn dark_active_timing_follows_the_darkness_of_the_face() {
    let state_of = |hour: f64, active: Activity, x_m: f64| -> (State, f64, f64, u64) {
        let world = roofed(hour);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = FaunaConfig::default();
        c.founders[SHREDDER.index()].active = active;
        let mut fauna = Fauna::new(c);
        let s = introduce(&mut fauna, &world, SHREDDER, StartingStores::FULL);
        place(&mut fauna, s, x_m, 1.6, EAST);
        drive(&mut fauna, s, 0.0, false);
        fauna.step_with(&world, &mut flora, 1);
        let a = animal(&fauna, s);
        let v = world.view();
        let sky = v.sky_visibility((x_m / VOXEL_M).floor() as i64, a.sense_layer(VOXEL_M), a.site.z);
        let daylight = world.life_daylight();
        (a.state, sky, daylight, fauna.ledger.dark_awake_ticks_by_founder[SHREDDER.index()])
    };
    let life = FaunaConfig::default().life;
    let (under, open) = (2.5, 8.0);
    // Noon: lit in the open, dark under the roof.
    let (st, sky, day, dark) = state_of(12.0, Activity::DarkActive, under);
    assert!(sky * day < life.day_line, "fixture: the roofed face is dark ({sky} x {day})");
    assert_ne!(st, State::Asleep, "dark-active under the roof at noon is awake");
    assert!(dark > 0, "and the readout books an awake tick owed to the dark");
    let (st, sky, day, _) = state_of(12.0, Activity::DarkActive, open);
    assert!(sky * day >= life.day_line, "fixture: the open face is lit ({sky} x {day})");
    assert_eq!(st, State::Asleep, "dark-active in the open at noon sleeps");
    let (st, _, _, _) = state_of(12.0, Activity::Nocturnal, under);
    assert_eq!(st, State::Asleep, "a nocturnal body sleeps at noon even under the roof");
    // Midnight: dark everywhere.
    let (st, _, day, _) = state_of(0.0, Activity::DarkActive, open);
    assert!(day < life.day_line, "fixture: midnight is dark ({day})");
    assert_ne!(st, State::Asleep, "dark-active in the open at midnight is awake");
}

// ------------------------------------------------- C: clutch from surplus, eligibility

/// The test's own egg rule: a one-tick hold, a long interval, eggs of `egg`, floor
/// `floor`, at most `max` (0: a fixed clutch of `fixed`).
fn rule(egg: f64, floor: f64, max: u32, fixed: u32) -> Reproduction {
    Reproduction {
        surplus_floor: floor,
        surplus_hold_s: DT,
        birth_interval_s: 1000.0 * DT,
        clutch_size: fixed,
        clutch_max: max,
        egg_organic: egg,
        incubation_s: 1000.0 * DT,
        nest_radius_m: 0.0,
        ..Reproduction::EGGS_PLACEHOLDER
    }
}

#[test]
fn the_clutch_is_what_the_surplus_pays_for_and_none_when_unaffordable() {
    let (egg, floor) = (0.1, 0.05);
    let r = rule(egg, floor, 4, 1);
    for (reserve, want) in [(0.0, 0), (floor + 0.99 * egg, 0), (floor + 1.01 * egg, 1), (floor + 2.5 * egg, 2), (floor + 9.0 * egg, 4)] {
        assert_eq!(r.clutch_for(reserve), want, "reserve {reserve}");
    }
    // A fixed clutch is its size whatever the reserve.
    assert_eq!(rule(egg, floor, 0, 3).clutch_for(0.0), 3);
}

/// A shredder config whose egg rule is `r`, with a reserve cap of 1 so a whole adult's
/// reserve is room for several eggs, and an empty stomach so digestion moves nothing.
fn egg_config(r: Reproduction) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for p in c.founders.iter_mut() {
        p.gut.s0 = 0.0;
    }
    let core = &mut c.founders[SHREDDER.index()].core;
    core.reserve_cap = 1.0;
    core.reproduction = r;
    c
}

/// One adult on litter under `config`, with a reserve of `reserve_fraction` of its cap;
/// one tick. The layer, the world, the plants and the body's id.
fn one_layer(config: FaunaConfig, reserve_fraction: f64) -> (Fauna, World, Flora, u64) {
    let world = clock_off(35);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config);
    let s = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: reserve_fraction });
    drive(&mut fauna, s, 0.0, false);
    litter(&mut flora, animal(&fauna, s).site, 1.0);
    (fauna, world, flora, s)
}

/// **Eligibility is one affordable egg**: with 2.5 eggs above the floor, a clutch from
/// surplus is eligible and lays 2 (the reserve pays for), where a fixed clutch of 3 is not
/// eligible at all; a body short of one egg above the floor is not eligible and lays
/// nothing.
#[test]
fn eligibility_is_one_affordable_egg_and_the_clutch_follows_the_surplus() {
    let core = FaunaConfig::default().founders[SHREDDER.index()].core;
    let full = core.body_max; // reserve cap 1 x body_max
    let egg = core.body_min;
    let floor = full - 2.5 * egg;
    assert!(floor > 0.0, "fixture: room for 2.5 eggs above a floor");

    let (mut fauna, world, mut flora, s) = one_layer(egg_config(rule(egg, floor, 4, 1)), 1.0);
    let before = animal(&fauna, s).reserve;
    fauna.step_with(&world, &mut flora, 1);
    let want = rule(egg, floor, 4, 1).clutch_for(before);
    assert_eq!(want, 2, "fixture: the reserve pays for two");
    assert_eq!(fauna.clutches.len(), 1, "a clutch from surplus is laid");
    assert_eq!(fauna.clutches[0].count, want, "of what the surplus pays for");
    assert!((fauna.clutches[0].organic - f64::from(want) * egg).abs() < 1e-12);
    assert_eq!(fauna.ledger.clutches_by_founder[SHREDDER.index()], 1);
    assert_closes(&fauna, "after a clutch from surplus");

    // The same reserve with a fixed clutch of 3: not eligible.
    let (mut fauna, world, mut flora, s) = one_layer(egg_config(rule(egg, floor, 0, 3)), 1.0);
    fauna.step_with(&world, &mut flora, 1);
    assert!(!animal(&fauna, s).reproduction.eligible_now, "a fixed 3 is not affordable");
    assert!(fauna.clutches.is_empty());

    // Under one egg above the floor: not eligible, nothing laid, nothing paid.
    let short = (floor + 0.5 * egg) / full;
    let (mut fauna, world, mut flora, s) = one_layer(egg_config(rule(egg, floor, 4, 1)), short);
    fauna.step_with(&world, &mut flora, 1);
    assert!(!animal(&fauna, s).reproduction.eligible_now);
    assert!(fauna.clutches.is_empty(), "no clutch when not one egg is affordable");
    assert_closes(&fauna, "after no clutch");
}

// ------------------------------------------------------------- D: nest-site brake

/// An adult ready to lay with `neighbours` of its lineage within the nest radius (hungry,
/// so none of them lays): the clutch is refused above the crowd limit and laid at it.
#[test]
fn the_nest_site_brake_refuses_a_crowded_clutch_and_allows_one_at_the_limit() {
    let core = FaunaConfig::default().founders[SHREDDER.index()].core;
    let (egg, limit, radius) = (core.body_min, 3u32, 1.0);
    let floor = core.body_max - 2.5 * egg;
    let run = |neighbours: u32| -> Fauna {
        let mut r = rule(egg, floor, 4, 1);
        r.nest_radius_m = radius;
        r.nest_crowd_max = limit;
        let (mut fauna, world, mut flora, s) = one_layer(egg_config(r), 1.0);
        let at = animal(&fauna, s).pose;
        for k in 0..neighbours {
            let n = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: 0.0 });
            let ang = f64::from(k) * 1.1;
            place(&mut fauna, n, at.x + 0.5 * radius * ang.cos(), at.z + 0.5 * radius * ang.sin(), EAST);
            drive(&mut fauna, n, 0.0, false);
        }
        // One more of the lineage outside the radius, which never counts.
        let far = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: 0.0 });
        place(&mut fauna, far, at.x + 3.0 * radius, at.z, EAST);
        drive(&mut fauna, far, 0.0, false);
        fauna.step_with(&world, &mut flora, 1);
        assert_closes(&fauna, "after the nest-site case");
        fauna
    };
    let crowded = run(limit + 1);
    assert!(crowded.clutches.is_empty(), "above the limit the clutch is refused");
    assert_eq!(crowded.ledger.nest_refusals_by_founder[SHREDDER.index()], 1);
    let at_limit = run(limit);
    assert_eq!(at_limit.clutches.len(), 1, "at the limit it lays");
    assert_eq!(at_limit.ledger.nest_refusals_by_founder[SHREDDER.index()], 0);
}

// --------------------------------------------------------- the body ceiling guard

/// The compute guard never acts in an ordinary fixture, and refuses (and books) a clutch
/// when the lineage holds the configured ceiling.
#[test]
fn the_body_ceiling_is_not_reached_normally_and_refuses_when_forced() {
    let core = FaunaConfig::default().founders[SHREDDER.index()].core;
    let egg = core.body_min;
    let floor = core.body_max - 2.5 * egg;

    let (mut fauna, world, mut flora, _) = one_layer(egg_config(rule(egg, floor, 4, 1)), 1.0);
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(fauna.clutches.len(), 1);
    assert_eq!(fauna.ledger.ceiling_refusals_by_founder.iter().sum::<u64>(), 0, "a normal fixture never meets the guard");

    let mut c = egg_config(rule(egg, floor, 4, 1));
    c.lineage_body_ceiling = 2;
    let (mut fauna, world, mut flora, _) = one_layer(c, 1.0);
    let other = introduce(&mut fauna, &world, SHREDDER, StartingStores { body: 1.0, reserve: 0.0 });
    place(&mut fauna, other, 9.0, 2.0, EAST);
    drive(&mut fauna, other, 0.0, false);
    fauna.step_with(&world, &mut flora, 1);
    assert!(fauna.clutches.is_empty(), "at the ceiling no clutch is laid");
    assert_eq!(fauna.ledger.ceiling_refusals_by_founder[SHREDDER.index()], 1, "and the activation is booked");
    assert_closes(&fauna, "after the guard refused");
}

// ------------------------------------------------------------ F: the film's refuge

/// A grazer cannot take a site's film below the configured refuge share of its capacity,
/// whatever it asks for; with no refuge it can strip the site.
#[test]
fn grazing_leaves_the_film_refuge() {
    for refuge in [0.0, 0.2] {
        let mut world = clock_off(37);
        let cell = world.config().voxel_volume();
        let depth = world.config().depth;
        for z in 0..depth {
            for x in 0..12 {
                world.apply(WorldCommand::AddWater { x, y: SURFACE_Y + 1, z, volume_m3: cell });
            }
        }
        let mut fc = FloraConfig::default();
        fc.voxel_m = VOXEL_M;
        fc.film_seed_per_m2 = 0.6 * fc.film_capacity_per_m2;
        fc.film_graze_refuge = refuge;
        let mut flora = Flora::new(fc.clone());
        assert!(flora.seed_film(&world) > 0, "fixture: wet faces");
        let site = Site { x: 4, y: SURFACE_Y, z: 4 };
        let before = flora.view().ground_at(site).expect("filmed").film;
        let taken = flora.take_film(site, 10.0 * before).map_or(0.0, |t| t.organic);
        let left = flora.view().ground_at(site).expect("filmed").film;
        let floor = refuge * fc.film_capacity_per_m2 * VOXEL_M * VOXEL_M;
        assert!((left - floor).abs() <= 1e-12, "refuge {refuge}: {left} left, the refuge is {floor}");
        assert!((before - taken - left).abs() <= 1e-12, "what was taken is what left the site");
        // Nothing more is taken from a site at its refuge.
        assert!(flora.take_film(site, 1.0).is_none_or(|t| t.organic == 0.0));
    }
}
