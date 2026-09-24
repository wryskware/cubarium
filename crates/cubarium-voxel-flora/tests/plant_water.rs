//! Package F (plant-water, `design/handoffs/voxel-plant-viability-2026-09-23.md`): plants
//! measure the water the ground **offers**, not the fraction of its pores that are full.
//!
//! Every pore fraction here is written through the material's own `wilting_point` and
//! `field_capacity`, never as a literal, so the tests hold whatever numbers the
//! soil-retention package settles on.
//!
//! `voxel_m` is 1 m, so one soil voxel holds `pore_capacity` cubic metres of pore water.
//! The fixtures wet the soil exactly: free water into an air cell, then the cell turned
//! to soil (the same construction `tests/flora.rs` uses).

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Flora, FloraConfig, Site, Species, available_water, establishment_gates,
};

/// A strip one slab deep: bedrock floor, soil at `y = 1..=2` holding `pore` of its
/// capacity, air above. Every column's support face is `y = 2`, in open sky.
fn plain(width: u32, pore: f64) -> World {
    let config = VoxelConfig {
        width,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 7,
        ..VoxelConfig::default()
    };
    let mut w = World::empty(config);
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    for x in 0..width as i64 {
        for y in 1..=2u32 {
            if pore > 0.0 {
                let got = w.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z: 0,
                    volume_m3: pore * cap,
                });
                assert!((got - pore * cap).abs() < 1e-12, "the void took {got}");
            }
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

/// The soil pore fraction at which the available water is `a`.
fn soil_pore_at(a: f64) -> f64 {
    let (wp, fc) = (
        Material::Soil.wilting_point(),
        Material::Soil.field_capacity(),
    );
    wp + a * (fc - wp)
}

fn site(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

/// Seed one stand of `species` at its adult size on column `x` and step one tick: the
/// stand's `moisture` is then the `μ` the tick read, before anything was drunk.
fn moisture_after_one_tick(world: &mut World, flora: &mut Flora, x: u32, species: Species) -> f64 {
    let wood = flora.config().species(species).wood_max;
    assert!(flora.apply(
        world,
        Command::Seed {
            x: i64::from(x),
            z: 0,
            species,
            wood,
        }
    ));
    flora.step(world);
    flora
        .view()
        .stand_at(site(x))
        .expect("still standing")
        .moisture
}

// ------------------------------------------------------------------ 1. the scale

/// `a = (p − wp)/(fc − wp)`, floored at zero: zero at the wilting point, one at field
/// capacity, more than one in wetter ground and `(1 − wp)/(fc − wp)` at saturation — for
/// soil and for rock, each on its own numbers. A cell with no pore space offers nothing.
#[test]
fn available_water_is_zero_at_the_wilting_point_one_at_field_capacity_and_more_above() {
    for m in [Material::Soil, Material::Rock] {
        let (wp, fc) = (m.wilting_point(), m.field_capacity());
        assert!(0.0 < wp && wp < fc && fc < 1.0, "{m:?}: wp {wp}, fc {fc}");
        assert!(available_water(m, wp).abs() < 1e-12, "{m:?} at wp");
        assert!((available_water(m, fc) - 1.0).abs() < 1e-12, "{m:?} at fc");
        let wetter = available_water(m, 0.5 * (fc + 1.0));
        assert!(
            wetter > 1.0,
            "{m:?} between fc and saturation reads {wetter}"
        );
        let sat = (1.0 - wp) / (fc - wp);
        assert!(
            (available_water(m, 1.0) - sat).abs() < 1e-12,
            "{m:?} saturated"
        );
        assert_eq!(available_water(m, 0.5 * wp), 0.0, "{m:?}: floored at zero");
        assert_eq!(available_water(m, 0.0), 0.0, "{m:?}: dry");
    }
    for m in [Material::Air, Material::Bedrock] {
        assert_eq!(available_water(m, 0.7), 0.0, "{m:?} holds no pore water");
    }
}

// ------------------------------------------------------------------ 2. the ramp

/// `μ` is zero at the species' wilt threshold and one at its full threshold, on the
/// available-water scale, for every species — and a stand on soil wetted to exactly those
/// two levels reads them.
#[test]
fn moisture_is_zero_at_the_wilt_threshold_and_one_at_the_full_threshold() {
    let config = FloraConfig::default();
    for &s in Species::ALL.iter() {
        let sc = config.species(s);
        assert!(
            sc.wilt_water < sc.full_water,
            "{s:?}: wilt {} full {}",
            sc.wilt_water,
            sc.full_water
        );
        assert_eq!(sc.moisture_at(sc.wilt_water), 0.0, "{s:?} at wilt");
        assert_eq!(sc.moisture_at(sc.full_water), 1.0, "{s:?} at full");
        let mid = sc.moisture_at(0.5 * (sc.wilt_water + sc.full_water));
        assert!((mid - 0.5).abs() < 1e-12, "{s:?}: linear between, {mid}");
    }

    // In the world: a velvetpad (a damp-lover, both thresholds inside drained-to-wet
    // soil) on soil at exactly its two thresholds.
    let sc = config.species(Species::Velvetpad).clone();
    for (a, want) in [(sc.wilt_water, 0.0), (sc.full_water, 1.0)] {
        let mut world = plain(6, soil_pore_at(a));
        let mut flora = Flora::new(FloraConfig::default());
        let mu = moisture_after_one_tick(&mut world, &mut flora, 2, Species::Velvetpad);
        assert!(
            (mu - want).abs() < 1e-9,
            "velvetpad at a = {a}: μ {mu}, want {want}"
        );
    }
}

// ------------------------------------------------------------ 3. upland at capacity

/// Drained ground is not dry ground: an upland stand on soil at field capacity reads full
/// moisture.
#[test]
fn an_upland_stand_on_soil_at_field_capacity_reads_full_moisture() {
    for species in [Species::Bloomcrown, Species::Springturf, Species::Vaulttree] {
        let mut world = plain(8, Material::Soil.field_capacity());
        let mut flora = Flora::new(FloraConfig::default());
        let mu = moisture_after_one_tick(&mut world, &mut flora, 3, species);
        assert!(
            (mu - 1.0).abs() < 1e-12,
            "{species:?} at field capacity: μ {mu}"
        );
    }
}

// ------------------------------------------------------------------ 4. the floor

/// However hard a stand pulls, drinking leaves every soil voxel at or above its wilting
/// point, and a root box it could drain is drained to exactly that point.
#[test]
fn drinking_never_takes_pore_water_below_the_wilting_point() {
    let wp = Material::Soil.wilting_point();
    // Just above the wilting point, with a thirst no box can satisfy: stonecushion wilts
    // only at zero available water, so it keeps drinking all the way down.
    let mut config = FloraConfig::default();
    config.stonecushion.transpiration_m3_per_s = 1.0e3;
    let mut world = plain(7, soil_pore_at(0.05));
    let mut flora = Flora::new(config);
    let wood = flora.config().species(Species::Stonecushion).wood_max;
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 3,
            z: 0,
            species: Species::Stonecushion,
            wood,
        }
    ));
    for _ in 0..10 {
        flora.step(&mut world);
    }
    assert!(
        flora.view().ledger.transpired_m3 > 0.0,
        "the stand drank something"
    );
    let view = world.view();
    for x in 0..7i64 {
        for y in 1..=2u32 {
            let p = view.pore_at(x, y, 0);
            assert!(p >= wp - 1e-12, "({x}, {y}) at pore {p}, under wp {wp}");
            if (2..=4).contains(&x) {
                assert!((p - wp).abs() < 1e-9, "root cell ({x}, {y}) left at {p}");
            } else {
                assert!(
                    (p - soil_pore_at(0.05)).abs() < 1e-12,
                    "({x}, {y}) is outside the root box and was touched: {p}"
                );
            }
        }
    }
}

