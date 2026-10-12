//! **SP-STUCK-2: the stuck bodies the first fix left** (Wrysk 2026-10-11: "black holes on the
//! map where animals just get stuck and rotate around forever"). Three cases from the SP-STUCK
//! check (branch 9455ef0c, eight seeds), each on a hand-built world at the world's own 0.125 m:
//!
//! - **Down a cliff, the last half voxel.** On a face the receptors read from the layer the
//!   feet have reached, rounded, over the column the body hangs in; the last half voxel above
//!   the foot read the foot's face as ground under it. The capgnawer forager took the face
//!   for a floor, answered with its ground logic, and its turns backed it up the face (TRAPS);
//!   up again it read the face and climbed on. Every stuck juvenile of the check hung within
//!   half a voxel of a cliff's foot, for hours.
//! - **A ledge under a roof.** The contact arcs probe at the step up, so a roof two layers
//!   over the feet read as a wall ahead, left and right wherever the body stood; two capgnawer
//!   founders turned on a ledge for four hours and never walked to its edge to climb down.
//! - **A jump in two.** A seedporter on a shelf whose only jumps land on a lower shelf, itself
//!   cornered, with the floor within one more jump: no single jump reached its graph, so it
//!   never jumped.
//!
//! Every bound comes from the config (the drop wait, the controller period, the cruise, the
//! climbing pace); no default, ratio or verdict is asserted.

use super::{OffGraph, off_graph_exit};
use crate::controller::{BlindForager, Controller, Response};
use crate::{Actions, Command, Fauna, FaunaConfig, Founder, MobilityMode, Scripted, StartingStores};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, TICK_HZ, World};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site};
use std::f64::consts::{FRAC_PI_2, PI};

const V: f64 = 0.125;
const FLOOR: u32 = 2;
const GNAWER: Founder = Founder::Capgnawer;
const SP: Founder = Founder::Seedporter;
/// `Pose::forward` is `(sin h, cos h)`.
const SOUTH: f64 = PI;
const EAST: f64 = FRAC_PI_2;

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

fn plain(height: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 48,
        height,
        depth: 24,
        voxel_m: V,
        seed: 11,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    rock(&mut w, 0..=47, 0..=23, 1..=FLOOR);
    w
}

fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = crate::Activity::Always;
    }
    c
}

/// A body of `founder` with body `body` on `site` at `(x, z)`, facing `heading`, driven by
/// its own heuristic.
fn place(fauna: &mut Fauna, world: &World, founder: Founder, body: f64, site: Site, (x, z): (f64, f64), heading: f64) -> u64 {
    assert!(fauna.apply(world, Command::IntroduceFounder { x: 2, z: 2, founder, stores: StartingStores::FULL, heading_rad: heading }));
    let id = fauna.animals.last().expect("just placed").id;
    let k = fauna.animals.len() - 1;
    let a = &mut fauna.animals[k];
    (a.pose.x, a.pose.z, a.pose.heading_rad) = (x, z, heading);
    a.site = site;
    a.body = body;
    a.pose.y_m = crate::surface_m(site.y, V);
    let t = fauna.config.teachers;
    fauna.controllers.set(id, crate::controller::heuristic(founder, t));
    id
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("the body")
}

fn ticks(s: f64) -> usize {
    (s / DT).round() as usize
}

fn period_s(c: &FaunaConfig, f: Founder) -> f64 {
    c.manifest(f).cadence_ticks().max(1) as f64 / f64::from(TICK_HZ)
}

// ------------------------------------------------------------------ down a cliff

/// The cliff: rock over `z ≥ 12` up to three layers over the floor (the stuck juveniles' 0.375
/// m face), its edge at the plane `z = 12`.
fn cliff() -> World {
    let mut w = plain(24);
    rock(&mut w, 0..=47, 12..=23, FLOOR + 1..=FLOOR + 3);
    w
}

