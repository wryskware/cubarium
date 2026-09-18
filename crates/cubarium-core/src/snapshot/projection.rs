//! The `CubeProjection`: the one comparison that can cross the schema 16 → 17 break.
//!
//! `design/flat-world-plan-2026-09-16.md` §4. Schema 17 **refuses** a schema 16 snapshot by
//! design, and `ecology_hash` hashes the config, so neither can carry the claim that a cube
//! world's behaviour is unchanged by the ring work. The projection can, because it is
//! defined as **[`crate::world::WorldState`] verbatim with exactly one substitution**:
//! `config: ConfigProjection`, which is [`crate::config::WorldConfig`] minus the **three**
//! fields that had to move — `version`, `topology` and `world_scale`.
//!
//! Everything else is carried whole, at its own type: the tick, every field vector, the
//! weather, `organisms` **including the allocator's `entries`, `free` and `live`**, every
//! counter and total, and all seven extensions. An enumerated subset would silently drop
//! whatever it forgot; this drops three named fields and nothing else.
//!
//! The three excluded fields are asserted **separately and explicitly** by the comparator
//! (the fixture must report schema 16 / config 8, this build schema 17 / config 9), so a
//! silent version change and a silent world change cannot cancel or mask each other.

use serde::{Deserialize, Serialize};

use crate::accounting::EnergyCorrection;
use crate::care::CareState;
use crate::config::{
    CapacityConfig, DetritusConfig, DriveConfig, FounderConfig, FruitConfig, HabitatConfig,
    MechanismToggles, MutationConfig, NutrientConfig, OrganismConfig, PlantConfig, ProducerConfig,
    WaterConfig, WeatherConfig, WorldConfig,
};
use crate::dormancy::ApexDormancyState;
use crate::encounter::ApexEncounterState;
use crate::fields::{EcologyV1State, Fields};
use crate::habitat::Weather;
use crate::hunter::HunterState;
use crate::ids::Slots;
use crate::neural::NeuralState;
use crate::organism::Organism;
use crate::quiet::QuietState;
use crate::world::WorldState;

/// [`WorldConfig`] minus `version`, `topology` and `world_scale`, in declaration order.
///
/// Every remaining field is behaviour-bearing, so none of them may be dropped: excluding the
/// whole config would let a rate change pass the comparison unnoticed.
///
/// Its postcard encoding is also, by construction, the tail of a schema 16 `WorldConfig` —
/// which is what lets [`super::v16::WorldConfigV16`] be `version` followed by this.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigProjection {
    pub seed: u64,
    pub producer: ProducerConfig,
    pub plant: PlantConfig,
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

impl From<&WorldConfig> for ConfigProjection {
    fn from(c: &WorldConfig) -> ConfigProjection {
        ConfigProjection {
            seed: c.seed,
            producer: c.producer.clone(),
            plant: c.plant.clone(),
            detritus: c.detritus.clone(),
            nutrient: c.nutrient.clone(),
            habitat: c.habitat.clone(),
            weather: c.weather.clone(),
            water: c.water.clone(),
            fruit: c.fruit.clone(),
            organism: c.organism.clone(),
            drives: c.drives.clone(),
            founders: c.founders.clone(),
            mutation: c.mutation.clone(),
            capacity: c.capacity.clone(),
            mechanisms: c.mechanisms.clone(),
        }
    }
}

/// A world state with everything two schemas mean identically, and nothing else.
///
/// Build one from a live [`WorldState`] or from a decoded [`super::v16::WorldStateV16`] and
/// compare them. They are equal exactly when the two builds ran the same world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CubeProjection {
    pub config: ConfigProjection,
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
    pub rain_in_total: f64,
    pub evap_out_total: f64,
    pub care: CareState,
    pub energy_correction: EnergyCorrection,
    pub hunters: HunterState,
    pub quiet: QuietState,
    pub apex_dormancy: ApexDormancyState,
    pub apex_encounters: ApexEncounterState,
    pub neural: NeuralState,
    pub ecology: EcologyV1State,
}

impl From<&WorldState> for CubeProjection {
    fn from(s: &WorldState) -> CubeProjection {
        CubeProjection {
            config: ConfigProjection::from(&s.config),
            tick: s.tick,
            fields: s.fields.clone(),
            weather: s.weather.clone(),
            organisms: s.organisms.clone(),
            births_total: s.births_total,
            deaths_total: s.deaths_total,
            cap_rejections_total: s.cap_rejections_total,
            external_material_in: s.external_material_in,
            light_in_total: s.light_in_total,
            heat_out_total: s.heat_out_total,
            rain_in_total: s.rain_in_total,
            evap_out_total: s.evap_out_total,
            care: s.care.clone(),
            energy_correction: s.energy_correction,
            hunters: s.hunters.clone(),
            quiet: s.quiet.clone(),
            apex_dormancy: s.apex_dormancy.clone(),
            apex_encounters: s.apex_encounters.clone(),
            neural: s.neural.clone(),
            ecology: s.ecology.clone(),
        }
    }
}

/// FNV-1a 64 over the projection's postcard encoding: the one-line CI signal.
///
/// Not a substitute for the field-by-field comparison — it says *that* two runs differ, never
/// *where* — but it is what a report can quote.
pub fn projection_hash(p: &CubeProjection) -> u64 {
    super::fnv1a(&postcard::to_allocvec(p).expect("a projection is always postcard-encodable"))
}

/// The first field that differs, named, or `None` when the two projections are equal.
///
/// A hash mismatch alone cannot be acted on. This walks the fields in declaration order and
/// returns the first disagreement, which is what makes a failure a diagnosis.
pub fn first_difference(a: &CubeProjection, b: &CubeProjection) -> Option<&'static str> {
    macro_rules! check {
        ($($f:ident),* $(,)?) => {
            $(if a.$f != b.$f { return Some(stringify!($f)); })*
        };
    }
    // `config` is compared field by field too, so "the config moved" names which block moved.
    macro_rules! check_config {
        ($($f:ident),* $(,)?) => {
            $(if a.config.$f != b.config.$f { return Some(concat!("config.", stringify!($f))); })*
        };
    }
    check_config!(
        seed, producer, plant, detritus, nutrient, habitat, weather, water, fruit, organism,
        drives, founders, mutation, capacity, mechanisms,
    );
    check!(
        tick,
        fields,
        weather,
        organisms,
        births_total,
        deaths_total,
        cap_rejections_total,
        external_material_in,
        light_in_total,
        heat_out_total,
        rain_in_total,
        evap_out_total,
        care,
        energy_correction,
        hunters,
        quiet,
        apex_dormancy,
        apex_encounters,
        neural,
        ecology,
    );
    None
}