// ---------------------------------------------------------- 5. wetland at capacity

/// A wetland species needs ground wetter than drained: on soil at field capacity its root
/// box reads available water 1, which is its wilt threshold, and `μ` is zero.
#[test]
fn a_wetland_species_on_soil_at_field_capacity_reads_no_moisture() {
    let config = FloraConfig::default();
    let world = plain(6, Material::Soil.field_capacity());
    for species in [Species::Umbrellafrond, Species::Siphonreed] {
        let sc = config.species(species);
        let g = establishment_gates(&world.view(), site(2), sc);
        let a = g.mean_water.expect("soil");
        assert!((a - 1.0).abs() < 1e-12, "{species:?}: box reads {a}");
        assert_eq!(sc.moisture_at(a), 0.0, "{species:?} at field capacity");
    }
    // And a live umbrellafrond (no standing-water rule on it) reads the same.
    let mut world = plain(6, Material::Soil.field_capacity());
    let mut flora = Flora::new(FloraConfig::default());
    let mu = moisture_after_one_tick(&mut world, &mut flora, 2, Species::Umbrellafrond);
    assert!(mu.abs() < 1e-12, "umbrellafrond at field capacity: μ {mu}");
}

// ------------------------------------------------------- the establishment gate

/// The germination floor is on the same scale: a site whose root box holds just more than
/// `establish_water_min` passes the water gate, and just less does not.
#[test]
fn the_establishment_water_gate_reads_the_available_water_scale() {
    let config = FloraConfig::default();
    for species in [
        Species::Springturf,
        Species::Velvetpad,
        Species::Umbrellafrond,
    ] {
        let sc = config.species(species);
        let min = sc.establish_water_min;
        let wet = plain(6, soil_pore_at(min + 0.01));
        let g = establishment_gates(&wet.view(), site(2), sc);
        assert!(g.pore_ok, "{species:?} just above its floor: {g:?}");
        let dry = plain(6, soil_pore_at(min - 0.01));
        let g = establishment_gates(&dry.view(), site(2), sc);
        assert!(!g.pore_ok, "{species:?} just below its floor: {g:?}");
    }
}

// ------------------------------------------------------------- lanternberry (LB)

/// Package LB: a lanternberry keeps full moisture through the top half of what drained
/// ground offers — the upland ramp — so a root box drying from field capacity to half of
/// it between showers does not cut its income, while it still germinates only on moist
/// ground (a floor above the ramp's bottom).
#[test]
fn a_lanternberry_is_at_full_moisture_on_half_drained_ground_and_germinates_only_on_moist() {
    let config = FloraConfig::default();
    let sc = config.species(Species::Lanternberry);
    assert_eq!(sc.moisture_at(0.5), 1.0, "half of field capacity's water");
    assert!(
        sc.establish_water_min > sc.wilt_water + 0.2,
        "germinates on moist ground only: floor {} over wilt {}",
        sc.establish_water_min,
        sc.wilt_water
    );
    let mut world = plain(6, soil_pore_at(0.5));
    let mut flora = Flora::new(FloraConfig::default());
    let mu = moisture_after_one_tick(&mut world, &mut flora, 2, Species::Lanternberry);
    assert!(
        (mu - 1.0).abs() < 1e-12,
        "a lanternberry at a = 0.5: μ {mu}"
    );
}
