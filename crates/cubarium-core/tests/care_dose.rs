//! The adjustable care dose in the core: what it scales, what it refuses, and what it must
//! leave exactly alone (`design/7_Research/adjustable-care-dose-handoff-2026-09-13.md`).
//!
//! The dose is a **bounded total multiplier** on the nominal amount one command moves. It is
//! not a per-cell multiplier, not a way to queue several commands as one, and not a licence to
//! spend more than the 30-unit net feed allowance or to take more than half of any one cell's
//! litter. A standard dose is the pre-dose arithmetic bit for bit; the byte-for-byte proof of
//! that against genuine pre-dose fixtures lives in `care_dose_migration.rs`.

use cubarium_core::care::{
    ActiveShower, CLEAN_FRACTION, CLEAN_MATERIAL, CareCommand, CareDose, CareKind, CareOutcome,
    CareTarget, FEED_ALLOWANCE, FEED_MATERIAL, RAIN_DEPTH_TOTAL, RAIN_TICKS, footprint,
};
use cubarium_core::snapshot::{SnapshotError, state_hash};
use cubarium_core::{World, WorldConfig, decode_snapshot, encode_snapshot};
use cubarium_surface::{CELL_COUNT, CellId, Face, FieldGraph};

// ---------------------------------------------------------------- helpers

fn target_of(cell: CellId) -> CareTarget {
    let c = cell.center();
    CareTarget { face: c.face.index() as u8, u: c.u, v: c.v }
}

fn dosed(seq: u64, tick: u64, kind: CareKind, cell: CellId, permille: u16) -> CareCommand {
    CareCommand {
        seq,
        apply_after_tick: tick,
        kind,
        target: target_of(cell),
        dose: CareDose::new(permille).expect("the test's dose is in range"),
    }
}

/// A dose the typed constructor would refuse, as it would actually arrive: decoded from the
/// wire. `CareDose` keeps its integer private precisely so nothing in-process can build one
/// out of range, so the only honest way to test the decode-side check is to decode one.
fn wire_dose(permille: u16) -> CareDose {
    postcard::from_bytes(&postcard::to_allocvec(&permille).expect("encodable"))
        .expect("a u16 is a CareDose on the wire")
}

/// A world with no natural rain and no flow or evaporation, so every drop in it came from a
/// shower and stays where it fell.
fn still_water_config() -> WorldConfig {
    let mut cfg = WorldConfig::default();
    cfg.water.rain_rate = 0.0;
    cfg.water.flow = 0.0;
    cfg.water.evap = 0.0;
    cfg
}

/// The documented interior / seam / open-rim trio, so every scaling claim is checked where the
/// footprint is full, where it crosses a face boundary, and where it is short a neighbour.
const PLACES: [(&str, Face, u8, u8); 3] =
    [("interior", Face::Front, 8, 8), ("seam", Face::Front, 15, 8), ("rim", Face::Front, 8, 15)];

fn places() -> Vec<(&'static str, CellId)> {
    PLACES.iter().map(|(n, f, u, v)| (*n, CellId::new(*f, *u, *v))).collect()
}

/// Every dose the contract names, plus both bounds and the standard one.
const DOSES: [u16; 5] = [250, 500, 1000, 1500, 2000];

// ---------------------------------------------------------------- the type itself

/// The bounds are 250..=2000 inclusive, the default is 1000, and an out-of-range value is
/// **refused**: a request for twice the maximum is a mistake to report, not a request for the
/// maximum. Clamping would silently serve an amount nobody asked for.
#[test]
fn the_dose_refuses_out_of_range_values_instead_of_clamping() {
    assert_eq!(CareDose::MIN_PERMILLE, 250);
    assert_eq!(CareDose::MAX_PERMILLE, 2000);
    assert_eq!(CareDose::STANDARD_PERMILLE, 1000);
    assert_eq!(CareDose::VERSION, 1);
    assert_eq!(CareDose::default(), CareDose::STANDARD);
    assert!(CareDose::STANDARD.is_standard());

    for ok in [250u16, 251, 500, 999, 1000, 1001, 1999, 2000] {
        assert_eq!(CareDose::new(ok).expect("in range").permille(), ok);
    }
    for bad in [0u16, 1, 249, 2001, 4000, 10_000, u16::MAX] {
        let err = CareDose::new(bad).expect_err("out of range");
        assert!(err.contains(&bad.to_string()), "{err}");
        assert!(err.contains("250") && err.contains("2000"), "the error names the bounds: {err}");
        // The refusal is not a clamp in disguise.
        assert!(!err.contains("clamp"));
    }
    // And the ordering is the integer's, so duplicate detection is exact.
    assert!(CareDose::new(250).unwrap() < CareDose::STANDARD);
    assert_eq!(CareDose::new(1000).unwrap(), CareDose::STANDARD);
}

