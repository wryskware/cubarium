//! Schema 10 migration and the empty-extension continuations.
//!
//! The load-bearing test here is
//! [`an_empty_extension_reproduces_the_pre_hunter_binarys_next_600_ticks`]: two fixtures
//! produced by the *pre-hunter* schema 9 release build (root's `1f0fc3a`, provenance in
//! `tests/fixtures/pre-hunter-v9-provenance.md`) prove that a world carrying the hunter
//! extension but no hunters steps exactly as the pre-hunter binary did — care, the signed
//! energy corrections and the raw counters included. The genuine schema 8 and schema 7
//! continuations in `care.rs` and `energy_correction.rs` must keep passing beside it.

use std::path::PathBuf;

use cubarium_core::genome::Genome;
use cubarium_core::hunter::{HunterPhase, HunterRole, HunterState};
use cubarium_core::snapshot::{
    HEADER_FIXED_BYTES, MAGIC, SnapshotError, state_hash, v7, v8, v9, v10,
};
use cubarium_core::{
    SCHEMA_V9, SCHEMA_V10, SCHEMA_VERSION, World, WorldConfig, decode_snapshot, ecology_hash,
    encode_snapshot,
};

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

/// A genuine live schema 9 world loads with an empty, inert extension and its schema 9
/// projection is the original payload byte for byte.
#[test]
fn the_live_schema_nine_snapshot_migrates_without_a_hunter() {
    let bytes = std::fs::read(fixture("pre-hunter-v9-173400.cubw")).expect("the fixture is committed");
    let (meta, state) = decode_snapshot(&bytes).expect("a genuine schema 9 snapshot loads");
    assert_eq!(meta.schema, SCHEMA_V9, "the header still reports what was read");
    assert_ne!(SCHEMA_VERSION, SCHEMA_V9);
    assert_eq!(state.tick, 173_400);
    assert_eq!(state.organisms.len(), 80, "the provenance records 80 organisms");
    assert_eq!(state.care.admitted_seq, 5, "the fixture carries a real care history");
    assert_ne!(
        state.energy_correction,
        cubarium_core::EnergyCorrection::default(),
        "the provenance says this world already carries 600 ticks of corrections"
    );

    // Migration introduces no predator, no profile and no import.
    assert_eq!(state.hunters, HunterState::default());
    assert!(!state.hunters.active());
    assert_eq!(state.hunters.gut_material_total(), 0.0);
    assert_eq!(state.hunters.imported_material(), 0.0);
    assert_eq!(state.hunters.imported_energy(), 0.0);

    let projected =
        postcard::to_allocvec(&v9::project(&state).expect("standard care projects")).expect("encodable");
    assert_eq!(projected, payload(&bytes), "re-encoding the projection is not the original payload");
    // The provenance's own schema 9 hash, computed here from the file's bytes.
    assert_eq!(format!("{:x}", fnv1a(payload(&bytes))), "134f4db0135d8a0a");
    // The older projections still mean what they meant.
    assert_eq!(ecology_hash(&state), fnv1a(&postcard::to_allocvec(&v7::project(&state)).unwrap()));
    assert_ne!(state_hash(&state), fnv1a(payload(&bytes)), "the full hash covers the extension");

    let world = World::from_state(state).expect("the migrated state is a valid world");
    world.check_invariants().expect("invariants hold on the migrated world");
    assert!(world.mass_residual().abs() < 1e-9, "residual {}", world.mass_residual());
    assert!(world.hunter_view().is_empty(), "a migrated world has no hunters to draw");
}

