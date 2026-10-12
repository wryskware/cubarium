//! **TEST-PASS-4 (independent), MIMIC + MIMIC-2**: the seedporter's catch at exactly
//! `catch_ceiling_m` and exactly at its `catch_gain_min`; its switch off restoring the
//! fruit eater that hunts nothing (no predator class, no carrion); its foray at exactly
//! `catch_foray_m`, at exactly `catch_foray_min` bodies, for exactly its `catch_foray_s`
//! budget, and never while it flees a Veilhound; the ledgers of both layers closing
//! through a catch; the capgnawer's light approach at the edge of its near and far
//! budgets, and (with GLOWCAP) a capgnawer that gave up a lure grazing cropped mycelium;
//! and a postcard round trip of the new lineage fields.
//!
//! A child of `step` beside the predation files, sharing their fixture. Every case sets the
//! config it relies on; no shipped default, ratio or verdict is asserted.

use super::predation_fixture::*;
use crate::light::{self, BeaconKind};
use crate::{Actions, Activity, Command, Diet, Fauna, FaunaConfig, Food, Founder, HuntMode, Hunter, MobilityMode, Pose, StartingStores};
use cubarium_voxel::World;
use cubarium_voxel::weather::DayClock;
use cubarium_voxel_flora::{Command as FloraCommand, Deposit, DepositKind, FaceLayer, Flora, FloraConfig, Site, Species};

const SP: Founder = Founder::Seedporter;
const BW: Founder = Founder::Bellwing;
const LJ: Founder = Founder::Lanternjaw;
const CAP: Founder = Founder::Capgnawer;
const VH: Founder = Founder::Chorister;

fn noon(seed: u64) -> World {
    plain_of_height(DayClock { start_hour: 12.0, ..DayClock::default() }, seed, 24)
}

/// An explicit catch table and switches for the seedporter; the light rules for the lure.
fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.hunt.reachable_only = false;
    c.light.visit_s = 0.5;
    c.light.mark_s = 10.0;
    c.founders[BW.index()].active = Activity::Always;
    let mut prey = [false; Founder::COUNT];
    prey[BW.index()] = true;
    let sp = &mut c.founders[SP.index()];
    sp.hunter = Some(Hunter {
        prey,
        strike_power: 8.0,
        prey_max_ratio: 4.0,
        strike_reach_length_fraction: 0.8,
        jaw_band_height_fraction: 2.0,
        vertical_reach_m: 0.0,
        jaw_forward_length_fraction: 0.5,
        strike_warning_scale: 1.0,
        windup_s: 0.1,
        lunge_s: 0.15,
        recovery_s: 0.5,
        strike_cost_s: 0.1,
        lure_power: 0.0,
        prey_weight: [1.0; Founder::COUNT],
        prey_juveniles_only: [false; Founder::COUNT],
        prey_ashore_only: [false; Founder::COUNT],
        lunge_blocks: false,
    });
    sp.catch_bellwings = true;
    sp.catch_ceiling_m = 0.5;
    sp.catch_gain_min = 0.0;
    sp.catch_foray_m = 4.0;
    sp.catch_foray_min = 2.0;
    sp.catch_foray_s = 1.0;
    sp.active = Activity::Always;
    sp.diet = Diet::of(&[Food::Fruit, Food::Carrion]);
    c
}

fn bellwing(fauna: &mut Fauna, world: &World, x: f64, lift: f64) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft {
            pose: Pose { x, z: LANE_Z, heading_rad: WEST, y_m: SURFACE_M + lift },
            founder: BW,
            stores: StartingStores::FULL,
        }
    ));
    let id = fauna.animals.last().unwrap().id;
    drive(fauna, id, 0.0, false);
    id
}

fn seedporter(fauna: &mut Fauna, world: &World, x: f64) -> u64 {
    let id = introduce(fauna, world, SP, HUNTING);
    place(fauna, id, x, LANE_Z, EAST);
    let a = animal_mut(fauna, id);
    a.pose.y_m = SURFACE_M;
    a.eating = true;
    id
}

