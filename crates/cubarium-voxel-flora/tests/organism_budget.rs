//! **The organism budget kernel, through the plant layer's public step**: a few hundred
//! ticks of stands and of a latticevine, every path of the kernel exercised (income built
//! and stored, upkeep from reserve, unpaid upkeep to dieback, a saprotroph's arrived
//! mineral settled), and the plant ledger's three residuals at rounding on every tick.
//! Fixtures follow `glowcap.rs` and `latticevine.rs`; no pinned numbers.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, DT, Deposit, DepositKind, Face, FaceDir, Flora, FloraConfig, Site, Species,
    VineConfig, VineSeed,
};

fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    for (name, got, scale) in [
        (
            "organic",
            v.organic() - v.ledger.expected_organic(),
            v.organic(),
        ),
        (
            "mineral",
            v.mineral() - v.ledger.expected_mineral(),
            v.mineral(),
        ),
        (
            "energy",
            v.energy() - v.ledger.expected_energy(),
            v.energy(),
        ),
    ] {
        assert!(
            got.abs() <= 1e-9 * scale.abs().max(1.0),
            "{when}: {name} residual {got:e}"
        );
    }
}

/// Soil at `y = 1..=2` across a `width × 1` strip of 1 m voxels, holding `pore` of its
/// capacity, open sky, the day clock off (full daylight for life).
fn strip(width: u32, pore: f64) -> World {
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
    let cap = cubarium_voxel::Materials::DEFAULT.pore_capacity(Material::Soil)
        * w.config().voxel_volume();
    for x in 0..width as i64 {
        for y in 1..=2 {
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z: 0,
                volume_m3: pore * cap,
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

fn plant(flora: &mut Flora, world: &World, x: i64, species: Species) -> Site {
    let wood = 0.5 * flora.config().species(species).wood_max;
    assert!(flora.apply(
        world,
        Command::Seed {
            x,
            z: 0,
            species,
            wood
        }
    ));
    Site {
        x: x as u32,
        y: 2,
        z: 0,
    }
}

/// Stands on the kernel: a lit bloomcrown that earns and builds, a velvetpad whose upkeep
/// is far beyond its income (reserve drawn, then unpaid upkeep sent to dead wood), and a
/// glowcap on a log whose income arrives with mineral to settle. 300 ticks, the ledger
/// closed on each.
#[test]
fn stands_close_the_plant_ledger_through_income_upkeep_build_and_dieback() {
    let mut world = strip(8, 0.5);
    let mut config = FloraConfig::default();
    config.species_mut(Species::Velvetpad).maintenance *= 1e4;
    let mut flora = Flora::new(config);
    let lit = plant(&mut flora, &world, 1, Species::Bloomcrown);
    let starved = plant(&mut flora, &world, 4, Species::Velvetpad);
    let fungus = plant(&mut flora, &world, 6, Species::Glowcap);
    let sc = flora.config().species(Species::Glowcap).clone();
    assert!(flora.deposit(
        fungus,
        Deposit {
            kind: DepositKind::DeadWood,
            organic: 0.2,
            mineral: sc.n_tissue * 0.2,
            energy: sc.energy_density * 0.2,
        }
    ));
    assert_residuals(&flora, "seeded");

    let lit0 = *flora.view().stand_at(lit).expect("bloomcrown");
    let starved_reserve0 = flora.view().stand_at(starved).expect("velvetpad").reserve;
    let mut starved_reserve_ran_out = false;
    for tick in 1..=300 {
        flora.step(&mut world);
        let when = format!("tick {tick}");
        assert_residuals(&flora, &when);
        let v = flora.view();
        for s in v.stands.iter() {
            for (x, what) in [
                (s.wood, "wood"),
                (s.foliage, "foliage"),
                (s.reserve, "reserve"),
                (s.mineral, "mineral"),
            ] {
                assert!(
                    x.is_finite() && x >= -1e-12,
                    "{when}: {:?} at {:?} has {what} {x:e}",
                    s.species,
                    s.site
                );
            }
        }
        for g in v.ground.iter() {
            assert!(g.mineral >= -1e-12, "{when}: pool mineral {:e}", g.mineral);
        }
        if let Some(s) = v.stand_at(starved) {
            starved_reserve_ran_out |= s.reserve <= 1e-12 * starved_reserve0;
        }
    }
    let v = flora.view();
    assert!(v.ledger.fixed_in > 0.0, "nothing was fixed");
    assert!(v.ledger.respired_out > 0.0, "nothing was respired");
    let lit1 = v.stand_at(lit).expect("the lit bloomcrown died");
    assert!(
        lit1.wood > lit0.wood,
        "a lit, watered bloomcrown built no wood: {} -> {}",
        lit0.wood,
        lit1.wood
    );
    assert!(
        starved_reserve_ran_out,
        "the over-taxed velvetpad never spent its reserve"
    );
    assert!(
        v.ground_at(starved).map_or(0.0, |g| g.dead_wood) > 0.0,
        "unpaid upkeep sent no wood to dead wood"
    );
    assert!(
        v.stand_at(fungus).is_some_and(|s| s.mineral > 0.0),
        "the glowcap holds no mineral from its log"
    );
}

/// A wall at `x = 6`, `y = 1..=10`, soil at `y = 1..=2` holding available water `avail`
/// (0 wilting point, 1 field capacity), on the 0.25 m voxel: `latticevine.rs`'s plan.
fn wall(avail: f64) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 12,
        height: 14,
        depth: 1,
        voxel_m: 0.25,
        seed: 11,
        ..VoxelConfig::default()
    });
    let m = cubarium_voxel::Materials::DEFAULT;
    let pore = m.wilting_point(Material::Soil)
        + avail * (m.field_capacity(Material::Soil) - m.wilting_point(Material::Soil));
    let cap = m.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for x in 0..12i64 {
        for y in 1..=10u32 {
            let soil = y <= 2 && x != 6;
            if soil && pore > 0.0 {
                w.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z: 0,
                    volume_m3: pore * cap,
                });
            }
            let material = if x == 6 {
                Material::Rock
            } else if soil {
                Material::Soil
            } else {
                continue;
            };
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z: 0,
                material,
            });
        }
    }
    w
}

