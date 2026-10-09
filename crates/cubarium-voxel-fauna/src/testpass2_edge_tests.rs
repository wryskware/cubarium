//! **TEST-PASS-2 (independent), EDGE and EDGE-2**: the pressed walk's re-plan and its shun at
//! their edges, the shun list's bound, the press shun against a seedporter's only reachable
//! fruit stand, the strip's end against a hunter on either side, the cornered and the pressed
//! flight on the route graph, the motor's step out of solid at its edges, the level tolerance
//! at its edge, the route's `path` and `waypoint`, and the ledgers through a wedged and a
//! drowned body. A child of `step`.
//!
//! Each case builds its scene by hand at 0.25 m and sets the config it relies on; its bounds
//! come from that config. No default, ratio or verdict is asserted.

use super::predation_fixture::{animal, animal_mut, assert_closes};
use crate::perception::{self, class};
use crate::{Actions, Activity, Command, Fauna, FaunaConfig, Founder, StartingStores, Track, VigilMode, Vigilance};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, TICK_HZ, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};
use std::f64::consts::{FRAC_PI_2, PI};

const V: f64 = 0.25;
const SP: Founder = Founder::Seedporter;
const VH: Founder = Founder::Chorister;
const GRAZER: Founder = Founder::Browser;
const NORTH: f64 = 0.0;
const SOUTH: f64 = PI;
const EAST: f64 = FRAC_PI_2;
const WEST: f64 = -FRAC_PI_2;
const HUNGRY: StartingStores = StartingStores { body: 1.0, reserve: 0.3 };
const FLOOR: u32 = 2;

// ------------------------------------------------------------------ fixtures

/// Soil at `y = 1..=2` at field capacity, `width × depth` columns, the clock at noon.
fn plain(width: u32, depth: u32, height: u32) -> World {
    let mut w = World::empty(VoxelConfig { width, height, depth, voxel_m: V, seed: 13, day: DayClock { start_hour: 12.0, ..DayClock::default() }, ..VoxelConfig::default() });
    let m = cubarium_voxel::Materials::DEFAULT;
    let fc = m.field_capacity(Material::Soil) * m.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=FLOOR {
                w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc });
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

fn fruit_flora() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.lanternberry.fruit_per_s = 1e-3;
    c.lanternberry.fruit_cap = 2e-3;
    c.lanternberry.fruit_drop_s = 1e9;
    c.lanternberry.propagule_rate = 0.0;
    // F3's fruit alone (FLOWER-FRUIT): no flowering stage, and no rest after a crop is
    // eaten out, so the fixture's supply refills as it did.
    c.lanternberry.flower_s = 0.0;
    c.lanternberry.fruit_rest_s = 0.0;
    c
}

fn stand(flora: &mut Flora, world: &World, x: i64, z: u32) -> Site {
    let wood = flora.config().species(Plant::Lanternberry).wood_max;
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species: Plant::Lanternberry, wood }));
    flora.view().stands.iter().find(|s| i64::from(s.site.x) == x && s.site.z == z).expect("the stand").site
}

fn stand_id(flora: &Flora, site: Site) -> u64 {
    flora.view().stand_at(site).expect("the stand").id
}

fn centre(x: i64, z: u32) -> (f64, f64) {
    ((x as f64 + 0.5) * V, (f64::from(z) + 0.5) * V)
}

fn put(fauna: &mut Fauna, world: &World, f: Founder, (x_m, z_m): (f64, f64), heading: f64, stores: StartingStores) -> u64 {
    let (x, z) = ((x_m / V).floor() as i64, (z_m / V).floor() as u32);
    assert!(fauna.apply(world, Command::IntroduceFounder { x, z, founder: f, stores, heading_rad: heading }), "placing a {}", f.name());
    let id = fauna.animals.last().expect("just placed").id;
    let a = animal_mut(fauna, id);
    a.pose.x = x_m;
    a.pose.z = z_m;
    a.pose.heading_rad = heading;
    id
}

fn heuristic(fauna: &mut Fauna, id: u64) {
    let f = animal(fauna, id).founder.expect("a founder");
    let t = fauna.config.teachers;
    fauna.controllers.set(id, crate::controller::heuristic(f, t));
}

fn hold(fauna: &mut Fauna, id: u64, act: Actions) {
    fauna.controllers.set(id, Box::new(crate::Scripted::new(vec![act])));
}

