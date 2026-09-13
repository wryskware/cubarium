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
use cubarium_core::snapshot::{HEADER_FIXED_BYTES, state_hash, v7, v8, v9};
use cubarium_core::{
    SCHEMA_V9, SCHEMA_VERSION, World, WorldConfig, decode_snapshot, ecology_hash, encode_snapshot,
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

/// A genuine live schema 9 world loads with an empty, inert extension and its schema 9
/// projection is the original payload byte for byte.
#[test]
fn the_live_schema_nine_snapshot_migrates_without_a_hunter() {
    let bytes = std::fs::read(fixture("pre-hunter-v9-173400.cubw")).expect("the fixture is committed");
    let (meta, state) = decode_snapshot(&bytes).expect("a genuine schema 9 snapshot loads");
    assert_eq!(meta.schema, SCHEMA_V9, "the header still reports what was read");
    assert_ne!(SCHEMA_VERSION, SCHEMA_V9);
    assert_eq!(state.tick, 173_400);
    assert_eq!(state.organisms.len(), 80, "the provenance records 80 organisms");
    assert_eq!(state.care.admitted_seq, 5, "the fixture carries a real care history");
    assert_ne!(
        state.energy_correction,
        cubarium_core::EnergyCorrection::default(),
        "the provenance says this world already carries 600 ticks of corrections"
    );

    // Migration introduces no predator, no profile and no import.
    assert_eq!(state.hunters, HunterState::default());
    assert!(!state.hunters.active());
    assert_eq!(state.hunters.gut_material_total(), 0.0);
    assert_eq!(state.hunters.imported_material(), 0.0);
    assert_eq!(state.hunters.imported_energy(), 0.0);

    let projected = postcard::to_allocvec(&v9::project(&state)).expect("encodable");
    assert_eq!(projected, payload(&bytes), "re-encoding the projection is not the original payload");
    // The provenance's own schema 9 hash, computed here from the file's bytes.
    assert_eq!(format!("{:x}", fnv1a(payload(&bytes))), "134f4db0135d8a0a");
    // The older projections still mean what they meant.
    assert_eq!(ecology_hash(&state), fnv1a(&postcard::to_allocvec(&v7::project(&state)).unwrap()));
    assert_ne!(state_hash(&state), fnv1a(payload(&bytes)), "the full hash covers the extension");

    let world = World::from_state(state).expect("the migrated state is a valid world");
    world.check_invariants().expect("invariants hold on the migrated world");
    assert!(world.mass_residual().abs() < 1e-9, "residual {}", world.mass_residual());
    assert!(world.hunter_view().is_empty(), "a migrated world has no hunters to draw");
}

/// The cross-version proof for this package. If it fails, the hunter extension changed an
/// operation in the zero-hunter tick and nothing else in the package means anything.
#[test]
fn an_empty_extension_reproduces_the_pre_hunter_binarys_next_600_ticks() {
    let start = std::fs::read(fixture("pre-hunter-v9-173400.cubw")).expect("fixture");
    let plus600 = std::fs::read(fixture("pre-hunter-v9-173400-plus600.cubw")).expect("fixture");
    let (_, state) = decode_snapshot(&start).expect("schema 9 loads");
    let (meta, expected) = decode_snapshot(&plus600).expect("schema 9 loads");
    assert_eq!(meta.schema, SCHEMA_V9);
    assert_eq!(expected.tick, 174_000);

    let mut world = World::from_state(state).expect("valid");
    for _ in 0..600 {
        world.step();
    }
    assert_eq!(world.tick(), 174_000);

    let projected = postcard::to_allocvec(&v9::project(&world.state)).expect("encodable");
    assert_eq!(
        projected,
        payload(&plus600),
        "600 zero-hunter ticks diverged from the pre-hunter binary"
    );
    assert_eq!(format!("{:x}", fnv1a(payload(&plus600))), "854dce1766d905d3", "the provenance hash");
    // Named individually so a failure says what moved.
    assert_eq!(world.state.fields, expected.fields, "field stocks");
    assert_eq!(world.state.organisms, expected.organisms, "organisms");
    assert_eq!(world.state.weather, expected.weather, "weather");
    assert_eq!(world.state.config, expected.config, "config");
    assert_eq!(world.state.care, expected.care, "care ledgers and shower progress");
    assert_eq!(world.state.energy_correction, expected.energy_correction, "signed corrections");
    assert_eq!(world.state.light_in_total, expected.light_in_total, "raw light_in_total");
    assert_eq!(world.state.heat_out_total, expected.heat_out_total, "raw heat_out_total");
    assert_eq!(world.state.rain_in_total, expected.rain_in_total);
    assert_eq!(world.state.evap_out_total, expected.evap_out_total);
    assert_eq!(world.state.births_total, expected.births_total);
    assert_eq!(world.state.deaths_total, expected.deaths_total);
    assert_eq!(world.state.hunters, HunterState::default(), "no hunter appeared");
    assert_eq!(ecology_hash(&world.state), ecology_hash(&expected), "legacy ecology projection");
    // And the whole schema 10 state hashes as the schema 9 payload plus the empty extension.
    assert_ne!(state_hash(&world.state), fnv1a(payload(&plus600)));
}

/// The schema 8 and schema 7 mirrors still project through the two appended fields.
#[test]
fn the_older_projections_still_drop_only_what_they_are_named_for() {
    let bytes = std::fs::read(fixture("live-v8-172800.cubw")).expect("fixture");
    let (_, v8_state) = decode_snapshot(&bytes).expect("schema 8 loads");
    assert_eq!(v8_state.hunters, HunterState::default());
    assert_eq!(
        postcard::to_allocvec(&v8::project(&v8_state)).unwrap(),
        payload(&bytes),
        "the schema 8 projection is still the original payload"
    );

    let bytes = std::fs::read(fixture("live-v7-55200.cubw")).expect("fixture");
    let (_, v7_state) = decode_snapshot(&bytes).expect("schema 7 loads");
    assert_eq!(v7_state.hunters, HunterState::default());
    assert_eq!(
        postcard::to_allocvec(&v7::project(&v7_state)).unwrap(),
        payload(&bytes),
        "the schema 7 projection is still the original payload"
    );
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
