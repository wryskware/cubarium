//! The frozen schema 12 `WorldState`: the last build before the ordinary quiet extension.
//!
//! This is the field list of [`crate::world::WorldState`] exactly as it stood at commit
//! `1e6d053`, with the same serde attributes. **Never change it.**
//!
//! Schema 13 appends [`crate::quiet::QuietState`]. Postcard is not self-describing, so a schema
//! 12 payload cannot be read as a schema 13 one with a missing trailing field — a missing
//! trailing field is not a missing field, it is the next value read from the wrong offset. Hence
//! this mirror.
//!
//! Every nested type here is still the **live** one, because the quiet extension touched none of
//! them: it adds a field beside them rather than changing any of their layouts. That borrow is
//! the same bet schemas 8–11 made about care, and it came due once; so it is guarded rather than
//! assumed. `tests/quiet_migration.rs` decodes genuine schema 12 payloads written by the
//! pre-quiet binary — one plain and one carrying real care ledgers — and any change to a nested
//! shape that forgets to freeze it here fails those tests rather than misreading a live world.
//!
//! 1. [`decode_snapshot`](super::decode_snapshot) reads a schema 12 payload into this and
//!    converts, opening the quiet extension **Off with no entries**. There are no retroactive
//!    pauses: that build never ran one, and a timer inferred from an old birth log would be a
//!    fiction.
//! 2. [`project`] goes the other way and **refuses** a world whose quiet policy is enabled or
//!    which holds a pause. An old-format projection of an enabled policy is not behaviourally
//!    equivalent, so it is not offered; Off continuation still projects exactly.

use serde::{Deserialize, Serialize};

use crate::accounting::EnergyCorrection;
use crate::care::CareState;
use crate::config::WorldConfig;
use crate::fields::Fields;
use crate::habitat::Weather;
use crate::hunter::HunterState;
use crate::ids::Slots;
use crate::organism::Organism;
use crate::quiet::QuietState;
use crate::world::WorldState;

/// The schema this mirror speaks.
pub const SCHEMA_V12: u32 = 12;

/// `WorldState` as of commit `1e6d053`. Frozen; see the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldStateV12 {
    pub config: WorldConfig,
    pub tick: u64,
    pub fields: Fields,
    pub weather: Weather,
    pub organisms: Slots<Organism>,
    pub births_total: u64,
    pub deaths_total: [u64; 3],
    pub cap_rejections_total: u64,
    pub external_material_in: f64,
    pub light_in_total: f64,
    pub heat_out_total: f64,
    #[serde(default)]
    pub rain_in_total: f64,
    #[serde(default)]
    pub evap_out_total: f64,
    #[serde(default)]
    pub care: CareState,
    #[serde(default)]
    pub energy_correction: EnergyCorrection,
    #[serde(default)]
    pub hunters: HunterState,
}

/// The schema 12 image of a current state, or `None` when there is no honest one.
///
/// **Refuses** an enabled policy, and refuses a held pause even under a policy that somehow is
/// not enabled. The old shape has nowhere to put either, and a projection that dropped them
/// would claim two worlds are the same world — precisely the comparison these projections exist
/// to make trustworthy. An Off world with no entries projects exactly, which is what the
/// reference arm of the experiment needs.
pub fn project(state: &WorldState) -> Option<WorldStateV12> {
    if state.quiet.policy.enabled() || !state.quiet.pauses.is_empty() {
        return None;
    }
    Some(WorldStateV12 {
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
        care: state.care.clone(),
        energy_correction: state.energy_correction,
        hunters: state.hunters.clone(),
    })
}

/// Migration: everything carries across, and the quiet extension opens Off and empty.
impl From<WorldStateV12> for WorldState {
    fn from(old: WorldStateV12) -> WorldState {
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
            care: old.care,
            energy_correction: old.energy_correction,
            hunters: old.hunters,
            // No retroactive pauses: that build never ran one.
            quiet: QuietState::default(),
            apex_dormancy: crate::dormancy::ApexDormancyState::default(),
            apex_encounters: crate::encounter::ApexEncounterState::default(),
        }
    }
}
