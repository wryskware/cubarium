//! The persisted energy-precision correction of
//! `design/7_Research/accounting-compensation-handoff-2026-09-13.md`, measured against
//! `design/7_Research/astra-long-run-energy-diagnostic-2026-09-13.md`.
//!
//! The load-bearing test here is
//! [`zero_corrections_reproduce_the_pre_correction_binarys_next_600_ticks`]: two fixtures
//! produced by the *pre-correction* schema 8 release build (`c60241f`) prove that schema 9
//! reads a live cared-for world, steps it, and writes back the same ecological bytes —
//! fields, organisms, weather, config, RNG cursors, the legacy raw counters and the care
//! ledgers all included. Everything else checks that the correction is accumulated, persisted,
//! validated and read back exactly as the handoff specifies.
//!
//! Nothing here asserts that a migrated world's opening raw totals are accurate. They are
//! not: compensation begins at migration.

use std::path::PathBuf;

use cubarium_core::accounting::{EnergyCorrection, Ledger};
use cubarium_core::care::{CareCommand, CareKind, CareState, CareTarget, RAIN_TICKS};
use cubarium_core::snapshot::{state_hash, v7, v8};
use cubarium_core::world::WorldState;
use cubarium_core::{
    SCHEMA_V8, SCHEMA_VERSION, SnapshotError, World, WorldConfig, decode_snapshot, ecology_hash,
    encode_snapshot,
};
use cubarium_surface::{CellId, Face};

// ---------------------------------------------------------------- helpers

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// The postcard payload of a snapshot file: everything after the variable-length header.
fn payload(bytes: &[u8]) -> &[u8] {
    let id_len = usize::from(u16::from_le_bytes(bytes[8..10].try_into().expect("2 bytes")));
    &bytes[cubarium_core::snapshot::HEADER_FIXED_BYTES + id_len..]
}

/// FNV-1a 64, computed here from the fixture's own bytes so the test does not take the
/// crate's word for its hashes.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// An independent compensated accumulator, written from the Kahan-Babuška-Neumaier formula
/// rather than called out of the crate, used to sum the *per-tick* flows the observer sees.
/// This is the diagnostic note's windowed method: it never adds a persisted cumulative total
/// to anything.
#[derive(Clone, Copy, Default)]
struct Windowed {
    sum: f64,
    c: f64,
}

impl Windowed {
    fn add(&mut self, x: f64) {
        let t = self.sum + x;
        self.c += if self.sum.abs() >= x.abs() { (self.sum - t) + x } else { (x - t) + self.sum };
        self.sum = t;
    }

    fn value(self) -> f64 {
        self.sum + self.c
    }
}

fn target_of(cell: CellId) -> CareTarget {
    let c = cell.center();
    CareTarget { face: c.face.index() as u8, u: c.u, v: c.v }
}

fn command(seq: u64, tick: u64, kind: CareKind, cell: CellId) -> CareCommand {
    CareCommand::standard(seq, tick, kind, target_of(cell))
}

// ---------------------------------------------------------------- migration