fn remember_fruit(fauna: &mut Fauna, id: u64, flora: &Flora, site: Site) {
    let (x, z) = centre(i64::from(site.x), site.z);
    let tick = fauna.tick;
    let p = fauna.percepts.entry(id).or_default();
    p.best[class::FOOD as usize] = Track {
        known: true,
        id: stand_id(flora, site),
        class: class::FOOD,
        modes: perception::SIGHT,
        x,
        z,
        y_m: crate::surface_m(site.y, V),
        site: (site.x, site.y, site.z),
        tick,
        ..Track::default()
    };
}

fn ticks(s: f64) -> u64 {
    cubarium_voxel::ticks_at_least_one(s)
}

// ------------------------------------------------------------------ the pressed walk's re-plan

/// **A pressed walk gives its target up at `press_replan_s`, not a tick before, and the shun
/// lapses at `press_shun_s`**: a body walking to target 77 since `press_replan_s` ago, pressed
/// that long, walked there this tick: given up. One tick short of either, or its last walk there
/// older than two controller periods, it is not. Given up at `t`, the target is shunned until
/// `t + press_shun_s` and not at it.
#[test]
fn a_pressed_walk_gives_up_at_press_replan_s_and_its_shun_lapses_at_press_shun_s() {
    let mut c = config();
    c.teachers.press_replan_s = 2.0;
    c.teachers.press_shun_s = 5.0;
    let p = ticks(2.0);
    let shun = ticks(5.0);
    let gap = 2 * SP.manifest().cadence_ticks().max(1);
    let now = 10_000u64;
    let case = |since: u64, pressed: u64, last: u64| -> Vec<u64> {
        let mut fauna = Fauna::new(c.clone());
        fauna.tick = now;
        fauna.walk_targets.insert(1, (77, since, last));
        fauna.pressed.insert(1, pressed);
        fauna.walk_shunned(1, SP)
    };
    assert_eq!(case(now - p, p, now), vec![77], "pressed press_replan_s on a walk that old: given up");
    assert!(case(now - p + 1, p, now).is_empty(), "the walk a tick younger than press_replan_s");
    assert!(case(now - p, p - 1, now).is_empty(), "pressed a tick short");
    assert!(case(now - p, p, now - gap - 1).is_empty(), "its last walk there older than two periods");
    // The shun's span.
    let mut fauna = Fauna::new(c.clone());
    fauna.tick = now;
    fauna.walk_targets.insert(1, (77, now - p, now));
    fauna.pressed.insert(1, p);
    assert_eq!(fauna.walk_shunned(1, SP), vec![77]);
    assert!(fauna.walk_targets.get(&1).is_none(), "the walk is dropped with its target");
    fauna.tick = now + shun - 1;
    assert_eq!(fauna.walk_shunned(1, SP), vec![77], "a tick before press_shun_s");
    fauna.tick = now + shun;
    assert!(fauna.walk_shunned(1, SP).is_empty(), "at press_shun_s it lapses");
}

/// **The shun list holds at most `WALK_SHUN_SLOTS`, and a new give-up drops the soonest to
/// lapse**: a body with the list full of live shuns, given up on one more target, keeps the
/// most and holds the new one, without the one that would have lapsed first.
#[test]
fn the_shun_list_is_bounded_and_drops_the_soonest_to_lapse() {
    let mut c = config();
    c.teachers.press_replan_s = 1.0;
    c.teachers.press_shun_s = 100.0;
    let p = ticks(1.0);
    let now = 10_000u64;
    let mut fauna = Fauna::new(c);
    fauna.tick = now;
    let slots = crate::WALK_SHUN_SLOTS as u64;
    // Live shuns lapsing at now + 10, now + 11, ...: the first lapses soonest.
    fauna.walk_shun.insert(1, (0..slots).map(|k| (100 + k, now + 10 + k)).collect());
    fauna.walk_targets.insert(1, (999, now - p, now));
    fauna.pressed.insert(1, p);
    let got = fauna.walk_shunned(1, SP);
    assert_eq!(got.len() as u64, slots, "the list's bound: {got:?}");
    assert!(got.contains(&999), "the new give-up is held: {got:?}");
    assert!(!got.contains(&100), "the soonest to lapse is dropped: {got:?}");
}

// ------------------------------------------------------------------ the press shun x the only fruit stand

