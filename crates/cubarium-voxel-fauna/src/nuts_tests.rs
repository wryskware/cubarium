//! **NUTS, from the brief** (Wrysk 2026-10-12: "vaulttree should probably have nuts … might
//! help with seedporter nutrition"; "(a) is fine. important part is thre nutrition aspect";
//! "somehow seedporters should know to liek \"check\" trees and stuff cuz they wont be able
//! to see larvae or nuts from the ground in a tall vaulttree"; the plan is
//! `design/larva-nuts-plan-2026-10-12.md` §2):
//!
//! - a nut bite moves `fruit_mineral_fraction` × organic of mineral (and energy at the
//!   density) out of the stand into the eater; at equal stock a handled fruit's bite is
//!   `fruit_handling` × an unhandled one; with `fruit_seeds_in_gut` off the gut stows no seeds;
//! - nuts are seen only within `fruit_sight_m`: from the ground under a tall crown there is no
//!   FOOD track;
//! - a hungry seedporter with no food in sight checks a nearby crowned vaulttree, climbs into
//!   its crown and eats; it goes on from an empty one and does not check it again within its
//!   memory; one remembered holding food goes before one unchecked;
//! - a sated one does not check, a fleeing one flees, a dependent young keeps to its mother;
//! - other plants' fruit and other lineages behave as with the nuts and the check off.
//!
//! Every scene sets the knobs it relies on (fast fixture fruit; the vaulttree's handling,
//! seeds and sight are read from the config); no shipped number, ratio or world hash is
//! asserted.

use crate::{
    Activity, Actions, Command, Fauna, FaunaConfig, Food, Founder, Herding, MobilityMode, Nursing, PerchAnchor, Pose,
    Scripted, SenseTable, StartingStores, Steer,
};
use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};

const V: f64 = 0.25;
const SP: Founder = Founder::Seedporter;
const EAST: f64 = std::f64::consts::FRAC_PI_2;
const WEST: f64 = -std::f64::consts::FRAC_PI_2;
/// Hungry enough to eat, an adult's body.
const HUNGRY: StartingStores = StartingStores { body: 1.0, reserve: 0.3 };
/// The trees' row: `z = 12`, 3 m in.
const ROW: u32 = 12;
/// Fast fixture nuts: full in 2 s of daylight, never falling, no rest.
const NUT_RATE: f64 = 1e-3;
const NUT_CAP: f64 = 2e-3;

// ------------------------------------------------------------------ fixtures

/// A wet soil plain 24 m around (ring in x), 6 m deep and 10 m tall at 0.25 m, held at noon.
fn world() -> World {
    let (width, depth) = (96u32, 24u32);
    let mut w = World::empty(VoxelConfig {
        width,
        height: 40,
        depth,
        voxel_m: V,
        seed: 29,
        day: DayClock { start_hour: 12.0, ..DayClock::default() },
        ..VoxelConfig::default()
    });
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
    assert!(w.life_daylight() > 0.5, "fixture: noon");
    w
}

/// The vaulttree's nut crop at fixture speed (its handling, seeds and sight as configured);
/// no lanternberry fruit; no seed parcels.
fn nut_flora() -> FloraConfig {
    let mut c = FloraConfig::default();
    let t = &mut c.vaulttree;
    t.fruit_per_s = NUT_RATE;
    t.fruit_cap = NUT_CAP;
    t.fruit_drop_s = 1e9;
    t.fruit_rest_s = 1e9;
    t.flower_s = 0.0;
    t.propagule_rate = 0.0;
    c.lanternberry.fruit_per_s = 0.0;
    c.lanternberry.propagule_rate = 0.0;
    c
}

/// Every lineage awake, no ground response, no troop pull, no bellwing catching; the
/// seedporter's senses live and all-round at 16 m, sensed every tick.
fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for p in &mut c.founders {
        p.active = Activity::Always;
        p.vigilance = crate::Vigilance::default();
        p.catch_bellwings = false;
        p.herding = Herding::default();
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.founders[SP.index()].senses = SenseTable {
        live: true,
        sight_m: 16.0,
        sight_night_m: 16.0,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: 180.0,
        kin_tracks: 1,
        ..SenseTable::default()
    };
    c
}

