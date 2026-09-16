---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1 — the finer movement-price ladder, gated on the grazer

Workstream I, under
[the ladder brief](../handoffs/ecology-v1-ladder-opus-2026-09-16.md): step 2 of the
reconciled next steps in
[the next-steps result](ecology-v1-next-steps-results-2026-09-16.md), from
[Astra's review](ecology-v1-next-steps-review-2026-09-16.md) findings 1–2 and
next-steps item 2.

Evidence, not a decision. Nothing here changes an equation, a §11 shipped default,
an ordering, the motor contract or the snapshot schema, and `cubarium-core` is not
modified at all. The accepted ecology v1 (`design/ecology-v1-contract.md`,
schema 16), [workstream A's calibration](ecology-v1-calibration-2026-09-15.md) and
[workstream F's movement matrix](ecology-v1-movement-2026-09-16.md) are the
baseline and are not re-reviewed here.

**The question.** F charged `organism.move_cost` at 0.00036 (shipped), 0.0018 and
0.006 and found that the price buys range and buys local depletion, monotonically,
and buys almost no recovery — 13 recovery crossings in 72 raised-price runs. Two
things stopped F's matrix from answering whether concentrated grazing produces the
depletion/recovery cycle ecology v1 was built to show:

1. **Every raised price kills the founder grazers before their first brood.** 180
   founder grazers, zero grazer births, mean age at death 179.6 s at 0.0018. The
   matrix contains no observation of a world with concentrated grazing *and* a
   breeding grazer. The response between 0.00036 and 0.0018 is unsampled.
2. **Why a depleted cell stays below 50 % of its opening foliage is unresolved.**
   F recorded neither the depleted cells' own `L·μ` and trajectory nor their
   post-depletion visits and bites, so slow regrowth, returning consumers and
   intrinsically poor cells cannot be told apart (Astra's finding 2).

This workstream samples the gap — 0.0006, 0.0009, 0.0012 between F's two lowest
levels — with the founder **grazer's** first completed brood as the gate
(Astra's finding 1: "first herbivore brood" is already satisfied at `fast-leaf`
× 0.0018 by the glider, so the gate must name the grazer), and adds the
per-depleted-cell record the four-way reading needs.

---

## Pre-registration

*Everything in this section was written and committed before any row of this
campaign was produced. The commit that carries it is named under "Build and
commits"; nothing below it existed when it was written.*

### The arms

One knob, five levels, crossed with two configurations and A's six training
seeds, at apex arm 0 only.

| axis | levels |
| --- | --- |
| `organism.move_cost` (e per unit structure per pixel) | **0.00036** (shipped default, the control), **0.0006**, **0.0009**, **0.0012**, **0.0018** |
| configuration | `baseline` (shipped §11 defaults), `fast-leaf` (A's selected configuration: `plant.foliage_rate` 0.006, `plant.maintenance` 0.0001, `plant.reserve_share` 0.35) |
| seed | `TRAINING_SEEDS[..6]` = 1001–1006 |
| apex arm | 0 only |

5 × 2 × 6 = **60 trials**, horizon 180,000 ticks (150 simulated minutes),
sampling every 600 ticks, foraging probe every 20 ticks, `lanternjaw_trial`
profile unsearched, **per-body budget ledger on**. Every other condition of A's
screen and F's matrix is unchanged.

Why arm 0 only: A measured every paired apex difference at under 0.2 seed
standard deviations and F's matrix reproduced that, so the apex arms buy no
information about the price and cost three times the compute. The two prices
this campaign shares with F (0.00036 and 0.0018) are what tie it to F's matrix,
and they are tied by state hash, not by a mean.

Why these three interior levels: 0.0006, 0.0009 and 0.0012 are the three equal
steps of 0.0003 that fit between the shipped price and F's lowest raised level,
so the ladder is a regular grid and no level is chosen after seeing a row.

### The measures: the per-depleted-cell record

Stated here as definitions, because the measures are new and a definition
written after seeing the rows is not a measurement. F's measures (visit,
residence, revisit interval, per-cell crossings, census, terminal stores) are
unchanged and are restated only where this record reads them.

A cell is **watched** if its tick-0 foliage exceeds `1e-9`, **depleted** below
`0.25 × P₀` and **recovered** above `0.5 × P₀`, with `P₀` its own tick-0
foliage — A's thresholds and A's hysteresis, unchanged. A record is opened for a
cell the first probe it crosses to depleted, and never re-opened; later
crossings in the same cell increment that record's counters.

- **`L·μ`, the cell's habitat quality.** The **static** habitat product
  `light_base[i] · moisture_base[i]`, reconstructed on the search side by
  `cubarium_core::habitat::Habitat::new(&config.habitat, config.seed)` —
  a pure function of config and seed, which is exactly how
  `World::new` builds the habitat it then uses. It excludes two things and says
  so: the slow weather perturbation `Weather::sample` adds each tick (world-wide
  blobs, not a property of the cell), and the water factors `algae_light` and
  `water_factors` apply on top (a pool's algae floor, `wet_gain`, `drown`).
  It is the quantity §11 uses to set the cell's opening wood, and the
  reconstruction is **checked bit-exactly in every run**:
  `fields::initial_wood(&config, L₀[i], μ₀[i])` must equal
  `world.state.ecology.wood[i]` at tick 0 in all 1,280 cells. A run whose
  reconstruction does not match is reported as a failed check, not interpreted.
- **`(L·μ)_crit`, the cell's critical habitat quality.** The contract names no
  such constant, so it is *derived* here from the contract's own breakeven line
  (§13, "Income then has to cover senescence:
  `c·P − m_w·W* = (1 + c_g)·m_p·P`") evaluated **at the depletion threshold**,
  which is the point a depleted cell has to climb away from:

  ```
  (L·μ)_crit = [ (1 + c_g)·m_p + m_w·W₀ / (0.25·P₀) ] / ( g · monod_ref )
  ```

  with `c_g = plant.build`, `m_p = producer.mortality`,
  `m_w = plant.maintenance`, `g = producer.growth`, `W₀` the cell's tick-0 wood,
  `P₀` its tick-0 foliage, and `monod_ref = 0.4/(0.4 + K_N) = 0.615` the
  contract's own §13 reference nutrient. At the shipped defaults and an
  α-limited cell (`P₀ = 0.4·α·W₀`) the wood term is cell-independent
  (`m_w/(0.1·α)`) and the value is **0.447** for `baseline` and **0.346** for
  `fast-leaf` (whose `plant.maintenance` is halved); it is computed per cell
  anyway so the `P_max`-limited cells are handled.

  Three things make this threshold **permissive** — it calls a cell adequate
  more readily than the true condition would — and all three are stated rather
  than buried: it uses the contract's reference nutrient rather than the cell's
  own (a poorer cell has a higher true threshold); it takes the contract's
  breakeven with the reserve full, so the `q_share` cut off the top of every
  surplus is not charged; and it ignores fruit ripening, which is a further
  sink. A cell classified **marginal** by it is marginal under a generous test.
- **`P/P₀` after depletion.** Sampled at every tick divisible by 600 strictly
  after the cell's first depletion tick, in order, capped at 300 samples (the
  whole horizon). Stored as `f32` because it is a ratio read to two decimals.
- **The last consumer visit before depletion.** At each probe the recorder
  writes, for every cell holding at least one prey body, that probe's tick and
  that probe's foliage. A record opened at probe `k` snapshots the values
  standing after probe `k`'s positions were read, so a body standing in the cell
  as it crosses counts as the last visit and the recorded stock is the
  just-depleted stock — reported as `last_visit_tick`, `last_visit_stock` and
  `ticks_since_visit_at_depletion`, which is 0 in exactly that case.
  `last_visit_tick` is `null` for a cell no prey body was ever observed in.
- **Post-depletion visits.** Probes, strictly after the record's first depletion
  tick, at which at least one prey body was observed in the cell
  (`post_visit_probes`), and the sum over probes of bodies observed there
  (`post_visit_body_probes`). One second of resolution, as F's visits are.
- **Post-depletion served material.** E's ledger records `served[FOLIAGE]` per
  **body**, not per cell, and per-cell served is not available without a core
  change. It is recovered by attribution: at each probe the recorder differences
  each live body's `served[FOLIAGE]` against its value at the previous probe and
  credits the difference to the cell that body is observed in **now**. A body
  seen for the first time is differenced against zero, so its first credit
  carries everything it ate since birth; a body that dies between probes is
  differenced against its closed record's final `served[FOLIAGE]` and credited
  to the cell it was **last** observed in. The attribution has one-second
  resolution and is therefore wrong for the fraction of a body's eating that
  happened in a cell it left inside the probe interval; at F's measured
  residences (4–16 s) that is at most one probe in four to sixteen. It is called
  *attributed* served material everywhere and never *measured per cell*.
- **First recovery and first re-depletion.** The tick of the record's first
  crossing above `0.5·P₀` after its first depletion, and the tick of its first
  crossing back below `0.25·P₀` after that recovery. Both `null` if they did not
  happen.
- **Per-record extremes.** `P/P₀` at the horizon, and the largest and smallest
  `P/P₀` observed at any probe after the first depletion.

**Bounds.** At most 512 records are opened per run, in the order cells first
deplete; a run that would open more counts them in `records_dropped` rather than
silently truncating. F's matrix depleted at most 55.9 cells per run at these
prices, so the cap is not expected to bind and is a storage guard, not a
sampling rule.

### The measures: the founder grazer's brood, and the net margin

- **Completed founder-grazer brood.** A `LifeEvent::Birth { id, parent }` whose
  `parent` is in the tick-0 founder set **and** whose parent's census key has
  `form == 0` (the grazer rig; the founder roster's form mapping is measured at
  tick 0 in every run, never assumed). The core emits `Birth` when gestation
  completes and the child is committed, so a birth event *is* a completed brood
  — nothing is inferred from a reserve or an age. Reported per run:
  `founder_broods[form]` for all five forms, the first such tick per form, and
  the number of distinct founder parents that produced one. The glider (form 1)
  is reported beside the grazer, as F's next-task note asks.
- **Net energy margin per body.** From E's ledger, over every prey body whose
  record closed (it died) plus every prey body alive at the horizon:

  ```
  e_food_in = Σ_channels ( battery_credit + e_r · reserve_credit )
              + gut_battery_credit + e_r · gut_reserve_credit
  e_owed    = bill_total + other_energy_paid + growth_energy + reproduction_energy
  margin    = e_food_in − e_owed
  ```

  `bill_total` is what the body **owed**, not `bill_paid` (what it could raise);
  that is the point of the measure — a starving body's unpaid bill is exactly
  its deficit. `e_r · reserve_credit` values reserve material at its stated
  density rather than at the 0.8 the later oxidation returns, so `margin` is an
  **upper bound** on the energy the body could actually have spent. Reported as
  a mean per body and a mean rate per second of recorded life, binned by
  `(form, diet bin)` — the census's bins, unchanged — with `bill_total`,
  `bill_total − bill_paid`, the motor share `(translation + turn)/bill_total`
  and `served_total` beside it so a reader can recompute.
  `records_dropped` from the ledger's drain is reported; a run that dropped any
  record says so rather than reporting a mean over an unknown denominator.

**The ledger must change no dynamics.** E's module states that the flag is read
only at sites that add to a counter, consumes no draw and moves no value the
simulation reads. This campaign does not take that on trust: the row check
below compares 24 of this campaign's rows against F's, which were produced with
the ledger **off**. If enabling the ledger moved a world, that check fails and
the campaign stops.

### The four-way classification of a depleted cell

Astra's four readings, made into an **exhaustive and disjoint** priority
classification over the cells that opened a record. Each cell gets exactly one
class, tested in this order:

1. **`Recovered`** — the cell crossed back above `0.5·P₀` at least once after
   its first depletion. Recovery is possible here and the question is its rate,
   not its existence: the *time-scale* reading. Reported with the latency
   `first_recovery_tick − first_depletion_tick` and whether it re-depleted.
2. **`Pressure`** — not recovered, and attributed post-depletion served foliage
   `≥ 0.25 · P₀`: after the cell was already down to a quarter of its opening
   leaf, mouths took at least another quarter of that opening leaf out of it.
   The threshold is the depletion fraction itself, so "pressure" means the
   post-depletion take is at least as large as the whole standing stock the
   threshold leaves. The *continued bites* reading.
3. **`PlantLimited`** — not recovered, post-depletion take below that, and
   `L·μ ≥ (L·μ)_crit`: pressure stopped and the cell still did not regrow
   although its habitat is adequate under the permissive test. The *plant
   equation or parameters* reading.
4. **`Marginal`** — not recovered, post-depletion take below that, and
   `L·μ < (L·μ)_crit`. The *intrinsically poor cell* reading.

Because a cell can be both marginal and under pressure and the priority order
resolves it to `Pressure`, three further fractions are reported over **all**
records, so the reading can be re-sliced without re-running: the fraction with
`L·μ < (L·μ)_crit`, the fraction with any attributed post-depletion bite at all
(`> 1e-9`, the sensitivity on rule 2's threshold), and the fraction with no
post-depletion visit at all.

### The confirmation and refutation rules

From the brief, evaluated separately for each of the ten
`(configuration, price)` configurations, each against **its own
configuration's 0.00036 control**, over that cell's 6 seeds.

Let `C₀` be the control's mean late-window `cells_per_body_window_late` and
`C_p` the arm's.

- **(i) The grazer breeds.** At least one completed founder-grazer brood in
  **≥ 4 of 6** seeds.
- **(ii) Range falls.** `C_p < 0.60 · C₀`.
- **Confirmation of a viable spatial intervention at a price**: **(i) and (ii)**.
- **Informative about recovery**: confirmed, **and additionally** ≥ 1 recovery
  crossing per run in **≥ 4 of 6** seeds.
- **Refutation of the ladder**: **no** price meets (i) and (ii) together.

The 0.00036 control is scored against the same rules for completeness; it
cannot satisfy (ii) against itself and is reported as *control*.

Verdict names, per configuration: **CONFIRMED** (i and ii), **CONFIRMED +
INFORMATIVE** (i, ii and the recovery clause), **PARTIAL** (exactly one of i,
ii), **NEITHER** (neither).

### The reproduction check, and the stop rule

**The row check.** Every one of this campaign's 24 rows at `move_cost ∈
{0.00036, 0.0018}` must carry the same `metrics.final_state_hash` as F's
retained arm-0 row for the same candidate, seed and price
(`runs/ecology-v1-movement/matrix/evals.jsonl`, copied to
`runs/ecology-v1-ladder/f-movement-evals.jsonl`). F's rows were produced with
the ledger off and without the per-depleted-cell record, so this check is
simultaneously the control check and the test of E's dynamics-neutrality claim.

**If it fails, the campaign stops and the note reports why rather than
interpreting the ladder.** A control that does not reproduce F's world is not a
control.

A cheaper preflight runs first, so a failure is diagnosed rather than merely
observed: one short world (`baseline`, seed 1001, 20,000 ticks, arm 0) is
evaluated with the ledger off and with it on, and the two `final_state_hash`
values are compared. That separates "the ledger moved the world" from "my
recorder moved the world".

### Budgets

≤ 8 wall minutes of simulation, ≤ 8 workers, `runs/ecology-v1-ladder/` ≤ 40 MiB,
nothing left running. 60 trials × 180,000 ticks = 10.8 M ticks; at the 58,729
ticks/s F measured on 8 workers that predicts **184 s = 3.1 minutes** before the
ledger's and the new record's overhead. The stage runs under a hard
`--wall-seconds 480`; a trial not started by the cap is recorded as skipped and
reported, and no horizon is shortened.

---

*Everything above was written before the campaign was launched. Everything below
is what it measured.*
## Build and commits

| | |
| --- | --- |
| Branch | `worktree-agent-aa271e48ebe0b3ba2` (a worktree of `main`) |
| Parent | `2a1cedd` |
| Pre-registration commit | `c7e498d` — everything above the rule, written before a row existed; no code touched |
| Test-authoring commit | `7164352` — the 28 definition tests against a stub module, 25 of 28 red |
| Implementation commit | `7ce8a6d` — the measures, `--ledger`, `LADDER_PRICES` |
| Result commit | *(this note, below)* |
| Build stamp in every row | **`7ce8a6d7b4a3`** — clean, not `-dirty`: nothing was uncommitted in the row-producing build |
| Host | 8 workers, as the brief caps; peak RSS 23 MiB |

`cubarium-core` is **not modified**; `crates/cubarium-search/src/es/` and
`apex_audit.rs` are not modified. The files this workstream touched are the new
`depletion.rs`, `movement.rs`, `evaluate.rs`, `calibrate.rs`, its one-line module
declaration in `lib.rs`, two lines of `main.rs` (the `--ledger` argument and its
dispatch), and the new `tests/ladder_measures.rs`.

### Tests

| Command | Result |
| --- | --- |
| `cargo test --release -p cubarium-search` | **145 passed**, 0 failed (83 unit, 3 `budget_feasibility`, 4 `es_repair`, 12 `harness`, **28 `ladder_measures`**, 15 `movement_measures`) |

**29 tests are new**: the 28 in `tests/ladder_measures.rs` and one in
`calibrate.rs` (the ladder is the declared five levels, inside the box, sharing
two rungs bit-for-bit with F). The authoring order is on the record rather than
asserted: commit `7164352` carries the 28 tests against a module of
`unimplemented!()` bodies, and 25 of the 28 fail there. The three that pass are
named in that commit's message rather than counted as evidence — they are the
three things that commit implements rather than stubs, and one of them
(`observe_reporting`) is a structural test that cannot fail by construction,
which is the point of it.

**A pre-registration error, disclosed.** The pre-registration calls the ladder
"the three equal steps of 0.0003 that fit between the shipped price and F's
lowest raised level". The three *interior* rungs are 0.0003 apart; the end
intervals are 0.00024 and 0.0006, because the two end rungs are F's and not this
campaign's to choose. The arms are the brief's five levels and are unchanged;
only the sentence justifying them was wrong, and it is left standing above the
rule with this correction below it.

## Exact commands

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo build --release -p cubarium-search
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test  --release -p cubarium-search   # 145 passed

# preflight: does turning the ledger on move the world?
cubarium-search calibrate --stage preflight-off --candidates baseline --seed-set training --seeds 1 \
  --arms 0 --prices 0.00036          --ticks 20000 --sample-every 600 --introduce-tick 6000 \
  --workers 1 --wall-seconds 300 --out runs/ecology-v1-ladder/preflight
cubarium-search calibrate --stage preflight-on  --candidates baseline --seed-set training --seeds 1 \
  --arms 0 --prices 0.00036 --ledger --ticks 20000 --sample-every 600 --introduce-tick 6000 \
  --workers 1 --wall-seconds 300 --out runs/ecology-v1-ladder/preflight

# the ladder: 5 prices x 2 configurations x 6 training seeds x arm 0
cubarium-search calibrate --stage ladder \
  --candidates baseline,fast-leaf --seed-set training --seeds 6 --arms 0 \
  --prices 0.00036,0.0006,0.0009,0.0012,0.0018 --ledger \
  --ticks 180000 --sample-every 600 --introduce-tick 6000 \
  --workers 8 --wall-seconds 480 --out runs/ecology-v1-ladder
```

`runs/ecology-v1-ladder/ladder/{evals.jsonl,summary.json}`, 60 rows.

## The reproduction checks, before anything was interpreted

**1. The ledger moves no bit of the world.** The same short world
(`baseline`, seed 1001, 20,000 ticks, arm 0) run with `record_body_budgets(false)`
and `record_body_budgets(true)` carries the identical `final_state_hash`
`4692485605019165535` and the identical `final_ecology_hash`. It costs 4 % of
throughput (2,689 ms against 2,798 ms). E's claim that the flag is read only at
sites that add to a counter is therefore tested here rather than taken on trust.

**2. The row check passes on all 24 shared rows.** Every `(candidate, seed)` at
`move_cost ∈ {0.00036, 0.0018}` carries the same `metrics.final_state_hash` as
workstream F's retained arm-0 row: **24 of 24, 0 mismatches**. `final_ecology_hash`,
`cells_per_body_window_late`, `prey_births`, `prey_deaths`, `deaths_starvation`,
`final_population` and the whole `crossings` block also match **exactly** on all
24. F's rows were produced with the ledger off and without the per-depleted-cell
record; both arms of this campaign's instrumentation are therefore shown not to
have disturbed the world it measures.

**3. The habitat reconstruction is bit-exact in all 60 rows.**
`fields::initial_wood(config, L₀, μ₀)` from the search-side `Habitat::new` equals
the world's own tick-0 wood in every one of the 1,280 cells of every run:
`habitat_max_wood_error = 0.0`, `habitat_reconstruction_ok = true`, 60 of 60.
The `L·μ` every classification rests on is the world's own, not an approximation
of it.

**Three internal identities hold in all 60 rows**: `cells_cycled == cells_recovered`;
the per-cell records' depletion and recovery counts sum exactly to the aggregate
counters; and `records_dropped` is 0 for both the per-cell record (cap 512, most
in any run 66) and E's ledger (cap 4,096, 0 dropped in 60 runs).

## The ladder

60 trials, **199.5 s = 3.3 wall minutes** on 8 workers, 0 skipped, 0 invalid, 0
failed, 54,134 ticks/s, peak RSS 23 MiB, **0 extinctions**. Each row is a mean
over the 6 training seeds at apex arm 0.

| cand | `move_cost` | cells/body (late) | ÷ control | residence s | revisit s | depletions | recoveries | seeds ≥ 1 rec | **founder-grazer broods** | **seeds that bred** | founder-glider broods | late prey | ΣP/ΣP₀ | leaf eaten (late) | motor % of bill | kinds at end | **verdict** |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | **0.00036** | 303.1 | 1.00 | 5.62 | 233.6 | 11.3 | 0.17 | 1/6 | **1.5** | **2/6** | 6.8 | 42.8 | 2.31 | 336.3 | 11 % | 2.33 | *control* |
| baseline | 0.0006 | 183.1 | **0.604** | 8.81 | 189.5 | 36.0 | 0.33 | 2/6 | **0.0** | **0/6** | 2.2 | 39.0 | 3.13 | 142.6 | 12 % | 1.50 | **NEITHER** |
| baseline | 0.0009 | 170.3 | **0.562** | 9.05 | 179.2 | 41.2 | 0.17 | 1/6 | **1.0** | **1/6** | 1.2 | 39.3 | 3.15 | 133.5 | 15 % | 1.33 | **PARTIAL** |
| baseline | 0.0012 | 52.9 | **0.175** | 11.25 | 140.1 | 56.8 | 0.33 | 2/6 | **0.0** | **0/6** | 0.0 | 37.1 | 3.73 | 0.0 | 14 % | 1.00 | **PARTIAL** |
| baseline | 0.0018 | 53.5 | **0.177** | 12.51 | 157.2 | 56.2 | 0.33 | 2/6 | **0.0** | **0/6** | 0.0 | 33.5 | 3.72 | 4.2 | 19 % | 1.00 | **PARTIAL** |
| fast-leaf | **0.00036** | 377.5 | 1.00 | 4.17 | 266.5 | 0.8 | 0.00 | 0/6 | **8.8** | **6/6** | 12.8 | 60.8 | 2.11 | 526.5 | 12 % | 3.00 | *control* |
| fast-leaf | 0.0006 | 387.7 | 1.027 | 4.02 | 261.0 | 0.8 | 0.00 | 0/6 | **7.7** | **6/6** | 11.7 | 59.7 | 2.11 | 529.1 | 18 % | 3.00 | **PARTIAL** |
| fast-leaf | 0.0009 | 368.9 | 0.977 | 4.92 | 263.3 | 2.5 | 0.00 | 0/6 | **4.7** | **3/6** | 5.8 | 51.3 | 2.33 | 484.6 | 24 % | 2.33 | **NEITHER** |
| fast-leaf | 0.0012 | 252.9 | 0.670 | 7.70 | 223.3 | 5.5 | 0.17 | 1/6 | **1.5** | **1/6** | 2.7 | 41.9 | 2.97 | 332.6 | 26 % | 2.33 | **NEITHER** |
| fast-leaf | 0.0018 | 147.3 | **0.390** | 9.50 | 188.5 | 15.3 | 0.00 | 0/6 | **0.0** | **0/6** | 1.0 | 45.2 | 3.65 | 167.9 | 26 % | 1.50 | **PARTIAL** |

Column key: *cells/body*, *residence* and *revisit* are the late window over
bodies meeting the 90 % coverage rule; *depletions* and *recoveries* are
whole-run crossings over the 1,079–1,136 watched cells of 1,280 (it varies with the seed's habitat); *founder-grazer broods* is
births whose parent is a tick-0 form-0 founder; *late prey* is the late-window
mean prey population; *ΣP/ΣP₀* is late-window mean foliage over opening foliage;
*leaf eaten* is late-window served leaf material; *motor % of bill* is
`(body_bill_total − body_bill_upkeep)/body_bill_total` late; *kinds at end* is
founder forms alive at the horizon, of 4.

### The verdicts against the pre-registered rules

| configuration | (i) grazer bred in ≥ 4 of 6 | (ii) range < 0.60 × control | recovery in ≥ 4 of 6 | verdict |
| --- | --- | --- | --- | --- |
| baseline × 0.0006 | **no** (0/6) | **no** (0.604, misses by 0.004) | no (2/6) | **NEITHER** |
| baseline × 0.0009 | **no** (1/6) | **yes** (0.562) | no (1/6) | **PARTIAL** |
| baseline × 0.0012 | **no** (0/6) | **yes** (0.175) | no (2/6) | **PARTIAL** |
| baseline × 0.0018 | **no** (0/6) | **yes** (0.177) | no (2/6) | **PARTIAL** |
| fast-leaf × 0.0006 | **yes** (6/6) | **no** (1.027) | no (0/6) | **PARTIAL** |
| fast-leaf × 0.0009 | **no** (3/6) | **no** (0.977) | no (0/6) | **NEITHER** |
| fast-leaf × 0.0012 | **no** (1/6) | **no** (0.670) | no (1/6) | **NEITHER** |
| fast-leaf × 0.0018 | **no** (0/6) | **yes** (0.390) | no (0/6) | **PARTIAL** |

**No price meets (i) and (ii) together, so the pre-registered refutation branch
is reached: the ladder is REFUTED.** Nothing is CONFIRMED and nothing is
informative about recovery.

The refutation is not a near miss hidden by a threshold. It is a clean
**trade-off with no overlap**, and the ladder resolves it rung by rung:

- At `fast-leaf` the grazer breeds in 6 of 6 seeds at 0.00036 **and** at 0.0006,
  and the range does not move at all at either (1.000, 1.027). Paying 1.7 × for
  travel buys no concentration whatsoever.
- The first `fast-leaf` rung where the range moves is 0.0012 (0.670 — still
  above the 0.60 line), and by then the grazer breeds in 1 of 6.
- The `fast-leaf` rung that clears (ii), 0.0018, breeds in 0 of 6.
- At `baseline` the gate cannot be met **at any price, including the control**:
  the founder grazer bred in only 2 of 6 seeds at the shipped price. That is a
  correction to F's reading, which named the raised prices as what kills the
  founder grazer before its brood; at `baseline` the founder grazer mostly does
  not breed anyway, and the raised prices remove the two seeds that did.

The one arm that comes close to (ii) on its own — `baseline` × 0.0006 at 0.604,
missing the 0.600 line by 0.004 — is also the arm with the largest seed spread in
the campaign (per-seed 416, 58, 150, 42, 364, 69; sd 151 on a mean of 183). Two
of its six seeds keep the control's wide range and four collapse to a fifth of
it. Reading a mean ratio of 0.604 as "the range nearly fell" would be reading a
bimodal cell as a central tendency, and the rule's outcome does not depend on
which side of 0.600 that mean lands.

## The depleted cells: the four-way reading

1,355 per-cell records over the 60 runs, every one of them classified.

| cand | `move_cost` | records | recovered | pressure | plant-limited | marginal | below `(L·μ)_crit` | any post-depletion bite | never visited after | never visited at all |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.00036 | 68 | 1.5 % | 0 % | 0 % | **98.5 %** | **100 %** | 4.4 % | 72.1 % | 39.7 % |
| baseline | 0.0006 | 215 | 0.9 % | 0 % | 0 % | **99.1 %** | **100 %** | 0.5 % | 91.2 % | 65.1 % |
| baseline | 0.0009 | 247 | 0.4 % | 0.4 % | 0 % | **99.2 %** | **100 %** | 0.8 % | 93.5 % | 72.9 % |
| baseline | 0.0012 | 340 | 0.6 % | 0 % | 0 % | **99.4 %** | **100 %** | 0 % | 91.8 % | 74.1 % |
| baseline | 0.0018 | 336 | 0.6 % | 0 % | 0 % | **99.4 %** | **100 %** | 0 % | 92.6 % | 76.2 % |
| fast-leaf | 0.00036 | 5 | 0 % | 0 % | 0 % | **100 %** | **100 %** | 0 % | 100 % | 80.0 % |
| fast-leaf | 0.0006 | 5 | 0 % | 0 % | 0 % | **100 %** | **100 %** | 0 % | 60.0 % | 40.0 % |
| fast-leaf | 0.0009 | 15 | 0 % | 0 % | 0 % | **100 %** | **100 %** | 0 % | 100 % | 66.7 % |
| fast-leaf | 0.0012 | 32 | 3.1 % | 0 % | 0 % | **96.9 %** | **100 %** | 0 % | 84.4 % | 75.0 % |
| fast-leaf | 0.0018 | 92 | 0 % | 0 % | 0 % | **100 %** | **100 %** | 0 % | 90.2 % | 82.6 % |
| **whole campaign** | | **1,355** | **9 cells** | **1 cell** | **0 cells** | **1,345 cells** | **1,355 of 1,355** | **6 cells** | **1,232 cells** | **971 cells** |

**Astra's four-way question has a one-sided answer, and it is not the one the
campaign was built around.** Every one of the 1,355 cells that crossed the
depletion threshold is below its own `(L·μ)_crit` — under a threshold that is
*permissive* in three named ways. The brightest cell that ever depleted carries
`L·μ = 0.378` against the `baseline` critical value 0.447 and the `fast-leaf`
value 0.345. Not one cell above its critical value depleted in 60 runs, in a
world where 1,079–1,136 cells are watched and the surviving stands end at 2.1–3.7 × their
opening foliage.

Three further measurements say the same thing from directions that do not use
the derived threshold at all:

1. **971 of the 1,355 depleted cells were never observed to hold a prey body at
   any probe of the whole 150-minute run** — not before the crossing, not after.
   A body cannot be missed for a whole run by a one-second probe unless it never
   went there.
2. **Of the 384 that were ever visited, 73 were last visited within 5 simulated
   minutes of the crossing**, and 5 within one second of it. The median gap
   between a depleted cell's last consumer visit and its crossing is **57,000 to
   174,000 ticks — 48 to 145 simulated minutes**. The cell had been alone for
   most of the run when it gave way.
3. **6 cells of 1,355 have any attributed post-depletion bite at all**, and one
   of those reaches the pre-registered pressure threshold. Post-depletion grazing
   pressure is not what holds these cells down, because there is essentially
   none.

And the cells go down **slowly and late**: the median first-depletion tick is
119,200–145,400 (99–121 simulated minutes) at `baseline` and 75,400–154,600
(63–129 minutes) at `fast-leaf`, out of a 150-minute horizon. A grazed-out cell
is stripped in minutes; these decline over hours.

**The reading, stated as the classification supports it and no further.** In
ecology v1 as it stands, a "depletion event" is almost never a grazed-out cell.
It is a cell whose static habitat cannot hold the foliage §11 seeds it with,
losing that foliage slowly over an hour or two with nothing eating it. A's screen
counted 0–9 of these per run and called them depletion; F's matrix counted up to
56 and read the rise as concentrated grazing. Both counters are correct; the
name on them is not.

**A limit of the classification, stated.** Because every record is below
`(L·μ)_crit`, the `PlantLimited` class is empty **by the threshold**, not by the
data: the campaign cannot distinguish "the plant equation fails in an adequate
cell" from "there are no adequate depleted cells to test it in", because there
were none. The threshold is permissive, which makes "all 1,355 are marginal" a
strong statement rather than a weak one, but it remains a statement about a
derived constant. A run that produced even one depletion above `(L·μ)_crit`
would be the one that separates classes 3 and 4, and this campaign produced none.

### The nine recoveries

Every recovery in the campaign, listed rather than averaged, because there are
nine of them in 60 runs and they occur in **three cells**:

| cand | price | seed | cell | `L·μ` | depleted at | recovered at | latency | re-depleted | post-depletion bites |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.00036 | 1006 | 718 | 0.096 | 80,460 | 113,440 | 1,649 s | — | 0 |
| baseline | 0.0006 | 1004 | 200 | 0.102 | 105,580 | 173,060 | 3,374 s | — | 0 |
| baseline | 0.0006 | 1006 | 718 | 0.096 | 79,940 | 134,480 | 2,727 s | 150,220 | 0 |
| baseline | 0.0009 | 1004 | 200 | 0.102 | 106,120 | 174,380 | 3,413 s | — | 0.0008 m |
| baseline | 0.0012 | 1004 | 200 | 0.102 | 105,920 | 173,320 | 3,370 s | — | 0 |
| baseline | 0.0012 | 1006 | 718 | 0.096 | 80,100 | 121,680 | 2,079 s | 151,580 | 0 |
| baseline | 0.0018 | 1004 | 200 | 0.102 | 106,140 | 173,800 | 3,383 s | — | 0 |
| baseline | 0.0018 | 1006 | 718 | 0.096 | 80,200 | 121,540 | 2,067 s | 151,800 | 0 |
| fast-leaf | 0.0012 | 1006 | 435 | 0.077 | 78,920 | 117,140 | 1,911 s | 161,920 | 0 |

Recovery in this world takes **27 to 57 simulated minutes**, happens in cells
that are among the dimmest that deplete at all (`L·μ` 0.077–0.102), and happens
with **no** post-depletion grazing to speak of. It is also almost
price-independent: cell 200 of seed 1004 recovers at essentially the same tick
(173,060–174,380) at four different prices, and cell 718 of seed 1006 at
121,540–134,480 at three. These are not responses to the price knob; they are the
same slow plant-side events happening in the same places whatever the animals
cost, which is consistent with the finding that the animals were not there.

## Net energy margin per body, by form × diet bin

From E's ledger, over every prey body whose record closed plus every body alive
at the horizon; `margin = e_food_in − e_owed`, body-weighted over the 6 seeds,
with the rate given as a ratio of means (total margin over total recorded
seconds) rather than a mean of ratios, because short-lived bodies dominate a mean
of ratios and say more about lifetime than about margin. **0 ledger records were
dropped in 60 runs**, and `bill_total − bill_paid` is 0.000 for every bin: no
body in this campaign ever failed to raise its bill, because the shortfall burn
always covered it — a body starves by running its reserve out, not by defaulting.

| cand | price | rig (form, diet bin) | bodies | margin e | e/s | owed e | motor share | served m |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.00036 | grazer f0 d2 | 130 | **+2.13** | +0.00117 | 12.38 | 0.15 | 10.41 |
| baseline | 0.00036 | glider f1 d2 | 267 | +7.60 | +0.00176 | 30.82 | 0.13 | 26.64 |
| baseline | 0.00036 | burrower f2 d0 | 623 | +2.20 | +0.00170 | 5.99 | 0.06 | 14.16 |
| baseline | 0.00036 | skimmer f3 d1 | 96 | +0.91 | +0.00073 | 6.09 | 0.14 | 8.87 |
| baseline | 0.0006 | grazer f0 d2 | 60 | **−2.34** | **−0.00813** | 2.46 | 0.27 | **0.08** |
| baseline | 0.0009 | grazer f0 d2 | 171 | +2.32 | +0.00132 | 12.54 | 0.30 | 10.22 |
| baseline | 0.0012 | grazer f0 d2 | 60 | **−2.35** | **−0.01062** | 2.41 | 0.43 | **0.04** |
| baseline | 0.0012 | glider f1 d2 | 30 | **−2.34** | −0.01015 | 2.49 | 0.43 | 0.10 |
| baseline | 0.0018 | grazer f0 d2 | 60 | **−2.36** | **−0.01312** | 2.38 | 0.53 | **0.01** |
| baseline | 0.0018 | burrower f2 d0 | 1,107 | +2.59 | +0.00194 | 7.19 | 0.20 | 17.08 |
| fast-leaf | 0.00036 | grazer f0 d2 | 260 | **+5.76** | +0.00161 | 24.82 | 0.13 | 22.32 |
| fast-leaf | 0.0006 | grazer f0 d2 | 277 | **+5.23** | +0.00170 | 22.74 | 0.20 | 20.33 |
| fast-leaf | 0.0009 | grazer f0 d2 | 395 | +3.95 | +0.00196 | 15.83 | 0.27 | 14.14 |
| fast-leaf | 0.0012 | grazer f0 d2 | 132 | +1.95 | +0.00131 | 12.54 | 0.36 | 9.98 |
| fast-leaf | 0.0018 | grazer f0 d2 | 60 | **−2.36** | **−0.01315** | 2.37 | 0.53 | **0.01** |
| fast-leaf | 0.0018 | glider f1 d2 | 137 | +4.40 | +0.00223 | 19.70 | 0.42 | 16.74 |
| fast-leaf | 0.0018 | skimmer f3 d1 | 120 | +2.17 | +0.00151 | 10.40 | 0.41 | 15.22 |
| fast-leaf | 0.0012 | skimmer f3 d2 | 48 | +4.61 | +0.00194 | 15.83 | 0.34 | 19.84 |

(Five `(form, diet bin)` groups are ever occupied; the full table for every cell
is in each row under `movement.margins.bins`.)

**The margin measures what F could only infer, and it says the grazer's failure
is an intake failure, not a bill.** At every cell where the founder grazers die
without breeding, `bodies = 60` — exactly the 10 founders × 6 seeds, no
descendants — their served material is **0.01–0.08 m over a whole life** against
10–22 m for a grazer that feeds, and their total energy owed is 2.4 e against
12–25 e. They did not spend their way into a deficit: **they barely ate at all**.
The motor share of the bill rises 0.13 → 0.53 as the price rises, and the margin
rate falls +0.0016 → −0.0131 e/s, but the owed side of the ledger *falls* (24.8 →
2.4 e) because these bodies die at ~180 s. The measured failure is that a founder
grazer at a raised price never accumulates intake, not that its travel bill grew
past its income.

The `fast-leaf` ladder shows the same quantity degrading smoothly: the founder
grazer's margin per body falls 5.76 → 5.23 → 3.95 → 1.95 → −2.36 across the five
rungs while its motor share rises 0.13 → 0.20 → 0.27 → 0.36 → 0.53. The gate
(i) crosses between 0.0006 (6/6) and 0.0009 (3/6), which is where the margin is
still positive (+3.95) but the population of grazer bodies has stopped growing.

The skimmer, which A and F could not explain, has a **positive** margin wherever
it survives: +3.52 (f3 d1) and +4.61 (f3 d2) at `fast-leaf` × 0.0012, its best
cell, against −0.09 at the `fast-leaf` control. The margin is measured, not
inferred, and it supports F's leading hypothesis from the other side: the
mid-diet skimmer sits near zero (−0.09, −0.04, +1.92) at the same prices where
the grazer sits at +5.76 and the glider at +6.52, which is exactly "a lower
realised diet yield". It is still an observational comparison across arms, not
the matched factorial workstream J runs.

## What the price knob actually did, corrected

1. **It buys range, but not cheaply and not at `fast-leaf` until 0.0012.** At
   `baseline` the late-window range falls 303 → 183 → 170 → 53 → 54; at
   `fast-leaf` it does not move at all until 0.0009 (378 → 388 → 369) and then
   falls 253 → 147. The `fast-leaf` configuration — A's selected one — is
   markedly less responsive to the price than `baseline`, which F's three-level
   matrix could not see because it had no rung between 0.00036 and 0.0018.
2. **It buys depletion crossings, and the crossings are not grazing.** See above.
   The clearest single piece of evidence is seed 1001 at `baseline`: the depleted
   set is **the identical 14 cells** at 0.00036, 0.0006 and 0.0009 — same cell
   ids, the 14 dimmest (`L·μ` 0.075–0.193) — and then jumps to the same 66 and 65
   cells at 0.0012 and 0.0018, the next-dimmest band up to `L·μ` 0.364. The knob
   does not pick out *where* grazers congregate; it moves a **brightness
   threshold** down the habitat.
3. **What moves that threshold is a whole-world nutrient decline, which is
   measured as a correlate and not established as the cause.** As the price rises
   at `baseline`, late-window leaf eaten collapses 336 → 0, foliage rises
   2.31 → 3.73 × opening, litter rises 161 → 209, and the world's nutrient stock
   falls **790 → 530** at 0.0018, with its minimum 528 at 0.0012 (late-window 695 → 378). A cell's income carries the Monod
   factor `N/(N + K_N)`, so a falling nutrient raises every cell's true critical
   `L·μ` and pushes the next band of dim cells below breakeven. The direction is
   right and the magnitudes are the right order, but this campaign did not record
   per-cell nutrient, so **the nutrient path is a hypothesis with a measured
   correlate, not a demonstration**. Two mechanisms are consistent with it and
   are not separated here: the surviving stands draw the pool down, and the dead
   herbivores stop redistributing nutrient across the surface (a body that ranges
   300–400 cells deposits litter and remains in all of them). What would separate
   them: record per-cell `N` at the depleted cells' own probes, and run one arm
   with the herbivores removed at tick 0 rather than priced out.
4. **It kills the founder grazer by starving it of intake.** See the margin
   section. 60 of 60 founder grazers at the cells that fail eat 0.01–0.08 m in a
   whole life.
5. **No world died.** F's four extinctions were all at 0.006, which this ladder
   does not reach; at 0.0018 and below, 60 of 60 worlds ran the full horizon.
6. **The 0.0012 rung is where `fast-leaf` loses its third kind and the skimmer
   gains.** `fast-leaf` × 0.0012 is the only cell in this campaign where the
   foliage-diet skimmer (f3 d2) appears in numbers (48 bodies, margin +4.61), and the skimmer rig as a whole ends 69 of 232 alive
   there against 0 of 78 at its control, reproducing F's finding 7 at a different price. It
   still ends with 2.33 founder kinds against the control's 3.00.

## Accounting, and what was not measured

- **Conservation.** Worst `|mass residual|` over 60 rows **3.47e-10**; worst
  `|energy residual|` **4.87e-10** — both inside the contract's 1e-9 tolerance
  and in the band A and F measured.
- **Refusals and failures: none.** 0 of 60 rows `Invalid`, 0 `Failed`, 0 skipped
  at the wall cap, 0 extinctions, so no cell is censored and every mean is over
  6 of 6 seeds.
- **Served material per cell is *attributed*, not measured.** E's ledger records
  `served[FOLIAGE]` per body; the per-cell figure is that quantity differenced at
  one-second probes and credited to the cell the body stands in. Whatever a body
  ate in a cell it left inside a probe interval is credited to the wrong cell.
  This matters least where the conclusion rests: 971 of the depleted cells never
  held a body at any probe, so their attributed take is zero because nothing was
  ever there to attribute.
- **`(L·μ)` is the static habitat**, excluding the weather perturbation and the
  water factors (`algae_light`, `wet_gain`, `drown`). A cell whose pool lights it
  above its sky value is treated as darker than the plant step sees it, which
  makes the "marginal" verdict conservative for wet cells and not for dry ones.
- **`(L·μ)_crit` is derived here, not shipped.** The contract names no critical
  value; this one is its §13 breakeven read at the depletion threshold, at the
  contract's reference nutrient, with the reserve assumed full and ripening
  ignored. All three make it permissive. It is a *reading of the contract's
  arithmetic*, and Fable owns whether the contract should name one.
- **Per-cell nutrient was not recorded**, which is why finding 3 above is a
  hypothesis.

## What this does not establish

- **One price knob, at five levels, at arm 0, on two configurations, over six
  training seeds.** No held-out seed was touched, nothing was tuned, and no
  configuration is proposed. Site fidelity, patch memory, territory, consumer
  density and available area are all still untested.
- **The refutation is of *this* ladder, not of spatial coupling.** What is
  refuted is the pre-registered claim that some price between the shipped one and
  0.0018 buys concentration while leaving the founder grazer able to breed. A
  mechanism that concentrates foraging *without* charging for travel is untouched.
- **The four-way classification's third class is empty by the threshold.** See
  above; no depleted cell in 60 runs was above `(L·μ)_crit`, so nothing here
  tests the plant equation in an adequate cell.
- **The nutrient mechanism is a correlate.** Per-cell `N` was not recorded and no
  arm removed the herbivores without pricing them out.
- **150 simulated minutes.** Every median first-depletion tick is at 63–129
  minutes of a 150-minute horizon, so this campaign is watching the leading edge
  of a process that has not finished. A longer horizon would move every depletion
  count and is the one axis most likely to change these numbers.
- **The margin values reserve material at `e_r`**, not at the 0.8 the later
  oxidation returns, so every positive margin is an upper bound.

## Because the ladder is refuted, one mechanism is named — and only named

The brief permits naming a mechanism if the ladder is refuted. The measured
reason it is refuted is not the price knob's strength but **the identity of the
thing the campaign was counting**: in ecology v1 as it stands, `depletion_events`
counts dim cells failing to hold their seeded foliage, not cells grazed out, and
`recovery_events` counts them slowly climbing back. Under that reading, the
depletion/recovery cycle §4.4's reflush was written for has not been observed at
all — not because it is rare, but because nothing has yet been measured that
would be one.

The mechanism named, not proposed and not implemented: **`P₀` is seeded at
`0.4 · P_cap` in every alive cell regardless of whether that cell's `L·μ` can
sustain it** (`design/ecology-v1-contract.md` §11, `producer.initial_fraction`),
so a band of cells begins above its own equilibrium and must decline. Making the
opening foliage a function of the cell's own breakeven rather than a fixed
fraction of its structural cap would remove the whole population of
never-visited depletions this campaign found, and would let a depletion counter
mean what its name says. **Whether §11 should change is Fable's decision, not
this workstream's**, and nothing here is implemented, exported or proposed as a
default.

## Routine decisions made here, and their visible effect

| decision | effect |
| --- | --- |
| the ledger is a `RunOptions` field reached through a new `evaluate_with`, not a `Protocol` field | `Protocol` is serialised into every retained row and is built by the trainer and the genetic search too; a new field would have changed every retained row's shape and charged the trainer for throughput it does not use. F's 108 rows deserialise unchanged and `replay` is untouched |
| `--ledger` is opt-in rather than always on inside `run_stage` | `replay` and `harness`'s replay test evaluate with the ledger off; making the stage always enable it would have made a replay differ from the stage it replays in one respect, and the neutrality that makes that safe is exactly what was being tested |
| `(L·μ)` is the **static** habitat, reconstructed from `(config.habitat, seed)` | `World::habitat` is `pub(crate)` and this workstream may not change core. The reconstruction is checked bit-exactly against the world's own tick-0 wood in every run rather than assumed, so the number is the world's |
| `(L·μ)_crit` uses the contract's **reference** nutrient, not each cell's own | the cell's own `N` moves through the run and would make the threshold a time series rather than a property of the cell; the reference value is permissive, and every cell was below it anyway, so a per-cell nutrient could only have strengthened the finding |
| pressure is `post_served ≥ 0.25 · P₀`, the depletion fraction itself | it means "mouths took at least as much again as the standing stock the threshold leaves". The "any bite at all" sensitivity is reported beside it and moves the answer from 1 cell to 6 of 1,355, so the threshold is not what produced the result |
| the classification is **priority-ordered** and exhaustive | a cell can be both marginal and bitten; the order resolves it to `Pressure`, and the three re-slicing fractions are reported over all records so a reader can undo the order without re-running |
| served material is attributed to the cell the body is in **now** | feeding happens during the interval, not at its end; at F's measured 4–16 s residences the misattribution is one probe in four to sixteen, and the finding rests on cells that were never occupied at any probe |
| a body's record closes to the cell it was **last** observed in | a body that dies between probes is off the ledger's live list by the next probe; crediting its tail to its last known cell is the only attribution available and is stated |
| records capped at 512 per run, trajectories at 300 samples | a storage guard. The most any run opened was 66 and `records_dropped` is 0 in all 60 rows, so the cap never bound |
| arm 0 only | A measured every paired apex difference under 0.2 seed standard deviations and F reproduced it; three arms would have tripled the compute for no information about the price. The two shared rungs are tied to F by state hash, which is a stronger link than a matched mean |
| the brood gate reads the **parent's** form, not the child's | `form` is immutable under mutation so the two agree; reading the parent's makes "a founder grazer's brood" true by construction rather than by a property of the mutation operator |

## Compute and storage actually used

| | |
| --- | --- |
| Trials | 60, 0 skipped, 0 invalid, 0 failed, 0 extinctions |
| Ticks | 10.8 M |
| Wall | **199.5 s = 3.3 minutes** against the brief's 8-minute cap; 184 s predicted |
| Throughput | 54,134 ticks/s on 8 workers, against F's 58,729 on the same host without the
ledger or the per-cell record — about 8 %, though the two runs are not a
controlled comparison (different arms, different machine load) |
| Workers | 8, the cap |
| Peak RSS | 23 MiB |
| Storage | `runs/ecology-v1-ladder/` **4.3 MiB** against the 40 MiB budget, of which 1.4 MiB is a copy of F's rows for the row check; `runs/` is git-ignored, so none of it is committed |
| Background processes | none; nothing is left running |

Model usage: the harness exposes none and this session's is not visible to it.
The measured resource is the table above.

## The next task this implies, named and not launched

**Record per-cell nutrient and light at the depleted cells' own probes, and run
one arm with the herbivore rigs absent at tick 0, on the same six seeds.** The
campaign has shown that ecology v1's depletion counter is dominated by cells no
animal visits; what it has not shown is *why the count rises with the price*, and
the two candidates — the surviving stands drawing the pool down, and the dead
herbivores no longer redistributing nutrient — are separated by exactly that
pair of measurements. It is 12 extra trials and about 40 wall seconds at this
campaign's measured throughput.

Two smaller things would make the depletion counter readable either way, and
both are cheap: **report `depletion_events` split by whether the cell was ever
visited**, so a screen's headline number stops mixing two different events; and
**record the depleted cells' `Q` (plant reserve) trajectory beside `P`**, because
§4.4's reflush is the mechanism the recovery threshold was chosen to detect and
this campaign measured `P` without it.

## Stop

The note and the commits are the deliverable. Nothing here changed an equation, a
§11 value, an ordering, the snapshot schema, the trainer, the display, the
running process, `state/`, or port 7393; no `cubarium-core` file, no file under
`crates/cubarium-search/src/es/` and no line of `apex_audit.rs` was modified; and
no held-out seed was touched.