/// A grounded seedporter and a perched bellwing `lift` above its base, side by side.
fn pair(c: FaunaConfig, lift: f64) -> (Fauna, u64, u64) {
    let world = noon(601);
    let mut fauna = Fauna::new(c);
    let sp = seedporter(&mut fauna, &world, 3.0);
    let bw = bellwing(&mut fauna, &world, 3.5, lift);
    let b = animal_mut(&mut fauna, bw);
    b.mobility.mode = MobilityMode::Perched;
    b.mobility.falling = false;
    (fauna, sp, bw)
}

#[test]
fn a_bellwing_exactly_at_the_catch_ceiling_is_prey_and_a_hair_above_is_not() {
    let c = config();
    let ceiling = c.founder(SP).catch_ceiling_m;
    for (lift, prey) in [(ceiling, true), (ceiling + 1e-9, false), (0.0, true)] {
        let (fauna, sp, bw) = pair(c.clone(), lift);
        let (h, p) = (animal(&fauna, sp), animal(&fauna, bw));
        assert_eq!(p.pose.y_m - h.pose.y_m > ceiling, !prey, "fixture: lift {lift}");
        assert_eq!(crate::is_prey_of(&fauna.config, &h, &p), prey, "lift {lift}");
    }
}

/// The meal less the strikes' upkeep, the catch's worth, from the config the case set.
fn worth(c: &FaunaConfig, h: &crate::Animal, p: &crate::Animal) -> f64 {
    let phys = c.founder(SP);
    let t = phys.hunter.unwrap();
    let armour = c.founder(BW).armour;
    let damage = t.strike_power * h.body / p.body * (1.0 - armour);
    let strikes = ((1.0 - p.wound).max(0.0) / damage).ceil().max(1.0);
    let cost = strikes * t.strike_cost_s * phys.core.maintenance_per_s * h.body;
    p.organic() * phys.yield_by_food[Food::Carrion.index()] - cost
}

#[test]
fn a_catch_worth_exactly_its_minimum_gain_is_declined_and_a_hair_more_is_taken() {
    let (fauna, sp, bw) = pair(config(), 0.0);
    let (h, p) = (animal(&fauna, sp), animal(&fauna, bw));
    let w = worth(&fauna.config, &h, &p);
    assert!(w > 0.0, "fixture: a catch that pays");
    for (gain, prey) in [(w, false), (w * (1.0 - 1e-9), true)] {
        let mut c = *fauna.config;
        c.founders[SP.index()].catch_gain_min = gain;
        assert_eq!(crate::is_prey_of(&c, &h, &p), prey, "gain floor {gain} against worth {w}");
    }
}

#[test]
fn with_catching_off_a_seedporter_is_no_predator_of_bellwings() {
    let mut c = config();
    c.founders[SP.index()].catch_bellwings = false;
    let (fauna, sp, bw) = pair(c, 0.0);
    let (s, b) = (animal(&fauna, sp), animal(&fauna, bw));
    assert!(!crate::is_prey_of(&fauna.config, &s, &b), "no catch with the switch off");
    assert_eq!(crate::perception::class_of(&fauna.config, &s, &b), None, "the seedporter sees no prey in a bellwing");
    assert_eq!(crate::perception::class_of(&fauna.config, &b, &s), None, "a bellwing sees no threat in a seedporter");
}

