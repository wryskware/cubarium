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
use cubarium_core::snapshot::{state_hash, v7};
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
#[allow(dead_code)]
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

// ---------------------------------------- retired migrations, kept as refusals (§15.1)

/// **Retired by ecology v1** (`design/ecology-v1-contract.md` §15.1). Three tests lived here:
/// the live schema 8 snapshot loading with zero corrections and projecting back byte for
/// byte, its 600-tick cross-version continuation, and the same opening claim for a migrated
/// schema 7 world.
///
/// Worlds always restart fresh and are never migrated (Wrysk, 2026-09-15), so schema 16
/// refuses schemas 7 and 8 by name. The artifacts cannot be loaded and this build cannot make
/// a schema 7 or 8 world to re-anchor against, so the continuations are retired rather than
/// re-recorded. The fixtures stay in the tree; the refusal is what is checked on their bytes.
///
/// The compensation claim itself — that the corrections begin at zero on a fresh world, track
/// the real flows, survive a restart and are refused when unusable — is unchanged and is
/// tested below on worlds this build creates.
#[test]
fn the_pre_correction_fixtures_are_refused_by_name() {
    for name in [
        "live-v7-55200.cubw",
        "live-v8-172800.cubw",
        "live-v8-172800-plus600.cubw",
        "live-v8-172800-plus600-r0b.cubw",
    ] {
        let bytes = std::fs::read(fixture(name)).expect("the fixture is committed");
        let schema = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        assert!(schema < SCHEMA_VERSION, "{name} is schema {schema}");
        assert_eq!(
            decode_snapshot(&bytes),
            Err(cubarium_core::SnapshotError::UnsupportedSchema(schema)),
            "{name}: an old world is refused by name, never migrated"
        );
    }
    assert_eq!(SCHEMA_V8, 8);
}

/// A fresh world opens with zero corrections and is compensated from there, and its schema 7
/// projection is what `ecology_hash` hashes. This is the part of the retired pair that does
/// not need an old artifact.
#[test]
fn a_fresh_world_opens_at_zero_and_is_compensated_from_there() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    assert_eq!(world.state.care, CareState::default());
    assert_eq!(world.state.energy_correction, EnergyCorrection::default());
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
