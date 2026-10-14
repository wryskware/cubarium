//! N-POOL: protein digestibility per food and the absorbed-nitrogen pool. A child of `step`,
//! so a case can hand a body bites straight to [`assimilate_food`] and run the pool's own
//! step; every direct edit is booked as introduced matter, so the layer's residuals stay
//! closed. Laws only: no tuned value is pinned.

use super::{assimilate_food, n_pool_step};
use crate::{Command, DT, Fauna, FaunaConfig, Founder, SpeciesConfig, StartingStores};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Taken};

/// A flat soil plain, one voxel deep, top face at y = 2, with the day clock off.
fn plain(width: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 10,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
        day: cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
        ..VoxelConfig::default()
    });
    for x in 0..width as i64 {
        for y in 1..=2 {
            let want = 0.3 * w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: want });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    w
}

/// The browser's core edited, no upkeep (so respiration sheds nothing a case weighs) and an
/// empty stomach at arrival (`s0` 0).
fn config_with(edit: impl FnOnce(&mut SpeciesConfig)) -> FaunaConfig {
    let mut c = FaunaConfig::placeholder();
    for p in c.founders.iter_mut() {
        p.gut.s0 = 0.0;
    }
    let core = &mut c.founders[Founder::Browser.index()].core;
    core.maintenance_per_s = 0.0;
    edit(core);
    c
}

/// A grown browser with its reserve at `reserve` of its cap (1: no room), no fuel, on
/// column `x`.
fn grazer(fauna: &mut Fauna, world: &World, x: i64, reserve: f64) -> usize {
    assert!(fauna.apply(
        world,
        Command::IntroduceFounder {
            x,
            z: 0,
            founder: Founder::Browser,
            stores: StartingStores { body: 1.0, reserve },
            heading_rad: 0.0,
        }
    ));
    let i = fauna.animals.len() - 1;
    assert_eq!(fauna.animals[i].fuel, 0.0, "a case starts without fuel");
    assert_eq!(fauna.animals[i].n_pool, 0.0, "and with an empty pool");
    i
}

/// One bite `t` straight to [`assimilate_food`], booked as introduced. Returns what it
/// placed in the reserve.
fn bite(fauna: &mut Fauna, i: usize, t: Taken, y: f64, d: f64) -> f64 {
    fauna.ledger.introduced_organic_in += t.organic;
    fauna.ledger.introduced_mineral_in += t.mineral;
    fauna.ledger.introduced_energy_in += t.energy;
    let sc = super::body::effective_config(&fauna.config, &fauna.animals[i]);
    assimilate_food(fauna, i, &sc, t, y, d)
}

/// The pool emptied into the gut, as its decay empties it: mineral only moves.
fn empty_pool(fauna: &mut Fauna, i: usize) {
    let a = &mut fauna.animals[i];
    a.gut.mineral += a.n_pool;
    a.n_pool = 0.0;
}

fn assert_fauna_closes(fauna: &Fauna, when: &str) {
    let v = fauna.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() <= 1e-12 * held.abs().max(1e-6),
            "{when}: fauna {what} holds {held}, the ledger expects {expected}"
        );
    }
}

fn assert_flora_closes(flora: &Flora, when: &str) {
    let v = flora.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() <= 1e-9 * held.abs().max(1.0),
            "{when}: flora {what} holds {held}, the ledger expects {expected}"
        );
    }
}

/// An N-rich bite (its digestible N more than its own absorbed carbon can match) and a
/// carbon-rich, N-poor one (absorbed carbon its own N cannot fund).
fn two_bites(n: f64) -> [(Taken, f64, f64); 2] {
    let b = 1e-4;
    let rich = Taken { organic: b, mineral: 2.0 * n * b, energy: b };
    let poor = Taken { organic: b, mineral: 0.1 * n * b, energy: b };
    [(rich, 0.5, 0.8), (poor, 0.6, 0.5)]
}

