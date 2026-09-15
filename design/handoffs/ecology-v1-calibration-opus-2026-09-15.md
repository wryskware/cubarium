---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Workstream A (Opus): ecology v1 calibration and matched apex comparisons

Fable orchestrates under
[the next-steps handoff](ecology-v1-next-fable-2026-09-15.md), section "A".
You own everything in this brief; Fable reviews once, with at most two repair
cycles. Run in `/home/wrysk/wryskware/cubarium` on `main` under `AGENTS.md` and
`WORKING_POLICY.md`. Model: Opus 5, high reasoning effort. Time target: one
working session. Ecology v1 is accepted as the baseline; do not review it again
and do not reopen the food-web design.

Scope discipline: this brief only; one milestone; link artifacts instead of
pasting them; report real usage where the harness shows it; if the budget runs
out, finish without finishing the wider goal and say which parts are missing.

**Another worker is editing the presenter in a separate worktree at the same
time.** Do not touch `crates/cubarium/`, `crates/cubarium-render/`, `assets/`,
`crates/cubarium-core/src/view.rs` or `crates/cubarium-core/src/world/view.rs`.
If you need a new read-only accessor on `World`, put it in a new
`crates/cubarium-core/src/world/diagnostics.rs` (or `world/mod.rs`), not in
`world/view.rs`. Commit on `main`, staging only the files you authored by path
(`git add <path>`), never `git add -A`. Leave `.claude/*`, `WORKING_POLICY.md`,
`.agents/` and the other uncommitted design documents untouched. Do not push,
tag, restart the cube or touch `state/`, port 7393, or the running `cubarium`
process. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Objective

Find plausible ecological parameter ranges for ecology v1 under the existing
legacy controllers, with matched zero/one/two-apex arms, and select at most two
candidate configurations validated on held-out seeds. Name the population
feedbacks the model is missing. Export the selected configuration as a file the
trainer (workstream C) and the display runner (workstream D) can load. Do not
change any ecological equation, the digestive exclusions, or the motor contract.

## Read first

- `design/ecology-v1-contract.md` §3–§6, §11 (provisional values and the
  arithmetic behind them), §13.2 (what the B scenarios found), §15.3.
- `design/7_Research/ecology-v1-implementation-2026-09-15.md`, sections
  "Run 3" and "Still open, carried to the later whole-ecosystem search". Ignore
  runs 1 and 2. The motivating findings: one mobile grazer ran a bright 25-stand
  region down and starved; the reproducing 7 × 7 fixture doubled its population
  in 150 s against 823 s for half-foliage recovery, peaked at nine, went extinct.
  These are fixture results, not whole-world forecasts.
- `design/handoffs/ecology-search-2026-09-14-result.md`: the M1 harness
  (`crates/cubarium-search/src/{params,evaluate,metrics,search,rng}.rs`,
  `main.rs` subcommands `params`, `baseline`, `search`, `replay`). Its measured
  throughput (≈70 k ticks/s at 8 workers, pre-ecology-v1) and its default-world
  baseline (371 births per 100 sim min, apex never mates: 0 matings in 32 seeds,
  `MATING_RADIUS_PX` = 10) are the last measurements; **none of its scores or
  fixtures are evidence for ecology v1**. Its `Scoring` reference scales were
  calibrated on the old world.
- `crates/cubarium-core/src/world/state.rs` (`Telemetry`, `IntakeDiagnostics`
  with `litter_eaten`, `carrion_eaten`, `undigested`, `plant_income`,
  `plant_maintenance_unpaid`, `propagule_sent`, `body_bill_total/paid/upkeep`),
  `events.rs` (birth/death events and their causes), `genome.rs`
  (`Phenotype::cap_foliage`, `cap_detrital`), `config.rs` (`PlantConfig`,
  `DetritusConfig`, `OrganismConfig`, `validate`).
- `crates/cubarium-core/examples/ecology_v1_scenarios.rs`: the patch probes
  B0–B7. Keep them as diagnostics; do not extend their horizon.

## Deliverables

