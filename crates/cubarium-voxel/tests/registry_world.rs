//! The world's parameter registry (package R1): coverage, get/set round trip, ranges, and
//! the refusals the hand-written `Config` / `DayClock` / `Climate` validators made, now
//! driven by the table and the cross-field rules.

#[path = "support/registry_spec.rs"]
mod spec;

use cubarium_voxel::registry;
use cubarium_voxel::{Config, Landform, Recipe, Terrarium};
use cubarium_voxel::weather::{DayClock, WeatherSpeed};

fn populated() -> Config {
    // The optional rows: a local clock's place, a fixed weather speed.
    let mut c = Config::default();
    c.day.latitude = Some(47.6);
    c.day.longitude = Some(-122.3);
    c.climate.weather_speed = WeatherSpeed::Fixed(5.0);
    c
}

/// The terrain generators' variants: each holds numbers the default `ridge` has none of.
fn staged() -> Config {
    Config { landform: Landform::Staged(Recipe::SMALL), ..Config::default() }
}

fn terrarium() -> Config {
    Config { landform: Landform::Terrarium(Terrarium::DESK), ..Config::default() }
}

#[test]
fn every_world_number_has_a_row_and_every_row_a_number() {
    spec::coverage(
        registry::world(),
        &[&Config::default(), &populated(), &staged(), &terrarium()],
    );
}

/// The landform rows are all optional (the default landform has none of their fields), so
/// the generic coverage cannot tell a stale one: every row under `world.landform` must
/// match a leaf of the staged recipe or the terrarium.
#[test]
fn every_landform_row_matches_a_field_of_a_variant() {
    let reg = registry::world();
    let mut leaves = Vec::new();
    for cfg in [staged(), terrarium()] {
        let tree = registry::to_value(&cfg).expect("toml");
        leaves.extend(reg.numeric_leaves(&tree).into_iter().map(|(p, _)| p));
    }
    let stale: Vec<&str> = reg
        .rows()
        .iter()
        .filter(|r| r.path.starts_with("world.landform."))
        .filter(|r| !leaves.iter().any(|p| r.matches(p)))
        .map(|r| r.path.as_str())
        .collect();
    assert!(stale.is_empty(), "landform rows that match no field: {stale:?}");
}

#[test]
fn get_and_set_round_trip_on_every_world_row() {
    for cfg in [Config::default(), populated(), staged(), terrarium()] {
        spec::round_trip(registry::world(), &cfg);
    }
}

#[test]
fn a_world_value_outside_its_range_is_refused_naming_path_and_range() {
    for cfg in [Config::default(), populated(), staged(), terrarium()] {
        spec::range_refusals(registry::world(), &cfg);
    }
}

/// A TOML `[world.landform]` table replaces the variant whole, and the numbers it carries
/// are range-checked against the rows; a `--set` of one number needs its variant in force.
#[test]
fn a_landform_table_replaces_the_variant_and_a_number_sets_inside_it() {
    let reg = registry::world();
    let mut c = Config::default();
    let table: toml::Table = toml::from_str("[landform.terrarium]\nsoil_m = 0.5\n")
        .map(|t: toml::Table| match t.get("landform") {
            Some(toml::Value::Table(l)) => l.clone(),
            _ => panic!("no landform"),
        })
        .map(|l| {
            let mut w = toml::Table::new();
            w.insert("landform".into(), toml::Value::Table(l));
            w
        })
        .expect("toml");
    reg.apply_table(&mut c, &table).expect("a whole landform table");
    assert!(matches!(&c.landform, Landform::Terrarium(t) if t.soil_m == 0.5));

    reg.set(&mut c, "world.landform.terrarium.grove_soil_m", toml::Value::Float(1.0))
        .expect("a number inside the variant in force");
    assert!(matches!(&c.landform, Landform::Terrarium(t) if t.grove_soil_m == 1.0));
    let e = reg
        .set(&mut c, "world.landform.terrarium.soil_m", toml::Value::Float(-1.0))
        .expect_err("a negative soil depth");
    assert!(e.contains("soil_m") && e.contains("[0, inf)"), "{e}");
    let e = reg
        .set(&mut Config::default(), "world.landform.terrarium.soil_m", toml::Value::Float(0.5))
        .expect_err("the ridge has no terrarium numbers");
    assert!(e.contains("unknown parameter"), "{e}");
}

#[test]
fn an_unknown_world_path_is_refused() {
    spec::unknown_paths_refused(registry::world(), &Config::default());
}

#[test]
fn the_defaults_validate() {
    Config::default().validate().expect("the default world");
    populated().validate().expect("a populated world");
}

