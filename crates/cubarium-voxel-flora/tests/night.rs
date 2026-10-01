//! WX3 (`design/handoffs/voxel-weather-2026-09-24.md`): night stops photosynthesis. Light
//! income is multiplied by `World::life_daylight()`; maintenance and respiration go on.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Species};

/// Soil this dry keeps a velvetpad's moisture factor small, so its income stays well under
/// its demand caps and follows the light.
fn dry_pore() -> f64 {
    let sc = FloraConfig::default().species(Species::Velvetpad).clone();
    let a = sc.wilt_water + 0.002 * (sc.full_water - sc.wilt_water);
    let (wp, fc) = (Material::Soil.wilting_point(), Material::Soil.field_capacity());
    wp + a * (fc - wp)
}

/// A strip of soil under open sky, on a day clock `day`.
fn world_on(day: DayClock) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 6,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 7,
        day,
        ..VoxelConfig::default()
    });
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    for x in 0..6i64 {
        for y in 1..=2u32 {
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z: 0,
                volume_m3: dry_pore() * cap,
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

fn clock(hour: f64) -> DayClock {
    DayClock {
        start_hour: hour,
        ..DayClock::default()
    }
}

/// One small velvetpad (income well under its demand caps), one tick: the energy fixed.
fn fixed_after_one_tick(world: &mut World) -> (f64, Flora) {
    let mut flora = Flora::new(FloraConfig::default());
    let wood = 0.3 * flora.config().species(Species::Velvetpad).wood_max;
    assert!(flora.apply(
        world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Velvetpad,
            wood,
        }
    ));
    flora.step(world);
    (flora.view().ledger.fixed_in, flora)
}

fn assert_closed(flora: &Flora) {
    let v = flora.view();
    let o = v.organic() - v.ledger.expected_organic();
    let e = v.energy() - v.ledger.expected_energy();
    assert!(o.abs() <= 1e-9 * v.organic().abs().max(1.0), "organic {o}");
    assert!(e.abs() <= 1e-9 * v.energy().abs().max(1.0), "energy {e}");
}

#[test]
fn a_lit_stand_earns_nothing_at_midnight() {
    let mut w = world_on(clock(0.0));
    assert_eq!(w.life_daylight(), 0.0);
    let (fixed, flora) = fixed_after_one_tick(&mut w);
    assert_eq!(fixed, 0.0);
    assert_eq!(flora.view().ledger.light_in, 0.0);
    assert_closed(&flora);
}

#[test]
fn at_noon_it_earns_what_it_did_with_the_clock_off() {
    let mut noon = world_on(clock(12.0));
    assert_eq!(noon.life_daylight(), 1.0);
    let mut off = world_on(DayClock {
        mode: DayMode::Off,
        ..DayClock::default()
    });
    let (a, fa) = fixed_after_one_tick(&mut noon);
    let (b, fb) = fixed_after_one_tick(&mut off);
    assert!(a > 0.0, "the fixture earns something: {a}");
    assert_eq!(a, b, "off is full daylight");
    assert_closed(&fa);
    assert_closed(&fb);
}

#[test]
fn at_half_daylight_it_earns_half() {
    // Sunrise itself: elevation zero, the twilight ramp's midpoint.
    let d = DayClock::default();
    let mut dim = world_on(clock(24.0 * (0.5 - d.daylight / 2.0)));
    assert!(
        (dim.life_daylight() - 0.5).abs() < 1e-6,
        "{}",
        dim.life_daylight()
    );
    let mut noon = world_on(clock(12.0));
    let (half, flora) = fixed_after_one_tick(&mut dim);
    let (full, _) = fixed_after_one_tick(&mut noon);
    assert!(
        (half - 0.5 * full).abs() < 1e-6 * full,
        "half {half}, full {full}"
    );
    assert_closed(&flora);
}

#[test]
fn off_means_income_at_any_hour() {
    let mut a = world_on(DayClock {
        mode: DayMode::Off,
        start_hour: 0.0,
        ..DayClock::default()
    });
    let mut b = world_on(DayClock {
        mode: DayMode::Off,
        start_hour: 12.0,
        ..DayClock::default()
    });
    assert_eq!(
        fixed_after_one_tick(&mut a).0,
        fixed_after_one_tick(&mut b).0
    );
}

#[test]
fn maintenance_goes_on_at_night() {
    let mut w = world_on(clock(0.0));
    let (_, mut flora) = fixed_after_one_tick(&mut w);
    let before = flora.view().stands[0].reserve + flora.view().stands[0].wood;
    for _ in 0..20 {
        flora.step(&mut w);
    }
    let v = flora.view();
    let after = v.stands.first().map_or(0.0, |s| s.reserve + s.wood);
    assert!(after < before, "no income, upkeep paid: {before} -> {after}");
    assert_closed(&flora);
}

/// A bloomcrown below `donor_min` (so no propagule draws on its reserve), browsed below
/// its reflush line, on the day clock `day`: its stocks before and after one tick.
fn browsed_tick(world: &mut World) -> ((f64, f64), (f64, f64), f64) {
    let mut flora = Flora::new(FloraConfig::default());
    let sc = flora.config().species(Species::Bloomcrown).clone();
    let wood = 0.5 * sc.donor_min;
    assert!(flora.apply(
        world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood,
        }
    ));
    let site = flora.view().stands[0].site;
    let s = flora.view().stands[0];
    // Browse it to a tenth of its cap, well under `reflush_below`.
    let want = s.foliage - 0.1 * sc.alpha * s.wood;
    assert!(flora.take_foliage(site, want).is_some());
    let s = flora.view().stands[0];
    assert!(s.foliage < sc.reflush_below * sc.alpha * s.wood);
    let before = (s.foliage, s.reserve);
    flora.step(world);
    let s = flora.view().stands[0];
    assert_closed(&flora);
    (before, (s.foliage, s.reserve), sc.maintenance * s.wood)
}

/// Reflushing a browsed stand out of its reserve builds leaves to earn with. At night
/// they earn nothing and senesce before dawn, and the reserve they cost is the one the
/// night's maintenance is paid from: a stand browsed low at dusk reflushed its reserve
/// to nothing and starved before sunrise. At midnight the reserve pays maintenance only.
#[test]
fn a_browsed_stand_does_not_reflush_its_reserve_in_the_dark() {
    let mut w = world_on(clock(0.0));
    assert_eq!(w.life_daylight(), 0.0);
    let ((p0, q0), (p1, q1), m) = browsed_tick(&mut w);
    assert!(p1 <= p0, "no leaves built at midnight: foliage {p0} -> {p1}");
    // Maintenance at most (the night is cooler than the mean, so the factor is <= ~1.5).
    let dt = cubarium_voxel_flora::DT;
    assert!(
        q0 - q1 <= 1.5 * m * dt,
        "the reserve pays upkeep only: {q0} -> {q1}, upkeep {}",
        m * dt
    );
}

/// The same browsed stand at noon does reflush: the gate is the day, not the rule.
#[test]
fn the_same_browsed_stand_reflushes_at_noon() {
    let mut w = world_on(clock(12.0));
    assert_eq!(w.life_daylight(), 1.0);
    let ((p0, _), (p1, _), _) = browsed_tick(&mut w);
    assert!(p1 > p0, "leaves rebuilt at noon: foliage {p0} -> {p1}");
}