/// **A seedporter whose only fruit stand it can reach is shunned walks to it again once the
/// shun lapses, and not before**: a hungry seedporter remembers the one fruiting stand on open
/// ground behind it; the stand is in its give-up list until `t + press_shun_s`. Its fruit walk does not
/// take it meanwhile (no sample held by the fruit search), and does within two controller
/// periods of the shun's end.
#[test]
fn a_seedporter_takes_its_only_fruit_stand_again_once_the_press_shun_lapses() {
    let (width, depth) = (64u32, 16u32);
    let mut world = plain(width, depth, 16);
    let mut flora = Flora::new(fruit_flora());
    let site = stand(&mut flora, &world, 40, 8);
    for _ in 0..60 {
        flora.step(&mut world);
    }
    assert!(flora.view().stand_at(site).expect("stand").fruit > 0.0, "fixture: fruit hangs");
    let mut c = config();
    c.teachers.press_shun_s = 3.0;
    c.teachers.press_replan_s = 1e6;
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    // Facing away from the stand, so its own forager does not walk to it.
    let sp = put(&mut fauna, &world, SP, centre(30, 8), WEST, HUNGRY);
    heuristic(&mut fauna, sp);
    let id = stand_id(&flora, site);
    let until = fauna.tick + ticks(3.0);
    fauna.walk_shun.insert(sp, vec![(id, until)]);
    let mut fruit_while_shunned = 0;
    let mut fruit_after: Option<u64> = None;
    let period = c.manifest(SP).cadence_ticks().max(1);
    for _ in 0..(ticks(3.0) + 4 * period) {
        remember_fruit(&mut fauna, sp, &flora, site);
        fauna.step(&world, &mut flora);
        let fruit = fauna.steer_of(sp) == Some(crate::Steer::Fruit);
        if fauna.tick < until {
            fruit_while_shunned += usize::from(fruit);
        } else if fruit && fruit_after.is_none() {
            fruit_after = Some(fauna.tick);
        }
    }
    assert_eq!(fruit_while_shunned, 0, "it walked to the shunned stand");
    let after = fruit_after.expect("once the shun lapses it walks to the stand again");
    assert!(after - until <= 2 * period, "{} ticks after the shun lapsed", after - until);
}

// ------------------------------------------------------------------ the strip's end x a hunter

/// One flight response of a seedporter standing at `z` facing `heading`, fleeing a still
/// Veilhound at `(threat_x, threat_z)` that it has seen (the stillness rule out of reach). The
/// Veilhound faces it: FEAR's watch leaves a still one facing away unfled.
fn end_response(z: f64, heading: f64, threat: (f64, f64)) -> Actions {
    let (width, depth) = (48u32, 16u32);
    let world = plain(width, depth, 12);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.predation.still_after_s = 1e6;
    c.perception.remote_hz = f64::from(TICK_HZ);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let x = 6.0;
    let toward = (threat.0 - x).atan2(threat.1 - z);
    let sp = put(&mut fauna, &world, SP, (x, z), toward, StartingStores::FULL);
    heuristic(&mut fauna, sp);
    let vh = put(&mut fauna, &world, VH, threat, (x - threat.0).atan2(z - threat.1), StartingStores::FULL);
    hold(&mut fauna, vh, Actions::REST);
    let i = fauna.animals.iter().position(|a| a.id == sp).unwrap();
    fauna.tick += 1;
    perception::sense_pass(&mut fauna, &world.view(), false, 1.0, 1);
    perception::refresh(&mut fauna, i, &world.view(), &flora.view(), None, perception::Want::default());
    let _ = crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST);
    assert!(fauna.percept(sp).is_some_and(|p| p.vigil.fleeing), "fixture: the seedporter flees the Veilhound");
    animal_mut(&mut fauna, sp).pose.heading_rad = heading;
    crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST).expect("in flight")
}

/// **At the strip's end a flight turns along it away from the hunter, on either side**: a
/// seedporter against the south end facing out through it, a Veilhound inward of it and to its
/// west: it does not walk out, and facing east (away) it walks while facing west (toward the
/// hunter's side) it does not. The mirror with the hunter to its east. With the hunter straight
/// inward of it (no side), it does not walk out through the end either.
#[test]
fn at_the_strips_end_a_flight_turns_along_it_away_from_the_hunter_on_either_side() {
    let r = config().founder(SP).adult_body().footprint_radius();
    let z = r;
    for (dx, away, toward) in [(-0.6, EAST, WEST), (0.6, WEST, EAST)] {
        let threat = (6.0 + dx, 2.0);
        let act = end_response(z, SOUTH, threat);
        assert_eq!(act.forward, 0.0, "hunter at dx {dx}: facing out through the end it walks ({act:?})");
        let act = end_response(z, away, threat);
        assert!(act.forward > 0.0, "hunter at dx {dx}: along the end away from it, it walks ({act:?})");
        let act = end_response(z, toward, threat);
        assert_eq!(act.forward, 0.0, "hunter at dx {dx}: along the end toward its side, it turns first ({act:?})");
    }
    let act = end_response(z, SOUTH, (6.0, 2.0));
    assert_eq!(act.forward, 0.0, "hunter straight inward: facing out through the end it walks ({act:?})");
}