/// A full-grown vaulttree on column `(x, ROW)`; its site.
fn vaulttree(flora: &mut Flora, w: &World, x: i64) -> Site {
    let wood = flora.config().species(Plant::Vaulttree).wood_max;
    assert!(flora.apply(w, FloraCommand::Seed { x, z: ROW, species: Plant::Vaulttree, wood }));
    let s = *flora.view().stands.iter().find(|s| i64::from(s.site.x) == x && s.site.z == ROW).expect("the stand");
    assert!(s.profile_stage >= 1, "fixture: a crowned vaulttree");
    s.site
}

fn lanternberry(flora: &mut Flora, w: &World, x: i64, z: u32) -> Site {
    let wood = flora.config().species(Plant::Lanternberry).wood_max;
    assert!(flora.apply(w, FloraCommand::Seed { x, z, species: Plant::Lanternberry, wood }));
    flora.view().stands.iter().find(|s| i64::from(s.site.x) == x && s.site.z == z).expect("the stand").site
}

fn grow(flora: &mut Flora, w: &mut World, ticks: usize) {
    for _ in 0..ticks {
        flora.step(w);
    }
}

fn fruit(flora: &Flora, site: Site) -> f64 {
    flora.view().stand_at(site).expect("the stand").fruit
}

fn stand_id(flora: &Flora, site: Site) -> u64 {
    flora.view().stand_at(site).expect("the stand").id
}

/// The top of a stand's crown, metres.
fn crown_top(flora: &Flora, site: Site) -> f64 {
    let top = flora.view().layers_at(site).filter(|l| l.foliage_index.is_some()).map(|l| l.cell).max().expect("a crown");
    (top + 1) as f64 * V
}

fn centre(site: Site) -> (f64, f64) {
    ((f64::from(site.x) + 0.5) * V, (f64::from(site.z) + 0.5) * V)
}

/// A seedporter on the ground at metres `(x_m, ROW)` facing `heading`, on its own heuristic.
fn on_ground(f: &mut Fauna, w: &World, x_m: f64, heading: f64, stores: StartingStores) -> u64 {
    let x = (x_m / V).floor() as i64;
    assert!(f.apply(w, Command::IntroduceFounder { x, z: ROW, founder: SP, stores, heading_rad: heading }));
    let id = f.animals.last().expect("introduced").id;
    let teachers = f.config.teachers;
    f.controllers.set(id, crate::heuristic(SP, teachers));
    id
}

/// A seedporter perched on the crown top of `site`, holding still and feeding.
fn perched(f: &mut Fauna, w: &World, flora: &Flora, site: Site, stores: StartingStores) -> u64 {
    let (x, z) = centre(site);
    let y_m = crown_top(flora, site);
    assert!(f.apply(w, Command::IntroduceAloft { pose: Pose { x, z, heading_rad: EAST, y_m }, founder: SP, stores }));
    let id = f.animals.last().expect("introduced").id;
    assert!(f.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 0.0, turn: 0.0, feed: 1.0 }]))));
    id
}

fn animal(f: &Fauna, id: u64) -> crate::Animal {
    *f.view().animal(id).expect("alive")
}

fn animal_mut(f: &mut Fauna, id: u64) -> &mut crate::Animal {
    f.animals.iter_mut().find(|a| a.id == id).expect("alive")
}

/// The stand a body is perched on.
fn perched_on(f: &Fauna, id: u64) -> Option<Site> {
    let a = animal(f, id);
    match (a.mobility.mode, a.mobility.perch.map(|p| p.anchor)) {
        (MobilityMode::Perched, Some(PerchAnchor::Stand { site, .. })) => Some(site),
        _ => None,
    }
}

fn step(f: &mut Fauna, w: &World, flora: &mut Flora) {
    f.step_with(w, flora, 1);
}

fn nuts_eaten(f: &Fauna) -> f64 {
    f.ledger.fruit_eaten_by_plant[Plant::Vaulttree.index()]
}

