//! **The study recorder in a session**: `cubarium_observatory::vitals::Recorder` run on the
//! loop the way `voxel_census vitals=` runs it, so a row means the same thing in both
//! places (the training harness reads the census's `vitals.csv` and `plants_vitals.csv`).
//!
//! - Every tick: [`vitals::Recorder::observe`] (life events, 3-D path). Every simulated
//!   second (`tick % TICK_HZ == 0`): [`vitals::Recorder::sample`] (activity, the stuck
//!   history). Every simulated minute (`tick % (60 × TICK_HZ) == 0`):
//!   [`vitals::Recorder::minute`], one animal row per lineage and one plant row per species.
//! - At install the recorder starts fresh and writes the start minute's boundary row, as the
//!   census does at a founding or a load. Rows are never saved: a load starts them again.
//! - The rows cross to the hub as cells already in CSV form, turned into JSON (blank is
//!   `null`), and the hub keeps [`VitalsConfig::span_s`] of them ([`VitalsRing`]).
//!
//! Only `observe` and `sample` run on most ticks; the minute's snapshot and its conversion
//! run once a minute. The cost is reported as `Status::vitals_ms` (mean per tick) and
//! `Status::vitals_peak_ms` (the slowest tick).

use std::collections::VecDeque;
use std::time::Instant;

use cubarium_observatory::vitals::{self, PLANTS_COLUMNS, VITALS_COLUMNS};
use cubarium_voxel::TICK_HZ;
use cubarium_voxel_sim::Sim;
use serde_json::{Value, json};

use crate::params::VitalsConfig;

/// One minute's rows: each a JSON array in [`vitals::VITALS_HEADER`] or
/// [`vitals::PLANTS_HEADER`] order.
#[derive(Clone, Debug)]
pub(crate) struct VitalsMinute {
    pub minute: f64,
    pub animals: Vec<Value>,
    pub plants: Vec<Value>,
}

/// The recorder on the loop, and what it has cost.
pub(crate) struct VitalsRun {
    recorder: vitals::Recorder,
    /// Ticks between rows: a simulated minute ([`VitalsRun::MINUTE`]); tests shorten it.
    every: u64,
    /// Nanoseconds spent in the recorder over this world, and on its slowest tick (a
    /// minute's rows).
    pub ns: u64,
    pub peak_ns: u64,
}

impl VitalsRun {
    /// A simulated minute, in ticks.
    pub const MINUTE: u64 = 60 * TICK_HZ as u64;

    /// A fresh recorder on `sim` at its current tick, and its start minute's boundary row.
    pub fn new(sim: &Sim, config: &VitalsConfig, segment: u32, every: u64) -> (VitalsRun, VitalsMinute) {
        let at = Instant::now();
        let c = sim.world().config();
        let ring_m = f64::from(c.width) * c.voxel_m;
        let minute = Self::minute_of(sim.world().tick());
        let recorder = vitals::Recorder::new(config.options(), ring_m, segment, sim.fauna(), minute);
        let mut run = VitalsRun { recorder, every: every.max(1), ns: 0, peak_ns: 0 };
        let first = run.rows(sim, minute);
        run.ns += at.elapsed().as_nanos() as u64;
        (run, first)
    }

    fn minute_of(tick: u64) -> f64 {
        tick as f64 / Self::MINUTE as f64
    }

    /// After a tick: observe, sample on the second, and the minute's rows on the minute.
    pub fn tick(&mut self, sim: &Sim) -> Option<VitalsMinute> {
        let at = Instant::now();
        let tick = sim.world().tick();
        self.recorder.observe(sim.fauna(), tick);
        if tick % u64::from(TICK_HZ) == 0 {
            self.recorder.sample(sim.fauna(), tick as f64 / f64::from(TICK_HZ));
        }
        let out = (tick % self.every == 0).then(|| self.rows(sim, Self::minute_of(tick)));
        let ns = at.elapsed().as_nanos() as u64;
        self.ns += ns;
        self.peak_ns = self.peak_ns.max(ns);
        out
    }