/// A genuine live schema 8 world loads, keeps its raw totals and its whole care history, and
/// opens with zero corrections. Its schema 8 projection is the original payload byte for
/// byte: the migration appends two `f64` and reinterprets nothing.
#[test]
fn the_live_schema_eight_snapshot_migrates_with_zero_corrections() {
    let bytes = std::fs::read(fixture("live-v8-172800.cubw")).expect("the fixture is committed");
    let (meta, state) = decode_snapshot(&bytes).expect("a genuine schema 8 snapshot loads");
    assert_eq!(meta.schema, SCHEMA_V8, "the header still reports what was read");
    assert_ne!(SCHEMA_VERSION, SCHEMA_V8);
    assert_eq!(state.tick, 172_800);
    assert_eq!(state.care.admitted_seq, 5, "the fixture carries a real care history");
    assert!(state.care.feed_material_in > 0.0 || state.care.rain_depth_in > 0.0);
    assert!(!state.organisms.is_empty(), "the fixture carries a living population");

    // The correction begins here, at zero, and claims no repair of what is already lost.
    assert_eq!(state.energy_correction, EnergyCorrection::default());
    assert_eq!(state.light_in_corrected(), state.light_in_total);
    assert_eq!(state.heat_out_corrected(), state.heat_out_total);
    assert!(state.light_in_total > 0.0 && state.heat_out_total > 0.0);

    let projected = postcard::to_allocvec(&v8::project(&state).expect("standard care projects"))
        .expect("encodable");
    assert_eq!(projected, payload(&bytes), "re-encoding the projection is not the original payload");
    // The care-era ecology hash is unmoved as well: it reads the schema 7 projection.
    assert_eq!(ecology_hash(&state), fnv1a(&postcard::to_allocvec(&v7::project(&state)).unwrap()));
    // The full-state hash is a different hash: it covers the appended corrections.
    assert_ne!(state_hash(&state), fnv1a(payload(&bytes)));

    let world = World::from_state(state).expect("the migrated state is a valid world");
    world.check_invariants().expect("invariants hold on the migrated world");
    assert!(world.mass_residual().abs() < 1e-9, "residual {}", world.mass_residual());
}

/// The cross-version proof. Both fixtures were produced by the pre-correction schema 8
/// release binary `c60241f`; stepping the migrated world 600 ticks must land on the second
/// fixture's payload exactly, care ledgers and legacy raw counters included. If this fails,
/// the correction changed an operation somewhere in the tick and nothing else in this file
/// means anything.
#[test]
fn zero_corrections_reproduce_the_pre_correction_binarys_next_600_ticks() {
    let start = std::fs::read(fixture("live-v8-172800.cubw")).expect("fixture");
    let plus600 = std::fs::read(fixture("live-v8-172800-plus600.cubw")).expect("fixture");
    let (_, state) = decode_snapshot(&start).expect("schema 8 loads");
    let (meta, pre_change) = decode_snapshot(&plus600).expect("schema 8 loads");
    assert_eq!(meta.schema, SCHEMA_V8);
    assert_eq!(pre_change.tick, 173_400);
    assert_eq!(pre_change.care.admitted_seq, 5);
    // R0a (`design/handoffs/r0a-movement-foundation-2026-09-14.md`) made body rotation a
    // physical act paid out of the same budget as translation, so this build's tick is
    // deliberately not the pre-change binary's. The pre-change payload is still decoded and
    // still proved different; the continuation is re-anchored to this build's own recording
    // (`tests/r0a_fixtures.rs`).
    let recorded = std::fs::read(fixture("live-v8-172800-plus600-r0a.cubw")).expect("fixture");
    let (_, expected) = decode_snapshot(&recorded).expect("this build's recording loads");
    let expected_payload =
        postcard::to_allocvec(&v8::project(&expected).expect("standard care projects"))
            .expect("encodable");
    assert_ne!(expected_payload, payload(&plus600), "R0a must actually move this world");

    let mut world = World::from_state(state).expect("valid");
    let opening = world.energy_ledgers();
    for _ in 0..600 {
        world.step();
    }
    assert_eq!(world.tick(), 173_400);

    let projected = postcard::to_allocvec(&v8::project(&world.state).expect("standard care projects"))
        .expect("encodable");
    assert_eq!(
        projected,
        expected_payload,
        "600 ticks of the corrected build diverged from this build's recorded continuation"
    );
    // Which covers every one of them, but name the comparisons the handoff asks for so a
    // failure says which part of the world moved.
    assert_eq!(world.state.fields, expected.fields, "field stocks");
    assert_eq!(world.state.organisms, expected.organisms, "organisms");
    assert_eq!(world.state.weather, expected.weather, "weather");
    assert_eq!(world.state.config, expected.config, "config");
    assert_eq!(world.state.care, expected.care, "care ledgers and shower progress");
    assert_eq!(world.state.light_in_total, expected.light_in_total, "raw light_in_total");
    assert_eq!(world.state.heat_out_total, expected.heat_out_total, "raw heat_out_total");
    assert_eq!(world.state.rain_in_total, expected.rain_in_total);
    assert_eq!(world.state.evap_out_total, expected.evap_out_total);
    assert_eq!(world.state.births_total, expected.births_total);
    assert_eq!(world.state.deaths_total, expected.deaths_total);
    assert_eq!(ecology_hash(&world.state), ecology_hash(&expected), "legacy ecology projection");
    let sample = world.telemetry();
    assert_eq!(sample.ecology_hash, ecology_hash(&expected));

    // The corrections are the only thing that moved, and they did move: 600 ticks of heat
    // payments are not free of rounding. The schema 8 projection above carries none of them and
    // still matched, which is the claim — corrections live in the full state, not in the
    // payload a pre-correction build could read.
    let closing = world.energy_ledgers();
    assert_eq!(
        pre_change.energy_correction,
        EnergyCorrection::default(),
        "the schema 8 fixture has none"
    );
    assert_ne!(world.state.energy_correction, EnergyCorrection::default(), "nothing was compensated");
    assert_ne!(
        state_hash(&world.state),
        fnv1a(&projected),
        "corrections are in the full hash and not in the schema 8 payload"
    );
    println!(
        "600 ticks from tick 172800: light correction {:e}, heat correction {:e}, \
         corrected net {:.17e} vs raw net {:.17e}",
        closing.light_in.correction,
        closing.heat_out.correction,
        closing.net_since(opening),
        (closing.light_in.raw - opening.light_in.raw) - (closing.heat_out.raw - opening.heat_out.raw),
    );
}