/// Both layers' books close.
fn closes(f: &Fauna, flora: &Flora, when: &str) {
    let (av, fv) = (f.view(), flora.view());
    for (name, got, want) in [
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
    ] {
        assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "{when}: the {name} ledger must close: {got} against {want}");
    }
}

/// A vaulttree hanging fixture nuts at `x` with a hungry seedporter perched on its crown,
/// holding still to feed, under `flora_cfg`: `(world, flora, layer, body, tree)`.
fn crown_scene(flora_cfg: FloraConfig, stores: StartingStores) -> (World, Flora, Fauna, u64, Site) {
    let mut w = world();
    let mut flora = Flora::new(flora_cfg);
    let site = vaulttree(&mut flora, &w, 40);
    grow(&mut flora, &mut w, 200);
    assert!(fruit(&flora, site) > 0.0, "fixture: the vaulttree hangs nuts");
    let mut f = Fauna::new(config());
    f.set_births_enabled(false);
    let id = perched(&mut f, &w, &flora, site, stores);
    let mut flora = flora;
    if perched_on(&f, id).is_none() {
        step(&mut f, &w, &mut flora);
    }
    (w, flora, f, id, site)
}

/// Step until the first fruit bite or `cap` ticks.
fn first_bite(f: &mut Fauna, w: &World, flora: &mut Flora, cap: usize) {
    for _ in 0..cap {
        if f.ledger.bites_by_food[Food::Fruit.index()] > 0 {
            return;
        }
        step(f, w, flora);
    }
    panic!("no fruit bite within {cap} ticks");
}

// ------------------------------------------------------------------ the nut and its bite

/// **A nut bite moves `fruit_mineral_fraction` × organic of mineral, and energy at the
/// species' density, out of the stand into the eater; nothing appears.**
#[test]
fn a_nut_bite_moves_the_mineral_share_and_its_energy_from_the_stand_into_the_eater() {
    let (w, mut flora, mut f, id, site) = crown_scene(nut_flora(), HUNGRY);
    assert_eq!(perched_on(&f, id), Some(site), "fixture: perched on the crown");
    let sc = flora.config().species(Plant::Vaulttree).clone();
    let s0 = *flora.view().stand_at(site).expect("the stand");
    let l0 = f.ledger;
    first_bite(&mut f, &w, &mut flora, 400);
    let s1 = *flora.view().stand_at(site).expect("the stand");
    let took = s0.fruit - s1.fruit;
    assert!(took > 0.0, "a bite came off the crop");
    assert!(sc.fruit_mineral_fraction * took < s0.mineral, "fixture: the stand's mineral does not bind");
    let mineral = s0.mineral - s1.mineral;
    assert!((mineral - sc.fruit_mineral_fraction * took).abs() <= 1e-12, "the stand gives {mineral} of mineral for {took} of nut");
    let (o, m, e) = (f.ledger.eaten_organic_in - l0.eaten_organic_in, f.ledger.eaten_mineral_in - l0.eaten_mineral_in, f.ledger.eaten_energy_in - l0.eaten_energy_in);
    assert!((o - took).abs() <= 1e-12, "the eater books the organic it took: {o} against {took}");
    assert!((m - mineral).abs() <= 1e-12, "the eater books the mineral the stand gave: {m} against {mineral}");
    assert!((e - sc.energy_density * took).abs() <= 1e-12, "energy at the density: {e}");
    assert!((nuts_eaten(&f) - took).abs() <= 1e-12, "booked as vaulttree fruit");
    closes(&f, &flora, "after the bite");
}

/// **At equal stock and time, a handled fruit's bite is `fruit_handling` × an unhandled one.**
#[test]
fn a_handled_fruits_bite_is_its_handling_times_a_plain_one() {
    let h = FloraConfig::default().vaulttree.fruit_handling;
    assert!(h > 0.0 && h < 1.0, "fixture: the vaulttree's nuts are handled");
    let bite = |handling: f64| {
        let mut c = nut_flora();
        c.vaulttree.fruit_handling = handling;
        let (w, mut flora, mut f, _, site) = crown_scene(c, HUNGRY);
        let before = fruit(&flora, site);
        first_bite(&mut f, &w, &mut flora, 400);
        before - fruit(&flora, site)
    };
    let (handled, plain) = (bite(h), bite(1.0));
    assert!(plain > 0.0 && handled < plain, "fixture: both bite, the handled less");
    assert!((handled - h * plain).abs() <= 1e-9 * plain, "handled {handled} = {h} × plain {plain}");
}

