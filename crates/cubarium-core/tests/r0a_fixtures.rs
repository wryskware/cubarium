//! Regenerates the `*-plus600-r0a.cubw` continuation fixtures.
//!
//! Milestone R0a (`design/handoffs/r0a-movement-foundation-2026-09-14.md`) made body rotation
//! a physical, paid act sharing one budget with translation, so every trajectory that contains
//! a body wider than `motor::REFERENCE_RADIUS_PX` — and every apex member — now runs
//! differently. The `*-plus600.cubw` fixtures were written by release binaries that predate
//! that change, and this build cannot reproduce them; the claim they proved is false now, by
//! design, not by accident.
//!
//! Those original fixtures are **kept**: they are genuine artefacts of the builds named in
//! `tests/fixtures/*-provenance.md`, and every test that used them still asserts their own
//! payload hashes and still migrates them. What is re-anchored is the *continuation*: each
//! start fixture is stepped 600 ticks by this build and the result saved beside it, so the
//! same tests keep guarding the same thing — that nothing moves the tick unintentionally —
//! against an oracle this build can actually reach.
//!
//! Run deliberately, never in CI:
//!
//! ```text
//! cargo test -p cubarium-core --test r0a_fixtures -- --ignored --nocapture
//! ```
//!
//! Regenerating is a decision, not a repair. If one of these files needs rewriting, the tick
//! changed; say why in the commit before running this.

use std::path::PathBuf;

use cubarium_core::{World, decode_snapshot, encode_snapshot};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// `(start, end, ticks the end must carry)`. Every pair is 600 ticks apart, matching the
/// continuation each test performs.
const PAIRS: &[(&str, &str, u64)] = &[
    ("live-v7-55200.cubw", "live-v7-55200-plus600-r0a.cubw", 55_800),
    ("live-v8-172800.cubw", "live-v8-172800-plus600-r0a.cubw", 173_400),
    ("pre-hunter-v9-173400.cubw", "pre-hunter-v9-173400-plus600-r0a.cubw", 174_000),
    ("care-v11-shower-360.cubw", "care-v11-shower-360-plus600-r0a.cubw", 960),
    ("care-v11-hunters-200.cubw", "care-v11-hunters-200-plus600-r0a.cubw", 800),
    ("hunter-v3-charge-active.cubw", "hunter-v3-charge-active-plus600-r0a.cubw", 6_170),
    ("quiet-v12-plain-3000.cubw", "quiet-v12-plain-3000-plus600-r0a.cubw", 147_600),
    ("quiet-v12-care-3000.cubw", "quiet-v12-care-3000-plus600-r0a.cubw", 147_600),
];

/// The build id every regenerated fixture carries, so a reader can tell at a glance that it is
/// this milestone's recording and not a release binary's.
const BUILD_ID: &str = "r0a-motor-foundation";

fn continue_600(start: &str) -> cubarium_core::WorldState {
    let bytes = std::fs::read(fixture(start)).expect("the start fixture");
    let (_, state) = decode_snapshot(&bytes).expect("the start fixture decodes");
    let mut world = World::from_state(state).expect("the migrated state is a valid world");
    world.check_invariants().expect("invariants hold on the migrated world");
    for _ in 0..600 {
        world.step();
        world.drain_events();
        world.drain_hunter_events();
        world.drain_quiet_events();
    }
    world.check_invariants().expect("invariants hold after the continuation");
    world.state
}

#[test]
#[ignore = "rewrites checked-in fixtures; run only when the tick is meant to have changed"]
fn regenerate() {
    for (start, end, tick) in PAIRS {
        let state = continue_600(start);
        assert_eq!(state.tick, *tick, "{start}: unexpected end tick");
        let bytes = encode_snapshot(&state, BUILD_ID);
        std::fs::write(fixture(end), &bytes).expect("write the continuation fixture");
        println!("{end}: {} bytes at tick {}", bytes.len(), state.tick);
    }
}

/// The cheap guard the ordinary suite does run: each regenerated fixture is still exactly what
/// this build produces from its start fixture. It is the same claim the individual migration
/// tests make, stated once over the whole set, so a fixture that drifts out of date fails here
/// even if its own test was skipped.
#[test]
fn every_regenerated_fixture_is_this_builds_own_continuation() {
    for (start, end, tick) in PAIRS {
        let state = continue_600(start);
        assert_eq!(state.tick, *tick, "{start}: unexpected end tick");
        let recorded = std::fs::read(fixture(end)).expect("the continuation fixture");
        let (meta, expected) = decode_snapshot(&recorded).expect("it decodes");
        assert_eq!(meta.build_id, BUILD_ID, "{end}: not a milestone recording");
        assert_eq!(state, expected, "{end} is not what this build produces from {start}");
    }
}
