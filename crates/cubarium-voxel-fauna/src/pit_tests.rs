//! **PIT**: a Veilhound does not walk into a hole it cannot climb out of, and one that is in
//! one walks out where a way out exists.
//!
//! The probe (seed 2, the desktop terrarium): every Veilhound wedged off its route graph was a
//! pup of the first litter, born where the breeder stood — off the graph, in a stream channel
//! or on a slope ledge, metres from the den — that walked straight at the den into a riser
//! and stood there for hours, growing into an adult the ledge no longer let out. An adult
//! walking home straight at a den in a covered pocket stepped down a slope off the graph too.
//!
//! The scene: a plateau with a **trench** across the straight line home — its near end a
//! stair one step down and one more to the floor, its far end a riser two steps tall, its
//! width room for the body but not for the route graph's disc (the body's footprint plus
//! `FaunaConfig::route_clearance_m`, which each case sets). Every case sets the config it
//! relies on and takes its bounds and the trench's dimensions from it (the step, the cruise,
//! the controller period); no shipped default, ratio or verdict is asserted.

use crate::{Activity, Animal, Command, Fauna, FaunaConfig, Founder, PackMember, Pose, StartingStores};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use std::f64::consts::FRAC_PI_2;

const CH: Founder = Founder::Chorister;
const V: f64 = 0.125;
const WIDTH: u32 = 160;
const DEPTH: u32 = 40;
const HEIGHT: u32 = 32;
/// The plateau's support layer.
const TOP: u32 = 16;
/// The trench's columns: x from `X0` to `X1`, z from `Z0` for `trench_cols` columns.
const X0: u32 = 56;
const X1: u32 = 80;
const Z0: u32 = 16;
const EAST: f64 = FRAC_PI_2;

/// The trench's width in columns: room for the body's disc with a margin of a voxel each side.
fn trench_cols(c: &FaunaConfig) -> u32 {
    let r = c.founder(CH).adult_body().footprint_radius();
    ((2.0 * (r + V)) / V).ceil() as u32
}

/// The z centre line of the trench, metres.
fn lane_z(c: &FaunaConfig) -> f64 {
    (f64::from(Z0) + 0.5 * f64::from(trench_cols(c))) * V
}

