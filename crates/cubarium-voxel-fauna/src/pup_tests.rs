//! **PUP, written before the rule** (Wrysk 2026-10-06: "veilhound cubs should be very expensive
//! to birth and to rear"; LIFE-HISTORY: "a pup the pack can't feed starves"): the giver feeds
//! itself first, food at the den goes to the strongest begging pup, a pup pays its lineage's
//! pup upkeep, and a fed pack still raises its litter.
//!
//! A child of `step` beside the predation files, sharing their fixture (a flat soil plain at
//! 0.25 m, the day clock off so every rhythm is awake). Every body stands still under a
//! scripted controller, so a case is provisioning and upkeep alone. Every number a case asserts
//! is read off the config it built; nothing pins a shipped value.

use super::predation_fixture::*;
use crate::controller::{Actions, Controller, Scripted};
use crate::{Activity, Command, Departure, Fauna, FaunaConfig, Food, Founder, StartingStores};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site, Taken};

const C: Founder = Founder::Chorister;

fn den() -> Site {
    Site { x: 8, y: SURFACE_Y, z: 6 }
}

/// The den's centre, metres.
fn den_m() -> (f64, f64) {
    ((f64::from(den().x) + 0.5) * VOXEL_M, (f64::from(den().z) + 0.5) * VOXEL_M)
}

fn still() -> Box<dyn Controller> {
    Box::new(Scripted::new(vec![Actions { forward: 0.0, turn: 0.0, feed: 0.0 }]))
}

/// The default Veilhound, always awake, breeding off; `edit` changes its pack table.
fn config(edit: impl FnOnce(&mut crate::Pack)) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.founders[C.index()].active = Activity::Always;
    let mut p = c.founders[C.index()].pack.expect("the Veilhound is a pack lineage");
    p.pack_size = 1;
    edit(&mut p);
    c.founders[C.index()].pack = Some(p);
    c
}

/// A pack of `adults` (the first its breeder) at [`den`], every adult standing still at the
/// den with full stores; returns the pack and the adults' ids.
fn pack_at_den(fauna: &mut Fauna, world: &World, adults: usize) -> (u64, Vec<u64>) {
    let mut p = fauna.config.founder(C).pack.expect("pack");
    p.pack_size = adults;
    fauna.config.founders[C.index()].pack = Some(p);
    assert!(fauna.apply(world, Command::FoundPack { den: den(), founder: C, stores: StartingStores::FULL, heading_rad: EAST }));
    let pack = fauna.packs().last().expect("founded").id;
    let ids: Vec<u64> = fauna.animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == pack)).map(|a| a.id).collect();
    let (x, z) = den_m();
    for (k, &id) in ids.iter().enumerate() {
        place(fauna, id, x + 0.3 * k as f64, z, EAST);
        fauna.controllers.set(id, still());
    }
    (pack, ids)
}

/// A litter of `young` born at the den ([`Fauna::whelp`]), every pup standing still there.
fn litter(fauna: &mut Fauna, world: &World, pack: u64, young: u32) -> Vec<u64> {
    let pups = fauna.whelp(&world.view(), pack, young).expect("the breeder pays");
    let (x, z) = den_m();
    for (k, &id) in pups.iter().enumerate() {
        place(fauna, id, x - 0.3 * k as f64, z + 0.25, EAST);
        fauna.controllers.set(id, still());
    }
    pups
}

fn reserve_cap(fauna: &Fauna, id: u64) -> f64 {
    let a = animal(fauna, id);
    crate::body::effective_config(&fauna.config, &a).reserve_of(a.body)
}

fn set_reserve_fraction(fauna: &mut Fauna, id: u64, f: f64) {
    let cap = reserve_cap(fauna, id);
    animal_mut(fauna, id).reserve = f * cap;
}

/// Fill `id`'s stomach to `fraction` of its capacity with carrion of the tissue's mineral and
/// energy, booked introduced.
fn fill_stomach(fauna: &mut Fauna, id: u64, fraction: f64) {
    let a = animal(fauna, id);
    let k = fauna.config.founder(C).gut.capacity(a.body);
    let organic = fraction * k;
    let t = Taken { organic, mineral: 0.01 * organic, energy: organic };
    let a = animal_mut(fauna, id);
    a.stomach = crate::Stomach::default();
    a.stomach.pools[Food::Carrion.index()] = t;
    let l = &mut fauna.ledger;
    l.introduced_organic_in += t.organic;
    l.introduced_mineral_in += t.mineral;
    l.introduced_energy_in += t.energy;
}

