//! **TEST-PASS (independent), DRY-SEASON**: the season's first and last tick on the world's
//! day clock, its independence from the clock's start hour, the held downpour's release on
//! the first tick past the season, and the water books closing through it. Every test sets
//! the knobs it reads; none reads a preset.

use super::*;
use crate::{Config, World};

const MIN: u64 = TICKS_PER_MIN;

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

fn seasonal(every: f64, len: f64, phase: f64, rain: f64) -> Climate {
    let mut c = calm();
    c.dry_season = true;
    c.dry_season_every_days = every;
    c.dry_season_len_days = len;
    c.dry_season_phase_days = phase;
    c.dry_season_rain = rain;
    c
}

fn sky_world(climate: Climate, aloft: f64, length_min: f64) -> World {
    World::empty(Config {
        width: 8,
        height: 6,
        depth: 4,
        closed_water_budget: true,
        initial_atmosphere_m3: aloft,
        rain_m_per_s: 1e-2,
        day: DayClock { mode: DayMode::Off, length_min, ..DayClock::default() },
        climate,
        ..Config::default()
    })
}

/// The tick `days` world days in on `clock`, in integer arithmetic (the day a whole number of
/// seconds, `days` a whole number of quarter-days in the cases).
fn exact_tick(clock: &DayClock, days: f64) -> u64 {
    let day_ticks = (clock.day_s() as u64) * u64::from(TICK_HZ);
    ((days * 4.0).round() as u64) * day_ticks / 4
}

/// **The season opens on its first tick and shuts on its last**: on cycle, off and local
/// clocks, the tick at `phase` days is in the season and the one before is not; the tick at
/// `phase + len` days is out and the one before is in; and the same a period later.
#[test]
fn the_season_opens_and_shuts_on_the_exact_ticks_of_its_knobs() {
    let (every, len, phase) = (3.0, 0.5, 2.0);
    let c = seasonal(every, len, phase, 0.0);
    for clock in [
        DayClock::default(),
        DayClock { length_min: 45.0, ..DayClock::default() },
        DayClock { mode: DayMode::Off, length_min: 300.0, ..DayClock::default() },
        DayClock { mode: DayMode::Local, ..DayClock::default() },
    ] {
        for k in [0.0, 1.0, 7.0] {
            let open = exact_tick(&clock, phase + k * every);
            let shut = exact_tick(&clock, phase + k * every + len);
            assert!(!c.in_dry_season(&clock, open - 1), "{clock:?} period {k}: the tick before the season");
            assert!(c.in_dry_season(&clock, open), "{clock:?} period {k}: its first tick");
            assert!(c.in_dry_season(&clock, shut - 1), "{clock:?} period {k}: its last tick");
            assert!(!c.in_dry_season(&clock, shut), "{clock:?} period {k}: the tick after");
        }
    }
}

/// **World days run from tick 0, not from the clock's hour**: two cycle clocks that start at
/// midnight and at 06:00 put the season on the same ticks.
#[test]
fn the_season_counts_world_days_from_tick_zero_whatever_the_start_hour() {
    let c = seasonal(2.0, 0.25, 1.0, 0.0);
    let a = DayClock { start_hour: 0.0, ..DayClock::default() };
    let b = DayClock { start_hour: 6.0, ..DayClock::default() };
    let span = exact_tick(&a, 6.0);
    let step = (span / 997).max(1);
    let mut t = 0;
    while t < span {
        assert_eq!(c.in_dry_season(&a, t), c.in_dry_season(&b, t), "tick {t}");
        t += step;
    }
}

/// **The held downpour falls on the first tick past the season, and the books close**: a sky
/// at its hard limit through a season forces nothing; the first step past the season's last
/// tick forces the downpour; the water ledger closes on every tick throughout.
#[test]
fn the_held_downpour_falls_on_the_first_tick_past_the_season_and_the_books_close() {
    let mut c = seasonal(1.0, 0.5, 0.0, 0.0);
    c.capacity_share = 1.0 / (c.rh_max + 0.1);
    let mut w = sky_world(c, 1.0, 120.0);
    let shut = exact_tick(&w.config.day, 0.5);
    assert!(w.rh() >= c.rh_max && w.in_dry_season());
    let residual = |w: &World| w.view().total_residual().abs();
    for _ in 0..2 * MIN {
        w.step();
        assert!(residual(&w) < 1e-12, "{}", residual(&w));
    }
    // To the season's last tick: still held.
    w.tick = shut - 1;
    assert!(w.in_dry_season());
    w.step();
    assert_eq!(w.weather().counts.forced, 0, "held through the season's last tick");
    assert!(residual(&w) < 1e-12);
    assert!(!w.in_dry_season() && w.tick == shut);
    w.step();
    assert_eq!(w.weather().counts.forced, 1, "forced on the first tick past the season");
    assert_eq!(w.weather().mode(), RainMode::Downpour);
    for _ in 0..2 * MIN {
        w.step();
        assert!(residual(&w) < 1e-12, "{}", residual(&w));
    }
    assert!(w.view().ledger.rain_in > 0.0, "the stored water fell");
}

/// **A downpour already falling when the season opens is not cut**: the season scales new
/// starts and holds the forced one; an event under way runs on by its own rules.
#[test]
fn an_event_under_way_when_the_season_opens_runs_on() {
    let mut c = seasonal(1.0, 0.5, 0.5, 0.0);
    c.capacity_share = 1.0 / (c.rh_max + 0.1);
    let mut w = sky_world(c, 1.0, 120.0);
    let open = exact_tick(&w.config.day, 0.5);
    w.tick = open - 2;
    assert!(!w.in_dry_season());
    w.step();
    assert_eq!(w.weather().mode(), RainMode::Downpour, "forced before the season");
    for _ in 0..3 {
        w.step();
    }
    assert!(w.in_dry_season());
    assert_eq!(w.weather().mode(), RainMode::Downpour, "still falling inside the season");
}
