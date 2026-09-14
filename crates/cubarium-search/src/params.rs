//! The joint ecological parameter vector this milestone searches.
//!
//! Every entry names a field that **already exists** in [`WorldConfig`] or in
//! [`FixedHunterProfile`]; nothing here invents new biology to create a knob. Bounds are
//! hypotheses around the shipped defaults, not established viable ranges, and they are
//! deliberately allowed to straddle the core's own validity constraints so that an invalid
//! candidate is *recorded as rejected* rather than quietly repaired (see `drives.bud_reserve`).
//!
//! Parameters that are plausible candidates but are **not** searched in this milestone are
//! listed in [`EXCLUDED`], with the reason.

use cubarium_core::WorldConfig;
use cubarium_core::hunter::FixedHunterProfile;

/// One searched scalar.
#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    /// Dotted path: `<section>.<field>` for the world config, `hunter.<field>` for the profile.
    pub name: &'static str,
    pub lo: f64,
    pub hi: f64,
    /// The value the shipped default carries, recorded so a run can say what it moved away from.
    pub default: f64,
    pub why: &'static str,
}

/// The thirteen searched parameters: five for production and recycling, six for prey cost,
/// intake and reproduction, and two for the apex's income per paid attempt.
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "producer.growth",
        lo: 0.003,
        hi: 0.020,
        default: 0.008,
        why: "primary production; every other income in the world is derived from it",
    },
    ParamSpec {
        name: "producer.mortality",
        lo: 0.0003,
        hi: 0.0040,
        default: 0.001,
        why: "the P->D leak that keeps standing crop turning over and feeds the litter layer",
    },
    ParamSpec {
        name: "detritus.decomposition",
        lo: 0.0005,
        hi: 0.0080,
        default: 0.002,
        why: "D->N: the rate at which the nutrient loop actually closes",
    },
    ParamSpec {
        name: "nutrient.half_saturation",
        lo: 0.05,
        hi: 0.80,
        default: 0.25,
        why: "K_N in the Monod uptake term; how nutrient-limited regrowth is after a graze",
    },
    ParamSpec {
        name: "fruit.ripen",
        lo: 0.005,
        hi: 0.060,
        default: 0.02,
        why: "the second plant food channel; frugivore variety depends on it existing",
    },
    ParamSpec {
        name: "organism.maintenance",
        lo: 0.0020,
        hi: 0.0100,
        default: 0.005,
        why: "baseline upkeep: the income floor every animal, prey and apex, has to clear",
    },
    ParamSpec {
        name: "organism.mouth_rate",
        lo: 0.020,
        hi: 0.120,
        default: 0.05,
        why: "intake ceiling; how fast standing crop can become animal reserve",
    },
    ParamSpec {
        name: "organism.intake_half_saturation",
        lo: 0.10,
        hi: 1.00,
        default: 0.45,
        why: "K_P: how thin a patch can still feed a grazer, so how survivable a grazed world is",
    },
    ParamSpec {
        name: "organism.growth_rate",
        lo: 0.004,
        hi: 0.025,
        default: 0.01,
        why: "juvenile maturation speed; gates whether offspring ever become breeding adults",
    },
    ParamSpec {
        name: "drives.bud_reserve",
        lo: 0.55,
        hi: 0.92,
        default: 0.7,
        why: "reproduction threshold. Its range straddles the core's own `child material \
              exceeds the conception reserve` constraint at 0.60, so the harness is required \
              to reject and record part of its own search space",
    },
    ParamSpec {
        name: "drives.bud_min_age_seconds",
        lo: 45.0,
        hi: 400.0,
        default: 120.0,
        why: "prey generation time: the dominant term in how fast a lineage can recover",
    },
    ParamSpec {
        name: "hunter.capture_base",
        lo: 0.30,
        hi: 0.90,
        default: 0.65,
        why: "apex income per paid attempt; the direct lever on predator survival",
    },
    ParamSpec {
        name: "hunter.digest_rate",
        lo: 0.03,
        hi: 0.30,
        default: 0.10,
        why: "how fast a carried carcass becomes usable reserve, so how long a kill lasts",
    },
];