// ------------------------------------------------------------------ the flight on the graph

/// A frondgrazer fleeing a walking Veilhound 2 m west of it on open ground (hunker off, flight
/// only); `cornered` makes it still past `still_after_s` in its flight, `pressed_ticks` sets its
/// pressed run. Returns the fauna, its index, and the response.
fn open_flight(cornered: bool, pressed_ticks: Option<i64>) -> (Fauna, u64, Actions) {
    let (width, depth) = (48u32, 16u32);
    let world = plain(width, depth, 12);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[GRAZER.index()].hunker.enabled = false;
    c.founders[GRAZER.index()].vigilance =
        Vigilance { mode: VigilMode::Herd, flee_start_m: 4.0, flee_stop_m: 8.0, calm_s: 10.0, group_min: 1, run_noise: 1.0, ..Vigilance::default() };
    c.perception.remote_hz = f64::from(TICK_HZ);
    c.teachers.press_replan_s = 1.0;
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let gr = put(&mut fauna, &world, GRAZER, centre(24, 8), EAST, StartingStores::FULL);
    heuristic(&mut fauna, gr);
    let vh = put(&mut fauna, &world, VH, centre(16, 8), EAST, StartingStores::FULL);
    hold(&mut fauna, vh, Actions::REST);
    animal_mut(&mut fauna, vh).speed_m_s = 1.0;
    animal_mut(&mut fauna, vh).still_ticks = 0;
    let i = fauna.animals.iter().position(|a| a.id == gr).unwrap();
    fauna.tick += 1;
    perception::sense_pass(&mut fauna, &world.view(), false, 1.0, 1);
    perception::refresh(&mut fauna, i, &world.view(), &flora.view(), None, perception::Want::default());
    let _ = crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST);
    assert!(fauna.percept(gr).is_some_and(|p| p.vigil.fleeing), "fixture: the grazer flees");
    if cornered {
        let still = c.predation.still_after_ticks();
        fauna.tick += still + 1;
        fauna.percepts.get_mut(&gr).unwrap().vigil.close_tick = fauna.tick;
        animal_mut(&mut fauna, gr).still_ticks = still + 1;
    }
    if let Some(k) = pressed_ticks {
        let p = ticks(c.teachers.press_replan_s) as i64 + k;
        fauna.pressed.insert(gr, p.max(0) as u64);
    }
    animal_mut(&mut fauna, gr).pose.heading_rad = EAST;
    let act = crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST).expect("in flight");
    (fauna, gr, act)
}

/// **A cornered flight on open ground steps on along its graph, away from the hunter**: still
/// past `still_after_s` in its flight but on its route graph with farther nodes round it, the
/// grazer (facing away) walks on, not holding still.
#[test]
fn a_cornered_flight_on_open_ground_steps_on_along_its_graph() {
    let (_, _, act) = open_flight(true, None);
    assert!(act.forward > 0.0, "cornered on open ground, facing away from the hunter: {act:?}");
}

/// **A pressed flight goes by its graph from `press_replan_s` on, not a tick before**: pressed
/// one tick short of `press_replan_s` the flight keeps its own heading rule (no route flight
/// recorded); pressed `press_replan_s`, the rest of that flight is a route flight.
#[test]
fn a_pressed_flight_goes_by_its_graph_from_press_replan_s() {
    let (fauna, gr, _) = open_flight(false, Some(-1));
    let flee = fauna.percept(gr).unwrap().vigil.flee_tick;
    assert_ne!(fauna.route_flee.get(&gr), Some(&flee), "a tick short of press_replan_s");
    let (fauna, gr, _) = open_flight(false, Some(0));
    let flee = fauna.percept(gr).unwrap().vigil.flee_tick;
    assert_eq!(fauna.route_flee.get(&gr), Some(&flee), "at press_replan_s");
}

