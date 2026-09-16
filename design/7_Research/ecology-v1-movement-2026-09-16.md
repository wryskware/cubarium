---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1 — the movement-cost arm and the variety census

Workstream F, under
[the movement brief](../handoffs/ecology-v1-movement-opus-2026-09-16.md): steps 3
and 4 of the reconciled next steps in
[the consolidated result](ecology-v1-next-results-2026-09-15.md), from
[Astra's review](ecology-v1-next-review-2026-09-15.md) findings 3 and 4 and
next-steps items 3 and 4.

Evidence, not a decision. Nothing here changes an equation, a §11 shipped
default, an ordering, the motor contract or the snapshot schema, and
`cubarium-core` is not modified at all. The accepted ecology v1
(`design/ecology-v1-contract.md`, schema 16) and
[workstream A's calibration](ecology-v1-calibration-2026-09-15.md) are the
baseline and are not re-reviewed here.

**The question.** A's screen measured a prey body visiting 259–451 distinct cells
per 30-minute window out of 1,280, 0–9 foliage depletion events per run and
**0 recovery events in all 270 runs**. A's note names cheap, wide-ranging
movement as the leading hypothesis for that, and Astra's review (finding 4)
accepts it as a hypothesis while pointing out that no arm manipulated movement
price, site fidelity, density or area with everything else held fixed, so high
range as a *response* to thin local food is not ruled out. This workstream
manipulates the one price knob that already exists — `organism.move_cost`, the
per-structure-pixel coefficient of the motor bill
`move_cost · S · (speed + k·r·|ω|) · dt` (`crates/cubarium-core/src/motor.rs:354-408`)
— and holds every other condition of A's screen fixed.

---

## Pre-registration

*Everything in this section was written and committed before any row of this
campaign was produced. The commit that carries it is named under "Build and
commits"; nothing below it existed when it was written.*

### The arms

One knob, three levels, crossed with two configurations, A's six training seeds
and A's three matched apex arms.

| axis | levels |
| --- | --- |
| `organism.move_cost` (e per unit structure per pixel) | **0.00036** (shipped default, the control), **0.0018** (5 ×), **0.006** (16.67 ×) |
| configuration | `baseline` (shipped §11 defaults), `fast-leaf` (A's selected configuration: `plant.foliage_rate` 0.006, `plant.maintenance` 0.0001, `plant.reserve_share` 0.35) |
| seed | `TRAINING_SEEDS[..6]` = 1001–1006 |
| apex arm | 0, 1, 2 adults, introduced at tick 6,000, never restocked |

3 × 2 × 6 × 3 = **108 trials**, horizon 180,000 ticks (150 simulated minutes),
sampling every 600 ticks, foraging probe every 20 ticks, `lanternjaw_trial`
profile unsearched — every one of A's screen conditions, unchanged.

Why these three levels. 0.00036 is what the world ships and is therefore the
control. 0.006 is not an invented number: it is the per-second motor coefficient
the world carried **before** the pace calibration, which
`crates/cubarium-core/src/config.rs:558-565` records was divided by the new
`speed_max = 5.0` to give 0.00036 — so the top level restores the per-pixel
price of travel that existed before cruise speed rose, and the bracket is
"the price the model used to charge" rather than "a number large enough to
work". 0.0018 is the geometric-ish midpoint (5 ×) and is there so the response
is a curve and not a two-point line.

### The measures

Stated here as definitions, because the measures are new and a definition
written after seeing the rows is not a measurement. Probe cadence is
`PROBE_EVERY = 20` ticks (1 simulated second); a window is one fifth of the
declared horizon (36,000 ticks = 1,800 probes), and the fifth window is the
same interval as A's late window.

- **Visit.** For one prey body in one window, a *visit* is a maximal run of
  consecutive probes at which the body was observed in the same cell. A probe at
  which the body was not observed (it was not yet born, or is already dead)
  breaks the run. A body is included in a window only if it was seen on at least
  90 % of that window's probes — A's `SPATIAL_COVERAGE` rule, unchanged — so
  these are foraging measures and not lifespan measures.
- **Residence time.** `probes × 20` ticks for a visit of `probes` probes. The
  true occupancy of a visit seen on `n` consecutive probes lies in
  `((n−1)·20, (n+1)·20)` ticks, and `n·20` is its midpoint; an occupancy
  shorter than one probe interval is not resolved and appears as a 20-tick
  visit. The last visit in a window is truncated at the window boundary and is
  counted as observed. Reported as the mean over a body's visits, then the mean
  over qualifying bodies, then over windows weighted by qualifying bodies —
  the same weighting A's `cells_per_body_window` uses.
- **Revisit interval.** For one body and one cell, the ticks between the
  **start** of one visit to that cell and the start of the *next* visit to the
  same cell. Consecutive probes in the same cell are one visit, not a revisit;
  only a departure and a return produce an interval. A body that never returns
  to any cell inside a window contributes no interval and is counted in
  `bodies_without_revisit`. Reported as the mean over intervals per body, then
  over the bodies that have at least one.
- **Depletion and recovery crossings, per cell.** A cell whose opening foliage
  exceeds `1e-9` is *watched*. Its state starts `Ok`; it crosses to `Depleted`
  the first probe its foliage falls below `0.25 × P₀` and back to `Ok` the first
  probe it rises above `0.5 × P₀`, with `P₀` the cell's own tick-0 foliage —
  A's thresholds and A's hysteresis, unchanged, now counted **per cell** rather
  than only in total. A **cycle** is a depletion followed later by a recovery in
  the same cell. Reported per run: total crossings, crossings per watched cell,
  cells with ≥ 1 depletion, cells with ≥ 1 recovery, cells with ≥ 1 complete
  cycle, and the largest number of cycles any one cell completed. The totals
  must equal A's `cum_depletion_events` / `cum_recovery_events` exactly; a unit
  test asserts it.
- **Deaths by cause per arm.** A's four causes (starvation, age, collapse,
  predation), already recorded, reported per price level.
- **Terminal stores at death.** The body's `energy`, `reserve`, `structure` and
  `hunger` at the **last probe before its death event** — at most 20 ticks
  (1 s) stale, and stated as such. Summed and meaned per death cause and per
  census cell. `usable = energy + reserve_energy_density · reserve` is reported
  beside them because that is the store the bill is actually drawn against.
- **Net energy margin per body: not measured.** The per-organism budget
  accumulator is workstream E's deliverable and, checked at `main` =
  `b70624d` before this campaign was planned and again before it ran, has not
  landed. Nothing on the search side can substitute for it: intake is served
  inside `World::step` and only the world-level `intake_diagnostics()` is
  exposed, so a per-body credit cannot be separated from a per-body bill by
  differencing stores. Death cause and terminal stores are reported instead,
  and the margin is named as unmeasured.

### The variety census

Every prey body is classified **once**, when it is first seen (at tick 0 for a
founder, at its birth event for a descendant), and the classification is never
revised — `diet` and `form` are set at conception and `form` is immutable under
mutation, so a census keyed on them moves only through birth and death.

- **form**: `phenotype.form`, the visual creature kind. The shipped founder
  roster (`crates/cubarium-core/src/config.rs:490-519`) is burrower → form 2,
  grazer → form 0, glider → form 1, skimmer → form 3. The mapping is *measured*
  at tick 0 in every run rather than assumed.
- **diet bin**: `phenotype.diet` in `[0.0, 0.35)`, `[0.35, 0.65)`,
  `[0.65, 1.0]`. The founders land burrower 0.10 → bin 0, skimmer 0.60 → bin 1,
  grazer 0.85 and glider 0.90 → bin 2, so the middle bin is the skimmer's and
  the bins separate the four founder kinds into exactly the groups the question
  is about.
- **guild**: A's `guild_of` on the decoded caps, unchanged.

Per `(form, diet bin, guild)` cell: bodies present at tick 0, births, deaths by
cause, mean lifetime in seconds (from `LifeEvent::Death { age_ticks }`, exact,
not sampled), mean terminal stores, and bodies alive at the end. Reported
sparsely — only cells that were ever occupied.

Per run additionally: **the skimmer's loss tick** (the last tick at which any
form-3 body was observed alive, `null` if one is alive at the horizon) and the
**cause of the last form-3 death**.

### The confirmation and refutation rules

Evaluated separately for each of the four `(configuration, raised price)`
configurations — `baseline` × 0.0018, `baseline` × 0.006, `fast-leaf` × 0.0018,
`fast-leaf` × 0.006 — each against **its own configuration's 0.00036 control**,
with every statistic a mean over the 18 runs (6 seeds × 3 apex arms) of that
cell unless the rule says otherwise.

Let `C₀`, `C_p` be the late-window `cells_per_body_window_late`; `N₀`, `N_p` the
late-window mean prey population; `R_p` the mean recovery crossings per run;
`D_p` the mean depletion crossings per run.

- **R — range falls substantially**: `C_p < 0.60 · C₀`.
- **D — repeated local depletion *and* recovery**: `D_p ≥ 2.0` **and**
  `R_p ≥ 1.0` **and** at least **12 of the 18** runs record ≥ 1 recovery
  crossing. (A measured 0 recoveries in 270 of 270 runs, so "repeated" is set
  where a single replicated recovery is already a qualitative change.)
- **S — starvation collapse**: `N_p < 0.50 · N₀`, **or** any of the 18 worlds
  ends with an empty world. Its negation is "without a starvation collapse".
- **U — range unchanged**: `C_p ≥ 0.90 · C₀`.
- **K — deaths rise**: whole-run prey deaths ≥ `1.25 ×` the control's.

Verdicts, in this order:

1. **CONFIRMED** — R and D and not S.
2. **REFUTED** — U, **or** (K and not D).
3. **PARTIAL** — otherwise; the note names which of R, D, S held.

A CONFIRMED cell is evidence that spatial coupling alone produces the local
depletion/recovery cycles ecology v1 was built to show. A REFUTED cell is
evidence against the price knob, not against the spatial hypothesis in general
— site fidelity and memory are untested here either way.

### The reproduction check, and the stop rule

Two checks, both run before the campaign's results are read:

1. **The no-op check.** Writing `organism.move_cost` at its shipped default must
   change no bit of the configuration. `calibrate-export` of `baseline` and
   `fast-leaf` at seed 1 must still print A's recorded config hashes
   `fc1aefa33ebd70a1` and `09e244392ec91768`.
2. **The row check.** Every one of the 36 rows at `move_cost = 0.00036` must
   carry the same `metrics.final_state_hash` as A's retained screen row for the
   same candidate, seed and apex arm
   (`runs/ecology-v1-calibration/screen/evals.jsonl`, 270 rows).

**If either fails, the campaign stops and the note reports why rather than
interpreting the raised-price arms.** A control that does not reproduce A's
world is not a control.

### Budgets

≤ 20 wall minutes of simulation, ≤ 8 workers, `runs/ecology-v1-movement/` ≤ 30
MiB, nothing left running. 108 trials × 180,000 ticks = 19.44 M ticks; at the
45,056 ticks/s A measured on 8 workers that predicts **432 s = 7.2 minutes**.
The stage runs under a hard `--wall-seconds 1080`; a trial not started by the
cap is recorded as skipped and reported, and no horizon is shortened.

---

*Everything above was written before the campaign was launched. Everything below
is what it measured.*
