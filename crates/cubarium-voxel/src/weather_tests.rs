//! The weather model's function tests (WX1, `design/handoffs/voxel-weather-2026-09-24.md`
//! "Tests first"). Tiny worlds, a few hundred ticks at most.

use super::*;
use crate::{Command, Config, World};

const MIN: u64 = TICKS_PER_MIN;

/// A calm climate: no drift, no fronts, no rain unless a test asks for one. Hazards are
/// zero, so nothing starts on its own.
fn calm() -> Climate {
    let mut c = Climate::tropical_jungle();
    c.weather_speed = WeatherSpeed::Fixed(1.0);
    c.drift_sd_c = 0.0;
    c.fronts_per_day = 0.0;
    for m in [&mut c.drizzle, &mut c.shower, &mut c.downpour] {
        m.lambda_max_per_h = 0.0;
        m.g_aft = 0.0;
        m.g_cool = 0.0;
        m.g_front = 0.0;
    }
    c
}

/// A closed 8 x 4 world on a bedrock floor with `aloft` cubic metres in the sky and
/// nothing on the ground, the clock off (full day, no diurnal term). With every drop
/// aloft, RH is `1 / capacity_share` until rain moves some of it down.
fn sky_world(climate: Climate, aloft: f64) -> World {
    World::empty(Config {
        width: 8,
        height: 6,
        depth: 4,
        closed_water_budget: true,
        initial_atmosphere_m3: aloft,
        rain_m_per_s: 1e-2,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        climate,
        ..Config::default()
    })
}

fn residual(w: &World) -> f64 {
    w.view().total_residual().abs()
}

#[test]
fn capacity_halves_with_ten_degrees_of_cooling() {
    let c = Climate::tropical_jungle();
    let warm = c.capacity_m3(100.0, c.temperature_c);
    let cold = c.capacity_m3(100.0, c.temperature_c - 10.0);
    assert!(
        (warm - 0.03 * 100.0).abs() < 1e-12,
        "3 % of the world at the mean"
    );
    let ratio = cold / warm;
    assert!(
        (0.45..0.55).contains(&ratio),
        "10 °C of cooling halves it: {ratio}"
    );
}

#[test]
fn rh_at_the_hard_limit_forces_a_downpour_inside_the_dry_gap() {
    let mut c = calm();
    c.capacity_share = 1.0 / 1.6;
    let mut w = sky_world(c, 1.0);
    w.weather.gap_until = 100 * MIN;
    assert!(w.rh() >= Climate::default().rh_max, "{}", w.rh());
    w.step();
    assert_eq!(w.weather().mode(), RainMode::Downpour);
    assert_eq!(w.weather().counts.forced, 1);
    assert_eq!(w.weather().counts.downpours, 1);
    assert!(w.view().is_raining());
}

#[test]
fn a_certain_hazard_starts_at_the_next_minute_and_the_gap_blocks_it() {
    let mut c = calm();
    c.capacity_share = 1.0 / 1.2;
    c.shower.lambda_max_per_h = f64::INFINITY;
    c.dry_gap_min_s = 300.0;
    c.dry_gap_max_s = 900.0;
    let mut w = sky_world(c, 1.0);
    // Tick 0 is a minute boundary and nothing has rained yet: no gap.
    w.step();
    assert_eq!(w.weather().mode(), RainMode::Shower, "starts at the minute");
    assert_eq!(w.weather().counts.showers, 1);

    // Inside a gap the minute's draw still happens, counts, and starts nothing.
    let mut w = sky_world(w.config().climate.clone(), 1.0);
    w.weather.gap_until = 10 * MIN;
    w.step();
    assert_eq!(w.weather().mode(), RainMode::Clear);
    assert_eq!(w.weather().counts.gap_blocked, [0, 1, 0]);
    // Not again until the next minute.
    for _ in 1..MIN {
        w.step();
    }
    assert_eq!(w.weather().counts.gap_blocked, [0, 1, 0]);
    w.step();
    assert_eq!(w.weather().counts.gap_blocked, [0, 2, 0]);
    assert_eq!(w.weather().counts.showers, 0);
}

