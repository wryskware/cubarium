---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream E (Opus): the per-body store budget, one matched feasibility experiment, and the apex opportunity audit

Fable orchestrates. This is step 1 and step 5 of the reconciled next steps in
[the consolidated result](../7_Research/ecology-v1-next-results-2026-09-15.md)
("Next recommendation"), as reordered by
[Astra's review](../7_Research/ecology-v1-next-review-2026-09-15.md) ("Next steps:
Astra's opinion", items 1 and 5). You own this brief; Fable reviews once with at
most two repair cycles. Model: Opus 5, high reasoning effort (core accounting,
the ES fixtures and the whole-world runner are coupled). Time target: one
working session. Run in `/home/wrysk/wryskware/cubarium` on `main` under
`AGENTS.md` and `WORKING_POLICY.md`.

**Two other workers are active in separate worktrees**: one edits
`crates/cubarium-search/src/{calibrate,evaluate,population}.rs`, one edits the
presenter (`crates/cubarium/src/art_present/`, `crates/cubarium-render/`). Do
not edit those files. Your files: `crates/cubarium-core/src/**` (diagnostics
only, see constraints), `crates/cubarium-search/src/es/**`, a **new**
`crates/cubarium-search/src/apex_audit.rs` (+ its one dispatch line in the CLI),
`crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`. All three
workers share one `target/`; a concurrent build of the same crate can make a
doctest step fail spuriously — re-run, do not "fix". Commit on `main`, staging
only your files by path (never `git add -A`); leave `.claude/*`,
`WORKING_POLICY.md`, `.agents/` and other uncommitted design documents alone.
Do not push, tag, restart the cube, or touch `state/`, port 7393, the shim or
the running `cubarium` process. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Objective

1. Expose, per organism, the full store budget as **read-only diagnostics**
   that change no dynamics, then measure it for four controllers on the same
   `fast-leaf` layouts so that body infeasibility, controller failure and the
   need to relocate are separated for the first time.
2. Instrument the apex mating predicate so the campaign can say *why* no two
   introduced apexes ever mate, and run that audit on the two-apex arms.

## Read first

- `design/7_Research/ecology-v1-training-2026-09-15.md`: "Correction after
  review", "What the intake columns say", "Intake by food per controller — what
  is measured and what is not", and next task 1. The body is
  `Genome::founder` with `diet` 0.7 — a generalist (`cap_foliage` 0.7,
  `cap_detrital` 0.3), see `crates/cubarium-core/src/genome.rs:56-58,207-223,408-433`.
- `design/7_Research/ecology-v1-next-review-2026-09-15.md` findings 1 and 5
  and next-steps items 1 and 5: the experiment design and the audit you are
  implementing. Follow them.
- `design/ecology-v1-contract.md` §6.4 (a served bite → capability →
  digestible share → energy density → assimilation → battery/reserve →
  oxidation), §6 upkeep, §13 apex.
- `crates/cubarium-core/src/world/state.rs:425` `IntakeDiagnostics` (world
  totals) and `world/hunter.rs:51`; `crates/cubarium-core/src/motor.rs:354-408`
  (the motor bill); `crates/cubarium-core/src/world/step.rs:600-665` (the
  mating predicate: within `MATING_RADIUS_PX`, both `Perched`, both mature,
  both `may_reproduce`, neither committed, capacity, and then the paid
  contribution `a.reserve >= material_due && … energy_due`).
- `crates/cubarium-search/src/es/{episode,commands,fixture}.rs`: `Driver`,
  `Control::{NoIntake, StationaryGrazing, MobileScript}`, the four training
  and eight held-out layouts, `Layout::config`/`build`, `--config`.
- `crates/cubarium-search/src/calibrate.rs`: the whole-world matrix runner
  (candidates × seeds × arms, apex introduction at tick 6,000). Reuse its
  public pieces from your new `apex_audit.rs`; do not edit it.

## Deliverables

1. **Core: per-organism budget accumulator.** For every organism, accumulated
   from birth, per food channel (foliage, fruit, detritus, carrion): served
   material `q`, digestible `q_d = cap·q`, reserve credit, direct battery
   credit; and totals: oxidation from reserve, upkeep billed, motor bill
   (translation and turning separately), growth/reproduction outlay, and the
   terminal stores when it dies (structure, reserve, energy) with the death
   cause. Read-only accessor(s) on `World` (per id, and a drain of the
   records of organisms that died since the last drain). **No dynamics
   change**: add a test that steps the same seeded world for 4,000 ticks with
   and without reading the accumulator and compares `final_state_hash`
   byte-for-byte, and a test that the accumulator's identity holds per body:
   `Σ credits − Σ debits = Δ(stores)` to 1e-9 over a life. The snapshot schema
   is unchanged: the accumulator is not persisted (state it).
2. **Core: mating opportunity counters.** Per tick, for the apex profile:
   number of adult members alive, number mature and `Perched`, number
   `may_reproduce`-ready, number simultaneously ready (≥ 2), the minimum
   distance between two ready members, and a per-predicate failure count for
   every candidate pair that got as far as the sorted `pairs` list (radius,
   readiness a/b, committed, capacity, contribution unaffordable). Read-only,
   drained by the caller, no dynamics change (same hash test).
3. **The feasibility experiment** (≤ 10 wall minutes of simulation, 8
   workers): on `fast-leaf` (`runs/ecology-v1-calibration/selected/fast-leaf.toml`),
   the 4 training + 8 held-out layouts, one episode each, horizon 36,000, for
   four drivers: `StationaryGrazing`, `MobileScript`, the initial centre
   (`initial_center(20260915)` as `es-train` seeded it), and generation 9
   (`runs/es-eco-v1-fastleaf/selected/center-00009-policy.json`, whose
   ecology check must pass). Record per driver per layout: lifetime, the full
   budget above, the sustained credit/bill ratio over the last 2,000 ticks
   alive, distinct cells, and the route's foliage at start and end. Then
   answer, in one table and one paragraph each, Astra's three branches: if
   the mobile control also dies with its sustained ratio < 1 while reaching
   food, the body's budget binds; if it survives while generation 9 dies, the
   controller or the 16-update search binds; if stationary dies and mobile
   survives, relocation is necessary. State which branch the data supports,
   or that they are still confounded and why.
4. **The apex opportunity audit** (≤ 6 wall minutes, 8 workers): a new
   `apex-audit` subcommand running the two-apex arm exactly as A's screen
   (introduce tick 6,000, same profile, never restocked), for `baseline.toml`
   and `fast-leaf.toml`, on the 4 held-out seeds (`HELDOUT_SEEDS[..4]`),
   horizon 180,000. Report per run: apex lifetimes, ticks with ≥ 2 adults
   alive, ticks with ≥ 2 simultaneously ready, the minimum ready-pair distance
   ever, the first-failing-predicate histogram, and whether a ready pair ever
   came within 10 px. Conclude: is readiness overlap zero (radius cannot
   matter), or does overlap exist and no ready pair closes to 10 px (radius or
   encounter policy is a real choice), or something else.
5. **Result note** `design/7_Research/ecology-v1-budget-2026-09-16.md`: build,
   commits, the exact commands, the budget definitions with the contract
   sections they implement, the feasibility table and verdict, the audit
   table and verdict, what this does not establish, and the next task it
   implies (named, not launched). Plain language; say when the answer is
   "confounded".
6. `cargo test -p cubarium-core`, `cargo test -p cubarium-search`, and
   `cargo test -p cubarium --release --test run_neural_seed` green; `graft
   build`; commit.

## Constraints

- No change to any ecological equation, the motor contract, the GRU,
  observation/action layout, the optimizer, the snapshot schema, or any
  constant. Diagnostics are additive and must be provably inert (the hash
  tests). Per-frame/per-tick cost of the accumulator: measure it on the
  `ecology_v1_scenarios` B0 world (ticks/s before and after) and keep it
  under 3 %; if it costs more, gate it behind a `World` flag that the runner
  never sets.
- Compute: ≤ 16 wall minutes of simulation in total, ≤ 8 workers, nothing left
  running when you return. Storage: outputs under `runs/ecology-v1-budget/`
  ≤ 20 MiB, compact JSON.
- Tests are their own pass: write the two hash tests, the identity test and
  the predicate-counter test **before** the experiment code, from the
  definitions above, not from what the implementation happens to produce.
- Ask no questions; record routine choices in the note.

## Decision authority

Yours: accumulator layout and accessor names, where the counters live, the
audit subcommand's flags and JSON, how "sustained ratio" is windowed (state
it). Fable's: anything that changes a number the simulation produces, any
equation, the layouts, the ES score. Wrysk's: none needed.

## Verification

```bash
cargo test -p cubarium-core
cargo test -p cubarium-search
cargo test -p cubarium --release --test run_neural_seed
```

Fable's review re-runs the hash tests, one feasibility row and one audit row
and checks they reproduce field for field.

## Return format

The result note, plus in the final message: commit hashes, test totals, the
feasibility table in compact form with the branch verdict, the audit verdict,
the accumulator's measured cost, actual wall time per stage, and measured
usage. Link files; paste no logs.

## Stop

Stop after the note and commit. Nothing is deployed from this workstream.
