//! **Adopting the reach envelope as the shipped pursuit stopping rule**, and the schema bump
//! that keeps the adoption honest.
//!
//! Workstream V of the ecology v1 next steps
//! (`design/handoffs/ecology-v1-predicate-adoption-opus-2026-09-16.md`). The paired
//! measurement is `design/7_Research/ecology-v1-apex-predicate-2026-09-16.md`: the shipped
//! forward half-space held a member at its resting effort on 89.4 % of the bursts it had just
//! paid for, and correcting it to the reach envelope the predicate's own comment names raised
//! contacts 88 → 140 and captures 38 → 67 over 32 lives.
//!
//! What these tests fix:
//!
//! - an **ordinary world runs the envelope without being told**, and so does a world rebuilt
//!   from a state;
//! - **schema 17 refuses schema 16 by number**, because a world's saved bytes do not carry the
//!   rule: resuming schema-16 bytes under this build would change behaviour silently, which is
//!   a migration by another name (Wrysk's always-fresh rule of 2026-09-15);
//! - the **resume regression**: a world saved and resumed runs the envelope on both sides and
//!   the resumed hash equals the uninterrupted one;
//! - a world told [`PursuitStop::ForwardHalfSpace`] before its first tick reproduces **the
//!   hashes printed by the brief commit `960b6c2`, before the default was flipped**, so the
//!   retained rows stay reproducible;
//! - and the new default reproduces the **envelope** hashes printed by that same commit, so
//!   the flip moved the default and nothing else.
//!
//! The four pinned hash sets below were printed by `960b6c2` — the tree before this
//! workstream changed a line — from exactly the fixture in this file.

use cubarium_core::hunter::{FixedHunterProfile, HunterTarget, PursuitStop};
use cubarium_core::snapshot::{SCHEMA_VERSION, state_hash};
use cubarium_core::{SnapshotError, World, WorldConfig, decode_snapshot, encode_snapshot};

/// The schema this brief's semantics-only bump refuses.
const SCHEMA_V16: u32 = 16;

// ---------------------------------------------------------------- the fixture

/// Two apex adults introduced into an ordinary default world after a 1,000-tick warm-up.
///
/// The warm-up holds no hunter, so it reaches no pursuit predicate and is identical under
/// either rule; everything that follows is the rule under test. `stop` is `None` for a world
/// that is never told, which is the point of the whole brief.
fn two_apex_world(stop: Option<PursuitStop>) -> World {
    let mut world = World::new(WorldConfig::default()).expect("the default world is valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    for _ in 0..1_000 {
        world.step();
        world.drain_events();
    }
    world
        .introduce_hunters(
            profile,
            &[
                HunterTarget { face: 1, u: 23.0, v: 31.0 },
                HunterTarget { face: 4, u: 55.0, v: 3.0 },
            ],
        )
        .expect("two adults are placed");
    world.record_strike_attempts(true);
    if let Some(stop) = stop {
        world.set_pursuit_stop(stop);
    }
    world
}

/// Step `ticks`, draining everything every tick, and return the state hash at every
/// 1,500-tick boundary together with how many paid attempts the arm actually made. A run that
/// reached no attempt is not evidence about a pursuit rule, so the count is returned rather
/// than assumed.
fn marks(world: &mut World, ticks: u64) -> (Vec<u64>, usize) {
    let mut out = Vec::new();
    let mut attempts = 0usize;
    for tick in 1..=ticks {
        world.step();
        world.drain_events();
        world.drain_hunter_events();
        attempts += world.drain_strike_records().0.len();
        if tick.is_multiple_of(1_500) {
            out.push(state_hash(&world.state));
        }
    }
    (out, attempts)
}

/// Printed by `960b6c2` from `two_apex_world(Some(ForwardHalfSpace))` — and, at that commit,
/// from an untouched world too, because the half-space was the default there.
const HALF_SPACE_9K: [u64; 6] = [
    0x7ff9ba312bda102e,
    0xc895158b0a7affc9,
    0xcd96643f5f85f604,
    0x4537eb7aac6e8d69,
    0x80210a045914d62c,
    0x1d45c6949f734e14,
];

