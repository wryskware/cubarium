---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Paired capture recovery: bounded observer review

Reviewed the in-progress [harness](../../crates/cubarium/examples/hunter_compare.rs),
[spatial bridge](../../crates/cubarium/examples/hunter_compare/spatial.rs) and
[recovery observer](../../crates/cubarium/examples/hunter_compare/recovery.rs).
Lore's verified source spans confirmed the current core settlement contract;
the relevant implementation was then read directly. No simulation, renderer,
live/preview process or worker-owned source was changed.

## Sound integration properties

- Recovery centers on `ContactEvidence.prey_pos` recorded before removal, not
  the hunter root, grasp, previous render position or a reused prey slot. Capture
  and paid Attempt must agree on full hunter ID, counter, tick, prey ID, outcome
  and the entire evidence record. Every capture must match an ordinary predation
  death, and the reconstructed live full-ID set must equal the actual arena.
- The settled-key map retains only living full hunter IDs after validating the
  batch, with generation included. Repeating/regressing keys fails across ticks;
  a dead hunter's same-tick settlement can still be checked before pruning.
  Failed spatial validation does not commit the cloned counter map.
- Each seed steps all six independent worlds before the paired observation.
  Unequal state ticks fail. A capture batch is handed to local recovery BEFORE
  the same-tick cadence sample, so capture-boundary occupancy cannot enter its
  own six-sample prior reference. Counts and exposure forms use identical cells
  across arms and exclude hunters by explicit membership.
- Neighborhoods are breadth-first graph distance three, not chart-space circles:
  they cross real seams, join the three incident faces at a vertex and do not
  wrap across the open rim. Sorted unique cells support the form census's binary
  search. Tests check interior size 25, an ordinary rim size 16, seam/vertex
  face coverage and reciprocal neighborhoods.
- Local histories/windows are bounded and records stream to disk. Exposures are
  explicitly END-OF-STEP counts, not an instantaneous before/after kill census.
  Local milestones measure total prey; immediate form counts are not mislabeled
  per-form recovery. Treatment-selected locations and recurrent captures remain
  visible limitations rather than implied randomized independent samples.
- A failed arm stops the paired progression; summary fields distinguish actual
  closing state, last complete observer tick and potentially partial statistics.
  Local finalization receives the failure reason; top-level technical completion
  stays false. Initializer/observer/finalization failures preserve available
  evidence rather than turning a shorter run into a completed horizon. A partial
  stream written before a later failure is not independently valid merely because
  its JSON parses: retain its enclosing failed seed summary.

## Geometry-validation gap in the reviewed draft — resolved in `6bc725c`

`CaptureAudit::observe` calls `evidence.grants_capture()`. The current core method
only requires a present capture center and a stored measure whose `in_contact()`
is true. It does not recompute either field from the evidence positions.

Counterexample: take the genuine certain-capture fixture and change **both** its
matching Attempt and Capture `evidence.prey_pos` to a different canonical cell,
leaving their stored measure and centers untouched. Identity, canonicality,
evidence equality, live-prey absence and `grants_capture()` all still pass, and
the observer attributes recovery to the wrong cell. The existing malformed test
changes only the Capture copy, so equality rejection does not test this case.
This is a source-derived counterexample, not a claimed production capture bug;
the core geometry tests independently establish ordinary event construction.

Minimal strengthening: recompute `measure_contact` and `body_point` using the
recorded hunter ROOT, heading, scaled geometry, prey position and extent; require
agreement with the stored measurement and capture center. Validate finite input
geometry and canonical centers. Reuse the core root-chart helpers, never travel
a separate mouth anchor with rim reflection. Add a paired-copy tamper regression
and retain the genuine seam/rim capture cases. Otherwise explicitly describe
spatial validity as trusted core evidence, not independently validated geometry.
This finding was sent to root while its implementation remained active.
Root accepted the paired-copy counterexample and is adding the recomputation and
regression. This will establish internal evidence consistency using shared core
helpers, not independently prove those helpers' geometry implementation. The fix
was not yet reviewed or included in the passing counts below.

Follow-up: read the exact committed source at `6bc725c`. The fix checks the
reported scale against the admitted range, requires offsets/reach/query extent
to equal the immutable trial profile scaled by that value, validates finite
heading/prey extent, and recomputes contact measurement plus grasp and ingestion
centers from the recorded root basis. A stale cached measure can no longer
authorize a different reported prey cell. The new
`matching_but_corrupted_pose_records_do_not_authorize_a_wrong_recovery_cell`
regression changes both evidence copies by four pixels, confirms refusal, then
successfully submits the original batch: rejection preserved the dedup state.
The specific finding is closed. This is **shared-helper consistency verification**,
not an independent geometry algorithm or proof the event positions reproduce
unrecorded historical ground truth.

## Deduplication is not complete attempt telemetry

Do not replace the monotonic check with equality to persisted `attack_counter`:
the core increments at paid strike entry, while Attempt arrives at settlement;
death can interrupt an in-flight strike. The current map correctly supplies
bounded key deduplication, not proof every paid attempt eventually emitted a
record. Optional independent capture-total/predation-total delta reconciliation
would strengthen coverage beyond the existing life-event cross-check. Exact
all-attempt coverage would additionally need explicit interrupted-attempt semantics.

## Verification and limits

Ran `cargo test -p cubarium --test hunter_observers --example hunter_compare`:
**22 observer-target tests and 32 example tests passed**, no failures or ignored
tests. This includes real certain-capture evidence, malformed/missing/duplicate
records, nonmutation, graph neighborhoods, recovery ordering/bounds and partial
observer failure summaries. Root's additional paired-orchestration tests and
72 short arm runs were still being added/running and are not claimed here.
No long-horizon ecological success, reproduction-funding completeness or live
Lanternjaw deployment follows from these technical observer checks.

Follow-up test run: the same command now passes **22 observer-target tests and
36 example tests**, with no failures/ignored tests. Newly included scope:

- The paired-copy geometry corruption regression above.
- Genuine capture followed by checkpoint/reload in handling and 1200 ticks of
  continuation: hunter events match every tick and full-state hashes match every
  200 ticks despite observer reads at 20 versus 200 ticks. This tests nonmutation
  and continuation, not full paired-recovery restart support.
- A partly advanced six-arm seed is refused without advancing paired coverage;
  finalization truthfully marks the resulting statistics incomplete.
- A deliberately synthetic capture at census tick 200 checks streamed exposure
  form totals, full key serialization, strict-prior history (`pre_samples = 1`,
  not 2), insufficient-pre labeling and horizon closure at tick 400. This
  orchestration fixture is not mislabeled a real capture or confirmed recovery.

The harness and observer sources used in this rerun are identical to `6bc725c`,
checked with Git. The shared worktree already includes concurrent uncommitted
core post-settlement timing changes, so the run is current integration evidence,
not verification of an isolated `6bc725c` binary. Root's detached release build
at `/tmp/cubarium-hunter-spatial-frozen-DIZmNc` and its 72-arm smoke output were
not touched or claimed. No additional source-level blocker was found within
this bounded follow-up.
