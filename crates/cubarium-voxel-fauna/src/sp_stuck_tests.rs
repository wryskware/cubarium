//! **SP-STUCK: no black holes** (Wrysk 2026-10-11: "theres like black holes on the map where
//! animals just get stuck and rotate around forever").
//!
//! The scene is a piece of the steep terraced vine slope seedporters stood on for hours, at
//! the world's own 0.125 m: a shelf `S` a voxel deep, its top `T` four layers over the floor
//! (three sides drop to it: past the step, short of a glide); behind it the next shelf `N`, a
//! voxel deep and two layers up (within the step), and behind that the plateau `P` one more
//! up, where the lineage's route graph is. The adult seedporter's disc is two voxels across,
//! so from `S` it cannot get onto `N` (its front reaches the plateau's riser, three layers
//! over where it stands), though the clearance rule at the faces' centres admits the step.
//! `N`'s riser over `S` is covered in latticevine.
//!
//! Every bound comes from the config (the drop wait, the controller period, the cruise, the
//! yaw cap); no default, ratio or verdict is asserted.

use super::{OffGraph, Trapped, off_graph_exit, reflex};
use crate::{Actions, Command, DropRule, Fauna, FaunaConfig, Founder, MobilityMode, RouteGraph, Scripted, StartingStores};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, TICK_HZ, World};
use cubarium_voxel_flora::{Face, FaceDir, Flora, FloraConfig, Site, VineSeed};
use std::f64::consts::PI;

const V: f64 = 0.125;
const SP: Founder = Founder::Seedporter;
/// The support face of a plain column: rock at `y = 1..=2`.
const FLOOR: u32 = 2;
/// The shelf's layer: four over the floor.
const T: u32 = FLOOR + 4;
/// The shelf's row and columns.
const SZ: u32 = 10;
const SX: std::ops::RangeInclusive<i64> = 20..=23;
/// Facing `+z` (`Pose::forward` is `(sin h, cos h)`): into the riser behind the shelf.
const NORTH: f64 = 0.0;

// ------------------------------------------------------------------ fixtures

fn rock(w: &mut World, xs: std::ops::RangeInclusive<i64>, zs: std::ops::RangeInclusive<u32>, ys: std::ops::RangeInclusive<u32>) {
    for z in zs {
        for x in xs.clone() {
            for y in ys.clone() {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
            }
        }
    }
}

fn plain() -> World {
    let mut w = World::empty(VoxelConfig {
        width: 48,
        height: 24,
        depth: 24,
        voxel_m: V,
        seed: 11,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    rock(&mut w, 0..=47, 0..=23, 1..=FLOOR);
    w
}

/// The terraced slope: `S` at `(20..=23, 10)` on layer `T`, `N` a voxel deep behind it two
/// layers up, the plateau `P` behind that one more up; `N`'s riser over `S` covered.
fn terraces() -> (World, Flora) {
    let mut w = plain();
    rock(&mut w, SX, SZ..=SZ, FLOOR + 1..=T);
    rock(&mut w, 8..=36, SZ + 1..=SZ + 1, FLOOR + 1..=T + 2);
    rock(&mut w, 8..=36, SZ + 2..=SZ + 10, FLOOR + 1..=T + 3);
    let flora = cover(&w, T + 2);
    (w, flora)
}

/// The control: behind `S` a wide plateau `rise` layers up, its riser over `S` covered.
fn wide_step(rise: u32) -> (World, Flora) {
    let mut w = plain();
    rock(&mut w, SX, SZ..=SZ, FLOOR + 1..=T);
    rock(&mut w, 8..=36, SZ + 1..=SZ + 10, FLOOR + 1..=T + rise);
    let flora = cover(&w, T + rise);
    (w, flora)
}

/// Latticevine on the riser over `S` (the faces of row `SZ + 1` looking `-z`), from `S`'s top
/// up to `top`.
fn cover(w: &World, top: u32) -> Flora {
    let mut flora = Flora::new(FloraConfig::default());
    let first = Face::new(21, T + 1, SZ + 1, FaceDir::NegZ);
    let id = flora
        .seed_vine(w, VineSeed { root: Site { x: 21, y: T, z: SZ }, first, reserve: 100.0, lineage: None })
        .expect("fixture: the vine founder");
    for y in T + 1..=top {
        for x in [21, 22, 20, 23] {
            let f = Face::new(x, y, SZ + 1, FaceDir::NegZ);
            if f != first {
                assert!(flora.cover_face(w, id, f, 1.0), "fixture: cover {f:?}");
            }
        }
    }
    flora
}

/// Every lineage awake at every hour; a short drop wait.
fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = crate::Activity::Always;
    }
    c.founders[SP.index()].drop.after_s = 1.0;
    c
}