/// Every refusal the deleted hand validators made, each with the word its message must
/// carry.
#[test]
fn the_old_world_validator_refusals_still_refuse() {
    let refuse = |what: &str, edit: &dyn Fn(&mut Config)| {
        let mut c = Config::default();
        edit(&mut c);
        let e = format!("{:#}", c.validate().expect_err(what));
        assert!(e.contains(what), "{what}: {e}");
    };
    // Config::validate.
    refuse("width", &|c| c.width = 0);
    refuse("height", &|c| c.height = 0);
    refuse("depth", &|c| c.depth = 0);
    refuse("overflows", &|c| {
        c.width = u32::MAX;
        c.height = u32::MAX;
        c.depth = u32::MAX;
    });
    refuse("voxel_m", &|c| c.voxel_m = 0.0);
    refuse("voxel_m", &|c| c.voxel_m = f64::NAN);
    refuse("aquifer_porosity", &|c| c.aquifer_porosity = 0.0);
    for (name, edit) in [
        ("rain_m_per_s", (|c: &mut Config| c.rain_m_per_s = -1.0) as fn(&mut Config)),
        ("evaporation_m_per_s", |c| c.evaporation_m_per_s = f64::INFINITY),
        ("spring_k_m2_per_s", |c| c.spring_k_m2_per_s = -1.0),
        ("outlet_m3_per_s", |c| c.outlet_m3_per_s = -1.0),
        ("free_transfer_cap", |c| c.free_transfer_cap = f64::NAN),
        ("reentry_floor_head_m", |c| c.reentry_floor_head_m = -1.0),
        ("lake_drain_m2_per_s", |c| c.lake_drain_m2_per_s = -1.0),
        ("initial_aquifer_head_m", |c| c.initial_aquifer_head_m = -1.0),
        ("initial_atmosphere_m3", |c| c.initial_atmosphere_m3 = -1.0),
        ("water_substeps", |c| c.water_substeps = 0),
    ] {
        refuse(name, &edit);
    }
    // DayClock::validate.
    refuse("length_min", &|c| c.day.length_min = 0.0);
    refuse("daylight", &|c| c.day.daylight = 0.01);
    refuse("daylight", &|c| c.day.daylight = 0.99);
    refuse("start_hour", &|c| c.day.start_hour = 24.0);
    refuse("start_hour", &|c| c.day.start_hour = -1.0);
    refuse("plant_night_factor", &|c| c.day.plant_night_factor = 1.5);
    refuse("plant_night_factor", &|c| c.day.plant_night_factor = -0.1);
    refuse("plant_night_factor", &|c| c.day.plant_night_factor = f64::NAN);
    refuse("latitude", &|c| c.day.latitude = Some(91.0));
    refuse("longitude", &|c| c.day.longitude = Some(-181.0));
    // Climate::validate.
    refuse("weather_speed", &|c| c.climate.weather_speed = WeatherSpeed::Fixed(0.0));
    refuse("temperature_c", &|c| c.climate.temperature_c = -1.0);
    refuse("strike_mean_s", &|c| c.climate.strike_mean_s = f64::NAN);
    refuse("capacity_share", &|c| c.climate.capacity_share = 0.0);
    refuse("cannot be longer", &|c| {
        c.climate.dry_gap_min_s = 900.0;
        c.climate.dry_gap_max_s = 300.0;
    });
    refuse("dry_gap_min_s", &|c| c.climate.dry_gap_min_s = -1.0);
    refuse("front_min_c", &|c| {
        c.climate.front_min_c = 5.0;
        c.climate.front_max_c = 4.0;
    });
    refuse("strike_min_s", &|c| {
        c.climate.strike_min_s = 70.0;
        c.climate.strike_max_s = 60.0;
    });
    refuse("lambda_max_per_h", &|c| c.climate.shower.lambda_max_per_h = -1.0);
    refuse("rate_r0", &|c| c.climate.downpour.rate_r0 = -1.0);
    refuse("stop_rh", &|c| c.climate.drizzle.stop_rh = -1.0);
    refuse("width", &|c| c.climate.shower.width = 0.0);
    refuse("longest_min", &|c| c.climate.downpour.longest_min = 0.0);
    refuse("g_aft", &|c| c.climate.drizzle.g_aft = f64::NAN);

    // What the old validators allowed still passes: a negative afternoon gain (the
    // drizzle's own), a gap of zero to zero, a fixed gap.
    let ok = |edit: &dyn Fn(&mut Config)| {
        let mut c = Config::default();
        edit(&mut c);
        c.validate().expect("allowed before");
    };
    ok(&|c| c.climate.drizzle.g_aft = -0.8);
    ok(&|c| {
        c.climate.dry_gap_min_s = 0.0;
        c.climate.dry_gap_max_s = 0.0;
    });
    ok(&|c| {
        c.climate.dry_gap_min_s = 600.0;
        c.climate.dry_gap_max_s = 600.0;
    });
    ok(&|c| c.day = DayClock { plant_night_factor: 1.0, ..DayClock::default() });
}
