//! Snapshot encoding: header + postcard payload with CRC32.

use serde::{Deserialize, Serialize};

use crate::world::WorldState;

pub mod v10;
pub mod v11;
pub mod v12;
pub mod v13;
pub mod v14;
pub mod v7;
pub mod v8;
pub mod v9;

pub mod care_v1;

pub use v7::{SCHEMA_V7, WorldStateV7};
pub use v8::{SCHEMA_V8, WorldStateV8};
pub use v9::{SCHEMA_V9, WorldStateV9};
pub use v10::{SCHEMA_V10, WorldStateV10};
pub use v11::{SCHEMA_V11, WorldStateV11};
pub use v12::{SCHEMA_V12, WorldStateV12};
pub use v13::{SCHEMA_V13, WorldStateV13};
pub use v14::{SCHEMA_V14, WorldStateV14};

/// Bumped whenever `WorldState` or any nested type changes shape.
///
/// **Version 16 is ecology v1** (`design/ecology-v1-contract.md` §15.1) and it is a hard
/// break. Wrysk's standing rule of 2026-09-15 is that worlds always restart fresh and are
/// never migrated, so schema 16 **refuses every older snapshot by name**
/// ([`SnapshotError::UnsupportedSchema`]) rather than synthesising wood, a reserve or a
/// remains pool for a world that never had them. There is no migration rule, no synthesised
/// material and no subsidy booking anywhere in this module.
///
/// The frozen mirror structs for schemas 7–14 stay in the tree only for the refusal tests
/// that name their versions, and for [`v7::project`], which [`ecology_hash`] still uses for
/// the care/no-care comparison. Their `From<WorldStateVn> for WorldState` conversions are
/// gone: a conversion into the current shape is exactly the migration the rule forbids.
///
/// Historical shape notes, kept because the mirrors still encode them: version 8 appends
/// `care`; 9 appends `energy_correction`; 10 appends `hunters`; 11 reshapes the hunter
/// extension; 12 reshapes `care` for the persisted shower dose; 13 appends ordinary quiet;
/// 14 appends apex dormancy and encounters; 15 appends the recurrent extension; 16 appends
/// [`crate::fields::EcologyV1State`].
pub const SCHEMA_VERSION: u32 = 16;
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

/// Decode the current state or a frozen mirror, requiring the payload to be **fully consumed**.
///
/// `postcard::from_bytes` tolerates trailing bytes, and every schema since 8 has grown by
/// *appending*. Without this check a newer payload relabelled with an older schema number would
/// decode cleanly and silently drop whatever was appended — a care dose, a hunter extension, an
/// ordinary quiet timer. The length is the only evidence a non-self-describing format offers
/// that the reader and the writer agreed about the shape, so it is checked.
fn decode_exact<'a, T>(payload: &'a [u8], schema: u32) -> Result<T, SnapshotError>
where
    T: serde::Deserialize<'a>,
{
    let (value, rest) = postcard::take_from_bytes::<T>(payload)
        .map_err(|e| SnapshotError::Decode(e.to_string()))?;
    if !rest.is_empty() {
        return Err(SnapshotError::Decode(format!(
            "a schema {schema} payload has {} trailing byte(s); its shape does not match \
             the declared schema",
            rest.len()
        )));
    }
    Ok(value)
}