/// **A body hanging down a cliff reads no ground under it until its climb ends, and its own
/// forager climbs on.** A newborn capgnawer walked south off the cliff's top hangs on the face
/// on its way down; at every tick of the descent, the last half voxel above the foot
/// included, its underside receptor reads nothing under it and its forager answers with the
/// climb (forward, no turn: a turn on a face backs it up). It stands on the foot at the end.
#[test]
fn a_body_down_a_cliff_reads_no_ground_under_it_until_its_climb_ends() {
    let world = cliff();
    let mut flora = Flora::new(FloraConfig::default());
    let c = config();
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let newborn = c.founder(GNAWER).core.birth_body;
    let top = Site { x: 20, y: FLOOR + 3, z: 13 };
    let id = place(&mut fauna, &world, GNAWER, newborn, top, (20.5 * V, 13.5 * V), SOUTH);
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 1.0, turn: 0.0, feed: 0.0 }]))));
    let manifest = c.manifest(GNAWER);
    let contact = manifest.modules.iter().find(|m| m.name == "Contact(4)").expect("a contact module").offset;
    let mut forager = BlindForager::for_founder(GNAWER, c.teachers);
    let (mut faced, mut last_half, mut climb) = (0, 0, None);
    for _ in 0..ticks(60.0) {
        fauna.step(&world, &mut flora);
        let i = index(&fauna, id);
        let a = fauna.animals[i];
        let Some(on) = a.mobility.climb else {
            if climb.is_some() {
                break;
            }
            continue;
        };
        assert!(!on.ascending, "fixture: a descent");
        climb = Some(on);
        faced += 1;
        if on.progress_m > on.rise_m - 0.5 * V {
            last_half += 1;
        }
        let obs = crate::body::observation(&fauna, i, &world.view(), &flora.view(), &manifest, None, None);
        assert_eq!(obs[contact + 3], 0.0, "nothing under it on the face at {:.4} of {:.4} m", on.progress_m, on.rise_m);
        let asked = match forager.drive(&obs) {
            Response::Bounded(a) | Response::Body(a, _) => a,
            Response::Logits(_) => unreachable!("a heuristic"),
        };
        assert!(asked.forward > 0.0 && asked.turn == 0.0, "its forager climbs on at {:.4} of {:.4} m: {asked:?}", on.progress_m, on.rise_m);
    }
    let on = climb.expect("fixture: it climbed down the cliff");
    assert!(faced > 0 && last_half > 0, "fixture: read along the face and in its last half voxel ({faced}, {last_half})");
    let a = fauna.animals[index(&fauna, id)];
    assert!(a.mobility.climb.is_none(), "its climb ended");
    assert_eq!(a.site, on.to, "it stands on the foot");
}

/// **Its own heuristic takes it down from the last half voxel, with food at the foot.** The
/// same newborn, set on the face with less than half a voxel to go and handed back to its
/// forager — which stays on any food it tastes (`capgnawer_stay_worth` 0), with litter on the
/// foot's faces — stands on the foot within the time the rest of the face takes at its
/// slowest pace, a pause and two controller periods (inside its trap wait). (Reading the foot
/// as ground under it, its forager held still on the face to eat the litter below until it
/// gave the patch up.)
#[test]
fn a_heuristic_body_in_the_last_half_voxel_of_a_cliff_ends_its_climb() {
    let world = cliff();
    let mut flora = Flora::new(FloraConfig::default());
    for z in 9..=11 {
        for x in 18..=22 {
            let face = Site { x, y: FLOOR, z };
            assert!(flora.deposit(face, Deposit { kind: DepositKind::Litter, organic: 1.0, mineral: 0.02, energy: 2.0 }), "fixture: litter on {face:?}");
        }
    }
    let mut c = config();
    c.teachers.capgnawer_stay_worth = 0.0;
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let newborn = c.founder(GNAWER).core.birth_body;
    let top = Site { x: 20, y: FLOOR + 3, z: 13 };
    let id = place(&mut fauna, &world, GNAWER, newborn, top, (20.5 * V, 13.5 * V), SOUTH);
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 1.0, turn: 0.0, feed: 0.0 }]))));
    let mut set = None;
    for _ in 0..ticks(60.0) {
        fauna.step(&world, &mut flora);
        let a = fauna.animals[index(&fauna, id)];
        if let Some(on) = a.mobility.climb
            && on.progress_m > on.rise_m - 0.5 * V
        {
            set = Some(on);
            break;
        }
    }
    let on = set.expect("fixture: in the last half voxel of the face");
    fauna.controllers.set(id, crate::controller::heuristic(GNAWER, c.teachers));
    // The rest of the face at the slowest bout's pace, a pause, and two controller periods.
    let p = c.founder(GNAWER);
    let pace = p.cadence;
    let slowest = p.climb_speed_fraction * c.manifest(GNAWER).cruise_m_per_s * if pace.on() { pace.speed_min } else { 1.0 };
    let bound = 0.5 * V / slowest + if pace.on() { pace.pause_max_s } else { 0.0 } + 2.0 * period_s(&c, GNAWER);
    assert!(bound < p.drop.after_s, "fixture: the bound is within the trap wait");
    let mut down = false;
    for _ in 0..ticks(bound) {
        fauna.step(&world, &mut flora);
        let a = fauna.animals[index(&fauna, id)];
        if a.mobility.climb.is_none() {
            assert_eq!(a.site, on.to, "it ends its climb on the foot");
            down = true;
            break;
        }
    }
    assert!(down, "it ends its climb within {bound:.2} s");
}

