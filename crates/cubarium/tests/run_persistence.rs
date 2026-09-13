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

/// This test used to assert the opposite: that a directory full of unloadable snapshots
/// silently produced a brand new world at tick 0. The care contract changes it (and the
/// shared-care handoff authorizes the change), because that behaviour is how a world gets
/// lost. The pruner ranks snapshots by tick with no notion of world identity, so tick 0
/// beside `world-999999.cubw` is deleted as fast as it is written, and the next resume
/// finds the damaged file again. A damaged world is an error; only an *empty* directory is
/// a new world.
#[test]
fn a_snapshot_that_is_not_a_snapshot_at_all_is_now_a_hard_error() {
    let scratch = Scratch::new("garbage");
    let state = scratch.join("state");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("world-999999.cubw"), b"not a snapshot").unwrap();
    std::fs::write(state.join("world-oops.cubw"), b"ignored entirely").unwrap();
    std::fs::write(state.join("tmp-5.cubw"), b"ignored entirely").unwrap();

    let args = parse(&[
        "--sink", "none", "--speed", "0", "--seconds", "1",
        "--state", state.to_str().unwrap(),
    ]);
    let err = format!("{:#}", cubarium::run_world(&args).unwrap_err());
    assert!(err.contains("world-999999.cubw"), "the error must name the file: {err}");
    assert!(err.contains("none of them loaded"), "{err}");
    assert!(err.contains("no new world is created here"), "{err}");
    // And nothing was written: the damaged file is still the only snapshot name, there is
    // no tick-0 snapshot beside it, and no log was opened.
    assert_eq!(snapshot_ticks(&state), vec![999_999], "a refused run creates no snapshot");
    assert!(!state.join("telemetry.jsonl").exists(), "a refused run opens no log");

    // A directory with names that do not parse as snapshots at all is still empty, and
    // still becomes a new world.
    let empty = scratch.join("nothing-recognizable");
    std::fs::create_dir_all(&empty).unwrap();
    std::fs::write(empty.join("world-oops.cubw"), b"not a snapshot name").unwrap();
    let out = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "1",
        "--state", empty.to_str().unwrap(),
    ]);
    assert_eq!(out.loaded_tick, None);
    assert_eq!(out.final_tick, 20);
}

#[test]
fn require_resume_refuses_an_empty_directory_and_accepts_a_real_one() {
    let scratch = Scratch::new("require-resume");
    let state = scratch.join("state");
    let args = parse(&[
        "--sink", "none", "--speed", "0", "--seconds", "1", "--require-resume",
        "--state", state.to_str().unwrap(),
    ]);
    let err = format!("{:#}", cubarium::run_world(&args).unwrap_err());
    assert!(err.contains("--require-resume"), "{err}");
    assert!(err.contains("no snapshot to resume from"), "{err}");
    assert!(snapshot_ticks(&state).is_empty(), "a refused run writes nothing");

    // Seed a world, then the same flag succeeds.
    let seeded = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "5", "--fresh",
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(seeded.final_tick, 100);
    let resumed = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "1", "--require-resume",
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(resumed.loaded_tick, Some(100));
    assert_eq!(resumed.final_tick, 120);
}

/// The `--fresh` defect the review traced in `state.rs::checkpoint_loop`: eight old
/// snapshots with higher ticks than anything a new world will reach, so the pruner deletes
/// the new world's checkpoints in favour of theirs. The guard must refuse *before* touching
/// a single byte.
#[test]
fn fresh_is_refused_in_an_occupied_directory_before_anything_changes() {
    let scratch = Scratch::new("fresh-guard");
    let state = scratch.join("state");
    std::fs::create_dir_all(&state).unwrap();

    // Eight higher-tick snapshots from some earlier world, plus the logs beside them.
    let mut before: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..8u64 {
        let tick = 400_000 + i * 1000;
        let name = format!("world-{tick}.cubw");
        let bytes = format!("old world snapshot {tick}").into_bytes();
        std::fs::write(state.join(&name), &bytes).unwrap();
        before.push((name, bytes));
    }
    for (name, body) in
        [("telemetry.jsonl", "{\"tick\":446277}\n"), ("events.jsonl", "{\"kind\":\"birth\"}\n")]
    {
        std::fs::write(state.join(name), body).unwrap();
        before.push((name.to_string(), body.as_bytes().to_vec()));
    }

    let args = parse(&[
        "--sink", "none", "--speed", "0", "--seconds", "1", "--fresh",
        "--state", state.to_str().unwrap(),
    ]);
    let err = format!("{:#}", cubarium::run_world(&args).unwrap_err());
    assert!(err.contains("--fresh refuses to start"), "{err}");
    assert!(err.contains("8 snapshots"), "the refusal counts what it found: {err}");
    assert!(err.contains("407000"), "the refusal names the newest tick: {err}");
    assert!(err.contains("Choose a new --state directory"), "{err}");

    // Nothing changed. Every pre-existing file is byte-identical, and the only new name is
    // the `.lock` — which the review requires to be taken *before* this check, so that two
    // launchers cannot race between checking and writing.
    for (name, bytes) in &before {
        assert_eq!(&std::fs::read(state.join(name)).unwrap(), bytes, "{name} was modified");
    }
    let mut names: Vec<String> = std::fs::read_dir(&state)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut expected: Vec<String> = before.iter().map(|(n, _)| n.clone()).collect();
    expected.push(".lock".to_string());
    expected.sort();
    assert_eq!(names, expected, "a refused --fresh created or removed files");
}

