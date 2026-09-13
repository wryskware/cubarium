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
    POST_BIRTH_PAUSE_TICKS, QuietPause, QuietPolicy, QuietState, QUIET_VERSION,
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
fn as_v12(state: &WorldState) -> Vec<u8> {
    postcard::to_allocvec(&v12::project(state).expect("an Off world has a schema 12 image"))
        .expect("the projection is encodable")
}

// ---------------------------------------------------------------- genuine continuation

/// Both genuine pre-change worlds — the plain one and the one carrying real care — migrate with
/// the quiet extension Off and no retroactive pauses, and their schema 12 projections are the
/// original payloads byte for byte.
#[test]
fn the_genuine_pre_quiet_worlds_migrate_off_and_project_back_exactly() {
    for (name, hash) in [
        ("quiet-v12-plain-3000.cubw", 0x55e9_f1e2_2395_4635u64),
        ("quiet-v12-care-3000.cubw", 0x342d_78f1_ea06_cb0bu64),
    ] {
        let bytes = std::fs::read(fixture(name)).expect("the fixture is committed");
        assert_eq!(fnv1a(payload(&bytes)), hash, "{name}: the provenance's own hash");

        let (meta, state) = decode_snapshot(&bytes).expect("a genuine schema 12 snapshot loads");
        assert_eq!(meta.schema, SCHEMA_V12, "the header reports what was read");
        assert_eq!(meta.build_id, "pre-quiet-fixture");
        assert_ne!(SCHEMA_VERSION, SCHEMA_V12);
        assert_eq!(state.tick, 147_000);
        assert!(state.births_total > 0, "{name}: a world that has really reproduced");
        assert_eq!(state.hunters, Default::default(), "and carries no hunter");

        // Off, empty, and version 1: no timer was invented from an old birth log.
        assert_eq!(state.quiet, QuietState::default());
        assert_eq!(state.quiet.version, QUIET_VERSION);
        assert_eq!(state.quiet.policy, QuietPolicy::Off);
        assert!(state.quiet.pauses.is_empty());
        assert!(!state.quiet.active());

        // The care world really does carry care; the plain one really does not.
        if name.contains("care") {
            assert_eq!(state.care.admitted_seq, 1);
            assert!(state.care.feed_material_in > 0.0 && state.care.feed_energy_in > 0.0);
        } else {
            assert_eq!(state.care, Default::default());
        }

        assert_eq!(as_v12(&state), payload(&bytes), "{name}: the projection is not the original");
        assert_ne!(state_hash(&state), fnv1a(payload(&bytes)), "schema 13 appends three bytes");
    }
}

/// The claim the migration rests on: an Off world resuming a pre-quiet snapshot produces the
/// pre-quiet binary's next 600 ticks **byte for byte** — for the default no-care trajectory and
/// for the default care one alike.
#[test]
fn an_off_world_reproduces_the_pre_quiet_binarys_next_600_ticks() {
    for (open, plus600, hash) in [
        (
            "quiet-v12-plain-3000.cubw",
            "quiet-v12-plain-3000-plus600.cubw",
            0x7729_2b3e_79cc_fcd1u64,
        ),
        (
            "quiet-v12-care-3000.cubw",
            "quiet-v12-care-3000-plus600.cubw",
            0x19ef_5ad1_afd0_2b2bu64,
        ),
    ] {
        let start = std::fs::read(fixture(open)).expect("fixture");
        let after = std::fs::read(fixture(plus600)).expect("fixture");
        assert_eq!(fnv1a(payload(&after)), hash, "{plus600}: the provenance's own hash");

        let (_, state) = decode_snapshot(&start).expect("schema 12 loads");
        let mut world = World::from_state(state).expect("the migrated state is a valid world");
        world.check_invariants().expect("invariants hold on the migrated world");
        for _ in 0..600 {
            world.step();
            world.drain_events();
            assert!(world.drain_quiet_events().is_empty(), "{open}: an Off world published a record");
        }
        assert_eq!(world.tick(), 147_600);
        assert_eq!(
            as_v12(&world.state),
            payload(&after),
            "{open}: 600 ticks of the quiet build diverged from the pre-quiet binary"
        );
        assert!(world.quiet().pauses.is_empty());
    }
}

// ---------------------------------------------------------------- refusing a lossy image

/// An enabled policy has no schema 12 image, and neither does a held pause. The old shape has
/// nowhere to put either, and a projection that dropped them would make two different worlds
/// compare equal — exactly the comparison these projections exist to make trustworthy.
#[test]
fn an_enabled_policy_or_a_held_pause_has_no_old_image() {
    let bytes = std::fs::read(fixture("quiet-v12-plain-3000.cubw")).expect("fixture");
    let (_, off) = decode_snapshot(&bytes).expect("loads");

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

/// Schema 12 payloads and schema 13 payloads are not interchangeable, in either direction: the
/// appended extension is why the frozen mirror exists.
#[test]
fn a_relabelled_payload_is_refused_rather_than_misread() {
    let bytes = std::fs::read(fixture("quiet-v12-plain-3000.cubw")).expect("fixture");
    let (_, state) = decode_snapshot(&bytes).expect("loads");

    // A schema 13 payload relabelled as schema 12: the CRC still matches, and the mirror must
    // refuse the trailing extension rather than read the payload at the wrong offsets.
    let mut relabelled = encode_snapshot(&state, "mislabelled");
    assert_eq!(u32::from_le_bytes(relabelled[4..8].try_into().unwrap()), SCHEMA_VERSION);
    relabelled[4..8].copy_from_slice(&SCHEMA_V12.to_le_bytes());
    assert!(
        decode_snapshot(&relabelled).is_err(),
        "a schema 13 payload read as schema 12 must fail, not silently misdecode"
    );

    // And the other way: the genuine schema 12 file relabelled as schema 13 is short by the
    // extension it never had.
    let mut forward = bytes.clone();
    forward[4..8].copy_from_slice(&SCHEMA_VERSION.to_le_bytes());
    assert!(decode_snapshot(&forward).is_err());

    // A corrupt schema 12 payload is a decode failure, not a panic.
    let mut corrupt = bytes.clone();
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