/// Candidates deliberately left out of this milestone's vector, and why. Recorded so the
/// exclusion is a decision rather than an oversight.
pub const EXCLUDED: &[(&str, &str)] = &[
    (
        "dormancy::{SUSTAIN_TICKS, STAGGER_TICKS, RECHECK_TICKS, PREY_RADIUS_PX, PREY_REQUIRED, \
         MAINTENANCE_PER_STRUCTURE_SECOND, EMERGENCE_RESERVE_FRACTION, EMERGENCE_ENERGY_FRACTION}",
        "hardcoded `pub const` in crates/cubarium-core/src/dormancy.rs; these govern whether a \
         paid apex offspring ever emerges and are the strongest apex-side candidates, but \
         making them searchable means adding a persisted policy config and bumping the \
         snapshot schema - out of scope here",
    ),
    (
        "encounter::{MATING_RADIUS_PX, COMBAT_RADIUS_PX, INJURY_ADULT_FRACTION}",
        "hardcoded `pub const` in crates/cubarium-core/src/encounter.rs; same reason",
    ),
    (
        "water.{rain_rate, evap, flow, algae_light}",
        "configurable and ecologically real, but water moves the habitat as well as the \
         ecology; changing it changes what a seed means. Held fixed so this milestone's \
         candidates are comparable on one landscape",
    ),
    (
        "habitat.*, weather.*",
        "these define the landscape and the seed's meaning, not the ecology running on it",
    ),
    (
        "founders.{count, kinds}",
        "the initial stock is an accounted input held equal across candidates, not a searched \
         parameter (handoff: comparable starting resources)",
    ),
    (
        "organism.{assimilation_material, assimilation_energy, reserve_energy_density}, \
         producer.energy_density, fruit.energy_density",
        "energy-density and efficiency terms are cross-constrained by `WorldConfig::validate`; \
         searching them jointly would spend most of the budget on rejections",
    ),
    (
        "hunter.{reproduce_min_age_seconds, reproduce_interval_seconds, gestation_seconds}",
        "real knobs, but their defaults (1200 s / 1800 s / 120 s) are far longer than this \
         milestone's horizons, so a short run cannot score them. Named here for the \
         longer-horizon follow-up",
    ),
    (
        "capacity.max_organisms",
        "a budget ceiling, not an ecological parameter; held at the default so a candidate \
         cannot win by being allowed more bodies",
    ),
];

/// The default vector, in [`PARAMS`] order.
pub fn defaults() -> Vec<f64> {
    PARAMS.iter().map(|p| p.default).collect()
}

/// `(lo, hi)` per parameter, in [`PARAMS`] order.
pub fn bounds() -> Vec<(f64, f64)> {
    PARAMS.iter().map(|p| (p.lo, p.hi)).collect()
}

/// Clamp every component into its bounds. Used after mutation and crossover, so the search
/// never proposes a value outside the declared box; it may still propose a value the core
/// itself rejects, which is the point.
pub fn clamp(values: &mut [f64]) {
    for (v, p) in values.iter_mut().zip(PARAMS) {
        if !v.is_finite() {
            *v = p.default;
        }
        *v = v.clamp(p.lo, p.hi);
    }
}