/// A standard dose applies a documented identity branch, so it is the **same arithmetic, bit
/// for bit**, as the pre-dose build. `x * 1000.0 / 1000.0` is not universally `x`, and a world
/// that takes no nonstandard dose must step exactly as it did before this field existed.
#[test]
fn a_standard_dose_is_bit_for_bit_the_nominal_amount() {
    let awkward = [
        FEED_MATERIAL,
        RAIN_DEPTH_TOTAL,
        CLEAN_MATERIAL,
        0.1,
        1.0 / 3.0,
        f64::MIN_POSITIVE,
        1e-300,
        1e300,
        f64::MAX,
        std::f64::consts::PI,
    ];
    for x in awkward {
        assert_eq!(CareDose::STANDARD.scale(x), x, "the standard dose moved {x}");
        assert_eq!(CareDose::STANDARD.scale(x).to_bits(), x.to_bits());
    }
    // f64::MAX is the case that shows the branch is load-bearing rather than decorative: the
    // naive expression overflows to infinity where the identity does not.
    assert!((f64::MAX * 1000.0 / 1000.0).is_infinite());
    assert_eq!(CareDose::STANDARD.scale(f64::MAX), f64::MAX);

    // A nonstandard dose is the plain proportion.
    assert_eq!(CareDose::new(500).unwrap().scale(3.0), 1.5);
    assert_eq!(CareDose::new(2000).unwrap().scale(4.0), 8.0);
    assert_eq!(CareDose::new(250).unwrap().scale(2.0), 0.5);
}

/// An explicit standard dose and a command built by the standard constructor are the same
/// command: same effect, same ledgers, same state hash. "Omitted" and "explicit 1000" are one
/// semantic payload, so a viewer that starts sending the field changes nothing.
#[test]
fn an_explicit_standard_dose_is_the_same_command_as_no_dose_at_all() {
    let cell = CellId::new(Face::Front, 8, 8);
    let mut implicit = World::new(still_water_config()).expect("valid");
    let mut explicit = World::new(still_water_config()).expect("valid");
    for (seq, kind) in [(1, CareKind::Feed), (2, CareKind::Clean), (3, CareKind::Rain)] {
        let a = implicit.apply_care(&CareCommand::standard(seq, 0, kind, target_of(cell)));
        let b = explicit.apply_care(&dosed(seq, 0, kind, cell, 1000));
        assert_eq!(a.outcome, b.outcome, "{kind:?}");
    }
    for _ in 0..(RAIN_TICKS + 5) {
        implicit.step();
        explicit.step();
        assert_eq!(
            state_hash(&explicit.state),
            state_hash(&implicit.state),
            "diverged at tick {}",
            implicit.tick()
        );
    }
}

// ---------------------------------------------------------------- feed

