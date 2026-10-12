//! **SP-FOLLOW, from the brief** (Wrysk 2026-10-11: "sure lets add nursing"; the follow did
//! not work): a hungry dependent young goes to its mother as to food and reaches nursing
//! contact, on the ground and up a tree; a sated one forages on its own within its leash and
//! goes back past it; out of its senses it heads to where it last sensed her; it weighs its
//! mother past a nearer troop-mate; flight still wins; and a weaned young, an orphan and
//! every other lineage behave exactly as with the follow off.
//!
//! Every scene sets the knobs it relies on; no shipped number, ratio or world hash is asserted.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use crate::{
    Activity, Command, Fauna, FaunaConfig, Founder, Herding, MobilityMode, Nursing, PerchAnchor, Pose, SenseTable,
    StartingStores, Steer,
};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};
use fixture::{EAST, LANE_Z, VOXEL_M, WEST, animal, animal_mut, assert_closes, drive, introduce, place};

const SP: Founder = Founder::Seedporter;
const RADIUS: f64 = 0.5;
const LEASH: f64 = 3.0;
const WEAN_S: f64 = 1000.0;

/// The seedporter's nursing for these scenes: a rate that never binds, the leash on.
fn nursing() -> Nursing {
    Nursing {
        weaning_age_s: WEAN_S,
        nurse_radius_m: RADIUS,
        nurse_rate_per_s: 1.0,
        nurse_reserve_floor: 0.3,
        nurse_efficiency: 0.7,
        follow_m: 0.4,
        follow_pull: 0.8,
        leash_m: LEASH,
        mother_memory_s: 60.0,
    }
}

/// The follow off: the leash and the memory at 0 (the drift as before SP-FOLLOW).
fn leash_off(n: &mut Nursing) {
    n.leash_m = 0.0;
    n.mother_memory_s = 0.0;
}

/// Every lineage awake, no ground response, no troop pull, no bellwing catching; the
/// seedporter's senses live and all-round at 16 m; its nursing [`nursing`] edited by `edit`.
fn config(edit: impl FnOnce(&mut Nursing)) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for p in &mut c.founders {
        p.active = Activity::Always;
        p.vigilance = crate::Vigilance::default();
        p.catch_bellwings = false;
        p.herding = Herding::default();
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let p = &mut c.founders[SP.index()];
    p.senses = SenseTable {
        live: true,
        sight_m: 16.0,
        sight_night_m: 16.0,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: 180.0,
        kin_tracks: 1,
        ..SenseTable::default()
    };
    let mut n = nursing();
    edit(&mut n);
    p.nursing = n;
    c
}

/// A flat soil plain 12 m around and 3 m deep at field capacity, the clock at noon (fruit
/// grows by day) or off.
fn world(noon: bool) -> World {
    let (width, depth) = (48u32, 12u32);
    let day = if noon { DayClock { start_hour: 12.0, ..DayClock::default() } } else { DayClock { mode: DayMode::Off, ..DayClock::default() } };
    let mut w = World::empty(VoxelConfig { width, height: 16, depth, voxel_m: VOXEL_M, seed: 61, day, ..VoxelConfig::default() });
    let m = cubarium_voxel::Materials::DEFAULT;
    let cap = m.pore_capacity(Material::Soil) * w.config().voxel_volume();
    let fc = m.field_capacity(Material::Soil);
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=2 {
                w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc * cap });
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

/// Lanternberries that hang fruit fast and never drop it.
fn fruit_flora() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.lanternberry.fruit_per_s = 1e-3;
    c.lanternberry.fruit_cap = 2e-3;
    c.lanternberry.fruit_drop_s = 1e9;
    c.lanternberry.fruit_mineral_fraction = 0.01;
    c.lanternberry.propagule_rate = 0.0;
    c.lanternberry.flower_s = 0.0;
    c.lanternberry.fruit_rest_s = 0.0;
    c
}

/// A full-grown lanternberry on column `(x, z)`; its site.
fn tree(flora: &mut Flora, w: &World, x: i64, z: u32) -> Site {
    let wood = flora.config().species(Plant::Lanternberry).wood_max;
    assert!(flora.apply(w, FloraCommand::Seed { x, z, species: Plant::Lanternberry, wood }));
    flora.view().stands.iter().find(|s| i64::from(s.site.x) == x && s.site.z == z).expect("the stand").site
}

fn crown_top(flora: &Flora, site: Site) -> f64 {
    let top = flora.view().layers_at(site).filter(|l| l.foliage_index.is_some()).map(|l| l.cell).max().expect("a crown");
    (top + 1) as f64 * VOXEL_M
}