/// **With `fruit_seeds_in_gut` off the gut stows no seeds; on, as before.**
#[test]
fn an_eaten_nut_stows_no_seeds_and_a_seeded_fruit_still_does() {
    let seeds = |ride: bool| {
        let mut c = nut_flora();
        c.vaulttree.fruit_seeds_in_gut = ride;
        let (w, mut flora, mut f, id, _) = crown_scene(c, HUNGRY);
        first_bite(&mut f, &w, &mut flora, 400);
        let a = animal(&f, id);
        let organic: f64 = a.gut_seeds.lots.iter().map(|l| l.organic).sum();
        (organic, a.gut_seeds.lots.iter().any(|l| l.plant.is_some()))
    };
    assert_eq!(seeds(false), (0.0, false), "a nut is the seed: none ride in the gut");
    let (organic, any) = seeds(true);
    assert!(organic > 0.0 && any, "with the switch on the seeds ride as before");
    assert!(!FloraConfig::default().vaulttree.fruit_seeds_in_gut, "the vaulttree's nut is a dead seed");
}

// ------------------------------------------------------------------ hidden food

/// **A seedporter on the ground beyond a vaulttree's nut sight does not see its nuts**: no
/// FOOD track names the tree, and its fruit is not among the still fruit it sees; with no
/// sight limit the same tree is seen (the knob, not the scene, hides it).
#[test]
fn nuts_beyond_their_sight_give_no_food_track() {
    let food_of = |sight_m: f64| -> (bool, bool, f64) {
        let mut c = nut_flora();
        c.vaulttree.fruit_sight_m = sight_m;
        let mut w = world();
        let mut flora = Flora::new(c);
        let site = vaulttree(&mut flora, &w, 40);
        grow(&mut flora, &mut w, 200);
        assert!(fruit(&flora, site) > 0.0, "fixture: nuts hang");
        let mut cfg = config();
        cfg.founders[SP.index()].glider.as_mut().expect("a glider").check_search_m = 0.0;
        let mut f = Fauna::new(cfg);
        f.set_births_enabled(false);
        let (sx, _) = centre(site);
        let id = on_ground(&mut f, &w, sx - 3.0, EAST, HUNGRY);
        for _ in 0..8 {
            step(&mut f, &w, &mut flora);
        }
        let tree = stand_id(&flora, site);
        let food = crate::perception::class::FOOD as usize;
        let track = f.percepts.get(&id).map(|p| p.best[food]).unwrap_or_default();
        let i = f.animals.iter().position(|a| a.id == id).expect("alive");
        let range = f.config.founder(SP).senses.sight_m;
        let seen = crate::step::glider_fruit_candidates(&f, i, &w.view(), &flora.view(), range)
            .unwrap_or_default()
            .iter()
            .any(|c| c.1 == tree);
        // The eye's distance to the crown's middle.
        let a = animal(&f, id);
        let my = crate::surface_m(site.y, V) + 0.5 * flora.config().species(Plant::Vaulttree).crown_height_m_at(flora.view().stand_at(site).expect("tree").wood);
        let (dx, _) = (sx - a.pose.x, 0.0);
        let d = dx.abs().hypot(my - a.pose.y_m);
        (track.known && track.id == tree, seen, d)
    };
    let sight = FloraConfig::default().vaulttree.fruit_sight_m;
    let (tracked, seen, d) = food_of(sight);
    assert!(d > sight, "fixture: the body stands beyond the nuts' sight ({d} m > {sight} m)");
    assert!(!tracked && !seen, "beyond its sight the nuts give no FOOD track (tracked {tracked}, seen {seen})");
    let (tracked, seen, _) = food_of(0.0);
    assert!(tracked && seen, "with no sight limit the same nuts are its food");
}

// ------------------------------------------------------------------ the check

