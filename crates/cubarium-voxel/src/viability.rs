//! Is this world's water cycle one a habitat could live in?
//!
//! A closed budget conserves water whatever it does, so conservation says nothing about
//! whether the cycle *works*. A world can hold every drop it started with and still lock
//! dry: the water settles where no return flow can lift it, the store never reaches its
//! trigger, and the last shower was the last shower. That is a feature in the game —
//! reserves a player brings back with tech — and a reason to reject or hand-water a
//! generated world for the ambient piece (Wrysk, 2026-09-20).
//!
//! This module **reports**, it does not reject. Three questions, measured over a window
//! after a warm-up:
//!
//! 1. Is the in-world stored water bounded, or drifting one way?
//! 2. Did at least `min_showers` showers actually run?
//! 3. What fraction of the soil columns hold pore water inside the bands the seeded
//!    species need to establish?
//!
//! The bands come from the caller, not from here: this crate does not know what a plant
//! is, and the flora crate's `establish_pore_min` is the number to pass in.

use std::fmt;

use crate::{Material, World};

/// One species' pore window, as the caller's plant layer states it. `min` is the flora
/// crate's `establish_pore_min`; `max` is whatever the caller counts as too wet — 1.0
/// when the only gate is a floor.
#[derive(Clone, Debug, PartialEq)]
pub struct PoreBand {
    pub name: String,
    pub min: f64,
    pub max: f64,
}

impl PoreBand {
    pub fn new(name: impl Into<String>, min: f64, max: f64) -> PoreBand {
        PoreBand {
            name: name.into(),
            min,
            max,
        }
    }

    pub fn holds(&self, pore: f64) -> bool {
        pore >= self.min && pore <= self.max
    }
}

/// What [`measure`] runs and what it calls viable.
#[derive(Clone, Debug, PartialEq)]
pub struct ViabilitySpec {
    /// Ticks stepped before any sample is taken: the cycle's transient.
    pub warmup_ticks: u64,
    /// Ticks the window covers.
    pub window_ticks: u64,
    /// Sample the stores every this many ticks. At least one.
    pub sample_every: u64,
    /// Showers the window must contain.
    pub min_showers: u64,
    /// How far the stored water may drift across the window and still count as bounded,
    /// as a fraction of the world's total water: the second half's mean against the
    /// first half's. A bounded oscillation has a drift near zero however wide it swings;
    /// a world locking dry has a one-way drift however smooth it looks.
    pub drift_tolerance: f64,
    /// How many soil voxels below a column's surface face are averaged for its pore
    /// reading. A proxy for the flora layer's root box, which this crate cannot see.
    pub root_cells: u32,
    /// The fraction of soil columns a band must hold for that species to pass.
    pub min_column_fraction: f64,
    /// The seeded species' establishment bands.
    pub bands: Vec<PoreBand>,
    /// Threads for the stepping. One is the serial run.
    pub threads: usize,
}

impl Default for ViabilitySpec {
    fn default() -> ViabilitySpec {
        ViabilitySpec {
            warmup_ticks: 0,
            // Twenty simulated minutes, sampled every ten seconds.
            window_ticks: 20 * 60 * crate::TICK_HZ as u64,
            sample_every: 10 * crate::TICK_HZ as u64,
            min_showers: 3,
            drift_tolerance: 0.02,
            root_cells: 2,
            min_column_fraction: 0.05,
            bands: Vec::new(),
            threads: 1,
        }
    }
}

/// One band's result.
#[derive(Clone, Debug, PartialEq)]
pub struct BandResult {
    pub name: String,
    /// Soil columns whose mean pore reading is inside the band.
    pub columns: usize,
    /// That count over the soil columns there are.
    pub fraction: f64,
    pub passed: bool,
}

