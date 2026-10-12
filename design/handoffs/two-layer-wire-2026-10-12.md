---
status: open
date: 2026-10-12
owner: Opus (high), worktree `.claude/worktrees/two-layer-wire`, branch `two-layer-wire`
---

# TWO-LAYER (half 2): wire the brain to the animals, and clone the first menus

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first. It covers windows, the
5-minute limit, labd, builds, tests, knobs, saves, commits and the return format. Use graft
(`graft ask … --source`, `graft callers`) before grepping.

## Why

This is package 7 of `design/behaviour-ai-2026-10-11.md`, its second half. Read §0 row 3,
§10 (all) and §11.3, including the 2026-10-12 amendment there.

Two pieces are merged:
- **The core,** in `crates/cubarium-search/src/es/voxel/two_layer/`: the net, arbitration,
  the policy file, ES ranges, `BrainState`, and cloning on a synthetic teacher. Its
  handback: `design/handoffs/two-layer-core-2026-10-12.md` plus the commit message of
  cb6f2fd2.
- **The fauna seam,** from CONTRACTS (fauna schema 81):
  - `Controller::decide(obs, &DecisionInput) -> (Response, Option<Decision>)`,
    `controller.rs` ~180;
  - `Controller::memory`/`restore` with `BrainMemory { tag, bytes }`, ~188-258, already
    saved in the envelope;
  - the producer, `subjects::decision_prep`, ~390;
  - row keys, `subject_key`, ~100;
  - cloning records, `ActionTap::decisions(id)` → `TapDecision`, `lib.rs` ~4494, carrying
    the label before the hold and what was held.
  - Read `crates/cubarium-voxel-fauna/src/intent_tests.rs` first.

## Do

Commit after each numbered step.

1. **The same-sample readout** (§11.3, amended).
   - The bottom's action readout reads the new hidden state with the **newly settled**
     intent and subject, in the same sample. The recurrent update reads them from the next
     sample.
   - The top's label at t still never reaches the top's input at t: update the order test
     to say exactly that.
   - Plain files stay identical.
   - Rerun the synthetic cloning check, including the variant where modes act at once,
     which the plain GRU won before. Report both.
2. **The adapter.** `TwoLayerBrain` implements `Controller::decide` against
   `DecisionInput`:
   - map rows by `keys`, so the core's key-held subject finds its row;
   - `held_row`, `candidate_mask` and `early_end` map to the core's inputs;
   - `memory`/`restore` carry `BrainState` as `BrainMemory` bytes.
   - A save and load mid-hold continues identically in a fauna-level test.
3. **Loading.** Menu-lineage brains load through `BrainFile`. Wire CONTRACTS'
   `Manifest::digest_refusal(stored)` into the loader (`store.rs`), so an old menu-lineage
   policy is refused with a clear message.
4. **Knobs.** Mount the core's arbitration rows (`arbitration_rows(prefix)`) under
   `fauna.founders.*.brain` and regenerate `design/parameters.md`.
5. **Recording teacher streams for cloning** (§10.2, §11.3 "Cloning data"):
   - From heuristic-driven worlds, record `TapDecision` streams into the core's
     `TrajectoryRecord`, seedporter and lanternjaw first (their label sources are in
     §10.2).
   - **Alternative-mode examples.** Where a heuristic can be asked to run a given mode
     (add the smallest hook that does it), record staged samples in modes and at
     transitions the teacher wouldn't have chosen. List the modes that can't be forced, and
     why.
   - **Closed-loop correction.** A shadow teacher runs beside a brain-driven body and labels
     the states the learner actually reaches (its proposal before the hold, and its
     actions).
6. **Clone the seedporter and lanternjaw** through labd, with the core's `fit`.
   - Report per-intent agreement and per-mode action error on held-out streams, beside a
     plain GRU cloned on the same data.
   - Then one short closed-loop run of each clone in a live world (a few sim-minutes)
     against the heuristic on matched seeds: energy and deaths. This is a first read, not
     a verdict.
   - Batches go to labd. Hand back with the job ids if they run long.

## Boundaries

- **The trainers' scoring** (`live/trainer.rs` `pooled_scores`, the roster, the comparison
  switch, stage-1 wiring) is KIN-TRAIN's. It's running in parallel on branch `kin-train`.
  Don't edit those parts. If you need a hook there, name it in the handback.
- **The fauna crate:** CALLS-REST runs in parallel and owns perception, rest and calls.
  Your fauna-side changes stay small: the mode-forcing hook, the shadow teacher, the
  `brain` config table.
- **No loftstrider or Veilhound cloning yet.** Their intent layers come with LS-DEFEND and
  VH-TACTICS.

## Checks (written into tests as you go)

- the same-sample readout, the order and plain identity (step 1);
- the adapter round trip and save mid-hold (step 2);
- a forced mode yields that mode's actions from the teacher, and an unforced run is
  unchanged;
- the shadow teacher doesn't change the brain-driven body's behaviour.

## Return

The rules file's format, plus the cloning numbers (two-layer against plain GRU, per mode),
the closed-loop first read, job ids, and every departure from §10–§11.3, with why.
