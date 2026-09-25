//! The weather `--sink gpu` draws: the world's own [`WeatherView`], or a scripted preview
//! (package WX2, `design/handoffs/voxel-weather-2026-09-24.md`).
//!
//! The preview (`--weather-preview`) is a **capture-only dev tool**: it overrides the
//! view's weather with a loop that runs on the frame clock, so every weather state can be
//! drawn and captured without waiting on the simulation. The world under it goes on as it
//! is; only the picture's weather is scripted.

use std::str::FromStr;

use cubarium_gpu::weather::{Rain, Strike, Weather};
use cubarium_voxel::weather::{RainMode, WeatherView};

/// The renderer's copy of the view's weather.
pub fn weather_of(v: &WeatherView) -> Weather {
    Weather {
        rain: match v.mode {
            RainMode::Clear => Rain::Clear,
            RainMode::Drizzle => Rain::Drizzle,
            RainMode::Shower => Rain::Shower,
            RainMode::Downpour => Rain::Downpour,
        },
        rain_m_per_s: v.rain_m_per_s,
        fog: v.fog,
        cloud_cover: v.cloud_cover,
        day_phase: v.day_phase,
        daylight: v.daylight,
        sun_elevation: v.sun_elevation,
        last_strike: v.last_strike.map(|s| Strike {
            tick: s.tick,
            x: s.x,
            z: s.z,
            seed: s.seed,
        }),
    }
}

/// A scripted weather loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WeatherPreview {
    /// About 90 s: dawn fog, a clear morning, clouds building, a downpour with strikes,
    /// clearing, dusk, night drizzle, a clear night, and round to the dawn again.
    Loop,
    /// 60 s: one whole day and night with no rain, the clouds coming and going.
    Day,
    /// A still: this day phase and cloud cover, no rain, no fog (`fixed:PHASE[,CLOUD]`).
    Fixed { phase: f32, cloud: f32 },
}

impl FromStr for WeatherPreview {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<WeatherPreview> {
        match s {
            "loop" => Ok(WeatherPreview::Loop),
            "day" => Ok(WeatherPreview::Day),
            _ => {
                let bad = || anyhow::anyhow!("--weather-preview is `loop`, `day` or `fixed:PHASE[,CLOUD]`, not {s:?}");
                let rest = s.strip_prefix("fixed:").ok_or_else(bad)?;
                let mut it = rest.split(',').map(str::parse::<f32>);
                let phase = it.next().ok_or_else(bad)?.map_err(|_| bad())?;
                let cloud = it.next().transpose().map_err(|_| bad())?.unwrap_or(0.0);
                Ok(WeatherPreview::Fixed { phase, cloud })
            }
        }
    }
}

/// The preview's day clock: the `[world.day]` cycle's defaults (two thirds of it
/// daylight, the sun's elevation a sine peaking at 70° at noon).
const DAYLIGHT: f32 = 0.67;
const PEAK_DEG: f32 = 70.0;

/// The sun's elevation (radians) and the daylight at `phase`, as the day clock has them:
/// a sine over the day crossing zero at sunrise and sunset, negative through the night,
/// and the daylight a smoothstep of it over civil twilight (−6°..+6°).
pub fn day_clock(phase: f32) -> (f32, f32) {
    let rise = 0.5 - DAYLIGHT / 2.0;
    let set = 0.5 + DAYLIGHT / 2.0;
    let p = phase.rem_euclid(1.0);
    let el = if (rise..set).contains(&p) {
        (std::f32::consts::PI * (p - rise) / DAYLIGHT).sin()
    } else {
        let since = (p - set).rem_euclid(1.0);
        -(std::f32::consts::PI * since / (1.0 - DAYLIGHT)).sin()
    } * PEAK_DEG.to_radians();
    let t = ((el.to_degrees() + 6.0) / 12.0).clamp(0.0, 1.0);
    (el, t * t * (3.0 - 2.0 * t))
}

/// One key of a script: from `t` seconds on, heading to the next key linearly.
struct Key {
    t: f64,
    /// The day phase, unwrapped (it only grows through a loop).
    phase: f64,
    cloud: f32,
    fog: f32,
    rain: RainMode,
}

const fn key(t: f64, phase: f64, cloud: f32, fog: f32, rain: RainMode) -> Key {
    Key { t, phase, cloud, fog, rain }
}

use RainMode::{Clear, Downpour, Drizzle};

/// The loop, 90 s. The last key is the first a whole day later, so it wraps seamlessly.
const LOOP: [Key; 11] = [
    key(0.0, 0.17, 0.25, 0.8, Clear),    // dawn fog
    key(10.0, 0.23, 0.2, 0.45, Clear),   // clear morning
    key(20.0, 0.38, 0.12, 0.0, Clear),   // clouds building
    key(32.0, 0.54, 0.95, 0.0, Downpour), // a downpour with strikes
    key(48.0, 0.62, 1.0, 0.0, Clear),    // clearing
    key(58.0, 0.74, 0.3, 0.0, Clear),    // dusk
    key(68.0, 0.86, 0.35, 0.0, Drizzle), // night drizzle
    key(76.0, 0.95, 0.7, 0.1, Drizzle),
    key(82.0, 1.02, 0.6, 0.2, Clear),    // a clear night
    key(86.0, 1.10, 0.25, 0.5, Clear),
    key(90.0, 1.17, 0.25, 0.8, Clear),
];

