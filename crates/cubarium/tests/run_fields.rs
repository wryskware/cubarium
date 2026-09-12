//! `design/m2-world-spec.md` "Observer", field dumps: `fields.jsonl` opens with a
//! neighbor header and then carries one 1,280-cell sample per `field_dump_seconds` of
//! simulated time, on the same absolute-tick cadence rule as telemetry.

mod support;

use std::collections::HashMap;
use std::path::Path;

use support::{Scratch, run, telemetry_lines};

/// Five faces of 16×16 cells.
const CELLS: usize = 1280;

/// Dumps every 60 simulated seconds; everything else is the built-in default.
const EVERY_MINUTE: &str = "[capacity]\nfield_dump_seconds = 60.0\n";

/// Every JSON line of a file, parsed.
fn lines(path: &Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("not JSON: {e}\n{l}")))
        .collect()
}

/// Population by tick from a telemetry file.
fn population_by_tick(path: &Path) -> HashMap<u64, u64> {
    telemetry_lines(path)
        .iter()
        .map(|s| (s["tick"].as_u64().unwrap(), s["population"].as_u64().unwrap()))
        .collect()
}

/// Check one dump line: the four fields are 1,280 finite numbers at four decimals, and
/// the per-cell organism counts sum to the population telemetry reported for that tick.
fn check_sample(sample: &serde_json::Value, population: Option<u64>) {
    let tick = sample["tick"].as_u64().unwrap();
    for field in ["n", "p", "d", "de"] {
        let values = sample[field]
            .as_array()
            .unwrap_or_else(|| panic!("tick {tick}: {field} is not an array"));
        assert_eq!(values.len(), CELLS, "tick {tick}: {field} is not one value per cell");
        for (i, v) in values.iter().enumerate() {
            let x = v
                .as_f64()
                .unwrap_or_else(|| panic!("tick {tick}: {field}[{i}] is not a number: {v}"));
            assert!(x.is_finite(), "tick {tick}: {field}[{i}] is {x}");
            let scaled = x * 1e4;
            assert!(
                (scaled - scaled.round()).abs() < 1e-6,
                "tick {tick}: {field}[{i}] = {x} is not rounded to four decimals"
            );
        }
    }
    let organisms = sample["organisms"].as_array().unwrap();
    assert_eq!(organisms.len(), CELLS, "tick {tick}: organisms is not one count per cell");
    let total: u64 = organisms.iter().map(|v| v.as_u64().unwrap()).sum();
    if let Some(population) = population {
        assert_eq!(total, population, "tick {tick}: per-cell counts must sum to the population");
    }
}

#[test]
fn a_dumping_run_writes_a_header_and_one_sample_per_interval() {
    let scratch = Scratch::new("fields");
    let state = scratch.join("state");
    let config = scratch.write("fields.toml", EVERY_MINUTE);

    let out = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "300", "--fresh",
        "--config", config.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(out.final_tick, 6000);
    assert_eq!(out.config.capacity.field_dump_seconds, 60.0);

    let dumps = lines(&state.join("fields.jsonl"));
    // The header, then the world's opening state and one sample per minute.
    assert_eq!(dumps.len(), 7, "expected a header and 6 samples, got {}", dumps.len());

    // The header names four neighbors per cell in `Edge` order, `null` at a rim.
    let cells = dumps[0]["cells"].as_array().expect("the first line must be the header");
    assert_eq!(cells.len(), CELLS);
    let mut rim = 0;
    for (i, cell) in cells.iter().enumerate() {
        let neighbors = cell.as_array().unwrap();
        assert_eq!(neighbors.len(), 4, "cell {i} does not have four edges");
        for n in neighbors {
            match n.as_u64() {
                None => {
                    assert!(n.is_null(), "cell {i}: a neighbor is neither an index nor null: {n}");
                    rim += 1;
                }
                Some(id) => assert!((id as usize) < CELLS, "cell {i}: neighbor {id} is out of range"),
            }
        }
        assert!(
            neighbors.iter().any(|n| !n.is_null()),
            "cell {i} is isolated; the header cannot be a usable graph"
        );
    }
    // The cube is open on one face, so some edges must be rim edges and most must not.
    assert!(rim > 0, "no rim edges at all");
    assert!(rim < CELLS, "too many rim edges: {rim}");

    let population = population_by_tick(&state.join("telemetry.jsonl"));
    for (i, sample) in dumps[1..].iter().enumerate() {
        let tick = sample["tick"].as_u64().unwrap();
        assert_eq!(tick, i as u64 * 1200, "sample {i} is not on the cadence");
        // Telemetry has no tick-0 sample to compare the opening dump against.
        check_sample(sample, population.get(&tick).copied());
    }
}

#[test]
fn a_resumed_run_appends_samples_without_a_second_header() {
    let scratch = Scratch::new("fields-resume");
    let state = scratch.join("state");
    let config = scratch.write("fields.toml", EVERY_MINUTE);
    let args = [
        "--sink", "none", "--speed", "0", "--seconds", "300",
        "--config", config.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ];

    let first = run(&[&args[..], &["--fresh"]].concat());
    assert_eq!(first.final_tick, 6000);
    let resumed = run(&args);
    assert_eq!(resumed.start_tick, 6000);
    assert_eq!(resumed.final_tick, 12_000);

    let dumps = lines(&state.join("fields.jsonl"));
    assert_eq!(dumps.len(), 12, "a header, 6 samples, then 5 more");
    assert!(dumps[0]["cells"].is_array(), "the header must be the first line");
    assert!(
        dumps[1..].iter().all(|d| d["cells"].is_null()),
        "a resumed run must not write a second header"
    );
    let ticks: Vec<u64> = dumps[1..].iter().map(|d| d["tick"].as_u64().unwrap()).collect();
    // Tick 6000 is written once, by the run that reached it: the resume starts after it.
    assert_eq!(ticks, vec![0, 1200, 2400, 3600, 4800, 6000, 7200, 8400, 9600, 10_800, 12_000]);
}

#[test]
fn field_dumps_are_off_by_default_and_leave_no_file() {
    let scratch = Scratch::new("fields-off");
    let state = scratch.join("state");
    let out = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "10", "--fresh",
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(out.config.capacity.field_dump_seconds, 0.0, "dumps must default to off");
    assert!(
        !state.join("fields.jsonl").exists(),
        "a run that dumps nothing must not create the file"
    );
}

#[test]
fn the_fields_option_moves_the_file() {
    let scratch = Scratch::new("fields-path");
    let state = scratch.join("state");
    let config = scratch.write("fields.toml", EVERY_MINUTE);
    let elsewhere = scratch.join("elsewhere").join("dump.jsonl");

    run(&[
        "--sink", "none", "--speed", "0", "--seconds", "60", "--fresh",
        "--config", config.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
        "--fields", elsewhere.to_str().unwrap(),
    ]);
    assert!(!state.join("fields.jsonl").exists(), "--fields must move the file");
    let dumps = lines(&elsewhere);
    assert_eq!(dumps.len(), 3, "a header and the ticks 0 and 1200 samples");
    assert_eq!(dumps[2]["tick"].as_u64(), Some(1200));
}
