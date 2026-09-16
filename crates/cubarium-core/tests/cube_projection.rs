//! The cube regression evidence for the ring world: `CubeProjection` equality across the
//! schema 16 → 17 break (`design/flat-world-plan-2026-09-16.md` §4).
//!
//! Why this and not a hash. `ecology_hash` and `state_hash` both cover `config`, which gained
//! `topology` and `world_scale` and whose `version` went 8 → 9, so no hash of a cube world
//! can be equal before and after — and schema 17 *refuses* a schema 16 snapshot outright, so
//! the new build cannot even load the old run's output to try. The projection is the one
//! comparison that survives: `WorldState` verbatim with `config` minus exactly those three
//! fields, which are then asserted separately and explicitly so a silent version change and a
//! silent world change cannot cancel each other.
//!
//! The fixture is one snapshot from an unmodified `main` build; see
//! `tests/fixtures/cube-projection-provenance.md` for the commit, the generator and its
//! hashes. Its generator is `examples/export_cube_fixture.rs`, committed beside it, and the
//! constants below are that file's — they are duplicated rather than imported because the
//! example must stay droppable into a pre-change checkout that has none of this module.

use cubarium_core::snapshot::{
    CONFIG_VERSION_V16, CubeProjection, SCHEMA_VERSION, SnapshotError, decode_snapshot, decode_v16,
    encode_snapshot, first_difference, projection_hash,
};
use cubarium_core::{World, WorldConfig};
use cubarium_surface::{Scale, Topology};

/// `examples/export_cube_fixture.rs`'s `FIXTURE_TICKS`.
const FIXTURE_TICKS: u64 = 6000;
/// `examples/export_cube_fixture.rs`'s `FIXTURE_SEED`.
const FIXTURE_SEED: u64 = 20_260_916;

const FIXTURE: &[u8] = include_bytes!("fixtures/cube-projection-v16-2a1cedd.cubw");

/// Exactly the run the fixture's generator made, on this build.
fn rerun() -> World {
    let config = WorldConfig {
        seed: FIXTURE_SEED,
        ..WorldConfig::default()
    };
    let mut world = World::new(config).expect("the default cube world is valid");
    for _ in 0..FIXTURE_TICKS {
        world.step();
    }
    world
}

/// The claim. A cube world runs exactly as it did before the topology existed: same fields,
/// same weather, same organisms **including the slot allocator's free list**, same counters,
/// same extensions, same behaviour-bearing config.
#[test]
fn a_cube_world_is_unchanged_by_the_ring_work() {
    let (meta, before) = decode_v16(FIXTURE).expect("the fixture is a schema 16 snapshot");

    // The three excluded fields, asserted on their own rather than folded into the
    // comparison: the fixture is schema 16 / config 8, this build is schema 17 / config 9.
    assert_eq!(meta.schema, 16, "the fixture must be a pre-ring snapshot");
    assert_eq!(before.config.version, CONFIG_VERSION_V16);
    assert_eq!(CONFIG_VERSION_V16, 8);
    assert_eq!(SCHEMA_VERSION, 17);
    assert_eq!(cubarium_core::config::CONFIG_VERSION, 9);

    // The fixture is the run the constants above describe, not some other world.
    assert_eq!(before.tick, FIXTURE_TICKS);
    assert_eq!(before.config.rest.seed, FIXTURE_SEED);

    let world = rerun();
    assert_eq!(world.state.config.topology, Topology::Cube);
    assert_eq!(world.state.config.world_scale, Scale::ONE);
    assert_eq!(world.state.config.version, 9);

    let a = CubeProjection::from(&before);
    let b = CubeProjection::from(&world.state);
    assert_eq!(
        first_difference(&a, &b),
        None,
        "a cube world moved: the field above is the first one that differs"
    );
    assert_eq!(
        projection_hash(&a),
        projection_hash(&b),
        "the projection hashes differ although no field did"
    );

    // The run is worth comparing: it is not an empty world that trivially matches.
    assert_eq!(world.state.organisms.len(), 46);
    assert!(world.state.births_total > 0, "the run committed births");
}