/// **A hungry seedporter with no food in sight goes to a nearby crowned vaulttree holding
/// nuts, climbs into its crown and eats them** — walking for the check, not for a sight.
#[test]
fn a_hungry_seedporter_checks_a_vaulttree_climbs_into_its_crown_and_eats_its_nuts() {
    let mut w = world();
    let mut flora = Flora::new(nut_flora());
    let site = vaulttree(&mut flora, &w, 40);
    grow(&mut flora, &mut w, 200);
    let before = fruit(&flora, site);
    assert!(before > 0.0, "fixture: nuts hang");
    let mut f = Fauna::new(config());
    f.set_births_enabled(false);
    let (sx, _) = centre(site);
    let id = on_ground(&mut f, &w, sx - 5.0, WEST, HUNGRY);
    let (mut checked, mut climbed, mut on_tree_at_bite) = (false, false, None);
    for _ in 0..6000 {
        step(&mut f, &w, &mut flora);
        checked |= f.steer_of(id) == Some(Steer::Check);
        climbed |= animal(&f, id).mobility.mode == MobilityMode::Climb;
        if nuts_eaten(&f) > 0.0 {
            let a = animal(&f, id);
            on_tree_at_bite = Some(matches!(a.mobility.perch.map(|p| p.anchor), Some(PerchAnchor::Stand { site: s, .. }) if s == site));
            break;
        }
    }
    assert!(checked, "it walks to the tree on the check");
    assert!(climbed, "it climbs");
    assert_eq!(on_tree_at_bite, Some(true), "it eats the nuts from the tree it climbed: {:?}", animal(&f, id).mobility);
    assert!(fruit(&flora, site) < before, "the tree lost nuts");
    let a = animal(&f, id);
    let track = f.percepts.get(&id).map(|p| p.best[crate::perception::class::FOOD as usize]).unwrap_or_default();
    assert!(!(track.known && track.id == stand_id(&flora, site)) || a.mobility.mode != MobilityMode::Ground, "no sight of the nuts sent it");
    closes(&f, &flora, "after the meal");
}

/// Two full vaulttrees on the row, `near` at 6 m and `far` at 12 m (one with nuts, one
/// eaten out), and a hungry seedporter at 3 m facing west: `(world, flora, layer, body, near,
/// far)`. The empty tree's crop is taken off and does not come back.
fn two_trees(empty_near: bool) -> (World, Flora, Fauna, u64, Site, Site) {
    let mut w = world();
    let mut flora = Flora::new(nut_flora());
    let near = vaulttree(&mut flora, &w, 24);
    let far = vaulttree(&mut flora, &w, 48);
    grow(&mut flora, &mut w, 200);
    let empty = if empty_near { near } else { far };
    assert!(flora.take_stand_fruit(empty, 1e9).is_some(), "fixture: the crop is taken");
    grow(&mut flora, &mut w, 2);
    assert_eq!(fruit(&flora, empty), 0.0, "fixture: one tree is empty");
    assert!(fruit(&flora, if empty_near { far } else { near }) > 0.0, "fixture: the other hangs nuts");
    let mut f = Fauna::new(config());
    f.set_births_enabled(false);
    let id = on_ground(&mut f, &w, 3.0, WEST, HUNGRY);
    (w, flora, f, id, near, far)
}

