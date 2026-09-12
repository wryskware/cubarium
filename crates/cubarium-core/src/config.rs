//! World configuration: every rate, bound, and toggle, versioned and serializable.
//!
//! Defaults are the initial tuning hypotheses in `design/m2-world-spec.md`. Units:
//! material `m`, energy `e`, seconds, pixels. Per-tick quantities are derived by the
//! consumer by multiplying per-second rates with `DT`; the config never stores per-tick
//! values so that the tick rate can change without editing worlds.

use serde::{Deserialize, Serialize};

/// Bumped whenever a field's meaning changes; stored in snapshots.
pub const CONFIG_VERSION: u32 = 7;

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
    pub water: WaterConfig,
    pub fruit: FruitConfig,
    pub organism: OrganismConfig,
    pub drives: DriveConfig,
    pub founders: FounderConfig,
    pub mutation: MutationConfig,
    pub capacity: CapacityConfig,
    pub mechanisms: MechanismToggles,
}

/// The fruit pool `F` of `design/fauna-v2.md` "Fruit": a fifth conserved per-cell material
/// that rich producers ripen into, that drops back to detritus, and that grazers with
/// `diet ≥ 0.5` eat first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FruitConfig {
    /// Ripening `P → F` per second: `ripen · P · (P/P_max − fruit_min)⁺ · L`.
    pub ripen: f64,
    /// Fraction of `P_max` a cell must exceed before it fruits.
    pub fruit_min: f64,
    /// Drop `F → D` per second.
    pub drop: f64,
    /// `e_f`: energy per material unit of fruit (e/m); must be at least the producer's,
    /// since ripening spends light to add the difference.
    pub energy_density: f64,
}

/// Sparse mutation at birth (`design/fauna-v2.md` "Mutation"), active when
/// `mechanisms.mutation` is on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MutationConfig {
    /// `p_mut`: probability that a child differs from its parent at all.
    pub probability: f64,
    /// Gaussian step as a fraction of each locus range.
    pub step: f64,
}

/// One founder kind (`design/fauna-v2.md` "Founders come in kinds"): how many, and which
/// genome fields it fixes. An unset field takes the v1 founder value (or, for `hue`, the
/// founder's own draw).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FounderKind {
    pub name: String,
    pub count: u32,
    pub diet: Option<f32>,
    pub depth: Option<f32>,
    pub speed: Option<f32>,
    pub size: Option<f32>,
    /// Genome range 0.5–2; multiplies maintenance.
    pub metabolism: Option<f32>,
    pub swim: Option<f32>,
    pub hue: Option<f32>,
    pub form: Option<u8>,
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
    /// `e_d_max`: maximum retained energy per material unit (e/m). Equal to `e_r` since
    /// fauna v2 (2026-09-12), so fresh detritus is fully edible and a soil scavenger can
    /// live on litter; `De ≤ e_d_max · D` holds everywhere.
    pub energy_cap: f64,
    /// Fraction of a cell's `D` (and of its `De`, in the same proportion) that slides to
    /// its downhill neighbour per second. A pure transfer: nothing is created or
    /// destroyed. Must satisfy `fall · DT ≤ 1` so a cell can never over-drain.
    pub fall: f64,
    /// Initial litter (m): a new world starts with `D = initial_dark · (1 − L₀)` and
    /// `De = e_d_max · D` (fully charged) per cell, the layer a world that has been shedding for a
    /// while would hold, so the dark soil is littered and the lit canopy clean
    /// (`design/stratified-world.md`). Initial material, booked with the producer seed.
    pub initial_dark: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NutrientConfig {
    /// Initial `N` per cell (m).
    pub initial: f64,
    /// Diffusion exchange coefficient per second per edge (dimensionless per tick after `· DT`).
    pub diffusion: f64,
    /// `K_N`: half-saturation constant of the Monod uptake term `N / (N + K_N)` (m).
    /// Zero makes uptake nutrient-independent again.
    pub half_saturation: f64,
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
    /// Terrain hollows for standing water: `z = h + basin_gain · n_b(p)` with the patch
    /// noise sampled at a third offset (`design/water.md`). Zero makes the terrain the
    /// bare embedded height, so nothing pools on the level top face.
    pub basin_gain: f64,
}