/// A grown seedporter at `(x, z)` with its reserve at 0.9 of its cap, held still.
fn mother(f: &mut Fauna, w: &World, x: f64, z: f64) -> u64 {
    let id = introduce(f, w, SP, StartingStores { body: 1.0, reserve: 0.9 });
    place(f, id, x, z, EAST);
    drive(f, id, 0.0, false);
    id
}

/// A newborn-sized seedporter at `(x, z)` facing `heading`, its reserve at `fill` of its cap,
/// 10 s old, the young of `mother`, on its own heuristic.
fn young(f: &mut Fauna, w: &World, x: f64, z: f64, heading: f64, mother: Option<u64>, fill: f64) -> u64 {
    let core = f.config.founder(SP).core;
    let id = introduce(f, w, SP, StartingStores { body: core.body_min / core.body_max, reserve: fill });
    place(f, id, x, z, heading);
    let a = animal_mut(f, id);
    a.mother = mother;
    a.age_ticks = (10.0 * f64::from(crate::TICK_HZ)) as u64;
    let teachers = f.config.teachers;
    f.controllers.set(id, crate::heuristic(SP, teachers));
    id
}

fn ring(w: &World) -> f64 {
    f64::from(w.view().config.width) * w.view().config.voxel_m
}

/// The 3D distance between two bodies, metres.
fn apart(f: &Fauna, w: &World, a: u64, b: u64) -> f64 {
    let (p, q) = (animal(f, a).pose, animal(f, b).pose);
    let (dx, dz) = crate::pose::wrapped_delta(p.x, p.z, q.x, q.z, ring(w));
    dx.hypot(dz).hypot(p.y_m - q.y_m)
}

/// The ground distance between two bodies, metres.
fn ground_apart(f: &Fauna, w: &World, a: u64, b: u64) -> f64 {
    let (p, q) = (animal(f, a).pose, animal(f, b).pose);
    let (dx, dz) = crate::pose::wrapped_delta(p.x, p.z, q.x, q.z, ring(w));
    dx.hypot(dz)
}

fn received(f: &Fauna) -> f64 {
    f.ledger.nursed_received_by_founder[SP.index()]
}

fn perched_on(f: &Fauna, id: u64) -> Option<Site> {
    let a = animal(f, id);
    match (a.mobility.mode, a.mobility.perch.map(|p| p.anchor)) {
        (MobilityMode::Perched, Some(PerchAnchor::Stand { site, .. })) => Some(site),
        _ => None,
    }
}

fn steps(f: &mut Fauna, w: &World, flora: &mut Flora, n: u32) {
    for _ in 0..n {
        f.step_with(w, flora, 1);
    }
}

/// A hungry young 4 m west of its held mother with a fruiting lanternberry 1 m west of it:
/// `(world, flora, layer, mother, young, the fruit stand)`.
fn fruit_scene(edit: impl FnOnce(&mut Nursing)) -> (World, Flora, Fauna, u64, u64, Site) {
    let mut w = world(true);
    let mut flora = Flora::new(fruit_flora());
    let site = tree(&mut flora, &w, 10, 6);
    for _ in 0..200 {
        flora.step(&mut w);
    }
    assert!(flora.view().stand_at(site).expect("the stand").fruit > 0.0, "fixture: the stand hangs fruit");
    let mut f = Fauna::new(config(edit));
    f.set_births_enabled(false);
    let m = mother(&mut f, &w, 7.5, LANE_Z);
    let y = young(&mut f, &w, 3.5, LANE_Z, WEST, Some(m), 0.1);
    (w, flora, f, m, y, site)
}

#[test]
fn a_hungry_dependent_young_goes_to_its_mother_over_nearby_fruit_and_is_nursed() {
    // Fixture: with the follow off, this young goes for the fruit and climbs it.
    let (w, mut flora, mut f, _, y, site) = fruit_scene(|n| {
        leash_off(n);
        n.follow_m = 0.0;
    });
    let mut climbed = false;
    for _ in 0..600 {
        f.step_with(&w, &mut flora, 1);
        climbed |= perched_on(&f, y) == Some(site) || animal(&f, y).mobility.mode == MobilityMode::Climb;
    }
    assert!(climbed, "fixture: unfollowed, the young goes up the fruiting stand");

    let (w, mut flora, mut f, m, y, site) = fruit_scene(|_| {});
    let before = received(&f);
    let (mut near_fruit, mut contact) = (false, false);
    for _ in 0..600 {
        f.step_with(&w, &mut flora, 1);
        near_fruit |= perched_on(&f, y) == Some(site);
        contact |= apart(&f, &w, y, m) <= RADIUS;
    }
    assert!(!near_fruit, "it does not go up the fruit stand");
    assert!(contact, "it reaches nursing contact");
    assert!(received(&f) > before, "and its mother's reserve flows to it");
    assert!(apart(&f, &w, y, m) <= LEASH, "fed, it stays within its leash of her ({:.2} m)", apart(&f, &w, y, m));
    assert_closes(&f, "after the walk to its mother");
}