/// An adult seedporter on `site` at `(x, z)`, facing `heading`, driven by its own heuristic.
fn place(fauna: &mut Fauna, world: &World, site: Site, (x, z): (f64, f64), heading: f64) -> u64 {
    assert!(fauna.apply(world, Command::IntroduceFounder { x: 2, z: 2, founder: SP, stores: StartingStores::FULL, heading_rad: heading }));
    let id = fauna.animals.last().expect("just placed").id;
    let k = fauna.animals.len() - 1;
    let a = &mut fauna.animals[k];
    (a.pose.x, a.pose.z, a.pose.heading_rad) = (x, z, heading);
    a.site = site;
    a.pose.y_m = crate::surface_m(site.y, V);
    let t = fauna.config.teachers;
    fauna.controllers.set(id, crate::controller::heuristic(SP, t));
    id
}

fn centre(x: i64, z: u32) -> (f64, f64) {
    ((x as f64 + 0.5) * V, (f64::from(z) + 0.5) * V)
}

/// A body on `S` pressed against the riser behind it, as one walking into it stands (its
/// disc's front at the plateau's riser).
fn pressed() -> (f64, f64) {
    (centre(21, SZ).0, f64::from(SZ + 1) * V - 1e-9)
}

fn shelf() -> Site {
    Site { x: 21, y: T, z: SZ }
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("the body")
}

/// The adult's walking geometry: (radius, headroom, step, wade depth, own columns).
fn geometry(fauna: &Fauna) -> (f64, u32, crate::StepLimits, f64, u32) {
    let p = fauna.config.founder(SP);
    let b = p.adult_body();
    let r = b.footprint_radius();
    (r, b.headroom_voxels(V), p.step_limits(V), p.wade_depth_m(&b), (r / V).ceil() as u32)
}

/// The time a drop may take once trapped: the wait, a controller period to see it, a half turn
/// at the yaw cap, a voxel at cruise, and two controller periods.
fn drop_bound_s(c: &FaunaConfig) -> f64 {
    let m = c.manifest(SP);
    let period = m.cadence_ticks().max(1) as f64 / f64::from(TICK_HZ);
    c.founder(SP).drop.after_s + period + PI / m.yaw_cap_rad_per_s + 2.0 * V / m.cruise_m_per_s + 2.0 * period
}

fn ticks(s: f64) -> usize {
    (s / DT).round() as usize
}

// ------------------------------------------------------------------ the law

/// **A body on the shelf with no walkable way off counts as cornered, drops within the trap
/// wait, and lands on ground it can walk away from.** Off its graph (the shelf is an island,
/// or no graph node at all, and the plateau's node beside it is a step its motor refuses), no
/// walking way back on, no glide: cornered. Within the wait and the walk to the edge it jumps
/// down, once, onto a face where it is on its graph again (EDGE-2 answers `On`: a node under
/// it it walks to, on no island).
#[test]
fn a_body_on_a_shelf_it_cannot_walk_off_drops_and_lands_where_it_can_walk_on() {
    let (world, mut flora) = terraces();
    let c = config();
    let glide = c.founder(SP).glider.expect("a glider").glide_min_height_m;
    assert!(f64::from(T - FLOOR) * V < glide, "fixture: the drops are short of a glide");
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let id = place(&mut fauna, &world, shelf(), pressed(), NORTH);
    let a = fauna.animals[index(&fauna, id)];
    assert_eq!(off_graph_exit(&mut fauna, &world.view(), &flora.view(), &a, (0.0, 0.0)), OffGraph::Cornered, "on the shelf it is cornered");
    let bound = drop_bound_s(&c);
    let mut landed = None;
    for _ in 0..ticks(bound) {
        fauna.step(&world, &mut flora);
        let a = fauna.animals[index(&fauna, id)];
        if a.site.y < T {
            landed = Some(a);
            break;
        }
    }
    let a = landed.unwrap_or_else(|| panic!("it jumps down within {bound:.2} s"));
    assert_eq!(fauna.view().ledger.drops_by_founder[SP.index()], 1, "one drop booked");
    assert_eq!(a.mobility.mode, MobilityMode::Ground, "a jump, not a glide");
    assert_eq!(off_graph_exit(&mut fauna, &world.view(), &flora.view(), &a, (0.0, 0.0)), OffGraph::On, "where it lands it walks on its graph ({:?})", a.site);
}

