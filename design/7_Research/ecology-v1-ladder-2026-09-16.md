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
