//! Determinism and replay, from `design/m2-world-spec.md` ("Randomness", "Persistence",
//! "Observer": "fixed-seed replay hash of the state each 60 s") and the crate doc:
//! "`World::step` is a pure function of its inputs plus counter-based keyed draws".

use cubarium_core::rng::{Stream, draw, unit};
use cubarium_core::snapshot::state_hash;
use cubarium_core::{World, WorldConfig, decode_snapshot, encode_snapshot};

const STREAMS: [Stream; 5] =
    [Stream::Weather, Stream::OrganismTurn, Stream::Birth, Stream::Founders, Stream::Habitat];

fn hashes_at(config: WorldConfig, checkpoints: &[u64]) -> Vec<u64> {
    let mut world = World::new(config).expect("valid config");
    let mut out = Vec::new();
    let last = *checkpoints.last().expect("at least one checkpoint");
    for tick in 1..=last {
        world.step();
        if checkpoints.contains(&tick) {
            assert_eq!(world.tick(), tick, "world tick label");
            out.push(state_hash(&world.state));
        }
    }
    out
}

/// The same config replays bit-for-bit.
#[test]
fn the_same_config_replays_to_the_same_state_hash() {
    let checkpoints = [1u64, 100, 1000];
    let a = hashes_at(WorldConfig::default(), &checkpoints);
    let b = hashes_at(WorldConfig::default(), &checkpoints);
    assert_eq!(a, b, "two runs of the default config diverged");

    // A hash that never changes would make the test vacuous.
    assert_ne!(a[0], a[1], "the state hash did not change between ticks 1 and 100");
    assert_ne!(a[1], a[2], "the state hash did not change between ticks 100 and 1000");
}

/// Spec: every draw mixes the world seed, so different seeds are different worlds.
#[test]
fn different_seeds_diverge_by_tick_one_hundred() {
    let first = WorldConfig { seed: 1, ..WorldConfig::default() };
    let second = WorldConfig { seed: 2, ..WorldConfig::default() };

    let a = hashes_at(first, &[100]);
    let b = hashes_at(second, &[100]);
    assert_ne!(a, b, "seeds 1 and 2 produced identical states at tick 100");
}

/// Spec, "Persistence": a snapshot round trip plus `World::from_state` must resume the
/// same world, so the replay hash at tick 1000 is identical whether or not the run was
/// interrupted at tick 500.
#[test]
fn a_snapshot_resume_replays_exactly() {
    let mut original = World::new(WorldConfig::default()).expect("defaults are valid");
    for _ in 0..500 {
        original.step();
    }

    let bytes = encode_snapshot(&original.state, "test-build");
    let (meta, state) = decode_snapshot(&bytes).expect("a freshly encoded snapshot must decode");
    assert_eq!(meta.build_id, "test-build");
    assert_eq!(state, original.state, "the snapshot round trip changed the state");

    let mut resumed = World::from_state(state).expect("a valid state must rebuild");
    assert_eq!(resumed.tick(), 500);

    for _ in 0..500 {
        original.step();
        resumed.step();
    }
    assert_eq!(original.tick(), 1000);
    assert_eq!(
        state_hash(&resumed.state),
        state_hash(&original.state),
        "resumed run diverged from the uninterrupted run by tick 1000"
    );
    assert_eq!(resumed.state, original.state, "resumed state differs field-by-field");

    let from_original = original.telemetry();
    let from_resumed = resumed.telemetry();
    assert_eq!(from_original.population, from_resumed.population);
    assert_eq!(from_original.state_hash, from_resumed.state_hash);
    assert_eq!(original.state.births_total, resumed.state.births_total);
    assert_eq!(original.state.deaths_total, resumed.state.deaths_total);
    assert_eq!(original.state.cap_rejections_total, resumed.state.cap_rejections_total);
}

/// Spec, "Randomness": "Rendering and logging never draw." The render view and telemetry
/// are observers; sampling them cannot perturb the world.
#[test]
fn observing_the_world_never_perturbs_it() {
    let mut plain = World::new(WorldConfig::default()).expect("defaults are valid");
    let mut observed = World::new(WorldConfig::default()).expect("defaults are valid");

    for tick in 1..=500u64 {
        plain.step();
        observed.step();

        // Observe the second world repeatedly and out of any fixed pattern.
        let view = observed.render_view();
        assert_eq!(view.tick, tick, "render view tick label");
        let _ = observed.telemetry();
        let again = observed.render_view();
        assert_eq!(again, view, "two render views of the same tick differ");
        let _ = observed.telemetry();
        let _ = observed.render_view();

        assert_eq!(
            state_hash(&plain.state),
            state_hash(&observed.state),
            "observing the world changed it at tick {tick}"
        );
    }
}

/// Spec, "Randomness": `draw(stream, key, counter)` is a keyed, counter-based function;
/// changing any coordinate changes the result, and `unit` lands in `[0, 1)`.
#[test]
fn draws_are_keyed_by_stream_key_and_counter() {
    let seed = 0xC0FFEE_u64;
    let base = draw(seed, Stream::OrganismTurn, 5, 9);

    for stream in STREAMS {
        if stream != Stream::OrganismTurn {
            assert_ne!(base, draw(seed, stream, 5, 9), "stream {stream:?} collides");
        }
        // Reproducible for each stream in its own right.
        assert_eq!(draw(seed, stream, 5, 9), draw(seed, stream, 5, 9));
    }
    assert_ne!(base, draw(seed, Stream::OrganismTurn, 6, 9), "key change did not change the draw");
    assert_ne!(
        base,
        draw(seed, Stream::OrganismTurn, 5, 10),
        "counter change did not change the draw"
    );
    assert_ne!(base, draw(seed + 1, Stream::OrganismTurn, 5, 9), "seed change did not change it");

    // Dense block: no collisions across the three coordinates at once.
    let mut seen = std::collections::HashSet::new();
    for stream in STREAMS {
        for key in 0..32u64 {
            for counter in 0..32u64 {
                assert!(
                    seen.insert(draw(seed, stream, key, counter)),
                    "collision at {stream:?} key {key} counter {counter}"
                );
            }
        }
    }
}

proptest::proptest! {
    /// Spec: "`u64 → f64` in `[0,1)`".
    #[test]
    fn unit_is_always_in_the_half_open_unit_interval(
        seed in proptest::prelude::any::<u64>(),
        key in 0..10_000u64,
        counter in 0..10_000u64,
    ) {
        for stream in STREAMS {
            let u = unit(seed, stream, key, counter);
            proptest::prop_assert!(
                (0.0..1.0).contains(&u),
                "unit({seed}, {stream:?}, {key}, {counter}) = {u} is outside [0, 1)"
            );
        }
    }
}