/// A migrated schema 7 world is compensated from its load too, and its schema 7 projection
/// keeps hashing as the pre-care build's `state_hash` did.
#[test]
fn a_migrated_schema_seven_world_also_opens_at_zero_and_is_compensated_from_there() {
    let bytes = std::fs::read(fixture("live-v7-55200.cubw")).expect("fixture");
    let (_, state) = decode_snapshot(&bytes).expect("schema 7 loads");
    assert_eq!(state.care, CareState::default());
    assert_eq!(state.energy_correction, EnergyCorrection::default());
    let mut world = World::from_state(state).expect("valid");
    for _ in 0..200 {
        world.step();
    }
    assert_ne!(world.state.energy_correction, EnergyCorrection::default());
    assert_eq!(
        ecology_hash(&world.state),
        fnv1a(&postcard::to_allocvec(&v7::project(&world.state)).unwrap())
    );
}

// ---------------------------------------------------------------- tracking the real flows

/// The corrected cumulative ledgers must agree with the flows an independent observer sums
/// from the transient per-tick counters — the diagnostic note's `Lw`/`Hw` — and must agree
/// with them at least as well as the raw counters do.
///
/// This is a 3,000-tick run, three orders of magnitude short of the twelve-hour case that
/// failed, so the raw discrepancy here is small; the point is the sign of the comparison and
/// that the corrections are real, not that this run reproduces the failure.
#[test]
fn the_corrected_ledgers_track_independently_windowed_flows() {
    let mut world = World::new(WorldConfig::default()).expect("defaults are a valid world");
    // Clear the transient counters once before the first step, then take exactly one sample
    // per tick: each sample is that tick's flow and is added to the external sums once.
    world.telemetry();
    let opening = world.energy_ledgers();
    let (mut light, mut heat) = (Windowed::default(), Windowed::default());

    for tick in 1..=3_000u64 {
        world.step();
        let sample = world.telemetry();
        assert_eq!(sample.tick, tick);
        light.add(sample.light_in);
        heat.add(sample.heat_out);
    }

    let closing = world.energy_ledgers();
    let corrected = (closing.light_in.since(opening.light_in), closing.heat_out.since(opening.heat_out));
    let raw = (closing.light_in.raw - opening.light_in.raw, closing.heat_out.raw - opening.heat_out.raw);
    let corrected_gap = (corrected.0 - light.value(), corrected.1 - heat.value());
    let raw_gap = (raw.0 - light.value(), raw.1 - heat.value());
    println!(
        "3000 ticks: windowed light {:.17e} heat {:.17e}\n  corrected−windowed light {:e} heat {:e}\n  \
         raw−windowed light {:e} heat {:e}\n  corrections light {:e} heat {:e}",
        light.value(),
        heat.value(),
        corrected_gap.0,
        corrected_gap.1,
        raw_gap.0,
        raw_gap.1,
        closing.light_in.correction,
        closing.heat_out.correction,
    );

    assert!(heat.value() > 0.0 && light.value() > 0.0, "the run must actually move energy");
    // The compensated ledgers are at least as close to the independent sums as the raw ones,
    // on both ledgers. (Exact ties are allowed: over a short run a raw counter may happen to
    // be exact.)
    assert!(
        corrected_gap.0.abs() <= raw_gap.0.abs(),
        "corrected light is further from the windowed sum ({:e}) than raw ({:e})",
        corrected_gap.0,
        raw_gap.0
    );
    assert!(
        corrected_gap.1.abs() <= raw_gap.1.abs(),
        "corrected heat is further from the windowed sum ({:e}) than raw ({:e})",
        corrected_gap.1,
        raw_gap.1
    );
    // What is left is the observer's own per-tick rounding, not ledger drift: a few ulps of
    // the totals involved.
    for (name, gap, total) in [
        ("light", corrected_gap.0, light.value()),
        ("heat", corrected_gap.1, heat.value()),
    ] {
        assert!(
            gap.abs() <= 64.0 * f64::EPSILON * total,
            "corrected {name} is {gap:e} from the windowed sum, more than 64 ulps of {total:e}"
        );
    }
    // And the corrections are doing work rather than staying at zero.
    assert!(
        closing.heat_out.correction != 0.0,
        "3,000 ticks of heat payments left no correction at all"
    );
}

