//! Independent mechanism fixtures for the glowcap substrate and mycelium budget.
//! Every expectation is computed from this fixture's config; no preset is pinned.

use super::*;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

const FUNGUS: Species = Species::Glowcap;

fn site() -> Site {
    Site { x: 1, y: 2, z: 0 }
}

fn world() -> World {
    let mut w = World::empty(VoxelConfig {
        width: 3,
        height: 7,
        depth: 1,
        voxel_m: 1.0,
        seed: 83,
        ..VoxelConfig::default()
    });
    let water = w.config().materials.field_capacity(Material::Soil)
        * w.config().materials.pore_capacity(Material::Soil)
        * w.config().voxel_volume();
    for x in 0..3 {
        for y in 1..=2 {
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z: 0,
                volume_m3: water,
            });
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Soil,
            });
        }
    }
    w
}

fn config() -> FloraConfig {
    let mut c = FloraConfig::for_voxel_size(1.0);
    c.decomposition = 0.0;
    c.wood_decomposition = 0.0;
    c.carrion_decomposition = 0.0;
    c.upkeep_q10 = 1.0;
    c.initial_mineral = 1.0;
    c.step_every = 1;
    for sp in Species::ALL {
        let s = c.species_mut(sp);
        s.propagule_rate = 0.0;
        s.seed_attrition_per_s = 0.0;
        s.burr_pickup = 0.0;
        s.cure_after_s = 0.0;
    }
    let s = c.species_mut(FUNGUS);
    s.substrate_budgeted = true;
    s.substrate_required = true;
    s.mycelium_graze_fraction = 0.75;
    s.rooting_radius = 0;
    s.rooting_depth = 1;
    s.wood_max = 1.0;
    s.alpha = 0.5;
    s.reserve_cap = 0.2;
    s.maintenance = 0.4;
    s.dieback = 1.0;
    s.alive_min = 0.3;
    s.foliage_rate = 0.0;
    s.wood_rate = 0.0;
    s.senescence = 0.0;
    s.dark_uptake_per_s = 0.0;
    s.substrate_uptake_per_s = 10.0;
    s.substrate_yield = 0.5;
    s.energy_density = 2.0;
    s.defence_per_bite = 0.0;
    s.establish_substrate_min = 0.1;
    s.establish_water_min = 0.0;
    s.establish_saturated_max = 1.0;
    s.wilt_water = 0.0;
    s.full_water = 0.5;
    c.litter_energy_cap = c.glowcap.energy_density;
    c
}

fn seed(c: FloraConfig, w: &World) -> Flora {
    let wood = c.glowcap.wood_max;
    let mut f = Flora::new(c);
    assert!(f.apply(
        w,
        Command::SeedOnFace {
            site: site(),
            species: FUNGUS,
            wood
        }
    ));
    let gates = f.view().establishment_gates(&w.view(), site(), FUNGUS);
    assert_eq!(
        f.config.glowcap.moisture_at(gates.mean_water.unwrap()),
        1.0,
        "fixture: pore water funds full-moisture uptake"
    );
    f
}

fn litter(f: &mut Flora, organic: f64) {
    let s = &f.config.glowcap;
    assert!(f.deposit(
        site(),
        Deposit {
            kind: DepositKind::Litter,
            organic,
            mineral: s.n_tissue * organic,
            energy: s.energy_density * organic,
        }
    ));
}

fn close(f: &Flora) {
    let v = f.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() < 1e-10 * held.abs().max(1.0),
            "{what}: held {held}, expected {expected}"
        );
    }
}

fn crop(f: &mut Flora) {
    f.take_foliage_of(site(), FaceLayer::Fungus, f64::MAX, &(i64::MIN..=i64::MAX))
        .expect("caps above the configured refuge");
    let v = f.view();
    assert!(v.edible_foliage(v.fungus_at(site()).unwrap()) <= 1e-12);
    close(f);
}

#[test]
fn a_full_budgeted_fungus_takes_only_upkeep_and_leaves_litter_for_a_shredder() {
    let mut w = world();
    let c = config();
    let sc = c.glowcap.clone();
    let stock = 1.0;
    let expected = sc.maintenance * sc.wood_max * DT / sc.substrate_yield;
    let legacy = sc.substrate_uptake_per_s * sc.wood_max * DT;
    assert!(
        expected < legacy && legacy < stock,
        "fixture: an excessive raw uptake"
    );
    for budgeted in [true, false] {
        let mut c = c.clone();
        c.glowcap.substrate_budgeted = budgeted;
        let mut f = seed(c, &w);
        litter(&mut f, stock);
        f.step(&mut w);
        let take = if budgeted { expected } else { legacy };
        let actual = f.view().ledger.substrate_uptake[FUNGUS.index()];
        assert!(
            (actual - take).abs() < 1e-12,
            "budgeted={budgeted}: took {actual}, expected {take}"
        );
        assert!((f.view().ground_at(site()).unwrap().litter - (stock - take)).abs() < 1e-12);
        let shredder = f
            .take_litter(site(), stock - take)
            .expect("remaining litter is edible");
        assert!((shredder.organic - (stock - take)).abs() < 1e-12);
        close(&f);
    }
}