// ------------------------------------------------------------------ a ledge under a roof

/// The ledge: a block up to `L` (twelve layers over the floor, past an adult capgnawer's
/// jump); on it, a pocket two columns deep (`x = 18..=19`) and four long (`z = 10..=13`),
/// walled behind and at both ends over its two layers of room, under a roof slab that runs
/// two columns out past the cliff's edge (the overhang). The only way off is east, down the
/// cliff.
const L: u32 = FLOOR + 12;

fn ledge() -> World {
    let mut w = plain(L + 8);
    rock(&mut w, 10..=19, 0..=23, FLOOR + 1..=L);
    rock(&mut w, 17..=17, 9..=14, L + 1..=L + 2);
    rock(&mut w, 17..=19, 9..=9, L + 1..=L + 2);
    rock(&mut w, 17..=19, 14..=14, L + 1..=L + 2);
    rock(&mut w, 10..=21, 8..=15, L + 3..=L + 4);
    w
}

/// **A roof over the body is not a wall; a wall still is.** Under the ledge's roof, facing
/// along the pocket, its arcs read open where its own layer is clear; facing the pocket's back
/// wall, pressed to it, its front reads the wall.
#[test]
fn a_roof_over_the_body_is_not_a_wall_and_a_wall_still_is() {
    let world = ledge();
    let c = config();
    let p = c.founder(GNAWER);
    let body = p.adult_body();
    let r = body.footprint_radius();
    let climb = p.step_limits(V).up;
    let view = world.view();
    assert!(body.contact_layer(L, V) + climb >= L + 3, "fixture: the arcs probe at the roof");
    let pose = |x: f64, z: f64, h: f64| crate::Pose { x, z, y_m: crate::surface_m(L, V), heading_rad: h };
    let open = crate::body::contact_readings(&view, &pose(19.0 * V, 11.5 * V, EAST), L, &body, climb);
    assert_eq!((open.front, open.left, open.right), (0.0, 0.0, 0.0), "under the roof, nothing ahead or beside: {open:?}");
    let walled = crate::body::contact_readings(&view, &pose(18.0 * V + r, 11.5 * V, -EAST), L, &body, climb);
    assert_eq!(walled.front, 1.0, "the back wall ahead: {walled:?}");
}

/// **A capgnawer on a ledge under a roof walks off its edge and climbs down.** An adult
/// founder in the pocket (cornered: the cliff is past its jump, and its route graph has no
/// way down), driven by its forager with nothing to eat, ends on the floor within its trap
/// wait, the walk across the pocket and the climb down the cliff.
#[test]
fn a_climber_on_a_ledge_under_a_roof_walks_off_its_edge_and_climbs_down() {
    let world = ledge();
    let mut flora = Flora::new(FloraConfig::default());
    let c = config();
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let adult = c.founder(GNAWER).core.body_max;
    let id = place(&mut fauna, &world, GNAWER, adult, Site { x: 18, y: L, z: 11 }, (18.5 * V, 11.5 * V), 0.0);
    let a = fauna.animals[index(&fauna, id)];
    assert_eq!(off_graph_exit(&mut fauna, &world.view(), &flora.view(), &a, (0.0, 0.0)), OffGraph::Cornered, "fixture: cornered on the ledge");
    let p = c.founder(GNAWER);
    let cruise = c.manifest(GNAWER).cruise_m_per_s;
    let climb_s = f64::from(L - FLOOR) * V / (p.climb_speed_fraction * cruise);
    let walk_s = 6.0 * V / cruise;
    let bound = p.drop.after_s + walk_s + climb_s + 2.0 * period_s(&c, GNAWER);
    let mut down = None;
    for t in 0..ticks(bound) {
        fauna.step(&world, &mut flora);
        let a = fauna.animals[index(&fauna, id)];
        if a.site.y == FLOOR && a.mobility.climb.is_none() {
            down = Some(t);
            break;
        }
    }
    assert!(down.is_some(), "it climbs down to the floor within {bound:.1} s");
    assert!(fauna.view().ledger.wall_descents_by_founder[GNAWER.index()] >= 1, "down the cliff, not a jump");
}

