//! **Retired.** The `*-plus600-*.cubw` continuation comparisons, and the regenerator that
//! produced them.
//!
//! What this file used to do: each `*-plus600-r0b` / `-r0d` fixture was this build's own
//! recording of its start fixture stepped 600 ticks, and the ordinary suite re-derived every
//! one of them so that a tick that moved unintentionally failed here even if an individual
//! migration test was skipped.
//!
//! Why it is gone. Wrysk decided on 2026-09-15 that **worlds always restart fresh and are
//! never migrated** (`design/ecology-v1-contract.md` §15.1). Schema 16 therefore refuses every
//! older snapshot by name instead of synthesising wood, a reserve and a remains pool for a
//! world that never had them. Every start fixture in the set is schema 7 through 12, so none
//! of them can be loaded, and this build cannot write a schema 7 (or 8, 9, 11, 12) world to
//! re-anchor the comparison against. The continuation is therefore **retired, not
//! re-recorded** — re-recording it would need exactly the migration the rule forbids.
//!
//! Nothing is deleted. Every `.cubw` file stays where it is; they are genuine artefacts of the
//! builds named in `tests/fixtures/*-provenance.md`, each provenance note records this
//! retirement, and the tests that used to migrate them now assert the refusal on the same
//! bytes (`tests/care.rs`, `tests/care_dose_migration.rs`, `tests/hunter_migration.rs`,
//! `tests/quiet_migration.rs`, `tests/astra_quiet_policy.rs`).
//!
//! New continuation fixtures, if wanted, are generated from a schema 16 world in a later
//! assignment. Until then the guard against an unintentional change in the tick is the
//! determinism and replay coverage in `tests/determinism.rs` and `world::tests`, which build
//! their own worlds rather than loading someone else's.

use std::path::PathBuf;

use cubarium_core::{SCHEMA_VERSION, SnapshotError, decode_snapshot};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// Every fixture the retired comparisons touched, start and continuation alike.
const RETIRED: &[&str] = &[
    "live-v7-55200.cubw",
    "live-v7-55200-plus600.cubw",
    "live-v7-55200-plus600-r0b.cubw",
    "live-v8-172800.cubw",
    "live-v8-172800-plus600.cubw",
    "live-v8-172800-plus600-r0b.cubw",
    "pre-hunter-v9-173400.cubw",
    "pre-hunter-v9-173400-plus600.cubw",
    "pre-hunter-v9-173400-plus600-r0b.cubw",
    "care-v11-shower-360.cubw",
    "care-v11-shower-360-plus600.cubw",
    "care-v11-shower-360-plus600-r0b.cubw",
    "care-v11-hunters-200.cubw",
    "care-v11-hunters-200-plus600.cubw",
    "care-v11-hunters-200-plus600-r0b.cubw",
    "hunter-v3-charge-window.cubw",
    "hunter-v3-charge-active.cubw",
    "hunter-v3-charge-active-plus600.cubw",
    "hunter-v3-charge-active-plus600-r0d.cubw",
    "quiet-v12-plain-3000.cubw",
    "quiet-v12-plain-3000-plus600.cubw",
    "quiet-v12-plain-3000-plus600-r0b.cubw",
    "quiet-v12-care-3000.cubw",
    "quiet-v12-care-3000-plus600.cubw",
    "quiet-v12-care-3000-plus600-r0b.cubw",
];

/// The whole set is still on disk, still older than schema 16, and refused by name — one
/// statement over every fixture, so a file that is quietly removed or replaced with a schema
/// 16 world fails here.
#[test]
fn every_retired_fixture_is_present_and_refused_by_name() {
    for name in RETIRED {
        let bytes = std::fs::read(fixture(name))
            .unwrap_or_else(|e| panic!("{name} must stay in the tree: {e}"));
        let schema = u32::from_le_bytes(bytes[4..8].try_into().expect("4 bytes"));
        assert!(
            schema < SCHEMA_VERSION,
            "{name} reports schema {schema}; the retired set is pre-{SCHEMA_VERSION} by definition"
        );
        assert_eq!(
            decode_snapshot(&bytes),
            Err(SnapshotError::UnsupportedSchema(schema)),
            "{name}: an old world is refused by name, never migrated"
        );
    }
}
