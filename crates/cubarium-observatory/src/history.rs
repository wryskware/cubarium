//! **The history ring**: population and environment series, one sample every
//! [`HistoryConfig::sample_s`] simulated seconds over the last [`HistoryConfig::span_s`],
//! so a dashboard opened late still sees the past.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::query::{Environment, Populations, WorldClock};

/// `[observatory]` in a host's config: how the history is sampled and how far back it
/// reaches. Both in simulated seconds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryConfig {
    /// Simulated seconds between samples.
    pub sample_s: f64,
    /// Simulated seconds of history kept; the oldest samples go first.
    pub span_s: f64,
}

impl Default for HistoryConfig {
    fn default() -> HistoryConfig {
        HistoryConfig { sample_s: 10.0, span_s: 6.0 * 3600.0 }
    }
}

impl HistoryConfig {
    /// Samples the ring holds: `span_s / sample_s`, at least one.
    pub fn capacity(&self) -> usize {
        if self.sample_s > 0.0 && self.span_s.is_finite() {
            ((self.span_s / self.sample_s).ceil() as usize).max(1)
        } else {
            1
        }
    }
}

/// One sample: counts and stocks, never positions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub tick: u64,
    pub sim_s: f64,
    /// The world's day and hour then.
    pub day: i64,
    pub hour: f64,
    /// Per lineage, in [`cubarium_voxel_fauna::Founder::ALL`] order: adults, juveniles
    /// (pups and riders included), eggs.
    pub adults: Vec<u32>,
    pub juveniles: Vec<u32>,
    pub eggs: Vec<u64>,
    /// Per plant species, in their fixed order: stands and tissue organic matter.
    pub stands: Vec<u32>,
    pub plant_organic: Vec<f64>,
    pub temperature_c: f64,
    pub rh: f64,
    pub rain_mm_per_h: f64,
    pub daylight: f64,
    pub atmosphere_m3: f64,
    pub aquifer_m3: f64,
    pub standing_m3: f64,
    pub soil_m3: f64,
}

impl Sample {
    pub fn of(clock: &WorldClock, pops: &Populations, env: &Environment) -> Sample {
        Sample {
            tick: clock.tick,
            sim_s: clock.sim_elapsed_s,
            day: clock.day,
            hour: clock.hour,
            adults: pops.animals.iter().map(|a| a.adults).collect(),
            juveniles: pops.animals.iter().map(|a| a.juveniles + a.pups + a.riders).collect(),
            eggs: pops.animals.iter().map(|a| a.eggs).collect(),
            stands: pops.plants.iter().map(|p| p.stands).collect(),
            plant_organic: pops.plants.iter().map(|p| p.organic).collect(),
            temperature_c: env.temperature_c,
            rh: env.rh,
            rain_mm_per_h: env.rain_mm_per_h,
            daylight: env.daylight,
            atmosphere_m3: env.water.atmosphere_m3,
            aquifer_m3: env.water.aquifer_m3,
            standing_m3: env.water.standing_m3,
            soil_m3: env.water.soil_m3,
        }
    }
}

/// The ring itself.
#[derive(Clone, Debug)]
pub struct History {
    config: HistoryConfig,
    samples: VecDeque<Sample>,
}

impl History {
    pub fn new(config: HistoryConfig) -> History {
        History { config, samples: VecDeque::with_capacity(config.capacity().min(1 << 16)) }
    }

    pub fn config(&self) -> HistoryConfig {
        self.config
    }

    /// Whether a sample at `sim_s` is due: the first one, one at least `sample_s` after
    /// the newest, or one from before the newest (a world loaded or replaced under the
    /// host: the ring starts again).
    pub fn due(&self, sim_s: f64) -> bool {
        match self.samples.back() {
            None => true,
            Some(last) => sim_s < last.sim_s || sim_s - last.sim_s >= self.config.sample_s - 1e-9,
        }
    }

    /// Add `sample` if one is due; returns whether it was taken.
    pub fn offer(&mut self, sample: Sample) -> bool {
        if !self.due(sample.sim_s) {
            return false;
        }
        if self.samples.back().is_some_and(|last| sample.sim_s < last.sim_s) {
            self.samples.clear();
        }
        self.samples.push_back(sample);
        while self.samples.len() > self.config.capacity() {
            self.samples.pop_front();
        }
        true
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// The samples after `tick` (all of them for `None`), oldest first.
    pub fn since(&self, tick: Option<u64>) -> impl Iterator<Item = &Sample> {
        self.samples.iter().filter(move |s| tick.is_none_or(|t| s.tick > t))
    }
}
