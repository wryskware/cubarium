---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Workstream C (Opus): one fresh forager training campaign in the selected ecology

Fable orchestrates under
[the next-steps handoff](ecology-v1-next-fable-2026-09-15.md), section "C".
Workstream A is complete: its
[result note](../7_Research/ecology-v1-calibration-2026-09-15.md) selected the
`fast-leaf` configuration (`plant.foliage_rate` 0.006, `plant.maintenance`
0.0001, `plant.reserve_share` 0.35, nothing else moved) and exported it as
`runs/ecology-v1-calibration/selected/fast-leaf.toml`, config hash
`09e244392ec91768`. You own this brief; Fable reviews once with at most two
repair cycles. Model: Opus 5, high reasoning effort (trainer, matrix harness
and the core founding door are coupled). Time target: one working session.
Run in `/home/wrysk/wryskware/cubarium` on `main` under `AGENTS.md` and
`WORKING_POLICY.md`.

**Another worker is editing the presenter in a separate worktree.** Do not
touch `crates/cubarium/`, `crates/cubarium-render/`, `assets/`,
`crates/cubarium-core/src/view.rs` or `crates/cubarium-core/src/world/view.rs`.
Commit on `main`, staging only your files by path (never `git add -A`); leave
`.claude/*`, `WORKING_POLICY.md`, `.agents/` and other uncommitted design
documents alone. Do not push, tag, restart the cube, or touch `state/`, port
7393 or the running `cubarium` process. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Objective

Train one fresh recurrent forager from scratch in the `fast-leaf` ecology with
the existing ES trainer, frozen and hashed against that configuration; then
run a fixed held-out behaviour check and a small population-level evaluation
with ordinary reproduction and matched predator arms, comparing food use,
travel and vegetation pressure against the legacy controller. Record policy
provenance for every body. Do not quietly train against the old ecology or
resume an old policy.

## Read first

- `design/7_Research/ecology-v1-calibration-2026-09-15.md`: "The selected
  configuration", "The population feedbacks the model is missing", and the
  `calibrate` subcommands under "Reproducing every stage".
  `crates/cubarium-search/src/calibrate.rs` is A's whole-world matrix runner
  (candidates × seeds × apex arms, ecology v1 component vector, guild census,
  late window); reuse it for the population evaluation.
- `crates/cubarium-search/src/es/{commands,trainer,episode,fixture,export}.rs`:
  `es-train`, `es-evaluate`, `es-export`, the frozen `Protocol` (its hash
  covers everything a score depends on that is not the policy), the four
  training and eight held-out layouts, `Layout::config` (starts from
  `WorldConfig::default()`, clears founders, zeroes weather amplitude and
  rain, disables mutation, sets the seed) and `Layout::build`, which paints
  `W = P/α`, `Q = q_cap·W` from the config it is given.
- `design/7_Research/r2c-learning-screens-2026-09-15.md` §1–§4 and
  `design/7_Research/r2d-forager-review-2026-09-15.md` findings 1–3: the
  previous campaign's selection rule (highest recorded centre score, earliest
  generation on ties, frozen before held-out), the censoring correction for
  group runs, and the caution that survival on a patch is not sustained
  foraging. `design/handoffs/r2b-fable-first-learning-2026-09-15.md`: the
  bounded-run discipline you repeat here.
- `design/7_Research/r3a-display-seed-result-2026-09-15.md`:
  `World::found_training_animal` / `found_neural_animal`, and the statement
  that the runtime gives an ordinary child its parent's policy with fresh
  hidden state. Verify that in `crates/cubarium-core/src/neural/` and
  `world/lifecycle.rs` before relying on it, and record what you found.
- `crates/cubarium-core/src/world/lifecycle.rs:52-63`: with `founders.kinds`
  set, `founders.count` is ignored and the default world places 24 animals.

## Deliverables

1. **Config plumbing**: `es-train`, `es-evaluate` and `es-export` gain
   `--config <toml>`; `Layout::config` starts from that file instead of the
   defaults and still applies its own overrides (no founders, no weather
   swing, no rain, no mutation, seed). The `Protocol` gains the config hash
   (and a `config` label) so the protocol hash changes with the ecology; a
   policy file exported from this run records it. Keep the R2 fixtures and
   the GRU/optimizer untouched; the existing `es_repair`/`harness` tests must
   pass, updated only where the protocol text now carries the config.
   Refuse a policy whose recorded config hash differs from the evaluation's
   config, by name.
2. **Plumbing smoke** (≤ 2 wall minutes): `es-smoke`-equivalent on the
   `fast-leaf` config; report the protocol hash, layout hashes and that every
   training layout still builds a valid world with one grazer whose painted
   stands are live under the new plant constants (state `W`, `P`, `Q` painted).