/// Printed by `960b6c2` from `two_apex_world(Some(ReachEnvelope))`, when naming the envelope
/// was an opt-in variant. It is the default now, so an untouched world must reproduce it.
const ENVELOPE_9K: [u64; 6] = [
    0x7ff9ba312bda102e,
    0xc895158b0a7affc9,
    0xcd96643f5f85f604,
    0xd09b8d15bc154c3f,
    0x437e9f506e2c6814,
    0x1132f4500386b81b,
];

/// The two arms agree until the first attempt whose rule differs, which is what makes the
/// pair a measurement of the predicate rather than of two unrelated worlds.
const FIRST_DIVERGENT_MARK: usize = 3;

// ---------------------------------------------------------------- the shipped rule

/// The whole of deliverable 1 in one reading: the enum's default, an ordinary world's rule,
/// and a world rebuilt from a state — nobody is told anything.
#[test]
fn an_ordinary_world_runs_the_reach_envelope_without_being_told() {
    assert_eq!(
        PursuitStop::default(),
        PursuitStop::ReachEnvelope,
        "the shipped rule is the reach envelope from 2026-09-16"
    );

    let fresh = World::new(WorldConfig::default()).expect("the default world is valid");
    assert_eq!(fresh.pursuit_stop(), PursuitStop::ReachEnvelope);

    // A world rebuilt from a state carries no rule in its bytes, so it must land on the
    // shipped one — which is exactly why the schema had to move.
    let rebuilt = World::from_state(fresh.state.clone()).expect("a valid state rebuilds");
    assert_eq!(rebuilt.pursuit_stop(), PursuitStop::ReachEnvelope);

    // The rule before 2026-09-16 is still reachable, so the retained rows stay reproducible.
    let mut opted = World::new(WorldConfig::default()).expect("valid");
    opted.set_pursuit_stop(PursuitStop::ForwardHalfSpace);
    assert_eq!(opted.pursuit_stop(), PursuitStop::ForwardHalfSpace);
}

// ---------------------------------------------------------------- schema 17

/// Schema 17 is a **semantics-only** bump: the payload shape did not move, so nothing but the
/// header can tell a schema-16 world from a schema-17 one. A schema-16 header over a payload
/// that is in every other way valid is refused by number, which is what stops a world saved
/// under the half-space from being silently resumed under the envelope.
#[test]
fn a_schema_sixteen_header_over_a_valid_payload_is_refused_by_number() {
    assert_eq!(SCHEMA_VERSION, 17, "the predicate adoption is schema 17");

    let world = World::new(WorldConfig::default()).expect("valid");
    let bytes = encode_snapshot(&world.state, "adoption-test");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), SCHEMA_VERSION);
    // The payload is untouched and its CRC covers the payload only, so the only thing wrong
    // with this file is the version it claims.
    let mut relabelled = bytes.clone();
    relabelled[4..8].copy_from_slice(&SCHEMA_V16.to_le_bytes());
    assert_eq!(
        decode_snapshot(&relabelled),
        Err(SnapshotError::UnsupportedSchema(SCHEMA_V16)),
        "a schema 16 world must be refused by number, not resumed under a rule it never ran"
    );
    // And the same file under its own number still loads: the refusal is about the version.
    let (meta, _) = decode_snapshot(&bytes).expect("schema 17 decodes");
    assert_eq!(meta.schema, 17);

    // Every older schema is still refused by its own number, 16 included.
    for old in 7..SCHEMA_VERSION {
        let mut older = bytes.clone();
        older[4..8].copy_from_slice(&old.to_le_bytes());
        assert_eq!(decode_snapshot(&older), Err(SnapshotError::UnsupportedSchema(old)), "schema {old}");
    }
}

