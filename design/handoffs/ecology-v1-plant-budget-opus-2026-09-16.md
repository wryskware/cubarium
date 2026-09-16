---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream M (Opus): the plant budget of a depleted cell, measured without herbivores and with exact withdrawal

Fable orchestrates. Step 3 of the reconciled next steps in
[the round-2 result](../7_Research/ecology-v1-round2-results-2026-09-16.md),
from Astra's [round-2 review](../7_Research/ecology-v1-round2-review-2026-09-16.md)
(P2 on I, next steps item 3). You own this brief; Fable reviews once with at
most two repair cycles. Model: Opus 5, high reasoning effort (a core
diagnostic in the plant step, a matched design, and the answer gates a
contract decision). Time target: one working session.

**You are in a separate git worktree.** `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`
on every cargo command; first `git log --oneline -1` and if the worktree is not
at the brief's commit, `git reset --hard <that commit>` (two earlier worktrees
were handed out at an ancestor). Shared-cache races: a build failing on a
symbol the tests just used means another tree was building — `touch
crates/cubarium-core/src/lib.rs` and rebuild; pin
`CUBARIUM_SEARCH_BUILD=<your commit>` in recorded stamps. Touch only
`crates/cubarium-core/src/world/step.rs` (the plant and intake accounting, a
read-only per-cell diagnostic), `crates/cubarium-core/src/world/budget.rs` only
if the per-cell record must live beside the body ledger (say so),
`crates/cubarium-core/tests/`, `crates/cubarium-search/src/{depletion,calibrate,evaluate,movement}.rs`,
new files you create under `crates/cubarium-search/src/`, the CLI dispatch
line, `crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`. Other
workers own `neural/`, `es/`, `hunter.rs`/`world/hunter.rs`, `apex_audit.rs`,
`factorial.rs`, `lifecycle.rs`. Commit on your worktree branch by path. Do not
push, tag, restart the cube, or touch `state/`, port 7393, the shim or the
running `cubarium`. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Objective

I found that 971 of 1,355 counted depletion crossings were in cells never
observed to hold a prey body at a one-second probe, all in the dim band, and
read this as cells seeded at `0.4·P_cap` (§11) that cannot hold that foliage.
Astra: that is a leading hypothesis, because visits were sampled, bites were
attributed, and the critical `L·μ` was a static proxy. Measure the plant budget
directly, so the owner can decide what "depletion" means and whether §11's
seeding should change.

## Read first

- `design/7_Research/ecology-v1-ladder-2026-09-16.md` as corrected: the
  depletion probe (watched cells = opening foliage > 0; depleted below 25 % of
  opening; recovered above 50 %), the per-depleted-cell record
  (`crates/cubarium-search/src/depletion.rs`), the 60 rows
  `runs/ecology-v1-ladder/ladder/evals.jsonl` (main checkout; `runs/` is
  git-ignored), the derived `(L·μ)_crit` and its three permissive assumptions,
  the seeding reading and the nutrient correlate.
- Astra's round-2 review, P2 on I and next-steps item 3: what to record and
  the confirmation / refutation rule. Do not put a universal critical `L·μ`
  into anything; a documented analysis proxy at reference `N` is fine.
- `design/ecology-v1-contract.md` §4 (plant stocks `P`, `W`, `Q`, income,
  maintenance, dieback, the §4.4 emergency reflush), §11 (the arithmetic and
  `producer.initial_fraction`), §13.
- `crates/cubarium-core/src/world/step.rs`: the plant subphase (find it with
  `graft ask "where foliage income, maintenance and reflush are applied per
  cell" --source`) and the intake site where a mouth withdraws from a cell's
  `P`, `F`, `D`, `C` (the exact withdrawal you will record per cell);
  `crates/cubarium-core/src/world/lifecycle.rs:52-63` (with `founders.kinds`
  empty and `founders.count` 0 the world has no animals — verify).

## Deliverables