/// The cross-version proof for this package. If it fails, the hunter extension changed an
/// operation in the zero-hunter tick and nothing else in the package means anything.
#[test]
fn an_empty_extension_reproduces_the_pre_hunter_binarys_next_600_ticks() {
    let start = std::fs::read(fixture("pre-hunter-v9-173400.cubw")).expect("fixture");
    let plus600 = std::fs::read(fixture("pre-hunter-v9-173400-plus600.cubw")).expect("fixture");
    let (_, state) = decode_snapshot(&start).expect("schema 9 loads");
    let (meta, expected) = decode_snapshot(&plus600).expect("schema 9 loads");
    assert_eq!(meta.schema, SCHEMA_V9);
    assert_eq!(expected.tick, 174_000);

    let mut world = World::from_state(state).expect("valid");
    for _ in 0..600 {
        world.step();
    }
    assert_eq!(world.tick(), 174_000);

    let projected = postcard::to_allocvec(&v9::project(&world.state).expect("standard care projects"))
        .expect("encodable");
    assert_eq!(
        projected,
        payload(&plus600),
        "600 zero-hunter ticks diverged from the pre-hunter binary"
    );
    assert_eq!(format!("{:x}", fnv1a(payload(&plus600))), "854dce1766d905d3", "the provenance hash");
    // Named individually so a failure says what moved.
    assert_eq!(world.state.fields, expected.fields, "field stocks");
    assert_eq!(world.state.organisms, expected.organisms, "organisms");
    assert_eq!(world.state.weather, expected.weather, "weather");
    assert_eq!(world.state.config, expected.config, "config");
    assert_eq!(world.state.care, expected.care, "care ledgers and shower progress");
    assert_eq!(world.state.energy_correction, expected.energy_correction, "signed corrections");
    assert_eq!(world.state.light_in_total, expected.light_in_total, "raw light_in_total");
    assert_eq!(world.state.heat_out_total, expected.heat_out_total, "raw heat_out_total");
    assert_eq!(world.state.rain_in_total, expected.rain_in_total);
    assert_eq!(world.state.evap_out_total, expected.evap_out_total);
    assert_eq!(world.state.births_total, expected.births_total);
    assert_eq!(world.state.deaths_total, expected.deaths_total);
    assert_eq!(world.state.hunters, HunterState::default(), "no hunter appeared");
    assert_eq!(ecology_hash(&world.state), ecology_hash(&expected), "legacy ecology projection");
    // And the whole schema 10 state hashes as the schema 9 payload plus the empty extension.
    assert_ne!(state_hash(&world.state), fnv1a(payload(&plus600)));
}

/// The schema 8 and schema 7 mirrors still project through the two appended fields.
#[test]
fn the_older_projections_still_drop_only_what_they_are_named_for() {
    let bytes = std::fs::read(fixture("live-v8-172800.cubw")).expect("fixture");
    let (_, v8_state) = decode_snapshot(&bytes).expect("schema 8 loads");
    assert_eq!(v8_state.hunters, HunterState::default());
    assert_eq!(
        postcard::to_allocvec(&v8::project(&v8_state).expect("standard care projects")).unwrap(),
        payload(&bytes),
        "the schema 8 projection is still the original payload"
    );

    let bytes = std::fs::read(fixture("live-v7-55200.cubw")).expect("fixture");
    let (_, v7_state) = decode_snapshot(&bytes).expect("schema 7 loads");
    assert_eq!(v7_state.hunters, HunterState::default());
    assert_eq!(
        postcard::to_allocvec(&v7::project(&v7_state)).unwrap(),
        payload(&bytes),
        "the schema 7 projection is still the original payload"
    );
}

/// A world that never opts in is inert in every observable way, and its own snapshots
/// round-trip through schema 10 unchanged.
#[test]
fn a_world_that_never_opts_in_carries_an_empty_extension() {
    let mut world = World::new(WorldConfig::default()).expect("defaults are valid");
    for _ in 0..200 {
        world.step();
    }
    assert_eq!(world.state.hunters, HunterState::default());
    assert!(world.hunter_view().is_empty());
    assert!(world.drain_hunter_events().is_empty());
    let sample = world.telemetry();
    assert_eq!(sample.hunters, 0);
    assert_eq!(sample.hunter_captures, 0);
    assert_eq!(sample.hunter_attacks, 0);
    assert_eq!(sample.deaths_predation, 0);

    let bytes = encode_snapshot(&world.state, "hunter-inert");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), SCHEMA_VERSION);
    let (meta, back) = decode_snapshot(&bytes).expect("round trip");
    assert_eq!(meta.schema, SCHEMA_VERSION);
    assert_eq!(back, world.state);
}

// ---------------------------------------------------------------- schema 10

