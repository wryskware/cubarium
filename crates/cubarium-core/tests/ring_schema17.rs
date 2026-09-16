//! FW-6: the ring world's schema break, the extended `WorldState::validate`, and the
//! `CubeProjection` negative
//! tests, written from `design/flat-world-plan-2026-09-16.md` §4 — "**Negative tests are
//! part of the definition** … perturb one organism field, one allocator free-list entry,
//! one weather blob, one field vector, one extension state and one non-added config field,
//! and assert equality **fails** in every case. A comparator that cannot fail is not
//! evidence."
//!
//! **The number moved after this file was written.** The ring break was schema 17 on this
//! branch; merging `main` found that `main` had spent 17 on a semantics-only bump of its own
//! (the reach-envelope pursuit rule), so the merged build is **schema 18** and refuses 7
//! through 17 by name — see `crates/cubarium-core/src/snapshot.rs` and
//! `design/7_Research/flat-world-sync-main-2026-09-16.md`. The path stays as the plan
//! reserved it (§9, FW-6's row); every claim below is the same claim with one more predecessor
//! in the refusal range.

use cubarium_core::config::{CONFIG_VERSION, WorldConfig};
use cubarium_core::snapshot::projection::{CubeProjection, first_difference, projection_hash};
use cubarium_core::snapshot::v16::{
    CONFIG_VERSION_V16, SCHEMA_V16, WorldConfigV16, WorldStateV16, decode_v16,
};
use cubarium_core::snapshot::{
    MAGIC, SCHEMA_VERSION, SnapshotError, decode_snapshot, encode_snapshot,
};
use cubarium_core::world::{World, WorldState};
use cubarium_surface::{Face, Scale, Topology};

const SEED: u64 = 20260916;

fn ring() -> Topology {
    Topology::Ring { w: 320, h: 180 }
}

fn cube_state(ticks: u32) -> WorldState {
    let mut world =
        World::new(WorldConfig { seed: SEED, ..WorldConfig::default() }).expect("legal cube world");
    for _ in 0..ticks {
        world.step();
    }
    world.state
}

fn ring_state(ticks: u32) -> WorldState {
    let mut cfg = WorldConfig { seed: SEED, ..WorldConfig::default() };
    cfg.topology = ring();
    let mut world = World::new(cfg).expect("legal ring world");
    for _ in 0..ticks {
        world.step();
    }
    world.state
}

// ---------------------------------------------------------------------------
// The break
// ---------------------------------------------------------------------------

#[test]
fn the_schema_is_eighteen_and_the_config_version_is_nine() {
    assert_eq!(SCHEMA_VERSION, 18, "§4: a hard bump — 17 on this branch, 18 after main's own");
    assert_eq!(CONFIG_VERSION, 9, "§4: CONFIG_VERSION 9");
    assert_eq!(SCHEMA_V16, 16);
    assert_eq!(cubarium_core::snapshot::SCHEMA_V17, 17);
    assert_eq!(CONFIG_VERSION_V16, 8, "what the fixture must report");
}

/// §4: "refusing 7..=16 by name", now 7..=17 — `main`'s schema 17 is a predecessor like any
/// other. Every older schema — and anything newer — comes back as `UnsupportedSchema` carrying
/// the version that was read, so a caller can say which world it was instead of migrating it.
#[test]
fn schemas_seven_through_seventeen_are_refused_by_name() {
    let bytes = encode_snapshot(&ring_state(0), "fw6");
    for schema in 7u32..=17 {
        let mut relabelled = bytes.clone();
        relabelled[4..8].copy_from_slice(&schema.to_le_bytes());
        assert_eq!(
            decode_snapshot(&relabelled),
            Err(SnapshotError::UnsupportedSchema(schema)),
            "schema {schema} must be refused by name"
        );
    }
    // And a future schema is refused the same way, not guessed at.
    let mut newer = bytes.clone();
    newer[4..8].copy_from_slice(&19u32.to_le_bytes());
    assert_eq!(decode_snapshot(&newer), Err(SnapshotError::UnsupportedSchema(19)));
}

#[test]
fn a_ring_snapshot_round_trips_at_the_current_schema() {
    for state in [ring_state(5), cube_state(5)] {
        let bytes = encode_snapshot(&state, "fw6-test");
        let (meta, back) = decode_snapshot(&bytes).expect("a fresh snapshot decodes");
        assert_eq!(meta.schema, SCHEMA_VERSION);
        assert_eq!(meta.build_id, "fw6-test");
        assert_eq!(back.config.topology, state.config.topology, "the topology is carried");
        assert_eq!(back.config.world_scale, state.config.world_scale);
        assert_eq!(back, state, "the whole state round trips");
    }
}

// ---------------------------------------------------------------------------
// The extended WorldState::validate — §4's list
// ---------------------------------------------------------------------------

