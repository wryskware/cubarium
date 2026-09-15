//! Frozen schema 13 `WorldState`, immediately before apex dormancy and encounters were appended.
//!
//! Every nested type is unchanged by schema 14; the new extension is a trailing sibling field.

use serde::{Deserialize, Serialize};

use crate::accounting::EnergyCorrection;
use crate::care::CareState;
use crate::config::WorldConfig;
use crate::dormancy::ApexDormancyState;
use crate::encounter::ApexEncounterState;
use crate::fields::Fields;
use crate::habitat::Weather;
use crate::hunter::HunterState;
use crate::ids::Slots;
use crate::organism::Organism;
use crate::quiet::QuietState;
use crate::world::WorldState;

pub const SCHEMA_V13: u32 = 13;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldStateV13 {
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
    #[serde(default)]
    pub quiet: QuietState,
}

/// A current state has an honest schema-13 image only when neither apex extension was enabled.
pub fn project(state: &WorldState) -> Option<WorldStateV13> {
    if state.apex_dormancy != ApexDormancyState::default()
        || state.apex_encounters != ApexEncounterState::default()
    {
        return None;
    }
    Some(WorldStateV13 {
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
        quiet: state.quiet.clone(),
    })
}

// The `From<WorldStateVn> for WorldState` conversion this module used to carry is **gone**
// (`design/ecology-v1-contract.md` §15.1). Converting an old payload into the current shape
// is exactly the migration Wrysk's 2026-09-15 rule forbids: worlds always restart fresh.
// What survives is the frozen wire shape above and its schema constant, which the refusal
// tests name.