// ------------------------------------------------------------------ EDGE-2: the motor out of solid

/// **The motor's step out of solid, at its edges**: a seedporter whose disc is a third of its
/// radius into a rock wall east of it may step west (out) and north along the wall at the same
/// depth (TRAPS: no deeper, where EDGE-2 had "less deep"), not east (deeper); a clear body a
/// centimetre off the same wall may step north along it and not east into it.
#[test]
fn the_motors_step_out_of_solid_at_its_edges() {
    let mut world = plain(32, 16, 12);
    rock(&mut world, 16..=19, 0..=15, FLOOR + 1..=FLOOR + 6);
    let c = config();
    let p = c.founder(SP);
    let body = p.adult_body();
    let r = body.footprint_radius();
    let wade = p.wade_depth_m(&body);
    let step = 0.05;
    let probe = |x: f64, dir: (f64, f64)| crate::motor_probe(&world.view(), x, 8.5 * V, dir, FLOOR, step, &body, wade, p.step_limits(V));
    let caught = 16.0 * V - r + r / 3.0;
    let (out, clear) = probe(caught, (-1.0, 0.0));
    assert!(!clear, "fixture: caught in the wall");
    assert!(out, "a step out of the wall");
    assert!(!probe(caught, (1.0, 0.0)).0, "a step deeper");
    assert!(probe(caught, (0.0, 1.0)).0, "a step along the wall at the same depth");
    let free = 16.0 * V - r - 0.01;
    let (along, clear) = probe(free, (0.0, 1.0));
    assert!(clear, "fixture: clear of the wall");
    assert!(along, "a clear body steps along the wall");
    assert!(!probe(free, (1.0, 0.0)).0, "a clear body does not step into the wall");
}

/// **The level tolerance at its edge**: a seedporter standing in water deeper than it wades,
/// pushed east onto a column deeper than its own by just under `LEVEL_TOLERANCE_M`, steps
/// onto it; by just over it, it stays on its own column.
#[test]
fn the_level_tolerance_at_its_edge() {
    let tol = crate::body::LEVEL_TOLERANCE_M;
    let run = |extra: f64| -> f64 {
        let mut world = plain(32, 8, 12);
        let c = config();
        let wade = c.founder(SP).wade_depth_m(&c.founder(SP).adult_body());
        let cell = world.config().voxel_volume();
        let base = wade + 0.05;
        for z in 0..8u32 {
            for x in 0..32i64 {
                let d = if x >= 12 { base + extra } else { base };
                world.apply(WorldCommand::AddWater { x, y: FLOOR + 1, z, volume_m3: cell * d / V });
            }
        }
        let wv = world.view();
        let (here, there) = (wv.water_depth_m(11, FLOOR, 4), wv.water_depth_m(12, FLOOR, 4));
        assert!(here > wade, "fixture: deeper than it wades");
        assert!(((there - here) - extra).abs() < 1e-6, "fixture: the step deeper by {extra} ({here} -> {there})");
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let x0 = 12.0 * V - 0.02;
        let sp = put(&mut fauna, &world, SP, (x0, 4.5 * V), EAST, StartingStores::FULL);
        hold(&mut fauna, sp, Actions { forward: 1.0, turn: 0.0, feed: 0.0 });
        for _ in 0..ticks(0.5) {
            fauna.step(&world, &mut flora);
        }
        animal(&fauna, sp).pose.x - 12.0 * V
    };
    // Its own face (the column it stands on) it may cross; the deeper column it enters or not.
    assert!(run(tol - 0.001) > 0.0, "deeper by under the tolerance: it steps onto the deeper column");
    assert!(run(tol + 0.001) <= 0.0, "deeper by over the tolerance: it stays on its own column");
}

// ------------------------------------------------------------------ the route

