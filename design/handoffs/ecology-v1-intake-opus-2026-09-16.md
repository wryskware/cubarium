---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream H (Opus): why generation 9 does not eat — the per-tick intake diagnostic

Fable orchestrates. This is step 1 of the reconciled next steps in
[the next-steps result](../7_Research/ecology-v1-next-steps-results-2026-09-16.md)
("Next recommendation"), Astra's "single most informative cheap experiment now"
([review](../7_Research/ecology-v1-next-steps-review-2026-09-16.md), next steps
item 1). You own this brief; Fable reviews once with at most two repair cycles.
Model: Opus 5, high reasoning effort (the tick's intake path, the neural action
adapter and the ES fixtures are coupled, and the answer decides the next design
move). Time target: one working session. Run in `/home/wrysk/wryskware/cubarium`
on `main` under `AGENTS.md` and `WORKING_POLICY.md`.

**Three other workers are active in worktrees**: one owns
`crates/cubarium-search/src/{calibrate,evaluate,movement,params}.rs`; one owns
`crates/cubarium-core/src/world/lifecycle.rs` and a new founding door plus a new
search file; one owns `crates/cubarium-core/src/world/hunter.rs` and
`crates/cubarium-search/src/apex_audit.rs`. Do not edit those. Your files:
`crates/cubarium-core/src/world/{step,budget}.rs`, `crates/cubarium-core/src/neural/**`
(read-only diagnostics only), `crates/cubarium-search/src/es/**`,
`crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`. One shared
`target/`: a spurious doctest/extern failure means another worker was building;
re-run (`touch crates/cubarium-core/src/lib.rs` clears a stale artifact). Commit
on `main` by path only; leave `.claude/*`, `WORKING_POLICY.md`, `.agents/` and
other uncommitted design documents alone. Do not push, tag, restart the cube, or
touch `state/`, port 7393, the shim or the running `cubarium` (the cube is live
for the owner). Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Objective

Separate, with per-tick records, three explanations for generation 9 taking in
fifteen times less material than the scripted mobile control while travelling
more: (a) its mouth efforts are off when it stands on edible food (action /
score problem); (b) it is rarely on edible food at all (observation /
navigation problem); (c) efforts are on and food is present but the served
bite is clamped (action adapter / settlement problem).

## Read first

- `design/7_Research/ecology-v1-budget-2026-09-16.md`: the ledger
  (`World::record_body_budgets`, `body_budget`, `crates/cubarium-core/src/world/budget.rs`),
  the feasibility table, "Astra's three branches" (as corrected), the served
  channel split. Generation 9 is
  `runs/es-eco-v1-fastleaf/selected/center-00009-policy.json`; the mobile
  control is `Control::MobileScript`; the 12 `fast-leaf` layouts and
  `episode::run_prepared` are in `crates/cubarium-search/src/es/`.
- `design/ecology-v1-contract.md` §6.4 (served bite → capability → digestible
  → credit) and the `feed_min` / mouth-rate rules; `crates/cubarium-core/src/neural/action.rs`
  (`Action7`, `Capability`, `effort`, the three efforts normalised to sum ≤ 1,
  versus a legacy decision that can set all three to 1) and
  `neural/obs.rs` (what the policy sees); the intake step in
  `crates/cubarium-core/src/world/step.rs` (find it with `graft ask "where is a
  served bite computed from mouth effort and the cell's stocks" --source`).
- `design/7_Research/ecology-v1-training-2026-09-15.md` "Correction after
  review" and next task 1; `design/7_Research/r2d-forager-review-2026-09-15.md`
  findings 1–3.

## Deliverables

1. **Core: per-tick intake trace, read-only, opt-in, inert.** For a chosen
   organism, per tick: the occupied cell; its edible stocks by channel
   (foliage, fruit, litter, carrion) and whether each is above its feed
   threshold; the controller's three efforts as decoded (and the raw action
   before squashing, for a neural body); requested bite per channel; served;
   digestible; credited; the limiting term per channel (effort zero, stock
   below threshold, clamp by mouth rate, clamp by stock, clamp by battery /
   reserve headroom, capability zero); and the tick's bill. Same inertness
   proof as E: state hash identical with the trace on and off over 9,000
   ticks; not persisted; snapshot schema unchanged.
2. **Tests first**, from the definitions above: the inertness hash test; a
   hand-built tick where effort is on, stock is present and the bite is
   served, and the trace's served equals the ledger's served; a tick where
   stock is below threshold and the limiting term says so; a tick where
   effort is zero and the limiting term says so.
3. **The experiment** (≤ 6 wall minutes of simulation, 8 workers): generation
   9, the initial centre (`initial_center(20260915)`) and `MobileScript` on
   the 12 `fast-leaf` layouts, horizon 36,000, trace on, with the ledger. Per
   driver per layout report: fraction of ticks on a cell with any edible stock
   above threshold; fraction of those ticks with the matching effort on (> 0.05
   and > 0.5); fraction with effort on but no edible food; served / requested
   when both are positive; distribution of the limiting term; credit / bill.
   Then the three-way verdict, per driver, with the numbers that decide it.
   If the answer is (a), say what the score or action would have to reward;
   if (b), what the observation lacks (with the obs layout); if (c), which
   clamp and what it would take. If mixed, give the split.
4. **Result note** `design/7_Research/ecology-v1-intake-2026-09-16.md`: build,
   commits, trace definitions with the contract sections they implement,
   exact commands, the table, the verdict, what this does not establish, the
   next task named (a score change, an observation change, an adapter change
   or a settlement change — named, not implemented).
5. `cargo test -p cubarium-core`, `cargo test -p cubarium-search` green; `graft
   build`; commit.

## Constraints

- No change to any equation, the GRU, observation/action layout, optimizer,
  motor contract, snapshot, or any constant. No training. Diagnostics inert
  and opt-in, cost measured (< 3 % when on; zero when off).
- Compute ≤ 8 wall minutes total, ≤ 8 workers; storage `runs/ecology-v1-intake/`
  ≤ 40 MiB (per-tick traces compact: one row per tick per body, binary or
  gzipped JSONL, or aggregate per 100 ticks with the per-tick file for two
  layouts only — say which).
- Ask no questions; record routine choices in the note.

## Decision authority

Yours: trace layout, accessor names, aggregation windows, file format.
Fable's: anything that changes a simulated number, any equation, the layouts.

## Verification

```bash
cargo test -p cubarium-core
cargo test -p cubarium-search
```

Fable re-runs one trace row per driver and checks the fractions reproduce.

## Return format

The note, plus: commit hashes, test totals, the per-driver table in compact
form, the three-way verdict with its deciding numbers, the trace's measured
cost, wall time, measured usage, and the evidence and reasoning behind each
decision. Link files; paste no logs.

## Stop

Stop after the note and commit. No training, no deployment.