/// Feed scales the nominal 3 material units and the chemical energy that rides with them, and
/// scales **nothing else**: the footprint, the per-cell weights and the energy density are the
/// same at every dose, at a seam and on the open rim as in the interior.
#[test]
fn a_feed_dose_scales_the_total_and_leaves_the_footprint_alone() {
    let graph = FieldGraph::new();
    for (name, cell) in places() {
        let fp = footprint(&graph, cell, 1);
        for permille in DOSES {
            let mut world = World::new(WorldConfig::default()).expect("valid");
            let rho = world.config().detritus.energy_cap;
            let m = f64::from(permille) / 1000.0 * FEED_MATERIAL;
            let before: Vec<f64> = world.state.fields.d.clone();
            let de_before: Vec<f64> = world.state.fields.de.clone();

            let receipt = world.apply_care(&dosed(1, 0, CareKind::Feed, cell, permille));
            let CareOutcome::Applied(q) = receipt.outcome.clone() else {
                panic!("{name} at {permille}: {:?}", receipt.outcome)
            };
            assert_eq!(q.cells as usize, fp.len(), "{name} at {permille}: the footprint moved");
            assert!((q.material_in - m).abs() < 1e-12, "{name} at {permille}: {}", q.material_in);
            assert!((q.energy_in - rho * m).abs() < 1e-12, "{name} at {permille}");
            assert_eq!((q.material_out, q.energy_out, q.water_depth), (0.0, 0.0, 0.0));

            // Per cell it is exactly `m · w_c`, with the same weights as any other dose, and
            // the energy is `rho · (m · w_c)` — the contract's grouping, not `(rho·m)·w`.
            let mut inside = vec![false; CELL_COUNT];
            for (c, w) in &fp {
                let i = c.index();
                inside[i] = true;
                assert_eq!(world.state.fields.d[i], before[i] + m * w, "{name} at {permille}");
                assert_eq!(world.state.fields.de[i], de_before[i] + rho * (m * w));
            }
            for i in 0..CELL_COUNT {
                assert!(inside[i] || world.state.fields.d[i] == before[i], "{name}: cell {i} moved");
            }
            // The ledgers book the actual sums, and the allowance books the same number.
            assert_eq!(world.care().feed_material_in, q.material_in);
            assert_eq!(world.care().feed_energy_in, q.energy_in);
            assert_eq!(world.care().allowance_used, q.material_in);
            world.check_invariants().expect("a dosed feed leaves a consistent world");
        }
    }
}

/// The 30-unit net allowance is a **rule about totals**, unchanged by the dose: twelve feeds at
/// 2.5 units and five at 6 both buy exactly 30 and no more. And a dose the remaining allowance
/// cannot cover is **refused**, not quietly served smaller — the viewer asked for an amount, and
/// a smaller one is a different answer.
#[test]
fn the_allowance_is_thirty_units_at_every_dose_and_refuses_rather_than_shrinking() {
    for (permille, expected) in [(250u16, 40u32), (500, 20), (1000, 10), (1500, 6), (2000, 5)] {
        let cell = CellId::new(Face::Front, 8, 8);
        let mut world = World::new(WorldConfig::default()).expect("valid");
        let mut seq = 0;
        let mut accepted = 0;
        let reason = loop {
            seq += 1;
            match world.apply_care(&dosed(seq, 0, CareKind::Feed, cell, permille)).outcome {
                CareOutcome::Applied(_) => accepted += 1,
                CareOutcome::Rejected(r) => break r,
                other => panic!("a feed came back {other:?}"),
            }
            assert!(seq < 128, "the allowance never ran out at {permille}");
        };
        assert_eq!(accepted, expected, "at {permille} permille");
        assert_eq!(reason, "allowance exhausted");
        assert_eq!(world.care().admitted_seq, seq, "a refusal still spends its seq");
        let spent = f64::from(accepted) * f64::from(permille) / 1000.0 * FEED_MATERIAL;
        assert!((world.care().feed_material_in - spent).abs() < 1e-9);
        assert!(world.care().allowance_used <= FEED_ALLOWANCE + 1e-9);
        world.check_invariants().expect("consistent");
    }
}

/// The refusal is exact about *which* dose does not fit: with 27 of 30 spent, a 3-unit standard
/// feed still lands and a 6-unit double does not — and the refused double changes no field and
/// no ledger while still spending its sequence.
#[test]
fn a_dose_larger_than_the_remaining_allowance_is_refused_without_a_trace() {
    let cell = CellId::new(Face::Front, 8, 8);
    let mut world = World::new(WorldConfig::default()).expect("valid");
    for seq in 1..=9 {
        assert!(matches!(
            world.apply_care(&dosed(seq, 0, CareKind::Feed, cell, 1000)).outcome,
            CareOutcome::Applied(_)
        ));
    }
    assert!((world.care().allowance_used - 27.0).abs() < 1e-9);

    let d_before: Vec<f64> = world.state.fields.d.clone();
    let de_before: Vec<f64> = world.state.fields.de.clone();
    let ledgers_before = world.care().clone();

    let refused = world.apply_care(&dosed(10, 0, CareKind::Feed, cell, 2000)).outcome;
    assert_eq!(refused, CareOutcome::Rejected("allowance exhausted".into()));
    assert_eq!(world.state.fields.d, d_before, "a refused feed moved the litter");
    assert_eq!(world.state.fields.de, de_before);
    assert_eq!(world.care().feed_material_in, ledgers_before.feed_material_in);
    assert_eq!(world.care().allowance_used, ledgers_before.allowance_used);
    assert_eq!(world.care().admitted_seq, 10, "but it still spent its seq");

    // The 3 units that do fit still land, and then nothing else does.
    assert!(matches!(
        world.apply_care(&dosed(11, 0, CareKind::Feed, cell, 1000)).outcome,
        CareOutcome::Applied(_)
    ));
    assert!(matches!(
        world.apply_care(&dosed(12, 0, CareKind::Feed, cell, 250)).outcome,
        CareOutcome::Rejected(_)
    ));
}

