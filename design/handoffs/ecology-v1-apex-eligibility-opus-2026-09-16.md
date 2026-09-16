---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream K (Opus): why an introduced apex dies, and one eligibility probe

Fable orchestrates. Step 4 of the reconciled next steps in
[the next-steps result](../7_Research/ecology-v1-next-steps-results-2026-09-16.md),
from Astra's [review](../7_Research/ecology-v1-next-steps-review-2026-09-16.md)
(finding 4 apex paragraph, next steps item 4). You own this brief; Fable
reviews once with at most two repair cycles. Model: Opus 5, high reasoning
effort (the hunter lifecycle and the introduction door). Time target: one
working session.

**You are in a separate git worktree.** `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`
on every cargo command (spurious extern/doctest failures mean another worker
was building — re-run; `touch crates/cubarium-core/src/lib.rs` clears a stale
artifact). Touch only `crates/cubarium-core/src/world/hunter.rs` (the
introduction door only, see below), `crates/cubarium-core/src/hunter.rs` /
`encounter.rs` **read-only**, `crates/cubarium-core/tests/`,
`crates/cubarium-search/src/apex_audit.rs`, `crates/cubarium-search/tests/`,
`design/7_Research/`, `runs/`. Other workers own `step.rs`/`budget.rs`/`neural/`/
`es/`, `lifecycle.rs`, and `calibrate/evaluate/movement/params`. Commit on your
worktree branch by path. Do not push, tag, restart the cube, or touch `state/`,
port 7393, the shim or the running `cubarium`. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Read first

- `design/7_Research/ecology-v1-budget-2026-09-16.md` "The apex opportunity
  audit" and its verdict: `max_ready = 0` in 8/8 runs; two adults alive
  together 10,315–12,859 ticks, mature-and-perched together ~5,300 ticks;
  **zero candidate pairs formed** (they never sensed each other,
  `crates/cubarium-core/src/encounter.rs:465-475`); the oldest apex reached
  14,101 ticks against `reproduce_min_age_seconds` 1,200 s = 24,000 ticks;
  each adult died at 43–59 % of its minimum reproduction age against a
  7,200 s lifespan. Astra: "lowering age would not guarantee mating either";
  test one eligibility intervention at a time, already-age-eligible
  introduction first; the radius stays untouched.
- `crates/cubarium-core/src/world/hunter.rs` `introduce_hunters` /
  `derive_hunter_founder`, `FixedHunterProfile`, `HunterTarget`; the apex
  lifecycle (`HunterPhase`, `may_reproduce`, dormancy, strike / retreat /
  handling, injury) in `crates/cubarium-core/src/hunter.rs` and `world/step.rs:600-665`
  (read-only); E's ledger, which already closes for hunters
  (`World::record_body_budgets`, closed records with death cause).
- `design/ecology-v1-contract.md` §13 (apex), `design/7_Research/ecology-v1-calibration-2026-09-15.md`
  item 5 as corrected.

## Deliverables

1. **Death diagnosis** (≤ 4 wall minutes, 8 workers): the two-apex arm of
   `apex-audit` exactly as E ran it (baseline and `fast-leaf`, 4 held-out
   seeds, introduce at 6,000, horizon 180,000), ledger on for the apex
   members: per member, lifetime, death cause, intake and gut credit by prey
   kind, oxidation, upkeep, motor, combat and handling bills, terminal stores,
   phase occupancy (active / perched / dormant / handling) over life, attacks,
   captures, and the last 2,000 ticks' credit / bill. Answer: what kills an
   introduced adult near 11,000 ticks — starvation (prey too scarce or too
   fast), injury, dormancy accounting, or an age/lifespan rule — with the
   ledger rows that show it.
2. **One eligibility probe, already-age-eligible introduction:** extend
   `introduce_hunters` (or add a sibling `introduce_hunters_with_age`) so an
   introduced founder can be given an age at introduction, validated against
   the profile (≥ 0, ≤ lifespan), otherwise unchanged; tests first: a founder
   introduced at age ≥ `reproduce_min_age_seconds` passes `may_reproduce`'s age
   term immediately and fails nothing else it would not otherwise fail;
   introduction at age 0 is byte-identical to the current door (state hash).
   Then re-run the two-apex arm with both adults introduced at age
   `reproduce_min_age_seconds` (all else as E), same seeds and horizon, with
   the opportunity counters: simultaneously-ready ticks, candidate pairs,
   minimum ready-pair distance, first-failing-predicate histogram, matings,
   births, emergences. Interpret Astra's branches: readiness still never
   opens → stock / encounter readiness terms bind, not age; readiness opens
   but no candidate pair forms → sensing / meeting is next; candidate pairs
   form and fail distance → the radius has become a real choice (report the
   distances; do not change the radius).
3. **Result note** `design/7_Research/ecology-v1-apex-eligibility-2026-09-16.md`:
   build, commits, commands, the death table and diagnosis, the probe table
   and which branch it reached, what this does not establish (one
   intervention, four seeds, no profile constant changed for the display),
   the next task named (not launched). The display world's apex profile is
   unchanged by this work; say so.
4. `cargo test -p cubarium-core`, `cargo test -p cubarium-search` green;
   `graft build`; commit on the branch.

## Constraints

- No change to any apex constant, the mating radius, any equation, snapshot
  or neural; the door gains an optional age and nothing else.
- Compute ≤ 8 wall minutes, ≤ 8 workers; `runs/ecology-v1-apex/` ≤ 20 MiB.
- Ask no questions; record routine choices.

## Decision authority

Yours: how the age is passed, the ledger fields you report, JSON layout.
Fable's: any constant or equation; whether the age door ever reaches the
viewer's spawn control (it does not, here).

## Verification

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-core
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-search
```

Fable re-runs one seed of each stage and checks it reproduces.

## Return format

The note, plus: branch and commits, test totals, the death diagnosis in one
paragraph with its deciding rows, the probe's branch with its counters, wall
time, usage, and the evidence and reasoning behind each decision.

## Stop

Stop after the note and commit. Fable merges.