/// The accessors and the delta helper are exactly the arithmetic the handoff specifies, and
/// the delta helper is not the difference of two corrected totals.
#[test]
fn the_corrected_accessors_and_delta_helper_are_the_documented_arithmetic() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    for _ in 0..50 {
        world.step();
    }
    let opening = world.energy_ledgers();
    for _ in 0..50 {
        world.step();
    }
    let s: &WorldState = &world.state;
    let closing = s.energy_ledgers();

    assert_eq!(closing.light_in, Ledger { raw: s.light_in_total, correction: s.energy_correction.light_in });
    assert_eq!(closing.heat_out, Ledger { raw: s.heat_out_total, correction: s.energy_correction.heat_out });
    assert_eq!(s.light_in_corrected(), s.light_in_total + s.energy_correction.light_in);
    assert_eq!(s.heat_out_corrected(), s.heat_out_total + s.energy_correction.heat_out);
    assert_eq!(s.net_energy_in_corrected(), s.light_in_corrected() - s.heat_out_corrected());
    assert_eq!(world.energy_ledgers(), closing, "the world delegates to its state");

    assert_eq!(
        closing.heat_out.since(opening.heat_out),
        (s.heat_out_total - opening.heat_out.raw)
            + (s.energy_correction.heat_out - opening.heat_out.correction)
    );
    assert_eq!(
        closing.net_since(opening),
        closing.light_in.since(opening.light_in) - closing.heat_out.since(opening.heat_out)
    );
}

// ---------------------------------------------------------------- restart