/// **A route is found within `max_steps` exactly, and not across components**: on an open
/// plain the route between two nodes `L` columns apart is `L` steps — found with `max_steps`
/// `L`, not with `L − 1`; from a node to itself it is that node; across two rock walls cutting
/// the ring there is none.
#[test]
fn a_route_is_found_within_max_steps_exactly_and_not_across_components() {
    let mut world = plain(48, 12, 16);
    // Two walls across the strip: the ring between them is cut in two.
    rock(&mut world, 30..=31, 0..=11, FLOOR + 1..=FLOOR + 10);
    rock(&mut world, 44..=45, 0..=11, FLOOR + 1..=FLOOR + 10);
    let c = config();
    let g = crate::RouteGraph::for_founder(&world.view(), c.founder(SP), 0.0);
    let node = |x: u32, z: u32| g.face(Site { x, y: FLOOR, z }).expect("a node on the plain");
    let (a, b) = (node(10, 6), node(20, 3));
    let l = 10 + 3;
    let p = g.path(a, b, l).expect("found within its own length");
    assert_eq!(p.len() as u32, l + 1, "{l} steps");
    assert_eq!((p[0], *p.last().unwrap()), (a, b));
    assert!(g.path(a, b, l - 1).is_none(), "not within a step less");
    assert_eq!(g.path(a, a, 0), Some(vec![a]), "to itself");
    let beyond = node(36, 6);
    assert_ne!(g.components[a], g.components[beyond], "fixture: the walls cut the ring");
    assert!(g.path(a, beyond, 1000).is_none(), "across the walls");
}

/// **The waypoint is the farthest of the lookahead it can walk straight to**: on an open plain,
/// a straight route of twenty columns: with a lookahead of 12 the waypoint is the 12th node
/// on; with a lookahead of 1, the next node.
#[test]
fn the_waypoint_is_the_farthest_of_the_lookahead_it_can_walk_to() {
    let world = plain(48, 12, 8);
    let c = config();
    let g = crate::RouteGraph::for_founder(&world.view(), c.founder(SP), 0.0);
    let node = |x: u32| g.face(Site { x, y: FLOOR, z: 6 }).expect("a node");
    let p = g.path(node(10), node(30), 100).expect("a route");
    assert_eq!(p.len(), 21, "fixture: straight along the row");
    let at = g.centre(p[0]);
    assert_eq!(g.waypoint(&p, at, 12), Some(g.centre(p[12])), "the 12th node on");
    assert_eq!(g.waypoint(&p, at, 1), Some(g.centre(p[1])), "lookahead 1: the next node");
}

// ------------------------------------------------------------------ the ledgers

/// **The ledgers close through a wedged and a drowned body**: a seedporter caught in a wall
/// and pushed into it for a second, then out of it; and one held in water over its head until
/// it drowns (`drown_after_s` cut to half a second). The fauna's organic matter, mineral and
/// energy match the ledger after each.
#[test]
fn the_ledgers_close_through_a_wedged_and_a_drowned_body() {
    let mut world = plain(32, 16, 12);
    rock(&mut world, 16..=19, 0..=15, FLOOR + 1..=FLOOR + 6);
    let mut flora = Flora::new(FloraConfig::default());
    let c = config();
    let r = c.founder(SP).adult_body().footprint_radius();
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = put(&mut fauna, &world, SP, (16.0 * V - r + r / 3.0, 8.5 * V), EAST, StartingStores::FULL);
    hold(&mut fauna, sp, Actions { forward: 1.0, turn: 0.0, feed: 0.0 });
    for _ in 0..ticks(1.0) {
        fauna.step(&world, &mut flora);
    }
    assert_closes(&fauna, "wedged against the wall");
    animal_mut(&mut fauna, sp).pose.heading_rad = WEST;
    for _ in 0..ticks(1.0) {
        fauna.step(&world, &mut flora);
    }
    assert_closes(&fauna, "out of the wall");

    let mut world = plain(32, 8, 12);
    let mut c = config();
    c.founders[SP.index()].drown_after_s = 0.5;
    let h = c.founder(SP).adult_body().height_m;
    let cell = world.config().voxel_volume();
    for z in 0..8u32 {
        for x in 0..32i64 {
            world.apply(WorldCommand::AddWater { x, y: FLOOR + 1, z, volume_m3: cell * (h + 0.3) / V });
        }
    }
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let sp = put(&mut fauna, &world, SP, centre(16, 4), NORTH, StartingStores::FULL);
    hold(&mut fauna, sp, Actions::REST);
    // A Veilhound standing by (its own drowning a minute off): the books are not all zero.
    let vh = put(&mut fauna, &world, VH, centre(4, 4), NORTH, StartingStores::FULL);
    hold(&mut fauna, vh, Actions::REST);
    let mut gone = false;
    for _ in 0..ticks(5.0) {
        fauna.step(&world, &mut flora);
        if fauna.view().animal(sp).is_none() {
            gone = true;
            break;
        }
    }
    assert!(gone && fauna.ledger.departed_founder(SP, crate::Departure::Drowned) == 1, "fixture: it drowns");
    assert_closes(&fauna, "after the drowning");
}

