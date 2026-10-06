//! Weather: the day clock, temperature, the sky's capacity and the rain modes
//! (`design/handoffs/voxel-weather-2026-09-24.md`).
//!
//! **The top of this file is the pinned seam between the simulation and the renderer.**
//! The weather model (below the seam) fills [`WeatherView`] from the world's weather
//! state; the presenter draws from it and from nothing else about the weather.
//!
//! The model is one lumped air store with a temperature: a day clock
//! ([`DayClock`]), a climate ([`Climate`]) whose temperature sets how much water the air
//! can hold, and rain as **events** of three modes drawn once a simulated minute from
//! hazards weighted by humidity, the afternoon, cooling air and fronts. Every draw comes
//! from the world's weather stream keyed by seed and minute, so a world's weather follows
//! from its seed — except under `[world.day] mode = "local"`, the one place a wall clock
//! enters the simulation.

use serde::{Deserialize, Serialize};

use crate::{DT, TICK_HZ, World};

/// What is falling (or hanging) in the sky.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Cloud cover, 0 (clear) to 1 (overcast): `clamp((RH − 0.4) / 0.6)`, and at least
    /// 0.6 / 0.75 / 0.9 by default while it drizzles / showers / pours ([`cloud_floor`]).
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
        // The model's noon peak, 70° (`DayClock::noon_elevation_deg`).
        sun_elevation: 1.221_730_5,
        last_strike: None,
    };
}

impl Default for WeatherView {
    fn default() -> WeatherView {
        WeatherView::CLEAR_NOON
    }
}

// ================================================================== the model (WX1)

/// Ticks in one simulated minute: the rain draw's cadence.
pub const TICKS_PER_MIN: u64 = 60 * TICK_HZ as u64;
// The Clausius–Clapeyron slope, the forced-downpour humidity, the evaporation span, the
// stream's floor, the swell's time constants and the auto speed's exponent are
// `Climate` fields (`cc_per_c`, `rh_max`, `evaporation_rh_span`, `stream_floor_rh`,
// `swell_rise_s`, `swell_fall_s`, `auto_speed_exponent`); the sun's noon elevation,
// twilight and temperature peak are `DayClock` fields.

/// A lightning strike picks the highest of this many random sky-open columns.
pub const STRIKE_COLUMNS: usize = 4;

/// The weather stream: splitmix64 over the world seed, a key (the minute, or the tick for
/// what happens inside one) and a per-draw tag. The generator's own idiom.
const WEATHER_STREAM: u64 = 0x_5745_4154_4845_525F;
const TAG_DRIFT_A: u64 = 1;
const TAG_DRIFT_B: u64 = 2;
const TAG_FRONT: u64 = 3;
const TAG_FRONT_DEPTH: u64 = 4;
const TAG_RAIN: u64 = 5;
const TAG_GAP: u64 = 6;
const TAG_STRIKE_WAIT: u64 = 7;
const TAG_STRIKE_SEED: u64 = 8;
const TAG_STRIKE_COLUMN: u64 = 16;

/// A uniform draw in `[0, 1)` from the weather stream.
pub(crate) fn draw(seed: u64, key: u64, tag: u64) -> f64 {
    let z = WEATHER_STREAM
        ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ key.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ tag.wrapping_mul(0x1656_67B1_9E37_79F9);
    crate::unit_from_bits(crate::mix(z))
}

/// A standard normal draw (Box–Muller over two stream draws).
fn normal(seed: u64, key: u64) -> f64 {
    let u1 = draw(seed, key, TAG_DRIFT_A).max(1e-300);
    let u2 = draw(seed, key, TAG_DRIFT_B);
    (-2.0 * u1.ln()).sqrt() * (core::f64::consts::TAU * u2).cos()
}

// ------------------------------------------------------------------ the day clock

/// How the day clock runs: `[world.day] mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DayMode {
    /// A day and night of [`DayClock::length_min`] simulated minutes.
    #[default]
    Cycle,
    /// The wall clock, with today's sunrise and sunset at [`DayClock::latitude`] and
    /// [`DayClock::longitude`] (06:00 and 18:00 on the local clock without them).
    Local,
    /// No night in the simulation: plants always lit, animals never sleep, no diurnal
    /// temperature. The picture holds [`DayClock::look`].
    Off,
}

/// The fixed picture of a clock that is off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Look {
    #[default]
    Day,
    Night,
}

/// `[world.day]`: Wrysk's decision 1 (2026-09-24).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DayClock {
    pub mode: DayMode,
    /// Cycle: one day and night, minutes of simulated time. Also the length of a "day"
    /// the climate's per-day rates count in when the clock is off.
    pub length_min: f64,
    /// Cycle: the share of it the sun is up.
    pub daylight: f64,
    /// Cycle: where a fresh world starts, on a 24-hour clock of its day.
    pub start_hour: f64,
    /// Local: the place whose sunrise and sunset the day follows, degrees (north, east).
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// Off only: the picture alone.
    pub look: Look,
    /// WX3 round 4, the plant night rule: while it is dark, plant maintenance and leaf
    /// loss (senescence) run at this fraction of their daytime rate — dark respiration
    /// and a resting leaf, instead of the leftover of the day clock (~12× nature) meeting
    /// the biological one (~3,000×), where one night is months of a plant's upkeep
    /// (`design/7_Research/ecology-parameter-audit-2026-09-28.md` §2). The applied rate is
    /// `rate × (d + (1 − d) · plant_night_factor)` with `d` = `World::life_daylight`, so
    /// twilight is continuous. Default 0.05 (Wrysk, 2026-09-29, from the round-4 arms:
    /// no plant dies in the dark; stands 72–88 % of the pre-night reference); 1 is no rule.
    /// Saprotrophs do not read it.
    pub plant_night_factor: f64,
    /// The sun's elevation at noon, degrees above the horizon. By day the elevation is
    /// a sine that peaks at this at the middle of the daylight.
    pub noon_elevation_deg: f64,
    /// Civil twilight, degrees: daylight is a smoothstep of the elevation over ±this many
    /// degrees about the horizon.
    pub twilight_deg: f64,
    /// Where in the daylight the day's temperature peaks, as a share of it from sunrise
    /// (2/3: two thirds of the way through); the afternoon convection bump is centred
    /// there.
    pub temperature_peak_fraction: f64,
    /// The share of the day the sun is up on the **civil** clock — the local mode without a
    /// place, and the fixed picture of a clock that is off — centred on noon (0.5: up from
    /// 06:00 to 18:00).
    pub civil_daylight: f64,
}

impl Default for DayClock {
    fn default() -> DayClock {
        DayClock {
            mode: DayMode::Cycle,
            length_min: 120.0,
            daylight: 0.67,
            start_hour: 8.0,
            latitude: None,
            longitude: None,
            look: Look::Day,
            plant_night_factor: 0.05,
            noon_elevation_deg: 70.0,
            twilight_deg: 6.0,
            temperature_peak_fraction: 2.0 / 3.0,
            civil_daylight: 0.5,
        }
    }
}

impl DayClock {
    /// Seconds in one of this clock's days: the unit the climate's per-day rates count.
    pub fn day_s(&self) -> f64 {
        match self.mode {
            DayMode::Local => 86_400.0,
            DayMode::Cycle | DayMode::Off => self.length_min * 60.0,
        }
    }

    /// The cycle's phase at `tick`: 0 midnight, 0.5 noon.
    pub fn cycle_phase(&self, tick: u64) -> f64 {
        (self.start_hour / 24.0 + tick as f64 * DT / (self.length_min * 60.0)).rem_euclid(1.0)
    }

    /// The cycle's sunrise and sunset phases: `0.5 ∓ daylight / 2`.
    pub fn cycle_rise_set(&self) -> (f64, f64) {
        (0.5 - self.daylight / 2.0, 0.5 + self.daylight / 2.0)
    }

    /// `(phase, sunrise, sunset)` at wall time `unix_s` (seconds since 1970, UTC).
    ///
    /// With a place: solar time there, from the standard NOAA approximation (the
    /// equation of time and the declination for the date), so solar noon is phase 0.5
    /// and the season sets the day's length. Without one: [`DayClock::civil_phase`] on
    /// the machine's own UTC offset.
    pub fn local_phase(&self, unix_s: f64) -> (f64, f64, f64) {
        let (Some(lat), Some(lon)) = (self.latitude, self.longitude) else {
            return self.civil_phase(unix_s, local_utc_offset_s());
        };
        let days = (unix_s / 86_400.0).floor();
        let utc_min = (unix_s - days * 86_400.0) / 60.0;
        let (doy, year_days) = day_of_year(days as i64);
        let g = core::f64::consts::TAU / year_days * (doy as f64 + (utc_min / 60.0 - 12.0) / 24.0);
        let eqtime = 229.18
            * (0.000075 + 0.001868 * g.cos()
                - 0.032077 * g.sin()
                - 0.014615 * (2.0 * g).cos()
                - 0.040849 * (2.0 * g).sin());
        let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos()
            + 0.000907 * (2.0 * g).sin()
            - 0.002697 * (3.0 * g).cos()
            + 0.00148 * (3.0 * g).sin();
        let lat = lat.to_radians();
        let cos_ha = (90.833f64.to_radians().cos() / (lat.cos() * decl.cos())
            - lat.tan() * decl.tan())
        .clamp(-1.0, 1.0);
        let day = (cos_ha.acos().to_degrees() / 180.0).clamp(0.05, 0.95);
        let noon_min = 720.0 - 4.0 * lon - eqtime;
        let phase = (0.5 + (utc_min - noon_min) / 1440.0).rem_euclid(1.0);
        (phase, 0.5 - day / 2.0, 0.5 + day / 2.0)
    }