/// What the window showed.
#[derive(Clone, Debug, PartialEq)]
pub struct Viability {
    pub ticks: u64,
    pub samples: usize,
    pub stored_min: f64,
    pub stored_max: f64,
    pub stored_drift: f64,
    pub bounded: bool,
    pub atmosphere_min: f64,
    pub atmosphere_max: f64,
    pub showers: u64,
    pub showers_enough: bool,
    pub soil_columns: usize,
    pub bands: Vec<BandResult>,
    /// The closed-budget conservation residual at the end of the window. A viability
    /// report that came out of a leaking world is worthless, so it is in the report.
    pub residual: f64,
    /// Every question answered yes. **Not** a rejection: nothing acts on this yet.
    pub viable: bool,
}

impl fmt::Display for Viability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "water cycle {}: stored {:.2}–{:.2} m3 (drift {:+.2}% {}), \
             atmosphere {:.2}–{:.2} m3, {} shower(s) {}, residual {:.1e}",
            if self.viable { "VIABLE" } else { "NOT VIABLE" },
            self.stored_min,
            self.stored_max,
            100.0 * self.stored_drift,
            if self.bounded { "bounded" } else { "DRIFTING" },
            self.atmosphere_min,
            self.atmosphere_max,
            self.showers,
            if self.showers_enough { "ok" } else { "TOO FEW" },
            self.residual
        )?;
        if self.soil_columns == 0 {
            return write!(f, "; no soil columns at all");
        }
        write!(f, "; {} soil columns, in band:", self.soil_columns)?;
        for b in &self.bands {
            write!(
                f,
                " {} {}/{} ({:.1}%{})",
                b.name,
                b.columns,
                self.soil_columns,
                100.0 * b.fraction,
                if b.passed { "" } else { ", SHORT" }
            )?;
        }
        Ok(())
    }
}