/// Frame a payload as a snapshot of `schema`, the way `encode_snapshot` frames the current one.
fn frame(schema: u32, payload: &[u8], build: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&schema.to_le_bytes());
    out.extend_from_slice(&(build.len() as u16).to_le_bytes());
    out.extend_from_slice(build.as_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// A schema 10 world that never opted in migrates exactly, like every other older schema: the
/// extension it did not use opens empty and nothing else moves.
#[test]
fn a_schema_ten_snapshot_without_a_trial_migrates_exactly() {
    let mut world = World::new(WorldConfig::default()).expect("defaults are valid");
    for _ in 0..60 {
        world.step();
    }
    let old = v10::project(&world.state).expect("an empty extension has a schema 10 image");
    let bytes = frame(SCHEMA_V10, &postcard::to_allocvec(&old).expect("encodable"), "schema-ten");

    let (meta, back) = decode_snapshot(&bytes).expect("an empty schema 10 world loads");
    assert_eq!(meta.schema, SCHEMA_V10);
    assert_eq!(back, world.state, "the migration moved something it should not have");
    assert_eq!(state_hash(&back), state_hash(&world.state));
    assert_eq!(back.hunters, HunterState::default());
}

/// A schema 10 world carrying **any** hunter extension is refused by name — a running trial,
/// but equally a budget-matched control or an extinct lineage's counters. Their profile and
/// members have a different shape in schema 11 — the measured capture effector, the ingestion
/// mouth, the body scale, the transition origin — and there is no honest way to fill those in,
/// so the load fails instead of quietly changing what the experiment measured.
#[test]
fn a_schema_ten_snapshot_with_any_hunter_history_is_refused_not_reinterpreted() {
    let mut world = World::new(WorldConfig::default()).expect("defaults are valid");
    for _ in 0..10 {
        world.step();
    }
    let mut old = v10::project(&world.state).expect("an empty extension projects");
    let founder = world.state.organisms.iter().next().expect("founders exist").0;
    old.hunters.profile = Some(v10::FixedHunterProfileV10 {
        version: 1,
        role: HunterRole::Lanternjaw,
        genome: Genome::founder(0.08, &world.config().drives),
        attacks_enabled: true,
        body_extent_px: 9.0,
        jaw_offset_px: 6.0,
        jaw_reach_px: 1.5,
        founder_reserve_fraction: 0.5,
        founder_energy_fraction: 0.75,
        perch_reserve_fraction: 0.65,
        seek_reserve_fraction: 0.35,
        prey_structure_min: 0.15,
        prey_structure_fraction_max: 0.75,
        stalk_timeout_seconds: 8.0,
        windup_seconds: 0.6,
        strike_seconds: 1.0,
        strike_speed_px_s: 1.0,
        strike_energy_cost: 0.08,
        recovery_seconds: 5.0,
        capture_base: 0.65,
        capture_min: 0.1,
        capture_max: 0.75,
        escape_speed_multiple: 2.0,
        escape_turn_rate_deg: 240.0,
        gut_capacity_material: 4.0,
        handling_cost_per_second: 0.002,
        digest_rate: 0.1,
        meal_recovery_seconds: 20.0,
        scavenge_fraction: 0.0,
        reproduce_min_age_seconds: 1200.0,
        reproduce_reserve_fraction: 0.8,
        reproduce_energy_fraction: 0.75,
        reproduce_interval_seconds: 1800.0,
        gestation_seconds: 120.0,
        juvenile_growth_rate: 0.002,
    });
    old.hunters.members.push(v10::HunterMemberV10 {
        id: founder,
        phase: HunterPhase::Perched,
        phase_started_tick: old.tick,
        phase_ends_tick: old.tick,
        target: None,
        attack_counter: 0,
        next_reproduction_tick: 0,
        gut_material: 0.0,
        gut_energy: 0.0,
    });
    old.hunters.founder_material_in = 4.0;
    old.hunters.founder_energy_in = 7.0;
    old.hunters.founders_placed = 1;

    let bytes = frame(SCHEMA_V10, &postcard::to_allocvec(&old).expect("encodable"), "schema-ten");
    match decode_snapshot(&bytes) {
        Err(SnapshotError::Invalid(why)) => {
            assert!(why.contains("non-empty hunter extension"), "{why}");
            assert!(why.contains("schema 10"), "the refusal must name the schema: {why}");
            assert!(why.contains("refused"), "the refusal must say so: {why}");
        }
        other => panic!("an active schema 10 trial decoded as {other:?}"),
    }

    // The same refusal for history that is not a live hunt: a budget-matched control deposit,
    // and a world whose hunters all died but whose counters remain.
    /// One way to leave hunter history in an otherwise empty schema 10 extension.
    type History = fn(&mut v10::HunterStateV10);
    let cases: [(&str, History); 2] = [
        ("a control deposit", |h| {
            h.profile = None;
            h.members.clear();
            h.control_deposited = true;
            h.control_material_in = 4.0;
            h.control_energy_in = 7.0;
        }),
        ("an extinct lineage", |h| {
            h.profile = None;
            h.members.clear();
            h.hunter_deaths_total = 1;
            h.captures_total = 3;
        }),
    ];
    for (what, poison) in cases {
        let mut old = v10::project(&world.state).expect("an empty extension projects");
        poison(&mut old.hunters);
        let bytes = frame(SCHEMA_V10, &postcard::to_allocvec(&old).expect("encodable"), "schema-ten");
        match decode_snapshot(&bytes) {
            Err(SnapshotError::Invalid(why)) => assert!(why.contains("non-empty"), "{what}: {why}"),
            other => panic!("{what} decoded as {other:?}"),
        }
    }

    // And a current world with a trial has no honest schema 10 image either.
    let profile = cubarium_core::FixedHunterProfile::lanternjaw_trial(world.config());
    let target = cubarium_core::HunterTarget { face: 4, u: 22.0, v: 34.0 };
    world.start_hunter_trial(profile, target).expect("the trial starts");
    assert!(v10::project(&world.state).is_none(), "a running trial must not project backwards");
}
