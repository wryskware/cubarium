---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream P (Opus): the apex pursuit predicate, paired

Fable orchestrates. Item 1 of the reconciled next steps in
[the round-3 result](../7_Research/ecology-v1-round3-results-2026-09-16.md),
Astra's "single most informative cheap experiment now"
([round-3 review](../7_Research/ecology-v1-round3-review-2026-09-16.md)). You own
this brief; Fable reviews once with at most two repair cycles. Model: Opus 5,
high reasoning effort. Time target: half a session. **Worktree.** Run under `AGENTS.md` and `WORKING_POLICY.md`. Shared `target/`: set `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`; a build failing on a symbol the tests just used means another tree was building — `touch crates/cubarium-core/src/lib.rs` and rebuild; pin `CUBARIUM_SEARCH_BUILD=<your commit>` in recorded stamps. If you are in a worktree, first `git log --oneline -1` and `git reset --hard <the brief commit>` if HEAD differs. `runs/` is git-ignored: read retained rows from the main checkout and copy your outputs back there at the end. Commit by path only (never `git add -A`); leave `.claude/*`, `WORKING_POLICY.md`, `.agents/` and other uncommitted design documents alone. Do not push, tag, restart the cube, or touch `state/`, port 7393, the shim or the running `cubarium` (the cube is live for the owner). Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Tests are their own pass, written from the definitions before the implementation. Ask no questions; record routine choices in the note. Never change `WorldConfig`'s fields: adding one changes `calibrate::config_hash` for every existing TOML and breaks policy provenance and every retained row.

**Files you own:** the hunt-intent pass in `crates/cubarium-core/src/world/step.rs`
(the `inside` predicate region only; another worker owns `lifecycle.rs`, none
owns the plant region this round), `crates/cubarium-core/src/hunter.rs`,
`crates/cubarium-core/src/world/hunter.rs`, `crates/cubarium-core/tests/`,
`crates/cubarium-search/src/apex_audit.rs`, its CLI lines in `main.rs`,
`crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`.

## Read first

- [N's note](../7_Research/ecology-v1-apex-reach-2026-09-16.md) as corrected:
  the per-attempt strike record, the classification, the verdict naming the
  pursuit stopping predicate `inside` (`c.body.x < geometry.capture_offset_body.x
  + c.tolerance`, a one-sided forward half-space, held on 408 of 449 paid
  attempts, dropping the member to `rest_effort` and suppressing the strike
  boost) and its own comment saying it means the reach envelope as
  `in_contact` does; N's cautions (the same hold governs the stalk; an apex
  that closes spends more on motor). [K's note](../7_Research/ecology-v1-apex-eligibility-2026-09-16.md)
  for the ledger fields. Astra's review, P2 on N and next steps item 1: what to
  record and the confirmation / refutation rule; strike constants' adequacy
  for a *delivered* lunge is untested (1.25 s needed at the escape cap against
  a 1.0 s strike).

## Deliverables

1. **A paired variant of the predicate, opt-in, default unchanged.** A
   `World`-level switch (not a `WorldConfig` field) that makes the hunt-intent
   pass read `inside` as the reach envelope (`in_contact()`) instead of the
   forward half-space. Default off = byte-identical to today (state hash over
   9,000 ticks of a two-apex world). Tests first: off is identical; on, a
   hand-built prey ahead but outside reach no longer satisfies the hold and the
   burst is requested; a prey inside reach still holds under both.
2. **The pair** (≤ 4 wall minutes, 8 workers): K's/N's two-apex death arm
   (baseline and `fast-leaf`, 4 held-out seeds, introduce at 6,000, horizon
   180,000) with the predicate off and on, identical seeds and introductions,
   strike record and ledger on. Report per arm: held fraction, initial gap,
   relative closure during delivered bursts, realised translation and turn
   consumption, contact and capture class histogram, captures per life by
   initial-gap bin, phase occupancy (stalk, perched, handling), apex lifetime,
   and K's credited / billed / net energy. Also the prey side: prey deaths by
   predation, prey population.
3. **Verdict by Astra's rule:** confirmed if the corrected arm delivers the
   burst, closes the gap and raises contacts and captures; refuted if the held
   fraction falls but closure and contact do not improve. Then, separately,
   whether the strike constants are adequate once delivered (closure achieved
   versus the 1.25 s arithmetic), and whether the apex's ledger is better or
   worse. State what Wrysk would be approving if he accepted the correction:
   the one-line change of the predicate to its own comment, visible whenever
   an apex is spawned from the viewer. Do not change escape speed, sense
   radius, strike duration or the mating radius.
4. **Result note** `design/7_Research/ecology-v1-apex-predicate-2026-09-16.md`,
   tests green (`cargo test -p cubarium-core`, `-p cubarium-search`), `graft
   build`, commit on the branch. Storage `runs/ecology-v1-apex-predicate/`
   ≤ 20 MiB.

Return: branch and commits, test totals, the paired table, the verdict, the
adequacy reading, wall time, usage, evidence and reasoning behind each decision.
