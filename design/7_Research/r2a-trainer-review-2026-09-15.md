---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# R2a trainer review — 2026-09-15

**Current status:** **R2a clears review at `dbb769e`** after repair cycle 2.
All 65 search tests and the unchanged independent seam regression pass. No remaining
blocking finding from this review. Earlier findings and repair requests below are
historical. This clears the bounded first-learning screen, not learned-behavior or
ecosystem-sustainability claims.

Reviewed `bf96ecb..80bf718` against the [R2a brief](../handoffs/r2a-fable-trainer-2026-09-15.md)
and [delivery report](r2a-trainer-result-2026-09-15.md). **Initial verdict: changes requested before the
learning campaign.** The optimizer and fixture controls stand; the findings concern
execution limits, compatibility, persistence and trustworthy measurements. No new
ecological or optimizer decision is needed for this repair.

## Findings

### 1. P1 — The wall-time cap is not checked inside running episodes

`crates/cubarium-search/src/es/trainer.rs:278–287, 357–366` checks the deadline before
starting a job. `episode.rs:228–231` checks only an atomic flag every 128 ticks;
nothing sets that flag as time passes inside the running jobs. If all workers are
in their final episodes, they finish after the deadline and the generation still
applies Adam at `trainer.rs:405–406`. Earlier batches can also run over the deadline
until another job is dequeued. `commands.rs:257` gives the smoke no deadline at all.

**Reproduced:** one pair, one layout, 4,000 ticks, two workers, deadline 50 ms.
`run_generation` returned success after **235 ms**, with Adam step **1**. Expected:
cancellation and unchanged weights/moments. The existing cancellation test starts
with cancellation already set, so it does not exercise expiration during a job.

Pass a deadline into the episode's periodic check or arrange a bounded shared timer;
check cancellation before committing the update. Apply the same mechanism to controls,
smoke and final-center evaluation. Count attempted/completed work from discarded
generations separately from optimizer progress: currently returning `Cancelled`
discards the work counts as well. Keep the existing budgets.

### 2. P1 — Policy import silently ignores the saved compatibility digest

`crates/cubarium-search/src/es/export.rs:54–62` checks the file schema and parameter
count, then calls `tensor::policy`, which creates a new policy stamped with the
**current** digest (`tensor.rs:83–87`). The saved `PolicyFile.policy_digest` is never
compared. A policy from a different observation/action convention with the same
weight count is therefore silently reinterpreted.

**Reproduced:** export a valid policy, flip one bit in `policy_digest`, call
`PolicyFile::policy()`: success. Reject the mismatch by name before rebuilding the
policy. The existing test changes the file-format string, not this compatibility digest.

### 3. P2 — Resuming erases generation history and repeats a center evaluation

`crates/cubarium-search/src/es/commands.rs:452` uses `File::create` on
`generations.jsonl`, including when resuming into the same output directory. This
truncates the previous generation records. The final-center evaluation at lines
527–550 is also repeated as the first center evaluation after resume; its score
is appended again and its work charged again.

**Reproduced through the CLI:** one pair, four layouts, 40 ticks, default center
evaluation. Two uninterrupted updates use **28 episodes / 1,120 ticks** with center
IDs `[0,1,2]`. One update followed by one resumed update uses **32 / 1,280**, with
IDs `[0,1,1,2]`. Weights and Adam match, but the resumed log contains only generation
`1`; generation `0` is gone. The report's claim that CLI resume preserves counted
work does not hold with the default center evaluation enabled.

Preserve validated history when resuming, refuse accidental fresh-run overwrite of
an existing run, and reuse an already recorded center evaluation. Test the real CLI
with center evaluation enabled and the same output directory. Preserve the last
completed checkpoint when replacing it (write then atomically rename). Ensure the
next evaluation assignment can recover its selected center's exact weights; the
current checkpoint retains only the latest center despite recording earlier scores.

### 4. P2 — Training records unmeasured movement and paid costs as zero

`crates/cubarium-search/src/es/commands.rs:478` selects `Detail::Score` for training;
`episode.rs:277–281, 293–309` only collects movement, upkeep and visitation in
`Detail::Full`. Both modes serialize the same nonoptional numeric fields. Thus the
learning command's episode records report zero paid upkeep/motion and zero visited
cells even while the animal is alive, moving and feeding.

**Reproduced:** the same initial policy and 40-tick episode has identical terminal
stores in both modes. Full diagnostics record **0.0124 e upkeep** and positive
motion cost; score mode records **0** for both. This would directly mislead the
foraging/funding review that the trainer is meant to support.

