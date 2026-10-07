//! **TEST-PASS-4 (independent), PUP through the step**: the ledgers close through a
//! regurgitated meal and a pup that starves on an empty reserve (its corpse into the plant
//! layer's carrion); the pup readouts split a starved pup from one lost any other way and
//! never count an adult; the new pack switches, fraction and readouts survive a postcard
//! round trip at the current schema.
//!
//! A child of `step` beside the predation files, sharing their fixture. Every body stands
//! still under a scripted controller. Every case sets the config it relies on.

use super::predation_fixture::*;
use crate::controller::{Actions, Controller, Scripted};
use crate::{Activity, Command, Departure, Fauna, FaunaConfig, Food, Founder, StartingStores};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site, Taken};

const C: Founder = Founder::Chorister;

fn den() -> Site {
    Site { x: 8, y: SURFACE_Y, z: 6 }
}

fn still() -> Box<dyn Controller> {
    Box::new(Scripted::new(vec![Actions { forward: 0.0, turn: 0.0, feed: 0.0 }]))
}

fn config(edit: impl FnOnce(&mut crate::Pack)) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.founders[C.index()].active = Activity::Always;
    let mut p = c.founders[C.index()].pack.expect("pack lineage");
    p.pack_size = 1;
    edit(&mut p);
    c.founders[C.index()].pack = Some(p);
    c
}

/// One adult and `young` pups at the den, all standing still.
fn den_scene(fauna: &mut Fauna, world: &World, young: u32) -> (u64, Vec<u64>) {
    assert!(fauna.apply(world, Command::FoundPack { den: den(), founder: C, stores: StartingStores::FULL, heading_rad: EAST }));
    let pack = fauna.packs().last().unwrap().id;
    let adult = fauna.animals.iter().find(|a| a.pack.is_some_and(|m| m.pack == pack)).unwrap().id;
    let pups = fauna.whelp(&world.view(), pack, young).expect("the breeder pays");
    let (x, z) = ((f64::from(den().x) + 0.5) * VOXEL_M, (f64::from(den().z) + 0.5) * VOXEL_M);
    place(fauna, adult, x, z, EAST);
    fauna.controllers.set(adult, still());
    for (k, &id) in pups.iter().enumerate() {
        place(fauna, id, x - 0.3 * k as f64, z + 0.25, EAST);
        fauna.controllers.set(id, still());
    }
    (adult, pups)
}

/// Carrion in `id`'s stomach, booked introduced so the layer's books stay whole.
fn fill(fauna: &mut Fauna, id: u64, organic: f64) {
    let t = Taken { organic, mineral: 0.01 * organic, energy: 2.0 * organic };
    let a = animal_mut(fauna, id);
    a.stomach = crate::Stomach::default();
    a.stomach.pools[Food::Carrion.index()] = t;
    let l = &mut fauna.ledger;
    l.introduced_organic_in += t.organic;
    l.introduced_mineral_in += t.mineral;
    l.introduced_energy_in += t.energy;
}

/// Empty `id`'s stomach and reserve.
fn empty(fauna: &mut Fauna, id: u64) {
    let a = animal_mut(fauna, id);
    a.stomach = crate::Stomach::default();
    a.reserve = 0.0;
}

/// A fixture edited bodies outside any flow: book the difference as introduced, so the
/// books describe what the scene holds before the first tick.
fn rebase(fauna: &mut Fauna) {
    let v = fauna.view();
    let (o, m, e) = (v.organic() - v.ledger.expected_organic(), v.mineral() - v.ledger.expected_mineral(), v.energy() - v.ledger.expected_energy());
    fauna.ledger.introduced_organic_in += o;
    fauna.ledger.introduced_mineral_in += m;
    fauna.ledger.introduced_energy_in += e;
}

