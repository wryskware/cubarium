---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream N (Opus): why an apex strike ends out of reach

Fable orchestrates. Step 4 of the reconciled next steps in
[the round-2 result](../7_Research/ecology-v1-round2-results-2026-09-16.md),
from K's named next task and Astra's
[round-2 review](../7_Research/ecology-v1-round2-review-2026-09-16.md) (P3 on
K, next steps item 4). You own this brief; Fable reviews once with at most two
repair cycles. Model: Opus 5, high reasoning effort (strike kinematics across
the tick's phases). Time target: one working session.

**You are in a separate git worktree.** `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`
on every cargo command; first `git log --oneline -1` and if the worktree is not
at the brief's commit, `git reset --hard <that commit>`. Shared-cache races:
`touch crates/cubarium-core/src/lib.rs` and rebuild; pin
`CUBARIUM_SEARCH_BUILD=<your commit>` in recorded stamps. Touch only the hunter
strike path — `crates/cubarium-core/src/hunter.rs`, `crates/cubarium-core/src/world/hunter.rs`,
and in `crates/cubarium-core/src/world/step.rs` **only** the strike / pursuit /
handling region (another worker owns the plant and intake accounting region of
the same file; keep your hunk local and additive) — plus
`crates/cubarium-core/tests/`, `crates/cubarium-search/src/apex_audit.rs`,
`crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`. Commit on your
worktree branch by path. Do not push, tag, restart the cube, or touch `state/`,
port 7393, the shim or the running `cubarium`. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Read first

- `design/7_Research/ecology-v1-apex-eligibility-2026-09-16.md` (K): the
  death diagnosis (449 paid attempts, 402 `OutOfReach`, 31 missed, 15
  captures; 33 % of the bill on strikes), the per-member records, the
  `--founder-age-seconds` probe, and K's named next task. Rows in
  `runs/ecology-v1-apex/` (main checkout).
- Astra's review, next steps item 4: for every paid strike record separation
  at intent and at resolution, predator and prey displacement and heading,
  target identity continuity, whether the target crossed a cell or movement
  boundary; compare captures with `OutOfReach`. Failures that begin in range
  and resolve out implicate cadence or resolution; failures that begin out of
  range implicate target selection or pursuit. Age, reserve and radius stay
  unchanged.
- `design/ecology-v1-contract.md` §13 (apex: windup, strike, capture reach
  and offset, handling, retreat), `crates/cubarium-core/src/hunter.rs` (the
  profile: `capture_reach`, `capture_offset`, strike timing, escape multiple)
  and the strike phases in `world/step.rs` and `world/hunter.rs`; the prey's
  motor envelope (`crates/cubarium-core/src/motor.rs`, `requested_speed`,
  `omega_attain`).

## Deliverables

1. **Core: per-attempt strike record, read-only, opt-in, inert.** For every
   paid attempt: hunter id, target id, tick of intent (windup start), tick of
   strike start, tick of resolution; hunter and target position and heading at
   each of those; separation at each; the advertised reach at resolution
   (`|capture_offset| + capture_reach`); the target's realised speed over the
   windup and strike and its heading change; whether the target identity
   changed or the target died or crossed a face seam during the attempt;
   the outcome (`Captured`, `Missed`, `OutOfReach`, other) and the energy paid.
   Inertness by state hash on/off over 9,000 ticks of a two-apex world; not
   persisted. Tests first: a hand-built attempt with the prey stationary in
   reach captures and the record says so; one with the prey placed just beyond
   reach records `OutOfReach` with the separation the geometry predicts.
2. **The measurement** (≤ 6 wall minutes, 8 workers): the two-apex arm as K's
   death run (baseline and `fast-leaf`, 4 held-out seeds, introduce at 6,000,
   horizon 180,000), record on, plus the age-eligible probe arm if it adds
   attempts. Classify every attempt: began in reach and resolved out (cadence
   / resolution); began out of reach (target selection / pursuit never closed);
   prey outran (separation grew during the strike faster than the hunter
   closed); target lost (identity change, death, seam). Report per class the
   count, the separations at intent and resolution against reach, the prey's
   realised escape speed against the hunter's strike speed, and the energy
   spent per class. Compare the 15 captures with the 402 `OutOfReach` on the
   same fields.
3. **The verdict:** which of strike kinematics, escape multiple, or pursuit
   controller the attempts implicate, with the numbers; and what single
   constant or rule the evidence points at, **named, not changed** (the
   profile and radius are untouched here; the display's apex is unchanged).
4. **Result note** `design/7_Research/ecology-v1-apex-reach-2026-09-16.md`:
   build, commits, commands, the record's definitions, the classification
   table, the capture / out-of-reach comparison, the verdict, what this does
   not establish, the next task named.
5. `cargo test -p cubarium-core`, `cargo test -p cubarium-search` green; `graft
   build`; commit on the branch.

## Constraints

- No change to any apex constant, the radius, readiness, any equation,
  snapshot or neural. The record is inert and opt-in.
- Compute ≤ 6 wall minutes, ≤ 8 workers; `runs/ecology-v1-apex-reach/` ≤ 20 MiB.
- Ask no questions; record routine choices.

## Decision authority

Yours: record layout, classification thresholds (stated), flag names.
Fable's: any constant or equation.

## Verification

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-core
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-search
```

Fable re-runs one seed and checks the attempt table reproduces.

## Return format

The note, plus: branch and commits, test totals, the classification table in
compact form, the verdict with its deciding rows, wall time, usage, and the
evidence and reasoning behind each decision.

## Stop

Stop after the note and commit. Fable merges.
