//! **SNAIL-STUCK: ripple snails turning for hours in route pockets** (Wrysk 2026-10-11:
//! "black holes on the map where animals just get stuck and rotate around forever"). In the
//! eight-seed check after SP-STUCK-2 the snails still did it: juveniles in a flooded slot one
//! voxel tall under a rock shelf (seed 1, about (40.4, 8.95) m) and beside a trench or a wall
//! (seed 4), turning in place for hours.
//!
//! The snail's own walks — the giving-up density's walk to richer film, the roam onto land,
//! the trip ashore to lay, the thirsty walk to water — picked a face by a world query and
//! steered straight at its centre, wherever it was. A shelf's top over the snail's own column
//! is a different face from the one it stands on, a few millimetres away across the plane, so
//! its film per metre beat every other face and the snail turned round its own centre; a face
//! over a trench or up a wall it could not take straight held it pressing and turning back.
//!
//! With those walks routed, one more held a snail: in a dead end under a shelf its hungry
//! search's leg pointed into the end wall, its motor turned it round from the face (the shelf
//! over it), and the leg, never told, turned it back into the face (seed 4, body 322).
//!
//! Each fixture is a hand-built world at the world's own 0.125 m, under water, the day clock
//! off (the snail's roam hours). Every bound comes from the config (the cruise, the climbing
//! pace, the controller period); no default, ratio or verdict is asserted.