#[test]
fn an_event_stops_at_its_stop_rh() {
    let mut c = calm();
    c.capacity_share = 1.0 / 0.75;
    c.shower.lambda_max_per_h = f64::INFINITY;
    c.shower.stop_rh = 0.7;
    c.shower.longest_min = 60.0;
    let mut w = sky_world(c, 1.0);
    w.step();
    assert_eq!(w.weather().mode(), RainMode::Shower);
    let mut ticks = 1;
    while w.weather().mode() != RainMode::Clear && ticks < 400 {
        w.step();
        ticks += 1;
    }
    assert_eq!(
        w.weather().mode(),
        RainMode::Clear,
        "stopped within {ticks} ticks"
    );
    let rh = w.rh();
    assert!(
        rh <= 0.7 && rh > 0.6,
        "stopped at the stop RH, not past it: {rh}"
    );
    let last = w.weather().last_finished().expect("an event finished");
    assert_eq!(last.mode, RainMode::Shower);
    assert!(last.volume_m3 > 0.0);
}

#[test]
fn an_event_stops_at_its_longest_duration() {
    let mut c = calm();
    c.capacity_share = 1.0 / 1.2;
    c.drizzle.lambda_max_per_h = f64::INFINITY;
    c.drizzle.stop_rh = 0.0;
    c.drizzle.rate_r0 = 1e-4;
    c.drizzle.longest_min = 3.0 / 60.0; // three seconds: 60 ticks
    let mut w = sky_world(c, 1.0);
    for _ in 0..60 {
        w.step();
        assert_eq!(w.weather().mode(), RainMode::Drizzle);
    }
    w.step();
    assert_eq!(w.weather().mode(), RainMode::Clear);
    let last = w.weather().last_finished().expect("an event finished");
    assert_eq!(last.end_tick - last.start_tick, 60);
}

#[test]
fn lightning_strikes_only_in_a_downpour_and_the_last_strike_persists() {
    // A shower: no strikes however long it runs.
    let mut c = calm();
    c.capacity_share = 1.0 / 1.2;
    c.shower.lambda_max_per_h = f64::INFINITY;
    c.shower.stop_rh = 0.0;
    c.strike_min_s = 0.05;
    c.strike_max_s = 0.05;
    let mut w = sky_world(c.clone(), 1.0);
    for _ in 0..40 {
        w.step();
    }
    assert_eq!(w.weather().mode(), RainMode::Shower);
    assert_eq!(w.weather().counts.strikes, 0);
    assert!(w.view().weather.last_strike.is_none());

    // A forced downpour: strikes, and the view keeps the last one after it ends.
    c.capacity_share = 1.0 / 1.6;
    c.downpour.longest_min = 1.0;
    let mut w = sky_world(c, 1.0);
    for _ in 0..40 {
        w.step();
    }
    assert_eq!(w.weather().mode(), RainMode::Downpour);
    assert!(w.weather().counts.strikes > 0);
    while w.weather().mode() == RainMode::Downpour {
        w.step();
    }
    let strikes = w.weather().counts.strikes;
    let strike = w.view().weather.last_strike.expect("a strike");
    assert!(strike.x < 8 && strike.z < 4);
    for _ in 0..40 {
        w.step();
    }
    assert_eq!(
        w.weather().counts.strikes,
        strikes,
        "none outside the downpour"
    );
    assert_eq!(
        w.view().weather.last_strike,
        Some(strike),
        "the last one persists"
    );
}