#[test]
fn a_dependent_young_reaches_its_mother_up_a_tree_and_is_nursed() {
    let w = world(false);
    let mut flora = Flora::new(FloraConfig::default());
    let site = tree(&mut flora, &w, 24, 6);
    let mut f = Fauna::new(config(|_| {}));
    f.set_births_enabled(false);
    let (sx, sz) = ((f64::from(site.x) + 0.5) * VOXEL_M, (f64::from(site.z) + 0.5) * VOXEL_M);
    let pose = Pose { x: sx, z: sz, heading_rad: EAST, y_m: crown_top(&flora, site) };
    assert!(f.apply(&w, Command::IntroduceAloft { pose, founder: SP, stores: StartingStores { body: 1.0, reserve: 0.9 } }));
    let m = f.animals.last().expect("introduced").id;
    drive(&mut f, m, 0.0, false);
    steps(&mut f, &w, &mut flora, 1);
    assert_eq!(perched_on(&f, m), Some(site), "fixture: she is perched in the crown");
    let y = young(&mut f, &w, sx - 3.0, LANE_Z, EAST, Some(m), 0.1);
    let before = received(&f);
    let mut climbed = false;
    for _ in 0..800 {
        f.step_with(&w, &mut flora, 1);
        climbed |= animal(&f, y).mobility.mode == MobilityMode::Climb;
        if received(&f) > before {
            break;
        }
    }
    assert!(climbed, "it climbs");
    let on = match animal(&f, y).mobility.perch.map(|p| p.anchor) {
        Some(PerchAnchor::Stand { site, .. }) => Some(site),
        _ => None,
    };
    assert_eq!(on, Some(site), "her stand");
    assert!(received(&f) > before, "and gets milk up there ({:.2} m from her)", apart(&f, &w, y, m));
    assert_closes(&f, "after the climb to its mother");
}

/// A young `gap_m` east of its held mother on a bare plain, its reserve over its cap (sated),
/// stepped `secs`: the layer and the ids, and whether the follow steered it.
fn sated_scene(gap_m: f64, ticks: u32) -> (World, Fauna, u64, u64, bool) {
    let w = world(false);
    let mut flora = Flora::new(FloraConfig::default());
    let mut f = Fauna::new(config(|_| {}));
    f.set_births_enabled(false);
    let m = mother(&mut f, &w, 3.0, LANE_Z);
    let y = young(&mut f, &w, 3.0 + gap_m, LANE_Z, EAST, Some(m), 1.0);
    let cap = {
        let a = animal(&f, y);
        crate::body::effective_config(&f.config, &a).reserve_of(a.body)
    };
    // Sated: a reserve well over its cap, as a newborn's is (the books are not this case's).
    animal_mut(&mut f, y).reserve = 4.0 * cap;
    let mut steered = false;
    for _ in 0..ticks {
        f.step_with(&w, &mut flora, 1);
        steered |= f.steer_of(y) == Some(Steer::Nurse);
    }
    (w, f, m, y, steered)
}

#[test]
fn a_sated_dependent_young_forages_on_its_own_within_its_leash_and_comes_back_past_it() {
    let (_, _, _, _, steered) = sated_scene(LEASH * 0.5, 40);
    assert!(!steered, "within the leash a sated young is left to its own heuristic");
    let (w, f, m, y, steered) = sated_scene(LEASH * 2.0, 100);
    assert!(steered, "past the leash it goes back");
    assert!(ground_apart(&f, &w, y, m) < LEASH * 2.0, "and is nearer her than it started ({:.2} m)", ground_apart(&f, &w, y, m));
}

/// A hungry young facing east with its mother 3 m east, sensed once; then its senses go dark
/// and she is moved 4 m west of it: where it ends, east of its start or not.
fn lost_scene(memory_s: f64) -> f64 {
    let w = world(false);
    let mut flora = Flora::new(FloraConfig::default());
    let mut f = Fauna::new(config(|n| n.mother_memory_s = memory_s));
    f.set_births_enabled(false);
    let m = mother(&mut f, &w, 6.0, LANE_Z);
    let y = young(&mut f, &w, 3.0, LANE_Z, EAST, Some(m), 0.1);
    steps(&mut f, &w, &mut flora, 20);
    assert!(f.mother_seen.contains_key(&y), "fixture: it has sensed her");
    let x0 = animal(&f, y).pose.x;
    f.config.founders[SP.index()].senses = SenseTable { live: true, ..SenseTable::default() };
    place(&mut f, m, 1.0, LANE_Z, EAST);
    steps(&mut f, &w, &mut flora, 60);
    animal(&f, y).pose.x - x0
}

