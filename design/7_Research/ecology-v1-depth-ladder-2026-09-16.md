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
## The headline

> **Corrections after Astra's round-5 review (P1, P2).** The channel and margin
> sequences below are *not* monotone at every rung (fast-leaf descendant litter
> rises 2.60 → 2.75 before falling; descendant foliage peaks at 0.55; baseline
> descendant foliage goes 8.68 → 11.99 → 6.55 → 16.91 → 20.03 → 26.53; the
> founder's margin rate is less negative at 0.75 than at 0.55 in baseline;
> descendant margin dips in both configurations). What the served-channel data
> establish is **increasing dietary overlap with the grazer's foliage and
> declining litter use overall**; they do not establish that the skimmer removed
> material the grazer would otherwise have received, nor that the rescue and the
> cost are one mechanism — direct displacement was not isolated, and at arm 0
> the grazer's horizon population is 0.90–1.24× control at every rung. "The
> founder's margin rate falls monotonically" is withdrawn, and with it the
> "correction" to R: R had disclosed that its pooled ledger could not separate
> founders from descendants and called its founder mechanism a reading, so this
> campaign *refines* that record. "The apex is load-bearing" names the
> **apex-arm treatment** (arm number changes predator presence and count,
> predation, recycling and feedbacks together), not an apex mechanism. The
> no-rung verdict and the arm split stand.
>
> **No depth is acceptable.** Across the ladder the skimmer's descendants shift
> from litter toward the foliage channel the grazer also uses — in `fast-leaf`,
> 2.60 → 2.75 → 1.86 → 1.11 → 0.30 → **0.02** m of litter per body against
> 1.64 → 10.18 → 15.96 → 19.49 → 28.69 → **26.34** m of foliage — and lineage
> success is associated with that shift. In `fast-leaf`, the selected
> ecology, clause **L never holds at any rung**: the lineage establishes in at
> most 4 of 6 worlds (at 0.40) where 5 are needed. So the answer to R's
> question is Astra's second branch: *three viable heights and four kinds, or a
> wet-floor producer — a world question, not a genome one.*
>
> Two further things this campaign measured that it was not asked to. First,
> **at arm 0 the grazer never pays** in `fast-leaf` — V does not hold at any
> rung, and 0.55 reads 0.90× where R's pooled three arms read 0.58×. Splitting
> **R's own retained rows** by arm shows why: R's grazer cost lives in arms 1
> and 2 (0.47× and 0.46×) and is nearly absent at arm 0, and so does R's
> lineage (5/6 and 5/6 against **3/6** at arm 0). **R's "nothing here turns on
> the apex" is too strong for `fast-leaf`, and my own arm-0 design inherits
> that limitation.** Second, the generation split shows the rescue is a
> **descendant** phenomenon in *both* configurations — the founder skimmer's
> margin rate is lower off the rim than on it in both, though not monotonically
> — and locates R's founder-lifetime
> reversal in the founder's own **foliage intake**, which rises 0.23 → 9.98 m
> in `fast-leaf` and stays under 0.13 m in `baseline` until 0.75.

## Build and commits

| | |
| --- | --- |
| Branch | `worktree-agent-a030024f0e7c826ab`, a worktree of `main` |
| Parent | `44ac80d` |
| Pre-registration commit | `12d20d0` — everything above the rule, written before a row existed |
| Test-authoring commit | `bddfb4f` — the twelve definition tests against a stub; 10 of 12 red |
| Implementation commit | `d5be2c3` — the ladder, the two measures, the row-for-row check |
| Header fix | `68960db` — the campaign banner names the one arm it runs |
| Build stamp in every row | **`68960db`** (pinned through `CUBARIUM_SEARCH_BUILD`) |
| Rows | `runs/ecology-v1-depth-ladder/{runs.jsonl, summary.json, report.txt, analyse.py}`, 72 rows, **844 KiB** against the 30 MiB budget |

Files touched: `crates/cubarium-search/src/census.rs`, one doc line in
`lib.rs`, the `Census` variant's doc comment in `main.rs`, the new
`crates/cubarium-search/tests/depth_ladder.rs`, one narrowed test in
`crates/cubarium-search/tests/depth_census.rs`, and this note. `cubarium-core`
is not modified. `es/`, `evaluate.rs`, `calibrate.rs`, `factorial.rs`,
`apex_audit.rs`, `population.rs` and every other command in `main.rs` — this
round's other workers' files — are not modified. No `WorldConfig` field was
added and no `config_hash` moved.

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target CUBARIUM_SEARCH_BUILD=68960db \
  cargo build --release -p cubarium-search
