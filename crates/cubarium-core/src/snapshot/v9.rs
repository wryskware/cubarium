//! The frozen schema 9 `WorldState`.
//!
//! This is the field list of [`crate::world::WorldState`] exactly as it stood at commit
//! `1f0fc3a`, the last pre-hunter build, with the same serde attributes. **Never change it.**
//! `postcard` is not self-describing: the bytes of a schema 9 payload are this struct's
//! fields in this order, so editing it silently misreads every live snapshot the accounting
//! builds wrote.
//!
//! It has the same two jobs [`super::v7`] and [`super::v8`] have:
//!
//! 1. [`decode_snapshot`](super::decode_snapshot) reads a schema 9 payload into it and
//!    converts to the current [`WorldState`] with `hunters = HunterState::default()` — an
//!    empty, inert extension. Migration never introduces a predator.
//! 2. [`project`] goes the other way, dropping only `hunters`, so a world migrated from
//!    schema 9 and stepped forward with no hunters re-encodes to exactly the bytes the
//!    pre-hunter build would have written: care, and the signed energy corrections, included.

use serde::{Deserialize, Serialize};

use crate::accounting::EnergyCorrection;
use crate::config::WorldConfig;
use crate::fields::Fields;
use crate::habitat::Weather;
use crate::hunter::HunterState;
use crate::ids::Slots;
use crate::organism::Organism;
use crate::world::WorldState;

use super::care_v1::{self, CareStateV1};

/// The schema this mirror speaks.
pub const SCHEMA_V9: u32 = 9;

/// `WorldState` as of commit `1f0fc3a`. Frozen; see the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldStateV9 {
    pub config: WorldConfig,
    pub tick: u64,
    pub fields: Fields,
    pub weather: Weather,
    pub organisms: Slots<Organism>,
    /// Cumulative counters since world creation.
    pub births_total: u64,
    pub deaths_total: [u64; 3],
    pub cap_rejections_total: u64,
    /// Material admitted from outside (founders and any future stimuli), for the invariant.
    pub external_material_in: f64,
    /// Running energy audit: the raw, uncompensated counters.
    pub light_in_total: f64,
    pub heat_out_total: f64,
    /// Running water budget (`design/water.md`).
    #[serde(default)]
    pub rain_in_total: f64,
    #[serde(default)]
    pub evap_out_total: f64,
    /// Optional care, in the **frozen pre-dose shape** (`super::care_v1`).
    #[serde(default)]
    pub care: CareStateV1,
    /// The persisted signed energy corrections (`crate::accounting`).
    #[serde(default)]
    pub energy_correction: EnergyCorrection,
}

/// The schema 9 projection of a current state: every field but `hunters` — or `None` when the
/// active shower carries a nonstandard dose this shape cannot represent.
pub fn project(state: &WorldState) -> Option<WorldStateV9> {
    // An enabled ordinary quiet policy, or a held pause, has no image in a shape that predates
    // the extension: dropping a live timer would make two behaviourally different worlds compare
    // equal (`crate::quiet`).
    if state.quiet.policy.enabled() || !state.quiet.pauses.is_empty() {
        return None;
    }

    Some(WorldStateV9 {
        config: state.config.clone(),
        tick: state.tick,
        fields: state.fields.clone(),
        weather: state.weather.clone(),
        organisms: state.organisms.clone(),
        births_total: state.births_total,
        deaths_total: state.deaths_total,
        cap_rejections_total: state.cap_rejections_total,
        external_material_in: state.external_material_in,
        light_in_total: state.light_in_total,
        heat_out_total: state.heat_out_total,
        rain_in_total: state.rain_in_total,
        evap_out_total: state.evap_out_total,
        care: care_v1::project(&state.care)?,
        energy_correction: state.energy_correction,
    })
}

/// Migration: a schema 9 world has never had a hunter, so the extension opens empty —
/// no profile, no members, no imports, no counters. Nothing else is touched.
impl From<WorldStateV9> for WorldState {
    fn from(old: WorldStateV9) -> WorldState {
        WorldState {
            config: old.config,
            tick: old.tick,
            fields: old.fields,
            weather: old.weather,
            organisms: old.organisms,
            births_total: old.births_total,
            deaths_total: old.deaths_total,
            cap_rejections_total: old.cap_rejections_total,
            external_material_in: old.external_material_in,
            light_in_total: old.light_in_total,
            heat_out_total: old.heat_out_total,
            rain_in_total: old.rain_in_total,
            evap_out_total: old.evap_out_total,
            care: old.care.into(),
            energy_correction: old.energy_correction,
            hunters: HunterState::default(),
            // Off, with no retroactive pauses: a world written before the ordinary quiet
            // extension existed never ran one (`crate::quiet`).
            quiet: crate::quiet::QuietState::default(),
            apex_dormancy: crate::dormancy::ApexDormancyState::default(),
            apex_encounters: crate::encounter::ApexEncounterState::default(),
        }
    }
}
