//! **ROUTE-WALK: the walking route graph links the ground the motor walks**
//! (`design/handoffs/route-walk-2026-10-13.md`).
//!
//! In seed 9 of the Veilhound clone world a strip a voxel deep between the depth edge and a
//! wall, a little wider than the adult's disc, had no route-graph node: the graph judged
//! every face with the disc grown by the founding's clearance margin, at the face's centre,
//! while the motor walks the bare disc. Hounds walked the strip both ways; the graph cut the
//! den off from the basin beside it.
//!
//! The fixtures: a straight corridor between two walls, the narrowest odd number of columns
//! the lineage's bare disc fits (so its centre line runs on column centres), with a step
//! down halfway; and one a column narrower than the disc. The lines' clearance
//! (`FaunaConfig::route_line_clearance_m`) is set, from the geometry, so the grown disc does
//! not fit the corridor, as the founding's margin did not fit the strip. No shipped default
//! is asserted: each case takes its widths from the config's body and step.

use crate::{Actions, Activity, Command, Fauna, FaunaConfig, Founder, RouteGraph, Scripted, StartingStores};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use std::f64::consts::FRAC_PI_2;

const V: f64 = 0.125;
const WIDTH: u32 = 64;
const DEPTH: u32 = 32;
const HEIGHT: u32 = 24;
/// The floor's support layer west of the step.
const FLOOR: u32 = 4;
/// The corridor's columns, west and east ends, and the first column of the step down.
const XA: u32 = 24;
const XB: u32 = 40;
const XM: u32 = 32;
/// The corridor's first row.
const Z0: u32 = 12;
/// The seal's columns, across the whole strip.
const SEAL: std::ops::RangeInclusive<u32> = 54..=58;
const EAST: f64 = FRAC_PI_2;
const WEST: f64 = -FRAC_PI_2;

/// Every lineage awake; the walking graph on the bare disc; the lines' disc grown a quarter
/// voxel past a corridor `cols` wide.
fn config(f: Founder, cols: u32) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for g in Founder::ALL {
        c.founders[g.index()].active = Activity::Always;
    }
    let r = c.founder(f).adult_body().footprint_radius();
    c.route_clearance_m = 0.0;
    c.route_line_clearance_m = (0.5 * f64::from(cols) * V - r).max(0.0) + 0.25 * V;
    c
}

/// The narrowest odd column count the bare disc of `f`'s adult fits across, with room.
fn fit_cols(c: &FaunaConfig, f: Founder) -> u32 {
    let r = c.founder(f).adult_body().footprint_radius();
    let mut n = (2.0 * r / V).floor() as u32 + 1;
    if n % 2 == 0 {
        n += 1;
    }
    assert!(f64::from(n) * V > 2.0 * r + 1e-6, "fixture: the corridor has room for the disc");
    n
}