./bin/cubarium-search-y census --seeds 6 --ticks 180000 --sample-every 600 \
  --introduce-tick 6000 --workers 8 --wall-seconds 360 \
  --retained <A's screen>,<I's ladder>,<M's present-off> \
  --census-rows runs/ecology-v1-depth-census/runs.jsonl \
  --out runs/ecology-v1-depth-ladder
```

The row-producing binary was copied out of the shared `target/` before the
campaign ran, so no concurrent build could swap it underneath; every number
below comes from a binary that printed build stamp `68960db`.

### Tests

| Command | Result |
| --- | --- |
| `cargo test --release -p cubarium-search` | **302 passed**, 5 ignored, 0 failed |
| `cargo test --release -p cubarium-core` | **567 passed**, 4 ignored, 0 failed — unchanged, which is what "core untouched" has to look like |

**12 tests are new**, all in `tests/depth_ladder.rs`. The authoring order is on
the record rather than asserted: commit `bddfb4f` carries them against a census
module whose ladder API exists and does nothing — `DEPTH_LEVELS` is `[0.0; 6]`,
`ServedProfile::add` is empty, `GenerationMarginAccumulator::finish` returns no
bins, `Assessment::acceptable` is `false` — and **10 of the 12 fail there**.
The two that pass are named rather than counted as evidence:
`the_served_channels_are_the_ledgers_own_order` measures the core's own
`CHANNEL_NAMES` and needs no implementation at all, and
`f_and_d_are_reported_and_are_not_read_by_the_acceptance` is satisfied
trivially by an acceptance that is always false.

The load-bearing ones are `the_generation_split_sums_to_es_own_bins` — the new
margin split is required to sum back to `movement::MarginAccumulator`'s own
bins, body for body and energy for energy, so it cannot be a second and
different definition of the margin — and R's own
`a_control_run_reproduces_the_ordinary_harness_world`, which still pins this
module's run loop to `evaluate_with`'s hash, census, broods, crossings and
margins and is unchanged.

## The reproduction check, before anything was interpreted

**All 60 checks pass; 0 mismatches.**

| retained rows | rows checked | matched | not in that file |
| --- | --- | --- | --- |
| A's screen, `runs/ecology-v1-calibration/screen/evals.jsonl` | 12 | **12** | 0 |
| I's ladder at the shipped price, `runs/ecology-v1-ladder/ladder/evals.jsonl` | 12 | **12** | 0 |
| M's present arm, `runs/ecology-v1-plant-budget/present-off/evals.jsonl` | 12 | **12** | 0 |
| **R's own rows**, `runs/ecology-v1-depth-census/runs.jsonl` | **24** | **24** | 0 |

The last row is the one this campaign added. Every one of the 24 rows at R's
two levels — both configurations, six seeds, arm 0, at 0.10 and at 0.55 —
reproduces R's retained row **field for field**: **1,056 field comparisons, 0
mismatches**, with `build_id` and `elapsed_ms` the only exclusions and both
named in the pre-registration before the check ran.

**That is also a second measurement of workstream V's predicate adoption.**
Since R ran, the shipped pursuit predicate changed (schema 17; 16 refused). Arm
0 has no predator, so if the adoption reached anything without one, these 24
rows would say so in `final_state_hash` and in every census, brood, crossing,
margin and depth-profile field beside it. It reaches nothing.

## The ladder

Each row is a mean over the 6 runs (6 training seeds, arm 0) of that cell.
"Kinds at end" is founder forms alive at the horizon, of 4.

| cand | `depth` | late pop | kinds at end | grazer | glider | burrower | **skimmer** | skimmer births | foliage × | worlds lost |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | **0.10** | 40.0 | 2.33 | 7.5 | 17.5 | 13.2 | **1.8** | 11.5 | 2.39 | 0 |
| baseline | 0.20 | 46.3 | 2.17 | 9.8 | 19.2 | 14.3 | **3.0** | 17.5 | 2.28 | 0 |
| baseline | 0.30 | 47.3 | 2.50 | 10.7 | 22.8 | 12.8 | **1.0** | 6.0 | 2.11 | 0 |
| baseline | 0.40 | 42.7 | 2.83 | 10.7 | 14.7 | 10.7 | **6.7** | 16.0 | 2.08 | 0 |
| baseline | 0.55 | 42.5 | 3.00 | **3.0** | 13.8 | 10.7 | **15.0** | 32.3 | 2.08 | 0 |
| baseline | 0.75 | 45.5 | 2.33 | **2.7** | 12.2 | 13.2 | **17.5** | 36.5 | 2.14 | 0 |
| fast-leaf | **0.10** | 62.3 | 3.00 | 16.8 | 32.2 | 13.3 | **0.0** | 8.0 | 2.14 | 0 |
| fast-leaf | 0.20 | 59.7 | 3.33 | 20.7 | 26.2 | 12.3 | **0.5** | 5.8 | 2.13 | 0 |
| fast-leaf | 0.30 | 60.7 | 3.50 | 17.2 | 28.8 | 13.0 | **1.7** | 6.5 | 2.13 | 0 |
| fast-leaf | 0.40 | 62.8 | 3.67 | 15.8 | 31.7 | 12.2 | **3.2** | 8.8 | 2.13 | 0 |
| fast-leaf | 0.55 | 58.5 | 3.33 | 15.2 | 27.7 | 13.0 | **2.7** | 11.5 | 2.15 | 0 |
| fast-leaf | 0.75 | 59.2 | 3.33 | 20.8 | 25.7 | 12.0 | **0.7** | 12.3 | 2.14 | 0 |

No world ended empty in any of the 72 runs, so every mean is over 6 of 6.

## The verdict, per rung, by R's clauses and Astra's rule

Each rung's cell against the **0.10 control cell of its own configuration**.
6 runs over 6 seeds; a clause needs 4 runs and 5 seeds, and because arm 0 gives
one run per seed the binding threshold is the 5 seeds. **Acceptable = L and not
M and not V.** F and D are reported and are not read.

**`fast-leaf` — the selected ecology.**

| rung | **L** | M | V | F | *D* | skimmer at the horizon | grazer | ratio to control | **acceptable** |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0.20 | no (2/6 seeds) | no | no | yes | no | 0.5 | 20.7 | 1.23× | **no** |
| 0.30 | no (3/6) | no | no | no | no | 1.7 | 17.2 | 1.02× | **no** |
| 0.40 | no (**4/6**) | no | no | no | no | 3.2 | 15.8 | 0.94× | **no** |
| 0.55 | no (3/6) | no | no | no | no | 2.7 | 15.2 | 0.90× | **no** |
| 0.75 | no (2/6) | no | no | yes | no | 0.7 | 20.8 | 1.24× | **no** |

**`baseline` — reported, not weighed.**

| rung | **L** | M | V | F | *D* | skimmer at the horizon | grazer | ratio to control | **acceptable** |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0.20 | no (1/6) | **yes** | no | yes | no | 3.0 | 9.8 | 1.31× | **no** |
| 0.30 | no (1/6) | no | **yes** (skimmer 1.0 vs 1.8) | yes | no | 1.0 | 10.7 | 1.42× | **no** |
| 0.40 | no (**4/6**) | no | no | yes | no | 6.7 | 10.7 | 1.42× | **no** |
| 0.55 | **yes (6/6)** | no | **yes** (grazer 3.0 vs 7.5) | yes | no | 15.0 | 3.0 | **0.40×** | **no** |
| 0.75 | no (4/6) | no | **yes** (grazer 2.7 vs 7.5) | no | no | 17.5 | 2.7 | **0.36×** | **no** |

**No rung is acceptable in either configuration.** In `fast-leaf` the reason is
**L**, at every rung: the lineage never establishes in 5 of the 6 worlds. In
`baseline` the one rung that establishes a lineage in 6 of 6 worlds, 0.55, is
the one that takes the grazer to 0.40×, and 0.75 takes it to 0.36× without even
establishing.

### Per seed, because a mean of six is not agreement

A breeding skimmer lineage alive at the horizon, per seed (bodies in
parentheses):

| cand | `depth` | 1001 | 1002 | 1003 | 1004 | 1005 | 1006 | seeds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| fast-leaf | 0.10 | no | no | no | no | no | no | **0/6** |
| fast-leaf | 0.20 | no | no | yes (2) | no | yes (1) | no | 2/6 |
| fast-leaf | 0.30 | no | no | yes (3) | yes (5) | no | yes (2) | 3/6 |
| fast-leaf | 0.40 | no | yes (1) | yes (5) | yes (12) | yes (1) | no | **4/6** |
| fast-leaf | 0.55 | no | no | yes (13) | yes (2) | yes (1) | no | 3/6 |
| fast-leaf | 0.75 | no | yes (2) | yes (2) | no | no | no | 2/6 |
| baseline | 0.10 | no | no | no | no | no | yes (11) | 1/6 |
| baseline | 0.40 | no | yes (9) | yes (8) | yes (17) | yes (6) | no | 4/6 |
| baseline | 0.55 | yes (1) | yes (13) | yes (15) | yes (15) | yes (9) | yes (37) | **6/6** |
| baseline | 0.75 | no | no | yes (31) | yes (31) | yes (8) | yes (35) | 4/6 |

Seed 1001 never establishes a lineage at any `fast-leaf` rung and never at any
`baseline` rung except 0.55, where it manages one body. The `fast-leaf`
lineages are concentrated in 1003 and 1004, exactly as R found for its pooled
cell, and they are small: 1–13 bodies against `baseline`'s 8–37.

The grazer at the horizon, per seed:

| cand | `depth` | 1001 | 1002 | 1003 | 1004 | 1005 | 1006 | mean |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| fast-leaf | 0.10 | 17 | 19 | 14 | 22 | 14 | 15 | 16.8 |
| fast-leaf | 0.40 | 17 | 14 | 12 | 9 | 16 | 27 | 15.8 |
| fast-leaf | 0.55 | 24 | 20 | **0** | 17 | 15 | 15 | 15.2 |
| fast-leaf | 0.75 | 19 | 21 | 16 | 26 | 23 | 20 | 20.8 |
| baseline | 0.10 | 28 | 0 | 0 | 17 | 0 | 0 | 7.5 |
| baseline | 0.55 | 18 | 0 | 0 | 0 | 0 | 0 | 3.0 |
| baseline | 0.75 | 16 | 0 | 0 | 0 | 0 | 0 | 2.7 |

The `baseline` grazer reaches the horizon in only 2 of 6 control worlds, so
every `baseline` V above rests on those two worlds losing one of them — R made
the same disclosure and it is unchanged here. The `fast-leaf` grazer reaches it
in 6 of 6 at every rung except 0.55's seed 1003, which is the single world where
the lineage is largest (13 bodies) and the grazer is gone. **That one world is
the whole of R's `fast-leaf` grazer effect that survives at arm 0.**

## What the ladder actually separated: the skimmer's income, by channel

This is R's second cheap measure, and it answers the brief's question more
directly than any clause does. Mean material taken off the field per body (m),
pooled over the 6 runs of each cell.

| cand | `depth` | skimmer founder foliage | skimmer founder **litter** | skimmer descendant foliage | skimmer descendant **litter** | grazer founder foliage |
| --- | --- | --- | --- | --- | --- | --- |
| fast-leaf | **0.10** | 0.23 | **6.34** | 1.64 | **2.60** | 9.94 |
| fast-leaf | 0.20 | 0.68 | 4.44 | 10.18 | 2.75 | 10.99 |
| fast-leaf | 0.30 | 2.39 | 3.45 | 15.96 | 1.86 | 9.77 |
| fast-leaf | 0.40 | 6.12 | 2.66 | 19.49 | 1.11 | 8.81 |
| fast-leaf | 0.55 | **9.98** | 2.07 | **28.69** | 0.30 | **7.20** |
| fast-leaf | 0.75 | 7.46 | 1.23 | 26.34 | **0.02** | 7.65 |
| baseline | **0.10** | 0.02 | **6.19** | 8.68 | **2.04** | 1.35 |
| baseline | 0.20 | 0.05 | 4.46 | 11.99 | 1.24 | 1.34 |
| baseline | 0.30 | 0.05 | 3.37 | 6.55 | 1.10 | 1.96 |
| baseline | 0.40 | 0.08 | 2.75 | 16.91 | 0.57 | 1.50 |
| baseline | 0.55 | 0.13 | 1.97 | 20.03 | 0.25 | 0.87 |
| baseline | 0.75 | **7.61** | 1.27 | 26.53 | **0.04** | **0.44** |

Three things are measured here rather than inferred.

- **The shift is large overall, and not monotone at every rung.** The skimmer's
  litter falls from 6.34 to 1.23 m for founders and from 2.60 to 0.02 m for
  descendants in `fast-leaf` (with a rise at 0.20), and its foliage rises to
  26–29 m, peaking at 0.55 — more than the grazer's own descendants take (22–26
  m). By 0.40 a `fast-leaf` skimmer descendant already draws 19.5 m of leaf
  against 1.1 m of litter. What this measures is **dietary overlap with the
  grazer's foliage rising with height**; whether that overlap is what the grazer
  pays for, or whether the field's production and composition change with depth
  instead, is not isolated here (see the correction block).
- **And the grazer's founder pays at every rung it happens at.** The
  `fast-leaf` grazer founder's own foliage falls 9.94 → 7.20 m, 28 %, with the
  skimmer founder's rising 0.23 → 9.98 m. The horizon *population* does not
  move at arm 0, but the leaf on the plate does.
- **`baseline`'s founder is the exception that names the mechanism.** Up to
  0.55 its foliage never exceeds 0.13 m while its litter halves: it is off the
  pool and has not reached the leaf, which is why it starves *sooner* (575 s →
  372 s). At 0.75 it finally reaches the leaf, 7.61 m, and its lifetime jumps
  to 1,388 s. `baseline`'s `plant.foliage_rate` is 0.002 against `fast-leaf`'s
  0.006, so the leaf is thinner and the body has to climb further to find it.
  **This is exactly the reading R offered and named as a reading; it is now a
  measurement.**

## The margin, split by generation

R's first cheap measure. E's margin, unchanged in definition and required by
test to sum back to E's own bins, keyed by founder or descendant.

Skimmer margin **rate** (e/s), pooled over each cell:

| cand | `depth` | founder bodies | **founder rate** | descendant bodies | **descendant rate** |
| --- | --- | --- | --- | --- | --- |
| fast-leaf | **0.10** | 30 | −0.000234 | 48 | −0.000809 |
| fast-leaf | 0.20 | 30 | −0.001296 | 35 | +0.000045 |
| fast-leaf | 0.30 | 30 | −0.001495 | 39 | +0.000665 |
| fast-leaf | 0.40 | 30 | −0.001753 | 53 | **+0.001015** |
| fast-leaf | 0.55 | 30 | −0.001769 | 69 | +0.000970 |
| fast-leaf | 0.75 | 30 | **−0.002511** | 74 | +0.001096 |
| baseline | **0.10** | 30 | −0.000351 | 69 | +0.000108 |
| baseline | 0.20 | 30 | −0.001509 | 105 | +0.000760 |
| baseline | 0.30 | 30 | −0.002042 | 36 | −0.000296 |
| baseline | 0.40 | 30 | −0.002428 | 96 | +0.000881 |
| baseline | 0.55 | 30 | −0.003056 | 194 | +0.001485 |
| baseline | 0.75 | 30 | −0.002872 | 219 | **+0.001797** |

**The founder's rate falls monotonically with height in both configurations,
and the descendant's rises in both.** The rescue is a descendant phenomenon
everywhere, and it is bought with a founder that earns less per second the
higher it is put. R's finding that the *founder lifetime* effect is
configuration-dependent and opposite in sign stands — `baseline` 575 → 372 s
against `fast-leaf` 617 → 1,846 s — but it is not a sign change in the
founder's margin rate. What differs is the founder's **income**: the
`fast-leaf` founder's total margin turns positive (−0.20 → +0.73 e) because it
eats 43× more leaf and lives three times as long on a thinner per-second
margin, while the `baseline` founder's total margin stays negative
(−0.24 → −1.10 e) because there is no leaf at its new height to buy.

That is the measurement R said it could not make, and it **corrects R's
sentence** in one respect: the founder's per-second economics get worse with
height in both worlds, not better in one of them.

## What this campaign found that it was not asked to: the apex-arm treatment matters

R wrote that "nothing here turns on the apex" and carried three arms only for
its hash check. Dropping to arm 0 to buy four more rungs was this brief's
design, and it turns out to cost more than the brief expected. Splitting **R's
own retained rows** by arm — no new simulation, R's 72 rows re-read:

| cand | `depth` | arm | grazer alive | skimmer alive | runs with a breeding lineage |
| --- | --- | --- | --- | --- | --- |
| fast-leaf | 0.10 | 0 | 16.8 | 0.0 | 0/6 |
| fast-leaf | 0.10 | 1 | 23.0 | 0.0 | 0/6 |
| fast-leaf | 0.10 | 2 | 24.2 | 0.0 | 0/6 |
| fast-leaf | **0.55** | **0** | **15.2 (0.90×)** | **2.7** | **3/6** |
| fast-leaf | **0.55** | 1 | **10.8 (0.47×)** | 5.8 | 5/6 |
| fast-leaf | **0.55** | 2 | **11.2 (0.46×)** | 8.3 | 5/6 |
| baseline | 0.55 | 0 | 3.0 (0.40×) | 15.0 | 6/6 |
| baseline | 0.55 | 1 | 2.7 (0.35×) | 18.3 | 5/6 |
| baseline | 0.55 | 2 | 2.8 (0.31×) | 20.8 | 5/6 |

**Both halves of R's `fast-leaf` result live in the apex arms.** The grazer cost
that carried R's refutation is 0.47× and 0.46× with predators and 0.90× without
them; the lineage that carried R's L is 5/6 and 5/6 with predators and 3/6
without. `baseline` is not like this: its grazer cost is the same in all three
arms.

This is not a repair of R's verdict — R's rows are R's, they reproduce here
field for field, and R's pooled cell is what R said it was. It is a limitation
of **this** campaign, stated plainly: the ladder was run in the one arm where
R's effect is weakest, and a ladder at arm 2 could read differently. It is also
a refinement of R's "nothing turns on the apex", which this campaign is in a
position to make only because it re-read R's rows. Why the arm matters is not
identified: arm number changes predator presence and count, predation,
recycling, prey abundance and the resource feedbacks that follow, and the
census records do not isolate direct predation on skimmers from predation on
grazers or a plant-mediated path.

## What Wrysk would be approving

**Nothing.** This campaign proposes no roster change and finds no depth it
could propose one at.

What the ladder rules out is worth stating as a positive result, because it was
the open question R left: **at no tested height does the skimmer lineage
establish on litter; where it establishes, its diet has shifted onto the
foliage channel the grazer also uses.** Whether that overlap is the grazer's
cost was not isolated. The skimmer's litter income falls from the first rung
upward (not at every step), and its foliage income is what replaces it, in both
configurations. A value that buys a lineage without buying it out of the leaf
does not exist on this ladder.

So Astra's rule reaches its second branch, in Astra's own words: **the choice
becomes three viable heights and four kinds, or a wet-floor producer, which is
a new food web and not a repair.** In this world's terms:

- **Three viable heights and four kinds.** Accept that ecology v1 has room for
  the burrower on the floor, the grazer and the glider on the leaf, and nothing
  that lives off the rim — and that the roster skimmer is a fourth *kind* whose
  niche the world does not contain. On the cube that is what is already
  happening: `fast-leaf` ends its second hour with no skimmers at all in 6 of 6
  worlds at arm 0.
- **Or give the wet floor a food.** A producer in standing water would make the
  rim an income rather than a place to starve, which is the only change on the
  table that would let a skimmer be fed without eating the grazer's leaf. That
  is a **world question** — a new stock, a new channel or a new growth term —
  and it is Fable's and Wrysk's to decide, not this campaign's. Nothing here
  bundles it with anything.

## Accounting

- **Conservation.** Worst `|material residual|` over 72 rows **1.64e-11**,
  worst `|energy residual|` **1.31e-10** — both inside the contract's 1e-9.
- **Invariants.** `World::check_invariants` ran at every 600-tick sample of
  every run; no violation, or the run would have been an error rather than a
  row.
- **Ledger completeness.** **0** dropped closed records in 72 runs.
- **Refusals and failures: none.** 72 of 72 trials completed, 0 skipped.
- **Censoring: none.** No world ended empty.
- **Predation: none.** Arm 0 introduces no apex, so 0 of the 72 runs carry one.

## What this does not establish

- **Arm 0 only, and the apex turns out to matter.** See the arm table above.
  The `fast-leaf` grazer cost and the `fast-leaf` lineage both live largely in
  the arms this campaign did not run. A ladder at arm 2 is the obvious next
  measurement and it is **not** launched here.
- **Six rungs of one locus.** The ladder measures a direction and five
  contrasts; it does not fit a curve, and 0.40 — the rung with the most
  lineages in `fast-leaf`, 4 of 6 — is not shown to be a maximum.
- **One run per seed.** Six worlds per cell, one arm each, and the seed *is*
  the replicate. A clause that needs 5 of 6 seeds is a demanding threshold on
  six observations, and `fast-leaf`'s best rung misses it by one world.
- **The foliage-bin association is still an association.** Every bin-2 form-3
  body is a descendant; this campaign did not randomise anything.
- **150 simulated minutes.** Whether any of these lineages persists past the
  horizon is not measured.
- **Six training seeds.** No held-out seed was touched, nothing was tuned, no
  configuration is proposed and no §11 default is changed.
- **No equation, no ordering, no schema and no core file is changed**, and the
  running cube, `state/`, port 7393 and the shim were not touched.

## Routine decisions made here, and their visible effect

| decision | effect |
| --- | --- |
| the seed-agreement rule generalises to a **majority of a seed's own runs** | R's "2 of its 3 arms" is reproduced exactly at R's cell size, and at one arm per seed a seed agrees when its one run does; without it every clause would read 0 seeds and nothing could ever hold |
| the two new measures are taken at the **same two ledger sites** E's own margin is taken at | a body can never be in E's bins and not in the generation split, or in one channel table and not the other; the equality is a test, not a comment |
| generation is **identity**, not age: a tick-0 organism or not | a body born at tick 1 is a descendant, which is what "the founder-lifetime reversal" needs it to be; nothing is inferred from an age or an id's generation counter |
| `ServedProfile` is recorded for **every form**, not only form 3 | the skimmer's leaf is read against the grazer's and the glider's rather than against zero; the brief asked for form 3 and this is its superset at no extra cost |
| the channel names are the **core's own** `CHANNEL_NAMES` | a second list in the search crate could drift out of the ledger's order; the test asserts the order rather than a copy of it |
| the row-for-row check compares **every field R's row carries** and excludes only `build_id` and `elapsed_ms` | "field for field" is 44 fields and 1,056 comparisons rather than a hash; the two exclusions are named in the pre-registration, before the check ran |
| the four new rungs are **not** counted as "missing" from R's file | a rung R never ran is not a failed reproduction; only 0.10 and 0.55 are targets |
| R's clauses **F** and **D** are carried unrepaired and unread | R disclosed both as defective; repairing them here would make this campaign's tables incomparable with R's, and reading them would let a defect decide an acceptance |
| R's own rows are **re-read by arm** rather than re-simulated | the apex finding costs no compute and cannot disagree with R's rows, because it *is* R's rows |
| `analyse.py` shipped **beside the rows** | every table in this note that the harness does not print is reproducible from `runs.jsonl` by one command, so the note cannot disagree with the rows |

## Compute and storage actually used

| | |
| --- | --- |
| Trials | 72, 0 skipped, 0 invalid, 0 failed |
| Ticks | 12.96 M; every run reached the full 180,000 |
| Wall | **254.1 s = 4.2 minutes** against the brief's 6-minute cap |
| Throughput | 51,010 ticks/s on 8 workers |
| Workers | 8, the cap |
| Storage | `runs/ecology-v1-depth-ladder/` **844 KiB** against the 30 MiB budget |
| Test suites | `cubarium-search` 302 passed / 5 ignored; `cubarium-core` 567 passed / 4 ignored |
| Background processes | none; nothing is left running |

Model usage: the harness exposes none and this session's is not visible to it.
The measured resource is the table above.

## The next task this implies, named and not launched

**The same ladder at arm 2.** This campaign's own arm table says R's
`fast-leaf` effect — both the lineage and the grazer cost — is largely a
property of worlds with predators in them, and the ladder was run in the arm
where it is weakest. 72 more runs at arm 2 would say whether any rung is
acceptable in the world the cube actually runs, and would cost the same 4.2
wall minutes. It is named here and **not** launched.

Beyond that, the ladder's answer points away from the genome: if the question
is "can a fourth guild be fed without taking the leaf", the measurement above
says no height does it, and the remaining lever is the world — a wet-floor
producer — which is a food-web decision and not this workstream's.

## Stop

The note, the rows and the commits are the deliverable. Nothing here changed an
equation, a §11 value, an ordering, the snapshot schema, a `WorldConfig` field,
the trainer, the display, the running process, `state/` or port 7393; no
`cubarium-core` file and no file owned by another worker this round was
modified; no held-out seed was touched; and no roster change is proposed.
