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
use cubarium_core::snapshot::{HEADER_FIXED_BYTES, v8, v9, v10, v11};
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
#[allow(dead_code)]
fn as_v11(state: &cubarium_core::WorldState) -> Vec<u8> {
    postcard::to_allocvec(&v11::project(state).expect("a standard world has a schema 11 image"))
        .expect("the projection is encodable")
}

fn target(face: Face, u: f64, v: f64) -> CareTarget {
    let p = SurfacePoint::new(face, u, v);
    CareTarget { face: p.face.index() as u8, u: p.u, v: p.v }
}

// ---------------------------------------- retired migrations, kept as refusals (§15.1)

/// **Retired by ecology v1** (`design/ecology-v1-contract.md` §15.1). Three tests lived here,
/// all resting on genuine pre-dose schema 11 artifacts: the mid-shower world migrating at the
/// standard dose, its 600-tick continuation, and the same for a world carrying a real hunter
/// trial. Wrysk's standing rule of 2026-09-15 is that worlds always restart fresh and are
/// never migrated, so schema 16 refuses schema 11 by name. The artifacts cannot be loaded and
/// this build cannot produce a schema 11 world to re-anchor against, so the continuations are
/// retired rather than re-recorded; the fixture files are kept and the provenance note records
/// it.
///
/// What is still checkable on the real recorded bytes is the refusal — and it is checked
/// against each fixture's **own** hash, so a silently substituted file fails here.
#[test]
fn the_pre_dose_schema_eleven_fixtures_are_refused_by_name() {
    for (name, hash) in [
        ("care-v11-shower-360.cubw", Some(0x239b_a63f_523d_93abu64)),
        ("care-v11-shower-360-plus600.cubw", Some(0xdc91_056c_8370_e7fa)),
        ("care-v11-shower-360-plus600-r0b.cubw", None),
        ("care-v11-hunters-200.cubw", Some(0x23f2_87b7_b8fa_ff79)),
        ("care-v11-hunters-200-plus600.cubw", Some(0xa4fe_bcf3_2cf4_0b8b)),
        ("care-v11-hunters-200-plus600-r0b.cubw", None),
    ] {
        let bytes = std::fs::read(fixture(name)).expect("the fixture is committed");
        if let Some(h) = hash {
            assert_eq!(fnv1a(payload(&bytes)), h, "{name}: the provenance's own hash");
        }
        let schema = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert!(schema < SCHEMA_VERSION, "{name} is schema {schema}");
        assert_eq!(
            decode_snapshot(&bytes),
            Err(cubarium_core::SnapshotError::UnsupportedSchema(schema)),
            "{name}: an old world is refused by name, never migrated"
        );
    }
    assert_eq!(SCHEMA_V11, 11);
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
