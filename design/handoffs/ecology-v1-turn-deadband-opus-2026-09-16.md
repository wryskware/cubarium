---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream X (Opus): the turn deadband alone — replay first, then one bounded pair

Fable orchestrates. Item 4 of the reconciled next steps in
[the round-4 result](../7_Research/ecology-v1-round4-results-2026-09-16.md),
as Astra bounded it in
[Q's correction block](../7_Research/ecology-v1-es-antithetic-2026-09-16.md):
*`TURN_DEADBAND` 0.05 → 0.0 only, with `Aggregate::Min`, σ, pair count, score,
layouts and seed fixed and a new protocol hash; first replay the frozen centre
and candidates on their own layouts under both adapters; then a bounded pair
only if turn release raises on-food residence or `t_min`; and, cheaply,
bootstrap or split the retained sixteen pair contributions for
gradient-direction stability.* No score change. You own this brief; Fable
reviews once with at most two repair cycles. Model: Opus 5, **high**
reasoning effort (an adapter contract change plus a training decision the
replays must make honestly). Time target: one session; training itself is
capped below. **Worktree.** Run under `AGENTS.md` and `WORKING_POLICY.md`.
Shared `target/`: set `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`
and run `touch crates/cubarium-core/src/lib.rs` immediately before **every**
cargo invocation (another tree builds concurrently; cargo holds the lock only
within one invocation); pin `CUBARIUM_SEARCH_BUILD=<your commit>` and copy the
release binary out of the shared target before running anything. **Never run
`cargo fmt`** in this repository: there is no `rustfmt.toml`, the tree is not
rustfmt-formatted, and a run rewrites 117 files. First `git log --oneline -1`
and `git reset --hard <the brief commit>` if HEAD differs. `runs/` is
git-ignored: read retained artefacts from the main checkout and copy your
outputs back there at the end. Commit by path only; do not push, tag,
restart the cube, or touch `state/`, port 7393, the shim or the running
`cubarium`. Commit messages end with `Co-Authored-By: Claude Fable 5.1
<noreply@anthropic.com>`. Tests first, from the definitions. Never add a
`WorldConfig` field.

**Files you own:** `crates/cubarium-core/src/neural/**`,
`crates/cubarium-core/src/world/{mod.rs,lifecycle.rs}` (one transient beside
`motor_model`, `apex_turn_radius` and `apex_motor_model`, with setter and
getter), the `neural_decision` call in `crates/cubarium-core/src/world/step.rs`
and `neural_observation` in `world/view.rs` only where the adapter reaches
them, new files under `crates/cubarium-core/tests/`,
`crates/cubarium-search/src/es/**` (keep `es/export.rs` edits to the
minimum the protocol needs; workstream V is editing its docs), its `Es*`
lines in `crates/cubarium-search/src/main.rs`, `crates/cubarium-search/tests/`,
`design/7_Research/`, `runs/`. **Do not touch** `hunter/`, `motor.rs`,
`snapshot.rs`, `calibrate.rs`, `evaluate.rs`, `population.rs`, `apex_audit.rs`,
the non-`Es*` commands in `main.rs`, or anything under `crates/cubarium/`.

## Read first

- [Q's note](../7_Research/ecology-v1-es-antithetic-2026-09-16.md) as
  corrected, and its artefacts `runs/ecology-v1-es-antithetic/{pairs,deadband}.json`
  (`es/antithetic.rs`: `run_deadband`, the pair reconstruction). The deadband
  is the leading adapter hypothesis, r = 0.21 with score, untested by any
  trajectory under a different band.
- `crates/cubarium-core/src/neural/action.rs`: `DEADBAND = 0.05` applied to
  both `THRUST` and `TURN` in `Action7`'s squash → band → mask → normalise
  order; `neural/mod.rs::PROFILE_TEXT` (`cub-act-1`), which the policy
  digest covers, so a policy trained under another adapter is refused by name
  already (`es/export.rs`, `crates/cubarium/tests/run_neural_seed.rs`).
- The retained training run `runs/es-eco-v1-fastleaf/` (its command is in
  [the training brief](ecology-v1-training-opus-2026-09-15.md) §3: `--pairs
  16 --generations 16 --horizon 36000 --workers 8 --wall-seconds 1200
  --train-seed 20260915 --center-eval true`), its `centers/center-00009.json`
  and `selected/center-00009-policy.json`, `generations.jsonl`, `holdout.json`.
  [Q's brief](ecology-v1-es-antithetic-opus-2026-09-16.md) for the
  replay entry points and what "on-food fraction", "dwell" and `t_min` mean
  (workstream H's intake trace, L's `Control::Dwell`).

## Deliverables

1. **The adapter variant.** A second action adapter that differs from
   `cub-act-1` in exactly one constant: the `TURN` band is 0.0 (the `THRUST`
   band stays 0.05). Selected per `World` by a transient (no `WorldConfig`
   field; default `cub-act-1`, byte-identical: pin a state hash over ≥ 3,000
   ticks of a world with a neural animal before the code, as U and W did);
   its own profile text `cub-act-2` in place of `cub-act-1`, so the protocol
   hash changes and every existing policy is refused by name under it and a
   `cub-act-2` policy is refused by the host, which stays on `cub-act-1`
   (test that in `crates/cubarium-search/tests/`, not in the host crate).
   Recorded in `Protocol` and `PolicyFile` by name (missing = `cub-act-1`);
   `es-train`, `es-evaluate`, `es-population` and the replay commands take
   `--adapter {cub-act-1,cub-act-2}`. Tests first: a raw turn head of ±0.03
   is zero under `cub-act-1` and ±0.03 under `cub-act-2` at the same thrust;
   the thrust band is unchanged; masks and normalisation unchanged; the
   profile texts differ in that one token.
2. **Replay first.** The frozen gen-9 centre and its sixteen candidates (both
   antithetic halves) replayed on their own training layouts under both
   adapters, weights untouched: per trajectory, turn activity (fraction of
   ticks with a non-zero resolved turn, mean |ω|), on-food fraction, dwell
   bouts and `t_min`, plus the score. Report the paired differences per
   trajectory with an exact sign test. **Decision rule, stated before you
   run it:** turn release is *falsified as the bottleneck* if turn activity
   rises but on-food fraction, dwell and `t_min` do not (sign test p > 0.1
   on all three); it is *supported* if on-food fraction or `t_min` rises in
   most trajectories; anything else is *mixed*, and you say so.
3. **One bounded pair, only if step 2 is supported or mixed.** The retained
   command verbatim with `--adapter cub-act-2` and a new `--out
   runs/es-eco-v1-fastleaf-act2`: `Min`, σ, 16 pairs, 16 generations, horizon
   36,000, layouts, score and `--train-seed 20260915` fixed; 8 workers,
   **20 wall minutes cap**, no retry, continuation or tuning; the retained
   `cub-act-1` run is the control and is **not** re-run. Compare generation
   by generation (centre score, best candidate, on-food fraction, `t_min`)
   and on `holdout.json`'s layouts. If step 2 falsifies, do not train: say
   so and stop at step 4.
4. **Gradient-direction stability, cheap.** From `pairs.json`'s sixteen
   retained pair contributions to the gen-9 update: split-half cosine (all
   70 balanced splits, or 1,000 random ones) between the two halves' update
   directions, and a 1,000-sample bootstrap of the full direction's cosine
   with the recorded update. Report the distributions. Do not conclude that
   sixteen pairs suffice or do not; report what the spread says and what
   would decide it.
5. **Result note** `design/7_Research/ecology-v1-turn-deadband-2026-09-16.md`
   with a "what would change on the cube" paragraph (nothing, until a
   `cub-act-2` policy is selected and the host learns the adapter); tests
   green (`cargo test -p cubarium-core`, `-p cubarium-search`); `graft
   build`; commit on the branch. Storage `runs/ecology-v1-turn-deadband/` and
   `runs/es-eco-v1-fastleaf-act2/` ≤ 60 MiB together.

Return: branch and commits, test totals, the pinned hash, the replay table
and its verdict by the rule above, the training comparison if run (or why
not), the stability distributions, wall time, usage, evidence and reasoning.
