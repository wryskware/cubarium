//! Fixed observation neighbourhoods around the three care targets, and the global water
//! exposure summary beside them.
//!
//! The regions are frozen at construction and never re-chosen, so a wetter site cannot be
//! selected after the fact. They are grown exactly the way the care study's local observer grows
//! them — the target's own cell, expanded [`HOPS`] rings across the field graph, seams included —
//! so a number here is comparable with one from that study rather than merely similar.
//!
//! Two different things are recorded about water, and they are deliberately not mixed:
//!
//! * **sampled** depth is the stock at this instant, in depth units;
//! * **tick-integral** is `Σ depth` over every completed tick, in depth·ticks. It is cell-time,
//!   not an amount of water, and a reader must not compare it against a rainfall total.
//!
//! Flooding is read from the world's own `water.flood` threshold: growth is scaled by
//! `max(0, 1 − (w − flood)/flood)`, so `w ≥ flood` is where drowning begins and `w ≥ 2·flood` is
//! where producer growth stops entirely (`crate::fields`). Counting cells in those two states is
//! an exposure measure, not a claim about what grew.

use anyhow::{Context, Result};
use cubarium_core::{CareTarget, WorldState};
use cubarium_surface::{CELL_COUNT, CellId, FieldGraph};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Graph rings grown around each target cell. The care study's own region width.
pub const HOPS: usize = 3;

/// A compensated running total, so a 72-hour tick integral does not round its own tail away.
#[derive(Clone, Copy, Default)]
struct Integral {
    raw: f64,
    correction: f64,
}

impl Integral {
    fn add(&mut self, x: f64) {
        let next = self.raw + x;
        self.correction += if self.raw.abs() >= x.abs() {
            (self.raw - next) + x
        } else {
            (x - next) + self.raw
        };
        self.raw = next;
    }

    fn value(self) -> f64 {
        self.raw + self.correction
    }
}

struct Region {
    target: CareTarget,
    cells: Vec<usize>,
    water_integral: Integral,
    flooded_cell_ticks: u64,
    drowned_cell_ticks: u64,
}

impl Region {
    fn stocks(&self, s: &WorldState) -> (f64, f64, f64, f64, f64) {
        let mut water = 0.0;
        let mut producer = 0.0;
        let mut fruit = 0.0;
        let mut detritus = 0.0;
        let mut nutrient = 0.0;
        for &i in &self.cells {
            water += s.fields.w[i];
            producer += s.fields.p[i];
            fruit += s.fields.f[i];
            detritus += s.fields.d[i];
            nutrient += s.fields.n[i];
        }
        (water, producer, fruit, detritus, nutrient)
    }
}

/// The frozen regions plus the global exposure totals, advanced once per completed tick.
pub struct Regions {
    regions: Vec<Region>,
    flood: f64,
    opening_tick: u64,
    last_tick: u64,
    ticks: u64,
    global_water_integral: Integral,
    global_flooded_cell_ticks: u64,
    global_drowned_cell_ticks: u64,
}

impl Regions {
    pub fn new(state: &WorldState, targets: &[CareTarget]) -> Result<Self> {
        let graph = FieldGraph::new();
        let mut regions = Vec::new();
        for &target in targets {
            let mut cells = BTreeSet::from([target.resolve().context("invalid ambient target")?]);
            for _ in 0..HOPS {
                let neighbors: Vec<CellId> = cells
                    .iter()
                    .flat_map(|c| graph.neighbors(*c).iter().flatten().copied())
                    .collect();
                cells.extend(neighbors);
            }
            regions.push(Region {
                target,
                cells: cells.iter().map(|c| c.index()).collect(),
                water_integral: Integral::default(),
                flooded_cell_ticks: 0,
                drowned_cell_ticks: 0,
            });
        }
        Ok(Self {
            regions,
            flood: state.config.water.flood,
            opening_tick: state.tick,
            last_tick: state.tick,
            ticks: 0,
            global_water_integral: Integral::default(),
            global_flooded_cell_ticks: 0,
            global_drowned_cell_ticks: 0,
        })
    }

