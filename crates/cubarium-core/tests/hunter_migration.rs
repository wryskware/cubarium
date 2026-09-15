//! Schema 10 migration and the empty-extension continuations.
//!
//! The load-bearing test here is
//! [`an_empty_extension_reproduces_the_pre_hunter_binarys_next_600_ticks`]: two fixtures
//! produced by the *pre-hunter* schema 9 release build (root's `1f0fc3a`, provenance in
//! `tests/fixtures/pre-hunter-v9-provenance.md`) prove that a world carrying the hunter
//! extension but no hunters steps exactly as the pre-hunter binary did — care, the signed
//! energy corrections and the raw counters included. The genuine schema 8 and schema 7
//! continuations in `care.rs` and `energy_correction.rs` must keep passing beside it.

use std::path::PathBuf;

use cubarium_core::hunter::HunterState;
use cubarium_core::snapshot::{
    HEADER_FIXED_BYTES, MAGIC, SnapshotError, state_hash, v7, v8, v9, v10,
};
use cubarium_core::{
    SCHEMA_V9, SCHEMA_V10, SCHEMA_VERSION, World, WorldConfig, decode_snapshot, ecology_hash,
    encode_snapshot,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// The postcard payload of a snapshot file: everything after the variable-length header.
fn payload(bytes: &[u8]) -> &[u8] {
    let id_len = usize::from(u16::from_le_bytes(bytes[8..10].try_into().expect("2 bytes")));
    &bytes[HEADER_FIXED_BYTES + id_len..]
}

/// FNV-1a 64 over the fixture's own bytes, so the test does not take the crate's word for it.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// **Retired by ecology v1** (`design/ecology-v1-contract.md` §15.1). Two tests lived here:
/// a genuine live schema 9 world loading with an empty, inert extension and projecting back
/// byte for byte, and its 600-tick continuation against this build's recording.
///
/// Worlds always restart fresh and are never migrated (Wrysk, 2026-09-15), so schema 16
/// refuses schema 9 by name; the artifacts cannot be loaded and this build cannot write a
/// schema 9 world to re-anchor against. The fixtures stay in the tree with their provenance
/// note, and their own recorded hashes are still asserted here — so a file quietly replaced
/// still fails — before the refusal is checked on the same bytes.
#[test]
fn the_pre_hunter_schema_nine_fixtures_are_refused_by_name() {
    for (name, hash) in [
        ("pre-hunter-v9-173400.cubw", Some("134f4db0135d8a0a")),
        ("pre-hunter-v9-173400-plus600.cubw", Some("854dce1766d905d3")),
        ("pre-hunter-v9-173400-plus600-r0b.cubw", None),
    ] {
        let bytes = std::fs::read(fixture(name)).expect("the fixture is committed");
        if let Some(h) = hash {
            assert_eq!(format!("{:x}", fnv1a(payload(&bytes))), h, "{name}: the provenance hash");
        }
        let schema = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert!(schema < SCHEMA_VERSION, "{name} is schema {schema}");
        assert_eq!(
            decode_snapshot(&bytes),
            Err(SnapshotError::UnsupportedSchema(schema)),
            "{name}: an old world is refused by name, never migrated"
        );
    }
    assert_eq!(SCHEMA_V9, 9);
}

/// The schema 8 and schema 7 mirrors still project through the two appended fields.
#[test]
fn the_older_projections_still_drop_only_what_they_are_named_for() {
    // Stated on a world this build makes: the old files it used to read are schema 7 and 8
    // and no longer load (§15.1). The claim is about the projections, not about any recorded
    // world — each one is a strict prefix of the next, which is what "appended, never
    // reinterpreted" means, and each is shorter than the full schema 16 payload.
    let mut world = World::new(WorldConfig::default()).expect("defaults are valid");
    for _ in 0..40 {
        world.step();
    }
    let full = postcard::to_allocvec(&world.state).expect("encodable");
    let as_v9 =
        postcard::to_allocvec(&v9::project(&world.state).expect("standard care projects")).unwrap();
    let as_v8 =
        postcard::to_allocvec(&v8::project(&world.state).expect("standard care projects")).unwrap();
    let as_v7 = postcard::to_allocvec(&v7::project(&world.state)).unwrap();
    assert!(as_v7.len() < as_v8.len() && as_v8.len() < as_v9.len() && as_v9.len() < full.len());
    assert_eq!(&full[..as_v9.len()], &as_v9[..], "schema 9 is a prefix of schema 16");
    assert_eq!(&as_v9[..as_v8.len()], &as_v8[..], "schema 8 is a prefix of schema 9");
    assert_eq!(&as_v8[..as_v7.len()], &as_v7[..], "schema 7 is a prefix of schema 8");
    // `ecology_hash` is the care-masked hash of the current state (§15.1, revised in repair
    // cycle 2), not any of these projections.
    assert_ne!(ecology_hash(&world.state), fnv1a(&as_v7));
    assert_eq!(
        ecology_hash(&world.state),
        state_hash(&world.state),
        "with no care in the state the mask removes nothing"
    );
    assert_ne!(state_hash(&world.state), fnv1a(&as_v9));
}

/// A world that never opts in is inert in every observable way, and its own snapshots
/// round-trip through schema 10 unchanged.
#[test]
fn a_world_that_never_opts_in_carries_an_empty_extension() {
    let mut world = World::new(WorldConfig::default()).expect("defaults are valid");
    for _ in 0..200 {
        world.step();
    }
    assert_eq!(world.state.hunters, HunterState::default());
    assert!(world.hunter_view().is_empty());
    assert!(world.drain_hunter_events().is_empty());
    let sample = world.telemetry();
    assert_eq!(sample.hunters, 0);
    assert_eq!(sample.hunter_captures, 0);
    assert_eq!(sample.hunter_attacks, 0);
    assert_eq!(sample.deaths_predation, 0);

    let bytes = encode_snapshot(&world.state, "hunter-inert");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), SCHEMA_VERSION);
    let (meta, back) = decode_snapshot(&bytes).expect("round trip");
    assert_eq!(meta.schema, SCHEMA_VERSION);
    assert_eq!(back, world.state);
}