use crate::{Command, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, TICK_HZ, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const V: f64 = 0.125;
const FLOOR: u32 = 2;
const SNAIL: Founder = Founder::RippleSnail;
const W: u32 = 16;
const D: u32 = 8;
/// The shelf: rock at layers `FLOOR + 2 ..= FLOOR + 3` over columns `0..=SHELF_X1` on every
/// `z`, leaving the slot (layer `FLOOR + 1`) one voxel tall over the floor.
const SHELF_X1: u32 = 7;
const SHELF_TOP: u32 = FLOOR + 3;
/// Water stands to this layer everywhere it can.
const WATER_TOP: u32 = FLOOR + 4;

fn rock(w: &mut World, xs: std::ops::RangeInclusive<i64>, zs: std::ops::RangeInclusive<u32>, ys: std::ops::RangeInclusive<u32>) {
    for z in zs {
        for x in xs.clone() {
            for y in ys.clone() {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
            }
        }
    }
}

/// The flooded slot: a rock floor, the shelf over half the ring, water over everything to
/// `WATER_TOP` (the slot full, a cell over the shelf's top, four over the open floor).
fn slot_world() -> World {
    let mut w = World::empty(VoxelConfig {
        width: W,
        height: 10,
        depth: D,
        voxel_m: V,
        seed: 5,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    rock(&mut w, 0..=i64::from(W) - 1, 0..=D - 1, 1..=FLOOR);
    rock(&mut w, 0..=i64::from(SHELF_X1), 0..=D - 1, FLOOR + 2..=SHELF_TOP);
    let cell = w.config().voxel_volume();
    for z in 0..D {
        for x in 0..i64::from(W) {
            for y in FLOOR + 1..=WATER_TOP {
                if !w.view().is_solid(x, y, z) {
                    w.apply(WorldCommand::AddWater { x, y, z, volume_m3: cell });
                }
            }
        }
    }
    let v = w.view();
    assert!(v.standing_depth_m(3, FLOOR, 3) > 0.0, "fixture: the slot is flooded");
    assert!(v.standing_depth_m(3, SHELF_TOP, 3) > 0.0, "fixture: water over the shelf's top");
    assert!(v.standing_depth_m(12, FLOOR, 3) > 0.0, "fixture: water over the open floor");
    assert_eq!(v.highest_support_in(3, 3, 0, 9), Some(SHELF_TOP), "fixture: the shelf's top is its column's highest face");
    w
}

fn under_shelf(s: Site) -> bool {
    s.y == FLOOR && s.x <= SHELF_X1
}

/// Film at the full bed on every wet face, then every slot face grazed to the film's refuge:
/// the snail's face tastes of nothing and the shelf's top over it holds the most.
fn grazed_slot(world: &World) -> Flora {
    let fc = FloraConfig { film_seed_per_m2: FloraConfig::default().film_capacity_per_m2, ..FloraConfig::default() };
    let mut flora = Flora::new(fc);
    assert!(flora.seed_film(world) > 0, "fixture: film on the flooded faces");
    for z in 0..D {
        for x in 0..=SHELF_X1 {
            let _ = flora.take_film(Site { x, y: FLOOR, z }, 1.0e9);
        }
    }
    assert!(flora.view().edible_film_at(Site { x: 4, y: SHELF_TOP, z: 4 }) > 0.0, "fixture: film on the shelf's top");
    assert_eq!(flora.view().edible_film_at(Site { x: 4, y: FLOOR, z: 4 }), 0.0, "fixture: the slot grazed out");
    flora
}

fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    // No laying here: a layer's trip ashore is its own walk.
    c.founders[SNAIL.index()].core.reproduction.surplus_hold_s = 1.0e9;
    c
}

/// A hungry snail of body `body` (an empty reserve, as the stuck ones had) on slot face
/// `(x, FLOOR, z)` at `(px, pz)`, facing `heading`, driven by its own heuristic.
fn place(fauna: &mut Fauna, world: &World, body: f64, (x, z): (u32, u32), (px, pz): (f64, f64), heading: f64) -> u64 {
    assert!(fauna.apply(world, Command::IntroduceFounder { x: 12, z: 4, founder: SNAIL, stores: StartingStores::FULL, heading_rad: heading }));
    let k = fauna.animals.len() - 1;
    let id = fauna.animals[k].id;
    let a = &mut fauna.animals[k];
    (a.pose.x, a.pose.z, a.pose.heading_rad) = (px, pz, heading);
    a.site = Site { x, y: FLOOR, z };
    a.body = body;
    a.reserve = 0.0;
    a.pose.y_m = crate::surface_m(FLOOR, V);
    let t = fauna.config.teachers;
    fauna.controllers.set(id, crate::controller::heuristic(SNAIL, t));
    id
}

fn period_s(c: &FaunaConfig) -> f64 {
    c.manifest(SNAIL).cadence_ticks().max(1) as f64 / f64::from(TICK_HZ)
}

/// **A hungry snail under a shelf walks out of the slot** (seed 1's pocket). Its face is
/// grazed out and the shelf's top over its own column holds film; a few millimetres off its
/// column's centre, it once turned round that centre for hours. On its heuristic it now
/// leaves the slot — out at the shelf's edge, up onto the shelf or along the open floor — in
/// a few times the walk there and the climb up at its own pace.
#[test]
fn a_hungry_snail_under_a_shelf_walks_out_of_the_slot() {
    let world = slot_world();
    for (body, heading) in [(1.0, -std::f64::consts::FRAC_PI_2), (0.25, 0.0)] {
        let mut flora = grazed_slot(&world);
        let mut fauna = Fauna::new(config());
        // Column 6 of the slot, 1 cm off its centre: two columns from the shelf's east edge.
        let home = (6, 4);
        let id = place(&mut fauna, &world, body, home, ((6.5 * V) + 0.01, (4.5 * V) - 0.004), heading);
        let c = fauna.config().clone();
        let m = c.manifest(SNAIL);
        let phys = c.founder(SNAIL);
        let cruise = m.cruise_m_per_s * phys.body_at(body).length_m / phys.body_at(1.0).length_m;
        assert!(cruise > 0.0, "fixture: it walks");
        // The walk to the shelf's edge and the climb up its side, at its own paces.
        let walk = (f64::from(SHELF_X1 + 1) - 6.5) * V / cruise;
        let climb = f64::from(SHELF_TOP - FLOOR) * V / (phys.climb_speed_fraction * cruise);
        let bound = 4.0 * (walk + climb) + 4.0 * period_s(&c);
        let ticks = (bound / DT).ceil() as usize;
        let mut out = None;
        for t in 0..ticks {
            fauna.step(&world, &mut flora);
            let a = fauna.view().animal(id).expect("alive").clone();
            if t == 4 * c.manifest(SNAIL).cadence_ticks() as usize {
                assert!(a.eating, "fixture: it wants food");
            }
            if !under_shelf(a.site) && a.mobility.climb.is_none() {
                out = Some(t);
                break;
            }
        }
        let a = fauna.view().animal(id).expect("alive").clone();
        assert!(
            out.is_some(),
            "body {body}: still under the shelf after {bound:.0} s at {:?} ({:.3}, {:.3}), heading {:.2}",
            a.site,
            a.pose.x,
            a.pose.z,
            a.pose.heading_rad
        );
    }
}

// ------------------------------------------------------------------ a dead end under a shelf

/// The dead end (seed 4's body 322): a corridor three columns wide along `x` at `z = 3..=5`, open to
/// the west onto the open floor, its floor under water. A shelf over it at `FLOOR + 3` leaves
/// two layers of room; at its east end (column `END_X + 1`) a wall stands to `FLOOR + 3`. A
/// snail pressing east at the end meets a face it would climb, with the shelf over it: its
/// motor turns it round.
const END_X: u32 = 10;
const CORRIDOR_X0: u32 = 4;

fn dead_end_world() -> World {
    let mut w = World::empty(VoxelConfig {
        width: W,
        height: 10,
        depth: D,
        voxel_m: V,
        seed: 7,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    rock(&mut w, 0..=i64::from(W) - 1, 0..=D - 1, 1..=FLOOR);
    let (x0, x1) = (i64::from(CORRIDOR_X0), i64::from(END_X));
    // Its sides: rock to the shelf's top on both sides of the corridor, the corridor's length.
    rock(&mut w, x0..=x1 + 1, 0..=2, FLOOR + 1..=FLOOR + 3);
    rock(&mut w, x0..=x1 + 1, 6..=D - 1, FLOOR + 1..=FLOOR + 3);
    // The shelf over the corridor, and the wall closing its east end.
    rock(&mut w, x0..=x1, 3..=5, FLOOR + 3..=FLOOR + 3);
    rock(&mut w, x1 + 1..=x1 + 1, 3..=5, FLOOR + 1..=FLOOR + 3);
    let cell = w.config().voxel_volume();
    for z in 0..D {
        for x in 0..i64::from(W) {
            for y in FLOOR + 1..=FLOOR + 2 {
                if !w.view().is_solid(x, y, z) {
                    w.apply(WorldCommand::AddWater { x, y, z, volume_m3: cell });
                }
            }
        }
    }
    let v = w.view();
    assert_eq!(
        crate::body::wall_ascent(&v, i64::from(END_X), FLOOR, 4, (1, 0), 1, 2),
        crate::body::Face::Blocked,
        "fixture: the end wall turns a body back under the shelf"
    );
    w
}

fn in_corridor(s: Site) -> bool {
    (3..=5).contains(&s.z) && (CORRIDOR_X0..=END_X).contains(&s.x)
}

/// **A hungry snail searching at a dead end under a shelf walks out of it.** No film
/// anywhere, so nothing leads it and its search runs on legs; facing the end wall, its first
/// leg points into the face, and its motor turns it round there. A face that turns it round
/// is in its way, as an obstacle its heuristic answers is: the leg ends and the next turns
/// from it. It once turned back into the face for minutes. Bounded by its patience, the
/// corridor's length at its own pace and two legs.
#[test]
fn a_hungry_snail_searching_at_a_dead_end_under_a_shelf_walks_out() {
    let world = dead_end_world();
    let mut flora = Flora::new(FloraConfig::default());
    // It searches on legs at once (its patience spent), so its first leg points the way it
    // faces: into the end wall.
    let mut cfg = config();
    cfg.teachers.search_patience_s = 0.0;
    let mut fauna = Fauna::new(cfg);
    let east = std::f64::consts::FRAC_PI_2;
    let body = 0.25;
    let r = fauna.config().founder(SNAIL).body_at(body).footprint_radius();
    // At the end, pressed toward the wall's plane.
    let id = place(&mut fauna, &world, body, (END_X, 4), (f64::from(END_X + 1) * V - r - 0.002, 4.5 * V), east);
    let c = fauna.config().clone();
    let phys = c.founder(SNAIL);
    let cruise = c.manifest(SNAIL).cruise_m_per_s * phys.body_at(body).length_m / phys.body_at(1.0).length_m;
    let length = phys.body_at(body).length_m;
    let leg_s = 2.0 * c.teachers.search_leg_bl * length / cruise;
    let walk = f64::from(END_X - CORRIDOR_X0 + 1) * V / cruise;
    let bound = c.teachers.search_patience_s + 4.0 * walk + 2.0 * leg_s + 4.0 * period_s(&c);
    let ticks = (bound / DT).ceil() as usize;
    let mut out = None;
    for t in 0..ticks {
        fauna.step(&world, &mut flora);
        let a = fauna.view().animal(id).expect("alive").clone();
        if !in_corridor(a.site) {
            out = Some(t);
            break;
        }
    }
    let a = fauna.view().animal(id).expect("alive").clone();
    assert!(
        out.is_some(),
        "still in the dead end after {bound:.0} s at {:?} ({:.3}, {:.3}), heading {:.2}",
        a.site,
        a.pose.x,
        a.pose.z,
        a.pose.heading_rad
    );
}
