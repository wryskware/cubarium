//! **The plant clock**: what the day asks of a plant at a world time, from the world's own
//! day clock ([`cubarium_voxel::weather::DayClock::sun`], `weather.rs:300`) and the sim's
//! own rules for it (`World::life_daylight`, `plant_night_scale`, `life_to_dawn_s`,
//! `weather.rs:1597–1625`; `step::Climate::of`, `step.rs:2625`).
//!
//! **Aggregation.** The temperature is the climate's mean plus the diurnal curve
//! ([`cubarium_voxel::weather::diurnal_shape`], `weather.rs:1198`); the day-to-day drift,
//! fronts and rain cooling are left out (mean-zero or rare terms on maintenance's Q10:
//! `E[2^(x/10)]` for the drift's 1 °C sd is 1.002).

use cubarium_voxel::weather::{DayClock, DayMode, diurnal_shape};

/// One plant step's clock, the fields of `step::Climate` (`step.rs:2604`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    pub daylight: f64,
    pub upkeep: f64,
    pub night: f64,
    pub to_dawn_s: Option<f64>,
    pub dt: f64,
}

/// The world's day clock and the climate numbers a plant reads.
#[derive(Clone, Debug)]
pub struct DayModel {
    pub day: DayClock,
    pub diurnal_c: f64,
    pub upkeep_q10: f64,
}

impl DayModel {
    /// From a resolved world config and the plant layer's Q10.
    pub fn of(world: &cubarium_voxel::Config, upkeep_q10: f64) -> DayModel {
        DayModel {
            day: world.day.clone(),
            diurnal_c: world.climate.diurnal_c,
            upkeep_q10,
        }
    }

    /// The day's length, seconds.
    pub fn day_s(&self) -> f64 {
        self.day.day_s()
    }

    /// The clock at world time `t_s` seconds since tick 0, for a step of `dt` seconds.
    pub fn at(&self, t_s: f64, dt: f64) -> Clock {
        let tick = (t_s / cubarium_voxel::DT).round().max(0.0) as u64;
        let sun = self.day.sun(tick, None);
        let off = self.day.mode == DayMode::Off;
        let daylight = if off { 1.0 } else { sun.daylight };
        let night = daylight + (1.0 - daylight) * self.day.plant_night_factor;
        let to_dawn_s = if off {
            None
        } else {
            let day = (sun.set - sun.rise).rem_euclid(1.0);
            let since_rise = (sun.phase - sun.rise).rem_euclid(1.0);
            (since_rise > day).then(|| (1.0 - since_rise) * self.day.day_s())
        };
        let dtemp = if off {
            0.0
        } else {
            self.diurnal_c * diurnal_shape(sun.phase, sun.rise, sun.set, self.day.temperature_peak_fraction)
        };
        Clock {
            daylight,
            upkeep: self.upkeep_q10.powf(dtemp / 10.0),
            night,
            to_dawn_s,
            dt,
        }
    }
}