/// Write the vector into a config and a hunter profile.
///
/// Errors only on a length mismatch or a non-finite component; a value the *core* considers
/// invalid is written through unchanged, so that `WorldConfig::validate` is the single place
/// that decides validity.
pub fn apply(
    values: &[f64],
    config: &mut WorldConfig,
    profile: &mut FixedHunterProfile,
) -> Result<(), String> {
    if values.len() != PARAMS.len() {
        return Err(format!(
            "parameter vector has {} components, expected {}",
            values.len(),
            PARAMS.len()
        ));
    }
    for (v, p) in values.iter().zip(PARAMS) {
        if !v.is_finite() {
            return Err(format!("{} is not finite: {v}", p.name));
        }
        let v = *v;
        match p.name {
            "producer.growth" => config.producer.growth = v,
            "producer.mortality" => config.producer.mortality = v,
            "detritus.decomposition" => config.detritus.decomposition = v,
            "nutrient.half_saturation" => config.nutrient.half_saturation = v,
            "fruit.ripen" => config.fruit.ripen = v,
            "organism.maintenance" => config.organism.maintenance = v,
            "organism.mouth_rate" => config.organism.mouth_rate = v,
            "organism.intake_half_saturation" => config.organism.intake_half_saturation = v,
            "organism.growth_rate" => config.organism.growth_rate = v,
            "drives.bud_reserve" => config.drives.bud_reserve = v,
            "drives.bud_min_age_seconds" => config.drives.bud_min_age_seconds = v,
            "hunter.capture_base" => profile.capture_base = v,
            "hunter.digest_rate" => profile.digest_rate = v,
            other => return Err(format!("no writer for parameter {other}")),
        }
    }
    Ok(())
}

/// Read the vector back out, so a round trip can be tested and a replay can be checked.
pub fn read(config: &WorldConfig, profile: &FixedHunterProfile) -> Vec<f64> {
    PARAMS
        .iter()
        .map(|p| match p.name {
            "producer.growth" => config.producer.growth,
            "producer.mortality" => config.producer.mortality,
            "detritus.decomposition" => config.detritus.decomposition,
            "nutrient.half_saturation" => config.nutrient.half_saturation,
            "fruit.ripen" => config.fruit.ripen,
            "organism.maintenance" => config.organism.maintenance,
            "organism.mouth_rate" => config.organism.mouth_rate,
            "organism.intake_half_saturation" => config.organism.intake_half_saturation,
            "organism.growth_rate" => config.organism.growth_rate,
            "drives.bud_reserve" => config.drives.bud_reserve,
            "drives.bud_min_age_seconds" => config.drives.bud_min_age_seconds,
            "hunter.capture_base" => profile.capture_base,
            "hunter.digest_rate" => profile.digest_rate,
            other => panic!("no reader for parameter {other}"),
        })
        .collect()
}

/// A stable fingerprint of a parameter vector, used to cache repeat evaluations of an elite.
/// Bit-exact on the `f64` pattern: two vectors share a fingerprint only if they are identical.
pub fn fingerprint(values: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in values {
        for b in v.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
    }
    h
}

/// The vector as `name -> value`, for a result row. **Readable, not authoritative**: a decimal
/// JSON round trip is not guaranteed to return the same `f64` bit for bit, and one ULP on
/// `fruit.ripen` is a different world. Use [`bit_labels`] for anything that has to reproduce.
pub fn labelled(values: &[f64]) -> serde_json::Map<String, serde_json::Value> {
    PARAMS
        .iter()
        .zip(values)
        .map(|(p, v)| (p.name.to_string(), serde_json::json!(v)))
        .collect()
}

/// The exact IEEE-754 bit pattern of each component, as 16 lowercase hex digits, in
/// [`PARAMS`] order. This is what a replay reads: it is the only encoding that is guaranteed
/// to hand the simulation back the number it was given.
pub fn bit_labels(values: &[f64]) -> Vec<String> {
    values.iter().map(|v| format!("{:016x}", v.to_bits())).collect()
}

/// Inverse of [`bit_labels`], with the length and the digits checked.
pub fn from_bit_labels(labels: &[String]) -> Result<Vec<f64>, String> {
    if labels.len() != PARAMS.len() {
        return Err(format!(
            "recorded vector has {} components, expected {}",
            labels.len(),
            PARAMS.len()
        ));
    }
    labels
        .iter()
        .zip(PARAMS)
        .map(|(text, p)| {
            u64::from_str_radix(text, 16)
                .map(f64::from_bits)
                .map_err(|e| format!("{}: {text:?} is not 16 hex digits: {e}", p.name))
        })
        .collect()
}
