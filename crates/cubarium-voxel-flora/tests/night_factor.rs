//! WX3 round 4: the plant night factor. While it is dark a photosynthetic plant's
//! maintenance and senescence run at `plant_night_factor` of their daytime rate, through
//! `World::plant_night_scale()` = `d + (1 − d) · f` with `d` the life daylight. Income,
//! growth and saprotrophs are untouched; the round-2 Q10 composes with it.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Species, Stand};

/// A strip of soil at field capacity under open sky, on `day` with the night factor `f`.
fn world(day: DayClock, f: f64) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 6,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 7,
        day: DayClock {
            plant_night_factor: f,
            ..day
        },
        ..VoxelConfig::default()
    });
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

fn at(hour: f64, f: f64) -> World {
    world(
        DayClock {
            start_hour: hour,
            ..DayClock::default()
        },
        f,
    )
}

fn assert_closed(flora: &Flora) {
    let v = flora.view();
    let o = v.organic() - v.ledger.expected_organic();
    let e = v.energy() - v.ledger.expected_energy();
    assert!(o.abs() <= 1e-9 * v.organic().abs().max(1.0), "organic {o}");
    assert!(e.abs() <= 1e-9 * v.energy().abs().max(1.0), "energy {e}");
}

/// One stand of `species`, small enough to be no donor and seeded with full foliage (well
/// above its reflush line), one tick on `w`: (before, after), with the ledgers checked.
///
/// No nectar or fruit at night (`night_bloom_scale` 0, night survival 1b), so the
/// reserve's fall in the dark is the upkeep alone; full foliage takes no reflush.
fn one_tick(w: &mut World, species: Species) -> (Stand, Stand) {
    let mut config = FloraConfig::default();
    for s in Species::ALL {
        config.species_mut(s).night_bloom_scale = 0.0;
    }
    let mut flora = Flora::new(config);
    let sc = flora.config().species(species).clone();
    let wood = (0.5 * sc.donor_min).max(sc.alive_min);
    assert!(flora.apply(
        w,
        Command::Seed {
            x: 2,
            z: 0,
            species,
            wood,
        }
    ));
    let before = flora.view().stands[0];
    flora.step(w);
    let after = flora.view().stands[0];
    assert_closed(&flora);
    (before, after)
}

/// Reserve burned and foliage shed in one midnight tick at night factor `f`.
fn midnight_losses(f: f64, species: Species) -> (f64, f64) {
    let mut w = at(0.0, f);
    assert_eq!(w.life_daylight(), 0.0);
    let (b, a) = one_tick(&mut w, species);
    assert_eq!(a.wood, b.wood, "no growth and no dieback in one dark tick");
    (b.reserve - a.reserve, b.foliage - a.foliage)
}

#[test]
fn the_scale_is_the_factor_at_midnight_one_at_noon_and_one_with_the_clock_off() {
    assert_eq!(at(0.0, 0.25).plant_night_scale(), 0.25);
    assert_eq!(at(0.0, 1.0).plant_night_scale(), 1.0);
    assert_eq!(at(12.0, 0.0).plant_night_scale(), 1.0);
    let off = world(
        DayClock {
            mode: DayMode::Off,
            start_hour: 0.0,
            ..DayClock::default()
        },
        0.0,
    );
    assert_eq!(off.plant_night_scale(), 1.0);
    assert_eq!(DayClock::default().plant_night_factor, 0.05, "Wrysk's default (2026-09-29)");
}

#[test]
fn twilight_is_continuous() {
    // Sunrise itself: life daylight 0.5, so half the day's rate plus half the night's.
    let d = DayClock::default();
    let w = at(24.0 * (0.5 - d.daylight / 2.0), 0.0);
    assert!((w.life_daylight() - 0.5).abs() < 1e-6);
    assert!((w.plant_night_scale() - 0.5).abs() < 1e-6, "{}", w.plant_night_scale());
}

#[test]
fn a_factor_out_of_range_is_refused() {
    for f in [-0.1, 1.5, f64::NAN] {
        let c = cubarium_voxel::Config {
            day: DayClock {
                plant_night_factor: f,
                ..DayClock::default()
            },
            ..cubarium_voxel::Config::default()
        };
        assert!(c.validate().is_err(), "{f} accepted");
    }
}

#[test]
fn at_factor_one_the_night_burns_what_it_always_did() {
    let (q, p) = midnight_losses(1.0, Species::Bloomcrown);
    assert!(q > 0.0 && p > 0.0, "upkeep {q}, shed {p}");
}

#[test]
fn at_factor_zero_a_plant_neither_respires_nor_sheds_at_midnight() {
    let (q, p) = midnight_losses(0.0, Species::Bloomcrown);
    assert_eq!((q, p), (0.0, 0.0));
}

#[test]
// The differences are read off stocks ~10^5 times larger, so the ratio holds to float
// cancellation, not to the last ulp.
fn a_quarter_factor_is_a_quarter_of_both() {
    let (q1, p1) = midnight_losses(1.0, Species::Bloomcrown);
    let (q, p) = midnight_losses(0.25, Species::Bloomcrown);
    assert!((q - 0.25 * q1).abs() <= 1e-9 * q1, "upkeep {q} against {q1}");
    assert!((p - 0.25 * p1).abs() <= 1e-9 * p1, "shed {p} against {p1}");
}

#[test]
fn noon_does_not_read_the_factor() {
    let (_, a) = one_tick(&mut at(12.0, 0.0), Species::Bloomcrown);
    let (_, b) = one_tick(&mut at(12.0, 1.0), Species::Bloomcrown);
    assert_eq!(a, b);
}

#[test]
fn a_saprotroph_does_not_read_the_factor() {
    let (q0, p0) = midnight_losses(0.0, Species::Glowcap);
    let (q1, p1) = midnight_losses(1.0, Species::Glowcap);
    assert!(q1 > 0.0, "the fixture glowcap pays upkeep: {q1}");
    assert_eq!((q0, p0), (q1, p1));
}
