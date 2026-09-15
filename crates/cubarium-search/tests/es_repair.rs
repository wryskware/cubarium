//! R2a repair cycle 1: the review's findings, as permanent regressions.
//!
//! These are the cases from `design/7_Research/assets/r2a-review-regressions.rs` and the
//! review's CLI reproduction, promoted here. The episode-level and export-level cases live
//! beside their own modules; the two that need a whole generation or a whole run are here,
//! because they exercise the command the learning campaign will actually invoke.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use cubarium_search::es::{self, tensor, trainer};
use cubarium_search::es::trainer::{Checkpoint, GenerationError};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// **Review finding 1**, at the generation level and with the review's own parameters: one
/// pair, one layout, 4,000 ticks, two workers, a 50 ms deadline.
///
/// Before the repair the deadline was read only when a job was *dequeued*, so both workers
/// entered their episodes inside the deadline and ran to completion — the review measured 235 ms
/// and an applied Adam step. Now the deadline lives in `Limits`, every episode reads it every
/// 128 ticks, and `run_generation` re-checks it before committing the update.
#[test]
fn a_deadline_that_passes_during_the_last_batch_cancels_without_an_update() {
    let layouts = es::training_layouts();
    let protocol = es::Protocol::new(1, 4_000, 20_260_915, &layouts[..1]);
    let mut theta = tensor::initial_center(protocol.train_seed);
    let original = theta.clone();
    let mut adam = es::Adam::new(es::PARAMS);
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let plan = es::Plan {
        layouts: &layouts[..1],
        horizon: 4_000,
        workers: 2,
        evaluate_center: false,
        deadline: Some(started + Duration::from_millis(50)),
    };

    let result = trainer::run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel);

    let err = result.expect_err("the 50 ms deadline must stop the generation");
    assert!(
        matches!(err, GenerationError::Cancelled(_)),
        "expected cancellation, got {err:?}"
    );
    assert_eq!(theta, original, "a cancelled generation must not move the centre");
    assert_eq!(adam.step, 0, "and must not advance the optimizer");
    assert!(
        started.elapsed() < Duration::from_millis(600),
        "it stopped promptly, not after the whole batch: {:?}",
        started.elapsed()
    );

    // The work it did is counted, separately from optimizer progress.
    let discarded = err.discarded();
    assert!(discarded.episodes_attempted > 0, "it attempted work");
    assert!(discarded.ticks_run > 0, "and simulated ticks, which the budget must see");
    assert!(
        discarded.episodes_completed < discarded.episodes_attempted
            || discarded.ticks_run < 2 * 4_000,
        "it did not quietly finish the whole generation"
    );
}

/// **Review finding 3**, through the command the CLI calls, with centre evaluation on and the
/// same output directory: two uninterrupted updates must cost exactly what one update plus a
/// resumed update costs, and the history must survive.
///
/// Before the repair the resumed run truncated `generations.jsonl`, re-evaluated the centre it
/// had already scored, and charged that evaluation again: 32 episodes / 1,280 ticks and centre
/// ids `[0, 1, 1, 2]` against the uninterrupted run's 28 / 1,120 and `[0, 1, 2]`.
#[test]
fn a_resumed_run_continues_the_same_run_rather_than_repeating_it() {
    let root = scratch("resume");
    let whole = root.join("whole");
    let part = root.join("part");

    let run = |out: &Path, generations: u64, resume: Option<PathBuf>, workers: usize| {
        es::commands::train(
            1,
            generations,
            40,
            workers,
            120,
            20_260_915,
            true,
            resume,
            false,
            out.to_path_buf(),
        )
        .expect("the run completes")
    };

    run(&whole, 2, None, 2);
    run(&part, 1, None, 2);
    run(&part, 1, Some(part.join("checkpoint.json")), 3);

    let load = |dir: &Path| -> Checkpoint {
        serde_json::from_str(&fs::read_to_string(dir.join("checkpoint.json")).expect("checkpoint"))
            .expect("parse")
    };
    let a = load(&whole);
    let b = load(&part);

    assert_eq!(a.theta, b.theta, "resume must reach the same centre");
    assert_eq!(a.adam, b.adam);
    assert_eq!(a.generation_completed, b.generation_completed);
    assert_eq!(a.episodes_run, 28, "the uninterrupted cost the review measured");
    assert_eq!(a.ticks_run, 1_120);
    assert_eq!(b.episodes_run, a.episodes_run, "resume must not pay for a repeated centre");
    assert_eq!(b.ticks_run, a.ticks_run);
    assert_eq!(a.center_generations(), vec![0, 1, 2]);
    assert_eq!(b.center_generations(), vec![0, 1, 2], "no duplicated centre");
    assert!(a.centers.iter().all(|c| c.score.is_some()));
    assert!(b.centers.iter().all(|c| c.score.is_some()));

    // The generation log is a history, not the last run's fragment.
    let generations = |dir: &Path| -> Vec<u64> {
        fs::read_to_string(dir.join("generations.jsonl"))
            .expect("log")
            .lines()
            .map(|l| {
                serde_json::from_str::<serde_json::Value>(l).expect("row")["generation"]
                    .as_u64()
                    .expect("generation")
            })
            .collect()
    };
    assert_eq!(generations(&whole), vec![0, 1]);
    assert_eq!(generations(&part), vec![0, 1], "resuming must not truncate the log");

    // Every centre this run passed through is recoverable, weight for weight, not only the
    // last — which is what lets the evaluation assignment select any of them.
    for record in &b.centers {
        let file: es::export::PolicyFile =
            serde_json::from_str(&fs::read_to_string(part.join(&record.file)).expect("centre"))
                .expect("parse");
        let policy = file.policy().expect("the core accepts it");
        let theta = tensor::flatten(&policy.weights);
        assert_eq!(
            cubarium_search::es::fixture::fnv1a(
                cubarium_search::es::bits::encode(&theta).as_bytes()
            ),
            record.weights_fnv1a,
            "centre {} does not match its recorded hash",
            record.generation
        );
        if record.generation == b.generation_completed {
            assert_eq!(theta, b.theta, "the last centre is the checkpoint's own");
        }
    }
    // And the earlier centre is genuinely a different set of weights.
    assert_ne!(b.centers[0].weights_fnv1a, b.centers[2].weights_fnv1a);
}

/// A fresh run must not silently discard a run that is already in the directory.
#[test]
fn a_fresh_run_refuses_to_overwrite_an_existing_one_unless_told_to() {
    let out = scratch("no-clobber");
    let go = |overwrite: bool| {
        es::commands::train(1, 1, 40, 2, 120, 20_260_915, false, None, overwrite, out.clone())
    };

    go(false).expect("the first run is fine");
    let err = go(false).expect_err("a second fresh run must be refused");
    let message = err.to_string();
    assert!(message.contains("--resume"), "the refusal names the way forward: {message}");
    assert!(message.contains("--overwrite"), "{message}");

    // The refusal is not a lockout: an explicit overwrite starts over.
    go(true).expect("an explicit overwrite is allowed");
    let cp: Checkpoint =
        serde_json::from_str(&fs::read_to_string(out.join("checkpoint.json")).expect("checkpoint"))
            .expect("parse");
    assert_eq!(cp.generation_completed, 1);
    let rows = fs::read_to_string(out.join("generations.jsonl")).expect("log");
    assert_eq!(rows.lines().count(), 1, "an overwrite starts the history over, once");
}
