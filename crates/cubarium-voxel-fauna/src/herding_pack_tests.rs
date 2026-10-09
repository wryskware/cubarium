//! **HERD-PULL, the pack singles out the straggler** (Wrysk's decision of 2026-10-09,
//! training plan §6 item 15), from the spec: given an isolated grazer and one inside a herd
//! at equal reach, the pack's choice is the isolated one; an armoured adult it cannot open,
//! or one it has shunned, is still not taken, however isolated. A child of `pack`, so a case
//! calls the prey choice itself on a scene it built.
//!
//! Every case sets the config it relies on; no shipped default, ratio or verdict is asserted.

use super::*;
use crate::{Activity, Command, FaunaConfig, Founder, Hunker, Pose, StartingStores};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::Site;

const CH: Founder = Founder::Chorister;
const PREY: Founder = Founder::Browser;
const V: f64 = 0.25;
const FLOOR: u32 = 2;
const Z: f64 = 1.5;
/// The member choosing stands here; the isolated grazer `REACH` west of it, the herd's
/// grazer `REACH` east.
const MEMBER_X: f64 = 8.0;
const REACH: f64 = 3.0;

fn ground(width: u32, depth: u32) -> World {
    let mut w = World::empty(VoxelConfig { width, height: 8, depth, voxel_m: V, seed: 11, day: DayClock { mode: DayMode::Off, ..DayClock::default() }, ..VoxelConfig::default() });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=FLOOR {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

/// Every lineage awake; the grazer unarmoured (its hunker off) unless a case arms it; the
/// pack's kin factor neutral (1), so only the straggler's factor tells kin apart; the
/// straggler at `gain` over 4 m; the pack sees all round.
fn config(gain: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[PREY.index()].hunker = Hunker::default();
    c.founders[CH.index()].senses.sight_half_angle_deg = 180.0;
    let pk = c.founders[CH.index()].pack.as_mut().unwrap();
    pk.select_kin_factor = 1.0;
    pk.select_straggler_m = 4.0;
    pk.select_straggler_gain = gain;
    pk.select_min_prospect = 0.0;
    c
}

fn put(fauna: &mut Fauna, world: &World, f: Founder, x: f64, heading: f64) -> u64 {
    assert!(fauna.apply(world, Command::IntroduceFounder { x: (x / V) as i64, z: (Z / V) as u32, founder: f, stores: StartingStores::FULL, heading_rad: 0.0 }));
    let id = fauna.animals.last().unwrap().id;
    let a = fauna.animals.iter_mut().find(|a| a.id == id).unwrap();
    a.pose = Pose { x, z: Z, heading_rad: heading, y_m: crate::surface_m(FLOOR, V) };
    a.site = Site { x: (x / V).floor() as u32, y: FLOOR, z: (Z / V).floor() as u32 };
    id
}

/// A pack whose members all stand at `MEMBER_X` facing north, hungry; the isolated grazer
/// `REACH` west of them and a herd's grazer `REACH` east, two herd-mates beside it (east of
/// it, farther from the pack). Sensed once. The pack, the choosing member's index, the
/// isolated grazer and the herd's grazer.
fn scene(c: FaunaConfig) -> (World, Fauna, u64, usize, u64, u64) {
    let world = ground(96, 12);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    assert!(fauna.apply(&world, Command::FoundPack { den: Site { x: (MEMBER_X / V) as u32, y: FLOOR, z: (Z / V) as u32 }, founder: CH, stores: StartingStores::FULL, heading_rad: 0.0 }));
    let own = fauna.packs()[0].id;
    for a in fauna.animals.iter_mut().filter(|a| a.pack.is_some()) {
        a.pose = Pose { x: MEMBER_X, z: Z, heading_rad: 0.0, y_m: crate::surface_m(FLOOR, V) };
        a.eating = true;
    }
    let lone = put(&mut fauna, &world, PREY, MEMBER_X - REACH, 0.0);
    let herded = put(&mut fauna, &world, PREY, MEMBER_X + REACH, 0.0);
    put(&mut fauna, &world, PREY, MEMBER_X + REACH + 0.8, 0.0);
    put(&mut fauna, &world, PREY, MEMBER_X + REACH + 1.6, 0.0);
    fauna.tick += 1;
    let view = world.view();
    for _ in 0..fauna.config.perception.stagger_ticks() {
        fauna.tick += 1;
        crate::perception::sense_pass(&mut fauna, &view, false, 1.0, 1);
    }
    let i = fauna.animals.iter().position(|a| a.pack.is_some_and(|m| m.pack == own)).unwrap();
    (world, fauna, own, i, lone, herded)
}

/// The pack's choice at the member `i` with every member at hand.
fn choice(world: &World, fauna: &mut Fauna, own: u64, i: usize) -> Option<u64> {
    let view = world.view();
    let ring = f64::from(view.config.width) * V;
    let phys = *fauna.config.founder(CH);
    let (t, h) = (phys.pack.unwrap(), phys.active_hunter().unwrap());
    let geometry = phys.body_at(fauna.animals[i].body);
    let n = fauna.animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == own)).count() as u32;
    let cruise = fauna.config.manifest(CH).cruise_m_per_s;
    choose(fauna, i, &view, own, CH, &t, &geometry, &h, cruise, n, ring, None, 0.0, Weigh::Choice).0.map(|c| c.id)
}

