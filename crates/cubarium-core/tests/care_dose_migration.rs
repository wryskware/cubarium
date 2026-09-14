//! Schema 12 migration: the adjustable care dose against genuine pre-dose artifacts.
//!
//! The load-bearing tests here are the two continuations. Four fixtures written by the
//! *pre-dose* release build (commit `e55501d`, provenance in
//! `tests/fixtures/care-v11-provenance.md`) prove that schema 12 reads a real schema 11 world —
//! including one with an **unfinished shower** and one with an **actual hunter extension** —
//! steps it 600 ticks, and writes back the same payload the old binary did, byte for byte.
//! A standard dose is not "close enough" to the old arithmetic; it is the old arithmetic.
//!
//! The other direction is the honest one: a world whose shower is falling at a nonstandard
//! dose has **no** schema 11 image, and every legacy projection says so rather than quietly
//! writing a shower that lost the amount somebody asked for.

use std::path::PathBuf;

use cubarium_core::care::{CareCommand, CareDose, CareKind, CareTarget, RAIN_DEPTH_TOTAL};
use cubarium_core::snapshot::{HEADER_FIXED_BYTES, state_hash, v7, v8, v9, v10, v11};
use cubarium_core::{
    SCHEMA_V11, SCHEMA_VERSION, World, WorldConfig, decode_snapshot, ecology_hash, encode_snapshot,
};
use cubarium_surface::{Face, SurfacePoint};

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

/// The schema 11 image of a state, re-encoded — what the pre-dose binary would have written.
fn as_v11(state: &cubarium_core::WorldState) -> Vec<u8> {
    postcard::to_allocvec(&v11::project(state).expect("a standard world has a schema 11 image"))
        .expect("the projection is encodable")
}

fn target(face: Face, u: f64, v: f64) -> CareTarget {
    let p = SurfacePoint::new(face, u, v);
    CareTarget { face: p.face.index() as u8, u: p.u, v: p.v }
}

// ---------------------------------------------------------------- the genuine fixtures

/// A genuine schema 11 world caught **mid-shower** loads, keeps every care value it had, and
/// opens its in-flight rain at the standard dose — which is what it was, not a guess. Its own
/// schema 11 projection is the original payload byte for byte.
#[test]
fn the_genuine_pre_dose_shower_migrates_at_the_standard_dose() {
    let bytes = std::fs::read(fixture("care-v11-shower-360.cubw")).expect("the fixture is committed");
    assert_eq!(fnv1a(payload(&bytes)), 0x239b_a63f_523d_93ab, "the provenance's own hash");

    let (meta, state) = decode_snapshot(&bytes).expect("a genuine schema 11 snapshot loads");
    assert_eq!(meta.schema, SCHEMA_V11, "the header reports what was read, not what we write");
    assert_eq!(meta.build_id, "pre-dose-fixture");
    assert_ne!(SCHEMA_VERSION, SCHEMA_V11);
    assert_eq!(state.tick, 360);

    // The care history the old build actually accumulated, not a zeroed extension.
    assert_eq!(state.care.admitted_seq, 3);
    assert!(state.care.feed_material_in > 0.0 && state.care.clean_material_out > 0.0);
    assert!(state.care.rain_depth_in > 0.0, "60 of the shower's samples have already landed");

    // The shower itself: unfinished, and standard.
    let shower = &state.care.showers[0];
    assert_eq!((shower.seq, shower.apply_after_tick, shower.delivered), (3, 300, 60));
    assert_eq!(shower.dose_permille, CareDose::STANDARD_PERMILLE);
    assert!(shower.dose().is_standard());
    assert_eq!(shower.dose().scale(RAIN_DEPTH_TOTAL), RAIN_DEPTH_TOTAL, "the identity branch");

    // And the migration is lossless in both directions.
    assert_eq!(as_v11(&state), payload(&bytes), "re-encoding the projection is not the original");
    assert_ne!(state_hash(&state), fnv1a(payload(&bytes)), "the dose is inside the new full hash");
}