/// **After checking an empty tree it goes on to another and does not check the first again
/// within its memory**: the nearer, empty tree is checked first; the farther one is checked
/// and eaten from; the first is not perched on again.
#[test]
fn after_an_empty_tree_it_checks_another_and_not_the_empty_one_again() {
    let (w, mut flora, mut f, id, near, far) = two_trees(true);
    let mut visits: Vec<Site> = Vec::new();
    let mut ate_on = None;
    for _ in 0..12_000 {
        step(&mut f, &w, &mut flora);
        if let Some(s) = perched_on(&f, id)
            && visits.last() != Some(&s)
        {
            visits.push(s);
        }
        if nuts_eaten(&f) > 0.0 {
            ate_on = match animal(&f, id).mobility.perch.map(|p| p.anchor) {
                Some(PerchAnchor::Stand { site, .. }) => Some(site),
                _ => None,
            };
            break;
        }
    }
    assert_eq!(visits.first(), Some(&near), "it checks the nearer tree first: {visits:?}");
    assert!(nuts_eaten(&f) > 0.0, "it goes on and eats at the other: {visits:?}, {:?}", animal(&f, id).mobility);
    assert_eq!(visits.iter().filter(|&&s| s == near).count(), 1, "it does not check the empty tree again: {visits:?}");
    assert_eq!(ate_on, Some(far), "the meal is at the other tree: {visits:?}");
    let memory = f.tree_checks_of(id).expect("a memory");
    let near_id = stand_id(&flora, near);
    assert!(memory.seen.iter().any(|c| c.host == near_id && !c.held_food), "it remembers the empty tree empty");
    let t = f.check_tally();
    assert!(t.checks >= 2 && t.found >= 1 && t.checkers == 1, "the readout counts the checks: {t:?}");
}

/// **A remembered-empty tree is dropped from the check until its memory lapses**; a tree
/// remembered holding food goes before an unchecked one at equal distance, whichever is which.
#[test]
fn the_checks_order_drops_remembered_empty_trees_until_the_memory_lapses_and_puts_fed_ones_first() {
    use crate::treecheck::{Checked, order};
    let memory = 100u64;
    let now = 1_000u64;
    let cands = [(4.0, 7u64, ()), (4.0, 9u64, ())];
    let recall_of = |seen: Vec<Checked>| move |host: u64| seen.iter().find(|c| c.host == host && now.saturating_sub(c.tick) < memory).copied();
    let ids = |v: Vec<(f64, u64, ())>| v.into_iter().map(|c| c.1).collect::<Vec<_>>();
    // Empty within the memory: dropped; lapsed: back, as unchecked.
    assert_eq!(ids(order(&cands, None, recall_of(vec![Checked { host: 7, tick: now - 10, held_food: false }]))), vec![9]);
    assert_eq!(ids(order(&cands, None, recall_of(vec![Checked { host: 7, tick: now - memory, held_food: false }]))), vec![7, 9]);
    // Fed first, whichever id it carries.
    assert_eq!(ids(order(&cands, None, recall_of(vec![Checked { host: 9, tick: now - 10, held_food: true }]))), vec![9, 7]);
    assert_eq!(ids(order(&cands, None, recall_of(vec![Checked { host: 7, tick: now - 10, held_food: true }]))), vec![7, 9]);
    // The tree it is walking to stays first, over a fed one; not if remembered empty.
    assert_eq!(ids(order(&cands, Some(9), recall_of(vec![Checked { host: 7, tick: now - 10, held_food: true }]))), vec![9, 7]);
    assert_eq!(ids(order(&cands, Some(9), recall_of(vec![Checked { host: 9, tick: now - 10, held_food: false }]))), vec![7]);
    // In the layer: a remembered-empty host is not offered within the memory, and is after it.
    let (w, flora, mut f, id, near, _) = two_trees(true);
    let host = stand_id(&flora, near);
    let g = f.config.founder(SP).glider.expect("a glider");
    let ticks = cubarium_voxel::ticks_at_least_one(g.check_memory_s);
    f.tree_checks.entry(id).or_default().seen.push(Checked { host, tick: 0, held_food: false });
    let _ = w;
    assert!(crate::treecheck::recall(&f, id, host, ticks - 1, ticks).is_some(), "remembered within its memory");
    assert!(crate::treecheck::recall(&f, id, host, ticks, ticks).is_none(), "forgotten once its memory lapses");
}

