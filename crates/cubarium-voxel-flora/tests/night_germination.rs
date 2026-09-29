//! WX3 round 4: a photosynthetic seed stays banked in the dark. A newborn that comes up at
//! night has no income until dawn and a reserve sized for minutes, so it starves and frees
//! the site for the next seed: in the 2026-09-28 census about half of all plant deaths were
//! springturf newborns under ten minutes old, all in the dark. Every birth a tick makes —
//! the bank's lottery and a runner's daughter — goes through one gate.

use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Species};

/// A strip of soil at field capacity under open sky, starting at `hour`.
fn world_at(hour: f64) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 6,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 7,
        day: DayClock {
            start_hour: hour,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    for x in 0..6i64 {
        for y in 1..=2u32 {
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z: 0,
                volume_m3: Material::Soil.field_capacity() * cap,
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

/// A bloomcrown donor that lands one package a tick on a hop-1 neighbour, whose bank is
/// checked every tick and germinates on one whole seed. Its establishments after `ticks`.
fn bank_births(hour: f64, ticks: u32) -> (u64, usize) {
    let mut config = FloraConfig::default()
        .drop_seeds_checked_each_tick()
        .minimum_seeds();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    config.bloomcrown.reserve_cap = 4.0;
    config.bloomcrown.clonal_share = 0.0;
    config.bloomcrown.maintenance = 0.0;
    config.bloomcrown.senescence = 0.0;
    let mut world = world_at(hour);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6,
        }
    ));
    for _ in 0..ticks {
        flora.step(&mut world);
    }
    let banked = flora
        .view()
        .ground
        .iter()
        .filter(|g| !g.seeds.is_empty())
        .count();
    (flora.view().ledger.establishments, banked)
}

#[test]
fn a_banked_seed_does_not_germinate_at_midnight() {
    assert_eq!(world_at(0.0).life_daylight(), 0.0);
    let (born, banked) = bank_births(0.0, 6);
    assert_eq!(born, 0, "a seed came up in the dark");
    assert!(banked > 0, "the seeds are still in the bank");
}

#[test]
fn the_same_bank_germinates_at_noon() {
    assert_eq!(world_at(12.0).life_daylight(), 1.0);
    let (born, _) = bank_births(12.0, 6);
    assert!(born > 0, "nothing germinated at noon");
}

/// A springturf whose every package is a runner, funded in one tick.
fn runner_births(hour: f64) -> (u64, usize) {
    let mut config = FloraConfig::default();
    config.springturf.clonal_share = 1.0;
    config.springturf.propagule_rate = 1.0;
    config.springturf.donor_reserve_floor = 0.0;
    let wood = config.springturf.wood_max;
    let mut world = world_at(hour);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Springturf,
            wood,
        }
    ));
    flora.step(&mut world);
    (flora.view().ledger.establishments, flora.view().stands.len())
}

#[test]
fn a_runner_does_not_root_at_midnight() {
    let (born, stands) = runner_births(0.0);
    assert_eq!((born, stands), (0, 1), "a runner rooted in the dark");
}

#[test]
fn the_same_runner_roots_at_noon() {
    let (born, stands) = runner_births(12.0);
    assert_eq!((born, stands), (1, 2), "no runner at noon");
}