/// Rain, flow, pools and evaporation (`design/water.md`). Water is an open, audited
/// quantity, not material: rain adds it, evaporation removes it, every tick
/// `Δ Σw = rain_in − evap_out`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WaterConfig {
    /// Depth added per second per unit of moisture blob sum above `rain_threshold` (d/s).
    pub rain_rate: f64,
    /// Moisture *weather blob sum* a cell needs before it rains there; showers are the
    /// cores of the moving blobs, never the static habitat.
    pub rain_threshold: f64,
    /// Flow coefficient per second: per edge, `q = flow · dt · (s_a − s_b)` from the higher
    /// surface level to the lower, capped at a quarter of the source depth per substep.
    pub flow: f64,
    /// Evaporation per second per unit light: `evap · max(L, evap_floor) · w`.
    pub evap: f64,
    /// The light evaporation never falls below, so unlit soil still dries slowly and a moat
    /// cannot fill without bound. A fraction in `[0, 1]`.
    pub evap_floor: f64,
    /// Surface level per unit depth: `s = z + depth_gain · w`.
    pub depth_gain: f64,
    /// Producer growth sees `W_eff = clamp(W + wet_gain · min(w, 1), W_min, 1)`.
    pub wet_gain: f64,
    /// Depth above which producer growth is scaled by `max(0, 1 − (w − flood) / flood)`.
    pub flood: f64,
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
    /// `K_P`: half-saturation of the type-II intake term `X/(X + K_P)` on the food a cell
    /// holds (`P` for grazing, `D_eff` for scavenging). Zero restores a linear request.
    pub intake_half_saturation: f64,
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
    /// `k` while Resting: the fraction of `turn_rate_max` and of the turn-noise injection a
    /// resting body is allowed. Zero holds the heading while the noise decays.
    pub rest_turn_fraction: f64,
    /// `k` while Feeding. Both fractions at 1 restore the ungated behaviour of the E2 batches.
    pub feed_turn_fraction: f64,
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
    /// Gain of the depth term `w_depth · (h_pref − h) · up` (`design/fauna-v2.md`).
    pub w_depth: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FounderConfig {
    /// Founders placed at world creation, uniformly by area over the surface, when `kinds`
    /// is empty: the v1 path, one genotype with drawn hues.
    pub count: u32,
    pub initial_reserve_fraction: f64,
    pub initial_energy_fraction: f64,
    /// The founder kinds; when non-empty, `count` is ignored and each kind places its own
    /// founders (`design/fauna-v2.md`).
    pub kinds: Vec<FounderKind>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CapacityConfig {
    pub max_organisms: u32,
    pub max_neighbors: u32,
    pub checkpoint_seconds: f64,
    pub telemetry_seconds: f64,
    /// Seconds between observer field dumps; zero disables them. Carried by the core,
    /// consumed by the host.
    pub field_dump_seconds: f64,
    /// Whether the host writes the per-birth/per-death event log. The world always records
    /// the events; this only says whether anyone writes them down.
    pub event_log: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MechanismToggles {
    pub grazing: bool,
    pub scavenging: bool,
    /// Sparse mutation at birth (`design/fauna-v2.md`); off, every copy is exact.
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
            water: WaterConfig::default(),
            fruit: FruitConfig::default(),
            organism: OrganismConfig::default(),
            drives: DriveConfig::default(),
            founders: FounderConfig::default(),
            mutation: MutationConfig::default(),
            capacity: CapacityConfig::default(),
            mechanisms: MechanismToggles::default(),
        }
    }
}

impl Default for ProducerConfig {
    fn default() -> Self {
        ProducerConfig { growth: 0.008, max: 1.5, uptake_max: 0.5, energy_density: 2.0, mortality: 0.001, initial_fraction: 0.4 }
    }
}

