//! Frozen schema 17 `WorldState`: `main`'s semantics-only bump, immediately before the ring
//! world.
//!
//! Schema 17 is the one version in this tree that **moved no bytes**. It was bumped because
//! the shipped pursuit stopping rule changed and a world's payload cannot say which rule it
//! was run under (`design/7_Research/ecology-v1-predicate-adoption-2026-09-16.md`), so its
//! shape is schema 16's exactly. This module is therefore [`WorldStateV16`] under its own
//! number and its own decoder, not a second copy of eighteen struct definitions — duplicating
//! them would invite the two to drift apart and say nothing true about the wire.
//!
//! Like [`super::v16`], it exists for one reader: the `CubeProjection` comparator of
//! `design/flat-world-plan-2026-09-16.md` §4, which reads a pre-ring **payload** to show that
//! a cube world is unchanged across the break. [`super::decode_snapshot`] refuses schema 17
//! by name and keeps refusing it. The product refuses old worlds; only the comparator looks
//! inside one.

use super::v16::{WorldStateV16, decode_frozen};
use super::{SnapshotError, SnapshotMeta};

pub const SCHEMA_V17: u32 = 17;

/// The `CONFIG_VERSION` a schema 17 world was written with — still 8, like 16: the ring
/// world's `topology` and `world_scale` are what take it to 9.
pub const CONFIG_VERSION_V17: u32 = 8;

/// Schema 17's `WorldState` **is** schema 16's. See the module docs.
pub type WorldStateV17 = WorldStateV16;

/// Read a **schema 17 snapshot file** — header and all — and decode its payload through the
/// frozen mirror.
///
/// Deliberately not [`super::decode_snapshot`], which refuses schema 17 and keeps refusing it.
/// This one insists on schema 17, checks the same magic, length and CRC, and decodes with the
/// same exact-length rule. It never calls `validate`: a schema 17 world is not a world this
/// build can run, only one it can read the numbers out of.
pub fn decode_v17(bytes: &[u8]) -> Result<(SnapshotMeta, WorldStateV17), SnapshotError> {
    decode_frozen(bytes, SCHEMA_V17)
}