/// CATCH-SWITCH: the switch governs the whole predator role. No other lineage hunts
/// bellwings here, so with the catch off nothing makes a bellwing a quarry: it does not
/// flee the seedporter beside it, and neither body's remote senses class the other (prey,
/// threat). With it on, all three happen.
#[test]
fn with_catching_off_a_bellwing_does_not_flee_a_seedporter_nor_does_it_sense_one_as_prey() {
    for on in [true, false] {
        let mut c = config();
        for f in Founder::ALL {
            if f != SP {
                if let Some(h) = c.founders[f.index()].hunter.as_mut() {
                    h.prey[BW.index()] = false;
                }
            }
        }
        for f in [SP, BW] {
            c.founders[f.index()].senses.live = true;
        }
        // The bellwing sees the body beside it and runs the S1 flee rule on its record.
        c.founders[BW.index()].senses.sight_m = 4.0;
        c.founders[BW.index()].vigilance.mode = crate::VigilMode::None;
        c.founders[SP.index()].catch_bellwings = on;
        let world = noon(604);
        let flora = Flora::new(FloraConfig::default());
        let (mut fauna, sp, bw) = pair(c, 0.0);
        fauna.tick += 1;
        crate::perception::sense_pass(&mut fauna, &world.view(), false, 1.0, 1);
        let tracked = |who: u64, of: u64, class: u8| {
            fauna.percept(who).is_some_and(|p| p.tracks[..usize::from(p.n)].iter().any(|t| t.id == of && t.class == class))
        };
        assert_eq!(tracked(sp, bw, crate::perception::class::PREY), on, "catch on={on}: the seedporter tracks the bellwing as prey");
        assert_eq!(tracked(bw, sp, crate::perception::class::THREAT), on, "catch on={on}: the bellwing tracks the seedporter as a threat");
        let i = fauna.animals.iter().position(|a| a.id == bw).unwrap();
        let mut grid = crate::senses::LiveGrid::default();
        grid.refresh(&world.view(), &flora.view(), &fauna.view(), 1, (false, true));
        let occ = crate::senses::Occ::Live(grid.view(&world.view(), crate::senses::Flavour::Volume));
        let fled = super::flee(&mut fauna, i, &world.view(), &flora.view(), Some(occ), Actions::default());
        assert_eq!(fled.is_some(), on, "catch on={on}: the bellwing flees the seedporter beside it");
    }
}

#[test]
fn with_catching_off_a_seedporter_does_not_eat_carrion_at_its_mouth() {
    for on in [true, false] {
        let world = noon(602);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config();
        c.founders[SP.index()].catch_bellwings = on;
        c.founders[SP.index()].catch_foray_m = 0.0;
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let sp = seedporter(&mut fauna, &world, 3.0);
        drive(&mut fauna, sp, 0.0, true);
        let a = animal(&fauna, sp);
        let g = fauna.config.founder(SP).body_at(a.body);
        for (x, z) in crate::body::mouth_columns(&world.view(), &a.pose, &g) {
            let site = Site { x: x.rem_euclid(i64::from(world.view().config.width)) as u32, y: a.site.y, z };
            assert!(flora.deposit(site, Deposit { kind: DepositKind::Carrion, organic: 0.1, mineral: 0.001, energy: 0.2 }));
            assert!(flora.lay_carcass(site, cubarium_voxel_flora::Carcass { lineage: BW.index() as u8, heading_rad: EAST, organic: 0.1 }));
        }
        crate::testpass3_support::ledgers(&fauna, &flora);
        for _ in 0..40 {
            fauna.step_with(&world, &mut flora, 1);
            animal_mut(&mut fauna, sp).eating = true;
        }
        let bites = fauna.ledger.bites_by_founder_food[SP.index()][Food::Carrion.index()];
        assert_eq!(bites > 0, on, "catch on={on}: carrion at its mouth is food only while it catches");
        crate::testpass3_support::ledgers(&fauna, &flora);
    }
}

/// A grounded, hungry seedporter at x = 3 that has seen `count` low bellwings `d` metres
/// east of it on the lane (its record's sight tracks).
fn foray(c: FaunaConfig, count: usize, d: f64) -> (World, Flora, Fauna, u64) {
    let world = noon(603);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c);
    let sp = seedporter(&mut fauna, &world, 3.0);
    fauna.tick = 10;
    let tracks: Vec<_> = (0..count)
        .map(|_| {
            let x = 3.0 + d;
            let bw = bellwing(&mut fauna, &world, x, 0.0);
            crate::Track { known: true, id: bw, lineage: BW.index() as u8, modes: crate::perception::SIGHT, x, z: LANE_Z, y_m: SURFACE_M, tick: fauna.tick, ..Default::default() }
        })
        .collect();
    let p = fauna.percepts.entry(sp).or_default();
    p.n = tracks.len() as u8;
    p.tracks[..tracks.len()].copy_from_slice(&tracks);
    (world, flora, fauna, sp)
}