/// Step `world` through the warm-up and then the window, and report the cycle.
///
/// The world is left where the window ended; nothing is restored. Under an open budget
/// the shower count is always zero, so `min_showers` above zero makes every open world
/// report not viable — which is honest: an open world has no cycle to be viable.
pub fn measure(world: &mut World, spec: &ViabilitySpec) -> Viability {
    let threads = spec.threads.max(1);
    for _ in 0..spec.warmup_ticks {
        world.step_with(threads);
    }

    let showers_before = world.view().ledger.showers;
    let every = spec.sample_every.max(1);
    let mut stored: Vec<f64> = Vec::new();
    let mut atmosphere: Vec<f64> = Vec::new();
    let mut sample = |w: &World, stored: &mut Vec<f64>, atmosphere: &mut Vec<f64>| {
        stored.push(w.view().stored_m3());
        atmosphere.push(w.atmosphere_m3());
    };
    sample(world, &mut stored, &mut atmosphere);
    for t in 1..=spec.window_ticks {
        world.step_with(threads);
        if t % every == 0 {
            sample(world, &mut stored, &mut atmosphere);
        }
    }

    let total = world.view().ledger.expected_total();
    let half = stored.len() / 2;
    let (first, second) = stored.split_at(half.max(1).min(stored.len()));
    let stored_drift = if total > 0.0 {
        (mean(second) - mean(first)) / total
    } else {
        0.0
    };
    let showers = world.view().ledger.showers - showers_before;

    let (soil_columns, bands) = band_columns(world, spec);

    let bounded = stored_drift.abs() <= spec.drift_tolerance;
    let showers_enough = showers >= spec.min_showers;
    let all_bands = bands.iter().all(|b| b.passed);
    Viability {
        ticks: spec.window_ticks,
        samples: stored.len(),
        stored_min: stored.iter().copied().fold(f64::INFINITY, f64::min),
        stored_max: stored.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        stored_drift,
        bounded,
        atmosphere_min: atmosphere.iter().copied().fold(f64::INFINITY, f64::min),
        atmosphere_max: atmosphere.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        showers,
        showers_enough,
        soil_columns,
        bands,
        residual: world.view().total_residual(),
        viable: bounded && showers_enough && all_bands && soil_columns > 0,
    }
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// Every column whose surface face is soil, and how many of them each band holds.
///
/// The reading is the mean `pore` **fraction** over the contiguous soil voxels from the
/// surface face down, at most [`ViabilitySpec::root_cells`] of them: the same fraction
/// the flora layer's establishment gate compares against, averaged over a proxy for a
/// root box this crate cannot see.
fn band_columns(world: &World, spec: &ViabilitySpec) -> (usize, Vec<BandResult>) {
    let v = world.view();
    let c = v.config;
    let mut readings: Vec<f64> = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width as i64 {
            let Some(top) = v.surface_y(x, z) else {
                continue;
            };
            if v.material_at(x, top, z) != Material::Soil {
                continue;
            }
            let mut sum = 0.0;
            let mut n = 0u32;
            let mut y = top;
            while n < spec.root_cells.max(1) {
                if v.material_at(x, y, z) != Material::Soil {
                    break;
                }
                sum += v.pore[c.index(x, y, z)];
                n += 1;
                if y == 0 {
                    break;
                }
                y -= 1;
            }
            if n > 0 {
                readings.push(sum / n as f64);
            }
        }
    }
    let soil_columns = readings.len();
    let bands = spec
        .bands
        .iter()
        .map(|band| {
            let columns = readings.iter().filter(|&&p| band.holds(p)).count();
            let fraction = if soil_columns == 0 {
                0.0
            } else {
                columns as f64 / soil_columns as f64
            };
            BandResult {
                name: band.name.clone(),
                columns,
                fraction,
                passed: fraction >= spec.min_column_fraction,
            }
        })
        .collect();
    (soil_columns, bands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Config};

    fn spec() -> ViabilitySpec {
        ViabilitySpec {
            window_ticks: 200,
            sample_every: 10,
            min_showers: 1,
            root_cells: 2,
            min_column_fraction: 0.5,
            bands: vec![PoreBand::new("thirsty", 0.05, 1.0)],
            ..ViabilitySpec::default()
        }
    }

    /// A soil slab with a wet aquifer under it and water aloft: it showers, its stored
    /// water stays put, its soil is in band, and the books balance.
    #[test]
    fn a_cycling_world_reports_viable() {
        let c = Config {
            width: 8,
            height: 8,
            depth: 2,
            rain_m_per_s: 0.002,
            evaporation_m_per_s: 0.0004,
            closed_water_budget: true,
            initial_atmosphere_m3: 0.3,
            shower_trigger_fraction: 0.01,
            shower_volume_m3: 0.05,
            initial_aquifer_head_m: 0.4,
            ..Config::default()
        };
        let mut w = World::empty(c.clone());
        for x in 0..c.width as i64 {
            for z in 0..c.depth {
                w.apply(Command::SetMaterial {
                    x,
                    y: 1,
                    z,
                    material: Material::Soil,
                });
            }
        }
        let report = measure(&mut w, &spec());
        assert!(report.residual.abs() < 1e-9, "residual {}", report.residual);
        assert!(report.showers >= 1, "{report}");
        assert!(report.soil_columns > 0, "{report}");
        assert!(report.bands[0].passed, "{report}");
        assert!(report.viable, "{report}");
    }

    /// The same world with nothing aloft and nothing to lift: no shower ever runs, so
    /// the report says not viable while the books still balance.
    #[test]
    fn a_dry_locked_world_reports_not_viable() {
        let mut w = World::empty(Config {
            width: 8,
            height: 8,
            depth: 2,
            rain_m_per_s: 0.002,
            closed_water_budget: true,
            initial_atmosphere_m3: 0.0,
            ..Config::default()
        });
        let report = measure(&mut w, &spec());
        assert_eq!(report.showers, 0, "{report}");
        assert!(!report.viable, "{report}");
        assert!(report.residual.abs() < 1e-9, "residual {}", report.residual);
        assert!(
            report.to_string().contains("NOT VIABLE"),
            "{report}"
        );
    }
}