#[track_caller]
fn refused(state: &WorldState, what: &str) -> String {
    match state.validate() {
        Ok(()) => panic!("{what}: validate accepted a state it must refuse"),
        Err(e) => e,
    }
}

#[test]
fn a_fresh_state_of_either_topology_validates() {
    ring_state(3).validate().expect("a ring world validates");
    cube_state(3).validate().expect("a cube world validates");
}

/// "`config.topology` is one this build supports and `Topology::validate()` passes".
#[test]
fn validate_refuses_a_topology_this_build_cannot_run() {
    let mut state = ring_state(0);
    state.config.topology = Topology::Ring { w: 321, h: 180 };
    let e = refused(&state, "a ring whose width is not a cell multiple");
    assert!(e.to_lowercase().contains("multiple") || e.contains("321"), "{e}");

    let mut state = cube_state(0);
    state.config.world_scale = Scale::new(2.0);
    let e = refused(&state, "a cube at S = 2");
    assert!(e.to_lowercase().contains("scale"), "{e}");

    let mut state = ring_state(0);
    state.config.topology = Topology::Ring { w: 1920, h: 1080 };
    let e = refused(&state, "129,600 cells");
    assert!(e.contains("129600") || e.to_lowercase().contains("cell"), "{e}");
}

/// "every `Fields` vector … has length exactly `cell_count()`" — and the same for ecology
/// v1's vectors. A snapshot whose shape disagrees with its topology is refused, not resized.
#[test]
fn validate_refuses_a_per_cell_vector_of_the_wrong_length() {
    let mut state = ring_state(0);
    state.fields.p.push(0.0);
    let e = refused(&state, "one extra producer cell");
    assert!(e.contains("3601") && e.contains("3600"), "the error should name both counts: {e}");

    let mut state = ring_state(0);
    state.ecology.wood.pop();
    let e = refused(&state, "one missing wood cell");
    assert!(e.contains("wood"), "the error should name the vector: {e}");

    // The cube's 1,280 is not a legal length for a ring world.
    let mut state = ring_state(0);
    state.fields.n.truncate(1280);
    let e = refused(&state, "a cube-length field vector in a ring world");
    assert!(e.contains("1280") && e.contains("3600"), "{e}");
}

/// "`CareState.showers[].cells` is a `Vec<u16>` of raw `CellId` indices … Each of those
/// becomes `cell_count()`" — so a shower naming cell 3,600 of a 3,600-cell world is refused,
/// while the cube's old bound of 1,280 is no longer the test.
#[test]
fn validate_refuses_a_care_shower_cell_outside_the_world() {
    let mut state = ring_state(0);
    state.care.admitted_seq = 1;
    state.care.showers.push(cubarium_core::care::ActiveShower {
        seq: 1,
        apply_after_tick: 0,
        cells: vec![3599],
        weights: vec![1.0],
        delivered: 0,
        dose_permille: 1000,
    });
    state.validate().expect("cell 3,599 is the last cell of a 320×180 ring");

    state.care.showers[0].cells = vec![3600];
    let e = refused(&state, "a shower naming cell 3,600");
    assert!(e.contains("3600") || e.to_lowercase().contains("cell"), "{e}");

    // A cell that was legal on the cube is still legal here; one beyond the cube's 1,280 is
    // legal on a ring and must not be refused by a leftover constant.
    state.care.showers[0].cells = vec![2000];
    state.validate().expect("cell 2,000 exists on a ring even though the cube had 1,280");
}

/// "every organism's `pos.face` is a chart the topology has … with `u < w`, `v < h`".
#[test]
fn validate_refuses_an_organism_off_the_topology() {
    let base = ring_state(1);
    let id = base.organisms.iter().next().expect("a founder").0;

    let mut state = base.clone();
    state.organisms.get_mut(id).unwrap().pos.face = Face::Top;
    let e = refused(&state, "an organism on a chart the ring does not have");
    assert!(e.to_lowercase().contains("face") || e.contains("Top"), "{e}");

    let mut state = base.clone();
    state.organisms.get_mut(id).unwrap().pos.u = 320.0;
    let e = refused(&state, "an organism at u = w");
    assert!(e.to_lowercase().contains("canonical") || e.contains("320"), "{e}");

    let mut state = base.clone();
    state.organisms.get_mut(id).unwrap().pos.v = 180.0;
    let e = refused(&state, "an organism at v = h");
    assert!(e.to_lowercase().contains("canonical") || e.contains("180"), "{e}");

    // A position beyond the cube's 64 is perfectly legal on a ring.
    let mut state = base.clone();
    state.organisms.get_mut(id).unwrap().pos.u = 300.0;
    state.organisms.get_mut(id).unwrap().pos.v = 170.0;
    state.validate().expect("(300, 170) is inside a 320×180 ring");
}