impl Default for DetritusConfig {
    fn default() -> Self {
        DetritusConfig { decomposition: 0.002, energy_cap: 2.0, fall: 0.02, initial_dark: 1.2 }
    }
}

impl Default for NutrientConfig {
    fn default() -> Self {
        NutrientConfig { initial: 0.5, diffusion: 0.05, half_saturation: 0.25 }
    }
}

impl Default for HabitatConfig {
    fn default() -> Self {
        HabitatConfig {
            light_base: 0.45,
            light_height_gain: 0.55,
            light_noise_gain: 0.3,
            moisture_base: 0.8,
            moisture_height_gain: -0.3,
            moisture_noise_gain: 0.4,
            moisture_min: 0.1,
            noise_waves: 6,
            noise_wavelength: [0.6, 1.4],
            basin_gain: 0.15,
        }
    }
}

impl Default for WaterConfig {
    fn default() -> Self {
        WaterConfig {
            rain_rate: 0.6,
            rain_threshold: 0.35,
            flow: 3.0,
            evap: 0.008,
            evap_floor: 0.5,
            depth_gain: 0.4,
            wet_gain: 0.5,
            flood: 1.5,
        }
    }
}

impl Default for FruitConfig {
    fn default() -> Self {
        FruitConfig { ripen: 0.02, fruit_min: 0.3, drop: 0.004, energy_density: 3.0 }
    }
}

impl Default for MutationConfig {
    fn default() -> Self {
        MutationConfig { probability: 0.3, step: 0.08 }
    }
}

impl FounderKind {
    /// The four default kinds of `design/fauna-v2.md`: burrower (soil, 4), grazer (foliage,
    /// 8), glider (canopy, 6), skimmer (the wet floor, 3). `form` indices follow the pack's creature
    /// order lantern 0, sail 1, mossback 2, skimmer 3. The design table gives the glider
    /// `speed` 1.2, above the genome's `speed` range (0.3–1); it is placed at the range's
    /// top, 1.0, and is still the fastest kind through its small `size` (`v_max ∝
    /// size^−0.25`).
    pub fn defaults() -> Vec<FounderKind> {
        let kind = |name: &str, count, diet, depth, speed, size, swim, hue, form| FounderKind {
            name: name.to_string(),
            count,
            diet: Some(diet),
            depth: Some(depth),
            speed: Some(speed),
            size: Some(size),
            metabolism: None,
            swim: Some(swim),
            hue: Some(hue),
            form: Some(form),
        };
        // The burrower runs cool (`metabolism` 0.7, `size` 1.0): a scavenger's income from
        // litter is thin, and its upkeep has to fit it.
        let burrower = FounderKind { metabolism: Some(0.7), ..kind("burrower", 4, 0.10, 0.10, 0.6, 1.0, 0.0, 0.15, 2) };
        vec![
            burrower,
            kind("grazer", 8, 0.85, 0.55, 1.0, 1.0, 0.0, 0.50, 0),
            kind("glider", 6, 0.90, 1.00, 1.0, 0.8, 0.0, 0.85, 1),
            // The skimmer runs cool too: on the same floor litter it otherwise boomed to
            // thirty-five and starved back to none within the hour.
            FounderKind { metabolism: Some(0.7), ..kind("skimmer", 3, 0.20, 0.10, 0.9, 0.9, 1.0, 0.65, 3) },
        ]
    }
}

impl Default for WeatherConfig {
    fn default() -> Self {
        WeatherConfig {
            moving: true,
            blobs_per_channel: 3,
            blob_radius_deg: 55.0,
            amplitude: 0.3,
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
            speed_max: 0.3,
            mouth_rate: 0.05,
            intake_half_saturation: 0.45,
            sense_radius: 6.0,
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
            rest_turn_fraction: 0.0,
            feed_turn_fraction: 0.1,
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
            feed_min: 0.2,
            rest_effort: 0.05,
            feed_effort: 0.2,
            bud_reserve: 0.7,
            bud_energy: 0.3,
            bud_min_age_seconds: 120.0,
            tau_hunger_seconds: 10.0,
            turn_rate_max_deg: 90.0,
            turn_noise: 0.6,
            birth_offset_px: 2.5,
            w_depth: 1.0,
        }
    }
}