/// The two bites within the pool's life build more reserve tissue together than the sum of
/// each alone; with the pool emptied between them, exactly the sum. No fuel is kept (its cap
/// is zero), so the only meeting place is the pool.
#[test]
fn an_n_rich_then_a_carbon_rich_bite_build_more_together_than_apart() {
    let world = plain(8);
    let mut fauna = Fauna::new(config_with(|s| s.fuel_cap = 0.0));
    let n = fauna.config.founder(Founder::Browser).core.n_tissue;
    let [rich, poor] = two_bites(n);

    let alone_rich = grazer(&mut fauna, &world, 1, 0.1);
    let r = bite(&mut fauna, alone_rich, rich.0, rich.1, rich.2);
    let alone_poor = grazer(&mut fauna, &world, 2, 0.1);
    let p = bite(&mut fauna, alone_poor, poor.0, poor.1, poor.2);
    assert!(fauna.animals[alone_rich].n_pool > 0.0, "the N-rich bite leaves N its carbon cannot match");
    assert_eq!(fauna.animals[alone_poor].n_pool, 0.0, "the N-poor bite's N is all built");

    let together = grazer(&mut fauna, &world, 3, 0.1);
    let t = bite(&mut fauna, together, rich.0, rich.1, rich.2) + bite(&mut fauna, together, poor.0, poor.1, poor.2);
    assert!(t > (r + p) * (1.0 + 1e-9), "together {t} against apart {}", r + p);

    let emptied = grazer(&mut fauna, &world, 4, 0.1);
    let first = bite(&mut fauna, emptied, rich.0, rich.1, rich.2);
    empty_pool(&mut fauna, emptied);
    let e = first + bite(&mut fauna, emptied, poor.0, poor.1, poor.2);
    assert!((e - (r + p)).abs() <= 1e-15, "emptied between them {e} against apart {}", r + p);
    assert_fauna_closes(&fauna, "after the paired bites");
}

/// N left in the pool builds reserve tissue out of stored fuel's carbon, and only as far as
/// the pool's N funds it; the energy moves with the fuel.
#[test]
fn pool_n_builds_with_stored_fuel_as_far_as_it_funds() {
    let world = plain(6);
    let mut fauna = Fauna::new(config_with(|_| {}));
    let n = fauna.config.founder(Founder::Browser).core.n_tissue;
    let i = grazer(&mut fauna, &world, 2, 0.1);
    // Fuel from a mineral-free bite.
    bite(&mut fauna, i, Taken { organic: 1e-3, mineral: 0.0, energy: 1e-3 }, 0.9, 1.0);
    let fueled = fauna.animals[i];
    assert!(fueled.fuel > 0.0 && fueled.n_pool == 0.0);
    // Protein N with almost no carbon of its own.
    let m = 1e-6;
    let placed = bite(&mut fauna, i, Taken { organic: 1e-12, mineral: m, energy: 1e-12 }, 0.5, 1.0);
    let a = fauna.animals[i];
    assert!(placed > 0.0, "the fuel's carbon builds with the pool's N");
    assert!(placed * n <= m * (1.0 + 1e-12), "never more than the N funds");
    assert!(a.fuel < fueled.fuel, "out of the fuel");
    assert!(a.fuel_energy < fueled.fuel_energy && a.energy > fueled.energy, "its energy moved with it");
    assert!((a.mineral + a.n_pool - fueled.mineral - m).abs() <= 1e-18, "the N is the bite's");
    assert_fauna_closes(&fauna, "after pairing with fuel");
}

/// At equal N eaten, a more digestible food puts more N in the pool (and less in the gut).
#[test]
fn at_equal_n_eaten_a_more_digestible_food_puts_more_n_in_the_pool() {
    let world = plain(6);
    // No reserve room, so nothing is built and the pool shows what was absorbed.
    let mut fauna = Fauna::new(config_with(|_| {}));
    let hi = grazer(&mut fauna, &world, 1, 1.0);
    let lo = grazer(&mut fauna, &world, 3, 1.0);
    let t = Taken { organic: 1e-4, mineral: 4e-6, energy: 1e-4 };
    assert_eq!(bite(&mut fauna, hi, t, 0.5, 0.9), 0.0);
    assert_eq!(bite(&mut fauna, lo, t, 0.5, 0.5), 0.0);
    let (h, l) = (fauna.animals[hi], fauna.animals[lo]);
    assert!(h.n_pool > l.n_pool, "{} against {}", h.n_pool, l.n_pool);
    assert!(h.gut.mineral < l.gut.mineral);
    assert_fauna_closes(&fauna, "after the two bites");
}