3. **The campaign, once**:

   ```bash
   cargo run -p cubarium-search --release -- es-train \
       --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
       --pairs 16 --generations 16 --horizon 36000 --workers 8 \
       --wall-seconds 1200 --train-seed 20260915 --center-eval true \
       --out runs/es-eco-v1-fastleaf
   ```

   At most 16 updates, eight workers, **20 wall minutes for training in
   total** including the smoke; no retry, continuation, extra seed, horizon
   change or tuning. If `runs/es-eco-v1-fastleaf` exists, inspect it first
   and do not overwrite a completed run. Trained body: the unit grazer of
   `found_training_animal` (hue `TRAINING_FOUNDER_HUE`, `diet` 0.85 →
   herbivore guild, births disabled in training episodes). State this in the
   note. Select the highest **recorded centre score**, earliest generation on
   ties, training results only, before any held-out episode runs; export it
   with `es-export`.
4. **Evaluation, ≤ 10 wall minutes total, separately timed**:
   - `es-evaluate --set holdout` on the selected centre, one episode per
     held-out layout, plain probe: per-layout survival, stores, intake by
     food, travel, distinct cells, censoring stated as in R2d finding 1.
   - **Population-level**: extend `calibrate` (or add a sibling subcommand)
     so a whole `fast-leaf` world with its 24 legacy founders also receives
     `N = 4` neural copies of the selected policy at tick 0 through
     `found_neural_animal`, reproduction and mutation on for everyone, apex
     arms 0/1/2 matched exactly as A's screen (introduce tick 6,000, same
     profile, never restocked), on 2 training seeds, horizon 180,000 ticks;
     paired against the same worlds with 4 extra **legacy** copies of the
     same body instead (so population and imported material match). Report
     per arm: neural bodies alive over time, births by controller (neural
     offspring vs legacy), deaths by cause and controller, intake by food per
     controller, distinct cells per body per window, foliage retention and
     depletion events against the legacy-only arm, apex outcomes. Every body's
     controller provenance comes from the world, not from assumption; if
     offspring of neural parents are legacy-controlled, say so and report
     the population as mixed.
5. **Result note** `design/7_Research/ecology-v1-training-2026-09-15.md`:
   build, commits, protocol hash and config hash, the exact commands, smoke
   result, training trajectory (initial/best/final centre score, updates
   completed, completed vs discarded work, wall time), the selected centre
   file and weight hash, held-out table, the population comparison, a plain
   statement of what this does not establish (not every guild, not the apex,
   not sustainability), and the **next guild/lifecycle training tasks
   named**, not launched. If the trained policy feeds better but destabilises
   the world, report that; do not restart calibration or training.
6. `cargo test -p cubarium-search` green; `cargo test -p cubarium-core` green;
   `graft build`; commit.

## Constraints

- No change to the GRU, observation/action layout, optimizer, motor contract,
  any ecological equation, the snapshot schema, or `neural/` beyond what
  config plumbing strictly needs (ideally nothing). No change to `calibrate`'s
  existing candidates, gates or outputs; add, do not alter.
- Compute: training ≤ 20 wall minutes, evaluation ≤ 10 wall minutes, ≤ 8
  workers, nothing left running when you return, no automatic extension.
- Storage: `runs/es-eco-v1-fastleaf/` under 20 MiB (checkpoint, generation
  summaries, centre files); evaluation outputs compact JSON. One `target/`.
- Old policies (`runs/es-r2c-*`) are refused by the new digest and are not
  evidence; do not evaluate them.
- Ask no questions; record routine choices in the note.

## Decision authority

Yours: subcommand and flag names, where the config hash sits in the protocol,
the population-evaluation layout and its reporting, the seeds among
`TRAINING_SEEDS[..6]`. Fable's: any change to the trainer's score, the
layouts, an equation, or `view.rs`. Wrysk's: none needed.

## Verification

```bash
cargo test -p cubarium-search
cargo test -p cubarium-core
./target/release/cubarium-search es-protocol --config runs/ecology-v1-calibration/selected/fast-leaf.toml
```

Fable's review re-runs the held-out evaluation on the exported policy (it
must reproduce field for field, as R3a's guard did), checks the protocol
hash moves with the config, and reads the provenance accounting in the
population evaluation.

## Return format

The result note, plus in the final message: commit hashes, test totals,
protocol and config hashes, the training trajectory in three numbers, the
selected centre path and weight hash, the held-out table, the population
comparison per arm in compact form, the provenance finding for offspring,
actual wall time per stage, and measured usage. Link files; paste no logs.

## Stop

Stop after the note and commit. Deployment (D) is Fable's; the policy is made
available through the existing `cubarium run --neural` control only if Fable
decides so after review.