fn step(fauna: &mut Fauna, world: &World, flora: &mut Flora, n: u64) {
    for _ in 0..n {
        fauna.step_with(world, flora, 1);
    }
}

fn alive(fauna: &Fauna, id: u64) -> bool {
    fauna.view().animal(id).is_some()
}

/// A pup's basal upkeep per tick at its body now, under the config.
fn pup_upkeep_tick(fauna: &Fauna, id: u64) -> f64 {
    let a = animal(fauna, id);
    let sc = crate::body::effective_config(&fauna.config, &a);
    sc.maintenance_per_s * a.body * crate::DT
}

/// **The giver's condition**, one adult at the den with a full stomach and a begging pup there
/// with room: `regurgitated` organic the pup received over `ticks`, and the ticks withheld.
fn regurgitation_case(adult_fraction: f64, floor: f64, ticks: u64) -> (f64, u64, f64) {
    let world = clock_off(31);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(|p| p.regurgitate_reserve_fraction = floor));
    fauna.set_births_enabled(false);
    let (pack, adults) = pack_at_den(&mut fauna, &world, 1);
    let pup = litter(&mut fauna, &world, pack, 1)[0];
    let giver = adults[0];
    set_reserve_fraction(&mut fauna, giver, adult_fraction);
    fill_stomach(&mut fauna, giver, 0.9);
    fill_stomach(&mut fauna, pup, 0.0);
    animal_mut(&mut fauna, pup).eating = true;
    let held0 = animal(&fauna, giver).stomach.organic();
    step(&mut fauna, &world, &mut flora, ticks);
    let l = &fauna.ledger;
    (l.pup_regurgitated_surplus + l.pup_regurgitated_lean, l.provision_withheld_ticks, held0)
}

/// A starving adult (its reserve under `regurgitate_reserve_fraction` of its cap) with a full
/// stomach at the den keeps its meal: nothing is regurgitated to the begging pup, and every
/// tick it could have been is booked withheld.
#[test]
fn a_starving_adult_with_a_full_stomach_at_the_den_keeps_its_meal() {
    let floor = 0.5;
    let (given, withheld, held0) = regurgitation_case(0.5 * floor, floor, 20);
    assert!(held0 > 0.0);
    assert_eq!(given, 0.0, "nothing regurgitated by an adult under its floor");
    assert!(withheld > 0, "the meal was withheld while the pup begged");
}

/// An adult with a surplus (its reserve above `regurgitate_reserve_fraction`) still
/// provisions: the same scene regurgitates to the pup; and with an empty stomach, an adult
/// above `provision_reserve_fraction` still passes reserve.
#[test]
fn an_adult_with_a_surplus_still_provisions() {
    let floor = 0.5;
    let (given, withheld, _) = regurgitation_case(0.5 * (1.0 + floor), floor, 20);
    assert!(given > 0.0, "an adult above its floor regurgitates");
    assert_eq!(withheld, 0);
    // The reserve fallback.
    let world = clock_off(32);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(|p| p.regurgitate_reserve_fraction = floor));
    fauna.set_births_enabled(false);
    let (pack, adults) = pack_at_den(&mut fauna, &world, 1);
    let pup = litter(&mut fauna, &world, pack, 1)[0];
    let t = fauna.config.founder(C).pack.expect("pack");
    set_reserve_fraction(&mut fauna, adults[0], 0.5 * (1.0 + t.provision_reserve_fraction));
    animal_mut(&mut fauna, adults[0]).stomach = crate::Stomach::default();
    set_reserve_fraction(&mut fauna, pup, 0.1);
    step(&mut fauna, &world, &mut flora, 20);
    assert!(fauna.ledger.provision_received_organic > 0.0, "the reserve fallback still runs");
}