/// `latticevine.rs`'s `fast` numbers, with a woody upkeep a dormant vine's reserve cannot
/// carry for long.
fn vine_config() -> FloraConfig {
    let mut c = FloraConfig {
        decomposition: 0.0,
        wood_decomposition: 0.0,
        carrion_decomposition: 0.0,
        ..FloraConfig::default()
    };
    c.latticevine = VineConfig {
        rooting_depth: 2,
        rooting_radius: 1,
        transpiration_m3_per_s: 1e-9,
        wilt_water: 0.1,
        full_water: 0.5,
        dry_water: 0.2,
        wet_water: 0.5,
        dry_spell_s: 0.5,
        wet_spell_s: 1.0,
        leaf_fall_s: 0.5,
        assimilation: 2.5,
        underside_light: 0.15,
        leaf_mass: 0.1,
        runner_mass: 0.05,
        leaf_upkeep: 0.01,
        wood_upkeep: 1.0,
        regrow_rate: 1.0,
        spread_cost: 0.2,
        spread_check_s: DT,
        sister_cost: 0.5,
        contest_threshold: 0.3,
        contest_check_s: DT,
        spur_reserve_min: 1e12,
        energy_density: 1.0,
        ..VineConfig::default()
    };
    c
}

/// The latticevine on the kernel: on wet soil it earns, pays its upkeep from income,
/// stores the rest and regrows and spreads out of the reserve; moved to dry soil it goes
/// dormant, pays from the reserve until it is spent and then loses runners to the unpaid
/// rest. 300 ticks, the ledger closed on each and the reserve never negative.
#[test]
fn a_latticevine_closes_the_plant_ledger_through_wet_growth_and_dry_starvation() {
    let mut wet = wall(1.0);
    let mut dry = wall(0.0);
    let mut flora = Flora::new(vine_config());
    let id = flora
        .seed_vine(
            &wet,
            VineSeed {
                root: Site { x: 5, y: 2, z: 0 },
                first: Face::new(6, 3, 0, FaceDir::NegX),
                reserve: 1.0,
                lineage: None,
            },
        )
        .expect("a founder at the wall's foot");
    assert_residuals(&flora, "seeded");

    let mut most_faces = 0;
    let mut spent = false;
    for tick in 1..=300 {
        let world = if tick <= 100 { &mut wet } else { &mut dry };
        flora.step(world);
        let when = format!("tick {tick}");
        assert_residuals(&flora, &when);
        let v = flora.view();
        for vine in v.cover.vines() {
            assert!(
                vine.reserve.is_finite() && vine.reserve >= -1e-12,
                "{when}: vine {} reserve {:e}",
                vine.id,
                vine.reserve
            );
        }
        let faces = v.cover.faces_of(id).len();
        if tick <= 100 {
            most_faces = most_faces.max(faces);
        }
        spent |= v.cover.vine(id).is_none_or(|vine| vine.reserve <= 1e-9);
    }
    let v = flora.view();
    assert!(v.ledger.fixed_in > 0.0, "the wet vine earned nothing");
    assert!(
        most_faces > 1,
        "the wet vine never spread past its first face"
    );
    assert!(spent, "the dry vine never spent its reserve");
    assert!(
        v.cover.faces_of(id).len() < most_faces,
        "unpaid upkeep took no runner: {} faces of {most_faces}",
        v.cover.faces_of(id).len()
    );
}