/// The day, 60 s: sunrise a little after the start.
const DAY: [Key; 7] = [
    key(0.0, 0.12, 0.25, 0.0, Clear),
    key(10.0, 0.286, 0.1, 0.0, Clear),
    key(22.0, 0.486, 0.35, 0.0, Clear),
    key(32.0, 0.653, 0.55, 0.0, Clear),
    key(42.0, 0.82, 0.3, 0.0, Clear),
    key(52.0, 0.986, 0.2, 0.0, Clear),
    key(60.0, 1.12, 0.25, 0.0, Clear),
];

/// Seconds into the loop of each lightning strike, all inside the downpour.
const STRIKES: [f64; 6] = [34.5, 37.0, 38.2, 41.5, 44.0, 46.5];

/// The rain rate a mode falls at in the preview, m/s (R₀ = 3.5e-5, the brief's starting
/// multiples).
fn rate(mode: RainMode) -> f32 {
    3.5e-5
        * match mode {
            Clear => 0.0,
            Drizzle => 0.1,
            RainMode::Shower => 1.0,
            Downpour => 1.5,
        }
}

impl WeatherPreview {
    /// Seconds before it repeats.
    pub fn length_s(self) -> f64 {
        self.keys().last().map_or(1.0, |k| k.t)
    }

    fn keys(self) -> &'static [Key] {
        match self {
            WeatherPreview::Loop => &LOOP,
            WeatherPreview::Day | WeatherPreview::Fixed { .. } => &DAY,
        }
    }

    /// The weather `t` seconds into the preview (wrapping), whose first second was sim
    /// tick `start_tick` at `tick_hz` ticks a second (the strikes are stamped in ticks).
    pub fn at(self, t: f64, start_tick: u64, tick_hz: f64) -> WeatherView {
        if let WeatherPreview::Fixed { phase, cloud } = self {
            let (sun_elevation, daylight) = day_clock(phase);
            return WeatherView {
                cloud_cover: cloud.clamp(0.0, 1.0),
                day_phase: phase.rem_euclid(1.0),
                daylight,
                sun_elevation,
                ..WeatherView::CLEAR_NOON
            };
        }
        let keys = self.keys();
        let len = self.length_s();
        let lap = (t / len).floor();
        let t = t - lap * len;
        let i = keys.iter().rposition(|k| k.t <= t).unwrap_or(0).min(keys.len() - 2);
        let (a, b) = (&keys[i], &keys[i + 1]);
        let f = ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0);
        let phase = (a.phase + (b.phase - a.phase) * f).rem_euclid(1.0) as f32;
        let (sun_elevation, daylight) = day_clock(phase);
        let lerp = |x: f32, y: f32| x + (y - x) * f as f32;
        let mode = a.rain;
        let last_strike = if self == WeatherPreview::Loop {
            STRIKES.iter().rposition(|&s| s <= t).map(|k| {
                let at = lap * len + STRIKES[k];
                preview_strike(k as u32, start_tick + (at * tick_hz).round() as u64)
            })
        } else {
            None
        };
        let mut cloud = lerp(a.cloud, b.cloud);
        if mode == Downpour {
            cloud = cloud.max(0.9);
        }
        WeatherView {
            temperature_c: 26.0,
            rh: 0.4 + 0.6 * cloud,
            mode,
            rain_m_per_s: rate(mode),
            fog: lerp(a.fog, b.fog),
            cloud_cover: cloud,
            day_phase: phase,
            daylight,
            sun_elevation,
            last_strike,
        }
    }
}

/// A preview strike: `k` picks its column and its bolt.
fn preview_strike(k: u32, tick: u64) -> cubarium_voxel::weather::Strike {
    let h = k.wrapping_mul(0x9E37_79B9) ^ 0x5bd1_e995;
    cubarium_voxel::weather::Strike {
        tick,
        x: 40 + (h % 180),
        z: 10 + (h >> 8) % 30,
        seed: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_clock_crosses_the_horizon_at_sunrise_and_peaks_at_noon() {
        let (el, dl) = day_clock(0.5 - DAYLIGHT / 2.0);
        assert!(el.abs() < 1e-4 && (dl - 0.5).abs() < 1e-3);
        let (el, dl) = day_clock(0.5);
        assert!((el.to_degrees() - PEAK_DEG).abs() < 1e-3 && dl == 1.0);
        let (el, dl) = day_clock(0.0);
        assert!(el < 0.0 && dl == 0.0);
    }

    #[test]
    fn previews_wrap_without_a_jump() {
        for p in [WeatherPreview::Loop, WeatherPreview::Day] {
            let len = p.length_s();
            let (a, b) = (p.at(len - 1e-4, 0, 20.0), p.at(len, 0, 20.0));
            let d = (a.day_phase - b.day_phase + 0.5).rem_euclid(1.0) - 0.5;
            assert!(d.abs() < 1e-3, "{p:?} phase");
            assert!((a.cloud_cover - b.cloud_cover).abs() < 1e-3, "{p:?} cloud");
            assert!((a.fog - b.fog).abs() < 1e-3, "{p:?} fog");
        }
    }

    #[test]
    fn the_loop_storms_with_strikes_and_the_day_stays_dry() {
        let storm = WeatherPreview::Loop.at(40.0, 100, 20.0);
        assert_eq!(storm.mode, RainMode::Downpour);
        let s = storm.last_strike.expect("a strike by 40 s");
        assert!(s.tick > 100 && s.tick <= 100 + 800);
        assert!((0..600).all(|k| WeatherPreview::Day.at(k as f64 * 0.1, 0, 20.0).mode == Clear));
    }
}