    /// Sunrise and sunset, as phases, on the civil clock: centred on noon, the sun up for
    /// [`DayClock::civil_daylight`] of the day.
    pub fn civil_rise_set(&self) -> (f64, f64) {
        (0.5 - self.civil_daylight / 2.0, 0.5 + self.civil_daylight / 2.0)
    }

    /// `(phase, sunrise, sunset)` on the civil clock `utc_offset_s` from UTC: the local
    /// clock's own hour, with the sun up from 06:00 to 18:00 at the default
    /// [`DayClock::civil_daylight`].
    pub fn civil_phase(&self, unix_s: f64, utc_offset_s: f64) -> (f64, f64, f64) {
        let (rise, set) = self.civil_rise_set();
        (((unix_s + utc_offset_s) / 86_400.0).rem_euclid(1.0), rise, set)
    }

    /// Where the simulation's sun is at `tick`: the cycle, the wall clock, or noon when
    /// the clock is off.
    pub fn sun(&self, tick: u64, wall_s: Option<f64>) -> Sun {
        match self.mode {
            DayMode::Cycle => {
                let (rise, set) = self.cycle_rise_set();
                Sun::at(self, self.cycle_phase(tick), rise, set)
            }
            DayMode::Local => {
                let now = wall_s.unwrap_or_else(wall_clock_s);
                let (phase, rise, set) = self.local_phase(now);
                Sun::at(self, phase, rise, set)
            }
            DayMode::Off => {
                let (rise, set) = self.civil_rise_set();
                Sun::at(self, 0.5, rise, set)
            }
        }
    }
}

/// The wall clock, seconds since 1970 UTC. Read only by a `local` day clock.
fn wall_clock_s() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// The machine's UTC offset in seconds, from `date +%z` (this crate has no unsafe code
/// and no time-zone table), read once an hour. Zero if it cannot be read.
fn local_utc_offset_s() -> f64 {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<(f64, f64)>> = Mutex::new(None);
    let now = wall_clock_s();
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, offset)) = *cache
        && (now - at).abs() < 3600.0
    {
        return offset;
    }
    let offset = std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            let s = s.trim();
            let sign = if s.starts_with('-') { -1.0 } else { 1.0 };
            let digits = s.trim_start_matches(['+', '-']);
            (digits.len() == 4).then_some(())?;
            let h: f64 = digits[..2].parse().ok()?;
            let m: f64 = digits[2..].parse().ok()?;
            Some(sign * (h * 3600.0 + m * 60.0))
        })
        .unwrap_or(0.0);
    *cache = Some((now, offset));
    offset
}

/// `(day of the year from 0, days in that year)` for a day count since 1970-01-01.
fn day_of_year(days: i64) -> (i64, f64) {
    // Howard Hinnant's civil_from_days, for the year alone.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy_march = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy_march + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let jan1 = days_from_civil(year);
    (days - jan1, if leap { 366.0 } else { 365.0 })
}

/// Days since 1970-01-01 of 1 January of `year`.
fn days_from_civil(year: i64) -> i64 {
    let y = year - 1; // January counts in the previous March-based year
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = 306; // 1 January is day 306 of the March-based year
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The sun at one moment: where in the day it is, and how high.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sun {
    /// 0 midnight, 0.5 noon.
    pub phase: f64,
    /// Sunrise and sunset, as phases.
    pub rise: f64,
    pub set: f64,
    /// Radians above the horizon; negative at night.
    pub elevation: f64,
    /// 0 (night) to 1 (day), a smoothstep of the elevation over civil twilight.
    pub daylight: f64,
}

impl Sun {
    /// The sun at `phase` on a day that rises at `rise` and sets at `set`.
    ///
    /// By day the elevation is a sine that peaks at [`DayClock::noon_elevation_deg`] at the
    /// middle of the daylight and crosses zero at sunrise and sunset. By night it is the
    /// negative sine over the night, scaled so the curve's slope is continuous at the
    /// horizon. Daylight is a smoothstep over [`DayClock::twilight_deg`].
    pub fn at(clock: &DayClock, phase: f64, rise: f64, set: f64) -> Sun {
        let peak = clock.noon_elevation_deg.to_radians();
        let day = (set - rise).clamp(1e-6, 1.0 - 1e-6);
        let night = 1.0 - day;
        let since_rise = (phase - rise).rem_euclid(1.0);
        let elevation = if since_rise <= day {
            peak * (core::f64::consts::PI * since_rise / day).sin()
        } else {
            let depth = (peak * night / day).min(core::f64::consts::FRAC_PI_2);
            -depth * (core::f64::consts::PI * (since_rise - day) / night).sin()
        };
        let tw = clock.twilight_deg.to_radians();
        let x = ((elevation + tw) / (2.0 * tw)).clamp(0.0, 1.0);
        Sun {
            phase,
            rise,
            set,
            elevation,
            daylight: x * x * (3.0 - 2.0 * x),
        }
    }

    /// The temperature peak: `peak_fraction` of the way through the daylight
    /// ([`DayClock::temperature_peak_fraction`]).
    pub fn peak(&self, peak_fraction: f64) -> f64 {
        (self.rise + (self.set - self.rise) * peak_fraction).rem_euclid(1.0)
    }
}

/// The diurnal temperature curve, `−1` at sunrise and `+1` `peak_fraction` of the way
/// through the daylight: a half cosine up to the peak and a half cosine back down to the
/// next sunrise.
pub fn diurnal_shape(phase: f64, rise: f64, set: f64, peak_fraction: f64) -> f64 {
    let up = ((set - rise) * peak_fraction).clamp(1e-6, 1.0 - 1e-6);
    let p = (phase - rise).rem_euclid(1.0);
    let pi = core::f64::consts::PI;
    if p <= up {
        -(pi * p / up).cos()
    } else {
        -(pi * (1.0 + (p - up) / (1.0 - up))).cos()
    }
}

/// The afternoon (convection) bump: 1 at the temperature peak, a Gaussian `width` of the
/// cycle wide, measured round the clock.
pub fn afternoon_bump(sun: &Sun, width: f64, peak_fraction: f64) -> f64 {
    if !(width > 0.0) {
        return 0.0;
    }
    let d = (sun.phase - sun.peak(peak_fraction)).rem_euclid(1.0);
    let d = d.min(1.0 - d);
    (-0.5 * (d / width).powi(2)).exp()
}

// ------------------------------------------------------------------ the climate

/// One rain mode's row of the climate: its hazard, its rate, and when it stops.
///
/// `λ = lambda_max_per_h · σ((H − rh50) / width) · max(0, 1 + g_aft·A + g_cool·K + g_front·F)`
/// events per simulated hour ([`hazard`]). `H` is the mode's humidity reading
/// ([`reads_moisture`]): RH for drizzle, a saturation effect; the moisture `M` for the
/// convective modes, shower and downpour (WX1b).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RainParams {
    /// Events per simulated hour at saturation of the logistic and no boosts.
    pub lambda_max_per_h: f64,
    /// The humidity (RH, or `M` for a convective mode) at which the logistic is half.
    pub rh50: f64,
    /// The logistic's width in the same reading.
    pub width: f64,
    /// Gains on the afternoon bump `A`, the cooling rate `K` (°C per simulated hour,
    /// capped) and a fresh front `F`.
    pub g_aft: f64,
    pub g_cool: f64,
    pub g_front: f64,
    /// The rate it rains at, in multiples of [`crate::Config::rain_m_per_s`] (R₀).
    pub rate_r0: f64,
    /// The event ends when its humidity reading (RH, or `M`) falls to this.
    pub stop_rh: f64,
    /// Or when it has rained this long, minutes of nature (the weather speed divides it).
    pub longest_min: f64,
    /// While it rains the air relaxes toward this much cooler, °C.
    pub cool_c: f64,
}