/// Weather keeps today's finiteness-only check (§4: "the blob model is unchanged").
#[test]
fn validate_refuses_non_finite_weather_and_totals() {
    let mut state = ring_state(0);
    state.weather.light[0].center[0] = f64::NAN;
    refused(&state, "a NaN blob centre");

    let mut state = ring_state(0);
    state.light_in_total = f64::INFINITY;
    let e = refused(&state, "a non-finite energy total");
    assert!(e.contains("light_in_total"), "{e}");
}

// ---------------------------------------------------------------------------
// The CubeProjection, and the negative tests §4 calls part of its definition
// ---------------------------------------------------------------------------

#[track_caller]
fn assert_differs(base: &CubeProjection, perturbed: &WorldState, expect: &str, what: &str) {
    let other = CubeProjection::from(perturbed);
    assert_ne!(base, &other, "{what}: the projection must not compare equal");
    assert_ne!(projection_hash(base), projection_hash(&other), "{what}: the hash must move too");
    assert_eq!(
        first_difference(base, &other),
        Some(expect),
        "{what}: the comparator must name the field"
    );
}

#[test]
fn the_projection_of_a_state_equals_itself_and_survives_a_round_trip() {
    let state = cube_state(30);
    let base = CubeProjection::from(&state);
    assert_eq!(base, CubeProjection::from(&state.clone()));
    assert_eq!(first_difference(&base, &CubeProjection::from(&state)), None);

    let bytes = encode_snapshot(&state, "fw6");
    let (_, back) = decode_snapshot(&bytes).expect("round trip");
    assert_eq!(base, CubeProjection::from(&back), "encode/decode changes nothing");
    assert_eq!(projection_hash(&base), projection_hash(&CubeProjection::from(&back)));
}

/// §4's whole point: the projection drops exactly three fields, so a cube world that ran
/// before the ring work and one that ran after compare equal *despite* the schema break.
#[test]
fn the_projection_ignores_the_three_excluded_fields_and_nothing_else() {
    let state = cube_state(10);
    let base = CubeProjection::from(&state);

    let mut moved = state.clone();
    moved.config.version = 8;
    moved.config.topology = ring();
    moved.config.world_scale = Scale::new(2.0);
    assert_eq!(
        base,
        CubeProjection::from(&moved),
        "version, topology and world_scale are the three fields that may move"
    );
    assert_eq!(projection_hash(&base), projection_hash(&CubeProjection::from(&moved)));
    assert_eq!(first_difference(&base, &CubeProjection::from(&moved)), None);
}

#[test]
fn perturbing_one_organism_field_breaks_the_comparison() {
    let state = cube_state(10);
    let base = CubeProjection::from(&state);
    let id = state.organisms.iter().next().expect("a founder").0;

    let mut one = state.clone();
    one.organisms.get_mut(id).unwrap().structure += 1e-12;
    assert_differs(&base, &one, "organisms", "one organism's structure");

    // A field an enumerated subset would have dropped (§4's list of what the first draft
    // silently lost).
    let mut two = state.clone();
    two.organisms.get_mut(id).unwrap().born_tick += 1;
    assert_differs(&base, &two, "organisms", "one organism's born_tick");
}

/// §4 insists the projection carries "the allocator's `entries`, `free` and `live`". Here the
/// live organisms are identical and only the free list's generation differs.
#[test]
fn perturbing_one_free_list_entry_breaks_the_comparison() {
    let state = cube_state(10);
    let id = state.organisms.iter().next().expect("a founder").0;
    let organism = state.organisms.get(id).expect("live").clone();

    // A: the slot is freed once.
    let mut a = state.clone();
    a.organisms.remove(id).expect("removed");
    // B: the same slot is freed, refilled and freed again, so the *live* set and the entries
    // are identical to A's and only the free list's generation moved.
    let mut b = a.clone();
    let again = b.organisms.insert(organism);
    b.organisms.remove(again).expect("removed again");

    assert_eq!(a.organisms.len(), b.organisms.len(), "the same number of live organisms");
    for ((ida, oa), (idb, ob)) in a.organisms.iter().zip(b.organisms.iter()) {
        assert_eq!(ida, idb, "the same ids");
        assert_eq!(oa, ob, "the same organisms");
    }
    assert_differs(&CubeProjection::from(&a), &b, "organisms", "one free-list entry");
}

#[test]
fn perturbing_one_weather_blob_breaks_the_comparison() {
    let state = cube_state(10);
    let base = CubeProjection::from(&state);
    let mut one = state.clone();
    one.weather.light[0].center[0] += 1e-12;
    assert_differs(&base, &one, "weather", "one blob centre");

    let mut two = state.clone();
    two.weather.last_walk_minute += 1;
    assert_differs(&base, &two, "weather", "the walk clock");
}

