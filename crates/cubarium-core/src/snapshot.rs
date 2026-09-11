//! Snapshot encoding: header + postcard payload with CRC32.

use serde::{Deserialize, Serialize};

use crate::world::WorldState;

/// Bumped whenever `WorldState` or any nested type changes shape.
pub const SCHEMA_VERSION: u32 = 1;
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
    let _ = (state, build_id);
    todo!("encode_snapshot")
}

/// Validate magic, schema, length, CRC, decode, then `state.validate()`; every failure is a
/// distinct error so the loader can report why an older snapshot was tried.
pub fn decode_snapshot(bytes: &[u8]) -> Result<(SnapshotMeta, WorldState), SnapshotError> {
    let _ = bytes;
    todo!("decode_snapshot")
}

/// FNV-1a 64 over the postcard encoding of the state (the replay hash in telemetry).
pub fn state_hash(state: &WorldState) -> u64 {
    let _ = state;
    todo!("state_hash")
}