#[test]
fn the_substrate_budget_includes_construction_and_reserve_room() {
    let mut w = world();
    let mut c = config();
    c.glowcap.foliage_rate = 0.3;
    c.glowcap.wood_rate = 0.2;
    c.glowcap.build = 0.4;
    let sc = c.glowcap.clone();
    let mut f = Flora::new(c);
    let wood = 0.5 * sc.wood_max;
    assert!(f.apply(
        &w,
        Command::SeedOnFace {
            site: site(),
            species: FUNGUS,
            wood
        }
    ));
    crop(&mut f);
    // Transfer reserve out explicitly, keeping the fixture's books honest.
    let i = find_stand(&f.config, &f.stands, site(), FaceLayer::Fungus).unwrap();
    let removed = 0.5 * f.stands[i].reserve;
    f.stands[i].reserve -= removed;
    f.ledger.removed_organic_out += removed;
    f.ledger.removed_energy_out += sc.energy_density * removed;
    let before = *f.view().fungus_at(site()).unwrap();
    let upkeep = sc.maintenance * before.wood * DT;
    let caps = (sc.alpha * before.wood - before.foliage)
        .max(0.0)
        .min(sc.foliage_rate * before.wood * DT);
    let wood_room = (sc.wood_max - before.wood)
        .max(0.0)
        .min(sc.wood_rate * before.wood * DT);
    let reserve = (sc.reserve_cap * before.wood - before.reserve).max(0.0);
    let demand = upkeep + (1.0 + sc.build) * (caps + wood_room) + reserve;
    let raw = sc.substrate_uptake_per_s * before.wood * DT;
    assert!(
        demand / sc.substrate_yield < raw,
        "fixture: demand rather than uptake binds"
    );
    litter(&mut f, 1.0);
    f.step(&mut w);
    let actual = f.view().ledger.substrate_uptake[FUNGUS.index()];
    let expected = demand / sc.substrate_yield;
    assert!((actual - expected).abs() < 1e-12,
        "took {actual}, expected {expected}; upkeep={upkeep}, caps={caps}, wood={wood_room}, reserve={reserve}");
    close(&f);
}

#[test]
fn required_substrate_closes_the_dark_empty_face_gate() {
    let w = world();
    let mut sc = config().glowcap;
    sc.dark_establish_min = 0.25;
    sc.dark_sky_max = 0.75;
    sc.dark_sky_full = 0.25;
    for required in [true, false] {
        sc.substrate_required = required;
        let empty = step::establishment_gates_with_sky(&w.view(), site(), &sc, 0.0, 0.0, 0.0);
        assert_eq!(
            empty.substrate_ok, !required,
            "required={required}, {empty:?}"
        );
        assert_eq!(
            empty.passes(),
            !required,
            "the substrate gate decides this fixture"
        );
        let fed = step::establishment_gates_with_sky(
            &w.view(),
            site(),
            &sc,
            0.0,
            0.0,
            sc.establish_substrate_min,
        );
        assert!(fed.passes(), "litter opens the required gate: {fed:?}");
    }
}

#[test]
fn required_substrate_prevents_a_dark_seep_from_paying_on_an_empty_face() {
    let w = world();
    let mut c = config();
    c.glowcap.reserve_cap = 0.0;
    c.glowcap.maintenance = 0.1 / DT;
    c.glowcap.dark_uptake_per_s = 4.0 / DT;
    for required in [true, false] {
        c.glowcap.substrate_required = required;
        let mut f = seed(c.clone(), &w);
        let initial = f.stands[0].wood;
        let climate = step::Climate {
            daylight: 0.0,
            upkeep: 1.0,
            night: 1.0,
            to_dawn_s: None,
            draw_key: 1,
            dt: DT,
        };
        step::grow_stand_cause(
            &c,
            &mut f.stands[0],
            &mut f.ground[0],
            0.0,
            climate,
            step::Drink {
                moisture: 1.0,
                darkness: 1.0,
                ..Default::default()
            },
            step::Substrate::default(),
            &mut step::Booked::default(),
        );
        let lost = if required {
            c.glowcap.dieback * c.glowcap.maintenance * initial * DT
        } else {
            0.0
        };
        assert!(
            (f.stands[0].wood - (initial - lost)).abs() < 1e-12,
            "required={required}: the substrate-free seep paid unpaid upkeep"
        );
    }
}