1. **Harness adaptation** in `crates/cubarium-search` (new modules welcome;
   the old `search` GA and `replay` must keep working and their tests green):
   - `Components`/`Sample` gain the ecology v1 measurements: foliage `ΣP`,
     living wood `ΣW`, dead wood `ΣWd`, reserve, litter, remains, plant deaths
     and recolonisations, bare/establishing cells, intake by food (leaf, fruit,
     litter, remains), undigested, plant income vs plant maintenance unpaid,
     propagules sent, actual body bills, population and births by **guild**
     (from `cap_foliage`/`cap_detrital`: herbivore = foliage only, detritivore =
     detrital only, generalist = both; report visual founder kinds separately
     since diets mutate), deaths by cause (starvation, age, collapse, predation,
     and whatever else `events.rs` distinguishes), apex attempts, captures,
     matings, births, survival, imported apex material/energy, and a spatial
     measure (distinct cells visited per body per window, or foliage
     depletion/recovery events per cell: a cell whose `P` falls below a quarter
     of its band's B0 steady state and later returns above half). Report a
     **late window** (last 20 % of the horizon) beside whole-run values.
   - A **parameter vector for ecology v1**: keep the structure of `params.rs`
     (names, bounds, defaults, rationale, `EXCLUDED`) and replace its contents.
     Cover the four axes the handoff names: intake/assimilation pressure
     (`organism.mouth_rate`, `organism.intake_half_saturation`,
     `organism.maintenance`), foliage growth and reserve allocation
     (`plant.foliage_rate`, `plant.wood_rate`, `plant.maintenance`,
     `plant.reserve_share`, `plant.reflush_below`, `plant.alpha`),
     maturation/reproductive investment and timing (`organism.growth_rate`,
     `drives.bud_reserve`, `drives.bud_min_age_seconds`), and recycling
     (`detritus.decomposition`, `detritus.wood_decomposition`,
     `detritus.carrion_decomposition`). Add `organism.capability_gate` and
     `capability_exponent` only if the baseline shows generalist dominance.
     Keep the vector to roughly 8–12 searched names; record units, bounds and
     why for each, and `validate` remains the only validity gate. Remove
     `producer.energy_density` if any trace remains.
   - A **matrix runner** (a new subcommand, e.g. `calibrate`) that evaluates a
     declared list of candidate configurations × seeds × apex arms on the real
     `World` with reproduction and the ordinary lifecycle on, no care, default
     weather, the core's default mutation, and writes one JSONL row per run
     with `param_bits`, seed, arm, status and the full component vector. Reuse
     `evaluate.rs`'s panic/invariant capture, apex introduction through
     `introduce_hunters` (record `material_in`/`energy_in`), and the
     counter-based rng. Apex arms: 0, 1 and 2 adults introduced at the same
     tick in every arm (say which tick; tick 0 or a short settle is your call),
     never restocked; report the initial cohort apart from any apex birth.
   - A **config export**: the selected candidate written as a complete
     `WorldConfig` file that `cubarium run --config` accepts, plus its
     `param_bits`, the config hash and the build id, under
     `runs/ecology-v1-calibration/selected/`. If no candidate is selected,
     export the provisional default the same way and say so.
2. **Throughput smoke** (≤ 5 wall minutes): one default-config world per apex
   arm at the campaign horizon on 1 and 8 workers; report ticks/s, per-tick
   cost and peak RSS. Use it to size the matrix below.
3. **Whole-world baseline**: the provisional defaults on `TRAINING_SEEDS[..3]`
   at least, all three apex arms, campaign horizon. State founder stocks
   (`W_0`, `P_0`, `Q_0` from §11), light/weather, mutation settings, horizon and
   sampling cadence in the note.
4. **The checkpoint, published before the campaign**: in the result note,
   write the concrete run matrix (candidates, seeds, arms, horizon, sampling),
   the trial count, the predicted wall time from the smoke, and the seed split
   (training `TRAINING_SEEDS[..6]` at most; held-out `HELDOUT_SEEDS[..4]`,
   never touched before step 6). Then proceed without asking.
5. **Screen**: a designed joint set, not a broad GA: the baseline plus roughly
   10–16 candidates that move two or more axes together (plant and animal
   timescales tuned jointly), each on the same training seeds and all three
   apex arms. A small GA stage is allowed only if the designed screen finishes
   inside the budget with time to spare, and is reported as a separate stage.
   Do not expand every axis at once.
6. **Held-out validation**: the baseline and at most two shortlisted candidates
   on `HELDOUT_SEEDS[..4]` at a predeclared longer horizon (twice the campaign
   horizon is the default), all three apex arms. Do not tune on these results.
   If nothing is plausible, say so and name the smallest supported next
   intervention.
7. **Result note** `design/7_Research/ecology-v1-calibration-2026-09-15.md`:
   build and commits; the checkpoint; the smoke; the baseline; a **trade-off
   table** (rows candidates, columns the separate components: population by
   guild, births and deaths by cause, foliage and living/dead wood, plant
   deaths/recolonisation, intake by food, plant production against edible leaf
   replacement, actual body bills, depletion/recovery events, spatial use, apex
   kills/survival/reproduction, imported apex stocks) with late-window values
   and held-out outcomes; predator effects compared across candidates, not
   only against the default; the missing population feedbacks you can name from
   the measurements (with the evidence); extinctions, censoring, invalid
   accounting and refused configurations reported explicitly; actual wall time
   per stage and total, worker count, and model usage if exposed; the exact
   commands to reproduce every stage.
8. `cargo test -p cubarium-search` and `cargo test -p cubarium-core` green;
   `graft build` after the code change; commit.

## Constraints

- No change to `fields.rs`, the settlement in `step.rs`, `genome.rs` decode,
  `controller.rs`, `neural/`, `es/`, the snapshot schema, or any §4–§10
  equation. Diagnostics may be added to `Telemetry`/`IntakeDiagnostics` and to
  events; nothing persisted, nothing hashed, no behaviour change (the
  `hunter.rs` observation test and `ecology_hash` tests must still pass).
- No culling, quotas, free replenishment, or controller rules that protect
  plants. No apex architecture, flesh-capability or apex-controller change; the
  apex encounter and dormancy constants stay `pub const` and unsearched (report
  their effect instead).
- Compute: smoke ≤ 5 wall minutes; the whole simulation workload of this brief
  (smoke, baseline, screen, apex arms, held-out) ≤ 60 wall minutes on ≤ 8
  workers. Stop at the cap with incomplete evaluations reported. No background
  run left behind when you return; no automatic extension. Ask nothing; if
  the matrix does not fit, cut candidates before shortening every horizon.
- Storage: `runs/ecology-v1-calibration/` under 50 MiB; compact JSONL and
  summaries only; no per-tick archives. One normal `target/` cache.
- Old fixtures, old scores and old policies are not evidence. No training, no
  display change, no world reset, no captures.

## Decision authority

Yours: module layout, subcommand names, candidate design, horizon and sampling
cadence within the budget, the guild and depletion-event definitions (state
them), the apex introduction tick. Fable's: any change to an equation, an
ordering, a §11 value in the shipped defaults, `view.rs`, or the trainer.
Wrysk's: nothing in this brief needs him.

## Verification

```bash
cargo test -p cubarium-search
cargo test -p cubarium-core
./target/release/cubarium-search params
./target/release/cubarium-search calibrate --help      # or whatever you name it
```

Fable's review re-derives one candidate row from its `param_bits` (replay),
checks that the trade-off table's components sum consistently with the ledger
(births = deaths + Δpopulation per guild; intake by food ≤ plant production +
initial stocks), and reads the harness diff for any equation touch.

## Return format

The result note, plus in the final message: commit hashes, test totals, the
checkpoint (matrix, predicted vs actual wall time), the trade-off table in
compact form, the held-out outcome, the selected config path and hash (or the
statement that none was selected and why), the missing feedbacks named, and
measured usage. Link files; paste no logs.

## Stop

Stop after the note and commit. Training (C), presentation (B) and deployment
(D) are separate assignments.
