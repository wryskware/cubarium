//! **TEST-PASS-2 (independent), the pack's rules one call at a time**: STRIKE-2's herd shun
//! (`leave_herd`) and its room in the shun list, STRIKE's jaw band at its edges, PIT-2's swing
//! cap on a route (`route_go`) with its switch off, and PIT's way out (`out_of_pit`) with its
//! switch off. A child of `pack`, so a case calls the rule itself on a scene it built.
//!
//! Every case sets the config it relies on; no shipped default, ratio or verdict is asserted.

use super::*;
use crate::{Activity, Command, FaunaConfig, Founder, Hunker, Pose, StartingStores};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::Site;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

const CH: Founder = Founder::Chorister;
const PREY: Founder = Founder::Browser;
const V: f64 = 0.25;
const FLOOR: u32 = 2;
const EAST: f64 = FRAC_PI_2;

/// Soil at `y = 1..=top` on `width × depth` columns, the clock off.
fn ground(width: u32, depth: u32, height: u32, top: u32) -> World {
    let mut w = World::empty(VoxelConfig { width, height, depth, voxel_m: V, seed: 5, day: DayClock { mode: DayMode::Off, ..DayClock::default() }, ..VoxelConfig::default() });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=top {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

fn awake() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c
}

fn put(fauna: &mut Fauna, world: &World, f: Founder, x: f64, z: f64, y: u32) -> u64 {
    assert!(fauna.apply(world, Command::IntroduceFounder { x: (x / V) as i64, z: (z / V) as u32, founder: f, stores: StartingStores::FULL, heading_rad: 0.0 }));
    let id = fauna.animals.last().unwrap().id;
    let a = fauna.animals.iter_mut().find(|a| a.id == id).unwrap();
    a.pose = Pose { x, z, heading_rad: EAST, y_m: crate::surface_m(y, V) };
    a.site = Site { x: (x / V).floor() as u32, y, z: (z / V).floor() as u32 };
    id
}

fn body_mut(fauna: &mut Fauna, id: u64) -> &mut Animal {
    fauna.animals.iter_mut().find(|a| a.id == id).unwrap()
}

fn body(fauna: &Fauna, id: u64) -> Animal {
    *fauna.animals.iter().find(|a| a.id == id).unwrap()
}

// ------------------------------------------------------------------ STRIKE-2: the herd shun

/// A hunker with no ring bonus, adults needing three members, a fifth-grown juvenile one: the
/// opening read by the case, not by the shipped rates.
fn herd_config(herd_shun_m: f64) -> FaunaConfig {
    let mut c = awake();
    c.founders[PREY.index()].hunker = Hunker {
        enabled: true,
        pry_per_member_s: 1.0,
        resist_per_s: 2.5,
        shield_growth_exp: 1.0,
        ring_resist_per_kin: 0.0,
        ..crate::vigil::hunker_of(PREY)
    };
    let pk = c.founders[CH.index()].pack.as_mut().unwrap();
    pk.herd_shun_m = herd_shun_m;
    pk.shun_s = 30.0;
    c
}

/// A pack and a hunkered grazer herd on a strip: the target at x = 4 m and grazers at the
/// given x (all on z = 1.5 m) with the given body fraction of the adult; all hunkered.
fn herd(c: FaunaConfig, kin: &[(f64, f64)]) -> (World, Fauna, u64, u64, Vec<u64>) {
    let world = ground(64, 12, 8, FLOOR);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    assert!(fauna.apply(&world, Command::FoundPack { den: Site { x: 4, y: FLOOR, z: 6 }, founder: CH, stores: StartingStores::FULL, heading_rad: EAST }));
    let own = fauna.packs()[0].id;
    let adult = fauna.config.founder(PREY).core.birth_body;
    let target = put(&mut fauna, &world, PREY, 4.0, 1.5, FLOOR);
    body_mut(&mut fauna, target).body = adult;
    body_mut(&mut fauna, target).hunkered = true;
    let ids = kin
        .iter()
        .map(|&(x, f)| {
            let id = put(&mut fauna, &world, PREY, x, 1.5, FLOOR);
            body_mut(&mut fauna, id).body = f * adult;
            body_mut(&mut fauna, id).hunkered = true;
            id
        })
        .collect();
    (world, fauna, own, target, ids)
}

/// The tactic's drop: the target shunned, then its herd.
fn drop_target(fauna: &mut Fauna, world: &World, own: u64, target: u64) {
    let t = fauna.config.founder(CH).pack.unwrap();
    let ring = f64::from(world.config().width) * V;
    shun(fauna, own, target, t.shun_s);
    let o = body(fauna, target);
    leave_herd(fauna, own, &o, &t, ring);
}

fn shunned(fauna: &Fauna, own: u64, id: u64) -> bool {
    shuns(fauna, own, id)
}

/// **The herd shun takes armoured kin within `herd_shun_m` and no other**: an adult 2 m from
/// the dropped target (armour: three members' work), an adult 4 m off (beyond `herd_shun_m` 3
/// m), a fifth-grown juvenile 1 m off (one member opens it: not armour), and an adult 1.5 m
/// off that is turned over (its `opened_until` ahead): only the first is shunned with the
/// target.
#[test]
fn the_herd_shun_takes_armoured_kin_within_herd_shun_m_and_no_other() {
    let (world, mut fauna, own, target, kin) = herd(herd_config(3.0), &[(6.0, 1.0), (8.0, 1.0), (5.0, 0.2), (5.5, 1.0)]);
    let hk = fauna.config.founder(PREY).hunker;
    let adult = fauna.config.founder(PREY).core.birth_body;
    assert!(hk.members_to_open(adult, adult, 0).is_some_and(|n| n > 1), "fixture: an adult is armour");
    assert_eq!(hk.members_to_open(0.2 * adult, adult, 0), Some(1), "fixture: the juvenile is soft");
    let now = fauna.tick;
    fauna.percepts.entry(kin[3]).or_default().vigil.opened_until = now + 1000;
    drop_target(&mut fauna, &world, own, target);
    assert!(shunned(&fauna, own, target), "the target");
    assert!(shunned(&fauna, own, kin[0]), "the armoured adult within herd_shun_m");
    assert!(!shunned(&fauna, own, kin[1]), "the adult beyond herd_shun_m");
    assert!(!shunned(&fauna, own, kin[2]), "the soft juvenile");
    assert!(!shunned(&fauna, own, kin[3]), "the turned-over adult");
}

/// **The herd shun never pushes the dropped target out of the shun list**: the list's four
/// slots already hold live shuns of four other bodies (each lapsing before the new ones), and
/// four armoured adults stand within `herd_shun_m`: the target stays shunned, and the herd
/// takes the remaining slots, nearest first.
#[test]
fn the_herd_shun_never_pushes_the_dropped_target_out_of_the_shun_list() {
    let (world, mut fauna, own, target, kin) = herd(herd_config(3.0), &[(5.0, 1.0), (5.5, 1.0), (6.0, 1.0), (6.5, 1.0)]);
    let now = fauna.tick;
    let slots = fauna.pack(own).unwrap().shunned.len();
    {
        let p = fauna.pack_mut(own).unwrap();
        for k in 0..slots {
            p.shunned[k] = (1000 + k as u64, now + 10 + k as u64);
        }
    }
    drop_target(&mut fauna, &world, own, target);
    assert!(shunned(&fauna, own, target), "the target was pushed out: {:?}", fauna.pack(own).unwrap().shunned);
    let taken: Vec<bool> = kin.iter().map(|&k| shunned(&fauna, own, k)).collect();
    assert_eq!(taken, vec![true; slots - 1].into_iter().chain(std::iter::repeat(false)).take(kin.len()).collect::<Vec<_>>(), "nearest first, the rest of the room");
}

/// **The herd shun is off at `herd_shun_m` 0, and leaves the herd of a target that is not
/// armour**: an armoured adult 1 m from the target; with `herd_shun_m` 0 it is not shunned;
/// with the shun on but the target out of its hunker it is not either.
#[test]
fn the_herd_shun_is_off_at_zero_and_for_a_target_out_of_its_hunker() {
    let (world, mut fauna, own, target, kin) = herd(herd_config(0.0), &[(5.0, 1.0)]);
    drop_target(&mut fauna, &world, own, target);
    assert!(!shunned(&fauna, own, kin[0]), "herd_shun_m 0");
    let (world, mut fauna, own, target, kin) = herd(herd_config(3.0), &[(5.0, 1.0)]);
    body_mut(&mut fauna, target).hunkered = false;
    drop_target(&mut fauna, &world, own, target);
    assert!(!shunned(&fauna, own, kin[0]), "a target out of its hunker leaves no herd shun");
}

// ------------------------------------------------------------------ STRIKE: the jaw band

/// **The jaw band meets a body strictly inside it**: a prey whose base is at the band's top
/// is not met, a hair below it is; a prey whose top is at the hunter's surface is not met, a
/// hair above it is.
#[test]
fn the_jaw_band_meets_a_body_strictly_inside_it() {
    let c = awake();
    let phys = *c.founder(CH);
    let (g, h) = (phys.adult_body(), phys.hunter.unwrap());
    let s0 = 1.0;
    let top = s0 + h.jaw_band_height_fraction * g.height_m;
    let height = 0.3;
    assert!(!band_meets(&g, &h, s0, top, height), "base at the band's top");
    assert!(band_meets(&g, &h, s0, top - 1e-9, height), "base a hair under the band's top");
    assert!(!band_meets(&g, &h, s0, s0 - height, height), "top at the hunter's surface");
    assert!(band_meets(&g, &h, s0, s0 - height + 1e-9, height), "top a hair over the hunter's surface");
}

// ------------------------------------------------------------------ PIT-2: the swing on a route

/// **The swing cap holds a turn onto a route to `route_swing_voxels` across its line a
/// period, and 0 turns it off**: a Veilhound on open ground facing east, a route point 45° to
/// its left and three periods' walk away. With the cap off its forward is the full pace; with
/// `route_swing_voxels` s it is `s × voxel / (sin 45° × one period's walk)`, when that is under
/// 1. A point straight ahead is walked at the full pace either way.
#[test]
fn the_swing_cap_holds_a_turn_onto_a_route_and_zero_turns_it_off() {
    let world = ground(64, 24, 8, FLOOR);
    let view = world.view();
    let ring = f64::from(world.config().width) * V;
    for swing in [0.0, 0.25] {
        let mut c = awake();
        c.founders[CH.index()].pack.as_mut().unwrap().route_swing_voxels = swing;
        let mut fauna = Fauna::new(c);
        let id = put(&mut fauna, &world, CH, 4.0, 3.0, FLOOR);
        let a = body(&fauna, id);
        let m = fauna.config.manifest(CH);
        let travel = m.cruise_m_per_s * m.controller_period_s;
        let d = 3.0 * travel;
        // 45° to its left: headings run from +z toward +x, so left of east is toward +z.
        let want = EAST - FRAC_PI_4;
        let p = (a.pose.x + d * want.sin(), a.pose.z + d * want.cos());
        let act = route_go(&fauna, &view, &a, CH, p, ring, 0.0);
        let expected = if swing > 0.0 { (swing * V / (FRAC_PI_4.sin() * travel)).min(1.0) } else { 1.0 };
        assert!(expected < 1.0 || swing == 0.0, "fixture: the cap binds at swing {swing}");
        assert!((act.forward - expected).abs() < 1e-9, "swing {swing}: forward {} (expected {expected})", act.forward);
        let ahead = (a.pose.x + d, a.pose.z);
        let act = route_go(&fauna, &view, &a, CH, ahead, ring, 0.0);
        assert!((act.forward - 1.0).abs() < 1e-9, "swing {swing}, straight ahead: forward {}", act.forward);
    }
}

// ------------------------------------------------------------------ PIT: the way out

/// **The way out is off at `pit_search_m` 0**: a pack adult on the floor of a trench off its
/// route graph (room for its disc, not for the graph's margin; its floor two of its steps
/// down, its east end a stair of one step) holding a walk: with `pit_search_m` set,
/// `out_of_pit` replaces the walk with a way out, feed off; at 0 it leaves the walk as it is.
#[test]
fn the_way_out_is_off_at_pit_search_m_zero() {
    let top = 10;
    let c0 = awake();
    let step = c0.founder(CH).step_limits(V);
    assert!(step.up >= 1 && 2 * step.up < top, "fixture: the trench fits under the plateau");
    let r = c0.founder(CH).adult_body().footprint_radius();
    let cols = ((2.0 * (r + V)) / V).ceil() as u32;
    let z0 = 8u32;
    let mut world = ground(64, 24, 20, top);
    for x in 28..=35i64 {
        let floor = if x >= 34 { top - step.up } else { top - 2 * step.up };
        for z in z0..z0 + cols {
            for y in floor + 1..=top {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Air });
            }
        }
    }
    let floor = top - 2 * step.up;
    let zc = (f64::from(z0) + 0.5 * f64::from(cols)) * V;
    for pit in [4.0, 0.0] {
        let mut c = c0.clone();
        c.founding.clearance_margin_m = 0.5 * f64::from(cols) * V - r + V;
        let pk = c.founders[CH.index()].pack.as_mut().unwrap();
        pk.pit_search_m = pit;
        pk.pack_size = 1;
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        assert!(fauna.apply(&world, Command::FoundPack { den: Site { x: 10, y: top, z: 12 }, founder: CH, stores: StartingStores::FULL, heading_rad: EAST }));
        let id = fauna.animals.iter().find(|a| a.founder == Some(CH)).unwrap().id;
        {
            let a = body_mut(&mut fauna, id);
            a.pose = Pose { x: 29.5 * V, z: zc, heading_rad: EAST, y_m: crate::surface_m(floor, V) };
            a.site = Site { x: 29, y: floor, z: (zc / V).floor() as u32 };
        }
        let view = world.view();
        let graph = fauna.pack_routes(&view, CH);
        let a = body(&fauna, id);
        let geometry = fauna.config.founder(CH).body_at(a.body);
        let (own_r, _) = node_radii(&geometry, &fauna.config.founder(CH).hunter.unwrap(), V);
        assert!(graph.node_near(a.site, (a.pose.x, a.pose.z), own_r).is_none(), "fixture: the trench is off the graph");
        let i = fauna.animals.iter().position(|b| b.id == id).unwrap();
        let walk = Actions { forward: 1.0, turn: 0.0, feed: 1.0 };
        let out = out_of_pit(&mut fauna, i, &view, walk);
        if pit > 0.0 {
            let o = out.expect("a way out in reach");
            assert_eq!(o.feed, 0.0, "the way out holds no feed");
        } else {
            assert!(out.is_none(), "pit_search_m 0: the walk stands ({out:?})");
        }
    }
}