fn catch_step(world: &World, flora: &Flora, fauna: &mut Fauna, sp: u64) -> Option<Actions> {
    let i = fauna.animals.iter().position(|a| a.id == sp).unwrap();
    crate::hunt::catch_step(fauna, i, &world.view(), &flora.view())
}

#[test]
fn a_cluster_exactly_at_the_foray_radius_draws_a_foray_and_a_hair_beyond_does_not() {
    let c = config();
    let r = c.founder(SP).catch_foray_m;
    for (d, goes) in [(r, true), (r + 1e-9, false)] {
        let (w, flora, mut fauna, sp) = foray(c.clone(), 2, d);
        let go = catch_step(&w, &flora, &mut fauna, sp);
        assert_eq!(go.is_some_and(|a| a.forward > 0.0), goes, "cluster at {d} m, radius {r} m");
        assert_eq!(animal(&fauna, sp).hunt.mode == HuntMode::Travel, goes);
    }
}

#[test]
fn a_cluster_of_exactly_the_minimum_draws_a_foray() {
    let mut c = config();
    c.founders[SP.index()].catch_foray_min = 3.0;
    let d = 0.5 * c.founder(SP).catch_foray_m;
    for (count, goes) in [(3, true), (2, false)] {
        let (w, flora, mut fauna, sp) = foray(c.clone(), count, d);
        assert_eq!(catch_step(&w, &flora, &mut fauna, sp).is_some(), goes, "{count} bodies, minimum 3");
    }
}

#[test]
fn a_foray_runs_exactly_its_budget_then_marks_the_cluster_fruitless() {
    let c = config();
    let d = 0.5 * c.founder(SP).catch_foray_m;
    let (w, flora, mut fauna, sp) = foray(c.clone(), 2, d);
    let cadence_s = SP.manifest().cadence_ticks() as f64 / f64::from(crate::TICK_HZ);
    let budget = (c.founder(SP).catch_foray_s / cadence_s).ceil() as usize;
    assert!(budget >= 2, "fixture: a budget of several samples");
    let mut went = 0;
    while catch_step(&w, &flora, &mut fauna, sp).is_some() {
        went += 1;
        assert!(went <= budget + 1, "the foray never ends");
    }
    assert_eq!(went, budget, "a foray holds for exactly catch_foray_s of samples");
    let ring = f64::from(w.view().config.width) * VOXEL_M;
    let a = animal(&fauna, sp);
    assert!(a.hunt.is_fruitless(&fauna.config.hunt, 3.0 + d, LANE_Z, fauna.tick, ring), "the unrewarded cluster is remembered");
    assert!(catch_step(&w, &flora, &mut fauna, sp).is_none(), "and not tried again while remembered");
}

/// One sample of the seedporter in [`foray_or_flight`]: its steer, hunt mode, foray samples,
/// patch, x, and whether it is in flight.
type Sample = (Option<crate::Steer>, HuntMode, u32, (f64, f64), f64, bool);

/// A hungry seedporter heuristic at x = 4 with a low bellwing cluster east of it at 6.5 and,
/// with `threat`, a Veilhound beyond the cluster walking west toward it; one sample a tick.
fn foray_or_flight(threat: bool, ticks: u32) -> (Vec<Sample>, (f64, f64)) {
    let world = clock_off(604);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[VH.index()].active = Activity::Always;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let sp = introduce(&mut fauna, &world, SP, StartingStores { body: 1.0, reserve: 0.1 });
    place(&mut fauna, sp, 4.0, LANE_Z, EAST);
    animal_mut(&mut fauna, sp).eating = true;
    fauna.controllers.set(sp, crate::controller::heuristic(SP, fauna.config.teachers));
    let cluster = (6.5, LANE_Z);
    for k in 0..3 {
        let bw = bellwing(&mut fauna, &world, cluster.0 + 0.05 * k as f64, 0.0);
        let b = animal_mut(&mut fauna, bw);
        b.mobility.mode = MobilityMode::Perched;
        b.mobility.falling = false;
    }
    if threat {
        let vh = introduce(&mut fauna, &world, VH, StartingStores::FULL);
        place(&mut fauna, vh, 8.5, LANE_Z, WEST);
        drive(&mut fauna, vh, 0.3, false);
    }
    let mut out = Vec::new();
    for _ in 0..ticks {
        fauna.step_with(&world, &mut flora, 1);
        if let Some(a) = fauna.animals.iter_mut().find(|a| a.id == sp) {
            a.eating = true;
        }
        let a = animal(&fauna, sp);
        let fleeing = fauna.percept(sp).is_some_and(|p| p.vigil.fleeing);
        out.push((fauna.steer.get(&sp).copied(), a.hunt.mode, a.hunt.samples, a.hunt.patch, a.pose.x, fleeing));
    }
    (out, cluster)
}

