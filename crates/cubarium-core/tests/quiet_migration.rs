//! Schema 13 migration: the ordinary quiet extension against genuine pre-change artifacts.
//!
//! The load-bearing tests are the two continuations. Four fixtures written by the **pre-quiet**
//! release build (commit `1e6d053`, provenance in `tests/fixtures/quiet-v12-provenance.md`) prove
//! that schema 13 reads a real schema 12 world — one plain, one carrying actual care ledgers —
//! steps it 600 ticks with the policy Off, and reproduces the old binary's payload byte for byte.
//!
//! The other direction is the honest one: an enabled policy, or a held pause, has **no** schema
//! 12 image, and the projection refuses rather than writing a world that quietly lost a timer.

use std::path::PathBuf;

use cubarium_core::organism::Mode;
use cubarium_core::quiet::{
    POST_BIRTH_PAUSE_TICKS, QuietPause, QuietPolicy, QuietState,
};
use cubarium_core::snapshot::{HEADER_FIXED_BYTES, SnapshotError, state_hash, v11, v12};
use cubarium_core::{
    SCHEMA_V12, SCHEMA_VERSION, World, WorldConfig, WorldState, decode_snapshot, encode_snapshot,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// The postcard payload of a snapshot file: everything after the variable-length header.
fn payload(bytes: &[u8]) -> &[u8] {
    let id_len = usize::from(u16::from_le_bytes(bytes[8..10].try_into().expect("2 bytes")));
    &bytes[HEADER_FIXED_BYTES + id_len..]
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// The schema 12 image of a state, re-encoded — what the pre-quiet binary would have written.
#[allow(dead_code)]
fn as_v12(state: &WorldState) -> Vec<u8> {
    postcard::to_allocvec(&v12::project(state).expect("an Off world has a schema 12 image"))
        .expect("the projection is encodable")
}

// -------------------------------------- retired migrations, kept as refusals (§15.1)

/// **Retired by ecology v1** (`design/ecology-v1-contract.md` §15.1). Two tests lived here:
/// both genuine pre-quiet schema 12 worlds migrating Off and projecting back byte for byte,
/// and their 600-tick continuations. Worlds always restart fresh and are never migrated
/// (Wrysk, 2026-09-15), so schema 16 refuses schema 12 by name; the artifacts cannot be loaded
/// and this build cannot write a schema 12 world to re-anchor against.
///
/// The fixtures stay in the tree with their provenance note, their recorded hashes are still
/// asserted here — a file quietly replaced still fails — and the refusal is checked on the same
/// bytes.
#[test]
fn the_pre_quiet_schema_twelve_fixtures_are_refused_by_name() {
    for (name, hash) in [
        ("quiet-v12-plain-3000.cubw", Some(0x55e9_f1e2_2395_4635u64)),
        ("quiet-v12-plain-3000-plus600.cubw", Some(0x7729_2b3e_79cc_fcd1)),
        ("quiet-v12-plain-3000-plus600-r0b.cubw", None),
        ("quiet-v12-care-3000.cubw", Some(0x342d_78f1_ea06_cb0b)),
        ("quiet-v12-care-3000-plus600.cubw", Some(0x19ef_5ad1_afd0_2b2b)),
        ("quiet-v12-care-3000-plus600-r0b.cubw", None),
    ] {
        let bytes = std::fs::read(fixture(name)).expect("the fixture is committed");
        if let Some(h) = hash {
            assert_eq!(fnv1a(payload(&bytes)), h, "{name}: the provenance's own hash");
        }
        let schema = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert!(schema < SCHEMA_VERSION, "{name} is schema {schema}");
        assert_eq!(
            decode_snapshot(&bytes),
            Err(SnapshotError::UnsupportedSchema(schema)),
            "{name}: an old world is refused by name, never migrated"
        );
    }
    assert_eq!(SCHEMA_V12, 12);
}

// ---------------------------------------------------------------- refusing a lossy image

/// An enabled policy has no schema 12 image, and neither does a held pause. The old shape has
/// nowhere to put either, and a projection that dropped them would make two different worlds
/// compare equal — exactly the comparison these projections exist to make trustworthy.
///
/// Stated on a world this build makes: the schema 12 artifact it used to read no longer loads
/// (§15.1), and the claim is about the projection, not about that recording.
#[test]
fn an_enabled_policy_or_a_held_pause_has_no_old_image() {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 4;
    let mut world = World::new(cfg).expect("valid");
    for _ in 0..40 {
        world.step();
        world.drain_events();
    }
    let off = world.state;

    // The control: Off really does project, and so do the older mirrors.
    assert!(v12::project(&off).is_some());
    assert!(v11::project(&off).is_some());

    let mut enabled = off.clone();
    enabled.quiet = QuietState::post_birth_pause_v1();
    assert!(v12::project(&enabled).is_none(), "an enabled policy must refuse to project");
    assert!(v11::project(&enabled).is_none(), "and so must every older mirror");
    assert_ne!(state_hash(&enabled), state_hash(&off), "it is a different world, and hashes as one");

    // A held pause under a policy that is somehow not enabled is refused too: the entry is the
    // thing the old shape cannot hold, whatever the label beside it says.
    let parent = off.organisms.iter().next().expect("a populated world").0;
    let child = off.organisms.iter().nth(1).expect("two organisms").0;
    let mut stale = off.clone();
    stale.quiet.pauses.push(QuietPause {
        parent,
        child,
        start_tick: off.tick,
        end_tick: off.tick + POST_BIRTH_PAUSE_TICKS,
        underlying: Mode::Seeking,
    });
    assert!(v12::project(&stale).is_none());
    // And that state is itself invalid: Off does not carry entries.
    assert!(stale.validate().is_err());
}

/// Schema 12 payloads and schema 16 payloads are not interchangeable, in either direction.
#[test]
fn a_relabelled_payload_is_refused_rather_than_misread() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    world.step();
    let state = world.state;

    // A schema 16 payload relabelled as schema 12: refused by name, without ever reading the
    // payload at the wrong offsets.
    let mut relabelled = encode_snapshot(&state, "mislabelled");
    assert_eq!(u32::from_le_bytes(relabelled[4..8].try_into().unwrap()), SCHEMA_VERSION);
    relabelled[4..8].copy_from_slice(&SCHEMA_V12.to_le_bytes());
    assert_eq!(
        decode_snapshot(&relabelled),
        Err(SnapshotError::UnsupportedSchema(SCHEMA_V12))
    );

    // And the other way: a genuine schema 12 file relabelled as schema 16 is short by every
    // extension it never had, so it fails the exact-decode check rather than misreading.
    let bytes = std::fs::read(fixture("quiet-v12-plain-3000.cubw")).expect("fixture");
    let mut forward = bytes.clone();
    forward[4..8].copy_from_slice(&SCHEMA_VERSION.to_le_bytes());
    assert!(matches!(decode_snapshot(&forward), Err(SnapshotError::Decode(_))));

    // A corrupt payload is a checksum or decode failure, not a panic.
    let mut corrupt = relabelled.clone();
    corrupt[4..8].copy_from_slice(&SCHEMA_VERSION.to_le_bytes());
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0xff;
    assert!(decode_snapshot(&corrupt).is_err());
}

/// A hostile schema 13 state is refused at the decoder, one property at a time — the same
/// checks `QuietState::validate` makes, reached through the public door a real load uses.
#[test]
fn a_malformed_schema_thirteen_state_is_refused_by_the_decoder() {
    let mut cfg = WorldConfig::default();
    cfg.founders.count = 4;
    let mut base = World::new(cfg).expect("valid").state;
    // Past the window, so "already expired" is expressible against this world's own clock.
    base.tick = 1000;
    base.quiet = QuietState::post_birth_pause_v1();
    let parent = base.organisms.iter().next().expect("a founder").0;
    let child = base.organisms.iter().nth(1).expect("two founders").0;
    let sound = QuietPause {
        parent,
        child,
        start_tick: base.tick,
        end_tick: base.tick + POST_BIRTH_PAUSE_TICKS,
        underlying: Mode::Seeking,
    };

    // The control: an enabled world holding one sound pause really does decode.
    let mut good = base.clone();
    good.quiet.pauses.push(sound);
    decode_snapshot(&encode_snapshot(&good, "quiet")).expect("the control loads");

    /// One way of writing a state the decoder must refuse, named so a failure says which.
    type Damage = fn(&mut WorldState, QuietPause);
    let damage: [(&str, Damage); 7] = [
        ("version", |s, p| {
            s.quiet.pauses.push(p);
            s.quiet.version = 2;
        }),
        ("off-with-entries", |s, p| {
            s.quiet.pauses.push(p);
            s.quiet.policy = QuietPolicy::Off;
        }),
        ("wrong-duration", |s, mut p| {
            p.end_tick = p.start_tick + 39;
            s.quiet.pauses.push(p);
        }),
        ("stepped-past", |s, mut p| {
            p.start_tick = 0;
            p.end_tick = POST_BIRTH_PAUSE_TICKS;
            s.quiet.pauses.push(p);
        }),
        ("self-child", |s, mut p| {
            p.child = p.parent;
            s.quiet.pauses.push(p);
        }),
        ("duplicate", |s, p| {
            s.quiet.pauses.push(p);
            s.quiet.pauses.push(p);
        }),
        ("dead-parent", |s, mut p| {
            p.parent.generation += 7;
            s.quiet.pauses.push(p);
        }),
    ];
    for (name, break_it) in damage {
        let mut bad = base.clone();
        break_it(&mut bad, sound);
        assert!(bad.validate().is_err(), "{name}: validate accepted it");
        match decode_snapshot(&encode_snapshot(&bad, "quiet")) {
            Err(SnapshotError::Invalid(_)) => {}
            other => panic!("{name}: decoded as {other:?}"),
        }
    }
}

/// A future tick is refused too, and the exact boundaries are the ones the contract names.
#[test]
fn the_decoders_tick_window_is_the_contracts_window() {
    let mut cfg = WorldConfig::default();
    cfg.founders.count = 4;
    let mut base = World::new(cfg).expect("valid").state;
    base.tick = 1000;
    base.quiet = QuietState::post_birth_pause_v1();
    let parent = base.organisms.iter().next().expect("a founder").0;
    let child = base.organisms.iter().nth(1).expect("two founders").0;
    let at = |start: u64| QuietPause {
        parent,
        child,
        start_tick: start,
        end_tick: start + POST_BIRTH_PAUSE_TICKS,
        underlying: Mode::Seeking,
    };

    // Started at the current tick: the first held decision, valid.
    let mut now = base.clone();
    now.quiet.pauses.push(at(1000));
    now.validate().expect("a pause starting now is held");
    // Ending exactly now: the window is complete and the entry is awaiting the release decision
    // that consumes it, so a world snapshotted here is valid and resumes exactly.
    let mut done = base.clone();
    done.quiet.pauses.push(at(1000 - POST_BIRTH_PAUSE_TICKS));
    done.validate().expect("a pause at its release boundary is still loadable");
    // One tick past it is stale: the release should already have happened.
    let mut stale = base.clone();
    stale.quiet.pauses.push(at(1000 - POST_BIRTH_PAUSE_TICKS - 1));
    assert!(stale.validate().is_err());
    // Ending one tick from now: still held.
    let mut last = base.clone();
    last.quiet.pauses.push(at(1000 - POST_BIRTH_PAUSE_TICKS + 1));
    last.validate().expect("the last held decision is still held");
    // Starting after now: a timer the world has not reached.
    let mut future = base.clone();
    future.quiet.pauses.push(at(1001));
    assert!(future.validate().is_err());
    // Overflowing its end tick.
    let mut over = base.clone();
    over.tick = u64::MAX - 1;
    over.quiet.pauses.push(QuietPause {
        parent,
        child,
        start_tick: u64::MAX - 5,
        end_tick: u64::MAX,
        underlying: Mode::Seeking,
    });
    assert!(over.validate().is_err());
}
