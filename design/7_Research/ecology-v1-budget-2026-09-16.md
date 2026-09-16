---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The per-body store budget, one matched feasibility experiment, and the apex opportunity audit

Workstream E of the ecology v1 next steps
([brief](../handoffs/ecology-v1-budget-opus-2026-09-16.md)), which is step 1 and step 5 of
[Astra's reordering](ecology-v1-next-review-2026-09-15.md) ("Next steps: Astra's opinion").
Two questions were confounded and are not any more:

1. **Is the body's energy budget what ends the ES runs, or is it the controller?** The
   training note compared 0.6436 m of served material against a 2.4740 e upkeep bill, which
   are different units, so the question was open.
2. **Why does no pair of introduced apex adults ever mate?** The screen recorded the outcome
   and named the 10 px mating radius. It recorded none of the six other terms of the
   predicate.

Both are now measured, at the sites the world performs them, and the answers are clean.

- The **body is feasible** on these layouts and the **controller is what binds** — and its
  failure is not a failure to relocate. Generation 9 travels *more* than the control that
  survives and takes in fifteen times less.
- The apex **radius is the wrong knob**: two adults were never simultaneously eligible, in any
  of eight runs, because every one of them died at 43–59 % of the minimum reproduction age
  its own profile demands.

## Build and provenance

- Core diagnostics and their tests: `46ac238`.
- The two experiment commands and their tests: `3bfb602`.
- Both artifacts were produced by search build id `3bfb602e41cd-dirty`, i.e. the tree at
  `3bfb602` (the "dirty" is other workers' uncommitted design documents in the shared
  checkout, not source).
- Ecology: `runs/ecology-v1-calibration/selected/fast-leaf.toml`, config hash
  `09e244392ec91768` — the hash the generation-9 policy records, so
  `PolicyFile::check_ecology` accepts it. `baseline.toml` is `fc1aefa33ebd70a1`. Every audit
  row re-derived the declared screen candidate at its own seed and confirmed the loaded
  configuration is bit-for-bit that candidate (`matches_screen_candidate: true`, 8 of 8).
- Outputs are under `runs/`, which is git-ignored by design; the tables below carry what they
  say and the commands below reproduce them.

```bash
cargo run --release -p cubarium-search -- es-budget \
  --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
  --policy runs/es-eco-v1-fastleaf/selected/center-00009-policy.json \
  --workers 8 --out runs/ecology-v1-budget/feasibility.json

cargo run --release -p cubarium-search -- apex-audit \
  --config runs/ecology-v1-calibration/selected/baseline.toml,runs/ecology-v1-calibration/selected/fast-leaf.toml \
  --seeds 4 --apex 2 --ticks 180000 --introduce-tick 6000 \
  --workers 8 --out runs/ecology-v1-budget/apex-audit.json
```

Wall time of the two retained runs: 5.6 s for the feasibility experiment (48 episodes,
8 workers) and 28.5 s for the audit (8 runs of 180,000 ticks, 8 workers). Including one
earlier audit run, discarded and repeated so both artifacts carry the same build stamp, the
whole workstream spent about 63 s of simulation against the brief's 16-minute cap. The two
JSON artifacts total 108 KiB against a 20 MiB cap.

## What the budget records, and which contract section each line implements

`cubarium_core::BodyBudget` is a per-organism ledger, accumulated from birth, in the passes
that already perform each transaction. Nothing is reconstructed and nothing is divided.

| Line | What it is | Contract |
| --- | --- | --- |
| `served[ch]` | the bite `q` that actually left `P`, `F`, `D` or `C` | §6.4 |
| `digestible[ch]` | `q_d = cap·q` — what this body's machinery could work on | §6.1, §6.4 |
| `reserve_credit[ch]` | `η_m′·q_d` into `R`, with `η_m′ = η_m·min(1, ρ/e_r)` for detritus | §6.4 |
| `battery_credit[ch]` | `min(η_e·(ρ·q_d − e_r·η_m′·q_d), E_max − E)` into `E` | §6.4 |
| `gut_*_credit` | the same two, from digesting gut contents (hunters only) | §6, hunter ext. |
| `oxidation_reserve_burned` / `oxidation_battery_credit` | `R → N` at `oxidation_rate`, `η_ox` of `e_r` per unit to `E`, both passes that can do it | §7 |
| `upkeep_billed` | `(maintenance·S + sense_cost·r_sense)·dt` | §7 |
| `motor_translation_billed` / `motor_turn_billed` | `move_cost·S·\|v\|·dt` and `move_cost·S·k·sweep·dt` | motor contract |
| `bill_total` / `bill_paid` | what the tick's `MotorBill::total_cost` owed, and what the body could raise | §7 |
| `growth_material` / `growth_energy` | reserve into structure, and the `build_cost` energy | §7 |
| `reproduction_material` / `reproduction_energy` | escrow out of the stores; a refund is negative outlay | §7 |
| `injury_structure` | structure lost to apex combat | apex encounters |
| `end_*`, `closed_tick`, `death_cause` | the terminal stores the world booked, and why | §8 |

Because every write to `S`, `R` and `E` in the tick is recorded, the ledger closes:

```text
material:  Σ reserve_credit + gut_reserve_credit
             − oxidation_reserve_burned − reproduction_material − injury_structure
           = Δ(structure + reserve)

energy:    Σ battery_credit + gut_battery_credit + oxidation_battery_credit
             − bill_paid − other_energy_paid − growth_energy − reproduction_energy
           = Δ(energy)
```

Both hold to **8.2e-14 … 3.9e-12** across all 48 experiment episodes, and to 1e-10 in the
core test over a 9,000-tick ordinary world including bodies that ate, grew and conceived.

**The accumulator is not persisted.** It lives on `World`, not on `WorldState`; the snapshot
schema is unchanged; a reloaded world starts it empty. **It is off by default** and opted into
with `World::record_body_budgets(true)`. Cost, measured best-of-five alternating A/B (the
machine is shared by three workers and one `target/`, so a single pair of timings is noise):
**−0.84 %** of tick throughput on the shipped-defaults whole world (46–255 bodies, 8,567 →
8,495 ticks/s) and **−2.28 %** on a plant-only world of the B0 shape (10,933 → 10,684
ticks/s), both inside the brief's 3 % ceiling. The plant-only shape is the more affected one
only because its tick is cheaper, so the two fixed per-tick passes are a larger share of it;
B0 itself has no animals, so per-organism recording costs nothing there by construction, and
the whole-world number is the one to quote. Reproduce with
`cargo test -p cubarium-core --release --test body_budget -- --ignored --nocapture`.

Off is nonetheless the default, for a reason that is not about speed: a display world nobody
drains would hold one closed record per death for as long as it runs. An undrained recorder
now keeps at most 4,096 closed records and counts what it dropped.

**Inertness.** `crates/cubarium-core/tests/body_budget.rs` steps the same seeded world 9,000
ticks three ways — never recording, recording untouched, recording read and drained every
single tick — and requires one `snapshot::state_hash`; a fourth test toggles recording on and
off mid-run and requires the same. On the ES side,
`crates/cubarium-search/tests/budget_feasibility.rs` requires that a measured episode is
`assert_eq!`-identical to the episode the trainer would have scored, field for field.

## How the ratio is defined and windowed

**Energy income** is what a bite is worth to the battery once oxidation has converted what it
put into the reserve:

```text
income = Σ_ch battery_credit  +  η_ox · e_r · Σ_ch reserve_credit      (η_ox = 0.8, e_r = 2.0)
```

**Energy bill** is `bill_total + other_energy_paid + growth_energy + reproduction_energy`. The
last three are zero throughout this fixture (an adult body, `bud = false`, no combat, no gut),
and are summed anyway so the ratio means the same thing where they are not.

Both ratios are taken over **2,000 ticks (100 s)** of the *cumulative* trace, differenced —
never an average of per-tick ratios. `ratio_last_window` is the last 2,000 ticks alive;
`ratio_best_window` is the largest such window anywhere in the life, which is the charitable
reading Astra's branch test names. A life shorter than one window reports the whole life and
says so in `window_ticks`.

**One thing to read carefully.** Over any window in which the stores return to the same
values, income equals the bill *identically* — that is the energy identity with `ΔE = ΔR = 0`.
So a trailing ratio of exactly 1.000 means the body was in a stores-saturated steady state,
not that it was barely scraping by. Every surviving mobile-script body ends at `R = R_max`
with `E` parked at `0.5·E_max`, the oxidation threshold: its intake is limited by its own
storage, not by the food. For a **surviving** body the informative number is therefore
`ratio_best_window`; for a **dying** body the trailing window is the informative one, and it
is what the body was living on at the end.

## The feasibility experiment

`fast-leaf`, the 4 training and 8 held-out layouts, one episode each, horizon 36,000, four
drivers. Medians over the twelve layouts, with `[min–max]`.

| driver | survived | lifetime (ticks) | best 2 k window | trailing window | served (m) | distinct cells | body lengths |
| --- | --- | --- | --- | --- | --- | --- | --- |
| stationary-grazing | **0 / 12** | 10,441 [9,090–11,356] | **1.489** [0.829–1.828] | 0.000 | 0.98 [0.54–1.28] | 1 | 0 |
| mobile-script | **11 / 12** | 36,000 [26,355–36,000] | **1.788** [1.746–2.034] | 1.000 | 12.21 [7.39–12.51] | 52 [37–303] | 275 [79–1,060] |
| initial centre (seed 20260915) | 0 / 12 | 7,239 [6,456–9,361] | **0.333** [0.031–0.615] | 0.001 | 0.31 [0.02–1.23] | 148 [63–215] | 371 [331–478] |
| generation 9 | 0 / 12 | 8,421 [6,914–9,429] | **0.526** [0.183–0.812] | 0.212 | 0.83 [0.21–1.22] | 254 [168–290] | 442 [360–496] |

Every death is `Starvation`. Largest identity residual over all 48 rows: `3.87e-12`.

The bill, split at the site that levies it (medians, e over the life):

| driver | upkeep | translation | turn | total | motor share |
| --- | --- | --- | --- | --- | --- |
| stationary-grazing | 3.237 | 0.000 | 0.000 | 3.237 | 0 % |
| mobile-script | 11.160 | 0.247 | 0.028 | 11.375 | 2.4 % |
| initial centre | 2.244 | 0.334 | 0.026 | 2.605 | 13.8 % |
| generation 9 | 2.611 | 0.398 | 0.022 | 3.031 | 13.9 % |

Served material by channel, summed over the twelve layouts (m):

| driver | foliage | fruit | litter | carrion |
| --- | --- | --- | --- | --- |
| stationary-grazing | 11.571 | 0 | 0 | 0 |
| mobile-script | 141.131 | 0 | 0 | 0 |
| initial centre | 2.386 | 1.260 | 1.119 | 0 |
| generation 9 | 4.183 | 2.598 | 2.652 | 0 |

The two controls are scripted to graze only, so their zeros are the script and not the body.
The two policies are not, and they split their intake roughly 44 / 28 / 28 across foliage,
fruit and litter — which is what a `diet = 0.7` generalist with `cap_foliage = 0.7` and
`cap_detrital = 0.3` looks like from the inside, and is a direct measurement of the body the
training note misnamed.

### Astra's three branches

**Branch 1 — "the mobile control also dies with its sustained ratio < 1 while reaching food,
so the body's budget binds." Refuted.** The mobile script survives the full horizon on 11 of
12 layouts, with a best 2,000-tick ratio of 1.75–2.03 and a trailing ratio of 1.000 at
`R = R_max`. The body can pay for itself on these patches with room to spare. The training
note's "the energy budget, not the search, is what ends the run" does not survive the
measurement it lacked.

**Branch 2 — "it survives while generation 9 dies, so the controller or the 16-update search
binds." Supported, and sharper than stated.** Generation 9 dies on all twelve layouts and its
*best* 2,000-tick stretch anywhere in any life reaches only 0.812; its median best is 0.526.
It is never once, even briefly, taking in what it is spending. But it is **not** failing to
move: it visits 254 distinct cells (median) against the surviving control's 52, and travels
442 body lengths against 275. It moves more, over more ground, and takes in **fifteen times
less** material. The controller's failure is a failure to *feed*, not a failure to relocate.
The initial centre is worse on every column, so the sixteen updates did buy something — median
best window 0.333 → 0.526, median lifetime 7,239 → 8,421 ticks — just nowhere near enough.

**Branch 3 — "stationary dies and mobile survives, so relocation is necessary." Also
supported, and it is compatible with branch 2.** The stationary grazer's best window is 1.489
(median; above 1.0 on 11 of 12 layouts): standing on an opening patch, it pays for itself
comfortably *while the patch lasts*. Its trailing window is exactly 0.000 on every layout — by
the end it is taking nothing at all — and it starves at ~10,400 ticks having eaten 0.98 m from
one cell. So on `fast-leaf` a single cell funds a body for a few thousand ticks and then
stops. Relocation is necessary; the mobile script shows it is also sufficient.

**The combined answer.** The body's budget is not binding, relocation is required and
sufficient, and the trained controller relocates energetically while failing to eat. Branches
2 and 3 are both true and they are not in tension: the task is "walk to food and crop it", the
control solves it, and generation 9 solves only the walking half.

### One correction to the record, for free

The review noted that "the route stock itself falls from roughly 34 to 14 while the policy is
present". It does — but not because anything ate it. The initial-centre driver takes a median
of **0.31 m** over its whole life and its route still falls from 34.47 to 14.55 by the time it
dies at ~7,200 ticks. A 0.31 m bite cannot account for a 20 m decline. The fall is the
fixture's painted stands relaxing toward their own equilibrium under `fast-leaf`, and it
happens to every driver at roughly the same rate. Grazing is visible on top of it — the mobile
script, eating 12.2 m over 36,000 ticks, ends at 11.77 against the initial centre's 14.55 —
but it is the smaller term, and the layouts are not a depletion measurement.

## The apex opportunity audit

The screen's two-apex arm, re-run exactly: same candidate configuration, same held-out seeds,
two adults introduced at tick 6,000 at the same deterministic placements, never restocked,
horizon 180,000.

| config | seed | apex A life | apex B life | ticks both adult | ticks both perched | ticks **both ready** | candidate pairs | min ready distance |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 9001 | 10,575 | 13,819 | 10,575 | 5,251 | **0** | 0 | — |
| baseline | 9002 | 10,888 | 13,436 | 10,888 | 5,477 | **0** | 0 | — |
| baseline | 9003 | 11,369 | 14,101 | 11,369 | 5,245 | **0** | 0 | — |
| baseline | 9004 | 12,995 | 12,706 | 12,706 | 5,264 | **0** | 0 | — |
| fast-leaf | 9001 | 12,859 | 13,456 | 12,859 | 5,248 | **0** | 0 | — |
| fast-leaf | 9002 | 10,315 | 10,685 | 10,315 | 5,245 | **0** | 0 | — |
| fast-leaf | 9003 | 10,541 | 10,893 | 10,541 | 5,244 | **0** | 0 | — |
| fast-leaf | 9004 | 10,534 | 12,541 | 10,534 | 5,246 | **0** | 0 | — |

Lifetimes are ticks alive after introduction. Both members were adult for their whole lives
(`adults_alive_ticks == members_alive_ticks` in every run). Worlds did not collapse: 43–71
organisms alive at the horizon, mass residuals `≤ 2.9e-10`.

**Verdict: readiness overlap is exactly zero, so the mating radius cannot be the deciding
constant.** Not one member was `may_reproduce`-ready on any tick of any run — `max_ready = 0`,
eight runs, 180,000 ticks each. The reason is in the profile and the lifetimes:
`FixedHunterProfile::lanternjaw_trial` sets `reproduce_min_age_seconds = 1200`, which is
**24,000 ticks**, and the oldest apex any run produced reached **14,101**. Every introduced
adult died at 43–59 % of its own minimum reproduction age. The distance between them was never
consulted, because the pass never had two ready members to consult it about.

A second, independent fact points the same way: **zero candidate pairs formed at all**, over
5,244–5,477 ticks per run in which both members were simultaneously adult and `Perched`. A
candidate pair requires that each sensed the other; they never did. So even if the age gate
were removed, these two would have had to find each other first, and on this evidence they do
not.

This is the audit Astra asked for before the radius becomes an owner-facing choice, and its
answer is that it should not become one yet. Moving `MATING_RADIUS_PX` from 10 px to any value
would have changed nothing in any of these eight runs.

## What this does not establish

- **Nothing about the whole world's foraging.** The feasibility experiment is twelve frozen
  single-body layouts on one ecology. It says a *body* can pay for itself there, not that the
  population in a whole `fast-leaf` world can.
- **Nothing about a different controller.** Four named drivers were compared. A fifth could
  beat the mobile script, or a longer horizon could expose the mobile script — it already
  fails one layout (h2-holdout, dead at 26,355) and finishes h6-holdout alive but with an
  empty reserve and a trailing ratio of 0.234.
- **Nothing about why generation 9 does not eat.** The measurement localises the failure to
  intake rather than travel. It does not say whether the mouth efforts are off, whether the
  body is on food when they are on, or whether the `t_min` survival score rewards wandering.
- **Nothing about the apex beyond eligibility.** The audit says no member was ever ready and
  that the age gate alone is sufficient to explain it. It does not measure whether the stock
  fractions (`reserve ≥ 0.8·R_max`, `energy ≥ 0.75·E_max`) would also have failed, nor why an
  introduced adult dies at ~11,000 ticks against a 7,200 s natural lifespan.
- **Nothing about other configurations.** Two configurations and four seeds for the audit; one
  configuration for the feasibility experiment.
- `cargo clippy -p cubarium-core --all-targets` fails on a pre-existing `erasing_op` deny in
  `crates/cubarium-core/src/neural/gru.rs:233`, which this workstream did not touch and did
  not introduce.

## Verification

`cargo test -p cubarium-core` 476 passed / 0 failed / 3 ignored; `cargo test -p
cubarium-search` 94 / 0 / 0; `cargo test -p cubarium --release --test run_neural_seed` 7 / 0.
`graft build` rebuilt the graph (6,636 nodes, 13,859 edges). The shared `target/` is used by three
concurrent workers, and twice during this session a build of `cubarium-search` failed with
`unresolved import cubarium_core::BodyBudget` although `cargo test -p cubarium-core` had just
passed 476 — a stale `cubarium-core` artifact that cargo considered fresh. `touch
crates/cubarium-core/src/lib.rs` and rebuild clears it; both experiment runs above used a
private `CARGO_TARGET_DIR` for the same reason. It is a build-cache race, not a source
problem: the committed tree passes all three suites.

## The next tasks this implies

Named, not launched.

1. **Diagnose generation 9's intake, not its movement.** The cheapest informative measurement
   is now available: with the ledger on, record per tick whether the body stands on a cell with
   `P` above `feed_min`, what its three mouth efforts were, and what it took. Three outcomes
   separate cleanly — the efforts are off; the efforts are on and the body is never on food; or
   both happen and the bite is being clamped. That decides whether the next move is the score,
   the observation, or the action adapter, and it is a diagnostic rather than another search.
2. **Decide the apex eligibility question before the radius question.** The measured obstacle
   is that an introduced adult lives ~11,000 ticks and needs 24,000 to be eligible. The
   owner-facing choice is between lowering `reproduce_min_age_seconds`, making an introduced
   apex survive longer, and introducing adults that are already past the gate. Which of those
   is right depends on why it dies at 11,000 ticks, and the per-body ledger now answers that
   for an apex exactly as it does for a grazer.
3. **An input for the movement-cost arm (workstream F).** On this fixture the motor is
   **13.9 %** of a wandering body's whole bill (0.420 e of 3.031 e) and maintenance plus
   sensing is the rest; translation outweighs turning 18 : 1. Astra's proposed `move_cost`
   brackets of 0.0018 and 0.006 against the current 0.00036 would take that share to roughly
   44 % and 72 % for the same trajectory. That is a large intervention, and the arm should be
   read knowing it.
