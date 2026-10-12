//! **DROP**: a trapped walker jumps down, at some risk (Wrysk, 2026-10-06: "Should a trapped
//! animal be able to jump down a bigger drop than it can normally step, at some risk of
//! injury?" — "yes").
//!
//! A body is trapped when it stands off its lineage's route graph with no walking way back
//! on (`RouteGraph::way_on`) and no glide off (EDGE-2's `off_graph_exit` answers
//! `Cornered`), for its lineage's `drop.after_s`. It then jumps down past its step, at most
//! its `drop.max_m` (× its height over the adult's), the lowest drop that lands where its
//! graph is reachable, and takes `drop.wound_per_m` per metre beyond its `step_down_m`.
//!
//! Each case builds its scene by hand at 0.25 m (EDGE-2's ledge) and sets the drop rule it
//! relies on; its bounds come from that config (the controller period, the yaw cap, the
//! cruise). No default, ratio or verdict is asserted.

use super::predation_fixture::{animal, animal_mut};
use crate::{Activity, Command, DropRule, Fauna, FaunaConfig, Founder, MobilityMode, StartingStores};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, TICK_HZ, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use std::f64::consts::{FRAC_PI_2, PI};

const V: f64 = 0.25;
const SP: Founder = Founder::Seedporter;
const WEST: f64 = -FRAC_PI_2;
/// The support face of a plain column: soil at `y = 1..=2`.
const FLOOR: u32 = 2;
/// A young seedporter's stores: a third of the adult's structure.
const YOUNG: StartingStores = StartingStores { body: 0.3, reserve: 0.3 };

// ------------------------------------------------------------------ fixtures