#[test]
fn an_exhausted_fungus_spends_its_reserve_then_dies_back_to_litter() {
    let mut w = world();
    let mut c = config();
    c.glowcap.maintenance = 0.4 / DT;
    c.glowcap.reserve_cap = 0.8;
    c.glowcap.dieback = 3.0;
    let sc = c.glowcap.clone();
    let mut f = seed(c, &w);
    let initial = *f.view().fungus_at(site()).unwrap();
    let bill = sc.maintenance * initial.wood * DT;
    let paid_ticks = (initial.reserve / bill).round() as usize;
    for t in 1..=paid_ticks {
        f.step(&mut w);
        let s = f.view().fungus_at(site()).unwrap();
        assert!((s.reserve - (initial.reserve - bill * t as f64).max(0.0)).abs() < 1e-12);
        assert_eq!(s.wood, initial.wood);
        close(&f);
    }
    f.step(&mut w);
    assert!(
        f.view().fungus_at(site()).is_none(),
        "the unpaid configured bill kills this thin stand"
    );
    let g = f.view().ground_at(site()).unwrap();
    assert_eq!(
        g.dead_wood, 0.0,
        "mycelium cannot become a reusable wood log"
    );
    assert!((g.litter - (initial.wood + initial.foliage)).abs() < 1e-12);
    assert_eq!(
        f.view().ledger.deaths_by_cause[DeathCause::Starved.index()][FUNGUS.index()],
        1
    );
    close(&f);
}

#[test]
fn fungal_corpse_wood_and_all_other_stocks_enter_litter() {
    let w = world();
    let c = config();
    let mut f = seed(c, &w);
    let s = f.stands.remove(0);
    step::die(
        &f.config,
        &w.view(),
        &s,
        &mut f.ground,
        &mut f.ledger,
        DeathCause::Starved,
        &mut f.death_log,
    );
    let g = f.view().ground_at(site()).unwrap();
    assert_eq!(g.dead_wood, 0.0);
    assert!((g.litter - s.material()).abs() < 1e-12);
    assert!((g.litter_mineral - s.mineral).abs() < 1e-12);
    assert!((g.litter_energy - f.config.glowcap.energy_density * s.material()).abs() < 1e-12);
    close(&f);
}

#[test]
fn a_mycelium_bite_uses_the_configured_fraction_and_a_thin_fungus_dies_next_step() {
    let mut w = world();
    let mut c = config();
    c.glowcap.maintenance = 0.0;
    let sc = c.glowcap.clone();
    let mut f = seed(c, &w);
    assert!(f.apply(
        &w,
        Command::SeedOnFace {
            site: site(),
            species: Species::Springturf,
            wood: f.config.springturf.wood_max,
        }
    ));
    let plant = *f.view().stand_at(site()).unwrap();
    crop(&mut f);
    let before = *f.view().fungus_at(site()).unwrap();
    let offer = sc.mycelium_graze_fraction * before.wood;
    assert!((f.view().edible_mycelium(&before) - offer).abs() < 1e-12);
    let taken = f
        .take_mycelium(site(), 2.0 * offer)
        .expect("the exposed mycelium");
    assert!((taken.organic - offer).abs() < 1e-12);
    assert!((taken.mineral - before.mineral * offer / before.material()).abs() < 1e-12);
    assert!((taken.energy - sc.energy_density * offer).abs() < 1e-12);
    assert!((f.view().fungus_at(site()).unwrap().wood - (before.wood - offer)).abs() < 1e-12);
    assert!((f.view().ledger.mycelium_taken[FUNGUS.index()] - offer).abs() < 1e-12);
    assert_eq!(
        f.view().stand_at(site()).unwrap().material(),
        plant.material()
    );
    assert!(
        before.wood - offer < sc.alive_min,
        "fixture: the bite leaves the fungus below its threshold"
    );
    close(&f);
    f.step(&mut w);
    assert!(f.view().fungus_at(site()).is_none());
    assert!(
        f.view().stand_at(site()).is_some(),
        "the shared plant survives fungal grazing"
    );
    close(&f);
}
