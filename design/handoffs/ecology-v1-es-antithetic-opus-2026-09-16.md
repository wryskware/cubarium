---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream Q (Opus): where the ES search loses candidate variation

Fable orchestrates. Item 2 of the reconciled next steps in
[the round-3 result](../7_Research/ecology-v1-round3-results-2026-09-16.md),
from Astra's [round-3 review](../7_Research/ecology-v1-round3-review-2026-09-16.md)
(P1 on L, next steps item 2). You own this brief; Fable reviews once with at
most two repair cycles. Model: Opus 5, high reasoning effort. Time target: half
a session. Run on `main` in `/home/wrysk/wryskware/cubarium`. Run under `AGENTS.md` and `WORKING_POLICY.md`. Shared `target/`: set `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`; a build failing on a symbol the tests just used means another tree was building — `touch crates/cubarium-core/src/lib.rs` and rebuild; pin `CUBARIUM_SEARCH_BUILD=<your commit>` in recorded stamps. If you are in a worktree, first `git log --oneline -1` and `git reset --hard <the brief commit>` if HEAD differs. `runs/` is git-ignored: read retained rows from the main checkout and copy your outputs back there at the end. Commit by path only (never `git add -A`); leave `.claude/*`, `WORKING_POLICY.md`, `.agents/` and other uncommitted design documents alone. Do not push, tag, restart the cube, or touch `state/`, port 7393, the shim or the running `cubarium` (the cube is live for the owner). Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Tests are their own pass, written from the definitions before the implementation. Ask no questions; record routine choices in the note. Never change `WorldConfig`'s fields: adding one changes `calibrate::config_hash` for every existing TOML and breaks policy provenance and every retained row.

**Files you own:** `crates/cubarium-search/src/es/**` (new analysis module; no
change to the trainer's score, update, sampler or protocol),
`crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`. Other workers
own core files and the non-`es` search files this round.

## Read first

- [L's note](../7_Research/ecology-v1-score-checks-2026-09-16.md) as corrected
  and Astra's P1: the retained `runs/es-eco-v1-fastleaf/generations.jsonl`
  holds, per generation, the 32 ordered candidate scores and every
  candidate-layout episode (`GenerationReport`, `crates/cubarium-search/src/es/trainer.rs`);
  generation 9 spans 6,459–8,915 ticks (sd 643), r = 0.81 between candidate
  score and mean producer intake across the four training layouts, r = 0.54
  with mean ticks in the opening. The open question: where useful candidate
  variation is lost — in the four-layout minimum, the centred-rank reduction,
  the update, or the weights-to-residence mapping. [H's note](../7_Research/ecology-v1-intake-2026-09-16.md)
  for the residence trace (`World::trace_intake`, `es::intake`).
- The trainer: how antithetic pairs are formed (`θ ± σε`), how the centred
  rank turns 32 scores into an update, the step size, and how the centre's
  score is evaluated (`es/trainer.rs`, `es/episode.rs`).

## Deliverables

1. **The antithetic-pair reduction, no simulation:** for every generation and
   every plus/minus pair, the score difference, the per-layout score
   differences (is the four-layout *minimum* hiding gains on three layouts?),
   the intake and opening-residence differences from the retained episodes,
   and the pair's signed contribution to the centred-rank update. Report: how
   often a pair's better member also has better intake and residence; how
   much of the summed update is cancelled between pairs pointing in opposite
   directions; whether the minimum-over-layouts is dominated by one layout;
   and the centre's score against the best candidate's, generation by
   generation (is the update moving the centre toward the best candidates or
   past them?). Tests first on hand-built generation reports with known
   answers.
2. **Deadband occupancy under a σ-scale perturbation** (≤ 2 wall minutes):
   sample the frozen generation-9 weights ± σε for a few dozen ε, run each
   forward over L's recorded observation set (reset and carried state), and
   report the fraction of ticks where raw thrust and turn sit inside the
   adapter's ±0.05 deadbands, against the centre's.
3. **Only if (1) cannot say whether better candidates have better residence:**
   reconstruct generation 9's 32 candidates from the checkpoint (`θ ± σε`
   with the recorded seeds) and run H's residence trace on the four training
   layouts (≤ 3 wall minutes).
4. **Verdict by Astra's rule:** an optimiser/update problem is confirmed if
   individual perturbations with better residence and score cancel in the
   centred-rank gradient or are erased by the update; refuted if no candidate
   shows more on-food residence despite the score spread, which moves
   attention to parameterisation, recurrence, cadence or the adapter. Name the
   next change (σ, pairs, the rank reduction, the layout aggregation, the
   adapter) — named, not implemented; the score stays.
5. **Result note** `design/7_Research/ecology-v1-es-antithetic-2026-09-16.md`,
   tests green (`cargo test -p cubarium-search`), `graft build`, commit.
   Storage `runs/ecology-v1-es-antithetic/` ≤ 20 MiB.

Return: commits, test totals, the pair table in compact form, the deadband
numbers, the verdict, wall time, usage, evidence and reasoning behind each
decision.