fn plain(width: u32, depth: u32, height: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m: V,
        seed: 11,
        day: DayClock { start_hour: 12.0, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=FLOOR {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
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

/// Every lineage awake at every hour; wounds do not heal (so a drop's wound reads exactly).
fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c.predation.wound_heal_per_s = 0.0;
    c
}

fn centre(x: i64, z: u32) -> (f64, f64) {
    ((x as f64 + 0.5) * V, (f64::from(z) + 0.5) * V)
}

fn ticks(s: f64) -> usize {
    (s * f64::from(TICK_HZ)).round() as usize
}

/// EDGE-2's **ledge**: a face at `(20, 8)` with rock walls high over it east and north (so
/// the adult disc its lineage's route graph is built for does not stand on it); west lies a
/// platform `drop_w` layers below it, south (`(20, 4..=7)`) the floor, `h` layers below it.
/// Its layer.
fn ledge(world: &mut World, h: u32, drop_w: u32) -> u32 {
    let top = FLOOR + h;
    let low = top - drop_w;
    rock(world, 14..=19, 4..=8, FLOOR + 1..=low);
    rock(world, 20..=20, 8..=8, FLOOR + 1..=top);
    rock(world, 21..=23, 4..=12, FLOOR + 1..=top + 6);
    rock(world, 14..=23, 9..=11, FLOOR + 1..=top + 6);
    top
}

/// A young seedporter centred on the ledge, facing west, driven by its own heuristic; its id.
fn on_ledge(fauna: &mut Fauna, world: &World, top: u32) -> u64 {
    let (x, z) = (4i64, 2u32);
    assert!(fauna.apply(world, Command::IntroduceFounder { x, z, founder: SP, stores: YOUNG, heading_rad: WEST }));
    let id = fauna.animals.last().expect("just placed").id;
    let a = animal_mut(fauna, id);
    let (px, pz) = centre(20, 8);
    a.pose.x = px;
    a.pose.z = pz;
    a.pose.heading_rad = WEST;
    a.site = Site { x: 20, y: top, z: 8 };
    a.pose.y_m = crate::surface_m(top, V);
    let t = fauna.config.teachers;
    fauna.controllers.set(id, crate::controller::heuristic(SP, t));
    id
}

/// The young body's height over the adult's: what its drop reach is scaled by.
fn size_ratio(fauna: &Fauna, id: u64) -> f64 {
    let p = fauna.config.founder(SP);
    p.body_at(animal(fauna, id).body).height_m / p.adult_height_m
}

/// Whether, from `site`, body `id` reaches its route graph: on it, or by its own walking way.
fn graph_reachable(fauna: &mut Fauna, world: &World, id: u64, site: Site) -> bool {
    let a = animal(fauna, id);
    let p = *fauna.config.founder(SP);
    let body = p.body_at(a.body);
    let r = body.footprint_radius();
    let own = (r / V).ceil() as u32;
    let wv = world.view();
    let g = fauna.routes(&wv, SP, 1e6);
    let c = centre(i64::from(site.x), site.z);
    g.node_near(site, c, own).is_some() || g.way_on(&wv, site, c, r, body.headroom_voxels(V), p.step_limits(V), p.wade_depth_m(&body), own, 64).is_some()
}

/// The fixture's claims: the young body is off its graph on the ledge, and whether its own
/// body has a walking way back on is `way`.
fn assert_off_graph(fauna: &mut Fauna, world: &World, id: u64, way: bool) {
    let a = animal(fauna, id);
    let p = *fauna.config.founder(SP);
    let body = p.body_at(a.body);
    let r = body.footprint_radius();
    let own = (r / V).ceil() as u32;
    let wv = world.view();
    let g = fauna.routes(&wv, SP, 1e6);
    assert!(g.node_near(a.site, (a.pose.x, a.pose.z), own).is_none(), "fixture: the ledge is off the route graph");
    let w = g.way_on(&wv, a.site, (a.pose.x, a.pose.z), r, body.headroom_voxels(V), p.step_limits(V), p.wade_depth_m(&body), own, 64);
    assert_eq!(w.is_some(), way, "fixture: a walking way back on is {way}");
}

/// The time the drop may take once trapped: the rule's wait, a controller period to see it,
/// a half turn at the yaw cap, a voxel at cruise, and two controller periods.
fn drop_bound_s(c: &FaunaConfig) -> f64 {
    let m = c.manifest(SP);
    let period = m.cadence_ticks().max(1) as f64 / f64::from(TICK_HZ);
    c.founder(SP).drop.after_s + period + PI / m.yaw_cap_rad_per_s + V / m.cruise_m_per_s + 2.0 * period
}

/// Run `id` for `s` seconds; the layer of the first face it left the ledge to, if it did, and
/// the deepest single-tick fall in layers it made.
fn run(fauna: &mut Fauna, world: &World, flora: &mut Flora, id: u64, top: u32, s: f64) -> (Option<Site>, u32) {
    let mut deepest = 0;
    let mut y = animal(fauna, id).site.y;
    for _ in 0..ticks(s) {
        fauna.step(world, flora);
        let Some(a) = fauna.view().animal(id).copied() else { break };
        deepest = deepest.max(y.saturating_sub(a.site.y));
        y = a.site.y;
        if a.site.y < top {
            return (Some(a.site), deepest);
        }
    }
    (None, deepest)
}

fn the_glide_is_out_of_reach(c: &FaunaConfig, h: u32) {
    let glide = c.founder(SP).glider.expect("a glider").glide_min_height_m;
    assert!(f64::from(h) * V < glide, "fixture: every drop is short of a glide");
}

// ------------------------------------------------------------------ the drop

/// **A body in a pocket with no way out jumps down** within the rule's wait plus a controller
/// period (and the turn and walk to the edge), onto a face from which its graph is reachable:
/// the lower of the two drops on offer (the platform west, 2 layers, over the floor south, 3).
#[test]
fn a_trapped_body_jumps_down_and_lands_where_its_graph_is_reachable() {
    let mut world = plain(48, 16, 24);
    let mut c = config();
    let down = c.founder(SP).step_limits(V).down;
    let (h, w) = (down + 2, down + 1);
    the_glide_is_out_of_reach(&c, h);
    let top = ledge(&mut world, h, w);
    c.founders[SP.index()].drop = DropRule { after_s: 1.0, max_m: 1e3, wound_per_m: 0.0 };
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = on_ledge(&mut fauna, &world, top);
    assert_off_graph(&mut fauna, &world, sp, false);
    let bound = drop_bound_s(&c);
    let (landed, _) = run(&mut fauna, &world, &mut flora, sp, top, bound);
    let land = landed.unwrap_or_else(|| panic!("it jumps down within {bound:.2} s"));
    assert_eq!(animal(&fauna, sp).mobility.mode, MobilityMode::Ground, "a jump, not a glide");
    assert_eq!(land.y, top - w, "the lowest drop on offer (the platform west), not the floor ({land:?})");
    assert!(graph_reachable(&mut fauna, &world, sp, land), "its graph is reachable from where it lands ({land:?})");
    assert_eq!(fauna.view().ledger.drops_by_founder[SP.index()], 1, "one drop booked");
}

/// **A drop deeper than its reach is not taken.** The same ledge with both drops one layer
/// past the young body's reach (`drop.max_m` × its height over the adult's): it stays.
#[test]
fn a_drop_deeper_than_its_reach_is_not_taken() {
    let mut world = plain(48, 16, 24);
    let mut c = config();
    let down = c.founder(SP).step_limits(V).down;
    let (h, w) = (down + 2, down + 2);
    the_glide_is_out_of_reach(&c, h);
    let top = ledge(&mut world, h, w);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c.clone());
    let sp = on_ledge(&mut fauna, &world, top);
    // Reach one layer short of the shallower drop, in its own size.
    let reach_m = (f64::from(w) - 0.5) * V;
    let ratio = size_ratio(&fauna, sp);
    c.founders[SP.index()].drop = DropRule { after_s: 1.0, max_m: reach_m / ratio, wound_per_m: 0.0 };
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = on_ledge(&mut fauna, &world, top);
    assert_off_graph(&mut fauna, &world, sp, false);
    let bound = 3.0 * drop_bound_s(&c);
    let (landed, _) = run(&mut fauna, &world, &mut flora, sp, top, bound);
    assert!(landed.is_none(), "it stays on the ledge over {bound:.2} s ({landed:?})");
    assert_eq!(fauna.view().ledger.drops_by_founder[SP.index()], 0);
}

/// **The wound grows with the height past the step**: `drop.wound_per_m` × (the metres
/// dropped − `step_down_m`), and more from the deeper of two ledges.
#[test]
fn the_wound_grows_with_the_height_past_the_step() {
    let mut wounds = Vec::new();
    let c0 = config();
    let down = c0.founder(SP).step_limits(V).down;
    for w in [down + 1, down + 2] {
        let mut world = plain(48, 16, 24);
        let mut c = config();
        let h = down + 2;
        the_glide_is_out_of_reach(&c, h);
        let top = ledge(&mut world, h, w);
        let per_m = 0.4;
        c.founders[SP.index()].drop = DropRule { after_s: 1.0, max_m: 1e3, wound_per_m: per_m };
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let sp = on_ledge(&mut fauna, &world, top);
        assert_eq!(animal(&fauna, sp).wound, 0.0);
        let (landed, _) = run(&mut fauna, &world, &mut flora, sp, top, drop_bound_s(&c));
        let land = landed.expect("it jumps down");
        let dropped_m = f64::from(top - land.y) * V;
        let want = per_m * (dropped_m - c.founder(SP).step_down_m);
        let got = animal(&fauna, sp).wound;
        assert!((got - want).abs() < 1e-9, "a {dropped_m} m drop wounds {want}, got {got}");
        let l = &fauna.view().ledger;
        assert!((l.drop_wound_by_founder[SP.index()] - want).abs() < 1e-9, "the ledger books the wound");
        wounds.push(got);
    }
    assert!(wounds[1] > wounds[0], "the deeper drop wounds more ({wounds:?})");
}

/// **A body with a walking way out never drops.** The ledge of the first case (drops west
/// and south within reach), but a slot one column wide runs north from it through the wall,
/// on its own layer, to a plateau on its route graph: too narrow for the adult disc the graph
/// is built for, wide enough for the young body (its own walking way back on). Over three
/// times the drop's bound it never falls more than its step in a tick, and books no drop.
#[test]
fn a_body_with_a_walking_way_out_never_drops() {
    let mut world = plain(48, 16, 24);
    let mut c = config();
    let down = c.founder(SP).step_limits(V).down;
    let (h, w) = (down + 2, down + 1);
    the_glide_is_out_of_reach(&c, h);
    let top = ledge(&mut world, h, w);
    for z in 9..=11 {
        for y in top + 1..=top + 6 {
            world.apply(WorldCommand::SetMaterial { x: 20, y, z, material: Material::Air });
        }
    }
    rock(&mut world, 14..=20, 12..=15, FLOOR + 1..=top);
    c.founders[SP.index()].drop = DropRule { after_s: 1.0, max_m: 1e3, wound_per_m: 0.0 };
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = on_ledge(&mut fauna, &world, top);
    assert_off_graph(&mut fauna, &world, sp, true);
    let bound = 3.0 * drop_bound_s(&c);
    let mut deepest = 0;
    let mut y = top;
    for _ in 0..ticks(bound) {
        fauna.step(&world, &mut flora);
        let a = animal(&fauna, sp);
        deepest = deepest.max(y.saturating_sub(a.site.y));
        y = a.site.y;
    }
    assert!(deepest <= down, "it never falls more than its step in a tick ({deepest} layers)");
    assert_eq!(fauna.view().ledger.drops_by_founder[SP.index()], 0, "no drop booked");
}

/// **The route planner never uses a drop.** A plateau one layer past the step above the
/// floor: with every lineage's drop rule reaching far past it, its top and the floor are
/// apart on the route graph, and no route joins them.
#[test]
fn the_route_planner_never_uses_a_drop() {
    let mut world = plain(48, 16, 24);
    let mut c = config();
    let down = c.founder(SP).step_limits(V).down;
    let up = c.founder(SP).step_limits(V).up;
    let k = down.max(up) + 1;
    rock(&mut world, 10..=20, 3..=12, FLOOR + 1..=FLOOR + k);
    for f in Founder::ALL {
        c.founders[f.index()].drop = DropRule { after_s: 0.0, max_m: 1e3, wound_per_m: 0.0 };
    }
    let mut fauna = Fauna::new(c);
    let wv = world.view();
    let g = fauna.routes(&wv, SP, 1e6);
    let on_top = g.node_near(Site { x: 15, y: FLOOR + k, z: 7 }, centre(15, 7), 0).expect("fixture: a node on the plateau");
    let on_floor = g.node_near(Site { x: 30, y: FLOOR, z: 7 }, centre(30, 7), 0).expect("fixture: a node on the floor");
    assert_ne!(g.components[on_top], g.components[on_floor], "the drop joins no component");
    assert!(g.path(on_top, on_floor, 10_000).is_none(), "no route down the drop");
}


#[test]
fn drop_is_a_constraint_for_both_layers() {
    for layer in [crate::Layer::Brain, crate::Layer::Heuristic] {
        let mut world = plain(48, 16, 24);
        let mut c = config();
        let down = c.founder(SP).step_limits(V).down;
        let (h, w) = (down + 2, down + 1);
        the_glide_is_out_of_reach(&c, h);
        let top = ledge(&mut world, h, w);
        c.founders[SP.index()].drop = DropRule { after_s: 0.1, max_m: 1e3, wound_per_m: 0.0 };
        let bound = drop_bound_s(&c);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let id = on_ledge(&mut fauna, &world, top);
        if layer == crate::Layer::Brain {
            fauna.set_controller(id, Box::new(crate::Scripted::new(vec![crate::Actions::REST])));
        }
        assert_off_graph(&mut fauna, &world, id, false);
        let (landed, _) = run(&mut fauna, &world, &mut flora, id, top, bound);
        assert!(landed.is_some(), "{layer:?} is freed by DROP");
        assert_eq!(fauna.view().ledger.drops_by_founder[SP.index()], 1);
    }
}
