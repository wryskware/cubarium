//! **TEST-PASS-4 (independent), PUP's provisioning one call at a time**: the giver's condition
//! at exactly `regurgitate_reserve_fraction`, its zero (the old rule), the withheld readout
//! only while a pup begs, the surplus/lean split against `provision_reserve_fraction`, and
//! the scramble's order (largest body, then fullest reserve, then lowest id) in both the
//! regurgitation and the reserve fallback, against the hungriest-first rule with it off.
//!
//! A child of `pack`, so a case calls [`provision`] itself on a den it built: no upkeep,
//! digestion or movement runs between the scene and the rule. Every case sets the config it
//! relies on; no shipped default, ratio or verdict is asserted.

use super::*;
use crate::{Activity, Command, FaunaConfig, Food, Founder, StartingStores, Stomach};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::Taken;

const C: Founder = Founder::Chorister;
const V: f64 = 0.25;

fn ground() -> World {
    let mut w = World::empty(VoxelConfig {
        width: 48,
        height: 8,
        depth: 12,
        voxel_m: V,
        seed: 41,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    for z in 0..12 {
        for x in 0..48 {
            for y in 1..=2 {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

fn den() -> Site {
    Site { x: 8, y: 2, z: 6 }
}

/// The Veilhound awake always, its pack table edited by `edit` (a rate that never binds).
fn config(edit: impl FnOnce(&mut Pack)) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.founders[C.index()].active = Activity::Always;
    let mut p = c.founders[C.index()].pack.expect("the Veilhound is a pack lineage");
    p.provision_rate_per_s = 1.0e6;
    p.provision_efficiency = 1.0;
    edit(&mut p);
    c.founders[C.index()].pack = Some(p);
    c
}

/// A pack of `adults` and a litter of `young` at the den, everyone at its centre. Returns
/// the world, the layer, the adults' and the pups' ids.
fn scene(edit: impl FnOnce(&mut Pack), adults: usize, young: u32) -> (World, Fauna, Vec<u64>, Vec<u64>) {
    let w = ground();
    let mut c = config(edit);
    let mut p = c.founders[C.index()].pack.unwrap();
    p.pack_size = adults;
    c.founders[C.index()].pack = Some(p);
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    assert!(f.apply(&w, Command::FoundPack { den: den(), founder: C, stores: StartingStores::FULL, heading_rad: 0.0 }));
    let pack = f.packs().last().unwrap().id;
    let adults: Vec<u64> = f.animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == pack)).map(|a| a.id).collect();
    let pups = f.whelp(&w.view(), pack, young).expect("fixture: the breeder pays");
    let (cx, cz) = centre(den(), V);
    for id in adults.iter().chain(pups.iter()) {
        let a = get_mut(&mut f, *id);
        a.pose.x = cx;
        a.pose.z = cz;
        a.site = den();
        a.stomach = Stomach::default();
        a.eating = false;
    }
    (w, f, adults, pups)
}

fn get(f: &Fauna, id: u64) -> Animal {
    *f.animals.iter().find(|a| a.id == id).unwrap()
}

fn get_mut(f: &mut Fauna, id: u64) -> &mut Animal {
    f.animals.iter_mut().find(|a| a.id == id).unwrap()
}

fn cap(f: &Fauna, id: u64) -> f64 {
    let a = get(f, id);
    crate::body::effective_config(&f.config, &a).reserve_of(a.body)
}

/// Put carrion worth `organic` in `id`'s stomach.
fn feed(f: &mut Fauna, id: u64, organic: f64) {
    let a = get_mut(f, id);
    a.stomach.pools[Food::Carrion.index()] = Taken { organic, mineral: 0.01 * organic, energy: 2.0 * organic };
}

fn table(f: &Fauna) -> Pack {
    f.config.founder(C).pack.unwrap()
}

/// A meal a tenth of the giver's stomach capacity: small enough that any pup has room.
fn meal(f: &Fauna, giver: u64) -> f64 {
    let a = get(f, giver);
    0.1 * f.config.founder(C).gut.capacity(a.body)
}

#[test]
fn a_giver_exactly_at_its_regurgitation_fraction_feeds_and_a_hair_under_keeps_the_meal() {
    for (scale, gives) in [(1.0, true), (1.0 - 1e-12, false)] {
        let (w, mut f, adults, pups) = scene(|p| {
            p.regurgitate_reserve_fraction = 0.5;
            p.provision_reserve_fraction = 0.75;
        }, 1, 1);
        let (g, pup) = (adults[0], pups[0]);
        let floor = table(&f).regurgitate_reserve_fraction;
        let r = scale * floor * cap(&f, g);
        get_mut(&mut f, g).reserve = r;
        assert_eq!(reserve_fraction(&f, &get(&f, g)) >= floor, gives, "fixture: the giver's condition sits where the case says");
        let m = meal(&f, g);
        feed(&mut f, g, m);
        get_mut(&mut f, pup).eating = true;
        provision(&mut f, &w.view());
        let got = get(&f, pup).stomach.organic();
        if gives {
            assert!(got > 0.0, "at the fraction the giver regurgitates");
            assert_eq!(f.ledger.provision_withheld_ticks, 0);
        } else {
            assert_eq!(got, 0.0, "a hair under, it keeps its meal");
            assert_eq!(f.ledger.provision_withheld_ticks, 1);
            assert_eq!(get(&f, g).stomach.organic(), m, "the giver's meal stays whole");
        }
    }
}

#[test]
fn a_zero_regurgitation_fraction_is_the_old_rule_an_empty_reserve_still_feeds() {
    let (w, mut f, adults, pups) = scene(|p| p.regurgitate_reserve_fraction = 0.0, 1, 1);
    let (g, pup) = (adults[0], pups[0]);
    get_mut(&mut f, g).reserve = 0.0;
    let m = meal(&f, g);
    feed(&mut f, g, m);
    get_mut(&mut f, pup).eating = true;
    provision(&mut f, &w.view());
    assert!(get(&f, pup).stomach.organic() > 0.0, "with the fraction at 0 the giver's condition never matters");
    assert_eq!(f.ledger.provision_withheld_ticks, 0);
    assert!(f.ledger.pup_regurgitated_lean > 0.0 && f.ledger.pup_regurgitated_surplus == 0.0);
}

#[test]
fn a_meal_is_booked_withheld_only_while_a_pup_with_room_begs() {
    // Not begging (its eating latch off), then begging but full: neither is a withheld tick.
    for (begging, full) in [(false, false), (true, true)] {
        let (w, mut f, adults, pups) = scene(|p| p.regurgitate_reserve_fraction = 0.5, 1, 1);
        let (g, pup) = (adults[0], pups[0]);
        get_mut(&mut f, g).reserve = 0.0;
        let m = meal(&f, g);
        feed(&mut f, g, m);
        get_mut(&mut f, pup).eating = begging;
        if full {
            let k = f.config.founder(C).gut.capacity(get(&f, pup).body);
            feed(&mut f, pup, k);
        }
        provision(&mut f, &w.view());
        assert_eq!(f.ledger.provision_withheld_ticks, 0, "begging={begging} full={full}: no pup asked, so no meal was withheld");
        assert_eq!(get(&f, g).stomach.organic(), m);
    }
}

#[test]
fn a_regurgitated_meal_moves_whole_and_is_split_surplus_or_lean_by_the_provision_floor() {
    for (fraction, surplus) in [(0.9, true), (0.6, false)] {
        let (w, mut f, adults, pups) = scene(|p| {
            p.regurgitate_reserve_fraction = 0.5;
            p.provision_reserve_fraction = 0.75;
        }, 1, 1);
        let (g, pup) = (adults[0], pups[0]);
        get_mut(&mut f, g).reserve = fraction * cap(&f, g);
        let m = meal(&f, g);
        feed(&mut f, g, m);
        get_mut(&mut f, pup).eating = true;
        let before_g = get(&f, g).stomach.total();
        provision(&mut f, &w.view());
        let after_g = get(&f, g).stomach.total();
        let got = get(&f, pup).stomach.total();
        for (lost, gained) in [
            (before_g.organic - after_g.organic, got.organic),
            (before_g.mineral - after_g.mineral, got.mineral),
            (before_g.energy - after_g.energy, got.energy),
        ] {
            assert!((lost - gained).abs() <= 1e-15 * lost.abs().max(1e-12), "what the giver lost the pup gained: {lost} vs {gained}");
        }
        assert!(got.organic > 0.0);
        let l = &f.ledger;
        assert!((l.regurgitated.organic - got.organic).abs() <= 1e-15);
        let (want, other) = if surplus { (l.pup_regurgitated_surplus, l.pup_regurgitated_lean) } else { (l.pup_regurgitated_lean, l.pup_regurgitated_surplus) };
        assert!((want - got.organic).abs() <= 1e-15 && other == 0.0, "surplus={surplus}: {l:?}");
    }
}

/// Three begging pups, the giver's meal for one bite; returns the pup that received it.
fn fed_pup(scramble: bool, bodies: [f64; 3], fractions: [f64; 3], fallback: bool) -> (Vec<u64>, u64) {
    let (w, mut f, adults, pups) = scene(|p| {
        p.provision_scramble = scramble;
        p.regurgitate_reserve_fraction = 0.0;
        p.provision_reserve_fraction = 0.5;
    }, 1, 3);
    let g = adults[0];
    let birth = get(&f, pups[0]).body;
    for (k, &id) in pups.iter().enumerate() {
        get_mut(&mut f, id).body = bodies[k] * birth;
        let c = cap(&f, id);
        let a = get_mut(&mut f, id);
        a.reserve = fractions[k] * c;
        a.eating = true;
    }
    if fallback {
        let c = cap(&f, g);
        get_mut(&mut f, g).reserve = c;
    } else {
        let m = meal(&f, g);
        feed(&mut f, g, m);
    }
    let before: Vec<Animal> = pups.iter().map(|&id| get(&f, id)).collect();
    provision(&mut f, &w.view());
    let fed: Vec<u64> = pups
        .iter()
        .zip(before.iter())
        .filter(|(id, b)| {
            let a = get(&f, **id);
            a.stomach.organic() > b.stomach.organic() || a.reserve > b.reserve
        })
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(fed.len(), 1, "fixture: one giver feeds one pup a tick: {fed:?}");
    (pups, fed[0])
}

#[test]
fn the_scramble_feeds_the_largest_pup_even_when_it_is_the_fullest() {
    for fallback in [false, true] {
        // Pup 1 is the largest and the fullest; pup 2 the hungriest.
        let (pups, fed) = fed_pup(true, [1.0, 1.2, 1.1], [0.5, 0.8, 0.2], fallback);
        assert_eq!(fed, pups[1], "fallback={fallback}: the strongest beggar is fed first");
        let (pups, fed) = fed_pup(false, [1.0, 1.2, 1.1], [0.5, 0.8, 0.2], fallback);
        assert_eq!(fed, pups[2], "fallback={fallback}: off, the hungriest is fed");
    }
}

#[test]
fn the_scramble_breaks_a_body_tie_by_the_fuller_reserve_then_by_the_lowest_id() {
    for fallback in [false, true] {
        let (pups, fed) = fed_pup(true, [1.0, 1.1, 1.1], [0.9, 0.3, 0.6], fallback);
        assert_eq!(fed, pups[2], "fallback={fallback}: equal bodies, the fuller reserve wins");
        let (pups, fed) = fed_pup(true, [1.1, 1.1, 1.1], [0.4, 0.4, 0.4], fallback);
        assert_eq!(fed, pups[0], "fallback={fallback}: a full tie goes to the lowest id");
    }
}

#[test]
fn hungriest_first_breaks_a_tie_by_the_lowest_id() {
    for fallback in [false, true] {
        let (pups, fed) = fed_pup(false, [1.0, 1.2, 1.1], [0.3, 0.3, 0.3], fallback);
        assert_eq!(fed, pups[0], "fallback={fallback}");
    }
}

#[test]
fn a_withholding_giver_with_a_meal_does_not_fall_back_on_its_reserve() {
    // Its condition is under the regurgitation fraction yet above the reserve floor: the
    // meal is kept, and neither the meal nor the reserve reaches the pup.
    let (w, mut f, adults, pups) = scene(|p| {
        p.regurgitate_reserve_fraction = 0.6;
        p.provision_reserve_fraction = 0.2;
    }, 1, 1);
    let (g, pup) = (adults[0], pups[0]);
    get_mut(&mut f, g).reserve = 0.4 * cap(&f, g);
    let m = meal(&f, g);
    feed(&mut f, g, m);
    let c = cap(&f, pup);
    get_mut(&mut f, pup).reserve = 0.1 * c;
    get_mut(&mut f, pup).eating = true;
    let before = get(&f, pup);
    provision(&mut f, &w.view());
    let after = get(&f, pup);
    assert_eq!((after.reserve, after.stomach.organic()), (before.reserve, 0.0));
    assert_eq!(f.ledger.provision_given_organic, 0.0);
    assert_eq!(f.ledger.provision_withheld_ticks, 1);
}
