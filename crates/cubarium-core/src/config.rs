//! World configuration: every rate, bound, and toggle, versioned and serializable.
//!
//! Defaults are the initial tuning hypotheses in `design/m2-world-spec.md`. Units:
//! material `m`, energy `e`, seconds, pixels. Per-tick quantities are derived by the
//! consumer by multiplying per-second rates with `DT`; the config never stores per-tick
//! values so that the tick rate can change without editing worlds.

use serde::{Deserialize, Serialize};

/// Bumped whenever a field's meaning changes; stored in snapshots.
pub const CONFIG_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldConfig {
    pub version: u32,
    /// Seed for every keyed draw (habitat noise, weather, organisms, founders).
    pub seed: u64,
    pub producer: ProducerConfig,
    pub detritus: DetritusConfig,
    pub nutrient: NutrientConfig,
    pub habitat: HabitatConfig,
    pub weather: WeatherConfig,
    pub organism: OrganismConfig,
    pub drives: DriveConfig,
    pub founders: FounderConfig,
    pub capacity: CapacityConfig,
    pub mechanisms: MechanismToggles,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProducerConfig {
    /// `g`: growth rate per second at full light and moisture.
    pub growth: f64,
    /// `P_max`: carrying capacity per cell (m).
    pub max: f64,
    /// `f_max`: maximum fraction of a cell's `N` taken per second.
    pub uptake_max: f64,
    /// `e_p`: energy per material unit of producer (e/m); light is the source.
    pub energy_density: f64,
    /// `m_p`: mortality per second (`P → D`).
    pub mortality: f64,
    /// Initial `P` as a fraction of `P_max · L₀ · W₀`.
    pub initial_fraction: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DetritusConfig {
    /// `k_d`: decomposition per second (`D → N`).
    pub decomposition: f64,
    /// `e_d_max`: maximum retained energy per material unit (e/m).
    pub energy_cap: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NutrientConfig {
    /// Initial `N` per cell (m).
    pub initial: f64,
    /// Diffusion exchange coefficient per second per edge (dimensionless per tick after `· DT`).
    pub diffusion: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HabitatConfig {
    pub light_base: f64,
    pub light_height_gain: f64,
    pub light_noise_gain: f64,
    pub moisture_base: f64,
    pub moisture_height_gain: f64,
    pub moisture_noise_gain: f64,
    pub moisture_min: f64,
    /// Number of cosine waves in the patch noise.
    pub noise_waves: u32,
    /// Wavelength bounds in cube units.
    pub noise_wavelength: [f64; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeatherConfig {
    /// False freezes blob centers (static habitat control for E2).
    pub moving: bool,
    pub blobs_per_channel: u32,
    /// Angular radius of each blob cap in degrees.
    pub blob_radius_deg: f64,
    pub amplitude: f64,
    /// Orbit periods in minutes, one per blob (cycled if fewer than blobs).
    pub periods_min: Vec<f64>,
    /// Random-walk step of blob centers per minute, in degrees.
    pub walk_deg_per_min: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OrganismConfig {
    pub structure_adult: f64,
    pub reserve_max: f64,
    pub energy_max: f64,
    pub speed_max: f64,
    pub mouth_rate: f64,
    pub sense_radius: f64,
    pub assimilation_material: f64,
    pub assimilation_energy: f64,
    pub maintenance: f64,
    pub move_cost: f64,
    pub sense_cost: f64,
    pub oxidation_threshold: f64,
    pub oxidation_rate: f64,
    pub reserve_energy_density: f64,
    pub oxidation_efficiency: f64,
    pub growth_rate: f64,
    pub growth_reserve_min: f64,
    pub build_cost: f64,
    pub child_structure_fraction: f64,
    pub child_reserve_fraction: f64,
    pub child_energy_fraction: f64,
    pub gestation_seconds: f64,
    pub max_age_seconds: f64,
    pub min_structure: f64,
    pub body_extent_max: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DriveConfig {
    pub w_food: f64,
    pub w_detritus: f64,
    pub w_persist: f64,
    pub w_crowd: f64,
    pub seek_on: f64,
    pub seek_off: f64,
    pub feed_min: f64,
    pub rest_effort: f64,
    pub feed_effort: f64,
    pub bud_reserve: f64,
    pub bud_energy: f64,
    pub bud_min_age_seconds: f64,
    pub tau_hunger_seconds: f64,
    pub turn_rate_max_deg: f64,
    pub turn_noise: f64,
    pub birth_offset_px: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FounderConfig {
    /// Founders placed at world creation, uniformly by area over the surface.
    pub count: u32,
    pub initial_reserve_fraction: f64,
    pub initial_energy_fraction: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CapacityConfig {
    pub max_organisms: u32,
    pub max_neighbors: u32,
    pub checkpoint_seconds: f64,
    pub telemetry_seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MechanismToggles {
    pub grazing: bool,
    pub scavenging: bool,
    /// M3a and later; M2 keeps it false and the copy is exact.
    pub mutation: bool,
}

impl Default for WorldConfig {
    fn default() -> Self {
        WorldConfig {
            version: CONFIG_VERSION,
            seed: 1,
            producer: ProducerConfig::default(),
            detritus: DetritusConfig::default(),
            nutrient: NutrientConfig::default(),
            habitat: HabitatConfig::default(),
            weather: WeatherConfig::default(),
            organism: OrganismConfig::default(),
            drives: DriveConfig::default(),
            founders: FounderConfig::default(),
            capacity: CapacityConfig::default(),
            mechanisms: MechanismToggles::default(),
        }
    }
}

impl Default for ProducerConfig {
    fn default() -> Self {
        ProducerConfig { growth: 0.005, max: 2.0, uptake_max: 0.5, energy_density: 2.0, mortality: 0.0005, initial_fraction: 0.3 }
    }
}

impl Default for DetritusConfig {
    fn default() -> Self {
        DetritusConfig { decomposition: 0.002, energy_cap: 1.0 }
    }
}

impl Default for NutrientConfig {
    fn default() -> Self {
        NutrientConfig { initial: 1.0, diffusion: 0.05 }
    }
}

impl Default for HabitatConfig {
    fn default() -> Self {
        HabitatConfig {
            light_base: 0.55,
            light_height_gain: 0.35,
            light_noise_gain: 0.1,
            moisture_base: 0.8,
            moisture_height_gain: -0.3,
            moisture_noise_gain: 0.2,
            moisture_min: 0.1,
            noise_waves: 6,
            noise_wavelength: [0.6, 1.4],
        }
    }
}

impl Default for WeatherConfig {
    fn default() -> Self {
        WeatherConfig {
            moving: true,
            blobs_per_channel: 3,
            blob_radius_deg: 55.0,
            amplitude: 0.15,
            periods_min: vec![20.0, 33.0, 47.0],
            walk_deg_per_min: 2.0,
        }
    }
}

impl Default for OrganismConfig {
    fn default() -> Self {
        OrganismConfig {
            structure_adult: 1.0,
            reserve_max: 1.0,
            energy_max: 2.0,
            speed_max: 1.5,
            mouth_rate: 0.05,
            sense_radius: 8.0,
            assimilation_material: 0.6,
            assimilation_energy: 0.5,
            maintenance: 0.005,
            move_cost: 0.01,
            sense_cost: 0.0005,
            oxidation_threshold: 0.2,
            oxidation_rate: 0.02,
            reserve_energy_density: 2.0,
            oxidation_efficiency: 0.8,
            growth_rate: 0.01,
            growth_reserve_min: 0.3,
            build_cost: 0.5,
            child_structure_fraction: 0.4,
            child_reserve_fraction: 0.2,
            child_energy_fraction: 0.25,
            gestation_seconds: 30.0,
            max_age_seconds: 7200.0,
            min_structure: 0.1,
            body_extent_max: 9.0,
        }
    }
}

impl Default for DriveConfig {
    fn default() -> Self {
        DriveConfig {
            w_food: 1.0,
            w_detritus: 0.4,
            w_persist: 0.3,
            w_crowd: 0.6,
            seek_on: 0.3,
            seek_off: 0.1,
            feed_min: 0.05,
            rest_effort: 0.05,
            feed_effort: 0.2,
            bud_reserve: 0.7,
            bud_energy: 0.6,
            bud_min_age_seconds: 120.0,
            tau_hunger_seconds: 10.0,
            turn_rate_max_deg: 90.0,
            turn_noise: 0.6,
            birth_offset_px: 2.5,
        }
    }
}

impl Default for FounderConfig {
    fn default() -> Self {
        FounderConfig { count: 72, initial_reserve_fraction: 0.6, initial_energy_fraction: 0.7 }
    }
}

impl Default for CapacityConfig {
    fn default() -> Self {
        CapacityConfig { max_organisms: 512, max_neighbors: 16, checkpoint_seconds: 60.0, telemetry_seconds: 5.0 }
    }
}

impl Default for MechanismToggles {
    fn default() -> Self {
        MechanismToggles { grazing: true, scavenging: true, mutation: false }
    }
}

impl WorldConfig {
    /// Reject configurations that cannot produce a well-defined world: non-finite or
    /// negative rates, `seek_off >= seek_on`, capacities of zero, assimilation fractions
    /// outside `[0, 1]`, a child material fraction that exceeds what the parent can hold,
    /// or a body extent above `cubarium_surface::MAX_LOCAL_RADIUS`.
    pub fn validate(&self) -> Result<(), String> {
        todo!("WorldConfig::validate")
    }
}