#[test]
fn the_ledgers_close_through_a_regurgitated_meal_and_a_pup_that_starves() {
    let world = clock_off(41);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(|p| {
        p.regurgitate_reserve_fraction = 0.5;
        p.provision_scramble = true;
        p.pup_starves_on_empty = true;
    }));
    fauna.set_births_enabled(false);
    let (adult, pups) = den_scene(&mut fauna, &world, 2);
    let k = fauna.config.founder(C).gut.capacity(animal(&fauna, adult).body);
    fill(&mut fauna, adult, 0.5 * k);
    // Pup 0 begs and is fed; pup 1 has spent its reserve and has nothing in its stomach.
    animal_mut(&mut fauna, pups[0]).eating = true;
    empty(&mut fauna, pups[1]);
    rebase(&mut fauna);
    assert_closes(&fauna, "fixture");
    crate::testpass3_support::ledgers(&fauna, &flora);
    let starved0 = fauna.ledger.pups_starved;
    for t in 0..20 {
        fauna.step_with(&world, &mut flora, 1);
        assert_closes(&fauna, &format!("tick {t}"));
        crate::testpass3_support::ledgers(&fauna, &flora);
    }
    assert!(fauna.ledger.regurgitated.organic > 0.0, "fixture: a meal was regurgitated");
    assert!(fauna.view().animal(pups[1]).is_none(), "fixture: the spent pup starved");
    assert_eq!(fauna.ledger.pups_starved - starved0, 1);
    assert_eq!(fauna.ledger.departed_founder(C, Departure::Starved), 1);
}

#[test]
fn a_pup_lost_to_another_cause_is_not_counted_starved_and_an_adult_is_never_a_pup() {
    let world = clock_off(42);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(|p| {
        p.pup_starves_on_empty = false;
        p.provision_rate_per_s = 0.0;
    }));
    fauna.set_births_enabled(false);
    let (adult, pups) = den_scene(&mut fauna, &world, 2);
    let k = fauna.config.founder(C).gut.capacity(animal(&fauna, pups[0]).body);
    // Pup 0 fed but wounded past 1 (healing runs before the death check, so a margin of one
    // tick's healing); pup 1 and the adult with no stores and
    // their structure under body_min (the switch is off: only structure starves a pup here).
    fill(&mut fauna, pups[0], 0.5 * k);
    let heal = fauna.config.predation.wound_heal_per_s * crate::DT;
    animal_mut(&mut fauna, pups[0]).wound = 1.0 + 2.0 * heal;
    for id in [pups[1], adult] {
        let min = crate::body::effective_config(&fauna.config, &animal(&fauna, id)).body_min;
        empty(&mut fauna, id);
        animal_mut(&mut fauna, id).body = 0.5 * min;
    }
    rebase(&mut fauna);
    fauna.step_with(&world, &mut flora, 1);
    for id in [adult, pups[0], pups[1]] {
        assert!(fauna.view().animal(id).is_none(), "fixture: {id} died this tick");
    }
    assert_eq!(fauna.ledger.pups_starved, 1, "only the pup under body_min starved");
    assert_eq!(fauna.ledger.pups_lost_other, 1, "the wounded pup is lost to another cause");
    assert_eq!(fauna.ledger.departed_founder(C, Departure::Starved), 2, "the adult starved too, as an adult");
    // Everything has left the layer: the books close at zero (to rounding), and the plant
    // layer holds the corpses.
    crate::testpass3_support::ledgers(&fauna, &flora);
}

#[test]
fn pup_switches_fraction_and_readouts_survive_a_postcard_round_trip() {
    let world = clock_off(43);
    let mut fauna = Fauna::new(config(|p| {
        p.regurgitate_reserve_fraction = 0.375;
        p.provision_scramble = true;
        p.pup_starves_on_empty = true;
    }));
    let (_, pups) = den_scene(&mut fauna, &world, 1);
    animal_mut(&mut fauna, pups[0]).eating = true;
    let l = &mut fauna.ledger;
    l.pup_swallowed_organic = 0.125;
    l.pup_regurgitated_surplus = 0.25;
    l.pup_regurgitated_lean = 0.5;
    l.pup_upkeep_organic = 0.0625;
    l.pups_starved = 3;
    l.pups_lost_other = 5;
    l.provision_withheld_ticks = 7;
    let back = Fauna::load(&fauna.save()).expect("round trip");
    let t = back.config.founder(C).pack.unwrap();
    assert_eq!(t.regurgitate_reserve_fraction, 0.375);
    assert!(t.provision_scramble && t.pup_starves_on_empty);
    assert_eq!(back.ledger, fauna.ledger);
    assert_eq!(back.animals, fauna.animals);
    assert_eq!(back.packs(), fauna.packs());
    // And off stays off.
    let mut off = Fauna::new(config(|p| {
        p.provision_scramble = false;
        p.pup_starves_on_empty = false;
        p.regurgitate_reserve_fraction = 0.0;
    }));
    off.set_births_enabled(false);
    let back = Fauna::load(&off.save()).unwrap();
    let t = back.config.founder(C).pack.unwrap();
    assert!(!t.provision_scramble && !t.pup_starves_on_empty && t.regurgitate_reserve_fraction == 0.0);
}