// ---------------------------------------------------------------- clean

/// Clean scales the nominal 2-unit maximum export and **retains the 50%-per-cell limit**: a
/// bigger dose takes more litter where there is plenty, and still cannot take more than half of
/// what any one cell holds. It remains litter cleanup, not sterilization.
#[test]
fn a_clean_dose_scales_the_cap_but_never_the_per_cell_half() {
    let graph = FieldGraph::new();
    for (name, cell) in places() {
        let fp = footprint(&graph, cell, 1);
        for permille in DOSES {
            let cap = f64::from(permille) / 1000.0 * CLEAN_MATERIAL;
            // Plenty of litter: every cell's share is the dose's, not the half-limit.
            let mut world = World::new(WorldConfig::default()).expect("valid");
            let rho = world.config().detritus.energy_cap;
            world.state.fields.d.fill(20.0);
            world.state.fields.de.fill(rho * 20.0);
            let receipt = world.apply_care(&dosed(1, 0, CareKind::Clean, cell, permille));
            let CareOutcome::Applied(q) = receipt.outcome.clone() else {
                panic!("{name} at {permille}: {:?}", receipt.outcome)
            };
            let want: f64 = fp.iter().map(|(_, w)| cap * w).sum();
            assert!((q.material_out - want).abs() < 1e-12, "{name} at {permille}: {}", q.material_out);
            assert!((q.material_out - cap).abs() < 1e-9, "the dose is the total over the footprint");
            assert!((q.energy_out - rho * q.material_out).abs() < 1e-9);
            assert_eq!(q.cells as usize, fp.len(), "{name}: the footprint moved");
            for (c, w) in &fp {
                assert!(
                    (world.state.fields.d[c.index()] - (20.0 - cap * w)).abs() < 1e-12,
                    "{name} at {permille}"
                );
            }
            world.check_invariants().expect("a dosed clean preserves the energy density");

            // Scarce litter: the half-of-what-is-there limit bites at every dose alike, so the
            // largest dose removes exactly what the smallest does and the cell is not emptied.
            let mut world = World::new(WorldConfig::default()).expect("valid");
            world.state.fields.d.fill(0.0);
            world.state.fields.de.fill(0.0);
            world.state.fields.d[cell.index()] = 0.01;
            world.state.fields.de[cell.index()] = 0.02;
            let receipt = world.apply_care(&dosed(1, 0, CareKind::Clean, cell, permille));
            let CareOutcome::Partial(q) = receipt.outcome.clone() else {
                panic!("{name} scarce at {permille}: {:?}", receipt.outcome)
            };
            assert!((q.material_out - CLEAN_FRACTION * 0.01).abs() < 1e-15, "{name} at {permille}");
            assert!((world.state.fields.d[cell.index()] - 0.005).abs() < 1e-15);
            assert!((world.state.fields.de[cell.index()] - 0.01).abs() < 1e-15);
            assert!(world.state.fields.d[cell.index()] > 0.0, "cleanup is not sterilization");
        }
    }
}

