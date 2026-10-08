//! TRAPS (2026-10-08): the stuck states the STUCK-AUDIT and JUVENILE-FOOD audits found, each
//! checked as a mechanism on a hand-built world, against whatever the config says.
//!
//! - A turn with no forward on a wall face backs the body off it, onto the face it came from.
//! - A reflex walk reads "ahead" on a face as the way the climb goes (down a cliff too).
//! - A hunter up a stem with a carcass at its foot climbs down to it instead of "eating" it
//!   from the stem.
//!
//! (The motor's step along a wall a body is caught in is in `testpass2_edge_tests`.)

use super::predation_fixture::{animal, animal_mut};
use crate::{Actions, Activity, Command, Fauna, FaunaConfig, Founder, HuntMode, MobilityMode, StartingStores, WallClimb};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Carcass, Deposit, DepositKind, Flora, FloraConfig, Site};
use std::f64::consts::{FRAC_PI_2, PI};

const V: f64 = 0.25;
const EAST: f64 = FRAC_PI_2;
const FLOOR: u32 = 2;
const SHREDDER: Founder = Founder::Blind;
const SP: Founder = Founder::Seedporter;

fn plain(width: u32, depth: u32, height: u32) -> World {
    let day = DayClock { start_hour: 12.0, ..DayClock::default() };
    let mut w = World::empty(VoxelConfig { width, height, depth, voxel_m: V, seed: 17, day, ..VoxelConfig::default() });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=FLOOR {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

fn rock(w: &mut World, xs: std::ops::RangeInclusive<i64>, zs: std::ops::RangeInclusive<u32>, ys: std::ops::RangeInclusive<u32>) {
    for z in zs {
        for x in xs.clone() {
            for y in ys.clone() {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
            }
        }
    }
}

fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c
}

fn put(fauna: &mut Fauna, world: &World, f: Founder, (x_m, z_m): (f64, f64), heading: f64) -> u64 {
    let (x, z) = ((x_m / V).floor() as i64, (z_m / V).floor() as u32);
    let stores = StartingStores { body: 1.0, reserve: 0.5 };
    assert!(fauna.apply(world, Command::IntroduceFounder { x, z, founder: f, stores, heading_rad: heading }), "placing a {}", f.name());
    let id = fauna.animals.last().expect("just placed").id;
    let a = animal_mut(fauna, id);
    a.pose.x = x_m;
    a.pose.z = z_m;
    a.pose.heading_rad = heading;
    id
}

fn ticks(fauna: &mut Fauna, world: &World, flora: &mut Flora, n: u64) {
    for _ in 0..n {
        fauna.step_with(world, flora, 1);
    }
}

fn hold(fauna: &mut Fauna, id: u64, act: Actions) {
    fauna.controllers.set(id, Box::new(crate::Scripted::new(vec![act])));
}

/// A littershredder walked east into a wall it climbs, on the face and part way up.
fn on_the_wall() -> (World, Flora, Fauna, u64) {
    let mut world = plain(32, 8, 16);
    rock(&mut world, 16..=19, 0..=7, FLOOR + 1..=FLOOR + 6);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config());
    fauna.set_births_enabled(false);
    let id = put(&mut fauna, &world, SHREDDER, (13.5 * V, 4.5 * V), EAST);
    hold(&mut fauna, id, Actions { forward: 1.0, turn: 0.0, feed: 0.0 });
    for _ in 0..4000 {
        fauna.step_with(&world, &mut flora, 1);
        if animal(&fauna, id).mobility.climb.is_some_and(|c| c.progress_m > 0.0) {
            break;
        }
    }
    let c = animal(&fauna, id).mobility.climb.expect("fixture: it is on the wall");
    assert!(c.ascending && c.progress_m > 0.0 && c.progress_m < c.rise_m, "fixture: part way up: {c:?}");
    (world, flora, fauna, id)
}

/// **A turn with no forward backs a climber off its face** onto the face it started from, on
/// its own side of the wall, facing away from it; holding still (no turn, no forward) it stays
/// where it is on the face.
#[test]
fn a_turn_with_no_forward_backs_a_climber_off_its_face() {
    let (world, mut flora, mut fauna, id) = on_the_wall();
    hold(&mut fauna, id, Actions { forward: 0.0, turn: 0.0, feed: 0.0 });
    // The held actions change at the next controller sample.
    let cadence = fauna.config.manifest(SHREDDER).cadence_ticks().max(1);
    ticks(&mut fauna, &world, &mut flora, cadence + 1);
    let start = animal(&fauna, id).mobility.climb.expect("still on the face");
    ticks(&mut fauna, &world, &mut flora, 2 * cadence);
    let still = animal(&fauna, id).mobility.climb.expect("still on the face");
    assert_eq!(still.progress_m, start.progress_m, "no forward, no turn: it holds on");

    hold(&mut fauna, id, Actions { forward: 0.0, turn: 1.0, feed: 0.0 });
    let mut down = false;
    for _ in 0..4000 {
        fauna.step_with(&world, &mut flora, 1);
        let a = animal(&fauna, id);
        match a.mobility.climb {
            Some(c) => assert!(c.progress_m <= start.progress_m, "it goes back, never on up: {c:?}"),
            None => {
                down = true;
                break;
            }
        }
    }
    assert!(down, "it came off the face");
    let a = animal(&fauna, id);
    assert_eq!(a.site, start.from, "it stands on the face it started from");
    let r = fauna.config.founder(SHREDDER).body_at(a.body).footprint_radius();
    assert!(a.pose.x <= 16.0 * V - r + 1e-9, "on its own side of the wall: x {} plane {}", a.pose.x, 16.0 * V);
    assert!((a.pose.heading_rad - (EAST + PI)).abs() < 1e-9, "facing away from the wall: {}", a.pose.heading_rad);
}