Collect the required diagnostics for campaign episodes; mark any deliberately omitted
measurement unavailable rather than zero. Also fix or explicitly qualify the full
accounting: seam-crossing ticks currently omit physical turn cost; death ticks exit
before collecting costs; intake material is not an energy-credit measurement.
`Episode::store_start` claims these columns close the energy box, but there are no
explicit assimilation/handling/other conversion terms to establish that identity.
Use actual settlement/ledger evidence for claims about paid costs and credits, and
keep those diagnostics out of fitness as planned.

### 5. P2 — Release rollouts do not check runtime invariants before accepting scores

`crates/cubarium-search/src/es/episode.rs:284–334` steps the world and returns a scoreable
episode without a runtime invariant check. `fixture.rs:225–230` checks only the initial
world. Core's end-of-step checks are inside `#[cfg(debug_assertions)]`
(`crates/cubarium-core/src/world/step.rs:2460–2493`), and the release profile does not
enable them. Checking policy weights for finiteness does not cover field, organism,
ledger or neural-state failures during a rollout.

This is a source-verified missing check, **not a claim that the current controls
produced invalid worlds**. Add a bounded runtime validation cadence plus terminal
validation, including the applicable neural-state checks. Fail the experiment with
job identity on invalid state; preserve ordinary biological death as a completed
episode. Include a release-mode fault-injection regression so the error path is
actually exercised. Re-measure episode throughput if the checks materially change it.

## Verification and limits

- `cargo test -p cubarium-search --release`: **55 passed**, including all 43 library
  tests and 12 existing M1 harness tests. No full workspace rerun in this review.
- Three independent focused regressions reproduced findings 1, 2 and 4. Source is
  [r2a-review-regressions.rs](assets/r2a-review-regressions.rs); copy to the test path
  specified in its header to run. All three are expected to fail at the reviewed commit.
- Tiny CLI continuation reproduction confirmed finding 3. Temporary run artifacts
  and the temporary test file were removed; only the small regression source remains.
- Re-ran all 12 fixture controls: **6.4 s** wall time, all verdicts reproduced.
  No-intake died at 371 s. Stationary grazers died at 470–594 s. All four paid mobile
  controls survived 1,800 s, ending with 1.397–3.000 usable energy.
- ES ranking, ties, sign/normalization, Adam ascent, initialization, tensor ordering
  and deterministic reduction have no blocking finding in this review. Retain them.
- No learning campaign, held-out policy evaluation, live policy attachment, reset,
  deployment, implementation repair or agent launch was performed here.
- Actual token/billed usage is unavailable; no context-counter estimate is reported.

## Repair handoff

**Apex scope:** R2a runs one ordinary grazer, with births disabled. It includes no
apex predator. Neural apex attachment is currently rejected by the R1a runtime;
the [R3 stage](../recurrent-organism-plan.md) must integrate apex neural control and
train/evaluate hunting, prey escape, paid reproduction and inheritance. Existing
legacy-apex capture regression tests are runtime correctness checks, not predator
training. R2a completion will not make the user's spawned apex neurally controlled.

Fable can use this document as **R2a repair cycle 1**: fix the five findings, retain
the frozen optimizer and fixtures, add the targeted regressions, and correct the
delivery report's claims. Run the search checks plus bounded CLI smoke/resume/error
checks; refresh the compute estimate only if the repaired runtime checks or required
diagnostics change it. Do not run the learning campaign or evaluate held-out policies.
Return the commit, test results and any remaining blocker for verification. This is
the one targeted review under the original brief, with at most two repair cycles.

## Repair cycle 1 verification

Verified the repair diff against the existing five findings. No new optimizer,
fixture or ecological review was opened. The unrelated float-parsing/build-stamp
commit `af6808e` arrived during verification; final test results below include it.

| Finding | Verification |
| --- | --- |
| 1. Deadline enforcement | Fixed: deadline travels into rollouts, cancellation is checked every 128 ticks, generation checks limits after dispatch, and discarded work is retained. The original 50 ms/4,000-tick case now passes its permanent regression. |
| 2. Compatibility digest | Fixed: import checks the saved digest before constructing the policy; foreign-digest regression passes. |
| 3. Resume | Original reproduction fixed: same-directory resume keeps both log rows, uses 28 episodes/1,120 ticks, records centers `[0,1,2]`, and reproduces weights/moments. Fresh-run overwrite refusal and retained center-file hashes pass. Checkpoints use write-then-rename. |
| 4. Diagnostics | Zero-filled score mode removed; all episodes collect diagnostics. Billed costs are distinguished from payments and the unsupported energy-balance claim is removed. Seam-turn qualification remains incorrect; see B below. |
| 5. Runtime invariants | Cadence and terminal validation now include world and neural state. Release fault-injection and ordinary-death tests pass. Final-center error handling remains incorrect; see A below. |