#[test]
fn a_dependent_young_that_loses_its_mother_heads_to_where_it_last_sensed_her() {
    let went = lost_scene(60.0);
    assert!(went > 0.5, "it walks east, to where it last sensed her ({went:.2} m)");
    let still = lost_scene(0.0);
    assert!(still < went, "fixture: with no memory it does not ({still:.2} m vs {went:.2} m)");
}

#[test]
fn a_dependent_young_weighs_its_mother_past_a_nearer_troop_mate() {
    // One kin track: its twin beside it is the nearest of its kind; its mother is 3 m off.
    let scene = |edit: fn(&mut Nursing)| -> bool {
        let w = world(false);
        let mut f = Fauna::new(config(edit));
        f.set_births_enabled(false);
        let m = mother(&mut f, &w, 6.0, LANE_Z);
        let y = young(&mut f, &w, 3.0, LANE_Z, EAST, Some(m), 0.1);
        young(&mut f, &w, 3.3, LANE_Z, EAST, Some(m), 0.1);
        f.tick += 1;
        crate::perception::sense_pass(&mut f, &w.view(), false, 1.0, 1);
        f.percept(y).is_some_and(|p| p.tracks().iter().any(|t| t.id == m))
    };
    assert!(scene(|_| {}), "it senses its mother past its twin");
    assert!(!scene(|n| {
        leash_off(n);
        n.follow_m = 0.0;
    }), "fixture: with the follow off, one kin track holds only its twin");
}

#[test]
fn a_fleeing_dependent_young_flees_rather_than_going_to_its_mother() {
    let run = |threat: bool| -> Option<Steer> {
        let w = world(false);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config(|_| {});
        c.founders[Founder::Lanternjaw.index()].active = Activity::Always;
        let mut f = Fauna::new(c);
        f.set_births_enabled(false);
        let m = mother(&mut f, &w, 7.0, LANE_Z);
        let y = young(&mut f, &w, 3.0, LANE_Z, EAST, Some(m), 0.1);
        if threat {
            let lj = introduce(&mut f, &w, Founder::Lanternjaw, StartingStores { body: 1.0, reserve: 0.95 });
            place(&mut f, lj, 4.2, LANE_Z, WEST);
            drive(&mut f, lj, 0.0, false);
        }
        steps(&mut f, &w, &mut flora, 8);
        f.steer_of(y)
    };
    assert_eq!(run(false), Some(Steer::Nurse), "fixture: with no threat it goes to its mother");
    assert_eq!(run(true), Some(Steer::Flee), "with a threat ahead it flees");
}

/// A mixed scene stepped `ticks`: a weaned seedporter young and an orphan (its mother's id
/// names no body), each beside an adult seedporter, a loftstrider mother and calf linked,
/// and a grazer; the layer at the end.
fn mixed(edit: fn(&mut Nursing), ticks: u32) -> Fauna {
    let w = world(false);
    let mut flora = Flora::new(FloraConfig::default());
    let mut f = Fauna::new(config(edit));
    f.set_births_enabled(false);
    let m = mother(&mut f, &w, 3.0, LANE_Z);
    let weaned = young(&mut f, &w, 4.5, LANE_Z, EAST, Some(m), 0.1);
    animal_mut(&mut f, weaned).age_ticks = ((WEAN_S + 1.0) * f64::from(crate::TICK_HZ)) as u64;
    let other = mother(&mut f, &w, 6.0, LANE_Z + 0.5);
    young(&mut f, &w, 7.5, LANE_Z + 0.5, EAST, Some(other + 1000), 0.1);
    let lm = introduce(&mut f, &w, Founder::Loftstrider, StartingStores::FULL);
    place(&mut f, lm, 9.0, LANE_Z, EAST);
    let calf = introduce(&mut f, &w, Founder::Loftstrider, StartingStores { body: 0.2, reserve: 0.1 });
    place(&mut f, calf, 9.5, LANE_Z, EAST);
    animal_mut(&mut f, calf).mother = Some(lm);
    let g = introduce(&mut f, &w, Founder::Browser, StartingStores::FULL);
    place(&mut f, g, 11.0, LANE_Z, EAST);
    for id in [lm, calf, g] {
        let teachers = f.config.teachers;
        let founder = animal(&f, id).founder.expect("a founder");
        f.controllers.set(id, crate::heuristic(founder, teachers));
    }
    steps(&mut f, &w, &mut flora, ticks);
    f
}

#[test]
fn a_weaned_young_an_orphan_and_other_lineages_behave_as_with_the_follow_off() {
    let (on, off) = (mixed(|_| {}, 200), mixed(leash_off, 200));
    assert_eq!(on.animals, off.animals, "no body differs");
    assert_eq!(on.ledger, off.ledger, "no book differs");
}
