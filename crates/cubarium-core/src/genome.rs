//! Genome v1 and phenotype decode.

use serde::{Deserialize, Serialize};

use crate::config::{DriveConfig, OrganismConfig};

/// Bounded heritable parameters. M2 uses one fixed genotype (`Genome::founder`); M3a adds
/// sparse mutation. Every field has a documented closed range enforced by `clamp`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Genome {
    pub version: u32,
    /// 0.5–2: scales adult structure, reserve, energy, mouth (¾ power), speed (−¼ power).
    pub size: f32,
    /// 0.5–2: multiplies maintenance.
    pub metabolism: f32,
    /// 2–12 px sensing radius.
    pub sense: f32,
    /// 0.5–2: multiplies reserve capacity.
    pub reserve: f32,
    /// 0.2–1: multiplies mouth intake rate.
    pub mouth: f32,
    /// 0.3–1: multiplies maximum speed.
    pub speed: f32,
    /// 0–1 cosmetic hue accent.
    pub hue: f32,
    /// Named drives; M2 copies the config defaults, M3a mutates within bounds.
    pub drives: Drives,
}

/// Heritable behavioral gains and thresholds. Ranges: weights 0–2, thresholds 0–1 with
/// `seek_off < seek_on` enforced on decode, `tau_hunger` 1–60 s, `turn_noise` 0–2.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drives {
    pub w_food: f32,
    pub w_detritus: f32,
    pub w_persist: f32,
    pub w_crowd: f32,
    pub seek_on: f32,
    pub seek_off: f32,
    pub feed_min: f32,
    pub rest_effort: f32,
    pub feed_effort: f32,
    pub bud_reserve: f32,
    pub bud_energy: f32,
    pub bud_min_age_seconds: f32,
    pub tau_hunger_seconds: f32,
    pub turn_rate_max_deg: f32,
    pub turn_noise: f32,
}

impl Genome {
    pub const VERSION: u32 = 1;

    /// The M2 founder genotype: all multipliers 1 (mouth 1, speed 1, sense 8), hue from the
    /// argument, drives copied from `DriveConfig`.
    pub fn founder(hue: f32, drives: &DriveConfig) -> Genome {
        let _ = (hue, drives);
        todo!("Genome::founder")
    }

    /// Clamp every field into its documented range (in place). Returns true if anything changed.
    pub fn clamp(&mut self) -> bool {
        todo!("Genome::clamp")
    }

    /// A stable 64-bit digest of the genome (FNV-1a over the little-endian field bytes,
    /// in declaration order) for lineage records.
    pub fn digest(&self) -> u64 {
        todo!("Genome::digest")
    }
}

/// Decoded once at birth. All physical quantities in world units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Phenotype {
    pub structure_adult: f64,
    pub reserve_max: f64,
    pub energy_max: f64,
    pub speed_max: f64,
    pub mouth_rate: f64,
    pub sense_radius: f64,
    pub maintenance: f64,
    /// Body lobes in the body frame: `(offset_x, offset_y, radius)`, extent ≤ config max.
    pub lobes: Vec<(f64, f64, f64)>,
    pub extent: f64,
    pub hue: f32,
    pub drives: Drives,
}

/// Decode per `design/m2-world-spec.md` "Organism representation":
/// `S_adult = size · structure_adult`, `R_max = reserve · size · reserve_max`,
/// `E_max = energy_max · size`, `v_max = speed · speed_max · size^(−0.25)`,
/// `mouth_rate = mouth · mouth_rate · size^0.75`, `sense_radius = sense`,
/// `maintenance = metabolism · maintenance`; lobes: core `(0, 0, 0.9 + 0.5·size)`, head
/// `(1.6·size, 0, 0.6 + 0.3·size)`, and tail `(−1.4·size, 0, 0.5 + 0.2·size)` when
/// `speed > 0.6`; the extent is then clamped by scaling offsets down if it exceeds
/// `body_extent_max` (record nothing; the clamp is a decode rule).
pub fn decode(genome: &Genome, cfg: &OrganismConfig) -> Phenotype {
    let _ = (genome, cfg);
    todo!("decode")
}