impl Default for FounderConfig {
    fn default() -> Self {
        FounderConfig {
            count: 72,
            initial_reserve_fraction: 0.6,
            initial_energy_fraction: 0.7,
            kinds: FounderKind::defaults(),
        }
    }
}

impl Default for CapacityConfig {
    fn default() -> Self {
        CapacityConfig {
            max_organisms: 512,
            max_neighbors: 16,
            checkpoint_seconds: 60.0,
            telemetry_seconds: 5.0,
            field_dump_seconds: 0.0,
            event_log: false,
        }
    }
}

impl Default for MechanismToggles {
    fn default() -> Self {
        MechanismToggles { grazing: true, scavenging: true, mutation: true }
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
            ("detritus.fall", d.fall),
            ("detritus.initial_dark", d.initial_dark),
        ])?;
        // A per-tick fraction above one would drain a cell past zero.
        if d.fall * crate::DT > 1.0 {
            return Err(format!(
                "detritus.fall {} per second exceeds one per tick (DT = {})",
                d.fall,
                crate::DT
            ));
        }

        let n = &self.nutrient;
        finite_nonnegative(&[
            ("nutrient.initial", n.initial),
            ("nutrient.diffusion", n.diffusion),
            ("nutrient.half_saturation", n.half_saturation),
        ])?;

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
            ("habitat.basin_gain", h.basin_gain),
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

        let wa = &self.water;
        finite_nonnegative(&[
            ("water.rain_rate", wa.rain_rate),
            ("water.rain_threshold", wa.rain_threshold),
            ("water.flow", wa.flow),
            ("water.evap", wa.evap),
            ("water.depth_gain", wa.depth_gain),
            ("water.wet_gain", wa.wet_gain),
        ])?;
        positive("water.flood", wa.flood)?;
        fraction("water.evap_floor", wa.evap_floor)?;
        // Evaporation is a per-tick fraction of the depth; above one it would over-drain.
        if wa.evap * crate::DT > 1.0 {
            return Err(format!(
                "water.evap {} per second exceeds one per tick (DT = {})",
                wa.evap,
                crate::DT
            ));
        }

        let fr = &self.fruit;
        finite_nonnegative(&[
            ("fruit.ripen", fr.ripen),
            ("fruit.drop", fr.drop),
            ("fruit.energy_density", fr.energy_density),
        ])?;
        fraction("fruit.fruit_min", fr.fruit_min)?;
        if fr.drop * crate::DT > 1.0 {
            return Err(format!("fruit.drop {} per second exceeds one per tick (DT = {})", fr.drop, crate::DT));
        }
        // Ripening adds `e_f − e_p` per unit from light; a fruit poorer than leaf would need
        // energy to vanish.
        if fr.energy_density < self.producer.energy_density {
            return Err(format!(
                "fruit.energy_density {} is below producer.energy_density {}",
                fr.energy_density, self.producer.energy_density
            ));
        }

        let mu = &self.mutation;
        fraction("mutation.probability", mu.probability)?;
        finite_nonnegative(&[("mutation.step", mu.step)])?;

        let o = &self.organism;
        finite_nonnegative(&[
            ("organism.speed_max", o.speed_max),
            ("organism.mouth_rate", o.mouth_rate),
            ("organism.intake_half_saturation", o.intake_half_saturation),
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
        fraction("organism.rest_turn_fraction", o.rest_turn_fraction)?;
        fraction("organism.feed_turn_fraction", o.feed_turn_fraction)?;
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
            ("drives.w_depth", dr.w_depth),
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
        for (i, k) in f.kinds.iter().enumerate() {
            let who = |field: &str| format!("founders.kinds[{i}] ({}).{field}", k.name);
            let in_range = |field: &str, v: Option<f32>, lo: f32, hi: f32| -> Result<(), String> {
                match v {
                    Some(x) if !x.is_finite() || x < lo || x > hi => {
                        Err(format!("{} = {x}, expected [{lo}, {hi}]", who(field)))
                    }
                    _ => Ok(()),
                }
            };
            in_range("diet", k.diet, 0.0, 1.0)?;
            in_range("depth", k.depth, 0.0, 1.0)?;
            in_range("swim", k.swim, 0.0, 1.0)?;
            in_range("hue", k.hue, 0.0, 1.0)?;
            in_range("speed", k.speed, 0.3, 1.0)?;
            in_range("size", k.size, 0.5, 2.0)?;
            in_range("metabolism", k.metabolism, 0.5, 2.0)?;
            if let Some(form) = k.form
                && form >= crate::genome::MAX_FORMS
            {
                return Err(format!("{} = {form}, expected below {}", who("form"), crate::genome::MAX_FORMS));
            }
        }

        let c = &self.capacity;
        if c.max_organisms == 0 {
            return Err("capacity.max_organisms is zero".into());
        }
        if c.max_neighbors == 0 {
            return Err("capacity.max_neighbors is zero".into());
        }
        finite_nonnegative(&[("capacity.field_dump_seconds", c.field_dump_seconds)])?;
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
            ("detritus.fall", |c| c.detritus.fall = -0.01),
            ("detritus.fall", |c| c.detritus.fall = f64::NAN),
            // `fall · DT` must stay at or below one: DT = 0.05, so 20/s is the ceiling.
            ("detritus.fall", |c| c.detritus.fall = 20.0001),
            ("nutrient.diffusion", |c| c.nutrient.diffusion = -0.1),
            ("water.rain_rate", |c| c.water.rain_rate = -1.0),
            ("water.flow", |c| c.water.flow = f64::NAN),
            ("water.flood", |c| c.water.flood = 0.0),
            ("water.evap", |c| c.water.evap = 20.0001),
            ("water.evap_floor", |c| c.water.evap_floor = 1.5),
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
            ("organism.rest_turn_fraction", |c| c.organism.rest_turn_fraction = 1.5),
            ("organism.feed_turn_fraction", |c| c.organism.feed_turn_fraction = f64::NAN),
            ("fruit.energy_density", |c| c.fruit.energy_density = 1.0),
            ("fruit.fruit_min", |c| c.fruit.fruit_min = 1.5),
            ("fruit.drop", |c| c.fruit.drop = 30.0),
            ("mutation.probability", |c| c.mutation.probability = 1.2),
            ("founders.kinds[0] (burrower).diet", |c| c.founders.kinds[0].diet = Some(1.5)),
            ("founders.kinds[1] (grazer).form", |c| c.founders.kinds[1].form = Some(9)),
            ("founders.kinds[2] (glider).speed", |c| c.founders.kinds[2].speed = Some(0.1)),
            ("detritus.initial_dark", |c| c.detritus.initial_dark = -0.5),
            ("founders.kinds[0] (burrower).metabolism", |c| c.founders.kinds[0].metabolism = Some(0.1)),
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

    /// The host reads worlds from TOML with `#[serde(default, deny_unknown_fields)]`, so a
    /// config written before `detritus.fall` existed must still load, taking the default.
    #[test]
    fn a_config_toml_without_the_fall_rate_takes_the_default() {
        // Exactly the shape `scripts/e2-matrix.py` writes: dotted keys, no `version`.
        let text = "\
seed = 7
producer.growth = 0.008
detritus.decomposition = 0.002
detritus.energy_cap = 1.0
capacity.telemetry_seconds = 5.0
";
        let cfg: WorldConfig = toml::from_str(text).expect("an older config must still load");
        assert_eq!(cfg.seed, 7);
        assert_eq!(cfg.detritus.decomposition, 0.002);
        assert_eq!(cfg.detritus.fall, DetritusConfig::default().fall);
        assert_eq!(cfg.detritus.fall, 0.02);
        // The omitted `version` takes the current one, so validation passes.
        assert_eq!(cfg.version, CONFIG_VERSION);
        cfg.validate().unwrap();

        // And an explicit rate is honoured, including zero (the fall step then never runs).
        let off: WorldConfig = toml::from_str("detritus.fall = 0.0\n").unwrap();
        assert_eq!(off.detritus.fall, 0.0);
        off.validate().unwrap();
    }

    /// `design/fauna-v2.md`: the fauna v2 fields default from the design, and a config written
    /// before them loads with the default kinds, fruit and mutation settings.
    #[test]
    fn a_config_toml_without_fauna_v2_takes_the_design_defaults() {
        let cfg: WorldConfig = toml::from_str("seed = 3\nproducer.growth = 0.008\n").unwrap();
        assert_eq!(cfg.fruit, FruitConfig::default());
        assert_eq!((cfg.fruit.ripen, cfg.fruit.fruit_min, cfg.fruit.drop, cfg.fruit.energy_density), (0.02, 0.3, 0.004, 3.0));
        assert_eq!((cfg.mutation.probability, cfg.mutation.step), (0.3, 0.08));
        assert!(cfg.mechanisms.mutation, "mutation is on by default in fauna v2");
        assert_eq!(cfg.organism.rest_turn_fraction, 0.0);
        assert_eq!(cfg.organism.feed_turn_fraction, 0.1);
        assert_eq!(cfg.drives.w_depth, 1.0);
        let kinds = &cfg.founders.kinds;
        assert_eq!(kinds.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), ["burrower", "grazer", "glider", "skimmer"]);
        assert_eq!(kinds.iter().map(|k| k.count).collect::<Vec<_>>(), [4, 8, 6, 3]);
        assert_eq!(kinds.iter().map(|k| k.form).collect::<Vec<_>>(), [Some(2), Some(0), Some(1), Some(3)]);
        assert_eq!(kinds[3].swim, Some(1.0));
        cfg.validate().unwrap();

        // An explicit empty list is the v1 path; kinds may also be written in full.
        let v1: WorldConfig = toml::from_str("founders.kinds = []\nfounders.count = 12\n").unwrap();
        assert!(v1.founders.kinds.is_empty());
        assert_eq!(v1.founders.count, 12);
        v1.validate().unwrap();
        let one: WorldConfig = toml::from_str(
            "[[founders.kinds]]\nname = \"tester\"\ncount = 3\ndiet = 0.2\nform = 1\n",
        )
        .unwrap();
        assert_eq!(one.founders.kinds.len(), 1);
        assert_eq!(one.founders.kinds[0].diet, Some(0.2));
        assert_eq!(one.founders.kinds[0].depth, None);
        one.validate().unwrap();
    }

    #[test]
    fn a_config_toml_without_water_or_basins_takes_the_defaults() {
        let cfg: WorldConfig = toml::from_str("seed = 3\nproducer.growth = 0.008\n").unwrap();
        assert_eq!(cfg.water, WaterConfig::default());
        assert_eq!(cfg.water.rain_rate, 0.6);
        assert_eq!(cfg.water.rain_threshold, 0.35);
        assert_eq!(cfg.water.flow, 3.0);
        assert_eq!(cfg.water.evap, 0.008);
        assert_eq!(cfg.water.evap_floor, 0.5);
        assert_eq!(cfg.water.depth_gain, 0.4);
        assert_eq!(cfg.water.wet_gain, 0.5);
        assert_eq!(cfg.water.flood, 1.5);
        assert_eq!(cfg.habitat.basin_gain, 0.15);
        cfg.validate().unwrap();
        let dry: WorldConfig = toml::from_str("water.rain_rate = 0.0\nhabitat.basin_gain = 0.0\n").unwrap();
        assert_eq!(dry.water.rain_rate, 0.0);
        assert_eq!(dry.habitat.basin_gain, 0.0);
        dry.validate().unwrap();
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
