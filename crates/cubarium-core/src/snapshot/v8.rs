//! The frozen schema 8 `WorldState`.
//!
//! This is the field list of [`crate::world::WorldState`] exactly as it stood at commit
//! `c60241f`, the last pre-correction build, with the same serde attributes. **Never change
//! it.** `postcard` is not self-describing: the bytes of a schema 8 payload are this struct's
//! fields in this order, so editing it silently misreads every live snapshot the care builds
//! wrote.
//!
//! It has the same two jobs [`super::v7`] has:
//!
//! 1. [`decode_snapshot`](super::decode_snapshot) reads a schema 8 payload into it and
//!    converts to the current [`WorldState`] with `energy_correction =
//!    EnergyCorrection::default()` — zero, because the low-order bits that world's raw
//!    counters already lost cannot be recovered from a snapshot (`crate::accounting`).
//! 2. [`project`] goes the other way, dropping only `energy_correction`, so a world that was
//!    migrated from schema 8 and stepped forward re-encodes to exactly the bytes the
//!    pre-correction build would have written, `care` included.

use serde::{Deserialize, Serialize};

use crate::accounting::EnergyCorrection;
use crate::config::WorldConfig;
use crate::fields::Fields;
use crate::habitat::Weather;
use crate::ids::Slots;
use crate::organism::Organism;
use crate::world::WorldState;

use super::care_v1::{self, CareStateV1};

/// The schema this mirror speaks.
pub const SCHEMA_V8: u32 = 8;

/// `WorldState` as of commit `c60241f`. Frozen; see the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldStateV8 {
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
    /// Running energy audit, uncompensated: these are the raw counters, and schema 9 keeps
    /// them bit-identical.
    pub light_in_total: f64,
    pub heat_out_total: f64,
    /// Running water budget (`design/water.md`): `Σw == rain_in_total − evap_out_total`
    /// to rounding at every tick of a world created dry.
    #[serde(default)]
    pub rain_in_total: f64,
    #[serde(default)]
    pub evap_out_total: f64,
    /// Optional care (`design/7_Research/care-contract-2026-09-12.md`), in the **frozen
    /// pre-dose shape**: schema 12 added `ActiveShower::dose_permille`, and this payload's
    /// bytes are the shape without it (`super::care_v1`).
    #[serde(default)]
    pub care: CareStateV1,
}

/// The schema 8 projection of a current state: every field but `energy_correction` — or
/// `None` when the world's active shower carries a nonstandard dose, which this shape cannot
/// represent and must not silently drop (`super::care_v1::project`).
pub fn project(state: &WorldState) -> Option<WorldStateV8> {
    // An enabled ordinary quiet policy, or a held pause, has no image in a shape that predates
    // the extension: dropping a live timer would make two behaviourally different worlds compare
    // equal (`crate::quiet`).
    if state.quiet.policy.enabled() || !state.quiet.pauses.is_empty() {
        return None;
    }

    Some(WorldStateV8 {
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
    })
}

/// Migration: the corrections of a schema 8 world open at **zero**. Its raw totals are kept
/// exactly as saved and its care history is carried through untouched; compensation starts
/// from this load, and no historical rounding repair is claimed (`crate::accounting`).
impl From<WorldStateV8> for WorldState {
    fn from(old: WorldStateV8) -> WorldState {
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
            energy_correction: EnergyCorrection::default(),
            hunters: crate::hunter::HunterState::default(),
            // Off, with no retroactive pauses: a world written before the ordinary quiet
            // extension existed never ran one (`crate::quiet`).
            quiet: crate::quiet::QuietState::default(),
            apex_dormancy: crate::dormancy::ApexDormancyState::default(),
            apex_encounters: crate::encounter::ApexEncounterState::default(),
        }
    }
}