#[test]
fn a_seedporter_fleeing_a_veilhound_makes_no_foray() {
    let ticks = 6 * SP.manifest().cadence_ticks() as u32;
    let (calm, cluster) = foray_or_flight(false, ticks);
    assert!(
        calm.iter().any(|s| s.1 == HuntMode::Travel && (s.3 .0 - cluster.0).abs() < 0.5) && calm.iter().all(|s| !s.5),
        "fixture: unthreatened, it forays to the cluster: {calm:?}"
    );
    let (run, _) = foray_or_flight(true, ticks);
    let start = run.iter().position(|s| s.5).unwrap_or_else(|| panic!("fixture: the Veilhound sets it running: {run:?}"));
    let after = &run[start..];
    assert!(after.len() > 4, "fixture: several samples in flight: {run:?}");
    for s in after {
        assert_eq!(s.0, Some(crate::Steer::Flee), "in flight the flee steers, not the foray: {run:?}");
        assert_eq!(s.2, after[0].2, "the foray's clock does not run while it flees: {run:?}");
    }
    assert!(after.last().unwrap().4 < after[0].4, "it runs west, away from the cluster and the Veilhound beyond it: {run:?}");
}

#[test]
fn the_ledgers_of_both_layers_close_through_a_catch_and_its_meal() {
    let world = noon(605);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[SP.index()].catch_foray_m = 0.0;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let sp = seedporter(&mut fauna, &world, 3.0);
    fauna.controllers.set(sp, crate::controller::heuristic(SP, fauna.config.teachers));
    let phys = fauna.config.founder(SP);
    let h = phys.hunter.unwrap();
    let g = phys.body_at(animal(&fauna, sp).body);
    let ahead = (h.jaw_forward_length_fraction + 0.5 * h.strike_reach_length_fraction) * g.length_m;
    let bw = bellwing(&mut fauna, &world, 3.0 + ahead, 0.0);
    crate::testpass3_support::ledgers(&fauna, &flora);
    let mut killed = false;
    for t in 0..200 {
        if let Some(a) = fauna.animals.iter_mut().find(|a| a.id == bw) {
            a.pose.x = 3.0 + ahead;
            a.pose.z = LANE_Z;
            a.pose.y_m = SURFACE_M;
            a.mobility.mode = MobilityMode::Perched;
            a.mobility.falling = false;
        }
        fauna.step_with(&world, &mut flora, 1);
        crate::testpass3_support::ledgers(&fauna, &flora);
        killed |= fauna.view().animal(bw).is_none();
        if killed && fauna.ledger.bites_by_founder_food[SP.index()][Food::Carrion.index()] > 0 {
            assert!(fauna.ledger.eaten_by_founder_food[SP.index()][Food::Carrion.index()] > 0.0, "tick {t}");
            return;
        }
    }
    panic!("fixture: the catch and its meal did not happen in 200 ticks (killed={killed})");
}