/// Validate magic, schema, length, CRC, exact decode, then `state.validate()`; every failure is a
/// distinct error so the loader can report why a snapshot was refused.
///
/// **Exactly one schema decodes: [`SCHEMA_VERSION`].** Every older version — 7 through 15 —
/// is [`SnapshotError::UnsupportedSchema`] carrying the version it read, so a caller can say
/// which world it was and that it has to be restarted rather than converted
/// (`design/ecology-v1-contract.md` §15.1). `SnapshotMeta.schema` still reports what was read.
/// A schema 16 payload with trailing bytes — a newer shape relabelled 16 — is refused too.
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
    if schema != SCHEMA_VERSION {
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
    let state: WorldState = decode_exact::<WorldState>(payload, schema)?;
    state.validate().map_err(SnapshotError::Invalid)?;
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

/// FNV-1a 64 over the postcard encoding of the state (the replay hash in telemetry).
pub fn state_hash(state: &WorldState) -> u64 {
    fnv1a(&postcard::to_allocvec(state).expect("WorldState is always postcard-encodable"))
}

/// FNV-1a 64 over the postcard encoding of the **current** state with its care extension
/// masked to [`crate::care::CareState::default()`] and nothing else altered
/// (`design/ecology-v1-contract.md` §15.1, revised after the implementation review).
///
/// Every other field is hashed: the config, the tick, every field vector, the organisms, the
/// hunter, quiet, apex and neural extensions, **and every ecology v1 stock and counter**. Only
/// care is masked, so a care run and a matched no-care run of the same ecology still compare
/// directly — which is the one thing this hash exists for.
///
/// It replaces the schema 7 projection this used to hash. That projection predates wood, the
/// plant reserve, dead wood and animal remains, so two schema 16 worlds could differ in every
/// pool ecology v1 added and still hash alike; a care replay could pass after the ecology had
/// diverged. [`state_hash`] remains the full encoding, care included.
pub fn ecology_hash(state: &WorldState) -> u64 {
    let masked = WorldState {
        care: crate::care::CareState::default(),
        ..state.clone()
    };
    fnv1a(&postcard::to_allocvec(&masked).expect("WorldState is always postcard-encodable"))
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
        let habitat_config = config.clone();
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
            energy_correction: crate::accounting::EnergyCorrection::default(),
            hunters: crate::hunter::HunterState::default(),
            quiet: crate::quiet::QuietState::default(),
            apex_dormancy: crate::dormancy::ApexDormancyState::default(),
            apex_encounters: crate::encounter::ApexEncounterState::default(),
            neural: crate::neural::NeuralState::default(),
            ecology: crate::fields::EcologyV1State::new(
                &habitat_config,
                &habitat.light_base,
                &habitat.moisture_base,
            ),
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
        assert_eq!(
            bytes.len(),
            HEADER_FIXED_BYTES + build_id.len() + payload.len()
        );
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
        assert_eq!(
            encode_snapshot(&state(), "b"),
            encode_snapshot(&state(), "b")
        );
    }

    #[test]
    fn bad_magic_is_reported() {
        let mut bytes = encode_snapshot(&state(), "b");
        bytes[0] = b'X';
        assert_eq!(decode_snapshot(&bytes), Err(SnapshotError::BadMagic));
        assert_eq!(
            decode_snapshot(b"not a snapshot at all"),
            Err(SnapshotError::BadMagic)
        );
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
        assert_eq!(
            decode_snapshot(&bytes),
            Err(SnapshotError::UnsupportedSchema(SCHEMA_VERSION + 7))
        );
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
    fn the_schema_nine_projection_is_the_payload_without_the_hunters() {
        let s = state();
        // An empty hunter extension appends exactly `HunterState::default()`: no profile, an
        // empty member vector, four zero f64 imports, the control flag, the founder count and
        // five zero counters. The dose is *not* in this difference: schema 12 put it inside
        // `care`, and this state's care is empty, so the frozen mirror's care encodes alike.
        let full = postcard::to_allocvec(&s).unwrap();
        let projected = postcard::to_allocvec(&v9::project(&s).unwrap()).unwrap();
        assert_eq!(
            &full[..projected.len()],
            &projected[..],
            "the projection is a prefix of the payload"
        );
        assert_eq!(
            full.len(),
            projected.len()
                + EMPTY_HUNTERS
                + EMPTY_QUIET
                + EMPTY_APEX_DORMANCY
                + EMPTY_APEX_ENCOUNTERS
                + EMPTY_NEURAL
                + ecology_bytes(&s)
        );

        // Schema 16 has no way back from the mirror: the `From` conversion is gone
        // (`design/ecology-v1-contract.md` §15.1) and the frozen shape still decodes only
        // into itself. What the projection drops is still exactly the extension.
        postcard::from_bytes::<WorldStateV9>(&projected).expect("the frozen shape still decodes");
        let mut hunted = s.clone();
        hunted.hunters.founder_material_in = 4.0;
        hunted.hunters.captures_total = 3;
        assert_eq!(
            postcard::to_allocvec(&v9::project(&hunted).unwrap()).unwrap(),
            projected,
            "a hunter extension must not move the schema 9 projection"
        );
        assert_ne!(
            state_hash(&hunted),
            state_hash(&s),
            "it is in the full-state hash"
        );
        // **Revised in repair cycle 2** (`design/ecology-v1-contract.md` §15.1): `ecology_hash`
        // is no longer the schema 7 projection, so it now sees the hunter extension too. Only
        // care is masked.
        assert_ne!(
            ecology_hash(&hunted),
            ecology_hash(&s),
            "the care-masked hash covers every extension but care"
        );
    }

    /// Bytes an empty [`crate::hunter::HunterState`] appends to the payload: `Option::None`,
    /// an empty member vector, four `f64` imports, the control flag, the founder count and
    /// the five extension counters (all zero varints).
    const EMPTY_HUNTERS: usize = 1 + 1 + 4 * 8 + 1 + 1 + 5;

    /// Bytes an inert [`crate::quiet::QuietState`] appends: the version varint, the `Off`
    /// variant index and an empty pause vector's length. Three bytes, and every one of them is
    /// a constant in an Off world.
    const EMPTY_QUIET: usize = 1 + 1 + 1;

    /// Version, Off policy, empty entry vector, three zero varint counters and one zero f64.
    const EMPTY_APEX_DORMANCY: usize = 1 + 1 + 1 + 3 + 8;

    /// Version, Off policy, two empty vectors, seven zero counters and one zero f64.
    const EMPTY_APEX_ENCOUNTERS: usize = 1 + 1 + 2 + 7 + 8;

    /// Bytes an empty [`crate::neural::NeuralState`] appends: the version byte, an empty
    /// policy vector's length and an empty animal vector's length.
    const EMPTY_NEURAL: usize = 1 + 1 + 1;

    /// Bytes [`crate::fields::EcologyV1State`] appends: five 1,280-cell `f64` vectors and two
    /// varint counters. Unlike every earlier extension this one is never empty — a world
    /// either carries these pools or does not load — so it is measured, not counted by hand.
    fn ecology_bytes(s: &WorldState) -> usize {
        postcard::to_allocvec(&s.ecology).unwrap().len()
    }

    #[test]
    fn the_schema_eight_projection_is_the_payload_without_the_corrections() {
        let s = state();
        // Zero corrections append exactly two zero f64: schema 9 is schema 8 plus 16 bytes.
        let full = postcard::to_allocvec(&s).unwrap();
        let projected = postcard::to_allocvec(&v8::project(&s).unwrap()).unwrap();
        assert_eq!(
            &full[..projected.len()],
            &projected[..],
            "the projection is a prefix of the payload"
        );
        assert_eq!(
            full.len(),
            projected.len()
                + 2 * 8
                + EMPTY_HUNTERS
                + EMPTY_QUIET
                + EMPTY_APEX_DORMANCY
                + EMPTY_APEX_ENCOUNTERS
                + EMPTY_NEURAL
                + ecology_bytes(&s)
        );

        postcard::from_bytes::<WorldStateV8>(&projected).expect("the frozen shape still decodes");
        let mut compensated = s.clone();
        compensated.energy_correction.heat_out = -1.5e-9;
        assert_eq!(
            postcard::to_allocvec(&v8::project(&compensated).unwrap()).unwrap(),
            projected,
            "a correction must not move the schema 8 projection"
        );
        assert_ne!(
            state_hash(&compensated),
            state_hash(&s),
            "it is in the full-state hash"
        );
        assert_ne!(
            ecology_hash(&compensated),
            ecology_hash(&s),
            "the care-masked hash covers the signed corrections too"
        );
    }

    #[test]
    fn the_schema_seven_projection_is_the_payload_without_care() {
        let s = state();
        // Zero care appends exactly `CareState::default()`: a zero varint cursor, an empty
        // shower vector, and six zero f64 ledgers; then the two correction f64.
        let full = postcard::to_allocvec(&s).unwrap();
        let projected = postcard::to_allocvec(&v7::project(&s)).unwrap();
        assert_eq!(
            &full[..projected.len()],
            &projected[..],
            "the projection is a prefix of the payload"
        );
        assert_eq!(
            full.len(),
            projected.len()
                + 1
                + 1
                + 6 * 8
                + 2 * 8
                + EMPTY_HUNTERS
                + EMPTY_QUIET
                + EMPTY_APEX_DORMANCY
                + EMPTY_APEX_ENCOUNTERS
                + EMPTY_NEURAL
                + ecology_bytes(&s)
        );
        // `ecology_hash` **is not** this projection any more: it is the care-masked hash of
        // the current state (§15.1, revised in repair cycle 2), so it covers every pool the
        // schema 7 shape predates. What survives is the one property the hash exists for.
        assert_ne!(
            ecology_hash(&s),
            super::fnv1a(&projected),
            "the ecology hash is the current state, not the schema 7 projection"
        );
        assert_eq!(
            ecology_hash(&s),
            state_hash(&s),
            "with no care in it, masking care changes nothing"
        );
        // Care moves `state_hash` and never `ecology_hash`.
        let mut fed = s.clone();
        fed.care.feed_material_in = 1.0;
        assert_ne!(state_hash(&fed), state_hash(&s));
        assert_eq!(ecology_hash(&fed), ecology_hash(&s));
        postcard::from_bytes::<WorldStateV7>(&projected).expect("the frozen shape still decodes");
    }

    /// `design/ecology-v1-contract.md` §15.1 and §13.1 A7: **every** older schema is refused
    /// by name. Nothing is migrated, nothing is synthesised, and the error carries the
    /// version that was read so a caller can say which world has to be restarted.
    #[test]
    fn every_older_schema_is_refused_by_name() {
        let bytes = encode_snapshot(&state(), "b");
        for old in [
            SCHEMA_V7, SCHEMA_V8, SCHEMA_V9, SCHEMA_V10, SCHEMA_V11, SCHEMA_V12, SCHEMA_V13,
            SCHEMA_V14, 15,
        ] {
            let mut relabelled = bytes.clone();
            relabelled[4..8].copy_from_slice(&old.to_le_bytes());
            assert_eq!(
                decode_snapshot(&relabelled),
                Err(SnapshotError::UnsupportedSchema(old)),
                "schema {old} must be refused by name"
            );
        }
        assert_eq!(SCHEMA_VERSION, 16);
    }

    /// A newer payload relabelled 16 is refused too: the length is the only evidence a
    /// non-self-describing format offers that reader and writer agreed about the shape.
    #[test]
    fn a_relabelled_schema_sixteen_payload_is_refused() {
        let s = state();
        let mut payload = postcard::to_allocvec(&s).unwrap();
        payload.extend_from_slice(&[0u8; 4]);
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        out.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
        out.extend_from_slice(&payload);
        match decode_snapshot(&out) {
            Err(SnapshotError::Decode(msg)) => {
                assert!(msg.contains("trailing byte"), "{msg}");
            }
            other => panic!("expected a trailing-bytes refusal, got {other:?}"),
        }
    }

    #[test]
    fn round_trip_returns_the_state_and_its_header() {
        let s = state();
        let bytes = encode_snapshot(&s, "deadbeef");
        let (meta, back) = decode_snapshot(&bytes).expect("round trip");
        assert_eq!(meta.schema, SCHEMA_VERSION);
        assert_eq!(meta.build_id, "deadbeef");
        assert_eq!(
            meta.crc32,
            crc32fast::hash(&postcard::to_allocvec(&s).unwrap())
        );
        assert_eq!(back, s);
    }
}
