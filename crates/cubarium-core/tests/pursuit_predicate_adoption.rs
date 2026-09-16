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
//! - **the schema refuses 16 by number**, because a world's saved bytes do not carry the
//!   rule: resuming schema-16 bytes under this build would change behaviour silently, which is
//!   a migration by another name (Wrysk's always-fresh rule of 2026-09-15). The adoption made
//!   that schema 17; the ring world has since taken the current one to 18, and 17 is refused
//!   beside 16 for the shape reason rather than the rule one;
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
use cubarium_core::snapshot::{CubeProjection, SCHEMA_VERSION, projection_hash, state_hash};
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

/// Step `ticks`, draining everything every tick, and return the state hash **and the
/// projection hash** at every 1,500-tick boundary together with how many paid attempts the arm
/// actually made. A run that reached no attempt is not evidence about a pursuit rule, so the
/// count is returned rather than assumed.
///
/// The second vector is what makes the re-recorded first one honest: see `HALF_SPACE_9K`.
fn marks(world: &mut World, ticks: u64) -> (Vec<u64>, Vec<u64>, usize) {
    let mut out = Vec::new();
    let mut projections = Vec::new();
    let mut attempts = 0usize;
    for tick in 1..=ticks {
        world.step();
        world.drain_events();
        world.drain_hunter_events();
        attempts += world.drain_strike_records().0.len();
        if tick.is_multiple_of(1_500) {
            out.push(state_hash(&world.state));
            projections.push(projection_hash(&CubeProjection::from(&world.state)));
        }
    }
    (out, projections, attempts)
}

/// The half-space arm's state hashes, **re-recorded by SYNC-1 (2026-09-16)**.
///
/// They were printed by `960b6c2` from `two_apex_world(Some(ForwardHalfSpace))` — and, at that
/// commit, from an untouched world too, because the half-space was the default there. They had
/// to move on the merge, and not because any world did: `state_hash` covers `WorldConfig`,
/// which the ring world appends `topology` and `world_scale` to and takes from version 8 to 9
/// (`design/flat-world-plan-2026-09-16.md` §4,
/// `design/7_Research/flat-world-sync-main-2026-09-16.md`).
///
/// [`HALF_SPACE_9K_PROJECTION`] is the evidence that nothing else moved: the same states with
/// exactly those three config fields removed, **printed by an unmodified `main` build at
/// `15a2210` before the merge** and unchanged here. The arm below is re-recorded; the arm
/// beside it is not.
const HALF_SPACE_9K: [u64; 6] = [
    0x239d68ba1ce45828,
    0xb7a3b79d119de6b3,
    0x35503a3b6894f34e,
    0xf46a48a37b85a0db,
    0x657a562fa16eb772,
    0xa689f67f0f9a7b52,
];

/// `projection_hash` at the same six boundaries, from `main` at `15a2210`. Not re-recorded.
const HALF_SPACE_9K_PROJECTION: [u64; 6] = [
    11_129_109_387_498_121_176,
    13_019_671_601_490_239_843,
    4_089_255_216_940_132_638,
    15_047_421_345_914_814_923,
    458_113_154_018_978_050,
    15_240_484_321_456_572_066,
];

/// Printed by `960b6c2` from `two_apex_world(Some(ReachEnvelope))`, when naming the envelope
/// was an opt-in variant. It is the default now, so an untouched world must reproduce it.
/// Re-recorded by SYNC-1 for the same one reason as [`HALF_SPACE_9K`], with the same evidence.
const ENVELOPE_9K: [u64; 6] = [
    0x239d68ba1ce45828,
    0xb7a3b79d119de6b3,
    0x35503a3b6894f34e,
    0x7714d4b0ed2fdecd,
    0x7ecf395076d4d516,
    0x3f8ddf01f9e57099,
];

/// `projection_hash` at the same six boundaries, from `main` at `15a2210`. Not re-recorded.
const ENVELOPE_9K_PROJECTION: [u64; 6] = [
    11_129_109_387_498_121_176,
    13_019_671_601_490_239_843,
    4_089_255_216_940_132_638,
    13_634_415_709_825_699_197,
    7_492_135_629_701_802_822,
    10_441_275_121_617_245_321,
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

// ---------------------------------------------------------------- the schema break

/// Schema 17 was a **semantics-only** bump: the payload shape did not move, so nothing but the
/// header can tell a schema-16 world from a schema-17 one. A schema-16 header over a payload
/// that is in every other way valid is refused by number, which is what stops a world saved
/// under the half-space from being silently resumed under the envelope. The current schema is
/// 18 — the ring world moved the shape again — and the refusal is the one this test fixes: it
/// is stated against `SCHEMA_V16` and `SCHEMA_VERSION`, not against the number 17, so it keeps
/// meaning the same thing as the shape moves on.
#[test]
fn a_schema_sixteen_header_over_a_valid_payload_is_refused_by_number() {
    assert_eq!(SCHEMA_VERSION, 18, "the ring world is schema 18; the predicate adoption was 17");

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
    let (meta, _) = decode_snapshot(&bytes).expect("the current schema decodes");
    assert_eq!(meta.schema, SCHEMA_VERSION);

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
/// pair of claims that says a resume is the same world, not a quietly re-ruled one.
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
    let (_, _, before_attempts) = marks(&mut uninterrupted, BEFORE);
    assert!(before_attempts > 0, "the save point is past the first paid attempt");

    // Save, and rebuild from the bytes exactly as the host's resume path does.
    let bytes = encode_snapshot(&uninterrupted.state, "adoption-resume");
    let (meta, state) = decode_snapshot(&bytes).expect("a current-schema snapshot round-trips");
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

    let (uninterrupted_marks, _, straight_through) = marks(&mut uninterrupted, AFTER);
    let (resumed_marks, _, after_resume) = marks(&mut resumed, AFTER);
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
    let (got, projections, attempts) = marks(&mut world, 9_000);
    assert!(attempts > 0, "no paid attempt was made, so the run is not evidence");
    assert_eq!(
        projections, HALF_SPACE_9K_PROJECTION,
        "the opt-in half-space no longer reproduces the *world* the retained rows ran in — \
         this one is not a re-recording, it is what `main` printed"
    );
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
    let (got, projections, attempts) = marks(&mut untouched, 9_000);
    assert!(attempts > 0, "no paid attempt was made, so the run is not evidence");
    assert_eq!(
        projections, ENVELOPE_9K_PROJECTION,
        "an untouched world does not reproduce the *world* the paired note measured — this \
         one is not a re-recording, it is what `main` printed"
    );
    assert_eq!(
        got, ENVELOPE_9K,
        "an untouched world does not reproduce the envelope arm the paired note measured"
    );

    let mut named = two_apex_world(Some(PursuitStop::ReachEnvelope));
    let (named_marks, _, _) = marks(&mut named, 9_000);
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
