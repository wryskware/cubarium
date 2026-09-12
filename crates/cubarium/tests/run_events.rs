//! `design/m2-world-spec.md` "Observer", life events: with `capacity.event_log` on, the
//! host appends one line per birth and per death, and those lines account for exactly
//! the births and deaths telemetry counted.

mod support;

use std::path::Path;

use support::{Scratch, run, telemetry_lines};

const EVENT_LOG: &str = "[capacity]\nevent_log = true\n";

fn lines(path: &Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("not JSON: {e}\n{l}")))
        .collect()
}

/// `slot:generation`, both plain integers.
fn assert_organism_id(value: &serde_json::Value, what: &str) {
    let id = value.as_str().unwrap_or_else(|| panic!("{what} is not a string: {value}"));
    let (slot, generation) = id.split_once(':').unwrap_or_else(|| panic!("{what} is not slot:generation: {id}"));
    assert!(slot.parse::<u32>().is_ok(), "{what} has a non-numeric slot: {id}");
    assert!(generation.parse::<u32>().is_ok(), "{what} has a non-numeric generation: {id}");
}

#[test]
fn the_event_log_accounts_for_every_birth_and_death_telemetry_counted() {
    let scratch = Scratch::new("events");
    let state = scratch.join("state");
    let config = scratch.write("events.toml", EVENT_LOG);

    let out = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "600", "--fresh",
        "--config", config.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(out.final_tick, 12_000);
    assert!(out.config.capacity.event_log);

    // Telemetry counters are per sample, so the run's totals are their sums.
    let samples = telemetry_lines(&state.join("telemetry.jsonl"));
    let sum = |field: &str| -> u64 {
        samples.iter().map(|s| s[field].as_u64().unwrap()).sum()
    };
    let births = sum("births");
    let deaths = sum("deaths_starvation") + sum("deaths_age") + sum("deaths_collapse");
    assert!(births > 0 && deaths > 0, "this run must have had life and death: {births}/{deaths}");

    let events = lines(&state.join("events.jsonl"));
    let mut logged_births = 0u64;
    let mut logged_deaths = 0u64;
    let mut last_tick = 0u64;
    for (i, event) in events.iter().enumerate() {
        let tick = event["tick"].as_u64().unwrap_or_else(|| panic!("event {i} has no tick"));
        assert!(tick >= last_tick, "event {i} goes back in time: {tick} after {last_tick}");
        assert!(tick <= out.final_tick, "event {i} is past the end of the run: {tick}");
        last_tick = tick;
        assert_organism_id(&event["id"], &format!("event {i}: id"));
        assert!(event["genome"].as_u64().is_some(), "event {i}: genome is not an unsigned integer");

        match event["kind"].as_str() {
            Some("birth") => {
                logged_births += 1;
                assert_organism_id(&event["parent"], &format!("event {i}: parent"));
                assert!(event["parent_age_ticks"].as_u64().is_some(), "event {i}: parent_age_ticks");
                let parent_births = event["parent_births"].as_u64().unwrap();
                assert!(parent_births >= 1, "event {i}: parent_births counts this birth");
                let origin = event["origin"].as_str().unwrap();
                assert_eq!(origin, origin.to_lowercase(), "event {i}: origin must be lowercase");
                assert!(event["age_ticks"].is_null(), "event {i}: a birth has no age_ticks");
            }
            Some("death") => {
                logged_deaths += 1;
                assert!(event["age_ticks"].as_u64().is_some(), "event {i}: age_ticks");
                assert!(event["births"].as_u64().is_some(), "event {i}: births");
                let cause = event["cause"].as_str().unwrap();
                assert_eq!(cause, cause.to_lowercase(), "event {i}: cause must be lowercase");
                assert!(
                    ["starvation", "age", "collapse"].contains(&cause),
                    "event {i}: unknown cause {cause}"
                );
                assert!(event["parent"].is_null(), "event {i}: a death has no parent");
            }
            other => panic!("event {i} has an unknown kind: {other:?}"),
        }
    }

    assert_eq!(logged_births, births, "one line per birth telemetry counted");
    assert_eq!(logged_deaths, deaths, "one line per death telemetry counted");
    assert_eq!(events.len() as u64, births + deaths, "the log holds nothing else");
}

#[test]
fn the_event_log_is_off_by_default_and_the_option_moves_it() {
    let scratch = Scratch::new("events-off");
    let state = scratch.join("state");
    let out = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "60", "--fresh",
        "--state", state.to_str().unwrap(),
    ]);
    assert!(!out.config.capacity.event_log, "the event log must default to off");
    assert!(!state.join("events.jsonl").exists(), "an off event log writes no file");

    // `--events` decides where the log lives, including a directory that does not exist
    // yet. (A short run may legitimately record nothing, so only the file is asserted;
    // the accounting test above is the one that reads lines.)
    let elsewhere = scratch.join("elsewhere").join("life.jsonl");
    let state2 = scratch.join("state2");
    let config = scratch.write("events.toml", EVENT_LOG);
    run(&[
        "--sink", "none", "--speed", "0", "--seconds", "120", "--fresh",
        "--config", config.to_str().unwrap(),
        "--state", state2.to_str().unwrap(),
        "--events", elsewhere.to_str().unwrap(),
    ]);
    assert!(elsewhere.exists(), "--events must move the log");
    assert!(!state2.join("events.jsonl").exists(), "the default path must stay untouched");
}

#[test]
fn a_resumed_run_appends_to_the_same_log() {
    let scratch = Scratch::new("events-resume");
    let state = scratch.join("state");
    let config = scratch.write("events.toml", EVENT_LOG);
    let args = [
        "--sink", "none", "--speed", "0", "--seconds", "120",
        "--config", config.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ];

    let first = run(&[&args[..], &["--fresh"]].concat());
    let before = lines(&state.join("events.jsonl"));
    let resumed = run(&args);
    assert_eq!(resumed.start_tick, first.final_tick);
    let after = lines(&state.join("events.jsonl"));

    assert!(after.len() > before.len(), "the resumed run must have appended events");
    assert_eq!(
        after[..before.len()],
        before[..],
        "a resumed run must not rewrite the lines already on disk"
    );
    assert!(
        after[before.len()..].iter().all(|e| e["tick"].as_u64().unwrap() > first.final_tick),
        "every appended event belongs to the resumed stretch"
    );
}
