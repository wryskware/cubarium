---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream Y — the skimmer depth ladder, on R's census, arm 0

Six rungs of the roster skimmer's `depth` between the rim it is founded on and
the height the grazer already holds, measured on
[R's census](ecology-v1-depth-census-2026-09-16.md) with every other condition
of that campaign unchanged and the apex arms dropped to arm 0.

Evidence, not a decision. **No roster change is proposed by this workstream**;
it measures. No equation, no §11 shipped default, no ordering, no `WorldConfig`
field and no `cubarium-core` file is changed. The accepted ecology v1
(`design/ecology-v1-contract.md`), [A's calibration](ecology-v1-calibration-2026-09-15.md),
[F's census](ecology-v1-movement-2026-09-16.md),
[O's factorial](ecology-v1-depth-factorial-2026-09-16.md) and
[R's depth census](ecology-v1-depth-census-2026-09-16.md) are the baseline and
are not re-reviewed here.

**The question.** R moved the roster skimmer's `depth` from its own 0.10 to the
roster grazer's 0.55 in a reproducing world and found a fourth lineage that
persists — 0 of 18 `fast-leaf` worlds with a breeding skimmer alive at 150
minutes became 13 of 18 — **paid for out of the grazer**, whose horizon
population fell to 0.58× and 0.35× of its control and whose margin rate fell to
a forty-fifth of itself. R's verdict was REFUTED in both configurations, on the
variety clause, and R named the confound that decides it: **0.55 is the
grazer's own value**, so "off the wet floor" and "onto the grazer's height" are
the same move and nothing in R separates them.

This campaign separates them, by running the heights between. **Is there a
depth that rescues the skimmer's lineage without taking the grazer's horizon
population?**

Astra's rule, pre-registered here as the brief states it: *a depth is acceptable
only if a lineage persists across seeds without materially reducing the grazer;
if none exists, the choice becomes three viable heights and four kinds, or a
wet-floor producer, which is a new food web and not a repair.*

---

## Pre-registration

*Everything in this section was written and committed before any row of this
campaign existed. The commit that carries it is named under "Build and
commits"; nothing below the rule existed when it was written.*

### The treatment, and where it is applied

Unchanged from R, and re-used rather than re-implemented:
`census::apply_depth_override` writes `genome.depth` on every founder whose
genome equals the roster skimmer's — found by **genome equality** against
`Roster::of`, never by a form number — and re-decodes its phenotype with
`decode(&genome, &config.organism)`, between `World::new` and the first
`World::step`. Nothing else is written, no `WorldConfig` field is added, and a
`depth` outside the genome's declared `0..=1` is refused rather than clamped.

`depth` decodes to a preferred embedded height, `h_pref = −1 + 2 · depth`
(`crates/cubarium-core/src/genome.rs:434`), and enters the simulation in exactly
one place: the steering term `w_depth · (h_pref − h) · up`
(`crates/cubarium-core/src/controller.rs:234`). It carries no capacity, no rate
and no bill.

| rung | `depth` | `h_pref` | whose value |
| --- | --- | --- | --- |
| **control** | **0.10** | −0.80 | the roster skimmer's own (`config.rs:519`) |
| 1 | 0.20 | −0.60 | nobody's |
| 2 | 0.30 | −0.40 | nobody's |
| 3 | 0.40 | −0.20 | nobody's |
| 4 | **0.55** | +0.10 | the roster grazer's own (`config.rs:513`) — R's treatment |
| 5 | **0.75** | +0.50 | nobody's |

The glider sits at `depth = 1.00`, `h_pref = +1.00`, and R measured it
**unmoved** by the 0.55 treatment in every table (horizon population 27.9 → 30.3
and 14.3 → 12.4, margin rate ±1 %). 0.75 is therefore a height that belongs to
no roster kind and is above the grazer's: it is the rung that tests whether the
rescue needs the grazer's niche or merely needs to be off the rim. 1.00 is not
on the ladder because it is the glider's own value and would re-import the
confound this campaign exists to remove, one kind further up.

### The design

R's census, one factor widened and the apex arms dropped.

| axis | levels |
| --- | --- |
| roster skimmer `depth` | **0.10** (control), 0.20, 0.30, 0.40, **0.55**, **0.75** |
| configuration | `baseline` (shipped §11 defaults), `fast-leaf` (A's selected configuration) |
| seed | `TRAINING_SEEDS[..6]` = 1001–1006 |
| apex arm | **0 only** |

6 × 2 × 6 × 1 = **72 trials**, horizon 180,000 ticks (150 simulated minutes),
sampling every 600 ticks, foraging probe every 20 ticks, `organism.move_cost` at
its shipped 0.00036, `lanternjaw_trial` profile unsearched, E's per-body ledger
on, M's plant record off — every one of these R's, unchanged. 72 × 180,000 =
12.96 M ticks; at R's measured 51,699 ticks/s that predicts about 4.2 wall
minutes on 8 workers, inside the brief's 6.

**Why arm 0 only.** R measured 1–2 prey deaths per run by predation in every
cell and wrote that "nothing here turns on the apex"; the three arms were
carried only to make its 36-row state-hash reproduction possible. This campaign
trades that for three more rungs at the same cost, and keeps a reproduction
check by pinning to R's own arm-0 rows instead (below).

The replicate is the **world**: six seeds, one run each per cell. Per-seed
agreement is reported for every claim, and no body-level p-value is computed.

### The reproduction check, and the stop rule

Two checks, both before anything is interpreted.

1. **Against R's retained rows.** Every one of the **24 rows** this campaign
   runs at `depth ∈ {0.10, 0.55}`, both configurations, six seeds, arm 0, must
   reproduce R's arm-0 row of the same `(candidate, seed, depth)`
   (`runs/ecology-v1-depth-census/runs.jsonl`, build `c38b5a6`, schema 16)
   **field for field**, `final_state_hash` included. Every field R's row
   carries is compared; the only exclusions are `build_id` and `elapsed_ms`,
   which are a stamp and a stopwatch and are named here so the exclusion is not
   invented afterwards.

   This check is also a **second measurement of something else**. Since R ran,
   the shipped pursuit predicate changed
   ([V](ecology-v1-predicate-adoption-2026-09-16.md), schema 17; 16 refused).
   Arm 0 has no predator, so if the adoption reaches anything without one these
   24 rows will say so. A clean pass is evidence that it reaches nothing.

2. **Against the three retained `evals.jsonl` R checked.** The **12 control
   rows** (`depth = 0.10`, both configurations, six seeds, arm 0) must carry the
   same `metrics.final_state_hash` as A's screen
   (`runs/ecology-v1-calibration/screen/evals.jsonl`), I's ladder at the shipped
   price (`runs/ecology-v1-ladder/ladder/evals.jsonl`) and M's present arm
   (`runs/ecology-v1-plant-budget/present-off/evals.jsonl`), which is R's own
   check restricted to the arm this campaign runs.

**If either fails, the campaign stops and this note reports why rather than
interpreting the rungs.**

### The measures

F's and R's census definitions are inherited **verbatim** and are not restated:
a body is classified once, when first seen, on `(form, diet bin, guild)` with
bins `[0, 0.35) / [0.35, 0.65) / [0.65, 1.0]`, never revised; founder roster
forms are measured at tick 0 in every run; a `LifeEvent::Birth` whose parent is
a tick-0 founder *is* a completed brood; the composition series is six window
snapshots; the water-depth profile is per form on O's bands; founder lifetime
counts a survivor at the length of the run.

**New here — R's two named cheap measures, and nothing else.**

- **Net margin split by generation.** E's margin, computed exactly as
  `movement::MarginAccumulator::add` computes it —

  ```text
  e_food_in = Σ_channels ( battery_credit + e_r · reserve_credit )
              + gut_battery_credit + e_r · gut_reserve_credit
  e_owed    = bill_total + other_energy_paid + growth_energy + reproduction_energy
  margin    = e_food_in − e_owed
  ```

  — binned by `(generation, form, diet bin)` rather than by `(form, diet bin)`.
  **Generation** is a two-valued fact of identity, not of age: a body is a
  **founder** if its `OrganismId` is one of the tick-0 organisms of that run,
  and a **descendant** otherwise. There is no third value; an apex member is
  excluded from the census entirely, as it already is everywhere else in this
  module. R could not measure the founder-lifetime reversal it read between
  `baseline` and `fast-leaf` because E's accumulator bins by `(form, diet bin)`
  only; this is the measure that makes it a measurement.

  The split is required to **sum to E's own bins**: for every `(form, diet bin)`,
  founder bodies + descendant bodies = E's bodies, and Σ founder margin +
  Σ descendant margin = Σ E's margin, to floating-point association. That is a
  test, not a hope.

- **Foliage and litter served, per body, by channel.** Each body's own
  `BodyBudget::served[channel]` — the material `q` that actually left the field,
  in the ledger's own channel order `FOLIAGE, FRUIT, LITTER, CARRION`
  (`crates/cubarium-core/src/world/budget.rs:61–67`) — accumulated per
  `(form, generation)` and reported as a mean over the bodies of the group.
  Reported for **every** form, not only form 3, so the skimmer's leaf is read
  against the grazer's and the glider's rather than against zero; the brief asks
  for form 3 and this is its superset at no extra cost. This is what counts "the
  skimmer now eats the grazer's leaf" instead of inferring it from the grazer's
  falling margin.

Both are read out of the per-body ledger records the census already drains; no
new world state, no new tick work, and no `cubarium-core` change.

### The verdict rule

R's clauses **L, M, V, F, D**, evaluated on each rung's cell against the
**0.10 control cell of the same configuration**. Their definitions are R's and
are not rewritten:

- **L — the lineage establishes.** Form-3 bodies alive at the horizon ≥ 1 **and**
  form-3 births > 0.
- **M — a new monoculture.** The rung's mean founder forms alive at the horizon
  is lower than the control's, **or** the rung's mean share of the horizon
  population held by its single most abundant form is ≥ 0.80 while the
  control's is < 0.80.
- **V — variety harmed.** Any founder form whose control-cell mean horizon
  population is ≥ 1.0 falls below **0.60×** that mean in the rung's cell, **or**
  any form present at the horizon in the control cell is absent from the horizon
  in at least the run threshold of the rung's runs.
- **F — the founding rescue disappears.** The rung's mean founder skimmer
  lifetime is ≤ **1.10×** the control's, **or** form-3 alive at the horizon is 0
  in at least the run threshold of runs. *R disclosed that this clause measures
  a breeding founder's lifetime where O's quantity was a sterile clone's. It is
  carried unrepaired, for comparability with R, and is reported beside the
  verdict and not read by it — see the acceptance rule below.*
- **D — the diet drifts toward foliage.** Reported, never read by the verdict.
  R disclosed that its per-seed requirement is wrong for a rate measured over a
  handful of lineage-founding events. It is carried unrepaired for
  comparability, and the entry table is the evidence.

**Acceptance, which is Astra's rule and the brief's:** a depth is **acceptable**
if **L holds and neither M nor V holds**. That is R's `Verdict::Confirmed`
condition unchanged. F and D are reported beside it. **The selected ecology is
`fast-leaf`; `baseline` is reported, not weighed.**

**The thresholds, and the one thing that had to change.** R's rule scales with
the cell: a clause needs two thirds of the runs and five sixths of the seeds,
which at R's 18 runs over 6 seeds read 12 and 5, and a **seed agreed when at
least 2 of its 3 arms did**. At arm 0 a seed has exactly one run, so a fixed
"≥ 2 runs" rule would make every seed count disagree and no clause could ever
hold. The rule is therefore generalised to **a seed agrees when a majority of
its own runs agree** — `⌈runs_of_seed / 2⌉` — which is **2 of 3 at R's cell
size, reproducing R's numbers exactly**, and 1 of 1 here. The brief's "≥ 12 of
18 runs over ≥ 5 of 6 seeds" is R's cell arithmetic quoted; at this campaign's
6 runs over 6 seeds the same scaling gives **≥ 4 of 6 runs and ≥ 5 of 6 seeds**,
and because arm 0 gives one run per seed those are the same six runs, so **the
binding threshold is 5 of the 6 seeds**. This is stated here, before any row
exists, rather than discovered when a table came out empty.

The 0.10 cell is its own control and is reported as the control row of every
table; no clause is evaluated for it.

### What this campaign is not

- It is **not a proposal to change the roster**, and it names no value as
  shipped. The brief forbids proposing a roster change and this note does not.
- It does not run arms 1 or 2, does not touch the diet, and does not bundle
  anything with a wet-floor producer.
- It cannot separate "`depth` steers the body" from "`depth` puts the body where
  the food is": `depth` has exactly one site in the simulation, and every
  difference between rungs is that site's.
- Six training seeds, one arm, 150 simulated minutes. No held-out seed is
  touched and nothing is tuned.

### Budgets

≤ 6 wall minutes of simulation, ≤ 8 workers, `runs/ecology-v1-depth-ladder/`
≤ 30 MiB, nothing left running, no held-out seed touched, `state/`, port 7393,
the shim and the running `cubarium` untouched.

---

*Everything above was written before the campaign was launched. Everything below
is what it measured.*
