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

## The headline

> **The founding rescue survives reproduction — as a lineage, not as a founder —
> and its price is the grazer.** With the roster skimmer's `depth` at 0.55 the
> fourth guild goes from 0.9 bodies at the `baseline` horizon to **18.1**, and
> from **0 of 18** `fast-leaf` worlds to **13 of 18** with a breeding lineage
> alive at 150 minutes. O's diet prediction holds: the foliage-bin share of
> form-3 entrants rises 2 % → **27 %** and 0 % → **14 %**, and those bodies
> survive at **85–96 %**, the best in the world. But **0 of 90 founder skimmers
> reach the horizon in any of the four cells**, and the grazer pays: entered
> 519 → 248 and 854 → 536, alive 145 → 51 and 383 → 223, with its margin rate
> falling −0.00073 → −0.00409 e/s and +0.00107 → +0.00002 e/s. Astra's rule
> refutes the roster change in **both** configurations, on the variety clause,
> and the reason is not an accident: **0.55 is the grazer's own `depth`**, so
> the treatment moves the skimmer into the grazer's height and the two now
> compete for the same leaf.

## Build and commits

| | |
| --- | --- |
| Branch | `worktree-agent-a131175f4bf758fab`, a worktree of `main` |
| Parent | `15e13c8` |
| Pre-registration commit | `6431156` — everything above the rule, written before a row existed |
| Test-authoring commit | `d08884e` — the sixteen definition tests against a stub; 12 of 16 red |
| Implementation commit | `080bee6` — the override, the run loop, the driver, the rule |
| Repair commit | `c38b5a6` — the founder brood counter, counted once; the tables |
| Build stamp in every row | **`c38b5a6`** (pinned through `CUBARIUM_SEARCH_BUILD`) |
| Rows | `runs/ecology-v1-depth-census/{runs.jsonl, summary.json, report.txt, analyse.py}`, 72 rows, **628 KiB** against the 30 MiB budget |

Files touched: the new `crates/cubarium-search/src/census.rs`, its module line in
`lib.rs`, one `use` line, one subcommand variant and one dispatch line in
`main.rs`, the new `crates/cubarium-search/tests/depth_census.rs`, and this note.
`cubarium-core` is not modified. `lifecycle.rs`, `calibrate.rs`, `depletion.rs`,
`evaluate.rs`, `step.rs` and `es/` — this round's other workers' files — are not
modified. No `WorldConfig` field was added and no `config_hash` moved.

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target CUBARIUM_SEARCH_BUILD=c38b5a6 \
  cargo build --release -p cubarium-search
./target/release/cubarium-search census --seeds 6 --ticks 180000 --sample-every 600 \
  --introduce-tick 6000 --workers 8 --wall-seconds 600 \
  --retained runs/ecology-v1-calibration/screen/evals.jsonl,runs/ecology-v1-ladder/ladder/evals.jsonl,runs/ecology-v1-plant-budget/present-off/evals.jsonl \
  --out runs/ecology-v1-depth-census
