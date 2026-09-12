//! The contract's persistence verification: a run interrupted and resumed matches an
//! uninterrupted run exactly, a corrupted newest snapshot is skipped for the previous
//! one, and the state directory keeps the newest eight snapshots.

mod support;

use support::{Scratch, parse, run, snapshot_ticks};

/// A world whose checkpoints land every 5 simulated seconds, so a short run still
/// produces many snapshots. Everything else is the built-in default.
const FAST_CHECKPOINTS: &str = "[capacity]\ncheckpoint_seconds = 5.0\n";

#[test]
fn an_interrupted_and_resumed_run_matches_an_uninterrupted_one() {
    let scratch = Scratch::new("resume");
    let whole = scratch.join("whole");
    let parts = scratch.join("parts");

    // The default `checkpoint_seconds` is 60, so tick 2400 (120 s) is a checkpoint
    // instant in both runs and the split lands exactly on a snapshot.
    let uninterrupted = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "240", "--fresh",
        "--state", whole.to_str().unwrap(),
    ]);
    assert_eq!(uninterrupted.final_tick, 4800);
    assert_eq!(uninterrupted.config.capacity.checkpoint_seconds, 60.0);

    let first_half = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "120", "--fresh",
        "--state", parts.to_str().unwrap(),
    ]);
    assert_eq!(first_half.final_tick, 2400);
    // The clean stop wrote the snapshot the resume needs.
    assert!(snapshot_ticks(&parts).contains(&2400));

    let second_half = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "120",
        "--state", parts.to_str().unwrap(),
    ]);
    assert_eq!(second_half.loaded_tick, Some(2400), "the resume must start from tick 2400");
    assert_eq!(second_half.start_tick, 2400);
    assert_eq!(second_half.final_tick, 4800);

    assert_eq!(
        second_half.state_hash, uninterrupted.state_hash,
        "a resumed world must be bit-identical to an uninterrupted one"
    );
    assert_eq!(second_half.population, uninterrupted.population);
}

#[test]
fn a_corrupted_newest_snapshot_is_skipped_for_the_previous_one() {
    let scratch = Scratch::new("corrupt");
    let state = scratch.join("state");
    let config = scratch.write("fast.toml", FAST_CHECKPOINTS);

    let seeded = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "20", "--fresh",
        "--config", config.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(seeded.final_tick, 400);
    let ticks = snapshot_ticks(&state);
    assert!(ticks.len() >= 2, "the test needs a previous snapshot: {ticks:?}");
    let (newest, previous) = (ticks[0], ticks[1]);
    assert_eq!(newest, 400);

    // Flip one byte of the payload: the CRC in the header no longer matches.
    let path = state.join(format!("world-{newest}.cubw"));
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    std::fs::write(&path, &bytes).unwrap();

    let resumed = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "1",
        "--config", config.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(
        resumed.loaded_tick,
        Some(previous),
        "the corrupt newest snapshot must be skipped for tick {previous}"
    );
    assert_eq!(resumed.final_tick, previous + 20);
}

#[test]
fn a_snapshot_that_is_not_a_snapshot_at_all_is_skipped_too() {
    let scratch = Scratch::new("garbage");
    let state = scratch.join("state");
    std::fs::create_dir_all(&state).unwrap();
    // Files that do not parse as `world-<tick>.cubw` are ignored; ones that do but hold
    // garbage are reported and skipped.
    std::fs::write(state.join("world-999999.cubw"), b"not a snapshot").unwrap();
    std::fs::write(state.join("world-oops.cubw"), b"ignored entirely").unwrap();
    std::fs::write(state.join("tmp-5.cubw"), b"ignored entirely").unwrap();

    let out = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "1",
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(out.loaded_tick, None, "no snapshot loaded, so a fresh world is created");
    assert_eq!(out.start_tick, 0);
    assert_eq!(out.final_tick, 20);
}