/// A capgnawer `d` metres west of a lit, waiting lanternjaw's lure on a noon plain, its
/// light sense `light_m`; returns the scene and the lure point.
fn lure_scene(c: FaunaConfig, d: f64) -> (World, Flora, Fauna, u64, (f64, f64)) {
    let world = noon(606);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c);
    let lj = introduce(&mut fauna, &world, LJ, HUNTING);
    place(&mut fauna, lj, 5.0, LANE_Z, EAST);
    {
        let a = animal_mut(&mut fauna, lj);
        a.pose.y_m = SURFACE_M;
        a.founder_state.lure = 1.0;
    }
    drive(&mut fauna, lj, 0.0, false);
    let (lx, lz, _) = light::lure_point(&fauna.config, &animal(&fauna, lj), VOXEL_M).unwrap();
    let id = introduce(&mut fauna, &world, CAP, StartingStores { body: 1.0, reserve: 0.0 });
    place(&mut fauna, id, lx - d, lz, EAST);
    drive(&mut fauna, id, 0.0, false);
    animal_mut(&mut fauna, id).eating = true;
    fauna.beacons.fixed(&world.view(), &flora.view(), vec![]);
    (world, flora, fauna, id, (lx, lz))
}

fn cap_config(near_m: f64, light_m: f64) -> FaunaConfig {
    let mut c = config();
    c.light.visit_m = 0.1;
    c.light.near_m = near_m;
    c.light.visit_s = 0.5;
    c.light.mark_s = 6.0;
    let s = &mut c.founders[CAP.index()].senses;
    s.live = true;
    s.nir = false;
    s.light_m = light_m;
    s.light_min = 0.00001;
    c
}

fn glow(world: &World, flora: &Flora, fauna: &mut Fauna, id: u64) -> Option<Actions> {
    let i = fauna.animals.iter().position(|a| a.id == id).unwrap();
    light::steer_glow(fauna, i, &world.view(), &flora.view(), 1.0, Actions { forward: 1.0, turn: 0.0, feed: 0.0 }, None)
}

#[test]
fn a_far_light_approach_runs_exactly_its_budget_before_the_mark() {
    let c = cap_config(0.5, 2.0);
    let manifest = c.manifest(CAP);
    let travel_s = c.founder(CAP).senses.light_m / manifest.cruise_m_per_s;
    let cadence_s = manifest.cadence_ticks() as f64 / f64::from(crate::TICK_HZ);
    let budget = ((travel_s + c.light.visit_s) / cadence_s).ceil().max(1.0) as usize;
    let d = 0.5 * (c.light.near_m + c.founder(CAP).senses.light_m);
    let (w, flora, mut fauna, id, (lx, lz)) = lure_scene(c.clone(), d);
    let ring = f64::from(w.view().config.width) * VOXEL_M;
    for k in 0..budget {
        fauna.tick += manifest.cadence_ticks();
        assert!(glow(&w, &flora, &mut fauna, id).is_some(), "sample {k} of {budget}: still drawn");
        assert!(!fauna.percept(id).unwrap().marked(lx, lz, fauna.tick, c.light.mark_radius_m, ring));
    }
    fauna.tick += manifest.cadence_ticks();
    assert!(glow(&w, &flora, &mut fauna, id).is_none(), "the sample after the budget gives up");
    assert!(fauna.percept(id).unwrap().marked(lx, lz, fauna.tick, c.light.mark_radius_m, ring));
    assert_eq!(fauna.ledger.stalled_visits[CAP.index()], 1);
}

#[test]
fn a_near_light_approach_fails_exactly_at_visit_s_and_not_a_tick_before() {
    let c = cap_config(1.2, 8.0);
    let d = 0.5 * (c.light.visit_m + c.light.near_m);
    let (w, flora, mut fauna, id, (lx, lz)) = lure_scene(c.clone(), d);
    let ring = f64::from(w.view().config.width) * VOXEL_M;
    fauna.tick = 100;
    assert!(glow(&w, &flora, &mut fauna, id).is_some(), "the inspection starts");
    let visit = cubarium_voxel::ticks_at_least_one(c.light.visit_s);
    fauna.tick = 100 + visit - 1;
    assert!(glow(&w, &flora, &mut fauna, id).is_some(), "a tick short of visit_s it is still inspecting");
    assert!(!fauna.percept(id).unwrap().marked(lx, lz, fauna.tick, c.light.mark_radius_m, ring));
    fauna.tick = 100 + visit;
    assert!(glow(&w, &flora, &mut fauna, id).is_none(), "at visit_s the unrewarded inspection fails");
    assert!(fauna.percept(id).unwrap().marked(lx, lz, fauna.tick, c.light.mark_radius_m, ring));
    assert_eq!(fauna.ledger.failed_visits[CAP.index()][BeaconKind::Lure.index()], 1);
}

