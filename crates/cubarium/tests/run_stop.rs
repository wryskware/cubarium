//! The clean-stop path the SIGINT handler drives: a flag set from another thread ends
//! the world loop between ticks, and the run still takes the ordinary shutdown path —
//! a final snapshot named for the tick the loop stopped at.
//!
//! The binary wires that same flag to SIGINT (`crates/cubarium/src/run.rs`); these tests
//! exercise everything below the signal itself, which a test process cannot raise
//! without killing the harness.

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use support::{Scratch, parse, snapshot_ticks};

/// Set `stop` after `delay` on another thread, so the run under test is what blocks.
fn stop_after(delay: Duration) -> (Arc<AtomicBool>, std::thread::JoinHandle<()>) {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let handle = std::thread::spawn(move || {
        std::thread::sleep(delay);
        flag.store(true, Ordering::SeqCst);
    });
    (stop, handle)
}

#[test]
fn a_stop_flag_ends_an_unlimited_run_with_a_final_snapshot() {
    let scratch = Scratch::new("stop-unlimited");
    let state = scratch.join("state");
    // No `--seconds`: only the flag can end this run.
    let args = parse(&["--sink", "none", "--speed", "0", "--fresh", "--state", state.to_str().unwrap()]);

    let (stop, setter) = stop_after(Duration::from_millis(250));
    let started = Instant::now();
    let outcome = cubarium::run_world_until(&args, &stop).expect("the run must stop cleanly");
    let elapsed = started.elapsed();
    setter.join().unwrap();

    assert!(elapsed < Duration::from_secs(30), "the loop must stop promptly: {elapsed:?}");
    assert!(outcome.final_tick > 0, "the world must have advanced before the stop");
    assert_eq!(outcome.start_tick, 0, "a --fresh run starts at tick 0");

    // The shutdown path ran: the final snapshot is named for the tick it stopped at.
    let ticks = snapshot_ticks(&state);
    assert!(
        ticks.contains(&outcome.final_tick),
        "no world-{}.cubw among {ticks:?}",
        outcome.final_tick
    );
    assert_eq!(ticks.first(), Some(&outcome.final_tick), "the final snapshot is the newest");
}

#[test]
fn a_stopped_run_resumes_from_the_tick_it_stopped_at() {
    let scratch = Scratch::new("stop-resume");
    let state = scratch.join("state");
    let args = parse(&["--sink", "none", "--speed", "0", "--fresh", "--state", state.to_str().unwrap()]);

    let (stop, setter) = stop_after(Duration::from_millis(250));
    let stopped = cubarium::run_world_until(&args, &stop).expect("the run must stop cleanly");
    setter.join().unwrap();

    // Resume (no `--fresh`) for a bounded slice: the world continues from the snapshot
    // the interrupted run wrote, not from an older checkpoint.
    let resumed = support::run(&[
        "--sink", "none", "--speed", "0", "--seconds", "1",
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(resumed.loaded_tick, Some(stopped.final_tick));
    assert_eq!(resumed.start_tick, stopped.final_tick);
    assert_eq!(resumed.final_tick, stopped.final_tick + 20);
}

#[test]
fn a_stop_flag_ends_a_paced_run_too() {
    let scratch = Scratch::new("stop-paced");
    let state = scratch.join("state");
    // `--speed 1` takes the clocked loop, the one a preview or shim run uses.
    let args = parse(&["--sink", "none", "--speed", "1", "--fresh", "--state", state.to_str().unwrap()]);

    let (stop, setter) = stop_after(Duration::from_millis(400));
    let started = Instant::now();
    let outcome = cubarium::run_world_until(&args, &stop).expect("the run must stop cleanly");
    let elapsed = started.elapsed();
    setter.join().unwrap();

    assert!(elapsed < Duration::from_secs(30), "the loop must stop promptly: {elapsed:?}");
    assert!(outcome.final_tick > 0, "the world must have advanced before the stop");
    assert!(snapshot_ticks(&state).contains(&outcome.final_tick));
}

#[test]
fn a_flag_already_set_stops_before_the_first_tick_but_still_checkpoints() {
    let scratch = Scratch::new("stop-immediate");
    let state = scratch.join("state");
    let args = parse(&["--sink", "none", "--speed", "0", "--fresh", "--state", state.to_str().unwrap()]);

    let stop = AtomicBool::new(true);
    let outcome = cubarium::run_world_until(&args, &stop).expect("the run must stop cleanly");
    assert_eq!(outcome.final_tick, 0, "no tick may run once the stop is asked for");
    assert_eq!(snapshot_ticks(&state), vec![0], "the clean stop still writes tick 0");
}
