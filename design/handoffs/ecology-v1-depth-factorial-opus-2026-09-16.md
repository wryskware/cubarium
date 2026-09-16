---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream O (Opus): the depth-only factorial, counterbalanced

Fable orchestrates. Step 5 of the reconciled next steps in
[the round-2 result](../7_Research/ecology-v1-round2-results-2026-09-16.md),
from J's named next task and Astra's
[round-2 review](../7_Research/ecology-v1-round2-review-2026-09-16.md) (next
steps item 5). You own this brief; Fable reviews once with at most one repair
cycle. Model: Opus 5, medium reasoning effort (an existing harness gains one
factor; the design discipline is already written). Time target: half a session.

**You are in a separate git worktree.** `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`
on every cargo command; first `git log --oneline -1` and if the worktree is not
at the brief's commit, `git reset --hard <that commit>`. Shared-cache races:
`touch crates/cubarium-core/src/lib.rs` and rebuild; pin
`CUBARIUM_SEARCH_BUILD=<your commit>`. Touch only
`crates/cubarium-search/src/factorial.rs`, `crates/cubarium-search/tests/diet_factorial.rs`
(or a new test file), the CLI dispatch line if a flag is needed,
`design/7_Research/`, `runs/`. No core change. Commit on your worktree branch by
path. Do not push, tag, restart the cube, or touch `state/`, port 7393, the shim
or the running `cubarium`. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Read first

- `design/7_Research/ecology-v1-diet-factorial-2026-09-16.md` (J) **as
  corrected**, especially "Addendum after review: the counterbalanced arm",
  the verdict, "Reconciling with F", and the named next task (move `depth`
  and nothing else). `crates/cubarium-search/src/factorial.rs`: `Arm`,
  `Arm::ASwap`, `plan`, the eight placements, the scripted seam that
  switches clone reproduction off, the clone row. Tests in
  `crates/cubarium-search/tests/diet_factorial.rs`, including the one that
  pins `plan(Arm::A)` against `plan(Arm::ASwap)`.
- Astra's review: the world (seed) is the replicate, not clones sharing a
  world; treatments must be counterbalanced across cells; roster form
  bundles size, speed, swimming and metabolism, so "body" is not one thing.

## Deliverables

1. **The factor.** The skimmer genome with `depth` ∈ {0.10 (its roster value),
   0.55} crossed with `diet` ∈ {0.60, 0.85}, every other locus identical: four
   treatments. Two counterbalanced arms so that across the pair every one of
   the eight cells holds every treatment once (a 4 × 8 Latin-style assignment
   over two arms of eight slots; state it). Verify what `depth` does in the
   decode and the drives (`crates/cubarium-core/src/genome.rs`, `w_depth`) and
   say so; if 0.55 is outside the founder's bounds or means something other
   than "does not seek the wet floor", pick the value that does and say why.
   Tests first: the two arms are complements slot by slot on `depth` and
   `diet` with every other locus equal, and each treatment stands in every
   cell exactly once across the pair.
2. **The run** (≤ 3 wall minutes, 8 workers): both arms, the 4 training
   seeds, J's horizon and warm-up, ledger on, reproduction off for the clones
   as J did. The `diet` main effect at `depth` 0.10 must reproduce J's swapped
   pair qualitatively (state the numbers side by side).
3. **The verdict**, with the world as the replicate (per-seed medians and
   establishment counts, 4 of 4 agreement or not): does `depth` alone move
   the skimmer's establishment or its wet-probe fraction, at each diet? Does
   the diet effect depend on depth (interaction)? Report served / digestible /
   credited / billed by channel per treatment, wet-probe fraction, and the
   cell classes stood in. Say which of "depth couples the body to litter" /
   "depth is not the coupling" / "not resolvable at four seeds" the data
   support. No clone-level p-values.
4. **Result note** `design/7_Research/ecology-v1-depth-factorial-2026-09-16.md`:
   design with the assignment table, build, commits, commands, reproduction
   check against J, the treatment table, the verdict, what this does not
   establish, the next task named (not launched) — including whether the
   detrital-funding calibration question is now worth asking.
5. `cargo test -p cubarium-search` green; `graft build`; commit on the branch.

## Constraints

- No core change; no `γ` change; no reproduction for clones.
- Compute ≤ 3 wall minutes, ≤ 8 workers; `runs/ecology-v1-depth-factorial/`
  ≤ 10 MiB.
- Ask no questions; record routine choices.

## Decision authority

Yours: the second depth value (justified), arm names, the assignment table.
Fable's: core, equations.

## Verification

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-search
```

Fable re-runs both arms and checks hashes.

## Return format

The note, plus: branch and commits, test totals, the treatment table in
compact form with per-seed agreement, the verdict, wall time, usage, and the
evidence and reasoning behind each decision.

## Stop

Stop after the note and commit. Fable merges.