/// Every [`RainParams`] key as an optional override: one row of `[world.climate.<mode>]`.
macro_rules! rain_keys {
    ($($k:ident),* $(,)?) => {
        /// A `[world.climate.<mode>]` table: any [`RainParams`] key, each replacing the
        /// preset's.
        #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        pub struct RainSpec { $(pub $k: Option<f64>,)* }
        impl RainSpec {
            fn apply(&self, to: &mut RainParams) { $(if let Some(v) = self.$k { to.$k = v; })* }
            fn capture(from: &RainParams) -> RainSpec { RainSpec { $($k: Some(from.$k),)* } }
        }
    };
}
rain_keys!(
    lambda_max_per_h,
    rh50,
    width,
    g_aft,
    g_cool,
    g_front,
    rate_r0,
    stop_rh,
    longest_min,
    cool_c,
);

/// How much faster than nature the weather runs: `[world.climate] weather_speed`, a
/// number or `"auto"` (Wrysk's decision 8, 2026-09-24: on a short day, faster than
/// nature, but not by the whole compression of the day).
///
/// `auto` is `(24 h / cycle length)^0.65`: 5.0 on the default 2 h day, 1 on a 24 h
/// cycle and under the `local` clock (`off` counts `length_min`).
/// [`Climate::at_speed`] says what it scales and what it leaves alone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum WeatherSpeed {
    #[default]
    Auto,
    Fixed(f64),
}

/// Seconds in a day of nature: what the preset's "days" (the drift's time scale, a
/// front's recovery) count before the weather speed divides them.
pub const NATURE_DAY_S: f64 = 86_400.0;

impl WeatherSpeed {
    /// The factor on this clock.
    pub fn factor(self, day: &DayClock, auto_exponent: f64) -> f64 {
        match self {
            WeatherSpeed::Fixed(x) => x,
            WeatherSpeed::Auto => match day.mode {
                DayMode::Local => 1.0,
                DayMode::Cycle | DayMode::Off => {
                    (1440.0 / day.length_min).powf(auto_exponent)
                }
            },
        }
    }
}

/// `"auto"` or a number in a TOML file; a number in a snapshot (`auto` as −1), because
/// postcard cannot tell a string from a number without being told.
impl Serialize for WeatherSpeed {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match (self, s.is_human_readable()) {
            (WeatherSpeed::Auto, true) => s.serialize_str("auto"),
            (WeatherSpeed::Auto, false) => s.serialize_f64(-1.0),
            (WeatherSpeed::Fixed(x), _) => s.serialize_f64(*x),
        }
    }
}

impl<'de> Deserialize<'de> for WeatherSpeed {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<WeatherSpeed, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = WeatherSpeed;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("\"auto\" or a positive number")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<WeatherSpeed, E> {
                if v == "auto" {
                    Ok(WeatherSpeed::Auto)
                } else {
                    Err(E::custom(format!(
                        "weather_speed is \"auto\" or a number, not {v:?}"
                    )))
                }
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<WeatherSpeed, E> {
                Ok(if v < 0.0 {
                    WeatherSpeed::Auto
                } else {
                    WeatherSpeed::Fixed(v)
                })
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<WeatherSpeed, E> {
                self.visit_f64(v as f64)
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<WeatherSpeed, E> {
                self.visit_f64(v as f64)
            }
        }
        if d.is_human_readable() {
            d.deserialize_any(V)
        } else {
            d.deserialize_f64(V)
        }
    }
}

/// The named climates: `[world.climate] preset`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClimatePreset {
    /// Lowland tropical rainforest: warm, almost seasonless, wet, with afternoon storms.
    #[default]
    TropicalJungle,
}

impl ClimatePreset {
    pub const ALL: [ClimatePreset; 1] = [ClimatePreset::TropicalJungle];

    pub fn name(self) -> &'static str {
        match self {
            ClimatePreset::TropicalJungle => "tropical_jungle",
        }
    }

    pub fn find(name: &str) -> Option<ClimatePreset> {
        ClimatePreset::ALL.into_iter().find(|p| p.name() == name)
    }

    pub fn climate(self) -> Climate {
        match self {
            ClimatePreset::TropicalJungle => Climate::tropical_jungle(),
        }
    }
}

/// **A moment of the Cubarium year** ([`Climate::year`]): which year since the first dry
/// season (−1 before it) and where in it, `[0, 1)`, phase 0 being a season's start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Year {
    pub number: i64,
    pub phase: f64,
}

impl Year {
    /// Whether the phase lies in the window of `width` (a share of the year) opening at
    /// `start`, wrapping past the year's end; a width of 1 or more is the whole year, 0 none.
    pub fn in_window(&self, start: f64, width: f64) -> bool {
        if !(width > 0.0) {
            return false;
        }
        width >= 1.0 || (self.phase - start).rem_euclid(1.0) < width
    }
}

/// `[world.climate]`: a named preset of the keys below, any of which the table can
/// override one by one (`[world.climate.downpour] rate_r0 = 1.2`, say). The preset sets
/// temperature and wetness; the landform still sets how much water the world has.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ClimateSpec", into = "ClimateSpec")]
pub struct Climate {
    pub preset: ClimatePreset,
    /// How much faster than nature the weather runs ([`WeatherSpeed`]). The time
    /// constants below are **nature's**; [`Climate::at_speed`] applies it.
    pub weather_speed: WeatherSpeed,
    /// Mean air temperature, °C.
    pub temperature_c: f64,
    /// The diurnal swing, ± °C: coolest at sunrise, warmest two thirds through the day.
    pub diurnal_c: f64,
    /// Day-to-day drift: a mean-reverting random walk with this standard deviation, °C,
    /// over `drift_days` days of nature.
    pub drift_sd_c: f64,
    pub drift_days: f64,
    /// Cold fronts per day; each cools by a depth drawn from `front_min_c..front_max_c`
    /// over `front_onset_min` and recovers over `front_recovery_days`. `F` is 1 for the
    /// first `front_window_min` after one arrives.
    pub fronts_per_day: f64,
    pub front_min_c: f64,
    pub front_max_c: f64,
    pub front_onset_min: f64,
    pub front_recovery_days: f64,
    pub front_window_min: f64,
    /// Rain cooling's time constant, simulated minutes (the depth is per mode).
    pub rain_cool_min: f64,
    /// What the air holds at the mean temperature, as a share of the world's water.
    pub capacity_share: f64,
    /// The afternoon bump's width, as a share of the cycle.
    pub afternoon_width: f64,
    /// The cooling rate `K` is capped here, °C per simulated hour.
    pub cooling_cap_c_per_h: f64,
    /// After an event, the next natural start waits a gap drawn from this range, seconds
    /// of simulated time. `0 / 0` turns the gap off.
    pub dry_gap_min_s: f64,
    pub dry_gap_max_s: f64,
    /// **The dry season** (package DRY-SEASON, Wrysk 2026-10-06; `design/webmodel/meadow.md`
    /// §11.5-11.6): a periodic stretch of world days in which the natural rain hazards are
    /// multiplied by `dry_season_rain`, so drained ground dries down and the turf cures
    /// while woody seedlings starve back. Off: the weather is exactly the season-less one.
    ///
    /// The clock counts **world days** ([`DayClock::day_s`], the day the plants' clock
    /// counts too), from the world's tick 0: none before `dry_season_phase_days`, then one
    /// of `dry_season_len_days` every `dry_season_every_days`. It is not scaled by the
    /// weather speed. The forced downpour at `rh_max` is held off inside it (kept, the closed
    /// cycle refills to `rh_max` about 45 min after each downpour and forces another, so the
    /// ground never dries): the water the season keeps aloft falls as forced downpours once
    /// it ends, the rains' return.
    pub dry_season: bool,
    /// World days from one season's start to the next.
    pub dry_season_every_days: f64,
    /// World days a season lasts.
    pub dry_season_len_days: f64,
    /// World days from the world's founding to the first season's start.
    pub dry_season_phase_days: f64,
    /// The natural rain hazards' multiplier inside a season (0: no natural rain).
    pub dry_season_rain: f64,
    /// Fog forms at or above `fog_form_rh` (with no shower or downpour falling) in the
    /// dawn window — from `fog_dawn_before` of the cycle before sunrise to
    /// `fog_dawn_after` after — or while the air cools faster than
    /// `fog_cooling_c_per_h`. Its density rises toward
    /// `min(1, (RH − fog_clear_rh) / fog_span_rh)` over `fog_form_min`, and it clears over
    /// `fog_clear_min` once RH falls under `fog_clear_rh` or the sun is `fog_burn_after`
    /// of the cycle past sunrise.
    pub fog_form_rh: f64,
    pub fog_clear_rh: f64,
    pub fog_span_rh: f64,
    pub fog_dawn_before: f64,
    pub fog_dawn_after: f64,
    pub fog_burn_after: f64,
    pub fog_cooling_c_per_h: f64,
    pub fog_form_min: f64,
    pub fog_clear_min: f64,
    /// Lightning in a downpour: strikes a Poisson process of this mean interval, each
    /// interval clamped to `strike_min_s..strike_max_s`, seconds.
    pub strike_mean_s: f64,
    pub strike_min_s: f64,
    pub strike_max_s: f64,
    /// The **stream swell** (WX1 addendum): while it rains, the spring's stream runs at
    /// `reentry_m3_per_s · (1 + s)`, with `s` relaxing toward
    /// `storm_stream_gain · clamp(rate / downpour rate, 0, 1)`; 0 turns it off. Paid for
    /// by the store the stream already draws on, above its floor.
    pub storm_stream_gain: f64,
    /// The host's opening rain ([`World::open_with_rain`]) falls at least this long,
    /// seconds, before its stop humidity may end it.
    pub opening_min_s: f64,
    /// The Clausius–Clapeyron slope: what the air holds grows by this fraction per °C
    /// (0.07: 7 % per °C).
    pub cc_per_c: f64,
    /// Relative humidity never passes this outside a dry season: reaching it starts a forced
    /// downpour (held off inside one, `dry_season`).
    pub rh_max: f64,
    /// Evaporation from open water scales with `max(0, 1 − RH) / evaporation_rh_span`:
    /// the configured rate at RH `1 − span`, none at saturation.
    pub evaporation_rh_span: f64,
    /// The atmosphere-fed stream draws only while the sky stands above this RH (WX1b).
    pub stream_floor_rh: f64,
    /// The stream swell's time constants, seconds of simulated time: it rises toward its
    /// target over `swell_rise_s` while it rains and falls back over `swell_fall_s` after
    /// (~95 % gone 2.5 min after the event). Not scaled by the weather speed.
    pub swell_rise_s: f64,
    pub swell_fall_s: f64,
    /// The exponent of the `auto` weather speed: `(24 h / cycle length)^auto_speed_exponent`.
    pub auto_speed_exponent: f64,
    /// The relative humidity a closed world's sky is founded at (WX1b).
    pub founding_rh: f64,
    /// The sky's cloud cover (visual only) is `(RH - cloud_start_rh) / cloud_span_rh`,
    /// clamped to `0..=1`, with a floor by rain mode while it rains: drizzle, shower and
    /// downpour fall from at least `cloud_floor_drizzle` / `_shower` / `_downpour`.
    pub cloud_start_rh: f64,
    pub cloud_span_rh: f64,
    pub cloud_floor_drizzle: f64,
    pub cloud_floor_shower: f64,
    pub cloud_floor_downpour: f64,
    pub drizzle: RainParams,
    pub shower: RainParams,
    pub downpour: RainParams,
}