/// A pup with its reserve spent and an empty stomach starves when its lineage's pups live on
/// their reserve, though its structure is far above `body_min`; with food in its stomach it
/// does not; an adult in the same state does not; with the switch off neither does the pup.
#[test]
fn a_pup_with_a_spent_reserve_and_an_empty_stomach_starves() {
    let mut out = Vec::new();
    for (switch, fed, adult) in [(true, false, false), (true, true, false), (true, false, true), (false, false, false)] {
        let world = clock_off(33);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(config(|p| {
            p.pup_starves_on_empty = switch;
            p.provision_rate_per_s = 0.0;
        }));
        fauna.set_births_enabled(false);
        let (pack, _) = pack_at_den(&mut fauna, &world, 1);
        let id = litter(&mut fauna, &world, pack, 1)[0];
        let sc = crate::body::effective_config(&fauna.config, &animal(&fauna, id));
        {
            let a = animal_mut(&mut fauna, id);
            a.body = 2.0 * sc.body_min;
            a.reserve = 0.0;
            a.stomach = crate::Stomach::default();
            if adult {
                a.pack.as_mut().expect("member").pup = false;
            }
        }
        if fed {
            fill_stomach(&mut fauna, id, 0.5);
        }
        step(&mut fauna, &world, &mut flora, 2);
        out.push((switch, fed, adult, alive(&fauna, id), fauna.ledger.pups_starved));
    }
    assert!(!out[0].3 && out[0].4 == 1, "the spent pup starved: {out:?}");
    assert!(out[1].3, "a pup with food in its stomach lives: {out:?}");
    assert!(out[2].3, "an adult in the same state lives: {out:?}");
    assert!(out[3].3, "with the switch off the pup lives on its structure: {out:?}");
}

/// The litter of `young` at the den with one adult whose reserve fallback is the only food: a
/// ration of `ration` pups' upkeep per tick (`provision_rate_per_s` set so), pups starting on
/// `start` ticks of reserve at `body_min`, the strongest the largest by `step` of body. Runs
/// `ticks`; returns each pup's death tick (`None`: alive at the end), strongest first.
fn ration_case(scramble: bool, young: u32, ration: f64, start: f64, ticks: u64) -> Vec<Option<u64>> {
    let world = clock_off(34);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config(|p| {
        p.provision_scramble = scramble;
        p.regurgitate_reserve_fraction = 0.0;
        p.provision_efficiency = 1.0;
        p.pup_starves_on_empty = true;
    }));
    fauna.set_births_enabled(false);
    let (pack, adults) = pack_at_den(&mut fauna, &world, 1);
    let pups = litter(&mut fauna, &world, pack, young);
    let sc = crate::body::effective_config(&fauna.config, &animal(&fauna, pups[0]));
    // Bodies a hair apart, the first the largest; reserves `start` ticks of upkeep.
    for (k, &id) in pups.iter().enumerate() {
        let a = animal_mut(&mut fauna, id);
        a.body = sc.body_min * (1.0 + 1e-3 * f64::from(young - k as u32));
        a.stomach = crate::Stomach::default();
        let u = pup_upkeep_tick(&fauna, id);
        animal_mut(&mut fauna, id).reserve = start * u;
    }
    // The ration: one pup's upkeep × `ration` per tick, from a reserve far above the floor.
    let u = pup_upkeep_tick(&fauna, pups[0]);
    let giver = animal(&fauna, adults[0]);
    let mut t = fauna.config.founder(C).pack.expect("pack");
    t.provision_rate_per_s = ration * u / (giver.body * crate::DT);
    fauna.config.founders[C.index()].pack = Some(t);
    set_reserve_fraction(&mut fauna, adults[0], 1.0);
    animal_mut(&mut fauna, adults[0]).stomach = crate::Stomach::default();
    let mut died = vec![None; pups.len()];
    for tick in 0..ticks {
        fauna.step_with(&world, &mut flora, 1);
        // The giver's reserve stays topped up: the ration is the limit, not its stores.
        set_reserve_fraction(&mut fauna, adults[0], 1.0);
        for (k, &id) in pups.iter().enumerate() {
            if died[k].is_none() && !alive(&fauna, id) {
                died[k] = Some(tick);
            }
        }
    }
    died
}

/// With food for only part of a litter, the scramble feeds the strongest and the weakest
/// starves as soon as an unfed pup would; shared out to the hungriest, the same ration keeps
/// the weakest alive past that (the litter declines together instead).
#[test]
fn with_food_for_part_of_a_litter_the_weakest_pup_starves_and_the_strongest_lives() {
    // 1.2 pups' upkeep a tick for two pups that start on `start` ticks of reserve.
    let (start, ticks) = (40.0, 400);
    let scramble = ration_case(true, 2, 1.2, start, ticks);
    assert!(scramble[0].is_none(), "the strongest lives: {scramble:?}");
    let weak = scramble[1].expect("the weakest starves");
    assert!(weak as f64 <= 1.5 * start, "...as an unfed pup would: {scramble:?}");
    let shared = ration_case(false, 2, 1.2, start, ticks);
    assert!(shared[1].is_none_or(|t| t as f64 > 2.0 * start), "shared out, the weakest is fed: {shared:?}");
}