/// Empty and sparse footprints answer honestly at every dose: nothing to remove is a refusal,
/// and less than the dose asked for is a partial, never an "applied" that overstates itself.
#[test]
fn an_empty_or_sparse_footprint_is_refused_or_partial_at_every_dose() {
    let cell = CellId::new(Face::Front, 4, 4);
    for permille in DOSES {
        let mut world = World::new(WorldConfig::default()).expect("valid");
        world.state.fields.d.fill(0.0);
        world.state.fields.de.fill(0.0);
        let receipt = world.apply_care(&dosed(1, 0, CareKind::Clean, cell, permille));
        assert_eq!(receipt.outcome, CareOutcome::Rejected("nothing to remove".into()));
        assert_eq!(world.care().admitted_seq, 1, "a refusal still consumes its seq");
        assert_eq!(world.care().clean_material_out, 0.0);

        // Exactly enough litter for the *standard* cap in the centre cell alone. A dose above
        // standard cannot reach it, so it must report partial rather than applied.
        world.state.fields.d[cell.index()] = 2.0 * CLEAN_MATERIAL;
        world.state.fields.de[cell.index()] = 0.0;
        let receipt = world.apply_care(&dosed(2, 0, CareKind::Clean, cell, permille));
        let got = receipt.outcome.applied().expect("something was removed").material_out;
        let cap = f64::from(permille) / 1000.0 * CLEAN_MATERIAL;
        assert!(got + 1e-12 < cap, "{permille}: {got} should fall short of {cap}");
        assert!(
            matches!(receipt.outcome, CareOutcome::Partial(_)),
            "{permille}: a short clean must say partial, not applied: {:?}",
            receipt.outcome
        );
    }
}

/// Only the material a clean **actually removed** restores feed allowance, at any dose: a large
/// clean over an empty footprint gives nothing back, and the allowance never turns into credit.
#[test]
fn only_material_actually_removed_restores_the_allowance() {
    let cell = CellId::new(Face::Front, 8, 8);
    let mut world = World::new(WorldConfig::default()).expect("valid");
    for seq in 1..=10 {
        world.apply_care(&dosed(seq, 0, CareKind::Feed, cell, 1000));
    }
    let used = world.care().allowance_used;
    assert!(used > FEED_ALLOWANCE - FEED_MATERIAL);

    // The fed litter is there, so the big clean takes the smaller of the dose and half of it.
    let before: f64 = world.state.fields.d.iter().sum();
    let q = world
        .apply_care(&dosed(11, 0, CareKind::Clean, cell, 2000))
        .outcome
        .applied()
        .expect("there is litter here")
        .material_out;
    let after: f64 = world.state.fields.d.iter().sum();
    assert!((before - after - q).abs() < 1e-12, "the ledger is what left the fields");
    assert!((world.care().allowance_used - (used - q)).abs() < 1e-12);

    // And a clean that removes nothing restores nothing.
    let far = CellId::new(Face::Back, 10, 10);
    world.state.fields.d.fill(0.0);
    world.state.fields.de.fill(0.0);
    let held = world.care().allowance_used;
    assert!(matches!(
        world.apply_care(&dosed(12, 0, CareKind::Clean, far, 2000)).outcome,
        CareOutcome::Rejected(_)
    ));
    assert_eq!(world.care().allowance_used, held);
}

// ---------------------------------------------------------------- rain

/// Rain scales the nominal 4 depth units and keeps the same footprint and the same eased
/// 120-tick envelope: a bigger shower is not a longer one or a wider one.
#[test]
fn a_rain_dose_scales_the_depth_over_the_same_footprint_and_envelope() {
    let graph = FieldGraph::new();
    for (name, cell) in places() {
        let fp = footprint(&graph, cell, 2);
        for permille in DOSES {
            let want = f64::from(permille) / 1000.0 * RAIN_DEPTH_TOTAL;
            let mut world = World::new(still_water_config()).expect("valid");
            world.state.fields.w.fill(0.0);
            let rain_before = world.state.rain_in_total;

            let receipt = world.apply_care(&dosed(1, 0, CareKind::Rain, cell, permille));
            let CareOutcome::Applied(q) = receipt.outcome.clone() else {
                panic!("{name} at {permille}: {:?}", receipt.outcome)
            };
            assert_eq!(q.cells as usize, fp.len(), "{name} at {permille}: the footprint moved");
            assert_eq!(q.water_depth, want, "the receipt quotes the scheduled total honestly");
            assert_eq!(q.ends_tick, Some(u64::from(RAIN_TICKS)), "the envelope is the same length");
            assert_eq!(world.care().showers[0].dose_permille, permille);

            for _ in 0..RAIN_TICKS {
                world.step();
            }
            assert!(world.care().showers.is_empty(), "{name} at {permille}: the shower did not retire");
            let delivered: f64 = world.state.fields.w.iter().sum();
            assert!((delivered - want).abs() < 1e-9, "{name} at {permille} delivered {delivered}");
            assert!((world.care().rain_depth_in - want).abs() < 1e-9, "{name} at {permille}");
            assert_eq!(
                world.state.rain_in_total - rain_before,
                world.care().rain_depth_in,
                "{name}: manual water is inside rain_in_total exactly once"
            );
            for (c, w) in &fp {
                let got = world.state.fields.w[c.index()];
                assert!((got - want * w).abs() < 1e-9, "{name} at {permille}, cell {}: {got}", c.index());
            }
            world.check_invariants().expect("a dosed shower leaves a consistent world");
        }
    }
}