#[test]
fn a_capgnawer_that_gave_up_a_lure_grazes_cropped_mycelium_and_the_books_close() {
    let mut c = cap_config(0.5, 2.0);
    let g = &mut c.founders[CAP.index()];
    g.diet = Diet::only(Food::CapTissue);
    g.gut.s0 = 0.0;
    let d = 0.5 * (c.light.near_m + c.founder(CAP).senses.light_m);
    let (w, _, mut fauna, id, (lx, lz)) = lure_scene(c.clone(), d);
    let mut fc = FloraConfig::default();
    fc.glowcap.mycelium_graze_fraction = 0.5;
    fc.glowcap.defence_per_bite = 0.0;
    let mut flora = Flora::new(fc);
    let ring = f64::from(w.view().config.width) * VOXEL_M;
    // Give up the lure: the far budget runs out.
    let manifest = fauna.config.manifest(CAP);
    let mut gave_up = false;
    for _ in 0..1000 {
        fauna.tick += manifest.cadence_ticks();
        if glow(&w, &flora, &mut fauna, id).is_none() {
            gave_up = true;
            break;
        }
    }
    assert!(gave_up && fauna.percept(id).unwrap().marked(lx, lz, fauna.tick, c.light.mark_radius_m, ring), "fixture: the lure is marked");
    // A glowcap cropped to its refuge under its mouth.
    let at = animal(&fauna, id).site;
    let wood = flora.config().glowcap.wood_max;
    assert!(flora.apply(&w, FloraCommand::Seed { x: i64::from(at.x), z: at.z, species: Species::Glowcap, wood }));
    let fungus = Site { x: at.x, y: SURFACE_Y, z: at.z };
    flora.take_foliage_of(fungus, FaceLayer::Fungus, f64::MAX, &(i64::MIN..=i64::MAX)).expect("crop the caps");
    assert!(flora.view().edible_mycelium(flora.view().fungus_at(fungus).unwrap()) > 0.0, "fixture: exposed mycelium");
    drive(&mut fauna, id, 0.0, true);
    crate::testpass3_support::ledgers(&fauna, &flora);
    for _ in 0..20 {
        fauna.step_with(&w, &mut flora, 1);
        crate::testpass3_support::ledgers(&fauna, &flora);
        if fauna.ledger.eaten_mycelium_by_founder[CAP.index()] > 0.0 {
            break;
        }
    }
    let eaten = fauna.ledger.eaten_mycelium_by_founder[CAP.index()];
    assert!(eaten > 0.0, "the forager in control grazes the exposed mycelium");
    assert!((flora.view().ledger.mycelium_taken[Species::Glowcap.index()] - eaten).abs() < 1e-12, "one transfer, seen from both layers");
}

#[test]
fn mimic_lineage_fields_survive_a_postcard_round_trip() {
    let mut c = config();
    let lj = &mut c.founders[LJ.index()];
    lj.lure_daylight = false;
    lj.flower_ambush_m = 0.875;
    let sp = &mut c.founders[SP.index()];
    sp.catch_ceiling_m = 0.625;
    sp.catch_gain_min = 0.03125;
    sp.catch_foray_m = 3.5;
    sp.catch_foray_min = 4.0;
    sp.catch_foray_s = 12.5;
    let back = Fauna::load(&Fauna::new(c.clone()).save()).expect("round trip");
    for f in [LJ, SP, BW, VH] {
        let (a, b) = (back.config.founder(f), c.founder(f));
        assert_eq!(
            (a.lure_daylight, a.flower_ambush_m, a.catch_bellwings, a.catch_ceiling_m, a.catch_gain_min, a.catch_foray_m, a.catch_foray_min, a.catch_foray_s),
            (b.lure_daylight, b.flower_ambush_m, b.catch_bellwings, b.catch_ceiling_m, b.catch_gain_min, b.catch_foray_m, b.catch_foray_min, b.catch_foray_s),
            "{}",
            f.name()
        );
    }
    assert_eq!(*back.config, c);
}