#[test]
fn the_state_directory_keeps_the_newest_eight_snapshots() {
    let scratch = Scratch::new("prune");
    let state = scratch.join("state");
    let config = scratch.write("fast.toml", FAST_CHECKPOINTS);

    // 100 simulated seconds at a 5 s checkpoint cadence: twenty checkpoint instants.
    let out = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "100", "--fresh",
        "--config", config.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(out.final_tick, 2000);
    assert_eq!(out.checkpoints_queued, 21, "twenty cadence checkpoints plus the final one");

    let ticks = snapshot_ticks(&state);
    assert_eq!(ticks.len(), cubarium::state::SNAPSHOT_KEEP, "kept: {ticks:?}");
    assert_eq!(ticks[0], out.final_tick, "the newest is never pruned");
    // Newest first, strictly decreasing, and every file still decodes.
    for pair in ticks.windows(2) {
        assert!(pair[0] > pair[1], "{ticks:?}");
    }
    for tick in &ticks {
        let bytes = std::fs::read(state.join(format!("world-{tick}.cubw"))).unwrap();
        cubarium_core::decode_snapshot(&bytes)
            .unwrap_or_else(|e| panic!("world-{tick}.cubw does not decode: {e}"));
    }
    // No temp file survives a clean run.
    let strays: Vec<String> = std::fs::read_dir(&state)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("tmp-"))
        .collect();
    assert!(strays.is_empty(), "{strays:?}");
}

#[test]
fn a_loaded_world_keeps_its_own_config_but_takes_operational_settings() {
    let scratch = Scratch::new("precedence");
    let state = scratch.join("state");

    // Create a world with a distinctive seed and growth rate.
    let original = scratch.write(
        "original.toml",
        "seed = 31337\n[producer]\ngrowth = 0.012\n[capacity]\ncheckpoint_seconds = 5.0\n",
    );
    let first = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "10", "--fresh",
        "--config", original.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(first.config.seed, 31337);
    assert_eq!(first.config.producer.growth, 0.012);

    // Resume with a config that disagrees about everything.
    let other = scratch.write(
        "other.toml",
        "seed = 1\n[producer]\ngrowth = 0.008\n[weather]\nmoving = false\n\
         [capacity]\nmax_organisms = 256\ncheckpoint_seconds = 5.0\ntelemetry_seconds = 1.0\n",
    );
    let resumed = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "10",
        "--config", other.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(resumed.loaded_tick, Some(200));
    // The world's own config wins...
    assert_eq!(resumed.config.seed, 31337);
    assert_eq!(resumed.config.producer.growth, 0.012);
    // ...except for `capacity` and `weather.moving`, which are operational.
    assert_eq!(resumed.config.capacity.max_organisms, 256);
    assert_eq!(resumed.config.capacity.telemetry_seconds, 1.0);
    assert!(!resumed.config.weather.moving);
}

#[test]
fn an_invalid_config_is_a_fatal_error_and_writes_no_state() {
    let scratch = Scratch::new("bad-config");
    let state = scratch.join("state");
    let bad = scratch.write("bad.toml", "[capacity]\nmax_organisms = 0\n");
    let run_args = parse(&[
        "--sink", "none", "--speed", "0", "--seconds", "1", "--fresh",
        "--config", bad.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    let err = cubarium::run_world(&run_args).unwrap_err().to_string();
    assert!(err.contains("max_organisms"), "{err}");
    assert!(snapshot_ticks(&state).is_empty());

    let unknown = scratch.write("unknown.toml", "seed = 1\nnonsense = true\n");
    let run_args = parse(&[
        "--sink", "none", "--speed", "0", "--seconds", "1", "--fresh",
        "--config", unknown.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    assert!(cubarium::run_world(&run_args).is_err(), "unknown fields are errors");
}

#[test]
fn speed_zero_with_a_visual_sink_is_refused_before_anything_is_touched() {
    let scratch = Scratch::new("speed-zero");
    let state = scratch.join("state");
    let run_args = parse(&[
        "--sink", "png", "--speed", "0", "--seconds", "1",
        "--state", state.to_str().unwrap(),
    ]);
    let err = cubarium::run_world(&run_args).unwrap_err().to_string();
    assert!(err.contains("headless"), "{err}");
    assert!(!state.exists(), "a refused run must not create the state directory");
}