/// The plateau with the trench: its floor two of the chorister's steps under the plateau, its
/// two west columns a stair one step down, its east end the plateau again (a riser of two
/// steps). The clock is on at noon, so a nocturnal lineage wants its rest.
fn world(c: &FaunaConfig, seed: u64) -> World {
    let step = c.founder(CH).step_limits(V);
    assert!(step.up >= 1 && step.up == step.down, "the case needs a step up equal to the step down");
    assert!(2 * step.up < TOP, "the trench fits under the plateau");
    let mut w = World::empty(VoxelConfig {
        width: WIDTH,
        height: HEIGHT,
        depth: DEPTH,
        voxel_m: V,
        seed,
        day: DayClock { mode: DayMode::Cycle, start_hour: 12.0, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    for z in 0..DEPTH {
        for x in 0..i64::from(WIDTH) {
            for y in 1..=TOP {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    let cols = trench_cols(c);
    for z in Z0..Z0 + cols {
        for x in X0..=X1 {
            let floor = if x < X0 + 2 { TOP - step.up } else { TOP - 2 * step.up };
            for y in floor + 1..=TOP {
                w.apply(WorldCommand::SetMaterial { x: i64::from(x), y, z, material: Material::Air });
            }
        }
    }
    w
}

/// The chorister awake by night (so at noon it wants its rest), a pack of one, the den radius
/// `den_radius_m`, the route graph's margin wide enough that the trench holds no node, the pit
/// search out to `pit_search_m`.
fn config(pit_search_m: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.founders[CH.index()].active = Activity::Nocturnal;
    let r = c.founder(CH).adult_body().footprint_radius();
    let half = 0.5 * f64::from(trench_cols(&c)) * V;
    c.route_clearance_m = half - r + V;
    let pk = c.founders[CH.index()].pack.as_mut().expect("the chorister's pack table");
    pk.pack_size = 1;
    pk.den_radius_m = 0.5;
    pk.home_reserve_share = 1.0;
    pk.pit_search_m = pit_search_m;
    c
}

fn animal(fauna: &Fauna, id: u64) -> Animal {
    *fauna.view().animal(id).expect("alive")
}

fn animal_mut(fauna: &mut Fauna, id: u64) -> &mut Animal {
    fauna.animals.iter_mut().find(|a| a.id == id).expect("alive")
}

/// Put a body on the support face under `(x_m, z_m)` nearest `y` (the plateau or the floor).
fn place(fauna: &mut Fauna, world: &World, id: u64, x_m: f64, z_m: f64, y: u32) {
    let site = Site { x: (x_m / V).floor() as u32, y, z: (z_m / V).floor() as u32 };
    assert!(world.view().is_support(i64::from(site.x), site.y, site.z), "{site:?} is a support face");
    let a = animal_mut(fauna, id);
    a.pose = Pose { x: x_m, z: z_m, heading_rad: EAST, y_m: crate::surface_m(y, V) };
    a.site = site;
    a.founder_state.held = crate::controller::Actions::REST;
}

/// The pack's den on the plateau east of the trench, on its centre line; the one adult.
fn pack(fauna: &mut Fauna, world: &World, c: &FaunaConfig) -> (Site, u64, u64) {
    let den = Site { x: X1 + 12, y: TOP, z: (lane_z(c) / V).floor() as u32 };
    assert!(fauna.apply(world, Command::FoundPack { den, founder: CH, stores: StartingStores::FULL, heading_rad: EAST }));
    let p = fauna.packs()[0].id;
    let adult = fauna.animals.iter().find(|a| a.founder == Some(CH)).expect("founded").id;
    let teachers = fauna.config.teachers;
    fauna.controllers.set(adult, crate::controller::heuristic(CH, teachers));
    (den, p, adult)
}

fn to_den(fauna: &Fauna, id: u64, den: Site) -> f64 {
    let p = animal(fauna, id).pose;
    let (cx, cz) = ((f64::from(den.x) + 0.5) * V, (f64::from(den.z) + 0.5) * V);
    let (dx, dz) = crate::pose::wrapped_delta(p.x, p.z, cx, cz, f64::from(WIDTH) * V);
    (dx * dx + dz * dz).sqrt()
}

/// The walk's bound from the config: `metres` at cruise, three times over, and ten controller
/// periods.
fn bound(fauna: &Fauna, metres: f64) -> u32 {
    let m = fauna.config.manifest(CH);
    cubarium_voxel::ticks_at_least_one(3.0 * metres / m.cruise_m_per_s + 10.0 * m.controller_period_s) as u32
}

/// The case's premise: the trench holds no node of the route graph, and the plateau by it does.
fn assert_trench_off_the_graph(fauna: &Fauna, world: &World, c: &FaunaConfig) {
    let g = fauna.config.walking_graph(&world.view(), fauna.config.founder(CH));
    let zc = (lane_z(c) / V).floor() as u32;
    let step = c.founder(CH).step_limits(V);
    assert!(g.face(Site { x: X1 - 2, y: TOP - 2 * step.up, z: zc }).is_none(), "the trench floor is off the graph");
    assert!(g.face(Site { x: X0 - 8, y: TOP, z: zc }).is_some(), "the plateau is on it");
}

/// Walk `id` for `max` ticks: whether it came within the den radius, and the lowest layer it
/// stood on.
fn walk_home(fauna: &mut Fauna, world: &World, flora: &mut Flora, id: u64, den: Site, max: u32) -> (Option<u32>, u32) {
    let radius = fauna.config.founder(CH).pack.unwrap().den_radius_m;
    let mut lowest = animal(fauna, id).site.y;
    for t in 1..=max {
        fauna.step_with(world, flora, 1);
        let a = animal(fauna, id);
        lowest = lowest.min(a.site.y);
        if to_den(fauna, id, den) <= radius {
            return (Some(t), lowest);
        }
    }
    (None, lowest)
}

/// **A walker beside a pit deeper than it can climb out of does not step into it**: an adult
/// wanting its rest stands on the plateau before the trench's stair, its den straight across
/// the trench. It walks home without standing below the plateau, within the bound of a walk
/// round the trench. (Its straight walk took the stair down into the trench, and the riser at
/// the far end held it.)
#[test]
fn a_walker_beside_a_pit_it_cannot_climb_out_of_does_not_step_into_it() {
    let c = config(4.0);
    let world = world(&c, 501);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    assert_trench_off_the_graph(&fauna, &world, &c);
    let (den, _, adult) = pack(&mut fauna, &world, &c);
    let start = (f64::from(X0) - 6.0) * V;
    place(&mut fauna, &world, adult, start, lane_z(&c), TOP);
    let round = (f64::from(den.x) * V - start) + 2.0 * f64::from(trench_cols(&c)) * V + 2.0;
    let max = bound(&fauna, round);
    let (home, lowest) = walk_home(&mut fauna, &world, &mut flora, adult, den, max);
    let a = animal(&fauna, adult);
    assert_eq!(lowest, TOP, "it stepped down into the trench (at {:.2}, {:.2} on layer {})", a.pose.x, a.pose.z, a.site.y);
    assert!(home.is_some(), "not home within {max} ticks: {:.2} m from the den at ({:.2}, {:.2})", to_den(&fauna, adult, den), a.pose.x, a.pose.z);
}

/// **A body in a pit walks out where a way out exists**: the adult stands on the trench's
/// floor at its far end, facing the riser with its den beyond it. It walks back along the
/// trench, up the stair, and home round the trench within the bound of that walk. (It pressed
/// into the riser toward the den, and the refused motor's escape pressed on.)
#[test]
fn a_body_in_a_pit_walks_out_where_a_way_out_exists() {
    let c = config(6.0);
    let world = world(&c, 502);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    assert_trench_off_the_graph(&fauna, &world, &c);
    let (den, _, adult) = pack(&mut fauna, &world, &c);
    let step = c.founder(CH).step_limits(V);
    let x = (f64::from(X1) - 0.5) * V;
    place(&mut fauna, &world, adult, x, lane_z(&c), TOP - 2 * step.up);
    let out = 2.0 * (x - f64::from(X0) * V) + (f64::from(den.x) - f64::from(X1)) * V + 2.0 * f64::from(trench_cols(&c)) * V + 2.0;
    let max = bound(&fauna, out);
    let (home, _) = walk_home(&mut fauna, &world, &mut flora, adult, den, max);
    let a = animal(&fauna, adult);
    assert!(home.is_some(), "still in or by the trench after {max} ticks: at ({:.2}, {:.2}) on layer {}, {:.2} m from the den", a.pose.x, a.pose.z, a.site.y, to_den(&fauna, adult, den));
}

/// **The entry (PIT's probe): a pup away from its den walks home round a pit, not into it**: a
/// pup on the plateau before the trench's stair, its den straight across the trench, the adult
/// asleep at the den. It reaches the den without standing below the plateau, within the bound
/// of a walk round the trench. (Its straight walk home took the stair down, and it stood at the
/// riser for good.)
#[test]
fn a_pup_away_from_its_den_walks_home_round_a_pit() {
    let mut c = config(4.0);
    c.founders[CH.index()].active = Activity::Always;
    let world = world(&c, 503);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    assert_trench_off_the_graph(&fauna, &world, &c);
    let (den, p, adult) = pack(&mut fauna, &world, &c);
    let x_den = (f64::from(den.x) + 0.5) * V;
    place(&mut fauna, &world, adult, x_den, lane_z(&c), TOP);
    assert!(fauna.apply(
        &world,
        Command::IntroduceFounderOnFace {
            site: Site { x: X0 - 6, y: TOP, z: (lane_z(&c) / V).floor() as u32 },
            founder: CH,
            stores: StartingStores { body: 0.3, reserve: 0.2 },
            heading_rad: EAST,
        }
    ));
    let pup = fauna.animals.last().expect("introduced").id;
    animal_mut(&mut fauna, pup).pack = Some(PackMember { pack: p, pup: true, calling_ticks: 0 });
    let teachers = fauna.config.teachers;
    fauna.controllers.set(pup, crate::controller::heuristic(CH, teachers));
    let start = (f64::from(X0) - 6.0) * V;
    place(&mut fauna, &world, pup, start, lane_z(&c), TOP);
    let round = (x_den - start) + 2.0 * f64::from(trench_cols(&c)) * V + 2.0;
    let max = bound(&fauna, round);
    let radius = 0.5 * c.founder(CH).pack.unwrap().den_radius_m;
    let mut lowest = TOP;
    let mut home = None;
    for t in 1..=max {
        fauna.step_with(&world, &mut flora, 1);
        lowest = lowest.min(animal(&fauna, pup).site.y);
        if to_den(&fauna, pup, den) <= radius {
            home = Some(t);
            break;
        }
    }
    let a = animal(&fauna, pup);
    assert_eq!(lowest, TOP, "the pup stepped down into the trench (at {:.2}, {:.2} on layer {})", a.pose.x, a.pose.z, a.site.y);
    assert!(home.is_some(), "the pup is not home within {max} ticks: {:.2} m from the den at ({:.2}, {:.2})", to_den(&fauna, pup, den), a.pose.x, a.pose.z);
}
