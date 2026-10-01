//! The world's parameter registry (package R1): coverage, get/set round trip, ranges, and
//! the refusals the hand-written `Config` / `DayClock` / `Climate` validators made, now
//! driven by the table and the cross-field rules.

#[path = "support/registry_spec.rs"]
mod spec;

use cubarium_voxel::Config;
use cubarium_voxel::registry;
use cubarium_voxel::weather::{DayClock, WeatherSpeed};

fn populated() -> Config {
    // The optional rows: a local clock's place, a fixed weather speed.
    let mut c = Config::default();
    c.day.latitude = Some(47.6);
    c.day.longitude = Some(-122.3);
    c.climate.weather_speed = WeatherSpeed::Fixed(5.0);
    c
}

#[test]
fn every_world_number_has_a_row_and_every_row_a_number() {
    spec::coverage(registry::world(), &[&Config::default(), &populated()]);
}

#[test]
fn get_and_set_round_trip_on_every_world_row() {
    spec::round_trip(registry::world(), &Config::default());
    spec::round_trip(registry::world(), &populated());
}

#[test]
fn a_world_value_outside_its_range_is_refused_naming_path_and_range() {
    spec::range_refusals(registry::world(), &Config::default());
    spec::range_refusals(registry::world(), &populated());
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