fn knows(fauna: &Fauna, i: usize, id: u64, ring: f64) -> bool {
    known(fauna, i, ring).iter().any(|t| t.id == id)
}

#[test]
fn of_an_isolated_grazer_and_one_in_a_herd_at_equal_reach_the_pack_takes_the_isolated_one() {
    let (world, mut fauna, own, i, lone, herded) = scene(config(1.0));
    let ring = f64::from(world.view().config.width) * V;
    assert!(knows(&fauna, i, lone, ring) && knows(&fauna, i, herded, ring), "fixture: the pack knows both");
    assert_eq!(choice(&world, &mut fauna, own, i), Some(lone), "the straggler is singled out");
    // With the straggler off nothing tells them apart but the id: the rule, not the scene,
    // made the choice.
    let (world, mut fauna, own, i, lone, herded) = scene(config(0.0));
    let both = [lone, herded];
    assert!(choice(&world, &mut fauna, own, i).is_some_and(|c| both.contains(&c)), "fixture: either is takeable");
    let o_lone = *fauna.view().animal(lone).unwrap();
    let o_herd = *fauna.view().animal(herded).unwrap();
    let t = fauna.config.founder(CH).pack.unwrap();
    assert_eq!(straggler(&fauna, &o_lone, &t, ring), 1.0, "off: no factor");
    assert_eq!(straggler(&fauna, &o_herd, &t, ring), 1.0, "off: no factor");
}

#[test]
fn an_armoured_adult_it_cannot_open_or_one_it_shuns_is_not_taken_however_isolated() {
    // The isolated grazer is a grown adult under its shield that the members at hand cannot
    // pry open; the herd's grazer is a soft juvenile.
    let mut c = config(1.0);
    c.founders[PREY.index()].hunker = Hunker { enabled: true, pry_per_member_s: 1.0, resist_per_s: 100.0, shield_growth_exp: 2.0, ring_resist_per_kin: 0.0, ..crate::vigil::hunker_of(PREY) };
    let (world, mut fauna, own, i, _lone, herded) = scene(c);
    let adult = fauna.config.founder(PREY).core.birth_body;
    fauna.animals.iter_mut().find(|a| a.id == herded).unwrap().body = 0.1 * adult;
    let hk = fauna.config.founder(PREY).hunker;
    let n = fauna.animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == own)).count() as u32;
    assert!(hk.members_to_open(adult, adult, 0).is_none_or(|need| need > n), "fixture: the pack cannot open the adult");
    assert_eq!(choice(&world, &mut fauna, own, i), Some(herded), "armour still holds against the straggler's pull");
    // A shunned straggler (dropped alive) is not taken either.
    let (world, mut fauna, own, i, lone, herded) = scene(config(1.0));
    shun(&mut fauna, own, lone, 60.0);
    assert_eq!(choice(&world, &mut fauna, own, i), Some(herded), "the shun still holds");
}