// ---------------------------------------------------------------- schema 10

/// Frame a payload as a snapshot of `schema`, the way `encode_snapshot` frames the current one.
fn frame(schema: u32, payload: &[u8], build: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&schema.to_le_bytes());
    out.extend_from_slice(&(build.len() as u16).to_le_bytes());
    out.extend_from_slice(build.as_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// **Retired by ecology v1** (§15.1). Schema 10 used to be the one schema with a *conditional*
/// migration: an empty extension opened empty, and any hunter history — a running trial, a
/// budget-matched control, an extinct lineage's counters — was refused with a reason, because
/// the profile and member shape changed in schema 11.
///
/// Schema 16 refuses schema 10 outright, like every other older schema, so the condition is
/// gone with the migration. What survives is the stronger statement: a well-framed schema 10
/// payload is turned away **whatever** it carries, and it is named rather than reinterpreted.
#[test]
fn every_schema_ten_payload_is_refused_by_name_whatever_it_carries() {
    let mut world = World::new(WorldConfig::default()).expect("defaults are valid");
    for _ in 0..60 {
        world.step();
    }
    let empty = v10::project(&world.state).expect("an empty extension has a schema 10 image");

    /// One way to leave hunter history in an otherwise empty schema 10 extension.
    type History = fn(&mut v10::HunterStateV10);
    let cases: [(&str, Option<History>); 3] = [
        ("an empty extension", None),
        (
            "a control deposit",
            Some(|h| {
                h.control_deposited = true;
                h.control_material_in = 4.0;
                h.control_energy_in = 7.0;
            }),
        ),
        (
            "an extinct lineage",
            Some(|h| {
                h.hunter_deaths_total = 1;
                h.captures_total = 3;
            }),
        ),
    ];
    for (what, poison) in cases {
        let mut old = empty.clone();
        if let Some(p) = poison {
            p(&mut old.hunters);
        }
        let bytes = frame(
            SCHEMA_V10,
            &postcard::to_allocvec(&old).expect("encodable"),
            "schema-ten",
        );
        assert_eq!(
            decode_snapshot(&bytes),
            Err(SnapshotError::UnsupportedSchema(SCHEMA_V10)),
            "{what}: a schema 10 payload is refused by name"
        );
    }

    // And a current world with a trial has no honest schema 10 image either — the projection
    // side of the same rule, which is unchanged.
    let profile = cubarium_core::FixedHunterProfile::lanternjaw_trial(world.config());
    let target = cubarium_core::HunterTarget { face: 4, u: 22.0, v: 34.0 };
    world.start_hunter_trial(profile, target).expect("the trial starts");
    assert!(v10::project(&world.state).is_none(), "a running trial must not project backwards");
}