#[test]
fn a_front_raises_rh_at_a_fixed_store() {
    let mut c = calm();
    c.capacity_share = 1.0 / 0.6;
    c.front_onset_min = 0.05;
    let mut w = sky_world(c, 1.0);
    w.step();
    let (store, rh0, t0) = (w.atmosphere_m3(), w.rh(), w.weather().temperature_c());
    w.weather.front = Some(Front {
        start_tick: w.tick(),
        depth_c: 3.0,
    });
    for _ in 0..100 {
        w.step();
    }
    assert_eq!(w.atmosphere_m3(), store, "nothing rained or evaporated");
    assert!(
        w.weather().temperature_c() < t0 - 2.5,
        "{}",
        w.weather().temperature_c()
    );
    assert!(w.rh() > rh0 * 1.15, "{} against {rh0}", w.rh());
}

#[test]
fn evaporation_is_zero_at_saturation() {
    // A pond under a sky at `rh`, capacity share 0.5: RH = aloft / (0.5 * total).
    let pond = |rh: f64| {
        let mut c = calm();
        c.capacity_share = 0.5;
        let mut w = World::empty(Config {
            width: 8,
            height: 6,
            depth: 4,
            closed_water_budget: true,
            evaporation_m_per_s: 1e-4,
            day: DayClock {
                mode: DayMode::Off,
                ..DayClock::default()
            },
            climate: c,
            ..Config::default()
        });
        for x in 0..8 {
            for z in 0..4 {
                w.apply(Command::AddWater {
                    x,
                    y: 1,
                    z,
                    volume_m3: 0.01,
                });
            }
        }
        // aloft / (ground + aloft) = rh / 2.
        let ground = w.view().stored_m3();
        let aloft = ground * (rh / 2.0) / (1.0 - rh / 2.0);
        w.apply(Command::AddAtmosphere { volume_m3: aloft });
        assert!((w.rh() - rh).abs() < 1e-9, "{}", w.rh());
        w.step();
        w.view().ledger.evaporation_out
    };
    assert!(pond(0.5) > 0.0, "an unsaturated sky lifts water");
    assert!(pond(0.9) < pond(0.5), "less as it fills");
    assert!(pond(1.0) < pond(0.5) * 1e-9, "none at saturation");
    assert_eq!(pond(1.2), 0.0, "none past it");
    assert_eq!(
        evaporation_scale(&Climate::tropical_jungle(), 1.0, 26.0),
        0.0
    );
    assert_eq!(
        evaporation_scale(&Climate::tropical_jungle(), 1.3, 26.0),
        0.0
    );
    assert!((evaporation_scale(&Climate::tropical_jungle(), 0.6, 26.0) - 1.0).abs() < 1e-12);
}

#[test]
fn the_diurnal_low_is_at_sunrise() {
    let (rise, set) = (0.165, 0.835);
    let mut low = (f64::INFINITY, 0.0);
    let mut high = (f64::NEG_INFINITY, 0.0);
    for i in 0..10_000 {
        let p = i as f64 / 10_000.0;
        let d = diurnal_shape(p, rise, set, DayClock::default().temperature_peak_fraction);
        if d < low.0 {
            low = (d, p);
        }
        if d > high.0 {
            high = (d, p);
        }
    }
    assert!(
        (low.1 - rise).abs() < 1e-3,
        "coolest at sunrise, not {}",
        low.1
    );
    let peak = rise + (set - rise) * 2.0 / 3.0;
    assert!(
        (high.1 - peak).abs() < 1e-3,
        "warmest two thirds into the day, not {}",
        high.1
    );
    assert!((low.0 + 1.0).abs() < 1e-6 && (high.0 - 1.0).abs() < 1e-6);
}

