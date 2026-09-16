---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream R — the skimmer at `depth = 0.55` in a reproducing world

F's 150-minute variety census, re-run with **one locus of the founder roster
moved and nothing else**: the roster skimmer's `depth`, from its own 0.10 to the
roster grazer's 0.55, written into the founders of an ordinary world at tick 0.
Reproduction and mutation stay on for everyone and offspring inherit as usual.

Evidence, not a decision. No equation, no §11 shipped default, no ordering, no
`WorldConfig` field and no `cubarium-core` file is changed. The accepted ecology
v1 (`design/ecology-v1-contract.md`, schema 16),
[workstream A's calibration](ecology-v1-calibration-2026-09-15.md),
[F's census](ecology-v1-movement-2026-09-16.md) and
[O's factorial](ecology-v1-depth-factorial-2026-09-16.md) are the baseline and
are not re-reviewed here.

**The question.** [O](ecology-v1-depth-factorial-2026-09-16.md) showed that at
founding, in sterile cold-founded clones, moving the roster skimmer's `depth`
from 0.10 to 0.55 turns a body that starved in 32 of 32 clone lives into one
that reaches the 75-minute horizon in 26 of 32, and reverses the diet effect
with it. O also named what it could not show: those clones never bred. F
measured *descendants over 150 simulated minutes with reproduction on* and found
the skimmer rig lost in 18 of 18 `fast-leaf` control runs and surviving at 4 %
in the baseline control. O's named next task, and Astra's round-3 next-steps
item 3, is the one experiment that joins them: **F's census with only the roster
depth changed.** Does a founding-time rescue this large survive a lineage?

---

## Pre-registration

*Everything in this section was written and committed before any row of this
campaign existed. The commit that carries it is named under "Build and
commits"; nothing below the rule existed when it was written.*

### The treatment, and where it is applied

`depth` is not a water depth. It decodes to a preferred embedded height,
`h_pref = −1 + 2 · depth` (`crates/cubarium-core/src/genome.rs:434`), and enters
the simulation in exactly one place: the steering term
`w_depth · (h_pref − h) · up` (`crates/cubarium-core/src/controller.rs:234`). It
carries no capacity, no rate and no bill. O asserted that with a decode
comparison; this campaign re-asserts it on the *founders of a real world* rather
than on two decoded genomes, because that is where it is applied here.

| | `depth` | `h_pref` | whose value |
| --- | --- | --- | --- |
| **control** | 0.10 | −0.80 | the roster skimmer's own (`config.rs:519`) |
| **treatment** | 0.55 | +0.10 | the roster grazer's own (`config.rs:513`) |

The override is **search-side and post-build**. `World::new` founds the ordinary
24 founders (burrower 4, grazer 10, glider 5, skimmer 5) from the shipped
roster; then, before the first `step`, every founder whose genome is the roster
skimmer's genome — identified by genome equality against `Roster::of(&world)`,
not by a form number — has `genome.depth` set to the treatment value and its
phenotype re-decoded with `decode(&genome, &config.organism)`. Nothing else is
written.

Three properties are **tested, not asserted**:

1. the control override (0.10) changes no genome bit and no phenotype bit of any
   body, and a 2,000-tick run under it reproduces the untouched world's
   `final_state_hash`;
2. the treatment override (0.55) changes exactly two numbers on exactly the five
   roster skimmers — `genome.depth` and `phenotype.h_pref` — and nothing on the
   other nineteen founders; every capacity, rate, cap and drive is equal;
3. the campaign's own run loop reproduces `evaluate::evaluate_with`'s
   `final_state_hash`, census, founder broods and margins at 2,000 ticks, so the
   control arm is the ordinary harness's world and not a second implementation
   of it.

**Why no `WorldConfig` field.** Adding one changes `calibrate::config_hash` for
every existing TOML, which breaks policy provenance and invalidates every
retained row in `runs/`. The treatment is therefore not a configuration: it is a
write to a built world, recorded in the row, and the control rows prove the
write is a no-op when it writes the value that was already there.

### The design

Every condition of F's census, unchanged, crossed with the one new factor.

| axis | levels |
| --- | --- |
| roster skimmer `depth` | **0.10** (control), **0.55** (treatment) |
| configuration | `baseline` (shipped §11 defaults), `fast-leaf` (A's selected configuration) |
| seed | `TRAINING_SEEDS[..6]` = 1001–1006 |
| apex arm | 0, 1, 2 adults, introduced at tick 6,000, never restocked |

2 × 2 × 6 × 3 = **72 trials**, horizon 180,000 ticks (150 simulated minutes),
sampling every 600 ticks, foraging probe every 20 ticks, `organism.move_cost` at
its shipped 0.00036, `lanternjaw_trial` profile unsearched, **E's per-body
ledger on** (`RunOptions::ledger`), M's plant record off. 72 × 180,000 = 12.96 M
ticks; at I's measured throughput that predicts about 3.6 wall minutes on 8
workers, inside the brief's 6.

The replicate is the **world**: six seeds, each carrying three matched apex
arms. Per-seed agreement is reported for every claim, and no clone-level or
body-level p-value is computed — 18 runs sharing 6 worlds are not 18 independent
observations.

### The reproduction check, and the stop rule

The control is only a control if it is the world the retained rows were produced
in.

1. Every one of the **24 control rows** (`depth = 0.10`, both configurations, six
   seeds, three arms) must carry the same `metrics.final_state_hash` as A's
   retained screen row for the same `(candidate, seed, arm)`
   (`runs/ecology-v1-calibration/screen/evals.jsonl`).
2. The **12 arm-0 control rows** must additionally match I's ladder rows at the
   shipped price (`runs/ecology-v1-ladder/ladder/evals.jsonl`) and M's
   present-arm rows (`runs/ecology-v1-plant-budget/present-off/evals.jsonl`).
   Those three files already agree with each other on all 12, which is what
   makes a three-way check meaningful: it pins the ledger-on path as well.

**If either fails, the campaign stops and this note reports why rather than
interpreting the treatment arms.**

### The measures

Stated as definitions, because a definition written after seeing the rows is not
a measurement. F's census definitions are inherited verbatim and are not
restated: a body is classified **once**, when it is first seen, on
`(form, diet bin, guild)` with bins `[0, 0.35) / [0.35, 0.65) / [0.65, 1.0]`,
and the classification is never revised. Founder roster forms are **measured at
tick 0** in every run, not assumed.

New or sharpened here:

- **Founder skimmer survival.** The five tick-0 roster skimmers of each run are
  followed individually: alive at the horizon, exact age at death from
  `LifeEvent::Death { age_ticks }`, cause, and `births`. "Survival" is the
  founder's own, distinct from the rig's.
- **First brood.** `FounderBroods`, unchanged from I: a `LifeEvent::Birth` whose
  parent is a tick-0 founder **is** a completed brood (the core emits it when
  gestation completes and the child is committed), reported per form with its
  first tick and the number of distinct founder parents. Nothing is inferred
  from a reserve or an age.
- **The census over time.** The live composition by `(form, diet bin)` at the
  six window boundaries — ticks 0, 36,000, 72,000, 108,000, 144,000, 180,000 —
  beside the whole-run census at the horizon. Six snapshots, not 301, because a
  composition series is a shape and the shape is what the question needs.
- **Net margin by `(form, diet bin)`.** E's ledger, through I's
  `MarginAccumulator`, unchanged: `margin = e_food_in − e_owed` with
  `e_owed = bill_total + other + growth + reproduction`, per body, meaned over
  bodies, plus the per-second rate. Dropped records are counted, never silently
  zero.
- **The water depth under a skimmer.** O's finding was that the binary wet
  fraction barely moves (67 % → 65 %) while the **depth of the water** moves
  six-fold (0.306 d → 0.051 d) and the **algae-band fraction** moves five-fold.
  This campaign therefore reports, per probe and per visual form: mean water
  depth `fields.w` under the body, the wet fraction (`w > 0`), the algae-band
  fraction (`w ≥ 0.5 · water.algae_depth`), and a four-band histogram on O's own
  bands `≤ 1e-3 / ≤ 0.05 / ≤ 0.15 / >  0.15` d. Reported for every form, so the
  skimmer's habitat is read against its neighbours' and not against zero.
- **The other three kinds, and the vegetation.** Per form: bodies entered,
  births, deaths by cause, alive at the horizon, margin. Per run: opening and
  final foliage and litter, their means, and the per-cell depletion/recovery
  crossing counter (F's, unchanged).

### The confirmation and refutation rules

Astra's rule, from the brief: *confirm the roster correction if the skimmer
establishes a lineage across seeds without replacing the world with a new
monoculture; refute if the founding rescue disappears under reproduction or
harms variety.* Operationalised below, evaluated **separately for each
configuration**, each treatment cell against its own configuration's control
cell, matched run by run on `(seed, arm)`.

A **run agrees** with a clause when that run satisfies it. A **seed agrees**
when at least 2 of its 3 arms do. Thresholds are set where F's control already
measured the opposite, so no clause is one the control already clears.

- **L — the lineage establishes.** In the treatment cell: form-3 bodies alive at
  the horizon ≥ 1 **and** form-3 births > 0, in ≥ 12 of the 18 runs, with ≥ 5 of
  the 6 seeds agreeing. Both clauses are required: a founder alive at the
  horizon that never bred is survival, not a lineage. F's controls measured 0 of
  18 form-3 bodies alive at `fast-leaf` and 12 bodies across all 18 baseline
  runs, so 12 of 18 runs is far above either.
- **M — a new monoculture.** The treatment cell's mean founder forms alive at
  the horizon is **lower** than the control's, **or** the treatment's mean share
  of the horizon population held by its single most abundant form is ≥ 0.80
  while the control's is < 0.80.
- **V — variety harmed.** Any founder form whose control-cell mean horizon
  population is ≥ 1.0 falls below **0.60 ×** that mean in the treatment cell,
  **or** any form present at the horizon in the control cell is absent from the
  horizon in ≥ 12 of 18 treatment runs.
- **F — the founding rescue disappears.** The treatment cell's mean founder
  skimmer lifetime is ≤ **1.10 ×** the control's, **or** form-3 alive at the
  horizon is 0 in ≥ 12 of 18 treatment runs.

Verdicts per configuration, in this order:

1. **CONFIRMED** — L and not M and not V.
2. **REFUTED** — F, or V.
3. **PARTIAL** — otherwise; the note names which of L, M, V, F held.

Reported beside the verdict but **not part of it**, because O predicted it and a
prediction confirmed is evidence while a prediction promoted to a gate is not:

- **D — the diet drifts toward foliage.** The treatment cell's share of form-3
  entrants in diet bin `0.65–1.00` exceeds the control's, with the number of
  agreeing seeds stated.

### What this campaign is not

It is not a proposal to change the roster, and it does not bundle the change
with a wet-floor producer: Astra's brief forbids that bundling, and O named the
"give the wet floor a food" option as an ecology change and Fable's to decide.
This campaign measures one locus in a reproducing world and nothing else.

### Budgets

≤ 6 wall minutes of simulation, ≤ 8 workers, `runs/ecology-v1-depth-census/`
≤ 30 MiB, nothing left running, no held-out seed touched, `state/`, port 7393,
the shim and the running `cubarium` untouched.

---

*Everything above was written before the campaign was launched. Everything below
is what it measured.*