/// The resume regression the brief asks for. A two-apex world is saved mid-flight, rebuilt
/// from its own bytes, and run on; the rule is **never named** on either side. The resumed
/// hash must equal the uninterrupted one, and both sides must be running the envelope — the
/// pair of claims that says a schema-17 resume is the same world, not a quietly re-ruled one.
#[test]
fn a_saved_and_resumed_world_runs_the_envelope_on_both_sides_and_reproduces_the_hash() {
    // The two adults' first paid attempt in this fixture falls at tick 5,276, so the save has
    // to sit past it or neither side of the resume would have reached the predicate at all.
    const BEFORE: u64 = 5_500;
    const AFTER: u64 = 2_500;

    let mut uninterrupted = two_apex_world(None);
    assert_eq!(uninterrupted.pursuit_stop(), PursuitStop::ReachEnvelope);

    // One world up to the save: the arm that is never interrupted *is* the arm that is saved,
    // so "the same world up to the save" is an identity rather than a second assertion.
    let (_, before_attempts) = marks(&mut uninterrupted, BEFORE);
    assert!(before_attempts > 0, "the save point is past the first paid attempt");

    // Save, and rebuild from the bytes exactly as the host's resume path does.
    let bytes = encode_snapshot(&uninterrupted.state, "adoption-resume");
    let (meta, state) = decode_snapshot(&bytes).expect("a schema 17 snapshot round-trips");
    assert_eq!(meta.schema, SCHEMA_VERSION);
    let mut resumed = World::from_state(state).expect("the state rebuilds");
    assert_eq!(
        resumed.pursuit_stop(),
        PursuitStop::ReachEnvelope,
        "a resumed world runs the shipped rule; the bytes never carried one"
    );
    // The recorder is a transient too, so recording is asked for again. It changes no value
    // the tick reads back, which is what the matching hashes below also demonstrate.
    resumed.record_strike_attempts(true);

    let (uninterrupted_marks, straight_through) = marks(&mut uninterrupted, AFTER);
    let (resumed_marks, after_resume) = marks(&mut resumed, AFTER);
    assert_eq!(
        state_hash(&uninterrupted.state),
        state_hash(&resumed.state),
        "the resumed world diverged from the uninterrupted one over {AFTER} ticks"
    );
    assert_eq!(uninterrupted_marks, resumed_marks, "and at every boundary in between");
    assert!(
        after_resume > 0,
        "the resumed world reached no paid attempt in {AFTER} ticks, so the resume says \
         nothing about which rule it came back under"
    );
    assert_eq!(
        after_resume, straight_through,
        "the same attempts, on both sides of the save"
    );
}

// ---------------------------------------------------------------- the flip, both ways

/// The retained rows stay reproducible: a world told the rule before 2026-09-16 reproduces
/// the hashes `960b6c2` printed under its own default.
#[test]
fn a_world_told_the_half_space_reproduces_the_rule_before_this_brief() {
    let mut world = two_apex_world(Some(PursuitStop::ForwardHalfSpace));
    let (got, attempts) = marks(&mut world, 9_000);
    assert!(attempts > 0, "no paid attempt was made, so the run is not evidence");
    assert_eq!(
        got, HALF_SPACE_9K,
        "the opt-in half-space no longer reproduces the rule the retained rows ran under"
    );
}

/// And the flip moved the default and nothing else: an **untouched** world now reproduces the
/// hashes `960b6c2` printed for its opt-in variant, and naming the envelope changes nothing.
#[test]
fn the_new_default_reproduces_the_envelope_arm_measured_before_the_flip() {
    let mut untouched = two_apex_world(None);
    let (got, attempts) = marks(&mut untouched, 9_000);
    assert!(attempts > 0, "no paid attempt was made, so the run is not evidence");
    assert_eq!(
        got, ENVELOPE_9K,
        "an untouched world does not reproduce the envelope arm the paired note measured"
    );

    let mut named = two_apex_world(Some(PursuitStop::ReachEnvelope));
    let (named_marks, _) = marks(&mut named, 9_000);
    assert_eq!(named_marks, got, "naming the shipped rule changes nothing");

    // The pair is a measurement of the predicate: identical until the first attempt whose
    // rule differs, different after it.
    assert_eq!(
        HALF_SPACE_9K[..FIRST_DIVERGENT_MARK],
        ENVELOPE_9K[..FIRST_DIVERGENT_MARK],
        "the two arms must share the ticks before the first attempt the rules disagree about"
    );
    assert_ne!(
        HALF_SPACE_9K[FIRST_DIVERGENT_MARK..],
        ENVELOPE_9K[FIRST_DIVERGENT_MARK..],
        "the two rules must part, or the pin proves nothing"
    );
}