#[test]
fn a_cycle_clock_has_its_sunrise_noon_and_midnight() {
    let clock = DayClock {
        mode: DayMode::Cycle,
        length_min: 120.0,
        daylight: 0.67,
        start_hour: 8.0,
        ..DayClock::default()
    };
    assert!(
        (clock.cycle_phase(0) - 8.0 / 24.0).abs() < 1e-12,
        "starts at 08:00"
    );
    // Half a cycle later is 20:00.
    assert!((clock.cycle_phase(60 * MIN) - 20.0 / 24.0).abs() < 1e-9);
    let (rise, set) = clock.cycle_rise_set();
    assert!((rise - 0.165).abs() < 1e-12 && (set - 0.835).abs() < 1e-12);
    let noon = Sun::at(&DayClock::default(), 0.5, rise, set);
    assert!((noon.elevation - 70f64.to_radians()).abs() < 1e-9);
    assert!((noon.daylight - 1.0).abs() < 1e-12);
    let dawn = Sun::at(&DayClock::default(), rise, rise, set);
    assert!(dawn.elevation.abs() < 1e-9);
    assert!((dawn.daylight - 0.5).abs() < 1e-9, "twilight is half light");
    let midnight = Sun::at(&DayClock::default(), 0.0, rise, set);
    assert!(midnight.elevation < 0.0);
    assert_eq!(midnight.daylight, 0.0);
}

#[test]
fn an_off_clock_lights_the_simulation_and_shows_the_look() {
    let mut w = World::empty(Config {
        width: 4,
        height: 4,
        depth: 2,
        day: DayClock {
            mode: DayMode::Off,
            look: Look::Night,
            ..DayClock::default()
        },
        ..Config::default()
    });
    w.step();
    assert_eq!(w.life_daylight(), 1.0, "the simulation is always lit");
    assert_eq!(w.day_phase(), 0.5);
    let v = w.view().weather;
    assert_eq!(v.daylight, 0.0, "the picture is the night look");
    assert_eq!(v.day_phase, 0.0);
    assert!(v.sun_elevation < 0.0);
}

#[test]
fn a_local_clock_reads_the_injected_wall_time() {
    // 2026-03-20 12:00 UTC, the March equinox: on the prime meridian solar noon is within
    // the equation of time (~7.5 min) of 12:00, and at 90° W it is six hours later.
    let t = 1_774_008_000.0; // 2026-03-20T12:00:00Z
    let clock = |lon: f64| DayClock {
        mode: DayMode::Local,
        latitude: Some(0.0),
        longitude: Some(lon),
        ..DayClock::default()
    };
    let (phase, rise, set) = clock(0.0).local_phase(t);
    assert!((phase - 0.5).abs() < 0.01, "{phase}");
    assert!(
        ((set - rise) - 0.5).abs() < 0.02,
        "an equinox day is half the day: {rise}..{set}"
    );
    let (phase, ..) = clock(-90.0).local_phase(t);
    assert!((phase - 0.25).abs() < 0.01, "{phase}");

    // Without a place, 06:00 and 18:00 on the local wall clock.
    let (phase, rise, set) = DayClock::default().civil_phase(t, 3600.0);
    assert!((phase - 13.0 / 24.0).abs() < 1e-9);
    assert_eq!((rise, set), (0.25, 0.75));

    // A world in local mode takes its phase from the injected clock.
    let mut w = World::empty(Config {
        width: 4,
        height: 4,
        depth: 2,
        day: clock(-90.0),
        ..Config::default()
    });
    w.set_wall_clock(Some(t));
    w.step();
    assert!((w.day_phase() - 0.25).abs() < 0.01, "{}", w.day_phase());
}

#[test]
fn the_ledger_holds_through_a_forced_storm() {
    let mut c = calm();
    c.capacity_share = 0.05;
    let mut w = World::empty(Config {
        width: 8,
        height: 6,
        depth: 4,
        closed_water_budget: true,
        evaporation_m_per_s: 1e-4,
        rain_m_per_s: 1e-2,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        climate: c,
        ..Config::default()
    });
    w.apply(Command::AddWater {
        x: 2,
        y: 1,
        z: 1,
        volume_m3: 0.01,
    });
    w.apply(Command::AddAtmosphere { volume_m3: 0.05 });
    assert!(w.rh() >= Climate::default().rh_max);
    for _ in 0..200 {
        w.step();
        assert!(residual(&w) < 1e-12, "{}", residual(&w));
    }
    assert!(w.weather().counts.forced >= 1);
    assert!(w.view().ledger.rain_in > 0.0);
}