macro_rules! climate_keys {
    ($($k:ident),* $(,)?) => {
        /// The `[world.climate]` table as written: a preset name and any key of
        /// [`Climate`], each replacing the preset's. A snapshot writes every key.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default, deny_unknown_fields)]
        pub struct ClimateSpec {
            pub preset: Option<String>,
            pub weather_speed: Option<WeatherSpeed>,
            pub dry_season: Option<bool>,
            $(pub $k: Option<f64>,)*
            pub drizzle: Option<RainSpec>,
            pub shower: Option<RainSpec>,
            pub downpour: Option<RainSpec>,
        }
        impl ClimateSpec {
            fn apply(&self, to: &mut Climate) {
                if let Some(v) = self.weather_speed { to.weather_speed = v; }
                if let Some(v) = self.dry_season { to.dry_season = v; }
                $(if let Some(v) = self.$k { to.$k = v; })*
                if let Some(s) = &self.drizzle { s.apply(&mut to.drizzle); }
                if let Some(s) = &self.shower { s.apply(&mut to.shower); }
                if let Some(s) = &self.downpour { s.apply(&mut to.downpour); }
            }
        }
        impl From<Climate> for ClimateSpec {
            fn from(c: Climate) -> ClimateSpec {
                ClimateSpec {
                    preset: Some(c.preset.name().to_string()),
                    weather_speed: Some(c.weather_speed),
                    dry_season: Some(c.dry_season),
                    $($k: Some(c.$k),)*
                    drizzle: Some(RainSpec::capture(&c.drizzle)),
                    shower: Some(RainSpec::capture(&c.shower)),
                    downpour: Some(RainSpec::capture(&c.downpour)),
                }
            }
        }
    };
}
climate_keys!(
    temperature_c,
    diurnal_c,
    drift_sd_c,
    drift_days,
    fronts_per_day,
    front_min_c,
    front_max_c,
    front_onset_min,
    front_recovery_days,
    front_window_min,
    rain_cool_min,
    capacity_share,
    afternoon_width,
    cooling_cap_c_per_h,
    dry_gap_min_s,
    dry_gap_max_s,
    dry_season_every_days,
    dry_season_len_days,
    dry_season_phase_days,
    dry_season_rain,
    fog_form_rh,
    fog_clear_rh,
    fog_span_rh,
    fog_dawn_before,
    fog_dawn_after,
    fog_burn_after,
    fog_cooling_c_per_h,
    fog_form_min,
    fog_clear_min,
    strike_mean_s,
    strike_min_s,
    strike_max_s,
    storm_stream_gain,
    opening_min_s,
    cc_per_c,
    rh_max,
    evaporation_rh_span,
    stream_floor_rh,
    swell_rise_s,
    swell_fall_s,
    auto_speed_exponent,
    founding_rh,
    cloud_start_rh,
    cloud_span_rh,
    cloud_floor_drizzle,
    cloud_floor_shower,
    cloud_floor_downpour,
);

impl TryFrom<ClimateSpec> for Climate {
    type Error = String;
    fn try_from(spec: ClimateSpec) -> Result<Climate, String> {
        let preset = match spec.preset.as_deref() {
            None => ClimatePreset::default(),
            Some(name) => ClimatePreset::find(name).ok_or_else(|| {
                let known: Vec<&str> = ClimatePreset::ALL.iter().map(|p| p.name()).collect();
                format!("no climate preset is called {name:?}; the shipped ones are {known:?}")
            })?,
        };
        let mut climate = preset.climate();
        spec.apply(&mut climate);
        Ok(climate)
    }
}

impl Default for Climate {
    fn default() -> Climate {
        Climate::tropical_jungle()
    }
}

impl Climate {
    /// Lowland tropical rainforest (`design/handoffs/voxel-weather-2026-09-24.md`, "The
    /// model"): the brief's starting values, calibrated in WX1.
    ///
    /// **Calibration (WX1b).** Showers and downpours read the moisture `M`
    /// ([`reads_moisture`]); round 3's workarounds for the RH gate are undone and the
    /// values are fitted from the brief's shape in `M` on a 0-D mirror of this model (one
    /// store at the terrarium's capacity, evaporation and transpiration fitted to the
    /// round-3 census; 24 seeds x 4 h young and lush, 4 x 24 h lush). Convection is the
    /// afternoon gain: shower 4 (brief 1), downpour 20 (brief 3), with wider logistics
    /// (0.08) so a moist afternoon fires below rh50. Downpour 2 R0 (the brief's floor),
    /// shower λ 1.5, downpour λ 1.0 and rh50 1.13, drizzle rh50 0.98 (it stays nocturnal).
    /// Mirror: 0.40 / 0.51 of the volume in 12–18 (terrarium seeds 1 / 2), downpours
    /// 5.3 / 4.1 min, showers 4.3 / 3.1 min, downpours 56 / 43 % of the volume.
    pub fn tropical_jungle() -> Climate {
        Climate {
            preset: ClimatePreset::TropicalJungle,
            weather_speed: WeatherSpeed::Auto,
            temperature_c: 26.0,
            diurnal_c: 3.0,
            drift_sd_c: 1.0,
            drift_days: 3.0,
            fronts_per_day: 0.1,
            front_min_c: 2.0,
            front_max_c: 4.0,
            front_onset_min: 30.0,
            front_recovery_days: 0.3,
            front_window_min: 30.0,
            rain_cool_min: 20.0,
            capacity_share: 0.03,
            afternoon_width: 0.08,
            cooling_cap_c_per_h: 3.0,
            dry_gap_min_s: 300.0,
            dry_gap_max_s: 900.0,
            // The model's winner (meadow.md §11.6, `ew+run+d5:1:0.5`): every 5 world days a
            // 1-day season from day 5. Off in the preset; the display presets turn it on.
            dry_season: false,
            dry_season_every_days: 5.0,
            dry_season_len_days: 1.0,
            dry_season_phase_days: 5.0,
            dry_season_rain: 0.0,
            fog_form_rh: 0.9,
            fog_clear_rh: 0.85,
            fog_span_rh: 0.15,
            fog_dawn_before: 0.1,
            fog_dawn_after: 0.06,
            fog_burn_after: 0.1,
            fog_cooling_c_per_h: 0.5,
            fog_form_min: 10.0,
            fog_clear_min: 15.0,
            strike_mean_s: 30.0,
            strike_min_s: 10.0,
            strike_max_s: 60.0,
            // Wrysk's pick from the gain 0 | 2 | 4 video (2026-09-25): 5x the flow at a
            // downpour's peak.
            storm_stream_gain: 4.0,
            opening_min_s: 60.0,
            cc_per_c: 0.07,
            rh_max: 1.5,
            evaporation_rh_span: 0.4,
            stream_floor_rh: 0.5,
            swell_rise_s: 45.0,
            swell_fall_s: 50.0,
            auto_speed_exponent: 0.65,
            founding_rh: 0.6,
            cloud_start_rh: 0.4,
            cloud_span_rh: 0.6,
            cloud_floor_drizzle: 0.6,
            cloud_floor_shower: 0.75,
            cloud_floor_downpour: 0.9,
            drizzle: RainParams {
                lambda_max_per_h: 1.5,
                rh50: 0.98,
                width: 0.03,
                g_aft: -0.8,
                g_cool: 1.0,
                g_front: 0.5,
                rate_r0: 0.1,
                stop_rh: 0.8,
                longest_min: 90.0,
                cool_c: 0.5,
            },
            shower: RainParams {
                lambda_max_per_h: 1.5,
                rh50: 1.0,
                width: 0.08,
                g_aft: 4.0,
                g_cool: 0.25,
                g_front: 1.0,
                rate_r0: 1.0,
                stop_rh: 0.7,
                longest_min: 30.0,
                cool_c: 1.5,
            },
            downpour: RainParams {
                lambda_max_per_h: 1.0,
                rh50: 1.13,
                width: 0.08,
                g_aft: 20.0,
                g_cool: 0.5,
                g_front: 3.0,
                rate_r0: 2.0,
                stop_rh: 0.5,
                longest_min: 50.0,
                cool_c: 3.0,
            },
        }
    }