/// The dose belongs to the shower, not to whatever a panel currently offers. A nonstandard
/// shower survives a snapshot and delivers exactly its remaining samples at its own amount, and
/// a later command at a different dose cannot reach in and change it.
#[test]
fn a_nonstandard_shower_persists_its_own_dose_across_a_restart() {
    let cell = CellId::new(Face::Front, 6, 9);
    let mut original = World::new(still_water_config()).expect("valid");
    original.state.fields.w.fill(0.0);
    assert!(matches!(
        original.apply_care(&dosed(1, 0, CareKind::Rain, cell, 1500)).outcome,
        CareOutcome::Applied(_)
    ));
    for _ in 0..50 {
        original.step();
    }
    assert_eq!(original.care().showers[0].delivered, 50);

    // A second command, at a different dose, is refused — and changes nothing about the first.
    let second = original.apply_care(&dosed(2, 50, CareKind::Rain, cell, 250));
    assert_eq!(second.outcome, CareOutcome::Rejected("shower active".into()));
    assert_eq!(original.care().showers[0].dose_permille, 1500, "a selection cannot retune a shower");
    assert_eq!(original.care().admitted_seq, 2);

    // The dose is in the snapshot, so the reloaded world is the same world.
    let bytes = encode_snapshot(&original.state, "dose-resume-test");
    let (_, state) = decode_snapshot(&bytes).expect("a mid-shower nonstandard state round trips");
    assert_eq!(state.care.showers[0].dose_permille, 1500);
    let mut reloaded = World::from_state(state).expect("valid");
    assert_eq!(state_hash(&reloaded.state), state_hash(&original.state));

    for _ in 0..(RAIN_TICKS + 30) {
        original.step();
        reloaded.step();
        assert_eq!(
            state_hash(&reloaded.state),
            state_hash(&original.state),
            "diverged at tick {}",
            original.tick()
        );
    }
    assert!(original.care().showers.is_empty());
    let want = 1.5 * RAIN_DEPTH_TOTAL;
    let delivered: f64 = original.state.fields.w.iter().sum();
    assert!((delivered - want).abs() < 1e-9, "the resumed shower delivered {delivered}, not {want}");
    assert!((original.care().rain_depth_in - want).abs() < 1e-9);
}

/// A restart at the minimum dose is the same claim from the other end, and the depth booked
/// before the snapshot plus the depth after it is the whole shower, counted once.
#[test]
fn a_restart_delivers_exactly_the_remaining_samples_of_a_quarter_dose() {
    let cell = CellId::new(Face::Top, 10, 10);
    let mut world = World::new(still_water_config()).expect("valid");
    world.state.fields.w.fill(0.0);
    world.apply_care(&dosed(1, 0, CareKind::Rain, cell, 250));
    for _ in 0..90 {
        world.step();
    }
    let booked_before = world.care().rain_depth_in;
    assert!(booked_before > 0.0);

    let bytes = encode_snapshot(&world.state, "quarter");
    let (_, state) = decode_snapshot(&bytes).expect("round trips");
    assert_eq!(state.care.showers[0].dose_permille, 250);
    assert_eq!(state.care.showers[0].delivered, 90);
    let mut resumed = World::from_state(state).expect("valid");
    for _ in 0..30 {
        resumed.step();
    }
    assert!(resumed.care().showers.is_empty(), "the last 30 samples finished it");
    let want = 0.25 * RAIN_DEPTH_TOTAL;
    assert!((resumed.care().rain_depth_in - want).abs() < 1e-9, "{}", resumed.care().rain_depth_in);
    let delivered: f64 = resumed.state.fields.w.iter().sum();
    assert!((delivered - want).abs() < 1e-9, "{delivered}");
    assert!(booked_before < want, "and the pre-snapshot part was only part of it");
}