/// The comparator must be able to fail. FW-6's `ring_schema17.rs` owns the full negative
/// battery (§4's six perturbations); this is the one that keeps *this* file honest.
#[test]
fn the_comparator_can_fail() {
    let (_, before) = decode_v16(FIXTURE).expect("the fixture decodes");
    let a = CubeProjection::from(&before);
    let mut perturbed = a.clone();
    perturbed.fields.n[7] += 1e-12;
    assert_eq!(first_difference(&a, &perturbed), Some("fields"));
    assert_ne!(projection_hash(&a), projection_hash(&perturbed));

    let mut config_moved = a.clone();
    config_moved.config.producer.growth *= 1.000_001;
    assert_eq!(
        first_difference(&a, &config_moved),
        Some("config.producer"),
        "a behaviour-bearing config field must not be invisible to the projection"
    );
}

/// The product still refuses a schema 16 world. Only the comparator looks inside one, and it
/// does so through the frozen mirror, never through `decode_snapshot`.
#[test]
fn the_product_refuses_the_fixture_it_compares_against() {
    assert_eq!(
        decode_snapshot(FIXTURE),
        Err(SnapshotError::UnsupportedSchema(16)),
        "schema 17 must refuse a schema 16 world by name, fixture or not"
    );
    // And `decode_v16` is the mirror image: it refuses anything that is not schema 16.
    let mine = encode_snapshot(&rerun().state, "b");
    assert_eq!(decode_v16(&mine), Err(SnapshotError::UnsupportedSchema(17)));
}

/// A schema 16 `WorldConfig` is `version` followed by the projection's fifteen blocks.
///
/// `WorldConfigV16` states that as a two-field struct, which is only sound because postcard
/// writes a struct as its fields concatenated with no framing. The fixture proves it end to
/// end; this proves the encoding rule on its own, so a future reader does not have to take
/// the layout on trust.
#[test]
fn a_v16_config_is_version_then_the_projection() {
    let (_, before) = decode_v16(FIXTURE).expect("the fixture decodes");
    let nested = postcard::to_allocvec(&before.config).expect("encodes");
    let mut flat = postcard::to_allocvec(&before.config.version).expect("encodes");
    flat.extend_from_slice(&postcard::to_allocvec(&before.config.rest).expect("encodes"));
    assert_eq!(nested, flat, "postcard framed a nested struct");
}

/// §5a's promise: **no draw changes**, so the weather stream is exactly the same object on a
/// ring as on a cube. The blob model lives on the unit sphere after `normalize_or`; the
/// topology never reaches it.
///
/// This is the parity claim stated directly rather than inferred from the projection, which
/// only covers the cube.
#[test]
fn weather_draws_identically_on_a_ring_and_on_a_cube() {
    let cube = WorldConfig {
        seed: 4242,
        ..WorldConfig::default()
    };
    let ring = WorldConfig {
        topology: Topology::Ring { w: 320, h: 180 },
        ..cube.clone()
    };
    let mut a = World::new(cube).expect("cube");
    let mut b = World::new(ring).expect("ring");
    assert_eq!(
        a.state.weather, b.state.weather,
        "the initial blobs consumed different draws"
    );
    // Past two simulated minutes, so the per-minute random walk has fired twice.
    for _ in 0..2500 {
        a.step();
        b.step();
    }
    assert_eq!(
        a.state.weather, b.state.weather,
        "the weather stream diverged between two topologies"
    );
    assert_eq!(a.state.weather.last_walk_minute, 2);
}

/// The one-line CI signal, pinned. Both builds' projections hash to this.
#[test]
fn the_projection_hash_is_pinned() {
    let (_, before) = decode_v16(FIXTURE).expect("the fixture decodes");
    let h = projection_hash(&CubeProjection::from(&before));
    assert_eq!(h, projection_hash(&CubeProjection::from(&rerun().state)));
    assert_eq!(h, PINNED_PROJECTION_HASH);
}

/// FNV-1a 64 over the postcard encoding of the projection of the fixture run.
const PINNED_PROJECTION_HASH: u64 = 10_304_345_502_826_573_087;
