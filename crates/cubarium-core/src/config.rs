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
        ProducerConfig { growth: 0.008, max: 2.0, uptake_max: 0.5, energy_density: 2.0, mortality: 0.0005, initial_fraction: 0.3 }
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
            move_cost: 0.006,
            sense_cost: 0.0002,
            oxidation_threshold: 0.5,
            oxidation_rate: 0.01,
            reserve_energy_density: 2.0,
            oxidation_efficiency: 0.8,
            growth_rate: 0.01,
            growth_reserve_min: 0.3,
            build_cost: 0.5,
            child_structure_fraction: 0.4,
            child_reserve_fraction: 0.2,
            child_energy_fraction: 0.15,
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
            bud_energy: 0.3,
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
        if self.version != CONFIG_VERSION {
            return Err(format!(
                "config version {} is not the supported version {CONFIG_VERSION}",
                self.version
            ));
        }

        let p = &self.producer;
        finite_nonnegative(&[
            ("producer.growth", p.growth),
            ("producer.uptake_max", p.uptake_max),
            ("producer.energy_density", p.energy_density),
            ("producer.mortality", p.mortality),
        ])?;
        positive("producer.max", p.max)?;
        fraction("producer.initial_fraction", p.initial_fraction)?;

        let d = &self.detritus;
        finite_nonnegative(&[
            ("detritus.decomposition", d.decomposition),
            ("detritus.energy_cap", d.energy_cap),
        ])?;

        let n = &self.nutrient;
        finite_nonnegative(&[("nutrient.initial", n.initial), ("nutrient.diffusion", n.diffusion)])?;

        let h = &self.habitat;
        // Height and noise gains are signed (moisture falls with height), so they are only
        // required to be finite.
        finite(&[
            ("habitat.light_base", h.light_base),
            ("habitat.light_height_gain", h.light_height_gain),
            ("habitat.light_noise_gain", h.light_noise_gain),
            ("habitat.moisture_base", h.moisture_base),
            ("habitat.moisture_height_gain", h.moisture_height_gain),
            ("habitat.moisture_noise_gain", h.moisture_noise_gain),
        ])?;
        fraction("habitat.moisture_min", h.moisture_min)?;
        positive("habitat.noise_wavelength[0]", h.noise_wavelength[0])?;
        positive("habitat.noise_wavelength[1]", h.noise_wavelength[1])?;
        if h.noise_wavelength[1] < h.noise_wavelength[0] {
            return Err(format!(
                "habitat.noise_wavelength is inverted: {:?}",
                h.noise_wavelength
            ));
        }

        let w = &self.weather;
        finite_nonnegative(&[
            ("weather.amplitude", w.amplitude),
            ("weather.walk_deg_per_min", w.walk_deg_per_min),
        ])?;
        if w.blobs_per_channel > 0 {
            positive("weather.blob_radius_deg", w.blob_radius_deg)?;
            if w.blob_radius_deg > 180.0 {
                return Err(format!(
                    "weather.blob_radius_deg {} exceeds a hemisphere-and-a-half (180)",
                    w.blob_radius_deg
                ));
            }
            if w.periods_min.is_empty() {
                return Err("weather.periods_min is empty but blobs_per_channel > 0".into());
            }
            for (i, &period) in w.periods_min.iter().enumerate() {
                positive(&format!("weather.periods_min[{i}]"), period)?;
            }
        }

        let o = &self.organism;
        finite_nonnegative(&[
            ("organism.speed_max", o.speed_max),
            ("organism.mouth_rate", o.mouth_rate),
            ("organism.sense_radius", o.sense_radius),
            ("organism.maintenance", o.maintenance),
            ("organism.move_cost", o.move_cost),
            ("organism.sense_cost", o.sense_cost),
            ("organism.oxidation_rate", o.oxidation_rate),
            ("organism.reserve_energy_density", o.reserve_energy_density),
            ("organism.growth_rate", o.growth_rate),
            ("organism.build_cost", o.build_cost),
            ("organism.gestation_seconds", o.gestation_seconds),
            ("organism.min_structure", o.min_structure),
        ])?;
        positive("organism.structure_adult", o.structure_adult)?;
        positive("organism.reserve_max", o.reserve_max)?;
        positive("organism.energy_max", o.energy_max)?;
        positive("organism.max_age_seconds", o.max_age_seconds)?;
        fraction("organism.assimilation_material", o.assimilation_material)?;
        fraction("organism.assimilation_energy", o.assimilation_energy)?;
        fraction("organism.oxidation_threshold", o.oxidation_threshold)?;
        fraction("organism.oxidation_efficiency", o.oxidation_efficiency)?;
        fraction("organism.growth_reserve_min", o.growth_reserve_min)?;
        fraction("organism.child_structure_fraction", o.child_structure_fraction)?;
        fraction("organism.child_reserve_fraction", o.child_reserve_fraction)?;
        fraction("organism.child_energy_fraction", o.child_energy_fraction)?;
        positive("organism.body_extent_max", o.body_extent_max)?;
        if o.body_extent_max > cubarium_surface::MAX_LOCAL_RADIUS {
            return Err(format!(
                "organism.body_extent_max {} exceeds MAX_LOCAL_RADIUS {}",
                o.body_extent_max,
                cubarium_surface::MAX_LOCAL_RADIUS
            ));
        }
        // Grazing stores `e_r · η_m` per unit of food carrying `e_p`; the audit needs the
        // food to cover what the reserve stores.
        if self.producer.energy_density < o.reserve_energy_density * o.assimilation_material {
            return Err(format!(
                "producer.energy_density {} is below reserve_energy_density * assimilation_material {}",
                self.producer.energy_density,
                o.reserve_energy_density * o.assimilation_material
            ));
        }

        let dr = &self.drives;
        // Escrow is paid out of the parent's reserve at conception, which requires
        // `R >= bud_reserve * R_max`; a child that needs more than that could never be born.
        let child_material =
            o.child_structure_fraction * o.structure_adult + o.child_reserve_fraction * o.reserve_max;
        let conception_reserve = dr.bud_reserve * o.reserve_max;
        if child_material > conception_reserve {
            return Err(format!(
                "child material {child_material} exceeds the conception reserve {conception_reserve}"
            ));
        }

        finite_nonnegative(&[
            ("drives.w_food", dr.w_food),
            ("drives.w_detritus", dr.w_detritus),
            ("drives.w_persist", dr.w_persist),
            ("drives.w_crowd", dr.w_crowd),
            ("drives.feed_min", dr.feed_min),
            ("drives.bud_min_age_seconds", dr.bud_min_age_seconds),
            ("drives.turn_rate_max_deg", dr.turn_rate_max_deg),
            ("drives.turn_noise", dr.turn_noise),
            ("drives.birth_offset_px", dr.birth_offset_px),
        ])?;
        fraction("drives.seek_on", dr.seek_on)?;
        fraction("drives.seek_off", dr.seek_off)?;
        fraction("drives.rest_effort", dr.rest_effort)?;
        fraction("drives.feed_effort", dr.feed_effort)?;
        fraction("drives.bud_reserve", dr.bud_reserve)?;
        fraction("drives.bud_energy", dr.bud_energy)?;
        positive("drives.tau_hunger_seconds", dr.tau_hunger_seconds)?;
        if dr.seek_off >= dr.seek_on {
            return Err(format!(
                "drives.seek_off {} must be below drives.seek_on {} for hysteresis",
                dr.seek_off, dr.seek_on
            ));
        }
        if dr.birth_offset_px > cubarium_surface::MAX_LOCAL_RADIUS {
            return Err(format!(
                "drives.birth_offset_px {} exceeds MAX_LOCAL_RADIUS {}",
                dr.birth_offset_px,
                cubarium_surface::MAX_LOCAL_RADIUS
            ));
        }

        let f = &self.founders;
        fraction("founders.initial_reserve_fraction", f.initial_reserve_fraction)?;
        fraction("founders.initial_energy_fraction", f.initial_energy_fraction)?;

        let c = &self.capacity;
        if c.max_organisms == 0 {
            return Err("capacity.max_organisms is zero".into());
        }
        if c.max_neighbors == 0 {
            return Err("capacity.max_neighbors is zero".into());
        }
        positive("capacity.checkpoint_seconds", c.checkpoint_seconds)?;
        positive("capacity.telemetry_seconds", c.telemetry_seconds)?;
        if f.count > c.max_organisms {
            return Err(format!(
                "founders.count {} exceeds capacity.max_organisms {}",
                f.count, c.max_organisms
            ));
        }

        Ok(())
    }
}