#[test]
fn the_tropical_jungle_parses_from_a_toml_with_one_override() {
    let c: Config = toml::from_str(
        "[climate]\npreset = \"tropical_jungle\"\ntemperature_c = 27.0\n\n[climate.downpour]\nrate_r0 = 1.2\n",
    )
    .expect("a climate table");
    let preset = Climate::tropical_jungle();
    assert_eq!(c.climate.temperature_c, 27.0);
    assert_eq!(c.climate.downpour.rate_r0, 1.2);
    assert_eq!(
        c.climate.downpour.rh50, preset.downpour.rh50,
        "the rest is the preset"
    );
    assert_eq!(c.climate.diurnal_c, preset.diurnal_c);
    assert_eq!(c.climate.dry_gap_max_s, preset.dry_gap_max_s);

    let err = toml::from_str::<Config>("[climate]\npreset = \"tundra\"\n").unwrap_err();
    assert!(format!("{err}").contains("tropical_jungle"), "{err}");

    // A snapshot carries the climate whole.
    let bytes = postcard::to_stdvec(&c).expect("serializes");
    let back: Config = postcard::from_bytes(&bytes).expect("round trips");
    assert_eq!(back.climate, c.climate);
}

#[test]
fn auto_speed_is_nature_on_a_24_hour_day_and_five_on_a_2_hour_one() {
    let clock = |mode, length_min| DayClock {
        mode,
        length_min,
        ..DayClock::default()
    };
    let auto = WeatherSpeed::Auto;
    // The test's own exponent: the one that makes a 2-hour day five times nature's pace.
    let e = 5f64.ln() / 12f64.ln();
    assert!((auto.factor(&clock(DayMode::Cycle, 1440.0), e) - 1.0).abs() < 1e-12);
    let two_hours = auto.factor(&clock(DayMode::Cycle, 120.0), e);
    assert!((two_hours - 5.0).abs() < 1e-9, "{two_hours}");
    assert_eq!(auto.factor(&clock(DayMode::Local, 120.0), e), 1.0, "local is nature");
    assert_eq!(
        auto.factor(&clock(DayMode::Off, 120.0), e),
        two_hours,
        "off counts length_min"
    );
    assert_eq!(WeatherSpeed::Fixed(2.0).factor(&clock(DayMode::Local, 120.0), e), 2.0);

    // The longest durations shrink by it, the hazards grow by it, the gap does not move.
    let nature = Climate {
        weather_speed: WeatherSpeed::Auto,
        auto_speed_exponent: e,
        ..Climate::tropical_jungle()
    };
    let fast = nature.at_speed(&clock(DayMode::Cycle, 120.0));
    for (n, f) in [
        (&nature.drizzle, &fast.drizzle),
        (&nature.shower, &fast.shower),
        (&nature.downpour, &fast.downpour),
    ] {
        assert!((f.longest_min * two_hours - n.longest_min).abs() < 1e-9);
        assert!((f.lambda_max_per_h - n.lambda_max_per_h * two_hours).abs() < 1e-9);
        assert_eq!(f.rate_r0, n.rate_r0);
    }
    assert_eq!(fast.dry_gap_max_s, nature.dry_gap_max_s);
    assert_eq!(fast.strike_mean_s, nature.strike_mean_s);

    // A snapshot keeps `auto` as auto, and a TOML may say either.
    let c: crate::Config = toml::from_str("[climate]\nweather_speed = \"auto\"\n").unwrap();
    assert_eq!(c.climate.weather_speed, WeatherSpeed::Auto);
    let c: crate::Config = toml::from_str("[climate]\nweather_speed = 3\n").unwrap();
    assert_eq!(c.climate.weather_speed, WeatherSpeed::Fixed(3.0));
    let back: crate::Config = postcard::from_bytes(&postcard::to_stdvec(&c).unwrap()).unwrap();
    assert_eq!(back.climate.weather_speed, WeatherSpeed::Fixed(3.0));
    let auto_cfg = crate::Config::default();
    let back: crate::Config =
        postcard::from_bytes(&postcard::to_stdvec(&auto_cfg).unwrap()).unwrap();
    assert_eq!(back.climate.weather_speed, WeatherSpeed::Auto);
}