// ---------------------------------------------------------------- hostile input

/// A dose that never went through the typed constructor — one decoded straight off a wire — is
/// **rejected on admission**, not clamped. Like every other admitted-then-rejected command it
/// spends its sequence, so a replay of the journal reaches the same cursor.
#[test]
fn a_wire_dose_outside_the_bounds_is_rejected_and_still_spends_its_seq() {
    let cell = CellId::new(Face::Front, 8, 8);
    for (seq, permille) in [(1u64, 0u16), (2, 249), (3, 2001), (4, 50_000), (5, u16::MAX)] {
        let mut world = World::new(WorldConfig::default()).expect("valid");
        world.state.care.admitted_seq = seq - 1;
        let d_before: Vec<f64> = world.state.fields.d.clone();
        let cmd = CareCommand {
            seq,
            apply_after_tick: 0,
            kind: CareKind::Feed,
            target: target_of(cell),
            dose: wire_dose(permille),
        };
        let receipt = world.apply_care(&cmd);
        let reason = receipt.outcome.reason().unwrap_or_else(|| panic!("{permille} was accepted"));
        assert!(reason.contains("care dose"), "{reason}");
        assert!(reason.contains(&permille.to_string()), "the reason names the value: {reason}");
        assert_eq!(world.care().admitted_seq, seq, "a rejected dose still spends its seq");
        assert_eq!(world.state.fields.d, d_before, "and moves nothing");
        assert_eq!(world.care().feed_material_in, 0.0);
    }
}

/// The same check on the way in from a snapshot: a crafted shower carrying an impossible dose
/// is refused by the decoder rather than clamped into range, exactly as a crafted weight or a
/// crafted delivery count already is.
#[test]
fn a_crafted_shower_dose_is_refused_by_the_decoder() {
    let sound = |permille: u16| ActiveShower {
        seq: 1,
        apply_after_tick: 0,
        cells: vec![0],
        weights: vec![1.0],
        delivered: 0,
        dose_permille: permille,
    };
    let with = |permille: u16| {
        let mut world = World::new(still_water_config()).expect("valid");
        world.state.care.admitted_seq = 1;
        world.state.care.showers.push(sound(permille));
        world.state
    };

    // The controls: both documented bounds and the standard dose really do load.
    for ok in [CareDose::MIN_PERMILLE, CareDose::STANDARD_PERMILLE, CareDose::MAX_PERMILLE] {
        let state = with(ok);
        state.validate().unwrap_or_else(|e| panic!("{ok} should be valid: {e}"));
        decode_snapshot(&encode_snapshot(&state, "dose")).unwrap_or_else(|e| panic!("{ok}: {e:?}"));
    }

    for bad in [0u16, 1, 249, 2001, 9999, u16::MAX] {
        let state = with(bad);
        let err = state.validate().expect_err(&format!("{bad} should be refused"));
        assert!(err.contains("care shower 1"), "{err}");
        assert!(err.contains(&bad.to_string()), "{err}");
        match decode_snapshot(&encode_snapshot(&state, "dose")) {
            Err(SnapshotError::Invalid(reason)) => assert!(reason.contains(&bad.to_string()), "{reason}"),
            other => panic!("a shower at {bad} permille decoded as {other:?}"),
        }
    }
}

/// The dose is inside the replay hash: two worlds that differ only in the amount a falling
/// shower is delivering are different worlds, and nothing downstream may treat them as one.
#[test]
fn the_dose_is_part_of_the_state_hash() {
    let cell = CellId::new(Face::Front, 8, 8);
    let build = |permille: u16| {
        let mut world = World::new(still_water_config()).expect("valid");
        world.apply_care(&dosed(1, 0, CareKind::Rain, cell, permille));
        world
    };
    let standard = build(1000);
    let generous = build(1500);
    assert_ne!(state_hash(&standard.state), state_hash(&generous.state));
    assert_eq!(state_hash(&build(1000).state), state_hash(&standard.state));
}