    /// The climate as the model runs it on `day`: nature's time scales divided by the
    /// weather speed — the events' longest durations, the fog and rain-cooling time
    /// constants, a front's ramp, recovery and fresh window, the drift's time scale —
    /// and the hazards' `λmax` multiplied by it. The diurnal curve, the afternoon bump
    /// and the dawn window stay tied to the cycle; the dry gap, the lightning interval,
    /// the rain rates and the fronts per day are not scaled.
    pub fn at_speed(&self, day: &DayClock) -> Climate {
        let k = self.weather_speed.factor(day, self.auto_speed_exponent).max(1e-9);
        let mut c = *self;
        c.drift_days /= k;
        c.front_onset_min /= k;
        c.front_recovery_days /= k;
        c.front_window_min /= k;
        c.rain_cool_min /= k;
        c.fog_form_min /= k;
        c.fog_clear_min /= k;
        for m in [&mut c.drizzle, &mut c.shower, &mut c.downpour] {
            m.longest_min /= k;
            m.lambda_max_per_h *= k;
        }
        c
    }

    /// Whether the dry season holds at `tick` on `day`'s clock: the switch on, a period and
    /// a length above zero, and `tick` at or past the phase and within `len` of a period's
    /// start. World days count [`DayClock::day_s`] from tick 0.
    pub fn in_dry_season(&self, day: &DayClock, tick: u64) -> bool {
        if !self.dry_season || !(self.dry_season_every_days > 0.0) || !(self.dry_season_len_days > 0.0) {
            return false;
        }
        let days = tick as f64 * DT / day.day_s();
        let since = days - self.dry_season_phase_days;
        since >= 0.0 && since.rem_euclid(self.dry_season_every_days) < self.dry_season_len_days
    }

    /// **The Cubarium year** (LIFE-HISTORY, Wrysk 2026-10-06: the dry season "makes a natural
    /// marker for what a 'year' means"): one year is the season cycle,
    /// `dry_season_every_days` world days ([`DayClock::day_s`]), and phase 0 is a season's
    /// start (`dry_season_phase_days` from tick 0, then every period), so the season holds
    /// over phases `[0, dry_season_len_days / dry_season_every_days)`. The year runs whether
    /// or not the season is on: [`Climate::dry_season`] turns off the dryness, not the year.
    /// Before the first season the clock reads year −1 (and earlier, for a phase longer than a
    /// period). `None` only with no period (`dry_season_every_days` 0) or no day.
    pub fn year(&self, day: &DayClock, tick: u64) -> Option<Year> {
        let day_s = day.day_s();
        if !(self.dry_season_every_days > 0.0) || !(day_s > 0.0) {
            return None;
        }
        let days = tick as f64 * DT / day_s;
        let y = (days - self.dry_season_phase_days) / self.dry_season_every_days;
        let number = y.floor();
        Some(Year {
            number: number as i64,
            phase: (y - number).clamp(0.0, 1.0 - f64::EPSILON),
        })
    }

    /// A year's length, seconds (`dry_season_every_days` world days); 0 with no year.
    pub fn year_s(&self, day: &DayClock) -> f64 {
        (self.dry_season_every_days * day.day_s()).max(0.0)
    }

    /// The natural rain hazards' multiplier at `tick`: `dry_season_rain` inside a season,
    /// 1 outside it (and always, with the switch off).
    pub fn rain_scale(&self, day: &DayClock, tick: u64) -> f64 {
        if self.in_dry_season(day, tick) { self.dry_season_rain } else { 1.0 }
    }

    /// What the air holds at `t_c`, cubic metres, in a world of `total_m3`:
    /// `capacity_share · total · e^{0.07 (T − temperature_c)}`.
    pub fn capacity_m3(&self, total_m3: f64, t_c: f64) -> f64 {
        self.capacity_share * total_m3 * (self.cc_per_c * (t_c - self.temperature_c)).exp()
    }

    /// A mode's row. `Clear` has none.
    pub fn mode(&self, mode: RainMode) -> Option<&RainParams> {
        match mode {
            RainMode::Clear => None,
            RainMode::Drizzle => Some(&self.drizzle),
            RainMode::Shower => Some(&self.shower),
            RainMode::Downpour => Some(&self.downpour),
        }
    }
}

/// The multiplier on the configured evaporation rate: `max(0, 1 − RH) / 0.4` times what
/// the warmer or cooler air can take, `C(T) / C(temperature_c)`.
pub fn evaporation_scale(c: &Climate, rh: f64, t_c: f64) -> f64 {
    (1.0 - rh).max(0.0) / c.evaporation_rh_span * (c.cc_per_c * (t_c - c.temperature_c)).exp()
}

/// Whether a mode's start and stop read the moisture `M` rather than RH (WX1b).
///
/// With one lumped store, RH is lowest in the afternoon, when the capacity peaks with the
/// temperature, so an RH gate fought the afternoon bump: round 3 put 0–35 % of the rain
/// in 12–18 and started storms at RH 0.65–0.77. Convection is fed by how much water the
/// air carries, not by how near it is to saturation, so showers and downpours read
/// `M = atmosphere / C(temperature_c)`. Drizzle, fog, the soft evaporation limit and the
/// hard limit keep RH: those are saturation effects.
pub fn reads_moisture(mode: RainMode) -> bool {
    matches!(mode, RainMode::Shower | RainMode::Downpour)
}

/// A mode's hazard, events per simulated hour, at its humidity reading `rh` (RH, or `M`
/// for a convective mode). Zero at or under the mode's stop value: an event that would
/// end the tick it starts is not drawn.
pub fn hazard(p: &RainParams, rh: f64, a: f64, k: f64, f: f64) -> f64 {
    if rh <= p.stop_rh || !(p.lambda_max_per_h > 0.0) {
        return 0.0;
    }
    let s = 1.0 / (1.0 + (-(rh - p.rh50) / p.width).exp());
    let boost = (1.0 + p.g_aft * a + p.g_cool * k + p.g_front * f).max(0.0);
    let l = p.lambda_max_per_h * s * boost;
    if l.is_nan() { 0.0 } else { l }
}

// ------------------------------------------------------------------ the state

/// Mode indices for the per-mode counters: drizzle, shower, downpour.
fn mode_index(mode: RainMode) -> Option<usize> {
    match mode {
        RainMode::Clear => None,
        RainMode::Drizzle => Some(0),
        RainMode::Shower => Some(1),
        RainMode::Downpour => Some(2),
    }
}

const MODES: [RainMode; 3] = [RainMode::Drizzle, RainMode::Shower, RainMode::Downpour];

/// A cold front: arrived at `start_tick`, cooling by `depth_c` at its deepest.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Front {
    pub start_tick: u64,
    pub depth_c: f64,
}