/// The claim this whole package rests on: a schema 12 build resuming a pre-dose world at the
/// standard dose produces the pre-dose binary's next 600 ticks **byte for byte** — including
/// the 60 remaining samples of a shower admitted before the dose existed.
#[test]
fn a_standard_dose_reproduces_the_pre_dose_binarys_next_600_ticks() {
    let bytes = std::fs::read(fixture("care-v11-shower-360.cubw")).expect("fixture");
    let plus600 = std::fs::read(fixture("care-v11-shower-360-plus600.cubw")).expect("fixture");
    assert_eq!(fnv1a(payload(&plus600)), 0xdc91_056c_8370_e7fa, "the provenance's own hash");

    let (_, state) = decode_snapshot(&bytes).expect("schema 11 loads");
    let mut world = World::from_state(state).expect("the migrated state is a valid world");
    world.check_invariants().expect("invariants hold on the migrated world");
    for _ in 0..600 {
        world.step();
    }
    assert_eq!(world.tick(), 960);
    assert!(world.care().showers.is_empty(), "the migrated shower ran out its 120 samples");

    // R0a (`design/handoffs/r0a-movement-foundation-2026-09-14.md`) made body rotation a
    // physical act paid out of the same budget as translation, so this build's tick is
    // deliberately not the pre-change binary's. The pre-change payload is still read and still
    // hashed above; the continuation is re-anchored to this build's own recording
    // (`tests/r0a_fixtures.rs`).
    let recorded =
        std::fs::read(fixture("care-v11-shower-360-plus600-r0a.cubw")).expect("fixture");
    let (_, expected) = decode_snapshot(&recorded).expect("this build's recording loads");
    assert_ne!(as_v11(&expected), payload(&plus600), "R0a must actually move this world");
    assert_eq!(
        as_v11(&world.state),
        as_v11(&expected),
        "600 ticks of the dose build diverged from this build's recorded continuation"
    );
    // The ecology projection is unmoved too, so a care run stays comparable to a no-care one.
    assert_eq!(
        ecology_hash(&world.state),
        fnv1a(&postcard::to_allocvec(&v7::project(&world.state)).unwrap())
    );
}

/// Schema 11's own change was the hunter shape, and [`v11::WorldStateV11`] deliberately borrows
/// the live [`cubarium_core::HunterState`] rather than freezing a second copy. This is the
/// guard on that borrow: a genuine schema 11 payload with a **real** trial in it — one founder,
/// one live member, actual imports — must still load and continue exactly. If the hunter shape
/// ever changes without being frozen here, this fails instead of misreading a live world.
#[test]
fn a_genuine_schema_eleven_hunter_world_migrates_and_continues_exactly() {
    let bytes = std::fs::read(fixture("care-v11-hunters-200.cubw")).expect("fixture");
    let plus600 = std::fs::read(fixture("care-v11-hunters-200-plus600.cubw")).expect("fixture");
    assert_eq!(fnv1a(payload(&bytes)), 0x23f2_87b7_b8fa_ff79, "the provenance's own hash");
    assert_eq!(fnv1a(payload(&plus600)), 0xa4fe_bcf3_2cf4_0b8b, "the provenance's own hash");

    let (meta, state) = decode_snapshot(&bytes).expect("a genuine schema 11 hunter world loads");
    assert_eq!(meta.schema, SCHEMA_V11);
    assert_eq!(state.tick, 200);
    assert!(state.hunters.active(), "the fixture carries an actual trial, not an empty extension");
    assert_eq!(state.hunters.members.len(), 1, "one live member");
    assert!(state.hunters.imported_material() > 0.0 && state.hunters.imported_energy() > 0.0);
    assert_eq!(state.care.admitted_seq, 1, "and one real care command");
    assert!(state.care.showers.is_empty());
    assert_eq!(as_v11(&state), payload(&bytes), "the schema 11 projection is the original payload");

    let mut world = World::from_state(state).expect("valid");
    world.check_invariants().expect("invariants hold");
    for _ in 0..600 {
        world.step();
    }
    assert_eq!(world.tick(), 800);
    // R0a (`design/handoffs/r0a-movement-foundation-2026-09-14.md`) made body rotation a
    // physical act paid out of the same budget as translation, so this build's tick is
    // deliberately not the pre-change binary's. The pre-change payload is still read and still
    // hashed above; the continuation is re-anchored to this build's own recording
    // (`tests/r0a_fixtures.rs`).
    let recorded =
        std::fs::read(fixture("care-v11-hunters-200-plus600-r0a.cubw")).expect("fixture");
    let (_, expected) = decode_snapshot(&recorded).expect("this build's recording loads");
    assert_ne!(as_v11(&expected), payload(&plus600), "R0a must actually move this world");
    assert_eq!(
        as_v11(&world.state),
        as_v11(&expected),
        "600 ticks with a live hunter diverged from this build's recorded continuation"
    );
}

