//! Snapshot encoding: header + postcard payload with CRC32.

use serde::{Deserialize, Serialize};

use crate::world::WorldState;

pub mod v7;

pub use v7::{SCHEMA_V7, WorldStateV7};

/// Bumped whenever `WorldState` or any nested type changes shape. Version 8 appends
/// `WorldState.care`; [`SCHEMA_V7`] payloads are still accepted through [`v7`].
pub const SCHEMA_VERSION: u32 = 8;
pub const MAGIC: [u8; 4] = *b"CUBW";
/// Fixed header length: magic 4, schema 4, build-id length 2, then the build id bytes,
/// then payload length 8 and CRC32 4 (all little-endian).
pub const HEADER_FIXED_BYTES: usize = 4 + 4 + 2 + 8 + 4;

#[derive(Debug, PartialEq)]
pub enum SnapshotError {
    BadMagic,
    UnsupportedSchema(u32),
    Truncated,
    BadChecksum,
    Decode(String),
    Invalid(String),
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SnapshotError {}

/// Header metadata returned by decode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub schema: u32,
    pub build_id: String,
    pub payload_len: u64,
    pub crc32: u32,
}

/// `[magic][schema u32][build_id_len u16][build_id][payload_len u64][crc32 u32][payload]`
/// where payload is `postcard::to_allocvec(state)` and the CRC covers the payload only.
pub fn encode_snapshot(state: &WorldState, build_id: &str) -> Vec<u8> {
    let payload = postcard::to_allocvec(state).expect("WorldState is always postcard-encodable");
    // The build id is length-prefixed with a `u16`; longer ids are truncated at a char
    // boundary rather than corrupting the header.
    let mut cut = build_id.len().min(u16::MAX as usize);
    while cut > 0 && !build_id.is_char_boundary(cut) {
        cut -= 1;
    }
    let id = &build_id.as_bytes()[..cut];

    let mut out = Vec::with_capacity(HEADER_FIXED_BYTES + id.len() + payload.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    out.extend_from_slice(&(id.len() as u16).to_le_bytes());
    out.extend_from_slice(id);
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

/// Validate magic, schema, length, CRC, decode, then `state.validate()`; every failure is a
/// distinct error so the loader can report why an older snapshot was tried.
///
/// Two schemas decode: the current [`SCHEMA_VERSION`], and [`SCHEMA_V7`] through the frozen
/// [`WorldStateV7`] mirror with `care = CareState::default()`. Anything else is
/// [`SnapshotError::UnsupportedSchema`]. `SnapshotMeta.schema` reports what was read, not
/// what the build writes.
pub fn decode_snapshot(bytes: &[u8]) -> Result<(SnapshotMeta, WorldState), SnapshotError> {
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
    if schema != SCHEMA_VERSION && schema != SCHEMA_V7 {
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
        // Both a short read and trailing bytes mean the declared length is not the file.
        return Err(SnapshotError::Truncated);
    }
    if crc32fast::hash(payload) != crc32 {
        return Err(SnapshotError::BadChecksum);
    }
    let state: WorldState = if schema == SCHEMA_V7 {
        postcard::from_bytes::<WorldStateV7>(payload)
            .map(WorldState::from)
            .map_err(|e| SnapshotError::Decode(e.to_string()))?
    } else {
        postcard::from_bytes(payload).map_err(|e| SnapshotError::Decode(e.to_string()))?
    };
    state.validate().map_err(SnapshotError::Invalid)?;
    Ok((SnapshotMeta { schema, build_id, payload_len, crc32 }, state))
}

/// FNV-1a 64 over the postcard encoding of the state (the replay hash in telemetry).
pub fn state_hash(state: &WorldState) -> u64 {
    fnv1a(&postcard::to_allocvec(state).expect("WorldState is always postcard-encodable"))
}

/// FNV-1a 64 over the postcard encoding of the state's **schema 7 projection**: everything
/// but `care`. Two worlds with the same ecology and different care histories hash alike, so
/// a care run and a matched no-care run are directly comparable and a migrated schema 7
/// world with zero care hashes exactly as the pre-care build's `state_hash` did.
pub fn ecology_hash(state: &WorldState) -> u64 {
    fnv1a(&postcard::to_allocvec(&v7::project(state)).expect("the projection is encodable"))
}

fn fnv1a(payload: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in payload {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldConfig;
    use crate::fields::Fields;
    use crate::habitat::{Habitat, Weather};
    use crate::ids::Slots;

    fn state() -> WorldState {
        let config = WorldConfig::default();
        let habitat = Habitat::new(&config.habitat, config.seed);
        let fields = Fields::new(&config, &habitat.light_base, &habitat.moisture_base);
        let weather = Weather::new(&config.weather, config.seed);
        WorldState {
            config,
            tick: 1234,
            fields,
            weather,
            organisms: Slots::with_capacity(8),
            births_total: 0,
            deaths_total: [0; 3],
            cap_rejections_total: 0,
            external_material_in: 0.0,
            light_in_total: 0.0,
            heat_out_total: 0.0,
            rain_in_total: 0.0,
            evap_out_total: 0.0,
            care: crate::care::CareState::default(),
        }
    }

    fn split(bytes: &[u8]) -> (u32, String, u64, u32, &[u8]) {
        let schema = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let id_len = u16::from_le_bytes(bytes[8..10].try_into().unwrap()) as usize;
        let build_id = String::from_utf8(bytes[10..10 + id_len].to_vec()).unwrap();
        let at = 10 + id_len;
        let payload_len = u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
        let crc = u32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap());
        (schema, build_id, payload_len, crc, &bytes[at + 12..])
    }

    #[test]
    fn the_header_layout_is_exactly_as_documented() {
        let s = state();
        let bytes = encode_snapshot(&s, "abc123");
        assert_eq!(&bytes[..4], b"CUBW");
        let (schema, build_id, payload_len, crc, payload) = split(&bytes);
        assert_eq!(schema, SCHEMA_VERSION);
        assert_eq!(build_id, "abc123");
        assert_eq!(payload_len as usize, payload.len());
        assert_eq!(bytes.len(), HEADER_FIXED_BYTES + build_id.len() + payload.len());
        assert_eq!(crc, crc32fast::hash(payload));
        // The payload is the plain postcard encoding of the state.
        assert_eq!(payload, &postcard::to_allocvec(&s).unwrap()[..]);
        let round: WorldState = postcard::from_bytes(payload).unwrap();
        assert_eq!(round, s);
    }

    #[test]
    fn an_empty_build_id_still_produces_a_valid_header() {
        let bytes = encode_snapshot(&state(), "");
        let (_, build_id, payload_len, crc, payload) = split(&bytes);
        assert_eq!(build_id, "");
        assert_eq!(payload_len as usize, payload.len());
        assert_eq!(crc, crc32fast::hash(payload));
        assert_eq!(bytes.len(), HEADER_FIXED_BYTES + payload.len());
    }

    #[test]
    fn encoding_is_deterministic() {
        assert_eq!(encode_snapshot(&state(), "b"), encode_snapshot(&state(), "b"));
    }

    #[test]
    fn bad_magic_is_reported() {
        let mut bytes = encode_snapshot(&state(), "b");
        bytes[0] = b'X';
        assert_eq!(decode_snapshot(&bytes), Err(SnapshotError::BadMagic));
        assert_eq!(decode_snapshot(b"not a snapshot at all"), Err(SnapshotError::BadMagic));
    }

    #[test]
    fn truncation_is_reported_at_every_depth() {
        let bytes = encode_snapshot(&state(), "build");
        assert_eq!(decode_snapshot(&[]), Err(SnapshotError::Truncated));
        assert_eq!(decode_snapshot(b"CUB"), Err(SnapshotError::Truncated));
        for cut in [4, 6, 9, 10, 12, HEADER_FIXED_BYTES + 4, bytes.len() - 1] {
            assert_eq!(
                decode_snapshot(&bytes[..cut]),
                Err(SnapshotError::Truncated),
                "cut at {cut}"
            );
        }
        // Trailing bytes contradict the declared payload length.
        let mut long = bytes.clone();
        long.push(0);
        assert_eq!(decode_snapshot(&long), Err(SnapshotError::Truncated));
    }

    #[test]
    fn an_unsupported_schema_is_reported_with_its_version() {
        let mut bytes = encode_snapshot(&state(), "b");
        bytes[4..8].copy_from_slice(&(SCHEMA_VERSION + 7).to_le_bytes());
        assert_eq!(decode_snapshot(&bytes), Err(SnapshotError::UnsupportedSchema(SCHEMA_VERSION + 7)));
    }

    #[test]
    fn a_corrupt_payload_fails_the_checksum() {
        let bytes = encode_snapshot(&state(), "b");
        let payload_at = HEADER_FIXED_BYTES + 1;
        let mut flipped = bytes.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 0xff;
        assert_eq!(decode_snapshot(&flipped), Err(SnapshotError::BadChecksum));
        // A corrupt CRC field itself is equally a checksum failure.
        let mut bad_crc = bytes.clone();
        bad_crc[payload_at - 2] ^= 0xff;
        assert_eq!(decode_snapshot(&bad_crc), Err(SnapshotError::BadChecksum));
    }

    #[test]
    fn state_hash_is_stable_and_change_sensitive() {
        let a = state();
        assert_eq!(state_hash(&a), state_hash(&state()));
        let mut b = state();
        b.tick += 1;
        assert_ne!(state_hash(&a), state_hash(&b));
        let mut c = state();
        c.fields.n[17] += 1e-12;
        assert_ne!(state_hash(&a), state_hash(&c));
    }

    #[test]
    fn the_schema_seven_projection_is_the_payload_without_care() {
        let s = state();
        // Zero care appends exactly `CareState::default()`: a zero varint cursor, an empty
        // shower vector, and six zero f64 ledgers.
        let full = postcard::to_allocvec(&s).unwrap();
        let projected = postcard::to_allocvec(&v7::project(&s)).unwrap();
        assert_eq!(&full[..projected.len()], &projected[..], "the projection is a prefix of the payload");
        assert_eq!(full.len(), projected.len() + 1 + 1 + 6 * 8);
        assert_eq!(ecology_hash(&s), super::fnv1a(&projected));
        // Care moves `state_hash` and never `ecology_hash`.
        let mut fed = s.clone();
        fed.care.feed_material_in = 1.0;
        assert_ne!(state_hash(&fed), state_hash(&s));
        assert_eq!(ecology_hash(&fed), ecology_hash(&s));
        // A schema 7 payload round-trips through the mirror into an identical state.
        let back: WorldState = postcard::from_bytes::<WorldStateV7>(&projected).unwrap().into();
        assert_eq!(back, s);
    }

    #[test]
    fn round_trip_returns_the_state_and_its_header() {
        let s = state();
        let bytes = encode_snapshot(&s, "deadbeef");
        let (meta, back) = decode_snapshot(&bytes).expect("round trip");
        assert_eq!(meta.schema, SCHEMA_VERSION);
        assert_eq!(meta.build_id, "deadbeef");
        assert_eq!(meta.crc32, crc32fast::hash(&postcard::to_allocvec(&s).unwrap()));
        assert_eq!(back, s);
    }
}