/// **A reflex walk reads ahead on a face as the way the climb goes**: a body hanging on a
/// cliff on its way down (facing back up the face, as a descent hangs) with its target out
/// beyond the cliff's foot is told forward; with its target back behind the cliff's top it is
/// told to turn (which on a face backs it off).
#[test]
fn a_reflex_walk_on_a_face_reads_ahead_as_the_way_the_climb_goes() {
    let world = plain(32, 8, 16);
    let mut fauna = Fauna::new(config());
    let id = put(&mut fauna, &world, SHREDDER, (10.5 * V, 4.5 * V), EAST + PI);
    let from = animal(&fauna, id).site;
    let a = animal_mut(&mut fauna, id);
    a.mobility.climb = Some(WallClimb {
        ascending: false,
        from,
        to: Site { x: from.x + 1, y: from.y.saturating_sub(4), z: from.z },
        dir: (1, 0),
        progress_m: 0.1,
        rise_m: 4.0 * V,
    });
    let a = animal(&fauna, id);
    let on = super::glider::walk_toward(&a, 3.0, 0.0, 0.0);
    assert!(on.forward > 0.0, "target beyond the foot, the way it climbs: {on:?}");
    let back = super::glider::walk_toward(&a, -3.0, 0.0, 0.0);
    assert!(back.forward == 0.0 && back.turn != 0.0, "target behind the top: a turn, no forward: {back:?}");
}

/// A seedporter at a carcass laid under its mouth, standing as it is: the meal step's answer.
fn at_a_carcass(mode: MobilityMode) -> (Fauna, u64, Option<Actions>) {
    let world = plain(32, 8, 16);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.hunt.meal_first = true;
    c.founders[SP.index()].diet = crate::Diet::of(&[crate::Food::Fruit, crate::Food::Carrion]);
    let mut fauna = Fauna::new(c);
    let id = put(&mut fauna, &world, SP, (10.5 * V, 4.5 * V), EAST);
    fauna.step_with(&world, &mut flora, 1);
    let a = animal(&fauna, id);
    let g = fauna.config.founder(SP).body_at(a.body);
    for (x, z) in crate::body::mouth_columns(&world.view(), &a.pose, &g) {
        let site = Site { x: x.rem_euclid(i64::from(world.view().config.width)) as u32, y: a.site.y, z };
        assert!(flora.deposit(site, Deposit { kind: DepositKind::Carrion, organic: 0.1, mineral: 0.01, energy: 0.2 }));
        assert!(flora.lay_carcass(site, Carcass { lineage: Founder::Bellwing.index() as u8, heading_rad: EAST, organic: 0.1 }));
    }
    assert!(crate::hunt::mouth_on_carcass(&flora.view(), &world.view(), &a, &g), "fixture: its mouth is over the carcass");
    let a = animal_mut(&mut fauna, id);
    a.eating = true;
    a.mobility.mode = mode;
    a.mobility.falling = false;
    let i = fauna.animals.iter().position(|a| a.id == id).unwrap();
    let act = crate::hunt::meal_step(&mut fauna, i, &world.view(), &flora.view());
    (fauna, id, act)
}

/// **Up a stem, a carcass at its foot is no meal from there**: on its feet over a carcass the
/// seedporter holds still and feeds (the meal); on a stem at the same spot (its feeding there
/// takes only fruit) it climbs down instead, not feeding.
#[test]
fn a_hunter_up_a_stem_climbs_down_to_a_carcass_at_its_foot() {
    let (fauna, id, act) = at_a_carcass(MobilityMode::Ground);
    let act = act.expect("a meal on its feet");
    assert!(act.feed > 0.5 && act.forward == 0.0, "on its feet: it eats: {act:?}");
    assert_eq!(animal(&fauna, id).hunt.mode, HuntMode::Meal);

    let (fauna, id, act) = at_a_carcass(MobilityMode::Climb);
    let act = act.expect("an answer on the stem");
    assert!(act.feed == 0.0, "on the stem: no feeding at the carcass: {act:?}");
    let a = animal(&fauna, id);
    assert!(a.mobility.falling, "on the stem: it climbs down");
    assert_ne!(a.hunt.mode, HuntMode::Meal, "on the stem: no meal booked");
}