#[test]
fn the_stream_swells_in_a_downpour_and_is_gone_two_and_a_half_minutes_after() {
    let mut c = calm();
    c.capacity_share = 1.0;
    c.storm_stream_gain = 2.0;
    // The downpour stops with the sky well above the stream's floor (RH 0.5), so the
    // stream can still pay for the swell after it.
    c.downpour.stop_rh = 0.8;
    let mut w = World::empty(Config {
        width: 8,
        height: 8,
        depth: 2,
        closed_water_budget: true,
        initial_atmosphere_m3: 0.5,
        rain_m_per_s: 1e-2,
        reentry_m3_per_s: 0.002,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        climate: c,
        ..Config::default()
    });
    for x in 0..8 {
        for z in 0..2 {
            w.apply(Command::SetMaterial {
                x,
                y: 1,
                z,
                material: crate::Material::Soil,
            });
        }
    }
    w.set_spring_cell(Some((3, 2, 0)));
    w.step();
    let calm_flow = w.weather().stream_m3_s();
    assert!((calm_flow - 0.002).abs() < 1e-12, "{calm_flow}");

    assert!(w.force_rain(RainMode::Downpour));
    let mut peak = 0.0f64;
    while w.weather().mode() == RainMode::Downpour {
        w.step();
        peak = peak.max(w.weather().swell());
        assert!(w.view().total_residual().abs() < 1e-12);
    }
    assert!(peak > 0.1, "the swell rose: {peak}");
    assert!(
        w.weather().stream_m3_s() > calm_flow * 1.1,
        "and the stream with it"
    );
    for _ in 0..150 * crate::TICK_HZ {
        w.step();
    }
    assert!(
        w.weather().swell() <= 0.05 * peak,
        "{} of {peak}",
        w.weather().swell()
    );
    assert!(w.view().total_residual().abs() < 1e-12);
}

/// WX1b: showers and downpours start and stop on the moisture `M` (the store against the
/// capacity at the **mean** temperature); drizzle keeps RH. At the afternoon peak of a
/// ±5 °C day the same store reads M 1.0 and RH 0.70.
#[test]
fn convective_rain_reads_moisture_and_drizzle_reads_rh() {
    let at_peak = |mut climate: Climate| {
        climate.diurnal_c = 5.0;
        climate.capacity_share = 1.0; // every drop aloft: M = 1
        let day = DayClock::default();
        let (rise, set) = day.cycle_rise_set();
        World::empty(Config {
            width: 8,
            height: 6,
            depth: 4,
            closed_water_budget: true,
            initial_atmosphere_m3: 1.0,
            rain_m_per_s: 1e-2,
            day: DayClock {
                start_hour: 24.0 * (rise + (set - rise) * 2.0 / 3.0),
                ..day
            },
            climate,
            ..Config::default()
        })
    };
    let mut c = calm();
    c.shower.lambda_max_per_h = f64::INFINITY;
    c.shower.rh50 = 0.9;
    c.shower.width = 0.01;
    c.shower.stop_rh = 0.8;
    c.shower.longest_min = 60.0;
    let mut w = at_peak(c);
    assert!((w.moisture() - 1.0).abs() < 1e-9, "M {}", w.moisture());
    assert!(w.rh() < 0.72, "the warm afternoon reads RH {}", w.rh());
    w.step();
    assert_eq!(w.weather().mode(), RainMode::Shower, "M over rh50 starts it");
    let start = w.weather().event().copied().expect("a shower");
    assert!((start.moisture - 1.0).abs() < 1e-6 && start.rh < 0.72);
    let mut ticks = 1;
    while w.weather().mode() != RainMode::Clear && ticks < 2000 {
        w.step();
        ticks += 1;
    }
    let m = w.moisture();
    assert!(ticks > 1, "RH under the stop value does not end it");
    assert!(m <= 0.8 && m > 0.7, "it stopped at M 0.8: M {m}, RH {}", w.rh());
    assert!(residual(&w) < 1e-12);

    // Drizzle at the same store and the same gate stays dry: RH is 0.70.
    let mut c = calm();
    c.drizzle.lambda_max_per_h = f64::INFINITY;
    c.drizzle.rh50 = 0.9;
    c.drizzle.width = 0.01;
    c.drizzle.stop_rh = 0.8;
    let mut w = at_peak(c);
    for _ in 0..2 * MIN + 1 {
        w.step();
    }
    assert_eq!(w.weather().counts.drizzles, 0, "RH 0.70 is under the stop value");
    // With the clock off RH is M, 1.0, and the same drizzle starts at once.
    let mut w = sky_world(c, 1.0);
    w.config.climate.capacity_share = 1.0;
    w.step();
    assert_eq!(w.weather().mode(), RainMode::Drizzle);
}