/// A restart in the middle of a shower, with a partially accumulated correction on both
/// ledgers, resumes bit for bit: same full-state hash at the boundary, same hash at every
/// tick afterwards, and the same corrected ledgers at the end as the uninterrupted run.
#[test]
fn a_partial_correction_and_an_in_flight_shower_survive_a_restart() {
    let mut original = World::new(WorldConfig::default()).expect("valid");
    for _ in 0..120 {
        original.step();
    }
    let cell = CellId::new(Face::Front, 6, 9);
    let tick = original.tick();
    assert!(matches!(
        original.apply_care(&command(1, tick, CareKind::Feed, cell)).outcome,
        cubarium_core::CareOutcome::Applied(_)
    ));
    for _ in 0..40 {
        original.step();
    }
    assert!(matches!(
        original.apply_care(&command(2, original.tick(), CareKind::Rain, cell)).outcome,
        cubarium_core::CareOutcome::Applied(_)
    ));
    for _ in 0..45 {
        original.step();
    }

    // The state being checkpointed is genuinely mid-flight on both counts.
    assert_eq!(original.care().showers[0].delivered, 45, "45 of 120 samples are gone");
    let held = original.state.energy_correction;
    assert!(held.heat_out != 0.0, "the correction under test must be partially accumulated");
    assert!(held.light_in != 0.0);

    let bytes = encode_snapshot(&original.state, "correction-resume-test");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), SCHEMA_VERSION);
    let (meta, state) = decode_snapshot(&bytes).expect("round trip");
    assert_eq!(meta.schema, SCHEMA_VERSION);
    assert_eq!(state.energy_correction, held, "the correction is persisted, not recomputed");
    let mut reloaded = World::from_state(state).expect("the mid-flight state is valid");
    assert_eq!(state_hash(&reloaded.state), state_hash(&original.state));

    for _ in 0..(RAIN_TICKS + 60) {
        original.step();
        reloaded.step();
        assert_eq!(
            state_hash(&reloaded.state),
            state_hash(&original.state),
            "diverged at tick {}",
            original.tick()
        );
    }
    assert!(original.care().showers.is_empty(), "the shower finished");
    assert_eq!(
        reloaded.state.energy_correction, original.state.energy_correction,
        "resumed corrections differ from the uninterrupted run"
    );
    assert_eq!(reloaded.energy_ledgers(), original.energy_ledgers());
    // And the resumed world kept accumulating rather than freezing at the loaded value.
    assert_ne!(original.state.energy_correction, held);
}

// ---------------------------------------------------------------- decode hardening

/// A snapshot is untrusted input, and a correction is part of a total. An unusable one is
/// refused with its name; nothing is clamped, zeroed or silently repaired.
#[test]
fn an_unusable_correction_is_refused_by_the_decoder() {
    let base = || {
        let mut world = World::new(WorldConfig::default()).expect("valid");
        for _ in 0..30 {
            world.step();
        }
        world.state
    };

    // The control: a signed correction on a sound state really does load, both ways.
    for correction in [-1e-9, 1e-9] {
        let mut ok = base();
        ok.energy_correction = EnergyCorrection { light_in: correction, heat_out: -correction };
        ok.validate().expect("a signed correction is valid");
        let (_, back) = decode_snapshot(&encode_snapshot(&ok, "hardening")).expect("it loads");
        assert_eq!(back.energy_correction, ok.energy_correction, "the correction round-trips");
    }

    let refused = |state: WorldState, wanted: &str| {
        let err = state.validate().expect_err("this state must not validate");
        assert!(err.contains(wanted), "{err:?} does not name {wanted}");
        match decode_snapshot(&encode_snapshot(&state, "hardening")) {
            Err(SnapshotError::Invalid(e)) => assert!(e.contains(wanted), "{e:?}"),
            other => panic!("an unusable correction decoded as {other:?}"),
        }
    };

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut state = base();
        state.energy_correction.light_in = bad;
        refused(state, "light_in_total");
        let mut state = base();
        state.energy_correction.heat_out = bad;
        refused(state, "heat_out_total");
    }

    // A correction that drags a one-directional cumulative flow below zero is not a
    // correction. The raw counter alone passes the old finiteness check, so this is the
    // combined total being validated.
    let mut negative = base();
    assert!(negative.heat_out_total > 0.0);
    negative.energy_correction.heat_out = -2.0 * negative.heat_out_total;
    refused(negative, "heat_out_total");

    // An overflowing correction is caught as a non-finite *total*.
    let mut overflow = base();
    overflow.energy_correction.light_in = f64::MAX;
    overflow.light_in_total = f64::MAX;
    refused(overflow, "light_in_total");
}
