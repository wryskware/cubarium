---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream L (Opus): two falsification checks before any score change, and the score proposal written

Fable orchestrates. Step 2 of the reconciled next steps in
[the round-2 result](../7_Research/ecology-v1-round2-results-2026-09-16.md)
("Next recommendation (reconciled with Astra)"), from Astra's
[round-2 review](../7_Research/ecology-v1-round2-review-2026-09-16.md) (P2 on H,
next steps item 2). You own this brief; Fable reviews once with at most two
repair cycles. Model: Opus 5, high reasoning effort (the neural forward pass,
the ES episode scripting and the score are coupled, and the answer decides
whether a design change is proposed at all). Time target: one working session.
Run in `/home/wrysk/wryskware/cubarium` on `main` under `AGENTS.md` and
`WORKING_POLICY.md`.

**Three other workers are active in worktrees**: one owns
`crates/cubarium-core/src/world/step.rs` (the plant/intake accounting region)
and `crates/cubarium-search/src/{depletion,calibrate,evaluate,movement}.rs`; one
owns the hunter strike path in `crates/cubarium-core/src/{hunter.rs,world/hunter.rs}`
and `crates/cubarium-search/src/apex_audit.rs`; one owns
`crates/cubarium-search/src/factorial.rs`. Do not edit those. Your files:
`crates/cubarium-core/src/neural/**` (read-only accessors only, if strictly
needed), `crates/cubarium-search/src/es/**`, `crates/cubarium-search/tests/`,
`design/7_Research/`, `design/` for the proposal, `runs/`. Shared `target/`: a
build failing on a symbol the tests just used means another worker was
building; `touch crates/cubarium-core/src/lib.rs` and rebuild; pin
`CUBARIUM_SEARCH_BUILD=<your commit>` when recording build stamps (the shared
build script can bake another tree's commit). Commit on `main` by path only;
leave `.claude/*`, `WORKING_POLICY.md`, `.agents/` and other uncommitted design
documents alone. Do not push, tag, restart the cube, or touch `state/`, port
7393, the shim or the running `cubarium`. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Objective

H showed generation 9 stands on food 8 % of the time against the scripted
control's 95 %, with its mouth open on every on-food tick and no clamp, and the
"food here" scalar present in its observation. The leading hypothesis is that
the score `t_min + 0.25·stores` gives no gradient for staying. Before that
becomes a design change, try to falsify it two ways; then, only if it survives,
write (do not implement) the scoring proposal Astra specified.

## Read first

- `design/7_Research/ecology-v1-intake-2026-09-16.md` (H): the per-tick trace,
  the residence numbers, "What this does not establish", and the exclusions H
  named (frozen weights, hidden state, cadence, the constant 1/3 mouth split).
- Astra's round-2 review, P2 on H and next-steps item 2: the two checks and
  the auxiliary `A = (1/T) Σ_t clip((E_credited,t − E_billed,t)/b_ref, −1, 1)`,
  ticks after death scored −1, `S = t_min + λ·A`, with `T`, `b_ref`, clipping
  and `λ` fixed before any outcome, `λ` small enough not to erase a material
  survival difference, credited usable energy rather than served mass.
- `crates/cubarium-core/src/neural/{obs.rs,action.rs,gru.rs}` (the 70-scalar
  observation: `v[0] = P_here/P_max`, ring-sector food scalars, feedback
  channels; the GRU step; the action squash), `crates/cubarium-search/src/es/{episode,fixture,commands,budget,intake}.rs`
  (`Driver`, `Control::{StationaryGrazing, MobileScript}`, `run_prepared`, the
  score, the 12 `fast-leaf` layouts), the generation-9 policy
  `runs/es-eco-v1-fastleaf/selected/center-00009-policy.json`, and E's ledger
  (`World::record_body_budgets`, `body_budget`).

## Deliverables

1. **Check (a), the frozen controller's response to food.** Build a
   deterministic observation sweep: take real observations recorded from
   generation 9 on the 12 layouts (H's trace or a fresh run), and for each of
   a sample of ticks construct variants that differ only in the local food
   scalar `v[0]` (0, 0.25, 0.5, 0.75, 1.0) and, separately, only in the ring
   food sectors (all zero; food ahead; food behind), holding every other
   scalar fixed. Run the frozen weights forward twice: with the hidden state
   reset to zero, and with the hidden state carried from the real trajectory
   up to that tick. Report the movement head's response (thrust, turn) and
   the mouth efforts as functions of the varied scalars, with effect sizes
   against the natural variation of the action across the trajectory. Verdict:
   does the policy's movement respond to food at all? If it does (turns toward
   or slows on food) while residence still fails, cadence or recurrent
   dynamics come first and the score hypothesis is weakened. Tests first: a
   hand-built observation whose only difference is `v[0]` produces the
   documented action difference for a hand-built one-layer weight set.
2. **Check (b), the current score's dwell gradient.** Add a scripted control
   family with a dwell parameter (a `Control::Dwell(d)` or an argument on the
   mobile script: stay on a food cell for `d` ticks before moving on, for `d`
   ∈ {20, 100, 300, 1,000, 3,000, stay-until-below-threshold}), same layouts,
   horizon 36,000. Score each with the **current** `t_min + 0.25·stores`
   exactly as the trainer would. Verdict: is the current score strongly and
   monotonically increasing in dwell? If yes, the missing-gradient diagnosis
   is wrong and no score change is proposed. Tests first: the dwell control
   leaves a cell after exactly `d` ticks on food in a hand-built layout.
3. **If both checks leave the hypothesis standing, write the proposal**
   `design/forager-score-proposal-2026-09-16.md`: Astra's auxiliary with `T`,
   `b_ref`, clipping and `λ` fixed now with the reasoning; how ticks after
   death are scored; the pre-registered falsifiers (ranks short rich bursts,
   stationary starvation, or early death above sustained feasible residence;
   a dwell-script ladder must be monotone under `S` before any ES run); the
   exact bounded experiment that would follow (one campaign, budgets); and
   what it does not touch (the GRU, observation, action, ecology, the cube).
   Compute `S` for the dwell ladder from check (b) under the proposed
   constants and report whether it is monotone; that is the ladder Astra
   asked for and it costs nothing extra. **Do not implement the score in the
   trainer and do not train.** If either check falsifies the hypothesis, write
   the proposal as "not proposed" with the evidence, and name what the
   evidence points at instead.
4. **Result note** `design/7_Research/ecology-v1-score-checks-2026-09-16.md`:
   build, commits, commands, both checks' tables and verdicts, the proposal's
   status, what this does not establish, the next task named (not launched).
5. `cargo test -p cubarium-core`, `cargo test -p cubarium-search` green;
   `graft build`; commit.

## Constraints

- No change to the GRU, observation or action layout, optimizer, score as
  used by the trainer, motor contract, any equation, snapshot. Read-only
  accessors in `neural/` only if the forward pass cannot otherwise be driven
  from the search crate (say so).
- Compute ≤ 8 wall minutes of simulation, ≤ 8 workers; `runs/ecology-v1-score-checks/`
  ≤ 30 MiB.
- Ask no questions; record routine choices in the note.

## Decision authority

Yours: sweep sample, dwell values, file layout, control naming. Fable's: any
score change (you propose; Fable and Wrysk decide), any equation.

## Verification

```bash
cargo test -p cubarium-core
cargo test -p cubarium-search
```

Fable re-runs one dwell value and one sweep slice and checks they reproduce.

## Return format

The note, plus: commit hashes, test totals, check (a)'s verdict with its
effect sizes, check (b)'s dwell table under the current score, the proposal's
status and its ladder under `S`, wall time, usage, and the evidence and
reasoning behind each decision. Link files; paste no logs.

## Stop

Stop after the note and commit. No training, no deployment.
