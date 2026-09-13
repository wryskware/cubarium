//! The frozen schema 10 `WorldState` — and the one migration in this crate that can **refuse**.
//!
//! This is the field list of [`crate::world::WorldState`] exactly as it stood at commit
//! `31a8ab5`, the first hunter build, with the same serde attributes, including its own frozen
//! copies of the hunter extension. **Never change it.**
//!
//! Schema 11 kept every schema 10 field except the *shape of the hunter extension*: the
//! profile's single forward `jaw_offset_px` placeholder became the measured two-component
//! capture effector, a separate ingestion mouth, the visual query extent and the body-scale
//! mapping, and members gained a persisted transition origin and attack episode
//! (`design/7_Research/lanternjaw-core-art-integration-gaps-2026-09-13.md`).
//!
//! So this mirror does two things:
//!
//! 1. A schema 10 payload whose extension is **empty** — no profile, no members, no imports —
//!    migrates exactly, like any other older world: it never had a hunter and never will get
//!    one from a load.
//! 2. A schema 10 payload whose extension is **anything else** is refused with a named error.
//!    That is the whole of it, not only a live hunt: a profile with no members, a
//!    budget-matched control deposit, an extinct lineage's counters. None of it can be
//!    reinterpreted in the new shape without inventing a capture geometry and a scale mapping
//!    it never had, and inventing them would silently change what an experiment measured.
//!    Schema 10 was never deployed to the live world; the only payloads that can hit this are
//!    archived artifacts, which carry their own recipe and can be re-created from it.

use serde::{Deserialize, Serialize};

use cubarium_surface::Vec2;

use crate::accounting::EnergyCorrection;
use crate::config::WorldConfig;
use crate::fields::Fields;
use crate::genome::Genome;
use crate::habitat::Weather;
use crate::hunter::{HunterPhase, HunterRole, HunterState};
use crate::ids::{OrganismId, Slots};
use crate::organism::Organism;
use crate::world::WorldState;

use super::care_v1::{self, CareStateV1};

/// The schema this mirror speaks.
pub const SCHEMA_V10: u32 = 10;

/// The hunter profile as schema 10 wrote it (`PROFILE_VERSION` 1). Frozen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixedHunterProfileV10 {
    pub version: u32,
    pub role: HunterRole,
    pub genome: Genome,
    pub attacks_enabled: bool,
    pub body_extent_px: f64,
    /// The unconfirmed six-pixel forward placeholder schema 11 replaced.
    pub jaw_offset_px: f64,
    pub jaw_reach_px: f64,
    pub founder_reserve_fraction: f64,
    pub founder_energy_fraction: f64,
    pub perch_reserve_fraction: f64,
    pub seek_reserve_fraction: f64,
    pub prey_structure_min: f64,
    pub prey_structure_fraction_max: f64,
    pub stalk_timeout_seconds: f64,
    pub windup_seconds: f64,
    pub strike_seconds: f64,
    pub strike_speed_px_s: f64,
    pub strike_energy_cost: f64,
    pub recovery_seconds: f64,
    pub capture_base: f64,
    pub capture_min: f64,
    pub capture_max: f64,
    pub escape_speed_multiple: f64,
    pub escape_turn_rate_deg: f64,
    pub gut_capacity_material: f64,
    pub handling_cost_per_second: f64,
    pub digest_rate: f64,
    pub meal_recovery_seconds: f64,
    pub scavenge_fraction: f64,
    pub reproduce_min_age_seconds: f64,
    pub reproduce_reserve_fraction: f64,
    pub reproduce_energy_fraction: f64,
    pub reproduce_interval_seconds: f64,
    pub gestation_seconds: f64,
    pub juvenile_growth_rate: f64,
}

/// A member as schema 10 wrote it: no transition origin, no episode. Frozen.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HunterMemberV10 {
    pub id: OrganismId,
    pub phase: HunterPhase,
    pub phase_started_tick: u64,
    pub phase_ends_tick: u64,
    pub target: Option<OrganismId>,
    pub attack_counter: u64,
    pub next_reproduction_tick: u64,
    pub gut_material: f64,
    pub gut_energy: f64,
}

/// The extension as schema 10 wrote it. Frozen.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HunterStateV10 {
    pub profile: Option<FixedHunterProfileV10>,
    pub members: Vec<HunterMemberV10>,
    pub founder_material_in: f64,
    pub founder_energy_in: f64,
    pub control_material_in: f64,
    pub control_energy_in: f64,
    pub control_deposited: bool,
    pub founders_placed: u32,
    pub attacks_total: u64,
    pub captures_total: u64,
    pub predation_deaths_total: u64,
    pub hunter_deaths_total: u64,
    pub hunter_births_total: u64,
}

impl HunterStateV10 {
    /// True when this extension is the inert default: nothing to reinterpret.
    pub fn is_empty(&self) -> bool {
        *self == HunterStateV10::default()
    }
}

/// `WorldState` as of commit `31a8ab5`. Frozen; see the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldStateV10 {
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
    pub care: CareStateV1,
    #[serde(default)]
    pub energy_correction: EnergyCorrection,
    #[serde(default)]
    pub hunters: HunterStateV10,
}

/// The schema 10 projection of a current state: every field but `hunters`, with the hunter
/// extension left at its schema 10 default.
///
/// Only an **empty** extension projects: a schema 11 world with a trial running has no honest
/// schema 10 image, so this returns `None` rather than writing a world whose profile the old
/// shape cannot hold.
pub fn project(state: &WorldState) -> Option<WorldStateV10> {
    if state.hunters != crate::hunter::HunterState::default() {
        return None;
    }
    Some(WorldStateV10 {
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
        hunters: HunterStateV10::default(),
    })
}

/// Migration, or a refusal: an empty schema 10 extension opens empty; an active schema 10
/// trial is refused by name rather than reinterpreted into the new profile shape.
pub fn migrate(old: WorldStateV10) -> Result<WorldState, String> {
    if !old.hunters.is_empty() {
        return Err(
            "this schema 10 snapshot carries a non-empty hunter extension (a trial, a \
             budget-matched control, or the history of one), whose profile and member shape \
             changed in schema 11 (measured capture effector, ingestion mouth, body scale, \
             transition origin); it is refused rather than reinterpreted — re-create it from \
             its recorded recipe"
                .into(),
        );
    }
    Ok(WorldState {
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
    })
}

/// The unused `Vec2` import keeps the frozen profile's documentation honest about what schema
/// 10 did *not* have: a two-component capture offset.
const _: Option<Vec2> = None;