/// Pool N not built within its life leaves through the gut as dung: after enough
/// half-lives the pool is nearly empty, the dung carried its N to the ground, and both
/// layers' mineral ledgers balance.
#[test]
fn pool_n_not_built_leaves_through_the_gut_as_dung_and_the_ledgers_balance() {
    let world = plain(6);
    let mut flora = Flora::new(FloraConfig::default());
    let half_life = 2.0;
    let mut fauna = Fauna::new(config_with(|s| {
        s.n_pool_half_life_s = half_life;
        s.n_pool_cap = 1.0;
    }));
    let i = grazer(&mut fauna, &world, 2, 1.0);
    let t = Taken { organic: 1e-4, mineral: 4e-6, energy: 1e-4 };
    bite(&mut fauna, i, t, 0.5, 1.0);
    let held = fauna.animals[i].n_pool;
    assert!(held > 0.0);
    let deposited_before = fauna.ledger.deposited_mineral_out;
    // Ten half-lives, then on to the next void.
    let ticks = (10.0 * half_life / DT).ceil() as u64;
    let period = (fauna.config.life.gut_void_s / DT).round() as u64;
    let mut k = 0;
    while k < ticks || fauna.animals[i].age_ticks % period != 0 {
        fauna.step(&world, &mut flora);
        k += 1;
    }
    let a = fauna.animals[i];
    assert!(a.n_pool <= held * 0.5f64.powi(9), "the pool decayed: {} of {held}", a.n_pool);
    assert!(
        fauna.ledger.deposited_mineral_out - deposited_before >= (held - a.n_pool) * (1.0 - 1e-9),
        "its N left as dung"
    );
    assert_fauna_closes(&fauna, "after the pool decayed");
    assert_flora_closes(&flora, "after the dung landed");
}

/// The pool is held to its cap relative to body; the overflow goes to the gut.
#[test]
fn the_pool_is_held_to_its_cap_and_the_overflow_goes_to_the_gut() {
    let world = plain(6);
    let mut fauna = Fauna::new(config_with(|s| s.n_pool_cap = 1e-4));
    let i = grazer(&mut fauna, &world, 2, 1.0);
    let sc = super::body::effective_config(&fauna.config, &fauna.animals[i]);
    bite(&mut fauna, i, Taken { organic: 1e-4, mineral: 4e-5, energy: 1e-4 }, 0.5, 1.0);
    let before = fauna.animals[i];
    n_pool_step(&mut fauna);
    let a = fauna.animals[i];
    let cap = sc.n_pool_cap * sc.n_tissue * a.body;
    assert!(before.n_pool > cap && a.n_pool <= cap * (1.0 + 1e-12));
    assert!((a.gut.mineral - before.gut.mineral - (before.n_pool - a.n_pool)).abs() <= 1e-18);
    assert_fauna_closes(&fauna, "after the cap");
}

/// Organic matter, mineral and energy are conserved across bites of every kind, the pool's
/// step (pairing, decay, cap) and the gut's voids.
#[test]
fn organic_mineral_and_energy_are_conserved_across_bites_decay_and_voiding() {
    let world = plain(6);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config_with(|s| s.n_pool_half_life_s = 5.0));
    let n = fauna.config.founder(Founder::Browser).core.n_tissue;
    let i = grazer(&mut fauna, &world, 2, 0.05);
    let [rich, poor] = two_bites(n);
    for k in 0..700u32 {
        match k % 4 {
            0 => { bite(&mut fauna, i, rich.0, rich.1, rich.2); }
            1 => { bite(&mut fauna, i, poor.0, poor.1, poor.2); }
            2 => { bite(&mut fauna, i, Taken { organic: 1e-4, mineral: 0.0, energy: 2e-4 }, 0.95, 0.8); }
            _ => {}
        }
        fauna.step(&world, &mut flora);
        assert_fauna_closes(&fauna, &format!("tick {k}"));
    }
    assert!(fauna.ledger.deposited_mineral_out > 0.0, "dung was voided");
    assert_flora_closes(&flora, "after the voids");
}

/// A save and load keeps the pool.
#[test]
fn a_save_load_round_trip_keeps_the_pool() {
    let world = plain(6);
    let mut fauna = Fauna::new(config_with(|_| {}));
    let i = grazer(&mut fauna, &world, 2, 1.0);
    bite(&mut fauna, i, Taken { organic: 1e-4, mineral: 4e-6, energy: 1e-4 }, 0.5, 0.9);
    let pool = fauna.animals[i].n_pool;
    assert!(pool > 0.0);
    let loaded = crate::snapshot::decode(&crate::snapshot::encode(&fauna)).unwrap();
    let id = fauna.animals[i].id;
    let a = loaded.animals.iter().find(|a| a.id == id).expect("the body");
    assert_eq!(a.n_pool, pool);
    assert_fauna_closes(&loaded, "after a round trip");
}
