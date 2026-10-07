//! **TEST-PASS-2 (independent), LIFE-HISTORY**: the breeding season's count per pack (two
//! packs, a successor breeder, a failed gestation), the season on an egg-layer, the window's
//! and the count's edges, the litter with the condition rule off and under its funds, the
//! ledgers through a starving pup and a failed gestation at a raised upkeep, and PIT-2's
//! whelping at the den with a season. A child of `step` sharing the predation fixture.
//!
//! The year is made short (LIFE-HISTORY's own fixture: a 60 s day and a one-day year, the dry
//! season off); the world's clock moves with the fauna where a case reads the year. Every case
//! sets the config it relies on; no shipped default, ratio or verdict is asserted.

use super::predation_fixture::*;
use crate::{Activity, Command, Fauna, FaunaConfig, Founder, PackMember, Reproduction, Season, StartingStores};
use cubarium_voxel::weather::{DayClock, DayMode, Year};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const C: Founder = Founder::Chorister;
const LJ: Founder = Founder::Lanternjaw;

/// A flat soil plain `width` columns round, the clock off, a **one-day year of one minute**
/// (phase 0 at tick 0), the dry season off.
fn year_world(seed: u64, width: u32) -> World {
    let depth = 12u32;
    let mut climate = cubarium_voxel::weather::Climate::default();
    climate.dry_season = false;
    climate.dry_season_every_days = 1.0;
    climate.dry_season_len_days = 0.2;
    climate.dry_season_phase_days = 0.0;
    let mut w = World::empty(VoxelConfig {
        width,
        height: 8,
        depth,
        voxel_m: VOXEL_M,
        seed,
        day: DayClock { mode: DayMode::Off, length_min: 1.0, ..DayClock::default() },
        climate,
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=SURFACE_Y {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

fn year_ticks(world: &World) -> u64 {
    let c = world.config();
    (c.climate.year_s(&c.day) * f64::from(cubarium_voxel::TICK_HZ)).round() as u64
}

/// Fast live birth for the Veilhound (LIFE-HISTORY's fixture): a reserve cap ten times its
/// body, a 1.5 s hold, a `gestation_s` gestation, no litter interval, every body awake; the
/// season as given, no breeding reserve, the litter by condition.
fn veilhound_config(litters_per_year: u32, phase: f64, window: f64, gestation_s: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    let mut p = c.founders[C.index()].pack.expect("the Veilhound is a pack lineage");
    p.pack_size = 4;
    p.litter_size_max = 4;
    p.litter_interval_s = 0.0;
    p.breed_reserve_fraction = 0.0;
    p.litter_by_condition = true;
    c.founders[C.index()].pack = Some(p);
    let core = &mut c.founders[C.index()].core;
    core.reserve_cap = 10.0;
    core.reproduction = Reproduction {
        surplus_floor: 0.0,
        surplus_hold_s: 1.5,
        gestation_s,
        birth_interval_s: 0.05,
        litters_per_year,
        breeding_phase: phase,
        breeding_window: window,
        ..Reproduction::LIVE_BIRTH_PLACEHOLDER
    };
    c
}

fn found(fauna: &mut Fauna, world: &World, den: Site) -> u64 {
    assert!(fauna.apply(world, Command::FoundPack { den, founder: C, stores: StartingStores::FULL, heading_rad: EAST }));
    fauna.packs().last().expect("just founded").id
}

/// Step `n` ticks with the world's clock in step; the ticks (world) at which a gestation was
/// opened, with the pack of its breeder.
fn run(fauna: &mut Fauna, world: &mut World, flora: &mut Flora, n: u64) -> Vec<(u64, Option<u64>)> {
    let mut opened = Vec::new();
    for _ in 0..n {
        let before: Vec<(u64, bool)> = fauna.animals.iter().map(|a| (a.id, a.reproduction.escrow.is_some())).collect();
        let tick = world.tick();
        fauna.step_with(world, flora, 1);
        for a in &fauna.animals {
            let was = before.iter().find(|b| b.0 == a.id).is_some_and(|b| b.1);
            if a.reproduction.escrow.is_some_and(|e| e.ticks == 1) && !was {
                opened.push((tick, a.pack.map(|m| m.pack)));
            }
        }
        world.advance_tick();
    }
    opened
}

fn jump(world: &mut World, tick: u64) {
    while world.tick() < tick {
        world.advance_tick();
    }
}

fn set_reserve_fraction(fauna: &mut Fauna, id: u64, f: f64) {
    let a = animal(fauna, id);
    let cap = crate::body::effective_config(&fauna.config, &a).reserve_of(a.body);
    animal_mut(fauna, id).reserve = f * cap;
}

// ------------------------------------------------------------------ the season's count

/// **Two packs each breed once in one window**: the season counts per pack, not per lineage —
/// with `litters_per_year` 1, two packs of the lineage on one plain each conceive once in the
/// year's window.
#[test]
fn two_packs_each_breed_once_in_one_window() {
    let mut world = year_world(31, 96);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(veilhound_config(1, 0.0, 0.5, 1.0));
    fauna.set_births_enabled(true);
    let p1 = found(&mut fauna, &world, Site { x: 8, y: SURFACE_Y, z: 6 });
    let p2 = found(&mut fauna, &world, Site { x: 72, y: SURFACE_Y, z: 6 });
    let w = (0.5 * year_ticks(&world) as f64) as u64;
    let got = run(&mut fauna, &mut world, &mut flora, w - 5);
    let by = |p: u64| got.iter().filter(|c| c.1 == Some(p)).count();
    assert_eq!((by(p1), by(p2)), (1, 1), "conceptions by pack in the window: {got:?}");
    assert_eq!(fauna.pack(p1).unwrap().season.used_in(0), 1);
    assert_eq!(fauna.pack(p2).unwrap().season.used_in(0), 1);
}

/// **A successor breeder does not breed again in its pack's year**: the pack conceives once in
/// the window (`litters_per_year` 1); then another adult becomes its breeder (as when the first
/// dies) — it does not conceive in the rest of that window, and conceives in the next year's.
#[test]
fn a_successor_breeder_does_not_breed_again_in_the_packs_year() {
    let mut world = year_world(32, 48);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(veilhound_config(1, 0.0, 0.5, 1.0));
    let pack = found(&mut fauna, &world, Site { x: 8, y: SURFACE_Y, z: 6 });
    let yt = year_ticks(&world);
    let first = run(&mut fauna, &mut world, &mut flora, 200);
    assert_eq!(first.len(), 1, "fixture: one conception early in the window: {first:?}");
    let old = fauna.pack(pack).unwrap().breeder.unwrap();
    let next = fauna.animals.iter().find(|a| a.pack.is_some_and(|m| m.pack == pack && !m.pup) && a.id != old).unwrap().id;
    fauna.pack_mut(pack).unwrap().breeder = Some(next);
    let rest = run(&mut fauna, &mut world, &mut flora, yt / 2 - 220);
    assert!(rest.is_empty(), "the successor conceived in the pack's year: {rest:?}");
    jump(&mut world, yt + 5);
    let again = run(&mut fauna, &mut world, &mut flora, 200);
    assert_eq!(again.len(), 1, "the next year's window: {again:?}");
}

/// **A failed gestation uses the year's litter** (LIFE-HISTORY's decision): the breeder
/// conceives in its window (`litters_per_year` 1, a long gestation), its reserve is then
/// emptied so the next instalment fails the gestation; its reserve restored, it does not
/// conceive again in that window, and does in the next year's.
#[test]
fn a_failed_gestation_uses_the_years_litter() {
    let mut world = year_world(33, 48);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(veilhound_config(1, 0.0, 0.5, 30.0));
    let pack = found(&mut fauna, &world, Site { x: 8, y: SURFACE_Y, z: 6 });
    let yt = year_ticks(&world);
    let first = run(&mut fauna, &mut world, &mut flora, 200);
    assert_eq!(first.len(), 1, "fixture: one conception: {first:?}");
    let b = fauna.pack(pack).unwrap().breeder.unwrap();
    animal_mut(&mut fauna, b).reserve = 0.0;
    let failed0 = fauna.ledger.gestations_failed;
    run(&mut fauna, &mut world, &mut flora, 2);
    assert!(fauna.ledger.gestations_failed > failed0 && animal(&fauna, b).reproduction.escrow.is_none(), "fixture: the gestation failed");
    set_reserve_fraction(&mut fauna, b, 0.9);
    let rest = run(&mut fauna, &mut world, &mut flora, yt / 2 - 220);
    assert!(rest.is_empty(), "conceived again in the year of a failed gestation: {rest:?}");
    jump(&mut world, yt + 5);
    let again = run(&mut fauna, &mut world, &mut flora, 200);
    assert_eq!(again.len(), 1, "the next year's window: {again:?}");
}

/// **An egg-layer's clutches keep its season** (`lay`'s gate): a lanternjaw laying from
/// surplus with no interval between clutches lays none before its window, at most
/// `litters_per_year` clutches inside it, counted on its own season, and lays again in the
/// next year's window.
#[test]
fn an_egg_layers_clutches_keep_its_season() {
    let mut world = year_world(34, 48);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    c.founders[LJ.index()].active = Activity::Always;
    let (phase, window, lpy) = (0.4, 0.3, 2u32);
    {
        let core = &mut c.founders[LJ.index()].core;
        core.reserve_cap = 10.0;
        core.reproduction = Reproduction {
            surplus_floor: 0.0,
            surplus_hold_s: 0.5,
            birth_interval_s: 0.05,
            litters_per_year: lpy,
            breeding_phase: phase,
            breeding_window: window,
            ..core.reproduction
        };
        assert_eq!(core.reproduction.mode, crate::BirthMode::Eggs, "fixture: an egg-layer");
    }
    let mut fauna = Fauna::new(c);
    let lj = introduce(&mut fauna, &world, LJ, StartingStores::FULL);
    drive(&mut fauna, lj, 0.0, false);
    let yt = year_ticks(&world);
    let clutches = |fauna: &mut Fauna, world: &mut World, flora: &mut Flora, n: u64| -> Vec<u64> {
        let mut at = Vec::new();
        for _ in 0..n {
            let before = fauna.ledger.clutches_laid;
            let t = world.tick();
            fauna.step_with(world, flora, 1);
            if fauna.ledger.clutches_laid > before {
                at.push(t);
            }
            world.advance_tick();
        }
        at
    };
    let w0 = (phase * yt as f64).ceil() as u64;
    let w1 = ((phase + window) * yt as f64).floor() as u64;
    let early = clutches(&mut fauna, &mut world, &mut flora, w0 - 10);
    assert!(early.is_empty(), "laid before its window: {early:?}");
    assert!(fauna.ledger.season_waits > 0, "a ready layer waited on its season");
    let inside = clutches(&mut fauna, &mut world, &mut flora, w1 - w0 + 40);
    assert!(inside.iter().all(|&t| t >= w0 && t <= w1), "laid outside the window {w0}..{w1}: {inside:?}");
    assert_eq!(inside.len() as u32, lpy, "clutches in the window: {inside:?}");
    assert_eq!(animal(&fauna, lj).reproduction.season.used_in(0), lpy, "counted on its own season");
    jump(&mut world, yt + w0);
    let again = clutches(&mut fauna, &mut world, &mut flora, w1 - w0);
    assert_eq!(again.len() as u32, lpy, "the next year's window: {again:?}");
}

/// **The season's gate at its edges**: the window opens at `breeding_phase` (inclusive) and
/// shuts at `breeding_phase + breeding_window` (exclusive), wrapping past the year's end; the
/// year's count shuts it at `litters_per_year` and a new year's number reopens it; a world with
/// no year, or a lineage with no season, is never shut.
#[test]
fn the_seasons_gate_at_its_edges() {
    let r = Reproduction { litters_per_year: 2, breeding_phase: 0.9, breeding_window: 0.25, ..Reproduction::LIVE_BIRTH_PLACEHOLDER };
    let at = |number: i64, phase: f64| Some(Year { number, phase });
    let none = Season::default();
    assert!(r.season_allows(at(3, 0.9), none), "at the window's opening");
    assert!(!r.season_allows(at(3, 0.9 - 1e-9), none), "a hair before it");
    assert!(r.season_allows(at(3, 0.1), none), "past the year's end, inside the wrap");
    assert!(!r.season_allows(at(3, 0.15), none), "at the wrapped window's end");
    let one = Season { year: 3, litters: 1 };
    let two = Season { year: 3, litters: 2 };
    assert!(r.season_allows(at(3, 0.95), one), "under the year's litters");
    assert!(!r.season_allows(at(3, 0.95), two), "the year's litters used");
    assert!(r.season_allows(at(4, 0.95), two), "a new year's number reopens it");
    assert!(r.season_allows(None, two), "a world with no year");
    let off = Reproduction { litters_per_year: 0, ..r };
    assert!(off.season_allows(at(3, 0.5), two), "a lineage with no season");
    let mut s = Season { year: 3, litters: 2 };
    s.count(4);
    assert_eq!(s, Season { year: 4, litters: 1 }, "a count in a new year starts it at one");
}

// ------------------------------------------------------------------ the litter

/// The young of the first conception of a breeder held at reserve fraction `fraction`, with the
/// litter by condition `by_condition` and a breeding reserve `breed`; with `funds` the surplus
/// floor is set so that its reserve over it pays that many packages. `None` when it does not
/// conceive in 120 ticks.
fn first_litter(seed: u64, by_condition: bool, breed: f64, fraction: f64, funds: Option<f64>) -> Option<u32> {
    let mut world = year_world(seed, 48);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = veilhound_config(0, 0.0, 0.0, 1.0);
    {
        let p = c.founders[C.index()].pack.as_mut().unwrap();
        p.litter_by_condition = by_condition;
        p.breed_reserve_fraction = breed;
    }
    let mut fauna = Fauna::new(c.clone());
    let pack = found(&mut fauna, &world, Site { x: 8, y: SURFACE_Y, z: 6 });
    let b = fauna.pack(pack).unwrap().breeder.unwrap();
    set_reserve_fraction(&mut fauna, b, fraction);
    if let Some(n) = funds {
        let sc = crate::body::effective_config(&fauna.config, &animal(&fauna, b));
        let package = sc.reproduction.package_cost(&sc);
        c.founders[C.index()].core.reproduction.surplus_floor = animal(&fauna, b).reserve - n * package;
        assert!(c.founders[C.index()].core.reproduction.surplus_floor > 0.0, "fixture: a positive floor");
        fauna.replace_config(c).unwrap();
    }
    for _ in 0..120 {
        fauna.step_with(&world, &mut flora, 1);
        world.advance_tick();
        if let Some(e) = animal(&fauna, b).reproduction.escrow {
            return Some(e.young);
        }
        set_reserve_fraction(&mut fauna, b, fraction);
    }
    None
}

/// **With the litter by condition off, the old draw stands and the breeding reserve still
/// gates**: under a breeding reserve of 0.6, a breeder at 0.5 does not conceive with the switch
/// off; at 0.9 it carries between `min(2, litter_size_max)` and `litter_size_max`, whatever the
/// seed.
#[test]
fn with_the_litter_by_condition_off_the_old_draw_stands_and_the_reserve_still_gates() {
    let hi = veilhound_config(0, 0.0, 0.0, 1.0).founder(C).pack.unwrap().litter_size_max;
    assert_eq!(first_litter(41, false, 0.6, 0.5, None), None, "under the breeding reserve, switch off");
    for seed in 41..47 {
        let n = first_litter(seed, false, 0.6, 0.9, None).expect("over the breeding reserve it conceives");
        assert!(n >= hi.min(2) && n <= hi, "seed {seed}: {n} young outside {}..={hi}", hi.min(2));
    }
}

/// **A litter by condition never carries more than its reserve funds**: a breeder nearly full
/// (its condition would give `litter_size_max`) over a surplus floor that leaves funds for two
/// packages carries two; with funds for none it does not conceive.
#[test]
fn a_litter_by_condition_never_carries_more_than_its_reserve_funds() {
    let t = veilhound_config(0, 0.0, 0.0, 1.0).founder(C).pack.unwrap();
    let fraction = 0.98;
    assert_eq!(t.condition_litter(fraction), t.litter_size_max, "fixture: its condition gives the most");
    assert!(t.litter_size_max > 2, "fixture: the most is over two");
    assert_eq!(first_litter(48, true, 0.0, fraction, Some(2.5)), Some(2), "funds for two and a half packages");
    assert_eq!(first_litter(49, true, 0.0, fraction, Some(0.5)), None, "funds for none: no conception");
}

// ------------------------------------------------------------------ the ledgers

/// **The ledgers close through a failed gestation at a raised upkeep and a pup that starves**:
/// a pack founded with funds for one young and a tenth of the gestation's upkeep over, paying
/// `gestation_upkeep_factor` 50 (the founding's stomach digests into the reserve meanwhile),
/// whose gestation fails when its upkeep has eaten the margin; and a pup at a den whose adults
/// are under `provision_reserve_fraction` with empty stomachs (`gut.s0` 0), left unprovisioned
/// until it starves. Every store comes in through the founding (no hand-set stock); the
/// fauna's organic matter, mineral and energy match the ledger after each.
#[test]
fn the_ledgers_close_through_a_failed_gestation_and_a_starving_pup() {
    // The failed gestation.
    let mut world = year_world(51, 48);
    let mut flora = Flora::new(FloraConfig::default());
    let gestation_s = 30.0;
    let factor = 50.0;
    let mut c = veilhound_config(0, 0.0, 0.0, gestation_s);
    c.founders[C.index()].core.reproduction.gestation_upkeep_factor = factor;
    let sc = c.founder(C).core;
    let package = sc.reproduction.package_cost(&sc);
    let upkeep = factor * sc.maintenance_per_s * sc.body_max * gestation_s;
    // Over one package by a tenth of the gestation's raised upkeep: the hold before it costs
    // far less (basal upkeep, 1.5 s), the gestation more than that even at rest.
    let reserve = package + 0.1 * upkeep;
    let stores = StartingStores { body: 1.0, reserve: reserve / sc.reserve_of(sc.body_max) };
    assert!(stores.reserve < 1.0, "fixture: the stores fit the cap");
    let mut fauna = Fauna::new(c);
    assert!(fauna.apply(&world, Command::FoundPack { den: Site { x: 8, y: SURFACE_Y, z: 6 }, founder: C, stores, heading_rad: EAST }));
    let n = cubarium_voxel::ticks_at_least_one(gestation_s) + 200;
    let mut opened = false;
    for _ in 0..n {
        fauna.step_with(&world, &mut flora, 1);
        world.advance_tick();
        opened |= fauna.ledger.gestations_opened > 0;
        if fauna.ledger.gestations_failed > 0 {
            break;
        }
    }
    assert!(opened, "fixture: a conception");
    assert!(fauna.ledger.gestations_failed > 0, "fixture: the gestation failed on its upkeep");
    assert_closes(&fauna, "after the failed gestation at a raised upkeep");

    // The starving pup.
    let mut world = year_world(52, 48);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = veilhound_config(0, 0.0, 0.0, 1.0);
    c.founders[C.index()].core.reproduction.surplus_floor = 1e9; // no breeding here
    c.founders[C.index()].gut.s0 = 0.0; // founded and introduced with empty stomachs: nothing to regurgitate
    let t = c.founder(C).pack.unwrap();
    let sc = c.founder(C).core;
    let mut fauna = Fauna::new(c);
    let den = Site { x: 8, y: SURFACE_Y, z: 6 };
    let lean = StartingStores { body: 1.0, reserve: 0.5 * t.provision_reserve_fraction };
    assert!(fauna.apply(&world, Command::FoundPack { den, founder: C, stores: lean, heading_rad: EAST }));
    let pack = fauna.packs()[0].id;
    let small = sc.body_min / sc.body_max;
    let last = 20.0 * sc.maintenance_per_s * sc.body_min * crate::DT / sc.reserve_of(sc.body_min);
    let pup = introduce(&mut fauna, &world, C, StartingStores { body: small, reserve: last });
    animal_mut(&mut fauna, pup).pack = Some(PackMember { pack, pup: true, calling_ticks: 0 });
    let (cx, cz) = crate::pack::centre(den, VOXEL_M);
    place(&mut fauna, pup, cx, cz, EAST);
    let mut gone = false;
    for _ in 0..400 {
        fauna.step_with(&world, &mut flora, 1);
        world.advance_tick();
        if fauna.view().animal(pup).is_none() {
            gone = true;
            break;
        }
    }
    assert!(gone, "fixture: the unfed pup starved");
    assert_closes(&fauna, "after the pup starved");
}

// ------------------------------------------------------------------ PIT-2 x the season

/// **A breeder that conceives in its window away from its den gives birth at its den**
/// (PIT-2's whelping with LIFE-HISTORY's season): the breeder, on its heuristic with
/// `whelp_at_den`, stands 3 m from its den when its window opens; its gestation is longer than
/// three times the walk home at cruise. Its litter is born within half the den radius of the
/// den, where its pups stay.
#[test]
fn a_breeder_conceiving_away_from_its_den_gives_birth_at_its_den() {
    let mut world = year_world(61, 48);
    let mut flora = Flora::new(FloraConfig::default());
    let d = 3.0;
    let probe = veilhound_config(1, 0.0, 0.5, 1.0);
    let cruise = probe.manifest(C).cruise_m_per_s;
    let gestation_s = 3.0 * d / cruise + 2.0;
    let mut c = veilhound_config(1, 0.0, 0.5, gestation_s);
    {
        let p = c.founders[C.index()].pack.as_mut().unwrap();
        p.whelp_at_den = true;
        p.den_radius_m = 1.0;
        p.home_give_up_s = 1e6;
    }
    let mut fauna = Fauna::new(c);
    let den = Site { x: 8, y: SURFACE_Y, z: 6 };
    let pack = found(&mut fauna, &world, den);
    let b = fauna.pack(pack).unwrap().breeder.unwrap();
    let teachers = fauna.config.teachers;
    fauna.controllers.set(b, crate::controller::heuristic(C, teachers));
    let (cx, cz) = crate::pack::centre(den, VOXEL_M);
    place(&mut fauna, b, cx + d, cz, EAST);
    let n = cubarium_voxel::ticks_at_least_one(gestation_s + 3.0) + 60;
    let born0 = fauna.ledger.pups_born;
    let opened = run(&mut fauna, &mut world, &mut flora, n);
    assert_eq!(opened.len(), 1, "fixture: one conception in the window: {opened:?}");
    assert!(fauna.ledger.pups_born > born0, "fixture: the litter is born");
    let radius = fauna.config.founder(C).pack.unwrap().den_radius_m;
    let (bx, bz) = (animal(&fauna, b).pose.x, animal(&fauna, b).pose.z);
    let pups: Vec<(f64, f64)> = fauna.animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == pack && m.pup)).map(|a| (a.pose.x, a.pose.z)).collect();
    assert!(!pups.is_empty(), "fixture: pups");
    for (x, z) in pups {
        let (dx, dz) = crate::pose::wrapped_delta(x, z, cx, cz, 48.0 * VOXEL_M);
        assert!((dx * dx + dz * dz).sqrt() <= 0.5 * radius + VOXEL_M, "a pup born at ({x:.2}, {z:.2}), {:.2} m from the den (breeder at ({bx:.2}, {bz:.2}))", (dx * dx + dz * dz).sqrt());
    }
}

/// **FINDINGS-FIX-2: a layer's clutch wait is not spent while its season's window is shut**:
/// a lanternjaw (dormant eggs, no nest site) laying from surplus up to `clutch_max` 4, its
/// reserve paying three and a half eggs (short of the full clutch), with a clutch wait of
/// `wait_s` and a season whose window opens at `breeding_phase` of a one-minute year, its hold
/// met long before. No clutch before the window opens; once it opens the layer still waits its
/// full clutch time (the season gates before the wait counts), and then lays.
#[test]
fn a_layers_clutch_wait_counts_only_inside_its_season() {
    let mut world = year_world(35, 48);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    c.founders[LJ.index()].active = Activity::Always;
    c.founders[LJ.index()].gut.s0 = 0.0;
    let (wait_s, phase) = (2.0, 0.3);
    let (egg, floor) = {
        let core = &mut c.founders[LJ.index()].core;
        core.reserve_cap = 10.0;
        core.reproduction = Reproduction {
            surplus_hold_s: 0.1,
            birth_interval_s: 1e6,
            clutch_max: 4,
            clutch_hold_max_s: wait_s,
            litters_per_year: 1,
            breeding_phase: phase,
            breeding_window: 0.5,
            ..core.reproduction
        };
        (core.reproduction.egg_organic, core.reproduction.surplus_floor)
    };
    let sc = c.founder(LJ).core;
    let reserve = (floor + 3.5 * egg) / sc.reserve_of(sc.body_max);
    assert!(reserve < 1.0, "fixture: the reserve fits the cap");
    let mut fauna = Fauna::new(c);
    let lj = introduce(&mut fauna, &world, LJ, StartingStores { body: 1.0, reserve });
    drive(&mut fauna, lj, 0.0, false);
    let w0 = (phase * year_ticks(&world) as f64).ceil() as u64;
    let wait = cubarium_voxel::ticks_at_least_one(wait_s);
    let hold = cubarium_voxel::ticks_at_least_one(0.1);
    assert!(w0 > hold + wait + 10, "fixture: the hold and a whole wait fit before the window ({w0} ticks)");
    let mut laid = None;
    for _ in 0..w0 + wait + 40 {
        let t = world.tick();
        fauna.step_with(&world, &mut flora, 1);
        world.advance_tick();
        if fauna.ledger.clutches_laid > 0 {
            laid = Some(t);
            break;
        }
    }
    let t = laid.expect("the clutch is laid after the window opens and the wait runs");
    assert!(t >= w0 + wait - 1, "laid at tick {t}: the window opened at {w0} and the wait is {wait} ticks");
    assert!(fauna.ledger.eggs_laid >= 1 && fauna.ledger.eggs_laid < 4, "a short clutch: {} eggs", fauna.ledger.eggs_laid);
}
