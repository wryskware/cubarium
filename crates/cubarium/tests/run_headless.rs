//! The contract's headless verification: `run --sink none --speed 0 --seconds 600`
//! completes with a living population and a tiny mass residual, its telemetry is
//! parseable JSON lines, and two runs with the same seed agree exactly.

mod support;

use support::{Scratch, run, snapshot_ticks, telemetry_lines};

/// `crates/cubarium/README.md`: the residual must stay below this.
const RESIDUAL_LIMIT: f64 = 1e-6;

#[test]
fn a_headless_ten_minute_run_keeps_a_population_and_a_closed_mass_budget() {
    let scratch = Scratch::new("headless-600");
    let state = scratch.join("state");
    let out = run(&[
        "--sink", "none",
        "--speed", "0",
        "--seconds", "600",
        "--fresh",
        "--state", state.to_str().unwrap(),
    ]);

    // 600 simulated seconds at 20 Hz, from a fresh world.
    assert_eq!(out.start_tick, 0);
    assert_eq!(out.final_tick, 12_000);
    assert!(out.population > 0, "the world must still be alive: {out:?}");
    assert!(
        out.mass_residual.abs() < RESIDUAL_LIMIT,
        "mass residual {} exceeds {RESIDUAL_LIMIT}",
        out.mass_residual
    );
    assert_eq!(out.frames, 0, "--speed 0 must not render");
    assert!(out.loaded_from.is_none(), "--fresh must not load a snapshot");

    // One telemetry sample every `telemetry_seconds` of simulated time, all parseable.
    let telemetry = state.join("telemetry.jsonl");
    let samples = telemetry_lines(&telemetry);
    let expected = 600.0 / out.config.capacity.telemetry_seconds;
    assert_eq!(samples.len() as f64, expected, "one sample per telemetry interval");
    assert_eq!(samples.len() as u64, out.telemetry_samples);

    // The cadence is on the absolute tick, and every documented field is present.
    let step = (out.config.capacity.telemetry_seconds * 20.0) as u64;
    for (i, s) in samples.iter().enumerate() {
        assert_eq!(s["tick"].as_u64().unwrap(), (i as u64 + 1) * step);
        for field in [
            "population", "births", "deaths_starvation", "deaths_age", "deaths_collapse",
            "escrows", "cap_rejections", "nutrient", "producer", "detritus",
            "organism_material", "organism_energy", "light_in", "heat_out", "mass_residual",
            "population_by_face", "occupied_cells", "travel_fallbacks", "travel_ties",
            "state_hash",
        ] {
            assert!(!s[field].is_null(), "sample {i} is missing {field}");
        }
        assert!(
            s["mass_residual"].as_f64().unwrap().abs() < RESIDUAL_LIMIT,
            "sample {i} residual {}",
            s["mass_residual"]
        );
    }

    let last = samples.last().unwrap();
    assert_eq!(last["tick"].as_u64().unwrap(), out.final_tick);
    assert_eq!(last["state_hash"].as_u64().unwrap(), out.state_hash);
    assert!(last["population"].as_u64().unwrap() > 0);

    // The run left a resumable world behind.
    let ticks = snapshot_ticks(&state);
    assert_eq!(ticks.first().copied(), Some(out.final_tick));
}

#[test]
fn two_runs_with_the_same_seed_agree_exactly() {
    let scratch = Scratch::new("same-seed");
    let a = scratch.join("a");
    let b = scratch.join("b");
    let first = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "120", "--fresh",
        "--seed", "424242", "--state", a.to_str().unwrap(),
    ]);
    let second = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "120", "--fresh",
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
