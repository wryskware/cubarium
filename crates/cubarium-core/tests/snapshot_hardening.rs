//! Snapshot hardening, from `design/m2-world-spec.md` "Persistence": "Header: magic `CUBW`,
//! schema version `u32`, build ID (git hash string), payload length `u64`, CRC32 of payload
//! … Load validates magic, version, length, CRC, then value ranges (finite, nonnegative,
//! capacities, canonical positions); on failure try the next older file."

use cubarium_core::snapshot::{HEADER_FIXED_BYTES, MAGIC, SnapshotError};
use cubarium_core::{SCHEMA_VERSION, World, WorldConfig, decode_snapshot, encode_snapshot};

const BUILD: &str = "0123456789abcdef";

fn stepped_world(ticks: u64) -> World {
    let mut world = World::new(WorldConfig::default()).expect("defaults are a valid world");
    for _ in 0..ticks {
        world.step();
    }
    world
}

/// Spec: the schema version is stored in the header. Version 2 added the Monod nutrient
/// term's `K_N` and `capacity.field_dump_seconds` to the config, so version 1 payloads
/// would misdecode under `postcard` and are rejected outright.
#[test]
fn the_schema_version_is_two() {
    assert_eq!(SCHEMA_VERSION, 2);
    assert_eq!(MAGIC, *b"CUBW");
}

/// Spec, "Observer": "fixed-seed replay hash of the state"; encoding must not depend on
/// anything but the state.
#[test]
fn encoding_is_byte_for_byte_deterministic() {
    let world = stepped_world(120);
    let first = encode_snapshot(&world.state, BUILD);
    let second = encode_snapshot(&world.state, BUILD);
    assert_eq!(first, second, "two encodings of the same state differ");

    let (meta, back) = decode_snapshot(&first).expect("round trip");
    assert_eq!(meta.schema, SCHEMA_VERSION);
    assert_eq!(meta.build_id, BUILD);
    assert_eq!(meta.payload_len as usize, first.len() - HEADER_FIXED_BYTES - BUILD.len());
    assert_eq!(back, world.state);
}

#[test]
fn a_corrupt_magic_is_rejected() {
    let world = stepped_world(10);
    let mut bytes = encode_snapshot(&world.state, BUILD);
    bytes[0] ^= 0xff;
    assert_eq!(decode_snapshot(&bytes), Err(SnapshotError::BadMagic));
    assert_eq!(decode_snapshot(b"NOPE and then some"), Err(SnapshotError::BadMagic));
}

#[test]
fn a_truncated_payload_is_rejected() {
    let world = stepped_world(10);
    let bytes = encode_snapshot(&world.state, BUILD);

    assert_eq!(decode_snapshot(&[]), Err(SnapshotError::Truncated));
    for cut in [
        1usize,
        MAGIC.len(),
        MAGIC.len() + 2,
        HEADER_FIXED_BYTES + BUILD.len() - 1,
        HEADER_FIXED_BYTES + BUILD.len(),
        bytes.len() / 2,
        bytes.len() - 1,
    ] {
        assert_eq!(
            decode_snapshot(&bytes[..cut]),
            Err(SnapshotError::Truncated),
            "a snapshot cut to {cut} of {} bytes decoded",
            bytes.len()
        );
    }

    // Trailing bytes contradict the declared payload length just as a short read does.
    let mut longer = bytes.clone();
    longer.push(0x5a);
    assert_eq!(decode_snapshot(&longer), Err(SnapshotError::Truncated));
}

#[test]
fn a_flipped_payload_byte_fails_the_checksum() {
    let world = stepped_world(10);
    let bytes = encode_snapshot(&world.state, BUILD);
    let payload_start = HEADER_FIXED_BYTES + BUILD.len();

    for offset in [0usize, 1, (bytes.len() - payload_start) / 2, bytes.len() - payload_start - 1] {
        let mut corrupt = bytes.clone();
        corrupt[payload_start + offset] ^= 0x01;
        assert_eq!(
            decode_snapshot(&corrupt),
            Err(SnapshotError::BadChecksum),
            "flipping payload byte {offset} was not caught"
        );
    }
}