    fn rows(&mut self, sim: &Sim, minute: f64) -> VitalsMinute {
        let (animals, plants) = self.recorder.minute(sim.flora(), sim.fauna(), minute);
        // The session keeps no life table: the dead bodies' histories go.
        self.recorder.prune_dead();
        VitalsMinute {
            minute,
            animals: animals.iter().map(|r| row(r.cells())).collect(),
            plants: plants.iter().map(|r| row(r.cells())).collect(),
        }
    }
}

/// CSV cells as a JSON row: blank is `null` (NA), a number a number, anything else text.
fn row(cells: Vec<String>) -> Value {
    Value::Array(
        cells
            .into_iter()
            .map(|c| {
                if c.is_empty() {
                    Value::Null
                } else if let Ok(i) = c.parse::<i64>() {
                    Value::from(i)
                } else if let Ok(f) = c.parse::<f64>() {
                    serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number)
                } else {
                    Value::String(c)
                }
            })
            .collect(),
    )
}

/// **The hub's vitals rows**: every minute's rows since the recorder started, less those
/// older than [`VitalsConfig::span_s`] before the newest minute.
pub(crate) struct VitalsRing {
    config: VitalsConfig,
    /// The minute the recorder started at (its boundary row's), for this world.
    started: Option<f64>,
    minutes: VecDeque<VitalsMinute>,
}

impl VitalsRing {
    pub fn new(config: VitalsConfig) -> VitalsRing {
        VitalsRing { config, started: None, minutes: VecDeque::new() }
    }

    pub fn clear(&mut self) {
        *self = VitalsRing::new(self.config);
    }

    pub fn push(&mut self, m: VitalsMinute) {
        self.started.get_or_insert(m.minute);
        let newest = m.minute;
        self.minutes.push_back(m);
        let span_min = self.config.span_s / 60.0;
        while self.minutes.front().is_some_and(|f| newest - f.minute > span_min + 1e-9) {
            self.minutes.pop_front();
        }
    }

    /// The `vitals` query: rows after minute `since` (all with `None`), both kinds, the
    /// columns with their meanings, and which minutes the ring holds.
    pub fn query(&self, since: Option<f64>) -> Value {
        let kept = self.minutes.iter().filter(|m| since.is_none_or(|s| m.minute > s));
        let (mut animals, mut plants) = (Vec::new(), Vec::new());
        for m in kept {
            animals.extend(m.animals.iter().cloned());
            plants.extend(m.plants.iter().cloned());
        }
        let columns = |docs: &[(&str, &str)]| -> Vec<Value> {
            docs.iter().map(|(name, doc)| json!({ "name": name, "doc": doc })).collect()
        };
        json!({
            "enabled": self.config.enabled,
            "span_s": self.config.span_s,
            "started_minute": self.started,
            "from_minute": self.minutes.front().map(|m| m.minute),
            "to_minute": self.minutes.back().map(|m| m.minute),
            "animal_columns": columns(VITALS_COLUMNS),
            "plant_columns": columns(PLANTS_COLUMNS),
            "animals": animals,
            "plants": plants,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minute(m: f64) -> VitalsMinute {
        VitalsMinute { minute: m, animals: vec![json!([m, "a"])], plants: vec![json!([m, "p"])] }
    }

    #[test]
    fn rows_older_than_the_span_are_dropped() {
        let config = VitalsConfig { span_s: 120.0, ..VitalsConfig::default() };
        let mut ring = VitalsRing::new(config);
        for m in 0..6 {
            ring.push(minute(f64::from(m)));
        }
        let q = ring.query(None);
        let kept: Vec<f64> = q["animals"].as_array().unwrap().iter().map(|r| r[0].as_f64().unwrap()).collect();
        let newest = 5.0;
        assert!(kept.iter().all(|m| newest - m <= config.span_s / 60.0), "{kept:?}");
        assert_eq!(kept.first().copied(), Some(newest - config.span_s / 60.0), "the span's own oldest minute stays");
        assert_eq!(q["from_minute"].as_f64(), kept.first().copied());
        assert_eq!(q["started_minute"].as_f64(), Some(0.0), "where the recorder began, kept after its rows go");
        assert_eq!(q["plants"].as_array().unwrap().len(), kept.len());
        ring.clear();
        assert!(ring.query(None)["animals"].as_array().unwrap().is_empty());
    }
}
