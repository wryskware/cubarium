//! `design/m2-world-spec.md` "Observer", life events: with `capacity.event_log` on, the
//! host appends one line per birth and per death, and those lines account for exactly
//! the births and deaths telemetry counted.

mod support;

use support::{Scratch, run};

const EVENT_LOG: &str = "[capacity]\nevent_log = true\n";

#[test]
fn the_event_log_is_off_by_default_and_the_option_moves_it() {
    let scratch = Scratch::new("events-off");
    let state = scratch.join("state");
    let out = run(&[
        "--sink",
        "none",
        "--speed",
        "0",
        "--seconds",
        "20",
        "--fresh",
        "--state",
        state.to_str().unwrap(),
    ]);
    assert!(
        !out.config.capacity.event_log,
        "the event log must default to off"
    );
    assert!(
        !state.join("events.jsonl").exists(),
        "an off event log writes no file"
    );

    // `--events` decides where the log lives, including a directory that does not exist
    // yet. (A short run may legitimately record nothing, so only the file is asserted;
    // the accounting test above is the one that reads lines.)
    let elsewhere = scratch.join("elsewhere").join("life.jsonl");
    let state2 = scratch.join("state2");
    let config = scratch.write("events.toml", EVENT_LOG);
    run(&[
        "--sink",
        "none",
        "--speed",
        "0",
        "--seconds",
        "40",
        "--fresh",
        "--config",
        config.to_str().unwrap(),
        "--state",
        state2.to_str().unwrap(),
        "--events",
        elsewhere.to_str().unwrap(),
    ]);
    assert!(elsewhere.exists(), "--events must move the log");
    assert!(
        !state2.join("events.jsonl").exists(),
        "the default path must stay untouched"
    );
}
