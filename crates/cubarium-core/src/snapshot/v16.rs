//! Frozen schema 16 `WorldState`, immediately before the ring world.
//!
//! Schema 17 **refuses** a schema 16 snapshot, and that refusal is untouched: the product
//! never calls [`super::decode_snapshot`] on a v16 file and never will. This mirror exists
//! for one purpose — the `CubeProjection` comparator of
//! `design/flat-world-plan-2026-09-16.md` §4 reads a v16 **payload** through
//! [`decode_v16`], the same `decode_exact` mechanism the existing schema-refusal tests use
//! for 7–14. The product refuses old worlds; only the comparator looks inside one.
//!
//! **Why this mirror carries its own config.** Every earlier mirror reuses the live
//! [`crate::config::WorldConfig`], which works only while the config's shape does not move.
//! Schema 17 moves it: `topology` and `world_scale` are appended and `version` goes 8 → 9.
//! So schema 16's config is frozen here as [`WorldConfigV16`], `version` followed by the
//! fifteen blocks that did not move — which are exactly
//! [`super::projection::ConfigProjection`]. Postcard writes a struct as its fields
//! concatenated with no framing, so `{ version, rest }` is byte-for-byte the flat schema 16
//! `WorldConfig`; the comparator's own fixture is the proof, and
//! `a_v16_config_is_version_then_the_projection` pins it directly.
//!
//! Every other nested type is unchanged between 16 and 17 — `Fields` and
//! `EcologyV1State` hold `Vec<f64>`, whose *length* is now a property of the topology but
//! whose *shape* is not — so they are reused rather than duplicated.

use serde::{Deserialize, Serialize};

use crate::accounting::EnergyCorrection;
use crate::care::CareState;
use crate::dormancy::ApexDormancyState;
use crate::encounter::ApexEncounterState;
use crate::fields::{EcologyV1State, Fields};
use crate::habitat::Weather;
use crate::hunter::HunterState;
use crate::ids::Slots;
use crate::neural::NeuralState;
use crate::organism::Organism;
use crate::quiet::QuietState;

use super::projection::{ConfigProjection, CubeProjection};
use super::{MAGIC, SnapshotError, SnapshotMeta, decode_exact};

pub const SCHEMA_V16: u32 = 16;

/// The `CONFIG_VERSION` a schema 16 world was written with. Asserted explicitly by the
/// comparator, never folded into the projection.
pub const CONFIG_VERSION_V16: u32 = 8;

/// Schema 16's `WorldConfig`: `version`, then the fifteen blocks schema 17 left alone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldConfigV16 {
    pub version: u32,
    pub rest: ConfigProjection,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldStateV16 {
    pub config: WorldConfigV16,
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

impl From<&WorldStateV16> for CubeProjection {
    fn from(s: &WorldStateV16) -> CubeProjection {
        CubeProjection {
            config: s.config.rest.clone(),
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

/// Read a **schema 16 snapshot file** — header and all — and decode its payload through the
/// frozen mirror.
///
/// This is deliberately *not* [`super::decode_snapshot`]: that function refuses schema 16 and
/// keeps refusing it. This one insists on schema 16, checks the same magic, length and CRC,
/// and decodes with the same exact-length rule, so a v16 payload with trailing bytes is
/// refused here too. It never calls `validate`: a schema 16 world is not a world this build
/// can run, only one it can read the numbers out of.
pub fn decode_v16(bytes: &[u8]) -> Result<(SnapshotMeta, WorldStateV16), SnapshotError> {
    let take = |at: usize, n: usize| -> Result<&[u8], SnapshotError> {
        bytes.get(at..at + n).ok_or(SnapshotError::Truncated)
    };
    if bytes.len() < MAGIC.len() {
        return Err(SnapshotError::Truncated);
    }
    if bytes[..4] != MAGIC {
        return Err(SnapshotError::BadMagic);
    }
    let schema = u32::from_le_bytes(take(4, 4)?.try_into().expect("4 bytes"));
    if schema != SCHEMA_V16 {
        return Err(SnapshotError::UnsupportedSchema(schema));
    }
    let id_len = u16::from_le_bytes(take(8, 2)?.try_into().expect("2 bytes")) as usize;
    let build_id = String::from_utf8(take(10, id_len)?.to_vec())
        .map_err(|e| SnapshotError::Decode(format!("build id is not UTF-8: {e}")))?;
    let at = 10 + id_len;
    let payload_len = u64::from_le_bytes(take(at, 8)?.try_into().expect("8 bytes"));
    let crc32 = u32::from_le_bytes(take(at + 8, 4)?.try_into().expect("4 bytes"));
    let want = usize::try_from(payload_len).map_err(|_| SnapshotError::Truncated)?;
    let payload = bytes.get(at + 12..).ok_or(SnapshotError::Truncated)?;
    if payload.len() != want {
        return Err(SnapshotError::Truncated);
    }
    if crc32fast::hash(payload) != crc32 {
        return Err(SnapshotError::BadChecksum);
    }
    let state = decode_exact::<WorldStateV16>(payload, schema)?;
    Ok((
        SnapshotMeta {
            schema,
            build_id,
            payload_len,
            crc32,
        },
        state,
    ))
}
