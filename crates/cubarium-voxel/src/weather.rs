//! Weather: the day clock, temperature, the sky's capacity and the rain modes
//! (`design/handoffs/voxel-weather-2026-09-24.md`).
//!
//! **This file is the pinned seam between the simulation and the renderer.** The weather
//! model (package WX1) fills [`WeatherView`] from the world's weather state; the
//! presenter (package WX2) draws from it and from nothing else about the weather. Until
//! WX1 lands, [`WeatherView::legacy`] derives a view from the shower store alone, so the
//! presenter can draw real showers today: a fixed noon, a shower whenever one falls, a
//! cloud cover from how full the store is.

/// What is falling (or hanging) in the sky.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum RainMode {
    /// No rain event.
    #[default]
    Clear,
    Drizzle,
    Shower,
    /// A storm: the heaviest rain, and the only mode with lightning.
    Downpour,
}

/// One lightning strike. The view carries the **most recent** strike, not a per-tick
/// flag, so a presenter that stages every few ticks still sees it and animates its flash
/// and decay from `tick` on its own frame clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Strike {
    /// The tick it struck.
    pub tick: u64,
    /// The column it struck: `x` along the ring, `z` into the depth.
    pub x: u32,
    pub z: u32,
    /// A per-strike draw for the bolt's shape, so every strike looks different and the
    /// same strike looks the same on every frame.
    pub seed: u32,
}

/// Everything the presenter knows about the weather and the time of day. `Copy`, a few
/// dozen bytes, filled once per tick.
///
/// **Visual quantities.** `day_phase`, `daylight` and `sun_elevation` are what to
/// *draw*. With the day clock off (`[world.day] mode = "off"`) they hold the fixed look
/// (noon or midnight) while the simulation's own light stays full day; the simulation
/// reads its light from the world, never from this view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeatherView {
    /// Air temperature, °C. Only differences matter to the model.
    pub temperature_c: f32,
    /// Relative humidity: the atmosphere store over the capacity at this temperature.
    /// Above 1 is supersaturated; the model never lets it pass 1.5.
    pub rh: f32,
    pub mode: RainMode,
    /// The rate rain is falling at now, m/s. Zero exactly when `mode` is `Clear`.
    pub rain_m_per_s: f32,
    /// Ground fog density, 0 (none) to 1 (thick).
    pub fog: f32,
    /// Cloud cover, 0 (clear) to 1 (overcast).
    pub cloud_cover: f32,
    /// Where in the day, `0..1`: 0 midnight, 0.5 noon, wrapping.
    pub day_phase: f32,
    /// How much daylight, 0 (night) to 1 (full day), smooth through twilight.
    pub daylight: f32,
    /// The sun's elevation above the horizon, radians; negative below it.
    pub sun_elevation: f32,
    /// The most recent lightning strike, if the world has had one.
    pub last_strike: Option<Strike>,
}

impl WeatherView {
    /// A clear noon: what a world with no weather state draws.
    pub const CLEAR_NOON: WeatherView = WeatherView {
        temperature_c: 26.0,
        rh: 0.0,
        mode: RainMode::Clear,
        rain_m_per_s: 0.0,
        fog: 0.0,
        cloud_cover: 0.0,
        day_phase: 0.5,
        daylight: 1.0,
        sun_elevation: core::f32::consts::FRAC_PI_2,
        last_strike: None,
    };

    /// The view before the weather model exists: the shower store alone. A shower is
    /// falling at `rain_m_per_s` whenever one is; relative humidity and cloud cover read
    /// the store against 3 % of the world's water (the proposal's default capacity).
    /// Replaced by WX1.
    pub fn legacy(atmosphere_m3: f64, expected_total_m3: f64, raining: bool, rain_m_per_s: f64) -> WeatherView {
        let capacity = 0.03 * expected_total_m3;
        let rh = if capacity > 0.0 { (atmosphere_m3 / capacity) as f32 } else { 0.0 };
        WeatherView {
            rh,
            mode: if raining { RainMode::Shower } else { RainMode::Clear },
            rain_m_per_s: if raining { rain_m_per_s as f32 } else { 0.0 },
            cloud_cover: ((rh - 0.4) / 0.6).clamp(0.0, 1.0),
            ..WeatherView::CLEAR_NOON
        }
    }
}

impl Default for WeatherView {
    fn default() -> WeatherView {
        WeatherView::CLEAR_NOON
    }
}
