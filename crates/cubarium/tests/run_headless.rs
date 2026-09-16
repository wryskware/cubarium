//! The contract's headless verification: `run --sink none --speed 0 --seconds 600`
//! completes with a living population and a tiny mass residual, its telemetry is
//! parseable JSON lines, and two runs with the same seed agree exactly.

mod support;

use support::{Scratch, run, telemetry_lines};

#[test]
fn two_runs_with_the_same_seed_agree_exactly() {
    let scratch = Scratch::new("same-seed");
    let a = scratch.join("a");
    let b = scratch.join("b");
    let first = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "40", "--fresh",
        "--seed", "424242", "--state", a.to_str().unwrap(),
    ]);
    let second = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "40", "--fresh",
        "--seed", "424242", "--state", b.to_str().unwrap(),
    ]);

    assert_eq!(first.config.seed, 424_242, "--seed overrides the config for a fresh world");
    assert_eq!(first.state_hash, second.state_hash);
    assert_eq!(first.final_tick, second.final_tick);
    assert_eq!(first.population, second.population);

    // Telemetry is identical line for line, not merely equal at the end.
    let ta = telemetry_lines(&a.join("telemetry.jsonl"));
    let tb = telemetry_lines(&b.join("telemetry.jsonl"));
    assert_eq!(ta, tb);
    assert!(!ta.is_empty());
}

#[test]
fn a_different_seed_gives_a_different_world() {
    let scratch = Scratch::new("other-seed");
    let a = scratch.join("a");
    let b = scratch.join("b");
    let first = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "30", "--fresh",
        "--seed", "1", "--state", a.to_str().unwrap(),
    ]);
    let second = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "30", "--fresh",
        "--seed", "2", "--state", b.to_str().unwrap(),
    ]);
    assert_ne!(first.state_hash, second.state_hash);
}

#[test]
fn telemetry_goes_where_the_option_says_and_is_appended_across_runs() {
    let scratch = Scratch::new("telemetry-path");
    let log = scratch.join("elsewhere.jsonl");
    // A state directory each: `--fresh` is now refused in a directory that already holds a
    // world, so "two runs" means two worlds. The point of the test is the *telemetry* path,
    // which both runs share.
    let mut states = Vec::new();
    for i in 0..2 {
        let state = scratch.join(&format!("state-{i}"));
        run(&[
            "--sink", "none", "--speed", "0", "--seconds", "10", "--fresh",
            "--state", state.to_str().unwrap(),
            "--telemetry", log.to_str().unwrap(),
        ]);
        assert!(!state.join("telemetry.jsonl").exists(), "the default path must not be used");
        states.push(state);
    }
    // Two identical runs of 10 s at a 5 s cadence: four samples, appended not truncated.
    assert_eq!(telemetry_lines(&log).len(), 4);
}