#[test]
fn a_foreign_schema_is_rejected_with_its_version() {
    let world = stepped_world(10);
    let mut bytes = encode_snapshot(&world.state, BUILD);
    let foreign = SCHEMA_VERSION + 1;
    bytes[4..8].copy_from_slice(&foreign.to_le_bytes());
    assert_eq!(decode_snapshot(&bytes), Err(SnapshotError::UnsupportedSchema(foreign)));

    let mut older = encode_snapshot(&world.state, BUILD);
    older[4..8].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(decode_snapshot(&older), Err(SnapshotError::UnsupportedSchema(0)));
}

/// Spec: "Load validates magic, version, length, CRC, then value ranges". A header that is
/// perfectly well formed must still be rejected when the state it carries is out of range.
#[test]
fn a_well_formed_header_over_an_out_of_range_state_is_rejected() {
    let world = stepped_world(10);
    let bytes = encode_snapshot(&world.state, BUILD);
    let (_, mut state) = decode_snapshot(&bytes).expect("round trip");

    // A non-finite organism energy is a value-range failure, not a framing failure.
    let (_, first) = state.organisms.iter_mut().next().expect("founders exist");
    first.energy = f64::NAN;
    let poisoned = encode_snapshot(&state, BUILD);
    match decode_snapshot(&poisoned) {
        Err(SnapshotError::Invalid(_)) => {}
        other => panic!("a NaN organism energy decoded as {other:?}"),
    }

    // A negative reserve is material that does not exist.
    let (_, mut state) = decode_snapshot(&bytes).expect("round trip");
    let (_, first) = state.organisms.iter_mut().next().expect("founders exist");
    first.reserve = -1.0;
    match decode_snapshot(&encode_snapshot(&state, BUILD)) {
        Err(SnapshotError::Invalid(_)) => {}
        other => panic!("a negative reserve decoded as {other:?}"),
    }

    // A position off the chart is not canonical.
    let (_, mut state) = decode_snapshot(&bytes).expect("round trip");
    let (_, first) = state.organisms.iter_mut().next().expect("founders exist");
    first.pos.u = 64.0;
    match decode_snapshot(&encode_snapshot(&state, BUILD)) {
        Err(SnapshotError::Invalid(_)) => {}
        other => panic!("a non-canonical position decoded as {other:?}"),
    }

    // A negative field stock is equally impossible.
    let (_, mut state) = decode_snapshot(&bytes).expect("round trip");
    state.fields.n[0] = -1e-6;
    match decode_snapshot(&encode_snapshot(&state, BUILD)) {
        Err(SnapshotError::Invalid(_)) => {}
        other => panic!("a negative nutrient cell decoded as {other:?}"),
    }
}

/// Spec, "Capacity and IDs": cap 512 active. A snapshot at the cap must survive the trip.
#[test]
fn a_full_world_round_trips() {
    let mut config = WorldConfig::default();
    config.founders.count = 512;
    assert_eq!(config.capacity.max_organisms, 512, "the default cap is the spec's 512");
    let mut world = World::new(config).expect("a world at its cap is valid");
    assert_eq!(world.population(), 512);
    for _ in 0..50 {
        world.step();
    }

    let bytes = encode_snapshot(&world.state, BUILD);
    let (meta, state) = decode_snapshot(&bytes).expect("a full world must decode");
    assert_eq!(meta.build_id, BUILD);
    assert_eq!(state.organisms.len(), world.population());
    assert_eq!(state, world.state, "a full world did not survive the round trip");

    let restored = World::from_state(state).expect("a full world must rebuild");
    assert_eq!(restored.population(), 512);
    restored.check_invariants().expect("a restored full world satisfies its invariants");
}