/// A well-fed pack raises its whole litter: four adults with full stores and full stomachs at
/// the den carry a litter of four, each pup on a reserve that would starve it unfed well before
/// independence, through to independence; the same litter with the adults in poor condition
/// (under both floors, stomachs empty) is lost.
#[test]
fn a_well_fed_pack_raises_its_whole_litter() {
    let independence_s = 60.0;
    let mut fates = Vec::new();
    for fed in [true, false] {
        let world = clock_off(35);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(config(|p| {
            p.provision_scramble = true;
            p.regurgitate_reserve_fraction = 0.5;
            p.pup_starves_on_empty = true;
            p.independence_age_s = independence_s;
        }));
        fauna.set_births_enabled(false);
        let (pack, adults) = pack_at_den(&mut fauna, &world, 4);
        let pups = litter(&mut fauna, &world, pack, 4);
        // Each pup on a tenth of the dependence's upkeep.
        for &id in &pups {
            let u = pup_upkeep_tick(&fauna, id);
            animal_mut(&mut fauna, id).reserve = 0.1 * u * independence_s / crate::DT;
            fill_stomach(&mut fauna, id, 0.0);
        }
        let t = fauna.config.founder(C).pack.expect("pack");
        for &id in &adults {
            if fed {
                set_reserve_fraction(&mut fauna, id, 1.0);
                fill_stomach(&mut fauna, id, 0.9);
            } else {
                set_reserve_fraction(&mut fauna, id, 0.5 * t.regurgitate_reserve_fraction.min(t.provision_reserve_fraction));
                fill_stomach(&mut fauna, id, 0.0);
            }
        }
        let born = pups.len() as u64;
        let indep0 = fauna.ledger.pups_independent;
        step(&mut fauna, &world, &mut flora, (1.1 * independence_s / crate::DT) as u64);
        fates.push((fed, born, fauna.ledger.pups_independent - indep0, fauna.ledger.pups_starved));
    }
    let (_, born, independent, starved) = fates[0];
    assert_eq!(independent, born, "the fed pack raised its whole litter: {fates:?}");
    assert_eq!(starved, 0);
    let (_, born, independent, starved) = fates[1];
    assert_eq!(starved, born, "unfed, the litter starved: {fates:?}");
    assert_eq!(independent, 0);
}

/// **Diagnosis probe** (PUP job 1): how long a newborn the pack never feeds takes to starve under
/// the config's own numbers (the terrarium's faster growth as well), against independence, with
/// the old starvation line and with `pup_starves_on_empty`. A study: run by name.
#[test]
#[ignore = "study: PUP diagnosis, run by name"]
fn probe_unfed_newborn_starvation_time() {
    for (switch, growth) in [(false, None), (true, None), (false, Some(0.000_321_362_587_304_179_14)), (true, Some(0.000_321_362_587_304_179_14))] {
        let world = clock_off(36);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = config(|p| {
            p.provision_rate_per_s = 0.0;
            p.pup_starves_on_empty = switch;
        });
        if let Some(g) = growth {
            c.founders[C.index()].core.growth_max_per_s = g;
        }
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let (pack, _) = pack_at_den(&mut fauna, &world, 1);
        let pup = litter(&mut fauna, &world, pack, 1)[0];
        let t = fauna.config.founder(C).pack.expect("pack");
        let a0 = animal(&fauna, pup);
        let mut ticks = 0u64;
        let limit = (4.0 * t.independence_age_s / crate::DT) as u64;
        let mut traj = Vec::new();
        let mut upkeep = 0.0;
        while alive(&fauna, pup) && ticks < limit {
            if ticks % (1200 * 20) == 0 {
                let a = animal(&fauna, pup);
                traj.push(format!("{}s:b{:.3}/r{:.3}/u{:.2e}", ticks / 20, a.body, a.reserve, pup_upkeep_tick(&fauna, pup) / crate::DT));
            }
            let u0 = fauna.ledger.pup_upkeep_organic;
            fauna.step_with(&world, &mut flora, 1);
            upkeep += fauna.ledger.pup_upkeep_organic - u0;
            ticks += 1;
        }
        println!(
            "switch {switch} growth {:?}: newborn body {:.4} reserve {:.4}; {} after {:.0} s (independence {:.0} s, ratio {:.2}); upkeep paid {upkeep:.4}; starved {}; {}",
            growth,
            a0.body,
            a0.reserve,
            if alive(&fauna, pup) { "ALIVE" } else { "dead" },
            ticks as f64 * crate::DT,
            t.independence_age_s,
            ticks as f64 * crate::DT / t.independence_age_s,
            fauna.ledger.departed_founder(C, Departure::Starved),
            traj.join(" ")
        );
    }
}