/// **The planner gives no `Way` up a step the motor refuses from the body's pose.** The
/// clearance rule at the faces' centres admits the step from `S` onto `N` (the old way back
/// on, which the stranded seedporters pressed at for hours), and `N` is on the graph; but
/// from where the body stands the motor's own walk to `N` is refused, and so is the way. The
/// control: behind a wide step two layers up, from the same pose, the way is that step.
#[test]
fn no_way_on_up_a_step_the_motor_refuses_from_where_the_body_stands() {
    let (world, _) = terraces();
    let mut fauna = Fauna::new(config());
    let (r, headroom, step, wade, own) = geometry(&fauna);
    let view = world.view();
    let g = fauna.routes(&view, SP, 1e6);
    let n = Site { x: 21, y: T + 2, z: SZ + 1 };
    let (nx, nz) = centre(21, SZ + 1);
    let mid = (nx, f64::from(SZ + 1) * V);
    let clear = |(x, z): (f64, f64), y| crate::body::pose_clear(&view, x, z, y, r, headroom, step);
    assert!(clear((nx, nz), n.y) && clear(mid, T) && clear(mid, n.y), "fixture: the clearance rule admits the step onto N");
    assert!(g.node_near(n, (nx, nz), own).is_some(), "fixture: N is by the graph");
    for at in [pressed(), centre(21, SZ)] {
        let walked = crate::body::motor_walk(&view, at, (nx, nz), T, r, wade, headroom, step);
        assert_ne!(walked, Some(n.y), "fixture: the motor refuses the step from {at:?}");
        let way = g.way_on(&view, shelf(), at, r, headroom, step, wade, own, 32);
        assert_eq!(way, None, "no way on from {at:?}");
    }
    // The control: a real step.
    let (world, _) = wide_step(2);
    let view = world.view();
    let mut fauna = Fauna::new(config());
    let g = fauna.routes(&view, SP, 1e6);
    let on = g.node_under(&view, shelf(), pressed(), r, headroom, step, wade, own);
    let way = g.way_on(&view, shelf(), pressed(), r, headroom, step, wade, own, 32);
    assert!(on.is_some() || way.is_some_and(|f| f.z == SZ + 1), "behind a real step it is on its graph or its way is the step ({on:?}, {way:?})");
}

/// **A stalled step at a covered riser it cannot top starts no vine climb.** Held walking into
/// the covered riser over `S` (DROP off), for a stretch of several of the old loop's 2–3 s
/// cycles, it never leaves its feet and books no climb. The control: the same covered riser,
/// three layers up to a wide top it can stand on and walk from, it climbs.
#[test]
fn a_stalled_step_at_a_riser_it_cannot_top_starts_no_climb() {
    let run = |(world, mut flora): (World, Flora), s: f64| {
        let mut c = config();
        c.founders[SP.index()].drop = DropRule { max_m: 0.0, ..c.founders[SP.index()].drop };
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let id = place(&mut fauna, &world, shelf(), centre(21, SZ), NORTH);
        assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 1.0, turn: 0.0, feed: 0.0 }]))));
        let mut aloft = 0;
        for _ in 0..ticks(s) {
            fauna.step(&world, &mut flora);
            if fauna.animals[index(&fauna, id)].mobility.mode != MobilityMode::Ground {
                aloft += 1;
            }
        }
        (aloft, fauna.view().ledger.climbs)
    };
    let (aloft, climbs) = run(terraces(), 30.0);
    assert_eq!((aloft, climbs), (0, 0), "no climb at a riser it cannot top");
    let (_, climbs) = run(wide_step(3), 5.0);
    assert!(climbs > 0, "fixture: it climbs a covered riser it can top");
}