#[test]
fn fresh_into_a_new_directory_persists_and_resumes_its_own_tick() {
    let scratch = Scratch::new("fresh-new-dir");
    let state = scratch.join("brand-new");

    let first = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "10", "--fresh", "--seed", "11",
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(first.start_tick, 0);
    assert_eq!(first.final_tick, 200);
    assert!(snapshot_ticks(&state).contains(&200), "{:?}", snapshot_ticks(&state));

    // And the resume finds its *own* final tick, not some other world's.
    let second = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "10",
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(second.loaded_tick, Some(200));
    assert_eq!(second.final_tick, 400);
}

#[test]
fn a_care_journal_without_a_snapshot_refuses_rather_than_inventing_a_world() {
    let scratch = Scratch::new("journal-only");
    let state = scratch.join("state");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("care.jsonl"),
        "{\"rec\":\"epoch\",\"epoch\":\"abc\",\"build\":\"x\"}\n",
    )
    .unwrap();

    let args = parse(&[
        "--sink", "none", "--speed", "0", "--seconds", "1",
        "--state", state.to_str().unwrap(),
    ]);
    let err = format!("{:#}", cubarium::run_world(&args).unwrap_err());
    assert!(err.contains("care journal but no snapshot"), "{err}");
    assert!(snapshot_ticks(&state).is_empty());

    // `--fresh` is refused too: a non-empty journal is occupancy.
    let args = parse(&[
        "--sink", "none", "--speed", "0", "--seconds", "1", "--fresh",
        "--state", state.to_str().unwrap(),
    ]);
    let err = format!("{:#}", cubarium::run_world(&args).unwrap_err());
    assert!(err.contains("--fresh refuses to start"), "{err}");
    assert!(err.contains("care.jsonl"), "{err}");
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

/// Exactly one owner, and the lock reusable afterwards without deleting `.lock`.
///
/// Both launchers go through the library API from two threads, into one empty directory,
/// released together so they genuinely race. A pid file with a `/proc` liveness check
/// cannot pass this: both would see an absent owner and both would start writing.
#[test]
fn two_concurrent_launchers_into_one_empty_directory_leave_exactly_one_owner() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let scratch = Scratch::new("two-launchers");
    let state = scratch.join("state");
    std::fs::create_dir_all(&state).unwrap();

    // A barrier both threads wait on, so neither can finish before the other starts.
    let gate = Arc::new(std::sync::Barrier::new(2));
    let arrived = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let state = state.clone();
        let gate = Arc::clone(&gate);
        let arrived = Arc::clone(&arrived);
        handles.push(std::thread::spawn(move || {
            let args = parse(&[
                "--sink", "none", "--speed", "0", "--seconds", "30", "--fresh",
                "--state", state.to_str().unwrap(),
            ]);
            arrived.fetch_add(1, Ordering::SeqCst);
            gate.wait();
            cubarium::run_world(&args).map_err(|e| format!("{e:#}"))
        }));
    }
    let results: Vec<Result<cubarium::RunOutcome, String>> =
        handles.into_iter().map(|h| h.join().expect("a launcher thread")).collect();

    let owners = results.iter().filter(|r| r.is_ok()).count();
    let refusals: Vec<&String> = results.iter().filter_map(|r| r.as_ref().err()).collect();
    assert_eq!(owners, 1, "exactly one launcher may own the directory: {results:?}");
    assert_eq!(refusals.len(), 1);
    let refusal = refusals[0];
    assert!(
        refusal.contains("already in use") || refusal.contains("--fresh refuses to start"),
        "the loser must be refused by the lock or by the occupancy guard the winner created: \
         {refusal}"
    );
    if refusal.contains("already in use") {
        assert!(refusal.contains(state.to_str().unwrap()), "name the directory: {refusal}");
    }

    // The owner has exited, so the OS released the lock. The file is still there, and is
    // not deleted or replaced to make the directory usable again.
    let lock = state.join(".lock");
    assert!(lock.exists(), "the lock file must never be unlinked");
    let again = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "1",
        "--state", state.to_str().unwrap(),
    ]);
    assert!(again.final_tick > 0, "the lock is free once the owner exits, with no cleanup");
    assert!(lock.exists());
    // Content is diagnostic only; ownership came from the kernel, not from this text.
    let note = String::from_utf8(std::fs::read(&lock).unwrap()).unwrap();
    assert!(note.contains(&format!("pid {}", std::process::id())), "{note}");
}