/// Why an event started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cause {
    /// The minute's draw.
    Natural,
    /// RH reached `Climate::rh_max`.
    Forced,
    /// The host's opening rain ([`World::open_with_rain`]).
    Opening,
    /// A development command ([`World::force_rain`]).
    Commanded,
}

/// One rain event, running or finished: the `--weather-log` row.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub mode: RainMode,
    pub cause: Cause,
    pub start_tick: u64,
    /// The first tick without its rain. Equal to `start_tick` while it runs.
    pub end_tick: u64,
    /// RH, the moisture `M`, °C and the day's phase at the start.
    pub rh: f64,
    pub moisture: f64,
    pub temperature_c: f64,
    pub day_phase: f64,
    /// Seconds since the previous event ended; `None` for the world's first.
    pub since_previous_s: Option<f64>,
    /// What fell, cubic metres.
    pub volume_m3: f64,
}

/// What the weather has done since the world began.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WeatherCounts {
    /// Events started, per mode, whatever the cause.
    pub drizzles: u64,
    pub showers: u64,
    pub downpours: u64,
    /// Downpours forced by RH reaching `Climate::rh_max` (also counted in `downpours`).
    pub forced: u64,
    /// Minute draws that would have started drizzle, shower, downpour inside a dry gap.
    pub gap_blocked: [u64; 3],
    pub strikes: u64,
    /// What fell, per mode and per quarter of the day (00–06, 06–12, 12–18, 18–24).
    pub rain_by_mode_m3: [f64; 3],
    pub rain_by_quarter_m3: [f64; 4],
    /// Events finished: [`Weather::last_finished`] is the newest.
    pub events_finished: u64,
    /// The highest RH any tick ended at.
    pub max_rh: f64,
}

/// The world's weather state. Part of the snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub(crate) sun: Sun,
    pub(crate) temp_c: f64,
    pub(crate) drift_c: f64,
    pub(crate) front: Option<Front>,
    pub(crate) rain_cool_c: f64,
    /// Last tick's smooth temperature terms (diurnal, front, rain cooling): what the
    /// cooling rate is measured on. The random drift is left out of it, so a minute's
    /// drift step never reads as a cold snap.
    pub(crate) smooth_c: f64,
    pub(crate) cooling_c_per_h: f64,
    pub(crate) mode: RainMode,
    pub(crate) event: Option<EventRecord>,
    /// Natural starts wait until this tick.
    pub(crate) gap_until: u64,
    pub(crate) last_end: Option<u64>,
    pub(crate) fog: f64,
    pub(crate) last_strike: Option<Strike>,
    pub(crate) next_strike: u64,
    pub(crate) last_finished: Option<EventRecord>,
    /// The stream swell `s`: the spring's stream runs at `reentry_m3_per_s · (1 + s)`.
    pub(crate) swell: f64,
    /// What the stream actually brought up last tick, cubic metres per second.
    pub(crate) stream_m3_s: f64,
    pub counts: WeatherCounts,
    /// A wall time injected in place of the machine's clock, for a `local` day clock.
    #[serde(skip)]
    pub(crate) wall_clock_s: Option<f64>,
}

impl Weather {
    /// A world's weather at `tick` before anything has happened: the sun where the
    /// clock puts it, the air at its mean plus the diurnal term, no event, no gap.
    pub(crate) fn fresh(config: &crate::Config, tick: u64, wall_clock_s: Option<f64>) -> Weather {
        let sun = config.day.sun(tick, wall_clock_s);
        let smooth = diurnal_c(config, &sun);
        Weather {
            sun,
            temp_c: config.climate.temperature_c + smooth,
            drift_c: 0.0,
            front: None,
            rain_cool_c: 0.0,
            smooth_c: smooth,
            cooling_c_per_h: 0.0,
            // The open budget's prescribed rain falls from the first tick: a shower.
            mode: if !config.closed_water_budget && config.rain_m_per_s > 0.0 {
                RainMode::Shower
            } else {
                RainMode::Clear
            },
            event: None,
            gap_until: 0,
            last_end: None,
            fog: 0.0,
            last_strike: None,
            next_strike: 0,
            last_finished: None,
            swell: 0.0,
            stream_m3_s: 0.0,
            counts: WeatherCounts::default(),
            wall_clock_s,
        }
    }

    pub fn mode(&self) -> RainMode {
        self.mode
    }
    pub fn temperature_c(&self) -> f64 {
        self.temp_c
    }
    pub fn fog(&self) -> f64 {
        self.fog
    }
    /// How fast the air is cooling, °C per simulated hour (0 while warming, capped).
    pub fn cooling_c_per_h(&self) -> f64 {
        self.cooling_c_per_h
    }
    pub fn sun(&self) -> Sun {
        self.sun
    }
    pub fn front(&self) -> Option<Front> {
        self.front
    }
    /// The event running now, if any.
    pub fn event(&self) -> Option<&EventRecord> {
        self.event.as_ref()
    }
    /// The newest finished event; [`WeatherCounts::events_finished`] says whether it is
    /// new since a reader last looked.
    pub fn last_finished(&self) -> Option<&EventRecord> {
        self.last_finished.as_ref()
    }
    pub fn last_strike(&self) -> Option<Strike> {
        self.last_strike
    }
    /// The stream swell `s` ([`Climate::storm_stream_gain`]).
    pub fn swell(&self) -> f64 {
        self.swell
    }
    /// What the spring's stream brought up last tick, cubic metres per second.
    pub fn stream_m3_s(&self) -> f64 {
        self.stream_m3_s
    }
    /// The tick natural starts wait for; zero or past when there is no gap.
    pub fn gap_until(&self) -> u64 {
        self.gap_until
    }
}

impl Default for Weather {
    fn default() -> Weather {
        Weather::fresh(&crate::Config::default(), 0, None)
    }
}

/// The diurnal term, °C: zero with the clock off.
fn diurnal_c(config: &crate::Config, sun: &Sun) -> f64 {
    if config.day.mode == DayMode::Off {
        0.0
    } else {
        config.climate.diurnal_c * diurnal_shape(sun.phase, sun.rise, sun.set, config.day.temperature_peak_fraction)
    }
}

/// A front's term at `tick`, °C (negative), and whether it is still fresh (`F`). `None`
/// once it has recovered.
fn front_term(c: &Climate, front: Front, tick: u64) -> Option<(f64, bool)> {
    let min = tick.saturating_sub(front.start_tick) as f64 * DT / 60.0;
    let onset = c.front_onset_min.max(1e-9);
    let recovery = (c.front_recovery_days * NATURE_DAY_S / 60.0).max(1e-9);
    let fresh = min < c.front_window_min;
    if min < onset {
        Some((-front.depth_c * min / onset, fresh))
    } else if min < onset + recovery {
        Some((-front.depth_c * (1.0 - (min - onset) / recovery), fresh))
    } else {
        None
    }
}

// ------------------------------------------------------------------ the tick

/// One tick of weather: the clock, the temperature, and — under the closed budget — the
/// rain events, lightning and fog. The start of the tick's rain phase
/// ([`crate::water::rain`]); the open budget's prescribed rain falls from here too.
pub(crate) fn step(w: &mut World) {
    let tick = w.tick;
    let seed = w.config.seed;
    let clock = w.config.day;
    let c = w.config.climate.at_speed(&clock);
    let day_s = clock.day_s();
    let off = clock.mode == DayMode::Off;

    // The clock.
    let sun = clock.sun(tick, w.weather.wall_clock_s);
    w.weather.sun = sun;

    // The slow terms, once a minute: the drift's step and whether a front arrives.
    let minute = tick / TICKS_PER_MIN;
    if tick % TICKS_PER_MIN == 0 {
        let wx = &mut w.weather;
        if c.drift_sd_c > 0.0 && c.drift_days > 0.0 {
            let a = (-60.0 / (c.drift_days * NATURE_DAY_S)).exp();
            wx.drift_c =
                wx.drift_c * a + c.drift_sd_c * (1.0 - a * a).sqrt() * normal(seed, minute);
        } else {
            wx.drift_c = 0.0;
        }
        if c.fronts_per_day > 0.0 {
            let p = 1.0 - (-c.fronts_per_day * 60.0 / day_s).exp();
            if draw(seed, minute, TAG_FRONT) < p {
                let depth = c.front_min_c
                    + (c.front_max_c - c.front_min_c) * draw(seed, minute, TAG_FRONT_DEPTH);
                wx.front = Some(Front {
                    start_tick: tick,
                    depth_c: depth,
                });
            }
        }
    }

    // The temperature.
    let (front_c, fresh_front) = match w.weather.front.and_then(|f| front_term(&c, f, tick)) {
        Some(t) => t,
        None => {
            w.weather.front = None;
            (0.0, false)
        }
    };
    let cool_target = -c.mode(w.weather.mode).map_or(0.0, |m| m.cool_c);
    let relax = 1.0 - (-DT / (c.rain_cool_min.max(1e-9) * 60.0)).exp();
    w.weather.rain_cool_c += (cool_target - w.weather.rain_cool_c) * relax;
    let smooth = diurnal_c(&w.config, &sun) + front_c + w.weather.rain_cool_c;
    w.weather.cooling_c_per_h =
        ((w.weather.smooth_c - smooth) / DT * 3600.0).clamp(0.0, c.cooling_cap_c_per_h);
    w.weather.smooth_c = smooth;
    w.weather.temp_c = c.temperature_c + w.weather.drift_c + smooth;

    if w.config.closed_water_budget {
        let a = if off {
            0.0
        } else {
            afternoon_bump(&sun, c.afternoon_width, w.config.day.temperature_peak_fraction)
        };
        events(w, &c, a, if fresh_front { 1.0 } else { 0.0 });
        swell(w, &c);
    } else {
        w.weather.mode = if w.config.rain_m_per_s > 0.0 {
            RainMode::Shower
        } else {
            RainMode::Clear
        };
        crate::water::prescribed_rain(w);
    }

    fog(w, &c, off);
    let rh = w.rh();
    if rh > w.weather.counts.max_rh {
        w.weather.counts.max_rh = rh;
    }
}