    /// Accumulate one **completed** tick. Strictly sequential: a skipped or repeated tick is an
    /// error rather than a quietly wrong integral.
    pub fn observe(&mut self, s: &WorldState) -> Result<()> {
        anyhow::ensure!(
            s.tick == self.last_tick + 1,
            "ambient region observer skipped a tick: {} after {}",
            s.tick,
            self.last_tick
        );
        self.last_tick = s.tick;
        self.ticks += 1;
        let (flood, drowned) = (self.flood, 2.0 * self.flood);
        for region in &mut self.regions {
            let mut water = 0.0;
            for &i in &region.cells {
                let w = s.fields.w[i];
                water += w;
                if w >= flood {
                    region.flooded_cell_ticks += 1;
                    if w >= drowned {
                        region.drowned_cell_ticks += 1;
                    }
                }
            }
            region.water_integral.add(water);
        }
        let mut total = 0.0;
        for i in 0..CELL_COUNT {
            let w = s.fields.w[i];
            total += w;
            if w >= flood {
                self.global_flooded_cell_ticks += 1;
                if w >= drowned {
                    self.global_drowned_cell_ticks += 1;
                }
            }
        }
        self.global_water_integral.add(total);
        Ok(())
    }

    /// Global flooding exposure at this instant, in cells.
    fn flooded_now(&self, s: &WorldState) -> (u32, u32) {
        let (flood, drowned) = (self.flood, 2.0 * self.flood);
        let mut a = 0;
        let mut b = 0;
        for i in 0..CELL_COUNT {
            let w = s.fields.w[i];
            if w >= flood {
                a += 1;
                if w >= drowned {
                    b += 1;
                }
            }
        }
        (a, b)
    }

    /// One sample. Must be taken at the tick last observed, so a report can never describe a
    /// world the integral has not caught up with.
    pub fn sample(&self, s: &WorldState) -> Result<Value> {
        anyhow::ensure!(
            s.tick == self.last_tick,
            "ambient region sample at tick {} but observations stand at {}",
            s.tick,
            self.last_tick
        );
        let (flooded, drowned) = self.flooded_now(s);
        let regions: Vec<Value> = self
            .regions
            .iter()
            .map(|r| {
                let (water, producer, fruit, detritus, nutrient) = r.stocks(s);
                json!({
                    "target": r.target,
                    "cells": r.cells.len(),
                    "water_sampled": water,
                    "water_tick_integral": r.water_integral.value(),
                    "producer": producer,
                    "fruit": fruit,
                    "detritus": detritus,
                    "nutrient": nutrient,
                    "flooded_cell_ticks": r.flooded_cell_ticks,
                    "drowned_cell_ticks": r.drowned_cell_ticks,
                })
            })
            .collect();
        Ok(json!({
            "observed_ticks": self.ticks,
            "opening_tick": self.opening_tick,
            "flood_threshold": self.flood,
            "global_water_tick_integral": self.global_water_integral.value(),
            "global_flooded_cells_now": flooded,
            "global_drowned_cells_now": drowned,
            "global_flooded_cell_ticks": self.global_flooded_cell_ticks,
            "global_drowned_cell_ticks": self.global_drowned_cell_ticks,
            "targets": regions,
        }))
    }