```

**The shared-`target/` hazard, recorded because it can silently produce a wrong
answer.** A build of `cubarium-search` failed here on two `apex_audit.rs` calls
into `cubarium-core::hunter::strike`, because another worktree had overwritten
the core rlib between the two trees. `touch crates/cubarium-core/src/lib.rs` and
a rebuild fixed it, and the row-producing binary was then copied out of the
shared `target/` before the campaign ran, so no concurrent build could swap it
underneath. Every number below comes from a binary that printed build stamp
`c38b5a6`.

### Tests

| Command | Result |
| --- | --- |
| `cargo test --release -p cubarium-search` | **237 passed**, 3 ignored, 0 failed (217 before) |
| `cargo test --release -p cubarium-core` | **517 passed**, 4 ignored, 0 failed — unchanged, which is what "core untouched" has to look like |

**20 tests are new**: the 16 definition tests in `tests/depth_census.rs` and 4
unit tests in `census.rs` (the bands are increasing and the last is open; a
depth profile merges without losing a probe; a censored founder lifetime counts
a survivor at the length of the run; one run of a seed is not that seed
agreeing).

The authoring order is on the record rather than asserted: commit `d08884e`
carries the sixteen tests against a module that returns nothing, and **12 of the
16 fail there**. Of the four that pass, two are real measurements of the shipped
world that need no implementation at all — the two depth levels *are* the roster
skimmer's and the roster grazer's own, and the ordinary founding *does* place
10/5/4/5 — and two pass trivially and are not counted as evidence: a stub that
always errs satisfies "an out-of-bounds depth is refused", and a default verdict
of `Partial` satisfies "survival without breeding is not confirmed".

The load-bearing one is
`a_control_run_reproduces_the_ordinary_harness_world`. This campaign needs its
own run loop, because the override has to be written between `World::new` and
the first `World::step` and `evaluate::run` has no seam there; a second
implementation of the loop would be worthless unless it *is* the first. The test
runs `evaluate_with` and `census::run_one` at the same protocol with the control
override and requires equal `final_state_hash`, `ticks_run`, prey births and
deaths, deaths by cause, and — field for field — the `Census`, the
`FounderBroods`, the `Crossings` and the ledger `Margins`. It passes at arm 0
and at arm 2 with an apex introduction inside the horizon.

## The reproduction check, before anything was interpreted

**All 60 checks pass; 0 mismatches.**

| retained rows | control rows checked | matched | not in that file |
| --- | --- | --- | --- |
| A's screen, `runs/ecology-v1-calibration/screen/evals.jsonl` | 36 | **36** | 0 |
| I's ladder at the shipped price, `runs/ecology-v1-ladder/ladder/evals.jsonl` | 12 | **12** | 24 (arms 1 and 2; the ladder is arm 0 only) |
| M's present arm, `runs/ecology-v1-plant-budget/present-off/evals.jsonl` | 12 | **12** | 24 (same) |

Every one of the 24 control rows carries the same `metrics.final_state_hash` as
A's screen row for its `(candidate, seed, arm)`, and the 12 arm-0 rows
additionally match I's and M's. Writing the roster skimmer's own `depth` back
onto the roster skimmer moves no bit of the world, through 180,000 ticks, with
the ledger on.

A second, independent reproduction is worth stating because it was not required:
**the control cells reproduce F's published variety census row for row.** F's
`baseline` control reads grazer 519 / 374 / 363 / 1,652 s / 145 / 28 %, glider
678 / 421 / 172 / 5,220 s / 257 / 38 %, burrower 2,032 / 1,746 / 1,712 /
1,146 s / 286 / 14 %, skimmer bin 1 271 / 259 / 252 / 926 s / 12 / 4 % and
skimmer bin 2 5 / 0 / 0 / — / 5 / 100 %. This campaign's control cell reads the
same numbers, and the `fast-leaf` control matches F's too (grazer 854 / 471 /
328 / 3,695 s / 383 / 45 % against F's 3,696 s, one second of rounding). F's
census and this one were written by different sessions against the same
definitions, and they agree.

## The census

Each row is a mean over the 18 runs (6 training seeds × 3 apex arms) of that
cell. "Kinds at end" is founder forms alive at the horizon, of 4.

| cand | `depth` | late pop | kinds at end | grazer | glider | burrower | **skimmer** | skimmer births | foliage × | worlds lost |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | 39.4 | 2.11 | 8.1 | 14.3 | 16.1 | **0.9** | 10.3 | 2.49 | 0 |
| baseline | **0.55** | 43.7 | **2.72** | **2.8** | 12.4 | 10.4 | **18.1** | **43.8** | 2.10 | 0 |
| fast-leaf | 0.10 | 62.0 | 3.00 | 21.3 | 27.9 | 12.7 | **0.0** | 8.1 | 2.14 | 0 |
| fast-leaf | **0.55** | 60.8 | **3.50** | **12.4** | 30.3 | 12.6 | **5.6** | 15.2 | 2.15 | 0 |

No world ended empty in any of the 72 runs, so no late window is censored and
every mean is over 18 of 18.

### The variety census, F's columns

Pooled over the 18 runs of each cell; `entered` = tick-0 bodies plus births;
`survival` = alive at the horizon over `entered`; mean lifetime is over deaths
only. Cells with fewer than five entrants are omitted for width and are in
`runs.jsonl`.

| cand | `depth` | rig | diet bin | guild | entered | deaths | of which starvation | mean lifetime | alive | survival |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | grazer | 0.65–1.00 | herbivore | 519 | 374 | 363 | 1,652 s | 145 | 28 % |
| baseline | 0.10 | glider | 0.65–1.00 | herbivore | 678 | 421 | 172 | 5,220 s | 257 | 38 % |
| baseline | 0.10 | burrower | 0.00–0.35 | detritivore | 2,032 | 1,746 | 1,712 | 1,146 s | 286 | 14 % |
| baseline | 0.10 | **skimmer** | 0.35–0.65 | generalist | 271 | 259 | 252 | 926 s | 12 | **4 %** |
| baseline | 0.10 | **skimmer** | 0.65–1.00 | generalist | 5 | 0 | 0 | — | 5 | 100 % |
| baseline | **0.55** | grazer | 0.65–1.00 | herbivore | **248** | 197 | 188 | **827 s** | **51** | 21 % |
| baseline | **0.55** | glider | 0.65–1.00 | herbivore | 609 | 387 | 167 | 5,033 s | 222 | 36 % |
| baseline | **0.55** | burrower | 0.00–0.35 | detritivore | 1,983 | 1,798 | 1,778 | 999 s | **185** | 9 % |
| baseline | **0.55** | **skimmer** | 0.35–0.65 | generalist | **644** | 468 | 442 | **2,150 s** | 176 | **27 %** |
| baseline | **0.55** | **skimmer** | 0.65–1.00 | herbivore | 55 | 8 | 8 | 3,165 s | 47 | **85 %** |
| baseline | **0.55** | **skimmer** | 0.65–1.00 | generalist | 180 | 78 | 74 | 3,236 s | 102 | **57 %** |
| fast-leaf | 0.10 | grazer | 0.65–1.00 | herbivore | 854 | 471 | 328 | 3,695 s | 383 | 45 % |
| fast-leaf | 0.10 | glider | 0.65–1.00 | herbivore | 1,072 | 573 | 328 | 5,024 s | 499 | 47 % |
| fast-leaf | 0.10 | burrower | 0.00–0.35 | detritivore | 1,763 | 1,535 | 1,481 | 1,169 s | 228 | 13 % |
| fast-leaf | 0.10 | **skimmer** | 0.35–0.65 | generalist | 235 | 235 | 233 | 816 s | **0** | **0 %** |
| fast-leaf | **0.55** | grazer | 0.65–1.00 | herbivore | **536** | 313 | 238 | 2,887 s | **223** | 42 % |
| fast-leaf | **0.55** | glider | 0.65–1.00 | herbivore | 1,130 | 585 | 336 | 5,062 s | 545 | 48 % |
| fast-leaf | **0.55** | burrower | 0.00–0.35 | detritivore | 1,976 | 1,750 | 1,717 | 1,040 s | 226 | 11 % |
| fast-leaf | **0.55** | **skimmer** | 0.35–0.65 | generalist | **311** | 256 | 186 | **3,579 s** | 55 | **18 %** |
| fast-leaf | **0.55** | **skimmer** | 0.65–1.00 | herbivore | 28 | 1 | 0 | 7,200 s | 27 | **96 %** |
| fast-leaf | **0.55** | **skimmer** | 0.65–1.00 | generalist | 24 | 5 | 1 | 6,313 s | 19 | **79 %** |

### The composition over time

Mean live bodies by form at the six window boundaries. The shape is the finding:
in both configurations the skimmer's whole trajectory is decided inside the
first window and then holds, and the grazer's collapse is decided there too.

| cand | `depth` | form | tick 0 | 36k | 72k | 108k | 144k | 180k |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | skimmer | 5.0 | 0.2 | 1.7 | 2.1 | 1.6 | 0.9 |
| baseline | **0.55** | skimmer | 5.0 | **5.2** | **16.0** | 17.1 | 17.5 | **18.1** |
| baseline | 0.10 | grazer | 10.0 | 3.7 | 6.4 | 6.9 | 6.9 | 8.1 |
| baseline | **0.55** | grazer | 10.0 | **0.8** | 1.8 | 1.8 | 2.1 | **2.8** |
| fast-leaf | 0.10 | skimmer | 5.0 | 0.8 | 0.6 | 0.3 | 0.1 | **0.0** |
| fast-leaf | **0.55** | skimmer | 5.0 | **8.8** | 8.4 | 8.6 | 7.7 | **5.6** |
| fast-leaf | 0.10 | grazer | 10.0 | 15.7 | 18.4 | 19.6 | 20.6 | 21.3 |
| fast-leaf | **0.55** | grazer | 10.0 | **7.7** | 9.1 | 10.2 | 11.0 | **12.4** |

### Where the bodies stood

Mean water depth `fields.w` under the body, over every probe of every run of the
cell. O's warning was that the binary wet fraction says almost nothing; it is
confirmed here, and at 180,000 ticks the wet fraction moves *the wrong way*
(79 % → 81 %) while the depth of the water under a skimmer falls four-fold and
the algae-band fraction falls three-fold.

| cand | `depth` | grazer | glider | burrower | **skimmer** | founder skimmers | skimmer wet % | skimmer algae-band % |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | 0.061 | 0.015 | 0.848 | **0.188** | **0.241** | 79 % | **22 %** |
| baseline | **0.55** | 0.047 | 0.009 | 0.853 | **0.047** | **0.017** | 81 % | **7 %** |
| fast-leaf | 0.10 | 0.091 | 0.008 | 0.870 | **0.253** | **0.250** | 68 % | **28 %** |
| fast-leaf | **0.55** | 0.101 | 0.008 | 0.862 | **0.077** | **0.055** | 81 % | **10 %** |

The burrower stands in 0.85 d of water in every cell and is unmoved by the
treatment, which is the negative control this table came with: a measure that
moved for everybody would be measuring the world, not the locus.

### Net margin per body, from E's ledger

`margin = e_food_in − e_owed`, per body, meaned over the bodies of each
`(form, diet bin)` group. The rate column is each body's own margin divided by
its own recorded seconds, then meaned.

| cand | `depth` | form | diet bin | bodies | margin (e) | **margin (e/s)** | served (m) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | grazer | 0.65–1.00 | 522 | +2.683 | **−0.000729** | 11.23 |
| baseline | **0.55** | grazer | 0.65–1.00 | 248 | +0.255 | **−0.004086** | 6.32 |
| baseline | 0.10 | skimmer | 0.35–0.65 | 271 | +0.606 | **−0.000356** | 7.75 |
| baseline | **0.55** | skimmer | 0.35–0.65 | 644 | +3.148 | **+0.000652** | 15.89 |
| baseline | **0.55** | skimmer | 0.65–1.00 | 235 | +4.346 | **+0.001831** | 15.80 |
| baseline | 0.10 | glider | 0.65–1.00 | 678 | +7.471 | +0.002000 | 26.20 |
| baseline | **0.55** | glider | 0.65–1.00 | 610 | +6.973 | +0.002024 | 24.61 |
| baseline | 0.10 | burrower | 0.00–0.35 | 2,043 | +2.252 | +0.001860 | 14.37 |
| baseline | **0.55** | burrower | 0.00–0.35 | 1,999 | +1.999 | +0.001917 | 12.04 |
| fast-leaf | 0.10 | grazer | 0.65–1.00 | 855 | +5.553 | **+0.001068** | 20.67 |
| fast-leaf | **0.55** | grazer | 0.65–1.00 | 541 | +4.287 | **+0.000024** | 17.07 |
| fast-leaf | 0.10 | skimmer | 0.35–0.65 | 235 | −0.053 | **−0.000580** | 5.43 |
| fast-leaf | **0.55** | skimmer | 0.35–0.65 | 311 | +3.663 | **+0.000150** | 22.12 |
| fast-leaf | **0.55** | skimmer | 0.65–1.00 | 52 | +5.454 | **+0.002413** | 17.42 |
| fast-leaf | 0.10 | glider | 0.65–1.00 | 1,076 | +6.796 | +0.002124 | 23.18 |
| fast-leaf | **0.55** | glider | 0.65–1.00 | 1,131 | +6.828 | +0.002117 | 23.06 |
| fast-leaf | 0.10 | burrower | 0.00–0.35 | 1,777 | +2.224 | +0.001993 | 15.24 |
| fast-leaf | **0.55** | burrower | 0.00–0.35 | 1,989 | +2.071 | +0.001956 | 13.83 |

**The skimmer's margin rate changes sign in both configurations** — −0.000356 →
+0.000652 and −0.000580 → +0.000150 e/s — which is O's founding result carried
into a reproducing world, measured on the ledger rather than on lifetimes. **The
grazer's falls in both**, by 5.6× in `baseline` and to a forty-fifth of itself
in `fast-leaf`. The glider and the burrower are unmoved: ±1 % and ±3 %. The
treatment is not a general tax on the world; it is a transfer between two
bodies that now want the same height.

## The founder skimmers themselves

Five per run, 90 per cell. This is where the campaign disagrees with the
sentence the brief expected it to confirm.

| cand | `depth` | alive at the horizon | died | mean age at death | starv/age/collapse/predation | broods |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | **0 of 90** | 90 | 580 s | 89/0/0/1 | 146 |
| baseline | **0.55** | **0 of 90** | 90 | **373 s** | 90/0/0/0 | 87 |
| fast-leaf | 0.10 | **0 of 90** | 90 | 609 s | 90/0/0/0 | 144 |
| fast-leaf | **0.55** | **0 of 90** | 90 | **1,352 s** | 78/**9**/0/3 | 106 |

**Not one founder skimmer reaches the horizon in any of the four cells.** The
treatment does not stop the founders dying; it makes their descendants viable.
And the effect on the founder's own life is **configuration-dependent and
opposite in sign**: in `fast-leaf` the founder lives 2.2× longer (609 → 1,352 s,
longer in 5 of 6 seeds, and 9 of the 90 reach the age cap where none did before),
while in `baseline` it lives **shorter** (580 → 373 s, shorter in **6 of 6**
seeds, every death a starvation).

Per-seed mean age at death, so the agreement is visible rather than pooled:

| cand | `depth` | 1001 | 1002 | 1003 | 1004 | 1005 | 1006 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | 675 | 542 | 583 | 561 | 574 | 544 |
| baseline | **0.55** | 382 | 343 | 425 | 407 | 336 | 345 |
| fast-leaf | 0.10 | 715 | 635 | 660 | 481 | 504 | 658 |
| fast-leaf | **0.55** | 600 | **1,309** | **2,895** | **1,190** | 377 | **1,739** |

**A reading, named as a reading and not identified.** O measured that a
`depth = 0.55` body covers 3.5× the ground (426 distinct cells against 122) and
pays 4.4× the bill (20.51 e against 4.67 e) for it, and that the only income
that pays is foliage. In `fast-leaf` — `plant.foliage_rate` 0.006 against
0.002 — the foliage is there from the first minute and the founder's extra bill
buys it. In `baseline` it is not, and the same extra bill is spent finding
nothing: the founder starves *sooner* out of the pool than in it, while its
descendants, born later into a world whose grazers have already crashed and
whose foliage has reached 2.1× its opening, are fed. This campaign **cannot
separate founders from descendants inside the ledger's bins** — E's accumulator
reports by `(form, diet bin)`, not by generation — so the mechanism is a
plausible reading consistent with O's measurements, not a measurement made here.
What is measured is the founder ages above and their 6-of-6 and 5-of-6 seed
agreement.

## The diet drift: O's prediction, confirmed

O predicted that away from the wet floor the foliage-end diet becomes the better
one. The census sees exactly that, in the mutants the world produced on its own.

| cand | `depth` | form-3 entrants in bin 1 | in bin 2 | **bin-2 share** | alive from bin 1 | alive from bin 2 |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | 271 | 5 | **2 %** | 12 | 5 |
| baseline | **0.55** | 644 | **235** | **27 %** | 176 | **149** |
| fast-leaf | 0.10 | 235 | 0 | **0 %** | 0 | 0 |
| fast-leaf | **0.55** | 311 | **52** | **14 %** | 55 | **46** |

The foliage-bin skimmers are the best-surviving bodies in the world wherever
they appear: 85 % and 57 % in `baseline`, 96 % and 79 % in `fast-leaf`, against
21–48 % for every other rig. F measured the same association at
`fast-leaf` × 0.0018 and named the confound; the same confound applies here and
is not cancelled — every bin-2 form-3 body is a **descendant**, so later birth,
right-censoring, selection into the mutant lineage and possible mutation at
other loci are all mixed into that 96 %. What is new is that the *entry rate*
into the bin moves with the treatment, 2 % → 27 % and 0 % → 14 %, and an entry
rate is set at conception rather than by survival.

**The pre-registered clause D nevertheless reads "no", and that is a defect in
the clause rather than in the result.** D required the pooled share to rise
*and* five of six seeds to agree run by run. The pooled share rises 13× and by
14 points; the per-seed agreement is 2 of 6 and 1 of 6, because the mutant
lineages are concentrated in a few worlds. A rate measured over a handful of
lineage founding events is not a per-seed quantity, and asking it to agree seed
by seed was my error. D is reported as it was written, and the evidence for the
drift is the entry table above.

## The verdict, by the pre-registered rule

**baseline** — 18 runs over 6 seeds; a clause needs 12 runs and 5 seeds.

| clause | holds | runs | seeds | detail |
| --- | --- | --- | --- | --- |
| **L** the lineage establishes | **yes** | 16/18 | 5/6 | alive and breeding at the horizon in 16 of 18; the control managed 2 of 18 |
| **M** a new monoculture | no | 4/18 | 1/6 | kinds at the horizon 2.72 against 2.11; top-form share 0.59 against 0.65 |
| **V** another kind lost ground | **yes** | — | — | grazer 2.8 against 8.1 (0.35×, threshold 0.60×) |
| **F** the founding rescue disappeared | **yes** | 2/18 | 1/6 | founder mean lifetime 373 s against 580 s |
| *D* the diet drifts | no | 6/18 | 2/6 | 27 % of entrants in the foliage bin against 2 % |

**REFUTED.**

**fast-leaf** — 18 runs over 6 seeds; a clause needs 12 runs and 5 seeds.

| clause | holds | runs | seeds | detail |
| --- | --- | --- | --- | --- |
| **L** the lineage establishes | no | 13/18 | **4/6** | alive and breeding in 13 of 18 (enough) but only 4 seeds (5 needed); the control managed **0 of 18** |
| **M** a new monoculture | no | 0/18 | 0/6 | kinds at the horizon 3.50 against 3.00; top-form share 0.50 against 0.48 |
| **V** another kind lost ground | **yes** | — | — | grazer 12.4 against 21.3 (**0.58×**, threshold 0.60×) |
| **F** the founding rescue disappeared | no | 5/18 | 2/6 | founder mean lifetime 1,352 s against 609 s |
| *D* the diet drifts | no | 3/18 | 1/6 | 14 % of entrants in the foliage bin against 0 % |

**REFUTED.**

### What the verdict rests on, and how much of it is a threshold

Both refutations are carried by **V**, and V is a threshold, so it is worth
saying exactly how much daylight there is.

- **`baseline` V is decisive on its face and thin underneath.** 2.8 against 8.1
  is 0.35×, nowhere near the 0.60× line. But the `baseline` grazer reaches the
  horizon in only **2 of 6 control seeds** (1001 with 65 bodies and 1004 with 81;
  the other four end with none), so the whole contrast is those two worlds, and
  the treatment loses one of them: seed 1004 goes 81 → 0 and seed 1001 goes
  65 → 51. **A 2-of-6 base is not per-seed agreement**, and the honest statement
  for `baseline` is that the one world which lost its grazers is one world.
- **`fast-leaf` V is marginal on its face and solid underneath.** 12.4 against
  21.3 is 0.58× against a 0.60× threshold — a verdict that turns on 0.4 of a
  body, and I say so rather than round it away. But the per-seed table needs no
  threshold: the grazer's horizon population falls in **6 of 6 seeds**
  (81 → 58, 62 → 59, 55 → **0**, 70 → 22, 49 → 41, 67 → 43), and in seed 1003 it
  is gone from all three arms where the control had it in all three. That is the
  clause's intent met without its arithmetic.

Taken together: the grazer cost is real, it is evidenced by `fast-leaf`'s 6-of-6
seed agreement rather than by either mean crossing a line, and the refutation
stands.

### A defect in my own pre-registration, disclosed

**Clause F is measuring the wrong thing, and it changed the `baseline` verdict's
reasons though not its outcome.** F was written as "the founding rescue
disappears under reproduction" and operationalised as the founder skimmers' mean
lifetime failing to rise by 10 %. In `baseline` it holds — 373 s against 580 s —
in a cell where the rig goes from 0.9 bodies at the horizon to 18.1, the lineage
establishes in 16 of 18 runs, and the margin rate changes sign. The clause is
reporting "the founders die sooner", which is true and is the most interesting
single number in this note, and calling it "the rescue disappeared", which is
false. The mistake is that O's quantity was a **sterile clone's** lifetime and
mine is a **breeding founder's**, and those are not the same measurement: a
founder that funds broods spends on them. The clause is reported exactly as it
was written and is **not** repaired after the fact. Had it been written on the
rig's horizon population instead, `baseline` would still be REFUTED through V.

(Clause D's per-seed requirement is the second defect, disclosed above.)

## What Wrysk would be approving

Not "skimmers that stop dying on the rim". Every founder skimmer dies in every
cell of this campaign, at both depths, in both configurations — 360 of 360.

What one genome value in the founder roster — `skimmer.depth`, 0.10 → 0.55,
`crates/cubarium-core/src/config.rs:519` — actually buys is:

- **a fourth lineage that persists.** On the cube, a world that currently ends
  its second hour with no skimmers at all (`fast-leaf`, 18 of 18) would end it
  with a small school of them in 13 of 18 worlds, and the `baseline` world would
  carry 18 where it now carries 1. Founder forms alive at the horizon rise
  2.11 → 2.72 and 3.00 → 3.50.
- **paid for out of the grazer.** The grazer's horizon population falls to 0.35×
  and 0.58×, its margin rate falls 5.6× and to a forty-fifth, and one
  `fast-leaf` world loses its grazers entirely. This is a **trade of one kind
  for another**, not the addition of a kind — which is precisely what Astra's
  refutation clause was written to catch.
- **and it is not a surprise.** 0.55 was chosen, by O and by this brief, because
  it is *the grazer's own value*. The treatment therefore moves the skimmer to
  the grazer's preferred height, where the only income that pays in ecology v1
  is the leaf the grazer is already eating. "Off the wet floor" and "onto the
  grazer's height" are the same move in this design, and nothing here separates
  them.

The recommendation this supports is **do not take the roster change as tested**,
and run the one experiment that would separate the two halves of it (below)
before deciding. Nothing here is bundled with a wet-floor producer, per the
brief.

## Accounting

- **Conservation.** Worst `|material residual|` over 72 rows **1.64e-11**, worst
  `|energy residual|` **1.26e-10** — both inside the contract's 1e-9, from E's
  per-body ledger over every closed and live record.
- **Invariants.** `World::check_invariants` ran at every 600-tick sample of
  every run; no violation, or the run would have been an error rather than a row.
- **Ledger completeness.** **0** dropped closed records in 72 runs.
- **Refusals and failures: none.** 72 of 72 trials completed, 0 skipped.
- **Censoring: none.** No world ended empty, so no cell's mean is over fewer
  than 18 runs.
- **Predation.** 1–2 prey deaths per run by predation across every cell, and
  3 of the 360 founder skimmers died to it. Nothing here turns on the apex; the
  three arms are carried because A's rows are per arm and that is what makes the
  36-row hash reproduction possible.

## What this does not establish

- **One locus, two levels, and the high level is the grazer's.** The design
  cannot separate "does not seek the rim" from "seeks the grazer's height".
  That confound is total and is the single largest limit on this result.
- **`depth = 0.55` is not shown to be the best value**, or even a good one. Two
  levels measure a direction.
- **Six training seeds, two configurations, three matched apex arms.** No
  held-out seed was touched, nothing was tuned, no configuration is proposed and
  no §11 default is changed.
- **Generation is not separable inside the ledger.** E's accumulator bins by
  `(form, diet bin)`, so every margin above mixes founders and descendants, and
  the founder-lifetime reading is inferred from O's measurements rather than
  measured here.
- **The foliage-bin association is still an association.** Every bin-2 form-3
  body is a descendant; the 96 % survival is not a randomised comparison.
- **150 simulated minutes.** `fast-leaf`'s treatment skimmer population is
  *falling* through the last two windows (8.6 → 7.7 → 5.6) while `baseline`'s is
  still rising (17.1 → 17.5 → 18.1). Whether the `fast-leaf` lineage persists
  past the horizon is not measured, and the two configurations disagree about
  the trend.
- **No equation, no ordering, no schema and no core file is changed**, and the
  running cube, `state/`, port 7393 and the shim were not touched.

## The next task this implies, named and not launched

**A depth ladder between the rim and the grazer, on this same census.** The
confound that decides this note is that 0.55 is the grazer's own value, so the
treatment necessarily moves the skimmer into the grazer's niche. The experiment
that separates the two halves is `skimmer.depth` ∈ {0.10, 0.20, 0.30, 0.40,
0.55} — and, because the glider sits at 1.00 and is *unmoved* by this treatment
in every table above, a sixth level at 0.75 which is nobody's — on the same two
configurations, the same six seeds, arm 0 only, with this module's census
unchanged. The question is sharp: **is there a depth that rescues the skimmer's
lineage without taking the grazer's horizon population?** If there is, the
roster change is a one-line change with a defensible value; if the skimmer's
margin only turns positive where the grazer's turns negative, then ecology v1
has three viable heights and four kinds, and that is a world question rather
than a genome one. It costs 60 runs, about 3.5 wall minutes at this campaign's
measured 51,699 ticks/s.

Two smaller measurements would make either answer readable and are cheap:
**split E's margin bins by generation** (founder or descendant) so the
founder-lifetime reversal between `baseline` and `fast-leaf` can be measured
instead of read; and **record each form-3 body's own foliage and litter served
by channel**, which the ledger already holds per body, so "the skimmer now eats
the grazer's leaf" is counted rather than inferred from the grazer's falling
margin.

## Routine decisions made here, and their visible effect

| decision | effect |
| --- | --- |
| the override finds bodies by **genome equality** against `Roster::of`, not by `phenotype.form` | a form-3 body that is not the roster skimmer could never be caught by accident; every row also records the form-3 founder count, and the two are 5 and 5 in all 72 rows |
| a `depth` outside the genome's `0..=1` is **refused, not clamped** | the declared bounds stay a declaration; the refusal is tested on −0.1, 1.5 and NaN and writes nothing |
| the census carries its **own run loop** rather than a seam in `evaluate.rs` | no file another worker owns this round is touched, and the loop is pinned to `evaluate_with`'s hash, census, broods, crossings and margins by a test rather than by a claim |
| the subcommand's arguments are declared in `census.rs` with `clap::Args` | `main.rs` gains one `use`, one variant and one dispatch line, which is the footprint the brief allows |
| six **window snapshots** of the composition rather than 301 samples | a composition series is a shape; 301 samples per row would have cost more storage than the whole campaign used |
| the **water-depth histogram and mean** reported per form, with the burrower as an unplanned negative control | O's finding that the binary wet fraction is uninformative is confirmed at this horizon — it moves 79 % → 81 %, the wrong way — and the burrower's 0.85 d is identical in all four cells |
| founder lifetime counts a **survivor at the length of the run** and the uncensored **mean age at death** is reported beside it | in this campaign they are the same number in every cell, because 0 of 360 founder skimmers survived |
| thresholds **scale with the cell** (two thirds of runs, five sixths of seeds) | 12 of 18 and 5 of 6 here, and the same rule reads a smaller ladder without being rewritten |
| clause D is **reported as written** although its per-seed requirement is wrong for a rate | the defect is on the record beside the number it misreads, rather than repaired into agreement with the result |
| the three apex arms kept rather than dropped to arm 0 | A's rows are per arm, which is what makes the 36-row state-hash reproduction check possible at all |
| `analyse.py` shipped **beside the rows** | every table in this note that the harness does not print is reproducible from `runs.jsonl` by one command, so the note cannot disagree with the rows |

## Compute and storage actually used

| | |
| --- | --- |
| Trials | 72, 0 skipped, 0 invalid, 0 failed |
| Ticks | 12.96 M; every run reached the full 180,000 |
| Wall | **250.7 s = 4.2 minutes** against the brief's 6-minute cap |
| Throughput | 51,699 ticks/s on 8 workers; slowest single run 30.4 s |
| Workers | 8, the cap |
| Storage | `runs/ecology-v1-depth-census/` **628 KiB** against the 30 MiB budget |
| Test suites | `cubarium-search` 237 passed / 3 ignored, ~13 s; `cubarium-core` 517 passed / 4 ignored |
| Background processes | none; nothing is left running |

Model usage: the harness exposes none and this session's is not visible to it.
The measured resource is the table above.

## Stop

The note, the rows and the commits are the deliverable. Nothing here changed an
equation, a §11 value, an ordering, the snapshot schema, a `WorldConfig` field,
the trainer, the display, the running process, `state/` or port 7393; no
`cubarium-core` file and no file owned by another worker this round was
modified; and no held-out seed was touched.