/// The closed budget's rain: end an event past its longest, force a downpour at the hard
/// limit, draw once a minute, and rain the running event's mode.
fn events(w: &mut World, c: &Climate, a: f64, f: f64) {
    let tick = w.tick;
    let seed = w.config.seed;

    if let Some(e) = w.weather.event {
        let longest = c.mode(e.mode).map_or(0.0, |m| m.longest_min) * 60.0 * f64::from(TICK_HZ);
        if (tick - e.start_tick) as f64 >= longest.round() {
            end(w, c, tick);
        }
    }

    let rh = w.rh();
    // The dry season holds the forced downpour off: with the valve kept, the closed cycle
    // refills to `rh_max` about 45 min after each downpour and a season never dries the
    // ground (DRY-SEASON's fixture). The water the season keeps aloft falls once it ends.
    let season = c.in_dry_season(&w.config.day, tick);
    if rh >= c.rh_max && !season && w.weather.mode != RainMode::Downpour {
        if w.weather.event.is_some() {
            end(w, c, tick);
        }
        start(w, c, RainMode::Downpour, Cause::Forced);
    } else if w.weather.event.is_none() && tick % TICKS_PER_MIN == 0 {
        let k = w.weather.cooling_c_per_h;
        let moist = w.moisture();
        // The dry season scales the natural hazards.
        let scale = c.rain_scale(&w.config.day, tick).max(0.0);
        let p = MODES.map(|mode| {
            let m = c.mode(mode).expect("a rain mode");
            let h = if reads_moisture(mode) { moist } else { rh };
            let mut l = hazard(m, h, a, k, f);
            if scale != 1.0 {
                l = if scale > 0.0 { l * scale } else { 0.0 };
            }
            1.0 - (-l / 60.0).exp()
        });
        let scale = p.iter().sum::<f64>().max(1.0);
        let u = draw(seed, tick / TICKS_PER_MIN, TAG_RAIN);
        let mut acc = 0.0;
        let chosen = (0..3).find(|&m| {
            acc += p[m] / scale;
            u < acc
        });
        if let Some(m) = chosen {
            if tick < w.weather.gap_until {
                w.weather.counts.gap_blocked[m] += 1;
            } else {
                start(w, c, MODES[m], Cause::Natural);
            }
        }
    }

    let Some(e) = w.weather.event else {
        return;
    };
    let params = *c.mode(e.mode).expect("an event has a mode");
    let fell = crate::water::deliver(w, params.rate_r0 * w.config.rain_m_per_s);
    let m = mode_index(e.mode).expect("an event has a mode");
    let q = ((w.weather.sun.phase * 4.0) as usize).min(3);
    w.weather.counts.rain_by_mode_m3[m] += fell;
    w.weather.counts.rain_by_quarter_m3[q] += fell;
    if let Some(ev) = w.weather.event.as_mut() {
        ev.volume_m3 += fell;
    }

    if e.mode == RainMode::Downpour && tick >= w.weather.next_strike {
        strike(w, c);
    }

    let opening = (c.opening_min_s * f64::from(TICK_HZ)).round() as u64;
    let early = e.cause == Cause::Opening && tick + 1 - e.start_tick < opening;
    let reading = if reads_moisture(e.mode) { w.moisture() } else { w.rh() };
    if !(fell > 0.0) || w.atmosphere_m3 <= 0.0 || (reading <= params.stop_rh && !early) {
        end(w, c, tick + 1);
    }
}

/// The stream swell: toward the gain times how hard it is raining, against a downpour,
/// while it rains; back toward nothing after.
fn swell(w: &mut World, c: &Climate) {
    let (target, tau) = match c.mode(w.weather.mode) {
        Some(m) if c.downpour.rate_r0 > 0.0 => (
            c.storm_stream_gain * (m.rate_r0 / c.downpour.rate_r0).clamp(0.0, 1.0),
            c.swell_rise_s,
        ),
        _ => (0.0, c.swell_fall_s),
    };
    let s = &mut w.weather.swell;
    *s += (target - *s) * (1.0 - (-DT / tau).exp());
}

/// Start an event of `mode` now.
fn start(w: &mut World, c: &Climate, mode: RainMode, cause: Cause) {
    let tick = w.tick;
    let rh = w.rh();
    let moisture = w.moisture();
    let wx = &mut w.weather;
    wx.event = Some(EventRecord {
        mode,
        cause,
        start_tick: tick,
        end_tick: tick,
        rh,
        moisture,
        temperature_c: wx.temp_c,
        day_phase: wx.sun.phase,
        since_previous_s: wx.last_end.map(|e| tick.saturating_sub(e) as f64 * DT),
        volume_m3: 0.0,
    });
    wx.mode = mode;
    match mode {
        RainMode::Drizzle => wx.counts.drizzles += 1,
        RainMode::Shower => wx.counts.showers += 1,
        RainMode::Downpour => {
            wx.counts.downpours += 1;
            wx.next_strike = tick + strike_wait(c, w.config.seed, tick);
        }
        RainMode::Clear => {}
    }
    if cause == Cause::Forced {
        w.weather.counts.forced += 1;
    }
    w.ledger.rain_events += 1;
}

/// End the running event; its rain stops before `end_tick`. Draws the dry gap.
fn end(w: &mut World, c: &Climate, end_tick: u64) {
    let seed = w.config.seed;
    let wx = &mut w.weather;
    let Some(mut e) = wx.event.take() else {
        return;
    };
    e.end_tick = end_tick;
    wx.last_finished = Some(e);
    wx.counts.events_finished += 1;
    wx.last_end = Some(end_tick);
    wx.mode = RainMode::Clear;
    let hz = f64::from(TICK_HZ);
    let lo = (c.dry_gap_min_s * hz).round() as u64;
    let hi = ((c.dry_gap_max_s * hz).round() as u64).max(lo);
    wx.gap_until = if hi > 0 {
        let span = (hi - lo + 1) as f64;
        end_tick + lo + ((draw(seed, end_tick, TAG_GAP) * span) as u64).min(hi - lo)
    } else {
        0
    };
}

/// Ticks to the next strike: exponential with the climate's mean, clamped.
fn strike_wait(c: &Climate, seed: u64, tick: u64) -> u64 {
    let u = draw(seed, tick, TAG_STRIKE_WAIT);
    let s = (-c.strike_mean_s * (1.0 - u).ln()).clamp(c.strike_min_s, c.strike_max_s);
    crate::ticks_at_least_one(s)
}

/// A strike now: the highest top of a few random sky-open columns.
fn strike(w: &mut World, c: &Climate) {
    let tick = w.tick;
    let seed = w.config.seed;
    let picks: [f64; STRIKE_COLUMNS] =
        core::array::from_fn(|i| draw(seed, tick, TAG_STRIKE_COLUMN + i as u64));
    if let Some((x, z)) = crate::water::strike_column(w, &picks) {
        w.weather.last_strike = Some(Strike {
            tick,
            x,
            z,
            seed: (draw(seed, tick, TAG_STRIKE_SEED) * f64::from(u32::MAX)) as u32,
        });
        w.weather.counts.strikes += 1;
    }
    w.weather.next_strike = tick + strike_wait(c, seed, tick);
}