    /// The frozen region sizes, for the manifest: identical in every arm by construction.
    pub fn shape(&self) -> Vec<usize> {
        self.regions.iter().map(|r| r.cells.len()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::{World, WorldConfig};

    fn targets() -> [CareTarget; 2] {
        [
            CareTarget { face: 0, u: 32.0, v: 48.0 },
            // On the side seam, so the region has to cross a face boundary.
            CareTarget { face: 0, u: 63.5, v: 48.0 },
        ]
    }

    #[test]
    fn regions_are_frozen_at_construction_and_cross_a_seam() {
        let world = World::new(WorldConfig::default()).unwrap();
        let regions = Regions::new(&world.state, &targets()).unwrap();
        let shape = regions.shape();
        assert_eq!(shape.len(), 2);
        // A 3-hop interior ring is the cell plus 24 others on one face; a seam target reaches
        // the same count across two faces. Both are fixed, and neither counts a cell twice.
        assert!(shape.iter().all(|n| *n == 25), "{shape:?}");
        let cells: BTreeSet<usize> = regions.regions[1].cells.iter().copied().collect();
        assert_eq!(cells.len(), regions.regions[1].cells.len(), "a cell was counted twice");
    }

    #[test]
    fn the_integral_is_cell_time_and_the_sample_is_a_stock() {
        let mut cfg = WorldConfig::default();
        cfg.water.rain_rate = 0.0;
        cfg.water.flow = 0.0;
        cfg.water.evap = 0.0;
        let mut world = World::new(cfg).unwrap();
        world.state.fields.w.fill(0.0);
        let mut regions = Regions::new(&world.state, &targets()).unwrap();
        // A sample before any observation is legal and reports zero elapsed time.
        let opening = regions.sample(&world.state).unwrap();
        assert_eq!(opening["observed_ticks"], 0);
        assert_eq!(opening["global_water_tick_integral"], 0.0);

        // One unit of standing water in every cell of the first region, held still.
        for &i in &regions.regions[0].cells.clone() {
            world.state.fields.w[i] = 1.0;
        }
        let held: f64 = world.state.fields.w.iter().sum();
        for _ in 0..10 {
            world.step();
            regions.observe(&world.state).unwrap();
        }
        let s = regions.sample(&world.state).unwrap();
        assert_eq!(s["observed_ticks"], 10);
        // The stock is what is there; the integral is ten times it. Same water, different units.
        let sampled = s["targets"][0]["water_sampled"].as_f64().unwrap();
        let integral = s["targets"][0]["water_tick_integral"].as_f64().unwrap();
        assert!((sampled - held).abs() < 1e-9, "{sampled} vs {held}");
        assert!((integral - 10.0 * held).abs() < 1e-9, "{integral}");
        assert!((s["global_water_tick_integral"].as_f64().unwrap() - 10.0 * held).abs() < 1e-9);
    }

    #[test]
    fn flooding_exposure_counts_the_worlds_own_two_thresholds() {
        let mut cfg = WorldConfig::default();
        cfg.water.rain_rate = 0.0;
        cfg.water.flow = 0.0;
        cfg.water.evap = 0.0;
        let flood = cfg.water.flood;
        let mut world = World::new(cfg).unwrap();
        world.state.fields.w.fill(0.0);
        let mut regions = Regions::new(&world.state, &targets()).unwrap();
        let cells = regions.regions[0].cells.clone();
        // Just under the threshold, exactly on it, and past twice it.
        world.state.fields.w[cells[0]] = flood * 0.999;
        world.state.fields.w[cells[1]] = flood;
        world.state.fields.w[cells[2]] = flood * 2.0;
        world.step();
        regions.observe(&world.state).unwrap();
        let s = regions.sample(&world.state).unwrap();
        assert_eq!(s["global_flooded_cells_now"], 2, "on the threshold counts, under it does not");
        assert_eq!(s["global_drowned_cells_now"], 1);
        assert_eq!(s["targets"][0]["flooded_cell_ticks"], 2);
        assert_eq!(s["targets"][0]["drowned_cell_ticks"], 1);
        assert_eq!(s["targets"][1]["flooded_cell_ticks"], 0, "the other region is untouched");
    }

    #[test]
    fn a_skipped_or_repeated_tick_is_refused_and_a_stale_sample_too() {
        let mut world = World::new(WorldConfig::default()).unwrap();
        let mut regions = Regions::new(&world.state, &targets()).unwrap();
        world.step();
        regions.observe(&world.state).unwrap();
        assert!(regions.observe(&world.state).is_err(), "a repeated tick must be refused");
        world.step();
        world.step();
        assert!(regions.observe(&world.state).is_err(), "a skipped tick must be refused");
        // And a sample may not describe a world the integral has not reached.
        assert!(regions.sample(&world.state).is_err());
    }
}