/// WX1b: the atmosphere-fed stream draws only while the sky stands above RH 0.5, and
/// never takes it below.
#[test]
fn the_atmosphere_fed_stream_stops_at_rh_one_half() {
    let mut c = calm();
    c.capacity_share = 1.0 / 0.6; // every drop aloft: RH 0.6
    let mut w = World::empty(Config {
        width: 8,
        height: 8,
        depth: 2,
        closed_water_budget: true,
        initial_atmosphere_m3: 1.0,
        rain_m_per_s: 1e-2,
        reentry_m3_per_s: 0.01,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        climate: c,
        ..Config::default()
    });
    for x in 0..8 {
        for z in 0..2 {
            w.apply(Command::SetMaterial {
                x,
                y: 1,
                z,
                material: crate::Material::Soil,
            });
        }
    }
    w.set_spring_cell(Some((3, 2, 0)));
    assert!((w.rh() - 0.6).abs() < 1e-9, "{}", w.rh());
    assert!((w.stream_floor_m3() - 0.5 / 0.6).abs() < 1e-9);
    w.step();
    assert!(w.weather().stream_m3_s() > 0.0, "above the floor it runs");
    // 0.1 of the capacity above the floor at 0.01 m³/s: gone in well under a minute.
    for _ in 0..60 * crate::TICK_HZ {
        w.step();
        assert!(w.rh() >= 0.5 - 1e-9, "never below the floor: {}", w.rh());
    }
    assert!(w.rh() < 0.5 + 1e-6, "it drew down to it: {}", w.rh());
    assert_eq!(w.weather().stream_m3_s(), 0.0, "and stopped");
    assert!(residual(&w) < 1e-12);
}

/// Rain falls from cloud: a shower on a dry afternoon sky (RH ~0.3, which alone reads
/// clear) still shows at least 0.75 cover; drizzle 0.6, a downpour 0.9, clear sky none.
#[test]
fn rain_falls_from_at_least_its_modes_cloud() {
    let mut c = calm();
    c.capacity_share = 1.0 / 0.3; // every drop aloft: RH 0.3, cover 0 by the RH rule
    for m in [&mut c.drizzle, &mut c.shower, &mut c.downpour] {
        m.stop_rh = 0.0; // falls on however dry the sky
    }
    for (mode, floor) in [
        (RainMode::Drizzle, 0.6),
        (RainMode::Shower, 0.75),
        (RainMode::Downpour, 0.9),
    ] {
        let mut w = sky_world(c, 1.0);
        assert_eq!(w.view().weather.cloud_cover, 0.0, "a dry sky is clear");
        assert!(w.force_rain(mode));
        w.step();
        assert_eq!(w.weather().mode(), mode);
        let cover = w.view().weather.cloud_cover;
        assert!((cover - floor).abs() < 1e-6, "{mode:?}: {cover}");
    }
}