/// Open ground both ends, a corridor `cols` wide between walls from `XA` to `XB` (rows
/// `Z0..Z0 + cols`), its floor a voxel lower from `XM` east (the east plain with it).
fn corridor(c: &FaunaConfig, f: Founder, cols: u32) -> World {
    let step = c.founder(f).step_limits(V);
    assert!(step.up >= 1 && step.down >= 1, "fixture: a one-voxel step is within the lineage's step");
    let room = c.founder(f).adult_body().headroom_voxels(V);
    let mut w = World::empty(VoxelConfig {
        width: WIDTH,
        height: HEIGHT,
        depth: DEPTH,
        voxel_m: V,
        seed: 3,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    // Walls to the world's top (no face on them), and a seal across the strip far from the
    // corridor, so the ring's two plains meet only through it.
    let wall_top = HEIGHT - 1;
    assert!(FLOOR + room + step.up < wall_top, "fixture: the walls are past the lineage's step and headroom");
    for x in 0..WIDTH {
        let floor = if x >= XM { FLOOR - 1 } else { FLOOR };
        for z in 0..DEPTH {
            let in_walls = (XA..=XB).contains(&x) && (z < Z0 || z >= Z0 + cols);
            let top = if in_walls || SEAL.contains(&x) { wall_top } else { floor };
            for y in 1..=top {
                w.apply(WorldCommand::SetMaterial { x: i64::from(x), y, z, material: Material::Rock });
            }
        }
    }
    w
}

fn floor_at(x: u32) -> u32 {
    if x >= XM { FLOOR - 1 } else { FLOOR }
}

fn centre_row(cols: u32) -> u32 {
    Z0 + cols / 2
}

fn site(x: u32, z: u32) -> Site {
    Site { x, y: floor_at(x), z }
}

/// **A corridor the bare disc fits is linked end to end on the walking graph, and was not
/// on the grown disc's.** Its centre row is nodes from end to end, in one component with
/// both plains; the graph on the lines' disc (as the founding's margin judged the strip)
/// holds none of it and leaves the plains apart. Straight lines still keep the lines' slack:
/// none crosses the corridor.
#[test]
fn a_corridor_the_body_walks_is_linked_on_the_walking_graph() {
    for f in [Founder::Chorister, Founder::Browser] {
        let c0 = FaunaConfig::default();
        let cols = fit_cols(&c0, f);
        let c = config(f, cols);
        let world = corridor(&c, f, cols);
        let view = world.view();
        let phys = c.founder(f);
        let zc = centre_row(cols);
        let g = c.walking_graph(&view, phys);
        let west = g.face(site(XA - 4, zc)).expect("fixture: the west plain is on the graph");
        let east = g.face(site(XB + 4, zc)).expect("fixture: the east plain is on the graph");
        for x in XA..=XB {
            let i = g.face(site(x, zc)).unwrap_or_else(|| panic!("{f:?}: the corridor's centre row is a node at x = {x}"));
            assert_eq!(g.components[i], g.components[west], "{f:?}: x = {x} is in the plains' component");
        }
        assert_eq!(g.components[west], g.components[east], "{f:?}: the corridor links the plains");
        let grown = RouteGraph::for_founder(&view, phys, c.route_line_clearance_m);
        assert!((XA..=XB).all(|x| grown.face(site(x, zc)).is_none()), "{f:?}: fixture: the grown disc stands nowhere in the corridor");
        assert_ne!(grown.component_of(site(XA - 4, zc)), grown.component_of(site(XB + 4, zc)), "{f:?}: fixture: the grown disc's graph leaves the plains apart");
        let (a, b) = (g.centre(west), g.centre(east));
        assert!(!g.walk_line(west, a, b), "{f:?}: no straight line through the corridor (the lines keep their slack)");
    }
}

/// **A gap narrower than the body stays unlinked.** A column narrower than the disc, the
/// corridor holds no node and the plains stay apart.
#[test]
fn a_gap_narrower_than_the_body_stays_unlinked() {
    for f in [Founder::Chorister, Founder::Browser] {
        let c0 = FaunaConfig::default();
        let r = c0.founder(f).adult_body().footprint_radius();
        let cols = (2.0 * r / V).ceil() as u32 - 1;
        assert!(cols >= 1 && f64::from(cols) * V < 2.0 * r, "fixture: the gap is narrower than the disc");
        let c = config(f, cols);
        let world = corridor(&c, f, cols);
        let view = world.view();
        let g = c.walking_graph(&view, c.founder(f));
        for x in XA..=XB {
            for z in Z0..Z0 + cols {
                assert!(g.face(site(x, z)).is_none(), "{f:?}: no node in the gap at ({x}, {z})");
            }
        }
        let zc = Z0 + cols / 2;
        let (w, e) = (g.component_of(site(XA - 4, zc)), g.component_of(site(XB + 4, zc)));
        assert!(w.is_some() && e.is_some(), "fixture: the plains are on the graph");
        assert_ne!(w, e, "{f:?}: the gap does not link the plains");
    }
}

/// **The motor walks every edge the fix adds.** Each edge of the walking graph that the
/// grown disc's graph lacks is walked by the motor's own walk from centre to centre, both
/// ways; and a body driven straight along the corridor's centre row, east and then west,
/// steps through it, down the step and up it again, end to end.
#[test]
fn the_motor_walks_every_edge_the_fix_adds() {
    let f = Founder::Browser;
    let c0 = FaunaConfig::default();
    let cols = fit_cols(&c0, f);
    let c = config(f, cols);
    let world = corridor(&c, f, cols);
    let view = world.view();
    let phys = c.founder(f);
    let body = phys.adult_body();
    let (r, room, step, wade) = (body.footprint_radius(), body.headroom_voxels(V), phys.step_limits(V), phys.wade_depth_m(&body));
    let g = c.walking_graph(&view, phys);
    let grown = RouteGraph::for_founder(&view, phys, c.route_line_clearance_m);
    let mut added = 0;
    for (i, &a) in g.faces.iter().enumerate() {
        for &j in g.neighbours(i) {
            let b = g.faces[j];
            let old = grown.face(a).zip(grown.face(b)).is_some_and(|(p, q)| grown.neighbours(p).contains(&q));
            if old {
                continue;
            }
            added += 1;
            let (pa, pb) = (g.centre(i), g.centre(j));
            // Local coordinates: no edge here crosses the ring's seam.
            assert_eq!(crate::body::motor_walk(&view, pa, pb, a.y, r, wade, room, step), Some(b.y), "the motor walks {a:?} -> {b:?}");
        }
    }
    assert!(added > 0, "fixture: the fix adds edges");
    // A body driven along the centre row.
    let zc = centre_row(cols);
    let z = (f64::from(zc) + 0.5) * V;
    let reach = f64::from(XB - XA + 6) * V;
    let m = c.manifest(f);
    let bound = (3.0 * reach / m.cruise_m_per_s / DT).ceil() as usize + 10 * m.cadence_ticks().max(1) as usize;
    for (from, heading, to) in [(XA - 3, EAST, XB + 3), (XB + 3, WEST, XA - 3)] {
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let mut flora = Flora::new(FloraConfig::default());
        assert!(fauna.apply(&world, Command::IntroduceFounder { x: i64::from(from), z: zc, founder: f, stores: StartingStores::FULL, heading_rad: heading }));
        let id = fauna.animals.last().expect("just placed").id;
        {
            let a = fauna.animals.iter_mut().find(|a| a.id == id).expect("the body");
            (a.pose.x, a.pose.z, a.pose.heading_rad) = ((f64::from(from) + 0.5) * V, z, heading);
        }
        assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 1.0, turn: 0.0, feed: 0.0 }]))));
        let target = (f64::from(to) + 0.5) * V;
        let mut through = false;
        for _ in 0..bound {
            fauna.step(&world, &mut flora);
            let a = fauna.animals.iter().find(|a| a.id == id).expect("the body");
            if (heading > 0.0 && a.pose.x >= target) || (heading < 0.0 && a.pose.x <= target) {
                through = true;
                break;
            }
        }
        let a = fauna.animals.iter().find(|a| a.id == id).expect("the body");
        assert!(through, "driven {} it walks the corridor end to end within {bound} ticks (at x = {:.3}, z = {:.3}, {:?})", if heading > 0.0 { "east" } else { "west" }, a.pose.x, a.pose.z, a.site);
    }
}
