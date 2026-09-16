---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream F (Opus): the movement-cost arm and the variety census

Fable orchestrates. This is step 3 and step 4 of the reconciled next steps in
[the consolidated result](../7_Research/ecology-v1-next-results-2026-09-15.md)
("Next recommendation"), from
[Astra's review](../7_Research/ecology-v1-next-review-2026-09-15.md) ("Next steps:
Astra's opinion", items 3 and 4). You own this brief; Fable reviews once with at
most two repair cycles. Model: Opus 5, high reasoning effort (pre-registered
criteria, spatial measures, a matrix that must stay matched). Time target: one
working session. Work under `AGENTS.md` and `WORKING_POLICY.md`.

**You are working in a separate git worktree.** Set
`CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target` for every cargo
command (one build cache; the first build recompiles the workspace once; a
concurrent build of the same crate by another worker can make a doctest step
fail spuriously — re-run). Touch only
`crates/cubarium-search/src/{calibrate,evaluate,population,params}.rs`, new
files under `crates/cubarium-search/src/` that you create, the CLI dispatch
line, `crates/cubarium-search/tests/`, `design/7_Research/` and `runs/`.
Another worker owns `crates/cubarium-core/` and `crates/cubarium-search/src/es/`
on `main`; do not edit them. Commit on your worktree branch, staging by path.
Do not push, tag, restart the cube, or touch `state/`, port 7393, the shim or
the running `cubarium` process. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Objective

Test spatial coupling **alone**: does raising the per-pixel movement price,
with everything else held exactly as calibration A's screen, reduce range and
produce the local depletion/recovery cycles ecology v1 was built to show,
without a starvation collapse? And report the variety census by founder form
× diet bin × guild so the skimmer's loss can be attributed.

## Read first

- `design/7_Research/ecology-v1-calibration-2026-09-15.md`: the screen's
  conditions (15 candidates × 6 training seeds × arms 0/1/2, horizon 180,000,
  apex introduced at tick 6,000, never restocked), the six gates, the guild
  census, the late window (last 20 %), the depletion/recovery probe (90 %
  lifetime threshold), "The population feedbacks the model is missing" item 1
  as corrected after review (movement is billed **per distance** at
  `move_cost · S · (speed + k·r·|ω|) · dt`, price cut ≈ 16.7× with the pace
  calibration; `crates/cubarium-core/src/motor.rs:354-408`,
  `config.rs:558-565`), and "Reproducing every stage".
- `design/7_Research/ecology-v1-next-review-2026-09-15.md` findings 3, 4 and
  next-steps items 3 and 4: the criteria you pre-register.
- `crates/cubarium-search/src/calibrate.rs` (matrix runner, parameter box,
  `config_hash`), `evaluate.rs` (guild assignment at birth `:388-443`, late
  window `:856-887`, spatial probe `cells_per_body_window`, forms census),
  `params.rs`. `organism.move_cost` is listed in `WorldConfig`'s parameter
  table (`crates/cubarium-core/src/config.rs:830`); verify the calibration
  box can set it, and add it to the box if not.

## Deliverables

1. **Pre-registration first**, in the note before any run: the arms, the
   measures, and the confirmation/refutation rules — confirmation is a
   substantial fall in range **plus** repeated local depletion/recovery
   crossings without a starvation collapse; refutation is unchanged range, or
   deaths rising while depletion stays absent. Put numbers on "substantial"
   and "collapse" before running (for example, range < 60 % of the 0.00036
   arm; late population ≥ 50 % of the same arm).
2. **New measures** in `evaluate.rs` (search side only): per body per window,
   revisit interval (ticks between successive visits to the same cell) and
   residence time per cell; per run, depletion **and recovery** crossings per
   watched cell; deaths by cause per arm; terminal stores at death. If the
   per-organism budget accumulator from workstream E has landed on `main` by
   the time you need it (`git log --oneline main | head`), rebase and report
   net energy margin per body; if not, report death cause and terminal stores
   and say the margin is not measured.
3. **The matrix** (≤ 20 wall minutes of simulation, 8 workers): `organism.move_cost`
   ∈ {0.00036, 0.0018, 0.006} × {`baseline.toml`, `fast-leaf.toml` from
   `runs/ecology-v1-calibration/selected/`} × the screen's 6 training seeds ×
   arms 0/1/2, horizon 180,000, all other conditions identical to A's screen.
   Publish the run matrix in the note before the campaign. The 0.00036 rows
   must reproduce A's screen rows for the same candidate and seed (same
   `final_state_hash`); if they do not, stop and report why.
4. **The variety census**: in every run, births, deaths by cause, mean
   lifetime and (if available) lifetime intake by founder form × diet bin
   (0.0–0.35, 0.35–0.65, 0.65–1.0) × guild, plus the skimmer's loss tick and
   the cause of its last death. Answer: is the skimmer lost through its body,
   its controller, its habitat, or a lower realised diet yield? If the data
   cannot say, say so and name what would.
5. **Result note** `design/7_Research/ecology-v1-movement-2026-09-16.md`:
   build, commits, exact commands, the pre-registration verbatim, the matrix
   table, the verdict against the pre-registered rules per configuration,
   the census, what this does not establish (one price knob, no site-fidelity
   mechanism tested, 180 k ticks), and the next task it implies (named, not
   launched). Do not propose a new equation unless the refutation branch is
   reached, and then only name it.
6. `cargo test -p cubarium-search` green; `graft build`; commit on the
   worktree branch.

## Constraints

- No change to `crates/cubarium-core/`, no ecological equation, no change to
  A's existing candidates, gates or outputs (add, do not alter). One price
  knob only.
- Compute: ≤ 20 wall minutes of simulation, ≤ 8 workers, nothing left running.
  Storage: `runs/ecology-v1-movement/` ≤ 30 MiB.
- Tests are their own pass: unit tests for revisit interval, residence time
  and the crossing counter on hand-built samples, written from the
  definitions before the implementation.
- Ask no questions; record routine choices in the note.

## Decision authority

Yours: exact measure definitions (stated), diet bin edges (stated), JSON
layout, subcommand/flag names. Fable's: any change to a core file, an
equation, or the gates. Wrysk's: none needed.

## Verification

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-search
```

Fable's review re-runs one row per price level and checks the 0.00036 rows
against A's retained screen rows.

## Return format

The result note, plus in the final message: branch and commit hashes, test
totals, the matrix table in compact form with the verdict per configuration,
the skimmer attribution, actual wall time, and measured usage. Link files;
paste no logs.

## Stop

Stop after the note and commit. Fable merges the branch.