/// **A tree remembered holding food is checked before an unchecked one at equal distance**:
/// a seedporter midway between two trees with nuts goes first to the one it remembers fed,
/// whichever side it is on.
#[test]
fn a_tree_remembered_with_food_is_checked_before_an_unchecked_one_at_equal_distance() {
    let first = |remember_east: bool| -> Option<Site> {
        let mut w = world();
        let mut flora = Flora::new(nut_flora());
        let west = vaulttree(&mut flora, &w, 24);
        let east = vaulttree(&mut flora, &w, 56);
        grow(&mut flora, &mut w, 200);
        let mut f = Fauna::new(config());
        f.set_births_enabled(false);
        let id = on_ground(&mut f, &w, 10.125, 0.0, HUNGRY);
        let host = stand_id(&flora, if remember_east { east } else { west });
        f.tree_checks.entry(id).or_default().seen.push(crate::treecheck::Checked { host, tick: 0, held_food: true });
        for _ in 0..6000 {
            step(&mut f, &w, &mut flora);
            if let Some(s) = perched_on(&f, id) {
                return Some(s);
            }
        }
        None
    };
    let mut w = world();
    let mut flora = Flora::new(nut_flora());
    let (west, east) = (vaulttree(&mut flora, &w, 24), vaulttree(&mut flora, &w, 56));
    grow(&mut flora, &mut w, 1);
    let mid = 0.5 * (centre(west).0 + centre(east).0);
    assert!((mid - 10.125).abs() < 1e-9, "fixture: the body stands midway");
    assert_eq!(first(true), Some(east), "remembered fed to the east, it goes east");
    assert_eq!(first(false), Some(west), "remembered fed to the west, it goes west");
}

// ------------------------------------------------------------------ precedence

/// **A sated seedporter does not check.**
#[test]
fn a_sated_seedporter_does_not_check() {
    let run = |sated: bool| -> (bool, u64) {
        let mut w = world();
        let mut flora = Flora::new(nut_flora());
        let site = vaulttree(&mut flora, &w, 40);
        grow(&mut flora, &mut w, 200);
        let mut f = Fauna::new(config());
        f.set_births_enabled(false);
        let (sx, _) = centre(site);
        let id = on_ground(&mut f, &w, sx - 5.0, WEST, HUNGRY);
        if sated {
            // Sated: a full stomach, the eating latch off (the books are not this case's).
            let gut = f.config.founder(SP).gut;
            let a = animal_mut(&mut f, id);
            a.stomach.pools[Food::Fruit.index()].organic = gut.capacity(a.body);
            a.eating = false;
        }
        let mut checked = false;
        for _ in 0..200 {
            step(&mut f, &w, &mut flora);
            checked |= f.steer_of(id) == Some(Steer::Check);
        }
        (checked, f.check_tally().walks)
    };
    assert_eq!(run(true), (false, 0), "sated, it does not check");
    assert!(run(false).0, "fixture: hungry, the same body checks");
}

/// **A fleeing seedporter flees** rather than checking.
#[test]
fn a_fleeing_seedporter_flees_rather_than_checking() {
    let run = |threat: bool| -> Option<Steer> {
        let mut w = world();
        let mut flora = Flora::new(nut_flora());
        let site = vaulttree(&mut flora, &w, 40);
        grow(&mut flora, &mut w, 200);
        let mut f = Fauna::new(config());
        f.set_births_enabled(false);
        let (sx, _) = centre(site);
        let id = on_ground(&mut f, &w, sx - 5.0, EAST, HUNGRY);
        if threat {
            let x = ((sx - 5.0 + 1.2) / V).floor() as i64;
            assert!(f.apply(&w, Command::IntroduceFounder { x, z: ROW, founder: Founder::Lanternjaw, stores: StartingStores { body: 1.0, reserve: 0.95 }, heading_rad: WEST }));
            let lj = f.animals.last().expect("introduced").id;
            assert!(f.set_controller(lj, Box::new(Scripted::new(vec![Actions::REST]))));
        }
        for _ in 0..8 {
            step(&mut f, &w, &mut flora);
        }
        f.steer_of(id)
    };
    assert_eq!(run(false), Some(Steer::Check), "fixture: with no threat it checks the tree");
    assert_eq!(run(true), Some(Steer::Flee), "with a threat ahead it flees");
}