// ------------------------------------------------------------------ a jump in two

/// The shelves: the floor; a lower shelf `x = 16..=20`, `z = 9..=13`, seven layers up (short
/// of a glide, within a jump); on it an upper shelf `x = 19..=20`, `z = 11`, six layers higher
/// (thirteen over the floor: past a jump straight down); a tall wall east of both. Both
/// shelves are islands of the seedporter's graph; the floor is its graph.
const LOW: u32 = FLOOR + 7;
const HIGH: u32 = LOW + 6;

fn shelves() -> World {
    let mut w = plain(HIGH + 8);
    rock(&mut w, 16..=20, 9..=13, FLOOR + 1..=LOW);
    rock(&mut w, 19..=20, 11..=11, LOW + 1..=HIGH);
    rock(&mut w, 21..=24, 0..=23, FLOOR + 1..=HIGH + 4);
    w
}

fn sp_config() -> FaunaConfig {
    let mut c = config();
    c.founders[SP.index()].drop.after_s = 1.0;
    c
}

/// **A body whose only jumps land in a lower pocket jumps twice and walks away.** On the upper
/// shelf an adult seedporter is cornered: no walking way, no glide, and no single jump reaches
/// its graph; its way down is a jump onto the lower shelf, from which one more jump does.
/// Within two trap waits and their walks it is on the floor, two drops booked, on its graph.
#[test]
fn a_body_whose_only_jumps_land_in_a_lower_pocket_jumps_twice_and_walks_away() {
    let world = shelves();
    let mut flora = Flora::new(FloraConfig::default());
    let c = sp_config();
    let p = c.founder(SP);
    let body = p.adult_body();
    let (r, head, step, wade) = (body.footprint_radius(), body.headroom_voxels(V), p.step_limits(V), p.wade_depth_m(&body));
    let reach = crate::body::step_voxels(p.drop.reach_m(p, &body), V);
    assert!(HIGH - FLOOR > reach && LOW - FLOOR <= reach && HIGH - LOW <= reach, "fixture: a jump in two, not one");
    assert!(f64::from(LOW - FLOOR) * V < p.glider.expect("a glider").glide_min_height_m, "fixture: no glide off the lower shelf");
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let upper = Site { x: 20, y: HIGH, z: 11 };
    let at = (20.0 * V, 11.5 * V);
    let id = place(&mut fauna, &world, SP, p.core.body_max, upper, at, -EAST);
    let view = world.view();
    let a = fauna.animals[index(&fauna, id)];
    assert_eq!(off_graph_exit(&mut fauna, &view, &flora.view(), &a, (0.0, 0.0)), OffGraph::Cornered, "fixture: cornered on the upper shelf");
    let own = (r / V).ceil() as u32;
    let g = fauna.routes(&view, SP, 1e6);
    let way = g.drop_way(&view, upper, at, r, head, step, reach, wade, own, 32).expect("a way down");
    assert_eq!(way.land.y, LOW, "its jump lands on the lower shelf: {way:?}");
    let m = c.manifest(SP);
    let one = p.drop.after_s + period_s(&c, SP) + PI / m.yaw_cap_rad_per_s + 4.0 * V / m.cruise_m_per_s + 2.0 * period_s(&c, SP);
    let mut landed = None;
    for _ in 0..ticks(2.0 * one) {
        fauna.step(&world, &mut flora);
        let a = fauna.animals[index(&fauna, id)];
        if a.site.y == FLOOR {
            landed = Some(a);
            break;
        }
    }
    let a = landed.unwrap_or_else(|| panic!("on the floor within {:.2} s", 2.0 * one));
    assert_eq!(fauna.view().ledger.drops_by_founder[SP.index()], 2, "two drops booked");
    assert_eq!(a.mobility.mode, MobilityMode::Ground, "jumps, not a glide");
    assert_eq!(off_graph_exit(&mut fauna, &view, &flora.view(), &a, (0.0, 0.0)), OffGraph::On, "on its graph where it lands ({:?})", a.site);
}
