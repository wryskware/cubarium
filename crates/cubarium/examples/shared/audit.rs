//! Conservation arithmetic shared by the matched in-memory comparison examples.
//!
//! Extracted verbatim from `care_compare`, which is the only reason it reads the way it does:
//! every expression here is the one that example already ran, moved rather than rewritten, so
//! its published output stays byte for byte what it was. A test asserts exactly that against a
//! frozen binary.
//!
//! Observer only. Nothing here mutates a `World`, consumes an RNG draw, resets a baseline, or
//! relaxes a tolerance. The limits are always a fixed fraction of the **opening** inventory, so
//! a longer run or a larger input cannot buy itself a looser audit.
//!
//! `hunter_compare` keeps its own stricter `audit.rs`, which refuses a world that admits care at
//! all. That refusal is correct for a care-free predation trial and wrong for anything that
//! deliberately waters a world, so the two are deliberately not merged.

use anyhow::{Context, Result, ensure};
use cubarium_core::{LifeEvent, OrganismId, WorldState};
use std::collections::{BTreeMap, BTreeSet};

/// Kahan–Babuška–Neumaier accumulation, so a long run's low-order flow is not rounded away
/// before it can be compared against a stock difference.
#[derive(Default)]
pub struct AccurateSum {
    sum: f64,
    correction: f64,
}

impl AccurateSum {
    pub fn add(&mut self, value: f64) {
        let next = self.sum + value;
        self.correction += if self.sum.abs() >= value.abs() {
            (self.sum - next) + value
        } else {
            (value - next) + self.sum
        };
        self.sum = next;
    }

    pub fn value(&self) -> f64 {
        self.sum + self.correction
    }
}

/// An **independent** running total of the world's short-lived per-window counters.
///
/// The point is that it never reads a persisted cumulative ledger: it sums what telemetry
/// reported, so a residual computed from it is a second opinion rather than the same number
/// rearranged.
#[derive(Default)]
pub struct WindowAudit {
    pub light: AccurateSum,
    pub heat: AccurateSum,
    pub rain: AccurateSum,
    pub evap: AccurateSum,
    counts: cubarium_core::Telemetry,
}

impl WindowAudit {
    pub fn observe(&mut self, mut sample: cubarium_core::Telemetry) -> cubarium_core::Telemetry {
        self.light.add(sample.light_in);
        self.heat.add(sample.heat_out);
        self.rain.add(sample.rain_in);
        self.evap.add(sample.evap_out);
        macro_rules! counters {
            ($($field:ident),*) => { $(
                self.counts.$field += sample.$field;
                sample.$field = self.counts.$field;
            )* };
        }
        counters!(
            births,
            deaths_starvation,
            deaths_age,
            deaths_collapse,
            cap_rejections,
            travel_fallbacks,
            travel_ties,
            pairs_considered,
            pairs_unfolded,
            neighbor_truncations
        );
        sample.light_in = self.light.value();
        sample.heat_out = self.heat.value();
        sample.rain_in = self.rain.value();
        sample.evap_out = self.evap.value();
        sample
    }
}

/// Every unit of material the world is holding: the fields plus every body.
pub fn material(s: &WorldState) -> f64 {
    s.fields.total_material() + s.organisms.iter().map(|(_, o)| o.material()).sum::<f64>()
}

/// Every unit of chemical energy the world is holding, escrowed offspring included.
pub fn energy(s: &WorldState) -> f64 {
    let reserve = s.config.organism.reserve_energy_density;
    s.fields.p.iter().sum::<f64>() * s.config.producer.energy_density
        + s.fields.f.iter().sum::<f64>() * s.config.fruit.energy_density
        + s.fields.de.iter().sum::<f64>()
        + s.organisms
            .iter()
            .map(|(_, o)| {
                o.energy
                    + reserve * o.reserve
                    + o.escrow
                        .as_ref()
                        .map_or(0.0, |e| e.energy + reserve * (e.structure + e.reserve))
            })
            .sum::<f64>()
}

/// Bounded by the living population, not by total births in a long experiment.
/// A checkpoint's opening cohort is not necessarily its original founder lineage.
pub struct Ancestry {
    pub live: BTreeMap<OrganismId, (OrganismId, u64)>,
    pub maximum_depth: u64,
}

impl Ancestry {
    pub fn new(state: &WorldState) -> Self {
        Self {
            live: state
                .organisms
                .iter()
                .map(|(id, _)| (id, (id, 0)))
                .collect(),
            maximum_depth: 0,
        }
    }

    pub fn observe(&mut self, events: &[LifeEvent]) -> Result<()> {
        // Resolve births before removals: a parent can die in the birth's tick.
        for event in events {
            if let LifeEvent::Birth { id, parent, .. } = event {
                let (cohort, depth) = *self.live.get(parent).context("unobserved birth parent")?;
                let depth = depth.checked_add(1).context("ancestry depth overflow")?;
                ensure!(
                    self.live.insert(*id, (cohort, depth)).is_none(),
                    "duplicate birth id"
                );
                self.maximum_depth = self.maximum_depth.max(depth);
            }
        }
        for event in events {
            if let LifeEvent::Death { id, .. } = event {
                ensure!(self.live.remove(id).is_some(), "unobserved death id");
            }
        }
        Ok(())
    }

    pub fn surviving_cohorts(&self) -> usize {
        self.live
            .values()
            .map(|(cohort, _)| *cohort)
            .collect::<BTreeSet<_>>()
            .len()
    }
}

/// Every gate must pass, at the same opening-inventory limits: material and water against the
/// raw legacy ledgers, the persisted compensated energy, the independent windowed energy, and
/// the immediate care-boundary energy. The raw legacy energy drift stays visible separately and
/// is never relabelled as passing.
pub fn audit_passes(
    legacy: [f64; 3],
    corrected_energy: f64,
    windowed_energy: f64,
    care_boundary_energy: f64,
    limits: [f64; 3],
) -> bool {
    [
        legacy[0],
        legacy[2],
        corrected_energy,
        windowed_energy,
        care_boundary_energy,
    ]
    .into_iter()
    .zip([limits[0], limits[2], limits[1], limits[1], limits[1]])
    .all(|(drift, limit)| drift.is_finite() && drift >= 0.0 && drift < limit)
}