/// Fog: a state, visual only.
fn fog(w: &mut World, c: &Climate, off: bool) {
    let rh = w.rh();
    let wx = &mut w.weather;
    let sun = wx.sun;
    let since_rise = (sun.phase - sun.rise).rem_euclid(1.0);
    let (dawn, burning) = if off {
        (false, false)
    } else {
        (
            since_rise >= 1.0 - c.fog_dawn_before || since_rise <= c.fog_dawn_after,
            since_rise >= c.fog_burn_after && since_rise <= (sun.set - sun.rise).rem_euclid(1.0),
        )
    };
    let heavy = matches!(wx.mode, RainMode::Shower | RainMode::Downpour);
    let density = ((rh - c.fog_clear_rh) / c.fog_span_rh.max(1e-9)).clamp(0.0, 1.0);
    let clearing = rh < c.fog_clear_rh || burning;
    let forming = !clearing
        && rh >= c.fog_form_rh
        && !heavy
        && (dawn || wx.cooling_c_per_h > c.fog_cooling_c_per_h);
    let (target, tau_min) = if clearing {
        (0.0, c.fog_clear_min)
    } else if forming {
        (density, c.fog_form_min)
    } else {
        (wx.fog.min(density), c.fog_form_min)
    };
    wx.fog += (target - wx.fog) * (1.0 - (-DT / (tau_min.max(1e-9) * 60.0)).exp());
}

/// The least cloud cover a rain mode falls from: drizzle 0.6, shower 0.75, downpour 0.9.
pub fn cloud_floor(c: &Climate, mode: RainMode) -> f64 {
    match mode {
        RainMode::Clear => 0.0,
        RainMode::Drizzle => c.cloud_floor_drizzle,
        RainMode::Shower => c.cloud_floor_shower,
        RainMode::Downpour => c.cloud_floor_downpour,
    }
}

/// The view the presenter draws.
pub(crate) fn view(w: &World) -> WeatherView {
    let wx = &w.weather;
    let c = &w.config.climate;
    let rh = w.rh();
    let rain = match wx.mode {
        RainMode::Clear => 0.0,
        mode if w.config.closed_water_budget => {
            c.mode(mode).map_or(0.0, |m| m.rate_r0) * w.config.rain_m_per_s
        }
        _ => w.config.rain_m_per_s,
    };
    let (civil_rise, civil_set) = w.config.day.civil_rise_set();
    let sun = match (w.config.day.mode, w.config.day.look) {
        (DayMode::Off, Look::Day) => Sun::at(&w.config.day, 0.5, civil_rise, civil_set),
        (DayMode::Off, Look::Night) => Sun::at(&w.config.day, 0.0, civil_rise, civil_set),
        _ => wx.sun,
    };
    // Rain falls from cloud: while it rains the cover has a floor by mode, so an
    // afternoon shower (when RH reads low) does not fall from a clear sky.
    let cloud = ((rh - c.cloud_start_rh) / c.cloud_span_rh)
        .clamp(0.0, 1.0)
        .max(cloud_floor(c, wx.mode));
    WeatherView {
        temperature_c: wx.temp_c as f32,
        rh: rh as f32,
        mode: wx.mode,
        rain_m_per_s: rain as f32,
        fog: wx.fog as f32,
        cloud_cover: cloud as f32,
        day_phase: sun.phase as f32,
        daylight: sun.daylight as f32,
        sun_elevation: sun.elevation as f32,
        last_strike: wx.last_strike,
    }
}

impl World {
    /// The weather state: the clock, the temperature, the event running and the counts.
    pub fn weather(&self) -> &Weather {
        &self.weather
    }

    /// The Cubarium year now ([`Climate::year`]).
    pub fn year(&self) -> Option<Year> {
        self.config.climate.year(&self.config.day, self.tick)
    }

    /// Whether the climate's dry season holds now ([`Climate::in_dry_season`]).
    pub fn in_dry_season(&self) -> bool {
        self.config.climate.in_dry_season(&self.config.day, self.tick)
    }

    /// Relative humidity: the atmosphere store over what the air holds at its
    /// temperature. Zero under the open budget, which has no store.
    pub fn rh(&self) -> f64 {
        let cap = self
            .config
            .climate
            .capacity_m3(self.ledger.expected_total(), self.weather.temp_c);
        if cap > 0.0 {
            self.atmosphere_m3 / cap
        } else {
            0.0
        }
    }

    /// The air's moisture `M`: the atmosphere store over what the air holds at the
    /// climate's **mean** temperature, so it moves only with the store, not with the day's
    /// warmth. What showers and downpours start and stop on ([`reads_moisture`]). Zero
    /// under the open budget.
    pub fn moisture(&self) -> f64 {
        let cap = self
            .config
            .climate
            .capacity_m3(self.ledger.expected_total(), self.config.climate.temperature_c);
        if cap > 0.0 {
            self.atmosphere_m3 / cap
        } else {
            0.0
        }
    }

    /// What the atmosphere-fed stream leaves aloft, cubic metres: RH
    /// [`Climate::stream_floor_rh`] of what the air holds at its temperature now.
    pub fn stream_floor_m3(&self) -> f64 {
        self.config
            .climate
            .stream_floor_rh
            * self
                .config
                .climate
                .capacity_m3(self.ledger.expected_total(), self.weather.temp_c)
    }

    /// The simulation's daylight, 0..1 — what plants and animals live by. Always 1 with
    /// the day clock off, whatever the picture shows.
    pub fn life_daylight(&self) -> f64 {
        if self.config.day.mode == DayMode::Off {
            1.0
        } else {
            self.weather.sun.daylight
        }
    }

    /// What a plant's maintenance and senescence are multiplied by now: `d + (1 − d) ·
    /// plant_night_factor` with `d` = [`World::life_daylight`] — 1 by day and with the
    /// clock off, `plant_night_factor` at full night, continuous through twilight.
    pub fn plant_night_scale(&self) -> f64 {
        let d = self.life_daylight();
        d + (1.0 - d) * self.config.day.plant_night_factor
    }

    /// Seconds to the **ecological dawn** while the sun is down, `None` while it is up and
    /// always with the clock off (there is no night). The dawn is the simulation's own
    /// sunrise: the cycle's under `cycle`, the date's under today's coupled `local` mode
    /// (`design/ecology-timescales-2026-10-02.md` 1b). What the plants' night budget is
    /// spread over.
    pub fn life_to_dawn_s(&self) -> Option<f64> {
        if self.config.day.mode == DayMode::Off {
            return None;
        }
        let sun = self.weather.sun;
        let day = (sun.set - sun.rise).rem_euclid(1.0);
        let since_rise = (sun.phase - sun.rise).rem_euclid(1.0);
        (since_rise > day).then(|| (1.0 - since_rise) * self.config.day.day_s())
    }

    /// The temperature plants and animals live by, °C. The climate's mean with the day
    /// clock off, whatever the picture shows.
    pub fn life_temperature_c(&self) -> f64 {
        if self.config.day.mode == DayMode::Off {
            self.config.climate.temperature_c
        } else {
            self.weather.temp_c
        }
    }

    /// The simulation's place in the day: 0 midnight, 0.5 noon (always noon with the
    /// clock off).
    pub fn day_phase(&self) -> f64 {
        if self.config.day.mode == DayMode::Off {
            0.5
        } else {
            self.weather.sun.phase
        }
    }

    /// Use `unix_s` as the wall clock for a `local` day clock instead of the machine's
    /// (`None` to go back to it). Not saved.
    pub fn set_wall_clock(&mut self, unix_s: Option<f64>) {
        self.weather.wall_clock_s = unix_s;
        self.weather.sun = self.config.day.sun(self.tick, unix_s);
    }

    /// Start an event of `mode` now, whatever is falling and whatever the gap: the
    /// development flag `--weather-force MODE`, for captures. It stops by the ordinary
    /// rules. `Clear` ends whatever is falling. Returns whether anything changed; an open
    /// world or an empty sky cannot rain.
    pub fn force_rain(&mut self, mode: RainMode) -> bool {
        let c = self.config.climate.at_speed(&self.config.day);
        if mode == RainMode::Clear {
            let was = self.weather.event.is_some();
            end(self, &c, self.tick);
            return was;
        }
        if !self.config.closed_water_budget || !(self.atmosphere_m3 > 0.0) {
            return false;
        }
        if self.weather.event.is_some() {
            end(self, &c, self.tick);
        }
        start(self, &c, mode, Cause::Commanded);
        true
    }

    /// Start a shower now, gap or no gap: the host's opening rain, so a fresh world is
    /// seen wet before anything is planted. It rains at least
    /// [`Climate::opening_min_s`] before its stop humidity may end it, then stops by the
    /// ordinary rules. Returns whether the world is now raining: an event already
    /// falling is left as it is and counts; an open world or an empty sky cannot rain.
    pub fn open_with_rain(&mut self) -> bool {
        if self.weather.event.is_some() {
            return true;
        }
        if !self.config.closed_water_budget || !(self.atmosphere_m3 > 0.0) {
            return false;
        }
        let c = self.config.climate;
        start(self, &c, RainMode::Shower, Cause::Opening);
        true
    }
}

#[cfg(test)]
#[path = "weather_tests.rs"]
mod tests;