fn finite(fields: &[(&str, f64)]) -> Result<(), String> {
    for &(name, v) in fields {
        if !v.is_finite() {
            return Err(format!("{name} is not finite: {v}"));
        }
    }
    Ok(())
}

fn finite_nonnegative(fields: &[(&str, f64)]) -> Result<(), String> {
    finite(fields)?;
    for &(name, v) in fields {
        if v < 0.0 {
            return Err(format!("{name} is negative: {v}"));
        }
    }
    Ok(())
}

fn positive(name: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || v <= 0.0 {
        return Err(format!("{name} must be finite and positive, got {v}"));
    }
    Ok(())
}

fn fraction(name: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || !(0.0..=1.0).contains(&v) {
        return Err(format!("{name} must be in [0, 1], got {v}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_config_is_valid() {
        WorldConfig::default().validate().expect("defaults must be a well-defined world");
    }

    /// One mutation of a valid config, and the substring its rejection must mention.
    type Case = (&'static str, fn(&mut WorldConfig));

    /// Every rejection reason named in the doc comment, one mutation at a time.
    #[test]
    fn invalid_configs_are_rejected_with_a_named_reason() {
        let cases: Vec<Case> = vec![
            ("version", |c| c.version = CONFIG_VERSION + 1),
            ("producer.growth", |c| c.producer.growth = -1.0),
            ("producer.growth", |c| c.producer.growth = f64::NAN),
            ("producer.max", |c| c.producer.max = 0.0),
            ("producer.initial_fraction", |c| c.producer.initial_fraction = 1.5),
            ("detritus.decomposition", |c| c.detritus.decomposition = f64::INFINITY),
            ("nutrient.diffusion", |c| c.nutrient.diffusion = -0.1),
            ("habitat.moisture_min", |c| c.habitat.moisture_min = 2.0),
            ("habitat.noise_wavelength[0]", |c| c.habitat.noise_wavelength[0] = 0.0),
            ("habitat.noise_wavelength", |c| c.habitat.noise_wavelength = [1.4, 0.6]),
            ("weather.amplitude", |c| c.weather.amplitude = -0.1),
            ("weather.blob_radius_deg", |c| c.weather.blob_radius_deg = 0.0),
            ("weather.blob_radius_deg", |c| c.weather.blob_radius_deg = 200.0),
            ("weather.periods_min", |c| c.weather.periods_min.clear()),
            ("weather.periods_min[1]", |c| c.weather.periods_min[1] = 0.0),
            ("organism.structure_adult", |c| c.organism.structure_adult = 0.0),
            ("organism.maintenance", |c| c.organism.maintenance = -1.0),
            ("organism.assimilation_material", |c| c.organism.assimilation_material = 1.2),
            ("organism.assimilation_energy", |c| c.organism.assimilation_energy = -0.01),
            ("organism.body_extent_max", |c| c.organism.body_extent_max = 64.0),
            ("child material", |c| c.organism.child_reserve_fraction = 0.9),
            ("drives.seek_off", |c| c.drives.seek_off = c.drives.seek_on),
            ("drives.seek_off", |c| c.drives.seek_off = 0.9),
            ("drives.tau_hunger_seconds", |c| c.drives.tau_hunger_seconds = 0.0),
            ("drives.w_food", |c| c.drives.w_food = -1.0),
            ("founders.initial_reserve_fraction", |c| c.founders.initial_reserve_fraction = 1.1),
            ("founders.count", |c| c.founders.count = 100_000),
            ("capacity.max_organisms", |c| c.capacity.max_organisms = 0),
            ("capacity.max_neighbors", |c| c.capacity.max_neighbors = 0),
            ("capacity.checkpoint_seconds", |c| c.capacity.checkpoint_seconds = 0.0),
            ("capacity.telemetry_seconds", |c| c.capacity.telemetry_seconds = -5.0),
        ];
        for (expect, mutate) in cases {
            let mut cfg = WorldConfig::default();
            mutate(&mut cfg);
            let err = cfg.validate().unwrap_err();
            assert!(err.contains(expect), "expected an error mentioning {expect}, got: {err}");
        }
    }

    #[test]
    fn signed_gains_and_zero_blob_counts_stay_valid() {
        let mut cfg = WorldConfig::default();
        // The moisture height gain is negative by default; light may be too.
        cfg.habitat.light_height_gain = -0.5;
        cfg.validate().unwrap();
        // No weather at all is a legal (static) world; its periods are then unused.
        cfg.weather.blobs_per_channel = 0;
        cfg.weather.periods_min.clear();
        cfg.validate().unwrap();
    }

    #[test]
    fn serde_round_trips_through_json_like_defaults() {
        let cfg = WorldConfig::default();
        let bytes = postcard::to_allocvec(&cfg).unwrap();
        let back: WorldConfig = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(cfg, back);
        back.validate().unwrap();
    }
}
