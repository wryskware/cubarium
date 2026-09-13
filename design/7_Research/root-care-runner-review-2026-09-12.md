---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Integrator review of the in-progress care runner

Source inspected after the held-boundary loop was wired. Host worker owns fixes
and tests; these are existing persistence/presentation guarantees, not extra care
features. Some shutdown wiring may still be in progress.

## Existing history must not depend on enabling new input

`run_world_until` currently opens/replays the care journal only when `run.care`
is true. A crash can leave a snapshot at S and a durable accepted command at
B >= S. Restarting without `--care` then silently advances past B and checkpoints
the wrong history. Enabling care later refuses that command as older than the
snapshot, after the ecological history was already changed.

Disabling new care must not disable recovery of already accepted care. Validate
and replay an existing journal regardless of intake permission, with new requests
disabled when `--care` is absent. If this slice cannot support that split, refuse
startup clearly when pending history would otherwise be skipped; do not advance
or write a newer checkpoint. Test a pending feed and a pending rain restarted
without the flag, and compare their recovery to the enabled run (or assert the
explicit pre-write refusal).

## Hold presentation time too

The paced clock still cycles render fraction `f` from zero to one on each wall
tick while care holds the same last `RenderView`. Passing that cycling fraction
into `presenter.draw` repeatedly replays the last movement segment and clip
interval. A long fsync or failed-care hold therefore makes a stationary world
jitter back and forth at 20 Hz.

During a held boundary, draw the held world's endpoint at a fixed presentation
fraction (normally 1.0), or resend a deliberately frozen final frame. Continue
serving/submitting frames, but do not loop the old movement. On release the next
world view begins where the held one ended. Test a delayed acknowledgement with
a moving organism: held encoded frames must remain identical after reaching the
held endpoint, even as render sequence advances.

## Keep directory ownership through every writer's actual lifetime

Declaring the state lock before other locals is not sufficient by itself.
`Checkpointer::shutdown` still detaches after ten seconds; a library caller can
then return, release the local lock and start another owner while the detached
thread still writes. `JournalWorker` similarly has a plain JoinHandle; make sure
normal and early-error drop paths stop/join it or retain directory ownership
until it really exits, not only on the intended success path.

Use shared ownership of the same acquired lock handle inside persistent worker
threads (without reopening/relocking the inode), or guarantee joining on every
path. Preserve the lock during a checkpoint timeout if retaining bounded shutdown.
Verify a delayed writer outliving the runner cannot be overlapped by a second
launcher. This is specifically the contract's one-writer guarantee.

## Replay-test hook isolation

The new `tests/care_replay.rs` installs process-global journal hooks, but only
three of its eight tests take `hook_guard`. The other five also open journals
and can inherit or consume the delayed/failing hook under Rust's default
parallel test execution. A normal unlimited replay accidentally receiving an
uncertain write can hold indefinitely because it has no external stop signal.
Serialize every test that can open a journal against this global hook, or scope
the hook to a particular test runner/state directory. Use cleanup that uninstalls
the hook even after a test failure. Rerun the suite at default parallelism.

The automatic capture test also removes/writes a shared fixed
`/tmp/cubarium-care-captures` directory. Use the test's unique scratch root (or an
explicit opt-in capture path) so concurrent Cargo processes cannot erase each
other's captures. Root and the native worker may validate independently.