/// **A dependent young still keeps to its mother** rather than checking; with the follow off
/// the same young checks.
#[test]
fn a_dependent_young_keeps_to_its_mother_rather_than_checking() {
    let run = |follow: bool| -> Option<Steer> {
        let mut w = world();
        let mut flora = Flora::new(nut_flora());
        let site = vaulttree(&mut flora, &w, 40);
        grow(&mut flora, &mut w, 200);
        let mut c = config();
        c.founders[SP.index()].nursing = Nursing {
            weaning_age_s: 1000.0,
            nurse_radius_m: 0.5,
            nurse_rate_per_s: 1.0,
            nurse_reserve_floor: 0.3,
            nurse_efficiency: 0.7,
            follow_m: 0.4,
            follow_pull: 0.8,
            leash_m: if follow { 3.0 } else { 0.0 },
            mother_memory_s: if follow { 60.0 } else { 0.0 },
        };
        let mut f = Fauna::new(c);
        f.set_births_enabled(false);
        let (sx, _) = centre(site);
        let m = on_ground(&mut f, &w, sx - 9.0, EAST, StartingStores { body: 1.0, reserve: 0.9 });
        assert!(f.set_controller(m, Box::new(Scripted::new(vec![Actions::REST]))));
        let core = f.config.founder(SP).core;
        let y = on_ground(&mut f, &w, sx - 5.0, EAST, StartingStores { body: core.body_min / core.body_max, reserve: 0.1 });
        let a = animal_mut(&mut f, y);
        a.mother = Some(m);
        a.age_ticks = (10.0 * f64::from(crate::TICK_HZ)) as u64;
        for _ in 0..8 {
            step(&mut f, &w, &mut flora);
        }
        f.steer_of(y)
    };
    assert_eq!(run(true), Some(Steer::Nurse), "a hungry dependent young goes to its mother");
    assert_eq!(run(false), Some(Steer::Check), "fixture: with the follow off the same young checks");
}

// ------------------------------------------------------------------ the rest unchanged

/// **Other lineages and other plants' fruit behave as with the nuts and the check off**: a
/// hungry seedporter with a fruiting lanternberry in sight and a vaulttree (hanging nuts, or
/// none with the check off) behind, beside a grazer; every body and book the same.
#[test]
fn other_lineages_and_other_fruit_behave_as_with_the_nuts_and_the_check_off() {
    let run = |on: bool| -> Fauna {
        let mut w = world();
        let mut c = nut_flora();
        // An ample bush: its crop outlasts the run, so the body stays at it.
        c.lanternberry.fruit_per_s = 1e-3;
        c.lanternberry.fruit_cap = 0.2;
        c.lanternberry.fruit_drop_s = 1e9;
        c.lanternberry.flower_s = 0.0;
        c.lanternberry.fruit_rest_s = 0.0;
        if !on {
            c.vaulttree.fruit_per_s = 0.0;
        }
        let mut flora = Flora::new(c);
        let tree = vaulttree(&mut flora, &w, 24);
        let bush = lanternberry(&mut flora, &w, 52, ROW);
        grow(&mut flora, &mut w, 200);
        assert!(fruit(&flora, bush) > 0.0, "fixture: the bush fruits");
        assert_eq!(fruit(&flora, tree) > 0.0, on, "fixture: nuts only with the knobs on");
        let mut cfg = config();
        if !on {
            cfg.founders[SP.index()].glider.as_mut().expect("a glider").check_search_m = 0.0;
        }
        let mut f = Fauna::new(cfg);
        f.set_births_enabled(false);
        on_ground(&mut f, &w, 10.0, EAST, HUNGRY);
        let x = (16.0 / V) as i64;
        assert!(f.apply(&w, Command::IntroduceFounder { x, z: ROW + 6, founder: Founder::Browser, stores: StartingStores::FULL, heading_rad: EAST }));
        let g = f.animals.last().expect("introduced").id;
        let teachers = f.config.teachers;
        f.controllers.set(g, crate::heuristic(Founder::Browser, teachers));
        for _ in 0..400 {
            step(&mut f, &w, &mut flora);
        }
        f
    };
    let (on, off) = (run(true), run(false));
    assert!(on.ledger.eaten_by_food[Food::Fruit.index()] > 0.0, "fixture: the seedporter eats the bush's fruit");
    assert_eq!(on.animals, off.animals, "no body differs");
    assert_eq!(on.ledger, off.ledger, "no book differs");
}