#[test]
fn perturbing_one_field_vector_breaks_the_comparison() {
    let state = cube_state(10);
    let base = CubeProjection::from(&state);
    let mut one = state.clone();
    one.fields.p[7] += 1e-12;
    assert_differs(&base, &one, "fields", "one producer cell");

    let mut two = state.clone();
    two.ecology.wood[7] += 1e-12;
    assert_differs(&base, &two, "ecology", "one wood cell");
}

#[test]
fn perturbing_one_extension_state_breaks_the_comparison() {
    let state = cube_state(10);
    let base = CubeProjection::from(&state);

    let mut care = state.clone();
    care.care.allowance_used += 1e-12;
    assert_differs(&base, &care, "care", "the care allowance");

    let mut hunters = state.clone();
    hunters.hunters.founders_placed += 1;
    assert_differs(&base, &hunters, "hunters", "the hunter extension");

    let mut correction = state.clone();
    correction.energy_correction.light_in += 1e-12;
    assert_differs(&base, &correction, "energy_correction", "the energy correction");
}

#[test]
fn perturbing_one_non_added_config_field_breaks_the_comparison() {
    let state = cube_state(10);
    let base = CubeProjection::from(&state);

    let mut growth = state.clone();
    growth.config.producer.growth += 1e-12;
    assert_differs(&base, &growth, "config.producer", "a producer rate");

    let mut seed = state.clone();
    seed.config.seed += 1;
    assert_differs(&base, &seed, "config.seed", "the seed");

    let mut blobs = state.clone();
    blobs.config.weather.blobs_per_channel += 1;
    assert_differs(&base, &blobs, "config.weather", "the weather config");
}

// ---------------------------------------------------------------------------
// Reading a schema 16 payload without being able to run it
// ---------------------------------------------------------------------------

/// §4's procedure, exercised on a **synthetic** schema 16 file built from a current state:
/// the product refuses it, the frozen mirror reads it, and the two projections agree.
///
/// This proves the mechanism — that `WorldConfigV16` really is `version` followed by the
/// fifteen blocks the projection keeps, so nothing but the three excluded fields can differ.
/// It is *not* the behavioural fixture: that one has to be a real snapshot taken from a
/// `main` build before FW-1, which FW-2 owns capturing and this test cannot fabricate.
#[test]
fn a_schema_16_payload_is_refused_by_the_product_and_read_by_the_mirror() {
    let state = cube_state(10);
    let mirror = WorldStateV16 {
        config: WorldConfigV16 {
            version: CONFIG_VERSION_V16,
            rest: CubeProjection::from(&state).config,
        },
        tick: state.tick,
        fields: state.fields.clone(),
        weather: state.weather.clone(),
        organisms: state.organisms.clone(),
        births_total: state.births_total,
        deaths_total: state.deaths_total,
        cap_rejections_total: state.cap_rejections_total,
        external_material_in: state.external_material_in,
        light_in_total: state.light_in_total,
        heat_out_total: state.heat_out_total,
        rain_in_total: state.rain_in_total,
        evap_out_total: state.evap_out_total,
        care: state.care.clone(),
        energy_correction: state.energy_correction,
        hunters: state.hunters.clone(),
        quiet: state.quiet.clone(),
        apex_dormancy: state.apex_dormancy.clone(),
        apex_encounters: state.apex_encounters.clone(),
        neural: state.neural.clone(),
        ecology: state.ecology.clone(),
    };

    let payload = postcard::to_allocvec(&mirror).expect("encodable");
    let id = b"fw6-v16";
    let mut file = Vec::new();
    file.extend_from_slice(&MAGIC);
    file.extend_from_slice(&SCHEMA_V16.to_le_bytes());
    file.extend_from_slice(&(id.len() as u16).to_le_bytes());
    file.extend_from_slice(id);
    file.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    file.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    file.extend_from_slice(&payload);

    // The product refuses it, by name, and keeps refusing it.
    assert_eq!(decode_snapshot(&file), Err(SnapshotError::UnsupportedSchema(16)));

    // Only the comparator looks inside.
    let (meta, back) = decode_v16(&file).expect("the frozen mirror reads a schema 16 payload");
    assert_eq!(meta.schema, 16, "the fixture must report schema 16");
    assert_eq!(back.config.version, CONFIG_VERSION_V16, "…and config 8");
    assert_eq!(state.config.version, CONFIG_VERSION, "…while this build reports config 9");

    assert_eq!(
        CubeProjection::from(&back),
        CubeProjection::from(&state),
        "the two projections are the cube regression evidence"
    );
    assert_eq!(
        projection_hash(&CubeProjection::from(&back)),
        projection_hash(&CubeProjection::from(&state)),
        "and the one-line CI signal agrees"
    );
}