### A. P2 — Final-center invariant failures are still reported as successful timeouts

At `crates/cubarium-search/src/es/commands.rs:633–637`, the final-center evaluation's
`Err(e)` arm handles **both** `GenerationError::Cancelled` and `GenerationError::Invalid`
by setting `stop = "wall-time cap before the final centre evaluation"`. The function
then returns `Ok(())`. This drops the invalid job/detail and exits successfully even
though the experiment failed. The main generation loop already distinguishes these
variants correctly; the final evaluation needs equivalent handling.

This is verified directly from the error-handling branch, not a claim that ordinary
current policies triggered an invariant failure. Add a focused final-evaluation
error-path regression: `Invalid` preserves the job/detail, counts discarded work,
saves completed optimizer state and returns an error; cancellation remains a normal
budget stop. No broad exception-handling redesign is needed.

### B. P2 — Seam-crossing ticks omit physical turning but claim an exact motion price

At `crates/cubarium-search/src/es/episode.rs:447–452`, any changed face produces zero
turn. A tick can contain **both** a real body turn and a chart transport. Excluding
the coordinate-frame change is correct; excluding that tick's real turn is not.
The module still calls the reconstructed prices exact, and the report says excluding
seam-crossing rotation is correct. `turn_unmeasured_ticks` counts only death ticks,
so it also omits these unknown turns.

**Reproduced with a constant-action policy**, 40 ticks, one ordinary grazer,
identical heading and stores, starting near a seam versus in the face interior:

| Diagnostic | Interior | One seam crossing |
| --- | ---: | ---: |
| Turn sweep (rad) | 0.3431227672041359 | 0.3345446980240325 |
| Motor bill (e) | 0.003445594754758156 | 0.0034417346236271 |
| Unmeasured turn ticks | 0 | 0 |

The sweep deficit is exactly one tick's physical turn, approximately 0.008578 rad.
The independent [regression source](assets/r2a-repair1-seam-regression.rs) fails at
`af6808e`; its temporary test target was removed after the run.

Either measure turning with the heading transported into a common frame using the
existing geometry helpers, or explicitly count seam turns as unmeasured and label
the resulting sweep/motor price as partial. The latter is sufficient for this
bounded milestone. Correct the report and field/module documentation accordingly;
keep the score and simulation unchanged.

### Checks and next handoff

- Final `cargo test -p cubarium-search --release`: **63 passed** (48 library, three
  repair integration tests, 12 M1 harness tests). No full workspace rerun.
- The first invocation caught an intermediate mismatch while the separate
  float-parsing change was landing; the complete rerun at `af6808e` passes.
- Permanent repair tests exercise the original deadline/digest/diagnostic/resume
  cases and release invalid-state detection. The archived original reproduction
  source uses the old `Detail` API; the repaired API deliberately removes it.
- No controls/benchmark expansion, learning campaign, held-out evaluation, live
  change or implementation repair was performed in this verification. Actual token
  usage remains unavailable.

**Fable repair cycle 2:** address A and B only, run the corresponding focused checks
and the search suite, and update the delivery report. Do not reopen the optimizer,
fixtures, compute budget or apex scope. Return for verification before the learning
campaign. This is the second and final repair cycle within the current review budget.

## Repair cycle 2 verified — `dbb769e`

- Final-center evaluation now delegates to `finalize_center`: an invalid world
  preserves the job/detail, saves completed optimizer state and discarded work, and
  returns an error. Cancellation remains a normal budget stop. The permanent test
  injects invalid neural state through the actual dispatch and exercises both paths.
- Seam/death turns that cannot be reconstructed are counted as unmeasured and mark
  `motion_billed_partial`. Documentation correctly calls sweep/motor estimates
  lower bounds in those cases. The unchanged independent seam reproduction now
  passes; its temporary test target was removed afterwards.
- Independently ran `cargo test -p cubarium-search --release`: **65 passed**
  (49 library, four repair integration, 12 M1 harness), plus the independent seam
  regression. No full workspace rerun or benchmark expansion.
- The optimizer, fixtures, score and campaign budget are unchanged. The proposed
  first run remains at most 2,116 episodes / 76,176,000 ticks, eight workers and
  1,200 seconds. Actual campaign duration and outcome remain to be measured.
- No learning, held-out evaluation, live attachment or display restart occurred in
  this verification. Actual token usage is unavailable.

Next: [R2b first-learning screen](../handoffs/r2b-fable-first-learning-2026-09-15.md).
