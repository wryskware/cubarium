//! WX3 round 2: plant upkeep follows the air's temperature by a Q10. `Q10^((T - T_mean) / 10)`, Q10 = 2, on maintenance only.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Flora, FloraConfig, Species, upkeep_factor,
};

#[test]
fn the_factor_is_one_at_the_mean_half_ten_below_double_ten_above() {
    assert_eq!(FloraConfig::default().upkeep_q10, 2.0);
    assert_eq!(upkeep_factor(26.0, 26.0, 2.0), 1.0);
    assert!((upkeep_factor(16.0, 26.0, 2.0) - 0.5).abs() < 1e-12);
    assert!((upkeep_factor(36.0, 26.0, 2.0) - 2.0).abs() < 1e-12);
    assert!((upkeep_factor(31.0, 26.0, 2.0) - 2f64.sqrt()).abs() < 1e-12);
}


fn world(day: DayClock, diurnal_c: f64) -> World {
    let mut config = VoxelConfig {
        width: 6,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 7,
        day,
        ..VoxelConfig::default()
    };
    config.climate.diurnal_c = diurnal_c;
    let mut w = World::empty(config);
    let cap = cubarium_voxel::Materials::DEFAULT.pore_capacity(Material::Soil)
        * w.config().voxel_volume();
    for x in 0..6i64 {
        for y in 1..=2u32 {
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z: 0,
                volume_m3: cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil) * cap,
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

fn midnight(diurnal_c: f64) -> World {
    world(
        DayClock {
            start_hour: 0.0,
            ..DayClock::default()
        },
        diurnal_c,
    )
}

/// The reserve one small velvetpad burns in one tick with no light (midnight), and the
/// flora afterwards.
fn burned(w: &mut World) -> (f64, Flora) {
    let mut flora = Flora::new(FloraConfig::default());
    let wood = 0.3 * flora.config().species(Species::Velvetpad).wood_max;
    assert!(flora.apply(
        w,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Velvetpad,
            wood,
        }
    ));
    let before = flora.view().stands[0].reserve;
    flora.step(w);
    (before - flora.view().stands[0].reserve, flora)
}

fn assert_closed(flora: &Flora) {
    let v = flora.view();
    let o = v.organic() - v.ledger.expected_organic();
    let e = v.energy() - v.ledger.expected_energy();
    assert!(o.abs() <= 1e-9 * v.organic().abs().max(1.0), "organic {o}");
    assert!(e.abs() <= 1e-9 * v.energy().abs().max(1.0), "energy {e}");
}

#[test]
fn a_cooler_night_burns_less_reserve_by_the_factor() {
    let mut flat = midnight(0.0);
    let mut swing = midnight(8.0);
    let mean = flat.config().climate.temperature_c;
    assert_eq!(flat.life_temperature_c(), mean);
    let t = swing.life_temperature_c();
    assert!(t < mean - 1.0, "the swing's midnight is cooler: {t} vs {mean}");
    let (a, fa) = burned(&mut flat);
    let (b, fb) = burned(&mut swing);
    assert!(a > 0.0);
    let want = upkeep_factor(t, mean, 2.0);
    assert!(want < 1.0);
    assert!((b / a - want).abs() < 1e-6, "burned {b} / {a}, want {want}");
    assert_closed(&fa);
    assert_closed(&fb);
}

#[test]
fn with_the_clock_off_the_temperature_is_the_mean_and_upkeep_is_unchanged() {
    let off = DayClock {
        mode: DayMode::Off,
        ..DayClock::default()
    };
    let mut a = world(off, 8.0);
    let mean = a.config().climate.temperature_c;
    assert_eq!(a.life_temperature_c(), mean);
    let mut b = world(off, 0.0);
    assert_eq!(burned(&mut a).0, burned(&mut b).0);
}
