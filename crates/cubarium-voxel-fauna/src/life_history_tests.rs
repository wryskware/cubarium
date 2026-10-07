//! **LIFE-HISTORY, written before the rule** (Wrysk 2026-10-06: "veilhound cubs should be very
//! expensive to birth and to rear. like lions and wolves only have one litter a year"): the
//! Cubarium year (the dry-season cycle), per-lineage breeding seasons, and the Veilhound's
//! (code: `chorister`) breeding reserve, litter by condition, pregnancy upkeep and pup
//! provisioning.
//!
//! A child of `step` beside the predation files, sharing their fixture (a flat soil plain at
//! 0.25 m). The year is made short — a 60 s day and a one-day year — so a breeding window is
//! a few hundred ticks; the world's clock is moved with [`World::advance_tick`] (the fauna
//! reads the world's tick for the year), and the fauna is stepped only around the moments a
//! case looks at. Every number a case asserts is read off the config it built; nothing pins a
//! shipped value.

use super::predation_fixture::*;
use crate::{
    Activity, Command, Departure, Escrow, Fauna, FaunaConfig, Founder, PackMember, Reproduction,
    StartingStores,
};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const C: Founder = Founder::Chorister;
const DAY_MIN: f64 = 1.0;

/// A flat soil plain with the day clock off and a **one-day year of `DAY_MIN` minutes**
/// (phase 0 at tick 0), the dry season itself switched off: the year runs without it.
fn year_world(seed: u64, year_days: f64) -> World {
    let (width, depth) = (48u32, 12u32);
    let mut climate = cubarium_voxel::weather::Climate::default();
    climate.dry_season = false;
    climate.dry_season_every_days = year_days;
    climate.dry_season_len_days = 0.2 * year_days.max(0.0);
    climate.dry_season_phase_days = 0.0;
    let mut w = World::empty(VoxelConfig {
        width,
        height: 8,
        depth,
        voxel_m: VOXEL_M,
        seed,
        day: DayClock {
            mode: DayMode::Off,
            length_min: DAY_MIN,
            ..DayClock::default()
        },
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

/// Ticks in one year of [`year_world`] (`year_days` = 1).
fn year_ticks(world: &World) -> u64 {
    let c = world.config();
    (c.climate.year_s(&c.day) * f64::from(cubarium_voxel::TICK_HZ)).round() as u64
}

fn den() -> Site {
    Site { x: (STRIKER_X / VOXEL_M).floor() as u32, y: SURFACE_Y, z: (LANE_Z / VOXEL_M).floor() as u32 }
}

/// Fast live birth for the Veilhound (as `chorister_tests::breeding_config`): a reserve cap
/// ten times its body, a 1.5 s hold, a 1 s gestation, no litter interval, every body awake;
/// the season and the breeding reserve as given.
fn veilhound_config(litters_per_year: u32, phase: f64, window: f64, breed_reserve: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    let mut p = c.founders[C.index()].pack.expect("the Veilhound is a pack lineage");
    p.pack_size = 4;
    p.litter_size_max = 4;
    p.litter_interval_s = 0.0;
    p.breed_reserve_fraction = breed_reserve;
    p.litter_by_condition = true;
    c.founders[C.index()].pack = Some(p);
    c.founders[C.index()].active = Activity::Always;
    let core = &mut c.founders[C.index()].core;
    core.reserve_cap = 10.0;
    core.reproduction = Reproduction {
        surplus_floor: 0.0,
        surplus_hold_s: 1.5,
        gestation_s: 1.0,
        birth_interval_s: 0.05,
        litters_per_year,
        breeding_phase: phase,
        breeding_window: window,
        ..Reproduction::LIVE_BIRTH_PLACEHOLDER
    };
    c
}

fn found_pack(fauna: &mut Fauna, world: &World, stores: StartingStores) -> u64 {
    assert!(fauna.apply(world, Command::FoundPack { den: den(), founder: C, stores, heading_rad: EAST }));
    fauna.packs().last().expect("just founded").id
}

/// Step the fauna `n` ticks with the world's clock moving in step; returns the world ticks at
/// which a gestation was opened.
fn run(fauna: &mut Fauna, world: &mut World, flora: &mut Flora, n: u64) -> Vec<u64> {
    conceptions(fauna, world, flora, n).into_iter().map(|c| c.0).collect()
}

/// [`run`], with the young each conception carries (the escrow its first instalment opened).
fn conceptions(fauna: &mut Fauna, world: &mut World, flora: &mut Flora, n: u64) -> Vec<(u64, u32)> {
    let mut opened = Vec::new();
    for _ in 0..n {
        let before = fauna.ledger.gestations_opened;
        let tick = world.tick();
        fauna.step_with(world, flora, 1);
        if fauna.ledger.gestations_opened > before {
            let young = fauna.animals.iter().filter_map(|a| a.reproduction.escrow.filter(|e| e.ticks == 1)).map(|e| e.young.max(1)).sum();
            opened.push((tick, young));
        }
        world.advance_tick();
    }
    opened
}

/// Move the world's clock to `tick` without stepping the fauna.
fn jump(world: &mut World, tick: u64) {
    while world.tick() < tick {
        world.advance_tick();
    }
}

fn reserve_fraction(fauna: &Fauna, id: u64) -> f64 {
    let a = animal(fauna, id);
    a.reserve / crate::body::effective_config(&fauna.config, &a).reserve_of(a.body)
}

fn set_reserve_fraction(fauna: &mut Fauna, id: u64, f: f64) {
    let a = animal(fauna, id);
    let cap = crate::body::effective_config(&fauna.config, &a).reserve_of(a.body);
    animal_mut(fauna, id).reserve = f * cap;
}

/// A breeder conceives only inside its window, and at most `litters_per_year` times a year;
/// the next year's window opens it again. The year runs with the dry season off.
#[test]
fn a_breeder_conceives_only_inside_its_window_and_at_most_litters_per_year() {
    for lpy in [1u32, 2] {
        let (phase, window) = (0.5, 0.15);
        let mut world = year_world(21, 1.0);
        assert!(!world.config().climate.dry_season, "the dry season is off");
        let yt = year_ticks(&world);
        let open = |t: u64| {
            let p = (t % yt) as f64 / yt as f64;
            (p - phase).rem_euclid(1.0) < window
        };
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(veilhound_config(lpy, phase, window, 0.0));
        let pack = found_pack(&mut fauna, &world, StartingStores::FULL);
        // Before the window: the hold is met, nothing is conceived, and the season says why.
        let early = run(&mut fauna, &mut world, &mut flora, 100);
        assert!(early.is_empty(), "conceived before the window: {early:?}");
        assert!(fauna.ledger.season_waits > 0, "a ready breeder waited on its season");
        // Through the first window and past it.
        let w0 = (phase * yt as f64) as u64;
        let w1 = ((phase + window) * yt as f64) as u64;
        jump(&mut world, w0 - 20);
        let first = run(&mut fauna, &mut world, &mut flora, w1 - w0 + 120);
        assert!(first.iter().all(|&t| open(t)), "lpy {lpy}: conceived outside the window: {first:?} (window {w0}..{w1})");
        assert_eq!(first.len() as u32, lpy, "lpy {lpy}: conceptions in one year's window: {first:?}");
        assert_eq!(fauna.pack(pack).expect("pack").season.used_in(0), lpy, "counted on the pack");
        // The next year's window.
        jump(&mut world, yt + w0 - 20);
        let second = run(&mut fauna, &mut world, &mut flora, w1 - w0 + 120);
        assert!(second.iter().all(|&t| open(t)), "{second:?}");
        assert_eq!(second.len() as u32, lpy, "lpy {lpy}: the next year breeds again: {second:?}");
    }
}

/// A breeder under the breeding reserve does not conceive, though the ordinary surplus rule is
/// met; at the reserve it does. No season here.
#[test]
fn a_breeder_below_the_breeding_reserve_does_not_conceive() {
    let threshold = 0.6;
    let mut world = year_world(22, 1.0);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(veilhound_config(0, 0.0, 0.0, threshold));
    let pack = found_pack(&mut fauna, &world, StartingStores::FULL);
    let breeder = fauna.pack(pack).and_then(|p| p.breeder).expect("a breeder");
    let low = 0.5 * threshold;
    set_reserve_fraction(&mut fauna, breeder, low);
    let sc = crate::body::effective_config(&fauna.config, &animal(&fauna, breeder));
    assert!(animal(&fauna, breeder).reserve > sc.reproduction.surplus_reserve(&sc), "the surplus rule alone would let it breed");
    let none = run(&mut fauna, &mut world, &mut flora, 120);
    assert!(none.is_empty(), "conceived at reserve fraction {}: {none:?}", reserve_fraction(&fauna, breeder));
    set_reserve_fraction(&mut fauna, breeder, 0.5 * (1.0 + threshold));
    let some = run(&mut fauna, &mut world, &mut flora, 120);
    assert!(!some.is_empty(), "no conception at the breeding reserve");
}

/// The litter follows the breeder's condition: one young just over the breeding reserve, the
/// most near a full reserve, and never more as the condition falls.
#[test]
fn litter_size_falls_with_the_breeders_condition() {
    let (threshold, hi) = (0.6, 4u32);
    // The rule itself.
    let mut t = crate::Pack::CHORISTER;
    t.breed_reserve_fraction = threshold;
    t.litter_size_max = hi;
    let sizes: Vec<u32> = (0..=40).map(|k| t.condition_litter(threshold + (1.0 - threshold) * f64::from(k) / 40.0)).collect();
    assert_eq!(sizes[0], 1, "one young at the threshold");
    assert_eq!(*sizes.last().unwrap(), hi, "the most at a full reserve");
    assert!(sizes.windows(2).all(|w| w[0] <= w[1]), "never more young in poorer condition: {sizes:?}");
    // In the world: two breeders, one just over the reserve and one nearly full.
    let mut litter = |fraction: f64| -> u32 {
        let mut world = year_world(23, 1.0);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(veilhound_config(0, 0.0, 0.0, threshold));
        let pack = found_pack(&mut fauna, &world, StartingStores::FULL);
        let breeder = fauna.pack(pack).and_then(|p| p.breeder).expect("a breeder");
        set_reserve_fraction(&mut fauna, breeder, fraction);
        let got = conceptions(&mut fauna, &mut world, &mut flora, 60);
        assert!(!got.is_empty(), "a conception at {fraction}");
        got[0].1
    };
    let poor = litter(threshold + 0.05 * (1.0 - threshold));
    let fat = litter(1.0 - 0.02 * (1.0 - threshold));
    assert_eq!(poor, 1, "a breeder just over the reserve carries one");
    assert_eq!(fat, hi, "a nearly full breeder carries litter_size_max");
}

/// Pups go unprovisioned when the pack has no food — empty stomachs and every adult's reserve
/// under `provision_reserve_fraction` — and an unfed pup starves; the same pup at a fed den
/// is provisioned and lives.
#[test]
fn pups_go_unprovisioned_when_the_pack_has_no_food_and_an_unfed_pup_starves() {
    let mut out = Vec::new();
    for fed in [false, true] {
        let mut world = year_world(24, 1.0);
        let mut flora = Flora::new(FloraConfig::default());
        let mut config = veilhound_config(0, 0.0, 0.0, 0.0);
        config.founders[C.index()].core.reproduction.surplus_floor = 1e9; // no breeding here
        let mut fauna = Fauna::new(config);
        let pack = found_pack(&mut fauna, &world, StartingStores::FULL);
        let t = fauna.config.founder(C).pack.expect("pack");
        let adults: Vec<u64> = fauna.animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == pack)).map(|a| a.id).collect();
        for &id in &adults {
            let f = if fed { 0.5 * (1.0 + t.provision_reserve_fraction) } else { 0.5 * t.provision_reserve_fraction };
            set_reserve_fraction(&mut fauna, id, f);
            animal_mut(&mut fauna, id).stomach = crate::Stomach::default();
        }
        // A pup at the den on its last reserve: its body at `body_min`, a few ticks of upkeep.
        let pup = introduce(&mut fauna, &world, C, StartingStores { body: 0.3, reserve: 0.0 });
        let sc = crate::body::effective_config(&fauna.config, &animal(&fauna, pup));
        let upkeep_tick = sc.maintenance_per_s * sc.body_min * crate::DT;
        {
            let p = animal_mut(&mut fauna, pup);
            p.pack = Some(PackMember { pack, pup: true, calling_ticks: 0 });
            p.body = sc.body_min;
            p.reserve = 20.0 * upkeep_tick;
            p.stomach = crate::Stomach::default();
        }
        place(&mut fauna, pup, STRIKER_X, LANE_Z, EAST);
        let given0 = (fauna.ledger.provision_given_organic, fauna.ledger.regurgitated.organic);
        run(&mut fauna, &mut world, &mut flora, 150);
        let given = fauna.ledger.provision_given_organic - given0.0 + fauna.ledger.regurgitated.organic - given0.1;
        let alive = fauna.view().animal(pup).is_some();
        out.push((fed, given, alive, fauna.ledger.departed_founder(C, Departure::Starved)));
    }
    let (_, given, alive, starved) = out[0];
    assert_eq!(given, 0.0, "nothing passed to the pup by a pack with no food");
    assert!(!alive && starved >= 1, "the unfed pup starved: {out:?}");
    let (_, given, alive, _) = out[1];
    assert!(given > 0.0 && alive, "a fed pack provisions the same pup and it lives: {out:?}");
}

/// Pregnancy raises upkeep: a gestating body pays `gestation_upkeep_factor` × the basal upkeep
/// of the same body not gestating.
#[test]
fn pregnancy_raises_upkeep() {
    let factor = 2.5;
    let f = Founder::Browser;
    let mut upkeep = Vec::new();
    for pregnant in [false, true] {
        let mut world = year_world(25, 1.0);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = FaunaConfig::default();
        c.founders[f.index()].active = Activity::Always;
        let r = &mut c.founders[f.index()].core.reproduction;
        r.gestation_upkeep_factor = factor;
        r.gestation_s = 1.0e6;
        let mut fauna = Fauna::new(c);
        let id = introduce(&mut fauna, &world, f, StartingStores::FULL);
        if pregnant {
            animal_mut(&mut fauna, id).reproduction.escrow = Some(Escrow { young: 1, ..Escrow::default() });
        }
        let before = fauna.ledger.upkeep_by_founder[f.index()];
        run(&mut fauna, &mut world, &mut flora, 1);
        upkeep.push(fauna.ledger.upkeep_by_founder[f.index()] - before);
    }
    assert!(upkeep[0] > 0.0);
    assert!((upkeep[1] / upkeep[0] - factor).abs() < 1e-9, "pregnant / not: {upkeep:?}");
}

/// A lineage without a season breeds exactly as before: with `litters_per_year` 0 the window
/// knobs and the year clock change no conception (the same ticks with a year and with none,
/// under a window that would shut every tick); the same window on a seasonal lineage shuts
/// them all.
#[test]
fn a_lineage_without_a_season_breeds_exactly_as_before() {
    let f = Founder::Browser;
    let births = |litters_per_year: u32, year_days: f64| -> Vec<u64> {
        let mut world = year_world(26, year_days);
        let mut flora = Flora::new(FloraConfig::default());
        let mut c = FaunaConfig::default();
        c.founders[f.index()].active = Activity::Always;
        let core = &mut c.founders[f.index()].core;
        core.reserve_cap = 10.0;
        core.reproduction = Reproduction {
            surplus_floor: 0.0,
            surplus_hold_s: 1.5,
            gestation_s: 1.0,
            birth_interval_s: 0.05,
            litters_per_year,
            breeding_phase: 0.9,
            breeding_window: 0.0,
            ..Reproduction::LIVE_BIRTH_PLACEHOLDER
        };
        let mut fauna = Fauna::new(c);
        introduce(&mut fauna, &world, f, StartingStores::FULL);
        run(&mut fauna, &mut world, &mut flora, 200)
    };
    let with_year = births(0, 1.0);
    let no_year = births(0, 0.0);
    assert!(!with_year.is_empty(), "the fast rule breeds in 200 ticks");
    assert_eq!(with_year, no_year, "no season: the year changes nothing");
    assert!(births(1, 1.0).is_empty(), "the same shut window does hold a seasonal lineage");
}