/// A schema 11 payload that is not a snapshot of anything is a decode failure, not a panic,
/// and a schema 11 header over schema 12 bytes fails its checksum first.
#[test]
fn a_damaged_schema_eleven_payload_is_refused() {
    let bytes = std::fs::read(fixture("care-v11-shower-360.cubw")).expect("fixture");
    let mut corrupt = bytes.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0xff;
    assert!(decode_snapshot(&corrupt).is_err());

    // A current snapshot relabelled as schema 11: the CRC still matches, and the mirror must
    // refuse the trailing dose byte rather than read the payload at the wrong offsets.
    let mut world = World::new(WorldConfig::default()).expect("valid");
    world.step();
    let receipt = world.apply_care(&CareCommand {
        seq: 1,
        apply_after_tick: 1,
        kind: CareKind::Rain,
        target: target(Face::Top, 32.0, 32.0),
        dose: CareDose::STANDARD,
    });
    assert!(receipt.outcome.applied().is_some(), "{:?}", receipt.outcome);
    let mut relabelled = encode_snapshot(&world.state, "mislabelled");
    relabelled[4..8].copy_from_slice(&SCHEMA_V11.to_le_bytes());
    assert!(
        decode_snapshot(&relabelled).is_err(),
        "a schema 12 payload read as schema 11 must fail, not silently misdecode"
    );
}

// ---------------------------------------------------------------- refusing lossy export

/// A world whose shower is falling at a nonstandard dose has **no** old image. Every legacy
/// projection refuses rather than writing a shower whose amount has silently become 1000 —
/// the projections exist to make two payloads comparable, and one that lied about the dose
/// would make two different worlds compare equal.
#[test]
fn a_nonstandard_shower_has_no_legacy_image_at_all() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    world.step();
    // The standard control: this world does project, everywhere.
    assert!(v11::project(&world.state).is_some());
    assert!(v10::project(&world.state).is_some());
    assert!(v9::project(&world.state).is_some());
    assert!(v8::project(&world.state).is_some());

    let receipt = world.apply_care(&CareCommand {
        seq: 1,
        apply_after_tick: 1,
        kind: CareKind::Rain,
        target: target(Face::Top, 32.0, 32.0),
        dose: CareDose::new(1500).expect("in range"),
    });
    assert!(receipt.outcome.applied().is_some(), "{:?}", receipt.outcome);
    assert_eq!(world.care().showers[0].dose_permille, 1500);

    assert!(v11::project(&world.state).is_none(), "schema 11 cannot hold a nonstandard shower");
    assert!(v10::project(&world.state).is_none());
    assert!(v9::project(&world.state).is_none());
    assert!(v8::project(&world.state).is_none());
    // Schema 7 drops care entirely, so it is still total — and `ecology_hash` with it. The
    // dose is a care value, and care has never been in the ecology hash.
    let ecology = ecology_hash(&world.state);
    let mut standard = World::new(WorldConfig::default()).expect("valid");
    standard.step();
    let plain = ecology_hash(&standard.state);
    assert_eq!(ecology, plain, "admitting a shower does not move the ecology projection");

    // And once the shower is over there is a legacy image again: the dose lived on the shower,
    // and the cumulative ledgers it moved are amounts, which the old shape has always held.
    for _ in 0..121 {
        world.step();
    }
    assert!(world.care().showers.is_empty());
    let old = v11::project(&world.state).expect("a finished shower leaves a representable world");
    assert!(old.care.rain_depth_in > 0.0, "the ledger carries the nonstandard depth across");
    assert_eq!(old.care.admitted_seq, 1);
}

/// The same refusal through a completed-shower boundary: a nonstandard shower on its **last**
/// undelivered sample still has no old image, because that sample has not fallen yet.
#[test]
fn the_refusal_holds_until_the_last_sample_has_fallen() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    world.step();
    world.apply_care(&CareCommand {
        seq: 1,
        apply_after_tick: 1,
        kind: CareKind::Rain,
        target: target(Face::Top, 32.0, 32.0),
        dose: CareDose::new(CareDose::MIN_PERMILLE).expect("the minimum is in range"),
    });
    // 119 of 120 samples delivered: still in flight, still unrepresentable.
    for _ in 0..119 {
        world.step();
    }
    assert_eq!(world.care().showers[0].delivered, 119);
    assert!(v11::project(&world.state).is_none());
    world.step();
    assert!(world.care().showers.is_empty(), "the 120th sample ends it");
    assert!(v11::project(&world.state).is_some());
}
