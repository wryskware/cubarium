//! The frozen schema 11 `WorldState`: the last build before the adjustable care dose.
//!
//! This is the field list of [`crate::world::WorldState`] exactly as it stood at commit
//! `e55501d`, with the same serde attributes — and, critically, with the **frozen pre-dose
//! care shape** ([`super::care_v1::CareStateV1`]) rather than the live one, which grew
//! `ActiveShower::dose_permille` in schema 12. **Never change it.**
//!
//! The hunter extension is still the live [`HunterState`], because this package did not touch
//! it. That borrow is exactly the bet schemas 8–11 made about care, and it came due here — so
//! it is guarded rather than assumed: `tests/care_dose_migration.rs` decodes genuine schema 11
//! payloads written by the pre-dose binary, and any change to the hunter shape that forgets to
//! freeze it here fails that test rather than misreading a live world.
//!
//! 1. [`decode_snapshot`](super::decode_snapshot) reads a schema 11 payload into this and
//!    converts, opening any in-flight shower at the standard dose — the only dose that build
//!    could deliver.
//! 2. [`project`] goes the other way and **refuses** a world whose active shower is
//!    nonstandard: the old shape has nowhere to put the amount.

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
pub const SCHEMA_V11: u32 = 11;

/// `WorldState` as of commit `e55501d`. Frozen; see the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldStateV11 {
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
    /// The **pre-dose** care shape.
    #[serde(default)]
    pub care: CareStateV1,
    #[serde(default)]
    pub energy_correction: EnergyCorrection,
    #[serde(default)]
    pub hunters: HunterState,
}

/// The schema 11 image of a current state, or `None` when there is no honest one: a shower
/// falling at a nonstandard dose cannot be written into a shape with no dose.
pub fn project(state: &WorldState) -> Option<WorldStateV11> {
    // An enabled ordinary quiet policy, or a held pause, has no image in a shape that predates
    // the extension: dropping a live timer would make two behaviourally different worlds compare
    // equal (`crate::quiet`).
    if state.quiet.policy.enabled() || !state.quiet.pauses.is_empty() {
        return None;
    }

    Some(WorldStateV11 {
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
        hunters: state.hunters.clone(),
    })
}

/// Migration: everything carries across, and an in-flight shower opens at the standard dose.
impl From<WorldStateV11> for WorldState {
    fn from(old: WorldStateV11) -> WorldState {
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
            hunters: old.hunters,
            // Off, with no retroactive pauses: a world written before the ordinary quiet
            // extension existed never ran one (`crate::quiet`).
            quiet: crate::quiet::QuietState::default(),
        }
    }
}
