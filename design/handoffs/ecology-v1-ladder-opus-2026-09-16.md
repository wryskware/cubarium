---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream I (Opus): the finer movement-price ladder, gated on the grazer, with pressure after depletion

Fable orchestrates. Step 2 of the reconciled next steps in
[the next-steps result](../7_Research/ecology-v1-next-steps-results-2026-09-16.md),
from Astra's [review](../7_Research/ecology-v1-next-steps-review-2026-09-16.md)
(findings 1–2, next steps item 2). You own this brief; Fable reviews once with
at most two repair cycles. Model: Opus 5, high reasoning effort (pre-registered
criteria and a per-cell trajectory record). Time target: one working session.

**You are in a separate git worktree.** `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`
on every cargo command (one cache; a spurious extern/doctest failure means
another worker was building — re-run). Touch only
`crates/cubarium-search/src/{calibrate,evaluate,movement,params}.rs`, new files
you create under `crates/cubarium-search/src/`, the CLI dispatch line,
`crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`. No
`crates/cubarium-core/` edits (three other workers own core files and `es/`,
`apex_audit.rs`). Commit on your worktree branch by path. Do not push, tag,
restart the cube, or touch `state/`, port 7393, the shim or the running
`cubarium`. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Read first

- `design/7_Research/ecology-v1-movement-2026-09-16.md` **as corrected**:
  items 3 and 4 of "What the price knob actually did", the census, "The next
  task this implies", and its pre-registration section (the form you repeat).
  Its 108 rows are `runs/ecology-v1-movement/matrix/evals.jsonl` (in the main
  checkout; `runs/` is git-ignored, copy what you need).
- Astra's finding 2: recovery was 13 crossings in 72 raised runs; why a
  depleted cell stays below 50 % of its opening foliage is unresolved among
  slow regrowth (`L·μ` marginal), consumers returning, and intrinsically poor
  cells; post-depletion visits and bites were not recorded. Finding 1: the
  gate must be the founder **grazer's** first completed brood; the glider
  bred at `fast-leaf` × 0.0018.
- E's ledger: `World::record_body_budgets(true)`, `World::body_budget(id)`,
  and the closed-record drain in `crates/cubarium-core/src/world/budget.rs`
  (read-only use from the search side gives net energy margin per body).
- `crates/cubarium-search/src/movement.rs` (F's recorder: revisit, residence,
  crossings), `evaluate.rs` (depletion probe: watched cells = opening
  foliage > 0, depleted below 25 % of opening, recovered above 50 %), the
  `--prices` axis in `calibrate.rs`. Per-cell light and moisture: find where
  the plant step reads `L` and `μ` (`graft ask "per-cell light and moisture
  used by foliage growth" --source`) and read them from the world state,
  read-only.

## Deliverables

1. **Pre-registration first** (committed before any row): arms, measures,
   and rules. Confirmation of a *viable spatial intervention* at a price
   requires **both** (i) at least one completed founder-grazer brood in ≥ 4 of
   6 seeds and (ii) range below 60 % of the 0.00036 arm; a price is
   *informative about recovery* if it additionally shows ≥ 1 recovery per run
   in ≥ 4 of 6 seeds. Refutation of the ladder is no price meeting (i) and
   (ii) together. Define "completed brood" from the census (a form-0 birth
   from a founder parent; say how you identify founder parentage, or use
   "any form-0 birth" and say so).
2. **New per-depleted-cell record**, search side: for every cell that crosses
   the depletion threshold: its `L·μ` (and whether it is above the
   contract's `(L·μ)_crit`), `P/P₀` sampled every 600 ticks after depletion,
   tick and stock at the last consumer visit before depletion, post-depletion
   visit count and served material (from body positions and, if available,
   the ledger's per-body served by cell — if per-cell served is not available
   without a core change, use visits and say so), first recovery tick and
   first re-depletion tick. Tests first, from the definitions, on hand-built
   samples.
3. **The ladder** (≤ 8 wall minutes of simulation, 8 workers):
   `organism.move_cost` ∈ {0.00036, 0.0006, 0.0009, 0.0012, 0.0018} × {baseline,
   `fast-leaf`} × the 6 training seeds × arm 0, horizon 180,000, ledger on so
   net energy margin per body is reported by form × diet bin. The 0.00036 and
   0.0018 rows must reproduce F's arm-0 rows by `final_state_hash`.
4. **The verdict** per configuration against the rules; then, for the depleted
   cells, the four-way reading Astra named: recovery after grazing stops with
   adequate `L·μ` (time-scale problem); continued bites (pressure); no regrowth
   after pressure stops at adequate `L·μ` (plant equation/parameters);
   marginal `L·μ` (poor cells). Give the fraction of depleted cells in each
   class per price.
5. **Result note** `design/7_Research/ecology-v1-ladder-2026-09-16.md`: the
   pre-registration verbatim, build, commits, commands, the ladder table, the
   verdict, the depleted-cell classification, the grazer's brood counts, net
   margin by form × diet bin, what this does not establish, the next task
   named (not launched). Propose no equation unless the ladder is refuted,
   and then only name it.
6. `cargo test -p cubarium-search` green; `graft build`; commit on the branch.

## Constraints

- No core change; no gate, candidate or existing output altered (add only).
  One knob, arm 0 only.
- Compute ≤ 8 wall minutes, ≤ 8 workers; `runs/ecology-v1-ladder/` ≤ 40 MiB.
- Ask no questions; record routine choices.

## Decision authority

Yours: measure definitions (stated), file layout, flag names. Fable's: core,
equations, gates.

## Verification

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-search
```

Fable re-runs one row per price and checks hashes; checks the two shared
prices against F's rows.

## Return format

The note, plus: branch and commits, test totals, the ladder table with
verdicts, the depleted-cell classification, wall time, usage, and the
evidence and reasoning behind each decision.

## Stop

Stop after the note and commit. Fable merges.