/// **A brief cling does not reset the trapped clock; a long climb does.** Cornered on `S`, its
/// clock starts. Off its feet for under the wait and back, the clock still reads from when it
/// was first cornered, and runs out on time; off its feet longer than the wait, the clock
/// starts afresh.
#[test]
fn a_brief_cling_keeps_the_trapped_clock_and_a_long_climb_restarts_it() {
    let (world, flora) = terraces();
    let c = config();
    let wait = c.founder(SP).drop.after_s;
    let mut fauna = Fauna::new(c);
    let id = place(&mut fauna, &world, shelf(), pressed(), NORTH);
    let i = index(&fauna, id);
    let (view, fv) = (world.view(), flora.view());
    let at = |s: f64| (s / DT).round() as u64;
    fauna.tick = at(10.0);
    assert!(reflex(&mut fauna, i, &view, &fv).is_none(), "fixture: not yet due");
    let since = |f: &Fauna| f.trapped.get(&id).map(|t: &Trapped| (t.since, t.cornered));
    let t0 = fauna.tick;
    assert_eq!(since(&fauna), Some((t0, true)), "fixture: cornered, its clock started");
    let hop = |fauna: &mut Fauna, from_s: f64, for_s: f64| {
        fauna.animals[i].mobility.mode = MobilityMode::Climb;
        fauna.tick = at(from_s);
        let _ = reflex(fauna, i, &view, &fv);
        fauna.tick = at(from_s + for_s);
        let _ = reflex(fauna, i, &view, &fv);
        fauna.animals[i].mobility.mode = MobilityMode::Ground;
    };
    hop(&mut fauna, 10.0 + 0.2 * wait, 0.5 * wait);
    fauna.tick = at(10.0 + 0.8 * wait);
    assert!(reflex(&mut fauna, i, &view, &fv).is_none(), "not yet due");
    assert_eq!(since(&fauna).map(|s| s.0), Some(t0), "a brief cling keeps the clock");
    fauna.tick = t0 + at(wait);
    assert!(reflex(&mut fauna, i, &view, &fv).is_some(), "due on the first clock: it jumps");
    hop(&mut fauna, 20.0, 2.0 * wait);
    fauna.tick = at(20.0 + 2.0 * wait) + 1;
    let _ = reflex(&mut fauna, i, &view, &fv);
    assert_eq!(since(&fauna).map(|s| s.0), Some(fauna.tick), "a long climb restarts the clock");
}

/// **A body on ordinary ground with a real way on is unchanged.** Behind the shelf a wide
/// step it walks up; on that plateau, on the shelf and on the floor: the node it is on is the
/// plain nearest node, EDGE-2 answers `On`, there is no way back on to take, and driven by
/// its heuristic past the drop wait it never drops.
#[test]
fn a_body_on_open_ground_is_on_its_graph_and_never_drops() {
    let (world, mut flora) = wide_step(2);
    let c = config();
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let (r, headroom, step, wade, own) = geometry(&fauna);
    let spots = [
        (Site { x: 30, y: T + 2, z: SZ + 6 }, centre(30, SZ + 6)),
        (shelf(), centre(21, SZ)),
        (Site { x: 30, y: FLOOR, z: 4 }, centre(30, 4)),
    ];
    let ids: Vec<u64> = spots.iter().map(|&(s, at)| place(&mut fauna, &world, s, at, NORTH)).collect();
    for (&(site, at), &id) in spots.iter().zip(&ids) {
        let view = world.view();
        let g = fauna.routes(&view, SP, 1e6);
        let plain = g.node_near(site, at, own);
        assert!(plain.is_some(), "fixture: a node under {site:?}");
        assert_eq!(g.node_under(&view, site, at, r, headroom, step, wade, own), plain, "the same node");
        assert!(!g.island(plain.expect("a node"), RouteGraph::island_side(own)), "fixture: open ground");
        assert_eq!(g.way_on(&view, site, at, r, headroom, step, wade, own, 32), None, "no way back on: it is on");
        let a = fauna.animals[index(&fauna, id)];
        assert_eq!(off_graph_exit(&mut fauna, &view, &flora.view(), &a, (0.0, 0.0)), OffGraph::On);
    }
    for _ in 0..ticks(3.0 * drop_bound_s(&c)) {
        fauna.step(&world, &mut flora);
    }
    assert_eq!(fauna.view().ledger.drops_by_founder[SP.index()], 0, "no drop");
}
