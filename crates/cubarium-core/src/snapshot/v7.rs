//! The frozen schema 7 `WorldState`.
//!
//! This is the field list of [`crate::world::WorldState`] exactly as it stood at commit
//! `e3ad20f`, the last pre-care build, with the same serde attributes. **Never change it.**
//! `postcard` is not self-describing: the bytes of a schema 7 payload are this struct's
//! fields in this order, so editing it silently misreads every live snapshot.
//!
//! It has two jobs:
//!
//! 1. [`decode_snapshot`](super::decode_snapshot) reads a schema 7 payload into it and
//!    converts to the current [`WorldState`] with `care = CareState::default()`.
//! 2. [`project`] goes the other way, dropping only `care`, so
//!    [`ecology_hash`](super::ecology_hash) hashes exactly the bytes the old build would
//!    have written. For a migrated world with zero care that hash equals the old
//!    `state_hash` of the same world.

use serde::{Deserialize, Serialize};

use crate::config::WorldConfig;
use crate::fields::Fields;
use crate::habitat::Weather;
use crate::ids::Slots;
use crate::organism::Organism;
use crate::world::WorldState;

/// The schema this mirror speaks.
pub const SCHEMA_V7: u32 = 7;

/// `WorldState` as of commit `e3ad20f`. Frozen; see the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldStateV7 {
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
    /// Running energy audit.
    pub light_in_total: f64,
    pub heat_out_total: f64,
    /// Running water budget (`design/water.md`): `Σw == rain_in_total − evap_out_total`
    /// to rounding at every tick of a world created dry.
    #[serde(default)]
    pub rain_in_total: f64,
    #[serde(default)]
    pub evap_out_total: f64,
}

/// The schema 7 projection of a current state: every field but `care`.
pub fn project(state: &WorldState) -> WorldStateV7 {
    WorldStateV7 {
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
    }
}

/// Migration: a schema 7 world has never been given care, so its ledgers open at zero and
/// its sequence cursor at zero. No ecological value is touched.
impl From<WorldStateV7> for WorldState {
    fn from(old: WorldStateV7) -> WorldState {
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
            care: crate::care::CareState::default(),
        }
    }
}