1. **Core: per-cell plant-budget record, read-only, opt-in, inert.** For every
   cell, accumulated per tick or per fixed interval (state which): actual
   effective light and `N` used by the plant step, `P` and `Q` (and `W`),
   gross foliage income, maintenance and dieback loss, reserve transfer into
   `P` (the §4.4 reflush), and **exact consumer withdrawal** from `P` (and from
   `F`, `D`, `C`) at the withdrawal site — not attributed from positions.
   Same inertness proof as E and H: state hash identical on and off over
   9,000 ticks; per-cell identity `ΔP = income − loss − withdrawal ± transfer`
   closes to 1e-9; not persisted; snapshot unchanged. Cost measured, < 3 % on,
   zero off. Tests first from these definitions.
2. **The counter split**, search side: every depletion crossing is classified
   "crossed with exact withdrawal since the last recovery" or "crossed
   without", replacing the probe-based "ever visited" as the primary split
   (keep the old field for comparison). Tests first on hand-built sequences.
3. **The matched runs** (≤ 8 wall minutes, 8 workers): the same six training
   seeds and both configurations (`baseline.toml`, `fast-leaf.toml` from
   `runs/ecology-v1-calibration/selected/`) at the shipped price, arm 0,
   horizon 180,000, in two arms: **herbivores absent** (no animals at tick 0;
   verify the founders switch) and **herbivores present** (I's control rows;
   these must reproduce I's arm-0 0.00036 `final_state_hash` with the record
   off, and with the record on). Record per cell as above; per depleted cell
   report the crossing tick, whether any exact withdrawal preceded it since
   the last recovery, the cumulative withdrawal, the measured plant budget
   over the 6,000 ticks before the crossing (income − loss), and the actual
   `L·μ` and `N` there.
4. **The verdict, by Astra's rule.** If the plant-only worlds cross at the
   same cells and times as the herbivore worlds and their measured plant
   budget is negative there, the over-seeding reading is confirmed. If the
   crossings disappear without herbivores, or the crossing cells' plant
   budget is positive, it is refuted, and the next look is at missed
   consumption or animal-mediated nutrient and light. Report the fraction of
   crossings in each class per configuration, and the overlap of crossing
   cells between the two arms per seed.
5. **What the owner would be deciding**, stated but not decided: if
   confirmed, the candidate changes are a plant-only warm-up before founding
   animals, or seeding each cell below its own measured local equilibrium;
   describe the visible consequence of each on the display (dim cells begin
   less lush and more heterogeneous instead of greening uniformly and fading)
   with one contact-sheet-free number per option (e.g. the opening foliage
   distribution). Do not change §11, `producer.initial_fraction`, or any
   equation.
6. **Result note** `design/7_Research/ecology-v1-plant-budget-2026-09-16.md`:
   build, commits, commands, the record's definitions with the contract
   sections they implement, the reproduction checks, the two arms' tables,
   the verdict, the owner's options with their visible consequence, what this
   does not establish, the next task named (not launched).
7. `cargo test -p cubarium-core`, `cargo test -p cubarium-search` green; `graft
   build`; commit on the branch.

## Constraints

- No equation, constant, snapshot or neural change; the record is inert.
- Compute ≤ 8 wall minutes, ≤ 8 workers; `runs/ecology-v1-plant-budget/` ≤
  60 MiB (per-cell records only for depleted cells plus a coarse all-cell
  summary; say what you kept).
- Ask no questions; record routine choices.

## Decision authority

Yours: record layout, interval, file format, flag names. Fable's: any core
change beyond the record, any equation. Wrysk's: §11 seeding, after this.

## Verification

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-core
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-search
```

Fable re-runs one seed of each arm and checks hashes and the crossing table.

## Return format

The note, plus: branch and commits, test totals, the two arms' tables in
compact form, the verdict with the overlap numbers, the record's cost, wall
time, usage, and the evidence and reasoning behind each decision.

## Stop

Stop after the note and commit. Fable merges.
